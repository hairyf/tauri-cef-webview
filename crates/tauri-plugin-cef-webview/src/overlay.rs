//! Carves host-page overlays out of windowed child webviews.
//!
//! A windowed child webview is a native HWND: nothing painted by the host page
//! can cover it. The page reports the rectangles of its `data-cef-overlay`
//! elements (physical pixels, viewport origin) and this module subtracts them
//! from the native window region of every child webview with `SetWindowRgn`.
//! Inside those rectangles the host page is both visible and clickable.
//!
//! ponytail: Windows only, rectangle union only (no rounded corners, no
//! translucent overlay over webview pixels, one frame of lag between DOM and
//! region). Every other platform keeps the native behavior.

#[cfg(target_os = "windows")]
use std::collections::HashMap;
#[cfg(target_os = "windows")]
use std::{
    sync::{mpsc, Mutex},
    time::Duration,
};

use tauri::{webview::Webview, Runtime, State};

use crate::registry::Registry;

/// Overlay rectangles of the last call, keyed by HWND, to skip redundant work.
#[cfg(target_os = "windows")]
static APPLIED: Mutex<Option<HashMap<isize, u64>>> = Mutex::new(None);

#[tauri::command]
pub fn set_overlay_clip<R: Runtime>(
    webview: Webview<R>,
    registry: State<'_, Registry<R>>,
    views: Vec<(String, [f64; 4])>,
    rects: Vec<[f64; 4]>,
) -> Result<(), String> {
    if rects.iter().any(|rect| {
        !rect.iter().all(|value| value.is_finite()) || rect[2] < rect[0] || rect[3] < rect[1]
    }) {
        return Err("cef-webview: overlay rects must be [left, top, right, bottom]".into());
    }
    if views.iter().any(|(_, box_)| {
        !box_.iter().all(|value| value.is_finite()) || box_[2] <= 0.0 || box_[3] <= 0.0
    }) {
        return Err("cef-webview: webview boxes must be [x, y, width, height]".into());
    }

    #[cfg(target_os = "windows")]
    {
        let host = native_handle(&webview);
        // The element box in CSS px tells us the CSS → device scale of each
        // guest, so overlay rectangles land on the pixels the guest paints.
        let guests: Vec<(isize, [f64; 4])> = views
            .iter()
            .filter_map(|(label, box_)| {
                let child = registry.get(label)?;
                Some((native_handle(child.inner())?, *box_))
            })
            .collect();
        crate::overlay::clip::apply(&rects, host, &guests);
    }

    #[cfg(not(target_os = "windows"))]
    let _ = (&webview, &registry, &views);

    Ok(())
}

/// Native window handle behind a windowed webview, if the runtime exposes one.
#[cfg(target_os = "windows")]
fn native_handle<R: Runtime>(webview: &Webview<R>) -> Option<isize> {
    let (sender, receiver) = mpsc::channel();
    webview
        .with_webview(move |platform| {
            let _ = sender.send(window_handle(&platform));
        })
        .ok()?;
    receiver
        .recv_timeout(Duration::from_millis(500))
        .ok()
        .flatten()
}

#[cfg(all(target_os = "windows", feature = "layering"))]
fn window_handle<R: Runtime>(platform: &tauri::webview::PlatformWebview<R>) -> Option<isize> {
    use cef::{ImplBrowser, ImplBrowserHost};

    let webview = platform.downcast_ref::<tauri_runtime_cef::Webview>()?;
    let host = webview.browser().host()?;
    Some(host.window_handle().0 as isize)
}

#[cfg(all(target_os = "windows", not(feature = "layering")))]
fn window_handle<R: Runtime>(_platform: &tauri::webview::PlatformWebview<R>) -> Option<isize> {
    None
}

#[cfg(target_os = "windows")]
mod clip {
    use std::collections::HashMap;

    use windows_sys::Win32::{
        Foundation::{HWND, RECT},
        Graphics::Gdi::{
            CombineRgn, CreateRectRgn, DeleteObject, InvalidateRect, SetWindowRgn, RGN_DIFF,
        },
        UI::WindowsAndMessaging::{
            GetAncestor, GetParent, GetWindowLongPtrW, GetWindowRect, GWL_STYLE,
            SetWindowLongPtrW, GA_ROOT, WS_CLIPSIBLINGS,
        },
    };

    use super::APPLIED;

