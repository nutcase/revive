use super::super::super::{Cartridge, Mirroring};

impl Cartridge {
    pub(in crate::cartridge) fn read_prg_taito_tc0190(&self, addr: u16) -> u8 {
        if let Some(taito) = self.mappers.taito_tc0190.as_ref() {
            self.read_prg_taito_like(addr, &[taito.prg_banks[0], taito.prg_banks[1]])
        } else {
            0
        }
    }

    pub(in crate::cartridge) fn write_prg_taito_tc0190(&mut self, addr: u16, data: u8) {
        if let Some(taito) = self.mappers.taito_tc0190.as_mut() {
            match addr & 0xF003 {
                0x8000 => {
                    taito.prg_banks[0] = data & 0x3F;
                    self.prg_bank = taito.prg_banks[0];
                    self.mirroring = if data & 0x40 != 0 {
                        Mirroring::Horizontal
                    } else {
                        Mirroring::Vertical
                    };
                }
                0x8001 => {
                    taito.prg_banks[1] = data & 0x3F;
                }
                0x8002 => {
                    taito.chr_banks[0] = data;
                    self.chr_bank = data;
                }
                0x8003 => {
                    taito.chr_banks[1] = data;
                }
                0xA000 => {
                    taito.chr_banks[2] = data;
                }
                0xA001 => {
                    taito.chr_banks[3] = data;
                }
                0xA002 => {
                    taito.chr_banks[4] = data;
                }
                0xA003 => {
                    taito.chr_banks[5] = data;
                }
                _ => {}
            }
        }
    }

    pub(in crate::cartridge) fn write_prg_mapper48(&mut self, addr: u16, data: u8) {
        if let Some(taito) = self.mappers.taito_tc0190.as_mut() {
            match addr & 0xF003 {
                0x8000 => {
                    taito.prg_banks[0] = data & 0x3F;
                    self.prg_bank = taito.prg_banks[0];
                }
                0x8001 => {
                    taito.prg_banks[1] = data & 0x3F;
                }
                0x8002 => {
                    taito.chr_banks[0] = data;
                    self.chr_bank = data;
                }
                0x8003 => {
                    taito.chr_banks[1] = data;
                }
                0xA000 => {
                    taito.chr_banks[2] = data;
                }
                0xA001 => {
                    taito.chr_banks[3] = data;
                }
                0xA002 => {
                    taito.chr_banks[4] = data;
                }
                0xA003 => {
                    taito.chr_banks[5] = data;
                }
                0xC000 => {
                    taito.irq_latch = !data;
                }
                0xC001 => {
                    taito.irq_reload = true;
                }
                0xC002 => {
                    taito.irq_enabled = true;
                }
                0xC003 => {
                    taito.irq_enabled = false;
                    taito.irq_pending.set(false);
                    taito.irq_delay = 0;
                }
                0xE000 => {
                    self.mirroring = if data & 0x40 != 0 {
                        Mirroring::Horizontal
                    } else {
                        Mirroring::Vertical
                    };
                }
                _ => {}
            }
        }
    }

    pub(in crate::cartridge) fn read_chr_taito_tc0190(&self, addr: u16) -> u8 {
        if let Some(taito) = self.mappers.taito_tc0190.as_ref() {
            self.read_chr_taito_like(addr, &taito.chr_banks, false, false)
        } else {
            0
        }
    }

    pub(in crate::cartridge) fn write_chr_taito_tc0190(&mut self, addr: u16, data: u8) {
        if let Some(taito) = self.mappers.taito_tc0190.as_ref() {
            let chr_banks = taito.chr_banks;
            self.write_chr_taito_like(addr, &chr_banks, false, false, data);
        }
    }
}
