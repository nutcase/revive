use super::{DmaTarget, FRAME_HEIGHT, FRAME_WIDTH, Vdp, encode_md_color};

#[test]
fn supports_vram_read_write() {
    let mut vdp = Vdp::new();
    vdp.write_vram_u8(0x1234, 0xAB);
    assert_eq!(vdp.read_vram_u8(0x1234), 0xAB);
}

#[test]
fn supports_cram_read_write() {
    let mut vdp = Vdp::new();
    vdp.write_cram_u16(3, encode_md_color(7, 0, 0));
    assert_eq!(vdp.read_cram_u16(3), encode_md_color(7, 0, 0));
}

#[test]
fn supports_vsram_read_write() {
    let mut vdp = Vdp::new();
    vdp.write_vsram_u16(5, 0x1ABC);
    assert_eq!(vdp.read_vsram_u16(5), 0x02BC);
}

#[test]
fn supports_control_and_data_ports_for_vram_write() {
    let mut vdp = Vdp::new();
    vdp.write_control_port(0x4000);
    vdp.write_control_port(0x0000);
    vdp.write_data_port(0xABCD);
    assert_eq!(vdp.read_vram_u8(0), 0xAB);
    assert_eq!(vdp.read_vram_u8(1), 0xCD);
}

#[test]
fn respects_auto_increment_register_for_data_port_writes() {
    let mut vdp = Vdp::new();
    // Set register 15 (auto increment) to 4.
    vdp.write_control_port(0x8F04);
    // VRAM write command @ 0x0000.
    vdp.write_control_port(0x4000);
    vdp.write_control_port(0x0000);
    vdp.write_data_port(0xABCD);
    vdp.write_data_port(0x1234);

    assert_eq!(vdp.read_vram_u8(0x0000), 0xAB);
    assert_eq!(vdp.read_vram_u8(0x0001), 0xCD);
    assert_eq!(vdp.read_vram_u8(0x0004), 0x12);
    assert_eq!(vdp.read_vram_u8(0x0005), 0x34);
}

#[test]
fn allows_zero_auto_increment_for_data_port_writes() {
    let mut vdp = Vdp::new();
    // Set register 15 (auto increment) to 0.
    vdp.write_control_port(0x8F00);
    // VRAM write command @ 0x0000.
    vdp.write_control_port(0x4000);
    vdp.write_control_port(0x0000);
    vdp.write_data_port(0xABCD);
    vdp.write_data_port(0x1234);

    // Address does not advance, so second word overwrites the first.
    assert_eq!(vdp.read_vram_u8(0x0000), 0x12);
    assert_eq!(vdp.read_vram_u8(0x0001), 0x34);
    assert_eq!(vdp.read_vram_u8(0x0002), 0x00);
    assert_eq!(vdp.read_vram_u8(0x0003), 0x00);
}

#[test]
fn increments_address_on_data_port_read() {
    let mut vdp = Vdp::new();
    vdp.write_vram_u8(0x0000, 0x11);
    vdp.write_vram_u8(0x0001, 0x22);
    vdp.write_vram_u8(0x0002, 0x33);
    vdp.write_vram_u8(0x0003, 0x44);
    // VRAM read command @ 0x0000.
    vdp.write_control_port(0x0000);
    vdp.write_control_port(0x0000);

    assert_eq!(vdp.read_data_port(), 0x1122);
    assert_eq!(vdp.read_data_port(), 0x3344);
}

#[test]
fn vram_read_command_prefetches_buffer_before_first_data_port_read() {
    let mut vdp = Vdp::new();
    vdp.write_vram_u8(0x0000, 0x11);
    vdp.write_vram_u8(0x0001, 0x22);
    // VRAM read command @ 0x0000.
    vdp.write_control_port(0x0000);
    vdp.write_control_port(0x0000);

    // Mutate backing VRAM after read command setup.
    // First read should still return the prefetched value.
    vdp.write_vram_u8(0x0000, 0xAA);
    vdp.write_vram_u8(0x0001, 0xBB);

    assert_eq!(vdp.read_data_port(), 0x1122);
}

#[test]
fn zero_auto_increment_keeps_data_port_read_address_fixed() {
    let mut vdp = Vdp::new();
    vdp.write_vram_u8(0x0000, 0x11);
    vdp.write_vram_u8(0x0001, 0x22);
    vdp.write_vram_u8(0x0002, 0x33);
    vdp.write_vram_u8(0x0003, 0x44);
    // Auto-increment = 0.
    vdp.write_control_port(0x8F00);
    // VRAM read command @ 0x0000.
    vdp.write_control_port(0x0000);
    vdp.write_control_port(0x0000);

    assert_eq!(vdp.read_data_port(), 0x1122);
    assert_eq!(vdp.read_data_port(), 0x1122);
}

#[test]
fn supports_control_and_data_ports_for_cram_write() {
    let mut vdp = Vdp::new();
    vdp.write_control_port(0xC000);
    vdp.write_control_port(0x0000);
    vdp.write_data_port(0x0E0E);
    assert_eq!(vdp.read_cram_u16(0), 0x0E0E);
}

#[test]
fn supports_control_and_data_ports_for_vsram_write_and_read() {
    let mut vdp = Vdp::new();
    // VSRAM write command @ 0x0000.
    vdp.write_control_port(0x4000);
    vdp.write_control_port(0x0010);
    vdp.write_data_port(0x17AB);
    assert_eq!(vdp.read_vsram_u16(0), 0x07AB);

    // VSRAM read command @ 0x0000.
    vdp.write_control_port(0x0000);
    vdp.write_control_port(0x0010);
    assert_eq!(vdp.read_data_port(), 0x07AB);
}

#[test]
fn register_write_updates_name_table_base() {
    let mut vdp = Vdp::new();
    assert_eq!(vdp.nametable_base(), 0xC000);

    // Register 2 = 0x20 -> base 0x8000
    vdp.write_control_port(0x8220);
    assert_eq!(vdp.nametable_base(), 0x8000);
}

#[test]
fn initial_frame_starts_black() {
    let vdp = Vdp::new();
    assert_eq!(vdp.frame_buffer().len(), FRAME_WIDTH * FRAME_HEIGHT * 3);
    assert!(vdp.frame_buffer().iter().all(|&b| b == 0));
}

#[test]
fn frame_buffer_updates_after_vram_change() {
    let mut vdp = Vdp::new();
    let before = vdp.frame_buffer()[0..3].to_vec();

    // Top-left pixel uses tile 0, row 0, high nibble.
    vdp.write_cram_u16(2, encode_md_color(7, 7, 7));
    vdp.write_vram_u8(0, 0x20);
    let frame_ready = vdp.step(Vdp::CYCLES_PER_FRAME as u32);
    assert!(frame_ready);

    let after = vdp.frame_buffer()[0..3].to_vec();
    assert_ne!(before, after);
}

#[test]
fn display_disable_register_blacks_out_frame() {
    let mut vdp = Vdp::new();
    // Register 1 = 0x00 (display disable)
    vdp.write_control_port(0x8100);
    let frame_ready = vdp.step(Vdp::CYCLES_PER_FRAME as u32);
    assert!(frame_ready);
    assert!(vdp.frame_buffer().iter().all(|&b| b == 0));
}

#[test]
fn control_port_read_reports_and_clears_vblank() {
    let mut vdp = Vdp::new();
    vdp.step(Vdp::CYCLES_PER_FRAME as u32);
    let _ = vdp.read_control_port();

    let status_after = vdp.read_control_port();
    assert_eq!(status_after & super::STATUS_VBLANK, 0);
}

#[test]
fn status_odd_frame_bit_toggles_each_frame_in_interlace_mode() {
    let mut vdp = Vdp::new();
    // Reg 12 bits2:1 = 11 (interlace mode), keep H40 enabled.
    vdp.write_control_port(0x8C87);

    let status0 = vdp.read_control_port();
    assert_eq!(status0 & super::STATUS_ODD_FRAME, 0);

    vdp.step(Vdp::CYCLES_PER_FRAME as u32);
    let status1 = vdp.read_control_port();
    assert_ne!(status1 & super::STATUS_ODD_FRAME, 0);

    vdp.step(Vdp::CYCLES_PER_FRAME as u32);
    let status2 = vdp.read_control_port();
    assert_eq!(status2 & super::STATUS_ODD_FRAME, 0);
}

#[test]
fn interlace_mode_2_plane_pixel_alternates_between_field_rows() {
    let mut vdp = Vdp::new();
    vdp.vram.fill(0);
    vdp.cram.fill(0);
    vdp.vsram.fill(0);

    // Reg 12 bits2:1 = 11 (interlace mode 2), keep H40 enabled.
    vdp.write_control_port(0x8C87);
    vdp.write_cram_u16(1, encode_md_color(7, 0, 0));
    vdp.write_cram_u16(2, encode_md_color(0, 7, 0));

    // Tile 0: interlace field row 0 = color 1, field row 1 = color 2.
    for i in 0..4u16 {
        vdp.write_vram_u8(i, 0x11);
        vdp.write_vram_u8(4 + i, 0x22);
    }

    vdp.step(Vdp::CYCLES_PER_FRAME as u32);
    let field_a = vdp.frame_buffer()[0..3].to_vec();

    vdp.step(Vdp::CYCLES_PER_FRAME as u32);
    let field_b = vdp.frame_buffer()[0..3].to_vec();

    assert_ne!(field_a, field_b);
    let mut colors = vec![field_a, field_b];
    colors.sort();
    assert_eq!(colors, vec![vec![0, 252, 0], vec![252, 0, 0]]);
}

#[test]
fn interlace_mode_2_sprite_pixel_alternates_between_field_rows() {
    let mut vdp = Vdp::new();
    vdp.vram.fill(0);
    vdp.cram.fill(0);
    vdp.vsram.fill(0);

    vdp.write_control_port(0x8570); // SAT base @ 0xE000
    vdp.write_control_port(0x8C87); // Interlace mode 2
    vdp.write_cram_u16(1, encode_md_color(7, 0, 0));
    vdp.write_cram_u16(2, encode_md_color(0, 7, 0));

    // Sprite tile 3: field row 0 = color 1, field row 1 = color 2.
    let tile_base = 3u16 * 64;
    for i in 0..4u16 {
        vdp.write_vram_u8(tile_base + i, 0x11);
        vdp.write_vram_u8(tile_base + 4 + i, 0x22);
    }

    let sat = 0xE000u16;
    // Y position = 128 (screen y = 0)
    vdp.write_vram_u8(sat, 0x00);
    vdp.write_vram_u8(sat + 1, 0x80);
    // Size/link: 1x1 tile, end of list.
    vdp.write_vram_u8(sat + 2, 0x00);
    vdp.write_vram_u8(sat + 3, 0x00);
    // Attr: tile index = 3.
    vdp.write_vram_u8(sat + 4, 0x00);
    vdp.write_vram_u8(sat + 5, 0x03);
    // X position = 128 (screen x = 0)
    vdp.write_vram_u8(sat + 6, 0x00);
    vdp.write_vram_u8(sat + 7, 0x80);

    vdp.step(Vdp::CYCLES_PER_FRAME as u32);
    let field_a = vdp.frame_buffer()[0..3].to_vec();

    vdp.step(Vdp::CYCLES_PER_FRAME as u32);
    let field_b = vdp.frame_buffer()[0..3].to_vec();

    assert_ne!(field_a, field_b);
    let mut colors = vec![field_a, field_b];
    colors.sort();
    assert_eq!(colors, vec![vec![0, 252, 0], vec![252, 0, 0]]);
}

