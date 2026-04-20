use crate::ppu::{trace_sample_dot_config, Ppu};
use std::sync::OnceLock;

pub(crate) type SuperfxDirectSample = (usize, usize, usize, usize, u8, u8, u8, u8, u8, u8);

fn env_i32(name: &'static str) -> Option<i32> {
    std::env::var(name).ok()?.trim().parse::<i32>().ok()
}

fn env_u8(name: &'static str) -> Option<u8> {
    std::env::var(name).ok()?.trim().parse::<u8>().ok()
}

fn env_flag(name: &'static str) -> bool {
    std::env::var(name)
        .map(|value| {
            let value = value.trim();
            value == "1" || value.eq_ignore_ascii_case("true") || value.eq_ignore_ascii_case("yes")
        })
        .unwrap_or(false)
}

fn superfx_direct_x_offset_override() -> Option<i32> {
    static VALUE: OnceLock<Option<i32>> = OnceLock::new();
    *VALUE.get_or_init(|| env_i32("SUPERFX_DIRECT_X_OFFSET"))
}

fn superfx_direct_y_offset_override() -> Option<i32> {
    static VALUE: OnceLock<Option<i32>> = OnceLock::new();
    *VALUE.get_or_init(|| env_i32("SUPERFX_DIRECT_Y_OFFSET"))
}

fn superfx_direct_row_major() -> bool {
    static VALUE: OnceLock<bool> = OnceLock::new();
    // PLOT/RPIX address SuperFX screen RAM in the packed tile layout below.
    // Row-major remains available as a debug override for captured buffers
    // that have already been linearized.
    *VALUE.get_or_init(|| {
        std::env::var("SUPERFX_DIRECT_ROW_MAJOR")
            .map(|value| {
                let value = value.trim();
                value == "1"
                    || value.eq_ignore_ascii_case("true")
                    || value.eq_ignore_ascii_case("yes")
            })
            .unwrap_or(false)
    })
}

fn superfx_direct_lsb_first() -> bool {
    static VALUE: OnceLock<bool> = OnceLock::new();
    *VALUE.get_or_init(|| env_flag("SUPERFX_DIRECT_LSB_FIRST"))
}

fn superfx_direct_swap_xy() -> bool {
    static VALUE: OnceLock<bool> = OnceLock::new();
    *VALUE.get_or_init(|| env_flag("SUPERFX_DIRECT_SWAP_XY"))
}

fn superfx_direct_use_tile() -> bool {
    static VALUE: OnceLock<bool> = OnceLock::new();
    *VALUE.get_or_init(|| env_flag("SUPERFX_DIRECT_USE_TILE"))
}

fn superfx_direct_palette_bank_override() -> Option<u8> {
    static VALUE: OnceLock<Option<u8>> = OnceLock::new();
    *VALUE.get_or_init(|| env_u8("SUPERFX_DIRECT_PALETTE_BANK").filter(|v| *v < 8))
}

fn superfx_direct_priority_override() -> Option<u8> {
    static VALUE: OnceLock<Option<u8>> = OnceLock::new();
    *VALUE.get_or_init(|| env_u8("SUPERFX_DIRECT_PRIORITY").filter(|v| *v <= 1))
}

fn superfx_direct_grayscale() -> bool {
    static VALUE: OnceLock<bool> = OnceLock::new();
    *VALUE.get_or_init(|| env_flag("SUPERFX_DIRECT_GRAYSCALE"))
}

fn superfx_direct_min_color_index() -> u8 {
    static VALUE: OnceLock<u8> = OnceLock::new();
    *VALUE.get_or_init(|| env_u8("SUPERFX_DIRECT_MIN_COLOR").unwrap_or(0))
}

fn trace_superfx_direct_source() -> bool {
    static VALUE: OnceLock<bool> = OnceLock::new();
    *VALUE.get_or_init(|| env_flag("TRACE_SUPERFX_DIRECT_SOURCE"))
}

fn superfx_hide_bg2() -> bool {
    static VALUE: OnceLock<bool> = OnceLock::new();
    *VALUE.get_or_init(|| env_flag("SUPERFX_HIDE_BG2"))
}

