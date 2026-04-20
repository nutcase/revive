use super::super::super::super::super::Cartridge;
use super::super::super::Mmc3;

impl Cartridge {
    pub(in crate::cartridge) fn read_chr_mmc3(&self, addr: u16) -> u8 {
        if let Some(ref mmc3) = self.mappers.mmc3 {
            let chr_a12_invert = (mmc3.bank_select >> 7) & 1;
            let num_1k_banks = if !self.chr_ram.is_empty() {
                self.chr_ram.len() / 0x0400
            } else {
                self.chr_rom.len() / 0x0400
            };
            if num_1k_banks == 0 {
                return 0;
            }
            let bank_mask = num_1k_banks - 1;

            let (bank_1k, local_offset) =
                self.resolve_chr_bank_mmc3(addr, chr_a12_invert, bank_mask, mmc3);

            let chr_addr = bank_1k * 0x0400 + local_offset;
            if !self.chr_ram.is_empty() {
                if chr_addr < self.chr_ram.len() {
                    self.chr_ram[chr_addr]
                } else {
                    0
                }
            } else if chr_addr < self.chr_rom.len() {
                self.chr_rom[chr_addr]
            } else {
                0
            }
        } else {
            0
        }
    }

    pub(in crate::cartridge) fn write_chr_mmc3(&mut self, addr: u16, data: u8) {
        if !self.chr_ram.is_empty() {
            if let Some(ref mmc3) = self.mappers.mmc3 {
                let chr_a12_invert = (mmc3.bank_select >> 7) & 1;
                let num_1k_banks = self.chr_ram.len() / 0x0400;
                if num_1k_banks == 0 {
                    return;
                }
                let bank_mask = num_1k_banks - 1;

                let (bank_1k, local_offset) =
                    self.resolve_chr_bank_mmc3(addr, chr_a12_invert, bank_mask, mmc3);

                let chr_addr = bank_1k * 0x0400 + local_offset;
                if chr_addr < self.chr_ram.len() {
                    self.chr_ram[chr_addr] = data;
                }
            }
        }
    }

    fn resolve_chr_bank_mmc3(
        &self,
        addr: u16,
        _chr_a12_invert: u8,
        bank_mask: usize,
        mmc3: &Mmc3,
    ) -> (usize, usize) {
        let (raw_bank, local_offset) = self.resolve_chr_bank_raw_mmc3(addr, mmc3);
        (raw_bank & bank_mask, local_offset)
    }

    pub(in crate::cartridge) fn resolve_chr_bank_raw_mmc3(
        &self,
        addr: u16,
        mmc3: &Mmc3,
    ) -> (usize, usize) {
        // CHR A12 inversion swaps the 2KB and 1KB regions:
        // invert=0: R0,R1 at $0000-$0FFF (2KB each), R2-R5 at $1000-$1FFF (1KB each)
        // invert=1: R2-R5 at $0000-$0FFF (1KB each), R0,R1 at $1000-$1FFF (2KB each)
        let chr_a12_invert = (mmc3.bank_select >> 7) & 1;
        let slot = (addr >> 10) & 7; // 0-7, each 1KB slot
        let adjusted_slot = if chr_a12_invert != 0 {
            slot ^ 4 // swap upper and lower halves
        } else {
            slot
        };

        let bank_1k = match adjusted_slot {
            0 => mmc3.bank_registers[0] as usize & !1,       // R0 low
            1 => (mmc3.bank_registers[0] as usize & !1) | 1, // R0 high
            2 => mmc3.bank_registers[1] as usize & !1,       // R1 low
            3 => (mmc3.bank_registers[1] as usize & !1) | 1, // R1 high
            4 => mmc3.bank_registers[2] as usize,
            5 => mmc3.bank_registers[3] as usize,
            6 => mmc3.bank_registers[4] as usize,
            7 => mmc3.bank_registers[5] as usize,
            _ => 0,
        };

        let local_offset = (addr & 0x03FF) as usize;
        (bank_1k, local_offset)
    }
}
