use crate::ppu::{trace_sample_dot_config, Ppu};

impl Ppu {
    // メインスクリーン描画（レイヤID付き）共通処理
    pub(crate) fn render_main_screen_pixel_with_layer_internal(
        &mut self,
        x: u16,
        y: u16,
        enables: u8,
        has_bg: bool,
        has_obj: bool,
    ) -> (u32, u8, bool) {
        // BGとスプライトの情報を取得
        let (bg_color, bg_priority, bg_id) = if has_bg {
            self.get_main_bg_pixel(x, y, enables)
        } else {
            (0, 0, 0)
        };
        let (sprite_color, sprite_priority, sprite_math_allowed) = if has_obj {
            self.get_sprite_pixel_common(x, y, true, true)
        } else {
            (0, 0, true)
        };
        if let Some(cfg) = trace_sample_dot_config() {
            if self.frame == cfg.frame && x == cfg.x && y == cfg.y {
                let z_bg = self.z_rank_for_bg(bg_id, bg_priority);
                let z_obj = self.z_rank_for_obj(sprite_priority);
                println!(
                    "[TRACE_SAMPLE_DOT][MAIN] frame={} x={} y={} bg=(0x{:08X},id={},pr={},z={}) obj=(0x{:08X},pr={},z={},math={})",
                    self.frame,
                    x,
                    y,
                    bg_color,
                    bg_id,
                    bg_priority,
                    z_bg,
                    sprite_color,
                    sprite_priority,
                    z_obj,
                    sprite_math_allowed as u8
                );
            }
        }

        // プライオリティベースの合成（レイヤIDも取得）
        let (final_color, layer_id) = self.composite_pixel_with_layer(
            bg_color,
            bg_priority,
            bg_id,
            sprite_color,
            sprite_priority,
        );
        let obj_math_allowed = if layer_id == 4 {
            sprite_math_allowed
        } else {
            true
        };

        // NOTE: This function is called per pixel. Debug flags are OnceLock-cached
        // (single pointer read after init), and the outer check gates all inner work.
        {
            // Cache both flags in a single outer branch; avoids repeated OnceLock reads
            // inside the hot inner block.
            let rv = crate::debug_flags::render_verbose();
            let gd = crate::debug_flags::debug_graphics_detected();
            if rv || gd {
                static mut RENDER_DEBUG_COUNT: u32 = 0;
                static mut NON_BLACK_PIXELS: u32 = 0;
                static mut GRAPHICS_DETECTED_PRINTS: u32 = 0;
                let quiet = crate::debug_flags::quiet();

                unsafe {
                    RENDER_DEBUG_COUNT = RENDER_DEBUG_COUNT.saturating_add(1);
                    if final_color != 0xFF000000 {
                        NON_BLACK_PIXELS = NON_BLACK_PIXELS.saturating_add(1);
                    }

                    if rv && x == 10 && y == 10 {
                        static mut PIXEL_10_10_SHOWN: bool = false;
                        if !PIXEL_10_10_SHOWN && !quiet {
                            println!(
                                "🎯 PIXEL (10,10): bg_color=0x{:06X}, final_color=0x{:06X}, layer={}",
                                bg_color, final_color, layer_id
                            );
                            PIXEL_10_10_SHOWN = true;
                        }
                    }

                    if rv && !quiet && RENDER_DEBUG_COUNT.is_multiple_of(100000) {
                        println!(
                            "🖼️  RENDER STATS: {} pixels rendered, {} non-black ({:.1}%)",
                            RENDER_DEBUG_COUNT,
                            NON_BLACK_PIXELS,
                            (NON_BLACK_PIXELS as f32 / RENDER_DEBUG_COUNT as f32) * 100.0
                        );
                    }

                    if gd && !quiet && final_color != 0xFF000000 && GRAPHICS_DETECTED_PRINTS == 0 {
                        GRAPHICS_DETECTED_PRINTS = 1;
                        println!(
                            "🎨 GRAPHICS DETECTED: first non-black pixel 0x{:08X} at ({}, {}) layer={}",
                            final_color, x, y, layer_id
                        );
                    }
                }
            }
        }

        (final_color, layer_id, obj_math_allowed)
    }