#[test]
fn interlace_mode_2_sprite_y_position_uses_half_line_units() {
    let mut vdp = Vdp::new();
    vdp.vram.fill(0);
    vdp.cram.fill(0);
    vdp.vsram.fill(0);

    vdp.write_control_port(0x8570); // SAT base @ 0xE000
    vdp.write_control_port(0x8C87); // Interlace mode 2
    vdp.write_cram_u16(1, encode_md_color(7, 0, 0));

    // Sprite tile 3: first two interlace rows are opaque.
    let tile_base = 3u16 * 64;
    for i in 0..8u16 {
        vdp.write_vram_u8(tile_base + i, 0x11);
    }

    let sat = 0xE000u16;
    // Y position = 129 (one half-line). In interlace mode 2 this maps to y=0.
    vdp.write_vram_u8(sat, 0x00);
    vdp.write_vram_u8(sat + 1, 0x81);
    // Size/link: 1x1 tile, end of list.
    vdp.write_vram_u8(sat + 2, 0x00);
    vdp.write_vram_u8(sat + 3, 0x00);
    // Attr: tile index = 3.
    vdp.write_vram_u8(sat + 4, 0x00);
    vdp.write_vram_u8(sat + 5, 0x03);
    // X position = 128 (screen x = 0)
    vdp.write_vram_u8(sat + 6, 0x00);
    vdp.write_vram_u8(sat + 7, 0x80);

    vdp.step(Vdp::CYCLES_PER_FRAME as u32);
    assert_eq!(&vdp.frame_buffer()[0..3], &[252, 0, 0]);
}

#[test]
fn vblank_interrupt_becomes_pending_when_enabled() {
    let mut vdp = Vdp::new();
    // Register 1 = 0x60 (display enable + v-interrupt enable)
    vdp.write_control_port(0x8160);
    vdp.step(Vdp::CYCLES_PER_FRAME as u32);

    assert_eq!(vdp.pending_interrupt_level(), Some(6));
    vdp.acknowledge_interrupt(6);
    assert_eq!(vdp.pending_interrupt_level(), None);
}

#[test]
fn hblank_interrupt_becomes_pending_when_enabled() {
    let mut vdp = Vdp::new();
    // Register 0 bit4 enables H-INT. Register 10 = 0 triggers every line.
    vdp.write_control_port(0x8010);
    vdp.write_control_port(0x8A00);
    let cycles_per_line = (Vdp::CYCLES_PER_FRAME / Vdp::TOTAL_LINES) as u32;
    vdp.step(cycles_per_line * 2);

    assert_eq!(vdp.pending_interrupt_level(), Some(4));
    vdp.acknowledge_interrupt(4);
    assert_eq!(vdp.pending_interrupt_level(), None);
}

#[test]
fn hblank_interrupt_line_is_stable_across_frames() {
    let mut vdp = Vdp::new();
    // Enable H-INT and use a large line interval to surface frame-boundary drift.
    vdp.write_control_port(0x8010);
    vdp.write_control_port(0x8AB8);

    let mut first_hint_line_by_frame: [Option<u8>; 3] = [None, None, None];
    while vdp.frame_count() < 3 {
        vdp.step(1);
        if vdp.pending_interrupt_level() == Some(4) {
            let frame = vdp.frame_count() as usize;
            if frame < first_hint_line_by_frame.len() && first_hint_line_by_frame[frame].is_none() {
                let line = (vdp.read_hv_counter() >> 8) as u8;
                first_hint_line_by_frame[frame] = Some(line);
            }
            vdp.acknowledge_interrupt(4);
        }
    }

    let line_frame1 = first_hint_line_by_frame[1].expect("H-INT line for frame 1");
    let line_frame2 = first_hint_line_by_frame[2].expect("H-INT line for frame 2");
    assert_eq!(line_frame1, line_frame2);
}

#[test]
fn vblank_interrupt_has_priority_over_hblank_interrupt() {
    let mut vdp = Vdp::new();
    vdp.write_control_port(0x8010); // H-INT enable
    vdp.write_control_port(0x8160); // V-INT enable + display on
    vdp.write_control_port(0x8A00); // H-INT every line

    vdp.step(Vdp::CYCLES_PER_FRAME as u32);
    assert_eq!(vdp.pending_interrupt_level(), Some(6));
    vdp.acknowledge_interrupt(6);
    assert_eq!(vdp.pending_interrupt_level(), Some(4));
}

#[test]
fn hv_counter_changes_as_cycles_advance() {
    let mut vdp = Vdp::new();
    let before = vdp.read_hv_counter();
    vdp.step(1_000);
    let after = vdp.read_hv_counter();
    assert_ne!(before, after);
}

#[test]
fn uses_background_color_register_for_zero_pixels() {
    let mut vdp = Vdp::new();
    vdp.write_cram_u16(0x25, encode_md_color(0, 7, 0));
    // Register 7 = palette 2, color 5
    vdp.write_control_port(0x8725);
    // Force first pixel of tile 0 to color 0 (high nibble of first byte).
    vdp.write_vram_u8(0, 0x00);

    let frame_ready = vdp.step(Vdp::CYCLES_PER_FRAME as u32);
    assert!(frame_ready);
    let pixel = &vdp.frame_buffer()[0..3];
    assert_eq!(pixel, &[0, 252, 0]);
}

#[test]
fn applies_horizontal_scroll_from_table() {
    let mut vdp = Vdp::new();
    let base = 0xC000usize;

    // Register 13 = 0x3C -> hscroll table @ 0xF000.
    vdp.write_control_port(0x8D3C);
    vdp.write_vram_u8(0xF000, 0x00);
    vdp.write_vram_u8(0xF001, 0x00);

    // Place tile 0 at (0,0), tile 1 at (1,0).
    vdp.write_vram_u8(base as u16, 0x00);
    vdp.write_vram_u8((base + 1) as u16, 0x00);
    vdp.write_vram_u8((base + 2) as u16, 0x00);
    vdp.write_vram_u8((base + 3) as u16, 0x01);

    vdp.write_cram_u16(1, encode_md_color(7, 0, 0));
    vdp.write_cram_u16(2, encode_md_color(0, 7, 0));
    for i in 0..4 {
        vdp.write_vram_u8(i, 0x11);
        vdp.write_vram_u8((32 + i) as u16, 0x22);
    }

    vdp.step(Vdp::CYCLES_PER_FRAME as u32);
    assert_eq!(&vdp.frame_buffer()[0..3], &[252, 0, 0]);

    // Apply -8 pixel scroll so x=0 samples tile 1.
    vdp.write_vram_u8(0xF000, 0xFF);
    vdp.write_vram_u8(0xF001, 0xF8);
    vdp.step(Vdp::CYCLES_PER_FRAME as u32);
    assert_eq!(&vdp.frame_buffer()[0..3], &[0, 252, 0]);
}

#[test]
fn h32_mode_blacks_right_border_outside_active_width() {
    let mut vdp = Vdp::new();
    let base = 0xC000usize;

    // H32 mode (256 active pixels).
    vdp.write_control_port(0x8C80);

    // Tile 0 uses palette color 1 for all pixels.
    for i in 0..32u16 {
        vdp.write_vram_u8(i, 0x11);
    }
    // Plane A (0,0) = tile 0.
    vdp.write_vram_u8(base as u16, 0x00);
    vdp.write_vram_u8((base + 1) as u16, 0x00);
    vdp.write_cram_u16(1, encode_md_color(7, 0, 0));

    vdp.step(Vdp::CYCLES_PER_FRAME as u32);

    let x255 = (255usize * 3, 255usize * 3 + 3);
    assert_ne!(&vdp.frame_buffer()[x255.0..x255.1], &[0, 0, 0]);

    let x256 = (256usize * 3, 256usize * 3 + 3);
    assert_eq!(&vdp.frame_buffer()[x256.0..x256.1], &[0, 0, 0]);
}

#[test]
fn applies_per_line_horizontal_scroll_mode() {
    let mut vdp = Vdp::new();
    let base = 0xC000usize;
    vdp.vram.fill(0);
    vdp.cram.fill(0);
    vdp.vsram.fill(0);

    // Register 13 = 0x3C -> hscroll table @ 0xF000.
    vdp.write_control_port(0x8D3C);
    // Register 11 = 0x03 -> per-line hscroll mode.
    vdp.write_control_port(0x8B03);

    // Line 0: no scroll (plane A word at 0xF000).
    vdp.write_vram_u8(0xF000, 0x00);
    vdp.write_vram_u8(0xF001, 0x00);
    // Line 1: -8 scroll (plane A word at 0xF004).
    vdp.write_vram_u8(0xF004, 0xFF);
    vdp.write_vram_u8(0xF005, 0xF8);

    // Place tile 0 at (0,0), tile 1 at (1,0).
    vdp.write_vram_u8(base as u16, 0x00);
    vdp.write_vram_u8((base + 1) as u16, 0x00);
    vdp.write_vram_u8((base + 2) as u16, 0x00);
    vdp.write_vram_u8((base + 3) as u16, 0x01);

    vdp.write_cram_u16(1, encode_md_color(7, 0, 0));
    vdp.write_cram_u16(2, encode_md_color(0, 7, 0));
    for row in 0..8u16 {
        for i in 0..4u16 {
            vdp.write_vram_u8(row * 4 + i, 0x11);
            vdp.write_vram_u8(32 + row * 4 + i, 0x22);
        }
    }

    vdp.step(Vdp::CYCLES_PER_FRAME as u32);
    // y=0 uses line 0 scroll (tile 0).
    assert_eq!(&vdp.frame_buffer()[0..3], &[252, 0, 0]);
    // y=1 uses line 1 scroll (tile 1).
    let y1 = FRAME_WIDTH * 3;
    assert_eq!(&vdp.frame_buffer()[y1..y1 + 3], &[0, 252, 0]);
}

#[test]
fn applies_vertical_scroll_from_vsram() {
    let mut vdp = Vdp::new();
    let base = 0xC000usize;
    let default_plane_width_tiles = 32usize;

    // Register 13 = 0x3C -> hscroll table @ 0xF000, keep hscroll = 0.
    vdp.write_control_port(0x8D3C);
    vdp.write_vram_u8(0xF000, 0x00);
    vdp.write_vram_u8(0xF001, 0x00);

    // Place tile 0 at (0,0), tile 2 at (0,1).
    vdp.write_vram_u8(base as u16, 0x00);
    vdp.write_vram_u8((base + 1) as u16, 0x00);
    vdp.write_vram_u8((base + default_plane_width_tiles * 2) as u16, 0x00);
    vdp.write_vram_u8((base + default_plane_width_tiles * 2 + 1) as u16, 0x02);

    vdp.write_cram_u16(1, encode_md_color(7, 0, 0));
    vdp.write_cram_u16(2, encode_md_color(0, 7, 0));
    for i in 0..4 {
        vdp.write_vram_u8(i, 0x11);
        vdp.write_vram_u8((64 + i) as u16, 0x22);
    }

    vdp.write_vsram_u16(0, 0);
    vdp.step(Vdp::CYCLES_PER_FRAME as u32);
    assert_eq!(&vdp.frame_buffer()[0..3], &[252, 0, 0]);

    // Scroll down by one tile row so y=0 samples tile row 1.
    vdp.write_vsram_u16(0, 8);
    vdp.step(Vdp::CYCLES_PER_FRAME as u32);
    assert_eq!(&vdp.frame_buffer()[0..3], &[0, 252, 0]);
}

