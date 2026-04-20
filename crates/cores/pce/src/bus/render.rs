use crate::vdc::{
    FRAME_HEIGHT, FRAME_WIDTH, SPRITE_COUNT, SPRITE_PATTERN_HEIGHT, SPRITE_PATTERN_WIDTH,
    SPRITE_PATTERN_WORDS, TILE_HEIGHT, TILE_WIDTH, VDC_CTRL_ENABLE_BACKGROUND_LEGACY,
    VDC_CTRL_ENABLE_SPRITES_LEGACY, VDC_STATUS_OR, Vdc,
};

use super::Bus;

impl Bus {
    pub(crate) fn render_frame_from_vram(&mut self) {
        let (display_height, y_offset) = self.compute_display_height();
        self.current_display_height = display_height;
        self.current_display_y_offset = y_offset;
        self.vdc.clear_frame_trigger();
        let force_bg_only = Self::env_debug_bg_only();
        let force_spr_only = Self::env_debug_spr_only();
        let mut background_line_enabled = [false; FRAME_HEIGHT];
        let mut sprite_line_enabled = [false; FRAME_HEIGHT];
        let mut active_window_line = [false; FRAME_HEIGHT];
        let mut line_display_starts = [0usize; FRAME_HEIGHT];
        let mut line_display_widths = [0usize; FRAME_HEIGHT];
        let mut frame_x_offset = FRAME_WIDTH;
        let mut frame_x_end = 0usize;
        for y in 0..FRAME_HEIGHT {
            let line_idx = self.vdc.line_state_index_for_frame_row(y);
            let line_start = self
                .vdc
                .display_start_for_line(line_idx)
                .min(FRAME_WIDTH - 1);
            let line_width = self
                .vdc
                .display_width_for_line(line_idx)
                .max(1)
                .min(FRAME_WIDTH.saturating_sub(line_start));
            line_display_starts[y] = line_start;
            line_display_widths[y] = line_width;
            if y >= y_offset && y < y_offset.saturating_add(display_height) {
                frame_x_offset = frame_x_offset.min(line_start);
                frame_x_end = frame_x_end.max(line_start.saturating_add(line_width));
            }
            let in_active_window = self.vdc.output_row_in_active_window(y);
            active_window_line[y] = in_active_window;
            if !in_active_window {
                continue;
            }
            let ctrl = self.vdc.control_values_for_line(line_idx);
            let force_display_on = Self::env_force_display_on();
            let mut sprites_enabled =
                (ctrl & VDC_CTRL_ENABLE_SPRITES_LEGACY) != 0 || force_display_on;
            let mut background_enabled =
                (ctrl & VDC_CTRL_ENABLE_BACKGROUND_LEGACY) != 0 || force_display_on;
            if force_bg_only {
                sprites_enabled = false;
                background_enabled = true;
            }
            if force_spr_only {
                sprites_enabled = true;
                background_enabled = false;
            }
            background_line_enabled[y] = background_enabled;
            sprite_line_enabled[y] = sprites_enabled;
        }
        if frame_x_offset >= FRAME_WIDTH || frame_x_end <= frame_x_offset {
            frame_x_offset = 0;
            frame_x_end = line_display_widths
                .iter()
                .copied()
                .max()
                .unwrap_or(256)
                .min(FRAME_WIDTH);
        }
        let display_width = frame_x_end.saturating_sub(frame_x_offset).max(1);
        self.current_display_x_offset = super::types::TransientUsize(frame_x_offset);
        self.current_display_width = display_width;
        let any_bg = background_line_enabled.iter().any(|&e| e);
        let any_spr = sprite_line_enabled.iter().any(|&e| e);

        // Track burst-mode transitions: when the VDC goes through burst mode
        // (both BG and SPR off) and then enters SPR-only, the game is
        // preparing a new scene.  Sprite rendering is suppressed until BG is
        // re-enabled so that partially-loaded content doesn't flash.
        // Games that enter SPR-only WITHOUT a preceding burst (e.g.
        // Bikkuriman World result screen) render sprites normally.
        if !any_bg && !any_spr {
            // Burst mode: enter transition state.
            *self.burst_transition = true;
        } else if any_bg {
            // BG active: scene is ready, clear transition.
            *self.burst_transition = false;
        }

        if !any_bg && !any_spr {
            // Burst mode: both BG and SPR are disabled for the entire frame.
            // The VDC does not drive pixel data — the screen is black.
            for y in 0..FRAME_HEIGHT {
                let row_start = y * FRAME_WIDTH;
                self.framebuffer[row_start + frame_x_offset..row_start + frame_x_end]
                    .fill(0xFF000000);
            }
            self.apply_vce_palette_flicker(&line_display_starts, &line_display_widths);
            self.frame_ready = true;
            return;
        }

        if self.vdc.vram.is_empty() {
            let background_colour = self.vce.palette_rgb(0);
            let overscan_colour = self.vce.palette_rgb(0x100);
            for y in 0..FRAME_HEIGHT {
                let row_start = y * FRAME_WIDTH;
                let row_end = row_start + frame_x_end;
                self.framebuffer[row_start + frame_x_offset..row_end].fill(overscan_colour);
                if active_window_line[y] {
                    let line_start = row_start + line_display_starts[y];
                    self.framebuffer[line_start..line_start + line_display_widths[y]]
                        .fill(background_colour);
                }
            }
            self.apply_vce_palette_flicker(&line_display_starts, &line_display_widths);
            self.frame_ready = true;
            return;
        }

        #[derive(Clone, Copy, Default)]
        struct TileSample {
            chr0: u16,
            chr1: u16,
            tile_base: usize,
            palette_base: usize,
            priority: bool,
        }

        self.bg_opaque.fill(false);
        self.bg_priority.fill(false);
        for count in self.sprite_line_counts.iter_mut() {
            *count = 0;
        }
        self.vdc.clear_sprite_overflow();

        let background_colour = if *self.burst_transition {
            // During burst→SPR-only transition, the game is preparing a new
            // scene (loading VRAM/palettes).  Use black backdrop so that the
            // intermediate VCE palette[0] value doesn't flash on screen.
            0xFF000000
        } else {
            self.vce.palette_rgb(0)
        };
        let overscan_colour = self.vce.palette_rgb(0x100);
        for y in 0..FRAME_HEIGHT {
            let row_start = y * FRAME_WIDTH;
            self.framebuffer[row_start + frame_x_offset..row_start + frame_x_end]
                .fill(overscan_colour);
        }
        if Self::env_force_test_palette() {
            // デバッグ: パレットを簡易グラデーションに初期化
            for i in 0..self.vce.palette.len() {
                let v = i as u16;
                if let Some(slot) = self.vce.palette.get_mut(i) {
                    *slot = ((v & 0x0F) << 8) | ((v >> 4) & 0x0F) << 4 | (v & 0x0F);
                }
            }
        }
        if Self::env_force_palette_every_frame() {
            for i in 0..self.vce.palette.len() {
                let v = (i as u16) & 0x3FF;
                if let Some(slot) = self.vce.palette.get_mut(i) {
                    *slot = ((v & 0x0F) << 8) | (((v >> 4) & 0x0F) << 4) | (v & 0x0F);
                }
            }
        }
        if background_line_enabled.iter().any(|&enabled| enabled) {
            let mut tile_cache: Vec<TileSample> =
                Vec::with_capacity((display_width / TILE_WIDTH) + 2);
            let (map_width_tiles, map_height_tiles) = self.vdc.map_dimensions();
            let map_width = Self::env_bg_map_width_override()
                .unwrap_or(map_width_tiles)
                .max(1);
            let map_height = Self::env_bg_map_height_override()
                .unwrap_or(map_height_tiles)
                .max(1);
            let mwr = self.vdc.registers[0x09] as usize;
            let cg_mode_bit = (mwr >> 7) & 0x01;
            let pixel_width_mode = mwr & 0x03;
            let restrict_planes = pixel_width_mode == 0x03;
            let vram_mask = self.vdc.vram.len().saturating_sub(1);
            let vram_byte_mask = self.vdc.vram.len().saturating_mul(2).saturating_sub(1);
            let plane_major = Self::env_bg_plane_major();

            for y in 0..FRAME_HEIGHT {
                let line_display_start = line_display_starts[y];
                let line_display_width = line_display_widths[y];
                let line_state_index = self.vdc.line_state_index_for_frame_row(y);
                if !background_line_enabled[y] {
                    // BG disabled on this line: VCE palette[0] backdrop.
                    // When BG is off the VDC doesn't drive BG pixel data;
                    // the VCE fills with palette[0] (per MAME huc6260).
                    // Sprites may overlay on top if SPR is enabled.
                    let row_start = y * FRAME_WIDTH;
                    let row_end = row_start + line_display_start + line_display_width;
                    self.framebuffer[row_start + line_display_start..row_end]
                        .fill(background_colour);
                    continue;
                }
                let _active_row = self.vdc.active_row_for_output_row(y).unwrap_or(0);
                if Self::env_force_test_palette() {
                    // パレットを毎行クリアして強制表示色を維持
                    for i in 0..self.vce.palette.len() {
                        let v = i as u16;
                        if let Some(slot) = self.vce.palette.get_mut(i) {
                            *slot = ((v & 0x0F) << 8) | (((v >> 4) & 0x0F) << 4) | (v & 0x0F);
                        }
                    }
                }
                let (x_scroll, y_scroll, y_offset) =
                    self.vdc.scroll_values_for_line(line_state_index);
                let (zoom_x_raw, zoom_y_raw) = self.vdc.zoom_values_for_line(line_state_index);
                let step_x = Vdc::zoom_step_value(zoom_x_raw);
                let step_y = Vdc::zoom_step_value(zoom_y_raw);
                // BG Y scroll: y_scroll is the latched BYR value, y_offset
                // is the number of active lines since BYR was last set.
                // The offset (not active_row) is used so that mid-frame
                // BYR writes (split-screen) produce the correct Y position.
                let y_origin_bias = 0i32;
                let effective_y_scroll = y_scroll as i32;
                let vram = &self.vdc.vram;
                let read_vram_byte = |byte_addr: usize| -> u8 {
                    let word = vram[(byte_addr >> 1) & vram_mask];
                    if (byte_addr & 1) == 0 {
                        (word & 0x00FF) as u8
                    } else {
                        (word >> 8) as u8
                    }
                };
                let swap_words = Self::env_bg_swap_words();
                let swap_bytes = Self::env_bg_swap_bytes();
                let bit_lsb = Self::env_bg_bit_lsb();
                let start_x_fp = (x_scroll as usize) << 4;
                let sample_y_fp =
                    ((effective_y_scroll + y_origin_bias) << 4) + (step_y as i32 * y_offset as i32);
                let sample_y = {
                    let raw = (sample_y_fp >> 4) + Self::env_bg_y_bias();
                    raw.rem_euclid((map_height * TILE_HEIGHT) as i32) as usize
                };
                let tile_row = (sample_y / TILE_HEIGHT) % map_height;
                let line_in_tile = (sample_y % TILE_HEIGHT) as usize;
                let start_sample_x = start_x_fp >> 4;
                let start_tile_int = start_sample_x / TILE_WIDTH;
                let end_sample_x_fp = start_x_fp + step_x * (line_display_width - 1);
                let end_sample_x = (end_sample_x_fp >> 4) + 1;
                let end_tile_int = (end_sample_x + TILE_WIDTH - 1) / TILE_WIDTH;
                let mut tiles_needed = end_tile_int.saturating_sub(start_tile_int) + 2;
                tiles_needed = tiles_needed.max(1);

                tile_cache.clear();
                tile_cache.reserve(tiles_needed);

                for tile_offset in 0..tiles_needed {
                    let tile_col = (start_tile_int + tile_offset) % map_width;
                    let map_addr = {
                        let raw = self.vdc.map_entry_address(tile_row, tile_col) as i32
                            + Self::env_bg_map_base_bias();
                        raw.rem_euclid(self.vdc.vram.len() as i32) as usize
                    };
                    let tile_entry = vram.get(map_addr & vram_mask).copied().unwrap_or(0);
                    let tile_mask = if Self::env_bg_tile12() {
                        0x0FFF
                    } else {
                        0x07FF
                    };
                    let tile_id = (tile_entry & tile_mask) as usize;
                    let palette_bank = ((tile_entry >> 12) & 0x0F) as usize;
                    let tile_base = ((tile_id as i32 * 16 + Self::env_bg_tile_base_bias())
                        .rem_euclid(self.vdc.vram.len() as i32))
                        as usize;
                    let row_index = line_in_tile;
                    let (row_addr_a, row_addr_b) = if Self::env_bg_row_words() {
                        let a = (tile_base + row_index * 2) & vram_mask;
                        (a, (a + 1) & vram_mask)
                    } else {
                        let a = (tile_base + row_index) & vram_mask;
                        (a, (a + 8) & vram_mask)
                    };
                    let mut chr_a = vram.get(row_addr_a).copied().unwrap_or(0);
                    let mut chr_b = vram.get(row_addr_b).copied().unwrap_or(0);
                    if swap_words {
                        std::mem::swap(&mut chr_a, &mut chr_b);
                    }
                    if Self::env_bg_force_chr0_only() {
                        chr_b = 0;
                    }
                    if Self::env_bg_force_chr1_only() {
                        chr_a = 0;
                    }
                    if Self::env_bg_force_tile0_zero() && tile_id == 0 {
                        chr_a = 0;
                        chr_b = 0;
                    }
                    if restrict_planes {
                        if cg_mode_bit == 0 {
                            chr_b = 0;
                        } else {
                            chr_a = 0;
                        }
                    }
                    tile_cache.push(TileSample {
                        chr0: chr_a,
                        chr1: chr_b,
                        tile_base,
                        palette_base: (palette_bank << 4) & 0x1F0,
                        priority: !Self::env_bg_tile12() && (tile_entry & 0x0800) != 0,
                    });
                }

                let mut sample_x_fp = start_x_fp;
                let start_tile_int = start_tile_int;
                for x in 0..line_display_width {
                    let screen_index = y * FRAME_WIDTH + line_display_start + x;
                    let sample_x = (sample_x_fp >> 4) as usize;
                    let tile_idx_int = sample_x / TILE_WIDTH;
                    let tile_offset = tile_idx_int.saturating_sub(start_tile_int);
                    let sample = tile_cache.get(tile_offset).copied().unwrap_or_default();
                    let intra_tile_x = sample_x % TILE_WIDTH;
                    let bit_index = intra_tile_x;
                    let shift = if bit_lsb { bit_index } else { 7 - bit_index };
                    let (plane0, plane1, plane2, plane3) = if plane_major {
                        let base_byte = (sample.tile_base << 1) & vram_byte_mask;
                        let row = line_in_tile;
                        let mut planes = [
                            read_vram_byte((base_byte + row) & vram_byte_mask),
                            read_vram_byte((base_byte + 8 + row) & vram_byte_mask),
                            read_vram_byte((base_byte + 16 + row) & vram_byte_mask),
                            read_vram_byte((base_byte + 24 + row) & vram_byte_mask),
                        ];
                        if swap_words {
                            planes.swap(0, 2);
                            planes.swap(1, 3);
                        }
                        if swap_bytes {
                            planes.swap(0, 1);
                            planes.swap(2, 3);
                        }
                        if restrict_planes {
                            if cg_mode_bit == 0 {
                                planes[2] = 0;
                                planes[3] = 0;
                            } else {
                                planes[0] = 0;
                                planes[1] = 0;
                            }
                        }
                        (
                            ((planes[0] >> shift) & 0x01) as u8,
                            ((planes[1] >> shift) & 0x01) as u8,
                            ((planes[2] >> shift) & 0x01) as u8,
                            ((planes[3] >> shift) & 0x01) as u8,
                        )
                    } else if swap_bytes {
                        (
                            ((sample.chr0 >> (shift + 8)) & 0x01) as u8,
                            ((sample.chr0 >> shift) & 0x01) as u8,
                            ((sample.chr1 >> (shift + 8)) & 0x01) as u8,
                            ((sample.chr1 >> shift) & 0x01) as u8,
                        )
                    } else {
                        (
                            ((sample.chr0 >> shift) & 0x01) as u8,
                            ((sample.chr0 >> (shift + 8)) & 0x01) as u8,
                            ((sample.chr1 >> shift) & 0x01) as u8,
                            ((sample.chr1 >> (shift + 8)) & 0x01) as u8,
                        )
                    };
                    let pixel = plane0 | (plane1 << 1) | (plane2 << 2) | (plane3 << 3);
                    if pixel == 0 {
                        if Self::env_bg_palette_zero_visible() {
                            let colour_idx = sample.palette_base & 0x1FF;
                            self.framebuffer[screen_index] = self.vce.palette_rgb(colour_idx);
                        } else {
                            self.framebuffer[screen_index] = background_colour;
                        }
                    } else {
                        self.bg_opaque[screen_index] = true;
                        self.bg_priority[screen_index] = sample.priority;
                        let colour_idx = (sample.palette_base | pixel as usize) & 0x1FF;
                        self.framebuffer[screen_index] = self.vce.palette_rgb(colour_idx);
                    }
                    sample_x_fp += step_x;
                }
            }
        } else {
            // No BG-enabled lines — fill with VCE palette[0] backdrop.
            // This covers sprite-only mode (BG off, SPR on) where the VDC
            // doesn't generate BG pixels; the VCE provides palette[0].
            for y in 0..FRAME_HEIGHT {
                let row_start = y * FRAME_WIDTH;
                let line_start = line_display_starts[y];
                self.framebuffer
                    [row_start + line_start..row_start + line_start + line_display_widths[y]]
                    .fill(background_colour);
            }
        }
        if any_spr && !*self.burst_transition {
            self.render_sprites(
                &sprite_line_enabled,
                &line_display_starts,
                &line_display_widths,
            );
        }
        self.apply_vce_palette_flicker(&line_display_starts, &line_display_widths);

        self.frame_ready = true;
    }

