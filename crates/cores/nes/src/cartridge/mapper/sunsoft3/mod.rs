use std::cell::Cell;

use super::super::{Cartridge, Mirroring};

#[derive(Debug, Clone)]
pub struct Sunsoft3 {
    pub chr_banks: [u8; 4],
    pub prg_bank: u8,
    pub irq_counter: u16,
    pub irq_enabled: bool,
    pub irq_pending: Cell<bool>,
    pub irq_write_high: bool,
}

impl Sunsoft3 {
    pub fn new() -> Self {
        Self {
            chr_banks: [0; 4],
            prg_bank: 0,
            irq_counter: 0,
            irq_enabled: false,
            irq_pending: Cell::new(false),
            irq_write_high: true,
        }
    }

    pub fn clock_irq_mut(&mut self, cycles: u32) {
        if !self.irq_enabled || cycles == 0 {
            return;
        }

        if cycles > self.irq_counter as u32 {
            self.irq_counter = 0xFFFF;
            self.irq_enabled = false;
            self.irq_pending.set(true);
        } else {
            self.irq_counter -= cycles as u16;
        }
    }

    pub fn decode_mirroring(data: u8) -> Mirroring {
        match data & 0x03 {
            0 => Mirroring::Vertical,
            1 => Mirroring::Horizontal,
            2 => Mirroring::OneScreenLower,
            _ => Mirroring::OneScreenUpper,
        }
    }
}

impl Cartridge {
    fn read_chr_bank_2k_sunsoft3(&self, bank: u8, offset: usize) -> u8 {
        if self.chr_rom.is_empty() {
            return 0;
        }

        let bank_count = (self.chr_rom.len() / 0x0800).max(1);
        let bank = (bank as usize) % bank_count;
        let chr_addr = bank * 0x0800 + offset;
        self.chr_rom[chr_addr % self.chr_rom.len()]
    }

    fn write_chr_bank_2k_sunsoft3(&mut self, bank: u8, offset: usize, data: u8) {
        if self.chr_rom.is_empty() {
            return;
        }

        let bank_count = (self.chr_rom.len() / 0x0800).max(1);
        let bank = (bank as usize) % bank_count;
        let chr_addr = bank * 0x0800 + offset;
        let chr_len = self.chr_rom.len();
        self.chr_rom[chr_addr % chr_len] = data;
    }

    pub(in crate::cartridge) fn read_prg_sunsoft3(&self, addr: u16) -> u8 {
        if self.prg_rom.is_empty() || addr < 0x8000 {
            return 0;
        }

        if addr < 0xC000 {
            let bank_count = (self.prg_rom.len() / 0x4000).max(1);
            let bank = (self.prg_bank as usize) % bank_count;
            let rom_addr = bank * 0x4000 + (addr - 0x8000) as usize;
            self.prg_rom[rom_addr % self.prg_rom.len()]
        } else {
            let rom_addr = self.prg_rom.len().saturating_sub(0x4000) + (addr - 0xC000) as usize;
            self.prg_rom[rom_addr % self.prg_rom.len()]
        }
    }

    pub(in crate::cartridge) fn write_prg_sunsoft3(&mut self, addr: u16, data: u8) {
        let mut new_mirroring = None;
        let mut new_prg_bank = None;
        let chr_bank_0;
        if let Some(sunsoft3) = self.mappers.sunsoft3.as_mut() {
            match addr & 0xF800 {
                0x8000 => sunsoft3.irq_pending.set(false),
                0x8800 => sunsoft3.chr_banks[0] = data,
                0x9800 => sunsoft3.chr_banks[1] = data,
                0xA800 => sunsoft3.chr_banks[2] = data,
                0xB800 => sunsoft3.chr_banks[3] = data,
                0xC800 => {
                    if sunsoft3.irq_write_high {
                        sunsoft3.irq_counter =
                            (sunsoft3.irq_counter & 0x00FF) | ((data as u16) << 8);
                    } else {
                        sunsoft3.irq_counter = (sunsoft3.irq_counter & 0xFF00) | data as u16;
                    }
                    sunsoft3.irq_write_high = !sunsoft3.irq_write_high;
                }
                0xD800 => {
                    sunsoft3.irq_enabled = data & 0x10 != 0;
                    sunsoft3.irq_write_high = true;
                }
                0xE800 => new_mirroring = Some(Sunsoft3::decode_mirroring(data)),
                0xF800 => {
                    sunsoft3.prg_bank = data & 0x0F;
                    new_prg_bank = Some(sunsoft3.prg_bank);
                }
                _ => {}
            }
            chr_bank_0 = sunsoft3.chr_banks[0];
        } else {
            return;
        }

        if let Some(mirroring) = new_mirroring {
            self.mirroring = mirroring;
        }
        if let Some(prg_bank) = new_prg_bank {
            self.prg_bank = prg_bank;
        }
        self.chr_bank = chr_bank_0;
    }

    pub(in crate::cartridge) fn read_chr_sunsoft3(&self, addr: u16) -> u8 {
        let Some(sunsoft3) = self.mappers.sunsoft3.as_ref() else {
            return 0;
        };
        let slot = ((addr as usize) >> 11) & 0x03;
        let offset = (addr as usize) & 0x07FF;
        self.read_chr_bank_2k_sunsoft3(sunsoft3.chr_banks[slot], offset)
    }

    pub(in crate::cartridge) fn write_chr_sunsoft3(&mut self, addr: u16, data: u8) {
        let Some(bank) = self
            .mappers
            .sunsoft3
            .as_ref()
            .map(|sunsoft3| sunsoft3.chr_banks[((addr as usize) >> 11) & 0x03])
        else {
            return;
        };
        let offset = (addr as usize) & 0x07FF;
        self.write_chr_bank_2k_sunsoft3(bank, offset, data);
    }
}
