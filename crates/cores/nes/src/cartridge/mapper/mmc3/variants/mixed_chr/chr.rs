use crate::cartridge::Cartridge;

impl Cartridge {
    pub(in crate::cartridge) fn read_chr_mapper74(&self, addr: u16) -> u8 {
        self.read_chr_mixed_mmc3(addr)
    }

    pub(in crate::cartridge) fn write_chr_mapper74(&mut self, addr: u16, data: u8) {
        self.write_chr_mixed_mmc3(addr, data)
    }

    pub(in crate::cartridge) fn read_chr_mapper119(&self, addr: u16) -> u8 {
        self.read_chr_mixed_mmc3(addr)
    }

    pub(in crate::cartridge) fn write_chr_mapper119(&mut self, addr: u16, data: u8) {
        self.write_chr_mixed_mmc3(addr, data)
    }

    pub(in crate::cartridge) fn read_chr_mapper191(&self, addr: u16) -> u8 {
        self.read_chr_mixed_mmc3(addr)
    }

    pub(in crate::cartridge) fn write_chr_mapper191(&mut self, addr: u16, data: u8) {
        self.write_chr_mixed_mmc3(addr, data)
    }

    pub(in crate::cartridge) fn read_chr_mapper192(&self, addr: u16) -> u8 {
        self.read_chr_mixed_mmc3(addr)
    }

    pub(in crate::cartridge) fn write_chr_mapper192(&mut self, addr: u16, data: u8) {
        self.write_chr_mixed_mmc3(addr, data)
    }

    pub(in crate::cartridge) fn read_chr_mapper194(&self, addr: u16) -> u8 {
        self.read_chr_mixed_mmc3(addr)
    }

    pub(in crate::cartridge) fn write_chr_mapper194(&mut self, addr: u16, data: u8) {
        self.write_chr_mixed_mmc3(addr, data)
    }

    pub(in crate::cartridge) fn read_chr_mapper195(&self, addr: u16) -> u8 {
        self.read_chr_mixed_mmc3(addr)
    }

    pub(in crate::cartridge) fn write_chr_mapper195(&mut self, addr: u16, data: u8) {
        self.write_chr_mixed_mmc3(addr, data)
    }

    pub(in crate::cartridge) fn read_chr_mapper245(&self, addr: u16) -> u8 {
        let chr_addr = (addr & 0x1FFF) as usize;
        if chr_addr < self.chr_ram.len() {
            self.chr_ram[chr_addr]
        } else if chr_addr < self.chr_rom.len() {
            self.chr_rom[chr_addr]
        } else {
            0
        }
    }

    pub(in crate::cartridge) fn write_chr_mapper245(&mut self, addr: u16, data: u8) {
        let chr_addr = (addr & 0x1FFF) as usize;
        if chr_addr < self.chr_ram.len() {
            self.chr_ram[chr_addr] = data;
        }
    }
}
