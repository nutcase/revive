use super::*;

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
    // Width=64 would wrap this position to x=0; width=128 keeps it distinct
    // in the same row-major nametable.
    let x64 = plane_a + 64 * 2;
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
fn plane_a_128x32_uses_linear_row_major_nametable() {
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
    vdp.write_vram_u8((linear_addr + 1) as u16, 0x02);
    vdp.write_vram_u8(paged_addr as u16, 0x00);
    vdp.write_vram_u8((paged_addr + 1) as u16, 0x01);

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
fn plane_b_128x32_uses_linear_row_major_nametable() {
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
    vdp.write_vram_u8((linear_addr + 1) as u16, 0x02);
    vdp.write_vram_u8(paged_addr as u16, 0x00);
    vdp.write_vram_u8((paged_addr + 1) as u16, 0x01);

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
