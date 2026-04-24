use super::{
    CRAM_COLORS, FRAME_HEIGHT, FRAME_WIDTH, TILE_SIZE_BYTES, VRAM_SIZE, Vdp, highlight_channel,
    md_color_to_rgb888, read_u16_be_wrapped, shadow_channel,
};
use crate::debug_flags;

impl Vdp {
    pub(super) fn render_sprites(&mut self, plane_meta: &[u8]) {
        let max_sat_sprites = if self.h40_mode() { 80usize } else { 64usize };
        let sat_use_line_latched = self.line_vram_latch_enabled
            && (self.debug_sat_flag(Self::DEBUG_SAT_LINE_LATCH_FLAG)
                || debug_flags::sat_line_latch());
        let sat_use_live = self.debug_sat_flag(Self::DEBUG_SAT_LIVE_FLAG)
            || debug_flags::sat_live()
            || !sat_use_line_latched;
        let sat_per_line =
            self.debug_sat_flag(Self::DEBUG_SAT_PER_LINE_FLAG) || debug_flags::sat_per_line();
        let sprite_x_offset = debug_flags::sprite_x_offset();
        let sprite_y_offset = debug_flags::sprite_y_offset();
        if sat_per_line {
            self.render_sprites_per_line(
                plane_meta,
                sat_use_live,
                sprite_x_offset,
                sprite_y_offset,
            );
            return;
        }
        let mut sprites_on_line = [0u8; FRAME_HEIGHT];
        let mut sprite_pixels_on_line = [0u16; FRAME_HEIGHT];
        let mut masked_line = [false; FRAME_HEIGHT];
        let mut sprite_filled = std::mem::take(&mut self.render_sprite_filled);
        if sprite_filled.len() != FRAME_WIDTH * FRAME_HEIGHT {
            sprite_filled.resize(FRAME_WIDTH * FRAME_HEIGHT, false);
        }
        sprite_filled.fill(false);
        let mut index = 0usize;

        for _ in 0..max_sat_sprites {
            let entry_addr = self.sprite_table_base() + index * 8;
            let (mut y_word, mut size_link, mut attr, mut x_word) = {
                // Use live SAT by default; line-latched SAT can be enabled for diagnostics.
                let sat_vram = if sat_use_live {
                    &self.vram
                } else {
                    self.line_vram.first().unwrap_or(&self.vram)
                };
                (
                    read_u16_be_wrapped(sat_vram, entry_addr),
                    read_u16_be_wrapped(sat_vram, entry_addr + 2),
                    read_u16_be_wrapped(sat_vram, entry_addr + 4),
                    read_u16_be_wrapped(sat_vram, entry_addr + 6),
                )
            };
            if sat_use_line_latched {
                let mut y = (y_word & 0x03FF) as i32 - 128;
                if self.interlace_mode_enabled() {
                    y >>= 1;
                }
                let line = y.clamp(0, (FRAME_HEIGHT - 1) as i32) as usize;
                let sat_vram = self.line_vram.get(line).unwrap_or(&self.vram);
                y_word = read_u16_be_wrapped(sat_vram, entry_addr);
                size_link = read_u16_be_wrapped(sat_vram, entry_addr + 2);
                attr = read_u16_be_wrapped(sat_vram, entry_addr + 4);
                x_word = read_u16_be_wrapped(sat_vram, entry_addr + 6);
            }

            self.draw_sprite(
                y_word,
                size_link,
                attr,
                x_word,
                plane_meta,
                &mut sprite_filled,
                &mut masked_line,
                &mut sprites_on_line,
                &mut sprite_pixels_on_line,
                sprite_x_offset,
                sprite_y_offset,
            );

            let link = (size_link & 0x007F) as usize;
            if link == 0 || link == index || link >= max_sat_sprites {
                break;
            }
            index = link;
        }
        self.render_sprite_filled = sprite_filled;
    }

