use super::*;

pub(in crate::cartridge::tests) fn make_mmc1_cart() -> Cartridge {
    let mut cart = base_cartridge(
        1,
        vec![0; 0x8000],
        vec![0; 0x2000],
        vec![0; 0x2000],
        vec![0; 0x2000],
        Mirroring::Vertical,
    );
    cart.has_valid_save_data = true;
    cart.has_battery = true;
    cart.mappers.mmc1 = Some(Mmc1::new());
    cart
}

pub(in crate::cartridge::tests) fn make_mmc5_cart() -> Cartridge {
    let prg_rom = filled_linear_banks(32, 0x2000);
    let chr_rom = filled_or_banks(128, 0x0400, 0x80);

    let mut cart = base_cartridge(
        5,
        prg_rom,
        chr_rom,
        vec![],
        vec![0; 0x20000],
        Mirroring::Horizontal,
    );
    cart.mappers.mmc5 = Some(Mmc5::new());
    cart
}

pub(in crate::cartridge::tests) fn make_mapper64_cart() -> Cartridge {
    let prg_rom = filled_linear_banks(32, 0x2000);
    let chr_rom = filled_or_banks(256, 0x0400, 0x80);

    let mut cart = base_cartridge(64, prg_rom, chr_rom, vec![], vec![], Mirroring::Vertical);
    cart.mappers.mmc3 = Some(Mmc3::new());
    cart
}

pub(in crate::cartridge::tests) fn make_mmc3_mixed_chr_cart(
    mapper: u8,
    prg_banks_8k: usize,
    chr_banks_1k: usize,
    chr_ram_size: usize,
) -> Cartridge {
    let prg_rom = filled_linear_banks(prg_banks_8k, 0x2000);
    let chr_rom = filled_wrapping_banks(chr_banks_1k, 0x0400, 0x50);

    let mut cart = base_cartridge(
        mapper,
        prg_rom,
        chr_rom,
        vec![0; chr_ram_size],
        vec![0; 0x2000],
        Mirroring::Vertical,
    );
    cart.mappers.mmc3 = Some(Mmc3::new());
    cart
}

pub(in crate::cartridge::tests) fn make_mapper245_cart() -> Cartridge {
    let prg_rom = filled_linear_banks(64, 0x2000);

    let mut cart = base_cartridge(
        245,
        prg_rom,
        vec![],
        vec![0; 0x2000],
        vec![0; 0x2000],
        Mirroring::Vertical,
    );
    cart.mappers.mmc3 = Some(Mmc3::new());
    cart
}

pub(in crate::cartridge::tests) fn make_mapper114_cart(mapper: u8) -> Cartridge {
    let prg_rom = filled_linear_banks(32, 0x2000);
    let chr_rom = filled_banks(512, 0x0400, |bank| {
        ((bank & 0xFF) as u8) ^ (((bank >> 8) as u8) << 7)
    });

    let mut cart = base_cartridge(
        mapper,
        prg_rom,
        chr_rom,
        vec![],
        vec![0; 0x2000],
        Mirroring::Vertical,
    );
    cart.mappers.mmc3 = Some(Mmc3::new());
    cart
}

pub(in crate::cartridge::tests) fn make_mapper123_cart() -> Cartridge {
    let prg_rom = filled_linear_banks(32, 0x2000);
    let chr_rom = filled_wrapping_banks(64, 0x0400, 0x60);

    let mut cart = base_cartridge(
        123,
        prg_rom,
        chr_rom,
        vec![],
        vec![0; 0x2000],
        Mirroring::Vertical,
    );
    cart.mappers.mmc3 = Some(Mmc3::new());
    cart
}

pub(in crate::cartridge::tests) fn make_mapper115_cart(mapper: u8) -> Cartridge {
    let prg_rom = filled_linear_banks(64, 0x2000);
    let chr_rom = filled_banks(512, 0x0400, |bank| {
        ((bank & 0xFF) as u8) ^ (((bank >> 8) as u8) << 7)
    });

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

pub(in crate::cartridge::tests) fn make_mapper205_cart() -> Cartridge {
    let prg_rom = filled_linear_banks(64, 0x2000);
    let chr_rom = filled_banks(512, 0x0400, |bank| {
        ((bank & 0x3F) as u8) | ((((bank >> 7) & 0x03) as u8) << 6)
    });

    let mut cart = base_cartridge(205, prg_rom, chr_rom, vec![], vec![], Mirroring::Vertical);
    cart.mappers.mmc3 = Some(Mmc3::new());
    cart
}

pub(in crate::cartridge::tests) fn make_mapper12_cart() -> Cartridge {
    let prg_rom = filled_linear_banks(32, 0x2000);
    let chr_rom = filled_banks(512, 0x0400, |bank| {
        ((bank & 0xFF) as u8) ^ (((bank >> 8) as u8) << 7)
    });

    let mut cart = base_cartridge(12, prg_rom, chr_rom, vec![], vec![], Mirroring::Vertical);
    cart.mappers.mmc3 = Some(Mmc3::new());
    cart
}

pub(in crate::cartridge::tests) fn make_mapper189_cart() -> Cartridge {
    let prg_rom = filled_linear_banks(16, 0x8000);
    let chr_rom = filled_or_banks(16, 0x0400, 0x60);

    let mut cart = base_cartridge(189, prg_rom, chr_rom, vec![], vec![], Mirroring::Vertical);
    cart.mappers.mmc3 = Some(Mmc3::new());
    cart
}

pub(in crate::cartridge::tests) fn make_mapper44_cart() -> Cartridge {
    let prg_rom = filled_linear_banks(128, 0x2000);
    let chr_rom = filled_banks(1024, 0x0400, |bank| {
        ((bank & 0x1F) as u8) | ((((bank >> 7) & 0x07) as u8) << 5)
    });

    let mut cart = base_cartridge(44, prg_rom, chr_rom, vec![], vec![], Mirroring::Vertical);
    cart.mappers.mmc3 = Some(Mmc3::new());
    cart
}

pub(in crate::cartridge::tests) fn make_mapper208_cart() -> Cartridge {
    let prg_rom = filled_linear_banks(4, 0x8000);
    let chr_rom = filled_wrapping_banks(16, 0x0400, 0x50);

    let mut cart = base_cartridge(208, prg_rom, chr_rom, vec![], vec![], Mirroring::Vertical);
    cart.prg_bank = 3;
    cart.mappers.mmc3 = Some(Mmc3::new());
    cart
}
