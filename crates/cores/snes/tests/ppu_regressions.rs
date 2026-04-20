use snes_emulator::ppu::Ppu;

#[test]
fn ppu_vram_write_and_savestate_roundtrip() {
    let mut ppu = Ppu::new();
    let word_addr = 0x1234u16;
    let idx = word_addr as usize * 2;

    ppu.write_vram_word(word_addr, 0xAA, 0x55);
    assert_eq!(ppu.get_vram()[idx], 0xAA);
    assert_eq!(ppu.get_vram()[idx + 1], 0x55);

    let state = ppu.to_save_state();
    let mut ppu2 = Ppu::new();
    ppu2.load_from_save_state(&state);

    assert_eq!(ppu2.get_vram()[idx], 0xAA);
    assert_eq!(ppu2.get_vram()[idx + 1], 0x55);
}

#[test]
fn ppu_force_framebuffer_color_marks_buffer_non_black() {
    let mut ppu = Ppu::new();
    ppu.force_framebuffer_color(0x00123456);

    assert!(!ppu.framebuffer_is_all_black());
    assert_eq!(ppu.get_framebuffer()[0], 0x00123456);
}
