use crate::cartridge::{Cartridge, Mirroring};

impl Cartridge {
    pub(in crate::cartridge) fn sync_mapper41_chr_bank(&mut self) {
        let outer_bank = self.chr_bank >> 2;
        self.chr_bank = (outer_bank << 2)
            | if self.prg_bank & 0x04 != 0 {
                self.mappers.simple.mapper41_inner_bank & 0x03
            } else {
                0
            };
    }

    pub(in crate::cartridge) fn write_prg_ram_mapper41(&mut self, addr: u16) {
        if !(0x6000..=0x67FF).contains(&addr) {
            return;
        }

        let prg_bank_count = (self.prg_rom.len() / 0x8000).max(1);
        let outer_chr_bank_count = (self.chr_rom.len() / 0x2000).max(1).div_ceil(4);
        let outer_bank = (((addr >> 3) & 0x03) as usize % outer_chr_bank_count.max(1)) as u8;

        self.prg_bank = ((addr as usize & 0x07) % prg_bank_count) as u8;
        self.chr_bank = outer_bank << 2;
        self.mirroring = if addr & 0x20 != 0 {
            Mirroring::Horizontal
        } else {
            Mirroring::Vertical
        };
        self.sync_mapper41_chr_bank();
    }

    pub(in crate::cartridge) fn write_prg_mapper41(&mut self, addr: u16, data: u8) {
        if addr < 0x8000 || self.prg_bank & 0x04 == 0 {
            return;
        }

        let effective = data & self.read_prg_axrom(addr);
        self.mappers.simple.mapper41_inner_bank = effective & 0x03;
        self.sync_mapper41_chr_bank();
    }
}
