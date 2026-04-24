use super::super::super::Cartridge;

impl Cartridge {
    pub(in crate::cartridge) fn read_chr_vrc7(&self, addr: u16) -> u8 {
        let Some(vrc7) = self.mappers.vrc7.as_ref() else {
            return 0;
        };
        let chr_data = self.vrc7_chr_data();
        if chr_data.is_empty() {
            return 0;
        }

        let bank_count = (chr_data.len() / 0x0400).max(1);
        let slot = ((addr as usize) >> 10) & 7;
        let bank = vrc7.chr_banks[slot] as usize % bank_count;
        let chr_addr = bank * 0x0400 + (addr as usize & 0x03FF);
        chr_data[chr_addr % chr_data.len()]
    }

    pub(in crate::cartridge) fn write_chr_vrc7(&mut self, addr: u16, data: u8) {
        let Some(vrc7) = self.mappers.vrc7.as_ref() else {
            return;
        };
        let chr_len = self.vrc7_chr_data().len();
        if chr_len == 0 {
            return;
        }

        let bank_count = (chr_len / 0x0400).max(1);
        let slot = ((addr as usize) >> 10) & 7;
        let bank = vrc7.chr_banks[slot] as usize % bank_count;
        let chr_addr = bank * 0x0400 + (addr as usize & 0x03FF);

        if !self.chr_ram.is_empty() {
            let chr_ram_len = self.chr_ram.len();
            self.chr_ram[chr_addr % chr_ram_len] = data;
        }
    }

    fn vrc7_chr_data(&self) -> &[u8] {
        if !self.chr_ram.is_empty() {
            &self.chr_ram
        } else {
            &self.chr_rom
        }
    }
}
