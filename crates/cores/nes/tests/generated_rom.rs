use nes_emulator::Nes;

#[test]
fn generated_nrom_reaches_first_frame() {
    let path = write_generated_nrom();
    let mut nes = Nes::new();

    nes.load_rom(path.to_str().expect("utf-8 path")).unwrap();

    let frame_complete = run_until_first_frame(&mut nes);

    let _ = std::fs::remove_file(path);

    assert!(frame_complete);
    assert_eq!(nes.get_frame_buffer().len(), 256 * 240 * 3);
}

#[test]
fn generated_nes2_nrom_with_chr_ram_reaches_first_frame() {
    let path = write_generated_nes2_nrom_with_chr_ram();
    let mut nes = Nes::new();

    nes.load_rom(path.to_str().expect("utf-8 path")).unwrap();

    let frame_complete = run_until_first_frame(&mut nes);

    let _ = std::fs::remove_file(path);

    assert!(frame_complete);
    assert_eq!(nes.get_frame_buffer().len(), 256 * 240 * 3);
}

fn run_until_first_frame(nes: &mut Nes) -> bool {
    for _ in 0..20_000 {
        if nes.step() {
            return true;
        }
    }
    false
}

fn write_generated_nrom() -> std::path::PathBuf {
    let mut rom = Vec::with_capacity(16 + 16 * 1024 + 8 * 1024);
    rom.extend_from_slice(b"NES\x1A");
    rom.push(1); // 16KB PRG
    rom.push(1); // 8KB CHR
    rom.extend_from_slice(&[0; 10]);

    let mut prg = vec![0xEA; 16 * 1024];
    prg[0x0000] = 0x4C; // JMP $8000
    prg[0x0001] = 0x00;
    prg[0x0002] = 0x80;
    prg[0x3FFC] = 0x00;
    prg[0x3FFD] = 0x80;
    rom.extend_from_slice(&prg);
    rom.extend_from_slice(&vec![0; 8 * 1024]);

    write_rom_file("nrom", rom)
}

fn write_generated_nes2_nrom_with_chr_ram() -> std::path::PathBuf {
    let mut rom = Vec::with_capacity(16 + 16 * 1024);
    rom.extend_from_slice(b"NES\x1A");
    rom.push(1); // 16KB PRG
    rom.push(0); // CHR RAM, size comes from NES 2.0 byte 11
    rom.push(0);
    rom.push(0x08); // NES 2.0 marker
    rom.push(0);
    rom.push(0);
    rom.push(0);
    rom.push(7); // 8KB CHR RAM
    rom.extend_from_slice(&[0; 4]);

    let mut prg = vec![0xEA; 16 * 1024];
    prg[0x0000] = 0x4C; // JMP $8000
    prg[0x0001] = 0x00;
    prg[0x0002] = 0x80;
    prg[0x3FFC] = 0x00;
    prg[0x3FFD] = 0x80;
    rom.extend_from_slice(&prg);

    write_rom_file("nes2_nrom_chr_ram", rom)
}

fn write_rom_file(name: &str, rom: Vec<u8>) -> std::path::PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!(
        "nes_generated_{name}_{}.nes",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time")
            .as_nanos()
    ));
    std::fs::write(&path, rom).expect("write generated rom");
    path
}
