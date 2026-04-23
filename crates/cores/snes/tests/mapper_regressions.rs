use std::path::Path;

use snes_emulator::bus::Bus;
use snes_emulator::cartridge::{Cartridge, MapperType};

fn load_rom_if_present(path: &str) -> Option<Cartridge> {
    let path = Path::new(path);
    if !path.exists() {
        eprintln!("skip: missing ROM {}", path.display());
        return None;
    }
    Some(Cartridge::load_from_file(path).expect("ROM should parse"))
}

#[test]
fn star_ocean_uses_sdd1_mapper_variant_header() {
    let Some(cart) = load_rom_if_present("roms/Star Ocean (Japan).sfc") else {
        return;
    };
    assert_eq!(cart.header.mapper_type, MapperType::Sdd1);
}

#[test]
fn tengai_makyou_zero_uses_spc7110_variant_header() {
    let Some(cart) = load_rom_if_present("roms/Tengai Makyou Zero (Japan).sfc") else {
        return;
    };
    assert_eq!(cart.header.mapper_type, MapperType::Spc7110);
}

#[test]
fn star_fox_uses_superfx_variant_header() {
    let Some(cart) = load_rom_if_present("roms/Star Fox (Japan).sfc") else {
        return;
    };
    assert_eq!(cart.header.mapper_type, MapperType::SuperFx);
}

#[test]
fn exhirom_6mb_uses_a23_inverted_segments() {
    let mut rom = vec![0xFF; 0x600000];
    rom[0x000000] = 0xC0;
    rom[0x008000] = 0x80;
    rom[0x400000] = 0x40;
    rom[0x408000] = 0x00;

    let mut bus = Bus::new_with_mapper(rom, MapperType::ExHiRom, 0);

    assert_eq!(bus.read_u8(0x008000), 0x00);
    assert_eq!(bus.read_u8(0x808000), 0x80);
    assert_eq!(bus.read_u8(0x400000), 0x40);
    assert_eq!(bus.read_u8(0xC00000), 0xC0);
}

#[test]
fn exhirom_6mb_mirrors_within_extended_segment() {
    let mut rom = vec![0xFF; 0x600000];
    rom[0x000000] = 0xC0;
    rom[0x008000] = 0x80;
    rom[0x400000] = 0x40;
    rom[0x408000] = 0x00;

    let mut bus = Bus::new_with_mapper(rom, MapperType::ExHiRom, 0);

    assert_eq!(bus.read_u8(0x600000), 0x40);
    assert_eq!(bus.read_u8(0x208000), 0x00);
}

#[test]
fn exhirom_sram_is_visible_only_in_80_bf_6000_7fff() {
    let rom = vec![0xFF; 0x600000];
    let mut bus = Bus::new_with_mapper(rom, MapperType::ExHiRom, 0x2000);

    bus.write_u8(0x806000, 0x5A);
    assert_eq!(bus.read_u8(0x806000), 0x5A);

    bus.write_u8(0x206000, 0x7B);
    assert_eq!(bus.read_u8(0x206000), 0xFF);
    assert_eq!(bus.read_u8(0x806000), 0x5A);

    bus.write_u8(0xC06000, 0x11);
    assert_eq!(bus.read_u8(0x806000), 0x5A);
}
