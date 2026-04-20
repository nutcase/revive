use super::super::{Cartridge, Mirroring};

impl Cartridge {
    pub(in crate::cartridge) fn write_prg_mapper57(&mut self, addr: u16, data: u8) {
        if addr < 0x8000 {
            return;
        }

        let prg_bank_count = (self.prg_rom.len() / 0x4000).max(1);
        let chr_bank_count = (self.chr_rom.len() / 0x2000).max(1);
        let address_latch_high = addr & 0x0800 != 0;

        let prg_bank = if address_latch_high {
            (((data >> 5) & 0x01) | ((data >> 2) & 0x02)) as usize
        } else {
            ((data >> 5) & 0x07) as usize
        };
        let chr_bank = if address_latch_high {
            ((data & 0x03) | ((data >> 1) & 0x04)) as usize
        } else {
            ((data & 0x07) | ((data >> 3) & 0x08)) as usize
        };

        self.prg_bank = (prg_bank % prg_bank_count) as u8;
        self.chr_bank = (chr_bank % chr_bank_count) as u8;
        self.mirroring = if data & 0x08 != 0 {
            Mirroring::Horizontal
        } else {
            Mirroring::Vertical
        };
    }
}
