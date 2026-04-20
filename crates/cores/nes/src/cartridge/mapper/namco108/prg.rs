use crate::cartridge::{Cartridge, Mirroring};

impl Cartridge {
    /// Mapper 76/88/95/154/206 and 112: Namco 108 family variants.
    pub(in crate::cartridge) fn read_prg_namco108(&self, addr: u16) -> u8 {
        if let Some(ref mmc3) = self.mappers.mmc3 {
            let num_8k_banks = self.prg_rom.len() / 0x2000;
            if num_8k_banks < 2 {
                return 0;
            }
            let bank_mask = num_8k_banks - 1;
            let second_last = (num_8k_banks - 2) & bank_mask;
            let last = (num_8k_banks - 1) & bank_mask;

            let (bank, offset) = match addr {
                0x8000..=0x9FFF => {
                    let reg = if self.mapper == 112 { 0 } else { 6 };
                    ((mmc3.bank_registers[reg] as usize) & bank_mask, 0x8000)
                }
                0xA000..=0xBFFF => {
                    let reg = if self.mapper == 112 { 1 } else { 7 };
                    ((mmc3.bank_registers[reg] as usize) & bank_mask, 0xA000)
                }
                0xC000..=0xDFFF => (second_last, 0xC000),
                0xE000..=0xFFFF => (last, 0xE000),
                _ => return 0,
            };

            let rom_addr = bank * 0x2000 + (addr - offset) as usize;
            if rom_addr < self.prg_rom.len() {
                self.prg_rom[rom_addr]
            } else {
                0
            }
        } else {
            0
        }
    }

    pub(in crate::cartridge) fn write_prg_namco108(&mut self, addr: u16, data: u8) {
        if self.mapper == 112 {
            if let Some(ref mut mmc3) = self.mappers.mmc3 {
                match addr {
                    0x8000..=0x9FFF => mmc3.bank_select = data & 0x07,
                    0xA000..=0xBFFF => {
                        let reg = (mmc3.bank_select & 0x07) as usize;
                        mmc3.bank_registers[reg] = data;
                    }
                    0xE000..=0xFFFF => {
                        self.mirroring = if data & 0x01 != 0 {
                            Mirroring::Horizontal
                        } else {
                            Mirroring::Vertical
                        };
                    }
                    _ => {}
                }
            }
            return;
        }

        if self.mapper == 154 && addr >= 0x8000 {
            self.mirroring = if data & 0x40 != 0 {
                Mirroring::OneScreenUpper
            } else {
                Mirroring::OneScreenLower
            };
        }

        let mut mapper95_banks = None;
        if let Some(ref mut mmc3) = self.mappers.mmc3 {
            match addr {
                0x8000..=0x9FFF if (addr & 1) == 0 => {
                    mmc3.bank_select = data & 0x07;
                }
                0x8000..=0x9FFF => {
                    let reg = (mmc3.bank_select & 0x07) as usize;
                    mmc3.bank_registers[reg] =
                        Self::mask_namco108_bank_data(self.mapper, reg, data);
                    if self.mapper == 95 && (reg == 0 || reg == 1) {
                        mapper95_banks = Some(mmc3.bank_registers);
                    }
                }
                _ => {}
            }
        }

        if let Some(bank_registers) = mapper95_banks {
            self.update_mapper95_mirroring(&bank_registers);
        }
    }
}
