use super::*;

impl Cartridge {
    pub(in crate::cartridge) fn read_prg_namco163(&self, addr: u16) -> u8 {
        let Some(namco163) = self.mappers.namco163.as_ref() else {
            return 0;
        };
        let bank_count = self.namco163_prg_rom_bank_count_8k();
        let last_bank = bank_count - 1;

        let (bank, base_addr) = match addr {
            0x8000..=0x9FFF => (namco163.prg_banks[0] as usize, 0x8000),
            0xA000..=0xBFFF => (namco163.prg_banks[1] as usize, 0xA000),
            0xC000..=0xDFFF => (namco163.prg_banks[2] as usize, 0xC000),
            0xE000..=0xFFFF => (last_bank, 0xE000),
            _ => return 0,
        };

        let prg_addr = (bank % bank_count) * 0x2000 + (addr - base_addr) as usize;
        self.prg_rom.get(prg_addr).copied().unwrap_or(0)
    }

    pub(in crate::cartridge) fn write_prg_namco163(&mut self, addr: u16, data: u8) {
        let Some(namco163) = self.mappers.namco163.as_mut() else {
            return;
        };

        match addr {
            0x8000..=0xDFFF => {
                let index = ((addr - 0x8000) / 0x0800) as usize;
                namco163.chr_banks[index] = data;
            }
            0xE000..=0xE7FF => {
                namco163.sound_disable = data & 0x40 != 0;
                namco163.prg_banks[0] = data & 0x3F;
            }
            0xE800..=0xEFFF => {
                namco163.chr_nt_disabled_low = data & 0x40 != 0;
                namco163.chr_nt_disabled_high = data & 0x80 != 0;
                namco163.prg_banks[1] = data & 0x3F;
            }
            0xF000..=0xF7FF => {
                namco163.prg_banks[2] = data & 0x3F;
            }
            0xF800..=0xFFFF => {
                namco163.wram_write_enable = (data & 0xF0) == 0x40;
                namco163.wram_write_protect = data & 0x0F;
                namco163.internal_auto_increment = data & 0x80 != 0;
                namco163.internal_addr.set(data & 0x7F);
            }
            _ => {}
        }
    }

    pub(in crate::cartridge) fn read_prg_low_namco163(&self, addr: u16) -> u8 {
        let Some(namco163) = self.mappers.namco163.as_ref() else {
            return 0;
        };

        match addr {
            0x4800..=0x4FFF => {
                let chip_addr = namco163.chip_ram_addr();
                let value = self.prg_ram.get(chip_addr).copied().unwrap_or(0);
                if namco163.internal_auto_increment {
                    namco163
                        .internal_addr
                        .set(namco163.internal_addr.get().wrapping_add(1) & 0x7F);
                }
                value
            }
            0x5000..=0x57FF => namco163.irq_counter as u8,
            0x5800..=0x5FFF => {
                ((namco163.irq_enabled as u8) << 7) | ((namco163.irq_counter >> 8) as u8 & 0x7F)
            }
            _ => 0,
        }
    }

    pub(in crate::cartridge) fn write_prg_low_namco163(&mut self, addr: u16, data: u8) {
        let Some(namco163) = self.mappers.namco163.as_mut() else {
            return;
        };

        match addr {
            0x4800..=0x4FFF => {
                let chip_addr = namco163.chip_ram_addr();
                if let Some(cell) = self.prg_ram.get_mut(chip_addr) {
                    *cell = data;
                    if self.has_battery {
                        self.has_valid_save_data = true;
                    }
                }
                if namco163.internal_auto_increment {
                    namco163
                        .internal_addr
                        .set(namco163.internal_addr.get().wrapping_add(1) & 0x7F);
                }
            }
            0x5000..=0x57FF => {
                namco163.irq_counter = (namco163.irq_counter & 0x7F00) | data as u16;
                namco163.irq_pending.set(false);
            }
            0x5800..=0x5FFF => {
                namco163.irq_enabled = data & 0x80 != 0;
                namco163.irq_counter =
                    (namco163.irq_counter & 0x00FF) | (((data & 0x7F) as u16) << 8);
                namco163.irq_pending.set(false);
            }
            _ => {}
        }
    }
}
