use crate::ppu::Ppu;

impl Ppu {
    // Mode 7変換
    #[allow(dead_code)]
    pub(crate) fn mode7_transform(&self, screen_x: u16, screen_y: u16) -> (i32, i32) {
        // 画面座標を中心基準に変換
        let sx = screen_x as i32 - 128;
        let sy = screen_y as i32 - 128;

        // 回転中心からの相対座標
        let rel_x = sx - (self.mode7_center_x as i32);
        let rel_y = sy - (self.mode7_center_y as i32);

        // 変換行列適用 (固定小数点演算)
        let a = self.mode7_matrix_a as i32;
        let b = self.mode7_matrix_b as i32;
        let c = self.mode7_matrix_c as i32;
        let d = self.mode7_matrix_d as i32;

        let transformed_x = ((a * rel_x + b * rel_y) >> 8) + (self.mode7_center_x as i32);
        let transformed_y = ((c * rel_x + d * rel_y) >> 8) + (self.mode7_center_y as i32);

        (transformed_x, transformed_y)
    }

    // Mode 7 affine transform producing integer world pixels.
    #[inline]
    pub(crate) fn mode7_world_xy_int(&self, sx: i32, sy: i32) -> (i32, i32) {
        // Promote to i64 to avoid overflow in affine products.
        //
        // Mode 7 affine (SNESdev):
        //   [X]   [A B] [SX + HOFS - CX] + [CX]
        //   [Y] = [C D] [SY + VOFS - CY]   [CY]
        //
        // A..D are signed 8.8 fixed; SX/SY, HOFS/VOFS, CX/CY are signed integers.
        let a = self.mode7_matrix_a as i64;
        let b = self.mode7_matrix_b as i64;
        let c = self.mode7_matrix_c as i64;
        let d = self.mode7_matrix_d as i64;
        let cx = self.mode7_center_x as i64;
        let cy = self.mode7_center_y as i64;
        let hofs = self.mode7_hofs as i64;
        let vofs = self.mode7_vofs as i64;

        let dx = (sx as i64) + hofs - cx;
        let dy = (sy as i64) + vofs - cy;

        let x = ((a * dx + b * dy) >> 8) + cx;
        let y = ((c * dx + d * dy) >> 8) + cy;
        (x as i32, y as i32)
    }