    // メインスクリーン描画（レイヤID付き）
    pub(crate) fn render_main_screen_pixel_with_layer(
        &mut self,
        x: u16,
        y: u16,
    ) -> (u32, u8, bool) {
        let enables = self.effective_main_screen_designation();
        let has_bg = (enables & 0x0F) != 0;
        let has_obj = (enables & 0x10) != 0;
        self.render_main_screen_pixel_with_layer_internal(x, y, enables, has_bg, has_obj)
    }

    // Render path for active scanlines using cached per-line enables.
    pub(crate) fn render_main_screen_pixel_with_layer_cached(
        &mut self,
        x: u16,
        y: u16,
    ) -> (u32, u8, bool) {
        self.render_main_screen_pixel_with_layer_internal(
            x,
            y,
            self.line_main_enables,
            self.line_main_has_bg,
            self.line_main_has_obj,
        )
    }

    // メインスクリーン用BGの最前面色とその優先度を取得
    pub(crate) fn get_main_bg_pixel(&mut self, x: u16, y: u16, enables: u8) -> (u32, u8, u8) {
        // Hot path: avoid per-pixel heap allocations.

        let mut best: Option<(u32, u8, u8, i16)> = None;

        // Keep ordering consistent with the previous max_by_key:
        // (z_rank_for_bg(layer, pr), layer_id)
        macro_rules! consider {
            ($color:expr, $pr:expr, $id:expr) => {{
                let color: u32 = $color;
                if color == 0 {
                    // Transparent
                } else {
                    let pr: u8 = $pr;
                    let id: u8 = $id;
                    let z = self.z_rank_for_bg(id, pr);
                    let replace = match best {
                        None => true,
                        Some((_, _, best_id, best_z)) => {
                            z > best_z || (z == best_z && id > best_id)
                        }
                    };
                    if replace {
                        best = Some((color, pr, id, z));
                    }
                }
            }};
        }

        match self.bg_mode {
            0 => {
                // Mode 0: BG1-4 all 2bpp
                if (enables & 0x01) != 0 && !self.should_mask_bg(x, 0, true) {
                    let (c, p) = self.render_bg_mode0_with_priority(x, y, 0);
                    consider!(c, p, 0);
                }
                if (enables & 0x02) != 0 && !self.should_mask_bg(x, 1, true) {
                    let (c, p) = self.render_bg_mode0_with_priority(x, y, 1);
                    consider!(c, p, 1);
                }
                if (enables & 0x04) != 0 && !self.should_mask_bg(x, 2, true) {
                    let (c, p) = self.render_bg_mode0_with_priority(x, y, 2);
                    consider!(c, p, 2);
                }
                if (enables & 0x08) != 0 && !self.should_mask_bg(x, 3, true) {
                    let (c, p) = self.render_bg_mode0_with_priority(x, y, 3);
                    consider!(c, p, 3);
                }
            }
            1 => {
                // Mode 1: BG1/BG2 are 4bpp, BG3 is 2bpp
                if (enables & 0x01) != 0 && !self.should_mask_bg(x, 0, true) {
                    let (c, p) = self.render_bg_4bpp_with_priority(x, y, 0);
                    consider!(c, p, 0);
                }
                if (enables & 0x02) != 0 && !self.should_mask_bg(x, 1, true) {
                    let (c, p) = self.render_bg_4bpp_with_priority(x, y, 1);
                    consider!(c, p, 1);
                }
                if (enables & 0x04) != 0 && !self.should_mask_bg(x, 2, true) {
                    let (c, p) = self.render_bg_mode0_with_priority(x, y, 2);
                    consider!(c, p, 2);
                }
            }
            2 => {
                // Mode 2: BG1/BG2 are 4bpp with offset-per-tile
                if (enables & 0x01) != 0 {
                    let (c, p) = self.render_bg_mode2_window_aware(x, y, 0, true);
                    consider!(c, p, 0);
                }
                if (enables & 0x02) != 0 {
                    let (c, p) = self.render_bg_mode2_window_aware(x, y, 1, true);
                    consider!(c, p, 1);
                }
            }
            3 => {
                // Mode 3: BG1 is 8bpp, BG2 is 4bpp
                if (enables & 0x01) != 0 && !self.should_mask_bg(x, 0, true) {
                    let (c, p) = self.render_bg_8bpp_with_priority(x, y, 0);
                    consider!(c, p, 0);
                }
                if (enables & 0x02) != 0 && !self.should_mask_bg(x, 1, true) {
                    let (c, p) = self.render_bg_4bpp_with_priority(x, y, 1);
                    consider!(c, p, 1);
                }
            }
            4 => {
                // Mode 4: BG1 is 8bpp, BG2 is 2bpp
                if (enables & 0x01) != 0 && !self.should_mask_bg(x, 0, true) {
                    let (c, p) = self.render_bg_8bpp_with_priority(x, y, 0);
                    consider!(c, p, 0);
                }
                if (enables & 0x02) != 0 && !self.should_mask_bg(x, 1, true) {
                    let (c, p) = self.render_bg_mode0_with_priority(x, y, 1);
                    consider!(c, p, 1);
                }
            }
            5 => {
                // Mode 5: BG1 is 4bpp, BG2 is 2bpp (hires); some games also use BG3
                if (enables & 0x01) != 0 && !self.should_mask_bg(x, 0, true) {
                    let (c, p) = self.render_bg_mode5_with_priority(x, y, 0);
                    consider!(c, p, 0);
                }
                if (enables & 0x02) != 0 && !self.should_mask_bg(x, 1, true) {
                    let (c, p) = self.render_bg_mode5_with_priority(x, y, 1);
                    consider!(c, p, 1);
                }
                if (enables & 0x04) != 0 && !self.should_mask_bg(x, 2, true) {
                    let (c, p) = self.render_bg_mode5_with_priority(x, y, 2);
                    consider!(c, p, 2);
                }
            }
            6 => {
                // Mode 6: BG1 is 4bpp (hires)
                if (enables & 0x01) != 0 && !self.should_mask_bg(x, 0, true) {
                    let (c, p) = self.render_bg_mode6_with_priority(x, y, 0);
                    consider!(c, p, 0);
                }
            }
            7 => {
                if self.extbg {
                    // EXTBG: BG1 and BG2 are independent layers from the same Mode 7 tilemap.
                    // BG1: full 8-bit color, single priority.
                    // BG2: 7-bit color, bit7 as priority (0 or 1).
                    // Each layer is checked independently for enable and window masking.
                    for layer_id in 0..=1u8 {
                        let en_bit = 1u8 << layer_id;
                        if (enables & en_bit) != 0 && !self.should_mask_bg(x, layer_id, true) {
                            let (c, p, _) = self.render_mode7_single_layer(x, y, layer_id);
                            if c != 0 {
                                consider!(c, p, layer_id);
                            }
                        }
                    }
                } else if (enables & 0x01) != 0 {
                    let (c, p, _) = self.render_mode7_with_layer(x, y);
                    if c != 0 && !self.should_mask_bg(x, 0, true) {
                        consider!(c, p, 0);
                    }
                }
            }
            _ => {}
        }

        if let Some((best_color, best_pr, best_id, _)) = best {
            (best_color, best_pr, best_id)
        } else {
            (0, 0, 0)
        }
    }

