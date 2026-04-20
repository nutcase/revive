mod low;
mod read;
mod write;

use crate::cartridge::Cartridge;

impl Cartridge {
    fn read_linear_prg_ram(&self, addr: u16) -> u8 {
        let offset = (addr - 0x6000) as usize;
        self.prg_ram.get(offset).copied().unwrap_or(0)
    }

    fn write_linear_prg_ram(&mut self, addr: u16, data: u8) {
        let offset = (addr - 0x6000) as usize;
        if let Some(slot) = self.prg_ram.get_mut(offset) {
            *slot = data;
        }
    }
}
