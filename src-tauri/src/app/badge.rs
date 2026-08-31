//! Tray badge: overdue-count icon rendered as a pure rasterizer.

use tauri::image::Image;
use tauri::AppHandle;

pub(super) fn update_tray_badge(app: &AppHandle, overdue: usize) {
    let Some(tray) = app.tray_by_id("main") else {
        return;
    };
    if overdue == 0 {
        tray.set_icon(app.default_window_icon().cloned()).ok();
        tray.set_tooltip(Some("MayDolist")).ok();
    } else {
        tray.set_icon(Some(draw_overdue_badge(overdue))).ok();
        tray.set_tooltip(Some(&format!("MayDolist — {overdue} 项逾期")))
            .ok();
    }
}

/// Render a red badge icon with the overdue count (up to two digits) on a
/// transparent 16x16 RGBA image. Pure rasterizer: no fonts or OS calls, so it
/// works on any platform and is unit-testable.
fn draw_overdue_badge(count: usize) -> Image<'static> {
    const SIZE: usize = 16;
    const DIGIT_W: usize = 3;
    const DIGIT_H: usize = 5;
    const SCALE: usize = 2;
    const DIGITS: [[u8; DIGIT_W * DIGIT_H]; 10] = [
        [0, 1, 0, 1, 0, 1, 1, 0, 1, 1, 0, 1, 0, 1, 0], // 0
        [0, 1, 0, 1, 1, 0, 0, 1, 0, 0, 1, 0, 1, 1, 1], // 1
        [1, 1, 1, 0, 0, 1, 0, 1, 0, 1, 0, 0, 1, 1, 1], // 2
        [1, 1, 1, 0, 0, 1, 0, 1, 0, 0, 0, 1, 1, 1, 1], // 3
        [1, 0, 1, 1, 0, 1, 1, 1, 1, 0, 0, 1, 0, 0, 1], // 4
        [1, 1, 1, 1, 0, 0, 1, 1, 1, 0, 0, 1, 1, 1, 1], // 5
        [0, 1, 0, 1, 0, 0, 1, 1, 1, 1, 0, 1, 1, 1, 1], // 6
        [1, 1, 1, 0, 0, 1, 0, 1, 0, 1, 0, 0, 1, 0, 0], // 7
        [0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0], // 8
        [1, 1, 1, 1, 0, 1, 1, 1, 1, 0, 0, 1, 0, 1, 0], // 9
    ];
    let mut rgba = vec![0u8; SIZE * SIZE * 4];
    let text = count.min(99).to_string();
    let digits_w = text.len() * DIGIT_W * SCALE;
    let digits_h = DIGIT_H * SCALE;
    let start_x = (SIZE - digits_w) / 2;
    let start_y = (SIZE - digits_h) / 2;
    for y in 0..SIZE {
        for x in 0..SIZE {
            let dx = x as f32 + 0.5 - 7.5;
            let dy = y as f32 + 0.5 - 7.5;
            let mut pixel = [0u8; 4];
            if dx * dx + dy * dy <= 8.0 * 8.0 {
                pixel = [232, 68, 68, 255];
            }
            for (index, ch) in text.chars().enumerate() {
                let glyph = &DIGITS[ch.to_digit(10).unwrap_or(0) as usize];
                let gx = (x as i32 - (start_x + index * DIGIT_W * SCALE) as i32) / SCALE as i32;
                let gy = (y as i32 - start_y as i32) / SCALE as i32;
                if gx >= 0
                    && gy >= 0
                    && (gx as usize) < DIGIT_W
                    && (gy as usize) < DIGIT_H
                    && glyph[gy as usize * DIGIT_W + gx as usize] == 1
                {
                    pixel = [255, 255, 255, 255];
                }
            }
            if pixel[3] != 0 {
                let offset = (y * SIZE + x) * 4;
                rgba[offset..offset + 4].copy_from_slice(&pixel);
            }
        }
    }
    Image::new_owned(rgba, SIZE as u32, SIZE as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn badge_has_expected_size_and_opaque_pixels() {
        let badge = draw_overdue_badge(3);
        assert_eq!(badge.width(), 16);
        assert_eq!(badge.height(), 16);
        let rgba = badge.rgba();
        assert_eq!(rgba.len(), 16 * 16 * 4);
        assert!(
            rgba.chunks_exact(4).any(|p| p[3] != 0),
            "badge must contain visible pixels"
        );
    }

    #[test]
    fn badge_digits_differ_and_count_is_capped() {
        let one = draw_overdue_badge(1).rgba().to_vec();
        let twelve = draw_overdue_badge(12).rgba().to_vec();
        assert_ne!(one, twelve, "different counts must render differently");
        let capped = draw_overdue_badge(9999);
        // 99 renders the same glyphs as 9999 (capped at two digits).
        assert_eq!(
            capped.rgba().to_vec(),
            draw_overdue_badge(99).rgba().to_vec()
        );
    }

    #[test]
    fn badge_zero_still_renders_red_circle() {
        // `draw_overdue_badge(0)` shows "0" — used only while a count exists;
        // the tray is reset to the default icon when the count reaches 0.
        let badge = draw_overdue_badge(0);
        assert!(badge.rgba().chunks_exact(4).any(|p| p[0] == 232));
    }
}
