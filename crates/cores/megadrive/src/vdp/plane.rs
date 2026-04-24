use super::*;

impl Vdp {
    #[cfg(test)]
    pub(super) fn nametable_base(&self) -> usize {
        Self::nametable_base_from_regs(&self.registers)
    }

    fn nametable_base_from_regs(regs: &[u8; REG_COUNT]) -> usize {
        ((regs[REG_PLANE_A_NAMETABLE] as usize & 0x38) << 10) % VRAM_SIZE
    }

    fn plane_b_nametable_base_from_regs(regs: &[u8; REG_COUNT]) -> usize {
        ((regs[REG_PLANE_B_NAMETABLE] as usize & 0x07) << 13) % VRAM_SIZE
    }

    pub(super) fn hscroll_table_base_from_regs(regs: &[u8; REG_COUNT]) -> usize {
        ((regs[REG_HSCROLL_TABLE] as usize & 0x3F) << 10) % VRAM_SIZE
    }

    fn window_nametable_base_from_regs(regs: &[u8; REG_COUNT]) -> usize {
        let mask = if Self::h40_mode_from_regs(regs) {
            0x3C
        } else {
            0x3E
        };
        ((regs[REG_WINDOW_NAMETABLE] as usize & mask) << 10) % VRAM_SIZE
    }

    fn plane_tile_dimensions_from_regs(regs: &[u8; REG_COUNT]) -> (usize, usize) {
        let width_code = regs[REG_PLANE_SIZE] & 0x03;
        let height_code = (regs[REG_PLANE_SIZE] >> 4) & 0x03;
        (
            plane_size_code_to_tiles(width_code),
            plane_size_code_to_tiles(height_code),
        )
    }

    fn window_tile_dimensions_from_regs(regs: &[u8; REG_COUNT]) -> (usize, usize) {
        let width_tiles = if Self::h40_mode_from_regs(regs) {
            64
        } else {
            32
        };
        (width_tiles, 32)
    }

    fn sign_extend_11(value: u16) -> i16 {
        let masked = (value & 0x07FF) as i16;
        (masked << 5) >> 5
    }

    fn vscroll_index_for_x_from_regs(regs: &[u8; REG_COUNT], plane: usize, x: usize) -> usize {
        if (regs[11] & 0x04) == 0 {
            return plane;
        }
        ((x / 16) * 2 + plane) % VSRAM_WORDS
    }

    pub(super) fn hscroll_word_index_for_line_from_regs(
        regs: &[u8; REG_COUNT],
        plane: usize,
        y: usize,
    ) -> usize {
        match regs[11] & 0x03 {
            // Full-screen scroll (and reserved mode treated as full-screen).
            0x00 | 0x01 => plane,
            // 8-line strips.
            0x02 => (y / 8) * 2 + plane,
            // Per-line scroll.
            0x03 => y * 2 + plane,
            _ => plane,
        }
    }