impl Ppu {
    pub(crate) fn render_bg_mode2(&mut self, x: u16, y: u16, bg_num: u8) -> (u32, u8) {
        // Mode 2: BG1/BG2 are 4bpp with offset-per-tile (OPT) using BG3 tilemap entries.
        if bg_num > 1 {
            return (0, 0);
        }
        if bg_num == 1 && self.should_bypass_bg1_window_for_superfx_direct() && superfx_hide_bg2() {
            return (0, 0);
        }

        let y_line = self.bg_interlace_y(y);
        let (mosaic_x, mosaic_y_base) = self.apply_mosaic(x, y, bg_num);
        let mosaic_y_line = if y_line == y {
            mosaic_y_base
        } else {
            self.apply_mosaic(x, y_line, bg_num).1
        };
        let col = self.mode2_opt_column(mosaic_x, bg_num);

        let scroll_x = self.mode2_opt_hscroll_lut[bg_num as usize][col];
        let scroll_y = self.mode2_opt_vscroll_lut[bg_num as usize][col];
        if self.should_bypass_bg1_window_for_superfx_direct() && bg_num == 0 {
            let have_direct_buffer = self.has_superfx_direct_bg1_source();
            let have_tile_buffer = self.has_superfx_tile_bg1_source();

            // The live/display screen buffer is the authoritative source for the
            // SuperFX scene. When present, it replaces the BG1 tile plane rather
            // than filling only transparent standard BG1 pixels.
            if have_direct_buffer {
                return self.render_bg_superfx_direct(mosaic_x, mosaic_y_line);
            }

            if have_tile_buffer {
                return self.render_bg_superfx_tile_fallback(
                    bg_num,
                    mosaic_x,
                    mosaic_y_base,
                    mosaic_y_line,
                    scroll_x,
                    scroll_y,
                );
            }
        }
        let standard = self.render_bg_4bpp_impl(
            bg_num,
            mosaic_x,
            mosaic_y_base,
            mosaic_y_line,
            scroll_x,
            scroll_y,
        );

        if bg_num == 0 && !self.should_mask_bg(mosaic_x, 0, true) {
            if self.has_superfx_direct_bg1_source() && standard.0 == 0 {
                let direct = self.render_bg_superfx_direct(mosaic_x, mosaic_y_line);
                if direct.0 != 0 {
                    return direct;
                }
            }
            if self.has_superfx_tile_bg1_source() && standard.0 == 0 {
                let tile = self.render_bg_superfx_tile_fallback(
                    bg_num,
                    mosaic_x,
                    mosaic_y_base,
                    mosaic_y_line,
                    scroll_x,
                    scroll_y,
                );
                if tile.0 != 0 {
                    return tile;
                }
            }
        }

        standard
    }

    fn render_bg_mode2_superfx_fallback_only(&mut self, x: u16, y: u16) -> (u32, u8) {
        if !self.has_superfx_direct_bg1_source() && !self.has_superfx_tile_bg1_source() {
            return (0, 0);
        }

        let y_line = self.bg_interlace_y(y);
        let (mosaic_x, mosaic_y_base) = self.apply_mosaic(x, y, 0);
        let mosaic_y_line = if y_line == y {
            mosaic_y_base
        } else {
            self.apply_mosaic(x, y_line, 0).1
        };
        let col = self.mode2_opt_column(mosaic_x, 0);
        let scroll_x = self.mode2_opt_hscroll_lut[0][col];
        let scroll_y = self.mode2_opt_vscroll_lut[0][col];

        let standard = self.render_bg_4bpp_impl(
            0,
            mosaic_x,
            mosaic_y_base,
            mosaic_y_line,
            scroll_x,
            scroll_y,
        );
        if standard.0 != 0 {
            return (0, 0);
        }

        if self.has_superfx_direct_bg1_source() {
            let direct = self.render_bg_superfx_direct(mosaic_x, mosaic_y_line);
            if direct.0 != 0 {
                return direct;
            }
        }
        if self.has_superfx_tile_bg1_source() {
            let tile = self.render_bg_superfx_tile_fallback(
                0,
                mosaic_x,
                mosaic_y_base,
                mosaic_y_line,
                scroll_x,
                scroll_y,
            );
            if tile.0 != 0 {
                return tile;
            }
        }

        (0, 0)
    }

    #[inline]
    pub(super) fn render_bg_mode2_window_aware(
        &mut self,
        x: u16,
        y: u16,
        bg_num: u8,
        is_main: bool,
    ) -> (u32, u8) {
        if !self.should_mask_bg(x, bg_num, is_main) {
            return self.render_bg_mode2_with_priority(x, y, bg_num);
        }
        if bg_num == 0 {
            return self.render_bg_mode2_superfx_fallback_only(x, y);
        }
        (0, 0)
    }

