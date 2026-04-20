use super::*;

pub(in crate::cartridge::tests) fn make_mapper18_cart() -> Cartridge {
    let mut prg_rom = vec![0; 64 * 0x2000];
    for bank in 0..64 {
        prg_rom[bank * 0x2000..(bank + 1) * 0x2000].fill(bank as u8);
    }

    let mut chr_rom = vec![0; 256 * 0x0400];
    for bank in 0..256 {
        chr_rom[bank * 0x0400..(bank + 1) * 0x0400].fill(0x80 | bank as u8);
    }

    let mut cart = base_cartridge(
        18,
        prg_rom,
        chr_rom,
        vec![],
        vec![0; 0x2000],
        Mirroring::Horizontal,
    );
    cart.mappers.jaleco_ss88006 = Some(JalecoSs88006::new());
    cart
}

pub(in crate::cartridge::tests) fn make_mapper40_cart() -> Cartridge {
    let mut prg_rom = vec![0; 8 * 0x2000];
    for bank in 0..8 {
        prg_rom[bank * 0x2000..(bank + 1) * 0x2000].fill(bank as u8);
    }

    let mut cart = base_cartridge(
        40,
        prg_rom,
        vec![0; 0x2000],
        vec![],
        vec![],
        Mirroring::Horizontal,
    );
    cart.mappers.mapper40 = Some(Mapper40::new());
    cart
}

pub(in crate::cartridge::tests) fn make_mapper42_cart() -> Cartridge {
    let mut prg_rom = vec![0; 16 * 0x2000];
    for bank in 0..16 {
        prg_rom[bank * 0x2000..(bank + 1) * 0x2000].fill(bank as u8);
    }

    let mut cart = base_cartridge(
        42,
        prg_rom,
        vec![0; 0x2000],
        vec![],
        vec![],
        Mirroring::Vertical,
    );
    cart.mappers.mapper42 = Some(Mapper42::new());
    cart
}

pub(in crate::cartridge::tests) fn make_mapper43_cart() -> Cartridge {
    let mut prg_rom = vec![0; 0x14000];
    for bank in 0..8 {
        prg_rom[bank * 0x2000..(bank + 1) * 0x2000].fill(bank as u8);
    }
    prg_rom[0x10000..0x10800].fill(0xF2);
    prg_rom[0x10800..0x12000].fill(0xF2);
    prg_rom[0x12000..0x14000].fill(0xE8);

    let mut cart = base_cartridge(
        43,
        prg_rom,
        vec![0; 0x2000],
        vec![],
        vec![],
        Mirroring::Vertical,
    );
    cart.mappers.mapper43 = Some(Mapper43::new());
    cart
}

pub(in crate::cartridge::tests) fn make_mapper50_cart() -> Cartridge {
    let mut prg_rom = vec![0; 16 * 0x2000];
    for bank in 0..16 {
        prg_rom[bank * 0x2000..(bank + 1) * 0x2000].fill(bank as u8);
    }

    let mut cart = base_cartridge(
        50,
        prg_rom,
        vec![0; 0x2000],
        vec![],
        vec![],
        Mirroring::Horizontal,
    );
    cart.mappers.mapper50 = Some(Mapper50::new());
    cart
}

pub(in crate::cartridge::tests) fn make_mapper65_cart() -> Cartridge {
    let mut prg_rom = vec![0; 16 * 0x2000];
    for bank in 0..16 {
        prg_rom[bank * 0x2000..(bank + 1) * 0x2000].fill(bank as u8);
    }

    let mut chr_rom = vec![0; 16 * 0x0400];
    for bank in 0..16 {
        chr_rom[bank * 0x0400..(bank + 1) * 0x0400].fill(0x90 | bank as u8);
    }

    let mut cart = base_cartridge(65, prg_rom, chr_rom, vec![], vec![], Mirroring::Vertical);
    cart.mappers.irem_h3001 = Some(IremH3001::new());
    cart
}

pub(in crate::cartridge::tests) fn make_mapper159_cart() -> Cartridge {
    let mut prg_rom = vec![0; 32 * 0x4000];
    for bank in 0..32 {
        prg_rom[bank * 0x4000..(bank + 1) * 0x4000].fill(bank as u8);
    }

    let mut chr_rom = vec![0; 128 * 0x0400];
    for bank in 0..128 {
        chr_rom[bank * 0x0400..(bank + 1) * 0x0400].fill(0x80 | bank as u8);
    }

    let mut cart = base_cartridge(
        159,
        prg_rom,
        chr_rom,
        vec![],
        vec![0xFF; 0x80],
        Mirroring::Vertical,
    );
    cart.has_battery = true;
    cart.mappers.bandai_fcg = Some(BandaiFcg::new());
    if let Some(ref mut bandai) = cart.mappers.bandai_fcg {
        bandai.configure_mapper(159, true);
    }
    cart
}

pub(in crate::cartridge::tests) fn make_mapper142_cart() -> Cartridge {
    let mut prg_rom = vec![0; 16 * 0x2000];
    for bank in 0..16 {
        prg_rom[bank * 0x2000..(bank + 1) * 0x2000].fill(bank as u8);
    }

    let mut cart = base_cartridge(
        142,
        prg_rom,
        vec![0; 0x2000],
        vec![],
        vec![],
        Mirroring::Horizontal,
    );
    cart.mappers.vrc3 = Some(Vrc3::new());
    cart
}

pub(in crate::cartridge::tests) fn make_mapper67_cart() -> Cartridge {
    let mut prg_rom = vec![0; 8 * 0x4000];
    for bank in 0..8 {
        prg_rom[bank * 0x4000..(bank + 1) * 0x4000].fill(bank as u8);
    }

    let mut chr_rom = vec![0; 8 * 0x0800];
    for bank in 0..8 {
        chr_rom[bank * 0x0800..(bank + 1) * 0x0800].fill(0x70 | bank as u8);
    }

    let mut cart = base_cartridge(67, prg_rom, chr_rom, vec![], vec![], Mirroring::Vertical);
    cart.mappers.sunsoft3 = Some(Sunsoft3::new());
    cart
}
