use super::Ppu;

impl Ppu {
    pub fn step(&mut self, cycles: u16) {
        // Per-CPU-cycle PPU stepping (approx 1 CPU cycle -> 1 PPU dot)
        let first_hblank = self.first_hblank_dot();
        let first_visible = self.first_visible_dot();
        let render_enabled = self.framebuffer_rendering_enabled;
        let mut vis_lines = self.get_visible_height();
        let mut vblank_start = self.vblank_start_line();
        for _ in 0..cycles {
            // Advance any deferred control effects before processing this dot
            self.tick_deferred_ctrl_effects();
            let x = self.cycle;
            let y = self.scanline;

            // Update HBlank state from dot counters.
            //
            // Official burn-in tests (HVBJOY/VH FLAG) expect $4212 bit6 (HBlank) to be set only
            // for the right-side blanking period. Do not treat the pre-visible dots as "HBlank"
            // for this flag.
            let hblank_now = x >= first_hblank;
            if hblank_now != self.h_blank {
                self.h_blank = hblank_now;
                if hblank_now && x == first_hblank {
                    // Entering right-side HBlank; guard a few dots at HBlank head for HDMA operations only.
                    let guard = crate::debug_flags::hblank_hdma_guard_dots();
                    self.hdma_head_busy_until = first_hblank.saturating_add(guard);
                }
            }

            // Start-of-line duties
            if x == 0 {
                // Commit latched regs at the beginning of each scanline
                self.commit_latched_display_regs();
                self.update_line_render_state();
                // Visible height depends on display regs (e.g., overscan) latched at line start.
                vis_lines = self.get_visible_height();
                vblank_start = self.vblank_start_line();
                if render_enabled && y < vis_lines {
                    // Prepare window LUTs at line start (OBJ list is prepared during previous HBlank)
                    self.prepare_line_window_luts();
                    self.prepare_line_opt_luts();
                    if self.line_sprites.is_empty() {
                        // Skip sprite evaluation if no sprites are present on this scanline.
                        self.line_main_has_obj = false;
                        self.line_sub_has_obj = false;
                    }
                    // Mode 7 flicker debug: log matrix state at scanline 50
                    if crate::debug_flags::trace_m7_scanline() && y == 50 && self.bg_mode == 7 {
                        eprintln!(
                            "[M7-SL50] frame={} A={} B={} C={} D={} HOFS={} VOFS={} CX={} CY={} latch=0x{:02X}",
                            self.frame,
                            self.mode7_matrix_a, self.mode7_matrix_b,
                            self.mode7_matrix_c, self.mode7_matrix_d,
                            self.mode7_hofs, self.mode7_vofs,
                            self.mode7_center_x, self.mode7_center_y,
                            self.mode7_latch
                        );
                    }
                }
            }

            // After guard period, commit any pending control registers (VMADD/CGADD)
            if self.h_blank && x == self.hdma_head_busy_until {
                self.commit_pending_ctrl_if_any();
            }

            // Visible pixel render (scanline 0 is not visible on real hardware)
            if !self.v_blank && y >= 1 && y <= vis_lines && x >= first_visible && x < first_hblank {
                let fb_x = (x - first_visible) as usize;
                if (y - 1) < 239 && render_enabled {
                    self.render_dot(fb_x, y as usize);
                }
            }

            // Advance dot; end-of-line at DOTS_PER_LINE
            let dots_per_line = self.dots_per_line();
            self.cycle += 1;
            if self.cycle >= dots_per_line {
                // End of scanline
                self.cycle = 0;
                self.h_blank = false; // dot 0 is not treated as HBlank for HVBJOY
                self.scanline = self.scanline.wrapping_add(1);

                // VBlank transitions
                // 通常: 可視領域終了の次のラインでVBlank突入
                if !self.v_blank && self.scanline == vblank_start {
                    if crate::debug_flags::boot_verbose() {
                        println!("📺 ENTERING VBLANK at scanline {}", self.scanline);
                    }
                    self.enter_vblank();
                } else if self.scanline == self.scanlines_per_frame() {
                    // NTSC frame end (coarse). Wrap to next frame.
                    if crate::debug_flags::boot_verbose() {
                        println!(
                            "📺 FRAME END: scanline {}, resetting to 0",
                            self.scanlines_per_frame()
                        );
                    }
                    // Present the last completed 256x224 frame before the next frame starts
                    // overwriting the top scanlines. This avoids visible tearing when the
                    // outer loop overshoots the boundary at instruction granularity.
                    if render_enabled {
                        std::mem::swap(&mut self.framebuffer, &mut self.render_framebuffer);
                        std::mem::swap(
                            &mut self.subscreen_buffer,
                            &mut self.render_subscreen_buffer,
                        );
                        // The next frame only redraws the visible region. Clear the back
                        // buffers here so overscan / non-rendered border pixels do not
                        // retain stale colors from older frames.
                        self.render_framebuffer.fill(0xFF000000);
                        self.render_subscreen_buffer.fill(0);
                    }
                    self.exit_vblank();
                    self.scanline = 0;
                    self.frame = self.frame.wrapping_add(1);
                    // Prepare first visible line sprites ahead (scanline 0)
                    self.prepare_line_obj_pipeline(0);
                } else {
                    // Prepare next visible scanline sprites during HBlank end
                    let ny = self.scanline;
                    if ny < vis_lines {
                        let vy = ny;
                        self.prepare_line_obj_pipeline(vy);
                    }
                }
            }

            // External HV latch via WRIO ($4201 bit7 1->0): latch occurs 1 dot later than $2137.
            // SLHV ($2137) latches 1 dot after the read, so WRIO latch uses 2 dots.
            if self.wio_latch_pending_dots > 0 {
                self.wio_latch_pending_dots = self.wio_latch_pending_dots.saturating_sub(1);
                if self.wio_latch_pending_dots == 0 {
                    self.latch_hv_counters();
                }
            }
            if self.slhv_latch_pending_dots > 0 {
                self.slhv_latch_pending_dots = self.slhv_latch_pending_dots.saturating_sub(1);
                if self.slhv_latch_pending_dots == 0 {
                    self.latch_hv_counters();
                }
            }
        }
    }
}
