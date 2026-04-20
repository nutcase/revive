use super::super::prg;
use crate::cartridge::{Cartridge, Mirroring};

impl Cartridge {
    pub(in crate::cartridge) fn read_prg_mapper64(&self, addr: u16) -> u8 {
        if let Some(ref mmc3) = self.mappers.mmc3 {
            let num_8k_banks = self.prg_rom.len() / 0x2000;
            if num_8k_banks == 0 {
                return 0;
            }

            let prg_mode = (mmc3.bank_select & 0x40) != 0;
            let bank_6 = mmc3.rambo1_register(6) as usize % num_8k_banks;
            let bank_7 = mmc3.rambo1_register(7) as usize % num_8k_banks;
            let bank_f = mmc3.rambo1_register(0x0F) as usize % num_8k_banks;
            let last = num_8k_banks - 1;

            let slot = match prg::resolve_prg_slot(addr, prg_mode, bank_6, bank_7, bank_f, last) {
                Some(slot) => slot,
                None => return 0,
            };

            let rom_addr = slot.bank * 0x2000 + slot.offset;
            self.prg_rom.get(rom_addr).copied().unwrap_or(0)
        } else {
            0
        }
    }

    pub(in crate::cartridge) fn write_prg_mapper64(&mut self, addr: u16, data: u8) {
        if let Some(ref mut mmc3) = self.mappers.mmc3 {
            let even = (addr & 1) == 0;
            match addr {
                0x8000..=0x9FFF => {
                    if even {
                        mmc3.bank_select = data;
                    } else {
                        let reg = (mmc3.bank_select & 0x0F) as usize;
                        mmc3.set_rambo1_register(reg, data);
                    }
                }
                0xA000..=0xBFFF => {
                    if even {
                        self.mirroring = if data & 0x01 != 0 {
                            Mirroring::Horizontal
                        } else {
                            Mirroring::Vertical
                        };
                    }
                }
                0xC000..=0xDFFF => {
                    if even {
                        mmc3.irq_latch = data;
                    } else {
                        mmc3.irq_cycle_mode = (data & 0x01) != 0;
                        mmc3.irq_reload = true;
                        mmc3.irq_counter = 0;
                        mmc3.irq_prescaler = 4;
                    }
                }
                0xE000..=0xFFFF => {
                    if even {
                        mmc3.irq_enabled = false;
                        mmc3.irq_pending.set(false);
                        mmc3.irq_delay = 0;
                    } else {
                        mmc3.irq_enabled = true;
                    }
                }
                _ => {}
            }
        }
    }
}
