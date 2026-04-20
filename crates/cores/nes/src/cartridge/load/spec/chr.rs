use super::MapperSpec;

const EMPTY_CHR_ROM_MAPPERS: &[u16] = &[63, 153];
const EMPTY_IF_CHRLESS_MAPPERS: &[u16] = &[18, 221, 231];
const STANDARD_8K_CHR_RAM_MAPPERS: &[u16] = &[12, 44, 115, 119, 189, 195, 205, 248];
const CHRLESS_8K_CHR_RAM_MAPPERS: &[u16] = &[1, 4, 10, 18, 48, 65, 118, 221, 231, 236, 245];
const SMALL_MMC3_CHR_RAM_MAPPERS: &[u16] = &[74, 191, 194];

#[derive(Debug, Clone, Copy)]
pub(in crate::cartridge::load) enum ChrRomLoad {
    Standard,
    Cprom,
    Empty,
    Namco163ChrRamBacked,
    Mapper77,
}

impl MapperSpec {
    pub(in crate::cartridge::load) fn chr_rom_load(self) -> ChrRomLoad {
        match self.mapper {
            13 => ChrRomLoad::Cprom,
            19 if self.chr_rom_size == 0 => ChrRomLoad::Namco163ChrRamBacked,
            mapper if EMPTY_CHR_ROM_MAPPERS.contains(&mapper) => ChrRomLoad::Empty,
            77 => ChrRomLoad::Mapper77,
            mapper if self.chr_rom_size == 0 && EMPTY_IF_CHRLESS_MAPPERS.contains(&mapper) => {
                ChrRomLoad::Empty
            }
            _ => ChrRomLoad::Standard,
        }
    }

    pub(in crate::cartridge::load) fn chr_ram_size(self) -> usize {
        let explicit_size = self.explicit_chr_ram_size();
        if explicit_size > 0 {
            explicit_size
        } else if self.mapper == 19 {
            0x0800
        } else if (self.mapper == 210 && self.chr_rom_size == 0)
            || matches!(self.mapper, 63 | 77 | 153)
        {
            0x2000
        } else if self.mapper == 99 {
            0x1000
        } else if matches!(self.mapper, 21..=26) && self.chr_rom_size == 0 {
            0x2000
        } else if SMALL_MMC3_CHR_RAM_MAPPERS.contains(&self.mapper) {
            0x0800
        } else if self.mapper == 192 {
            0x1000
        } else if STANDARD_8K_CHR_RAM_MAPPERS.contains(&self.mapper)
            || (self.chr_rom_size == 0 && CHRLESS_8K_CHR_RAM_MAPPERS.contains(&self.mapper))
        {
            8192
        } else {
            0
        }
    }

    pub(in crate::cartridge::load) fn initial_chr_bank_1(self) -> u8 {
        if self.mapper == 184 {
            4
        } else {
            1
        }
    }
}
