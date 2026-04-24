use nes_emulator::Nes;

#[test]
fn synthetic_vrc7_audio_reaches_output_buffer() {
    let rom_path = std::env::temp_dir().join("revive_synthetic_vrc7.nes");
    std::fs::write(&rom_path, synthetic_vrc7_rom()).unwrap();

    let mut nes = Nes::new();
    nes.load_rom(rom_path.to_str().unwrap()).unwrap();

    let mut peak = 0.0f32;
    for _ in 0..20 {
        while !nes.step() {}
        for sample in nes.get_audio_buffer() {
            peak = peak.max(sample.abs());
        }
    }

    assert!(peak > 0.01, "peak={peak}");
}

fn synthetic_vrc7_rom() -> Vec<u8> {
    let mut rom = Vec::new();
    rom.extend_from_slice(b"NES\x1A");
    rom.push(2); // 32 KiB PRG
    rom.push(0); // CHR RAM
    rom.push(0x52); // mapper 85, battery
    rom.push(0x50);
    rom.extend_from_slice(&[0; 8]);

    let mut prg = vec![0xEA; 0x8000];
    let program = [
        0xA9, 0x80, 0x8D, 0x00, 0xE0, // enable WRAM, clear sound reset
        0xA9, 0x10, 0x8D, 0x10, 0x90, // select frequency low
        0xA9, 0x80, 0x8D, 0x30, 0x90, // write frequency low
        0xA9, 0x20, 0x8D, 0x10, 0x90, // select frequency high/key
        0xA9, 0x19, 0x8D, 0x30, 0x90, // key on, octave 4, high bit 1
        0xA9, 0x30, 0x8D, 0x10, 0x90, // select instrument/volume
        0xA9, 0x10, 0x8D, 0x30, 0x90, // instrument 1, volume 0
        0x4C, 0x23, 0xE0, // loop
    ];
    let start = 0x6000;
    prg[start..start + program.len()].copy_from_slice(&program);
    prg[0x7FFC] = 0x00;
    prg[0x7FFD] = 0xE0;
    rom.extend_from_slice(&prg);
    rom
}
