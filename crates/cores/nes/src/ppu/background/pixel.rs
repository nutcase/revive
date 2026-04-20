use super::*;

impl Ppu {
    #[inline]
    pub(in crate::ppu) fn render_background_pixel(
        &mut self,
        x: u16,
        y: i16,
        cartridge: Option<&crate::cartridge::Cartridge>,
    ) -> (u8, u8) {
        let mut bg_color = self.palette[0];
        let mut bg_pixel = 0u8;

        if self.scanline_bg_enable {
            if !self.scanline_bg_left && x < 8 {
                // bg_color stays palette[0], bg_pixel stays 0
            } else if let Some(cart) = cartridge {
                if let Some((low_byte, high_byte, palette_num)) =
                    mapper_hooks::mmc5_split_bg_fetch(cartridge, x as u8, y as u8, self.x)
                {
                    let pixel_bit = 7 - (x as u8 & 0x07);
                    let low_bit = (low_byte >> pixel_bit) & 1;
                    let high_bit = (high_byte >> pixel_bit) & 1;
                    let pixel_value = (high_bit << 1) | low_bit;

                    bg_pixel = pixel_value;

                    if pixel_value != 0 {
                        let palette_idx = (palette_num as usize * 4) + pixel_value as usize;
                        bg_color = self.palette[palette_idx];
                    }
                } else {
                    let fine_y = (self.v >> 12) & 7;
                    let coarse_y = ((self.v >> 5) & 0x1F) as usize;
                    let logical_nt = ((self.v >> 10) & 3) as usize;
                    let coarse_x = (self.v & 0x1F) as usize;

                    let pixel_col = (x & 7) as u8;
                    let scrolled_col = pixel_col + self.x;
                    let (tile_cx, tile_nt, tile_fx) = if scrolled_col >= 8 {
                        let next_cx = if coarse_x == 31 { 0 } else { coarse_x + 1 };
                        let next_nt = if coarse_x == 31 {
                            logical_nt ^ 1
                        } else {
                            logical_nt
                        };
                        (next_cx, next_nt, scrolled_col - 8)
                    } else {
                        (coarse_x, logical_nt, scrolled_col)
                    };

                    let physical_nt = self.cached_nt_map[tile_nt & 3] as usize;
                    let nt_addr = coarse_y * 32 + tile_cx;
                    let tile_id = self.read_nametable_byte(physical_nt, nt_addr, cartridge);

                    let pattern_table = if self.control.contains(PpuControl::BG_PATTERN) {
                        0x1000u16
                    } else {
                        0x0000u16
                    };
                    let tile_addr = pattern_table + (tile_id as u16 * 16) + fine_y;

                    if tile_addr < 0x2000 {
                        // Tile cache: reuse CHR data within the same tile (same
                        // tile_addr).  The cache is invalidated every 8 pixels at
                        // tile boundaries so that MMC2/MMC4 latch changes between
                        // tiles always trigger a fresh CHR read.
                        let (low_byte, high_byte) = if tile_addr == self.cached_tile_addr {
                            (self.cached_tile_low, self.cached_tile_high)
                        } else {
                            let low = cart.read_chr(tile_addr);
                            let high = cart.read_chr(tile_addr + 8);
                            self.cached_tile_addr = tile_addr;
                            self.cached_tile_low = low;
                            self.cached_tile_high = high;
                            (low, high)
                        };
                        let pixel_bit = 7 - tile_fx;
                        let low_bit = (low_byte >> pixel_bit) & 1;
                        let high_bit = (high_byte >> pixel_bit) & 1;
                        let pixel_value = (high_bit << 1) | low_bit;

                        bg_pixel = pixel_value;

                        if pixel_value != 0 {
                            let attr_x = tile_cx >> 2;
                            let attr_y = coarse_y >> 2;
                            let attr_offset = 960 + (attr_y << 3) + attr_x;
                            let attr_byte =
                                self.read_nametable_byte(physical_nt, attr_offset, cartridge);

                            let block_x = (tile_cx & 3) >> 1;
                            let block_y = (coarse_y & 3) >> 1;
                            let shift = (block_y * 2 + block_x) * 2;
                            let palette_num = (attr_byte >> shift) & 0x03;

                            let palette_idx = (palette_num as usize * 4) + pixel_value as usize;
                            bg_color = self.palette[palette_idx];
                        }
                    }
                }
            }
        }

        (bg_color, bg_pixel)
    }
}
