use super::{Ppu, WindowLutConfig};
use std::sync::OnceLock;

#[inline]
fn cached_env_flag(name: &'static str) -> bool {
    if cfg!(test) {
        return std::env::var_os(name).is_some();
    }

    match name {
        "IGNORE_WINDOWS" => {
            static ON: OnceLock<bool> = OnceLock::new();
            *ON.get_or_init(|| std::env::var_os("IGNORE_WINDOWS").is_some())
        }
        "IGNORE_BG_WINDOWS" => {
            static ON: OnceLock<bool> = OnceLock::new();
            *ON.get_or_init(|| std::env::var_os("IGNORE_BG_WINDOWS").is_some())
        }
        "IGNORE_OBJ_WINDOWS" => {
            static ON: OnceLock<bool> = OnceLock::new();
            *ON.get_or_init(|| std::env::var_os("IGNORE_OBJ_WINDOWS").is_some())
        }
        "IGNORE_COLOR_WINDOW" => {
            static ON: OnceLock<bool> = OnceLock::new();
            *ON.get_or_init(|| std::env::var_os("IGNORE_COLOR_WINDOW").is_some())
        }
        _ => std::env::var_os(name).is_some(),
    }
}

#[inline(always)]
fn window_mask_from_inside(w1_inside: bool, w2_inside: bool, mask_setting: u8, logic: u8) -> bool {
    if mask_setting == 0 {
        return false;
    }

    let w1_enabled = (mask_setting & 0x02) != 0;
    let w2_enabled = (mask_setting & 0x08) != 0;

    if !w1_enabled && !w2_enabled {
        return false;
    }

    let w1_result = if w1_enabled {
        w1_inside ^ ((mask_setting & 0x01) != 0)
    } else {
        false
    };
    let w2_result = if w2_enabled {
        w2_inside ^ ((mask_setting & 0x04) != 0)
    } else {
        false
    };

    if !w2_enabled {
        return w1_result;
    }
    if !w1_enabled {
        return w2_result;
    }

    match logic & 0x03 {
        0 => w1_result || w2_result,
        1 => w1_result && w2_result,
        2 => w1_result ^ w2_result,
        _ => !(w1_result ^ w2_result),
    }
}

impl Ppu {
    // ウィンドウマスク関連関数
    #[inline]
    fn ignore_windows_debug() -> bool {
        cached_env_flag("IGNORE_WINDOWS")
    }

    #[inline]
    fn ignore_bg_windows_debug() -> bool {
        Self::ignore_windows_debug() || cached_env_flag("IGNORE_BG_WINDOWS")
    }

    #[inline]
    fn ignore_obj_windows_debug() -> bool {
        Self::ignore_windows_debug() || cached_env_flag("IGNORE_OBJ_WINDOWS")
    }

    #[inline]
    fn ignore_color_window_debug() -> bool {
        Self::ignore_windows_debug() || cached_env_flag("IGNORE_COLOR_WINDOW")
    }

    pub(crate) fn is_inside_window1(&self, x: u16) -> bool {
        let x = x as u8;
        // SNES window effect is defined only for left <= x <= right.
        // If left > right, the window effect is nowhere (all 0 output).
        // See: https://snes.nesdev.org/wiki/Windows
        x >= self.window1_left && x <= self.window1_right
    }

    pub(crate) fn is_inside_window2(&self, x: u16) -> bool {
        let x = x as u8;
        // Same rule as window 1 (no wrap-around behavior).
        x >= self.window2_left && x <= self.window2_right
    }

    pub(crate) fn evaluate_window_mask(&self, x: u16, mask_setting: u8, logic: u8) -> bool {
        // マスク設定のビット構成 (W12SEL/W34SEL/WOBJSEL の各4bit):
        // Bit 0: Window 1 Inverted
        // Bit 1: Window 1 Enabled
        // Bit 2: Window 2 Inverted
        // Bit 3: Window 2 Enabled
        // Logic is provided by WBGLOG/WOBJLOG (00=OR, 01=AND, 10=XOR, 11=XNOR)

        if mask_setting == 0 {
            return false; // ウィンドウ無効
        }

        let res = window_mask_from_inside(
            self.is_inside_window1(x),
            self.is_inside_window2(x),
            mask_setting,
            logic,
        );

        if crate::debug_flags::render_metrics() && res {
            match logic & 0x03 {
                2 => {
                    /* XOR */
                    let v = self.dbg_win_xor_applied.saturating_add(1);
                    let _ = v;
                }
                3 => {
                    /* XNOR */
                    let v = self.dbg_win_xnor_applied.saturating_add(1);
                    let _ = v;
                }
                _ => {}
            }
        }
        res
    }

    pub(crate) fn should_mask_bg(&self, x: u16, bg_num: u8, is_main: bool) -> bool {
        if Self::ignore_bg_windows_debug() {
            return false;
        }
        if bg_num >= 4 {
            return false;
        }
        if bg_num == 0 && self.should_bypass_bg1_window_for_superfx_direct() {
            return false;
        }
        if self.line_window_prepared {
            let idx = x as usize;
            if is_main {
                self.main_bg_window_lut[bg_num as usize][idx] != 0
            } else {
                self.sub_bg_window_lut[bg_num as usize][idx] != 0
            }
        } else {
            // TMW/TSW gating: if disabled for this screen+BG, do not mask
            let enabled = if is_main {
                (self.tmw_mask & (1 << bg_num)) != 0
            } else {
                (self.tsw_mask & (1 << bg_num)) != 0
            };
            if !enabled {
                return false;
            }
            self.evaluate_window_mask(
                x,
                self.window_bg_mask[bg_num as usize],
                self.bg_window_logic[bg_num as usize],
            )
        }
    }