    // サブスクリーン描画
    #[allow(dead_code)]
    pub(crate) fn render_sub_screen_pixel(&mut self, x: u16, y: u16) -> u32 {
        let enables = self.sub_screen_designation;
        let has_bg = (enables & 0x0F) != 0;
        let has_obj = (enables & 0x10) != 0;
        let (color, _lid, _transparent, _obj_math_allowed) =
            self.render_sub_screen_pixel_with_layer_internal(x, y, has_bg, has_obj);
        color
    }

    // サブスクリーン描画（レイヤID付き）
    // 戻り値:
    // - color: 合成後のRGBA
    // - layer_id: 最前面レイヤ（0..4、5=backdrop）
    // - transparent: サブスクリーンが完全透明で、fixed color ($2132) をbackdropとして返した場合
    pub(crate) fn render_sub_screen_pixel_with_layer_internal(
        &mut self,
        x: u16,
        y: u16,
        has_bg: bool,
        has_obj: bool,
    ) -> (u32, u8, bool, bool) {
        if !has_bg && !has_obj {
            // For color math, an empty subscreen behaves like a transparent source
            // that falls back to fixed color rather than forcing backdrop-black math.
            return (self.cgram_to_rgb(0), 5, true, true);
        }
        let (bg_color, bg_priority, bg_id) = if has_bg {
            self.get_sub_bg_pixel(x, y)
        } else {
            (0, 0, 0)
        };
        let (sprite_color, sprite_priority, sprite_math_allowed) =
            self.get_sprite_pixel_common(x, y, has_obj, false);
        let (final_color, layer_id) = self.composite_pixel_with_layer(
            bg_color,
            bg_priority,
            bg_id,
            sprite_color,
            sprite_priority,
        );
        let obj_math_allowed = if layer_id == 4 {
            sprite_math_allowed
        } else {
            true
        };
        if final_color != 0 {
            (final_color, layer_id, false, obj_math_allowed)
        } else {
            (self.cgram_to_rgb(0), 5, true, true)
        }
    }

