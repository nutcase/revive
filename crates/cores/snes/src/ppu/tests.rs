use super::*;

#[test]
fn save_state_roundtrip_preserves_framebuffers() {
    let mut src = Ppu::new();
    src.framebuffer[0] = 0xFF112233;
    src.framebuffer[255] = 0xFF445566;
    src.subscreen_buffer[2] = 0xFF102030;
    src.subscreen_buffer[257] = 0xFF405060;
    src.render_framebuffer[1] = 0xFF778899;
    src.render_framebuffer[256] = 0xFFAABBCC;
    src.render_subscreen_buffer[3] = 0xFF0A0B0C;
    src.render_subscreen_buffer[258] = 0xFF0D0E0F;
    src.framebuffer_rendering_enabled = false;
    src.main_screen_designation_last_nonzero = 0x1F;
    src.vram_read_buf_lo = 0x12;
    src.vram_read_buf_hi = 0x34;
    src.cgram_read_second = true;
    src.interlace = true;
    src.obj_interlace = true;
    src.force_no_blank = true;
    src.superfx_bypass_bg1_window = true;
    src.superfx_authoritative_bg1_source = true;
    src.superfx_direct_buffer = vec![1, 2, 3, 4];
    src.superfx_direct_height = 160;
    src.superfx_direct_bpp = 4;
    src.superfx_direct_mode = 2;
    src.superfx_tile_buffer = vec![5, 6, 7, 8];
    src.superfx_tile_bpp = 4;
    src.superfx_tile_mode = 1;
    src.wio_latch_enable = true;
    src.stat78_latch_flag = true;
    src.interlace_field = true;
    src.sprite_overflow = true;
    src.sprite_time_over = true;
    src.sprite_overflow_latched = true;
    src.sprite_time_over_latched = true;

    let state = src.to_save_state();
    let mut dst = Ppu::new();
    dst.load_from_save_state(&state);

    assert_eq!(dst.framebuffer[0], 0xFF112233);
    assert_eq!(dst.framebuffer[255], 0xFF445566);
    assert_eq!(dst.subscreen_buffer[2], 0xFF102030);
    assert_eq!(dst.subscreen_buffer[257], 0xFF405060);
    assert_eq!(dst.render_framebuffer[1], 0xFF778899);
    assert_eq!(dst.render_framebuffer[256], 0xFFAABBCC);
    assert_eq!(dst.render_subscreen_buffer[3], 0xFF0A0B0C);
    assert_eq!(dst.render_subscreen_buffer[258], 0xFF0D0E0F);
    assert!(!dst.framebuffer_rendering_enabled);
    assert_eq!(dst.main_screen_designation_last_nonzero, 0x1F);
    assert_eq!(dst.vram_read_buf_lo, 0x12);
    assert_eq!(dst.vram_read_buf_hi, 0x34);
    assert!(dst.cgram_read_second);
    assert!(dst.interlace);
    assert!(dst.obj_interlace);
    assert!(dst.force_no_blank);
    assert!(dst.superfx_bypass_bg1_window);
    assert!(dst.superfx_authoritative_bg1_source);
    assert_eq!(dst.superfx_direct_buffer, vec![1, 2, 3, 4]);
    assert_eq!(dst.superfx_direct_height, 160);
    assert_eq!(dst.superfx_direct_bpp, 4);
    assert_eq!(dst.superfx_direct_mode, 2);
    assert_eq!(dst.superfx_tile_buffer, vec![5, 6, 7, 8]);
    assert_eq!(dst.superfx_tile_bpp, 4);
    assert_eq!(dst.superfx_tile_mode, 1);
    assert!(dst.wio_latch_enable);
    assert!(dst.stat78_latch_flag);
    assert!(dst.interlace_field);
    assert!(dst.sprite_overflow);
    assert!(dst.sprite_time_over);
    assert!(dst.sprite_overflow_latched);
    assert!(dst.sprite_time_over_latched);
}

