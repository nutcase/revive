use std::sync::OnceLock;

use super::Ppu;

impl Ppu {
    #[inline]
    pub(crate) fn force_display_active(&self) -> bool {
        crate::debug_flags::force_display()
    }

    #[inline]
    pub(crate) fn bg_interlace_active(&self) -> bool {
        self.interlace && (self.bg_mode == 5 || self.bg_mode == 6)
    }

    pub(crate) fn bg_interlace_y(&self, y: u16) -> u16 {
        if self.bg_interlace_active() {
            y.saturating_mul(2)
                .saturating_add(self.interlace_field as u16)
        } else {
            y
        }
    }

    #[inline]
    pub(crate) fn first_visible_dot(&self) -> u16 {
        22
    }

    #[inline]
    pub(crate) fn dots_per_line(&self) -> u16 {
        self.dots_per_scanline(self.scanline)
    }

    #[inline]
    pub(crate) fn scanlines_per_frame(&self) -> u16 {
        262 + u16::from(self.interlace && !self.interlace_field)
    }

    #[inline]
    pub(crate) fn dots_per_scanline(&self, scanline: u16) -> u16 {
        if !self.interlace && self.interlace_field && scanline == 240 {
            340
        } else {
            341
        }
    }

    #[inline]
    pub(crate) fn first_hblank_dot(&self) -> u16 {
        self.first_visible_dot() + 256
    }

    #[inline]
    pub(crate) fn last_dot_index(&self) -> u16 {
        self.dots_per_line() - 1
    }

    #[inline]
    pub(crate) fn last_scanline_index(&self) -> u16 {
        self.scanlines_per_frame() - 1
    }

    #[inline]
    pub(crate) fn remaining_dots_in_frame(&self) -> u32 {
        let current_line_remaining = self.dots_per_line().saturating_sub(self.cycle) as u32;
        let mut remaining = current_line_remaining;
        let frame_lines = self.scanlines_per_frame();
        let mut scanline = self.scanline.saturating_add(1);
        while scanline < frame_lines {
            remaining = remaining.saturating_add(self.dots_per_scanline(scanline) as u32);
            scanline = scanline.saturating_add(1);
        }
        remaining
    }

    #[inline]
    pub fn get_visible_height(&self) -> u16 {
        static OVERRIDE: OnceLock<Option<u16>> = OnceLock::new();
        let override_val = *OVERRIDE.get_or_init(|| {
            std::env::var("PPU_VIS_HEIGHT")
                .ok()
                .and_then(|v| v.parse::<u16>().ok())
                .filter(|v| *v >= 160 && *v <= 239)
        });
        if let Some(v) = override_val {
            return v;
        }

        if self.overscan {
            239
        } else {
            224
        }
    }

    #[inline]
    pub(crate) fn vblank_start_line(&self) -> u16 {
        self.get_visible_height().saturating_add(1)
    }

    #[inline]
    #[allow(dead_code)]
    pub(crate) fn fixed8_floor(val: i64) -> i32 {
        if val >= 0 {
            (val >> 8) as i32
        } else {
            -(((-val + 255) >> 8) as i32)
        }
    }
}
