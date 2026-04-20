use super::*;

pub(in crate::cartridge::tests) fn make_simple_bank_cart(
    mapper: u8,
    prg_banks_32k: usize,
    chr_banks_8k: usize,
) -> Cartridge {
    let prg_rom = filled_linear_banks(prg_banks_32k, 0x8000);
    let chr_rom = filled_or_banks(chr_banks_8k, 0x2000, 0x40);

    base_cartridge(
        mapper,
        prg_rom,
        chr_rom,
        vec![],
        vec![],
        Mirroring::Horizontal,
    )
}

pub(in crate::cartridge::tests) fn make_camerica_cart() -> Cartridge {
    let prg_rom = filled_linear_banks(8, 0x4000);

    base_cartridge(
        71,
        prg_rom,
        vec![0; 0x2000],
        vec![],
        vec![],
        Mirroring::Horizontal,
    )
}

pub(in crate::cartridge::tests) fn make_uxrom_like_cart(
    mapper: u8,
    prg_banks_16k: usize,
    chr_banks_8k: usize,
) -> Cartridge {
    let prg_rom = filled_linear_banks(prg_banks_16k, 0x4000);
    let chr_rom = filled_or_banks(chr_banks_8k, 0x2000, 0x70);

    base_cartridge(
        mapper,
        prg_rom,
        chr_rom,
        vec![],
        vec![],
        Mirroring::Horizontal,
    )
}

pub(in crate::cartridge::tests) fn make_mapper78_cart(hv_mirroring: bool) -> Cartridge {
    let mut cart = make_uxrom_like_cart(78, 8, 16);
    cart.mappers.simple.mapper78_hv_mirroring = hv_mirroring;
    cart.mirroring = if hv_mirroring {
        Mirroring::Horizontal
    } else {
        Mirroring::OneScreenLower
    };
    cart
}

pub(in crate::cartridge::tests) fn make_mapper57_cart() -> Cartridge {
    let prg_rom = filled_linear_banks(8, 0x4000);
    let chr_rom = filled_or_banks(16, 0x2000, 0xA0);

    base_cartridge(57, prg_rom, chr_rom, vec![], vec![], Mirroring::Vertical)
}

pub(in crate::cartridge::tests) fn make_mapper63_cart() -> Cartridge {
    let prg_rom = filled_linear_banks(8, 0x4000);

    base_cartridge(
        63,
        prg_rom,
        vec![],
        vec![0; 0x2000],
        vec![],
        Mirroring::Vertical,
    )
}

pub(in crate::cartridge::tests) fn make_mapper77_cart() -> Cartridge {
    let prg_rom = filled_linear_banks(4, 0x8000);
    let chr_rom = filled_or_banks(4, 0x0800, 0x80);

    base_cartridge(
        77,
        prg_rom,
        chr_rom,
        vec![0; 0x2000],
        vec![],
        Mirroring::FourScreen,
    )
}

pub(in crate::cartridge::tests) fn make_mapper99_cart() -> Cartridge {
    let prg_rom = filled_linear_banks(5, 0x2000);
    let chr_rom = filled_or_banks(2, 0x2000, 0x90);

    base_cartridge(
        99,
        prg_rom,
        chr_rom,
        vec![0; 0x1000],
        vec![0; 0x0800],
        Mirroring::FourScreen,
    )
}

pub(in crate::cartridge::tests) fn make_mapper137_cart() -> Cartridge {
    let prg_rom = filled_linear_banks(8, 0x8000);
    let chr_rom = filled_or_banks(32, 0x0400, 0xA0);

    base_cartridge(137, prg_rom, chr_rom, vec![], vec![], Mirroring::Vertical)
}

pub(in crate::cartridge::tests) fn make_mapper150_cart() -> Cartridge {
    let prg_rom = filled_linear_banks(4, 0x8000);
    let chr_rom = filled_or_banks(8, 0x2000, 0xB0);

    base_cartridge(150, prg_rom, chr_rom, vec![], vec![], Mirroring::Vertical)
}

pub(in crate::cartridge::tests) fn make_vrc1_cart(mapper: u8) -> Cartridge {
    let prg_rom = filled_linear_banks(8, 0x2000);
    let chr_rom = filled_or_banks(16, 0x1000, 0x60);

    let mut cart = base_cartridge(
        mapper,
        prg_rom,
        chr_rom,
        vec![],
        vec![],
        Mirroring::Vertical,
    );
    cart.mappers.vrc1 = Some(Vrc1::new());
    cart
}

pub(in crate::cartridge::tests) fn make_nina001_cart() -> Cartridge {
    let prg_rom = filled_linear_banks(4, 0x8000);
    let chr_rom = filled_or_banks(4, 0x1000, 0x50);

    let mut cart = base_cartridge(
        34,
        prg_rom,
        chr_rom,
        vec![],
        vec![0; 0x2000],
        Mirroring::Horizontal,
    );
    cart.mappers.simple.mapper34_nina001 = true;
    cart
}

pub(in crate::cartridge::tests) fn make_split_chr_cart(
    mapper: u8,
    chr_banks_4k: usize,
    upper_bank: u8,
) -> Cartridge {
    let chr_rom = filled_or_banks(chr_banks_4k, 0x1000, 0x60);

    let chr_bank = if mapper == 13 { upper_bank } else { 0 };
    let mut cart = base_cartridge(
        mapper,
        vec![0; 0x8000],
        chr_rom,
        vec![],
        vec![],
        Mirroring::Horizontal,
    );
    cart.chr_bank = chr_bank;
    cart.chr_bank_1 = upper_bank;
    cart
}

pub(in crate::cartridge::tests) fn make_mapper15_cart() -> Cartridge {
    let prg_rom = filled_linear_banks(16, 0x4000);

    let mut cart = base_cartridge(
        15,
        prg_rom,
        vec![0; 0x2000],
        vec![],
        vec![0; 0x2000],
        Mirroring::Vertical,
    );
    cart.mappers.mapper15 = Some(Mapper15::new());
    cart
}
