use crate::cartridge::Cartridge;

impl Cartridge {
    pub(super) fn jaleco_ss88006_chr_bank_count_1k(&self) -> usize {
        if self.chr_rom.is_empty() {
            (self.chr_ram.len() / 0x0400).max(1)
        } else {
            (self.chr_rom.len() / 0x0400).max(1)
        }
    }

    pub(super) fn jaleco_ss88006_write_chr_bank(&mut self, index: usize, high: bool, data: u8) {
        let Some(mapper18) = self.mappers.jaleco_ss88006.as_mut() else {
            return;
        };
        if index >= mapper18.chr_banks.len() {
            return;
        }
        let bank = &mut mapper18.chr_banks[index];
        if high {
            *bank = (*bank & 0x0F) | ((data & 0x0F) << 4);
        } else {
            *bank = (*bank & 0xF0) | (data & 0x0F);
        }
    }

    pub(in crate::cartridge) fn read_chr_mapper18(&self, addr: u16) -> u8 {
        let Some(mapper18) = self.mappers.jaleco_ss88006.as_ref() else {
            return 0;
        };
        let bank = mapper18.chr_banks[((addr as usize) >> 10) & 0x07] as usize;
        let offset = (addr as usize) & 0x03FF;
        let bank_count = self.jaleco_ss88006_chr_bank_count_1k();
        let chr_addr = (bank % bank_count) * 0x0400 + offset;
        if self.chr_rom.is_empty() {
            self.chr_ram.get(chr_addr).copied().unwrap_or(0)
        } else {
            self.chr_rom.get(chr_addr).copied().unwrap_or(0)
        }
    }

    pub(in crate::cartridge) fn write_chr_mapper18(&mut self, addr: u16, data: u8) {
        let Some(mapper18) = self.mappers.jaleco_ss88006.as_ref() else {
            return;
        };
        if self.chr_ram.is_empty() {
            return;
        }

        let bank = mapper18.chr_banks[((addr as usize) >> 10) & 0x07] as usize;
        let offset = (addr as usize) & 0x03FF;
        let bank_count = self.jaleco_ss88006_chr_bank_count_1k();
        let chr_addr = (bank % bank_count) * 0x0400 + offset;
        if let Some(slot) = self.chr_ram.get_mut(chr_addr) {
            *slot = data;
        }
    }
}