#[test]
fn plane_b_vertical_scroll_uses_positive_direction() {
    let mut vdp = Vdp::new();
    vdp.vram.fill(0);
    vdp.cram.fill(0);
    vdp.vsram.fill(0);

    // Plane B base @ 0xE000.
    vdp.write_control_port(0x8407);
    // Plane size: 32x64 (width=32, height=64).
    vdp.write_control_port(0x9010);

    let plane_b_base = 0xE000usize;
    let width_tiles = 32usize;
    // Row 0, col 0 -> tile 1 (red).
    vdp.write_vram_u8(plane_b_base as u16, 0x00);
    vdp.write_vram_u8((plane_b_base + 1) as u16, 0x01);
    // Row 1, col 0 -> tile 2 (green).
    let row1 = plane_b_base + width_tiles * 2;
    vdp.write_vram_u8(row1 as u16, 0x00);
    vdp.write_vram_u8((row1 + 1) as u16, 0x02);

    vdp.write_cram_u16(1, encode_md_color(7, 0, 0));
    vdp.write_cram_u16(2, encode_md_color(0, 7, 0));
    for i in 0..4u16 {
        vdp.write_vram_u8(32 + i, 0x11);
        vdp.write_vram_u8(64 + i, 0x22);
    }

    // Plane B full-screen vscroll (index 1) by +8.
    // Positive-direction sampling maps y=0 to row 1.
    vdp.write_vsram_u16(1, 8);
    vdp.step(Vdp::CYCLES_PER_FRAME as u32);
    assert_eq!(&vdp.frame_buffer()[0..3], &[0, 252, 0]);
}

#[test]
fn comix_pretitle_vscroll_swap_auto_swaps_plane_vsram_indices() {
    fn setup_plane_b_scroll_probe(vdp: &mut Vdp) {
        vdp.vram.fill(0);
        vdp.cram.fill(0);
        vdp.vsram.fill(0);

        // Plane B base @ 0xE000.
        vdp.write_control_port(0x8407);
        // Comix pre-title swap condition: plane size 32x32 and reg11=0x03.
        vdp.write_control_port(0x9001);
        vdp.write_control_port(0x8B03);

        let plane_b_base = 0xE000usize;
        let width_tiles = 64usize;

        // Row 0 -> tile 1 (red), row 1 -> tile 2 (green).
        vdp.write_vram_u8(plane_b_base as u16, 0x00);
        vdp.write_vram_u8((plane_b_base + 1) as u16, 0x01);
        let row1 = plane_b_base + width_tiles * 2;
        vdp.write_vram_u8(row1 as u16, 0x00);
        vdp.write_vram_u8((row1 + 1) as u16, 0x02);

        vdp.write_cram_u16(1, encode_md_color(7, 0, 0));
        vdp.write_cram_u16(2, encode_md_color(0, 7, 0));
        for i in 0..32u16 {
            vdp.write_vram_u8(32 + i, 0x11);
            vdp.write_vram_u8(64 + i, 0x22);
        }

        // Deliberately diverge plane A/B full-screen vscroll words.
        vdp.write_vsram_u16(0, 8);
        vdp.write_vsram_u16(1, 0);
    }

    let mut vdp = Vdp::new();
    setup_plane_b_scroll_probe(&mut vdp);
    vdp.step(Vdp::CYCLES_PER_FRAME as u32);
    assert_eq!(&vdp.frame_buffer()[0..3], &[0, 252, 0]);
}

#[test]
fn comix_title_roll_auto_keeps_default_plane_b_sampling_without_bias() {
    fn setup_comix_title_roll_probe(vdp: &mut Vdp) {
        vdp.vram.fill(0);
        vdp.cram.fill(0);
        vdp.vsram.fill(0);

        // Match Comix title-roll condition.
        vdp.write_control_port(0x8140); // Display on.
        vdp.write_control_port(0x8407); // Plane B base @ 0xE000.
        vdp.write_control_port(0x8D3C); // HScroll table @ 0xF000.
        vdp.write_control_port(0x8B00); // Full-screen h/v scroll mode.
        vdp.write_control_port(0x9011); // Plane size 64x64.
        vdp.write_control_port(0x8C81); // H40 mode.

        let plane_b_base = 0xE000usize;
        let width_tiles = 64usize;

        // Row 0 -> tile 1 (red), row 2 -> tile 2 (green).
        vdp.write_vram_u8(plane_b_base as u16, 0x00);
        vdp.write_vram_u8((plane_b_base + 1) as u16, 0x01);
        let row2 = plane_b_base + 2 * width_tiles * 2;
        vdp.write_vram_u8(row2 as u16, 0x00);
        vdp.write_vram_u8((row2 + 1) as u16, 0x02);

        vdp.write_cram_u16(1, encode_md_color(7, 0, 0));
        vdp.write_cram_u16(2, encode_md_color(0, 7, 0));
        for i in 0..32u16 {
            vdp.write_vram_u8(32 + i, 0x11);
            vdp.write_vram_u8(64 + i, 0x22);
        }

        // Title-roll quirk trigger values.
        vdp.write_vsram_u16(0, 0x00B8);
        vdp.write_vsram_u16(1, 0x0000);
    }

    let mut vdp = Vdp::new();
    setup_comix_title_roll_probe(&mut vdp);
    vdp.step(Vdp::CYCLES_PER_FRAME as u32);
    assert_eq!(&vdp.frame_buffer()[0..3], &[252, 0, 0]);
}

#[test]
fn comix_title_roll_auto_masks_lower_region_to_black() {
    let mut vdp = Vdp::new();
    vdp.vram.fill(0);
    vdp.cram.fill(0);
    vdp.vsram.fill(0);

    // Match Comix title-roll condition.
    vdp.write_control_port(0x8140); // Display on.
    vdp.write_control_port(0x8407); // Plane B base @ 0xE000.
    vdp.write_control_port(0x8D3C); // HScroll table @ 0xF000.
    vdp.write_control_port(0x8B00); // Full-screen h/v scroll mode.
    vdp.write_control_port(0x9011); // Plane size 64x64.
    vdp.write_control_port(0x8C89); // H40 + shadow/highlight bit set.
    vdp.write_vsram_u16(0, 0x00B8);
    // Plane B vscroll=128 so that screen line 150 maps to sample_y=278,
    // which falls inside the nametable/HSCROLL overlap region (>=256).
    vdp.write_vsram_u16(1, 0x0080);

    // Tile 1: solid palette index 1 (red).
    for i in 0..32u16 {
        vdp.write_vram_u8(32 + i, 0x11);
    }
    vdp.write_cram_u16(1, encode_md_color(7, 0, 0));

    // Fill Plane B map with tile 1 (priority high) so clipping effect is
    // easy to observe.  Priority bit keeps normal brightness in S/H mode.
    let plane_b_base = 0xE000usize;
    for row in 0..64usize {
        for col in 0..64usize {
            let addr = plane_b_base + (row * 64 + col) * 2;
            vdp.write_vram_u8(addr as u16, 0x80);
            vdp.write_vram_u8((addr + 1) as u16, 0x01);
        }
    }

    vdp.step(Vdp::CYCLES_PER_FRAME as u32);

    // Upper roll area remains visible.
    let top = (20 * FRAME_WIDTH + 20) * 3;
    assert_eq!(&vdp.frame_buffer()[top..top + 3], &[252, 0, 0]);

    // Lower roll area is masked to background black.
    let lower = (150 * FRAME_WIDTH + 20) * 3;
    assert_eq!(&vdp.frame_buffer()[lower..lower + 3], &[0, 0, 0]);
}

#[test]
fn applies_two_cell_column_vertical_scroll_mode() {
    let mut vdp = Vdp::new();
    vdp.vram.fill(0);
    vdp.cram.fill(0);
    vdp.vsram.fill(0);
    let base = 0xC000usize;
    let default_plane_width_tiles = 32usize;

    // Reg11 bit2 enables 2-cell column vertical scroll mode.
    vdp.write_control_port(0x8B04);

    // Name table:
    // Columns 0-1 row0 use tile 1 (red).
    vdp.write_vram_u8(base as u16, 0x00);
    vdp.write_vram_u8((base + 1) as u16, 0x01);
    vdp.write_vram_u8((base + 2) as u16, 0x00);
    vdp.write_vram_u8((base + 3) as u16, 0x01);
    // Columns 2-3 row0 use tile 1 (red) by default.
    vdp.write_vram_u8((base + 4) as u16, 0x00);
    vdp.write_vram_u8((base + 5) as u16, 0x01);
    vdp.write_vram_u8((base + 6) as u16, 0x00);
    vdp.write_vram_u8((base + 7) as u16, 0x01);
    // Columns 2-3 row1 use tile 2 (green).
    let row1 = base + default_plane_width_tiles * 2;
    vdp.write_vram_u8((row1 + 4) as u16, 0x00);
    vdp.write_vram_u8((row1 + 5) as u16, 0x02);
    vdp.write_vram_u8((row1 + 6) as u16, 0x00);
    vdp.write_vram_u8((row1 + 7) as u16, 0x02);

    vdp.write_cram_u16(1, encode_md_color(7, 0, 0));
    vdp.write_cram_u16(2, encode_md_color(0, 7, 0));
    for i in 0..32u16 {
        vdp.write_vram_u8(32 + i, 0x11);
        vdp.write_vram_u8(64 + i, 0x22);
    }

    // Plane A VSRAM entries are even indices:
    // col group 0 => index 0 (no scroll)
    // col group 1 => index 2 (+8px)
    vdp.write_vsram_u16(0, 0);
    vdp.write_vsram_u16(2, 8);

    vdp.step(Vdp::CYCLES_PER_FRAME as u32);
    assert_eq!(&vdp.frame_buffer()[0..3], &[252, 0, 0]);
    assert_eq!(&vdp.frame_buffer()[16 * 3..16 * 3 + 3], &[0, 252, 0]);
}

#[test]
fn applies_plane_tile_flip_bits() {
    let mut vdp = Vdp::new();
    vdp.vram.fill(0);
    vdp.cram.fill(0);
    vdp.vsram.fill(0);

    // Plane A entry (0,0): tile 1, hflip+vflip.
    let entry = 0x1801u16;
    vdp.write_vram_u8(0xC000, (entry >> 8) as u8);
    vdp.write_vram_u8(0xC001, entry as u8);

    // Tile 1, source pixel at (7,7) uses color index 2.
    let tile_base = 32usize;
    vdp.write_vram_u8((tile_base + 7 * 4 + 3) as u16, 0x02);
    vdp.write_cram_u16(2, encode_md_color(0, 7, 0));

    vdp.step(Vdp::CYCLES_PER_FRAME as u32);
    assert_eq!(&vdp.frame_buffer()[0..3], &[0, 252, 0]);
}

