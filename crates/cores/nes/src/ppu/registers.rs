mod read;
mod write;

use super::{Ppu, PpuControl, PpuMask, PpuStatus};

impl Ppu {
    #[inline]
    fn register_vram_increment(&self) -> u16 {
        if self.control.contains(PpuControl::VRAM_INCREMENT) {
            32
        } else {
            1
        }
    }

    #[inline]
    fn increment_register_vram_addr(&mut self) {
        self.v = self.v.wrapping_add(self.register_vram_increment()) & 0x3FFF;
    }

    #[inline]
    fn normalize_register_vram_addr(addr: u16) -> u16 {
        if (0x3000..0x3F00).contains(&addr) {
            addr - 0x1000
        } else {
            addr
        }
    }

    #[inline]
    fn mirrored_palette_addr(addr: u16) -> usize {
        match (addr & 0x1F) as usize {
            0x10 => 0x00,
            0x14 => 0x04,
            0x18 => 0x08,
            0x1C => 0x0C,
            palette_addr => palette_addr & 0x1F,
        }
    }
}
