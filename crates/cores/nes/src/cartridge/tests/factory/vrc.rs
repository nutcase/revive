use super::*;

pub(in crate::cartridge::tests) fn make_mapper22_cart() -> Cartridge {
    let mut prg_rom = vec![0; 32 * 0x2000];
    for bank in 0..32 {
        prg_rom[bank * 0x2000..(bank + 1) * 0x2000].fill(bank as u8);
    }

    let mut chr_rom = vec![0; 128 * 0x0400];
    for bank in 0..128 {
        chr_rom[bank * 0x0400..(bank + 1) * 0x0400].fill(0x80 | bank as u8);
    }

    let mut cart = base_cartridge(22, prg_rom, chr_rom, vec![], vec![], Mirroring::Vertical);
    cart.mappers.vrc2_vrc4 = Some(Vrc2Vrc4::new());
    cart
}

pub(in crate::cartridge::tests) fn make_mapper21_cart() -> Cartridge {
    let mut prg_rom = vec![0; 64 * 0x2000];
    for bank in 0..64 {
        prg_rom[bank * 0x2000..(bank + 1) * 0x2000].fill(bank as u8);
    }

    let mut chr_rom = vec![0; 256 * 0x0400];
    for bank in 0..256 {
        chr_rom[bank * 0x0400..(bank + 1) * 0x0400].fill(0x80 | bank as u8);
    }

    let mut cart = base_cartridge(
        21,
        prg_rom,
        chr_rom,
        vec![],
        vec![0; 0x2000],
        Mirroring::Vertical,
    );
    let mut vrc = Vrc2Vrc4::new();
    vrc.vrc4_mode = true;
    cart.mappers.vrc2_vrc4 = Some(vrc);
    cart
}

pub(in crate::cartridge::tests) fn make_mapper23_cart() -> Cartridge {
    let mut prg_rom = vec![0; 64 * 0x2000];
    for bank in 0..64 {
        prg_rom[bank * 0x2000..(bank + 1) * 0x2000].fill(bank as u8);
    }

    let mut chr_rom = vec![0; 128 * 0x0400];
    for bank in 0..128 {
        chr_rom[bank * 0x0400..(bank + 1) * 0x0400].fill(0x80 | bank as u8);
    }

    let mut cart = base_cartridge(
        23,
        prg_rom,
        chr_rom,
        vec![],
        vec![0; 0x2000],
        Mirroring::Vertical,
    );
    cart.mappers.vrc2_vrc4 = Some(Vrc2Vrc4::new());
    cart
}

pub(in crate::cartridge::tests) fn make_mapper24_26_cart(mapper: u8) -> Cartridge {
    let mut prg_rom = vec![0; 64 * 0x2000];
    for bank in 0..64 {
        prg_rom[bank * 0x2000..(bank + 1) * 0x2000].fill(bank as u8);
    }

    let mut chr_rom = vec![0; 256 * 0x0400];
    for bank in 0..256 {
        chr_rom[bank * 0x0400..(bank + 1) * 0x0400].fill(0x80 | bank as u8);
    }

    let mut cart = base_cartridge(
        mapper,
        prg_rom,
        chr_rom,
        vec![],
        vec![0; 0x2000],
        Mirroring::Vertical,
    );
    cart.mappers.vrc6 = Some(Vrc6::new());
    cart
}

pub(in crate::cartridge::tests) fn make_mapper24_chr_ram_cart() -> Cartridge {
    let mut prg_rom = vec![0; 64 * 0x2000];
    for bank in 0..64 {
        prg_rom[bank * 0x2000..(bank + 1) * 0x2000].fill(bank as u8);
    }

    let mut cart = base_cartridge(
        24,
        prg_rom,
        vec![],
        vec![0; 0x2000],
        vec![0; 0x2000],
        Mirroring::Vertical,
    );
    cart.mappers.vrc6 = Some(Vrc6::new());
    cart
}

pub(in crate::cartridge::tests) fn make_mapper25_cart(has_battery: bool) -> Cartridge {
    let mut prg_rom = vec![0; 64 * 0x2000];
    for bank in 0..64 {
        prg_rom[bank * 0x2000..(bank + 1) * 0x2000].fill(bank as u8);
    }

    let mut chr_rom = vec![0; 128 * 0x0400];
    for bank in 0..128 {
        chr_rom[bank * 0x0400..(bank + 1) * 0x0400].fill(0x80 | bank as u8);
    }

    let mut cart = base_cartridge(
        25,
        prg_rom,
        chr_rom,
        vec![],
        vec![0; 0x2000],
        Mirroring::Vertical,
    );
    cart.has_battery = has_battery;
    cart.mappers.vrc2_vrc4 = Some(Vrc2Vrc4::new());
    cart
}

pub(in crate::cartridge::tests) fn make_vrc3_cart() -> Cartridge {
    let mut prg_rom = vec![0; 8 * 0x4000];
    for bank in 0..8 {
        prg_rom[bank * 0x4000..(bank + 1) * 0x4000].fill(bank as u8);
    }

    let mut cart = base_cartridge(
        73,
        prg_rom,
        vec![0; 0x2000],
        vec![],
        vec![0; 0x2000],
        Mirroring::Vertical,
    );
    cart.mappers.vrc3 = Some(Vrc3::new());
    cart
}