#[test]
fn renders_sprite_pixels_over_plane() {
    let mut vdp = Vdp::new();
    // Register 5 = 0x70 -> sprite table @ 0xE000.
    vdp.write_control_port(0x8570);
    vdp.write_cram_u16(2, encode_md_color(0, 7, 0));

    // Tile 3, first pixel = color index 2.
    vdp.write_vram_u8(3 * 32, 0x20);

    let sat = 0xE000u16;
    // Y position = 128 (screen y = 0)
    vdp.write_vram_u8(sat, 0x00);
    vdp.write_vram_u8(sat + 1, 0x80);
    // Size/link: 1x1 tile, end of list.
    vdp.write_vram_u8(sat + 2, 0x00);
    vdp.write_vram_u8(sat + 3, 0x00);
    // Attr: tile index = 3.
    vdp.write_vram_u8(sat + 4, 0x00);
    vdp.write_vram_u8(sat + 5, 0x03);
    // X position = 128 (screen x = 0)
    vdp.write_vram_u8(sat + 6, 0x00);
    vdp.write_vram_u8(sat + 7, 0x80);

    vdp.step(Vdp::CYCLES_PER_FRAME as u32);
    assert_eq!(&vdp.frame_buffer()[0..3], &[0, 252, 0]);
}

#[test]
fn renders_window_plane_over_plane_a() {
    let mut vdp = Vdp::new();
    let plane_a_base = 0xC000u16;
    let window_base = 0xD000u16;

    // Keep hscroll = 0 to make plane A baseline deterministic.
    vdp.write_control_port(0x8D3C);
    vdp.write_vram_u8(0xF000, 0x00);
    vdp.write_vram_u8(0xF001, 0x00);

    // Plane A base (reg2) is default 0x30; entry (0,0) uses tile 0.
    vdp.write_vram_u8(plane_a_base, 0x00);
    vdp.write_vram_u8(plane_a_base + 1, 0x00);

    // Window base (reg3) = 0x34 -> 0xD000, entry (0,0) uses tile 2.
    vdp.write_control_port(0x8334);
    vdp.write_vram_u8(window_base, 0x00);
    vdp.write_vram_u8(window_base + 1, 0x02);

    vdp.write_cram_u16(1, encode_md_color(7, 0, 0));
    vdp.write_cram_u16(2, encode_md_color(0, 7, 0));
    for i in 0..4 {
        vdp.write_vram_u8(i, 0x11);
        vdp.write_vram_u8(64 + i as u16, 0x22);
    }

    // Full-width window: bit7 set + split 0 => x >= 0.
    vdp.write_control_port(0x9180);
    // Full-height window: bit7 set + split 0 => y >= 0.
    vdp.write_control_port(0x9280);

    vdp.step(Vdp::CYCLES_PER_FRAME as u32);
    assert_eq!(&vdp.frame_buffer()[0..3], &[0, 252, 0]);
}

#[test]
fn window_horizontal_split_selects_region() {
    let mut vdp = Vdp::new();
    let plane_a_base = 0xC000u16;
    let window_base = 0xD000u16;

    vdp.write_control_port(0x8D3C);
    vdp.write_vram_u8(0xF000, 0x00);
    vdp.write_vram_u8(0xF001, 0x00);

    // Plane A entries (0,0) and (1,0) use tile 0.
    vdp.write_vram_u8(plane_a_base, 0x00);
    vdp.write_vram_u8(plane_a_base + 1, 0x00);
    vdp.write_vram_u8(plane_a_base + 2, 0x00);
    vdp.write_vram_u8(plane_a_base + 3, 0x00);

    // Window entries (0..3,0) use tile 2.
    vdp.write_control_port(0x8334);
    vdp.write_vram_u8(window_base, 0x00);
    vdp.write_vram_u8(window_base + 1, 0x02);
    vdp.write_vram_u8(window_base + 2, 0x00);
    vdp.write_vram_u8(window_base + 3, 0x02);
    vdp.write_vram_u8(window_base + 4, 0x00);
    vdp.write_vram_u8(window_base + 5, 0x02);
    vdp.write_vram_u8(window_base + 6, 0x00);
    vdp.write_vram_u8(window_base + 7, 0x02);

    vdp.write_cram_u16(1, encode_md_color(7, 0, 0));
    vdp.write_cram_u16(2, encode_md_color(0, 7, 0));
    for i in 0..4 {
        vdp.write_vram_u8(i, 0x11);
        vdp.write_vram_u8(64 + i as u16, 0x22);
    }

    // x<16: Plane A, x>=16: Window (bit7 set => right side active).
    vdp.write_control_port(0x9181);
    // Full-height window (bit7 set + split 0).
    vdp.write_control_port(0x9280);

    vdp.step(Vdp::CYCLES_PER_FRAME as u32);
    assert_eq!(&vdp.frame_buffer()[0..3], &[252, 0, 0]);
    assert_eq!(&vdp.frame_buffer()[16 * 3..16 * 3 + 3], &[0, 252, 0]);
}

#[test]
fn window_vertical_split_bit7_set_uses_bottom_region() {
    let mut vdp = Vdp::new();
    let plane_a_base = 0xC000u16;
    let window_base = 0xD000u16;

    vdp.vram.fill(0);
    vdp.cram.fill(0);
    vdp.vsram.fill(0);
    vdp.write_control_port(0x8D3C);
    vdp.write_vram_u8(0xF000, 0x00);
    vdp.write_vram_u8(0xF001, 0x00);

    // Plane A (row0/row1) = tile 1 (red).
    vdp.write_vram_u8(plane_a_base, 0x00);
    vdp.write_vram_u8(plane_a_base + 1, 0x01);
    vdp.write_vram_u8((plane_a_base + 64) as u16, 0x00);
    vdp.write_vram_u8((plane_a_base + 65) as u16, 0x01);

    // Window (row0/row1) = tile 2 (green).
    vdp.write_control_port(0x8334);
    vdp.write_vram_u8(window_base, 0x00);
    vdp.write_vram_u8(window_base + 1, 0x02);
    vdp.write_vram_u8((window_base + 64) as u16, 0x00);
    vdp.write_vram_u8((window_base + 65) as u16, 0x02);

    vdp.write_cram_u16(1, encode_md_color(7, 0, 0));
    vdp.write_cram_u16(2, encode_md_color(0, 7, 0));
    for i in 0..32u16 {
        vdp.write_vram_u8(32 + i, 0x11);
        vdp.write_vram_u8(64 + i, 0x22);
    }

    // Full-width window, vertical split at y=8, bit7=1 => window on/below split.
    vdp.write_control_port(0x9180);
    vdp.write_control_port(0x9281);

    vdp.step(Vdp::CYCLES_PER_FRAME as u32);
    // y=0 => plane A (red)
    assert_eq!(&vdp.frame_buffer()[0..3], &[252, 0, 0]);
    // y=8 => window (green)
    let y8 = 8 * FRAME_WIDTH * 3;
    assert_eq!(&vdp.frame_buffer()[y8..y8 + 3], &[0, 252, 0]);
}

#[test]
fn window_vertical_split_bit7_clear_uses_top_region() {
    let mut vdp = Vdp::new();
    let plane_a_base = 0xC000u16;
    let window_base = 0xD000u16;

    vdp.vram.fill(0);
    vdp.cram.fill(0);
    vdp.vsram.fill(0);
    vdp.write_control_port(0x8D3C);
    vdp.write_vram_u8(0xF000, 0x00);
    vdp.write_vram_u8(0xF001, 0x00);

    // Plane A (row0/row1) = tile 1 (red).
    vdp.write_vram_u8(plane_a_base, 0x00);
    vdp.write_vram_u8(plane_a_base + 1, 0x01);
    vdp.write_vram_u8((plane_a_base + 64) as u16, 0x00);
    vdp.write_vram_u8((plane_a_base + 65) as u16, 0x01);

    // Window (row0/row1) = tile 2 (green).
    vdp.write_control_port(0x8334);
    vdp.write_vram_u8(window_base, 0x00);
    vdp.write_vram_u8(window_base + 1, 0x02);
    vdp.write_vram_u8((window_base + 64) as u16, 0x00);
    vdp.write_vram_u8((window_base + 65) as u16, 0x02);

    vdp.write_cram_u16(1, encode_md_color(7, 0, 0));
    vdp.write_cram_u16(2, encode_md_color(0, 7, 0));
    for i in 0..32u16 {
        vdp.write_vram_u8(32 + i, 0x11);
        vdp.write_vram_u8(64 + i, 0x22);
    }

    // Full-width window, vertical split at y=8, bit7=0 => window above split.
    vdp.write_control_port(0x9180);
    vdp.write_control_port(0x9201);

    vdp.step(Vdp::CYCLES_PER_FRAME as u32);
    // y=0 => window (green)
    assert_eq!(&vdp.frame_buffer()[0..3], &[0, 252, 0]);
    // y=8 => plane A (red)
    let y8 = 8 * FRAME_WIDTH * 3;
    assert_eq!(&vdp.frame_buffer()[y8..y8 + 3], &[252, 0, 0]);
}

#[test]
fn window_vertical_region_takes_priority_over_horizontal_split() {
    let mut vdp = Vdp::new();
    let plane_a_base = 0xC000u16;
    let window_base = 0xD000u16;

    vdp.vram.fill(0);
    vdp.cram.fill(0);
    vdp.vsram.fill(0);
    vdp.write_control_port(0x8D3C);
    vdp.write_vram_u8(0xF000, 0x00);
    vdp.write_vram_u8(0xF001, 0x00);

    // Plane A rows use tile 1 (red).
    vdp.write_vram_u8(plane_a_base, 0x00);
    vdp.write_vram_u8(plane_a_base + 1, 0x01);
    vdp.write_vram_u8((plane_a_base + 64) as u16, 0x00);
    vdp.write_vram_u8((plane_a_base + 65) as u16, 0x01);

    // Window rows use tile 2 (green).
    vdp.write_control_port(0x8334);
    vdp.write_vram_u8(window_base, 0x00);
    vdp.write_vram_u8(window_base + 1, 0x02);
    vdp.write_vram_u8((window_base + 64) as u16, 0x00);
    vdp.write_vram_u8((window_base + 65) as u16, 0x02);

    vdp.write_cram_u16(1, encode_md_color(7, 0, 0));
    vdp.write_cram_u16(2, encode_md_color(0, 7, 0));
    for i in 0..32u16 {
        vdp.write_vram_u8(32 + i, 0x11);
        vdp.write_vram_u8(64 + i, 0x22);
    }

    // Horizontal split alone would enable the window only for x >= 16.
    vdp.write_control_port(0x9181);
    // Vertical split at y=8 with bit7=1 should force the entire line to window.
    vdp.write_control_port(0x9281);

    vdp.step(Vdp::CYCLES_PER_FRAME as u32);
    // Above the split: x=0 remains plane A.
    assert_eq!(&vdp.frame_buffer()[0..3], &[252, 0, 0]);
    // On/below the split: x=0 must still be window because vertical takes priority.
    let y8 = 8 * FRAME_WIDTH * 3;
    assert_eq!(&vdp.frame_buffer()[y8..y8 + 3], &[0, 252, 0]);
}

