use snes_emulator::bus::Bus;
use snes_emulator::cartridge::MapperType;

fn patterned_rom(size: usize) -> Vec<u8> {
    let mut rom = vec![0u8; size];
    for (i, byte) in rom.iter_mut().enumerate() {
        *byte = ((i >> 8) as u8).wrapping_add(i as u8);
    }
    rom
}

#[test]
fn spc7110_rtc_state_survives_bus_savestate_roundtrip() {
    let rom = patterned_rom(0x20_0000);
    let mut bus = Bus::new_with_mapper(rom.clone(), MapperType::Spc7110, 0x2000);

    bus.write_u8(0x00_4840, 0x01);
    bus.write_u8(0x00_4841, 0x03);
    bus.write_u8(0x00_4841, 0x0F);
    bus.write_u8(0x00_4841, 0x06);
    bus.write_u8(0x00_4841, 0x5);
    bus.write_u8(0x00_4841, 0x4);
    bus.write_u8(0x00_4841, 0x3);
    bus.write_u8(0x00_4841, 0x2);
    bus.write_u8(0x00_4841, 0x1);
    bus.write_u8(0x00_4841, 0x1);
    bus.write_u8(0x00_4841, 0x7);
    bus.write_u8(0x00_4841, 0x1);
    bus.write_u8(0x00_4841, 0x8);
    bus.write_u8(0x00_4841, 0x0);
    bus.write_u8(0x00_4841, 0x6);
    bus.write_u8(0x00_4841, 0x2);
    bus.write_u8(0x00_4841, 0x1);

    bus.write_u8(0x00_4840, 0x00);
    bus.write_u8(0x00_4840, 0x01);
    bus.write_u8(0x00_4841, 0x03);
    bus.write_u8(0x00_4841, 0x00);

    let state = bus.to_save_state();

    let mut restored = Bus::new_with_mapper(rom, MapperType::Spc7110, 0x2000);
    restored.load_from_save_state(&state);

    let expected = [5, 4, 3, 2, 1, 1, 7, 1, 8, 0, 6, 2, 1];
    let mut actual = [0u8; 13];
    for byte in &mut actual {
        *byte = restored.read_u8(0x00_4841);
    }

    assert_eq!(actual, expected);
    assert_eq!(restored.read_u8(0x00_4842), 0x80);
}

#[test]
fn superfx_state_survives_bus_savestate_roundtrip() {
    let rom = patterned_rom(0x20_0000);
    let mut bus = Bus::new_with_mapper(rom.clone(), MapperType::SuperFx, 0x2000);

    bus.write_u8(0x00_3034, 0x5A);
    bus.write_u8(0x00_303A, 0x00);
    bus.write_u8(0x00_3104, 0xCC);
    bus.write_u8(0x70_0010, 0xAA);
    bus.write_u8(0x00_301C, 0x34);
    bus.write_u8(0x00_301D, 0x12);
    bus.write_u8(0x00_3031, 0x80);

    let state = bus.to_save_state();

    let mut restored = Bus::new_with_mapper(rom, MapperType::SuperFx, 0x2000);
    restored.load_from_save_state(&state);

    assert_eq!(restored.read_u8(0x00_3034), 0x5A);
    assert_eq!(restored.read_u8(0x00_3104), 0xCC);
    assert_eq!(restored.read_u8(0x70_0010), 0xAA);
    assert_eq!(restored.read_u8(0x00_301C), 0x34);
    assert_eq!(restored.read_u8(0x00_301D), 0x12);
    assert!(restored.irq_is_pending());
}
