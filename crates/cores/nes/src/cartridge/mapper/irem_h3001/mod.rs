use std::cell::Cell;

use super::super::{Cartridge, Mirroring};

#[derive(Debug, Clone)]
pub(in crate::cartridge) struct IremH3001 {
    pub(in crate::cartridge) prg_banks: [u8; 2],
    pub(in crate::cartridge) chr_banks: [u8; 8],
    pub(in crate::cartridge) prg_mode: bool,
    pub(in crate::cartridge) irq_reload: u16,
    pub(in crate::cartridge) irq_counter: u16,
    pub(in crate::cartridge) irq_enabled: bool,
    pub(in crate::cartridge) irq_pending: Cell<bool>,
}

impl IremH3001 {
    pub(in crate::cartridge) fn new() -> Self {
        Self {
            prg_banks: [0, 1],
            chr_banks: [0; 8],
            prg_mode: false,
            irq_reload: 0,
            irq_counter: 0,
            irq_enabled: false,
            irq_pending: Cell::new(false),
        }
    }

    pub(in crate::cartridge) fn clock_irq_mut(&mut self, cycles: u32) {
        if !self.irq_enabled || self.irq_counter == 0 {
            return;
        }

        let remaining = self.irq_counter as u32;
        if cycles >= remaining {
            self.irq_counter = 0;
            self.irq_pending.set(true);
        } else {
            self.irq_counter -= cycles as u16;
        }
    }
}

impl Cartridge {
    pub(in crate::cartridge) fn read_prg_mapper65(&self, addr: u16) -> u8 {
        if let Some(ref h3001) = self.mappers.irem_h3001 {
            let bank_count = (self.prg_rom.len() / 0x2000).max(1);
            let fixed_second_last = bank_count.saturating_sub(2);
            let fixed_last = bank_count.saturating_sub(1);
            let bank = match addr {
                0x8000..=0x9FFF => {
                    if h3001.prg_mode {
                        fixed_second_last
                    } else {
                        h3001.prg_banks[0] as usize % bank_count
                    }
                }
                0xA000..=0xBFFF => h3001.prg_banks[1] as usize % bank_count,
                0xC000..=0xDFFF => {
                    if h3001.prg_mode {
                        h3001.prg_banks[0] as usize % bank_count
                    } else {
                        fixed_second_last
                    }
                }
                0xE000..=0xFFFF => fixed_last,
                _ => return 0,
            };
            let base = bank * 0x2000;
            let offset = (addr as usize) & 0x1FFF;
            self.prg_rom.get(base + offset).copied().unwrap_or(0)
        } else {
            0
        }
    }

    pub(in crate::cartridge) fn write_prg_mapper65(&mut self, addr: u16, data: u8) {
        if let Some(ref mut h3001) = self.mappers.irem_h3001 {
            let prg_bank_count = (self.prg_rom.len() / 0x2000).max(1);
            let chr_bank_count = if self.chr_rom.is_empty() {
                (self.chr_ram.len() / 0x0400).max(1)
            } else {
                (self.chr_rom.len() / 0x0400).max(1)
            };

            match addr & 0xF007 {
                0x8000 => {
                    h3001.prg_banks[0] = (data as usize % prg_bank_count) as u8;
                }
                0x9000 => {
                    h3001.prg_mode = data & 0x80 != 0;
                }
                0x9001 => {
                    self.mirroring = match (data >> 6) & 0x03 {
                        0 => Mirroring::Vertical,
                        2 => Mirroring::Horizontal,
                        _ => Mirroring::OneScreenLower,
                    };
                }
                0x9003 => {
                    h3001.irq_enabled = data & 0x80 != 0;
                    h3001.irq_pending.set(false);
                }
                0x9004 => {
                    h3001.irq_counter = h3001.irq_reload;
                    h3001.irq_pending.set(false);
                }
                0x9005 => {
                    h3001.irq_reload = (h3001.irq_reload & 0x00FF) | ((data as u16) << 8);
                }
                0x9006 => {
                    h3001.irq_reload = (h3001.irq_reload & 0xFF00) | data as u16;
                }
                0xA000 => {
                    h3001.prg_banks[1] = (data as usize % prg_bank_count) as u8;
                }
                0xB000..=0xB007 => {
                    let slot = (addr & 0x0007) as usize;
                    h3001.chr_banks[slot] = (data as usize % chr_bank_count) as u8;
                }
                _ => {}
            }
        }
    }

    pub(in crate::cartridge) fn read_chr_mapper65(&self, addr: u16) -> u8 {
        if let Some(ref h3001) = self.mappers.irem_h3001 {
            let slot = ((addr >> 10) & 0x07) as usize;
            let bank = h3001.chr_banks[slot] as usize;
            let offset = (addr & 0x03FF) as usize;
            if !self.chr_rom.is_empty() {
                let bank_count = (self.chr_rom.len() / 0x0400).max(1);
                let chr_addr = ((bank % bank_count) * 0x0400 + offset) % self.chr_rom.len();
                self.chr_rom[chr_addr]
            } else if !self.chr_ram.is_empty() {
                let bank_count = (self.chr_ram.len() / 0x0400).max(1);
                let chr_addr = ((bank % bank_count) * 0x0400 + offset) % self.chr_ram.len();
                self.chr_ram[chr_addr]
            } else {
                0
            }
        } else {
            0
        }
    }

    pub(in crate::cartridge) fn write_chr_mapper65(&mut self, addr: u16, data: u8) {
        let (bank, offset, chr_len) = if let Some(ref h3001) = self.mappers.irem_h3001 {
            if self.chr_ram.is_empty() {
                return;
            }
            let slot = ((addr >> 10) & 0x07) as usize;
            (
                h3001.chr_banks[slot] as usize,
                (addr & 0x03FF) as usize,
                self.chr_ram.len(),
            )
        } else {
            return;
        };

        let bank_count = (chr_len / 0x0400).max(1);
        let chr_addr = ((bank % bank_count) * 0x0400 + offset) % chr_len;
        self.chr_ram[chr_addr] = data;
    }
}
