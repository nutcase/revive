use super::*;

pub(in crate::cartridge::tests) trait IntoMapperId {
    fn into_mapper_id(self) -> u16;
}

impl IntoMapperId for u8 {
    fn into_mapper_id(self) -> u16 {
        u16::from(self)
    }
}

impl IntoMapperId for u16 {
    fn into_mapper_id(self) -> u16 {
        self
    }
}

impl IntoMapperId for i32 {
    fn into_mapper_id(self) -> u16 {
        self as u16
    }
}

pub(in crate::cartridge::tests) fn base_cartridge(
    mapper: impl IntoMapperId,
    prg_rom: Vec<u8>,
    chr_rom: Vec<u8>,
    chr_ram: Vec<u8>,
    prg_ram: Vec<u8>,
    mirroring: Mirroring,
) -> Cartridge {
    let mapper = mapper.into_mapper_id();
    let chr_rom_size = chr_rom.len();

    Cartridge {
        prg_rom,
        chr_rom,
        chr_ram,
        prg_ram,
        has_valid_save_data: false,
        mapper,
        mirroring,
        has_battery: false,
        chr_bank: 0,
        chr_bank_1: 1,
        prg_bank: 0,
        mappers: MapperRuntime {
            simple: SimpleMapperState::new(mapper, false, true, false),
            multicart: MulticartMapperState::new(mapper, false),
            mmc3_variant: Mmc3VariantState::new(mapper, chr_rom_size),
            mmc1: None,
            mmc2: None,
            mmc3: None,
            mmc5: None,
            namco163: None,
            namco210: None,
            jaleco_ss88006: None,
            vrc2_vrc4: None,
            mapper40: None,
            mapper42: None,
            mapper43: None,
            mapper50: None,
            fme7: None,
            bandai_fcg: None,
            irem_g101: None,
            irem_h3001: None,
            vrc1: None,
            vrc3: None,
            vrc6: None,
            vrc7: None,
            mapper15: None,
            sunsoft3: None,
            sunsoft4: None,
            taito_tc0190: None,
            taito_x1005: None,
            taito_x1017: None,
            mapper246: None,
        },
    }
}

pub(in crate::cartridge::tests) fn filled_banks<F>(
    bank_count: usize,
    bank_size: usize,
    mut fill_value: F,
) -> Vec<u8>
where
    F: FnMut(usize) -> u8,
{
    let mut data = vec![0; bank_count * bank_size];
    for bank in 0..bank_count {
        data[bank * bank_size..(bank + 1) * bank_size].fill(fill_value(bank));
    }
    data
}

pub(in crate::cartridge::tests) fn filled_linear_banks(
    bank_count: usize,
    bank_size: usize,
) -> Vec<u8> {
    filled_banks(bank_count, bank_size, |bank| bank as u8)
}

pub(in crate::cartridge::tests) fn filled_or_banks(
    bank_count: usize,
    bank_size: usize,
    prefix: u8,
) -> Vec<u8> {
    filled_banks(bank_count, bank_size, |bank| prefix | bank as u8)
}

pub(in crate::cartridge::tests) fn filled_wrapping_banks(
    bank_count: usize,
    bank_size: usize,
    base: u8,
) -> Vec<u8> {
    filled_banks(bank_count, bank_size, |bank| base.wrapping_add(bank as u8))
}
