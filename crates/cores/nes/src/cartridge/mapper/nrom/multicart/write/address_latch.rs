use crate::cartridge::{Cartridge, Mirroring};

impl Cartridge {
    pub(in crate::cartridge) fn write_prg_mapper200(&mut self, addr: u16) {
        if addr >= 0x8000 {
            let bank = (addr & 0x000F) as usize;
            let prg_bank_count = (self.prg_rom.len() / 0x4000).max(1);
            let chr_bank_count = (self.chr_rom.len() / 0x2000).max(1);

            self.prg_bank = (bank % prg_bank_count) as u8;
            self.chr_bank = (bank % chr_bank_count) as u8;
            self.mirroring = if addr & 0x0008 != 0 {
                Mirroring::Horizontal
            } else {
                Mirroring::Vertical
            };
        }
    }

    pub(in crate::cartridge) fn write_prg_mapper201(&mut self, addr: u16) {
        if addr >= 0x8000 {
            let bank = (addr & 0x00FF) as usize;
            let prg_bank_count = (self.prg_rom.len() / 0x8000).max(1);
            let chr_bank_count = (self.chr_rom.len() / 0x2000).max(1);

            self.prg_bank = (bank % prg_bank_count) as u8;
            self.chr_bank = (bank % chr_bank_count) as u8;
        }
    }

    pub(in crate::cartridge) fn write_prg_mapper203(&mut self, addr: u16, data: u8) {
        if addr >= 0x8000 {
            let prg_bank_count = (self.prg_rom.len() / 0x4000).max(1);
            let chr_bank_count = (self.chr_rom.len() / 0x2000).max(1);
            self.prg_bank = (((data >> 2) as usize) % prg_bank_count) as u8;
            self.chr_bank = ((data as usize & 0x03) % chr_bank_count) as u8;
        }
    }

    pub(in crate::cartridge) fn write_prg_mapper227(&mut self, addr: u16) {
        if addr >= 0x8000 {
            self.mappers.multicart.mapper227_latch = addr & 0x07FF;
            self.mirroring = if addr & 0x0002 != 0 {
                Mirroring::Horizontal
            } else {
                Mirroring::Vertical
            };
            self.prg_bank = (self.mapper227_outer_bank() * 8 + self.mapper227_inner_bank()) as u8;
        }
    }

    pub(in crate::cartridge) fn write_chr_mapper227(&mut self, addr: u16, data: u8) {
        let write_protected =
            !self.has_battery && self.mappers.multicart.mapper227_latch & 0x0080 != 0;
        if !write_protected {
            self.write_chr_uxrom(addr, data);
        }
    }

    pub(in crate::cartridge) fn write_prg_mapper202(&mut self, addr: u16) {
        if addr >= 0x8000 {
            let bank = ((addr >> 1) & 0x07) as usize;
            let prg_bank_count_16k = (self.prg_rom.len() / 0x4000).max(1);
            let chr_bank_count = (self.chr_rom.len() / 0x2000).max(1);

            self.prg_bank = (bank % prg_bank_count_16k) as u8;
            self.chr_bank = (bank % chr_bank_count) as u8;
            self.mirroring = if addr & 0x0001 != 0 {
                Mirroring::Horizontal
            } else {
                Mirroring::Vertical
            };
            self.mappers.multicart.mapper202_32k_mode = (addr & 0x0009) == 0x0009;
        }
    }

    pub(in crate::cartridge) fn write_prg_mapper212(&mut self, addr: u16) {
        if addr >= 0x8000 {
            let bank = (addr & 0x0007) as usize;
            let chr_bank_count = (self.chr_rom.len() / 0x2000).max(1);
            let prg_bank_count = (self.prg_rom.len() / 0x4000).max(1);

            self.prg_bank = (bank % prg_bank_count) as u8;
            self.chr_bank = (bank % chr_bank_count) as u8;
            self.mirroring = if addr & 0x0008 != 0 {
                Mirroring::Horizontal
            } else {
                Mirroring::Vertical
            };
            self.mappers.multicart.mapper212_32k_mode = addr & 0x4000 != 0;
        }
    }

    pub(in crate::cartridge) fn write_prg_mapper242(&mut self, addr: u16) {
        if addr >= 0x8000 {
            self.mappers.multicart.mapper242_latch = addr & 0x07FF;
            self.mirroring = if addr & 0x0002 != 0 {
                Mirroring::Horizontal
            } else {
                Mirroring::Vertical
            };
        }
    }

    pub(in crate::cartridge) fn write_chr_mapper242(&mut self, addr: u16, data: u8) {
        if self.mappers.multicart.mapper242_latch & 0x0080 == 0 {
            self.write_chr_nrom(addr, data);
        }
    }
}