    #[inline]
    pub(super) fn mode2_opt_column(&self, x: u16, bg_num: u8) -> usize {
        let fine_x = match bg_num {
            0 => self.bg1_hscroll & 0x0007,
            1 => self.bg2_hscroll & 0x0007,
            _ => 0,
        };
        ((x.wrapping_add(fine_x)) / 8).min(32) as usize
    }

    /// Direct SuperFX screen rendering. This path must handle the early startup
    /// 2bpp screen modes in addition to the later 4bpp title/logo scene.
    pub(super) fn render_bg_superfx_direct(&mut self, x: u16, y: u16) -> (u32, u8) {
        let sfx_x = i32::from(x) + self.superfx_direct_x_offset();
        let sfx_y = i32::from(y) + self.superfx_direct_y_offset();
        let sfx_height = if self.superfx_direct_height != 0 {
            self.superfx_direct_height
        } else {
            192
        };
        if !(0..256).contains(&sfx_x) || !(0..i32::from(sfx_height)).contains(&sfx_y) {
            return (0, 0);
        }
        let mut sx = sfx_x as usize;
        let mut sy = sfx_y as usize;
        if superfx_direct_swap_xy() {
            std::mem::swap(&mut sx, &mut sy);
        }
        let rel_x = (sx & 7) as u8;
        let direct_bpp = self.superfx_direct_bpp;
        let use_buffer = matches!(direct_bpp, 2 | 4 | 8) && !self.superfx_direct_buffer.is_empty();
        if trace_superfx_direct_source() {
            use std::sync::atomic::{AtomicU32, Ordering};
            static COUNT: AtomicU32 = AtomicU32::new(0);
            let n = COUNT.fetch_add(1, Ordering::Relaxed);
            if n < 16 {
                eprintln!(
                    "[SFX-DIRECT] xy=({}, {}) sxy=({}, {}) use_buffer={} len={} h={} bpp={} mode={} row_major={} lsb_first={} swap_xy={}",
                    x,
                    y,
                    sx,
                    sy,
                    use_buffer,
                    self.superfx_direct_buffer.len(),
                    self.superfx_direct_height,
                    direct_bpp,
                    self.superfx_direct_mode,
                    superfx_direct_row_major()
                    ,
                    superfx_direct_lsb_first(),
                    superfx_direct_swap_xy()
                );
            }
        }
        let screen_x = usize::from(x);
        let screen_y = usize::from(y);
        let tile_color_index = {
            let tile_x = screen_x >> 3;
            let tile_y = screen_y >> 3;
            let rel_y = (screen_y & 7) as u8;
            let rel_x_screen = (screen_x & 7) as u8;
            let tile_base = self.bg1_tile_base;
            let tile_addr = (tile_base.wrapping_add((tile_y * 32 + tile_x) as u16 * 16)) & 0x7FFF;
            self.sample_bg_direct(tile_addr, rel_y, rel_x_screen)
        };
        let map_entry =
            self.get_bg_map_entry_cached(0, (screen_x >> 3) as u16, (screen_y >> 3) as u16);
        let mut used_direct_buffer = false;
        let color_index = if use_buffer {
            let buffer_color_index = self.sample_superfx_buffer_pixel(sx, sy, rel_x);
            if buffer_color_index != 0 {
                used_direct_buffer = true;
                buffer_color_index
            } else if !self.should_bypass_bg1_window_for_superfx_direct()
                && (self.has_superfx_tile_bg1_source() || superfx_direct_use_tile())
            {
                tile_color_index
            } else {
                0
            }
        } else {
            tile_color_index
        };
        if let Some(cfg) = trace_sample_dot_config() {
            if self.frame == cfg.frame && x == cfg.x && y == cfg.y {
                println!(
                    "[TRACE_SAMPLE_DOT][SFX-DIRECT] frame={} x={} y={} sxy=({}, {}) use_buffer={} len={} h={} bpp={} mode={} rel_x={} color_index=0x{:02X}",
                    self.frame,
                    x,
                    y,
                    sx,
                    sy,
                    use_buffer as u8,
                    self.superfx_direct_buffer.len(),
                    self.superfx_direct_height,
                    direct_bpp,
                    self.superfx_direct_mode,
                    rel_x,
                    color_index
                );
            }
        }
        if color_index == 0 {
            return (0, 0);
        }
        if color_index < superfx_direct_min_color_index() {
            return (0, 0);
        }
        let palette =
            superfx_direct_palette_bank_override().unwrap_or(((map_entry >> 10) & 0x07) as u8);
        let priority = superfx_direct_priority_override().unwrap_or({
            if used_direct_buffer {
                1
            } else {
                ((map_entry >> 13) & 0x01) as u8
            }
        });
        let palette_index = self.get_bg_palette_index(palette, color_index, direct_bpp);
        let color = if superfx_direct_grayscale() {
            let max_color = ((1u16 << direct_bpp.min(8)) - 1) as u32;
            let shade = if max_color == 0 {
                0
            } else {
                ((u32::from(color_index) * 255) / max_color) as u8
            };
            0xFF000000 | (u32::from(shade) << 16) | (u32::from(shade) << 8) | u32::from(shade)
        } else {
            self.cgram_to_rgb(palette_index)
        };
        (color, priority)
    }