#[test]
fn cgram_rgb555_to_rgb888_mapping() {
    let mut ppu = Ppu::new();
    // RGB555 (SNES): bit0-4=R, 5-9=G, 10-14=B.
    ppu.write_cgram_color(0, 0x001F); // red
    ppu.write_cgram_color(1, 0x03E0); // green
    ppu.write_cgram_color(2, 0x7C00); // blue
    ppu.write_cgram_color(3, 0x7FFF); // white

    assert_eq!(ppu.cgram_to_rgb(0), 0xFFFF0000);
    assert_eq!(ppu.cgram_to_rgb(1), 0xFF00FF00);
    assert_eq!(ppu.cgram_to_rgb(2), 0xFF0000FF);
    assert_eq!(ppu.cgram_to_rgb(3), 0xFFFFFFFF);
}

#[test]
fn coldata_updates_fixed_color_components() {
    let mut ppu = Ppu::new();
    // Set R=31, G=0, B=0
    ppu.write(0x32, 0x20 | 0x1F); // R enable + intensity
    ppu.write(0x32, 0x40 | 0x00); // G enable + intensity
    ppu.write(0x32, 0x80 | 0x00); // B enable + intensity
    assert_eq!(ppu.fixed_color_to_rgb(), 0xFFFF0000);

    // Set R=0, G=31, B=0
    ppu.write(0x32, 0x20 | 0x00);
    ppu.write(0x32, 0x40 | 0x1F);
    ppu.write(0x32, 0x80 | 0x00);
    assert_eq!(ppu.fixed_color_to_rgb(), 0xFF00FF00);

    // Set R=0, G=0, B=31
    ppu.write(0x32, 0x20 | 0x00);
    ppu.write(0x32, 0x40 | 0x00);
    ppu.write(0x32, 0x80 | 0x1F);
    assert_eq!(ppu.fixed_color_to_rgb(), 0xFF0000FF);
}

#[test]
fn ntsc_odd_field_non_interlace_shortens_scanline_240() {
    let mut ppu = Ppu::new();
    ppu.interlace = false;
    ppu.interlace_field = true;
    ppu.scanline = 240;
    ppu.cycle = 339;

    ppu.step(1);

    assert_eq!(ppu.scanline, 241);
    assert_eq!(ppu.cycle, 0);
}

#[test]
fn ntsc_even_field_interlace_adds_extra_scanline() {
    let mut ppu = Ppu::new();
    ppu.interlace = true;
    ppu.interlace_field = false;
    ppu.v_blank = true;
    ppu.scanline = 261;
    ppu.cycle = 340;

    ppu.step(1);

    assert_eq!(ppu.scanline, 262);
    assert_eq!(ppu.cycle, 0);
    assert_eq!(ppu.frame, 0);

    ppu.cycle = 340;
    ppu.step(1);

    assert_eq!(ppu.scanline, 0);
    assert_eq!(ppu.cycle, 0);
    assert_eq!(ppu.frame, 1);
}

#[test]
fn forced_blank_allows_non_hdma_graphics_writes_outside_vblank() {
    let mut ppu = Ppu::new();
    ppu.screen_display = 0x80;
    ppu.v_blank = false;
    ppu.h_blank = false;
    ppu.scanline = 42;
    ppu.cycle = 12;

    assert!(ppu.can_write_vram_non_hdma_now());
    assert!(ppu.can_write_cgram_non_hdma_now());
    assert!(ppu.can_write_oam_non_hdma_now());
}

#[test]
fn active_hblank_allows_non_hdma_vram_and_cgram_but_not_oam() {
    let mut ppu = Ppu::new();
    ppu.screen_display = 0x00;
    ppu.v_blank = false;
    ppu.h_blank = true;
    ppu.scanline = 10;
    ppu.cycle = ppu.first_hblank_dot().saturating_add(16);

    assert!(ppu.can_write_vram_non_hdma_now());
    assert!(ppu.can_write_cgram_non_hdma_now());
    assert!(!ppu.can_write_oam_non_hdma_now());
}

