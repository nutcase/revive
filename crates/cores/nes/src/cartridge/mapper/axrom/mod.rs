use super::super::{Cartridge, Mirroring};

impl Cartridge {
    /// AxROM PRG read - 32KB switchable bank at $8000-$FFFF
    pub(in crate::cartridge) fn read_prg_axrom(&self, addr: u16) -> u8 {
        let bank = self.prg_bank as usize;
        let offset = bank * 0x8000 + (addr - 0x8000) as usize;
        if offset < self.prg_rom.len() {
            self.prg_rom[offset]
        } else {
            self.prg_rom[offset % self.prg_rom.len()]
        }
    }

    /// AxROM PRG write - bits 0-2: 32KB PRG bank, bit 4: nametable select
    pub(in crate::cartridge) fn write_prg_axrom(&mut self, addr: u16, data: u8) {
        if addr >= 0x8000 {
            self.prg_bank = data & 0x07;
            self.mirroring = if data & 0x10 != 0 {
                Mirroring::OneScreenUpper
            } else {
                Mirroring::OneScreenLower
            };
        }
    }

    /// Mapper 77: switchable 32KB PRG, one 2KB CHR-ROM page at $0000-$07FF,
    /// and 6KB of CHR-RAM across $0800-$1FFF. Bus conflicts apply to writes.
    pub(in crate::cartridge) fn write_prg_mapper77(&mut self, addr: u16, data: u8) {
        if addr < 0x8000 {
            return;
        }

        let effective = data & self.read_prg_axrom(addr);
        let prg_bank_count = (self.prg_rom.len() / 0x8000).max(1);
        let chr_bank_count = (self.chr_rom.len() / 0x0800).max(1);

        self.prg_bank = ((effective as usize & 0x0F) % prg_bank_count) as u8;
        self.chr_bank = (((effective as usize >> 4) & 0x0F) % chr_bank_count) as u8;
    }

    pub(in crate::cartridge) fn read_chr_mapper77(&self, addr: u16) -> u8 {
        if addr < 0x0800 {
            if self.chr_rom.is_empty() {
                return 0;
            }

            let bank_count = (self.chr_rom.len() / 0x0800).max(1);
            let bank = (self.chr_bank as usize) % bank_count;
            let chr_addr = bank * 0x0800 + addr as usize;
            return self.chr_rom[chr_addr % self.chr_rom.len()];
        }

        let chr_addr = addr as usize - 0x0800;
        self.chr_ram.get(chr_addr).copied().unwrap_or(0)
    }

    pub(in crate::cartridge) fn write_chr_mapper77(&mut self, addr: u16, data: u8) {
        if addr < 0x0800 {
            return;
        }

        let chr_addr = addr as usize - 0x0800;
        if let Some(slot) = self.chr_ram.get_mut(chr_addr) {
            *slot = data;
        }
    }
}
