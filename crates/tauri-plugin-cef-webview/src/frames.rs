//! Latest off-screen frame shared between the CEF render handler and the page.
//!
//! CEF paints BGRA buffers from its UI thread. The host page polls
//! [`FrameSlot::packet`] and only receives the rectangles painted since the
//! previous poll, so an idle page costs an empty response.

use std::sync::{Arc, Mutex};

/// Header words: sequence, width, height, scale (thousandths), rectangle count,
/// cursor type.
pub(crate) const HEADER_WORDS: usize = 6;
/// Rectangle words: x, y, width, height.
pub(crate) const RECT_WORDS: usize = 4;
/// Dirty rectangles are merged into a full frame after this many.
const MAX_DIRTY_RECTS: usize = 32;

#[derive(Default)]
struct Frame {
    sequence: u64,
    sent: u64,
    width: usize,
    height: usize,
    scale: f32,
    pixels: Vec<u8>,
    dirty: Vec<[usize; 4]>,
    full: bool,
    cursor: i32,
    cursor_sent: i32,
}

/// Shared handle to the frame last painted by CEF.
#[derive(Clone, Default)]
pub(crate) struct FrameSlot(Arc<Mutex<Frame>>);

impl FrameSlot {
    /// Stores a CEF paint callback. `dirty` is in physical pixels and only
    /// describes the rectangles that changed; `None` means the whole view.
    ///
    /// # Safety contract
    ///
    /// `bgra` must hold `width * height * 4` readable bytes, which CEF
    /// guarantees for the lifetime of `on_paint`.
    pub(crate) fn paint(
        &self,
        width: usize,
        height: usize,
        scale: f32,
        dirty: Option<&[[i32; 4]]>,
        bgra: &[u8],
    ) {
        if width == 0 || height == 0 || bgra.len() < width * height * 4 {
            return;
        }
        let mut frame = self.lock();
        let resized = frame.width != width || frame.height != height;
        if resized {
            frame.width = width;
            frame.height = height;
            frame.pixels = vec![0; width * height * 4];
            frame.full = true;
        }
        frame.scale = scale;

        let mut rects = Vec::new();
        if !frame.full {
            match dirty {
                Some(dirty) => {
                    for rect in dirty {
                        let Some(rect) = clamp(rect, width, height) else {
                            continue;
                        };
                        rects.push(rect);
                    }
                }
                None => {
                    frame.full = true;
                    frame.dirty.clear();
                }
            }
            if frame.dirty.len() + rects.len() > MAX_DIRTY_RECTS {
                frame.full = true;
                rects.clear();
                frame.dirty.clear();
            } else {
                frame.dirty.extend(rects.iter().copied());
            }
        }
        if frame.full {
            frame.dirty.clear();
            copy(&mut frame.pixels, width, bgra, width, height, 0, 0);
        } else {
            for [x, y, w, h] in rects {
                copy(&mut frame.pixels, width, bgra, w, h, x, y);
            }
        }
        frame.sequence += 1;
    }

    /// Stores the cursor CEF last reported for the view.
    pub(crate) fn cursor(&self, cursor: i32) {
        self.lock().cursor = cursor;
    }

