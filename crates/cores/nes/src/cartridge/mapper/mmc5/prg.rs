use super::super::super::Cartridge;

impl Cartridge {
    fn mmc5_prg_rom_banks_8k(&self) -> usize {
        (self.prg_rom.len() / 0x2000).max(1)
    }

    fn mmc5_prg_ram_banks_8k(&self) -> usize {
        (self.prg_ram.len() / 0x2000).max(1)
    }

    fn mmc5_prg_target(&self, raw_bank: u8, size_8k: usize, offset: usize, rom_only: bool) -> u8 {
        let offset_in_bank = offset & (size_8k * 0x2000 - 1);
        if !rom_only && raw_bank & 0x80 == 0 {
            if self.prg_ram.is_empty() {
                return 0;
            }
            let bank_count = self.mmc5_prg_ram_banks_8k();
            let bank_base = ((raw_bank as usize) & !((size_8k - 1).max(1) - 1)) % bank_count;
            let ram_addr = bank_base * 0x2000 + offset_in_bank;
            return self.prg_ram[ram_addr % self.prg_ram.len()];
        }

        let bank_count = self.mmc5_prg_rom_banks_8k();
        let bank_base = (((raw_bank & 0x7F) as usize) & !((size_8k - 1).max(1) - 1)) % bank_count;
        let rom_addr = bank_base * 0x2000 + offset_in_bank;
        self.prg_rom[rom_addr % self.prg_rom.len()]
    }

    pub(super) fn write_mmc5_prg_target(
        &mut self,
        raw_bank: u8,
        size_8k: usize,
        offset: usize,
        data: u8,
        rom_only: bool,
    ) {
        let Some(mmc5) = self.mappers.mmc5.as_ref() else {
            return;
        };
        if rom_only
            || raw_bank & 0x80 != 0
            || !mmc5.prg_ram_write_enabled()
            || self.prg_ram.is_empty()
        {
            return;
        }

        let offset_in_bank = offset & (size_8k * 0x2000 - 1);
        let bank_count = self.mmc5_prg_ram_banks_8k();
        let bank_base = ((raw_bank as usize) & !((size_8k - 1).max(1) - 1)) % bank_count;
        let ram_addr = bank_base * 0x2000 + offset_in_bank;
        if let Some(slot) = self.prg_ram.get_mut(ram_addr) {
            *slot = data;
        }
    }

    pub(in crate::cartridge) fn read_prg_mmc5(&self, addr: u16) -> u8 {
        let Some(mmc5) = self.mappers.mmc5.as_ref() else {
            return 0;
        };

        match mmc5.prg_mode & 0x03 {
            0 => self.mmc5_prg_target(mmc5.prg_banks[3], 4, (addr - 0x8000) as usize, true),
            1 => {
                if addr < 0xC000 {
                    self.mmc5_prg_target(mmc5.prg_banks[1], 2, (addr - 0x8000) as usize, false)
                } else {
                    self.mmc5_prg_target(mmc5.prg_banks[3], 2, (addr - 0xC000) as usize, true)
                }
            }
            2 => {
                if addr < 0xC000 {
                    self.mmc5_prg_target(mmc5.prg_banks[1], 2, (addr - 0x8000) as usize, false)
                } else if addr < 0xE000 {
                    self.mmc5_prg_target(mmc5.prg_banks[2], 1, (addr - 0xC000) as usize, false)
                } else {
                    self.mmc5_prg_target(mmc5.prg_banks[3], 1, (addr - 0xE000) as usize, true)
                }
            }
            _ => match addr {
                0x8000..=0x9FFF => {
                    self.mmc5_prg_target(mmc5.prg_banks[0], 1, (addr - 0x8000) as usize, false)
                }
                0xA000..=0xBFFF => {
                    self.mmc5_prg_target(mmc5.prg_banks[1], 1, (addr - 0xA000) as usize, false)
                }
                0xC000..=0xDFFF => {
                    self.mmc5_prg_target(mmc5.prg_banks[2], 1, (addr - 0xC000) as usize, false)
                }
                _ => self.mmc5_prg_target(mmc5.prg_banks[3], 1, (addr - 0xE000) as usize, true),
            },
        }
    }

    pub(in crate::cartridge) fn read_prg_ram_mmc5(&self, addr: u16) -> u8 {
        let Some(mmc5) = self.mappers.mmc5.as_ref() else {
            return 0;
        };
        if self.prg_ram.is_empty() {
            return 0;
        }
        let bank = (mmc5.prg_ram_bank as usize) % self.mmc5_prg_ram_banks_8k();
        let ram_addr = bank * 0x2000 + (addr as usize & 0x1FFF);
        self.prg_ram[ram_addr % self.prg_ram.len()]
    }

    pub(in crate::cartridge) fn write_prg_ram_mmc5(&mut self, addr: u16, data: u8) {
        let Some(mmc5) = self.mappers.mmc5.as_ref() else {
            return;
        };
        if !mmc5.prg_ram_write_enabled() || self.prg_ram.is_empty() {
            return;
        }
        let bank = (mmc5.prg_ram_bank as usize) % self.mmc5_prg_ram_banks_8k();
        let ram_addr = bank * 0x2000 + (addr as usize & 0x1FFF);
        if let Some(slot) = self.prg_ram.get_mut(ram_addr) {
            *slot = data;
        }
    }
}