    #[allow(clippy::too_many_arguments)]
    fn render_sprites_per_line(
        &mut self,
        plane_meta: &[u8],
        sat_use_live: bool,
        sprite_x_offset: i32,
        sprite_y_offset: i32,
    ) {
        let swap_size = debug_flags::sprite_swap_size();
        let sprite_pattern_line0 = self.sprite_pattern_line0_enabled();
        let sprite_row_major = debug_flags::sprite_row_major();
        let disable_mask_sprite = debug_flags::disable_sprite_mask();

        let mut sprite_filled = std::mem::take(&mut self.render_sprite_filled);
        if sprite_filled.len() != FRAME_WIDTH * FRAME_HEIGHT {
            sprite_filled.resize(FRAME_WIDTH * FRAME_HEIGHT, false);
        }
        sprite_filled.fill(false);
        let sat_base = self.sprite_table_base();
        for dy in 0..FRAME_HEIGHT {
            let regs = self
                .line_registers
                .get(dy)
                .copied()
                .unwrap_or(self.registers);
            let interlace_mode_2 = Self::interlace_mode_2_from_regs(&regs);
            let interlace_field = if interlace_mode_2 {
                (self.frame_count & 1) as usize
            } else {
                0
            };
            if !Self::display_enabled_from_regs(&regs) {
                continue;
            }
            let line_active_height = Self::active_display_height_from_regs(&regs);
            if dy >= line_active_height {
                continue;
            }
            let line_active_width = Self::active_display_width_from_regs(&regs);
            let (max_sprites_per_line, max_pixels_per_line) = if Self::h40_mode_from_regs(&regs) {
                (20usize, line_active_width)
            } else {
                (16usize, line_active_width)
            };
            let max_sat_sprites = if Self::h40_mode_from_regs(&regs) {
                80usize
            } else {
                64usize
            };
            let sat_vram = if sat_use_live {
                &self.vram
            } else {
                self.line_vram.get(dy).unwrap_or(&self.vram)
            };
            let pattern_vram = if sprite_pattern_line0 {
                if sat_use_live {
                    &self.vram
                } else {
                    self.line_vram.first().unwrap_or(&self.vram)
                }
            } else {
                if sat_use_live {
                    &self.vram
                } else {
                    self.line_vram.get(dy).unwrap_or(&self.vram)
                }
            };

            let mut masked = false;
            let mut line_sprites = 0usize;
            let mut line_pixels = 0usize;
            let mut index = 0usize;
            let mut visited = vec![false; max_sat_sprites];

            for _ in 0..max_sat_sprites {
                if index >= max_sat_sprites || visited[index] {
                    break;
                }
                visited[index] = true;
                let entry_addr = sat_base + index * 8;
                let y_word = read_u16_be_wrapped(sat_vram, entry_addr);
                let size_link = read_u16_be_wrapped(sat_vram, entry_addr + 2);
                let attr = read_u16_be_wrapped(sat_vram, entry_addr + 4);
                let x_word = read_u16_be_wrapped(sat_vram, entry_addr + 6);
                let link = (size_link & 0x007F) as usize;

                let x = (x_word & 0x01FF) as i32 - 128 + sprite_x_offset;
                let mut y = (y_word & 0x03FF) as i32 - 128 + sprite_y_offset;
                if interlace_mode_2 {
                    y >>= 1;
                }
                let is_mask_sprite = (x_word & 0x01FF) == 0 && !disable_mask_sprite;
                let (width_tiles, height_tiles) = if swap_size {
                    (
                        ((size_link >> 8) & 0x3) as usize + 1,
                        ((size_link >> 10) & 0x3) as usize + 1,
                    )
                } else {
                    (
                        ((size_link >> 10) & 0x3) as usize + 1,
                        ((size_link >> 8) & 0x3) as usize + 1,
                    )
                };
                let width_px = width_tiles * 8;
                let height_px = height_tiles * 8;
                let dy_i32 = dy as i32;
                let covered = dy_i32 >= y && dy_i32 < y + height_px as i32;
                if covered {
                    if is_mask_sprite {
                        masked = true;
                    } else if !masked {
                        if line_sprites >= max_sprites_per_line {
                            self.sprite_overflow = true;
                        } else {
                            line_sprites += 1;
                            let sprite_priority_high = (attr & 0x8000) != 0;
                            let tile_base = (attr & 0x07FF) as usize;
                            let palette_line = ((attr >> 13) & 0x3) as usize;
                            let hflip = (attr & 0x0800) != 0;
                            let vflip = (attr & 0x1000) != 0;
                            let line_shadow_highlight =
                                Self::shadow_highlight_mode_from_regs(&regs);
                            let sy = (dy_i32 - y) as usize;
                            let src_y = if vflip { height_px - 1 - sy } else { sy };
                            let tile_row = src_y / 8;
                            let in_tile_y = src_y & 7;
                            let in_tile_y = if interlace_mode_2 {
                                (in_tile_y << 1) | interlace_field
                            } else {
                                in_tile_y
                            };
                            let tile_stride = if interlace_mode_2 {
                                TILE_SIZE_BYTES * 2
                            } else {
                                TILE_SIZE_BYTES
                            };
                            for sx in 0..width_px {
                                if line_pixels >= max_pixels_per_line {
                                    self.sprite_overflow = true;
                                    break;
                                }
                                // Consume sprite dot budget including transparent/offscreen dots.
                                line_pixels += 1;

                                let src_x = if hflip { width_px - 1 - sx } else { sx };
                                let dx = x + sx as i32;
                                if !(0..line_active_width as i32).contains(&dx) {
                                    continue;
                                }
                                let tile_col = src_x / 8;
                                let in_tile_x = src_x & 7;
                                let tile_index = if sprite_row_major {
                                    tile_base + tile_row * width_tiles + tile_col
                                } else {
                                    tile_base + tile_col * height_tiles + tile_row
                                };
                                let tile_addr =
                                    tile_index * tile_stride + in_tile_y * 4 + in_tile_x / 2;
                                let tile_byte = pattern_vram[tile_addr % VRAM_SIZE];
                                let pixel = if in_tile_x & 1 == 0 {
                                    tile_byte >> 4
                                } else {
                                    tile_byte & 0x0F
                                };
                                if pixel == 0 {
                                    continue;
                                }

                                let meta_index = dy * FRAME_WIDTH + dx as usize;
                                let meta = plane_meta[meta_index];
                                let plane_opaque = (meta & 0x01) != 0;
                                let plane_priority_high = (meta & 0x02) != 0;
                                if !sprite_priority_high && plane_opaque && plane_priority_high {
                                    continue;
                                }

                                if line_shadow_highlight
                                    && palette_line == 3
                                    && (pixel == 14 || pixel == 15)
                                {
                                    let plane_ci = ((meta >> 2) & 0x3F) as usize;
                                    let plane_color = self.line_cram[dy][plane_ci % CRAM_COLORS];
                                    let (pr, pg, pb) = md_color_to_rgb888(plane_color);
                                    let out = meta_index * 3;
                                    if pixel == 15 {
                                        self.frame_buffer[out] = shadow_channel(pr);
                                        self.frame_buffer[out + 1] = shadow_channel(pg);
                                        self.frame_buffer[out + 2] = shadow_channel(pb);
                                    } else {
                                        if !plane_priority_high {
                                            self.frame_buffer[out] = pr;
                                            self.frame_buffer[out + 1] = pg;
                                            self.frame_buffer[out + 2] = pb;
                                        } else {
                                            self.frame_buffer[out] = highlight_channel(pr);
                                            self.frame_buffer[out + 1] = highlight_channel(pg);
                                            self.frame_buffer[out + 2] = highlight_channel(pb);
                                        }
                                    }
                                    continue;
                                }

                                let color_index = palette_line * 16 + pixel as usize;
                                let color = self.line_cram[dy][color_index % CRAM_COLORS];
                                let (r, g, b) = md_color_to_rgb888(color);
                                // S/H mode: high-priority sprite → normal,
                                // low-priority sprite → shadow.
                                // Both sprite & plane high priority → highlight.
                                let (r, g, b) = if line_shadow_highlight {
                                    if sprite_priority_high && plane_opaque && plane_priority_high {
                                        (
                                            highlight_channel(r),
                                            highlight_channel(g),
                                            highlight_channel(b),
                                        )
                                    } else if sprite_priority_high {
                                        (r, g, b)
                                    } else {
                                        (shadow_channel(r), shadow_channel(g), shadow_channel(b))
                                    }
                                } else {
                                    (r, g, b)
                                };
                                let out = meta_index * 3;
                                if sprite_filled[meta_index] {
                                    self.sprite_collision = true;
                                    continue;
                                }
                                self.frame_buffer[out] = r;
                                self.frame_buffer[out + 1] = g;
                                self.frame_buffer[out + 2] = b;
                                sprite_filled[meta_index] = true;
                            }
                        }
                    }
                }

                if link == 0 || link == index || link >= max_sat_sprites {
                    break;
                }
                index = link;
            }
        }
        self.render_sprite_filled = sprite_filled;
    }

