//! Off-screen rendering support: CEF paints into a shared buffer that the host
//! page displays in a canvas, and pointer/keyboard/IME events are forwarded
//! back to CEF. Only available with the `offscreen` feature.

use cef::rc::Rc as _;
use cef::{
    wrap_render_handler, Browser, BrowserHost, CefString, CompositionUnderline, ImplBrowser,
    ImplBrowserHost, ImplRenderHandler, KeyEvent, KeyEventType, MouseButtonType, MouseEvent,
    PaintElementType, Range, Rect, RenderHandler, ScreenInfo, WrapRenderHandler,
};
use serde_json::Value;
use tauri::{Runtime, Webview, WebviewBuilder};
use tauri_runtime::dynamic::DynWebviewAttributes;
use tauri_runtime_cef::{CefWebviewAttributes, Offscreen, OffscreenView};

use crate::frames::FrameSlot;

/// CEF modifier bits, see `cef_event_flags_t`.
const SHIFT: u32 = 1 << 1;
const CONTROL: u32 = 1 << 2;
const ALT: u32 = 1 << 3;
const LEFT_MOUSE_BUTTON: u32 = 1 << 4;
const MIDDLE_MOUSE_BUTTON: u32 = 1 << 5;
const RIGHT_MOUSE_BUTTON: u32 = 1 << 6;
const COMMAND: u32 = 1 << 7;
const NUM_LOCK: u32 = 1 << 8;
const IS_KEYPAD: u32 = 1 << 9;
const IS_LEFT: u32 = 1 << 10;
const IS_RIGHT: u32 = 1 << 11;
const IS_REPEAT: u32 = 1 << 13;

/// Attaches an off-screen render handler to a webview being built. Returns
/// `false` when the active runtime is not CEF, in which case the webview is a
/// native child and needs no host rendering.
pub(crate) fn attach<R: Runtime>(builder: &mut WebviewBuilder<R>, frames: FrameSlot) -> bool {
    let attributes: &mut dyn std::any::Any = builder.runtime_specific_attributes_mut();
    let mut offscreen = Offscreen::new({
        let frames = frames.clone();
        move |view| Renderer::new(view, frames.clone())
    });
    let cursors = frames.clone();
    offscreen.cursor_handler = Some(std::sync::Arc::new(
        move |cursor: cef::CursorType, _info: Option<&cef::CursorInfo>| {
            cursors.cursor(*cursor.as_ref() as i32);
        },
    ));
    if let Some(attributes) = attributes.downcast_mut::<CefWebviewAttributes>() {
        attributes.offscreen = Some(offscreen);
        return true;
    }
    // Tauri wraps the runtime attributes when the app uses the dynamic runtime.
    let Some(attributes) = attributes.downcast_mut::<DynWebviewAttributes>() else {
        return false;
    };
    let Some(attributes) = attributes.get_or_default::<CefWebviewAttributes>() else {
        return false;
    };
    attributes.offscreen = Some(offscreen);
    true
}

/// Raises the CEF paint rate; CEF defaults to 30 frames per second.
pub(crate) fn set_frame_rate<R: Runtime>(webview: &Webview<R>) {
    let _ = webview.with_webview(|platform| {
        let Some(webview) = platform.downcast_ref::<tauri_runtime_cef::Webview>() else {
            return;
        };
        let Some(host) = webview.browser().host() else {
            return;
        };
        host.set_windowless_frame_rate(60);
    });
}

wrap_render_handler! {
    struct Renderer {
        view: OffscreenView,
        frames: FrameSlot,
    }

    impl RenderHandler {
        fn view_rect(&self, _browser: Option<&mut Browser>, rect: Option<&mut Rect>) {
            let Some(rect) = rect else { return };
            let (width, height) = self.viewport();
            rect.x = 0;
            rect.y = 0;
            rect.width = width;
            rect.height = height;
        }

        fn screen_info(&self, _browser: Option<&mut Browser>, info: Option<&mut ScreenInfo>) -> i32 {
            let Some(info) = info else { return 0 };
            let (width, height) = self.viewport();
            *info = ScreenInfo {
                device_scale_factor: self.scale(),
                depth: 32,
                depth_per_component: 8,
                rect: Rect { x: 0, y: 0, width, height },
                available_rect: Rect { x: 0, y: 0, width, height },
                ..Default::default()
            };
            1
        }

        fn on_paint(
            &self,
            _browser: Option<&mut Browser>,
            kind: PaintElementType,
            dirty_rects: Option<&[Rect]>,
            buffer: *const u8,
            width: i32,
            height: i32,
        ) {
            if kind != PaintElementType::VIEW || buffer.is_null() || width <= 0 || height <= 0 {
                return;
            }
            let Some(len) = (width as usize)
                .checked_mul(height as usize)
                .and_then(|pixels| pixels.checked_mul(4))
            else {
                return;
            };
            // SAFETY: CEF guarantees a `width * height * 4` BGRA buffer for the
            // duration of the callback.
            let bgra = unsafe { std::slice::from_raw_parts(buffer, len) };
            let rects: Option<Vec<[i32; 4]>> = dirty_rects.map(|rects| {
                rects
                    .iter()
                    .map(|rect| [rect.x, rect.y, rect.width, rect.height])
                    .collect()
            });
            self.frames.paint(
                width as usize,
                height as usize,
                self.scale(),
                rects.as_deref(),
                bgra,
            );
            self.view.request_redraw();
        }
    }
}