#[test]
fn invalid_oam_write_does_not_change_latch_memory_or_internal_address() {
    let mut ppu = Ppu::new();
    ppu.screen_display = 0x00;
    ppu.scanline = 0;
    ppu.cycle = 32;
    ppu.v_blank = false;
    ppu.h_blank = false;
    ppu.oam_write_latch = 0xCC;
    ppu.oam_internal_addr = 0;
    ppu.oam[0] = 0x11;
    ppu.oam[1] = 0x22;

    ppu.write(0x04, 0x77);

    assert_eq!(ppu.oam_write_latch, 0xCC);
    assert_eq!(ppu.oam_internal_addr, 0);
    assert_eq!(ppu.oam[0], 0x11);
    assert_eq!(ppu.oam[1], 0x22);
}

#[test]
fn invalid_cgram_write_does_not_stage_or_advance_address() {
    let mut ppu = Ppu::new();
    ppu.screen_display = 0x00;
    ppu.scanline = 0;
    ppu.cycle = 32;
    ppu.v_blank = false;
    ppu.h_blank = false;
    ppu.cgram_addr = 0;
    ppu.cgram_latch_lo = 0xAA;
    ppu.cgram_second = false;
    ppu.cgram[0] = 0x34;
    ppu.cgram[1] = 0x12;

    ppu.write(0x22, 0x56);

    assert_eq!(ppu.cgram_latch_lo, 0xAA);
    assert!(!ppu.cgram_second);
    assert_eq!(ppu.cgram_addr, 0);
    assert_eq!(ppu.cgram[0], 0x34);
    assert_eq!(ppu.cgram[1], 0x12);
}

#[test]
fn pending_vmadd_commit_updates_vram_address_and_read_latch() {
    let mut ppu = Ppu::new();
    ppu.screen_display = 0x80;
    ppu.vram_mapping = 0x00;
    ppu.vram_addr = 0;
    ppu.latched_vmadd_lo = Some(0x02);
    ppu.latched_vmadd_hi = Some(0x00);
    ppu.vram[4] = 0xAB;
    ppu.vram[5] = 0xCD;

    ppu.commit_pending_ctrl_if_any();

    assert_eq!(ppu.vram_addr, 0x0002);
    assert!(ppu.latched_vmadd_lo.is_none());
    assert!(ppu.latched_vmadd_hi.is_none());
    assert_eq!(ppu.vram_read_buf_lo, 0xAB);
    assert_eq!(ppu.vram_read_buf_hi, 0xCD);
}

#[test]
fn deferred_vmain_effect_updates_mapping_and_starts_gap() {
    let mut ppu = Ppu::new();
    ppu.vram_mapping = 0x00;
    ppu.vram_last_vmain = 0x00;
    ppu.vram_increment = 1;
    ppu.vmain_effect_pending = Some(0x81);
    ppu.vmain_effect_ticks = 1;
    ppu.vmain_data_gap_ticks = 0;

    ppu.tick_deferred_ctrl_effects();

    assert!(ppu.vmain_effect_pending.is_none());
    assert_eq!(ppu.vram_mapping, 0x81);
    assert_eq!(ppu.vram_last_vmain, 0x81);
    assert_eq!(ppu.vram_increment, 32);
    assert_eq!(
        ppu.vmain_data_gap_ticks,
        crate::debug_flags::vram_gap_after_vmain().saturating_sub(1)
    );
}

#[test]
fn deferred_cgadd_effect_resets_staging_and_starts_gap() {
    let mut ppu = Ppu::new();
    ppu.cgram_addr = 0x10;
    ppu.cgram_second = true;
    ppu.cgram_read_second = true;
    ppu.cgadd_effect_pending = Some(0x3C);
    ppu.cgadd_effect_ticks = 1;
    ppu.cgram_data_gap_ticks = 0;

    ppu.tick_deferred_ctrl_effects();

    assert!(ppu.cgadd_effect_pending.is_none());
    assert_eq!(ppu.cgram_addr, 0x3C);
    assert!(!ppu.cgram_second);
    assert!(!ppu.cgram_read_second);
    assert_eq!(
        ppu.cgram_data_gap_ticks,
        crate::debug_flags::cgram_gap_after_cgadd()
    );
}

