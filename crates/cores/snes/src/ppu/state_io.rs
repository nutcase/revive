use super::Ppu;

impl Ppu {
    // Dump first n colors from CGRAM as 15-bit BGR
    // --- Save state serialization ---
    pub fn to_save_state(&self) -> crate::savestate::PpuSaveState {
        use crate::savestate::PpuSaveState;
        let mut st = PpuSaveState {
            scanline: self.scanline,
            dot: self.cycle,
            frame_count: self.frame,
            vblank: self.v_blank,
            hblank: self.h_blank,
            hv_latched_h: Some(self.hv_latched_h),
            hv_latched_v: Some(self.hv_latched_v),
            wio_latch_pending_dots: Some(self.wio_latch_pending_dots),
            slhv_latch_pending_dots: Some(self.slhv_latch_pending_dots),
            ophct_second: Some(self.ophct_second),
            opvct_second: Some(self.opvct_second),
            brightness: self.brightness,
            forced_blank: (self.screen_display & 0x80) != 0,
            nmi_enabled: self.nmi_enabled,
            nmi_pending: self.nmi_flag,
            nmi_latched: self.nmi_latched,
            rdnmi_read_in_vblank: self.rdnmi_read_in_vblank,
            bg_mode: self.bg_mode,
            mosaic_size: self.mosaic_size,
            ..Default::default()
        };
        st.main_screen_designation = Some(self.main_screen_designation);
        st.sub_screen_designation = Some(self.sub_screen_designation);
        // BG enable is approximated from TM register mirror (main_screen_designation)
        for i in 0..4 {
            st.bg_enabled[i] = (self.effective_main_screen_designation() & (1 << i)) != 0;
            st.bg_priority[i] = 0; // priority not explicitly tracked; leave 0
            st.bg_scroll_x[i] = match i {
                0 => self.bg1_hscroll,
                1 => self.bg2_hscroll,
                2 => self.bg3_hscroll,
                _ => self.bg4_hscroll,
            };
            st.bg_scroll_y[i] = match i {
                0 => self.bg1_vscroll,
                1 => self.bg2_vscroll,
                2 => self.bg3_vscroll,
                _ => self.bg4_vscroll,
            };
            st.bg_tilemap_address[i] = match i {
                0 => self.bg1_tilemap_base,
                1 => self.bg2_tilemap_base,
                2 => self.bg3_tilemap_base,
                _ => self.bg4_tilemap_base,
            };
            st.bg_character_address[i] = match i {
                0 => self.bg1_tile_base,
                1 => self.bg2_tile_base,
                2 => self.bg3_tile_base,
                _ => self.bg4_tile_base,
            };
        }
        st.vram = self.vram.clone();
        st.cgram = self.cgram.clone();
        st.oam = self.oam.clone();
        st.framebuffer = self.framebuffer.clone();
        st.subscreen_buffer = self.subscreen_buffer.clone();
        st.render_framebuffer = self.render_framebuffer.clone();
        st.render_subscreen_buffer = self.render_subscreen_buffer.clone();
        st.vram_address = self.vram_addr;
        st.vram_increment = self.vram_increment;
        st.vram_read_buf_lo = Some(self.vram_read_buf_lo);
        st.vram_read_buf_hi = Some(self.vram_read_buf_hi);
        st.cgram_address = self.cgram_addr;
        st.cgram_read_second = Some(self.cgram_read_second);
        st.oam_address = self.oam_addr;
        st.main_screen_designation_last_nonzero = Some(self.main_screen_designation_last_nonzero);
        st.oam_internal_address = Some(self.oam_internal_addr);
        st.oam_priority_rotation_enabled = Some(self.oam_priority_rotation_enabled);
        st.oam_eval_base = Some(self.oam_eval_base);
        st.sprite_size = Some(self.sprite_size);
        st.sprite_name_base = Some(self.sprite_name_base);
        st.sprite_name_select = Some(self.sprite_name_select);
        st.sprite_overflow = Some(self.sprite_overflow);
        st.sprite_time_over = Some(self.sprite_time_over);
        st.sprite_overflow_latched = Some(self.sprite_overflow_latched);
        st.sprite_time_over_latched = Some(self.sprite_time_over_latched);

        // Mode 7
        st.mode7_matrix_a = Some(self.mode7_matrix_a);
        st.mode7_matrix_b = Some(self.mode7_matrix_b);
        st.mode7_matrix_c = Some(self.mode7_matrix_c);
        st.mode7_matrix_d = Some(self.mode7_matrix_d);
        st.mode7_center_x = Some(self.mode7_center_x);
        st.mode7_center_y = Some(self.mode7_center_y);
        st.mode7_hofs = Some(self.mode7_hofs);
        st.mode7_vofs = Some(self.mode7_vofs);
        st.mode7_latch = Some(self.mode7_latch);
        st.m7sel = Some(self.m7sel);
        st.mode7_mul_b = Some(self.mode7_mul_b);
        st.mode7_mul_result = Some(self.mode7_mul_result);

        // Color math
        st.cgwsel = Some(self.cgwsel);
        st.cgadsub = Some(self.cgadsub);
        st.fixed_color = Some(self.fixed_color);

        // Windows
        st.window1_left = Some(self.window1_left);
        st.window1_right = Some(self.window1_right);
        st.window2_left = Some(self.window2_left);
        st.window2_right = Some(self.window2_right);
        st.window_bg_mask = Some(self.window_bg_mask);
        st.bg_window_logic = Some(self.bg_window_logic);
        st.window_obj_mask = Some(self.window_obj_mask);
        st.obj_window_logic = Some(self.obj_window_logic);
        st.window_color_mask = Some(self.window_color_mask);
        st.color_window_logic = Some(self.color_window_logic);
        st.tmw_mask = Some(self.tmw_mask);
        st.tsw_mask = Some(self.tsw_mask);

        // Display settings
        st.setini = Some(self.setini);
        st.pseudo_hires = Some(self.pseudo_hires);
        st.extbg = Some(self.extbg);
        st.overscan = Some(self.overscan);
        st.screen_display = Some(self.screen_display);
        st.interlace = Some(self.interlace);
        st.obj_interlace = Some(self.obj_interlace);
        st.force_no_blank = Some(self.force_no_blank);

        // BG config
        st.bg_tile_16 = Some(self.bg_tile_16);
        st.bg_screen_size = Some(self.bg_screen_size);
        st.mode1_bg3_priority = Some(self.mode1_bg3_priority);

        // VRAM mapping
        st.vram_mapping = Some(self.vram_mapping);

        // Latches
        st.bgofs_latch = Some(self.bgofs_latch);
        st.bghofs_latch = Some(self.bghofs_latch);
        st.cgram_second = Some(self.cgram_second);
        st.cgram_latch_lo = Some(self.cgram_latch_lo);
        st.bg_mosaic = Some(self.bg_mosaic);
        st.oam_write_latch = Some(self.oam_write_latch);
        st.hdma_head_busy_until = Some(self.hdma_head_busy_until);
        st.framebuffer_rendering_enabled = Some(self.framebuffer_rendering_enabled);
        st.superfx_bypass_bg1_window = Some(self.superfx_bypass_bg1_window);
        st.superfx_authoritative_bg1_source = Some(self.superfx_authoritative_bg1_source);
        st.superfx_direct_buffer = self.superfx_direct_buffer.clone();
        st.superfx_direct_height = Some(self.superfx_direct_height);
        st.superfx_direct_bpp = Some(self.superfx_direct_bpp);
        st.superfx_direct_mode = Some(self.superfx_direct_mode);
        st.superfx_tile_buffer = self.superfx_tile_buffer.clone();
        st.superfx_tile_bpp = Some(self.superfx_tile_bpp);
        st.superfx_tile_mode = Some(self.superfx_tile_mode);
        st.wio_latch_enable = Some(self.wio_latch_enable);
        st.stat78_latch_flag = Some(self.stat78_latch_flag);
        st.interlace_field = Some(self.interlace_field);
        st.latched_inidisp = self.latched_inidisp;
        st.latched_tm = self.latched_tm;
        st.latched_ts = self.latched_ts;
        st.latched_tmw = self.latched_tmw;
        st.latched_tsw = self.latched_tsw;
        st.latched_cgwsel = self.latched_cgwsel;
        st.latched_cgadsub = self.latched_cgadsub;
        st.latched_fixed_color = self.latched_fixed_color;
        st.latched_setini = self.latched_setini;
        st.latched_vmadd_lo = self.latched_vmadd_lo;
        st.latched_vmadd_hi = self.latched_vmadd_hi;
        st.latched_cgadd = self.latched_cgadd;
        st.latched_vmain = self.latched_vmain;
        st.vmain_effect_pending = self.vmain_effect_pending;
        st.vmain_effect_ticks = Some(self.vmain_effect_ticks);
        st.cgadd_effect_pending = self.cgadd_effect_pending;
        st.cgadd_effect_ticks = Some(self.cgadd_effect_ticks);
        st.vmain_data_gap_ticks = Some(self.vmain_data_gap_ticks);
        st.cgram_data_gap_ticks = Some(self.cgram_data_gap_ticks);
        st.latched_wbglog = self.latched_wbglog;
        st.latched_wobjlog = self.latched_wobjlog;

        st
    }

