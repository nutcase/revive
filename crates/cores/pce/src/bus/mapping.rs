use super::*;

impl Bus {
    pub(super) fn resolve(&self, addr: u16) -> (BankMapping, usize) {
        let index = (addr as usize) >> 13;
        let offset = (addr as usize) & (PAGE_SIZE - 1);
        (self.banks[index], offset)
    }

    pub(super) fn update_mpr(&mut self, bank: usize) {
        let value = self.mpr[bank];
        let rom_pages = self.rom_pages();
        let cart_pages = self.cart_ram_pages();
        let mapping = match value {
            0xFF => BankMapping::Hardware,
            BRAM_PAGE => BankMapping::Bram,
            0xF8..=0xFD => {
                let ram_pages = self.total_ram_pages().max(1);
                let logical = (value - 0xF8) as usize % ram_pages;
                BankMapping::Ram {
                    base: logical * PAGE_SIZE,
                }
            }
            _ => {
                let logical = value as usize;
                if cart_pages > 0 && value >= 0x80 {
                    let cart_page = (logical - 0x80) % cart_pages.max(1);
                    BankMapping::CartRam {
                        base: cart_page * PAGE_SIZE,
                    }
                } else if rom_pages > 0 {
                    let rom_page = Self::mirror_rom_bank(logical, rom_pages);
                    BankMapping::Rom {
                        base: rom_page * PAGE_SIZE,
                    }
                } else {
                    BankMapping::Ram { base: 0 }
                }
            }
        };
        let mapping = if bank == 1 && Self::env_force_mpr1_hardware() {
            BankMapping::Hardware
        } else {
            mapping
        };
        self.banks[bank] = mapping;
    }

    pub(super) fn total_ram_pages(&self) -> usize {
        (self.ram.len() / PAGE_SIZE).max(1)
    }

    pub(super) fn rom_pages(&self) -> usize {
        self.rom.len() / PAGE_SIZE
    }

    /// Map a logical ROM bank number to a physical ROM page, handling
    /// mirroring for non-power-of-2 ROM sizes.
    ///
    /// For power-of-2 ROMs, simple modulo works.  For non-power-of-2 (e.g.
    /// 384 KB = 48 banks), the PC Engine hardware splits the 128-bank
    /// address space in half:
    ///   Banks  0-63  → lower power-of-2 portion (e.g. 32 banks = 256 KB)
    ///   Banks 64-127 → upper remainder        (e.g. 16 banks = 128 KB)
    /// Each half mirrors within its own range.  Confirmed by Mednafen's
    /// pce/huc.cpp for the m_len == 0x60000 case.
    pub(super) fn mirror_rom_bank(logical: usize, rom_pages: usize) -> usize {
        if rom_pages == 0 {
            return 0;
        }
        if rom_pages.is_power_of_two() {
            return logical % rom_pages;
        }
        // Largest power-of-2 that fits inside rom_pages.
        let lower = rom_pages.next_power_of_two() >> 1; // e.g. 32 for 48, 64 for 96
        let upper = rom_pages - lower; // e.g. 16 for 48, 32 for 96

        // Mask to 7-bit bank number (128 banks = 1 MB address space).
        // The 128-bank space is always split at the midpoint (bank 64):
        //   Banks  0-63  → lower portion (mirrored within `lower` pages)
        //   Banks 64-127 → upper portion (mirrored within `upper` pages)
        let bank = logical & 0x7F;
        if bank < 64 {
            bank % lower.max(1)
        } else {
            ((bank - 64) % upper.max(1)) + lower
        }
    }

    pub(super) fn cart_ram_pages(&self) -> usize {
        self.cart_ram.len() / PAGE_SIZE
    }

    pub(super) fn read_bram_byte(&self, offset: usize) -> u8 {
        if !*self.bram_unlocked {
            return 0xFF;
        }
        self.bram.get(offset).copied().unwrap_or(0xFF)
    }

    pub(super) fn write_bram_byte(&mut self, offset: usize, value: u8) {
        if !*self.bram_unlocked {
            return;
        }
        if let Some(slot) = self.bram.get_mut(offset) {
            *slot = value;
        }
    }
}
