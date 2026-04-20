use super::super::super::super::super::{Cartridge, Mirroring};
use super::super::super::prg;

impl Cartridge {
    pub(in crate::cartridge) fn read_prg_mmc3(&self, addr: u16) -> u8 {
        if let Some(ref mmc3) = self.mappers.mmc3 {
            let num_8k_banks = self.prg_rom.len() / 0x2000;
            if num_8k_banks == 0 {
                return 0;
            }
            let bank_mask = num_8k_banks - 1;
            let prg_mode = (mmc3.bank_select >> 6) & 1 != 0;
            let second_last = (num_8k_banks - 2) & bank_mask;
            let last = (num_8k_banks - 1) & bank_mask;
            let bank_6 = (mmc3.bank_registers[6] as usize) & bank_mask;
            let bank_7 = (mmc3.bank_registers[7] as usize) & bank_mask;

            let slot =
                match prg::resolve_prg_slot(addr, prg_mode, bank_6, bank_7, second_last, last) {
                    Some(slot) => slot,
                    None => return 0,
                };

            let rom_addr = slot.bank * 0x2000 + slot.offset;
            if rom_addr < self.prg_rom.len() {
                self.prg_rom[rom_addr]
            } else {
                0
            }
        } else {
            0
        }
    }

    pub(in crate::cartridge) fn write_prg_mmc3(&mut self, addr: u16, data: u8) {
        if let Some(ref mut mmc3) = self.mappers.mmc3 {
            let even = (addr & 1) == 0;
            match addr {
                0x8000..=0x9FFF => {
                    if even {
                        // Bank Select
                        mmc3.bank_select = data;
                    } else {
                        // Bank Data
                        let reg = (mmc3.bank_select & 0x07) as usize;
                        mmc3.bank_registers[reg] = data;
                    }
                }
                0xA000..=0xBFFF => {
                    if even {
                        // Mirroring
                        self.mirroring = if data & 0x01 != 0 {
                            Mirroring::Horizontal
                        } else {
                            Mirroring::Vertical
                        };
                    } else {
                        // PRG-RAM protect
                        mmc3.prg_ram_write_protect = (data & 0x40) != 0;
                        mmc3.prg_ram_enabled = (data & 0x80) != 0;
                    }
                }
                0xC000..=0xDFFF => {
                    if even {
                        // IRQ Latch
                        mmc3.irq_latch = data;
                    } else {
                        // IRQ Reload
                        mmc3.irq_reload = true;
                    }
                }
                0xE000..=0xFFFF => {
                    if even {
                        // IRQ Disable
                        mmc3.irq_enabled = false;
                        mmc3.irq_pending.set(false);
                    } else {
                        // IRQ Enable
                        mmc3.irq_enabled = true;
                    }
                }
                _ => {}
            }
        }
    }
}
