use crate::cartridge::{Cartridge, Mirroring};

impl Cartridge {
    pub(in crate::cartridge) fn write_prg_mapper79_146(&mut self, addr: u16, data: u8) {
        if (addr & 0xE100) == 0x4100 {
            let prg_bank_count = (self.prg_rom.len() / 0x8000).max(1);
            let chr_bank_count = (self.chr_rom.len() / 0x2000).max(1);
            self.prg_bank = (((data >> 3) & 0x01) as usize % prg_bank_count) as u8;
            self.chr_bank = ((data & 0x07) as usize % chr_bank_count) as u8;
        }
    }

    pub(in crate::cartridge) fn write_prg_mapper133(&mut self, addr: u16, data: u8) {
        if (addr & 0xE100) == 0x4100 {
            let prg_bank_count = (self.prg_rom.len() / 0x8000).max(1);
            let chr_bank_count = (self.chr_rom.len() / 0x2000).max(1);
            self.prg_bank = (((data >> 2) & 0x01) as usize % prg_bank_count) as u8;
            self.chr_bank = ((data & 0x03) as usize % chr_bank_count) as u8;
        }
    }

    pub(in crate::cartridge) fn write_prg_mapper113(&mut self, addr: u16, data: u8) {
        if (addr & 0xE100) == 0x4100 {
            let prg_bank_count = (self.prg_rom.len() / 0x8000).max(1);
            let chr_bank_count = (self.chr_rom.len() / 0x2000).max(1);
            self.prg_bank = (((data >> 3) & 0x07) as usize % prg_bank_count) as u8;
            self.chr_bank =
                ((((data >> 3) & 0x08) | (data & 0x07)) as usize % chr_bank_count) as u8;
            self.mirroring = if data & 0x80 != 0 {
                Mirroring::Horizontal
            } else {
                Mirroring::Vertical
            };
        }
    }
}
