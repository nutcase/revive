mod chr;
mod prg;
mod ram;

use super::super::super::super::Cartridge;
use super::super::prg as mmc3_prg;

const MAPPER114_INDEX_SCRAMBLE: [u8; 8] = [0, 3, 1, 5, 6, 7, 2, 4];

impl Cartridge {
    fn mapper114_scramble_index(data: u8) -> u8 {
        (data & !0x07) | MAPPER114_INDEX_SCRAMBLE[(data & 0x07) as usize]
    }

    fn translate_mapper114_addr(addr: u16) -> Option<u16> {
        match addr & 0xE001 {
            0x8000 => Some(0xA001),
            0x8001 => Some(0xA000),
            0xA000 => Some(0x8000),
            0xA001 => Some(0xC000),
            0xC000 => Some(0x8001),
            0xC001 => Some(0xC001),
            0xE000 => Some(0xE000),
            0xE001 => Some(0xE001),
            _ => None,
        }
    }

    fn read_prg_nrom_override(
        &self,
        addr: u16,
        bank_16k: usize,
        replace_bit0_with_a14: bool,
    ) -> u8 {
        let bank_count = (self.prg_rom.len() / 0x4000).max(1);
        let bank = if replace_bit0_with_a14 {
            (bank_16k & !1) | (((addr as usize - 0x8000) >> 14) & 0x01)
        } else {
            bank_16k
        } % bank_count;
        let rom_addr = bank * 0x4000 + (addr as usize & 0x3FFF);
        self.prg_rom.get(rom_addr).copied().unwrap_or(0)
    }

    fn mapper114_selected_16k_bank(&self) -> usize {
        (self.mappers.mmc3_variant.mapper114_override as usize & 0x0F)
            | (((self.mappers.mmc3_variant.mapper114_override as usize) & 0x20) >> 1)
    }

    fn mapper123_selected_16k_bank(&self) -> usize {
        let data = self.mappers.mmc3_variant.mapper123_override as usize;
        (data & 0x01) | ((data & 0x10) >> 3) | (data & 0x04) | ((data & 0x20) >> 2)
    }

    fn mapper115_selected_16k_bank(&self) -> usize {
        (self.mappers.mmc3_variant.mapper115_override as usize & 0x0F)
            | (((self.mappers.mmc3_variant.mapper115_override as usize) & 0x40) >> 2)
    }

    fn mapper205_prg_window(&self) -> (usize, usize) {
        match self.mappers.mmc3_variant.mapper205_block & 0x03 {
            0 => (0x00, 0x1F),
            1 => (0x10, 0x1F),
            2 => (0x20, 0x0F),
            _ => (0x30, 0x0F),
        }
    }

    fn mapper205_chr_window(&self) -> (usize, usize) {
        match self.mappers.mmc3_variant.mapper205_block & 0x03 {
            0 => (0x000, 0xFF),
            1 => (0x080, 0xFF),
            2 => (0x100, 0x7F),
            _ => (0x180, 0x7F),
        }
    }

    fn read_prg_windowed_mmc3(&self, addr: u16, base: usize, bank_mask: usize) -> u8 {
        if let Some(ref mmc3) = self.mappers.mmc3 {
            let num_8k_banks = self.prg_rom.len() / 0x2000;
            if num_8k_banks == 0 {
                return 0;
            }

            let prg_mode = (mmc3.bank_select >> 6) & 1 != 0;
            let second_last = base + bank_mask.saturating_sub(1);
            let last = base + bank_mask;
            let bank_6 = base + ((mmc3.bank_registers[6] as usize) & bank_mask);
            let bank_7 = base + ((mmc3.bank_registers[7] as usize) & bank_mask);

            let slot =
                match mmc3_prg::resolve_prg_slot(addr, prg_mode, bank_6, bank_7, second_last, last)
                {
                    Some(slot) => slot,
                    None => return 0,
                };

            let rom_addr = (slot.bank % num_8k_banks) * 0x2000 + slot.offset;
            self.prg_rom.get(rom_addr).copied().unwrap_or(0)
        } else {
            0
        }
    }

    fn read_chr_windowed_mmc3(&self, addr: u16, base: usize, bank_mask: usize) -> u8 {
        let chr_data = if !self.chr_ram.is_empty() {
            &self.chr_ram
        } else {
            &self.chr_rom
        };
        if chr_data.is_empty() {
            return 0;
        }

        if let Some(ref mmc3) = self.mappers.mmc3 {
            let (raw_bank, local_offset) = self.resolve_chr_bank_raw_mmc3(addr, mmc3);
            let bank_count = (chr_data.len() / 0x0400).max(1);
            let bank = (base + (raw_bank & bank_mask)) % bank_count;
            let chr_addr = bank * 0x0400 + local_offset;
            chr_data[chr_addr % chr_data.len()]
        } else {
            0
        }
    }

    fn write_chr_windowed_mmc3(&mut self, addr: u16, base: usize, bank_mask: usize, data: u8) {
        if self.chr_ram.is_empty() {
            return;
        }

        if let Some(ref mmc3) = self.mappers.mmc3 {
            let (raw_bank, local_offset) = self.resolve_chr_bank_raw_mmc3(addr, mmc3);
            let bank_count = (self.chr_ram.len() / 0x0400).max(1);
            let bank = (base + (raw_bank & bank_mask)) % bank_count;
            let chr_addr = bank * 0x0400 + local_offset;
            let chr_len = self.chr_ram.len();
            self.chr_ram[chr_addr % chr_len] = data;
        }
    }

    fn mapper37_prg_window(&self) -> (usize, usize) {
        match self.mappers.mmc3_variant.mapper37_outer_bank & 0x07 {
            0..=2 => (0, 0x07),
            3 => (8, 0x07),
            4..=6 => (16, 0x0F),
            _ => (24, 0x07),
        }
    }

    fn mapper44_prg_window(&self) -> (usize, usize) {
        let block = self.mappers.mmc3_variant.mapper44_outer_bank & 0x07;
        if block < 6 {
            ((block as usize) << 4, 0x0F)
        } else {
            (0x60 + (((block & 0x01) as usize) << 4), 0x0F)
        }
    }

    fn mapper44_chr_window(&self) -> (usize, usize) {
        let block = self.mappers.mmc3_variant.mapper44_outer_bank & 0x07;
        if block < 6 {
            ((block as usize) << 7, 0x7F)
        } else {
            (0x300 + (((block & 0x01) as usize) << 7), 0x7F)
        }
    }
}
