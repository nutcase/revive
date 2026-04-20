use super::super::NAMCO163_WRAM_LEN;
use super::*;

impl Cartridge {
    pub(in crate::cartridge) fn read_prg_ram_namco163(&self, addr: u16) -> u8 {
        if self.prg_ram.len() < NAMCO163_WRAM_LEN {
            return 0;
        }
        self.prg_ram[(addr as usize - 0x6000) & 0x1FFF]
    }

    pub(in crate::cartridge) fn write_prg_ram_namco163(&mut self, addr: u16, data: u8) {
        let Some(namco163) = self.mappers.namco163.as_ref() else {
            return;
        };
        let window = ((addr as usize - 0x6000) >> 11) & 0x03;
        if !namco163.wram_write_enable || (namco163.wram_write_protect >> window) & 1 != 0 {
            return;
        }
        if self.prg_ram.len() >= NAMCO163_WRAM_LEN {
            self.prg_ram[(addr as usize - 0x6000) & 0x1FFF] = data;
            if self.has_battery {
                self.has_valid_save_data = true;
            }
        }
    }
}
