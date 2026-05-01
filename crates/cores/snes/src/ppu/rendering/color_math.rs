use crate::ppu::{trace_sample_dot_config, Ppu};
use wide::u32x8;

impl Ppu {
    #[allow(dead_code)]
    pub(crate) fn apply_hires_enhancement(&self, color: u32) -> u32 {
        // 高解像度モード用の色調整（鮮明度向上）
        let r = ((color >> 16) & 0xFF) as u8;
        let g = ((color >> 8) & 0xFF) as u8;
        let b = (color & 0xFF) as u8;

        // 軽微な彩度向上
        let enhanced_r = ((r as u16 * 110 / 100).min(255)) as u8;
        let enhanced_g = ((g as u16 * 110 / 100).min(255)) as u8;
        let enhanced_b = ((b as u16 * 110 / 100).min(255)) as u8;

        0xFF000000 | ((enhanced_r as u32) << 16) | ((enhanced_g as u32) << 8) | (enhanced_b as u32)
    }

    pub(crate) fn apply_brightness(&self, color: u32) -> u32 {
        // Forced blank overrides everything (unless FORCE_DISPLAY or FORCE_NO_BLANK)
        if (self.screen_display & 0x80) != 0 && !self.force_display_active() && !self.force_no_blank
        {
            return 0xFF000000;
        }
        // Apply INIDISP brightness level (0..15). 15 = full.
        let factor = if self.force_display_active() {
            15
        } else {
            (self.brightness as u32).min(15)
        };
        if factor >= 15 {
            return (color & 0x00FFFFFF) | 0xFF000000;
        }
        let r = ((((color >> 16) & 0xFF) * factor / 15) & 0xFF) << 16;
        let g = ((((color >> 8) & 0xFF) * factor / 15) & 0xFF) << 8;
        let b = ((color & 0xFF) * factor / 15) & 0xFF;
        0xFF000000 | r | g | b
    }

    #[inline]
    pub(crate) fn apply_brightness_with_factor(color: u32, factor: u8) -> u32 {
        let factor = (factor as u32).min(15);
        if factor >= 15 {
            return (color & 0x00FF_FFFF) | 0xFF000000;
        }
        let r = ((((color >> 16) & 0xFF) * factor / 15) & 0xFF) << 16;
        let g = ((((color >> 8) & 0xFF) * factor / 15) & 0xFF) << 8;
        let b = ((color & 0xFF) * factor / 15) & 0xFF;
        0xFF000000 | r | g | b
    }

    #[inline]
    pub(crate) fn apply_brightness_simd_block(colors: [u32; 8], factor: u8) -> [u32; 8] {
        let factor = (factor as u32).min(15);
        if factor >= 15 {
            return colors.map(|c| (c & 0x00FF_FFFF) | 0xFF000000);
        }
        let v = u32x8::from(colors);
        let mask = u32x8::splat(0xFF);
        let r = (v >> u32x8::splat(16)) & mask;
        let g = (v >> u32x8::splat(8)) & mask;
        let b = v & mask;
        let f = u32x8::splat(factor);
        let recip = u32x8::splat(0x8889); // exact for 0..3825 when >> 19
        let shift = u32x8::splat(19);
        let r = ((r * f) * recip) >> shift;
        let g = ((g * f) * recip) >> shift;
        let b = ((b * f) * recip) >> shift;
        let out =
            u32x8::splat(0xFF000000) | (r * u32x8::splat(1 << 16)) | (g * u32x8::splat(1 << 8)) | b;
        out.into()
    }

    #[inline]
    pub(crate) fn flush_brightness_simd(&mut self) {
        let len = self.brightness_simd_len as usize;
        if len == 0 {
            return;
        }
        let start = self.brightness_simd_start;
        let factor = self.brightness_simd_factor;
        if start + len <= self.render_framebuffer.len() {
            if len == self.brightness_simd_buf.len() {
                let out = Self::apply_brightness_simd_block(self.brightness_simd_buf, factor);
                self.render_framebuffer[start..start + len].copy_from_slice(&out);
            } else {
                for i in 0..len {
                    let color = self.brightness_simd_buf[i];
                    self.render_framebuffer[start + i] =
                        Self::apply_brightness_with_factor(color, factor);
                }
            }
        }
        self.brightness_simd_len = 0;
    }

