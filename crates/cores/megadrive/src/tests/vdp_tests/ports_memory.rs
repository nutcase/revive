use super::*;

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
