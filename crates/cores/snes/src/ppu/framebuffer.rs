use super::{trace_sample_dot_config, trace_scanline_state_config, Ppu};

impl Ppu {
    pub fn set_framebuffer_rendering_enabled(&mut self, enabled: bool) {
        self.framebuffer_rendering_enabled = enabled;
    }

    #[allow(dead_code)]
    pub fn framebuffer_rendering_enabled(&self) -> bool {
        self.framebuffer_rendering_enabled
    }

    pub(crate) fn rebuild_presented_framebuffer(&mut self) {
        if !self.framebuffer_rendering_enabled {
            return;
        }

        let saved_scanline = self.scanline;
        let saved_cycle = self.cycle;
        let saved_h_blank = self.h_blank;
        let saved_v_blank = self.v_blank;
        let saved_simd_start = self.brightness_simd_start;
        let saved_simd_len = self.brightness_simd_len;
        let saved_simd_factor = self.brightness_simd_factor;
        let saved_simd_buf = self.brightness_simd_buf;

        self.flush_brightness_simd();
        self.render_framebuffer.fill(0xFF000000);
        self.render_subscreen_buffer.fill(0);

        let vis_lines = self.get_visible_height();
        for y in 1..=vis_lines {
            self.scanline = y;
            self.update_line_render_state();
            self.prepare_line_obj_pipeline(y);
            self.prepare_line_window_luts();
            self.prepare_line_opt_luts();
            self.render_scanline();
        }

        self.flush_brightness_simd();
        std::mem::swap(&mut self.framebuffer, &mut self.render_framebuffer);
        std::mem::swap(
            &mut self.subscreen_buffer,
            &mut self.render_subscreen_buffer,
        );
        self.render_framebuffer.fill(0xFF000000);
        self.render_subscreen_buffer.fill(0);

        self.scanline = saved_scanline;
        self.cycle = saved_cycle;
        self.h_blank = saved_h_blank;
        self.v_blank = saved_v_blank;
        self.brightness_simd_start = saved_simd_start;
        self.brightness_simd_len = saved_simd_len;
        self.brightness_simd_factor = saved_simd_factor;
        self.brightness_simd_buf = saved_simd_buf;
        self.update_line_render_state();
    }

