use super::trace::env_presence_flag;
use crate::ppu::Ppu;

impl Ppu {
    pub(crate) fn prepare_line_opt_luts(&mut self) {
        if self.bg_mode != 2 {
            return;
        }

        // DEBUG: bypass OPT to test raw tile rendering
        if env_presence_flag("BYPASS_OPT") {
            for col in 0..=32usize {
                self.mode2_opt_hscroll_lut[0][col] = self.bg1_hscroll;
                self.mode2_opt_vscroll_lut[0][col] = self.bg1_vscroll;
                self.mode2_opt_hscroll_lut[1][col] = self.bg2_hscroll;
                self.mode2_opt_vscroll_lut[1][col] = self.bg2_vscroll;
            }
            return;
        }

        // Column 0 is never affected (per OPT rules).
        self.mode2_opt_hscroll_lut[0][0] = self.bg1_hscroll;
        self.mode2_opt_vscroll_lut[0][0] = self.bg1_vscroll;
        self.mode2_opt_hscroll_lut[1][0] = self.bg2_hscroll;
        self.mode2_opt_vscroll_lut[1][0] = self.bg2_vscroll;

        for col in 1..=32usize {
            // Match bsnes: the lookup coordinates are pixel-based and therefore
            // respect BG3's tile size instead of assuming 8x8 entries.
            let lookup_x = ((col as u16 - 1) * 8).wrapping_add(self.bg3_hscroll & !0x0007);
            let h_entry = self.read_bg_tilemap_entry_word_at_pixel(2, lookup_x, self.bg3_vscroll);
            let v_entry = self.read_bg_tilemap_entry_word_at_pixel(
                2,
                lookup_x,
                self.bg3_vscroll.wrapping_add(8),
            );

            // bit13 applies to BG1, bit14 applies to BG2.
            let bg1_apply = (h_entry & 0x2000) != 0;
            let bg2_apply = (h_entry & 0x4000) != 0;
            let bg1_apply_v = (v_entry & 0x2000) != 0;
            let bg2_apply_v = (v_entry & 0x4000) != 0;

            // OPT scroll values are 13-bit. The low 3 bits are ignored for horizontal
            // replacement, but vertical replacement uses the full 13-bit value.
            let h_val = h_entry & 0x1FFF;
            let v_val = v_entry & 0x1FFF;

            let bg1_fine = self.bg1_hscroll & 0x0007;
            let bg2_fine = self.bg2_hscroll & 0x0007;

            let bg1_h = if bg1_apply {
                (h_val & !0x0007) | bg1_fine
            } else {
                self.bg1_hscroll
            };
            let bg2_h = if bg2_apply {
                (h_val & !0x0007) | bg2_fine
            } else {
                self.bg2_hscroll
            };
            let bg1_v = if bg1_apply_v { v_val } else { self.bg1_vscroll };
            let bg2_v = if bg2_apply_v { v_val } else { self.bg2_vscroll };

            self.mode2_opt_hscroll_lut[0][col] = bg1_h;
            self.mode2_opt_vscroll_lut[0][col] = bg1_v;
            self.mode2_opt_hscroll_lut[1][col] = bg2_h;
            self.mode2_opt_vscroll_lut[1][col] = bg2_v;
        }
    }
    #[inline]
    pub(crate) fn update_line_render_state(&mut self) {
        let main = self.effective_main_screen_designation();
        let sub = self.sub_screen_designation;
        self.line_main_enables = main;
        self.line_sub_enables = sub;
        self.line_main_has_bg = (main & 0x0F) != 0;
        self.line_main_has_obj = (main & 0x10) != 0;
        self.line_sub_has_bg = (sub & 0x0F) != 0;
        self.line_sub_has_obj = (sub & 0x10) != 0;
        self.line_hires_out = self.pseudo_hires || matches!(self.bg_mode, 5 | 6);
        let color_mask = self.cgadsub & 0x3F;
        let use_sub_src = (self.cgwsel & 0x02) != 0;
        self.line_color_math_enabled = (self.cgwsel & 0xF0) != 0 || color_mask != 0;
        self.line_need_subscreen = self.line_hires_out || (use_sub_src && color_mask != 0);
    }
}