#[test]
fn oamadd_low_write_updates_internal_addr_and_gap() {
    let mut ppu = Ppu::new();
    ppu.oam_addr = 0x100;
    ppu.oam_priority_rotation_enabled = true;
    ppu.oam_eval_base = 0;
    ppu.oam_data_gap_ticks = 0;

    ppu.write(0x02, 0x02);

    assert_eq!(ppu.oam_addr, 0x102);
    assert_eq!(ppu.oam_internal_addr, 0x204);
    assert_eq!(
        ppu.oam_eval_base,
        ((ppu.oam_internal_addr >> 2) & 0x7F) as u8
    );
    assert_eq!(
        ppu.oam_data_gap_ticks,
        crate::debug_flags::oam_gap_after_oamadd()
    );
}

#[test]
fn oamadd_high_write_updates_rotation_mode_and_eval_base() {
    let mut ppu = Ppu::new();
    ppu.oam_addr = 0x002;
    ppu.oam_internal_addr = 0x004;
    ppu.oam_priority_rotation_enabled = false;
    ppu.oam_eval_base = 0;
    ppu.oam_data_gap_ticks = 0;

    ppu.write(0x03, 0x81);

    assert_eq!(ppu.oam_addr, 0x102);
    assert_eq!(ppu.oam_internal_addr, 0x204);
    assert!(ppu.oam_priority_rotation_enabled);
    assert_eq!(
        ppu.oam_eval_base,
        ((ppu.oam_internal_addr >> 2) & 0x7F) as u8
    );
    assert_eq!(
        ppu.oam_data_gap_ticks,
        crate::debug_flags::oam_gap_after_oamadd()
    );
}

#[test]
fn oamadd_high_write_disabling_rotation_resets_eval_base() {
    let mut ppu = Ppu::new();
    ppu.oam_addr = 0x102;
    ppu.oam_internal_addr = 0x204;
    ppu.oam_priority_rotation_enabled = true;
    ppu.oam_eval_base = ((ppu.oam_internal_addr >> 2) & 0x7F) as u8;

    ppu.write(0x03, 0x01);

    assert_eq!(ppu.oam_addr, 0x102);
    assert_eq!(ppu.oam_internal_addr, 0x204);
    assert!(!ppu.oam_priority_rotation_enabled);
    assert_eq!(ppu.oam_eval_base, 0);
}

#[test]
fn enter_vblank_resets_oam_internal_addr_when_display_enabled() {
    let mut ppu = Ppu::new();
    ppu.screen_display = 0x00;
    ppu.oam_addr = 0x102;
    ppu.oam_internal_addr = 0x000;
    ppu.oam_priority_rotation_enabled = true;
    ppu.oam_eval_base = 0;

    ppu.enter_vblank();

    assert!(ppu.v_blank);
    assert_eq!(ppu.oam_internal_addr, 0x204);
    assert_eq!(
        ppu.oam_eval_base,
        ((ppu.oam_internal_addr >> 2) & 0x7F) as u8
    );
}

#[test]
fn enter_vblank_does_not_reset_oam_internal_addr_during_forced_blank() {
    let mut ppu = Ppu::new();
    ppu.screen_display = 0x80;
    ppu.oam_addr = 0x102;
    ppu.oam_internal_addr = 0x066;
    ppu.oam_priority_rotation_enabled = true;
    ppu.oam_eval_base = 0x19;

    ppu.enter_vblank();

    assert!(ppu.v_blank);
    assert_eq!(ppu.oam_internal_addr, 0x066);
    assert_eq!(ppu.oam_eval_base, 0x19);
}

#[test]
fn forced_blank_deactivation_resets_oam_internal_addr() {
    let mut ppu = Ppu::new();
    ppu.oam_addr = 0x102;
    ppu.oam_internal_addr = 0x000;
    ppu.oam_priority_rotation_enabled = true;
    ppu.oam_eval_base = 0;

    ppu.maybe_reset_oam_on_inidisp(0x80, 0x00);

    assert_eq!(ppu.oam_internal_addr, 0x204);
    assert_eq!(
        ppu.oam_eval_base,
        ((ppu.oam_internal_addr >> 2) & 0x7F) as u8
    );
}

