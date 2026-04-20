use std::cell::Cell;

use crate::cartridge::Cartridge;

use super::common::clock_one_shot_irq;

#[derive(Debug, Clone)]
pub(in crate::cartridge) struct Mapper40 {
    pub(in crate::cartridge) irq_counter: u16,
    pub(in crate::cartridge) irq_enabled: bool,
    pub(in crate::cartridge) irq_pending: Cell<bool>,
}

impl Mapper40 {
    pub(in crate::cartridge) fn new() -> Self {
        Self {
            irq_counter: 0,
            irq_enabled: false,
            irq_pending: Cell::new(false),
        }
    }

    pub(in crate::cartridge) fn clock_irq_mut(&mut self, cycles: u32) {
        clock_one_shot_irq(
            &mut self.irq_counter,
            &mut self.irq_enabled,
            &self.irq_pending,
            cycles,
            4096,
        );
    }
}

impl Cartridge {
    pub(in crate::cartridge) fn read_prg_mapper40(&self, addr: u16) -> u8 {
        let bank_count = (self.prg_rom.len() / 0x2000).max(1);
        let fixed_6000 = bank_count.saturating_sub(2);
        let fixed_8000 = bank_count.saturating_sub(4);
        let fixed_a000 = bank_count.saturating_sub(3);
        let fixed_e000 = bank_count.saturating_sub(1);

        match addr {
            0x8000..=0x9FFF => self.read_prg_8k_bank(fixed_8000, 0x8000, addr),
            0xA000..=0xBFFF => self.read_prg_8k_bank(fixed_a000, 0xA000, addr),
            0xC000..=0xDFFF => self.read_prg_8k_bank(self.prg_bank as usize, 0xC000, addr),
            0xE000..=0xFFFF => self.read_prg_8k_bank(fixed_e000, 0xE000, addr),
            _ => self.read_prg_8k_bank(fixed_6000, 0x6000, addr),
        }
    }

    pub(in crate::cartridge) fn read_prg_ram_mapper40(&self, addr: u16) -> u8 {
        let bank_count = (self.prg_rom.len() / 0x2000).max(1);
        let fixed_6000 = bank_count.saturating_sub(2);
        self.read_prg_8k_bank(fixed_6000, 0x6000, addr)
    }

    pub(in crate::cartridge) fn write_prg_mapper40(&mut self, addr: u16, data: u8) {
        if let Some(mapper40) = self.mappers.mapper40.as_mut() {
            match addr & 0xE000 {
                0x8000 => {
                    mapper40.irq_enabled = false;
                    mapper40.irq_counter = 0;
                    mapper40.irq_pending.set(false);
                }
                0xA000 => {
                    mapper40.irq_enabled = true;
                    mapper40.irq_counter = 0;
                    mapper40.irq_pending.set(false);
                }
                0xE000 => {
                    let bank_count = (self.prg_rom.len() / 0x2000).max(1);
                    self.prg_bank = (((data as usize) & 0x07) % bank_count) as u8;
                }
                _ => {}
            }
        }
    }
}
