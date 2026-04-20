use super::*;

pub(in crate::cartridge::tests) fn make_mapper225_cart(mapper: u8) -> Cartridge {
    let prg_rom = filled_linear_banks(128, 0x4000);
    let chr_rom = filled_wrapping_banks(128, 0x2000, 0x80);

    base_cartridge(
        mapper,
        prg_rom,
        chr_rom,
        vec![],
        if mapper == 225 { vec![0; 4] } else { vec![] },
        Mirroring::Vertical,
    )
}

pub(in crate::cartridge::tests) fn make_mapper228_cart() -> Cartridge {
    let prg_rom = filled_linear_banks(3 * 0x80000 / 0x4000, 0x4000);
    let chr_rom = filled_wrapping_banks(64, 0x2000, 0x20);

    base_cartridge(228, prg_rom, chr_rom, vec![], vec![], Mirroring::Vertical)
}

pub(in crate::cartridge::tests) fn make_mapper242_cart() -> Cartridge {
    let prg_rom = filled_linear_banks(32, 0x4000);

    base_cartridge(
        242,
        prg_rom,
        vec![0; 0x2000],
        vec![],
        vec![],
        Mirroring::Vertical,
    )
}

pub(in crate::cartridge::tests) fn make_mapper235_cart() -> Cartridge {
    let prg_rom = filled_linear_banks(128, 0x4000);

    base_cartridge(
        235,
        prg_rom,
        vec![0; 0x2000],
        vec![],
        vec![],
        Mirroring::Horizontal,
    )
}

pub(in crate::cartridge::tests) fn make_mapper227_cart() -> Cartridge {
    let prg_rom = filled_linear_banks(64, 0x4000);

    base_cartridge(
        227,
        prg_rom,
        vec![0; 0x2000],
        vec![],
        vec![],
        Mirroring::Vertical,
    )
}

pub(in crate::cartridge::tests) fn make_mapper246_cart() -> Cartridge {
    let prg_rom = filled_linear_banks(64, 0x2000);
    let chr_rom = filled_or_banks(64, 0x0800, 0x40);

    let mut cart = base_cartridge(
        246,
        prg_rom,
        chr_rom,
        vec![],
        vec![0; 0x800],
        Mirroring::Vertical,
    );
    cart.mappers.mapper246 = Some(Mapper246::new());
    cart
}

pub(in crate::cartridge::tests) fn make_mapper236_cart(chr_ram_variant: bool) -> Cartridge {
    let prg_bank_count = if chr_ram_variant { 64 } else { 16 };
    let prg_rom = filled_linear_banks(prg_bank_count, 0x4000);

    let chr_rom = if chr_ram_variant {
        vec![]
    } else {
        filled_or_banks(16, 0x2000, 0x80)
    };

    let mut cart = base_cartridge(
        236,
        prg_rom,
        chr_rom,
        if chr_ram_variant {
            vec![0; 0x2000]
        } else {
            vec![]
        },
        vec![],
        Mirroring::Vertical,
    );
    cart.mappers.multicart.mapper236_chr_ram = chr_ram_variant;
    cart
}

pub(in crate::cartridge::tests) fn make_mapper231_cart() -> Cartridge {
    let prg_rom = filled_linear_banks(32, 0x4000);

    base_cartridge(
        231,
        prg_rom,
        vec![],
        vec![0; 0x2000],
        vec![],
        Mirroring::Vertical,
    )
}

pub(in crate::cartridge::tests) fn make_mapper230_cart() -> Cartridge {
    let prg_rom = filled_linear_banks(40, 0x4000);

    let mut cart = base_cartridge(
        230,
        prg_rom,
        vec![0; 0x2000],
        vec![],
        vec![],
        Mirroring::Vertical,
    );
    cart.mappers.multicart.mapper230_contra_mode = true;
    cart
}

pub(in crate::cartridge::tests) fn make_mapper221_cart() -> Cartridge {
    let prg_rom = filled_linear_banks(64, 0x4000);

    base_cartridge(
        221,
        prg_rom,
        vec![],
        vec![0; 0x2000],
        vec![],
        Mirroring::Vertical,
    )
}

pub(in crate::cartridge::tests) fn make_mapper59_cart() -> Cartridge {
    let prg_rom = filled_linear_banks(8, 0x4000);
    let chr_rom = filled_or_banks(8, 0x2000, 0x40);

    base_cartridge(59, prg_rom, chr_rom, vec![], vec![], Mirroring::Vertical)
}

pub(in crate::cartridge::tests) fn make_mapper60_cart() -> Cartridge {
    let prg_rom = filled_linear_banks(4, 0x4000);
    let chr_rom = filled_or_banks(4, 0x2000, 0x50);

    base_cartridge(60, prg_rom, chr_rom, vec![], vec![], Mirroring::Horizontal)
}

pub(in crate::cartridge::tests) fn make_mapper61_cart() -> Cartridge {
    let prg_rom = filled_linear_banks(32, 0x4000);
    let chr_rom = filled_or_banks(16, 0x2000, 0x40);

    base_cartridge(61, prg_rom, chr_rom, vec![], vec![], Mirroring::Vertical)
}
