use super::*;

impl SuperFx {
    #[cfg(test)]
    pub(super) fn screen_height(&self) -> Option<usize> {
        self.screen_height_for_mode(self.screen_height_mode())
    }

    pub(super) fn effective_screen_height(&self) -> Option<usize> {
        self.screen_height_for_mode(self.effective_screen_layout_mode())
    }

    pub(super) fn screen_height_for_mode(&self, mode: u8) -> Option<usize> {
        match mode {
            0 => Some(128),
            1 => Some(160),
            2 => Some(192),
            3 => Some(256),
            _ => unreachable!(),
        }
    }

    pub(super) fn screen_height_mode(&self) -> u8 {
        (((self.scmr >> 5) & 0x01) << 1) | ((self.scmr >> 2) & 0x01)
    }

    pub(super) fn effective_screen_layout_mode(&self) -> u8 {
        if (self.por & 0x10) != 0 {
            3
        } else {
            self.screen_height_mode()
        }
    }

    pub(super) fn bits_per_pixel(&self) -> Option<usize> {
        match self.scmr & 0x03 {
            0 => Some(2),
            1 => Some(4),
            2 => Some(4),
            3 => Some(8),
            _ => None,
        }
    }

    pub(super) fn screen_base_addr(&self) -> usize {
        (self.scbr as usize) << 10
    }

    pub(super) fn screen_buffer_len(&self) -> Option<usize> {
        let height = self.effective_screen_height()?;
        let bpp = self.bits_per_pixel()?;
        let bytes_per_tile = match bpp {
            2 => 16,
            4 => 32,
            8 => 64,
            _ => return None,
        };
        Some(32 * (height / 8) * bytes_per_tile)
    }

    pub(super) fn trace_screen_word_write(&self, addr: u16, value: u16) {
        if !trace_superfx_screen_words_enabled() {
            return;
        }
        if !trace_superfx_matches_current_frame("TRACE_SUPERFX_SCREEN_WORDS_AT_FRAME") {
            return;
        }
        let Some(idx) = self.ram_addr(addr) else {
            return;
        };
        let Some(len) = self.screen_buffer_len() else {
            return;
        };
        let start = self.screen_base_addr();
        let end = start.saturating_add(len);
        if idx < start || idx >= end {
            return;
        }
        if !trace_superfx_screen_idx_matches(idx) {
            return;
        }
        use std::sync::atomic::{AtomicU32, Ordering};
        static COUNT: OnceLock<AtomicU32> = OnceLock::new();
        let n = COUNT
            .get_or_init(|| AtomicU32::new(0))
            .fetch_add(1, Ordering::Relaxed);
        let capped =
            trace_superfx_screen_idx_min().is_none() && trace_superfx_screen_idx_max().is_none();
        if capped && n >= 128 {
            return;
        }
        println!(
            "[SFX-SCREEN-W] pc={:02X}:{:04X} op={:02X} pbr={:02X} r15={:04X} rambr={:02X} scbr={:02X} scmr={:02X} addr_reg=r{}({:04X}) addr={:04X} idx={:05X} off={:05X} odd={} value={:04X} src=r{}({:04X}) dst=r{}({:04X})",
            self.current_exec_pbr,
            self.current_exec_pc,
            self.current_exec_opcode,
            self.current_exec_pbr,
            self.regs[15],
            self.rambr,
            self.scbr,
            self.scmr,
            self.current_exec_opcode & 0x0F,
            self.reg(self.current_exec_opcode & 0x0F),
            addr,
            idx,
            idx - start,
            (addr & 1) != 0,
            value,
            self.src_reg,
            self.reg(self.src_reg),
            self.dst_reg,
            self.reg(self.dst_reg),
        );
    }

