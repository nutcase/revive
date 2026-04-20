use super::super::super::super::Cartridge;

impl Cartridge {
    fn mmc5_chr_len(&self) -> usize {
        if !self.chr_rom.is_empty() {
            self.chr_rom.len()
        } else {
            self.chr_ram.len()
        }
    }

    fn mmc5_chr_bank_1k(&self, page: usize, sprite: bool) -> usize {
        let Some(mmc5) = self.mappers.mmc5.as_ref() else {
            return page;
        };

        let upper = (mmc5.chr_upper as usize) << 8;
        let use_bg_banks = if sprite {
            false
        } else if !mmc5.substitutions_enabled() {
            mmc5.ppu_data_uses_bg_banks
        } else {
            (mmc5.ppu_ctrl.get() & 0x20) != 0
        };

        let raw = if !use_bg_banks {
            match mmc5.chr_mode & 0x03 {
                0 => mmc5.sprite_chr_banks[7] as usize,
                1 => mmc5.sprite_chr_banks[if page < 4 { 3 } else { 7 }] as usize,
                2 => mmc5.sprite_chr_banks[(page | 1) & 7] as usize,
                _ => mmc5.sprite_chr_banks[page & 7] as usize,
            }
        } else {
            match mmc5.chr_mode & 0x03 {
                0 | 1 => mmc5.bg_chr_banks[3] as usize,
                2 => mmc5.bg_chr_banks[if page & 0x02 == 0 { 1 } else { 3 }] as usize,
                _ => mmc5.bg_chr_banks[page & 3] as usize,
            }
        };

        let (unit_pages, local_page) = match mmc5.chr_mode & 0x03 {
            0 => (8, page & 7),
            1 => (4, page & 3),
            2 => (2, page & 1),
            _ => (1, 0),
        };

        (upper | raw) * unit_pages + local_page
    }

    pub(super) fn read_mmc5_chr_1k(&self, bank_1k: usize, local_offset: usize) -> u8 {
        let len = self.mmc5_chr_len();
        if len == 0 {
            return 0;
        }
        let addr = (bank_1k * 0x0400 + local_offset) % len;
        if !self.chr_rom.is_empty() {
            self.chr_rom[addr]
        } else {
            self.chr_ram[addr]
        }
    }

    fn write_mmc5_chr_1k(&mut self, bank_1k: usize, local_offset: usize, data: u8) {
        let len = self.mmc5_chr_len();
        if len == 0 {
            return;
        }
        let addr = (bank_1k * 0x0400 + local_offset) % len;
        if !self.chr_rom.is_empty() {
            if let Some(slot) = self.chr_rom.get_mut(addr) {
                *slot = data;
            }
        } else if let Some(slot) = self.chr_ram.get_mut(addr) {
            *slot = data;
        }
    }

    pub(in crate::cartridge) fn read_chr_mmc5(&self, addr: u16) -> u8 {
        let Some(mmc5) = self.mappers.mmc5.as_ref() else {
            return 0;
        };

        if mmc5.substitutions_enabled() && mmc5.exram_mode == 0x01 {
            let bank_4k = mmc5.cached_ext_bank.get() as usize;
            let chr_addr = bank_4k * 0x1000 + (addr as usize & 0x0FFF);
            return self.read_mmc5_chr_1k(chr_addr >> 10, chr_addr & 0x03FF);
        }

        let page = ((addr as usize) >> 10) & 0x07;
        let local_offset = addr as usize & 0x03FF;
        self.read_mmc5_chr_1k(self.mmc5_chr_bank_1k(page, false), local_offset)
    }

    pub(in crate::cartridge) fn read_chr_sprite_mmc5(&self, addr: u16, _sprite_y: u8) -> u8 {
        let page = ((addr as usize) >> 10) & 0x07;
        let local_offset = addr as usize & 0x03FF;
        self.read_mmc5_chr_1k(self.mmc5_chr_bank_1k(page, true), local_offset)
    }

    pub(in crate::cartridge) fn write_chr_mmc5(&mut self, addr: u16, data: u8) {
        let page = ((addr as usize) >> 10) & 0x07;
        let local_offset = addr as usize & 0x03FF;
        self.write_mmc5_chr_1k(self.mmc5_chr_bank_1k(page, false), local_offset, data);
    }
}
