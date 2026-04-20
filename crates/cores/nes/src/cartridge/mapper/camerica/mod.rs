use super::super::{Cartridge, Mirroring};

impl Cartridge {
    /// Mapper 232 (BF9096): select a 64KB PRG block with writes in the
    /// $8000-$BFFF range, then choose one 16KB page inside that block at
    /// $C000-$FFFF. The upper 16KB is fixed to page 3 of the current block.
    pub(in crate::cartridge) fn read_prg_mapper232(&self, addr: u16) -> u8 {
        if self.prg_rom.is_empty() {
            return 0;
        }

        let bank_count = (self.prg_rom.len() / 0x4000).max(1);
        let bank = if addr < 0xC000 {
            ((self.mappers.multicart.mapper232_outer_bank as usize) << 2)
                | (self.prg_bank as usize & 0x03)
        } else {
            ((self.mappers.multicart.mapper232_outer_bank as usize) << 2) | 0x03
        };
        let offset = bank % bank_count * 0x4000 + ((addr - 0x8000) as usize & 0x3FFF);
        self.prg_rom[offset % self.prg_rom.len()]
    }

    pub(in crate::cartridge) fn write_prg_mapper232(&mut self, addr: u16, data: u8) {
        match addr {
            0x8000..=0xBFFF => {
                self.mappers.multicart.mapper232_outer_bank = (data >> 3) & 0x03;
            }
            0xC000..=0xFFFF => {
                self.prg_bank = data & 0x03;
            }
            _ => {}
        }
    }

    /// Mapper 71 (Camerica): 16KB switchable bank at $8000-$BFFF with
    /// the last 16KB fixed at $C000-$FFFF. Some boards also expose a
    /// one-screen mirroring register at $9000-$9FFF.
    pub(in crate::cartridge) fn write_prg_camerica(&mut self, addr: u16, data: u8) {
        match addr {
            0x9000..=0x9FFF => {
                self.mirroring = if data & 0x10 != 0 {
                    Mirroring::OneScreenUpper
                } else {
                    Mirroring::OneScreenLower
                };
            }
            0xC000..=0xFFFF => {
                let bank_count = (self.prg_rom.len() / 0x4000).max(1);
                self.prg_bank = (data as usize % bank_count) as u8;
            }
            _ => {}
        }
    }
}