    #[inline]
    pub(crate) fn render_mode7_with_layer(&mut self, x: u16, y: u16) -> (u32, u8, u8) {
        // Mode 7: affine transform into 1024x1024 world; tiles: 8x8 8bpp, map: 128x128 bytes.
        // Helper: sample for a desired layer (0:BG1, 1:BG2 when EXTBG). Applies mosaic per layer.
        let sample_for_layer = |desired_layer: u8| -> (u32, u8, u8, bool, bool, bool, bool) {
            // Screen mosaic per layer
            let (mx, my) = self.apply_mosaic(x, y, desired_layer);
            // Apply flips around 255
            let sx = if (self.m7sel & 0x01) != 0 {
                255 - (mx as i32)
            } else {
                mx as i32
            };
            let sy = if (self.m7sel & 0x02) != 0 {
                255 - (my as i32)
            } else {
                my as i32
            };

            let (wx, wy) = self.mode7_world_xy_int(sx, sy);
            let repeat_off = (self.m7sel & 0x80) != 0; // R
            let fill_char0 = (self.m7sel & 0x40) != 0; // F (only when R=1)
            let inside = (0..1024).contains(&wx) && (0..1024).contains(&wy);
            let (ix, iy, outside, wrapped) = if inside {
                (wx, wy, false, false)
            } else if !repeat_off {
                // 1024 is a power of two, so masking matches Euclidean modulo for signed i32.
                (wx & 0x03FF, wy & 0x03FF, false, true)
            } else {
                (wx, wy, true, false)
            };

            if outside {
                if !fill_char0 {
                    return (0, 0, desired_layer, false, true, false, false);
                }
                let px = (ix & 7) as u8;
                let py = (iy & 7) as u8;
                if self.extbg {
                    let (c, pr, lid) = self.sample_mode7_for_layer(0, px, py, desired_layer);
                    return (c, pr, lid, false, true, true, false);
                } else {
                    let (c, pr) = self.sample_mode7_color_only(0, px, py);
                    return (c, pr, 0, false, true, true, false);
                }
            }

            // In-bounds or wrapped sampling
            let tile_x = (ix >> 3) & 0x7F; // 0..127
            let tile_y = (iy >> 3) & 0x7F; // 0..127
            let px = (ix & 7) as u8;
            let py = (iy & 7) as u8;
            // Mode 7 VRAM layout:
            // - Tilemap: low byte of VRAM words 0x0000..0x3FFF (128x128 bytes)
            // - Tile data: high byte of the same VRAM words (256 tiles * 64 bytes = 16384 bytes)
            let map_word = ((tile_y as usize) << 7) | (tile_x as usize);
            let map_index = map_word * 2;
            if map_index >= self.vram.len() {
                return (0, 0, desired_layer, wrapped, false, false, false);
            }
            let tile_id = self.vram[map_index] as u16;

            let edge = ix == 0 || ix == 1023 || iy == 0 || iy == 1023;
            if self.extbg {
                let (c, pr, lid) = self.sample_mode7_for_layer(tile_id, px, py, desired_layer);
                (c, pr, lid, wrapped, false, false, edge)
            } else {
                let (c, pr) = self.sample_mode7_color_only(tile_id, px, py);
                (c, pr, 0, wrapped, false, false, edge)
            }
        };

        if self.extbg {
            let (c2, p2, lid2, wrap2, clip2, fill2, edge2) = sample_for_layer(1);
            let (c1, p1, lid1, wrap1, clip1, fill1, edge1) = sample_for_layer(0);
            // Metrics
            if crate::debug_flags::render_metrics() {
                if wrap1 || wrap2 {
                    self.dbg_m7_wrap = self.dbg_m7_wrap.saturating_add(1);
                }
                if clip1 || clip2 {
                    self.dbg_m7_clip = self.dbg_m7_clip.saturating_add(1);
                }
                if fill1 || fill2 {
                    self.dbg_m7_fill = self.dbg_m7_fill.saturating_add(1);
                }
                if c1 != 0 {
                    self.dbg_m7_bg1 = self.dbg_m7_bg1.saturating_add(1);
                }
                if c2 != 0 {
                    self.dbg_m7_bg2 = self.dbg_m7_bg2.saturating_add(1);
                }
                if edge1 || edge2 {
                    self.dbg_m7_edge = self.dbg_m7_edge.saturating_add(1);
                }
            }
            // Prefer BG1 over BG2 when both present; actual sort happens in z-rank stage.
            if c1 != 0 {
                return (c1, p1, lid1);
            }
            if c2 != 0 {
                return (c2, p2, lid2);
            }
            (0, 0, 0)
        } else {
            let (c, p, lid, wrapped, clipped, filled, edge) = sample_for_layer(0);
            if crate::debug_flags::render_metrics() {
                if wrapped {
                    self.dbg_m7_wrap = self.dbg_m7_wrap.saturating_add(1);
                }
                if clipped {
                    self.dbg_m7_clip = self.dbg_m7_clip.saturating_add(1);
                }
                if filled {
                    self.dbg_m7_fill = self.dbg_m7_fill.saturating_add(1);
                }
                if c != 0 {
                    self.dbg_m7_bg1 = self.dbg_m7_bg1.saturating_add(1);
                }
                if edge {
                    self.dbg_m7_edge = self.dbg_m7_edge.saturating_add(1);
                }
            }
            (c, p, lid)
        }
    }