#[test]
fn latched_inidisp_toggle_does_not_rebuild_and_clear_prior_scanlines() {
    let mut ppu = Ppu::new();
    ppu.screen_display = 0x0F;
    ppu.brightness = 0x0F;
    ppu.scanline = 120;
    ppu.cycle = 0;
    ppu.render_framebuffer[0] = 0xFF12_3456;
    ppu.render_subscreen_buffer[0] = 0x0000_00AA;
    ppu.latched_inidisp = Some(0x80);

    ppu.commit_latched_display_regs();

    assert_eq!(ppu.screen_display, 0x80);
    assert_eq!(ppu.brightness, 0x00);
    assert_eq!(ppu.render_framebuffer[0], 0xFF12_3456);
    assert_eq!(ppu.render_subscreen_buffer[0], 0x0000_00AA);
}

#[test]
fn immediate_inidisp_toggle_does_not_rebuild_and_clear_prior_scanlines() {
    let mut ppu = Ppu::new();
    ppu.screen_display = 0x0F;
    ppu.brightness = 0x0F;
    ppu.scanline = 120;
    ppu.cycle = 128;
    ppu.render_framebuffer[0] = 0xFF12_3456;
    ppu.render_subscreen_buffer[0] = 0x0000_00AA;

    ppu.write(0x00, 0x80);

    assert_eq!(ppu.screen_display, 0x80);
    assert_eq!(ppu.brightness, 0x00);
    assert_eq!(ppu.render_framebuffer[0], 0xFF12_3456);
    assert_eq!(ppu.render_subscreen_buffer[0], 0x0000_00AA);
}

#[test]
fn enter_vblank_sets_nmi_flag_and_resets_rdnmi_consumed_state() {
    let mut ppu = Ppu::new();
    ppu.nmi_enabled = false;
    ppu.nmi_flag = false;
    ppu.nmi_latched = false;
    ppu.rdnmi_read_in_vblank = true;

    ppu.enter_vblank();

    assert!(ppu.v_blank);
    assert!(ppu.nmi_flag);
    assert!(!ppu.nmi_latched);
    assert!(!ppu.rdnmi_read_in_vblank);
}

#[test]
fn enter_vblank_latches_nmi_only_when_enabled() {
    let mut ppu = Ppu::new();
    ppu.nmi_enabled = true;
    ppu.nmi_flag = false;
    ppu.nmi_latched = false;

    ppu.enter_vblank();
    assert!(ppu.nmi_flag);
    assert!(ppu.nmi_latched);

    let mut ppu = Ppu::new();
    ppu.nmi_enabled = false;
    ppu.nmi_flag = false;
    ppu.nmi_latched = false;

    ppu.enter_vblank();
    assert!(ppu.nmi_flag);
    assert!(!ppu.nmi_latched);
}

#[test]
fn clear_nmi_only_clears_latch_not_rdnmi_flag() {
    let mut ppu = Ppu::new();
    ppu.nmi_flag = true;
    ppu.nmi_latched = true;

    ppu.clear_nmi();

    assert!(ppu.nmi_flag);
    assert!(!ppu.nmi_latched);
}

#[test]
fn stat78_read_reports_and_clears_latch_flag() {
    let mut ppu = Ppu::new();
    ppu.interlace_field = true;
    ppu.stat78_latch_flag = true;
    ppu.ophct_second = true;
    ppu.opvct_second = true;

    let value = ppu.read(0x3F);

    assert_eq!(value & 0xC0, 0xC0);
    assert_eq!(value & 0x0F, 0x03);
    assert!(!ppu.stat78_latch_flag);
    assert!(!ppu.ophct_second);
    assert!(!ppu.opvct_second);
}

#[test]
fn enter_vblank_toggles_interlace_field_each_time() {
    let mut ppu = Ppu::new();
    ppu.interlace_field = false;

    ppu.enter_vblank();
    assert!(ppu.interlace_field);

    ppu.v_blank = false;
    ppu.enter_vblank();
    assert!(!ppu.interlace_field);
}

