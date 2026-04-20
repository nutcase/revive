use super::super::super::{prg, Mmc3};
use crate::cartridge::Cartridge;

impl Cartridge {
    pub(in crate::cartridge) fn read_prg_mapper191(&self, addr: u16) -> u8 {
        if let Some(ref mmc3) = self.mappers.mmc3 {
            let num_8k_banks = self.prg_rom.len() / 0x2000;
            if num_8k_banks == 0 {
                return 0;
            }
            let bank_mask = num_8k_banks - 1;
            let prg_mode = (mmc3.bank_select >> 6) & 1 != 0;
            let fixed_base = (0x18 + self.mapper191_effective_outer_bank() * 2) & bank_mask;
            let second_last = fixed_base;
            let last = (fixed_base + 1) & bank_mask;
            let bank_6 = (mmc3.bank_registers[6] as usize) & bank_mask;
            let bank_7 = (mmc3.bank_registers[7] as usize) & bank_mask;

            let slot =
                match prg::resolve_prg_slot(addr, prg_mode, bank_6, bank_7, second_last, last) {
                    Some(slot) => slot,
                    None => return 0,
                };

            let rom_addr = slot.bank * 0x2000 + slot.offset;
            self.prg_rom.get(rom_addr).copied().unwrap_or(0)
        } else {
            0
        }
    }

    pub(in crate::cartridge) fn write_prg_mapper191(&mut self, addr: u16, data: u8) {
        if (addr & 0xF0FF) == 0x90AA {
            if self.mapper191_outer_bank_writable() {
                self.mappers.mmc3_variant.mapper191_outer_bank = data & 0x03;
            }
            return;
        }

        self.write_prg_mmc3(addr, data);
    }

    pub(in crate::cartridge) fn write_prg_mapper189(&mut self, addr: u16, data: u8) {
        if addr < 0x8000 {
            let bank_count = (self.prg_rom.len() / 0x8000).max(1);
            let bank = (((data >> 4) | (data & 0x0F)) as usize) % bank_count;
            self.mappers.mmc3_variant.mapper189_prg_bank = bank as u8;
            self.prg_bank = bank as u8;
            return;
        }

        self.write_prg_mmc3(addr, data);
    }

    pub(in crate::cartridge) fn write_prg_ram_mapper189(&mut self, data: u8) {
        let bank_count = (self.prg_rom.len() / 0x8000).max(1);
        let bank = (((data >> 4) | (data & 0x0F)) as usize) % bank_count;
        self.mappers.mmc3_variant.mapper189_prg_bank = bank as u8;
        self.prg_bank = bank as u8;
    }

    fn mapper245_prg_bank_base(mmc3: &Mmc3) -> usize {
        let high_bit_source = if (mmc3.bank_select & 0x80) == 0 {
            mmc3.bank_registers[0]
        } else {
            mmc3.bank_registers[2]
        };

        ((high_bit_source >> 1) as usize & 0x01) << 5
    }

    pub(in crate::cartridge) fn read_prg_mapper245(&self, addr: u16) -> u8 {
        if let Some(ref mmc3) = self.mappers.mmc3 {
            let num_8k_banks = self.prg_rom.len() / 0x2000;
            if num_8k_banks == 0 {
                return 0;
            }

            let prg_mode = (mmc3.bank_select >> 6) & 1 != 0;
            let base = Self::mapper245_prg_bank_base(mmc3);
            let second_last = (base | 30) % num_8k_banks;
            let last = (base | 31) % num_8k_banks;
            let bank_6 = (base | (mmc3.bank_registers[6] as usize & 0x1F)) % num_8k_banks;
            let bank_7 = (base | (mmc3.bank_registers[7] as usize & 0x1F)) % num_8k_banks;

            let slot =
                match prg::resolve_prg_slot(addr, prg_mode, bank_6, bank_7, second_last, last) {
                    Some(slot) => slot,
                    None => return 0,
                };

            let rom_addr = slot.bank * 0x2000 + slot.offset;
            self.prg_rom.get(rom_addr).copied().unwrap_or(0)
        } else {
            0
        }
    }
}
