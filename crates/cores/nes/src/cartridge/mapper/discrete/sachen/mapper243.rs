use crate::cartridge::{Cartridge, Mirroring};

impl Cartridge {
    pub(in crate::cartridge) fn update_mapper243_state(&mut self) {
        let prg_bank_count = (self.prg_rom.len() / 0x8000).max(1);
        let chr_bank_count = (self.chr_rom.len() / 0x2000).max(1);

        self.prg_bank = (((self.mappers.multicart.mapper243_registers[5] as usize) & 0x03)
            % prg_bank_count) as u8;
        self.chr_bank = ((((self.mappers.multicart.mapper243_registers[6] & 0x03) << 2)
            | ((self.mappers.multicart.mapper243_registers[4] & 0x01) << 1)
            | (self.mappers.multicart.mapper243_registers[2] & 0x01))
            as usize
            % chr_bank_count) as u8;
        self.mirroring = match (self.mappers.multicart.mapper243_registers[7] >> 1) & 0x03 {
            0 => Mirroring::ThreeScreenLower,
            1 => Mirroring::Vertical,
            2 => Mirroring::Horizontal,
            _ => Mirroring::OneScreenUpper,
        };
    }

    pub(in crate::cartridge) fn write_prg_mapper243(&mut self, addr: u16, data: u8) {
        match addr & 0xC101 {
            0x4100 => {
                self.mappers.multicart.mapper243_index = data & 0x07;
            }
            0x4101 => {
                let reg = self.mappers.multicart.mapper243_index as usize & 0x07;
                self.mappers.multicart.mapper243_registers[reg] = data & 0x07;
                self.update_mapper243_state();
            }
            _ => {}
        }
    }

    pub(in crate::cartridge) fn read_prg_low_mapper243(&self, addr: u16) -> u8 {
        if (addr & 0xC101) == 0x4101 {
            self.mappers.multicart.mapper243_registers
                [self.mappers.multicart.mapper243_index as usize & 0x07]
                & 0x07
        } else {
            0
        }
    }
}