impl Renderer {
    fn scale(&self) -> f32 {
        self.view
            .window()
            .map(|window| window.scale_factor() as f32)
            .unwrap_or(1.0)
    }

    /// View size in logical pixels, as CEF expects for windowless rendering.
    fn viewport(&self) -> (i32, i32) {
        let logical = match self.view.bounds().size {
            tauri::Size::Logical(size) => size,
            tauri::Size::Physical(size) => size.to_logical(self.scale() as f64),
        };
        (
            logical.width.round().max(1.0) as i32,
            logical.height.round().max(1.0) as i32,
        )
    }
}

/// Forwards a `sendInputEvent` payload to CEF's browser host.
pub(crate) fn send_input<R: Runtime>(webview: &Webview<R>, event: Value) {
    let _ = webview.with_webview(move |platform| {
        let Some(webview) = platform.downcast_ref::<tauri_runtime_cef::Webview>() else {
            return;
        };
        let Some(host) = webview.browser().host() else {
            return;
        };
        forward(&host, &event);
    });
}

fn forward(host: &BrowserHost, event: &Value) {
    let modifiers = modifiers(event);
    let x = integer(event, "x");
    let y = integer(event, "y");
    let mouse = || MouseEvent { x, y, modifiers };
    match event
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or_default()
    {
        "focus" => host.set_focus(1),
        "blur" => host.set_focus(0),
        "mouseDown" | "mouseUp" => host.send_mouse_click_event(
            Some(&mouse()),
            match event.get("button").and_then(Value::as_str) {
                Some("right") => MouseButtonType::RIGHT,
                Some("middle") => MouseButtonType::MIDDLE,
                _ => MouseButtonType::LEFT,
            },
            i32::from(event.get("type").and_then(Value::as_str) == Some("mouseUp")),
            integer(event, "clickCount").max(1),
        ),
        "mouseMove" => host.send_mouse_move_event(Some(&mouse()), 0),
        "mouseLeave" => host.send_mouse_move_event(Some(&mouse()), 1),
        "mouseWheel" => host.send_mouse_wheel_event(
            Some(&mouse()),
            // Chromium reports wheel deltas with the opposite sign of the DOM.
            -integer(event, "deltaX"),
            -integer(event, "deltaY"),
        ),
        "keyDown" | "keyUp" | "char" => {
            let kind = match event.get("type").and_then(Value::as_str) {
                Some("keyUp") => KeyEventType::KEYUP,
                Some("char") => KeyEventType::CHAR,
                _ => KeyEventType::KEYDOWN,
            };
            let character = event
                .get("character")
                .and_then(Value::as_str)
                .and_then(|text| text.encode_utf16().next())
                .unwrap_or(0);
            let key = KeyEvent {
                type_: kind,
                modifiers,
                windows_key_code: integer(event, "keyCode"),
                native_key_code: integer(event, "keyCode"),
                character,
                unmodified_character: character,
                ..Default::default()
            };
            host.send_key_event(Some(&key));
        }
        "imeSetComposition" => {
            let value = event
                .get("text")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let text = CefString::from(value);
            let from = integer(event, "selectionStart").max(0) as u32;
            let to = integer(event, "selectionEnd").max(0) as u32;
            let underline = CompositionUnderline {
                range: Range {
                    from: 0,
                    to: value.encode_utf16().count() as u32,
                },
                color: 0xFF00_0000,
                ..Default::default()
            };
            host.ime_set_composition(
                Some(&text),
                Some(&[underline]),
                None,
                Some(&Range { from, to }),
            );
        }
        "imeCommitText" => {
            let text = CefString::from(
                event
                    .get("text")
                    .and_then(Value::as_str)
                    .unwrap_or_default(),
            );
            host.ime_commit_text(Some(&text), None, 0);
        }
        "imeFinishComposition" => host.ime_finish_composing_text(1),
        "imeCancelComposition" => host.ime_cancel_composition(),
        _ => {}
    }
}

fn integer(event: &Value, key: &str) -> i32 {
    event
        .get(key)
        .and_then(Value::as_i64)
        .unwrap_or(0)
        .clamp(i32::MIN as i64, i32::MAX as i64) as i32
}

fn modifiers(event: &Value) -> u32 {
    let flags = event.get("modifiers").and_then(Value::as_array);
    let Some(flags) = flags else { return 0 };
    flags.iter().fold(0, |bits, flag| {
        bits | match flag.as_str().unwrap_or_default() {
            "shift" => SHIFT,
            "control" => CONTROL,
            "alt" => ALT,
            "meta" => COMMAND,
            "capsLock" => 1,
            "numLock" => NUM_LOCK,
            "isKeypad" => IS_KEYPAD,
            "isLeft" => IS_LEFT,
            "isRight" => IS_RIGHT,
            "isAutoRepeat" => IS_REPEAT,
            "leftButtonDown" => LEFT_MOUSE_BUTTON,
            "middleButtonDown" => MIDDLE_MOUSE_BUTTON,
            "rightButtonDown" => RIGHT_MOUSE_BUTTON,
            _ => 0,
        }
    })
}
