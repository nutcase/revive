use crate::cartridge::{Cartridge, Mirroring};

impl Cartridge {
    pub(in crate::cartridge) fn mapper137_chr_bank_1k(&self, slot: usize) -> usize {
        match slot & 3 {
            0 => (self.mappers.simple.mapper137_registers[0] & 0x07) as usize,
            1 => {
                (((self.mappers.simple.mapper137_registers[4] & 0x01) << 4)
                    | (self.mappers.simple.mapper137_registers[1] & 0x07)) as usize
            }
            2 => {
                ((((self.mappers.simple.mapper137_registers[4] >> 1) & 0x01) << 4)
                    | (self.mappers.simple.mapper137_registers[2] & 0x07)) as usize
            }
            _ => {
                ((((self.mappers.simple.mapper137_registers[4] >> 2) & 0x01) << 4)
                    | ((self.mappers.simple.mapper137_registers[6] & 0x01) << 3)
                    | (self.mappers.simple.mapper137_registers[3] & 0x07)) as usize
            }
        }
    }

    pub(in crate::cartridge) fn update_mapper137_state(&mut self) {
        let prg_bank_count = (self.prg_rom.len() / 0x8000).max(1);
        self.prg_bank =
            ((self.mappers.simple.mapper137_registers[5] as usize & 0x07) % prg_bank_count) as u8;
        self.chr_bank = self.mapper137_chr_bank_1k(0) as u8;
        self.mirroring = match (self.mappers.simple.mapper137_registers[7] >> 1) & 0x03 {
            1 => Mirroring::Horizontal,
            2 => Mirroring::Vertical,
            3 => Mirroring::OneScreenUpper,
            _ => Mirroring::Vertical,
        };
    }

    pub(in crate::cartridge) fn write_prg_mapper137(&mut self, addr: u16, data: u8) {
        match addr & 0x4101 {
            0x4100 => {
                self.mappers.simple.mapper137_index = data & 0x07;
            }
            0x4101 => {
                let reg = self.mappers.simple.mapper137_index as usize & 0x07;
                self.mappers.simple.mapper137_registers[reg] = data & 0x07;
                self.update_mapper137_state();
            }
            _ => {}
        }
    }

    pub(in crate::cartridge) fn read_prg_low_mapper137(&self, addr: u16) -> u8 {
        if (addr & 0x4101) == 0x4101 {
            self.mappers.simple.mapper137_registers
                [self.mappers.simple.mapper137_index as usize & 0x07]
                & 0x07
        } else {
            0
        }
    }

    pub(in crate::cartridge) fn read_chr_mapper137(&self, addr: u16) -> u8 {
        if self.chr_rom.is_empty() {
            return 0;
        }

        let chr_addr = if addr < 0x1000 {
            let slot = (addr as usize) / 0x0400;
            let bank_count = (self.chr_rom.len() / 0x0400).max(1);
            let bank = self.mapper137_chr_bank_1k(slot) % bank_count;
            bank * 0x0400 + (addr as usize & 0x03FF)
        } else {
            self.chr_rom.len().saturating_sub(0x1000) + (addr as usize & 0x0FFF)
        };
        self.chr_rom[chr_addr % self.chr_rom.len()]
    }

    pub(in crate::cartridge) fn write_chr_mapper137(&mut self, addr: u16, data: u8) {
        if self.chr_rom.is_empty() {
            return;
        }

        let chr_addr = if addr < 0x1000 {
            let slot = (addr as usize) / 0x0400;
            let bank_count = (self.chr_rom.len() / 0x0400).max(1);
            let bank = self.mapper137_chr_bank_1k(slot) % bank_count;
            bank * 0x0400 + (addr as usize & 0x03FF)
        } else {
            self.chr_rom.len().saturating_sub(0x1000) + (addr as usize & 0x0FFF)
        };
        let chr_addr = chr_addr % self.chr_rom.len();
        self.chr_rom[chr_addr] = data;
    }
}