#[test]
fn exit_vblank_clears_sprite_latches_but_keeps_nmi_flag() {
    let mut ppu = Ppu::new();
    ppu.v_blank = true;
    ppu.nmi_flag = true;
    ppu.nmi_latched = true;
    ppu.rdnmi_read_in_vblank = true;
    ppu.sprite_overflow_latched = true;
    ppu.sprite_time_over_latched = true;

    ppu.exit_vblank();

    assert!(!ppu.v_blank);
    assert!(ppu.nmi_flag);
    assert!(ppu.nmi_latched);
    assert!(!ppu.rdnmi_read_in_vblank);
    assert!(!ppu.sprite_overflow_latched);
    assert!(!ppu.sprite_time_over_latched);
}

#[test]
fn slhv_read_latches_hv_counters_one_dot_later() {
    let mut ppu = Ppu::new();
    ppu.scanline = 0x34;
    ppu.cycle = 0x56;
    ppu.hv_latched_h = 0;
    ppu.hv_latched_v = 0;
    ppu.stat78_latch_flag = false;

    let value = ppu.read(0x37);
    assert_eq!(value, 0);
    assert_eq!(ppu.slhv_latch_pending_dots, 1);
    assert_eq!(ppu.hv_latched_h, 0);
    assert_eq!(ppu.hv_latched_v, 0);

    ppu.step(1);

    assert_eq!(ppu.hv_latched_h, 0x57);
    assert_eq!(ppu.hv_latched_v, 0x34);
    assert!(ppu.stat78_latch_flag);
    assert_eq!(ppu.slhv_latch_pending_dots, 0);
}

#[test]
fn ophct_and_opvct_reads_toggle_low_then_high_bit() {
    let mut ppu = Ppu::new();
    ppu.hv_latched_h = 0x123;
    ppu.hv_latched_v = 0x0AB;

    assert_eq!(ppu.read(0x3C), 0x23);
    assert_eq!(ppu.read(0x3C), 0x01);
    assert_eq!(ppu.read(0x3D), 0xAB);
    assert_eq!(ppu.read(0x3D), 0x00);
}

#[test]
fn latch_hv_counters_resets_ophct_and_opvct_selectors() {
    let mut ppu = Ppu::new();
    ppu.hv_latched_h = 0x155;
    ppu.hv_latched_v = 0x1AA;

    let _ = ppu.read(0x3C);
    let _ = ppu.read(0x3D);
    assert!(ppu.ophct_second);
    assert!(ppu.opvct_second);

    ppu.scanline = 0x12;
    ppu.cycle = 0x34;
    ppu.latch_hv_counters();

    assert!(!ppu.ophct_second);
    assert!(!ppu.opvct_second);
    assert_eq!(ppu.read(0x3C), 0x34);
    assert_eq!(ppu.read(0x3D), 0x12);
}

#[test]
fn ophct_opvct_reads_realize_pending_slhv_latch_immediately() {
    let mut ppu = Ppu::new();
    ppu.scanline = 0x34;
    ppu.cycle = 0x56;
    ppu.hv_latched_h = 0;
    ppu.hv_latched_v = 0;

    let _ = ppu.read(0x37);

    assert_eq!(ppu.read(0x3D), 0x34);
    assert_eq!(ppu.slhv_latch_pending_dots, 0);
    assert!(ppu.stat78_latch_flag);
}

#[test]
fn invalid_vram_low_write_keeps_memory_unchanged_and_increments_only_in_low_mode() {
    let mut ppu = Ppu::new();
    ppu.screen_display = 0x00;
    ppu.scanline = 0;
    ppu.cycle = 32;
    ppu.v_blank = false;
    ppu.h_blank = false;
    ppu.vram_increment = 1;
    ppu.vram_addr = 0;
    ppu.vram[0] = 0x00;

    ppu.vram_mapping = 0x00;
    ppu.write(0x18, 0x12);
    assert_eq!(ppu.vram[0], 0x00);
    assert_eq!(ppu.vram_addr, 1);

    ppu.vram_addr = 0;
    ppu.vram_mapping = 0x80;
    ppu.write(0x18, 0x34);
    assert_eq!(ppu.vram[0], 0x00);
    assert_eq!(ppu.vram_addr, 0);
}

