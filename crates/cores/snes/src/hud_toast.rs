use std::time::{Duration, Instant};

const HUD_TOAST_DURATION: Duration = Duration::from_millis(1600);
const HUD_MARGIN: usize = 8;
const HUD_PADDING: usize = 3;
const HUD_SCALE: usize = 1;
const HUD_BG_COLOR: u32 = 0x101010;
const HUD_TEXT_COLOR: u32 = 0xF8F8F8;

pub struct HudToast {
    text: String,
    expires_at: Instant,
}

pub fn show_hud_toast(slot: &mut Option<HudToast>, text: impl Into<String>) {
    *slot = Some(HudToast {
        text: text.into(),
        expires_at: Instant::now() + HUD_TOAST_DURATION,
    });
}

pub fn draw_hud_toast(
    frame: &mut [u32],
    width: usize,
    height: usize,
    toast: &mut Option<HudToast>,
) {
    let now = Instant::now();
    let text = match toast.as_ref() {
        Some(t) if t.expires_at > now => t.text.clone(),
        Some(_) => {
            *toast = None;
            return;
        }
        None => return,
    };

    draw_hud_text(frame, width, height, &text);
}

fn draw_hud_text(frame: &mut [u32], width: usize, height: usize, text: &str) {
    if width == 0 || height == 0 || text.is_empty() {
        return;
    }
    let glyph_w = 5 * HUD_SCALE;
    let glyph_h = 7 * HUD_SCALE;
    let spacing = HUD_SCALE;
    let char_count = text.chars().count();
    if char_count == 0 {
        return;
    }
    let text_w = char_count * glyph_w + (char_count.saturating_sub(1)) * spacing;
    let box_w = text_w + HUD_PADDING * 2;
    let box_h = glyph_h + HUD_PADDING * 2;
    let box_x = HUD_MARGIN.min(width.saturating_sub(1));
    let box_y = HUD_MARGIN.min(height.saturating_sub(1));
    fill_rect(
        frame,
        width,
        height,
        box_x,
        box_y,
        box_w,
        box_h,
        HUD_BG_COLOR,
    );

    let mut x = box_x + HUD_PADDING;
    let y = box_y + HUD_PADDING;
    for ch in text.chars() {
        draw_glyph(
            frame,
            width,
            height,
            x,
            y,
            glyph_5x7(ch.to_ascii_uppercase()),
            HUD_TEXT_COLOR,
            HUD_SCALE,
        );
        x += glyph_w + spacing;
    }
}

#[allow(clippy::too_many_arguments)]
fn fill_rect(
    frame: &mut [u32],
    width: usize,
    height: usize,
    x: usize,
    y: usize,
    w: usize,
    h: usize,
    color: u32,
) {
    let x_end = x.saturating_add(w).min(width);
    let y_end = y.saturating_add(h).min(height);
    for py in y..y_end {
        let row = py * width;
        for px in x..x_end {
            frame[row + px] = color;
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_glyph(
    frame: &mut [u32],
    width: usize,
    height: usize,
    x: usize,
    y: usize,
    rows: [u8; 7],
    color: u32,
    scale: usize,
) {
    for (ry, bits) in rows.iter().enumerate() {
        for rx in 0..5 {
            if (bits >> (4 - rx)) & 1 == 1 {
                fill_rect(
                    frame,
                    width,
                    height,
                    x + rx * scale,
                    y + ry * scale,
                    scale,
                    scale,
                    color,
                );
            }
        }
    }
}

fn glyph_5x7(ch: char) -> [u8; 7] {
    match ch {
        'A' => [
            0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001,
        ],
        'D' => [
            0b11110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b11110,
        ],
        'E' => [
            0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111,
        ],
        'I' => [
            0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b11111,
        ],
        'K' => [
            0b10001, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010, 0b10001,
        ],
        'L' => [
            0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111,
        ],
        'O' => [
            0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110,
        ],
        'R' => [
            0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001,
        ],
        'S' => [
            0b01111, 0b10000, 0b10000, 0b01110, 0b00001, 0b00001, 0b11110,
        ],
        'T' => [
            0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100,
        ],
        'V' => [
            0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01010, 0b00100,
        ],
        '0' => [
            0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110,
        ],
        '1' => [
            0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110,
        ],
        '2' => [
            0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b01000, 0b11111,
        ],
        '3' => [
            0b11110, 0b00001, 0b00001, 0b00110, 0b00001, 0b00001, 0b11110,
        ],
        '4' => [
            0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010,
        ],
        '5' => [
            0b11111, 0b10000, 0b10000, 0b11110, 0b00001, 0b00001, 0b11110,
        ],
        '6' => [
            0b01110, 0b10000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110,
        ],
        '7' => [
            0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000,
        ],
        '8' => [
            0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110,
        ],
        '9' => [
            0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00001, 0b01110,
        ],
        ' ' => [0, 0, 0, 0, 0, 0, 0],
        _ => [
            0b01110, 0b10001, 0b00010, 0b00100, 0b00100, 0b00000, 0b00100,
        ],
    }
}
