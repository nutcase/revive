use crate::cartridge::Mirroring;
use std::io::{Error, ErrorKind, Result};

#[derive(Debug)]
pub(super) struct CartridgeHeader {
    pub(super) prg_rom_size: usize,
    pub(super) chr_rom_size: usize,
    pub(super) prg_ram_size: usize,
    pub(super) prg_nvram_size: usize,
    pub(super) chr_ram_size: usize,
    pub(super) chr_nvram_size: usize,
    pub(super) trainer_size: usize,
    pub(super) has_battery: bool,
    pub(super) mapper: u16,
    pub(super) mapper34_nina001: bool,
    pub(super) mapper93_chr_ram_enabled: bool,
    pub(super) mapper78_hv_mirroring: bool,
    pub(super) mapper236_chr_ram: bool,
    pub(super) mirroring: Mirroring,
}

impl CartridgeHeader {
    pub(super) fn parse(data: &[u8]) -> Result<Self> {
        if data.len() < 16 || &data[0..4] != b"NES\x1a" {
            return Err(Error::new(
                ErrorKind::InvalidData,
                "Invalid NES file format",
            ));
        }

        let flags6 = data[6];
        let flags7 = data[7];
        let nes2 = (flags7 & 0x0C) == 0x08;

        let prg_rom_size = rom_size(data[4], nes2.then_some(data[9] & 0x0F), 16 * 1024);
        let chr_rom_size = rom_size(data[5], nes2.then_some(data[9] >> 4), 8 * 1024);
        let (prg_ram_size, prg_nvram_size, chr_ram_size, chr_nvram_size) = if nes2 {
            (
                ram_size_from_shift(data[10] & 0x0F),
                ram_size_from_shift(data[10] >> 4),
                ram_size_from_shift(data[11] & 0x0F),
                ram_size_from_shift(data[11] >> 4),
            )
        } else {
            (0, 0, 0, 0)
        };

        let has_battery = (flags6 & 0x02) != 0;
        let trainer_size = if flags6 & 0x04 != 0 { 512 } else { 0 };
        let mapper = mapper_number(flags6, flags7, nes2.then_some(data[8] & 0x0F))?;
        let submapper = if nes2 { data[8] >> 4 } else { 0 };
        let mapper34_nina001 = mapper == 34 && mapper34_is_nina001(submapper, chr_rom_size);
        let mapper93_chr_ram_enabled = true;
        let mapper78_hv_mirroring = mapper == 78 && ((flags6 & 0x08) != 0 || submapper == 3);
        let mapper236_chr_ram = mapper == 236 && chr_rom_size == 0;
        let mirroring = initial_mirroring(mapper, flags6, mapper78_hv_mirroring);

        Ok(Self {
            prg_rom_size,
            chr_rom_size,
            prg_ram_size,
            prg_nvram_size,
            chr_ram_size,
            chr_nvram_size,
            trainer_size,
            has_battery,
            mapper,
            mapper34_nina001,
            mapper93_chr_ram_enabled,
            mapper78_hv_mirroring,
            mapper236_chr_ram,
            mirroring,
        })
    }
}

fn mapper_number(flags6: u8, flags7: u8, nes2_upper_nibble: Option<u8>) -> Result<u16> {
    Ok(((nes2_upper_nibble.unwrap_or(0) as u16) << 8)
        | (flags7 & 0xF0) as u16
        | (flags6 >> 4) as u16)
}

fn rom_size(lsb_units: u8, nes2_msb_units: Option<u8>, unit_size: usize) -> usize {
    let units = lsb_units as usize | ((nes2_msb_units.unwrap_or(0) as usize) << 8);
    units * unit_size
}

fn ram_size_from_shift(shift: u8) -> usize {
    if shift == 0 {
        0
    } else {
        64usize << shift
    }
}

fn mapper34_is_nina001(submapper: u8, chr_rom_size: usize) -> bool {
    match submapper {
        1 => true,
        2 => false,
        _ => chr_rom_size > 8192,
    }
}

fn initial_mirroring(mapper: u16, flags6: u8, mapper78_hv_mirroring: bool) -> Mirroring {
    if matches!(mapper, 77 | 99) {
        Mirroring::FourScreen
    } else if matches!(mapper, 13 | 38 | 208 | 234) {
        Mirroring::Vertical
    } else if mapper == 235 {
        Mirroring::Horizontal
    } else if mapper == 78 {
        if mapper78_hv_mirroring {
            Mirroring::Horizontal
        } else {
            Mirroring::OneScreenLower
        }
    } else if flags6 & 0x08 != 0 {
        Mirroring::FourScreen
    } else if flags6 & 0x01 != 0 {
        Mirroring::Vertical
    } else {
        Mirroring::Horizontal
    }
}