#[test]
fn invalid_vram_high_write_keeps_memory_unchanged_and_increments_only_in_high_mode() {
    let mut ppu = Ppu::new();
    ppu.screen_display = 0x00;
    ppu.scanline = 0;
    ppu.cycle = 32;
    ppu.v_blank = false;
    ppu.h_blank = false;
    ppu.vram_increment = 1;
    ppu.vram_addr = 0;
    ppu.vram[1] = 0x00;

    ppu.vram_mapping = 0x80;
    ppu.write(0x19, 0x12);
    assert_eq!(ppu.vram[1], 0x00);
    assert_eq!(ppu.vram_addr, 1);

    ppu.vram_addr = 0;
    ppu.vram_mapping = 0x00;
    ppu.write(0x19, 0x34);
    assert_eq!(ppu.vram[1], 0x00);
    assert_eq!(ppu.vram_addr, 0);
}

#[test]
fn vblank_window_blocks_first_scanline_before_head_guard() {
    assert!(!Ppu::vblank_window_open(225, 3, 225, 261, 340, 4, 0));
    assert!(Ppu::vblank_window_open(225, 4, 225, 261, 340, 4, 0));
}

#[test]
fn vblank_window_blocks_last_scanline_after_tail_guard() {
    assert!(Ppu::vblank_window_open(261, 336, 225, 261, 340, 0, 4));
    assert!(!Ppu::vblank_window_open(261, 337, 225, 261, 340, 0, 4));
}

#[test]
fn vblank_window_stays_closed_before_vblank_begins() {
    assert!(!Ppu::vblank_window_open(224, 100, 225, 261, 340, 0, 0));
}

#[test]
fn hblank_window_blocks_before_head_guard() {
    assert!(!Ppu::hblank_window_open(281, 278, 340, 4, 0, 0));
    assert!(Ppu::hblank_window_open(282, 278, 340, 4, 0, 0));
}

#[test]
fn hblank_window_blocks_after_tail_guard() {
    assert!(Ppu::hblank_window_open(336, 278, 340, 0, 4, 0));
    assert!(!Ppu::hblank_window_open(337, 278, 340, 0, 4, 0));
}

#[test]
fn hblank_window_respects_busy_until_guard() {
    assert!(!Ppu::hblank_window_open(289, 278, 340, 4, 0, 290));
    assert!(Ppu::hblank_window_open(290, 278, 340, 4, 0, 290));
}

#[test]
fn cgram_non_hdma_write_requires_actual_hblank_cycle() {
    let mut ppu = Ppu::new();
    ppu.screen_display = 0x00;
    ppu.v_blank = false;
    ppu.h_blank = true;
    ppu.scanline = 10;
    ppu.cgram_data_gap_ticks = 0;

    ppu.cycle = ppu.first_hblank_dot().saturating_sub(1);
    assert!(!ppu.can_write_cgram_non_hdma_now());

    ppu.cycle = ppu.first_hblank_dot();
    assert!(!ppu.can_write_cgram_non_hdma_now());

    ppu.cycle = ppu.first_hblank_dot() + crate::debug_flags::cgram_mdma_head();
    assert!(ppu.can_write_cgram_non_hdma_now());
}

#[test]
fn cgram_non_hdma_write_respects_gap_ticks_inside_hblank() {
    let mut ppu = Ppu::new();
    ppu.screen_display = 0x00;
    ppu.v_blank = false;
    ppu.h_blank = true;
    ppu.scanline = 10;
    ppu.cycle = ppu.first_hblank_dot();
    ppu.cgram_data_gap_ticks = 1;

    assert!(!ppu.can_write_cgram_non_hdma_now());
}

#[test]
fn cgram_non_hdma_write_respects_tail_guard() {
    let mut ppu = Ppu::new();
    ppu.screen_display = 0x00;
    ppu.v_blank = false;
    ppu.h_blank = true;
    ppu.scanline = 10;
    ppu.cgram_data_gap_ticks = 0;

    ppu.cycle = ppu
        .last_dot_index()
        .saturating_sub(crate::debug_flags::cgram_mdma_tail());
    assert!(ppu.can_write_cgram_non_hdma_now());

    ppu.cycle = ppu
        .last_dot_index()
        .saturating_sub(crate::debug_flags::cgram_mdma_tail())
        + 1;
    assert!(!ppu.can_write_cgram_non_hdma_now());
}

