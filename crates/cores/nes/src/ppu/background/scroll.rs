use super::*;

impl Ppu {
    #[inline]
    pub(in crate::ppu) fn increment_coarse_x(&mut self) {
        if !self.rendering_enabled {
            return;
        }
        if (self.v & 0x001F) == 31 {
            self.v &= !0x001F; // coarse X = 0
            self.v ^= 0x0400; // toggle horizontal nametable
        } else {
            self.v += 1;
        }
    }

    #[inline]
    pub(in crate::ppu) fn increment_y(&mut self) {
        if !self.rendering_enabled {
            return;
        }
        if (self.v & 0x7000) != 0x7000 {
            // fine Y < 7, just increment
            self.v += 0x1000;
        } else {
            // fine Y overflow
            self.v &= !0x7000; // fine Y = 0
            let mut coarse_y = (self.v & 0x03E0) >> 5;
            if coarse_y == 29 {
                coarse_y = 0;
                self.v ^= 0x0800; // toggle vertical nametable
            } else if coarse_y == 31 {
                coarse_y = 0; // wrap without NT toggle
            } else {
                coarse_y += 1;
            }
            self.v = (self.v & !0x03E0) | (coarse_y << 5);
        }
    }
}
