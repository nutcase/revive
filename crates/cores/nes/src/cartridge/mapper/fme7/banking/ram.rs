use super::super::super::super::Cartridge;

impl Cartridge {
    pub(in crate::cartridge) fn read_prg_ram_fme7(&self, addr: u16) -> u8 {
        if let Some(ref fme7) = self.mappers.fme7 {
            if !fme7.prg_ram_enabled {
                return 0;
            }
            if fme7.prg_ram_select {
                let ram_addr = (addr - 0x6000) as usize;
                if ram_addr < self.prg_ram.len() {
                    self.prg_ram[ram_addr]
                } else {
                    0
                }
            } else {
                let num_8k_banks = self.prg_rom.len() / 0x2000;
                if num_8k_banks == 0 {
                    return 0;
                }
                let bank = (fme7.prg_bank_6000 as usize) % num_8k_banks;
                let offset = (addr - 0x6000) as usize;
                let rom_addr = bank * 0x2000 + offset;
                if rom_addr < self.prg_rom.len() {
                    self.prg_rom[rom_addr]
                } else {
                    0
                }
            }
        } else {
            0
        }
    }

    pub(in crate::cartridge) fn write_prg_ram_fme7(&mut self, addr: u16, data: u8) {
        if let Some(ref fme7) = self.mappers.fme7 {
            if !fme7.prg_ram_enabled || !fme7.prg_ram_select {
                return;
            }
            let ram_addr = (addr - 0x6000) as usize;
            if ram_addr < self.prg_ram.len() {
                self.prg_ram[ram_addr] = data;
            }
        }
    }
}