    fn superfx_direct_x_offset(&self) -> i32 {
        superfx_direct_x_offset_override().unwrap_or(self.superfx_direct_default_x_offset)
    }

    fn superfx_direct_y_offset(&self) -> i32 {
        superfx_direct_y_offset_override().unwrap_or(self.superfx_direct_default_y_offset)
    }

    fn render_bg_superfx_tile_fallback(
        &mut self,
        bg_num: u8,
        mosaic_x: u16,
        mosaic_y_base: u16,
        mosaic_y_line: u16,
        scroll_x: u16,
        scroll_y: u16,
    ) -> (u32, u8) {
        if self.superfx_tile_bpp != 4 || self.superfx_tile_buffer.is_empty() {
            return (0, 0);
        }

        let tile_16 = self.bg_tile_16[bg_num as usize];
        let tile_px = if tile_16 { 16 } else { 8 } as u16;
        let ss = self.bg_screen_size[bg_num as usize];
        let width_tiles = if ss == 1 || ss == 3 { 64 } else { 32 } as u16;
        let height_tiles = if ss == 2 || ss == 3 { 64 } else { 32 } as u16;
        let wrap_x = width_tiles * tile_px;
        let wrap_y = height_tiles * tile_px;

        let bg_x = (mosaic_x + scroll_x) % wrap_x;
        let bg_y_tile = (mosaic_y_base + scroll_y) % wrap_y;
        let bg_y_line = (mosaic_y_line + scroll_y) % wrap_y;
        let tile_x = bg_x / tile_px;
        let tile_y = bg_y_tile / tile_px;
        let map_entry = self.get_bg_map_entry_cached(bg_num, tile_x, tile_y);

        let mut tile_id = map_entry & 0x03FF;
        let palette = ((map_entry >> 10) & 0x07) as u8;
        let flip_x = (map_entry & 0x4000) != 0;
        let flip_y = (map_entry & 0x8000) != 0;
        let priority = (map_entry & 0x2000) != 0;

        let mut rel_x = (bg_x % tile_px) as u8;
        let mut rel_y = (bg_y_line % tile_px) as u8;
        if flip_x {
            rel_x = (tile_px as u8 - 1) - rel_x;
        }
        if flip_y {
            rel_y = (tile_px as u8 - 1) - rel_y;
        }
        if tile_16 {
            let sub_x = (rel_x / 8) as u16;
            let sub_y = (rel_y / 8) as u16;
            tile_id = tile_id
                .wrapping_add(sub_x)
                .wrapping_add(sub_y.wrapping_mul(16));
            rel_x %= 8;
            rel_y %= 8;
        }

        let color_index = self.sample_superfx_tile_4bpp(tile_id, rel_x, rel_y);
        if color_index == 0 {
            return (0, 0);
        }
        let palette_index = self.get_bg_palette_index(palette, color_index, 4);
        let color = self.cgram_to_rgb(palette_index);
        (color, if priority { 1 } else { 0 })
    }

    fn sample_superfx_tile_4bpp(&self, tile_id: u16, px: u8, py: u8) -> u8 {
        let tile_base = tile_id as usize * 32;
        let row01 = tile_base + py as usize * 2;
        let row23 = tile_base + 16 + py as usize * 2;
        if row23 + 1 >= self.superfx_tile_buffer.len() {
            return 0;
        }
        let p0 = self.superfx_tile_buffer[row01];
        let p1 = self.superfx_tile_buffer[row01 + 1];
        let p2 = self.superfx_tile_buffer[row23];
        let p3 = self.superfx_tile_buffer[row23 + 1];
        let bit = 7 - px;
        ((p0 >> bit) & 1)
            | (((p1 >> bit) & 1) << 1)
            | (((p2 >> bit) & 1) << 2)
            | (((p3 >> bit) & 1) << 3)
    }

