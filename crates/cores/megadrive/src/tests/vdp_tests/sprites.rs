use super::*;

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