    pub(super) fn trace_screen_byte_write(&self, addr: u16, value: u8) {
        if !trace_superfx_screen_bytes_enabled() {
            return;
        }
        let Some(idx) = self.ram_addr(addr) else {
            return;
        };
        let Some(len) = self.screen_buffer_len() else {
            return;
        };
        let start = self.screen_base_addr();
        let end = start.saturating_add(len);
        if idx < start || idx >= end {
            return;
        }
        if !trace_superfx_screen_idx_matches(idx) {
            return;
        }
        use std::sync::atomic::{AtomicU32, Ordering};
        static COUNT: OnceLock<AtomicU32> = OnceLock::new();
        let n = COUNT
            .get_or_init(|| AtomicU32::new(0))
            .fetch_add(1, Ordering::Relaxed);
        let capped =
            trace_superfx_screen_idx_min().is_none() && trace_superfx_screen_idx_max().is_none();
        if capped && n >= 256 {
            return;
        }
        println!(
            "[SFX-SCREEN-B] pc={:02X}:{:04X} op={:02X} pbr={:02X} r15={:04X} rambr={:02X} scbr={:02X} scmr={:02X} addr_reg=r{}({:04X}) addr={:04X} idx={:05X} off={:05X} value={:02X} src=r{}({:04X}) dst=r{}({:04X}) r10={:04X} r11={:04X}",
            self.current_exec_pbr,
            self.current_exec_pc,
            self.current_exec_opcode,
            self.current_exec_pbr,
            self.regs[15],
            self.rambr,
            self.scbr,
            self.scmr,
            self.current_exec_opcode & 0x0F,
            self.reg(self.current_exec_opcode & 0x0F),
            addr,
            idx,
            idx - start,
            value,
            self.src_reg,
            self.reg(self.src_reg),
            self.dst_reg,
            self.reg(self.dst_reg),
            self.regs[10],
            self.regs[11],
        );
    }

    pub(super) fn tile_pixel_addr(&self, x: u16, y: u16) -> Option<(usize, usize, usize)> {
        let height = self.effective_screen_height()?;
        let bpp = self.bits_per_pixel()?;
        let x = (x as u8) as usize;
        let y = (y as u8) as usize;
        if y >= height {
            return None;
        }
        let row_in_tile = y & 7;
        let bit = 7 - (x & 7);
        let cn = match height {
            128 => ((x & 0xF8) << 1) + ((y & 0xF8) >> 3),
            160 => ((x & 0xF8) << 1) + ((x & 0xF8) >> 1) + ((y & 0xF8) >> 3),
            192 => ((x & 0xF8) << 1) + (x & 0xF8) + ((y & 0xF8) >> 3),
            256 => ((y & 0x80) << 2) + ((x & 0x80) << 1) + ((y & 0x78) << 1) + ((x & 0x78) >> 3),
            _ => return None,
        };
        let bytes_per_tile = match bpp {
            2 => 16,
            4 => 32,
            8 => 64,
            _ => return None,
        };
        Some((
            self.screen_base_addr() + cn * bytes_per_tile,
            row_in_tile,
            bit,
        ))
    }

    pub(super) fn flush_pixel_cache(&mut self, cache_index: usize) {
        let cache = self.pixelcache[cache_index];
        if cache.bitpend == 0 {
            return;
        }
        self.in_cache_flush = true;
        if env_presence_cached("TRACE_CACHE_FLUSH") {
            use std::sync::atomic::{AtomicU64, Ordering};
            static CNT: AtomicU64 = AtomicU64::new(0);
            let n = CNT.fetch_add(1, Ordering::Relaxed);
            if n < 20 {
                let nz_data = cache.data.iter().filter(|&&d| d != 0).count();
                eprintln!(
                    "[FLUSH] #{} offset={} bitpend={:02X} nz_data={} data={:?}",
                    n, cache.offset, cache.bitpend, nz_data, cache.data
                );
            }
        }

        let x = (cache.offset << 3) as u16;
        let y = (cache.offset >> 5) as u16;

        let bpp = match self.bits_per_pixel() {
            Some(v) => v,
            None => {
                self.pixelcache[cache_index].bitpend = 0;
                return;
            }
        };

        let Some((tile_base, row, _)) = self.tile_pixel_addr(x, y) else {
            self.pixelcache[cache_index].bitpend = 0;
            return;
        };
        let addr_base = tile_base + row * 2;

        for n in 0..bpp {
            let byte_offset = ((n >> 1) << 4) + (n & 1);
            let addr = (addr_base + byte_offset) as u16;

            // Build the data byte from pixel cache
            let mut data: u8 = 0;
            for p in 0..8u8 {
                if cache.data[p as usize] & (1 << n) != 0 {
                    data |= 1 << p;
                }
            }

            // If not all 8 pixels are pending, merge with existing RAM data
            if cache.bitpend != 0xFF {
                let existing = self
                    .ram_addr(addr)
                    .map(|idx| self.game_ram[idx])
                    .unwrap_or(0);
                data = (existing & !cache.bitpend) | (data & cache.bitpend);
            }

            self.write_ram_byte(addr, data);
        }

        self.in_cache_flush = false;
        self.pixelcache[cache_index].bitpend = 0;
    }

