use std::cell::Cell;

use crate::cartridge::{Cartridge, Mirroring};

#[derive(Debug, Clone)]
pub(in crate::cartridge) struct Mapper42 {
    pub(in crate::cartridge) irq_counter: u16,
    pub(in crate::cartridge) irq_enabled: bool,
    pub(in crate::cartridge) irq_pending: Cell<bool>,
}

impl Mapper42 {
    pub(in crate::cartridge) fn new() -> Self {
        Self {
            irq_counter: 0,
            irq_enabled: false,
            irq_pending: Cell::new(false),
        }
    }

    pub(in crate::cartridge) fn clock_irq_mut(&mut self, cycles: u32) {
        if !self.irq_enabled {
            return;
        }

        let next = ((self.irq_counter as u32 + cycles) & 0x7FFF) as u16;
        self.irq_counter = next;
        self.irq_pending.set(next >= 0x6000);
    }
}

impl Cartridge {
    pub(in crate::cartridge) fn read_prg_mapper42(&self, addr: u16) -> u8 {
        let bank_count = (self.prg_rom.len() / 0x2000).max(1);
        let fixed_8000 = bank_count.saturating_sub(4);
        let fixed_a000 = bank_count.saturating_sub(3);
        let fixed_c000 = bank_count.saturating_sub(2);
        let fixed_e000 = bank_count.saturating_sub(1);

        match addr {
            0x8000..=0x9FFF => self.read_prg_8k_bank(fixed_8000, 0x8000, addr),
            0xA000..=0xBFFF => self.read_prg_8k_bank(fixed_a000, 0xA000, addr),
            0xC000..=0xDFFF => self.read_prg_8k_bank(fixed_c000, 0xC000, addr),
            0xE000..=0xFFFF => self.read_prg_8k_bank(fixed_e000, 0xE000, addr),
            _ => self.read_prg_8k_bank(self.prg_bank as usize, 0x6000, addr),
        }
    }

    pub(in crate::cartridge) fn read_prg_ram_mapper42(&self, addr: u16) -> u8 {
        self.read_prg_8k_bank(self.prg_bank as usize, 0x6000, addr)
    }

    pub(in crate::cartridge) fn write_prg_mapper42(&mut self, addr: u16, data: u8) {
        if addr < 0x8000 {
            return;
        }

        let bank_count = (self.prg_rom.len() / 0x2000).max(1);
        self.prg_bank = ((data as usize & 0x0F) % bank_count) as u8;
        self.mirroring = if data & 0x10 != 0 {
            Mirroring::Horizontal
        } else {
            Mirroring::Vertical
        };

        if let Some(mapper42) = self.mappers.mapper42.as_mut() {
            if data & 0x20 != 0 {
                mapper42.irq_enabled = true;
            } else {
                mapper42.irq_enabled = false;
                mapper42.irq_counter = 0;
                mapper42.irq_pending.set(false);
            }
        }
    }
}