    #[inline]
    pub(crate) fn average_rgb(a: u32, b: u32) -> u32 {
        let ar = ((a >> 16) & 0xFF) as u16;
        let ag = ((a >> 8) & 0xFF) as u16;
        let ab = (a & 0xFF) as u16;
        let br = ((b >> 16) & 0xFF) as u16;
        let bg = ((b >> 8) & 0xFF) as u16;
        let bb = (b & 0xFF) as u16;
        let r = ((ar + br) / 2) as u32;
        let g = ((ag + bg) / 2) as u32;
        let bl = ((ab + bb) / 2) as u32;
        0xFF000000 | (r << 16) | (g << 8) | bl
    }

    // カラー演算機能
    #[allow(dead_code)]
    pub(crate) fn apply_color_math(&self, main_color: u32, layer_id: u8) -> u32 {
        if !self.is_color_math_enabled(layer_id) {
            return main_color;
        }
        // Select subsource: CGWSEL bit1 (1=subscreen, 0=fixed)
        let sub_color = self.fixed_color_to_rgb();
        // Use CGADSUB for add/sub + halve
        let is_addition = (self.cgadsub & 0x80) == 0;
        let halve = (self.cgadsub & 0x40) != 0;
        self.blend_colors(main_color, sub_color, is_addition, halve)
    }

    pub(crate) fn is_color_math_enabled(&self, layer_id: u8) -> bool {
        let bit_mask = Self::color_math_layer_bit(layer_id);
        bit_mask != 0 && (self.cgadsub & bit_mask) != 0
    }

    #[inline]
    pub(crate) fn color_math_layer_bit(layer_id: u8) -> u8 {
        // レイヤーIDに対応するビットをチェック
        match layer_id {
            0 => 0x01, // BG1
            1 => 0x02, // BG2
            2 => 0x04, // BG3
            3 => 0x08, // BG4
            4 => 0x10, // Sprite
            5 => 0x20, // Backdrop
            _ => 0,
        }
    }

    pub(crate) fn fixed_color_to_rgb(&self) -> u32 {
        let r = (self.fixed_color & 0x1F) as u8;
        let g = ((self.fixed_color >> 5) & 0x1F) as u8;
        let b = ((self.fixed_color >> 10) & 0x1F) as u8;

        // 5bitから8bitに拡張
        let r = (r << 3) | (r >> 2);
        let g = (g << 3) | (g >> 2);
        let b = (b << 3) | (b >> 2);

        ((r as u32) << 16) | ((g as u32) << 8) | (b as u32) | 0xFF000000
    }

    pub(crate) fn blend_colors(
        &self,
        color1: u32,
        color2: u32,
        is_addition: bool,
        halve: bool,
    ) -> u32 {
        // Work in 5-bit space (SNES BGR555), then expand to 8-bit.
        //
        // SNES color math order (per docs):
        //   1) main +/- sub/fixed
        //   2) optional /2 (half color math)
        //   3) clamp to 0..31
        //
        // Doing saturating add before halving breaks Add+Half (it would cap at 31, then /2,
        // incorrectly darkening the result). Keep the full sum first, then halve, then clamp.
        let r1 = (((color1 >> 16) & 0xFF) as i16) >> 3;
        let g1 = (((color1 >> 8) & 0xFF) as i16) >> 3;
        let b1 = ((color1 & 0xFF) as i16) >> 3;

        let r2 = (((color2 >> 16) & 0xFF) as i16) >> 3;
        let g2 = (((color2 >> 8) & 0xFF) as i16) >> 3;
        let b2 = ((color2 & 0xFF) as i16) >> 3;

        let (mut r, mut g, mut b) = if is_addition {
            (r1 + r2, g1 + g2, b1 + b2)
        } else {
            (r1 - r2, g1 - g2, b1 - b2)
        };

        if halve {
            // Arithmetic shift matches hardware's divide-by-2 behavior for signed intermediates.
            r >>= 1;
            g >>= 1;
            b >>= 1;
        }

        r = r.clamp(0, 31);
        g = g.clamp(0, 31);
        b = b.clamp(0, 31);

        // Expand back to 8-bit (x<<3 | x>>2 format)
        let r8 = ((r as u32) << 3) | ((r as u32) >> 2);
        let g8 = ((g as u32) << 3) | ((g as u32) >> 2);
        let b8 = ((b as u32) << 3) | ((b as u32) >> 2);
        0xFF000000 | (r8 << 16) | (g8 << 8) | b8
    }