    pub(super) fn flush_all_pixel_caches(&mut self) {
        self.flush_pixel_cache(1);
        self.flush_pixel_cache(0);
    }

    pub(super) fn plot_pixel(&mut self, x: u16, y: u16, color: u8) {
        let x = x as u8;
        let y = y as u8;
        // bsnes: transparency is checked before dithering and differs for 8bpp.
        if (self.por & 0x01) == 0 {
            let transparent = match self.bits_per_pixel() {
                Some(8) if (self.por & 0x08) == 0 => color == 0,
                _ => (color & 0x0F) == 0,
            };
            if transparent {
                return;
            }
        }
        // Dithering
        let color = if (self.por & 0x02) != 0 && self.bits_per_pixel() != Some(8) {
            if (x ^ y) & 1 != 0 {
                (color >> 4) & 0x0F
            } else {
                color & 0x0F
            }
        } else {
            color
        };
        let height = match self.effective_screen_height() {
            Some(value) => value as u16,
            None => return,
        };
        if u16::from(y) >= height {
            return;
        }
        let offset = ((u16::from(y) << 5) | (u16::from(x) >> 3)) as u16;
        if offset != self.pixelcache[0].offset {
            self.flush_pixel_cache(1);
            self.pixelcache[1] = self.pixelcache[0];
            self.pixelcache[0].bitpend = 0;
            self.pixelcache[0].offset = offset;
            self.pixelcache[0].data = [0; 8];
        }
        let cache_x = ((x & 7) ^ 7) as usize;
        self.pixelcache[0].data[cache_x] = color;
        self.pixelcache[0].bitpend |= 1 << cache_x;
        let tile = self.tile_pixel_addr(u16::from(x), u16::from(y));
        self.trace_plot(
            "plot",
            u16::from(x),
            u16::from(y),
            color,
            tile.map(|(base, _, _)| base),
            tile.map(|(_, row, _)| row),
            tile.map(|(_, _, bit)| bit),
        );
        if self.pixelcache[0].bitpend == 0xFF {
            self.flush_pixel_cache(1);
            self.pixelcache[1] = self.pixelcache[0];
            self.pixelcache[0].bitpend = 0;
            self.pixelcache[0].data = [0; 8];
        }
    }

    pub(super) fn read_plot_pixel(&mut self, x: u16, y: u16) -> u8 {
        self.flush_all_pixel_caches();
        let Some((tile_base, row, bit)) = self.tile_pixel_addr(x, y) else {
            return 0;
        };
        let bpp = match self.bits_per_pixel() {
            Some(value) => value,
            None => return 0,
        };
        let plane_pairs = bpp / 2;
        let mut color = 0u8;
        for pair in 0..plane_pairs {
            let pair_base = tile_base + pair * 16 + row * 2;
            let low = self
                .ram_addr(pair_base as u16)
                .map(|idx| self.game_ram[idx])
                .unwrap_or(0);
            let high = self
                .ram_addr((pair_base + 1) as u16)
                .map(|idx| self.game_ram[idx])
                .unwrap_or(0);
            color |= ((low >> bit) & 0x01) << (pair * 2);
            color |= ((high >> bit) & 0x01) << (pair * 2 + 1);
        }
        self.trace_plot("rpix", x, y, color, Some(tile_base), Some(row), Some(bit));
        color
    }

    #[allow(dead_code, clippy::too_many_arguments)]
    pub(super) fn trace_plot(
        &self,
        kind: &str,
        x: u16,
        y: u16,
        color: u8,
        tile_base: Option<usize>,
        row: Option<usize>,
        bit: Option<usize>,
    ) {
        if !trace_superfx_plot_enabled() {
            return;
        }
        if !trace_superfx_matches_current_frame("TRACE_SUPERFX_PLOT_AT_FRAME") {
            return;
        }
        use std::sync::atomic::{AtomicU32, Ordering};
        static COUNT: AtomicU32 = AtomicU32::new(0);
        let n = COUNT.fetch_add(1, Ordering::Relaxed);
        if n >= 128 {
            return;
        }
        eprintln!(
            "[SFX-PLOT] kind={} pbr={:02X} r15={:04X} rambr={:02X} scbr={:02X} scmr={:02X} por={:02X} colr={:02X} xy=({}, {}) color={:02X} tile_base={:?} row={:?} bit={:?}",
            kind,
            self.pbr,
            self.regs[15],
            self.rambr,
            self.scbr,
            self.scmr,
            self.por,
            self.colr,
            x,
            y,
            color,
            tile_base,
            row,
            bit
        );
    }
}
