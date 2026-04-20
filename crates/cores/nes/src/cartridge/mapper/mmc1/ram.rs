use super::super::super::Cartridge;

impl Cartridge {
    /// MMC1 PRG-RAM read
    pub(in crate::cartridge) fn read_prg_ram_mmc1(&self, addr: u16) -> u8 {
        if !self.prg_ram.is_empty() {
            if let Some(ref mmc1) = self.mappers.mmc1 {
                let e_bit_clear = (mmc1.control & 0x10) == 0;
                let r_bit_clear = !mmc1.prg_ram_disable;

                // Only block if both bits indicate disable
                if !e_bit_clear && !r_bit_clear {
                    return 0x00;
                }
            }

            let ram_addr = (addr - 0x6000) as usize;
            if ram_addr < self.prg_ram.len() {
                self.prg_ram[ram_addr]
            } else {
                0
            }
        } else {
            0
        }
    }

    /// MMC1 PRG-RAM write
    pub(in crate::cartridge) fn write_prg_ram_mmc1(&mut self, addr: u16, data: u8) {
        if !self.prg_ram.is_empty() {
            if let Some(ref mmc1) = self.mappers.mmc1 {
                let e_bit_clear = (mmc1.control & 0x10) == 0;
                let r_bit_clear = !mmc1.prg_ram_disable;

                if !e_bit_clear && !r_bit_clear {
                    return;
                }
            }

            let ram_addr = (addr - 0x6000) as usize;
            if ram_addr < self.prg_ram.len() {
                self.prg_ram[ram_addr] = data;

                if addr == 0x60B7 && data == 0x5A {
                    self.has_valid_save_data = true;
                }
            }
        }
    }
}