#[test]
fn oam_vblank_write_window_blocks_before_head_guard() {
    assert!(!Ppu::oam_vblank_write_window_open(
        225, 3, 225, 261, 340, 4, 0, false, 0
    ));
    assert!(Ppu::oam_vblank_write_window_open(
        225, 4, 225, 261, 340, 4, 0, false, 0
    ));
}

#[test]
fn oam_vblank_write_window_blocks_after_tail_guard() {
    assert!(Ppu::oam_vblank_write_window_open(
        261, 336, 225, 261, 340, 0, 4, false, 0
    ));
    assert!(!Ppu::oam_vblank_write_window_open(
        261, 337, 225, 261, 340, 0, 4, false, 0
    ));
}

#[test]
fn oam_vblank_write_window_respects_gap_block() {
    assert!(!Ppu::oam_vblank_write_window_open(
        230, 100, 225, 261, 340, 0, 0, true, 1
    ));
    assert!(Ppu::oam_vblank_write_window_open(
        230, 100, 225, 261, 340, 0, 0, true, 0
    ));
}

#[test]
fn vram_non_hdma_write_respects_head_guard() {
    let mut ppu = Ppu::new();
    ppu.screen_display = 0x00;
    ppu.v_blank = false;
    ppu.h_blank = true;
    ppu.scanline = 10;
    ppu.vmain_data_gap_ticks = 0;
    ppu.hdma_head_busy_until = 0;

    ppu.cycle = ppu
        .first_hblank_dot()
        .saturating_add(crate::debug_flags::vram_mdma_head())
        .saturating_sub(1);
    assert!(!ppu.can_write_vram_non_hdma_now());

    ppu.cycle = ppu
        .first_hblank_dot()
        .saturating_add(crate::debug_flags::vram_mdma_head());
    assert!(ppu.can_write_vram_non_hdma_now());
}

#[test]
fn vram_non_hdma_write_respects_busy_until_guard() {
    let mut ppu = Ppu::new();
    ppu.screen_display = 0x00;
    ppu.v_blank = false;
    ppu.h_blank = true;
    ppu.scanline = 10;
    ppu.vmain_data_gap_ticks = 0;
    ppu.hdma_head_busy_until = ppu
        .first_hblank_dot()
        .saturating_add(crate::debug_flags::vram_mdma_head())
        .saturating_add(5);

    ppu.cycle = ppu.hdma_head_busy_until.saturating_sub(1);
    assert!(!ppu.can_write_vram_non_hdma_now());

    ppu.cycle = ppu.hdma_head_busy_until;
    assert!(ppu.can_write_vram_non_hdma_now());
}

#[test]
fn vram_non_hdma_write_respects_gap_ticks_inside_hblank() {
    let mut ppu = Ppu::new();
    ppu.screen_display = 0x00;
    ppu.v_blank = false;
    ppu.h_blank = true;
    ppu.scanline = 10;
    ppu.hdma_head_busy_until = 0;
    ppu.cycle = ppu
        .first_hblank_dot()
        .saturating_add(crate::debug_flags::vram_mdma_head());
    ppu.vmain_data_gap_ticks = 1;

    assert!(!ppu.can_write_vram_non_hdma_now());
}

#[test]
fn vram_non_hdma_write_respects_tail_guard() {
    let mut ppu = Ppu::new();
    ppu.screen_display = 0x00;
    ppu.v_blank = false;
    ppu.h_blank = true;
    ppu.scanline = 10;
    ppu.vmain_data_gap_ticks = 0;
    ppu.hdma_head_busy_until = 0;

    ppu.cycle = ppu
        .last_dot_index()
        .saturating_sub(crate::debug_flags::vram_mdma_tail());
    assert!(ppu.can_write_vram_non_hdma_now());

    ppu.cycle = ppu
        .last_dot_index()
        .saturating_sub(crate::debug_flags::vram_mdma_tail())
        + 1;
    assert!(!ppu.can_write_vram_non_hdma_now());
}
