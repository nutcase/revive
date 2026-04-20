use super::*;

#[test]
fn test_ppu_data_read_write() {
    let mut ppu = Ppu::new();

    // Set VRAM address to nametable area
    ppu.v = 0x2000;

    // Write data (increment by 1)
    ppu.write_register(0x2007, 0x42, None);
    assert_eq!(ppu.nametable[0][0], 0x42);
    assert_eq!(ppu.v, 0x2001); // Auto-increment by 1

    // Write another byte
    ppu.write_register(0x2007, 0x43, None);
    assert_eq!(ppu.nametable[0][1], 0x43);
    assert_eq!(ppu.v, 0x2002); // Auto-increment by 1

    // Test increment mode (32)
    ppu.control.insert(PpuControl::VRAM_INCREMENT);
    ppu.write_register(0x2007, 0x44, None);
    assert_eq!(ppu.nametable[0][2], 0x44);
    assert_eq!(ppu.v, 0x2022); // Auto-increment by 32
}

#[test]
fn test_palette_write() {
    let mut ppu = Ppu::new();

    // Write to palette RAM
    ppu.v = 0x3F00;
    ppu.write_register(0x2007, 0x0F, None); // Black
    assert_eq!(ppu.palette[0], 0x0F);

    // Test palette mirroring
    ppu.v = 0x3F10;
    ppu.write_register(0x2007, 0x30, None); // White
    assert_eq!(ppu.palette[0], 0x30); // Mirrors to 0x3F00
}

#[test]
fn test_nametable_mirroring() {
    let mut ppu = Ppu::new();

    // Test horizontal mirroring
    ppu.v = 0x2000;
    ppu.write_register(0x2007, 0x11, None);
    ppu.v = 0x2400;
    ppu.write_register(0x2007, 0x22, None);

    // In horizontal mirroring (default, no cartridge), 0x2000 and 0x2400 map to different nametables
    assert_eq!(ppu.nametable[0][0], 0x11);
    assert_eq!(ppu.nametable[1][0], 0x22);
}
