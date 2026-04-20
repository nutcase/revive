use snes_emulator::bus::Bus;
use snes_emulator::cartridge::MapperType;

fn make_test_rom(size: usize) -> Vec<u8> {
    let mut rom = vec![0u8; size];
    for (i, byte) in rom.iter_mut().enumerate() {
        *byte = ((i >> 8) as u8) ^ (i as u8);
    }
    rom
}

fn make_superfx_bus() -> (Bus, Vec<u8>) {
    let rom = make_test_rom(0x20_0000);
    let bus = Bus::new_with_mapper(rom.clone(), MapperType::SuperFx, 0x2000);
    (bus, rom)
}

#[test]
fn superfx_cpu_rom_windows_match_lorom_and_hirom_views() {
    let (mut bus, rom) = make_superfx_bus();

    assert_eq!(bus.read_u8(0x00_8000), rom[0x000000]);
    assert_eq!(bus.read_u8(0x01_8000), rom[0x008000]);
    assert_eq!(bus.read_u8(0x40_1234), rom[0x001234]);
    assert_eq!(bus.read_u8(0x41_1234), rom[0x011234]);
    assert_eq!(bus.read_u8(0xC1_1234), rom[0x011234]);
}

#[test]
fn superfx_register_window_mirrors_and_irq_clears_on_sfr_high_read() {
    let (mut bus, _) = make_superfx_bus();

    bus.write_u8(0x00_3002, 0x34);
    bus.write_u8(0x00_3003, 0x12);

    assert_eq!(bus.read_u8(0x00_3002), 0x34);
    assert_eq!(bus.read_u8(0x00_3003), 0x12);

    bus.write_u8(0x00_3031, 0x80);
    assert_eq!(bus.read_u8(0x00_3030) & 0x20, 0);
    assert!(bus.irq_is_pending());

    let sfr_hi = bus.read_u8(0x00_3031);
    assert_ne!(sfr_hi & 0x80, 0);
    assert!(!bus.irq_is_pending());

    bus.write_u8(0x00_3334, 0x5A);
    assert_eq!(bus.read_u8(0x00_3034), 0x5A);
}

#[test]
fn superfx_cache_ram_window_is_cpu_accessible() {
    let (mut bus, _) = make_superfx_bus();

    bus.write_u8(0x00_3100, 0xA5);
    bus.write_u8(0x00_32FF, 0x5A);

    assert_eq!(bus.read_u8(0x00_3100), 0xA5);
    assert_eq!(bus.read_u8(0x00_32FF), 0x5A);
}

#[test]
fn superfx_game_ram_and_backup_ram_windows_behave_as_expected() {
    let (mut bus, _) = make_superfx_bus();

    bus.write_u8(0x70_0001, 0xAA);
    assert_eq!(bus.read_u8(0x70_0001), 0xAA);
    assert_eq!(bus.read_u8(0x00_6001), 0xAA);

    bus.write_u8(0x7C_0000, 0x55);
    assert_eq!(bus.read_u8(0x7C_0000), 0xFF);

    bus.write_u8(0x00_3033, 0x01);
    bus.write_u8(0x7C_0000, 0x55);
    assert_eq!(bus.read_u8(0x7C_0000), 0x55);
}

#[test]
fn superfx_scmr_blocks_cpu_rom_and_ram_access() {
    let (mut bus, rom) = make_superfx_bus();

    bus.write_u8(0x70_0000, 0x99);
    assert_eq!(bus.read_u8(0x00_6000), 0x99);

    bus.write_u8(0x00_303A, 0x18);

    // SCMR alone does not block S-CPU access once the GSU has already gone idle.
    assert_eq!(bus.read_u8(0x00_8005), rom[0x000005]);
    assert_eq!(bus.read_u8(0x40_8005), rom[0x008005]);
    assert_eq!(bus.read_u8(0x00_6000), 0x99);
    assert_eq!(bus.read_u8(0x70_0000), 0x99);

    bus.write_u8(0x00_6000, 0x11);
    bus.write_u8(0x70_0000, 0x22);

    bus.write_u8(0x00_303A, 0x00);
    assert_eq!(bus.read_u8(0x00_6000), 0x22);
    assert_eq!(bus.read_u8(0x70_0000), 0x22);
}
