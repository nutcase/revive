use crate::cartridge::{Cartridge, Mirroring};

impl Cartridge {
    pub(in crate::cartridge) fn write_prg_mapper225(&mut self, addr: u16, data: u8) {
        if (0x5800..=0x5FFF).contains(&addr) && self.mapper == 225 && !self.prg_ram.is_empty() {
            self.prg_ram[(addr as usize) & 0x03] = data & 0x0F;
            return;
        }

        if addr >= 0x8000 {
            let high_bit = if addr & 0x4000 != 0 { 0x40 } else { 0x00 };
            let prg_bank_count = (self.prg_rom.len() / 0x4000).max(1);
            let chr_bank_count = (self.chr_rom.len() / 0x2000).max(1);

            self.prg_bank = ((((addr as usize >> 6) & 0x3F) | high_bit) % prg_bank_count) as u8;
            self.chr_bank = ((((addr as usize) & 0x3F) | high_bit) % chr_bank_count) as u8;
            self.mappers.multicart.mapper225_nrom128 = addr & 0x1000 != 0;
            self.mirroring = if addr & 0x2000 != 0 {
                Mirroring::Horizontal
            } else {
                Mirroring::Vertical
            };
        }
    }
}
