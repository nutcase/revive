use super::*;

pub(in crate::cartridge::tests) fn make_mapper19_cart() -> Cartridge {
    let mut prg_rom = vec![0; 64 * 0x2000];
    for bank in 0..64 {
        prg_rom[bank * 0x2000..(bank + 1) * 0x2000].fill(bank as u8);
    }

    let mut chr_rom = vec![0; 256 * 0x0400];
    for bank in 0..256 {
        chr_rom[bank * 0x0400..(bank + 1) * 0x0400].fill(0x80 | bank as u8);
    }

    let mut cart = base_cartridge(
        19,
        prg_rom,
        chr_rom,
        vec![0; 0x0800],
        vec![0; 0x2080],
        Mirroring::Horizontal,
    );
    cart.mappers.namco163 = Some(Namco163::new());
    cart
}

pub(in crate::cartridge::tests) fn make_mapper210_cart(namco340: bool) -> Cartridge {
    let mut prg_rom = vec![0; 64 * 0x2000];
    for bank in 0..64 {
        prg_rom[bank * 0x2000..(bank + 1) * 0x2000].fill(bank as u8);
    }

    let mut chr_rom = vec![0; 256 * 0x0400];
    for bank in 0..256 {
        chr_rom[bank * 0x0400..(bank + 1) * 0x0400].fill(0x80 | bank as u8);
    }

    let mut cart = base_cartridge(
        210,
        prg_rom,
        chr_rom,
        vec![],
        if namco340 { vec![] } else { vec![0; 0x0800] },
        if namco340 {
            Mirroring::Horizontal
        } else {
            Mirroring::Vertical
        },
    );
    cart.has_battery = !namco340;
    cart.mappers.namco210 = Some(Namco210::new(namco340));
    cart
}

pub(in crate::cartridge::tests) fn make_namco108_cart(
    mapper: u8,
    prg_banks_8k: usize,
    chr_banks_1k: usize,
) -> Cartridge {
    let mut prg_rom = vec![0; prg_banks_8k * 0x2000];
    for bank in 0..prg_banks_8k {
        prg_rom[bank * 0x2000..(bank + 1) * 0x2000].fill(bank as u8);
    }

    let mut chr_rom = vec![0; chr_banks_1k * 0x0400];
    for bank in 0..chr_banks_1k {
        chr_rom[bank * 0x0400..(bank + 1) * 0x0400].fill(0x20u8.wrapping_add(bank as u8));
    }

    let mut cart = base_cartridge(
        mapper,
        prg_rom,
        chr_rom,
        vec![],
        vec![],
        Mirroring::Vertical,
    );
    cart.mappers.mmc3 = Some(Mmc3::new());
    cart
}

pub(in crate::cartridge::tests) fn make_sunsoft4_cart() -> Cartridge {
    let mut prg_rom = vec![0; 4 * 0x4000];
    for bank in 0..4 {
        prg_rom[bank * 0x4000..(bank + 1) * 0x4000].fill(bank as u8);
    }

    let mut chr_rom = vec![0; 0x40000];
    for bank in 0..(chr_rom.len() / 0x0400) {
        chr_rom[bank * 0x0400..(bank + 1) * 0x0400].fill(bank as u8);
    }

    let mut cart = base_cartridge(
        68,
        prg_rom,
        chr_rom,
        vec![],
        vec![0; 0x2000],
        Mirroring::Vertical,
    );
    cart.mappers.sunsoft4 = Some(Sunsoft4::new());
    cart
}

pub(in crate::cartridge::tests) fn make_fme7_cart() -> Cartridge {
    let mut prg_rom = vec![0; 64 * 0x2000];
    for bank in 0..64 {
        prg_rom[bank * 0x2000..(bank + 1) * 0x2000].fill(bank as u8);
    }

    let mut chr_rom = vec![0; 256 * 0x0400];
    for bank in 0..256 {
        chr_rom[bank * 0x0400..(bank + 1) * 0x0400].fill(0x80 | bank as u8);
    }

    let mut cart = base_cartridge(
        69,
        prg_rom,
        chr_rom,
        vec![],
        vec![0; 0x2000],
        Mirroring::Vertical,
    );
    cart.mappers.fme7 = Some(Fme7::new());
    cart
}

pub(in crate::cartridge::tests) fn make_taito_tc0190_cart() -> Cartridge {
    let mut prg_rom = vec![0; 16 * 0x2000];
    for bank in 0..16 {
        prg_rom[bank * 0x2000..(bank + 1) * 0x2000].fill(bank as u8);
    }

    let mut chr_rom = vec![0; 32 * 0x0400];
    for bank in 0..32 {
        chr_rom[bank * 0x0400..(bank + 1) * 0x0400].fill(0x40u8.wrapping_add(bank as u8));
    }

    let mut cart = base_cartridge(33, prg_rom, chr_rom, vec![], vec![], Mirroring::Vertical);
    cart.mappers.taito_tc0190 = Some(TaitoTc0190::new());
    cart
}

pub(in crate::cartridge::tests) fn make_mapper48_cart() -> Cartridge {
    let mut prg_rom = vec![0; 16 * 0x2000];
    for bank in 0..16 {
        prg_rom[bank * 0x2000..(bank + 1) * 0x2000].fill(bank as u8);
    }

    let mut chr_rom = vec![0; 32 * 0x0400];
    for bank in 0..32 {
        chr_rom[bank * 0x0400..(bank + 1) * 0x0400].fill(0x80u8.wrapping_add(bank as u8));
    }

    let mut cart = base_cartridge(48, prg_rom, chr_rom, vec![], vec![], Mirroring::Vertical);
    cart.mappers.taito_tc0190 = Some(TaitoTc0190::new());
    cart
}

pub(in crate::cartridge::tests) fn make_taito_x1005_cart() -> Cartridge {
    let mut prg_rom = vec![0; 16 * 0x2000];
    for bank in 0..16 {
        prg_rom[bank * 0x2000..(bank + 1) * 0x2000].fill(bank as u8);
    }

    let mut chr_rom = vec![0; 32 * 0x0400];
    for bank in 0..32 {
        chr_rom[bank * 0x0400..(bank + 1) * 0x0400].fill(0x80u8.wrapping_add(bank as u8));
    }

    let mut cart = base_cartridge(
        80,
        prg_rom,
        chr_rom,
        vec![],
        vec![0; 128],
        Mirroring::Horizontal,
    );
    cart.mappers.taito_x1005 = Some(TaitoX1005::new());
    cart
}

pub(in crate::cartridge::tests) fn make_taito_x1017_cart() -> Cartridge {
    let mut prg_rom = vec![0; 16 * 0x2000];
    for bank in 0..16 {
        prg_rom[bank * 0x2000..(bank + 1) * 0x2000].fill(bank as u8);
    }

    let mut chr_rom = vec![0; 32 * 0x0400];
    for bank in 0..32 {
        chr_rom[bank * 0x0400..(bank + 1) * 0x0400].fill(0x90 | bank as u8);
    }

    let mut cart = base_cartridge(
        82,
        prg_rom,
        chr_rom,
        vec![],
        vec![0; 0x1400],
        Mirroring::Horizontal,
    );
    cart.has_battery = true;
    cart.mappers.taito_x1017 = Some(TaitoX1017::new());
    cart
}
