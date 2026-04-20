use super::super::super::Cartridge;

impl Cartridge {
    /// MMC1 CHR read - 8KB/4KB modes, CHR-RAM/ROM
    pub(in crate::cartridge) fn read_chr_mmc1(&self, addr: u16) -> u8 {
        if let Some(ref mmc1) = self.mappers.mmc1 {
            let chr_mode = (mmc1.control >> 4) & 0x01;

            if chr_mode == 0 {
                // 8KB mode: use CHR bank 0, ignore CHR bank 1
                let bank = (mmc1.chr_bank_0 & 0x1E) >> 1;
                let offset = (bank as usize) * 0x2000 + (addr as usize);

                if !self.chr_ram.is_empty() {
                    if offset < self.chr_ram.len() {
                        self.chr_ram[offset]
                    } else {
                        0
                    }
                } else if offset < self.chr_rom.len() {
                    self.chr_rom[offset]
                } else {
                    0
                }
            } else {
                // 4KB mode: separate banks for each 4KB region
                let (bank, local_addr) = if addr < 0x1000 {
                    (mmc1.chr_bank_0, addr as usize)
                } else {
                    (mmc1.chr_bank_1, (addr - 0x1000) as usize)
                };
                let offset = (bank as usize) * 0x1000 + local_addr;

                if !self.chr_ram.is_empty() {
                    if offset < self.chr_ram.len() {
                        self.chr_ram[offset]
                    } else {
                        0
                    }
                } else if offset < self.chr_rom.len() {
                    self.chr_rom[offset]
                } else {
                    0
                }
            }
        } else {
            0
        }
    }

    /// MMC1 CHR write - CHR-RAM/ROM with bank switching
    pub(in crate::cartridge) fn write_chr_mmc1(&mut self, addr: u16, data: u8) {
        if let Some(ref mmc1) = self.mappers.mmc1 {
            let chr_mode = (mmc1.control >> 4) & 0x01;

            if chr_mode == 0 {
                // 8KB mode
                let bank = (mmc1.chr_bank_0 & 0x1E) >> 1;
                let offset = (bank as usize) * 0x2000 + (addr as usize);

                if !self.chr_ram.is_empty() {
                    if offset < self.chr_ram.len() {
                        self.chr_ram[offset] = data;
                    }
                } else if offset < self.chr_rom.len() {
                    self.chr_rom[offset] = data;
                }
            } else {
                // 4KB mode
                let (bank, local_addr) = if addr < 0x1000 {
                    (mmc1.chr_bank_0, addr as usize)
                } else {
                    (mmc1.chr_bank_1, (addr - 0x1000) as usize)
                };
                let offset = (bank as usize) * 0x1000 + local_addr;

                if !self.chr_ram.is_empty() {
                    if offset < self.chr_ram.len() {
                        self.chr_ram[offset] = data;
                    }
                } else if offset < self.chr_rom.len() {
                    self.chr_rom[offset] = data;
                }
            }
        }
    }
}