    /// `rect` in screen pixels, converted to `window` coordinates and clamped
    /// to it. Empty when the rectangle misses the window entirely.
    fn local(window: &RECT, rect: &[f64; 4]) -> Option<RECT> {
        let left = (rect[0] - window.left as f64).round() as i32;
        let top = (rect[1] - window.top as f64).round() as i32;
        let right = (rect[2] - window.left as f64).round() as i32;
        let bottom = (rect[3] - window.top as f64).round() as i32;
        let rect = RECT {
            left: left.max(0),
            top: top.max(0),
            right: right.min(window.right - window.left),
            bottom: bottom.min(window.bottom - window.top),
        };
        if rect.right <= rect.left || rect.bottom <= rect.top {
            None
        } else {
            Some(rect)
        }
    }

    fn without_sibling_clipping(window: HWND) {
        let style = unsafe { GetWindowLongPtrW(window, GWL_STYLE) };
        if style & WS_CLIPSIBLINGS as isize == 0 {
            unsafe { SetWindowLongPtrW(window, GWL_STYLE, style | WS_CLIPSIBLINGS as isize) };
        }
    }

    /// The child webview and every wrapper window between it and the hosting
    /// window. Clipping the webview alone is not enough: hit testing of a
    /// clipped point falls through to the parent, which still owns the full
    /// rectangle and swallows the click.
    fn targets(guest: HWND) -> Vec<(HWND, RECT)> {
        let root = unsafe { GetAncestor(guest, GA_ROOT) };
        let mut targets = Vec::new();
        let mut current = guest;
        while !current.is_null() && current != root {
            without_sibling_clipping(current);
            let mut rect = RECT::default();
            if unsafe { GetWindowRect(current, &mut rect) } != 0 {
                targets.push((current, rect));
            }
            current = unsafe { GetParent(current) };
        }
        targets
    }

    fn signature(window: &RECT, clipped: &[RECT]) -> u64 {
        let mut hash = 0xcbf2_9ce4_8422_2325u64;
        let mut mix = |value: i64| {
            hash ^= value as u64;
            hash = hash.wrapping_mul(0x100_0000_01b3);
        };
        mix((window.right - window.left) as i64);
        mix((window.bottom - window.top) as i64);
        for rect in clipped {
            mix(rect.left as i64);
            mix(rect.top as i64);
            mix(rect.right as i64);
            mix(rect.bottom as i64);
        }
        hash
    }

    /// Applies the overlay rectangles to every child webview of the window.
    pub(super) fn apply(rects: &[[f64; 4]], host: Option<isize>, guests: &[(isize, [f64; 4])]) {
        if host.is_none() {
            return;
        }

        if let Some(host) = host {
            without_sibling_clipping(host as HWND);
        }

        let mut cache = APPLIED.lock().unwrap_or_else(|error| error.into_inner());
        let cache = cache.get_or_insert_with(HashMap::new);

        for (guest, css) in guests {
            let guest = *guest as HWND;
            let mut box_ = RECT::default();
            if unsafe { GetWindowRect(guest, &mut box_) } == 0 {
                continue;
            }
            let scale_x = (box_.right - box_.left) as f64 / css[2];
            let scale_y = (box_.bottom - box_.top) as f64 / css[3];
            let screen: Vec<[f64; 4]> = rects
                .iter()
                .map(|rect| {
                    [
                        box_.left as f64 + (rect[0] - css[0]) * scale_x,
                        box_.top as f64 + (rect[1] - css[1]) * scale_y,
                        box_.left as f64 + (rect[2] - css[0]) * scale_x,
                        box_.top as f64 + (rect[3] - css[1]) * scale_y,
                    ]
                })
                .collect();

            for (target, window) in targets(guest) {
                let clipped: Vec<RECT> = screen
                    .iter()
                    .filter_map(|rect| local(&window, rect))
                    .collect();
                let key = signature(&window, &clipped);
                if cache.get(&(target as isize)) == Some(&key) {
                    continue;
                }
                cache.insert(target as isize, key);

                let width = window.right - window.left;
                let height = window.bottom - window.top;
                let region = unsafe { CreateRectRgn(0, 0, width, height) };
                if region.is_null() {
                    continue;
                }
                for rect in &clipped {
                    let hole = unsafe { CreateRectRgn(rect.left, rect.top, rect.right, rect.bottom) };
                    if hole.is_null() {
                        continue;
                    }
                    unsafe { CombineRgn(region, region, hole, RGN_DIFF) };
                    unsafe { DeleteObject(hole) };
                }
                // `SetWindowRgn` takes ownership of the region on success.
                if unsafe { SetWindowRgn(target, region, 0) } == 0 {
                    unsafe { DeleteObject(region) };
                }
                unsafe { InvalidateRect(target, std::ptr::null(), 0) };
            }
        }
    }
}
