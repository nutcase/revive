use super::{trace::disable_authoritative_superfx_bg1_source, Ppu};

impl Ppu {
    pub(crate) fn has_superfx_direct_bg1_source(&self) -> bool {
        self.superfx_direct_height != 0
            && matches!(self.superfx_direct_bpp, 2 | 4 | 8)
            && !self.superfx_direct_buffer.is_empty()
    }

    pub(crate) fn has_superfx_tile_bg1_source(&self) -> bool {
        self.superfx_tile_bpp == 4 && !self.superfx_tile_buffer.is_empty()
    }

    pub(crate) fn has_authoritative_superfx_bg1_source(&self) -> bool {
        // Keep this opt-in. Leaving it enabled globally makes later Star Fox
        // phases sample intermediate SuperFX buffers instead of the normal PPU
        // scene composition.
        self.superfx_authoritative_bg1_source
            && !disable_authoritative_superfx_bg1_source()
            && self.bg_mode == 2
            && (self.has_superfx_direct_bg1_source() || self.has_superfx_tile_bg1_source())
    }

    pub(crate) fn should_bypass_bg1_window_for_superfx_direct(&self) -> bool {
        self.superfx_bypass_bg1_window || self.has_authoritative_superfx_bg1_source()
    }

    pub(crate) fn set_superfx_direct_buffer(
        &mut self,
        buffer: Vec<u8>,
        height: u16,
        bpp: u8,
        mode: u8,
    ) {
        self.superfx_direct_default_x_offset =
            Self::default_superfx_direct_x_offset(&buffer, height, bpp, mode, self.frame);
        self.superfx_direct_default_y_offset =
            Self::forced_blank_superfx_direct_y_offset(height, bpp, mode, self.is_forced_blank())
                .unwrap_or_else(|| {
                    Self::default_superfx_direct_y_offset(&buffer, height, bpp, mode, self.frame)
                });
        self.superfx_direct_buffer = buffer;
        self.superfx_direct_height = height;
        self.superfx_direct_bpp = bpp;
        self.superfx_direct_mode = mode & 0x03;
    }

    pub(crate) fn default_superfx_direct_x_offset(
        buffer: &[u8],
        height: u16,
        bpp: u8,
        mode: u8,
        _frame: u64,
    ) -> i32 {
        if height == 192 && bpp == 4 && (mode & 0x03) == 2 && buffer.len() >= 24_576 {
            // This SuperFX screen layout has a stable 224px-wide scene viewport.
            // Re-estimating the origin from current non-zero pixels makes sparse
            // intermediate frames jump horizontally.
            return -16;
        }
        -56
    }

    pub(crate) fn default_superfx_direct_y_offset(
        buffer: &[u8],
        height: u16,
        bpp: u8,
        mode: u8,
        frame: u64,
    ) -> i32 {
        if height == 192 && bpp == 4 && (mode & 0x03) == 2 {
            let nonzero_bytes = buffer.iter().filter(|&&byte| byte != 0).count();
            if frame < 240 && (384..=2_200).contains(&nonzero_bytes) {
                return -16;
            }
        }
        0
    }

    pub(crate) fn forced_blank_superfx_direct_y_offset(
        height: u16,
        bpp: u8,
        mode: u8,
        forced_blank: bool,
    ) -> Option<i32> {
        if forced_blank && height == 192 && bpp == 4 && (mode & 0x03) == 2 {
            return Some(-16);
        }
        None
    }

    pub(crate) fn set_superfx_authoritative_bg1_source(&mut self, enabled: bool) {
        self.superfx_authoritative_bg1_source = enabled;
    }

    pub(crate) fn set_starfox_title_bg1_suppression(&mut self, enabled: bool) {
        self.starfox_title_suppress_bg1 = enabled;
        self.update_line_render_state();
    }

    pub(crate) fn clear_superfx_direct_buffer(&mut self) {
        self.superfx_direct_buffer.clear();
        self.superfx_direct_height = 0;
        self.superfx_direct_bpp = 0;
        self.superfx_direct_mode = 0;
        self.superfx_direct_default_x_offset = -56;
        self.superfx_direct_default_y_offset = 0;
    }

    pub(crate) fn set_superfx_tile_buffer(&mut self, buffer: Vec<u8>, bpp: u8, mode: u8) {
        self.superfx_tile_buffer = buffer;
        self.superfx_tile_bpp = bpp;
        self.superfx_tile_mode = mode & 0x03;
    }

    pub(crate) fn clear_superfx_tile_buffer(&mut self) {
        self.superfx_tile_buffer.clear();
        self.superfx_tile_bpp = 0;
        self.superfx_tile_mode = 0;
    }
}
