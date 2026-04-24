use super::*;

impl Vdp {
    /// Convert a Mode 4 (SMS) CRAM byte to RGB888.
    /// Format: --BBGGRR (2 bits per channel), value range 0-3 mapped to 0/85/170/255.
    fn sms_cram_to_rgb888(cram_byte: u8) -> (u8, u8, u8) {
        let r = (cram_byte & 0x03) * 85;
        let g = ((cram_byte >> 2) & 0x03) * 85;
        let b = ((cram_byte >> 4) & 0x03) * 85;
        (r, g, b)
    }

    /// Decode a Mode 4 tile pixel from planar 4bpp data.
    /// Returns the 4-bit color index for the given pixel column (0=leftmost).
    fn sms_tile_pixel(vram: &[u8; VRAM_SIZE], tile_index: usize, row: usize, col: usize) -> u8 {
        let tile_addr = (tile_index * 32) + (row * 4);
        if tile_addr + 3 >= VRAM_SIZE {
            return 0;
        }
        let bit = 7 - col;
        let b0 = (vram[tile_addr] >> bit) & 1;
        let b1 = (vram[tile_addr + 1] >> bit) & 1;
        let b2 = (vram[tile_addr + 2] >> bit) & 1;
        let b3 = (vram[tile_addr + 3] >> bit) & 1;
        b0 | (b1 << 1) | (b2 << 2) | (b3 << 3)
    }

