use super::super::{mapper_hooks, Ppu, PpuControl, PpuStatus};

impl Ppu {
    pub(super) fn step_pre_render_scanline(
        &mut self,
        cartridge: Option<&crate::cartridge::Cartridge>,
    ) {
        // Pre-render scanline - clear flags at cycle 1
        if self.cycle == 1 {
            self.vblank_flag_set_this_frame = false;
            self.status.remove(PpuStatus::VBLANK);
            self.status.remove(PpuStatus::SPRITE_0_HIT);
            self.status.remove(PpuStatus::SPRITE_OVERFLOW);
        }

        // Pre-render BG tile fetches (cycles 1-256).
        // Real hardware fetches BG tiles during these cycles using
        // the current v register (stale from VBlank $2006/$2007
        // operations). For MMC2/MMC4, these CHR reads trigger latch
        // updates that set the correct CHR bank before scanline 0.
        if self.cycle >= 1
            && self.cycle <= 256
            && self.rendering_enabled
            && mapper_hooks::uses_latch_fetches(cartridge)
        {
            if let Some(cart) = cartridge {
                // CHR pattern fetch at cycle 5 of each 8-cycle group
                if self.cycle % 8 == 5 {
                    let fine_y = (self.v >> 12) & 7;
                    let coarse_y = ((self.v >> 5) & 0x1F) as usize;
                    let coarse_x = (self.v & 0x1F) as usize;
                    let logical_nt = ((self.v >> 10) & 3) as usize;
                    let physical_nt = self.resolve_nametable(logical_nt, cartridge);
                    let nt_addr = coarse_y * 32 + coarse_x;
                    if nt_addr < 1024 {
                        let tile_id = self.nametable[physical_nt][nt_addr];
                        let pattern_table: u16 = if self.control.contains(PpuControl::BG_PATTERN) {
                            0x1000
                        } else {
                            0x0000
                        };
                        let tile_addr = pattern_table + (tile_id as u16 * 16) + fine_y;
                        if tile_addr < 0x2000 {
                            cart.read_chr(tile_addr);
                            cart.read_chr(tile_addr + 8);
                        }
                    }
                }
                // Increment coarse X at end of each 8-cycle group
                if self.cycle.is_multiple_of(8) {
                    self.increment_coarse_x();
                }
            }
        }

        // MMC2/MMC4: 2 extra tile fetches for pipeline lookahead.
        if self.cycle == 256
            && self.rendering_enabled
            && mapper_hooks::uses_latch_fetches(cartridge)
        {
            self.pipeline_extra_tile_reads(cartridge);
        }

        // Copy horizontal scroll bits from t to v at cycle 257
        if self.cycle == 257 && self.rendering_enabled {
            self.v = (self.v & !0x041F) | (self.t & 0x041F);
        }

        // Update vertical scroll during pre-render scanline
        if (280..=304).contains(&self.cycle) && self.rendering_enabled {
            // Copy vertical scroll bits from t to v
            self.v = (self.v & !0x7BE0) | (self.t & 0x7BE0);
        }

        // BG tile prefetch (cycles 321-336 on real hardware).
        // Reads the first two BG tiles of scanline 0, which triggers
        // MMC2/MMC4 latch updates and resets the latch state after any
        // VBlank $2007 reads that may have corrupted it.
        if self.cycle == 321
            && self.rendering_enabled
            && mapper_hooks::uses_latch_fetches(cartridge)
        {
            self.prefetch_bg_tiles(cartridge);
        }
    }
}
