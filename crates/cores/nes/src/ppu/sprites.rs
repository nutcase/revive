use super::{Ppu, PpuControl, PpuMask, PpuStatus};

impl Ppu {
    pub(super) fn evaluate_scanline_sprites(
        &mut self,
        cartridge: Option<&crate::cartridge::Cartridge>,
    ) {
        self.scanline_sprite_count = 0;
        if self.scanline < 0 || self.scanline >= 240 {
            return;
        }

        // Update scanline-cached mask flags
        self.scanline_bg_enable = self.mask.contains(PpuMask::BG_ENABLE);
        self.scanline_sprite_enable = self.mask.contains(PpuMask::SPRITE_ENABLE);
        self.scanline_bg_left = self.mask.contains(PpuMask::BG_LEFT_ENABLE);
        self.scanline_sprite_left = self.mask.contains(PpuMask::SPRITE_LEFT_ENABLE);
        self.scanline_grayscale = self.mask.contains(PpuMask::GRAYSCALE);
        self.scanline_color_emphasis = self.mask.bits() & 0xE0;

        // Cache sprite control registers
        self.cached_sprite_size = if self.control.contains(PpuControl::SPRITE_SIZE) {
            16
        } else {
            8
        };
        self.cached_sprite_pattern_table = if self.control.contains(PpuControl::SPRITE_PATTERN) {
            0x1000
        } else {
            0x0000
        };

        // Cache nametable mirroring map
        if let Some(cart) = cartridge {
            if let Some(mapped0) = cart.resolve_nametable(0) {
                self.cached_nt_map = [
                    mapped0 as u8,
                    cart.resolve_nametable(1).unwrap_or(1) as u8,
                    cart.resolve_nametable(2).unwrap_or(0) as u8,
                    cart.resolve_nametable(3).unwrap_or(1) as u8,
                ];
            } else {
                match cart.mirroring() {
                    crate::cartridge::Mirroring::Vertical => self.cached_nt_map = [0, 1, 0, 1],
                    crate::cartridge::Mirroring::Horizontal => self.cached_nt_map = [0, 0, 1, 1],
                    crate::cartridge::Mirroring::HorizontalSwapped => {
                        self.cached_nt_map = [1, 1, 0, 0]
                    }
                    crate::cartridge::Mirroring::ThreeScreenLower => {
                        self.cached_nt_map = [0, 0, 0, 1]
                    }
                    crate::cartridge::Mirroring::FourScreen => self.cached_nt_map = [0, 1, 0, 1],
                    crate::cartridge::Mirroring::OneScreenLower => {
                        self.cached_nt_map = [0, 0, 0, 0]
                    }
                    crate::cartridge::Mirroring::OneScreenUpper => {
                        self.cached_nt_map = [1, 1, 1, 1]
                    }
                }
            }
        }

        // Invalidate tile cache for new scanline
        self.cached_tile_addr = 0xFFFF;

        let sprite_height: u16 = self.cached_sprite_size as u16;
        let current_scanline = self.scanline as u16;

        for sprite_num in 0u8..64 {
            let base = sprite_num as usize * 4;
            let sprite_y = self.oam[base];

            if sprite_y >= 0xEF {
                continue;
            }

            let sprite_top = sprite_y as u16 + 1;
            let sprite_bottom = sprite_top + sprite_height;

            if current_scanline >= sprite_top && current_scanline < sprite_bottom {
                let idx = self.scanline_sprite_count as usize;
                self.scanline_sprites[idx] = (
                    sprite_num,
                    sprite_y,
                    self.oam[base + 1],
                    self.oam[base + 2],
                    self.oam[base + 3],
                );
                self.scanline_sprite_count += 1;

                if self.scanline_sprite_count == 8 {
                    let next_sprite_num = sprite_num + 1;
                    if next_sprite_num < 64
                        && self.sprite_overflow_bug_matches(
                            next_sprite_num,
                            current_scanline,
                            sprite_height,
                        )
                    {
                        self.status.insert(PpuStatus::SPRITE_OVERFLOW);
                    }
                    break;
                }
            }
        }
    }