    // メイン・サブスクリーン間のカラー演算（対象レイヤ限定の簡易版）
    pub(crate) fn apply_color_math_screens(
        &mut self,
        main_color_in: u32,
        sub_color_in: u32,
        main_layer_id: u8,
        main_obj_math_allowed: bool,
        x: u16,
        y: u16,
        sub_transparent: bool,
    ) -> u32 {
        let force_display = self.force_display_active();
        // Forced blank produces black regardless of color math (unless FORCE_DISPLAY)
        if (self.screen_display & 0x80) != 0 && !force_display {
            return 0;
        }
        // Fast path: no window regions and no color-math enabled
        if (self.cgwsel & 0xF0) == 0 && (self.cgadsub & 0x3F) == 0 {
            return main_color_in;
        }

        // CGWSEL region types (MM / SS): 0=nowhere, 1=outside, 2=inside, 3=everywhere.
        let mm = (self.cgwsel >> 6) & 0x03; // main screen black region
        let ss = (self.cgwsel >> 4) & 0x03; // sub screen transparent region
        let layer_math_bit = Self::color_math_layer_bit(main_layer_id);
        let math_enabled = layer_math_bit != 0 && (self.cgadsub & layer_math_bit) != 0;
        let obj_math_blocked = main_layer_id == 4 && !main_obj_math_allowed;
        if mm == 0 && ss == 0 && (obj_math_blocked || !math_enabled) {
            return main_color_in;
        }

        // Color window W(x): 1=inside, 0=outside.
        // If the color window is disabled (WOBJSEL upper nibble = 0), do not apply MM/SS.
        let win = if mm == 0 && ss == 0 {
            false
        } else if self.window_color_mask == 0 {
            false
        } else if self.line_window_prepared {
            self.color_window_lut.get(x as usize).copied().unwrap_or(0) != 0
        } else {
            self.evaluate_window_mask(x, self.window_color_mask, self.color_window_logic)
        };

        let region_hit = |mode: u8, inside: bool| -> bool {
            match mode {
                0 => false,
                1 => !inside, // outside
                2 => inside,  // inside
                _ => true,    // everywhere
            }
        };

        // Apply main-screen black region *before* color math, but do not suppress color math.
        // (Real hardware still performs color math even when the main screen is replaced with black.)
        // NOTE: When no color windows are enabled (WOBJSEL upper nibble = 0), all pixels are
        // considered "outside" the window (win=false). The MM/SS region checks must still be
        // evaluated because modes 1 (outside window) and 3 (everywhere) apply even when no
        // windows are configured.
        let mut main_color = main_color_in;
        if region_hit(mm, win) && !force_display {
            if crate::debug_flags::render_metrics() {
                if mm == 1 {
                    self.dbg_clip_outside = self.dbg_clip_outside.saturating_add(1);
                    if main_layer_id == 4 {
                        self.dbg_clip_obj_outside = self.dbg_clip_obj_outside.saturating_add(1);
                    }
                } else if mm == 2 {
                    self.dbg_clip_inside = self.dbg_clip_inside.saturating_add(1);
                    if main_layer_id == 4 {
                        self.dbg_clip_obj_inside = self.dbg_clip_obj_inside.saturating_add(1);
                    }
                }
            }
            main_color = 0xFF000000;
        }

        // Mask color math via the sub-screen transparent region.
        // When active, output the (possibly black-clipped) main screen color unchanged.
        if region_hit(ss, win) && !force_display {
            if let Some(cfg) = trace_sample_dot_config() {
                if self.frame == cfg.frame && x == cfg.x && y == cfg.y {
                    println!(
                        "[TRACE_SAMPLE_DOT][MATH] frame={} x={} y={} mode=skip_by_ss win={} mm={} ss={} main_lid={} main=0x{:08X} sub=0x{:08X} sub_transparent={}",
                        self.frame,
                        x,
                        y,
                        win as u8,
                        mm,
                        ss,
                        main_layer_id,
                        main_color,
                        sub_color_in,
                        sub_transparent as u8
                    );
                }
            }
            return main_color;
        }

        // OBJ color math is disabled for palettes 0-3 on real hardware.
        if obj_math_blocked {
            return main_color;
        }

        // このメインレイヤにカラー演算が許可されているか
        if !math_enabled {
            if crate::debug_flags::render_metrics() {
                self.dbg_math_blocked = self.dbg_math_blocked.saturating_add(1);
                if main_layer_id == 4 {
                    self.dbg_math_blocked_obj = self.dbg_math_blocked_obj.saturating_add(1);
                }
                if main_layer_id == 5 {
                    self.dbg_math_blocked_backdrop =
                        self.dbg_math_blocked_backdrop.saturating_add(1);
                }
            }
            return main_color;
        }

        // Subsource is only used for color math; transparent main becomes backdrop earlier
        let use_sub_src = (self.cgwsel & 0x02) != 0; // 1=subscreen, 0=fixed
                                                     // NOTE: If the subscreen pixel is transparent, real hardware uses fixed color ($2132)
                                                     // as the addend, *and* disables half color math for that pixel.
        let sub_is_fixed_fallback = use_sub_src && sub_transparent;
        let sub_src = if use_sub_src && !sub_is_fixed_fallback {
            sub_color_in
        } else {
            self.fixed_color_to_rgb()
        };

        let src = sub_src;
        let src_is_fixed = !use_sub_src || sub_is_fixed_fallback;
        // Use CGADSUB for add/sub + halve
        let is_addition = (self.cgadsub & 0x80) == 0;
        let halve_flag = (self.cgadsub & 0x40) != 0;
        // In pseudo-hires, when actually blending main and sub (not fixed color),
        // halve the result to approximate 512px brightness. Skip when src is fixed color.
        let hires_halve =
            self.pseudo_hires && use_sub_src && !src_is_fixed && main_color != 0 && src != 0;
        let effective_halve = (halve_flag || hires_halve) && !sub_is_fixed_fallback;
        if let Some(cfg) = trace_sample_dot_config() {
            if self.frame == cfg.frame && x == cfg.x && y == cfg.y {
                println!(
                    "[TRACE_SAMPLE_DOT][MATH] frame={} x={} y={} mode=blend win={} mm={} ss={} main_lid={} main=0x{:08X} sub_in=0x{:08X} src=0x{:08X} use_sub={} sub_fixed_fallback={} sub_transparent={} math_enabled={} add={} halve={} hires_halve={} effective_halve={}",
                    self.frame,
                    x,
                    y,
                    win as u8,
                    mm,
                    ss,
                    main_layer_id,
                    main_color,
                    sub_color_in,
                    src,
                    use_sub_src as u8,
                    sub_is_fixed_fallback as u8,
                    sub_transparent as u8,
                    math_enabled as u8,
                    is_addition as u8,
                    halve_flag as u8,
                    hires_halve as u8,
                    effective_halve as u8
                );
            }
        }
        let out = self.blend_colors(main_color, src, is_addition, effective_halve);
        if crate::debug_flags::render_metrics() {
            if is_addition {
                if effective_halve {
                    self.dbg_math_add_half += 1;
                    if main_layer_id == 4 {
                        self.dbg_math_obj_add_half += 1;
                    }
                } else {
                    self.dbg_math_add += 1;
                    if main_layer_id == 4 {
                        self.dbg_math_obj_add += 1;
                    }
                }
            } else if effective_halve {
                self.dbg_math_sub_half += 1;
                if main_layer_id == 4 {
                    self.dbg_math_obj_sub_half += 1;
                }
            } else {
                self.dbg_math_sub += 1;
                if main_layer_id == 4 {
                    self.dbg_math_obj_sub += 1;
                }
            }
        }
        out
    }
}
