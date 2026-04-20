use std::cell::Cell;

use crate::cartridge::Cartridge;

#[derive(Debug, Clone)]
pub(in crate::cartridge) struct Mapper43 {
    pub(in crate::cartridge) irq_counter: u16,
    pub(in crate::cartridge) irq_enabled: bool,
    pub(in crate::cartridge) irq_pending: Cell<bool>,
}

impl Mapper43 {
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

        let next = self.irq_counter as u32 + cycles;
        if next >= 0x1000 {
            self.irq_counter = (next & 0x0FFF) as u16;
            self.irq_pending.set(true);
        } else {
            self.irq_counter = next as u16;
        }
    }
}

const MAPPER43_C000_BANKS: [u8; 8] = [4, 3, 4, 4, 4, 7, 5, 6];

impl Cartridge {
    pub(in crate::cartridge) fn read_prg_low_mapper43(&self, addr: u16) -> u8 {
        if self.prg_rom.len() <= 0x10000 {
            return 0;
        }

        match addr {
            0x5000..=0x5FFF => {
                let base = 0x10000;
                let offset = (addr as usize - 0x5000) & 0x07FF;
                self.prg_rom[(base + offset) % self.prg_rom.len()]
            }
            _ => 0,
        }
    }

    pub(in crate::cartridge) fn read_prg_mapper43(&self, addr: u16) -> u8 {
        match addr {
            0x8000..=0x9FFF => self.read_prg_8k_bank(1, 0x8000, addr),
            0xA000..=0xBFFF => self.read_prg_8k_bank(0, 0xA000, addr),
            0xC000..=0xDFFF => {
                let bank = MAPPER43_C000_BANKS[(self.prg_bank & 0x07) as usize] as usize;
                self.read_prg_8k_bank(bank, 0xC000, addr)
            }
            0xE000..=0xFFFF => {
                if self.prg_rom.len() > 0x12000 {
                    let offset = 0x12000 + (addr - 0xE000) as usize;
                    self.prg_rom[offset % self.prg_rom.len()]
                } else {
                    0
                }
            }
            _ => 0,
        }
    }

    pub(in crate::cartridge) fn read_prg_ram_mapper43(&self, addr: u16) -> u8 {
        self.read_prg_8k_bank(2, 0x6000, addr)
    }

    pub(in crate::cartridge) fn write_prg_mapper43(&mut self, addr: u16, data: u8) {
        match addr {
            0x4022 => {
                self.prg_bank = data & 0x07;
            }
            0x4122 | 0x8122 => {
                if let Some(mapper43) = self.mappers.mapper43.as_mut() {
                    if data & 0x01 != 0 {
                        mapper43.irq_enabled = true;
                    } else {
                        mapper43.irq_enabled = false;
                        mapper43.irq_counter = 0;
                        mapper43.irq_pending.set(false);
                    }
                }
            }
            _ => {}
        }
    }
}