    fn sprite_overflow_bug_matches(
        &self,
        start_sprite_num: u8,
        current_scanline: u16,
        sprite_height: u16,
    ) -> bool {
        let mut sprite_num = start_sprite_num;
        let mut byte_index = 0usize;

        while sprite_num < 64 {
            let base = sprite_num as usize * 4;
            let value = self.oam[base + byte_index];

            if Self::sprite_value_in_range(value, current_scanline, sprite_height) {
                return true;
            }

            // Hardware bug: after secondary OAM fills, both the sprite index and
            // byte index advance, so tile/attribute/X bytes may be tested as Y.
            sprite_num += 1;
            byte_index = (byte_index + 1) & 0x03;
        }

        false
    }

    fn sprite_value_in_range(value: u8, current_scanline: u16, sprite_height: u16) -> bool {
        if value >= 0xEF {
            return false;
        }

        let sprite_top = value as u16 + 1;
        current_scanline >= sprite_top && current_scanline < sprite_top + sprite_height
    }

    #[inline]
    pub(super) fn render_sprites(
        &self,
        x: u8,
        y: u8,
        cartridge: Option<&crate::cartridge::Cartridge>,
        sprite_0_hit: &mut bool,
    ) -> Option<(u8, bool)> {
        if let Some(cart) = cartridge {
            let sprite_size = self.cached_sprite_size;
            let count = self.scanline_sprite_count as usize;

            for i in 0..count {
                let (sprite_num, sprite_y, tile_id, attributes, sprite_x) =
                    self.scanline_sprites[i];

                // Check if pixel is within sprite horizontal bounds
                if x < sprite_x || (x as u16) >= sprite_x as u16 + 8 {
                    continue;
                }

                let sprite_top = sprite_y as u16 + 1;
                let mut pixel_x = x - sprite_x;
                let mut pixel_y = (y as u16 - sprite_top) as u8;

                // Handle horizontal flip
                if attributes & 0x40 != 0 {
                    pixel_x = 7 - pixel_x;
                }

                // Handle vertical flip
                if attributes & 0x80 != 0 {
                    pixel_y = (sprite_size - 1) - pixel_y;
                }

                // Calculate pattern table address
                let (pattern_table, actual_tile_id) = if sprite_size == 16 {
                    let pattern_table: u16 = if tile_id & 0x01 != 0 { 0x1000 } else { 0x0000 };
                    let actual_tile_id = tile_id & 0xFE;
                    (pattern_table, actual_tile_id)
                } else {
                    (self.cached_sprite_pattern_table, tile_id)
                };

                // For 8x16 sprites, select top or bottom half
                let final_tile_id = if sprite_size == 16 && pixel_y >= 8 {
                    actual_tile_id + 1
                } else {
                    actual_tile_id
                };

                let pattern_fine_y = (pixel_y & 7) as u16;
                let tile_addr = pattern_table + (final_tile_id as u16 * 16) + pattern_fine_y;

                // Read pattern data
                if tile_addr + 8 < 0x2000 {
                    let low_byte = cart.read_chr_sprite(tile_addr, sprite_y);
                    let high_byte = cart.read_chr_sprite(tile_addr + 8, sprite_y);
                    let pixel_bit = 7 - pixel_x;
                    let low_bit = (low_byte >> pixel_bit) & 1;
                    let high_bit = (high_byte >> pixel_bit) & 1;
                    let pixel_value = (high_bit << 1) | low_bit;

                    if pixel_value != 0 {
                        if sprite_num == 0 && x != 255 {
                            *sprite_0_hit = true;
                        }

                        let palette_num = attributes & 0x03;
                        let palette_idx = (16 + palette_num * 4 + pixel_value) as usize;
                        let color_index = self.palette[palette_idx];

                        let priority_behind_bg = (attributes & 0x20) != 0;
                        return Some((color_index, priority_behind_bg));
                    }
                }
            }
        }
        None
    }
}