    /// Render a complete frame in Mode 4 (SMS compatibility mode).
    /// Resolution: 256x192, centered in the 320x240 frame buffer.
    pub(super) fn render_frame_mode4(&mut self) {
        const MODE4_WIDTH: usize = 256;
        const MODE4_HEIGHT: usize = 192;
        const BORDER_X: usize = (FRAME_WIDTH - MODE4_WIDTH) / 2;
        const BORDER_Y: usize = (FRAME_HEIGHT - MODE4_HEIGHT) / 2;

        let regs = self.registers;

        // Backdrop color: palette 0, color 0 (CRAM index 0, low byte)
        let backdrop_byte = self.cram[0] as u8;
        let (bd_r, bd_g, bd_b) = Self::sms_cram_to_rgb888(backdrop_byte);

        // Nametable base address: reg 2 bits 3-1 * 0x800
        let nt_base = ((regs[2] as usize >> 1) & 0x07) * 0x800;

        // SAT base address: reg 5 bits 6-1 * 0x100
        let sat_base = ((regs[5] as usize >> 1) & 0x3F) * 0x100;

        // Sprite tile base offset: reg 6 bit 2 -> add 256 to tile index
        let sprite_tile_offset: usize = if (regs[6] & 0x04) != 0 { 256 } else { 0 };

        // Sprite size: reg 1 bit 1 -> 8x16 mode
        let sprites_8x16 = (regs[1] & 0x02) != 0;
        let sprite_height: usize = if sprites_8x16 { 16 } else { 8 };

        // Scroll values
        let hscroll_val = regs[8] as usize;
        let vscroll_val = regs[9] as usize;

        // Reg 0 flags
        let mask_left_column = (regs[0] & 0x20) != 0;
        let lock_top_hscroll = (regs[0] & 0x40) != 0;

        // Build sprite list: scan SAT Y table, stop at Y=0xD0 or 64 entries
        struct SpriteEntry {
            y: usize,
            x: usize,
            tile: usize,
        }
        let mut sprites: Vec<SpriteEntry> = Vec::with_capacity(64);
        for i in 0..64 {
            let y_byte = self.vram[(sat_base + i) % VRAM_SIZE];
            if y_byte == 0xD0 {
                break;
            }
            // Y position: sprite appears at line (y_byte + 1)
            let y = y_byte as usize;
            let xn_offset = sat_base + 0x80 + i * 2;
            let x = self.vram[xn_offset % VRAM_SIZE] as usize;
            let tile = self.vram[(xn_offset + 1) % VRAM_SIZE] as usize;
            sprites.push(SpriteEntry { y, x, tile });
        }

        // Fill entire frame buffer with backdrop first
        for i in 0..(FRAME_WIDTH * FRAME_HEIGHT) {
            let off = i * 3;
            self.frame_buffer[off] = bd_r;
            self.frame_buffer[off + 1] = bd_g;
            self.frame_buffer[off + 2] = bd_b;
        }

        // Render the 256x192 active area
        for screen_y in 0..MODE4_HEIGHT {
            // Collect sprites on this scanline (max 8)
            let mut line_sprites: Vec<&SpriteEntry> = Vec::with_capacity(8);
            for spr in &sprites {
                // Sprite Y is +1 offset: y_byte=0 means line 1
                let spr_top = spr.y.wrapping_add(1);
                if screen_y >= spr_top && screen_y < spr_top + sprite_height {
                    line_sprites.push(spr);
                    if line_sprites.len() >= 8 {
                        break;
                    }
                }
            }

            for screen_x in 0..MODE4_WIDTH {
                // --- Background ---
                // Determine effective scroll for this pixel
                let eff_hscroll = if lock_top_hscroll && screen_y < 16 {
                    0
                } else {
                    hscroll_val
                };

                let scrolled_x = (screen_x + MODE4_WIDTH - eff_hscroll) % MODE4_WIDTH;
                let scrolled_y = (screen_y + vscroll_val) % (28 * 8); // 224 pixel wrap

                let tile_col = scrolled_x / 8;
                let tile_row = scrolled_y / 8;
                let pixel_x_in_tile = scrolled_x % 8;
                let pixel_y_in_tile = scrolled_y % 8;

                let nt_addr = nt_base + (tile_row * 32 + tile_col) * 2;
                let nt_lo = self.vram[nt_addr % VRAM_SIZE];
                let nt_hi = self.vram[(nt_addr + 1) % VRAM_SIZE];
                let nt_word = (nt_hi as u16) << 8 | (nt_lo as u16);

                let bg_tile_index = (nt_word & 0x01FF) as usize;
                let bg_hflip = (nt_word & 0x0200) != 0;
                let bg_vflip = (nt_word & 0x0400) != 0;
                let bg_palette = if (nt_word & 0x0800) != 0 { 16 } else { 0 };
                let bg_priority = (nt_word & 0x1000) != 0;

                let eff_px = if bg_hflip {
                    7 - pixel_x_in_tile
                } else {
                    pixel_x_in_tile
                };
                let eff_py = if bg_vflip {
                    7 - pixel_y_in_tile
                } else {
                    pixel_y_in_tile
                };

                let bg_color_idx =
                    Self::sms_tile_pixel(&self.vram, bg_tile_index, eff_py, eff_px) as usize;
                let bg_opaque = bg_color_idx != 0;

                // --- Sprites ---
                let mut spr_color_idx: usize = 0;
                let mut spr_opaque = false;
                for spr in &line_sprites {
                    let spr_top = spr.y.wrapping_add(1);
                    let spr_x = if mask_left_column {
                        // Shift all sprites left by 8
                        spr.x.wrapping_sub(8)
                    } else {
                        spr.x
                    };
                    if screen_x >= spr_x && screen_x < spr_x + 8 {
                        let px = screen_x - spr_x;
                        let py = screen_y - spr_top;
                        let mut tile_idx = spr.tile + sprite_tile_offset;
                        let tile_row_in_spr;
                        if sprites_8x16 {
                            tile_idx &= !1; // Force bit 0 to 0
                            if py >= 8 {
                                tile_idx += 1;
                                tile_row_in_spr = py - 8;
                            } else {
                                tile_row_in_spr = py;
                            }
                        } else {
                            tile_row_in_spr = py;
                        }

                        let c = Self::sms_tile_pixel(&self.vram, tile_idx, tile_row_in_spr, px)
                            as usize;
                        if c != 0 {
                            spr_color_idx = c + 16; // Sprites always use palette 1
                            spr_opaque = true;
                            break;
                        }
                    }
                }

                // --- Priority compositing ---
                let final_color_idx = if bg_priority && bg_opaque {
                    bg_palette + bg_color_idx
                } else if spr_opaque {
                    spr_color_idx
                } else if bg_opaque {
                    bg_palette + bg_color_idx
                } else {
                    0 // backdrop
                };

                // Left column masking
                let masked = mask_left_column && screen_x < 8;

                let (r, g, b) = if masked {
                    (bd_r, bd_g, bd_b)
                } else {
                    let cram_byte = self.cram[final_color_idx % CRAM_COLORS] as u8;
                    Self::sms_cram_to_rgb888(cram_byte)
                };

                let fb_x = BORDER_X + screen_x;
                let fb_y = BORDER_Y + screen_y;
                let off = (fb_y * FRAME_WIDTH + fb_x) * 3;
                self.frame_buffer[off] = r;
                self.frame_buffer[off + 1] = g;
                self.frame_buffer[off + 2] = b;
            }
        }
    }
}
