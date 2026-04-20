use crate::cartridge::Cartridge;

impl Cartridge {
    /// MMC2/MMC4 PRG-RAM read ($6000-$7FFF)
    pub(in crate::cartridge) fn read_prg_ram_mmc2(&self, addr: u16) -> u8 {
        if !self.prg_ram.is_empty() {
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

    /// MMC2/MMC4 PRG-RAM write ($6000-$7FFF)
    pub(in crate::cartridge) fn write_prg_ram_mmc2(&mut self, addr: u16, data: u8) {
        if !self.prg_ram.is_empty() {
            let ram_addr = (addr - 0x6000) as usize;
            if ram_addr < self.prg_ram.len() {
                self.prg_ram[ram_addr] = data;
            }
        }
    }
}
