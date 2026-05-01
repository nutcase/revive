use crate::ppu::{BgMapCache, BgRowCache, Ppu};

impl Ppu {
    pub(super) fn read_bg_tilemap_entry_word_at_pixel(
        &self,
        bg_num: u8,
        pixel_x: u16,
        pixel_y: u16,
    ) -> u16 {
        let ss = self.bg_screen_size[bg_num as usize];
        let tile_shift = if self.bg_tile_16[bg_num as usize] {
            4
        } else {
            3
        };
        let width_px = 256u32 << (tile_shift - 3) << if ss == 1 || ss == 3 { 1 } else { 0 };
        let height_px = 256u32 << (tile_shift - 3) << if ss == 2 || ss == 3 { 1 } else { 0 };
        let hmask = width_px.saturating_sub(1);
        let vmask = height_px.saturating_sub(1);
        let wrapped_x = (u32::from(pixel_x) & hmask) >> tile_shift;
        let wrapped_y = (u32::from(pixel_y) & vmask) >> tile_shift;
        self.read_bg_tilemap_entry_word(bg_num, wrapped_x as u16, wrapped_y as u16)
    }

    // Read a tilemap entry word for BG1..BG4 at the given (tile_x, tile_y).
    // bg_num is 0..3 for BG1..BG4.
    #[inline]
    pub(crate) fn read_bg_tilemap_entry_word(&self, bg_num: u8, tile_x: u16, tile_y: u16) -> u16 {
        let ss = self.bg_screen_size[bg_num as usize];
        let width_screens = if ss == 1 || ss == 3 { 2 } else { 1 } as u32;

        let tilemap_base_word = match bg_num {
            0 => self.bg1_tilemap_base as u32,
            1 => self.bg2_tilemap_base as u32,
            2 => self.bg3_tilemap_base as u32,
            _ => self.bg4_tilemap_base as u32,
        };

        let map_tx = (tile_x % 32) as u32;
        let map_ty = (tile_y % 32) as u32;
        let scx = (tile_x / 32) as u32;
        let scy = (tile_y / 32) as u32;
        let quadrant = scx + scy * width_screens;

        let word_addr = tilemap_base_word
            .saturating_add(quadrant * 0x400)
            .saturating_add(map_ty * 32 + map_tx)
            & 0x7FFF;
        let addr = (word_addr * 2) as usize;
        if addr + 1 >= self.vram.len() {
            return 0;
        }
        let lo = self.vram[addr];
        let hi = self.vram[addr + 1];
        ((hi as u16) << 8) | (lo as u16)
    }

    pub(crate) fn invalidate_bg_caches(&mut self) {
        if !self.bg_cache_dirty {
            return;
        }
        for cache in &mut self.bg_map_cache {
            cache.valid = false;
        }
        for cache in &mut self.bg_row_cache {
            cache.valid = false;
        }
        self.bg_cache_dirty = false;
    }

    pub(crate) fn get_bg_map_entry_cached(&mut self, bg_num: u8, tile_x: u16, tile_y: u16) -> u16 {
        if self.bg_cache_dirty {
            self.invalidate_bg_caches();
        }
        let idx = bg_num as usize;
        if idx >= self.bg_map_cache.len() {
            return 0;
        }
        let res = {
            let cache = &self.bg_map_cache[idx];
            cache.valid && cache.tile_x == tile_x && cache.tile_y == tile_y
        };
        if res {
            return self.bg_map_cache[idx].map_entry;
        }
        let entry = self.read_bg_tilemap_entry_word(bg_num, tile_x, tile_y);
        self.bg_map_cache[idx] = BgMapCache {
            valid: true,
            tile_x,
            tile_y,
            map_entry: entry,
        };
        entry
    }

