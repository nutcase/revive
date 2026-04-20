use super::super::super::{Cartridge, Mirroring};

impl Cartridge {
    pub(super) fn sync_taito207_mirroring(&mut self) {
        if self.mapper != 207 {
            return;
        }
        if let Some(taito) = self.mappers.taito_x1005.as_ref() {
            let top = (taito.chr_banks[0] >> 7) & 1;
            let bottom = (taito.chr_banks[1] >> 7) & 1;
            self.mirroring = match (top, bottom) {
                (0, 0) => Mirroring::OneScreenLower,
                (1, 1) => Mirroring::OneScreenUpper,
                (0, 1) => Mirroring::Horizontal,
                (1, 0) => Mirroring::HorizontalSwapped,
                _ => Mirroring::Horizontal,
            };
        }
    }

    pub(super) fn read_prg_taito_like(&self, addr: u16, prg_banks: &[u8]) -> u8 {
        if self.prg_rom.is_empty() {
            return 0;
        }

        let bank_count = (self.prg_rom.len() / 0x2000).max(1);
        let second_last = bank_count.saturating_sub(2);
        let last = bank_count.saturating_sub(1);

        let (bank, base) = match addr {
            0x8000..=0x9FFF => (prg_banks.first().copied().unwrap_or(0) as usize, 0x8000),
            0xA000..=0xBFFF => (prg_banks.get(1).copied().unwrap_or(0) as usize, 0xA000),
            0xC000..=0xDFFF => (
                prg_banks
                    .get(2)
                    .copied()
                    .map(usize::from)
                    .unwrap_or(second_last),
                0xC000,
            ),
            0xE000..=0xFFFF => (last, 0xE000),
            _ => return 0,
        };

        let rom_addr = (bank % bank_count) * 0x2000 + (addr - base) as usize;
        self.prg_rom[rom_addr % self.prg_rom.len()]
    }

    fn resolve_taito_chr_bank(
        chr_banks: &[u8; 6],
        addr: u16,
        chr_invert: bool,
        mask_high_mirroring_bits: bool,
    ) -> usize {
        let slot = ((addr >> 10) & 0x07) as usize;
        let adjusted_slot = if chr_invert { slot ^ 4 } else { slot };
        let chr0 = if mask_high_mirroring_bits {
            chr_banks[0] & 0x7F
        } else {
            chr_banks[0]
        };
        let chr1 = if mask_high_mirroring_bits {
            chr_banks[1] & 0x7F
        } else {
            chr_banks[1]
        };

        match adjusted_slot {
            0 => (chr0 as usize) * 2,
            1 => (chr0 as usize) * 2 + 1,
            2 => (chr1 as usize) * 2,
            3 => (chr1 as usize) * 2 + 1,
            4 => chr_banks[2] as usize,
            5 => chr_banks[3] as usize,
            6 => chr_banks[4] as usize,
            _ => chr_banks[5] as usize,
        }
    }

    pub(super) fn read_chr_taito_like(
        &self,
        addr: u16,
        chr_banks: &[u8; 6],
        chr_invert: bool,
        mask_high_mirroring_bits: bool,
    ) -> u8 {
        let chr_data = if !self.chr_ram.is_empty() {
            &self.chr_ram
        } else {
            &self.chr_rom
        };
        if chr_data.is_empty() {
            return 0;
        }

        let bank_count = (chr_data.len() / 0x0400).max(1);
        let bank =
            Self::resolve_taito_chr_bank(chr_banks, addr, chr_invert, mask_high_mirroring_bits)
                % bank_count;
        let chr_addr = bank * 0x0400 + (addr as usize & 0x03FF);
        chr_data[chr_addr % chr_data.len()]
    }

    pub(super) fn write_chr_taito_like(
        &mut self,
        addr: u16,
        chr_banks: &[u8; 6],
        chr_invert: bool,
        mask_high_mirroring_bits: bool,
        data: u8,
    ) {
        if !self.chr_ram.is_empty() {
            let bank_count = (self.chr_ram.len() / 0x0400).max(1);
            let bank =
                Self::resolve_taito_chr_bank(chr_banks, addr, chr_invert, mask_high_mirroring_bits)
                    % bank_count;
            let chr_addr = bank * 0x0400 + (addr as usize & 0x03FF);
            let chr_len = self.chr_ram.len();
            self.chr_ram[chr_addr % chr_len] = data;
        } else if !self.chr_rom.is_empty() {
            let bank_count = (self.chr_rom.len() / 0x0400).max(1);
            let bank =
                Self::resolve_taito_chr_bank(chr_banks, addr, chr_invert, mask_high_mirroring_bits)
                    % bank_count;
            let chr_addr = bank * 0x0400 + (addr as usize & 0x03FF);
            let chr_len = self.chr_rom.len();
            self.chr_rom[chr_addr % chr_len] = data;
        }
    }

    pub(super) fn taito_x1005_register(addr: u16) -> Option<u8> {
        if (addr & 0xFF70) == 0x7E70 {
            Some((addr & 0x000F) as u8)
        } else {
            None
        }
    }
}