    pub fn load_from_save_state(&mut self, st: &crate::savestate::PpuSaveState) {
        self.scanline = st.scanline;
        self.cycle = st.dot;
        self.frame = st.frame_count;
        self.v_blank = st.vblank;
        self.h_blank = st.hblank;
        if let Some(v) = st.hv_latched_h {
            self.hv_latched_h = v;
        }
        if let Some(v) = st.hv_latched_v {
            self.hv_latched_v = v;
        }
        if let Some(v) = st.wio_latch_pending_dots {
            self.wio_latch_pending_dots = v;
        }
        if let Some(v) = st.slhv_latch_pending_dots {
            self.slhv_latch_pending_dots = v;
        }
        if let Some(v) = st.ophct_second {
            self.ophct_second = v;
        }
        if let Some(v) = st.opvct_second {
            self.opvct_second = v;
        }
        self.brightness = st.brightness;
        if st.forced_blank {
            self.screen_display |= 0x80;
        } else {
            self.screen_display &= !0x80;
        }
        self.nmi_enabled = st.nmi_enabled;
        self.nmi_flag = st.nmi_pending;
        self.nmi_latched = st.nmi_latched;
        self.rdnmi_read_in_vblank = st.rdnmi_read_in_vblank;
        self.bg_mode = st.bg_mode;
        self.mosaic_size = st.mosaic_size;
        let mut fallback_main = 0u8;
        for i in 0..4 {
            if st.bg_enabled[i] {
                fallback_main |= 1 << i;
            }
            match i {
                0 => {
                    self.bg1_hscroll = st.bg_scroll_x[0];
                    self.bg1_vscroll = st.bg_scroll_y[0];
                    self.bg1_tilemap_base = st.bg_tilemap_address[0];
                    self.bg1_tile_base = st.bg_character_address[0];
                }
                1 => {
                    self.bg2_hscroll = st.bg_scroll_x[1];
                    self.bg2_vscroll = st.bg_scroll_y[1];
                    self.bg2_tilemap_base = st.bg_tilemap_address[1];
                    self.bg2_tile_base = st.bg_character_address[1];
                }
                2 => {
                    self.bg3_hscroll = st.bg_scroll_x[2];
                    self.bg3_vscroll = st.bg_scroll_y[2];
                    self.bg3_tilemap_base = st.bg_tilemap_address[2];
                    self.bg3_tile_base = st.bg_character_address[2];
                }
                _ => {
                    self.bg4_hscroll = st.bg_scroll_x[3];
                    self.bg4_vscroll = st.bg_scroll_y[3];
                    self.bg4_tilemap_base = st.bg_tilemap_address[3];
                    self.bg4_tile_base = st.bg_character_address[3];
                }
            }
        }
        if let Some(tm) = st.main_screen_designation {
            self.main_screen_designation = tm;
        } else {
            self.main_screen_designation = fallback_main;
        }
        if self.main_screen_designation != 0 {
            self.main_screen_designation_last_nonzero = self.main_screen_designation;
        }
        if let Some(ts) = st.sub_screen_designation {
            self.sub_screen_designation = ts;
        }
        if self.vram.len() == st.vram.len() {
            self.vram.copy_from_slice(&st.vram);
        }
        if self.cgram.len() == st.cgram.len() {
            self.cgram.copy_from_slice(&st.cgram);
            self.rebuild_cgram_rgb_cache();
        }
        if self.oam.len() == st.oam.len() {
            self.oam.copy_from_slice(&st.oam);
        }
        if self.framebuffer.len() == st.framebuffer.len() {
            self.framebuffer.copy_from_slice(&st.framebuffer);
        }
        if self.subscreen_buffer.len() == st.subscreen_buffer.len() {
            self.subscreen_buffer.copy_from_slice(&st.subscreen_buffer);
        }
        if self.render_framebuffer.len() == st.render_framebuffer.len() {
            self.render_framebuffer
                .copy_from_slice(&st.render_framebuffer);
        }
        if self.render_subscreen_buffer.len() == st.render_subscreen_buffer.len() {
            self.render_subscreen_buffer
                .copy_from_slice(&st.render_subscreen_buffer);
        }
        self.vram_addr = st.vram_address;
        self.vram_increment = st.vram_increment;
        if let Some(v) = st.vram_read_buf_lo {
            self.vram_read_buf_lo = v;
        }
        if let Some(v) = st.vram_read_buf_hi {
            self.vram_read_buf_hi = v;
        }
        self.cgram_addr = st.cgram_address;
        if let Some(v) = st.cgram_read_second {
            self.cgram_read_second = v;
        }
        self.oam_addr = st.oam_address;
        if let Some(v) = st.main_screen_designation_last_nonzero {
            self.main_screen_designation_last_nonzero = v;
        }
        if let Some(addr) = st.oam_internal_address {
            self.oam_internal_addr = addr;
        }
        if let Some(enabled) = st.oam_priority_rotation_enabled {
            self.oam_priority_rotation_enabled = enabled;
        }
        if let Some(base) = st.oam_eval_base {
            self.oam_eval_base = base;
        } else {
            self.refresh_oam_eval_base_from_internal_addr();
        }
        if let Some(size) = st.sprite_size {
            self.sprite_size = size;
        }
        if let Some(base) = st.sprite_name_base {
            self.sprite_name_base = base;
        }
        if let Some(select) = st.sprite_name_select {
            self.sprite_name_select = select;
        }
        if let Some(v) = st.sprite_overflow {
            self.sprite_overflow = v;
        }
        if let Some(v) = st.sprite_time_over {
            self.sprite_time_over = v;
        }
        if let Some(v) = st.sprite_overflow_latched {
            self.sprite_overflow_latched = v;
        }
        if let Some(v) = st.sprite_time_over_latched {
            self.sprite_time_over_latched = v;
        }

        // Mode 7
        if let Some(v) = st.mode7_matrix_a {
            self.mode7_matrix_a = v;
        }
        if let Some(v) = st.mode7_matrix_b {
            self.mode7_matrix_b = v;
        }
        if let Some(v) = st.mode7_matrix_c {
            self.mode7_matrix_c = v;
        }
        if let Some(v) = st.mode7_matrix_d {
            self.mode7_matrix_d = v;
        }
        if let Some(v) = st.mode7_center_x {
            self.mode7_center_x = v;
        }
        if let Some(v) = st.mode7_center_y {
            self.mode7_center_y = v;
        }
        if let Some(v) = st.mode7_hofs {
            self.mode7_hofs = v;
        }
        if let Some(v) = st.mode7_vofs {
            self.mode7_vofs = v;
        }
        if let Some(v) = st.mode7_latch {
            self.mode7_latch = v;
        }
        if let Some(v) = st.m7sel {
            self.m7sel = v;
        }
        if let Some(v) = st.mode7_mul_b {
            self.mode7_mul_b = v;
        }
        if let Some(v) = st.mode7_mul_result {
            self.mode7_mul_result = v;
        }

        // Color math
        if let Some(v) = st.cgwsel {
            self.cgwsel = v;
            self.color_math_control = v;
        }
        if let Some(v) = st.cgadsub {
            self.cgadsub = v;
            self.color_math_designation = v;
        }
        if let Some(v) = st.fixed_color {
            self.fixed_color = v;
        }

        // Windows
        if let Some(v) = st.window1_left {
            self.window1_left = v;
        }
        if let Some(v) = st.window1_right {
            self.window1_right = v;
        }
        if let Some(v) = st.window2_left {
            self.window2_left = v;
        }
        if let Some(v) = st.window2_right {
            self.window2_right = v;
        }
        if let Some(v) = st.window_bg_mask {
            self.window_bg_mask = v;
        }
        if let Some(v) = st.bg_window_logic {
            self.bg_window_logic = v;
        }
        if let Some(v) = st.window_obj_mask {
            self.window_obj_mask = v;
        }
        if let Some(v) = st.obj_window_logic {
            self.obj_window_logic = v;
        }
        if let Some(v) = st.window_color_mask {
            self.window_color_mask = v;
        }
        if let Some(v) = st.color_window_logic {
            self.color_window_logic = v;
        }
        if let Some(v) = st.tmw_mask {
            self.tmw_mask = v;
        }
        if let Some(v) = st.tsw_mask {
            self.tsw_mask = v;
        }

        // Display settings
        if let Some(v) = st.setini {
            self.setini = v;
        }
        if let Some(v) = st.pseudo_hires {
            self.pseudo_hires = v;
        }
        if let Some(v) = st.extbg {
            self.extbg = v;
        }
        if let Some(v) = st.overscan {
            self.overscan = v;
        }
        if let Some(v) = st.screen_display {
            self.screen_display = v;
            self.brightness = v & 0x0F;
        }
        if let Some(v) = st.interlace {
            self.interlace = v;
        }
        if let Some(v) = st.obj_interlace {
            self.obj_interlace = v;
        }
        if let Some(v) = st.force_no_blank {
            self.force_no_blank = v;
        }

        // BG config
        if let Some(v) = st.bg_tile_16 {
            self.bg_tile_16 = v;
        }
        if let Some(v) = st.bg_screen_size {
            self.bg_screen_size = v;
        }
        if let Some(v) = st.mode1_bg3_priority {
            self.mode1_bg3_priority = v;
        }

        // VRAM mapping
        if let Some(v) = st.vram_mapping {
            self.vram_mapping = v;
        }

        // Latches
        if let Some(v) = st.bgofs_latch {
            self.bgofs_latch = v;
        }
        if let Some(v) = st.bghofs_latch {
            self.bghofs_latch = v;
        }
        if let Some(v) = st.cgram_second {
            self.cgram_second = v;
        }
        if let Some(v) = st.cgram_latch_lo {
            self.cgram_latch_lo = v;
        }
        if let Some(v) = st.bg_mosaic {
            self.bg_mosaic = v;
        }
        if let Some(v) = st.oam_write_latch {
            self.oam_write_latch = v;
        }
        if let Some(v) = st.hdma_head_busy_until {
            self.hdma_head_busy_until = v;
        }
        if let Some(v) = st.framebuffer_rendering_enabled {
            self.framebuffer_rendering_enabled = v;
        }
        if let Some(v) = st.superfx_bypass_bg1_window {
            self.superfx_bypass_bg1_window = v;
        }
        if let Some(v) = st.superfx_authoritative_bg1_source {
            self.superfx_authoritative_bg1_source = v;
        }
        if !st.superfx_direct_buffer.is_empty() {
            self.superfx_direct_buffer = st.superfx_direct_buffer.clone();
        } else {
            self.superfx_direct_buffer.clear();
        }
        if let Some(v) = st.superfx_direct_height {
            self.superfx_direct_height = v;
        }
        if let Some(v) = st.superfx_direct_bpp {
            self.superfx_direct_bpp = v;
        }
        if let Some(v) = st.superfx_direct_mode {
            self.superfx_direct_mode = v;
        }
        self.superfx_direct_default_x_offset = Self::default_superfx_direct_x_offset(
            &self.superfx_direct_buffer,
            self.superfx_direct_height,
            self.superfx_direct_bpp,
            self.superfx_direct_mode,
            self.frame,
        );
        self.superfx_direct_default_y_offset = Self::default_superfx_direct_y_offset(
            &self.superfx_direct_buffer,
            self.superfx_direct_height,
            self.superfx_direct_bpp,
            self.superfx_direct_mode,
            self.frame,
        );
        if !st.superfx_tile_buffer.is_empty() {
            self.superfx_tile_buffer = st.superfx_tile_buffer.clone();
        } else {
            self.superfx_tile_buffer.clear();
        }
        if let Some(v) = st.superfx_tile_bpp {
            self.superfx_tile_bpp = v;
        }
        if let Some(v) = st.superfx_tile_mode {
            self.superfx_tile_mode = v;
        }
        if let Some(v) = st.wio_latch_enable {
            self.wio_latch_enable = v;
        }
        if let Some(v) = st.stat78_latch_flag {
            self.stat78_latch_flag = v;
        }
        if let Some(v) = st.interlace_field {
            self.interlace_field = v;
        }
        self.latched_inidisp = st.latched_inidisp;
        self.latched_tm = st.latched_tm;
        self.latched_ts = st.latched_ts;
        self.latched_tmw = st.latched_tmw;
        self.latched_tsw = st.latched_tsw;
        self.latched_cgwsel = st.latched_cgwsel;
        self.latched_cgadsub = st.latched_cgadsub;
        self.latched_fixed_color = st.latched_fixed_color;
        self.latched_setini = st.latched_setini;
        self.latched_vmadd_lo = st.latched_vmadd_lo;
        self.latched_vmadd_hi = st.latched_vmadd_hi;
        self.latched_cgadd = st.latched_cgadd;
        self.latched_vmain = st.latched_vmain;
        self.vmain_effect_pending = st.vmain_effect_pending;
        if let Some(v) = st.vmain_effect_ticks {
            self.vmain_effect_ticks = v;
        }
        self.cgadd_effect_pending = st.cgadd_effect_pending;
        if let Some(v) = st.cgadd_effect_ticks {
            self.cgadd_effect_ticks = v;
        }
        if let Some(v) = st.vmain_data_gap_ticks {
            self.vmain_data_gap_ticks = v;
        }
        if let Some(v) = st.cgram_data_gap_ticks {
            self.cgram_data_gap_ticks = v;
        }
        self.latched_wbglog = st.latched_wbglog;
        self.latched_wobjlog = st.latched_wobjlog;

        self.oam_dirty = true;
        self.update_line_render_state();
        self.prepare_line_window_luts();
    }
}