    fn apply_vce_palette_flicker(
        &mut self,
        line_display_starts: &[usize; FRAME_HEIGHT],
        line_display_widths: &[usize; FRAME_HEIGHT],
    ) {
        for event in self.vce_palette_flicker.0.drain(..) {
            if event.row >= FRAME_HEIGHT {
                continue;
            }
            let row_start = line_display_starts[event.row];
            let row_width = line_display_widths[event.row];
            let row_end = row_start.saturating_add(row_width);
            if event.x < row_start || event.x >= row_end {
                continue;
            }
            let row_base = event.row * FRAME_WIDTH;
            let smear_colour = if event.x <= row_start {
                self.framebuffer[row_base + row_start]
            } else {
                self.framebuffer[row_base + event.x - 1]
            };
            let end = (event.x + event.len).min(row_end);
            for x in event.x..end {
                self.framebuffer[row_base + x] = smear_colour;
            }
        }
    }

    fn render_sprites(
        &mut self,
        line_enabled: &[bool; FRAME_HEIGHT],
        line_display_starts: &[usize; FRAME_HEIGHT],
        line_display_widths: &[usize; FRAME_HEIGHT],
    ) {
        if self.vdc.vram.is_empty() {
            return;
        }
        #[derive(Clone, Copy)]
        struct LineSprite {
            x: i32,
            visible_width: usize,
            full_width: usize,
            src_tile_y: usize,
            row_in_tile: usize,
            pattern_base_index: usize,
            palette_base: usize,
            high_priority: bool,
            h_flip: bool,
            use_upper_cg_pair: bool,
        }

        let vram = &self.vdc.vram;
        let vram_mask = vram.len().saturating_sub(1);
        let mut overflow_detected = false;
        let mwr = self.vdc.registers[0x09];
        let sprite_dot_period = (mwr >> 2) & 0x03;
        let cg_mode_enabled = sprite_dot_period == 0x01;
        let reverse_priority = Self::env_sprite_reverse_priority();
        let no_sprite_line_limit = Self::env_no_sprite_line_limit();
        let pattern_raw_index = Self::env_sprite_pattern_raw_index();
        let row_interleaved = Self::env_sprite_row_interleaved();
        let sprite_max_entries = Self::env_sprite_max_entries().unwrap_or(SPRITE_COUNT);

        for dest_row in 0..FRAME_HEIGHT {
            if !line_enabled[dest_row] {
                continue;
            }
            let Some(active_row) = self.vdc.active_row_for_output_row(dest_row) else {
                continue;
            };
            let line_display_start = line_display_starts[dest_row] as i32;
            let mut line_sprites = Vec::with_capacity(16);
            let mut slots_used = 0u8;
            let scanline_y = active_row as i32;

            for sprite_idx in 0..SPRITE_COUNT.min(sprite_max_entries) {
                let sprite = if reverse_priority {
                    SPRITE_COUNT - 1 - sprite_idx
                } else {
                    sprite_idx
                };
                let base = sprite * 4;
                let y_word = self.vdc.satb.get(base).copied().unwrap_or(0);
                let x_word = self.vdc.satb.get(base + 1).copied().unwrap_or(0);
                let pattern_word = self.vdc.satb.get(base + 2).copied().unwrap_or(0);
                let attr_word = self.vdc.satb.get(base + 3).copied().unwrap_or(0);

                // MAME sprite Y: src_y = (m_current_segment_start - sat_y) & 0x3FF
                // m_current_segment_start = 0x40 at first active line.
                // So sat_y=64 (0x40) → src_y=0 → first row at display row 0.
                // Screen Y = sat_y - 64 (no +1; the -1 in "raster_count - 1"
                // is already factored into m_current_segment_start).
                let y = (y_word & 0x03FF) as i32 - 64;
                let x = (x_word & 0x03FF) as i32 - 32 + line_display_start;
                let width_cells = if (attr_word & 0x0100) != 0 {
                    2usize
                } else {
                    1usize
                };
                let height_code = ((attr_word >> 12) & 0x03) as usize;
                let height_cells = match height_code {
                    0 => 1,
                    1 => 2,
                    _ => 4,
                };
                let full_width = width_cells * SPRITE_PATTERN_WIDTH;
                let full_height = height_cells * SPRITE_PATTERN_HEIGHT;
                if scanline_y < y || scanline_y >= y + full_height as i32 {
                    continue;
                }

                if !no_sprite_line_limit && slots_used >= 16 {
                    overflow_detected = true;
                    continue;
                }
                // MAME: accepted sprites always render full width even when
                // pushing the slot count past 16 (a 32px sprite at slot 15
                // uses slots 15+16 and renders both cells fully).
                slots_used = slots_used.saturating_add(width_cells as u8);

                let mut pattern_base_index = if pattern_raw_index {
                    (pattern_word & 0x03FF) as usize
                } else {
                    ((pattern_word >> 1) & 0x03FF) as usize
                };
                if width_cells == 2 {
                    pattern_base_index &= !0x0001;
                }
                // MAME: each height-code bit independently masks a pattern bit.
                //   cgy bit 0 → mask pattern bit 1
                //   cgy bit 1 → mask pattern bit 2
                if height_code & 1 != 0 {
                    pattern_base_index &= !0x0002;
                }
                if height_code & 2 != 0 {
                    pattern_base_index &= !0x0004;
                }

                let v_flip = (attr_word & 0x8000) != 0;
                let local_y = (scanline_y - y) as usize;
                let src_y = if v_flip {
                    full_height - 1 - local_y
                } else {
                    local_y
                };
                let src_tile_y = src_y / SPRITE_PATTERN_HEIGHT;
                let row_in_tile = src_y % SPRITE_PATTERN_HEIGHT;

                line_sprites.push(LineSprite {
                    x,
                    visible_width: full_width,
                    full_width,
                    src_tile_y,
                    row_in_tile,
                    pattern_base_index,
                    palette_base: 0x100usize | (((attr_word & 0x000F) as usize) << 4),
                    high_priority: (attr_word & 0x0080) != 0,
                    h_flip: (attr_word & 0x0800) != 0,
                    use_upper_cg_pair: (pattern_word & 0x0001) != 0,
                });
            }

            self.sprite_line_counts[dest_row] = slots_used;

            let line_display_start = line_display_starts[dest_row];
            let line_display_width = line_display_widths[dest_row];
            let line_display_end = line_display_start + line_display_width;
            for screen_x in line_display_start..line_display_end {
                let offset = dest_row * FRAME_WIDTH + screen_x;
                for sprite in line_sprites.iter() {
                    if (screen_x as i32) < sprite.x
                        || (screen_x as i32) >= sprite.x + sprite.visible_width as i32
                    {
                        continue;
                    }

                    let local_x = (screen_x as i32 - sprite.x) as usize;
                    let src_x = if sprite.h_flip {
                        sprite.full_width - 1 - local_x
                    } else {
                        local_x
                    };
                    let src_tile_x = src_x / SPRITE_PATTERN_WIDTH;
                    let col_in_tile = src_x % SPRITE_PATTERN_WIDTH;
                    let pattern_index =
                        sprite.pattern_base_index + sprite.src_tile_y * 2 + src_tile_x;
                    let pattern_base = (pattern_index * SPRITE_PATTERN_WORDS) & vram_mask;

                    let (plane0_word, plane1_word, plane2_word, plane3_word) = if row_interleaved {
                        let row_base = (pattern_base + sprite.row_in_tile * 4) & vram_mask;
                        (
                            vram[row_base],
                            vram[(row_base + 1) & vram_mask],
                            vram[(row_base + 2) & vram_mask],
                            vram[(row_base + 3) & vram_mask],
                        )
                    } else {
                        (
                            vram[(pattern_base + sprite.row_in_tile) & vram_mask],
                            vram[(pattern_base + 16 + sprite.row_in_tile) & vram_mask],
                            vram[(pattern_base + 32 + sprite.row_in_tile) & vram_mask],
                            vram[(pattern_base + 48 + sprite.row_in_tile) & vram_mask],
                        )
                    };
                    let shift = 15usize.saturating_sub(col_in_tile);
                    let mut plane0 = ((plane0_word >> shift) & 0x01) as u8;
                    let mut plane1 = ((plane1_word >> shift) & 0x01) as u8;
                    let mut plane2 = ((plane2_word >> shift) & 0x01) as u8;
                    let mut plane3 = ((plane3_word >> shift) & 0x01) as u8;

                    if cg_mode_enabled {
                        if sprite.use_upper_cg_pair {
                            plane0 = plane2;
                            plane1 = plane3;
                            plane2 = 0;
                            plane3 = 0;
                        } else {
                            plane2 = 0;
                            plane3 = 0;
                        }
                    }

                    let pixel = plane0 | (plane1 << 1) | (plane2 << 2) | (plane3 << 3);
                    if pixel == 0 {
                        continue;
                    }

                    let bg_opaque = self.bg_opaque[offset];
                    let bg_forces_front = self.bg_priority[offset];
                    if !bg_opaque || (sprite.high_priority && !bg_forces_front) {
                        let colour_index = (sprite.palette_base | pixel as usize) & 0x1FF;
                        self.framebuffer[offset] = self.vce.palette_rgb(colour_index);
                    }
                    // The first opaque sprite pixel wins, regardless of BG blend result.
                    break;
                }
            }
        }

        if overflow_detected {
            self.vdc.raise_status(VDC_STATUS_OR);
        }
    }
}