    #[inline(always)]
    #[allow(clippy::too_many_arguments)]
    fn sample_plane_pixel_cached(
        &self,
        cache: &mut PlaneTileCache,
        vram: &[u8; VRAM_SIZE],
        base: usize,
        sample_x: usize,
        sample_y: usize,
        plane_width_tiles: usize,
        plane_height_tiles: usize,
        scroll_plane_layout: bool,
        plane_paged_layout: bool,
        plane_paged_xmajor: bool,
        interlace_mode_2: bool,
        interlace_field: usize,
    ) -> (Option<PlaneSample>, bool) {
        let tile_x = (sample_x / 8) % plane_width_tiles.max(1);
        let tile_y = (sample_y / 8) % plane_height_tiles.max(1);
        let in_tile_x = sample_x & 7;

        if !cache.valid || cache.tile_x != tile_x || cache.sample_y != sample_y {
            let name_addr = if scroll_plane_layout {
                self.scroll_plane_name_addr(
                    base,
                    tile_x,
                    tile_y,
                    plane_width_tiles,
                    plane_height_tiles,
                    plane_paged_layout,
                    plane_paged_xmajor,
                )
            } else {
                base + (tile_y * plane_width_tiles + tile_x) * 2
            };
            let entry = read_u16_be_wrapped(vram, name_addr);
            let tile_index = (entry & 0x07FF) as usize;
            let palette_line = ((entry >> 13) & 0x3) as usize;
            let priority_high = (entry & 0x8000) != 0;
            let hflip = (entry & 0x0800) != 0;
            let vflip = (entry & 0x1000) != 0;
            let mut row_in_tile = sample_y & 7;
            if vflip {
                row_in_tile = 7 - row_in_tile;
            }

            let tile_stride = if interlace_mode_2 {
                TILE_SIZE_BYTES * 2
            } else {
                TILE_SIZE_BYTES
            };
            let row_in_tile = if interlace_mode_2 {
                (row_in_tile << 1) | (interlace_field & 1)
            } else {
                row_in_tile
            };
            let tile_row_addr = tile_index * tile_stride + row_in_tile * 4;
            for dst_x in 0..8 {
                let src_x = if hflip { 7 - dst_x } else { dst_x };
                let tile_byte = vram[(tile_row_addr + src_x / 2) % VRAM_SIZE];
                cache.pixels[dst_x] = if src_x & 1 == 0 {
                    tile_byte >> 4
                } else {
                    tile_byte & 0x0F
                };
            }

            cache.valid = true;
            cache.tile_x = tile_x;
            cache.sample_y = sample_y;
            cache.color_base = palette_line * 16;
            cache.priority_high = priority_high;
        }

        let pixel = cache.pixels[in_tile_x];
        if pixel == 0 {
            return (None, cache.priority_high);
        }

        (
            Some(PlaneSample {
                color_index: cache.color_base + pixel as usize,
                opaque: true,
                priority_high: cache.priority_high,
            }),
            cache.priority_high,
        )
    }

    fn scroll_plane_name_addr(
        &self,
        base: usize,
        tile_x: usize,
        tile_y: usize,
        plane_width_tiles: usize,
        plane_height_tiles: usize,
        paged_layout: bool,
        paged_xmajor: bool,
    ) -> usize {
        let wrapped_x = tile_x % plane_width_tiles.max(1);
        let wrapped_y = tile_y % plane_height_tiles.max(1);
        // Optional diagnostic mode: force 32x32-cell paged probing.
        if paged_layout {
            let page_width = plane_width_tiles.max(1).div_ceil(32);
            let page_x = wrapped_x / 32;
            let page_y = wrapped_y / 32;
            let in_page_x = wrapped_x & 31;
            let in_page_y = wrapped_y & 31;
            let page_height = plane_height_tiles.max(1).div_ceil(32);
            let page_index = if paged_xmajor {
                page_x * page_height + page_y
            } else {
                page_y * page_width + page_x
            };
            return base + page_index * 32 * 32 * 2 + (in_page_y * 32 + in_page_x) * 2;
        }
        base + (wrapped_y * plane_width_tiles + wrapped_x) * 2
    }

    fn compose_plane_samples(
        &self,
        front: Option<PlaneSample>,
        back: Option<PlaneSample>,
        ignore_priority: bool,
    ) -> Option<PlaneSample> {
        if ignore_priority {
            return front.or(back);
        }
        match (front, back) {
            (Some(front), Some(back)) => {
                if front.priority_high != back.priority_high {
                    if front.priority_high {
                        Some(front)
                    } else {
                        Some(back)
                    }
                } else {
                    Some(front)
                }
            }
            (Some(front), None) => Some(front),
            (None, Some(back)) => Some(back),
            (None, None) => None,
        }
    }

    fn window_active_at(&self, regs: &[u8; REG_COUNT], x: usize, y: usize) -> bool {
        let active_height = Self::active_display_height_from_regs(regs);
        let active_width = Self::active_display_width_from_regs(regs);
        let hreg = regs[REG_WINDOW_HPOS];
        let vreg = regs[REG_WINDOW_VPOS];
        let hsplit = (((hreg & 0x1F) as usize) * 16).min(active_width);
        let vsplit = (((vreg & 0x1F) as usize) * 8).min(active_height);
        let vactive = if (vreg & 0x80) != 0 {
            y >= vsplit
        } else {
            y < vsplit
        };
        // When an explicit vertical split is defined (vsplit > 0) and
        // the line falls inside the vertical window region, the ENTIRE
        // line uses the window plane (horizontal split is ignored).
        // This matches real hardware behavior where vertical window
        // takes priority over horizontal window.
        if vsplit > 0 && vactive {
            return true;
        }
        let hactive = if (hreg & 0x80) != 0 {
            x >= hsplit
        } else {
            x < hsplit
        };
        hactive && vactive
    }

