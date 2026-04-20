use crate::cartridge::Cartridge;

impl Cartridge {
    pub(in crate::cartridge) fn read_prg_mapper229(&self, addr: u16) -> u8 {
        if self.prg_rom.is_empty() {
            return 0;
        }

        let bank_count = (self.prg_rom.len() / 0x4000).max(1);
        let bank = (self.prg_bank as usize) % bank_count;
        let selected_bank = if bank == 0 && addr >= 0xC000 && bank_count > 1 {
            1
        } else {
            bank
        };
        self.read_multicart_prg_16k(addr, selected_bank, 0)
    }

    pub(in crate::cartridge) fn read_prg_mapper221(&self, addr: u16) -> u8 {
        if self.prg_rom.is_empty() {
            return 0;
        }

        let outer_base = (self.mappers.multicart.mapper221_outer_bank as usize) * 8;
        let inner_bank = (self.prg_bank as usize) & 0x07;
        let bank = match self.mappers.multicart.mapper221_mode {
            0 => outer_base + inner_bank,
            1 => outer_base + (inner_bank & !1) + usize::from(addr >= 0xC000),
            _ => {
                if addr >= 0xC000 {
                    outer_base + 7
                } else {
                    outer_base + inner_bank
                }
            }
        };
        self.read_multicart_prg_16k(addr, bank, 0)
    }

    pub(in crate::cartridge) fn read_prg_mapper231(&self, addr: u16) -> u8 {
        if self.prg_rom.is_empty() {
            return 0;
        }

        let bank = if addr < 0xC000 {
            (self.prg_bank as usize) & 0x1E
        } else {
            self.prg_bank as usize
        };
        self.read_multicart_prg_16k(addr, bank, 0)
    }

    pub(in crate::cartridge) fn read_prg_mapper233(&self, addr: u16) -> u8 {
        if self.prg_rom.is_empty() || addr < 0x8000 {
            return 0;
        }

        let bank = if self.mappers.multicart.mapper233_nrom128 {
            self.prg_bank as usize
        } else {
            ((self.prg_bank as usize) & !1) | usize::from(addr >= 0xC000)
        };
        self.read_multicart_prg_16k(addr, bank, 0)
    }

    pub(in crate::cartridge) fn read_prg_mapper234(&self, addr: u16) -> u8 {
        self.read_prg_axrom(addr)
    }

    pub(in crate::cartridge) fn read_prg_mapper226(&self, addr: u16) -> u8 {
        if self.prg_rom.is_empty() {
            return 0;
        }

        if self.mappers.multicart.mapper226_nrom128 {
            self.read_multicart_prg_16k(addr, self.prg_bank as usize, 0)
        } else {
            self.read_multicart_prg_32k(addr, (self.prg_bank as usize) >> 1, 0)
        }
    }

    pub(in crate::cartridge) fn read_prg_mapper230(&self, addr: u16) -> u8 {
        if self.prg_rom.is_empty() || addr < 0x8000 {
            return 0;
        }

        let offset_in_bank = (addr - 0x8000) as usize & 0x3FFF;
        if self.mappers.multicart.mapper230_contra_mode {
            let chip0_len = self.prg_rom.len().min(0x20000);
            let bank_count = (chip0_len / 0x4000).max(1);
            let bank = if addr < 0xC000 {
                (self.prg_bank as usize & 0x07) % bank_count
            } else {
                bank_count.saturating_sub(1)
            };
            return self.prg_rom[bank * 0x4000 + offset_in_bank];
        }

        let chip_base = 0x20000.min(self.prg_rom.len());
        let chip1_len = self.prg_rom.len().saturating_sub(chip_base);
        if chip1_len == 0 {
            return 0;
        }

        let bank_count = (chip1_len / 0x4000).max(1);
        let page = self.prg_bank as usize & 0x1F;
        let bank = if self.mappers.multicart.mapper230_nrom128 {
            page % bank_count
        } else {
            ((page & !1) | usize::from(addr >= 0xC000)) % bank_count
        };
        let offset = chip_base + bank * 0x4000 + offset_in_bank;
        self.prg_rom[offset % self.prg_rom.len()]
    }

    pub(in crate::cartridge) fn read_prg_mapper235(&self, addr: u16) -> u8 {
        if self.prg_rom.is_empty() {
            return 0xFF;
        }

        if self.prg_bank == u8::MAX {
            return 0xFF;
        }

        if self.mappers.multicart.mapper235_nrom128 {
            self.read_multicart_prg_16k(addr, self.prg_bank as usize, 0xFF)
        } else {
            self.read_multicart_prg_32k(addr, (self.prg_bank as usize) >> 1, 0xFF)
        }
    }

    pub(in crate::cartridge) fn read_prg_mapper228(&self, addr: u16) -> u8 {
        const CHIP_SIZE: usize = 0x80000;

        let Some(chip_base) = self.mapper228_chip_base() else {
            return 0;
        };

        let remaining = self.prg_rom.len().saturating_sub(chip_base);
        let chip_len = CHIP_SIZE.min(remaining).max(0x4000);
        let bank_count = (chip_len / 0x4000).max(1);
        let offset_in_bank = (addr - 0x8000) as usize & 0x3FFF;
        let bank = if self.mappers.multicart.mapper228_nrom128 {
            (self.prg_bank as usize) % bank_count
        } else {
            (((self.prg_bank as usize) & !1) | usize::from(addr >= 0xC000)) % bank_count
        };
        let offset = chip_base + bank * 0x4000 + offset_in_bank;
        self.prg_rom.get(offset).copied().unwrap_or(0)
    }
}