    /// Returns the rectangles painted since the previous call, or an empty
    /// vector when nothing changed. `full` resends the whole cached frame, for
    /// a page that just re-attached to an already painted webview. Layout:
    /// header, rectangles, then RGBA pixels for each rectangle in order.
    pub(crate) fn packet(&self, full: bool) -> Vec<u8> {
        let mut frame = self.lock();
        if frame.width == 0 || frame.height == 0 {
            frame.cursor_sent = frame.cursor;
            return Vec::new();
        }
        if !full && frame.sent == frame.sequence && frame.cursor == frame.cursor_sent {
            return Vec::new();
        }
        let width = frame.width;
        let height = frame.height;
        let fresh = frame.sent != frame.sequence;
        let rects: Vec<[usize; 4]> = if full {
            vec![[0, 0, width, height]]
        } else if !fresh {
            Vec::new()
        } else if frame.full || frame.dirty.is_empty() {
            vec![[0, 0, width, height]]
        } else {
            frame.dirty.clone()
        };
        let mut out = Vec::with_capacity(
            (HEADER_WORDS + rects.len() * RECT_WORDS) * 4
                + rects.iter().map(|r| r[2] * r[3] * 4).sum::<usize>(),
        );
        let scale = (frame.scale * 1000.0).round().max(0.0) as u32;
        for word in [
            frame.sequence.min(u32::MAX as u64) as u32,
            width as u32,
            height as u32,
            scale,
            rects.len() as u32,
            frame.cursor as u32,
        ] {
            out.extend_from_slice(&word.to_le_bytes());
        }
        for [x, y, w, h] in &rects {
            for word in [*x as u32, *y as u32, *w as u32, *h as u32] {
                out.extend_from_slice(&word.to_le_bytes());
            }
        }
        for [x, y, w, h] in &rects {
            for row in *y..*y + *h {
                let start = (row * width + *x) * 4;
                out.extend_from_slice(&frame.pixels[start..start + *w * 4]);
            }
        }
        frame.sent = frame.sequence;
        frame.cursor_sent = frame.cursor;
        frame.dirty.clear();
        frame.full = false;
        out
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Frame> {
        self.0.lock().unwrap_or_else(|error| error.into_inner())
    }
}

fn clamp(rect: &[i32; 4], width: usize, height: usize) -> Option<[usize; 4]> {
    let x = rect[0].max(0) as usize;
    let y = rect[1].max(0) as usize;
    if x >= width || y >= height {
        return None;
    }
    let w = (rect[2].max(0) as usize).min(width - x);
    let h = (rect[3].max(0) as usize).min(height - y);
    (w > 0 && h > 0).then_some([x, y, w, h])
}

/// Copies a BGRA region into an RGBA buffer.
fn copy(rgba: &mut [u8], stride: usize, bgra: &[u8], w: usize, h: usize, x: usize, y: usize) {
    for row in 0..h {
        let src = ((y + row) * stride + x) * 4;
        let dst = ((y + row) * stride + x) * 4;
        let (Some(source), Some(target)) =
            (bgra.get(src..src + w * 4), rgba.get_mut(dst..dst + w * 4))
        else {
            return;
        };
        for (pixel, source) in target.chunks_exact_mut(4).zip(source.chunks_exact(4)) {
            pixel[0] = source[2];
            pixel[1] = source[1];
            pixel[2] = source[0];
            pixel[3] = source[3];
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn u32s(bytes: &[u8]) -> Vec<u32> {
        bytes
            .chunks_exact(4)
            .map(|word| u32::from_le_bytes([word[0], word[1], word[2], word[3]]))
            .collect()
    }

    /// Words in the rectangle table, i.e. everything after the header.
    fn rects(bytes: &[u8]) -> Vec<u32> {
        u32s(bytes)[HEADER_WORDS..].to_vec()
    }

    #[test]
    fn first_paint_is_a_full_frame() {
        let slot = FrameSlot::default();
        slot.paint(2, 1, 1.0, None, &[1, 2, 3, 4, 5, 6, 7, 8]);
        let bytes = slot.packet(false);
        assert_eq!(&u32s(&bytes)[..HEADER_WORDS], &[1, 2, 1, 1000, 1, 0]);
        assert_eq!(rects(&bytes)[..RECT_WORDS], [0, 0, 2, 1]);
        assert_eq!(
            &bytes[(HEADER_WORDS + RECT_WORDS) * 4..],
            &[3, 2, 1, 4, 7, 6, 5, 8]
        );
    }

    #[test]
    fn forced_packets_resend_the_cached_frame() {
        let slot = FrameSlot::default();
        slot.paint(2, 1, 1.0, None, &[1, 2, 3, 4, 5, 6, 7, 8]);
        slot.packet(false);
        let bytes = slot.packet(true);
        assert_eq!(u32s(&bytes)[4], 1);
        assert_eq!(rects(&bytes)[..RECT_WORDS], [0, 0, 2, 1]);
    }

    #[test]
    fn packets_only_carry_new_pixels() {
        let slot = FrameSlot::default();
        slot.paint(2, 1, 1.0, None, &[1, 2, 3, 4, 5, 6, 7, 8]);
        assert!(!slot.packet(false).is_empty());
        assert!(slot.packet(false).is_empty());
        slot.paint(2, 1, 1.0, Some(&[[1, 0, 1, 1]]), &[1, 2, 3, 4, 5, 6, 7, 8]);
        let bytes = slot.packet(false);
        assert_eq!(u32s(&bytes)[4], 1);
        assert_eq!(rects(&bytes)[..RECT_WORDS], [1, 0, 1, 1]);
        assert_eq!(&bytes[(HEADER_WORDS + RECT_WORDS) * 4..], &[7, 6, 5, 8]);
    }

    #[test]
    fn cursor_changes_send_an_empty_frame() {
        let slot = FrameSlot::default();
        slot.paint(2, 1, 1.0, None, &[1, 2, 3, 4, 5, 6, 7, 8]);
        slot.packet(false);
        slot.cursor(3);
        let bytes = slot.packet(false);
        let words = u32s(&bytes);
        assert_eq!(words[4], 0);
        assert_eq!(words[5], 3);
        assert!(slot.packet(false).is_empty());
    }

    #[test]
    fn dirty_rectangles_accumulate_until_polled() {
        let slot = FrameSlot::default();
        slot.paint(2, 2, 1.0, None, &[0; 16]);
        slot.packet(false);
        slot.paint(2, 2, 1.0, Some(&[[0, 0, 1, 1]]), &[0; 16]);
        slot.paint(2, 2, 1.0, Some(&[[1, 1, 1, 1]]), &[0; 16]);
        let bytes = slot.packet(false);
        let table = rects(&bytes);
        assert_eq!(u32s(&bytes)[4], 2);
        assert_eq!(&table[..RECT_WORDS * 2], &[0, 0, 1, 1, 1, 1, 1, 1]);
    }
}
