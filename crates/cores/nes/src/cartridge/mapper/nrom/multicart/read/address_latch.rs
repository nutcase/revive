use crate::cartridge::Cartridge;

impl Cartridge {
    pub(in crate::cartridge) fn read_prg_mapper227(&self, addr: u16) -> u8 {
        if self.prg_rom.is_empty() {
            return 0;
        }

        let outer_base = self.mapper227_outer_bank() * 8;
        let inner_bank = self.mapper227_inner_bank();
        let nrom_mode = self.mappers.multicart.mapper227_latch & 0x0080 != 0;
        let mode_32k = self.mappers.multicart.mapper227_latch & 0x0001 != 0;
        let fixed_bank = if self.mappers.multicart.mapper227_latch & 0x0200 != 0 {
            7
        } else {
            0
        };
        let upper_half = addr >= 0xC000;
        let bank_count = (self.prg_rom.len() / 0x4000).max(1);

        let bank = if nrom_mode {
            if mode_32k {
                outer_base + (inner_bank & !1) + usize::from(upper_half)
            } else {
                outer_base + inner_bank
            }
        } else if upper_half {
            outer_base + fixed_bank
        } else if mode_32k {
            outer_base + (inner_bank & !1)
        } else {
            outer_base + inner_bank
        };

        let offset = (bank % bank_count) * 0x4000 + ((addr - 0x8000) as usize & 0x3FFF);
        self.prg_rom[offset % self.prg_rom.len()]
    }

    pub(in crate::cartridge) fn read_prg_mapper200(&self, addr: u16) -> u8 {
        self.read_multicart_prg_16k(addr, self.prg_bank as usize, 0)
    }

    pub(in crate::cartridge) fn read_prg_mapper212(&self, addr: u16) -> u8 {
        if self.prg_rom.is_empty() {
            return 0;
        }

        if self.mappers.multicart.mapper212_32k_mode {
            self.read_multicart_prg_32k(addr, (self.prg_bank as usize) >> 1, 0)
        } else {
            self.read_multicart_prg_16k(addr, self.prg_bank as usize, 0)
        }
    }

    pub(in crate::cartridge) fn read_prg_mapper202(&self, addr: u16) -> u8 {
        if self.prg_rom.is_empty() {
            return 0;
        }

        if self.mappers.multicart.mapper202_32k_mode {
            self.read_multicart_prg_32k(addr, self.prg_bank as usize, 0)
        } else {
            self.read_multicart_prg_16k(addr, self.prg_bank as usize, 0)
        }
    }

    pub(in crate::cartridge) fn read_prg_mapper242(&self, addr: u16) -> u8 {
        if self.prg_rom.is_empty() {
            return 0;
        }

        let reg = self.mappers.multicart.mapper242_latch as usize;
        let inner_bank = (reg >> 2) & 0x07;
        let outer_bank = (reg >> 5) & 0x03;
        let fixed_bank = if reg & 0x0200 != 0 { 7 } else { 0 };
        let nrom_mode = reg & 0x0080 != 0;
        let mode_32k = reg & 0x0001 != 0;
        let upper_half = addr >= 0xC000;

        let (chip_base, bank_count, bank16) = if self.prg_rom.len() > 0x80000 && reg & 0x0400 == 0 {
            let chip_base = 0x80000;
            let bank_count = (self.prg_rom.len().saturating_sub(chip_base) / 0x4000).max(1);
            let bank16 = if nrom_mode {
                if mode_32k {
                    (inner_bank & !1) | usize::from(upper_half)
                } else {
                    inner_bank
                }
            } else if upper_half {
                fixed_bank
            } else if mode_32k {
                inner_bank & !1
            } else {
                inner_bank
            };
            (chip_base, bank_count, bank16)
        } else {
            let chip_base = 0;
            let bank_count = (self.prg_rom.len() / 0x4000).max(1);
            let bank16 = outer_bank * 8
                + if nrom_mode {
                    if mode_32k {
                        (inner_bank & !1) | usize::from(upper_half)
                    } else {
                        inner_bank
                    }
                } else if upper_half {
                    fixed_bank
                } else if mode_32k {
                    inner_bank & !1
                } else {
                    inner_bank
                };
            (chip_base, bank_count, bank16)
        };

        let offset_in_bank = (addr - 0x8000) as usize & 0x3FFF;
        let bank = bank16 % bank_count;
        let offset = chip_base + bank * 0x4000 + offset_in_bank;
        self.prg_rom[offset % self.prg_rom.len()]
    }
}
