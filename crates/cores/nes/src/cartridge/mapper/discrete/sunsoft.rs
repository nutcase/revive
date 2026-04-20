use crate::cartridge::{Cartridge, Mirroring};

impl Cartridge {
    pub(in crate::cartridge) fn write_prg_mapper89(&mut self, addr: u16, data: u8) {
        if addr >= 0x8000 {
            let effective = data & self.bus_conflict_value_fixed_last_16k(addr);
            let prg_bank_count = (self.prg_rom.len() / 0x4000).max(1);
            let chr_bank_count = (self.chr_rom.len() / 0x2000).max(1);

            self.prg_bank = (((effective >> 4) & 0x07) as usize % prg_bank_count) as u8;
            self.chr_bank =
                ((((effective >> 4) & 0x08) | (effective & 0x07)) as usize % chr_bank_count) as u8;
            self.mirroring = if effective & 0x08 != 0 {
                Mirroring::OneScreenUpper
            } else {
                Mirroring::OneScreenLower
            };
        }
    }

    pub(in crate::cartridge) fn write_prg_mapper93(&mut self, addr: u16, data: u8) {
        if addr >= 0x8000 {
            let effective = data & self.bus_conflict_value_fixed_last_16k(addr);
            let prg_bank_count = (self.prg_rom.len() / 0x4000).max(1);
            self.prg_bank = (((effective >> 4) & 0x07) as usize % prg_bank_count) as u8;
            self.mappers.simple.mapper93_chr_ram_enabled = effective & 0x01 != 0;
        }
    }

    pub(in crate::cartridge) fn read_chr_mapper93(&self, addr: u16) -> u8 {
        if self.mappers.simple.mapper93_chr_ram_enabled {
            self.read_chr_uxrom(addr)
        } else {
            0xFF
        }
    }

    pub(in crate::cartridge) fn write_chr_mapper93(&mut self, addr: u16, data: u8) {
        if self.mappers.simple.mapper93_chr_ram_enabled {
            self.write_chr_uxrom(addr, data);
        }
    }
}
