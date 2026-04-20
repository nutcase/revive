use super::*;

impl Cartridge {
    pub(in crate::cartridge) fn read_chr_namco163(&self, addr: u16) -> u8 {
        let Some(namco163) = self.mappers.namco163.as_ref() else {
            return 0;
        };
        let slot = ((addr as usize) >> 10) & 7;
        let bank = namco163.chr_banks[slot];
        self.read_namco163_chr_bank(bank, addr as usize & 0x03FF, slot)
    }

    pub(in crate::cartridge) fn write_chr_namco163(&mut self, addr: u16, data: u8) {
        let Some(namco163) = self.mappers.namco163.as_ref() else {
            return;
        };
        let slot = ((addr as usize) >> 10) & 7;
        let bank = namco163.chr_banks[slot];
        self.write_namco163_chr_bank(bank, addr as usize & 0x03FF, slot, data);
    }
}
