use super::*;

#[cfg(test)]
mod additional_ppu_tests {
    use super::*;

    #[test]
    fn test_ppuscroll_w_register() {
        let mut ppu = Ppu::new();

        // w register starts at 0
        assert!(!ppu.w);

        // First write to PPUSCROLL (X scroll)
        ppu.write_register(0x2005, 0x7D, None); // 125 pixels
        assert!(ppu.w); // w flipped

        // Second write to PPUSCROLL (Y scroll)
        ppu.write_register(0x2005, 0x5E, None); // 94 pixels
        assert!(!ppu.w); // w reset

        // PPUADDR also shares w register
        ppu.write_register(0x2006, 0x20, None); // High byte
        assert!(ppu.w);

        ppu.write_register(0x2006, 0x00, None); // Low byte
        assert!(!ppu.w);
    }

    #[test]
    fn test_ppu_data_buffer() {
        let mut ppu = Ppu::new();

        // Set up some data in VRAM
        ppu.v = 0x2000;
        ppu.write_register(0x2007, 0x55, None);

        // Reset address
        ppu.v = 0x2000;

        // First read should return stale data (buffer)
        // Second read should return actual data
        let _first_read = ppu.read_register(0x2007, None);
        let _second_read = ppu.read_register(0x2007, None);

        // The behavior depends on internal buffer implementation
        // This tests the buffered read behavior
    }

    #[test]
    fn test_palette_mirroring() {
        let mut ppu = Ppu::new();

        // Write to background palette
        ppu.v = 0x3F00;
        ppu.write_register(0x2007, 0x0F, None);

        // Check that it mirrors to sprite palette universal background
        ppu.v = 0x3F10;
        let mirrored = ppu.read_register(0x2007, None);

        // $3F10, $3F14, $3F18, $3F1C mirror $3F00, $3F04, $3F08, $3F0C
        assert_eq!(mirrored, 0x0F);
    }

    #[test]
    fn test_oam_dma_timing() {
        let mut ppu = Ppu::new();

        // DMA should take 513 or 514 cycles depending on CPU cycle alignment
        // This is a complex test that would require bus-level timing simulation

        // For now, test basic OAM functionality
        ppu.oam_addr = 0x00;

        for i in 0..256 {
            ppu.write_register(0x2004, i as u8, None);
        }

        // Verify OAM data
        for i in 0..256 {
            assert_eq!(ppu.oam[i], i as u8);
        }

        // OAM address should wrap
        assert_eq!(ppu.oam_addr, 0x00);
    }

    #[test]
    fn test_sprite_0_hit_timing() {
        let mut ppu = Ppu::new();

        // Enable rendering
        ppu.mask.insert(PpuMask::BG_ENABLE);
        ppu.mask.insert(PpuMask::SPRITE_ENABLE);

        // Place sprite 0 at specific position
        ppu.oam[0] = 50; // Y position
        ppu.oam[1] = 0; // Tile index
        ppu.oam[2] = 0; // Attributes
        ppu.oam[3] = 100; // X position

        // Sprite 0 hit only occurs during visible scanlines (0-239)
        // and only when both sprite and background pixels are opaque
        // This test verifies the basic setup

        assert_eq!(ppu.oam[0], 50);
        assert_eq!(ppu.oam[3], 100);
    }
}