    // Render a single Mode 7 layer for EXTBG mode.
    // desired_layer: 0=BG1, 1=BG2
    #[inline]
    pub(crate) fn render_mode7_single_layer(
        &mut self,
        x: u16,
        y: u16,
        desired_layer: u8,
    ) -> (u32, u8, u8) {
        let (mx, my) = self.apply_mosaic(x, y, desired_layer);
        let sx = if (self.m7sel & 0x01) != 0 {
            255 - (mx as i32)
        } else {
            mx as i32
        };
        let sy = if (self.m7sel & 0x02) != 0 {
            255 - (my as i32)
        } else {
            my as i32
        };
        let (wx, wy) = self.mode7_world_xy_int(sx, sy);
        let repeat_off = (self.m7sel & 0x80) != 0;
        let fill_char0 = (self.m7sel & 0x40) != 0;
        let inside = (0..1024).contains(&wx) && (0..1024).contains(&wy);
        let (ix, iy, outside) = if inside {
            (wx, wy, false)
        } else if !repeat_off {
            (wx & 0x03FF, wy & 0x03FF, false)
        } else {
            (wx, wy, true)
        };
        if outside {
            if !fill_char0 {
                return (0, 0, desired_layer);
            }
            let px = (ix & 7) as u8;
            let py = (iy & 7) as u8;
            return self.sample_mode7_for_layer(0, px, py, desired_layer);
        }
        let tile_x = (ix >> 3) & 0x7F;
        let tile_y = (iy >> 3) & 0x7F;
        let px = (ix & 7) as u8;
        let py = (iy & 7) as u8;
        let map_word = ((tile_y as usize) << 7) | (tile_x as usize);
        let map_index = map_word * 2;
        if map_index >= self.vram.len() {
            return (0, 0, desired_layer);
        }
        let tile_id = self.vram[map_index] as u16;
        self.sample_mode7_for_layer(tile_id, px, py, desired_layer)
    }

    // Color only (legacy callers). Returns (ARGB, priority)
    // SNES Mode 7 tiles are 8x8, 8bpp, linear (64 bytes per tile).
    #[inline]
    pub(crate) fn sample_mode7_color_only(&self, tile_id: u16, px: u8, py: u8) -> (u32, u8) {
        // Mode 7 tile data is stored in the high byte of VRAM words 0x0000..0x3FFF.
        // Treating the high bytes as a contiguous byte array yields 256 tiles * 64 bytes.
        let data_word = ((tile_id as usize) << 6) | ((py as usize) << 3) | (px as usize); // 0..16383
        let addr = data_word * 2 + 1;
        if addr >= self.vram.len() {
            return (0, 0);
        }
        let color_index = self.vram[addr];
        if color_index == 0 {
            return (0, 0);
        }
        // Direct color mode (CGWSEL bit0) for 8bpp BGs; in Mode 7 there are no tilemap palette bits.
        let use_direct_color = (self.cgwsel & 0x01) != 0;
        let color = if use_direct_color {
            self.direct_color_to_rgb(0, color_index)
        } else {
            let palette_index = self.get_bg_palette_index(0, color_index, 8);
            self.cgram_to_rgb(palette_index)
        };
        (color, 1)
    }

    // Sample Mode 7 pixel for a specific layer in EXTBG mode.
    // desired_layer: 0=BG1, 1=BG2
    // BG1: uses full 8-bit color index, single priority level
    // BG2: uses lower 7 bits as color index, bit7 as priority (0 or 1)
    // Both layers sample from the SAME pixel data independently.
    #[inline]
    pub(crate) fn sample_mode7_for_layer(
        &self,
        tile_id: u16,
        px: u8,
        py: u8,
        desired_layer: u8,
    ) -> (u32, u8, u8) {
        let data_word = ((tile_id as usize) << 6) | ((py as usize) << 3) | (px as usize);
        let addr = data_word * 2 + 1;
        if addr >= self.vram.len() {
            return (0, 0, desired_layer);
        }
        let raw = self.vram[addr];
        if desired_layer == 0 {
            // BG1: full 8-bit color, single priority
            if raw == 0 {
                return (0, 0, 0);
            }
            let use_direct_color = (self.cgwsel & 0x01) != 0;
            let color = if use_direct_color {
                self.direct_color_to_rgb(0, raw)
            } else {
                let palette_index = self.get_bg_palette_index(0, raw, 8);
                self.cgram_to_rgb(palette_index)
            };
            (color, 1, 0)
        } else {
            // BG2: lower 7 bits as color, bit7 as priority
            let color_index = raw & 0x7F;
            let priority = (raw >> 7) & 1;
            if color_index == 0 {
                return (0, 0, 1);
            }
            let palette_index = self.get_bg_palette_index(0, color_index, 8);
            let color = self.cgram_to_rgb(palette_index);
            (color, priority, 1)
        }
    }
}
