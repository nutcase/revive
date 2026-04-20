mod pre_render;
mod vblank;
mod visible;

use super::Ppu;

impl Ppu {
    #[inline]
    pub fn step(&mut self, cartridge: Option<&crate::cartridge::Cartridge>) -> bool {
        let mut nmi = false;

        // Check for edge-triggered NMI from $2000 write
        if self.pending_nmi {
            self.pending_nmi = false;
            nmi = true;
        }

        match self.scanline {
            -1 => self.step_pre_render_scanline(cartridge),
            0..=239 => self.step_visible_scanline(cartridge),
            240 => self.step_post_render_scanline(cartridge),
            241 => {
                if self.step_vblank_start() {
                    nmi = true;
                }
            }
            242..=260 => {
                // Keep VBlank flag set during VBlank period
                // VBlank period runs from scanline 241 to 260
            }
            _ => {}
        }

        self.cycle += 1;

        // Odd-frame cycle skip: on pre-render scanline of odd frames,
        // skip the last cycle (340) when rendering is enabled
        let cycle_limit = if self.scanline == -1 && self.rendering_enabled && (self.frame & 1) == 1
        {
            340
        } else {
            341
        };

        if self.cycle >= cycle_limit {
            self.cycle = 0;
            self.scanline += 1;

            if self.scanline >= 261 {
                self.scanline = -1;
                self.frame += 1;
                self.frame_complete = true;
            }
        }

        nmi
    }
}
