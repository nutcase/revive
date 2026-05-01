use crate::ppu::Ppu;

impl Ppu {
    // プライオリティベースのピクセル合成
    #[allow(dead_code)]
    pub(crate) fn composite_pixel(
        &self,
        bg_color: u32,
        bg_priority: u8,
        sprite_color: u32,
        sprite_priority: u8,
    ) -> u32 {
        let (final_color, _layer_id) = self.composite_pixel_with_layer(
            bg_color,
            bg_priority,
            0,
            sprite_color,
            sprite_priority,
        );
        // 画面間のカラー演算は apply_color_math_screens() で一括適用する。
        // ここではレイヤー合成のみ行う。
        final_color
    }

    pub(crate) fn composite_pixel_with_layer(
        &self,
        bg_color: u32,
        bg_priority: u8,
        bg_layer_id: u8,
        sprite_color: u32,
        sprite_priority: u8,
    ) -> (u32, u8) {
        // 透明なピクセルをチェック
        let sprite_transparent = self.is_transparent_pixel(sprite_color);
        let bg_transparent = self.is_transparent_pixel(bg_color);
        if sprite_transparent && bg_transparent {
            // Fully transparent: caller will fall back to backdrop color.
            return (0, 5);
        }
        if sprite_transparent {
            return (bg_color, bg_layer_id);
        }
        if bg_transparent {
            return (sprite_color, 4);
        }

        // Unified priority model via z-rank table.
        let z_obj = self.z_rank_for_obj(sprite_priority);
        let z_bg = self.z_rank_for_bg(bg_layer_id, bg_priority);
        if z_obj >= z_bg {
            (sprite_color, 4)
        } else {
            (bg_color, bg_layer_id)
        }
    }

    #[inline]
    pub(crate) fn z_rank_for_obj(&self, pr: u8) -> i16 {
        match self.bg_mode {
            7 => match pr {
                3 => crate::debug_flags::m7_z_obj3(),
                2 => crate::debug_flags::m7_z_obj2(),
                1 => crate::debug_flags::m7_z_obj1(),
                _ => crate::debug_flags::m7_z_obj0(),
            },
            _ => match pr {
                3 => 90,
                2 => 70,
                1 => 50,
                _ => 40,
            },
        }
    }

    #[inline]
    pub(crate) fn z_rank_for_bg(&self, layer: u8, pr: u8) -> i16 {
        match self.bg_mode {
            0 => {
                // Priority order for Mode 0 (front -> back):
                //   S3 1H 2H S2 1L 2L S1 3H 4H S0 3L 4L
                // Where:
                //   S3..S0 = OBJ priorities 3..0
                //   1..4   = BG1..BG4 (layer 0..3)
                //   H/L    = tile priority bit (pr=1/0)
                match (layer, pr >= 1) {
                    // BG1
                    (0, true) => 85,  // between S3(90) and S2(70)
                    (0, false) => 65, // between S2(70) and S1(50)
                    // BG2
                    (1, true) => 80,  // between BG1H and S2
                    (1, false) => 60, // between BG1L and S1
                    // BG3
                    (2, true) => 45,  // between S1(50) and S0(40)
                    (2, false) => 35, // below S0
                    // BG4
                    (3, true) => 42,  // between BG3H and S0
                    (3, false) => 30, // bottom-most BG
                    _ => 30,
                }
            }
            1 => {
                // Priority order for Mode 1 (front -> back):
                //   If BGMODE bit3 (BG3 priority) = 1:
                //     3H S3 1H 2H S2 1L 2L S1 S0 3L
                //   If BGMODE bit3 = 0:
                //     S3 1H 2H S2 1L 2L S1 3H S0 3L
                // (BG4 not present.)
                let bg3_slot_high = self.mode1_bg3_priority; // $2105 bit3
                match layer {
                    // BG3
                    2 => {
                        if pr >= 1 {
                            if bg3_slot_high {
                                95 // above S3
                            } else {
                                45 // between S1 and S0
                            }
                        } else {
                            35 // 3L below S0
                        }
                    }
                    // BG2
                    1 => {
                        if pr >= 1 {
                            80 // between S3 and S2
                        } else {
                            60 // between S2 and S1
                        }
                    }
                    // BG1 (default)
                    _ => {
                        if pr >= 1 {
                            85 // between S3 and S2, above BG2H
                        } else {
                            65 // between S2 and S1, above BG2L
                        }
                    }
                }
            }
            2..=4 => {
                // Priority order for Modes 2/3/4 (front -> back):
                //   S3 1H S2 2H S1 1L S0 2L
                match (layer, pr >= 1) {
                    // BG1
                    (0, true) => 80,  // between S3 and S2
                    (0, false) => 45, // between S1 and S0
                    // BG2
                    (1, true) => 60,  // between S2 and S1
                    (1, false) => 35, // below S0
                    _ => 35,
                }
            }
            5 | 6 => {
                // Mode 5/6: BG1 and BG2 have distinct priority slots.
                // Order (front->back) from SNESdev: OBJ3, BG1H, OBJ2, BG2H, OBJ1, BG1L, OBJ0, BG2L.
                // We map into the generic OBJ z-ranks (90/70/50/40) by placing BG ranks between them.
                match (layer, pr) {
                    (0, 1) => 80, // BG1 high
                    (1, 1) => 60, // BG2 high
                    (0, _) => 45, // BG1 low
                    _ => 35,      // BG2 low (and any other BG)
                }
            }
            7 => {
                // EXTBG z-ranks are tunable via env for precise ordering experiments
                if layer == 1 {
                    crate::debug_flags::m7_z_bg2()
                } else {
                    crate::debug_flags::m7_z_bg1()
                }
            }
            _ => {
                if pr >= 1 {
                    60
                } else {
                    40
                }
            }
        }
    }
}