#[test]
fn window_plane_uses_h40_width_of_64_tiles_without_wrapping_at_32() {
    let mut vdp = Vdp::new();
    vdp.vram.fill(0);
    vdp.cram.fill(0);
    vdp.vsram.fill(0);

    // H40 mode on (default), full-screen window.
    vdp.write_control_port(0x8C81);
    vdp.write_control_port(0x9180);
    vdp.write_control_port(0x9280);

    // Window base at 0xD000.
    vdp.write_control_port(0x8334);
    let window_base = 0xD000usize;
    // Tile at x=0 -> red.
    vdp.write_vram_u8(window_base as u16, 0x00);
    vdp.write_vram_u8((window_base + 1) as u16, 0x01);
    // Tile at x=32 cells (pixel x=256) -> green.
    let x32 = window_base + 32 * 2;
    vdp.write_vram_u8(x32 as u16, 0x00);
    vdp.write_vram_u8((x32 + 1) as u16, 0x02);

    vdp.write_cram_u16(1, encode_md_color(7, 0, 0));
    vdp.write_cram_u16(2, encode_md_color(0, 7, 0));
    for i in 0..4u16 {
        vdp.write_vram_u8(32 + i, 0x11);
        vdp.write_vram_u8(64 + i, 0x22);
    }

    vdp.step(Vdp::CYCLES_PER_FRAME as u32);
    assert_eq!(&vdp.frame_buffer()[0..3], &[252, 0, 0]);
    let x256 = 256 * 3;
    assert_eq!(&vdp.frame_buffer()[x256..x256 + 3], &[0, 252, 0]);
}

#[test]
fn window_nametable_base_masks_bit1_in_h40_mode() {
    let mut vdp = Vdp::new();
    vdp.vram.fill(0);
    vdp.cram.fill(0);
    vdp.vsram.fill(0);

    // H40 mode on, full-screen window.
    vdp.write_control_port(0x8C81);
    vdp.write_control_port(0x9180);
    vdp.write_control_port(0x9280);

    // reg3=0x22: in H40 bit1 must be ignored => base 0x8000 (not 0x8800).
    vdp.write_control_port(0x8322);

    // Put red tile at 0x8000 and green tile at 0x8800 to detect wrong base decode.
    vdp.write_vram_u8(0x8000, 0x00);
    vdp.write_vram_u8(0x8001, 0x01);
    vdp.write_vram_u8(0x8800, 0x00);
    vdp.write_vram_u8(0x8801, 0x02);

    vdp.write_cram_u16(1, encode_md_color(7, 0, 0));
    vdp.write_cram_u16(2, encode_md_color(0, 7, 0));
    for i in 0..4u16 {
        vdp.write_vram_u8(32 + i, 0x11);
        vdp.write_vram_u8(64 + i, 0x22);
    }

    vdp.step(Vdp::CYCLES_PER_FRAME as u32);
    assert_eq!(&vdp.frame_buffer()[0..3], &[252, 0, 0]);
}

#[test]
fn low_priority_sprite_is_behind_high_priority_plane() {
    let mut vdp = Vdp::new();
    let plane_a_base = 0xC000u16;
    let sat = 0xE000u16;

    // Plane pixel: tile 0 with high priority.
    vdp.write_vram_u8(plane_a_base, 0x80);
    vdp.write_vram_u8(plane_a_base + 1, 0x00);
    vdp.write_vram_u8(0, 0x11);
    vdp.write_cram_u16(1, encode_md_color(7, 0, 0));

    // Sprite pixel at same position: tile 3, low priority.
    vdp.write_vram_u8(3 * 32, 0x20);
    vdp.write_cram_u16(2, encode_md_color(0, 7, 0));
    vdp.write_vram_u8(sat, 0x00);
    vdp.write_vram_u8(sat + 1, 0x80);
    vdp.write_vram_u8(sat + 2, 0x00);
    vdp.write_vram_u8(sat + 3, 0x00);
    vdp.write_vram_u8(sat + 4, 0x00);
    vdp.write_vram_u8(sat + 5, 0x03);
    vdp.write_vram_u8(sat + 6, 0x00);
    vdp.write_vram_u8(sat + 7, 0x80);

    vdp.step(Vdp::CYCLES_PER_FRAME as u32);
    assert_eq!(&vdp.frame_buffer()[0..3], &[252, 0, 0]);
}

#[test]
fn limits_sprites_per_line_in_h40_mode() {
    let mut vdp = Vdp::new();
    vdp.vram.fill(0);
    vdp.cram.fill(0);
    vdp.vsram.fill(0);

    // H40 mode and SAT at 0xE000.
    vdp.write_control_port(0x8C81);
    vdp.write_control_port(0x8570);
    vdp.write_cram_u16(1, encode_md_color(7, 0, 0));
    // Tile 1: fully opaque color index 1.
    for i in 0..32u16 {
        vdp.write_vram_u8(32 + i, 0x11);
    }

    let sat = 0xE000usize;
    for i in 0..21usize {
        let entry = sat + i * 8;
        let x_pos = 128 + (i as u16) * 8;
        let link = if i == 20 { 0 } else { (i + 1) as u16 };

        // Y = 128 (screen y=0)
        vdp.write_vram_u8(entry as u16, 0x00);
        vdp.write_vram_u8((entry + 1) as u16, 0x80);
        // 1x1 sprite + link
        vdp.write_vram_u8((entry + 2) as u16, 0x00);
        vdp.write_vram_u8((entry + 3) as u16, (link & 0x7F) as u8);
        // Tile index 1
        vdp.write_vram_u8((entry + 4) as u16, 0x00);
        vdp.write_vram_u8((entry + 5) as u16, 0x01);
        // X position
        vdp.write_vram_u8((entry + 6) as u16, (x_pos >> 8) as u8);
        vdp.write_vram_u8((entry + 7) as u16, x_pos as u8);
    }

    vdp.step(Vdp::CYCLES_PER_FRAME as u32);

    // First 20 sprites are visible.
    for i in 0..20usize {
        let x = i * 8;
        let p = x * 3;
        assert_eq!(&vdp.frame_buffer()[p..p + 3], &[252, 0, 0]);
    }
    // 21st sprite is dropped by per-line limit.
    let p = 20 * 8 * 3;
    assert_eq!(&vdp.frame_buffer()[p..p + 3], &[0, 0, 0]);
    let status = vdp.read_control_port();
    assert_ne!(status & super::STATUS_SPRITE_OVERFLOW, 0);
}

#[test]
fn limits_total_sat_entries_to_64_in_h32_mode() {
    let mut vdp = Vdp::new();
    vdp.vram.fill(0);
    vdp.cram.fill(0);
    vdp.vsram.fill(0);

    // H32 mode and SAT at 0xE000.
    vdp.write_control_port(0x8C80);
    vdp.write_control_port(0x8570);
    vdp.write_cram_u16(1, encode_md_color(7, 0, 0));
    // Tile 1: fully opaque color index 1.
    for i in 0..32u16 {
        vdp.write_vram_u8(32 + i, 0x11);
    }

    let sat = 0xE000usize;
    for i in 0..65usize {
        let entry = sat + i * 8;
        let y_word = if i < 64 { 0x0000u16 } else { 0x0080u16 }; // last one only is visible at y=0
        let link = if i == 64 { 0 } else { (i + 1) as u16 };
        let tile = if i < 64 { 0x0000u16 } else { 0x0001u16 };
        let x_word = 0x0080u16; // screen x=0

        vdp.write_vram_u8(entry as u16, (y_word >> 8) as u8);
        vdp.write_vram_u8((entry + 1) as u16, y_word as u8);
        vdp.write_vram_u8((entry + 2) as u16, 0x00); // 1x1 sprite
        vdp.write_vram_u8((entry + 3) as u16, (link & 0x7F) as u8);
        vdp.write_vram_u8((entry + 4) as u16, (tile >> 8) as u8);
        vdp.write_vram_u8((entry + 5) as u16, tile as u8);
        vdp.write_vram_u8((entry + 6) as u16, (x_word >> 8) as u8);
        vdp.write_vram_u8((entry + 7) as u16, x_word as u8);
    }

    vdp.step(Vdp::CYCLES_PER_FRAME as u32);
    // 65th SAT entry should be ignored in H32 total-sprite mode.
    assert_eq!(&vdp.frame_buffer()[0..3], &[0, 0, 0]);
}

#[test]
fn sprites_use_column_major_tile_layout() {
    let mut vdp = Vdp::new();
    vdp.vram.fill(0);
    vdp.cram.fill(0);
    vdp.vsram.fill(0);

    // SAT at 0xE000.
    vdp.write_control_port(0x8570);
    vdp.write_cram_u16(1, encode_md_color(7, 0, 0));
    vdp.write_cram_u16(2, encode_md_color(0, 7, 0));
    vdp.write_cram_u16(3, encode_md_color(0, 0, 7));
    vdp.write_cram_u16(4, encode_md_color(7, 7, 0));

    // Tiles 1..4 as solid colors 1..4.
    for i in 0..32u16 {
        vdp.write_vram_u8(32 + i, 0x11);
        vdp.write_vram_u8(64 + i, 0x22);
        vdp.write_vram_u8(96 + i, 0x33);
        vdp.write_vram_u8(128 + i, 0x44);
    }

    let sat = 0xE000u16;
    // Y = 128 (screen y = 0)
    vdp.write_vram_u8(sat, 0x00);
    vdp.write_vram_u8(sat + 1, 0x80);
    // Size: 2x2 tiles, link end.
    vdp.write_vram_u8(sat + 2, 0x05);
    vdp.write_vram_u8(sat + 3, 0x00);
    // Attr: tile index 1.
    vdp.write_vram_u8(sat + 4, 0x00);
    vdp.write_vram_u8(sat + 5, 0x01);
    // X = 128 (screen x = 0)
    vdp.write_vram_u8(sat + 6, 0x00);
    vdp.write_vram_u8(sat + 7, 0x80);

    vdp.step(Vdp::CYCLES_PER_FRAME as u32);

    let top_left = &vdp.frame_buffer()[0..3];
    let top_right = &vdp.frame_buffer()[8 * 3..8 * 3 + 3];
    let bottom_left = &vdp.frame_buffer()[FRAME_WIDTH * 8 * 3..FRAME_WIDTH * 8 * 3 + 3];
    let bottom_right =
        &vdp.frame_buffer()[FRAME_WIDTH * 8 * 3 + 8 * 3..FRAME_WIDTH * 8 * 3 + 8 * 3 + 3];

    assert_eq!(top_left, &[252, 0, 0]);
    assert_eq!(top_right, &[0, 0, 252]);
    assert_eq!(bottom_left, &[0, 252, 0]);
    assert_eq!(bottom_right, &[252, 252, 0]);
}

#[test]
fn lower_index_sprite_has_priority_when_overlapping() {
    let mut vdp = Vdp::new();
    vdp.vram.fill(0);
    vdp.cram.fill(0);
    vdp.vsram.fill(0);

    vdp.write_control_port(0x8570);
    vdp.write_cram_u16(1, encode_md_color(7, 0, 0));
    vdp.write_cram_u16(2, encode_md_color(0, 7, 0));

    // Tile 1 = red, tile 2 = green
    for i in 0..32u16 {
        vdp.write_vram_u8(32 + i, 0x11);
        vdp.write_vram_u8(64 + i, 0x22);
    }

    let sat = 0xE000usize;
    // Sprite 0: tile 1 at (0,0), link -> sprite 1
    vdp.write_vram_u8(sat as u16, 0x00);
    vdp.write_vram_u8((sat + 1) as u16, 0x80);
    vdp.write_vram_u8((sat + 2) as u16, 0x00);
    vdp.write_vram_u8((sat + 3) as u16, 0x01);
    vdp.write_vram_u8((sat + 4) as u16, 0x00);
    vdp.write_vram_u8((sat + 5) as u16, 0x01);
    vdp.write_vram_u8((sat + 6) as u16, 0x00);
    vdp.write_vram_u8((sat + 7) as u16, 0x80);

    // Sprite 1: tile 2 at same (0,0), end
    let sat1 = sat + 8;
    vdp.write_vram_u8(sat1 as u16, 0x00);
    vdp.write_vram_u8((sat1 + 1) as u16, 0x80);
    vdp.write_vram_u8((sat1 + 2) as u16, 0x00);
    vdp.write_vram_u8((sat1 + 3) as u16, 0x00);
    vdp.write_vram_u8((sat1 + 4) as u16, 0x00);
    vdp.write_vram_u8((sat1 + 5) as u16, 0x02);
    vdp.write_vram_u8((sat1 + 6) as u16, 0x00);
    vdp.write_vram_u8((sat1 + 7) as u16, 0x80);

    vdp.step(Vdp::CYCLES_PER_FRAME as u32);
    assert_eq!(&vdp.frame_buffer()[0..3], &[252, 0, 0]);
    let status = vdp.read_control_port();
    assert_ne!(status & super::STATUS_SPRITE_COLLISION, 0);
    let status_after = vdp.read_control_port();
    assert_eq!(status_after & super::STATUS_SPRITE_COLLISION, 0);
}