    fn comix_pretitle_vscroll_swap_active(regs: &[u8; REG_COUNT]) -> bool {
        // Comix Zone uses swapped A/B VSRAM sources during the early pre-title logo scene
        // (32x32 plane setup with per-line hscroll). Later title rollout uses normal mapping.
        regs[REG_PLANE_B_NAMETABLE] == 0x07
            && (regs[REG_MODE_SET_2] & 0x40) != 0
            && regs[REG_HSCROLL_TABLE] == 0x3C
            && regs[REG_PLANE_SIZE] == 0x01
            && regs[11] == 0x03
    }

    fn comix_title_roll_active(regs: &[u8; REG_COUNT], vsram: &[u16; VSRAM_WORDS]) -> bool {
        if !(regs[REG_PLANE_B_NAMETABLE] == 0x07
            && (regs[REG_MODE_SET_2] & 0x40) != 0
            && regs[REG_HSCROLL_TABLE] == 0x3C
            && regs[REG_PLANE_SIZE] == 0x11
            && regs[11] == 0x00
            && (regs[12] & 0x08) != 0)
        {
            return false;
        }
        // During the roll-down animation, VSRAM has non-zero scroll values
        // (the roll effect is driven by H-INT updating scroll per line).
        // On the static start menu, all VSRAM entries are zero.
        // Require at least one non-zero entry to avoid false-positive
        // sparse mask clipping on the start menu.
        vsram.iter().any(|&v| v != 0)
    }

    /// For the plane B nametable, compute the first pixel row whose nametable
    /// entries overlap with the HSCROLL table in VRAM.  Returns `None` if there
    /// is no overlap.
    fn plane_b_hscroll_overlap_pixel_row(regs: &[u8; REG_COUNT]) -> Option<usize> {
        let plane_b_base = Self::plane_b_nametable_base_from_regs(regs);
        let hscroll_base = Self::hscroll_table_base_from_regs(regs);
        let (plane_width_tiles, plane_height_tiles) = Self::plane_tile_dimensions_from_regs(regs);
        if plane_width_tiles == 0 {
            return None;
        }
        let row_bytes = plane_width_tiles * 2;
        let plane_size_bytes = row_bytes * plane_height_tiles;
        let plane_end = plane_b_base + plane_size_bytes;
        if hscroll_base >= plane_b_base && hscroll_base < plane_end {
            let overlap_tile_row = (hscroll_base - plane_b_base) / row_bytes;
            Some(overlap_tile_row * 8)
        } else {
            None
        }
    }

