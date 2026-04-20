use crate::cartridge::Cartridge;

impl Cartridge {
    pub(in crate::cartridge) fn read_prg_ram_bandai(&self, addr: u16) -> u8 {
        if self.mapper == 153 {
            if let Some(ref bandai) = self.mappers.bandai_fcg {
                if !bandai.prg_ram_enabled {
                    return 0;
                }
            }
            let ram_addr = (addr - 0x6000) as usize;
            return self.prg_ram.get(ram_addr).copied().unwrap_or(0);
        }

        if let Some(ref bandai) = self.mappers.bandai_fcg {
            if self.has_battery {
                return if bandai.eeprom_data_out { 0x10 } else { 0 };
            }
        }

        let ram_addr = (addr - 0x6000) as usize;
        if ram_addr < self.prg_ram.len() {
            self.prg_ram[ram_addr]
        } else {
            0
        }
    }

    pub(in crate::cartridge) fn write_prg_ram_bandai(&mut self, addr: u16, data: u8) {
        if self.mapper == 153 {
            if let Some(ref bandai) = self.mappers.bandai_fcg {
                if !bandai.prg_ram_enabled {
                    return;
                }
            }
            let ram_addr = (addr - 0x6000) as usize;
            if ram_addr < self.prg_ram.len() {
                self.prg_ram[ram_addr] = data;
            }
            return;
        }

        if self.has_battery {
            return;
        }
        let ram_addr = (addr - 0x6000) as usize;
        if ram_addr < self.prg_ram.len() {
            self.prg_ram[ram_addr] = data;
        }
    }
}
