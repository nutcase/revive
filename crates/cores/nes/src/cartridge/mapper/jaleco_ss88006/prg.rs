use crate::cartridge::{Cartridge, Mirroring};

impl Cartridge {
    fn jaleco_ss88006_prg_bank_count_8k(&self) -> usize {
        (self.prg_rom.len() / 0x2000).max(1)
    }

    fn jaleco_ss88006_write_prg_bank(&mut self, index: usize, high: bool, data: u8) {
        let Some(mapper18) = self.mappers.jaleco_ss88006.as_mut() else {
            return;
        };
        if index >= mapper18.prg_banks.len() {
            return;
        }
        let bank = &mut mapper18.prg_banks[index];
        if high {
            *bank = (*bank & 0x0F) | ((data & 0x03) << 4);
        } else {
            *bank = (*bank & 0x30) | (data & 0x0F);
        }
    }

    pub(in crate::cartridge) fn read_prg_mapper18(&self, addr: u16) -> u8 {
        let Some(mapper18) = self.mappers.jaleco_ss88006.as_ref() else {
            return 0;
        };
        let bank_count = self.jaleco_ss88006_prg_bank_count_8k();
        let last_bank = bank_count.saturating_sub(1);
        let bank = match addr {
            0x8000..=0x9FFF => mapper18.prg_banks[0] as usize % bank_count,
            0xA000..=0xBFFF => mapper18.prg_banks[1] as usize % bank_count,
            0xC000..=0xDFFF => mapper18.prg_banks[2] as usize % bank_count,
            0xE000..=0xFFFF => last_bank,
            _ => return 0,
        };
        let prg_addr = bank * 0x2000 + ((addr as usize) & 0x1FFF);
        self.prg_rom.get(prg_addr).copied().unwrap_or(0)
    }

    pub(in crate::cartridge) fn write_prg_mapper18(&mut self, addr: u16, data: u8) {
        match addr & 0xF003 {
            0x8000 => self.jaleco_ss88006_write_prg_bank(0, false, data),
            0x8001 => self.jaleco_ss88006_write_prg_bank(0, true, data),
            0x8002 => self.jaleco_ss88006_write_prg_bank(1, false, data),
            0x8003 => self.jaleco_ss88006_write_prg_bank(1, true, data),
            0x9000 => self.jaleco_ss88006_write_prg_bank(2, false, data),
            0x9001 => self.jaleco_ss88006_write_prg_bank(2, true, data),
            0x9002 => {
                if let Some(mapper18) = self.mappers.jaleco_ss88006.as_mut() {
                    mapper18.prg_ram_enabled = data & 0x01 != 0;
                    mapper18.prg_ram_write_enabled = data & 0x02 != 0;
                }
            }
            0xA000 => self.jaleco_ss88006_write_chr_bank(0, false, data),
            0xA001 => self.jaleco_ss88006_write_chr_bank(0, true, data),
            0xA002 => self.jaleco_ss88006_write_chr_bank(1, false, data),
            0xA003 => self.jaleco_ss88006_write_chr_bank(1, true, data),
            0xB000 => self.jaleco_ss88006_write_chr_bank(2, false, data),
            0xB001 => self.jaleco_ss88006_write_chr_bank(2, true, data),
            0xB002 => self.jaleco_ss88006_write_chr_bank(3, false, data),
            0xB003 => self.jaleco_ss88006_write_chr_bank(3, true, data),
            0xC000 => self.jaleco_ss88006_write_chr_bank(4, false, data),
            0xC001 => self.jaleco_ss88006_write_chr_bank(4, true, data),
            0xC002 => self.jaleco_ss88006_write_chr_bank(5, false, data),
            0xC003 => self.jaleco_ss88006_write_chr_bank(5, true, data),
            0xD000 => self.jaleco_ss88006_write_chr_bank(6, false, data),
            0xD001 => self.jaleco_ss88006_write_chr_bank(6, true, data),
            0xD002 => self.jaleco_ss88006_write_chr_bank(7, false, data),
            0xD003 => self.jaleco_ss88006_write_chr_bank(7, true, data),
            0xE000 => self.jaleco_ss88006_write_irq_reload_nibble(0, data),
            0xE001 => self.jaleco_ss88006_write_irq_reload_nibble(1, data),
            0xE002 => self.jaleco_ss88006_write_irq_reload_nibble(2, data),
            0xE003 => self.jaleco_ss88006_write_irq_reload_nibble(3, data),
            0xF000 => {
                if let Some(mapper18) = self.mappers.jaleco_ss88006.as_mut() {
                    mapper18.irq_counter = mapper18.irq_reload;
                    mapper18.irq_pending.set(false);
                }
            }
            0xF001 => {
                if let Some(mapper18) = self.mappers.jaleco_ss88006.as_mut() {
                    mapper18.irq_control = data & 0x0F;
                    mapper18.irq_pending.set(false);
                }
            }
            0xF002 => {
                self.mirroring = match data & 0x03 {
                    0 => Mirroring::Horizontal,
                    1 => Mirroring::Vertical,
                    2 => Mirroring::OneScreenLower,
                    _ => Mirroring::OneScreenUpper,
                };
            }
            0xF003 => {}
            _ => {}
        }
    }

    pub(in crate::cartridge) fn read_prg_ram_mapper18(&self, addr: u16) -> u8 {
        let Some(mapper18) = self.mappers.jaleco_ss88006.as_ref() else {
            return 0;
        };
        if !mapper18.prg_ram_enabled {
            return 0;
        }
        let offset = (addr - 0x6000) as usize;
        self.prg_ram.get(offset).copied().unwrap_or(0)
    }

    pub(in crate::cartridge) fn write_prg_ram_mapper18(&mut self, addr: u16, data: u8) {
        let Some(mapper18) = self.mappers.jaleco_ss88006.as_ref() else {
            return;
        };
        if !mapper18.prg_ram_enabled || !mapper18.prg_ram_write_enabled {
            return;
        }
        let offset = (addr - 0x6000) as usize;
        if let Some(slot) = self.prg_ram.get_mut(offset) {
            *slot = data;
            if self.has_battery {
                self.has_valid_save_data = true;
            }
        }
    }
}