#[test]
fn x_zero_sprite_masks_following_sprites_on_same_line() {
    let mut vdp = Vdp::new();
    vdp.vram.fill(0);
    vdp.cram.fill(0);
    vdp.vsram.fill(0);

    vdp.write_control_port(0x8570);
    vdp.write_cram_u16(1, encode_md_color(7, 0, 0));
    for i in 0..32u16 {
        vdp.write_vram_u8(32 + i, 0x11);
    }

    let sat = 0xE000usize;
    // Sprite 0: mask sprite (X=0 internal), covers y=0 line.
    vdp.write_vram_u8(sat as u16, 0x00);
    vdp.write_vram_u8((sat + 1) as u16, 0x80);
    vdp.write_vram_u8((sat + 2) as u16, 0x00);
    vdp.write_vram_u8((sat + 3) as u16, 0x01);
    vdp.write_vram_u8((sat + 4) as u16, 0x00);
    vdp.write_vram_u8((sat + 5) as u16, 0x00);
    vdp.write_vram_u8((sat + 6) as u16, 0x00);
    vdp.write_vram_u8((sat + 7) as u16, 0x00);

    // Sprite 1: red sprite at (0,0), should be masked.
    let sat1 = sat + 8;
    vdp.write_vram_u8(sat1 as u16, 0x00);
    vdp.write_vram_u8((sat1 + 1) as u16, 0x80);
    vdp.write_vram_u8((sat1 + 2) as u16, 0x00);
    vdp.write_vram_u8((sat1 + 3) as u16, 0x00);
    vdp.write_vram_u8((sat1 + 4) as u16, 0x00);
    vdp.write_vram_u8((sat1 + 5) as u16, 0x01);
    vdp.write_vram_u8((sat1 + 6) as u16, 0x00);
    vdp.write_vram_u8((sat1 + 7) as u16, 0x80);

    vdp.step(Vdp::CYCLES_PER_FRAME as u32);
    assert_eq!(&vdp.frame_buffer()[0..3], &[0, 0, 0]);
}

#[test]
fn transparent_sprite_dots_consume_line_dot_budget() {
    let mut vdp = Vdp::new();
    vdp.vram.fill(0);
    vdp.cram.fill(0);
    vdp.vsram.fill(0);

    // H40 mode (320-dot sprite line budget), SAT at 0xE000.
    vdp.write_control_port(0x8C81);
    vdp.write_control_port(0x8570);
    vdp.write_cram_u16(1, encode_md_color(7, 0, 0));
    // Tile 1: opaque red.
    for i in 0..32u16 {
        vdp.write_vram_u8(32 + i, 0x11);
    }

    let sat = 0xE000usize;
    // 10 transparent 32px sprites (4x1 tiles) at y=0 consume 320 dots.
    for i in 0..10usize {
        let entry = sat + i * 8;
        let link = (i + 1) as u16;
        vdp.write_vram_u8(entry as u16, 0x00);
        vdp.write_vram_u8((entry + 1) as u16, 0x80);
        vdp.write_vram_u8((entry + 2) as u16, 0x0C); // 4x1
        vdp.write_vram_u8((entry + 3) as u16, (link & 0x7F) as u8);
        vdp.write_vram_u8((entry + 4) as u16, 0x00);
        vdp.write_vram_u8((entry + 5) as u16, 0x00); // tile 0 transparent
        vdp.write_vram_u8((entry + 6) as u16, 0x00);
        vdp.write_vram_u8((entry + 7) as u16, 0x80); // x=0
    }

    // 11th sprite is opaque red at same line, should be dropped by dot budget.
    let entry = sat + 10 * 8;
    vdp.write_vram_u8(entry as u16, 0x00);
    vdp.write_vram_u8((entry + 1) as u16, 0x80);
    vdp.write_vram_u8((entry + 2) as u16, 0x00); // 1x1
    vdp.write_vram_u8((entry + 3) as u16, 0x00); // end
    vdp.write_vram_u8((entry + 4) as u16, 0x00);
    vdp.write_vram_u8((entry + 5) as u16, 0x01); // tile 1 opaque
    vdp.write_vram_u8((entry + 6) as u16, 0x00);
    vdp.write_vram_u8((entry + 7) as u16, 0x80); // x=0

    vdp.step(Vdp::CYCLES_PER_FRAME as u32);
    assert_eq!(&vdp.frame_buffer()[0..3], &[0, 0, 0]);
}

#[test]
fn high_priority_sprite_overrides_high_priority_plane() {
    let mut vdp = Vdp::new();
    let plane_a_base = 0xC000u16;
    let sat = 0xE000u16;

    // Plane pixel: tile 0 with high priority.
    vdp.write_vram_u8(plane_a_base, 0x80);
    vdp.write_vram_u8(plane_a_base + 1, 0x00);
    vdp.write_vram_u8(0, 0x11);
    vdp.write_cram_u16(1, encode_md_color(7, 0, 0));

    // Sprite pixel at same position: tile 3, high priority.
    vdp.write_vram_u8(3 * 32, 0x20);
    vdp.write_cram_u16(2, encode_md_color(0, 7, 0));
    vdp.write_vram_u8(sat, 0x00);
    vdp.write_vram_u8(sat + 1, 0x80);
    vdp.write_vram_u8(sat + 2, 0x00);
    vdp.write_vram_u8(sat + 3, 0x00);
    vdp.write_vram_u8(sat + 4, 0x80);
    vdp.write_vram_u8(sat + 5, 0x03);
    vdp.write_vram_u8(sat + 6, 0x00);
    vdp.write_vram_u8(sat + 7, 0x80);

    vdp.step(Vdp::CYCLES_PER_FRAME as u32);
    assert_eq!(&vdp.frame_buffer()[0..3], &[0, 252, 0]);
}

#[test]
fn shadow_highlight_control_colors_affect_underlying_pixel() {
    let mut vdp = Vdp::new();
    let plane_a_base = 0xC000u16;
    let sat = 0xE000u16;

    // Enable shadow/highlight mode while keeping H40.
    vdp.write_control_port(0x8C89);

    // Plane pixel: tile 0, color index 1 -> medium red (level 4).
    vdp.write_vram_u8(plane_a_base, 0x00);
    vdp.write_vram_u8(plane_a_base + 1, 0x00);
    vdp.write_vram_u8(0, 0x11);
    vdp.write_cram_u16(1, encode_md_color(4, 0, 0));

    // Sprite tile 3, first pixel uses control color 15 (shadow).
    vdp.write_vram_u8(3 * 32, 0xF0);
    vdp.write_vram_u8(sat, 0x00);
    vdp.write_vram_u8(sat + 1, 0x80);
    vdp.write_vram_u8(sat + 2, 0x00);
    vdp.write_vram_u8(sat + 3, 0x00);
    // Attr: palette 3, tile 3.
    vdp.write_vram_u8(sat + 4, 0x60);
    vdp.write_vram_u8(sat + 5, 0x03);
    vdp.write_vram_u8(sat + 6, 0x00);
    vdp.write_vram_u8(sat + 7, 0x80);

    vdp.step(Vdp::CYCLES_PER_FRAME as u32);
    // Plane has no priority → shadowed in S/H mode.
    // Shadow control on shadowed pixel → stays shadowed: level 4 → shadow = level 2 = 72.
    assert_eq!(&vdp.frame_buffer()[0..3], &[72, 0, 0]);

    // Switch to control color 14 (highlight).
    vdp.write_vram_u8(3 * 32, 0xE0);
    vdp.step(Vdp::CYCLES_PER_FRAME as u32);
    // Highlight on shadowed pixel → restored to normal: level 4 = 144.
    assert_eq!(&vdp.frame_buffer()[0..3], &[144, 0, 0]);
}

#[test]
fn defaults_plane_size_to_32x32_cells() {
    let mut vdp = Vdp::new();
    vdp.vram.fill(0);
    vdp.cram.fill(0);
    vdp.vsram.fill(0);

    // Plane size register defaults to 0x00 => 32x32.
    // Place tile 1 at (0,0) and tile 2 at (0,32) to verify vertical wrap.
    let plane_a = 0xC000usize;
    vdp.write_vram_u8(plane_a as u16, 0x00);
    vdp.write_vram_u8((plane_a + 1) as u16, 0x01);
    let row32 = plane_a + 32 * 32 * 2;
    vdp.write_vram_u8(row32 as u16, 0x00);
    vdp.write_vram_u8((row32 + 1) as u16, 0x02);

    vdp.write_cram_u16(1, encode_md_color(7, 0, 0));
    vdp.write_cram_u16(2, encode_md_color(0, 7, 0));
    for i in 0..4u16 {
        vdp.write_vram_u8(32 + i, 0x11);
        vdp.write_vram_u8(64 + i, 0x22);
    }

    // Scroll down by 32 tiles (256px). With 32-cell height this wraps to row 0.
    vdp.write_vsram_u16(0, 256);
    vdp.step(Vdp::CYCLES_PER_FRAME as u32);
    assert_eq!(&vdp.frame_buffer()[0..3], &[252, 0, 0]);
}

#[test]
fn out_of_range_sprite_link_stops_traversal() {
    let mut vdp = Vdp::new();
    vdp.vram.fill(0);
    vdp.cram.fill(0);
    vdp.vsram.fill(0);

    vdp.write_control_port(0x8570); // SAT @ 0xE000
    vdp.write_cram_u16(1, encode_md_color(7, 0, 0));

    // Tile 1 = solid red.
    for i in 0..32u16 {
        vdp.write_vram_u8(32 + i, 0x11);
    }

    let sat = 0xE000usize;
    // Sprite 0: offscreen mask-like position, but link points out of range (0x7F).
    vdp.write_vram_u8(sat as u16, 0x00);
    vdp.write_vram_u8((sat + 1) as u16, 0x80);
    vdp.write_vram_u8((sat + 2) as u16, 0x00);
    vdp.write_vram_u8((sat + 3) as u16, 0x7F);
    vdp.write_vram_u8((sat + 4) as u16, 0x00);
    vdp.write_vram_u8((sat + 5) as u16, 0x00);
    vdp.write_vram_u8((sat + 6) as u16, 0x00);
    vdp.write_vram_u8((sat + 7) as u16, 0x00);

    // Sprite 79: visible red sprite at (0,0). Must not be reached from out-of-range link.
    let sat79 = sat + 79 * 8;
    vdp.write_vram_u8(sat79 as u16, 0x00);
    vdp.write_vram_u8((sat79 + 1) as u16, 0x80);
    vdp.write_vram_u8((sat79 + 2) as u16, 0x00);
    vdp.write_vram_u8((sat79 + 3) as u16, 0x00);
    vdp.write_vram_u8((sat79 + 4) as u16, 0x00);
    vdp.write_vram_u8((sat79 + 5) as u16, 0x01);
    vdp.write_vram_u8((sat79 + 6) as u16, 0x00);
    vdp.write_vram_u8((sat79 + 7) as u16, 0x80);

    vdp.step(Vdp::CYCLES_PER_FRAME as u32);
    assert_eq!(&vdp.frame_buffer()[0..3], &[0, 0, 0]);
}