    pub(crate) fn sample_bg_cached(
        &mut self,
        bg_num: u8,
        tile_addr: u16,
        rel_y: u8,
        rel_x: u8,
        bpp: u8,
    ) -> u8 {
        if self.bg_cache_dirty {
            self.invalidate_bg_caches();
        }
        let idx = bg_num as usize;
        if idx >= self.bg_row_cache.len() {
            return 0;
        }
        let cache = &mut self.bg_row_cache[idx];
        if !(cache.valid
            && cache.tile_addr == tile_addr
            && cache.rel_y == rel_y
            && cache.bpp == bpp)
        {
            let mut row = [0u8; 8];
            match bpp {
                2 => {
                    let row_word = tile_addr.wrapping_add(rel_y as u16) & 0x7FFF;
                    let plane0_addr = (row_word as usize) * 2;
                    let plane1_addr = plane0_addr + 1;
                    if plane1_addr < self.vram.len() {
                        let plane0 = self.vram[plane0_addr];
                        let plane1 = self.vram[plane1_addr];
                        for x in 0..8u8 {
                            let bit = 7 - x;
                            let c = (((plane1 >> bit) & 1) << 1) | ((plane0 >> bit) & 1);
                            row[x as usize] = c;
                        }
                    }
                }
                4 => {
                    let row01_word = tile_addr.wrapping_add(rel_y as u16) & 0x7FFF;
                    let row23_word = tile_addr.wrapping_add(8).wrapping_add(rel_y as u16) & 0x7FFF;
                    let plane0_addr = (row01_word as usize) * 2;
                    let plane1_addr = plane0_addr + 1;
                    let plane2_addr = (row23_word as usize) * 2;
                    let plane3_addr = plane2_addr + 1;
                    if plane3_addr < self.vram.len() {
                        let p0 = self.vram[plane0_addr];
                        let p1 = self.vram[plane1_addr];
                        let p2 = self.vram[plane2_addr];
                        let p3 = self.vram[plane3_addr];
                        for x in 0..8u8 {
                            let bit = 7 - x;
                            let c = (((p3 >> bit) & 1) << 3)
                                | (((p2 >> bit) & 1) << 2)
                                | (((p1 >> bit) & 1) << 1)
                                | ((p0 >> bit) & 1);
                            row[x as usize] = c;
                        }
                    }
                }
                8 => {
                    let row01_word = tile_addr.wrapping_add(rel_y as u16) & 0x7FFF;
                    let row23_word = tile_addr.wrapping_add(8).wrapping_add(rel_y as u16) & 0x7FFF;
                    let row45_word = tile_addr.wrapping_add(16).wrapping_add(rel_y as u16) & 0x7FFF;
                    let row67_word = tile_addr.wrapping_add(24).wrapping_add(rel_y as u16) & 0x7FFF;
                    let plane0_addr = (row01_word as usize) * 2;
                    let plane1_addr = plane0_addr + 1;
                    let plane2_addr = (row23_word as usize) * 2;
                    let plane3_addr = plane2_addr + 1;
                    let plane4_addr = (row45_word as usize) * 2;
                    let plane5_addr = plane4_addr + 1;
                    let plane6_addr = (row67_word as usize) * 2;
                    let plane7_addr = plane6_addr + 1;
                    if plane7_addr < self.vram.len() {
                        let p0 = self.vram[plane0_addr];
                        let p1 = self.vram[plane1_addr];
                        let p2 = self.vram[plane2_addr];
                        let p3 = self.vram[plane3_addr];
                        let p4 = self.vram[plane4_addr];
                        let p5 = self.vram[plane5_addr];
                        let p6 = self.vram[plane6_addr];
                        let p7 = self.vram[plane7_addr];
                        for x in 0..8u8 {
                            let bit = 7 - x;
                            let mut c = 0u8;
                            c |= (p0 >> bit) & 1;
                            c |= ((p1 >> bit) & 1) << 1;
                            c |= ((p2 >> bit) & 1) << 2;
                            c |= ((p3 >> bit) & 1) << 3;
                            c |= ((p4 >> bit) & 1) << 4;
                            c |= ((p5 >> bit) & 1) << 5;
                            c |= ((p6 >> bit) & 1) << 6;
                            c |= ((p7 >> bit) & 1) << 7;
                            row[x as usize] = c;
                        }
                    }
                }
                _ => {}
            }
            *cache = BgRowCache {
                valid: true,
                tile_addr,
                rel_y,
                bpp,
                row,
            };
        }
        cache.row.get(rel_x as usize).copied().unwrap_or(0)
    }
}