    fn superfx_direct_pixel_addr(&self, x: usize, y: usize) -> Option<(usize, usize, usize)> {
        let height = self.superfx_direct_height as usize;
        let bpp = self.superfx_direct_bpp as usize;
        if x >= 256 || y >= height {
            return None;
        }
        let row_in_tile = y & 7;
        let bit = 7 - (x & 7);
        let bytes_per_tile = match bpp {
            2 => 16,
            4 => 32,
            8 => 64,
            _ => return None,
        };
        if superfx_direct_row_major() {
            let x_tile = x >> 3;
            let y_tile = y >> 3;
            let tiles_per_row = 32usize;
            let tile_base = (y_tile * tiles_per_row + x_tile) * bytes_per_tile;
            return Some((tile_base, row_in_tile, bit));
        }
        let cn = match height {
            128 => ((x & 0xF8) << 1) + ((y & 0xF8) >> 3),
            160 => ((x & 0xF8) << 1) + ((x & 0xF8) >> 1) + ((y & 0xF8) >> 3),
            192 => ((x & 0xF8) << 1) + (x & 0xF8) + ((y & 0xF8) >> 3),
            256 => ((y & 0x80) << 2) + ((x & 0x80) << 1) + ((y & 0x78) << 1) + ((x & 0x78) >> 3),
            _ => return None,
        };
        Some((cn * bytes_per_tile, row_in_tile, bit))
    }

    fn dbg_superfx_direct_sample_impl(
        &self,
        x: u16,
        y: u16,
        apply_offsets: bool,
    ) -> Option<SuperfxDirectSample> {
        let sfx_x = i32::from(x)
            + if apply_offsets {
                self.superfx_direct_x_offset()
            } else {
                0
            };
        let sfx_y = i32::from(y)
            + if apply_offsets {
                self.superfx_direct_y_offset()
            } else {
                0
            };
        let sfx_height = if self.superfx_direct_height != 0 {
            self.superfx_direct_height
        } else {
            192
        };
        if !(0..256).contains(&sfx_x) || !(0..i32::from(sfx_height)).contains(&sfx_y) {
            return None;
        }
        let mut sx = sfx_x as usize;
        let mut sy = sfx_y as usize;
        if superfx_direct_swap_xy() {
            std::mem::swap(&mut sx, &mut sy);
        }
        let rel_x = (sx & 7) as u8;
        let (tile_base, row_in_tile, _) = self.superfx_direct_pixel_addr(sx, sy)?;
        let row01 = tile_base + row_in_tile * 2;
        let row23 = tile_base + 16 + row_in_tile * 2;
        if row23 + 1 >= self.superfx_direct_buffer.len() {
            return None;
        }
        let p0 = self.superfx_direct_buffer[row01];
        let p1 = self.superfx_direct_buffer[row01 + 1];
        let p2 = self.superfx_direct_buffer[row23];
        let p3 = self.superfx_direct_buffer[row23 + 1];
        let msb_bit = 7 - rel_x;
        let lsb_bit = rel_x;
        let msb_color = ((p0 >> msb_bit) & 1)
            | (((p1 >> msb_bit) & 1) << 1)
            | (((p2 >> msb_bit) & 1) << 2)
            | (((p3 >> msb_bit) & 1) << 3);
        let lsb_color = ((p0 >> lsb_bit) & 1)
            | (((p1 >> lsb_bit) & 1) << 1)
            | (((p2 >> lsb_bit) & 1) << 2)
            | (((p3 >> lsb_bit) & 1) << 3);
        Some((
            sx,
            sy,
            tile_base,
            row_in_tile,
            p0,
            p1,
            p2,
            p3,
            msb_color,
            lsb_color,
        ))
    }

    pub(crate) fn dbg_superfx_direct_sample(&self, x: u16, y: u16) -> Option<SuperfxDirectSample> {
        self.dbg_superfx_direct_sample_impl(x, y, true)
    }

    pub(crate) fn dbg_superfx_direct_sample_unoffset(
        &self,
        x: u16,
        y: u16,
    ) -> Option<SuperfxDirectSample> {
        self.dbg_superfx_direct_sample_impl(x, y, false)
    }