    pub(crate) fn should_mask_sprite(&self, x: u16, is_main: bool) -> bool {
        if Self::ignore_obj_windows_debug() {
            return false;
        }
        if self.line_window_prepared {
            let idx = x as usize;
            if is_main {
                self.main_obj_window_lut[idx] != 0
            } else {
                self.sub_obj_window_lut[idx] != 0
            }
        } else {
            let enabled = if is_main {
                (self.tmw_mask & 0x10) != 0
            } else {
                (self.tsw_mask & 0x10) != 0
            };
            if !enabled {
                return false;
            }
            self.evaluate_window_mask(x, self.window_obj_mask, self.obj_window_logic)
        }
    }

    #[allow(dead_code)]
    pub(crate) fn render_bg_with_window_mask(
        &self,
        x: u16,
        y: u16,
        bg_num: u8,
        render_func: fn(&Self, u16, u16, u8) -> (u32, u8),
    ) -> (u32, u8) {
        // ウィンドウマスクでマスクされている場合は透明を返す
        if self.should_mask_bg(x, bg_num, true) {
            return (0, 0);
        }

        // 通常の描画処理
        render_func(self, x, y, bg_num)
    }

    pub(crate) fn prepare_line_window_luts(&mut self) {
        if Self::ignore_windows_debug() {
            self.line_window_prepared = false;
            self.line_window_cfg = None;
            return;
        }
        if self.bg_cache_dirty {
            self.invalidate_bg_caches();
        }
        if self.tmw_mask == 0 && self.tsw_mask == 0 && self.window_color_mask == 0 {
            self.line_window_prepared = false;
            self.line_window_cfg = None;
            return;
        }
        let cfg = WindowLutConfig {
            window1_left: self.window1_left,
            window1_right: self.window1_right,
            window2_left: self.window2_left,
            window2_right: self.window2_right,
            window_bg_mask: self.window_bg_mask,
            bg_window_logic: self.bg_window_logic,
            window_obj_mask: self.window_obj_mask,
            obj_window_logic: self.obj_window_logic,
            window_color_mask: self.window_color_mask,
            color_window_logic: self.color_window_logic,
            tmw_mask: self.tmw_mask,
            tsw_mask: self.tsw_mask,
        };
        if self.line_window_prepared && self.line_window_cfg == Some(cfg) {
            return;
        }
        self.line_window_prepared = true;
        self.line_window_cfg = Some(cfg);
        let ignore_color_window = Self::ignore_color_window_debug();
        let ignore_bg_windows = Self::ignore_bg_windows_debug();
        let ignore_obj_windows = Self::ignore_obj_windows_debug();
        let main_bg_enabled = [
            !ignore_bg_windows && (self.tmw_mask & 0x01) != 0,
            !ignore_bg_windows && (self.tmw_mask & 0x02) != 0,
            !ignore_bg_windows && (self.tmw_mask & 0x04) != 0,
            !ignore_bg_windows && (self.tmw_mask & 0x08) != 0,
        ];
        let sub_bg_enabled = [
            !ignore_bg_windows && (self.tsw_mask & 0x01) != 0,
            !ignore_bg_windows && (self.tsw_mask & 0x02) != 0,
            !ignore_bg_windows && (self.tsw_mask & 0x04) != 0,
            !ignore_bg_windows && (self.tsw_mask & 0x08) != 0,
        ];
        let obj_m_en = !ignore_obj_windows && (self.tmw_mask & 0x10) != 0;
        let obj_s_en = !ignore_obj_windows && (self.tsw_mask & 0x10) != 0;
        for x in 0..256u16 {
            let x_byte = x as u8;
            let w1_inside = x_byte >= self.window1_left && x_byte <= self.window1_right;
            let w2_inside = x_byte >= self.window2_left && x_byte <= self.window2_right;
            // Color window
            let wcol = if ignore_color_window || self.window_color_mask == 0 {
                false
            } else {
                window_mask_from_inside(
                    w1_inside,
                    w2_inside,
                    self.window_color_mask,
                    self.color_window_logic,
                )
            };
            let idx = x as usize;
            self.color_window_lut[idx] = if wcol { 1 } else { 0 };

            // BG1..BG4 (main/sub)
            for bg in 0..4usize {
                let bg_mask = window_mask_from_inside(
                    w1_inside,
                    w2_inside,
                    self.window_bg_mask[bg],
                    self.bg_window_logic[bg],
                );
                self.main_bg_window_lut[bg][idx] =
                    if main_bg_enabled[bg] && bg_mask { 1 } else { 0 };
                self.sub_bg_window_lut[bg][idx] = if sub_bg_enabled[bg] && bg_mask { 1 } else { 0 };
            }

            // OBJ main/sub
            let obj_mask = window_mask_from_inside(
                w1_inside,
                w2_inside,
                self.window_obj_mask,
                self.obj_window_logic,
            );
            self.main_obj_window_lut[idx] = if obj_m_en && obj_mask { 1 } else { 0 };
            self.sub_obj_window_lut[idx] = if obj_s_en && obj_mask { 1 } else { 0 };
        }
    }
}