    pub(crate) fn render_sub_screen_pixel_with_layer(
        &mut self,
        x: u16,
        y: u16,
    ) -> (u32, u8, bool, bool) {
        let enables = self.sub_screen_designation;
        let has_bg = (enables & 0x0F) != 0;
        let has_obj = (enables & 0x10) != 0;
        self.render_sub_screen_pixel_with_layer_internal(x, y, has_bg, has_obj)
    }

    pub(crate) fn render_sub_screen_pixel_with_layer_cached(
        &mut self,
        x: u16,
        y: u16,
    ) -> (u32, u8, bool, bool) {
        self.render_sub_screen_pixel_with_layer_internal(
            x,
            y,
            self.line_sub_has_bg,
            self.line_sub_has_obj,
        )
    }

    // サブスクリーン用BG描画（メインと同等のモード対応）
    pub(crate) fn get_sub_bg_pixel(&mut self, x: u16, y: u16) -> (u32, u8, u8) {
        // Hot path: avoid per-pixel heap allocations.
        let enables = self.sub_screen_designation;

        let mut best: Option<(u32, u8, u8, i16)> = None;

        // Keep ordering consistent with the previous max_by_key:
        // (z_rank_for_bg(layer, pr), layer_id)
        macro_rules! consider {
            ($color:expr, $pr:expr, $id:expr) => {{
                let color: u32 = $color;
                if color == 0 {
                    // Transparent
                } else {
                    let pr: u8 = $pr;
                    let id: u8 = $id;
                    let z = self.z_rank_for_bg(id, pr);
                    let replace = match best {
                        None => true,
                        Some((_, _, best_id, best_z)) => {
                            z > best_z || (z == best_z && id > best_id)
                        }
                    };
                    if replace {
                        best = Some((color, pr, id, z));
                    }
                }
            }};
        }

        match self.bg_mode {
            0 => {
                if (enables & 0x01) != 0 && !self.should_mask_bg(x, 0, false) {
                    let (c, p) = self.render_bg_mode0(x, y, 0);
                    consider!(c, p, 0);
                }
                if (enables & 0x02) != 0 && !self.should_mask_bg(x, 1, false) {
                    let (c, p) = self.render_bg_mode0(x, y, 1);
                    consider!(c, p, 1);
                }
                if (enables & 0x04) != 0 && !self.should_mask_bg(x, 2, false) {
                    let (c, p) = self.render_bg_mode0(x, y, 2);
                    consider!(c, p, 2);
                }
                if (enables & 0x08) != 0 && !self.should_mask_bg(x, 3, false) {
                    let (c, p) = self.render_bg_mode0(x, y, 3);
                    consider!(c, p, 3);
                }
            }
            1 => {
                if (enables & 0x01) != 0 && !self.should_mask_bg(x, 0, false) {
                    let (c, p) = self.render_bg_4bpp(x, y, 0);
                    consider!(c, p, 0);
                }
                if (enables & 0x02) != 0 && !self.should_mask_bg(x, 1, false) {
                    let (c, p) = self.render_bg_4bpp(x, y, 1);
                    consider!(c, p, 1);
                }
                if (enables & 0x04) != 0 && !self.should_mask_bg(x, 2, false) {
                    let (c, p) = self.render_bg_mode0(x, y, 2);
                    consider!(c, p, 2);
                }
            }
            2 => {
                if (enables & 0x01) != 0 {
                    let (c, p) = self.render_bg_mode2_window_aware(x, y, 0, false);
                    consider!(c, p, 0);
                }
                if (enables & 0x02) != 0 {
                    let (c, p) = self.render_bg_mode2_window_aware(x, y, 1, false);
                    consider!(c, p, 1);
                }
            }
            3 => {
                if (enables & 0x01) != 0 && !self.should_mask_bg(x, 0, false) {
                    let (c, p) = self.render_bg_8bpp(x, y, 0);
                    consider!(c, p, 0);
                }
                if (enables & 0x02) != 0 && !self.should_mask_bg(x, 1, false) {
                    let (c, p) = self.render_bg_4bpp(x, y, 1);
                    consider!(c, p, 1);
                }
            }
            4 => {
                if (enables & 0x01) != 0 && !self.should_mask_bg(x, 0, false) {
                    let (c, p) = self.render_bg_8bpp(x, y, 0);
                    consider!(c, p, 0);
                }
                if (enables & 0x02) != 0 && !self.should_mask_bg(x, 1, false) {
                    let (c, p) = self.render_bg_mode0(x, y, 1);
                    consider!(c, p, 1);
                }
            }
            5 => {
                if (enables & 0x01) != 0 && !self.should_mask_bg(x, 0, false) {
                    let (c, p) = self.render_bg_mode5(x, y, 0, false);
                    consider!(c, p, 0);
                }
                if (enables & 0x02) != 0 && !self.should_mask_bg(x, 1, false) {
                    let (c, p) = self.render_bg_mode5(x, y, 1, false);
                    consider!(c, p, 1);
                }
            }
            6 => {
                if (enables & 0x01) != 0 && !self.should_mask_bg(x, 0, false) {
                    let (c, p) = self.render_bg_mode6(x, y, 0, false);
                    consider!(c, p, 0);
                }
            }
            7 => {
                if self.extbg {
                    for layer_id in 0..=1u8 {
                        let en_bit = 1u8 << layer_id;
                        if (enables & en_bit) != 0 && !self.should_mask_bg(x, layer_id, false) {
                            let (c, p, _) = self.render_mode7_single_layer(x, y, layer_id);
                            if c != 0 {
                                consider!(c, p, layer_id);
                            }
                        }
                    }
                } else if (enables & 0x01) != 0 {
                    let (c, p, _) = self.render_mode7_with_layer(x, y);
                    if c != 0 && !self.should_mask_bg(x, 0, false) {
                        consider!(c, p, 0);
                    }
                }
            }
            _ => {}
        }

        if let Some((best_color, best_pr, best_id, _)) = best {
            (best_color, best_pr, best_id)
        } else {
            (0, 0, 0)
        }
    }
}
