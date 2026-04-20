use crate::cartridge::Cartridge;

impl Cartridge {
    pub(in crate::cartridge) fn read_chr_namco108(&self, addr: u16) -> u8 {
        if let Some(ref mmc3) = self.mappers.mmc3 {
            let chr_data = if !self.chr_ram.is_empty() {
                &self.chr_ram
            } else {
                &self.chr_rom
            };
            let num_1k_banks = chr_data.len() / 0x0400;
            if num_1k_banks == 0 {
                return 0;
            }
            let bank_mask = num_1k_banks - 1;
            let bank = self.resolve_chr_bank_namco108(addr, bank_mask, &mmc3.bank_registers);
            let chr_addr = bank * 0x0400 + (addr as usize & 0x03FF);
            if chr_addr < chr_data.len() {
                chr_data[chr_addr]
            } else {
                0
            }
        } else {
            0
        }
    }

    pub(in crate::cartridge) fn write_chr_namco108(&mut self, addr: u16, data: u8) {
        let bank_registers = if let Some(ref mmc3) = self.mappers.mmc3 {
            mmc3.bank_registers
        } else {
            return;
        };

        if !self.chr_ram.is_empty() {
            let num_1k_banks = self.chr_ram.len() / 0x0400;
            if num_1k_banks == 0 {
                return;
            }
            let bank_mask = num_1k_banks - 1;
            let bank = self.resolve_chr_bank_namco108(addr, bank_mask, &bank_registers);
            let chr_addr = bank * 0x0400 + (addr as usize & 0x03FF);
            if chr_addr < self.chr_ram.len() {
                self.chr_ram[chr_addr] = data;
            }
        } else if !self.chr_rom.is_empty() {
            let num_1k_banks = self.chr_rom.len() / 0x0400;
            if num_1k_banks == 0 {
                return;
            }
            let bank_mask = num_1k_banks - 1;
            let bank = self.resolve_chr_bank_namco108(addr, bank_mask, &bank_registers);
            let chr_addr = bank * 0x0400 + (addr as usize & 0x03FF);
            if chr_addr < self.chr_rom.len() {
                self.chr_rom[chr_addr] = data;
            }
        }
    }

    fn resolve_chr_bank_namco108(
        &self,
        addr: u16,
        bank_mask: usize,
        bank_registers: &[u8; 8],
    ) -> usize {
        let slot = ((addr >> 10) & 0x07) as usize;

        match self.mapper {
            112 => match slot {
                0 | 1 => (((bank_registers[2] as usize) << 1) | (slot & 1)) & bank_mask,
                2 | 3 => (((bank_registers[3] as usize) << 1) | (slot & 1)) & bank_mask,
                4 => (bank_registers[4] as usize) & bank_mask,
                5 => (bank_registers[5] as usize) & bank_mask,
                6 => (bank_registers[6] as usize) & bank_mask,
                7 => (bank_registers[7] as usize) & bank_mask,
                _ => 0,
            },
            76 => {
                let bank_2k = bank_registers[2 + (slot / 2)] as usize;
                ((bank_2k << 1) | (slot & 1)) & bank_mask
            }
            88 | 95 | 154 => match slot {
                0 => (bank_registers[0] as usize & !1) & bank_mask,
                1 => ((bank_registers[0] as usize & !1) | 1) & bank_mask,
                2 => (bank_registers[1] as usize & !1) & bank_mask,
                3 => ((bank_registers[1] as usize & !1) | 1) & bank_mask,
                4 => {
                    let high = if self.mapper == 88 || self.mapper == 154 {
                        0x40
                    } else {
                        0
                    };
                    ((bank_registers[2] as usize) | high) & bank_mask
                }
                5 => {
                    let high = if self.mapper == 88 || self.mapper == 154 {
                        0x40
                    } else {
                        0
                    };
                    ((bank_registers[3] as usize) | high) & bank_mask
                }
                6 => {
                    let high = if self.mapper == 88 || self.mapper == 154 {
                        0x40
                    } else {
                        0
                    };
                    ((bank_registers[4] as usize) | high) & bank_mask
                }
                7 => {
                    let high = if self.mapper == 88 || self.mapper == 154 {
                        0x40
                    } else {
                        0
                    };
                    ((bank_registers[5] as usize) | high) & bank_mask
                }
                _ => 0,
            },
            _ => match slot {
                0 => (bank_registers[0] as usize & !1) & bank_mask,
                1 => ((bank_registers[0] as usize & !1) | 1) & bank_mask,
                2 => (bank_registers[1] as usize & !1) & bank_mask,
                3 => ((bank_registers[1] as usize & !1) | 1) & bank_mask,
                4 => (bank_registers[2] as usize) & bank_mask,
                5 => (bank_registers[3] as usize) & bank_mask,
                6 => (bank_registers[4] as usize) & bank_mask,
                7 => (bank_registers[5] as usize) & bank_mask,
                _ => 0,
            },
        }
    }
}
