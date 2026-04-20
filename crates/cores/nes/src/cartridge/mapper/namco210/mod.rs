use super::super::{Cartridge, Mirroring};

#[derive(Debug, Clone)]
pub(in crate::cartridge) struct Namco210 {
    pub(in crate::cartridge) chr_banks: [u8; 8],
    pub(in crate::cartridge) prg_banks: [u8; 3],
    pub(in crate::cartridge) namco340: bool,
    pub(in crate::cartridge) prg_ram_enabled: bool,
}

impl Namco210 {
    pub(in crate::cartridge) fn new(namco340: bool) -> Self {
        Self {
            chr_banks: [0; 8],
            prg_banks: [0, 1, 2],
            namco340,
            prg_ram_enabled: false,
        }
    }
}

impl Cartridge {
    fn namco210_prg_bank_count_8k(&self) -> usize {
        (self.prg_rom.len() / 0x2000).max(1)
    }

    fn namco210_chr_bank_count_1k(&self) -> usize {
        if self.chr_rom.is_empty() {
            (self.chr_ram.len() / 0x0400).max(1)
        } else {
            (self.chr_rom.len() / 0x0400).max(1)
        }
    }

    pub(in crate::cartridge) fn read_prg_mapper210(&self, addr: u16) -> u8 {
        let Some(mapper210) = self.mappers.namco210.as_ref() else {
            return 0;
        };
        let bank_count = self.namco210_prg_bank_count_8k();
        let last_bank = bank_count.saturating_sub(1);
        let bank = match addr {
            0x8000..=0x9FFF => mapper210.prg_banks[0] as usize % bank_count,
            0xA000..=0xBFFF => mapper210.prg_banks[1] as usize % bank_count,
            0xC000..=0xDFFF => mapper210.prg_banks[2] as usize % bank_count,
            0xE000..=0xFFFF => last_bank,
            _ => return 0,
        };
        let prg_addr = bank * 0x2000 + ((addr as usize) & 0x1FFF);
        self.prg_rom.get(prg_addr).copied().unwrap_or(0)
    }

    pub(in crate::cartridge) fn write_prg_mapper210(&mut self, addr: u16, data: u8) {
        let Some(mapper210) = self.mappers.namco210.as_mut() else {
            return;
        };

        match addr & 0xF800 {
            0x8000 => mapper210.chr_banks[0] = data,
            0x8800 => mapper210.chr_banks[1] = data,
            0x9000 => mapper210.chr_banks[2] = data,
            0x9800 => mapper210.chr_banks[3] = data,
            0xA000 => mapper210.chr_banks[4] = data,
            0xA800 => mapper210.chr_banks[5] = data,
            0xB000 => mapper210.chr_banks[6] = data,
            0xB800 => mapper210.chr_banks[7] = data,
            0xC000 => {
                if !mapper210.namco340 {
                    mapper210.prg_ram_enabled = data & 0x01 != 0;
                }
            }
            0xE000 => {
                mapper210.prg_banks[0] = data & 0x3F;
                if mapper210.namco340 {
                    self.mirroring = match (data >> 6) & 0x03 {
                        0 => Mirroring::OneScreenLower,
                        1 => Mirroring::Vertical,
                        2 => Mirroring::OneScreenUpper,
                        _ => Mirroring::Horizontal,
                    };
                }
            }
            0xE800 => mapper210.prg_banks[1] = data & 0x3F,
            0xF000 => mapper210.prg_banks[2] = data & 0x3F,
            _ => {}
        }
    }

    pub(in crate::cartridge) fn read_chr_mapper210(&self, addr: u16) -> u8 {
        let Some(mapper210) = self.mappers.namco210.as_ref() else {
            return 0;
        };

        let slot = ((addr as usize) >> 10) & 0x07;
        let bank = mapper210.chr_banks[slot] as usize;
        let offset = (addr as usize) & 0x03FF;
        let bank_count = self.namco210_chr_bank_count_1k();
        let chr_addr = (bank % bank_count) * 0x0400 + offset;
        if self.chr_rom.is_empty() {
            self.chr_ram.get(chr_addr).copied().unwrap_or(0)
        } else {
            self.chr_rom.get(chr_addr).copied().unwrap_or(0)
        }
    }

    pub(in crate::cartridge) fn write_chr_mapper210(&mut self, addr: u16, data: u8) {
        let Some(mapper210) = self.mappers.namco210.as_ref() else {
            return;
        };
        if self.chr_ram.is_empty() {
            return;
        }

        let slot = ((addr as usize) >> 10) & 0x07;
        let bank = mapper210.chr_banks[slot] as usize;
        let offset = (addr as usize) & 0x03FF;
        let bank_count = self.namco210_chr_bank_count_1k();
        let chr_addr = (bank % bank_count) * 0x0400 + offset;
        if let Some(cell) = self.chr_ram.get_mut(chr_addr) {
            *cell = data;
        }
    }

    pub(in crate::cartridge) fn read_prg_ram_mapper210(&self, addr: u16) -> u8 {
        let Some(mapper210) = self.mappers.namco210.as_ref() else {
            return 0;
        };
        if mapper210.namco340 || !mapper210.prg_ram_enabled || self.prg_ram.is_empty() {
            return 0;
        }

        let offset = (addr as usize - 0x6000) % self.prg_ram.len();
        self.prg_ram[offset]
    }

    pub(in crate::cartridge) fn write_prg_ram_mapper210(&mut self, addr: u16, data: u8) {
        let Some(mapper210) = self.mappers.namco210.as_ref() else {
            return;
        };
        if mapper210.namco340 || !mapper210.prg_ram_enabled || self.prg_ram.is_empty() {
            return;
        }

        let offset = (addr as usize - 0x6000) % self.prg_ram.len();
        self.prg_ram[offset] = data;
        if self.has_battery {
            self.has_valid_save_data = true;
        }
    }
}
