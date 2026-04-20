use crate::cartridge::Cartridge;

impl Cartridge {
    fn current_mmc2_chr_bank(&self, addr: u16) -> Option<u8> {
        let mmc2 = self.mappers.mmc2.as_ref()?;
        let bank = if addr < 0x1000 {
            if mmc2.latch_0.get() {
                mmc2.chr_bank_0_fe
            } else {
                mmc2.chr_bank_0_fd
            }
        } else if mmc2.latch_1.get() {
            mmc2.chr_bank_1_fe
        } else {
            mmc2.chr_bank_1_fd
        };
        Some(bank)
    }

    /// MMC2/MMC4 CHR read with latch mechanism
    pub(in crate::cartridge) fn read_chr_mmc2(&self, addr: u16) -> u8 {
        if let Some(ref mmc2) = self.mappers.mmc2 {
            let Some(bank) = self.current_mmc2_chr_bank(addr) else {
                return 0;
            };
            let local_addr = (addr & 0x0FFF) as usize;
            let offset = (bank as usize) * 0x1000 + local_addr;

            let data = if !self.chr_ram.is_empty() {
                if offset < self.chr_ram.len() {
                    self.chr_ram[offset]
                } else {
                    0
                }
            } else if offset < self.chr_rom.len() {
                self.chr_rom[offset]
            } else {
                0
            };

            // Update latches AFTER the read based on the address fetched
            match addr {
                0x0FD8..=0x0FDF => mmc2.latch_0.set(false), // FD
                0x0FE8..=0x0FEF => mmc2.latch_0.set(true),  // FE
                0x1FD8..=0x1FDF => mmc2.latch_1.set(false), // FD
                0x1FE8..=0x1FEF => mmc2.latch_1.set(true),  // FE
                _ => {}
            }

            data
        } else {
            0
        }
    }

    /// MMC2/MMC4 CHR write (CHR-RAM)
    pub(in crate::cartridge) fn write_chr_mmc2(&mut self, addr: u16, data: u8) {
        let Some(bank) = self.current_mmc2_chr_bank(addr) else {
            return;
        };

        let local_addr = (addr & 0x0FFF) as usize;
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
