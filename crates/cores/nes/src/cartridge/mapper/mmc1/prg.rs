use super::super::super::Cartridge;

impl Cartridge {
    /// MMC1 PRG read with 4 banking modes + SUROM support
    pub(in crate::cartridge) fn read_prg_mmc1(&self, addr: u16, rom_addr: u16) -> u8 {
        if let Some(ref mmc1) = self.mappers.mmc1 {
            let prg_mode = (mmc1.control >> 2) & 0x03;
            let prg_size = self.prg_rom.len() / 0x4000; // Number of 16KB banks

            // SUROM support: Use CHR bank bit 4 for PRG bank extension
            let prg_bank_hi = if prg_size > 16 {
                // For SUROM: CHR bank 0 bit 4 selects which 256KB half
                ((mmc1.chr_bank_0 >> 4) & 0x01) as usize
            } else {
                0
            };

            match prg_mode {
                0 | 1 => {
                    // 32KB mode: switch 32KB at $8000
                    let bank_lo = ((mmc1.prg_bank & 0x0E) >> 1) as usize;
                    let bank = (prg_bank_hi << 3) | bank_lo;
                    let max_banks = self.prg_rom.len() / 0x8000;
                    let safe_bank = bank % max_banks;
                    let offset = safe_bank * 0x8000 + (rom_addr as usize);
                    if offset < self.prg_rom.len() {
                        self.prg_rom[offset]
                    } else {
                        0
                    }
                }
                2 => {
                    // Fix first bank at $8000, switch 16KB at $C000
                    if addr < 0xC000 {
                        let offset = (prg_bank_hi * 0x40000) + (rom_addr as usize);
                        if offset < self.prg_rom.len() {
                            self.prg_rom[offset]
                        } else {
                            0
                        }
                    } else {
                        let bank_lo = (mmc1.prg_bank & 0x0F) as usize;
                        let bank = (prg_bank_hi << 4) | bank_lo;
                        let max_banks = self.prg_rom.len() / 0x4000;
                        let safe_bank = bank % max_banks;
                        let offset = safe_bank * 0x4000 + ((addr - 0xC000) as usize);
                        if offset < self.prg_rom.len() {
                            self.prg_rom[offset]
                        } else {
                            0
                        }
                    }
                }
                _ => {
                    // Switch 16KB at $8000, fix last bank at $C000 (default after reset)
                    if addr < 0xC000 {
                        let bank_lo = (mmc1.prg_bank & 0x0F) as usize;
                        let bank = (prg_bank_hi << 4) | bank_lo;
                        let max_banks = self.prg_rom.len() / 0x4000;
                        let safe_bank = bank % max_banks;
                        let offset = safe_bank * 0x4000 + (rom_addr as usize);
                        if offset < self.prg_rom.len() {
                            self.prg_rom[offset]
                        } else {
                            0
                        }
                    } else {
                        // Fixed last bank at $C000
                        // For SUROM (512KB), the "last bank" depends on CHR bank 0 bit 4
                        let last_bank_offset = if self.prg_rom.len() > 0x40000 {
                            // SUROM: last bank of the selected 256KB region
                            let base = prg_bank_hi * 0x40000;
                            base + 0x3C000
                        } else {
                            // Standard MMC1: last bank of entire ROM
                            self.prg_rom.len() - 0x4000
                        };
                        let offset = last_bank_offset + ((addr - 0xC000) as usize);
                        if offset < self.prg_rom.len() {
                            self.prg_rom[offset]
                        } else {
                            0
                        }
                    }
                }
            }
        } else {
            0
        }
    }
}