#[cfg(test)]
mod tests {
    use super::{ram_size_from_shift, CartridgeHeader};
    use crate::cartridge::Mirroring;

    fn header() -> [u8; 16] {
        let mut data = [0; 16];
        data[0..4].copy_from_slice(b"NES\x1A");
        data[4] = 1;
        data[5] = 1;
        data
    }

    #[test]
    fn parses_ines_mapper_and_sizes() {
        let mut data = header();
        data[6] = 0x21;
        data[7] = 0x10;

        let parsed = CartridgeHeader::parse(&data).unwrap();

        assert_eq!(parsed.mapper, 0x12);
        assert_eq!(parsed.prg_rom_size, 16 * 1024);
        assert_eq!(parsed.chr_rom_size, 8 * 1024);
        assert_eq!(parsed.trainer_size, 0);
    }

    #[test]
    fn parses_ines_trainer_flag() {
        let mut data = header();
        data[6] = 0x04;

        let parsed = CartridgeHeader::parse(&data).unwrap();

        assert_eq!(parsed.mapper, 0);
        assert_eq!(parsed.trainer_size, 512);
    }

    #[test]
    fn parses_nes2_extended_sizes_and_ram_shifts() {
        let mut data = header();
        data[4] = 2;
        data[5] = 3;
        data[7] = 0x08;
        data[9] = 0x21;
        data[10] = 0x76;
        data[11] = 0x54;

        let parsed = CartridgeHeader::parse(&data).unwrap();

        assert_eq!(parsed.prg_rom_size, 0x102 * 16 * 1024);
        assert_eq!(parsed.chr_rom_size, 0x203 * 8 * 1024);
        assert_eq!(parsed.prg_ram_size, ram_size_from_shift(6));
        assert_eq!(parsed.prg_nvram_size, ram_size_from_shift(7));
        assert_eq!(parsed.chr_ram_size, ram_size_from_shift(4));
        assert_eq!(parsed.chr_nvram_size, ram_size_from_shift(5));
    }

    #[test]
    fn parses_nes2_mappers_outside_ines_range() {
        let mut data = header();
        data[7] = 0x08;
        data[8] = 0x01;

        let parsed = CartridgeHeader::parse(&data).unwrap();

        assert_eq!(parsed.mapper, 256);
    }

    #[test]
    fn mapper34_nes2_submapper_overrides_chr_size_heuristic() {
        let mut nina = header();
        nina[5] = 0;
        nina[6] = 0x20;
        nina[7] = 0x28;
        nina[8] = 0x10;

        let parsed_nina = CartridgeHeader::parse(&nina).unwrap();
        assert_eq!(parsed_nina.mapper, 34);
        assert!(parsed_nina.mapper34_nina001);

        let mut bnrom = header();
        bnrom[5] = 2;
        bnrom[6] = 0x20;
        bnrom[7] = 0x28;
        bnrom[8] = 0x20;

        let parsed_bnrom = CartridgeHeader::parse(&bnrom).unwrap();
        assert_eq!(parsed_bnrom.mapper, 34);
        assert!(!parsed_bnrom.mapper34_nina001);
    }

    #[test]
    fn mapper78_nes2_submapper_controls_mirroring_variant() {
        let mut cosmo = header();
        cosmo[6] = 0xE0;
        cosmo[7] = 0x08 | 0x40;
        cosmo[8] = 0x10;

        let parsed_cosmo = CartridgeHeader::parse(&cosmo).unwrap();
        assert_eq!(parsed_cosmo.mapper, 78);
        assert!(!parsed_cosmo.mapper78_hv_mirroring);
        assert_eq!(parsed_cosmo.mirroring, Mirroring::OneScreenLower);

        let mut holy_diver = header();
        holy_diver[6] = 0xE0;
        holy_diver[7] = 0x08 | 0x40;
        holy_diver[8] = 0x30;

        let parsed_holy_diver = CartridgeHeader::parse(&holy_diver).unwrap();
        assert_eq!(parsed_holy_diver.mapper, 78);
        assert!(parsed_holy_diver.mapper78_hv_mirroring);
        assert_eq!(parsed_holy_diver.mirroring, Mirroring::Horizontal);
    }
}
