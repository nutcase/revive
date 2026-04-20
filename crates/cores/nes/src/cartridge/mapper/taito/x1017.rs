use super::super::super::{Cartridge, Mirroring};

impl Cartridge {
    pub(in crate::cartridge) fn read_prg_taito_x1017(&self, addr: u16) -> u8 {
        if let Some(taito) = self.mappers.taito_x1017.as_ref() {
            self.read_prg_taito_like(addr, &taito.prg_banks)
        } else {
            0
        }
    }

    pub(in crate::cartridge) fn read_chr_taito_x1017(&self, addr: u16) -> u8 {
        if let Some(taito) = self.mappers.taito_x1017.as_ref() {
            self.read_chr_taito_like(addr, &taito.chr_banks, taito.chr_invert, false)
        } else {
            0
        }
    }

    pub(in crate::cartridge) fn write_chr_taito_x1017(&mut self, addr: u16, data: u8) {
        if let Some(taito) = self.mappers.taito_x1017.as_ref() {
            let chr_banks = taito.chr_banks;
            let chr_invert = taito.chr_invert;
            self.write_chr_taito_like(addr, &chr_banks, chr_invert, false, data);
        }
    }

    pub(in crate::cartridge) fn read_prg_ram_taito_x1017(&self, addr: u16) -> u8 {
        if let Some(taito) = self.mappers.taito_x1017.as_ref() {
            if self.prg_ram.is_empty() {
                return 0;
            }
            let (enabled, offset) = match addr {
                0x6000..=0x67FF => (taito.ram_enabled[0], (addr - 0x6000) as usize),
                0x6800..=0x6FFF => (taito.ram_enabled[1], (addr - 0x6000) as usize),
                0x7000..=0x73FF => (taito.ram_enabled[2], (addr - 0x6000) as usize),
                _ => return 0,
            };
            if enabled {
                return self.prg_ram.get(offset).copied().unwrap_or(0);
            }
        }
        0
    }

    pub(in crate::cartridge) fn write_prg_ram_taito_x1017(&mut self, addr: u16, data: u8) {
        if let Some(reg) = Self::taito_x1005_register(addr) {
            if let Some(taito) = self.mappers.taito_x1017.as_mut() {
                match reg {
                    0..=1 => {
                        taito.chr_banks[reg as usize] = data & 0x7F;
                        if reg == 0 {
                            self.chr_bank = data & 0x7F;
                        }
                    }
                    2..=5 => {
                        taito.chr_banks[reg as usize] = data;
                    }
                    6 => {
                        taito.chr_invert = data & 0x02 != 0;
                        self.mirroring = if data & 0x01 != 0 {
                            Mirroring::Vertical
                        } else {
                            Mirroring::Horizontal
                        };
                    }
                    7 => taito.ram_enabled[0] = data == 0xCA,
                    8 => taito.ram_enabled[1] = data == 0x69,
                    9 => taito.ram_enabled[2] = data == 0x84,
                    10..=12 => {
                        let bank = (data >> 2) & 0x0F;
                        taito.prg_banks[(reg - 10) as usize] = bank;
                        if reg == 10 {
                            self.prg_bank = bank;
                        }
                    }
                    13..=15 => {}
                    _ => {}
                }
            }
            return;
        }

        if let Some(taito) = self.mappers.taito_x1017.as_ref() {
            if self.prg_ram.is_empty() {
                return;
            }
            let (enabled, offset) = match addr {
                0x6000..=0x67FF => (taito.ram_enabled[0], (addr - 0x6000) as usize),
                0x6800..=0x6FFF => (taito.ram_enabled[1], (addr - 0x6000) as usize),
                0x7000..=0x73FF => (taito.ram_enabled[2], (addr - 0x6000) as usize),
                _ => return,
            };
            if enabled {
                if let Some(byte) = self.prg_ram.get_mut(offset) {
                    *byte = data;
                }
            }
        }
    }
}
