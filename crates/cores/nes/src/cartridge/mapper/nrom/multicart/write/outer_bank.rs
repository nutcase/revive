use crate::cartridge::{Cartridge, Mirroring};

impl Cartridge {
    pub(in crate::cartridge) fn write_prg_mapper233(&mut self, addr: u16, data: u8) {
        if addr < 0x8000 {
            return;
        }

        let bank = data & 0x1F;
        self.set_prg_bank(bank);
        self.set_chr_bank(bank);
        self.mappers.multicart.mapper233_nrom128 = data & 0x20 != 0;
        self.mirroring = match data >> 6 {
            0 => Mirroring::ThreeScreenLower,
            1 => Mirroring::Vertical,
            2 => Mirroring::Horizontal,
            _ => Mirroring::OneScreenUpper,
        };
    }

    pub(in crate::cartridge) fn write_prg_mapper234(&mut self, addr: u16, data: u8) {
        if addr >= 0x8000 {
            let effective = data & self.read_prg_mapper234(addr);
            self.apply_mapper234_value(addr, effective);
        }
    }

    pub(in crate::cartridge) fn write_prg_mapper235(&mut self, addr: u16, _data: u8) {
        if addr >= 0x8000 {
            let chip_bits = ((addr >> 8) & 0x03) as u8;
            let page = (addr & 0x001F) as usize;
            let chip_base = usize::from((chip_bits >> 1) & 0x01) * (0x100000 / 0x4000);
            let bank16 = chip_base + page * 2 + usize::from(addr & 0x1000 != 0);
            let bank_count = (self.prg_rom.len() / 0x4000).max(1);

            self.mappers.multicart.mapper235_nrom128 = addr & 0x0800 != 0;
            self.mirroring = if addr & 0x0400 != 0 {
                Mirroring::OneScreenLower
            } else if addr & 0x2000 != 0 {
                Mirroring::Vertical
            } else {
                Mirroring::Horizontal
            };

            self.prg_bank = if chip_bits & 0x01 != 0 || bank16 >= bank_count {
                u8::MAX
            } else {
                bank16 as u8
            };
        }
    }

    pub(in crate::cartridge) fn write_prg_mapper228(&mut self, addr: u16, data: u8) {
        if addr >= 0x8000 {
            let chr_bank_count = (self.chr_rom.len() / 0x2000).max(1);

            self.mappers.multicart.mapper228_chip_select = ((addr >> 11) & 0x03) as u8;
            self.prg_bank = ((addr >> 6) & 0x1F) as u8;
            self.mappers.multicart.mapper228_nrom128 = addr & 0x0020 != 0;
            self.chr_bank =
                ((((addr & 0x000F) << 2) | u16::from(data & 0x03)) as usize % chr_bank_count) as u8;
            self.mirroring = if addr & 0x2000 != 0 {
                Mirroring::Horizontal
            } else {
                Mirroring::Vertical
            };
        }
    }

    pub(in crate::cartridge) fn write_prg_mapper229(&mut self, addr: u16) {
        if addr >= 0x8000 {
            let bank = (addr & 0x001F) as usize;
            let prg_bank_count = (self.prg_rom.len() / 0x4000).max(1);
            let chr_bank_count = (self.chr_rom.len() / 0x2000).max(1);

            self.prg_bank = (bank % prg_bank_count) as u8;
            self.chr_bank = (bank % chr_bank_count) as u8;
            self.mirroring = if addr & 0x0020 != 0 {
                Mirroring::Horizontal
            } else {
                Mirroring::Vertical
            };
        }
    }

    pub(in crate::cartridge) fn write_prg_mapper221(&mut self, addr: u16) {
        if (0x8000..=0xBFFF).contains(&addr) {
            self.mappers.multicart.mapper221_outer_bank =
                (((addr >> 2) & 0x07) | (((addr >> 9) & 0x01) << 3)) as u8;
            self.mappers.multicart.mapper221_mode = if addr & 0x0002 == 0 {
                0
            } else if addr & 0x0100 == 0 {
                1
            } else {
                2
            };
            self.mirroring = if addr & 0x0001 != 0 {
                Mirroring::Horizontal
            } else {
                Mirroring::Vertical
            };
        } else if addr >= 0xC000 {
            self.prg_bank = (addr & 0x0007) as u8;
            self.mappers.multicart.mapper221_chr_write_protect = addr & 0x0008 != 0;
        }
    }

    pub(in crate::cartridge) fn write_prg_mapper231(&mut self, addr: u16) {
        if addr >= 0x8000 {
            self.prg_bank = ((addr & 0x001E) | ((addr >> 5) & 0x01)) as u8;
            self.mirroring = if addr & 0x0080 != 0 {
                Mirroring::Horizontal
            } else {
                Mirroring::Vertical
            };
        }
    }

    pub(in crate::cartridge) fn write_prg_mapper226(&mut self, addr: u16, data: u8) {
        if addr >= 0x8000 {
            let bank_count_16k = (self.prg_rom.len() / 0x4000).max(1);
            if addr & 0x0001 == 0 {
                let low_bits = (data & 0x1F) as usize | (((data >> 7) as usize) << 5);
                let high_bit = self.prg_bank as usize & 0x40;
                self.prg_bank = ((high_bit | low_bits) % bank_count_16k) as u8;
                self.mappers.multicart.mapper226_nrom128 = data & 0x20 != 0;
                self.mirroring = if data & 0x40 != 0 {
                    Mirroring::Vertical
                } else {
                    Mirroring::Horizontal
                };
            } else {
                let low_bits = self.prg_bank as usize & 0x3F;
                let high_bit = ((data & 0x01) as usize) << 6;
                self.prg_bank = ((high_bit | low_bits) % bank_count_16k) as u8;
            }
        }
    }

    pub(in crate::cartridge) fn write_prg_mapper230(&mut self, addr: u16, data: u8) {
        if addr < 0x8000 {
            return;
        }

        if self.mappers.multicart.mapper230_contra_mode {
            self.prg_bank = data & 0x07;
            self.mirroring = Mirroring::Vertical;
            return;
        }

        self.prg_bank = data & 0x1F;
        self.mappers.multicart.mapper230_nrom128 = data & 0x20 != 0;
        self.mirroring = if data & 0x40 != 0 {
            Mirroring::Vertical
        } else {
            Mirroring::Horizontal
        };
    }
}