#[test]
fn supports_64x64_plane_size_from_reg16() {
    let mut vdp = Vdp::new();
    vdp.vram.fill(0);
    vdp.cram.fill(0);
    vdp.vsram.fill(0);

    // reg16 = 0x11 => 64x64 cells.
    vdp.write_control_port(0x9011);

    let plane_a = 0xC000usize;
    // Tile at (0,0)
    vdp.write_vram_u8(plane_a as u16, 0x00);
    vdp.write_vram_u8((plane_a + 1) as u16, 0x01);
    // Tile at (0,32) within the 64-cell-tall map.
    let row32 = plane_a + 32 * 64 * 2;
    vdp.write_vram_u8(row32 as u16, 0x00);
    vdp.write_vram_u8((row32 + 1) as u16, 0x02);

    vdp.write_cram_u16(1, encode_md_color(7, 0, 0));
    vdp.write_cram_u16(2, encode_md_color(0, 7, 0));
    for i in 0..4u16 {
        vdp.write_vram_u8(32 + i, 0x11);
        vdp.write_vram_u8(64 + i, 0x22);
    }

    // Scroll 32 tiles down. On 64-cell height this should land on row 32 (green), not wrap.
    vdp.write_vsram_u16(0, 256);
    vdp.step(Vdp::CYCLES_PER_FRAME as u32);
    assert_eq!(&vdp.frame_buffer()[0..3], &[0, 252, 0]);
}

#[test]
fn scroll_plane_64_cell_width_uses_linear_nametable_layout() {
    let mut vdp = Vdp::new();
    vdp.vram.fill(0);
    vdp.cram.fill(0);
    vdp.vsram.fill(0);

    // reg16 = 0x11 => 64x64 cells.
    vdp.write_control_port(0x8C81);
    vdp.write_control_port(0x9011);
    let plane_a = 0xC000usize;
    // Tile at (0,0) -> red.
    vdp.write_vram_u8(plane_a as u16, 0x00);
    vdp.write_vram_u8((plane_a + 1) as u16, 0x01);
    // Tile at (32,0) -> green.
    let x32 = plane_a + 32 * 2;
    vdp.write_vram_u8(x32 as u16, 0x00);
    vdp.write_vram_u8((x32 + 1) as u16, 0x02);

    vdp.write_cram_u16(1, encode_md_color(7, 0, 0));
    vdp.write_cram_u16(2, encode_md_color(0, 7, 0));
    for i in 0..4u16 {
        vdp.write_vram_u8(32 + i, 0x11);
        vdp.write_vram_u8(64 + i, 0x22);
    }

    vdp.step(Vdp::CYCLES_PER_FRAME as u32);
    assert_eq!(&vdp.frame_buffer()[0..3], &[252, 0, 0]);
    let x256 = 256 * 3;
    assert_eq!(&vdp.frame_buffer()[x256..x256 + 3], &[0, 252, 0]);
}

#[test]
fn plane_size_code_3_decodes_to_128_cells() {
    let mut vdp = Vdp::new();
    vdp.vram.fill(0);
    vdp.cram.fill(0);
    vdp.vsram.fill(0);

    // reg16=0x03 decodes to 128x32.
    vdp.write_control_port(0x9003);
    vdp.write_control_port(0x8D3C); // hscroll table @ 0xF000
    let plane_a = 0xC000usize;
    // x=0 tile -> red
    vdp.write_vram_u8(plane_a as u16, 0x00);
    vdp.write_vram_u8((plane_a + 1) as u16, 0x01);
    // On 128x32 maps, the second half of the row lives in the second 64x32
    // page. Width=64 would wrap this position to x=0; width=128 keeps it
    // distinct and uses the paged nametable layout.
    let x64 = plane_a + 64 * 32 * 2;
    vdp.write_vram_u8(x64 as u16, 0x00);
    vdp.write_vram_u8((x64 + 1) as u16, 0x02);

    vdp.write_cram_u16(1, encode_md_color(7, 0, 0));
    vdp.write_cram_u16(2, encode_md_color(0, 7, 0));
    for i in 0..4u16 {
        vdp.write_vram_u8(32 + i, 0x11);
        vdp.write_vram_u8(64 + i, 0x22);
    }
    // Scroll by -64 cells so x=0 samples cell 64 on 128-cell maps.
    vdp.write_vram_u8(0xF000, 0xFE);
    vdp.write_vram_u8(0xF001, 0x00);

    vdp.step(Vdp::CYCLES_PER_FRAME as u32);
    assert_eq!(&vdp.frame_buffer()[0..3], &[0, 252, 0]);
}

#[test]
fn plane_a_64x32_paged_changes_second_page_row_addressing() {
    let mut vdp = Vdp::new();
    vdp.vram.fill(0);
    vdp.cram.fill(0);
    vdp.vsram.fill(0);

    // reg16=0x03 => 128x32 cells.
    vdp.write_control_port(0x9003);
    vdp.write_control_port(0x8D3C); // hscroll table @ 0xF000
    let plane_a = 0xC000usize;

    // Sample row 1, column 64 at screen origin.
    let linear_addr = plane_a + ((128 + 64) * 2);
    let paged_addr = plane_a + 64 * 32 * 2 + 64 * 2;
    vdp.write_vram_u8(linear_addr as u16, 0x00);
    vdp.write_vram_u8((linear_addr + 1) as u16, 0x01);
    vdp.write_vram_u8(paged_addr as u16, 0x00);
    vdp.write_vram_u8((paged_addr + 1) as u16, 0x02);

    vdp.write_cram_u16(1, encode_md_color(7, 0, 0));
    vdp.write_cram_u16(2, encode_md_color(0, 7, 0));
    for i in 0..4u16 {
        vdp.write_vram_u8(32 + i, 0x11);
        vdp.write_vram_u8(64 + i, 0x22);
    }
    vdp.write_vsram_u16(0, 8);
    vdp.write_vram_u8(0xF000, 0xFE);
    vdp.write_vram_u8(0xF001, 0x00);

    vdp.step(Vdp::CYCLES_PER_FRAME as u32);
    assert_eq!(&vdp.frame_buffer()[0..3], &[0, 252, 0]);
}

#[test]
fn plane_b_64x32_paged_changes_second_page_row_addressing() {
    let mut vdp = Vdp::new();
    vdp.vram.fill(0);
    vdp.cram.fill(0);
    vdp.vsram.fill(0);

    // Plane B base @ 0xE000, reg16=0x03 => 128x32 cells.
    vdp.write_control_port(0x8407);
    vdp.write_control_port(0x9003);
    vdp.write_control_port(0x8D3C); // hscroll table @ 0xF000
    let plane_b = 0xE000usize;

    // Sample row 1, column 64 at screen origin.
    let linear_addr = plane_b + ((128 + 64) * 2);
    let paged_addr = plane_b + 64 * 32 * 2 + 64 * 2;
    vdp.write_vram_u8(linear_addr as u16, 0x00);
    vdp.write_vram_u8((linear_addr + 1) as u16, 0x01);
    vdp.write_vram_u8(paged_addr as u16, 0x00);
    vdp.write_vram_u8((paged_addr + 1) as u16, 0x02);

    vdp.write_cram_u16(1, encode_md_color(7, 0, 0));
    vdp.write_cram_u16(2, encode_md_color(0, 7, 0));
    for i in 0..4u16 {
        vdp.write_vram_u8(32 + i, 0x11);
        vdp.write_vram_u8(64 + i, 0x22);
    }
    vdp.write_vsram_u16(1, 8);
    vdp.write_vram_u8(0xF002, 0xFE);
    vdp.write_vram_u8(0xF003, 0x00);

    vdp.step(Vdp::CYCLES_PER_FRAME as u32);
    assert_eq!(&vdp.frame_buffer()[0..3], &[0, 252, 0]);
}

#[test]
fn dma_fill_writes_repeated_bytes_to_vram() {
    let mut vdp = Vdp::new();
    // Register 1: display + DMA enable.
    vdp.write_control_port(0x8150);
    // Auto-increment = 2 bytes.
    vdp.write_control_port(0x8F02);
    // DMA length = 3 words.
    vdp.write_control_port(0x9303);
    vdp.write_control_port(0x9400);
    // DMA mode = fill.
    vdp.write_control_port(0x9780);

    // VRAM write DMA command @ 0x0000 (code with DMA bit set).
    vdp.write_control_port(0x4000);
    vdp.write_control_port(0x0080);
    // Fill value provided via data port.
    vdp.write_data_port(0xA1B2);
    // Flush the gradual DMA fill so bytes are written immediately for test.
    vdp.flush_pending_dma();

    // Current model writes the initial data-port word first, then fills the
    // byte lane selected by A0 while auto-incrementing.
    assert_eq!(vdp.read_vram_u8(0x0000), 0xA1);
    assert_eq!(vdp.read_vram_u8(0x0001), 0xB2);
    assert_eq!(vdp.read_vram_u8(0x0002), 0x00);
    assert_eq!(vdp.read_vram_u8(0x0003), 0xB2);
    assert_eq!(vdp.read_vram_u8(0x0004), 0x00);
    assert_eq!(vdp.read_vram_u8(0x0005), 0xB2);
}

#[test]
fn dma_copy_copies_vram_bytes() {
    let mut vdp = Vdp::new();
    // Register 1: display + DMA enable.
    vdp.write_control_port(0x8150);
    // Auto-increment = 1 byte.
    vdp.write_control_port(0x8F01);
    // DMA length = 4 bytes.
    vdp.write_control_port(0x9304);
    vdp.write_control_port(0x9400);
    // DMA source = 0x0100.
    vdp.write_control_port(0x9500);
    vdp.write_control_port(0x9601);
    // DMA mode = copy.
    vdp.write_control_port(0x97C0);

    vdp.write_vram_u8(0x0100, 0x11);
    vdp.write_vram_u8(0x0101, 0x22);
    vdp.write_vram_u8(0x0102, 0x33);
    vdp.write_vram_u8(0x0103, 0x44);

    // VRAM write DMA command @ 0x0200 (code with DMA bit set).
    vdp.write_control_port(0x4200);
    vdp.write_control_port(0x0080);
    // Flush the gradual DMA copy so bytes are written immediately for test.
    vdp.flush_pending_dma();

    assert_eq!(vdp.read_vram_u8(0x0200), 0x11);
    assert_eq!(vdp.read_vram_u8(0x0201), 0x22);
    assert_eq!(vdp.read_vram_u8(0x0202), 0x33);
    assert_eq!(vdp.read_vram_u8(0x0203), 0x44);
}

