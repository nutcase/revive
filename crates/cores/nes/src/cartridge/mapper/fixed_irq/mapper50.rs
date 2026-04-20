use std::cell::Cell;

use crate::cartridge::Cartridge;

use super::common::clock_one_shot_irq;

#[derive(Debug, Clone)]
pub(in crate::cartridge) struct Mapper50 {
    pub(in crate::cartridge) irq_counter: u16,
    pub(in crate::cartridge) irq_enabled: bool,
    pub(in crate::cartridge) irq_pending: Cell<bool>,
}

impl Mapper50 {
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
    pub(in crate::cartridge) fn read_prg_mapper50(&self, addr: u16) -> u8 {
        match addr {
            0x8000..=0x9FFF => self.read_prg_8k_bank(0x08, 0x8000, addr),
            0xA000..=0xBFFF => self.read_prg_8k_bank(0x09, 0xA000, addr),
            0xC000..=0xDFFF => self.read_prg_8k_bank(self.prg_bank as usize, 0xC000, addr),
            0xE000..=0xFFFF => self.read_prg_8k_bank(0x0B, 0xE000, addr),
            _ => self.read_prg_8k_bank(0x0F, 0x6000, addr),
        }
    }

    pub(in crate::cartridge) fn read_prg_ram_mapper50(&self, addr: u16) -> u8 {
        self.read_prg_8k_bank(0x0F, 0x6000, addr)
    }

    pub(in crate::cartridge) fn write_prg_mapper50(&mut self, addr: u16, data: u8) {
        match addr & 0xD160 {
            0x4120 => {
                if let Some(mapper50) = self.mappers.mapper50.as_mut() {
                    mapper50.irq_pending.set(false);
                    mapper50.irq_enabled = data & 0x01 != 0;
                    if !mapper50.irq_enabled {
                        mapper50.irq_counter = 0;
                    }
                }
            }
            0x4020 => {
                let bank_count = (self.prg_rom.len() / 0x2000).max(1);
                let bank = ((data & 0x01) << 2)
                    | ((data & 0x02) >> 1)
                    | ((data & 0x04) >> 1)
                    | (data & 0x08);
                self.prg_bank = (bank as usize % bank_count) as u8;
            }
            _ => {}
        }
    }
}