    fn draw_sprite(
        &mut self,
        y_word: u16,
        size_link: u16,
        attr: u16,
        x_word: u16,
        plane_meta: &[u8],
        sprite_filled: &mut [bool],
        masked_line: &mut [bool; FRAME_HEIGHT],
        sprites_on_line: &mut [u8; FRAME_HEIGHT],
        sprite_pixels_on_line: &mut [u16; FRAME_HEIGHT],
        sprite_x_offset: i32,
        sprite_y_offset: i32,
    ) {
        // Sprite X coordinate is 9-bit (0..511), offset by 128.
        let x = (x_word & 0x01FF) as i32 - 128 + sprite_x_offset;
        let mut y = (y_word & 0x03FF) as i32 - 128 + sprite_y_offset;
        if self.interlace_mode_enabled() {
            y >>= 1;
        }
        let swap_size = debug_flags::sprite_swap_size();
        let (width_tiles, height_tiles) = if swap_size {
            (
                ((size_link >> 8) & 0x3) as usize + 1,
                ((size_link >> 10) & 0x3) as usize + 1,
            )
        } else {
            (
                ((size_link >> 10) & 0x3) as usize + 1,
                ((size_link >> 8) & 0x3) as usize + 1,
            )
        };
        let sprite_priority_high = (attr & 0x8000) != 0;
        let tile_base = (attr & 0x07FF) as usize;
        let palette_line = ((attr >> 13) & 0x3) as usize;
        let hflip = (attr & 0x0800) != 0;
        let vflip = (attr & 0x1000) != 0;
        let width_px = width_tiles * 8;
        let height_px = height_tiles * 8;
        let disable_mask_sprite = debug_flags::disable_sprite_mask();
        let is_mask_sprite = (x_word & 0x01FF) == 0 && !disable_mask_sprite;
        let sprite_pattern_line0 = self.sprite_pattern_line0_enabled();
        let sprite_row_major = debug_flags::sprite_row_major();

        for sy in 0..height_px {
            let src_y = if vflip { height_px - 1 - sy } else { sy };
            let dy = y + sy as i32;
            if !(0..FRAME_HEIGHT as i32).contains(&dy) {
                continue;
            }
            let dy_index = dy as usize;
            let regs = self
                .line_registers
                .get(dy_index)
                .copied()
                .unwrap_or(self.registers);
            if !Self::display_enabled_from_regs(&regs) {
                continue;
            }
            let line_active_height = Self::active_display_height_from_regs(&regs);
            if dy_index >= line_active_height {
                continue;
            }
            let line_active_width = Self::active_display_width_from_regs(&regs);
            let (line_max_sprites_per_line, line_max_pixels_per_line) =
                if Self::h40_mode_from_regs(&regs) {
                    (20usize, line_active_width)
                } else {
                    (16usize, line_active_width)
                };
            let interlace_mode_2 = Self::interlace_mode_2_from_regs(&regs);
            let interlace_field = if interlace_mode_2 {
                (self.frame_count & 1) as usize
            } else {
                0
            };
            let line_shadow_highlight = Self::shadow_highlight_mode_from_regs(&regs);
            if is_mask_sprite {
                masked_line[dy_index] = true;
                continue;
            }
            if masked_line[dy_index] {
                continue;
            }
            if sprites_on_line[dy_index] as usize >= line_max_sprites_per_line {
                self.sprite_overflow = true;
                continue;
            }
            sprites_on_line[dy_index] = sprites_on_line[dy_index].saturating_add(1);

            let tile_row = src_y / 8;
            let in_tile_y = src_y & 7;
            let in_tile_y = if interlace_mode_2 {
                (in_tile_y << 1) | interlace_field
            } else {
                in_tile_y
            };
            let tile_stride = if interlace_mode_2 {
                TILE_SIZE_BYTES * 2
            } else {
                TILE_SIZE_BYTES
            };
            for sx in 0..width_px {
                let src_x = if hflip { width_px - 1 - sx } else { sx };
                let dx = x + sx as i32;
                if sprite_pixels_on_line[dy_index] as usize >= line_max_pixels_per_line {
                    self.sprite_overflow = true;
                    break;
                }
                // VDP line sprite budget is consumed by visible sprite dots,
                // including transparent/offscreen pixels.
                sprite_pixels_on_line[dy_index] = sprite_pixels_on_line[dy_index].saturating_add(1);
                if !(0..line_active_width as i32).contains(&dx) {
                    continue;
                }

                let tile_col = src_x / 8;
                let in_tile_x = src_x & 7;
                let tile_index = if sprite_row_major {
                    // Diagnostic: row-major order.
                    tile_base + tile_row * width_tiles + tile_col
                } else {
                    // Sprite pattern index advances in column-major order on the MD VDP.
                    tile_base + tile_col * height_tiles + tile_row
                };
                let tile_addr = tile_index * tile_stride + in_tile_y * 4 + in_tile_x / 2;
                let tile_byte = {
                    let vram = if self.line_vram_latch_enabled {
                        if sprite_pattern_line0 {
                            self.line_vram.first().unwrap_or(&self.vram)
                        } else {
                            self.line_vram.get(dy_index).unwrap_or(&self.vram)
                        }
                    } else {
                        &self.vram
                    };
                    vram[tile_addr % VRAM_SIZE]
                };
                let pixel = if in_tile_x & 1 == 0 {
                    tile_byte >> 4
                } else {
                    tile_byte & 0x0F
                };
                if pixel == 0 {
                    continue;
                }

                let meta_index = dy as usize * FRAME_WIDTH + dx as usize;
                let meta = plane_meta[meta_index];
                let plane_opaque = (meta & 0x01) != 0;
                let plane_priority_high = (meta & 0x02) != 0;
                if !sprite_priority_high && plane_opaque && plane_priority_high {
                    continue;
                }

                if line_shadow_highlight && palette_line == 3 && (pixel == 14 || pixel == 15) {
                    // S/H control sprites modify brightness of the underlying
                    // plane pixel.  They are transparent — they do NOT occupy
                    // the sprite layer and do NOT trigger collision.
                    let plane_ci = ((meta >> 2) & 0x3F) as usize;
                    let plane_color = self.line_cram[dy_index][plane_ci % CRAM_COLORS];
                    let (pr, pg, pb) = md_color_to_rgb888(plane_color);
                    let out = meta_index * 3;
                    if pixel == 15 {
                        // Shadow control: always shadow the plane color.
                        self.frame_buffer[out] = shadow_channel(pr);
                        self.frame_buffer[out + 1] = shadow_channel(pg);
                        self.frame_buffer[out + 2] = shadow_channel(pb);
                    } else {
                        // Highlight control: shadow→normal, normal→highlight.
                        if !plane_priority_high {
                            // Was shadowed → restore to normal.
                            self.frame_buffer[out] = pr;
                            self.frame_buffer[out + 1] = pg;
                            self.frame_buffer[out + 2] = pb;
                        } else {
                            // Was normal → highlight.
                            self.frame_buffer[out] = highlight_channel(pr);
                            self.frame_buffer[out + 1] = highlight_channel(pg);
                            self.frame_buffer[out + 2] = highlight_channel(pb);
                        }
                    }
                    continue;
                }

                let color_index = palette_line * 16 + pixel as usize;
                let color = self.line_cram[dy_index][color_index % CRAM_COLORS];
                let (r, g, b) = md_color_to_rgb888(color);
                // S/H mode: high-priority sprite → normal,
                // low-priority → shadow, both high → highlight.
                let (r, g, b) = if line_shadow_highlight {
                    if sprite_priority_high && plane_opaque && plane_priority_high {
                        (
                            highlight_channel(r),
                            highlight_channel(g),
                            highlight_channel(b),
                        )
                    } else if sprite_priority_high {
                        (r, g, b)
                    } else {
                        (shadow_channel(r), shadow_channel(g), shadow_channel(b))
                    }
                } else {
                    (r, g, b)
                };
                let out = meta_index * 3;
                if sprite_filled[meta_index] {
                    self.sprite_collision = true;
                    continue;
                }
                self.frame_buffer[out] = r;
                self.frame_buffer[out + 1] = g;
                self.frame_buffer[out + 2] = b;
                sprite_filled[meta_index] = true;
            }
        }
    }
}
