use super::super::{Cartridge, Mirroring};

impl Cartridge {
    fn bus_conflict_value_jaleco(&self, addr: u16) -> u8 {
        if self.prg_rom.is_empty() {
            return 0xFF;
        }

        let offset = if addr < 0xC000 {
            (self.prg_bank as usize) * 0x4000 + (addr - 0x8000) as usize
        } else {
            self.prg_rom.len().saturating_sub(0x4000) + (addr - 0xC000) as usize
        };
        self.prg_rom[offset % self.prg_rom.len()]
    }

    /// Mapper 70: 16KB switchable PRG at $8000, fixed last bank at $C000,
    /// plus an 8KB CHR bank. Mirroring comes from the ROM header.
    pub(in crate::cartridge) fn write_prg_mapper70(&mut self, addr: u16, data: u8) {
        if addr >= 0x8000 {
            let effective = data & self.bus_conflict_value_jaleco(addr);
            let prg_bank_count = (self.prg_rom.len() / 0x4000).max(1);
            let chr_bank_count = (self.chr_rom.len() / 0x2000).max(1);
            self.prg_bank = ((effective >> 4) as usize % prg_bank_count) as u8;
            self.chr_bank = ((effective & 0x0F) as usize % chr_bank_count) as u8;
        }
    }

    /// Mapper 152: Mapper 70 variant with one-screen mirroring control.
    pub(in crate::cartridge) fn write_prg_mapper152(&mut self, addr: u16, data: u8) {
        if addr >= 0x8000 {
            let effective = data & self.bus_conflict_value_jaleco(addr);
            let prg_bank_count = (self.prg_rom.len() / 0x4000).max(1);
            let chr_bank_count = (self.chr_rom.len() / 0x2000).max(1);
            self.prg_bank = (((effective >> 4) & 0x07) as usize % prg_bank_count) as u8;
            self.chr_bank = ((effective & 0x0F) as usize % chr_bank_count) as u8;
            self.mirroring = if effective & 0x80 != 0 {
                Mirroring::OneScreenUpper
            } else {
                Mirroring::OneScreenLower
            };
        }
    }

    /// Mappers 72/92: edge-triggered PRG/CHR latches after bus conflicts.
    pub(in crate::cartridge) fn write_prg_mapper72_92(&mut self, addr: u16, data: u8) {
        if addr < 0x8000 {
            return;
        }

        let conflict = if self.mapper == 92 {
            if self.prg_rom.is_empty() {
                0xFF
            } else if addr < 0xC000 {
                self.prg_rom[(addr - 0x8000) as usize % self.prg_rom.len()]
            } else {
                let bank_count = (self.prg_rom.len() / 0x4000).max(1);
                let bank = (self.prg_bank as usize) % bank_count;
                let offset = bank * 0x4000 + (addr - 0xC000) as usize;
                self.prg_rom[offset % self.prg_rom.len()]
            }
        } else {
            self.bus_conflict_value_jaleco(addr)
        };
        let effective = data & conflict;
        let previous = self.chr_bank_1;
        let prg_edge = (previous & 0x80 == 0) && (effective & 0x80 != 0);
        let chr_edge = (previous & 0x40 == 0) && (effective & 0x40 != 0);

        if prg_edge {
            let prg_bank_count = (self.prg_rom.len() / 0x4000).max(1);
            let prg_mask = if self.mapper == 92 { 0x0F } else { 0x07 };
            self.prg_bank = ((effective & prg_mask) as usize % prg_bank_count) as u8;
        }

        if chr_edge {
            let chr_bank_count = (self.chr_rom.len() / 0x2000).max(1);
            self.chr_bank = ((effective & 0x0F) as usize % chr_bank_count) as u8;
        }

        self.chr_bank_1 = effective & 0xC0;
    }
}
