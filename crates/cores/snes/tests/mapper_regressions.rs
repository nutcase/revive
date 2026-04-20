use std::path::Path;

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
