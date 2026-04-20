use crate::cartridge::Cartridge;

impl Cartridge {
    pub(in crate::cartridge) fn read_prg_mapper142(&self, addr: u16) -> u8 {
        if self.prg_rom.is_empty() || addr < 0x8000 {
            return 0;
        }

        let bank_count = (self.prg_rom.len() / 0x2000).max(1);
        let bank = match addr {
            0x8000..=0x9FFF => self.mappers.simple.mapper142_prg_banks[0] as usize,
            0xA000..=0xBFFF => self.mappers.simple.mapper142_prg_banks[1] as usize,
            0xC000..=0xDFFF => self.mappers.simple.mapper142_prg_banks[2] as usize,
            _ => bank_count.saturating_sub(1),
        } % bank_count;
        let offset = bank * 0x2000 + ((addr - 0x8000) as usize & 0x1FFF);
        self.prg_rom[offset % self.prg_rom.len()]
    }

    pub(in crate::cartridge) fn write_prg_vrc3(&mut self, addr: u16, data: u8) {
        match addr & 0xF000 {
            0x8000 => {
                if let Some(vrc3) = self.mappers.vrc3.as_mut() {
                    vrc3.write_latch_nibble(0, data);
                }
            }
            0x9000 => {
                if let Some(vrc3) = self.mappers.vrc3.as_mut() {
                    vrc3.write_latch_nibble(1, data);
                }
            }
            0xA000 => {
                if let Some(vrc3) = self.mappers.vrc3.as_mut() {
                    vrc3.write_latch_nibble(2, data);
                }
            }
            0xB000 => {
                if let Some(vrc3) = self.mappers.vrc3.as_mut() {
                    vrc3.write_latch_nibble(3, data);
                }
            }
            0xC000 => {
                if let Some(vrc3) = self.mappers.vrc3.as_mut() {
                    vrc3.write_control(data);
                }
            }
            0xD000 => {
                if let Some(vrc3) = self.mappers.vrc3.as_mut() {
                    vrc3.acknowledge();
                }
            }
            0xF000 => {
                let bank_count = (self.prg_rom.len() / 0x4000).max(1);
                self.prg_bank = (((data as usize) & 0x07) % bank_count) as u8;
            }
            _ => {}
        }
    }

    pub(in crate::cartridge) fn write_prg_mapper142(&mut self, addr: u16, data: u8) {
        match addr & 0xF000 {
            0x8000 => {
                if let Some(vrc3) = self.mappers.vrc3.as_mut() {
                    vrc3.write_latch_nibble(0, data);
                }
            }
            0x9000 => {
                if let Some(vrc3) = self.mappers.vrc3.as_mut() {
                    vrc3.write_latch_nibble(1, data);
                }
            }
            0xA000 => {
                if let Some(vrc3) = self.mappers.vrc3.as_mut() {
                    vrc3.write_latch_nibble(2, data);
                }
            }
            0xB000 => {
                if let Some(vrc3) = self.mappers.vrc3.as_mut() {
                    vrc3.write_latch_nibble(3, data);
                }
            }
            0xC000 => {
                if let Some(vrc3) = self.mappers.vrc3.as_mut() {
                    vrc3.write_control(data);
                }
            }
            0xD000 => {
                if let Some(vrc3) = self.mappers.vrc3.as_mut() {
                    vrc3.acknowledge();
                }
            }
            0xE000 => {
                self.mappers.simple.mapper142_bank_select = data & 0x07;
            }
            0xF000 => {
                if let Some(slot) = self.mappers.simple.mapper142_bank_select.checked_sub(1) {
                    if let Some(bank) = self
                        .mappers
                        .simple
                        .mapper142_prg_banks
                        .get_mut(slot as usize)
                    {
                        *bank = data & 0x0F;
                        if slot == 0 {
                            self.prg_bank = *bank;
                        }
                    }
                }
            }
            _ => {}
        }
    }
}
