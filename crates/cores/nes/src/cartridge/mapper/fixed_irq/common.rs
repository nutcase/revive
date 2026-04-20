use std::cell::Cell;

use crate::cartridge::Cartridge;

pub(super) fn clock_one_shot_irq(
    counter: &mut u16,
    enabled: &mut bool,
    pending: &Cell<bool>,
    cycles: u32,
    threshold: u32,
) {
    if !*enabled {
        return;
    }

    let next = *counter as u32 + cycles;
    if next >= threshold {
        *counter = threshold as u16;
        *enabled = false;
        pending.set(true);
    } else {
        *counter = next as u16;
    }
}

impl Cartridge {
    pub(in crate::cartridge) fn read_prg_8k_bank(&self, bank: usize, base: u16, addr: u16) -> u8 {
        if self.prg_rom.is_empty() || addr < base {
            return 0;
        }

        let bank_count = (self.prg_rom.len() / 0x2000).max(1);
        let offset = (bank % bank_count) * 0x2000 + (addr - base) as usize;
        self.prg_rom[offset % self.prg_rom.len()]
    }
}
