use megadrive_core::cartridge::Cartridge;
use megadrive_core::memory::MemoryMap;

fn write_ascii_field(rom: &mut [u8], start: usize, end: usize, text: &str) {
    let field = &mut rom[start..end];
    field.fill(b' ');
    let bytes = text.as_bytes();
    let len = bytes.len().min(field.len());
    field[..len].copy_from_slice(&bytes[..len]);
}

fn build_rom_with_header(
    product_code: &str,
    domestic_title: &str,
    overseas_title: &str,
) -> Vec<u8> {
    let mut rom = vec![0; 0x400];
    write_ascii_field(&mut rom, 0x100, 0x110, "SEGA MEGA DRIVE");
    write_ascii_field(&mut rom, 0x120, 0x150, domestic_title);
    write_ascii_field(&mut rom, 0x150, 0x180, overseas_title);
    write_ascii_field(&mut rom, 0x180, 0x18E, product_code);
    write_ascii_field(&mut rom, 0x1F0, 0x200, "J");
    rom
}

fn setup_plane_a_128x32_lookup_probe(memory: &mut MemoryMap) {
    let vdp = memory.vdp_mut();
    // 128x32 map.
    vdp.write_control_port(0x9003);
    let plane_a_base = 0xC000usize;

    // At (x=0, y=31): paged lookup should read this red tile entry.
    let paged_row31 = plane_a_base + 31 * 64 * 2;
    // Linear lookup would read this green tile entry.
    let linear_row31 = plane_a_base + 31 * 128 * 2;
    vdp.write_vram_u8(paged_row31 as u16, 0x00);
    vdp.write_vram_u8((paged_row31 + 1) as u16, 0x01);
    vdp.write_vram_u8(linear_row31 as u16, 0x00);
    vdp.write_vram_u8((linear_row31 + 1) as u16, 0x02);

    // Tile 1 -> color index 1 (red), tile 2 -> color index 2 (green).
    vdp.write_vram_u8(32, 0x11);
    vdp.write_vram_u8(64, 0x22);
    vdp.write_cram_u16(1, 0x000E);
    vdp.write_cram_u16(2, 0x00E0);

    // Sample tile row 31 at y=0.
    vdp.write_vsram_u16(0, 31 * 8);
}

#[test]
fn sonic3_product_code_enables_plane_a_64x32_paged_regression() {
    let rom = build_rom_with_header(
        "GM MK-1079 -00",
        "SONIC THE HEDGEHOG 3",
        "SONIC THE HEDGEHOG 3",
    );
    let cart = Cartridge::from_bytes(rom).expect("valid cart");
    let mut memory = MemoryMap::new(cart);
    setup_plane_a_128x32_lookup_probe(&mut memory);

    assert!(memory.step_vdp(130_000));
    assert_eq!(&memory.frame_buffer()[0..3], &[252, 0, 0]);
}

#[test]
fn non_sonic_rom_keeps_linear_plane_a_lookup() {
    let rom = build_rom_with_header("GM TEST-0000", "TEST GAME", "TEST GAME");
    let cart = Cartridge::from_bytes(rom).expect("valid cart");
    let mut memory = MemoryMap::new(cart);
    setup_plane_a_128x32_lookup_probe(&mut memory);

    assert!(memory.step_vdp(130_000));
    assert_eq!(&memory.frame_buffer()[0..3], &[0, 252, 0]);
}