    pub(super) fn render_frame(&mut self) {
        self.sprite_collision = false;
        self.sprite_overflow = false;

        // Mode 4 (SMS compatibility): active when Mode 5 bit is clear and
        // H40 mode is not set (H40 is a Mode 5-only feature).
        if !Self::mode5_enabled_from_regs(&self.registers)
            && !Self::h40_mode_from_regs(&self.registers)
        {
            self.render_frame_mode4();
            return;
        }

        let disable_plane_a = debug_flags::disable_plane_a();
        let disable_plane_b = debug_flags::disable_plane_b();
        let disable_window = debug_flags::disable_window() || debug_flags::force_window_off();
        let disable_sprites =
            debug_flags::disable_sprites() || debug_flags::force_disable_sprites();
        let invert_vscroll_a = debug_flags::invert_vscroll_a();
        let invert_vscroll_b = debug_flags::invert_vscroll_b();
        let debug_swap_vscroll_ab = debug_flags::vscroll_swap_ab();
        let plane_paged_layout = debug_flags::plane_paged();
        let plane_paged_layout_a = plane_paged_layout || debug_flags::plane_a_paged();
        let plane_paged_layout_b = plane_paged_layout || debug_flags::plane_b_paged();
        let plane_paged_xmajor = debug_flags::plane_paged_xmajor();
        let plane_paged_xmajor_a = plane_paged_xmajor || debug_flags::plane_a_paged_xmajor();
        let plane_paged_xmajor_b = plane_paged_xmajor || debug_flags::plane_b_paged_xmajor();
        let force_plane_live_vram = debug_flags::plane_live_vram();
        let use_plane_line_latch = self.line_vram_latch_enabled && debug_flags::plane_line_latch();
        let live_cram = debug_flags::live_cram();
        let line_offset = debug_flags::line_offset();
        let bottom_bg_mask = debug_flags::bottom_bg_mask();
        let hscroll_live = debug_flags::hscroll_live();
        let disable_comix_roll_fix = debug_flags::disable_comix_roll_fix();
        let comix_roll_offset = debug_flags::comix_roll_y();
        let disable_comix_roll_sparse_mask = debug_flags::disable_comix_roll_sparse_mask();
        let ignore_plane_priority = debug_flags::ignore_plane_priority();
        let mut plane_meta = std::mem::take(&mut self.render_plane_meta);
        if plane_meta.len() != FRAME_WIDTH * FRAME_HEIGHT {
            plane_meta.resize(FRAME_WIDTH * FRAME_HEIGHT, 0);
        }
        plane_meta.fill(0);
        let mut line_plane_b_opaque_pixels = [0usize; FRAME_HEIGHT];
        let mut comix_title_roll_any = false;
        let mut comix_title_roll_active_height = 0usize;
        for y in 0..FRAME_HEIGHT {
            let line_idx = y
                .saturating_add_signed(line_offset)
                .min(FRAME_HEIGHT.saturating_sub(1));
            let regs = self
                .line_registers
                .get(line_idx)
                .copied()
                .unwrap_or(self.registers);
            let vsram = self.line_vsram.get(line_idx).copied().unwrap_or(self.vsram);
            let hscroll_words = self
                .line_hscroll
                .get(line_idx)
                .copied()
                .unwrap_or_else(|| self.current_line_hscroll_words(y, &regs));
            let hscroll_words = if hscroll_live {
                self.current_line_hscroll_words(line_idx, &regs)
            } else {
                hscroll_words
            };
            let cram = if live_cram {
                self.cram
            } else {
                self.line_cram.get(line_idx).copied().unwrap_or(self.cram)
            };
            let vram = if use_plane_line_latch && !force_plane_live_vram {
                self.line_vram.get(line_idx).unwrap_or(&self.vram)
            } else {
                &self.vram
            };
            let row = y * FRAME_WIDTH * 3;
            if !Self::display_enabled_from_regs(&regs) {
                self.frame_buffer[row..row + FRAME_WIDTH * 3].fill(0);
                continue;
            }
            let line_active_height = Self::active_display_height_from_regs(&regs);
            if y >= line_active_height {
                self.frame_buffer[row..row + FRAME_WIDTH * 3].fill(0);
                continue;
            }

            let line_active_width = Self::active_display_width_from_regs(&regs);
            let plane_a_base = Self::nametable_base_from_regs(&regs);
            let plane_b_base = Self::plane_b_nametable_base_from_regs(&regs);
            let window_base = Self::window_nametable_base_from_regs(&regs);
            let (plane_width_tiles, plane_height_tiles) =
                Self::plane_tile_dimensions_from_regs(&regs);
            let (window_width_tiles, window_height_tiles) =
                Self::window_tile_dimensions_from_regs(&regs);
            let plane_width_px = plane_width_tiles * 8;
            let plane_height_px = plane_height_tiles * 8;
            let window_width_px = window_width_tiles * 8;
            let window_height_px = window_height_tiles * 8;
            let bg_color_index = Self::background_color_index_from_regs(&regs);
            let interlace_mode_2 = Self::interlace_mode_2_from_regs(&regs);
            let interlace_field = if interlace_mode_2 {
                (self.frame_count & 1) as usize
            } else {
                0
            };

            let a_hscroll =
                normalize_scroll(Self::sign_extend_11(hscroll_words[0]), plane_width_px);
            let b_hscroll =
                normalize_scroll(Self::sign_extend_11(hscroll_words[1]), plane_width_px);
            let comix_swap_fix_active = Self::comix_pretitle_vscroll_swap_active(&regs)
                || Self::comix_title_roll_active(&regs, &vsram);
            let comix_title_roll = comix_swap_fix_active
                && Self::comix_title_roll_active(&regs, &vsram)
                && !disable_comix_roll_fix;
            // Suppress plane B pixels whose nametable entries fall inside the
            // HSCROLL table region.  This is computed from the actual register
            // values each line (independent of comix_title_roll_active) so that
            // mid-frame register changes from H-INT don't create gaps.
            let comix_roll_overlap_limit = if comix_swap_fix_active {
                Self::plane_b_hscroll_overlap_pixel_row(&regs)
            } else {
                None
            };
            let swap_vscroll_ab =
                debug_swap_vscroll_ab || Self::comix_pretitle_vscroll_swap_active(&regs);
            if comix_title_roll {
                comix_title_roll_any = true;
                comix_title_roll_active_height = line_active_height;
            }
            let mut line_b_opaque = 0usize;
            let mut plane_a_tile_cache = PlaneTileCache::default();
            let mut plane_b_tile_cache = PlaneTileCache::default();
            let mut window_tile_cache = PlaneTileCache::default();

            for x in 0..FRAME_WIDTH {
                if x >= line_active_width {
                    let out = row + x * 3;
                    self.frame_buffer[out] = 0;
                    self.frame_buffer[out + 1] = 0;
                    self.frame_buffer[out + 2] = 0;
                    continue;
                }
                let (a_idx, b_idx) = if swap_vscroll_ab {
                    (1usize, 0usize)
                } else {
                    (0usize, 1usize)
                };
                let a_vscroll_raw = Self::sign_extend_11(
                    vsram[Self::vscroll_index_for_x_from_regs(&regs, a_idx, x) % VSRAM_WORDS],
                );
                let b_vscroll_raw = Self::sign_extend_11(
                    vsram[Self::vscroll_index_for_x_from_regs(&regs, b_idx, x) % VSRAM_WORDS],
                );
                let a_vscroll_raw = if interlace_mode_2 {
                    a_vscroll_raw >> 1
                } else {
                    a_vscroll_raw
                };
                let b_vscroll_raw = if interlace_mode_2 {
                    b_vscroll_raw >> 1
                } else {
                    b_vscroll_raw
                };
                let a_vscroll = normalize_scroll(a_vscroll_raw, plane_height_px);
                let b_vscroll = normalize_scroll(b_vscroll_raw, plane_height_px);
                let (plane_b, plane_b_raw_pri) = if disable_plane_b {
                    (None, false)
                } else {
                    let mut sample_y = if invert_vscroll_b {
                        (y + plane_height_px - b_vscroll) % plane_height_px
                    } else {
                        (y + b_vscroll) % plane_height_px
                    };
                    if comix_title_roll {
                        sample_y = (sample_y as isize + comix_roll_offset as isize)
                            .rem_euclid(plane_height_px as isize)
                            as usize;
                    }
                    if comix_roll_overlap_limit.map_or(false, |limit| sample_y >= limit) {
                        (None, false)
                    } else {
                        self.sample_plane_pixel_cached(
                            &mut plane_b_tile_cache,
                            vram,
                            plane_b_base,
                            (x + plane_width_px - b_hscroll) % plane_width_px,
                            sample_y,
                            plane_width_tiles,
                            plane_height_tiles,
                            true,
                            plane_paged_layout_b,
                            plane_paged_xmajor_b,
                            interlace_mode_2,
                            interlace_field,
                        )
                    }
                };
                if plane_b.is_some() {
                    line_b_opaque = line_b_opaque.saturating_add(1);
                }

                let (front_plane, front_raw_pri) =
                    if !disable_window && self.window_active_at(&regs, x, y) {
                        self.sample_plane_pixel_cached(
                            &mut window_tile_cache,
                            vram,
                            window_base,
                            x % window_width_px,
                            y % window_height_px,
                            window_width_tiles,
                            window_height_tiles,
                            false,
                            false,
                            false,
                            interlace_mode_2,
                            interlace_field,
                        )
                    } else {
                        let sample_y = if invert_vscroll_a {
                            (y + plane_height_px - a_vscroll) % plane_height_px
                        } else {
                            (y + a_vscroll) % plane_height_px
                        };
                        self.sample_plane_pixel_cached(
                            &mut plane_a_tile_cache,
                            vram,
                            plane_a_base,
                            (x + plane_width_px - a_hscroll) % plane_width_px,
                            sample_y,
                            plane_width_tiles,
                            plane_height_tiles,
                            true,
                            plane_paged_layout_a,
                            plane_paged_xmajor_a,
                            interlace_mode_2,
                            interlace_field,
                        )
                    };
                let front_plane = if disable_plane_a { None } else { front_plane };

                let mut composed =
                    self.compose_plane_samples(front_plane, plane_b, ignore_plane_priority);
                if bottom_bg_mask && y >= line_active_height.saturating_sub(32) {
                    composed = None;
                }
                let color_index = composed
                    .map(|sample| sample.color_index)
                    .unwrap_or(bg_color_index);
                let color = cram[color_index % CRAM_COLORS];
                let (r, g, b) = md_color_to_rgb888(color);

                // Shadow/Highlight mode: if ANY plane has priority set at this
                // pixel (even if transparent), the pixel is at normal brightness.
                // Otherwise it is shadowed.
                let line_sh = Self::shadow_highlight_mode_from_regs(&regs);
                let any_plane_priority = front_raw_pri || plane_b_raw_pri;
                let (r, g, b) = if line_sh && !any_plane_priority {
                    (shadow_channel(r), shadow_channel(g), shadow_channel(b))
                } else {
                    (r, g, b)
                };

                let out = row + x * 3;
                self.frame_buffer[out] = r;
                self.frame_buffer[out + 1] = g;
                self.frame_buffer[out + 2] = b;

                let meta_index = y * FRAME_WIDTH + x;
                // Encode: bit 0 = opaque, bit 1 = composed pixel priority
                // (for sprite vs plane ordering), bits 2..7 = color_index.
                // Note: S/H uses any_plane_priority (OR of raw priorities)
                // which is computed separately above and not stored here.
                let ci = (color_index as u8) & 0x3F;
                let opaque = composed.map(|s| s.opaque).unwrap_or(false);
                let composed_pri = composed.map(|s| s.priority_high).unwrap_or(false);
                plane_meta[meta_index] = (opaque as u8) | ((composed_pri as u8) << 1) | (ci << 2);
            }
            if comix_title_roll && !disable_comix_roll_sparse_mask {
                line_plane_b_opaque_pixels[y] = line_b_opaque;
            }
        }

        if comix_title_roll_any && !disable_comix_roll_sparse_mask {
            let min_pixels = debug_flags::comix_roll_min_pixels();
            let run_required = debug_flags::comix_roll_min_run().max(1);
            let search_start = (comix_title_roll_active_height / 3).max(48);
            let search_end = comix_title_roll_active_height.min(FRAME_HEIGHT);
            let mut run = 0usize;
            let mut clip_start = None;
            for y in search_start..search_end {
                if line_plane_b_opaque_pixels[y] < min_pixels {
                    run = run.saturating_add(1);
                    if run >= run_required {
                        clip_start = Some(y + 1 - run_required);
                        break;
                    }
                } else {
                    run = 0;
                }
            }
            if let Some(start) = clip_start {
                for y in start..search_end {
                    let row = y * FRAME_WIDTH * 3;
                    self.frame_buffer[row..row + FRAME_WIDTH * 3].fill(0);
                    plane_meta[y * FRAME_WIDTH..(y + 1) * FRAME_WIDTH].fill(0);
                }
            }
        }

        if !disable_sprites {
            self.render_sprites(&plane_meta);
        }
        self.render_plane_meta = plane_meta;
    }
}
