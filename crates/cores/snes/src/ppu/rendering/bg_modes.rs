use crate::ppu::{trace_sample_dot_config, Ppu};

impl Ppu {
    pub(crate) fn render_bg_mode0_with_priority(
        &mut self,
        x: u16,
        y: u16,
        bg_num: u8,
    ) -> (u32, u8) {
        self.render_bg_mode0(x, y, bg_num)
    }
    pub(crate) fn render_bg_4bpp_with_priority(&mut self, x: u16, y: u16, bg_num: u8) -> (u32, u8) {
        self.render_bg_4bpp(x, y, bg_num)
    }
    pub(crate) fn render_bg_8bpp_with_priority(&mut self, x: u16, y: u16, bg_num: u8) -> (u32, u8) {
        self.render_bg_8bpp(x, y, bg_num)
    }
    pub(crate) fn render_bg_mode2_with_priority(
        &mut self,
        x: u16,
        y: u16,
        bg_num: u8,
    ) -> (u32, u8) {
        self.render_bg_mode2(x, y, bg_num)
    }
    pub(crate) fn render_bg_mode5_with_priority(
        &mut self,
        x: u16,
        y: u16,
        bg_num: u8,
    ) -> (u32, u8) {
        self.render_bg_mode5(x, y, bg_num, true)
    }
    pub(crate) fn render_bg_mode6_with_priority(
        &mut self,
        x: u16,
        y: u16,
        bg_num: u8,
    ) -> (u32, u8) {
        self.render_bg_mode6(x, y, bg_num, true)
    }
    #[allow(dead_code)]
    #[inline]
    pub(crate) fn sample_tile_2bpp(&self, tile_base: u16, tile_id: u16, px: u8, py: u8) -> u8 {
        // 2bpp tile = 8 words (16 bytes)
        let tile_addr = tile_base.wrapping_add(tile_id.wrapping_mul(8)) & 0x7FFF;
        let row_word = tile_addr.wrapping_add(py as u16) & 0x7FFF;
        let plane0_addr = (row_word as usize) * 2;
        let plane1_addr = plane0_addr + 1;
        if plane1_addr >= self.vram.len() {
            return 0;
        }
        let plane0 = self.vram[plane0_addr];
        let plane1 = self.vram[plane1_addr];
        let bit = 7 - px;
        (((plane1 >> bit) & 1) << 1) | ((plane0 >> bit) & 1)
    }
    #[allow(dead_code)]
    #[inline]
    pub(crate) fn sample_tile_4bpp(&self, tile_base: u16, tile_id: u16, px: u8, py: u8) -> u8 {
        // 4bpp tile = 16 words (32 bytes)
        let tile_addr = (tile_base.wrapping_add(tile_id.wrapping_mul(16))) & 0x7FFF;
        let row01_word = (tile_addr.wrapping_add(py as u16)) & 0x7FFF;
        let row23_word = (tile_addr.wrapping_add(8).wrapping_add(py as u16)) & 0x7FFF;
        let plane0_addr = (row01_word as usize) * 2;
        let plane1_addr = plane0_addr + 1;
        let plane2_addr = (row23_word as usize) * 2;
        let plane3_addr = plane2_addr + 1;
        if plane3_addr >= self.vram.len() {
            return 0;
        }
        let plane0 = self.vram[plane0_addr];
        let plane1 = self.vram[plane1_addr];
        let plane2 = self.vram[plane2_addr];
        let plane3 = self.vram[plane3_addr];
        let bit = 7 - px;
        (((plane3 >> bit) & 1) << 3)
            | (((plane2 >> bit) & 1) << 2)
            | (((plane1 >> bit) & 1) << 1)
            | ((plane0 >> bit) & 1)
    }
    pub(crate) fn render_bg_mode0(&mut self, x: u16, y: u16, bg_num: u8) -> (u32, u8) {
        // Debug: Check if tilemap base addresses are set
        static mut BG_DEBUG_COUNT: u32 = 0;
        unsafe {
            if BG_DEBUG_COUNT < 5 && x == 0 && y == 1 && crate::debug_flags::boot_verbose() {
                let tilemap_base = match bg_num {
                    0 => self.bg1_tilemap_base,
                    1 => self.bg2_tilemap_base,
                    2 => self.bg3_tilemap_base,
                    3 => self.bg4_tilemap_base,
                    _ => 0,
                };
                let tile_base = match bg_num {
                    0 => self.bg1_tile_base,
                    1 => self.bg2_tile_base,
                    2 => self.bg3_tile_base,
                    3 => self.bg4_tile_base,
                    _ => 0,
                };
                if tilemap_base != 0 || tile_base != 0 {
                    BG_DEBUG_COUNT += 1;
                    println!(
                        "🎮 BG{} RENDER[{}]: tilemap_base=0x{:04X}, tile_base=0x{:04X}",
                        bg_num, BG_DEBUG_COUNT, tilemap_base, tile_base
                    );
                }
            }
        }

        let tile_16 = self.bg_tile_16[bg_num as usize];
        let tile_px = if tile_16 { 16 } else { 8 } as u16;
        let ss = self.bg_screen_size[bg_num as usize];
        let width_tiles = if ss == 1 || ss == 3 { 64 } else { 32 } as u16;
        let height_tiles = if ss == 2 || ss == 3 { 64 } else { 32 } as u16;
        let wrap_x = width_tiles * tile_px;
        let wrap_y = height_tiles * tile_px;

        let y_line = self.bg_interlace_y(y);
        let (mosaic_x, mosaic_y_base) = self.apply_mosaic(x, y, bg_num);
        let mosaic_y_line = if y_line == y {
            mosaic_y_base
        } else {
            self.apply_mosaic(x, y_line, bg_num).1
        };
        let (scroll_x, scroll_y) = match bg_num {
            0 => (self.bg1_hscroll, self.bg1_vscroll),
            1 => (self.bg2_hscroll, self.bg2_vscroll),
            2 => (self.bg3_hscroll, self.bg3_vscroll),
            3 => (self.bg4_hscroll, self.bg4_vscroll),
            _ => (0, 0),
        };
        let bg_x = (mosaic_x + scroll_x) % wrap_x;
        let bg_y_tile = (mosaic_y_base + scroll_y) % wrap_y;
        let bg_y_line = (mosaic_y_line + scroll_y) % wrap_y;

        let tile_x = bg_x / tile_px;
        let tile_y = bg_y_tile / tile_px;

        // Debug output disabled for performance

        let map_entry = self.get_bg_map_entry_cached(bg_num, tile_x, tile_y);

        // Debug and validate tilemap entries
        static mut TILEMAP_FOUND_COUNT: u32 = 0;
        static mut INVALID_TILEMAP_COUNT: u32 = 0;
        unsafe {
            if map_entry != 0 {
                let tile_id_raw = map_entry & 0x03FF;
                if TILEMAP_FOUND_COUNT < 20 && crate::debug_flags::boot_verbose() {
                    TILEMAP_FOUND_COUNT += 1;
                    println!(
                        "🗺️  TILEMAP[{}]: BG{} screen({},{}) bg({},{}) tile({},{}) entry=0x{:04X} tile_id={}",
                        TILEMAP_FOUND_COUNT,
                        bg_num,
                        x,
                        y,
                        bg_x,
                        bg_y_tile,
                        tile_x,
                        tile_y,
                        map_entry,
                        tile_id_raw
                    );
                }
            } else if TILEMAP_FOUND_COUNT == 0
                && INVALID_TILEMAP_COUNT < 5
                && crate::debug_flags::boot_verbose()
            {
                INVALID_TILEMAP_COUNT += 1;
                println!(
                    "⚠️  EMPTY TILEMAP[{}]: BG{} at ({},{}) entry=0x{:04X}",
                    INVALID_TILEMAP_COUNT, bg_num, x, y, map_entry
                );
            }
        }

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

        let tile_base = match bg_num {
            0 => self.bg1_tile_base,
            1 => self.bg2_tile_base,
            2 => self.bg3_tile_base,
            3 => self.bg4_tile_base,
            _ => 0,
        };

        // tile_base is in VRAM words (from BGxNBA registers)
        // 2bpp tile = 16 bytes = 8 words
        let tile_addr = tile_base.wrapping_add(tile_id.wrapping_mul(8)) & 0x7FFF;

        // Debug problematic tile addresses
        static mut BAD_ADDR_COUNT: u32 = 0;
        unsafe {
            if crate::debug_flags::debug_suspicious_tile() && (tile_base == 0 || tile_id > 1023) {
                BAD_ADDR_COUNT += 1;
                if BAD_ADDR_COUNT <= 3 && !crate::debug_flags::quiet() {
                    println!("⚠️ SUSPICIOUS TILE[{}]: BG{} tile_base=0x{:04X}, tile_id={}, addr=0x{:04X}",
                            BAD_ADDR_COUNT, bg_num, tile_base, tile_id, tile_addr);
                }
            }
        }
        let color_index = self.sample_bg_cached(bg_num, tile_addr, rel_y, rel_x, 2);

        // Debug first few non-zero pixels found
        static mut PIXEL_FOUND_COUNT: u32 = 0;
        if color_index != 0 {
            let palette_idx = self.get_bg_palette_index(palette, color_index, 2);
            let final_color = self.cgram_to_rgb(palette_idx);

            unsafe {
                if crate::debug_flags::debug_pixel_found()
                    && PIXEL_FOUND_COUNT < 5
                    && !crate::debug_flags::quiet()
                {
                    PIXEL_FOUND_COUNT += 1;
                    println!("🎯 PIXEL FOUND[{}]: BG{} at ({},{}) color_index={}, palette={}, palette_index={}",
                            PIXEL_FOUND_COUNT, bg_num, x, y, color_index, palette, palette_idx);
                    println!("   Final color: 0x{:08X}", final_color);
                }
            }
        }

        if color_index == 0 {
            return (0, 0);
        }
        // Mode 0 uses a dedicated CGRAM range per BG:
        // - BG1: palettes 0..7   (CGRAM 0..31)
        // - BG2: palettes 8..15  (CGRAM 32..63)
        // - BG3: palettes 16..23 (CGRAM 64..95)
        // - BG4: palettes 24..31 (CGRAM 96..127)
        //
        // For other modes, BG palettes share the lower CGRAM region (0..127).
        let palette_index = if self.bg_mode == 0 {
            let bg_off = (bg_num as u16).saturating_mul(32);
            let idx = bg_off + (palette as u16) * 4 + (color_index as u16);
            idx.min(127) as u8
        } else {
            self.get_bg_palette_index(palette, color_index, 2)
        };
        let color = self.cgram_to_rgb(palette_index);

        // Use palette result strictly as-is (no heuristic overrides)

        let priority_value = if priority { 1 } else { 0 };
        (color, priority_value)
    }
    #[allow(dead_code)]
    pub(crate) fn render_bg_mode1(&mut self, x: u16, y: u16, bg_num: u8) -> (u32, u8) {
        // Mode 1: BG1/BG2は4bpp、BG3は2bpp
        if bg_num <= 1 {
            // 4bpp描画
            self.render_bg_4bpp(x, y, bg_num)
        } else {
            // 2bpp描画
            self.render_bg_mode0(x, y, bg_num)
        }
    }
    pub(crate) fn render_bg_4bpp(&mut self, x: u16, y: u16, bg_num: u8) -> (u32, u8) {
        if crate::debug_flags::boot_verbose() {
            static mut DEBUG_FUNCTION_COUNT: u32 = 0;
            unsafe {
                DEBUG_FUNCTION_COUNT += 1;
                if DEBUG_FUNCTION_COUNT <= 5 && x < 32 && y < 32 {
                    println!(
                        "DBG: render_bg_4bpp BG{} at ({},{}), map_base=0x{:04X}",
                        bg_num,
                        x,
                        y,
                        match bg_num {
                            0 => self.bg1_tilemap_base,
                            1 => self.bg2_tilemap_base,
                            _ => 0,
                        }
                    );
                }
            }
        }

        let y_line = self.bg_interlace_y(y);
        let (mosaic_x, mosaic_y_base) = self.apply_mosaic(x, y, bg_num);
        let mosaic_y_line = if y_line == y {
            mosaic_y_base
        } else {
            self.apply_mosaic(x, y_line, bg_num).1
        };
        let (scroll_x, scroll_y) = match bg_num {
            0 => (self.bg1_hscroll, self.bg1_vscroll),
            1 => (self.bg2_hscroll, self.bg2_vscroll),
            2 => (self.bg3_hscroll, self.bg3_vscroll),
            3 => (self.bg4_hscroll, self.bg4_vscroll),
            _ => (0, 0),
        };
        self.render_bg_4bpp_impl(
            bg_num,
            mosaic_x,
            mosaic_y_base,
            mosaic_y_line,
            scroll_x,
            scroll_y,
        )
    }
    pub(crate) fn render_bg_4bpp_impl(
        &mut self,
        bg_num: u8,
        mosaic_x: u16,
        mosaic_y_base: u16,
        mosaic_y_line: u16,
        scroll_x: u16,
        scroll_y: u16,
    ) -> (u32, u8) {
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

        let tile_base = match bg_num {
            0 => self.bg1_tile_base,
            1 => self.bg2_tile_base,
            2 => self.bg3_tile_base,
            3 => self.bg4_tile_base,
            _ => 0,
        };
        // tile_base is in VRAM words (from BGxNBA registers)
        // 4bpp tile = 32 bytes = 16 words
        let tile_addr = (tile_base.wrapping_add(tile_id.wrapping_mul(16))) & 0x7FFF; // Mask to VRAM range

        if crate::debug_flags::boot_verbose() {
            static mut DEBUG_TILE_ADDR_COUNT: u32 = 0;
            unsafe {
                DEBUG_TILE_ADDR_COUNT += 1;
                if DEBUG_TILE_ADDR_COUNT <= 3 {
                    println!(
                        "DBG: BG{} tile_addr=0x{:04X} (base=0x{:04X}, id=0x{:03X})",
                        bg_num, tile_addr, tile_base, tile_id
                    );
                }
            }
        }

        let color_index = self.sample_bg_cached(bg_num, tile_addr, rel_y, rel_x, 4);

        if let Some(cfg) = trace_sample_dot_config() {
            if self.frame == cfg.frame && mosaic_x == cfg.x && mosaic_y_line == cfg.y {
                let palette_index = if color_index == 0 {
                    0
                } else {
                    self.get_bg_palette_index(palette, color_index, 4)
                };
                println!(
                    "[TRACE_SAMPLE_DOT][BG{}-4BPP] frame={} x={} y={} bg_xy=({}, {}) tile_xy=({}, {}) entry=0x{:04X} tile16={} tile_id=0x{:03X} tile_base=0x{:04X} tile_addr=0x{:04X} rel=({}, {}) color_index=0x{:02X} palette={} palette_index=0x{:02X}",
                    bg_num + 1,
                    self.frame,
                    cfg.x,
                    cfg.y,
                    bg_x,
                    bg_y_line,
                    tile_x,
                    tile_y,
                    map_entry,
                    tile_16 as u8,
                    tile_id,
                    tile_base,
                    tile_addr,
                    rel_x,
                    rel_y,
                    color_index,
                    palette,
                    palette_index
                );
            }
        }

        if color_index == 0 {
            return (0, 0);
        }
        let palette_index = self.get_bg_palette_index(palette, color_index, 4);

        if crate::debug_flags::boot_verbose() {
            static mut CGRAM_DEBUG_COUNT: u32 = 0;
            unsafe {
                CGRAM_DEBUG_COUNT += 1;
                if CGRAM_DEBUG_COUNT <= 10 && (palette_index as usize) < 32 {
                    println!("CGRAM[{}] sample", palette_index);
                }
            }
        }

        // Use CGRAM color as-is (no special fallbacks)
        let color = self.cgram_to_rgb(palette_index);

        let priority_value = if priority { 1 } else { 0 };
        (color, priority_value)
    }
    #[allow(dead_code)]
    pub(crate) fn render_bg_mode4(&mut self, x: u16, y: u16, bg_num: u8) -> (u32, u8) {
        // Mode 4: BG1は8bpp、BG2は2bpp
        if bg_num == 0 {
            // BG1: 8bpp描画（256色）
            self.render_bg_8bpp(x, y, bg_num)
        } else {
            // BG2: 2bpp描画
            self.render_bg_mode0(x, y, bg_num)
        }
    }
    pub(crate) fn render_bg_8bpp(&mut self, x: u16, y: u16, bg_num: u8) -> (u32, u8) {
        let tile_16 = self.bg_tile_16[bg_num as usize];
        let tile_px = if tile_16 { 16 } else { 8 } as u16;
        let ss = self.bg_screen_size[bg_num as usize];
        let width_tiles = if ss == 1 || ss == 3 { 64 } else { 32 } as u16;
        let height_tiles = if ss == 2 || ss == 3 { 64 } else { 32 } as u16;
        let wrap_x = width_tiles * tile_px;
        let wrap_y = height_tiles * tile_px;

        let y_line = self.bg_interlace_y(y);
        let (mosaic_x, mosaic_y_base) = self.apply_mosaic(x, y, bg_num);
        let mosaic_y_line = if y_line == y {
            mosaic_y_base
        } else {
            self.apply_mosaic(x, y_line, bg_num).1
        };
        let (scroll_x, scroll_y) = match bg_num {
            0 => (self.bg1_hscroll, self.bg1_vscroll),
            1 => (self.bg2_hscroll, self.bg2_vscroll),
            2 => (self.bg3_hscroll, self.bg3_vscroll),
            3 => (self.bg4_hscroll, self.bg4_vscroll),
            _ => (0, 0),
        };
        let bg_x = (mosaic_x.wrapping_add(scroll_x)) % wrap_x;
        let bg_y_tile = (mosaic_y_base.wrapping_add(scroll_y)) % wrap_y;
        let bg_y_line = (mosaic_y_line.wrapping_add(scroll_y)) % wrap_y;

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

        let tile_base = match bg_num {
            0 => self.bg1_tile_base,
            1 => self.bg2_tile_base,
            2 => self.bg3_tile_base,
            3 => self.bg4_tile_base,
            _ => 0,
        };
        let tile_addr = tile_base.wrapping_add(tile_id.wrapping_mul(32)) & 0x7FFF;
        let color_index = self.sample_bg_cached(bg_num, tile_addr, rel_y, rel_x, 8);

        if color_index == 0 {
            return (0, 0);
        }

        // Direct color mode (CGWSEL bit0) for 256-color BGs (Modes 3/4/7, BG1 only).
        let use_direct_color =
            bg_num == 0 && (self.cgwsel & 0x01) != 0 && matches!(self.bg_mode, 3 | 4 | 7);
        let color = if use_direct_color {
            self.direct_color_to_rgb(palette, color_index)
        } else {
            let palette_index = self.get_bg_palette_index(0, color_index, 8);
            self.cgram_to_rgb(palette_index)
        };
        let priority_value = if priority { 1 } else { 0 };
        (color, priority_value)
    }
    #[inline]
    pub(crate) fn direct_color_to_rgb(&self, palette: u8, pixel: u8) -> u32 {
        // Direct Color (MMIO $2130 bit0):
        // - Pixel value is interpreted as BBGGGRRR (8bpp character data).
        // - Tilemap palette bits ppp are interpreted as bgr (one extra bit per component).
        // Final RGB555: 0bbbbbgggggrrrrr, where LSB of each component is 0 (RGB443).
        let r5 = (((pixel & 0x07) as u32) << 2) | (((palette & 0x01) as u32) << 1);
        let g5 = ((((pixel >> 3) & 0x07) as u32) << 2) | ((((palette >> 1) & 0x01) as u32) << 1);
        let b5 = ((((pixel >> 6) & 0x03) as u32) << 3) | ((((palette >> 2) & 0x01) as u32) << 2);

        let r = (r5 << 3) | (r5 >> 2);
        let g = (g5 << 3) | (g5 >> 2);
        let b = (b5 << 3) | (b5 >> 2);
        0xFF000000 | (r << 16) | (g << 8) | b
    }
    pub(crate) fn render_bg_mode5(
        &mut self,
        x: u16,
        y: u16,
        bg_num: u8,
        is_main: bool,
    ) -> (u32, u8) {
        // Mode 5 (hi-res): BG tiles are effectively 16px wide by pairing tiles horizontally.
        // Background layers are de-interleaved between main/sub screens (even/odd columns).
        //
        // We keep a 256-wide framebuffer and treat the main screen as the even columns and
        // the sub screen as the odd columns. So BG sampling uses a doubled X coordinate with
        // a phase offset based on which screen we are rendering.
        if bg_num > 2 {
            return (0, 0);
        }

        let tile_base = match bg_num {
            0 => self.bg1_tile_base,
            1 => self.bg2_tile_base,
            2 => self.bg3_tile_base,
            _ => 0,
        };
        let ss = self.bg_screen_size[bg_num as usize];
        let width_tiles = if ss == 1 || ss == 3 { 64 } else { 32 } as u16;
        let height_tiles = if ss == 2 || ss == 3 { 64 } else { 32 } as u16;

        let tile_w: u16 = 16;
        let tile_h: u16 = if self.bg_tile_16[bg_num as usize] {
            16
        } else {
            8
        };
        let wrap_x = width_tiles * tile_w;
        let wrap_y = height_tiles * tile_h;

        let phase: u16 = if is_main { 0 } else { 1 };
        let x_hires = x.wrapping_mul(2).wrapping_add(phase);

        let (scroll_x, scroll_y) = match bg_num {
            0 => (self.bg1_hscroll, self.bg1_vscroll),
            1 => (self.bg2_hscroll, self.bg2_vscroll),
            2 => (self.bg3_hscroll, self.bg3_vscroll),
            _ => (0, 0),
        };
        let bg_x = (x_hires.wrapping_add(scroll_x)) % wrap_x;
        let mut y_eff = y;
        if self.bg_interlace_active() {
            y_eff = y_eff
                .saturating_mul(2)
                .saturating_add(self.interlace_field as u16);
        }
        let bg_y = (y_eff.wrapping_add(scroll_y)) % wrap_y;

        let tile_x = bg_x / tile_w;
        let tile_y = bg_y / tile_h;

        let entry = self.get_bg_map_entry_cached(bg_num, tile_x, tile_y);

        let mut tile_id = entry & 0x03FF;
        let palette = ((entry >> 10) & 0x07) as u8;
        let flip_x = (entry & 0x4000) != 0;
        let flip_y = (entry & 0x8000) != 0;
        let priority = (entry & 0x2000) != 0;

        let mut rel_x = (bg_x % tile_w) as u8; // 0..15 (even/odd depends on screen phase)
        let mut rel_y = (bg_y % tile_h) as u8; // 0..7 or 0..15
        if flip_x {
            rel_x = (tile_w as u8 - 1) - rel_x;
        }
        if flip_y {
            rel_y = (tile_h as u8 - 1) - rel_y;
        }

        // Select the paired tile horizontally, and optionally vertically (when tile_h=16).
        let sub_x = (rel_x / 8) as u16; // 0 or 1
        let sub_y = if tile_h == 16 { (rel_y / 8) as u16 } else { 0 };
        tile_id = tile_id
            .wrapping_add(sub_x)
            .wrapping_add(sub_y.wrapping_mul(16));
        rel_x %= 8;
        rel_y %= 8;

        let bpp = if bg_num == 1 { 2 } else { 4 };
        let tile_stride = if bpp == 2 { 8 } else { 16 };
        let tile_addr = tile_base.wrapping_add(tile_id.wrapping_mul(tile_stride)) & 0x7FFF;
        let color_index = self.sample_bg_cached(bg_num, tile_addr, rel_y, rel_x, bpp);
        if color_index == 0 {
            return (0, 0);
        }

        let bpp = if bg_num == 1 { 2 } else { 4 };
        let palette_index = self.get_bg_palette_index(palette, color_index, bpp);
        let color = self.cgram_to_rgb(palette_index);
        let priority_value = if priority { 1 } else { 0 };
        (color, priority_value)
    }
    pub(crate) fn render_bg_mode6(
        &mut self,
        x: u16,
        y: u16,
        bg_num: u8,
        is_main: bool,
    ) -> (u32, u8) {
        // Mode 6: BG1は4bpp（高解像度512x448）
        if bg_num != 0 {
            return (0, 0);
        }

        // Use the Mode 5 sampling rules for BG1 (16px wide tiles + main/sub phase),
        // but only BG1 is displayed in Mode 6.
        self.render_bg_mode5(x, y, 0, is_main)
    }
    pub(crate) fn apply_mosaic(&self, x: u16, y: u16, bg_num: u8) -> (u16, u16) {
        // 該当BGでモザイクが有効かチェック
        if !self.is_mosaic_enabled(bg_num) {
            return (x, y);
        }

        // モザイクブロックの左上の座標を計算
        let mosaic_x = (x / self.mosaic_size as u16) * self.mosaic_size as u16;
        let mosaic_y = (y / self.mosaic_size as u16) * self.mosaic_size as u16;

        (mosaic_x, mosaic_y)
    }
    pub(crate) fn is_mosaic_enabled(&self, bg_num: u8) -> bool {
        // BG別のモザイク有効フラグをチェック
        match bg_num {
            0 => self.bg_mosaic & 0x01 != 0, // BG1
            1 => self.bg_mosaic & 0x02 != 0, // BG2
            2 => self.bg_mosaic & 0x04 != 0, // BG3
            3 => self.bg_mosaic & 0x08 != 0, // BG4
            _ => false,
        }
    }
}
