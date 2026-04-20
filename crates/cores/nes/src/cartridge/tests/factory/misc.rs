use super::*;

pub(in crate::cartridge::tests) fn make_mapper32_cart() -> Cartridge {
    let mut prg_rom = vec![0; 8 * 0x2000];
    for bank in 0..8 {
        prg_rom[bank * 0x2000..(bank + 1) * 0x2000].fill(bank as u8);
    }

    let mut chr_rom = vec![0; 16 * 0x0400];
    for bank in 0..16 {
        chr_rom[bank * 0x0400..(bank + 1) * 0x0400].fill(0x30 | bank as u8);
    }

    let mut cart = base_cartridge(32, prg_rom, chr_rom, vec![], vec![], Mirroring::Vertical);
    cart.mappers.irem_g101 = Some(IremG101::new());
    cart
}

pub(in crate::cartridge::tests) fn make_mapper103_cart() -> Cartridge {
    let mut prg_rom = vec![0; 0x20000];
    for bank in 0..12 {
        prg_rom[bank * 0x2000..(bank + 1) * 0x2000].fill(bank as u8);
    }
    prg_rom[0x18000..0x1B800].fill(0xA1);
    prg_rom[0x1B800..0x1D800].fill(0xB2);
    prg_rom[0x1D800..0x20000].fill(0xC3);

    base_cartridge(
        103,
        prg_rom,
        vec![0; 0x2000],
        vec![],
        vec![0; 0x2000],
        Mirroring::Vertical,
    )
}

pub(in crate::cartridge::tests) fn make_mapper153_cart() -> Cartridge {
    let mut prg_rom = vec![0; 32 * 0x4000];
    for bank in 0..32 {
        prg_rom[bank * 0x4000..(bank + 1) * 0x4000].fill(bank as u8);
    }

    let mut cart = base_cartridge(
        153,
        prg_rom,
        vec![],
        vec![0; 0x2000],
        vec![0; 0x8000],
        Mirroring::Vertical,
    );
    cart.has_battery = true;
    cart.mappers.bandai_fcg = Some(BandaiFcg::new());
    if let Some(ref mut bandai) = cart.mappers.bandai_fcg {
        bandai.configure_mapper(153, true);
    }
    cart
}

pub(in crate::cartridge::tests) fn make_mapper185_cart() -> Cartridge {
    let mut chr_rom = vec![0; 4 * 0x2000];
    for bank in 0..4 {
        chr_rom[bank * 0x2000..(bank + 1) * 0x2000].fill(0x60 | bank as u8);
    }

    let cart = base_cartridge(
        185,
        vec![0xFF; 0x8000],
        chr_rom,
        vec![],
        vec![],
        Mirroring::Vertical,
    );
    cart.mappers.simple.mapper185_disabled_reads.set(2);
    cart
}
