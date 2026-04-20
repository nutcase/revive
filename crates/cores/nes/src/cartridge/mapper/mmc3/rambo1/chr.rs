use super::super::Mmc3;
use crate::cartridge::Cartridge;

impl Cartridge {
    fn resolve_chr_bank_raw_rambo1(&self, addr: u16, mmc3: &Mmc3) -> (usize, usize) {
        let slot = ((addr >> 10) & 0x07) as usize;
        let adjusted_slot = if (mmc3.bank_select & 0x80) != 0 {
            slot ^ 4
        } else {
            slot
        };
        let full_chr_mode = (mmc3.bank_select & 0x20) != 0;

        let bank_1k = match adjusted_slot {
            0 => mmc3.rambo1_register(0) as usize,
            1 => {
                if full_chr_mode {
                    mmc3.rambo1_register(8) as usize
                } else {
                    (mmc3.rambo1_register(0) as usize & !1) | 1
                }
            }
            2 => mmc3.rambo1_register(1) as usize,
            3 => {
                if full_chr_mode {
                    mmc3.rambo1_register(9) as usize
                } else {
                    (mmc3.rambo1_register(1) as usize & !1) | 1
                }
            }
            4 => mmc3.rambo1_register(2) as usize,
            5 => mmc3.rambo1_register(3) as usize,
            6 => mmc3.rambo1_register(4) as usize,
            7 => mmc3.rambo1_register(5) as usize,
            _ => 0,
        };

        let local_offset = (addr & 0x03FF) as usize;
        (bank_1k, local_offset)
    }

    pub(in crate::cartridge) fn read_chr_mapper64(&self, addr: u16) -> u8 {
        if let Some(ref mmc3) = self.mappers.mmc3 {
            let chr_data = if !self.chr_ram.is_empty() {
                &self.chr_ram
            } else {
                &self.chr_rom
            };
            if chr_data.is_empty() {
                return 0;
            }

            let (bank_1k, local_offset) = self.resolve_chr_bank_raw_rambo1(addr, mmc3);
            let bank_count = (chr_data.len() / 0x0400).max(1);
            let chr_addr = (bank_1k % bank_count) * 0x0400 + local_offset;
            chr_data[chr_addr % chr_data.len()]
        } else {
            0
        }
    }

    pub(in crate::cartridge) fn write_chr_mapper64(&mut self, addr: u16, data: u8) {
        if self.chr_ram.is_empty() {
            return;
        }

        if let Some(ref mmc3) = self.mappers.mmc3 {
            let (bank_1k, local_offset) = self.resolve_chr_bank_raw_rambo1(addr, mmc3);
            let bank_count = (self.chr_ram.len() / 0x0400).max(1);
            let chr_addr = (bank_1k % bank_count) * 0x0400 + local_offset;
            let chr_len = self.chr_ram.len();
            self.chr_ram[chr_addr % chr_len] = data;
        }
    }
}
