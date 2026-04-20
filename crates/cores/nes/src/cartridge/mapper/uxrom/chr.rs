use crate::cartridge::Cartridge;

impl Cartridge {
    /// UxROM CHR read - 8KB CHR RAM
    pub(in crate::cartridge) fn read_chr_uxrom(&self, addr: u16) -> u8 {
        let chr_addr = (addr & 0x1FFF) as usize;
        if chr_addr < self.chr_rom.len() {
            self.chr_rom[chr_addr]
        } else {
            0
        }
    }

    /// UxROM CHR write - 8KB CHR RAM (writable)
    pub(in crate::cartridge) fn write_chr_uxrom(&mut self, addr: u16, data: u8) {
        let chr_addr = (addr & 0x1FFF) as usize;
        if chr_addr < self.chr_rom.len() {
            self.chr_rom[chr_addr] = data;
        }
    }
}
