use super::super::super::super::Cartridge;

impl Cartridge {
    pub(in crate::cartridge) fn read_chr_vrc6(&self, addr: u16) -> u8 {
        let Some(vrc6) = self.mappers.vrc6.as_ref() else {
            return 0;
        };
        let chr_data = self.vrc6_chr_data();
        if chr_data.is_empty() {
            return 0;
        }

        let bank_count = (chr_data.len() / 0x0400).max(1);
        let slot = ((addr as usize) >> 10) & 7;
        let bank = vrc6.chr_banks[slot] as usize % bank_count;
        let chr_addr = bank * 0x0400 + (addr as usize & 0x03FF);
        chr_data[chr_addr % chr_data.len()]
    }

    pub(in crate::cartridge) fn write_chr_vrc6(&mut self, addr: u16, data: u8) {
        if self.chr_ram.is_empty() {
            return;
        }
        let chr_ram_len = self.chr_ram.len();
        let bank_count = (chr_ram_len / 0x0400).max(1);
        let slot = ((addr as usize) >> 10) & 7;
        let bank = {
            let Some(vrc6) = self.mappers.vrc6.as_ref() else {
                return;
            };
            vrc6.chr_banks[slot] as usize % bank_count
        };
        let chr_addr = bank * 0x0400 + (addr as usize & 0x03FF);
        if let Some(cell) = self.chr_ram.get_mut(chr_addr % chr_ram_len) {
            *cell = data;
        }
    }
}