    // Render one pixel at the current (x,y)
    pub(super) fn render_dot(&mut self, x: usize, y: usize) {
        // y = original scanline number (1-based; scanline 0 is skipped)
        // fb_y = framebuffer row (0-based: scanline 1 → row 0)
        let fb_y = y.wrapping_sub(1);
        if x == 0 {
            // Ensure any pending brightness batch from the previous line is flushed.
            self.flush_brightness_simd();
        }
        if x == 0 && y == 1 && crate::debug_flags::trace_disp_frame(self.frame) {
            eprintln!(
                "[DISP-STATE] frame={} INIDISP=0x{:02X} blank={} bright={} BGMODE={} TM=0x{:02X} NMI_en={} bg1h={} bg1v={}",
                self.frame,
                self.screen_display,
                (self.screen_display & 0x80) != 0,
                self.brightness,
                self.bg_mode,
                self.main_screen_designation,
                self.nmi_enabled,
                self.bg1_hscroll,
                self.bg1_vscroll,
            );
        }
        if x == 0 {
            if let Some(cfg) = trace_scanline_state_config() {
                let y_u16 = y as u16;
                if self.frame >= cfg.frame_min
                    && self.frame <= cfg.frame_max
                    && y_u16 >= cfg.y_min
                    && y_u16 <= cfg.y_max
                {
                    let forced_blank = (self.screen_display & 0x80) != 0;
                    let effective_tm = self.effective_main_screen_designation();
                    let w12sel =
                        ((self.window_bg_mask[1] & 0x0F) << 4) | (self.window_bg_mask[0] & 0x0F);
                    let w34sel =
                        ((self.window_bg_mask[3] & 0x0F) << 4) | (self.window_bg_mask[2] & 0x0F);
                    let wobjsel =
                        ((self.window_color_mask & 0x0F) << 4) | (self.window_obj_mask & 0x0F);
                    let wobjlog =
                        ((self.color_window_logic & 0x03) << 2) | (self.obj_window_logic & 0x03);
                    println!(
                        "[TRACE_SCANLINE_STATE] frame={} y={} BGMODE={} INIDISP=0x{:02X} (blank={} bright={}) TM=0x{:02X} TS=0x{:02X} CGWSEL=0x{:02X} CGADSUB=0x{:02X} WH0={} WH1={} WH2={} WH3={} W12SEL=0x{:02X} W34SEL=0x{:02X} WOBJSEL=0x{:02X} WBGLOG=0x{:02X} WOBJLOG=0x{:02X} TMW=0x{:02X} TSW=0x{:02X} M7SEL=0x{:02X} M7A={} M7B={} M7C={} M7D={} M7CX={} M7CY={} M7HOFS={} M7VOFS={}",
                        self.frame,
                        y,
                        self.bg_mode,
                        self.screen_display,
                        forced_blank as u8,
                        self.brightness & 0x0F,
                        effective_tm,
                        self.sub_screen_designation,
                        self.cgwsel,
                        self.cgadsub,
                        self.window1_left,
                        self.window1_right,
                        self.window2_left,
                        self.window2_right,
                        w12sel,
                        w34sel,
                        wobjsel,
                        (self.bg_window_logic[0] & 0x03)
                            | ((self.bg_window_logic[1] & 0x03) << 2)
                            | ((self.bg_window_logic[2] & 0x03) << 4)
                            | ((self.bg_window_logic[3] & 0x03) << 6),
                        wobjlog,
                        self.tmw_mask,
                        self.tsw_mask,
                        self.m7sel,
                        self.mode7_matrix_a,
                        self.mode7_matrix_b,
                        self.mode7_matrix_c,
                        self.mode7_matrix_d,
                        self.mode7_center_x,
                        self.mode7_center_y,
                        self.mode7_hofs,
                        self.mode7_vofs
                    );
                }
            }
        }

        // Debug at start of each scanline - only when not forced blank
        if x == 0 && crate::debug_flags::debug_render_dot() {
            static mut LINE_DEBUG_COUNT: u32 = 0;
            unsafe {
                let fblank = (self.screen_display & 0x80) != 0;
                if LINE_DEBUG_COUNT < 10 && (!fblank || LINE_DEBUG_COUNT < 3) {
                    LINE_DEBUG_COUNT += 1;
                    let effective = self.effective_main_screen_designation();
                    println!("🎬 RENDER_DOT[{}]: y={} main=0x{:02X} effective=0x{:02X} last_nz=0x{:02X} mode={} bright={} fblank={}",
                        LINE_DEBUG_COUNT, y, self.main_screen_designation, effective,
                        self.main_screen_designation_last_nonzero, self.bg_mode,
                        self.brightness, fblank);
                }
            }

            // Periodic CGRAM contents check (frames 1, 10, 30, 60, 100)
            static mut CGRAM_CHECK_COUNT: u32 = 0;
            unsafe {
                if y == 0 {
                    let frame = self.frame;
                    let should_check = matches!(frame, 1 | 10 | 30 | 60 | 100);
                    if should_check && CGRAM_CHECK_COUNT < 5 {
                        CGRAM_CHECK_COUNT += 1;
                        let mut nonzero_count = 0;
                        let mut first_colors = Vec::new();
                        for i in 0..256 {
                            let lo = self.cgram[i * 2] as u16;
                            let hi = (self.cgram[i * 2 + 1] & 0x7F) as u16;
                            let color = (hi << 8) | lo;
                            if color != 0 {
                                nonzero_count += 1;
                                if first_colors.len() < 8 {
                                    first_colors.push((i, color));
                                }
                            }
                        }
                        println!(
                            "🎨 CGRAM CHECK (frame {}): {} non-zero colors out of 256",
                            frame, nonzero_count
                        );
                        for (idx, color) in &first_colors {
                            // Convert 15-bit BGR color to RGB for display
                            let r = ((color & 0x001F) as u32) << 3;
                            let g = (((color >> 5) & 0x001F) as u32) << 3;
                            let b = (((color >> 10) & 0x001F) as u32) << 3;
                            let rgb = (r << 16) | (g << 8) | b;
                            println!("   Color[{}] = 0x{:04X} (RGB: 0x{:06X})", idx, color, rgb);
                        }
                    }
                }
            }
        }

        // Fast path: forced blank yields black without per-pixel composition.
        if (self.screen_display & 0x80) != 0 && !self.force_display_active() && !self.force_no_blank
        {
            self.flush_brightness_simd();
            let pixel_offset = fb_y * 256 + x;
            if pixel_offset < self.render_framebuffer.len() {
                self.render_framebuffer[pixel_offset] = 0xFF000000;
            }
            if pixel_offset < self.render_subscreen_buffer.len() {
                self.render_subscreen_buffer[pixel_offset] = 0;
            }
            return;
        }

        self.update_obj_time_over_at_x(x as u16);

        // Use existing per-pixel composition with color math and windows.
        let (mut main_color, mut main_layer_id, mut main_obj_math_allowed) =
            self.render_main_screen_pixel_with_layer_cached(x as u16, y as u16);
        let main_transparent = main_color == 0;
        // If main pixel is transparent, treat as backdrop for color math decisions
        if main_color == 0 {
            main_color = self.cgram_to_rgb(0);
            main_layer_id = 5; // Backdrop layer id
            main_obj_math_allowed = true;
        }
        let hires_out = self.line_hires_out;
        let need_subscreen = self.line_need_subscreen;
        let (sub_color, sub_layer_id, sub_transparent, sub_obj_math_allowed) = if need_subscreen {
            self.render_sub_screen_pixel_with_layer_cached(x as u16, y as u16)
        } else {
            (0, 5, true, true)
        };

        if let Some(cfg) = trace_sample_dot_config() {
            if self.frame == cfg.frame && (x as u16) == cfg.x && (y as u16) == cfg.y {
                let x_u = x as u16;
                let y_u = y as u16;
                let bg1_eval =
                    self.evaluate_window_mask(x_u, self.window_bg_mask[0], self.bg_window_logic[0]);
                let bg2_eval =
                    self.evaluate_window_mask(x_u, self.window_bg_mask[1], self.bg_window_logic[1]);
                let bg1_mask = self.should_mask_bg(x_u, 0, true);
                let bg2_mask = self.should_mask_bg(x_u, 1, true);
                let (bg1_color, bg1_pr) = self.render_bg_mode2_with_priority(x_u, y_u, 0);
                let (bg2_color, bg2_pr) = self.render_bg_mode2_with_priority(x_u, y_u, 1);
                println!(
                    "[TRACE_SAMPLE_DOT] frame={} x={} y={} TM=0x{:02X} TMW=0x{:02X} WH0={} WH1={} WH2={} WH3={} W12SEL=0x{:02X} WBGLOG=0x{:02X} w1_in={} w2_in={} bg1_eval={} bg2_eval={} bg1_mask={} bg2_mask={} bg1=(0x{:08X},pr={}) bg2=(0x{:08X},pr={}) main=(0x{:08X},lid={}) sub=(0x{:08X},lid={},t={}) objsz={} objbase=0x{:04X} objgap=0x{:04X}",
                    self.frame,
                    x_u,
                    y_u,
                    self.effective_main_screen_designation(),
                    self.tmw_mask,
                    self.window1_left,
                    self.window1_right,
                    self.window2_left,
                    self.window2_right,
                    ((self.window_bg_mask[1] & 0x0F) << 4) | (self.window_bg_mask[0] & 0x0F),
                    (self.bg_window_logic[0] & 0x03) | ((self.bg_window_logic[1] & 0x03) << 2),
                    self.is_inside_window1(x_u) as u8,
                    self.is_inside_window2(x_u) as u8,
                    bg1_eval as u8,
                    bg2_eval as u8,
                    bg1_mask as u8,
                    bg2_mask as u8,
                    bg1_color,
                    bg1_pr,
                    bg2_color,
                    bg2_pr,
                    main_color,
                    main_layer_id,
                    sub_color,
                    sub_layer_id,
                    sub_transparent as u8,
                    self.sprite_size,
                    self.sprite_name_base,
                    self.sprite_name_select_gap_words()
                );
            }
        }
        let final_color = if hires_out {
            let even_mix = if self.line_color_math_enabled {
                self.apply_color_math_screens(
                    main_color,
                    sub_color,
                    main_layer_id,
                    main_obj_math_allowed,
                    x as u16,
                    y as u16,
                    sub_transparent,
                )
            } else {
                main_color
            };
            let odd_mix = if self.line_color_math_enabled {
                self.apply_color_math_screens(
                    sub_color,
                    main_color,
                    sub_layer_id,
                    sub_obj_math_allowed,
                    x as u16,
                    y as u16,
                    main_transparent,
                )
            } else {
                sub_color
            };
            Self::average_rgb(even_mix, odd_mix)
        } else if !self.line_color_math_enabled {
            main_color
        } else {
            self.apply_color_math_screens(
                main_color,
                sub_color,
                main_layer_id,
                main_obj_math_allowed,
                x as u16,
                y as u16,
                sub_transparent,
            )
        };

        let pixel_offset = fb_y * 256 + x;
        if let Some(cfg) = trace_sample_dot_config() {
            if self.frame == cfg.frame && (x as u16) == cfg.x && (y as u16) == cfg.y {
                println!(
                    "[TRACE_SAMPLE_DOT][FINAL] frame={} x={} y={} main=0x{:08X} sub=0x{:08X} final=0x{:08X} bright_factor={}",
                    self.frame,
                    x as u16,
                    y as u16,
                    main_color,
                    sub_color,
                    final_color,
                    if self.force_display_active() { 15 } else { self.brightness & 0x0F }
                );
            }
        }
        let brightness_factor = if self.force_display_active() {
            15
        } else {
            self.brightness & 0x0F
        };
        if pixel_offset < self.render_framebuffer.len() {
            if brightness_factor >= 15 {
                if self.brightness_simd_len > 0 {
                    self.flush_brightness_simd();
                }
                self.render_framebuffer[pixel_offset] = (final_color & 0x00FF_FFFF) | 0xFF000000;
            } else {
                let expected_next = self.brightness_simd_start + self.brightness_simd_len as usize;
                if self.brightness_simd_len == 0 {
                    self.brightness_simd_start = pixel_offset;
                    self.brightness_simd_factor = brightness_factor;
                } else if self.brightness_simd_factor != brightness_factor
                    || expected_next != pixel_offset
                {
                    self.flush_brightness_simd();
                    self.brightness_simd_start = pixel_offset;
                    self.brightness_simd_factor = brightness_factor;
                }
                if (self.brightness_simd_len as usize) < self.brightness_simd_buf.len() {
                    self.brightness_simd_buf[self.brightness_simd_len as usize] = final_color;
                    self.brightness_simd_len += 1;
                    if self.brightness_simd_len as usize == self.brightness_simd_buf.len() {
                        self.flush_brightness_simd();
                    }
                } else {
                    self.flush_brightness_simd();
                    self.brightness_simd_start = pixel_offset;
                    self.brightness_simd_factor = brightness_factor;
                    self.brightness_simd_buf[0] = final_color;
                    self.brightness_simd_len = 1;
                }
            }
        }
        if need_subscreen && pixel_offset < self.render_subscreen_buffer.len() {
            self.render_subscreen_buffer[pixel_offset] = sub_color;
        }
        if x == 255 {
            self.flush_brightness_simd();
        }
    }

    pub fn get_framebuffer(&self) -> &[u32] {
        &self.framebuffer
    }

    // Mutable framebuffer accessor (debug use only)
    #[allow(dead_code)]
    pub fn get_framebuffer_mut(&mut self) -> &mut [u32] {
        &mut self.framebuffer
    }

    #[inline]
    #[allow(dead_code)]
    pub fn frame(&self) -> u64 {
        self.frame
    }

    // update_mode7_mul_result moved to registers.rs

    /// 現在のフレームバッファが全て黒（0x00FFFFFF=0）かどうか簡易判定
    #[allow(dead_code)]
    pub fn framebuffer_is_all_black(&self) -> bool {
        self.framebuffer.iter().all(|&p| (p & 0x00FF_FFFF) == 0)
    }

    /// フレームバッファを指定色で塗りつぶす（強制フォールバック用）
    #[allow(dead_code)]
    pub fn force_framebuffer_color(&mut self, color: u32) {
        // Fill both the present (front) and render (back) buffers so the forced color
        // remains visible even if the emulator overshoots a frame boundary and swaps.
        self.framebuffer.fill(color);
        self.render_framebuffer.fill(color);
        self.subscreen_buffer.fill(color);
        self.render_subscreen_buffer.fill(color);
    }
}
