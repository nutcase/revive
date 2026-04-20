use super::super::{mapper_hooks, Ppu};

impl Ppu {
    pub(super) fn step_visible_scanline(
        &mut self,
        cartridge: Option<&crate::cartridge::Cartridge>,
    ) {
        // Evaluate sprites for this scanline at cycle 0
        if self.cycle == 0 {
            self.evaluate_scanline_sprites(cartridge);
        }

        if self.cycle == 4 && self.rendering_enabled {
            mapper_hooks::tick_mmc5_scanline(cartridge);
        }

        if self.cycle >= 1 && self.cycle <= 256 {
            self.render_pixel(cartridge);

            // Increment coarse X every 8 pixels and invalidate tile
            // cache at tile boundaries so latch-based mappers
            // (MMC2/MMC4) always fetch fresh CHR for each tile.
            if self.cycle & 7 == 0 {
                self.cached_tile_addr = 0xFFFF;
                self.increment_coarse_x();
            }
        }

        // MMC2/MMC4: 2 extra tile fetches for pipeline lookahead.
        if self.cycle == 256
            && self.rendering_enabled
            && mapper_hooks::uses_latch_fetches(cartridge)
        {
            self.pipeline_extra_tile_reads(cartridge);
        }

        // Increment Y at cycle 256
        if self.cycle == 256 {
            self.increment_y();
        }

        // Copy horizontal scroll bits from t to v at cycle 257
        if self.cycle == 257 && self.rendering_enabled {
            self.v = (self.v & !0x041F) | (self.t & 0x041F);
        }

        // Clock mapper IRQ counter (MMC3) at cycle 260 during rendering
        if self.cycle == 260 && self.rendering_enabled {
            self.mapper_irq_clock = true;
        }

        // End-of-scanline BG tile prefetch (cycles 321-336).
        // Fetches the first 2 BG tiles of the next scanline,
        // triggering MMC2/MMC4 latch updates for the next line.
        if self.cycle == 321
            && self.rendering_enabled
            && mapper_hooks::uses_latch_fetches(cartridge)
        {
            self.prefetch_bg_tiles(cartridge);
        }
    }
}