    pub(crate) fn dbg_superfx_direct_pixel_bounds(
        &self,
    ) -> Option<(usize, usize, usize, usize, usize)> {
        if !matches!(self.superfx_direct_bpp, 2 | 4 | 8) || self.superfx_direct_buffer.is_empty() {
            return None;
        }
        let height = if self.superfx_direct_height != 0 {
            self.superfx_direct_height as usize
        } else {
            192
        };
        if height == 0 {
            return None;
        }
        let mut min_x = usize::MAX;
        let mut min_y = usize::MAX;
        let mut max_x = 0usize;
        let mut max_y = 0usize;
        let mut count = 0usize;
        for y in 0..height.min(256) {
            for x in 0..256usize {
                let rel_x = (x & 7) as u8;
                if self.sample_superfx_buffer_pixel(x, y, rel_x) != 0 {
                    count += 1;
                    min_x = min_x.min(x);
                    min_y = min_y.min(y);
                    max_x = max_x.max(x);
                    max_y = max_y.max(y);
                }
            }
        }
        if count == 0 {
            None
        } else {
            Some((min_x, min_y, max_x, max_y, count))
        }
    }

    fn sample_superfx_buffer_pixel(&self, x: usize, y: usize, rel_x: u8) -> u8 {
        let Some((tile_base, row_in_tile, _)) = self.superfx_direct_pixel_addr(x, y) else {
            return 0;
        };
        let bpp = self.superfx_direct_bpp as usize;
        if !matches!(bpp, 2 | 4 | 8) {
            return 0;
        }
        let bit = if superfx_direct_lsb_first() {
            rel_x
        } else {
            7 - rel_x
        };
        let row_base = tile_base + row_in_tile * 2;
        let mut color = 0u8;
        for plane in 0..bpp {
            let byte_addr = row_base + ((plane >> 1) << 4) + (plane & 1);
            if byte_addr >= self.superfx_direct_buffer.len() {
                return 0;
            }
            let byte = self.superfx_direct_buffer[byte_addr];
            color |= ((byte >> bit) & 1) << plane;
        }
        if let Some(cfg) = trace_sample_dot_config() {
            if self.frame == cfg.frame && x as u16 == cfg.x && y as u16 == cfg.y {
                let mut plane_bytes = [0u8; 8];
                for (plane, slot) in plane_bytes.iter_mut().enumerate().take(bpp.min(8)) {
                    let byte_addr = row_base + ((plane >> 1) << 4) + (plane & 1);
                    *slot = self.superfx_direct_buffer[byte_addr];
                }
                println!(
                    "[TRACE_SAMPLE_DOT][SFX-DIRECT-BYTES] frame={} x={} y={} tile_base=0x{:04X} row_in_tile={} row_base=0x{:04X} bpp={} bytes=[{:02X},{:02X},{:02X},{:02X},{:02X},{:02X},{:02X},{:02X}] bit={} color_index=0x{:02X}",
                    self.frame,
                    x,
                    y,
                    tile_base,
                    row_in_tile,
                    row_base,
                    bpp,
                    plane_bytes[0],
                    plane_bytes[1],
                    plane_bytes[2],
                    plane_bytes[3],
                    plane_bytes[4],
                    plane_bytes[5],
                    plane_bytes[6],
                    plane_bytes[7],
                    bit,
                    color
                );
            }
        }
        color
    }

    /// Read a 4bpp pixel directly from VRAM without caching.
    fn sample_bg_direct(&self, tile_addr: u16, rel_y: u8, rel_x: u8) -> u8 {
        let row01 = (tile_addr.wrapping_add(rel_y as u16)) & 0x7FFF;
        let row23 = (tile_addr.wrapping_add(8).wrapping_add(rel_y as u16)) & 0x7FFF;
        let p0_addr = (row01 as usize) * 2;
        let p1_addr = p0_addr + 1;
        let p2_addr = (row23 as usize) * 2;
        let p3_addr = p2_addr + 1;
        if p3_addr >= self.vram.len() {
            return 0;
        }
        let p0 = self.vram[p0_addr];
        let p1 = self.vram[p1_addr];
        let p2 = self.vram[p2_addr];
        let p3 = self.vram[p3_addr];
        let bit = 7 - rel_x;
        ((p0 >> bit) & 1)
            | (((p1 >> bit) & 1) << 1)
            | (((p2 >> bit) & 1) << 2)
            | (((p3 >> bit) & 1) << 3)
    }
}
