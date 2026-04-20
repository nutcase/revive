use super::{Ppu, PpuControl};

impl Ppu {
    /// Prefetch the first two BG tiles using the current v register.
    /// Emulates cycles 321-336 of real hardware, where the PPU reads the
    /// first two tiles of the next scanline.  For MMC2/MMC4, these CHR reads
    /// trigger latch updates that reset the latch to the correct state.
    pub(super) fn prefetch_bg_tiles(&self, cartridge: Option<&crate::cartridge::Cartridge>) {
        let cart = match cartridge {
            Some(c) => c,
            None => return,
        };

        let fine_y = (self.v >> 12) & 7;
        let coarse_y = ((self.v >> 5) & 0x1F) as usize;
        let coarse_x = (self.v & 0x1F) as usize;
        let logical_nt = ((self.v >> 10) & 3) as usize;

        let pattern_table: u16 = if self.control.contains(PpuControl::BG_PATTERN) {
            0x1000
        } else {
            0x0000
        };

        // First tile
        let physical_nt = self.resolve_nametable(logical_nt, cartridge);
        let nt_addr = coarse_y * 32 + coarse_x;
        if nt_addr < 1024 {
            let tile_id = self.read_nametable_byte(physical_nt, nt_addr, cartridge);
            let tile_addr = pattern_table + (tile_id as u16 * 16) + fine_y;
            if tile_addr < 0x2000 {
                cart.read_chr(tile_addr);
                cart.read_chr(tile_addr + 8);
            }
        }

        // Second tile (coarse_x + 1, wrapping nametable)
        let (next_cx, next_nt) = if coarse_x == 31 {
            (0, logical_nt ^ 1)
        } else {
            (coarse_x + 1, logical_nt)
        };
        let next_physical = self.resolve_nametable(next_nt, cartridge);
        let next_nt_addr = coarse_y * 32 + next_cx;
        if next_nt_addr < 1024 {
            let tile_id = self.read_nametable_byte(next_physical, next_nt_addr, cartridge);
            let tile_addr = pattern_table + (tile_id as u16 * 16) + fine_y;
            if tile_addr < 0x2000 {
                cart.read_chr(tile_addr);
                cart.read_chr(tile_addr + 8);
            }
        }
    }

    /// Perform 2 extra BG tile CHR reads at the current v position.
    /// On real hardware, the PPU's tile fetch pipeline runs 2 tiles ahead
    /// of display. At the end of each scanline's 32 tile fetches, 2 extra
    /// tiles are fetched beyond the visible area. These CHR reads trigger
    /// MMC2/MMC4 latch updates critical for correct CHR bank selection.
    pub(super) fn pipeline_extra_tile_reads(
        &mut self,
        cartridge: Option<&crate::cartridge::Cartridge>,
    ) {
        let cart = match cartridge {
            Some(c) => c,
            None => return,
        };

        let fine_y = (self.v >> 12) & 7;
        let pattern_table: u16 = if self.control.contains(PpuControl::BG_PATTERN) {
            0x1000
        } else {
            0x0000
        };

        for _ in 0..2 {
            let coarse_y = ((self.v >> 5) & 0x1F) as usize;
            let coarse_x = (self.v & 0x1F) as usize;
            let logical_nt = ((self.v >> 10) & 3) as usize;
            let physical_nt = self.resolve_nametable(logical_nt, cartridge);
            let nt_addr = coarse_y * 32 + coarse_x;
            if nt_addr < 1024 {
                let tile_id = self.read_nametable_byte(physical_nt, nt_addr, cartridge);
                let tile_addr = pattern_table + (tile_id as u16 * 16) + fine_y;
                if tile_addr < 0x2000 {
                    cart.read_chr(tile_addr);
                    cart.read_chr(tile_addr + 8);
                }
            }
            self.increment_coarse_x();
        }
    }
}
