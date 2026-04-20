use crate::cartridge::{Cartridge, Mirroring};

impl Cartridge {
    pub(in crate::cartridge) fn update_mapper150_state(&mut self) {
        let prg_bank_count = (self.prg_rom.len() / 0x8000).max(1);
        let chr_bank_count = (self.chr_rom.len() / 0x2000).max(1);

        self.prg_bank =
            ((self.mappers.simple.mapper150_registers[5] as usize & 0x03) % prg_bank_count) as u8;
        self.chr_bank = ((((self.mappers.simple.mapper150_registers[4] & 0x01) << 2)
            | (self.mappers.simple.mapper150_registers[6] & 0x03))
            as usize
            % chr_bank_count) as u8;
        self.mirroring = match (self.mappers.simple.mapper150_registers[7] >> 1) & 0x03 {
            0 => Mirroring::ThreeScreenLower,
            1 => Mirroring::Horizontal,
            2 => Mirroring::Vertical,
            _ => Mirroring::OneScreenUpper,
        };
    }

    pub(in crate::cartridge) fn write_prg_mapper150(&mut self, addr: u16, data: u8) {
        match addr & 0xC101 {
            0x4100 => {
                self.mappers.simple.mapper150_index = data & 0x07;
            }
            0x4101 => {
                let reg = self.mappers.simple.mapper150_index as usize & 0x07;
                self.mappers.simple.mapper150_registers[reg] = data & 0x07;
                self.update_mapper150_state();
            }
            _ => {}
        }
    }

    pub(in crate::cartridge) fn read_prg_low_mapper150(&self, addr: u16) -> u8 {
        if (addr & 0xC101) == 0x4101 {
            self.mappers.simple.mapper150_registers
                [self.mappers.simple.mapper150_index as usize & 0x07]
                & 0x07
        } else {
            0
        }
    }
}
