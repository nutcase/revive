use super::*;

#[test]
fn test_ppu_control_register() {
    let mut ppu = Ppu::new();

    // Write to PPUCTRL ($2000)
    ppu.write_register(0x2000, 0xFF, None);

    assert!(ppu.control.contains(PpuControl::NAMETABLE_X));
    assert!(ppu.control.contains(PpuControl::NAMETABLE_Y));
    assert!(ppu.control.contains(PpuControl::VRAM_INCREMENT));
    assert!(ppu.control.contains(PpuControl::SPRITE_PATTERN));
    assert!(ppu.control.contains(PpuControl::BG_PATTERN));
    assert!(ppu.control.contains(PpuControl::SPRITE_SIZE));
    assert!(ppu.control.contains(PpuControl::PPU_MASTER_SLAVE));
    assert!(ppu.control.contains(PpuControl::NMI_ENABLE));
}

#[test]
fn test_ppu_mask_register() {
    let mut ppu = Ppu::new();

    // Write to PPUMASK ($2001)
    ppu.write_register(0x2001, 0xFF, None);

    assert!(ppu.mask.contains(PpuMask::GRAYSCALE));
    assert!(ppu.mask.contains(PpuMask::BG_LEFT_ENABLE));
    assert!(ppu.mask.contains(PpuMask::SPRITE_LEFT_ENABLE));
    assert!(ppu.mask.contains(PpuMask::BG_ENABLE));
    assert!(ppu.mask.contains(PpuMask::SPRITE_ENABLE));
    assert!(ppu.mask.contains(PpuMask::EMPHASIZE_RED));
    assert!(ppu.mask.contains(PpuMask::EMPHASIZE_GREEN));
    assert!(ppu.mask.contains(PpuMask::EMPHASIZE_BLUE));
}

#[test]
fn test_ppu_status_register() {
    let mut ppu = Ppu::new();

    // Set some status flags
    ppu.status.insert(PpuStatus::VBLANK);
    ppu.status.insert(PpuStatus::SPRITE_0_HIT);
    ppu.status.insert(PpuStatus::SPRITE_OVERFLOW);

    // Read PPUSTATUS ($2002)
    let status = ppu.read_register(0x2002, None);

    assert_eq!(status & 0xE0, 0xE0); // Top 3 bits should be set

    // VBLANK should be cleared after read
    assert!(!ppu.status.contains(PpuStatus::VBLANK));

    // w register should be reset
    assert!(!ppu.w);
}

#[test]
fn test_oam_addr_and_data() {
    let mut ppu = Ppu::new();

    // Write OAM address
    ppu.write_register(0x2003, 0x10, None);
    assert_eq!(ppu.oam_addr, 0x10);

    // Write OAM data
    ppu.write_register(0x2004, 0x42, None);
    assert_eq!(ppu.oam[0x10], 0x42);

    // Write more data
    ppu.write_register(0x2004, 0x43, None);
    assert_eq!(ppu.oam[0x11], 0x43);
}

#[test]
fn test_oam_read() {
    let mut ppu = Ppu::new();

    // Set up some OAM data
    ppu.oam[0x20] = 0x55;
    ppu.oam_addr = 0x20;

    // Read OAM data
    let data = ppu.read_register(0x2004, None);
    assert_eq!(data, 0x55);
    // OAM address doesn't increment on read
    assert_eq!(ppu.oam_addr, 0x20);
}

#[test]
fn test_scroll_register() {
    let mut ppu = Ppu::new();

    // First write (X scroll)
    ppu.write_register(0x2005, 0x20, None);
    assert_eq!(ppu.x, 0x00); // Fine X = 0x20 & 7
    assert!(ppu.w);

    // Second write (Y scroll)
    ppu.write_register(0x2005, 0x30, None);
    assert!(!ppu.w);
}

#[test]
fn test_ppu_addr_register() {
    let mut ppu = Ppu::new();

    // First write (high byte)
    ppu.write_register(0x2006, 0x21, None);
    assert!(ppu.w);

    // Second write (low byte)
    ppu.write_register(0x2006, 0x08, None);
    assert_eq!(ppu.t, 0x2108);
    assert_eq!(ppu.v, 0x2108); // v = t on second write
    assert!(!ppu.w);
}
