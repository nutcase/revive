use super::super::{Cartridge, Mirroring};

impl Cartridge {
    /// Mapper 236 (Realtec): lower writes latch either CHR bank or outer PRG
    /// bank plus mirroring, and upper writes latch the PRG mode and inner PRG
    /// bank. Mode 1 depends on an unencoded solder pad, so it is approximated
    /// as mode 0.
    pub(in crate::cartridge) fn read_prg_mapper236(&self, addr: u16) -> u8 {
        if self.prg_rom.is_empty() {
            return 0;
        }

        let upper_half = usize::from(addr >= 0xC000);
        let bank_count = (self.prg_rom.len() / 0x4000).max(1);
        let bank = if self.mappers.multicart.mapper236_chr_ram {
            let outer_base = (self.mappers.multicart.mapper236_outer_bank as usize) * 8;
            let inner_bank = (self.prg_bank as usize) & 0x07;

            match self.mappers.multicart.mapper236_mode & 0x03 {
                0 | 1 => {
                    if upper_half != 0 {
                        outer_base + 7
                    } else {
                        outer_base + inner_bank
                    }
                }
                2 => outer_base + (inner_bank & !1) + upper_half,
                _ => outer_base + inner_bank,
            }
        } else {
            let inner_bank = (self.prg_bank as usize) & 0x0F;

            match self.mappers.multicart.mapper236_mode & 0x03 {
                0 | 1 => {
                    if upper_half != 0 {
                        (inner_bank & 0x08) | 0x07
                    } else {
                        inner_bank
                    }
                }
                2 => (inner_bank & !1) + upper_half,
                _ => inner_bank,
            }
        } % bank_count;

        let offset = bank * 0x4000 + ((addr - 0x8000) as usize & 0x3FFF);
        self.prg_rom[offset % self.prg_rom.len()]
    }

    pub(in crate::cartridge) fn write_prg_mapper236(&mut self, addr: u16, _data: u8) {
        match addr & 0xC000 {
            0x8000 => {
                self.mirroring = if addr & 0x0010 != 0 {
                    Mirroring::Horizontal
                } else {
                    Mirroring::Vertical
                };

                if self.mappers.multicart.mapper236_chr_ram {
                    self.mappers.multicart.mapper236_outer_bank = (addr & 0x0007) as u8;
                } else {
                    self.chr_bank = (addr & 0x000F) as u8;
                }
            }
            0xC000 => {
                self.mappers.multicart.mapper236_mode = ((addr >> 4) & 0x03) as u8;
                self.prg_bank = if self.mappers.multicart.mapper236_chr_ram {
                    (addr & 0x0007) as u8
                } else {
                    (addr & 0x000F) as u8
                };
            }
            _ => {}
        }
    }

    pub(in crate::cartridge) fn read_chr_mapper236(&self, addr: u16) -> u8 {
        if self.mappers.multicart.mapper236_chr_ram {
            let chr_addr = (addr & 0x1FFF) as usize;
            if chr_addr < self.chr_ram.len() {
                self.chr_ram[chr_addr]
            } else {
                0
            }
        } else if self.chr_rom.is_empty() {
            0
        } else {
            let bank_count = (self.chr_rom.len() / 0x2000).max(1);
            let bank = (self.chr_bank as usize) % bank_count;
            let chr_addr = bank * 0x2000 + ((addr as usize) & 0x1FFF);
            self.chr_rom[chr_addr % self.chr_rom.len()]
        }
    }

    pub(in crate::cartridge) fn write_chr_mapper236(&mut self, addr: u16, data: u8) {
        if self.mappers.multicart.mapper236_chr_ram {
            let chr_addr = (addr & 0x1FFF) as usize;
            if chr_addr < self.chr_ram.len() {
                self.chr_ram[chr_addr] = data;
            }
        }
    }
}
