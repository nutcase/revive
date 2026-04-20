use super::super::{Cartridge, Mirroring};

impl Cartridge {
    /// Mapper 78 (Irem 74HC161/32): 16KB switchable PRG at $8000, fixed
    /// last bank at $C000, and an 8KB CHR bank. Historical iNES ROMs use
    /// header flag 6 bit 3 to distinguish mirroring behavior between the
    /// Holy Diver and Cosmo Carrier board variants.
    pub(in crate::cartridge) fn write_prg_mapper78(&mut self, addr: u16, data: u8) {
        if addr >= 0x8000 {
            let rom_offset = if addr < 0xC000 {
                (self.prg_bank as usize) * 0x4000 + ((addr - 0x8000) as usize)
            } else {
                self.prg_rom.len().saturating_sub(0x4000) + ((addr - 0xC000) as usize)
            };

            let rom_value = if rom_offset < self.prg_rom.len() {
                self.prg_rom[rom_offset]
            } else {
                0xFF
            };

            let effective = data & rom_value;
            let prg_bank_count = (self.prg_rom.len() / 0x4000).max(1);
            let chr_bank_count = (self.chr_rom.len() / 0x2000).max(1);

            self.prg_bank = ((effective & 0x07) as usize % prg_bank_count) as u8;
            self.chr_bank = (((effective >> 4) & 0x0F) as usize % chr_bank_count) as u8;

            self.mirroring = if self.mappers.simple.mapper78_hv_mirroring {
                if effective & 0x08 != 0 {
                    Mirroring::Vertical
                } else {
                    Mirroring::Horizontal
                }
            } else if effective & 0x08 != 0 {
                Mirroring::OneScreenUpper
            } else {
                Mirroring::OneScreenLower
            };
        }
    }
}