#[test]
fn bus_dma_request_contains_expected_fields_for_vram_target() {
    let mut vdp = Vdp::new();
    // Register 1: display + DMA enable.
    vdp.write_control_port(0x8150);
    // Auto-increment = 2 bytes.
    vdp.write_control_port(0x8F02);
    // DMA length = 4 words.
    vdp.write_control_port(0x9304);
    vdp.write_control_port(0x9400);
    // DMA source = encoded 0x021123 -> bus 0x042246.
    vdp.write_control_port(0x9523);
    vdp.write_control_port(0x9611);
    vdp.write_control_port(0x9702); // bus mode (bit7=0)

    // VRAM write DMA command @ 0x0400 (DMA request bit set).
    vdp.write_control_port(0x4400);
    vdp.write_control_port(0x0080);

    let request = vdp
        .take_bus_dma_request()
        .expect("bus DMA request should be queued");
    assert_eq!(request.target, DmaTarget::Vram);
    assert_eq!(request.source_addr, 0x0042_246);
    assert_eq!(request.dest_addr, 0x0400);
    assert_eq!(request.auto_increment, 2);
    assert_eq!(request.words, 4);
}

#[test]
fn complete_bus_dma_updates_source_and_clears_length_registers() {
    let mut vdp = Vdp::new();
    vdp.write_control_port(0x8150); // display + DMA enable
    vdp.write_control_port(0x9302); // length low
    vdp.write_control_port(0x9400); // length high
    vdp.write_control_port(0x9500); // source low
    vdp.write_control_port(0x9600); // source mid
    vdp.write_control_port(0x9700); // source high / bus mode

    // Queue bus DMA request.
    vdp.write_control_port(0x4000);
    vdp.write_control_port(0x0080);
    let _ = vdp.take_bus_dma_request().expect("request expected");

    vdp.complete_bus_dma(0x0012_3456);
    // DMA length should be cleared.
    assert_eq!(vdp.register(19), 0);
    assert_eq!(vdp.register(20), 0);

    // Source LOW/MID should encode 0x123456 >> 1 = 0x091A2B (low=0x2B, mid=0x1A).
    assert_eq!(vdp.register(21), 0x2B);
    assert_eq!(vdp.register(22), 0x1A);
    // Source HIGH should NOT be updated (frozen during transfer).
    assert_eq!(
        vdp.register(23) & 0x7F,
        0x00,
        "DMA source high register should be frozen"
    );
}

#[test]
fn complete_bus_dma_freezes_source_high_register() {
    let mut vdp = Vdp::new();
    vdp.write_control_port(0x8150); // display + DMA enable
    vdp.write_control_port(0x9302); // length low
    vdp.write_control_port(0x9400); // length high
    vdp.write_control_port(0x9500); // source low = 0
    vdp.write_control_port(0x9600); // source mid = 0
    vdp.write_control_port(0x9710); // source high = 0x10 (bank at 0x200000+)

    vdp.write_control_port(0x4000);
    vdp.write_control_port(0x0080);
    let _ = vdp.take_bus_dma_request().expect("request expected");

    // Complete DMA with next_source_addr in a different 128KB region
    vdp.complete_bus_dma(0x0000_1234);
    // LOW/MID updated to reflect new address
    assert_eq!(vdp.register(21), 0x1A); // 0x1234 >> 1 = 0x091A, low = 0x1A
    assert_eq!(vdp.register(22), 0x09); // mid = 0x09
    // HIGH should remain frozen at original value
    assert_eq!(
        vdp.register(23) & 0x7F,
        0x10,
        "source high register must be frozen"
    );
}

#[test]
fn shadow_highlight_dac_accuracy() {
    // Verify shadow_channel and highlight_channel match the 4-bit DAC model.
    // Normal level L maps to L*36 (0-252).
    // Shadow = channel >> 1 (4-bit DAC output L vs normal 2L).
    // Highlight = channel + 18, clamped to 255 (4-bit DAC output 2L+1 vs normal 2L).
    use crate::vdp::{highlight_channel, shadow_channel};
    for level in 0..=7u8 {
        let normal = level as u16 * 36;
        let expected_shadow = normal / 2;
        let expected_highlight = (normal + 18).min(255);
        assert_eq!(
            shadow_channel(normal as u8) as u16,
            expected_shadow,
            "shadow of level {} (normal={})",
            level,
            normal
        );
        assert_eq!(
            highlight_channel(normal as u8) as u16,
            expected_highlight,
            "highlight of level {} (normal={})",
            level,
            normal
        );
    }
}

#[test]
fn shadow_highlight_transparent_plane_a_priority_prevents_shadow() {
    // On real VDP, a transparent plane A tile with priority=true still
    // prevents shadowing of the underlying plane B pixel.  The S/H
    // brightness is the OR of both planes' raw priority bits.
    let mut vdp = Vdp::new();
    let plane_a_base = 0xC000u16; // reg 2 = 0x30
    // Set plane B nametable to 0xE000 (reg 4 = 0x07).
    vdp.write_control_port(0x8407);

    // Enable S/H mode (reg 12 bit 3) + H40.
    vdp.write_control_port(0x8C89);

    // Use tile 2 for plane B to avoid overlap with tile 0 data.
    // Plane A at (0,0): tile 0 with priority=true.
    // Tile 0 data is all zeros → transparent.
    vdp.vram[0..32].fill(0); // ensure tile 0 is all transparent
    vdp.write_vram_u8(plane_a_base, 0x80); // priority bit set
    vdp.write_vram_u8(plane_a_base + 1, 0x00); // tile index 0

    // Plane B at (0,0): tile 2 with priority=false, opaque pixel.
    vdp.write_vram_u8(0xE000, 0x00);
    vdp.write_vram_u8(0xE001, 0x02); // tile index 2
    // Tile 2 pixel data: color index 1 for first row.
    let tile2_addr = 2 * 32;
    vdp.write_vram_u8(tile2_addr, 0x11);
    vdp.write_vram_u8(tile2_addr + 1, 0x11);
    vdp.write_vram_u8(tile2_addr + 2, 0x11);
    vdp.write_vram_u8(tile2_addr + 3, 0x11);
    vdp.write_cram_u16(1, encode_md_color(3, 0, 0)); // level 3 red = 108

    vdp.step(Vdp::CYCLES_PER_FRAME as u32);
    // Plane A is transparent but has priority → pixel should be NORMAL
    // brightness (not shadowed).  Level 3 = 108.
    assert_eq!(
        &vdp.frame_buffer()[0..3],
        &[108, 0, 0],
        "transparent plane A with priority should prevent shadow on plane B"
    );

    // Now remove plane A priority → pixel should be shadowed.
    vdp.write_vram_u8(plane_a_base, 0x00); // priority bit clear
    vdp.step(Vdp::CYCLES_PER_FRAME as u32);
    // Shadow of 108 = 108 >> 1 = 54.
    assert_eq!(
        &vdp.frame_buffer()[0..3],
        &[54, 0, 0],
        "without plane A priority, plane B should be shadowed"
    );
}

#[test]
fn shadow_highlight_control_sprite_does_not_occupy_sprite_layer() {
    // S/H control sprites (palette 3, color 14/15) should be transparent
    // to the sprite layer — a subsequent normal sprite must still render
    // at the same pixel position.
    let mut vdp = Vdp::new();
    let sat = 0xE000u16;

    // Enable S/H mode + H40.
    vdp.write_control_port(0x8C89);

    // Plane A at (0,0): tile 1, no priority, opaque (color index 1).
    vdp.write_vram_u8(0xC000, 0x00);
    vdp.write_vram_u8(0xC001, 0x01); // tile 1
    vdp.write_vram_u8(32, 0x11); // tile 1 pixel data: color 1
    vdp.write_cram_u16(1, encode_md_color(3, 0, 0)); // level 3 red = 108

    // Sprite 0: highlight control (palette 3, color 14) at position (0,0).
    // Tile 4 first pixel = 14 (0xE).
    vdp.write_vram_u8(4 * 32, 0xE0);
    vdp.write_vram_u8(sat, 0x00);
    vdp.write_vram_u8(sat + 1, 0x80); // y=128 → screen y=0
    vdp.write_vram_u8(sat + 2, 0x00); // 1x1 size
    vdp.write_vram_u8(sat + 3, 0x01); // link → sprite 1
    vdp.write_vram_u8(sat + 4, 0x60); // palette 3, no priority
    vdp.write_vram_u8(sat + 5, 0x04); // tile 4
    vdp.write_vram_u8(sat + 6, 0x00);
    vdp.write_vram_u8(sat + 7, 0x80); // x=128 → screen x=0

    // Sprite 1: normal high-priority sprite (green, palette 0 color 2).
    // Tile 5 first pixel = 2.
    vdp.write_vram_u8(5 * 32, 0x20);
    vdp.write_cram_u16(2, encode_md_color(0, 5, 0)); // green level 5 = 180
    vdp.write_vram_u8(sat + 8, 0x00);
    vdp.write_vram_u8(sat + 9, 0x80); // y=128
    vdp.write_vram_u8(sat + 10, 0x00);
    vdp.write_vram_u8(sat + 11, 0x00); // link=0 → end
    vdp.write_vram_u8(sat + 12, 0x80); // high priority, palette 0
    vdp.write_vram_u8(sat + 13, 0x05); // tile 5
    vdp.write_vram_u8(sat + 14, 0x00);
    vdp.write_vram_u8(sat + 15, 0x80); // x=128

    vdp.step(Vdp::CYCLES_PER_FRAME as u32);
    // Sprite 1 (normal, high-priority) should render because the
    // highlight control sprite does NOT occupy the sprite layer.
    // High-priority sprite in S/H mode → normal brightness = green 180.
    assert_eq!(
        &vdp.frame_buffer()[0..3],
        &[0, 180, 0],
        "normal sprite should render over S/H control sprite"
    );
}

#[test]
fn fifo_status_reports_empty_and_full() {
    let mut vdp = Vdp::default();
    // Set auto-increment to 2
    vdp.write_control_port(0x8F02);
    // Set VRAM write mode
    vdp.write_control_port(0x4000);
    vdp.write_control_port(0x0000);

    // Initially FIFO is empty
    let status = vdp.read_control_port();
    assert!(
        status & 0x0200 != 0,
        "FIFO empty bit should be set initially"
    );
    assert!(
        status & 0x0100 == 0,
        "FIFO full bit should be clear initially"
    );

    // Re-set write mode (read_control_port clears latch)
    vdp.write_control_port(0x4000);
    vdp.write_control_port(0x0000);

    // Write 4 words to fill FIFO
    for i in 0..4u16 {
        vdp.write_data_port(i);
    }
    let status = vdp.read_control_port();
    assert!(
        status & 0x0200 == 0,
        "FIFO empty bit should be clear after writes"
    );
    assert!(
        status & 0x0100 != 0,
        "FIFO full bit should be set after 4 writes"
    );

    // Step enough cycles to drain FIFO (at least 4 * 18 = 72 cycles)
    vdp.write_control_port(0x4000);
    vdp.write_control_port(0x0000);
    vdp.step(100);

    let status = vdp.read_control_port();
    assert!(
        status & 0x0200 != 0,
        "FIFO empty bit should be set after draining"
    );
    assert!(
        status & 0x0100 == 0,
        "FIFO full bit should be clear after draining"
    );
}
