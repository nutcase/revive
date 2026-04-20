use super::super::super::{Cartridge, Mirroring};

impl Cartridge {
    pub(in crate::cartridge) fn read_prg_taito_x1005(&self, addr: u16) -> u8 {
        if let Some(taito) = self.mappers.taito_x1005.as_ref() {
            self.read_prg_taito_like(addr, &taito.prg_banks)
        } else {
            0
        }
    }

    pub(in crate::cartridge) fn read_chr_taito_x1005(&self, addr: u16) -> u8 {
        if let Some(taito) = self.mappers.taito_x1005.as_ref() {
            self.read_chr_taito_like(addr, &taito.chr_banks, false, self.mapper == 207)
        } else {
            0
        }
    }

    pub(in crate::cartridge) fn write_chr_taito_x1005(&mut self, addr: u16, data: u8) {
        if let Some(taito) = self.mappers.taito_x1005.as_ref() {
            let chr_banks = taito.chr_banks;
            self.write_chr_taito_like(addr, &chr_banks, false, self.mapper == 207, data);
        }
    }

    pub(in crate::cartridge) fn read_prg_ram_taito_x1005(&self, addr: u16) -> u8 {
        if let Some(taito) = self.mappers.taito_x1005.as_ref() {
            if taito.ram_enabled && (0x7F00..=0x7FFF).contains(&addr) && !self.prg_ram.is_empty() {
                let ram_addr = ((addr - 0x7F00) & 0x007F) as usize;
                return self.prg_ram[ram_addr % self.prg_ram.len()];
            }
        }
        0
    }

    pub(in crate::cartridge) fn write_prg_ram_taito_x1005(&mut self, addr: u16, data: u8) {
        if let Some(reg) = Self::taito_x1005_register(addr) {
            if let Some(taito) = self.mappers.taito_x1005.as_mut() {
                match reg {
                    0..=5 => {
                        taito.chr_banks[reg as usize] = data;
                        if reg == 0 {
                            self.chr_bank = if self.mapper == 207 {
                                data & 0x7F
                            } else {
                                data
                            };
                        }
                        if reg <= 1 && self.mapper == 207 {
                            self.sync_taito207_mirroring();
                        }
                    }
                    6 => {
                        if self.mapper == 80 {
                            self.mirroring = if data & 0x01 != 0 {
                                Mirroring::Vertical
                            } else {
                                Mirroring::Horizontal
                            };
                        }
                    }
                    8..=10 => {
                        taito.prg_banks[(reg - 8) as usize] = data;
                        if reg == 8 {
                            self.prg_bank = data;
                        }
                        if reg == 10 {
                            taito.ram_enabled = data & 0x08 != 0;
                        }
                    }
                    _ => {}
                }
            }
            return;
        }

        if let Some(taito) = self.mappers.taito_x1005.as_ref() {
            if taito.ram_enabled && (0x7F00..=0x7FFF).contains(&addr) && !self.prg_ram.is_empty() {
                let ram_addr = ((addr - 0x7F00) & 0x007F) as usize;
                let ram_len = self.prg_ram.len();
                self.prg_ram[ram_addr % ram_len] = data;
            }
        }
    }
}
