use super::{trace_sample_dot_config, Ppu, SpriteData, SpriteSize};

impl Ppu {
    #[inline]
    pub(crate) fn sprite_x_signed(x_raw: u16) -> i16 {
        // OBJ X is 9-bit and treated as signed on the screen:
        // 0..255 => 0..255, 256..511 => -256..-1.
        let x9 = (x_raw & 0x01FF) as i16;
        if (x9 & 0x0100) != 0 {
            x9 - 512
        } else {
            x9
        }
    }

    #[inline]
    pub(crate) fn obj_interlace_active(&self) -> bool {
        self.interlace && self.obj_interlace
    }

    #[inline]
    pub(crate) fn obj_line_for_scanline(&self, scanline: u16) -> u16 {
        if self.obj_interlace_active() {
            (scanline << 1) | (self.interlace_field as u16)
        } else {
            scanline
        }
    }

    #[inline]
    pub(crate) fn obj_sprite_dy(&self, obj_line: u16, sprite_y: u8) -> u16 {
        if self.obj_interlace_active() {
            let sprite_y_line = (sprite_y as u16) << 1;
            obj_line.wrapping_sub(sprite_y_line) & 0x01FF
        } else {
            (obj_line as u8).wrapping_sub(sprite_y) as u16
        }
    }

    #[inline]
    pub(crate) fn obj_sprite_height_lines(&self, sprite_height: u16) -> u16 {
        // OBJ interlace shows alternate lines each field, so compare against the
        // original sprite height (not doubled). This makes pixels appear half-height
        // across a single field.
        sprite_height
    }

    #[inline]
    pub(crate) fn obj_sprite_rel_y(&self, dy_lines: u16) -> u8 {
        // For OBJ interlace, dy_lines already encodes even/odd lines per field.
        dy_lines as u8
    }

    #[inline]
    fn build_sprite_data(
        &self,
        x: u16,
        y: u8,
        tile: u16,
        palette: u8,
        priority: u8,
        flip_x: bool,
        flip_y: bool,
        size: SpriteSize,
        width: u8,
        height: u8,
        line_rel_y: u8,
    ) -> SpriteData {
        SpriteData {
            x,
            x_signed: Self::sprite_x_signed(x),
            y,
            tile,
            palette,
            priority,
            flip_x,
            flip_y,
            size,
            width,
            height,
            line_rel_y,
            line_tile_y: line_rel_y / 8,
            line_pixel_y: line_rel_y & 7,
        }
    }

    // 共通のスプライトピクセル取得（画面有効フラグを引数で渡す）
    #[inline]
    pub(crate) fn get_sprite_pixel_common(
        &self,
        x: u16,
        y: u16,
        enabled: bool,
        is_main: bool,
    ) -> (u32, u8, bool) {
        if !enabled {
            return (0, 0, true);
        }
        if self.line_sprites.is_empty() {
            return (0, 0, true);
        }
        if self.should_mask_sprite(x, is_main) {
            return (0, 0, true);
        }
        let x_i16 = x as i16;
        let sprites = &self.line_sprites;
        let drawable_sprite_limit = if self.sprite_draw_disabled && self.sprite_time_over {
            self.sprite_timeover_first_idx as usize
        } else {
            sprites.len()
        };

        let trace_dot = if let Some(cfg) = trace_sample_dot_config() {
            self.frame == cfg.frame && x == cfg.x && y == cfg.y
        } else {
            false
        };
        if trace_dot {
            println!(
                "[TRACE_SAMPLE_DOT][OBJ] frame={} x={} y={} enabled={} sprites={} obj_line={}",
                self.frame,
                x,
                y,
                enabled as u8,
                sprites.len(),
                self.obj_line_for_scanline(y)
            );
        }
        // スプライト同士の優先順位はOAM順（評価順）で決まる。
        // 優先度ビットはBGとの優先にのみ使用するので、ここでは順序を変えない。
        for (idx, sprite) in sprites.iter().take(drawable_sprite_limit).enumerate() {
            let sx = sprite.x_signed;
            if x_i16 < sx || x_i16 >= sx.saturating_add(sprite.width as i16) {
                continue;
            }

            // スプライト内相対座標→タイル/ピクセル座標
            let rel_x = (x_i16 - sx) as u8;
            let tile_x = rel_x / 8;
            let pixel_x = rel_x % 8;
            let color = self.render_sprite_tile(
                sprite,
                tile_x,
                sprite.line_tile_y,
                pixel_x,
                sprite.line_pixel_y,
            );
            if trace_dot {
                let tile_num =
                    self.calculate_sprite_tile_number(sprite, tile_x, sprite.line_tile_y);
                let tile_addr = self.sprite_tile_base_word_addr(tile_num);
                let row01_word = (tile_addr.wrapping_add(sprite.line_pixel_y as u16)) & 0x7FFF;
                let row23_word = (tile_addr
                    .wrapping_add(8)
                    .wrapping_add(sprite.line_pixel_y as u16))
                    & 0x7FFF;
                let plane0_addr = (row01_word as usize) * 2;
                let plane1_addr = plane0_addr + 1;
                let plane2_addr = (row23_word as usize) * 2;
                let plane3_addr = plane2_addr + 1;
                let plane0 = self.vram.get(plane0_addr).copied().unwrap_or(0);
                let plane1 = self.vram.get(plane1_addr).copied().unwrap_or(0);
                let plane2 = self.vram.get(plane2_addr).copied().unwrap_or(0);
                let plane3 = self.vram.get(plane3_addr).copied().unwrap_or(0);
                println!(
                        "[TRACE_SAMPLE_DOT][OBJ] idx={} pr={} pal={} size={:?} fx={} fy={} base_tile=0x{:03X} sx={} sy={} rel=({}, {}) tile=({}, {}) px=({}, {}) tile_num=0x{:03X} color=0x{:08X} tile_addr=0x{:04X} row01=0x{:04X} row23=0x{:04X} p0={:02X} p1={:02X} p2={:02X} p3={:02X}",
                        idx,
                        sprite.priority,
                        sprite.palette,
                        sprite.size,
                        sprite.flip_x as u8,
                        sprite.flip_y as u8,
                        sprite.tile,
                        sx,
                        sprite.y,
                        rel_x,
                        sprite.line_rel_y,
                        tile_x,
                        sprite.line_tile_y,
                        pixel_x,
                        sprite.line_pixel_y,
                        tile_num,
                        color,
                        tile_addr,
                        row01_word,
                        row23_word,
                        plane0,
                        plane1,
                        plane2,
                        plane3
                    );
            }
            if color != 0 {
                // OBJ color math is disabled for palettes 0-3 on real hardware.
                let math_allowed = sprite.palette >= 4;
                return (color, sprite.priority, math_allowed);
            }
        }
        (0, 0, true)
    }

    // メインスクリーン用スプライト
    pub(crate) fn get_sprite_pixel(&self, x: u16, y: u16) -> (u32, u8) {
        let enabled = (self.effective_main_screen_designation() & 0x10) != 0;
        let (color, pr, _math_allowed) = self.get_sprite_pixel_common(x, y, enabled, true);
        (color, pr)
    }

    // スキャンライン開始時のスプライト評価
    #[allow(dead_code)]
    pub(crate) fn evaluate_sprites_for_scanline(&mut self, scanline: u16) {
        self.sprites_on_line_count = 0;
        self.sprite_overflow = false;
        self.sprite_time_over = false;

        let mut sprite_time = 0u32;
        let mut tile_budget_used: u32 = 0; // ~34 tiles per scanline

        for step in 0..128 {
            let i = ((self.oam_eval_base as usize) + step) & 0x7F;
            let oam_offset = i * 4;
            if oam_offset + 3 >= self.oam.len() {
                break;
            }

            let sprite_y = self.oam[oam_offset + 1];
            let obj_line = self.obj_line_for_scanline(scanline);

            // 高位テーブルからサイズ情報を取得
            let high_table_offset = 0x200 + (i / 4);
            if high_table_offset >= self.oam.len() {
                break;
            }

            let high_table_byte = self.oam[high_table_offset];
            let bit_shift = (i % 4) * 2;
            let high_bits = (high_table_byte >> bit_shift) & 0x03;
            let size_bit = (high_bits & 0x02) != 0;
            let size = if size_bit {
                SpriteSize::Large
            } else {
                SpriteSize::Small
            };

            let (_, sprite_height) = self.get_sprite_pixel_size(&size);

            // このスプライトが現在のスキャンラインに表示されるかチェック
            let dy_lines = self.obj_sprite_dy(obj_line, sprite_y);
            let sprite_height_lines = self.obj_sprite_height_lines(sprite_height as u16);
            if dy_lines < sprite_height_lines {
                self.sprites_on_line_count += 1;

                // スプライト制限チェック
                if self.sprites_on_line_count > 32 {
                    self.sprite_overflow = true;
                    self.sprite_overflow_latched = true;
                    self.obj_overflow_lines = self.obj_overflow_lines.saturating_add(1);
                    break;
                }

                // タイル予算の概算消費（1ラインおよそ34タイル）
                let (sprite_w, _) = self.get_sprite_pixel_size(&size);
                let tiles_across = (sprite_w as u32).div_ceil(8); // 8px単位
                tile_budget_used = tile_budget_used.saturating_add(tiles_across);
                if tile_budget_used > 34 {
                    self.sprite_time_over = true;
                    self.sprite_time_over_latched = true;
                    self.obj_time_over_lines = self.obj_time_over_lines.saturating_add(1);
                    break;
                }

                // 処理時間シミュレーション
                sprite_time += match size {
                    SpriteSize::Small => 2,
                    SpriteSize::Large => 4,
                };

                // タイムオーバーチェック（概算）
                if sprite_time > 34 {
                    self.sprite_time_over = true;
                    self.sprite_time_over_latched = true;
                    self.obj_time_over_lines = self.obj_time_over_lines.saturating_add(1);
                    break;
                }
            }
        }
    }

    // スプライトステータス読み取り（デバッグ用）
    #[allow(dead_code)]
    pub(crate) fn get_sprite_status(&self) -> u8 {
        let mut status = 0u8;
        if self.sprite_overflow {
            status |= 0x40; // Sprite overflow flag
        }
        if self.sprite_time_over {
            status |= 0x80; // Sprite time over flag
        }
        status | (self.sprites_on_line_count & 0x3F)
    }

    // OAMデータからスプライト情報を解析
    // スキャンライン用スプライトキャッシュ（パフォーマンス向上）
    #[allow(dead_code)]
    pub(crate) fn get_cached_sprites_for_scanline(&self, y: u16) -> Vec<SpriteData> {
        let mut sprites = Vec::new();

        // 最大128個のスプライト
        for step in 0..128 {
            let i = ((self.oam_eval_base as usize) + step) & 0x7F;
            let oam_offset = i * 4;
            if oam_offset + 3 >= self.oam.len() {
                break;
            }

            // OAMの基本データ（4バイト/スプライト）
            let x_lo = self.oam[oam_offset] as u16;
            let sprite_y = self.oam[oam_offset + 1];
            let tile_lo = self.oam[oam_offset + 2] as u16;
            let attr = self.oam[oam_offset + 3];

            // Note: Y is 8-bit and wraps on hardware (e.g., 0xFE shows at top).

            // 高位テーブル（1ビット/スプライトを2つずつ）
            let high_table_offset = 0x200 + (i / 4);
            if high_table_offset >= self.oam.len() {
                break;
            }

            let high_table_byte = self.oam[high_table_offset];
            let bit_shift = (i % 4) * 2;
            let high_bits = (high_table_byte >> bit_shift) & 0x03;

            // X座標の最上位ビット
            let x = x_lo | (((high_bits & 0x01) as u16) << 8);

            // サイズビット
            let size_bit = (high_bits & 0x02) != 0;
            let size = if size_bit {
                SpriteSize::Large
            } else {
                SpriteSize::Small
            };

            // スプライトのサイズを取得
            let (sprite_width, sprite_height) = self.get_sprite_pixel_size(&size);

            let obj_line = self.obj_line_for_scanline(y);
            // このスプライトが現在のスキャンラインに表示されるかチェック
            let dy_lines = self.obj_sprite_dy(obj_line, sprite_y);
            let sprite_height_lines = self.obj_sprite_height_lines(sprite_height as u16);
            if dy_lines >= sprite_height_lines {
                continue;
            }

            // タイル番号（9ビット）
            let tile = tile_lo | (((attr & 0x01) as u16) << 8);

            // 属性ビット
            let palette = (attr >> 1) & 0x07;
            let priority = (attr >> 4) & 0x03;
            let flip_x = (attr & 0x40) != 0;
            let flip_y = (attr & 0x80) != 0;

            let line_rel_y = self.obj_sprite_rel_y(dy_lines);
            sprites.push(self.build_sprite_data(
                x,
                sprite_y,
                tile,
                palette,
                priority,
                flip_x,
                flip_y,
                size,
                sprite_width,
                sprite_height,
                line_rel_y,
            ));
        }

        sprites
    }

    #[allow(dead_code)]
    pub(crate) fn parse_sprites(&self) -> Vec<SpriteData> {
        let mut sprites = Vec::new();

        // 最大128個のスプライト
        for i in 0..128 {
            let oam_offset = i * 4;
            if oam_offset + 3 >= self.oam.len() {
                break;
            }

            // OAMの基本データ（4バイト/スプライト）
            let x_lo = self.oam[oam_offset] as u16;
            let y = self.oam[oam_offset + 1];
            let tile_lo = self.oam[oam_offset + 2] as u16;
            let attr = self.oam[oam_offset + 3];

            // 高位テーブル（1ビット/スプライトを2つずつ）
            let high_table_offset = 0x200 + (i / 4);
            if high_table_offset >= self.oam.len() {
                break;
            }

            let high_table_byte = self.oam[high_table_offset];
            let bit_shift = (i % 4) * 2;
            let high_bits = (high_table_byte >> bit_shift) & 0x03;

            // X座標の最上位ビット
            let x = x_lo | (((high_bits & 0x01) as u16) << 8);

            // サイズビット
            let size_bit = (high_bits & 0x02) != 0;
            let size = if size_bit {
                SpriteSize::Large
            } else {
                SpriteSize::Small
            };

            // タイル番号（9ビット）
            let tile = tile_lo | (((attr & 0x01) as u16) << 8);

            // 属性ビット
            let palette = (attr >> 1) & 0x07;
            let priority = (attr >> 4) & 0x03;
            let flip_x = (attr & 0x40) != 0;
            let flip_y = (attr & 0x80) != 0;

            // Note: Y is 8-bit and wraps on hardware (e.g., 0xFE shows at top).

            let (sprite_width, sprite_height) = self.get_sprite_pixel_size(&size);
            sprites.push(self.build_sprite_data(
                x,
                y,
                tile,
                palette,
                priority,
                flip_x,
                flip_y,
                size,
                sprite_width,
                sprite_height,
                0,
            ));
        }

        sprites
    }

    // スプライトの実際のピクセルサイズを取得
    #[inline]
    pub(crate) fn get_sprite_pixel_size(&self, size: &SpriteSize) -> (u8, u8) {
        match self.sprite_size {
            0 => match size {
                SpriteSize::Small => (8, 8),
                SpriteSize::Large => (16, 16),
            },
            1 => match size {
                SpriteSize::Small => (8, 8),
                SpriteSize::Large => (32, 32),
            },
            2 => match size {
                SpriteSize::Small => (8, 8),
                SpriteSize::Large => (64, 64),
            },
            3 => match size {
                SpriteSize::Small => (16, 16),
                SpriteSize::Large => (32, 32),
            },
            4 => match size {
                SpriteSize::Small => (16, 16),
                SpriteSize::Large => (64, 64),
            },
            5 => match size {
                SpriteSize::Small => (32, 32),
                SpriteSize::Large => (64, 64),
            },
            6 => match size {
                SpriteSize::Small => (16, 32),
                SpriteSize::Large => (32, 64),
            },
            _ => match size {
                SpriteSize::Small => (16, 32),
                SpriteSize::Large => (32, 64),
            },
        }
    }

    // スプライトタイル描画
    #[inline]
    pub(crate) fn render_sprite_tile(
        &self,
        sprite: &SpriteData,
        tile_x: u8,
        tile_y: u8,
        pixel_x: u8,
        pixel_y: u8,
    ) -> u32 {
        // 8x8タイル内での座標
        let mut local_x = pixel_x;
        let mut local_y = pixel_y;

        // フリップ処理
        if sprite.flip_x {
            local_x = 7 - local_x;
        }
        if sprite.flip_y {
            local_y = 7 - local_y;
        }

        // スプライトサイズに基づいたタイル番号計算（改善版）
        let tile_num = self.calculate_sprite_tile_number(sprite, tile_x, tile_y);

        // スプライトのbpp数を決定（BGモードによる）
        let bpp = self.get_sprite_bpp();

        match bpp {
            2 => self.render_sprite_2bpp(tile_num, local_x, local_y, sprite.palette),
            4 => self.render_sprite_4bpp(tile_num, local_x, local_y, sprite.palette),
            8 => self.render_sprite_8bpp(tile_num, local_x, local_y),
            _ => 0,
        }
    }

    #[inline]
    pub(crate) fn calculate_sprite_tile_number(
        &self,
        sprite: &SpriteData,
        tile_x: u8,
        tile_y: u8,
    ) -> u16 {
        // スプライトのタイルレイアウト計算
        let tiles_per_row = sprite.width / 8;

        // フリップを考慮したタイル座標
        let actual_tile_x = if sprite.flip_x {
            (tiles_per_row - 1) - tile_x
        } else {
            tile_x
        };
        let actual_tile_y = if sprite.flip_y {
            (sprite.height / 8 - 1) - tile_y
        } else {
            tile_y
        };

        // SNESスプライトタイル番号計算（16タイル幅で配置）
        // OBJ tiles are addressed in a 16-tile-wide grid (9-bit index).
        // Row stride is 16 tiles regardless of sprite size.
        sprite.tile + (actual_tile_y as u16) * 16 + (actual_tile_x as u16)
    }

    #[inline]
    pub(crate) fn get_sprite_bpp(&self) -> u8 {
        // SNES OBJ (sprites) are always 4bpp.
        4
    }

    #[inline]
    pub(crate) fn sprite_name_select_gap_words(&self) -> u16 {
        // OBSEL name select (nn) chooses the secondary 8KB table at (nn+1)*8KB words.
        // nn = 0 => +0x1000 words (contiguous).
        (self.sprite_name_select.wrapping_add(1)) * 0x1000
    }

    #[inline]
    pub(crate) fn sprite_tile_base_word_addr(&self, tile_num: u16) -> u16 {
        // VRAM word address (0x0000-0x7FFF) at the start of the given OBJ tile.
        //
        // - OBJ tile numbers are 9-bit (0..0x1FF). Bit8 selects "bank" 0x000-0x0FF vs 0x100-0x1FF.
        // - OBSEL ($2101) selects:
        //   - Base address for tiles 0x000-0x0FF in 8K-word (16KB) steps
        //   - Secondary table at (name_select+1)*8KB words from the base
        //
        // Object tiles are 4bpp: 16 words per 8x8 tile.
        let base_word = self.sprite_name_base & 0x7FFF;
        let gap_word = self.sprite_name_select_gap_words() & 0x7FFF;

        let t = tile_num & 0x01FF;
        let bank = (t >> 8) & 1;
        let index = t & 0x00FF;

        let mut word = base_word.wrapping_add(index.wrapping_mul(16));
        if bank != 0 {
            // Tiles 0x100-0x1FF use the secondary table offset selected by OBSEL.
            word = word.wrapping_add(gap_word);
        }
        word & 0x7FFF
    }

    pub(crate) fn render_sprite_2bpp(
        &self,
        tile_num: u16,
        pixel_x: u8,
        pixel_y: u8,
        palette: u8,
    ) -> u32 {
        // NOTE: SNES OBJ are 4bpp. This path is kept only for experiments/debugging.
        // 2bpp tile = 8 words.
        let tile_addr = (self.sprite_tile_base_word_addr(tile_num) & 0x7FFF) & !0x0007;

        // tile_addr is in words, convert to byte index
        let plane0_addr = ((tile_addr + pixel_y as u16) as usize) * 2;
        let plane1_addr = plane0_addr + 1;

        if plane0_addr >= self.vram.len() || plane1_addr >= self.vram.len() {
            return 0;
        }

        let plane0 = self.vram[plane0_addr];
        let plane1 = self.vram[plane1_addr];

        let bit = 7 - pixel_x;
        let color_index = ((plane1 >> bit) & 1) << 1 | ((plane0 >> bit) & 1);

        if color_index == 0 {
            return 0; // 透明
        }

        // スプライトパレットは128-255（CGRAM上位128バイト）
        let palette_base = 128 + (palette * 4);
        let palette_index = palette_base + color_index;

        self.cgram_to_rgb(palette_index)
    }

    pub(crate) fn render_sprite_4bpp(
        &self,
        tile_num: u16,
        pixel_x: u8,
        pixel_y: u8,
        palette: u8,
    ) -> u32 {
        // 4bpp sprite tile = 32 bytes = 16 words
        let tile_addr = self.sprite_tile_base_word_addr(tile_num);

        // 4bpp sprite tile layout matches BG 4bpp: (plane0/1) then (plane2/3), 8 words each.
        let row01_word = (tile_addr.wrapping_add(pixel_y as u16)) & 0x7FFF;
        let row23_word = (tile_addr.wrapping_add(8).wrapping_add(pixel_y as u16)) & 0x7FFF;
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

        let bit = 7 - pixel_x;
        let color_index = ((plane3 >> bit) & 1) << 3
            | ((plane2 >> bit) & 1) << 2
            | ((plane1 >> bit) & 1) << 1
            | ((plane0 >> bit) & 1);

        if color_index == 0 {
            return 0; // 透明
        }

        // スプライト4bppパレットは128-255（16色/パレット）
        let palette_base = 128 + (palette * 16);
        let palette_index = palette_base + color_index;

        self.cgram_to_rgb(palette_index)
    }

    pub(crate) fn render_sprite_8bpp(&self, tile_num: u16, pixel_x: u8, pixel_y: u8) -> u32 {
        // NOTE: SNES OBJ are 4bpp. This path is kept only for experiments/debugging.
        // 8bpp sprite tile = 64 bytes = 32 words
        let tile_addr = self.sprite_tile_base_word_addr(tile_num);

        // 8bpp layout is 4 plane-pairs of 8 words each:
        // - +0:  plane0/1 rows 0..7
        // - +8:  plane2/3 rows 0..7
        // - +16: plane4/5 rows 0..7
        // - +24: plane6/7 rows 0..7
        let row0 = (tile_addr.wrapping_add(pixel_y as u16)) & 0x7FFF;
        let row1 = (tile_addr.wrapping_add(8).wrapping_add(pixel_y as u16)) & 0x7FFF;
        let row2 = (tile_addr.wrapping_add(16).wrapping_add(pixel_y as u16)) & 0x7FFF;
        let row3 = (tile_addr.wrapping_add(24).wrapping_add(pixel_y as u16)) & 0x7FFF;
        let a0 = (row0 as usize) * 2;
        let a1 = (row1 as usize) * 2;
        let a2 = (row2 as usize) * 2;
        let a3 = (row3 as usize) * 2;
        if a3 + 1 >= self.vram.len() {
            return 0;
        }
        let p0 = self.vram[a0];
        let p1 = self.vram[a0 + 1];
        let p2 = self.vram[a1];
        let p3 = self.vram[a1 + 1];
        let p4 = self.vram[a2];
        let p5 = self.vram[a2 + 1];
        let p6 = self.vram[a3];
        let p7 = self.vram[a3 + 1];

        let bit = 7 - pixel_x;
        let color_index = ((p7 >> bit) & 1) << 7
            | ((p6 >> bit) & 1) << 6
            | ((p5 >> bit) & 1) << 5
            | ((p4 >> bit) & 1) << 4
            | ((p3 >> bit) & 1) << 3
            | ((p2 >> bit) & 1) << 2
            | ((p1 >> bit) & 1) << 1
            | ((p0 >> bit) & 1);

        if color_index == 0 {
            return 0; // 透明
        }

        let palette_index = self.get_sprite_palette_index(0, color_index, 8);
        self.cgram_to_rgb(palette_index)
    }

    // Summarize and reset OBJ timing metrics (for headless logs)
    pub(crate) fn take_obj_summary(&mut self) -> String {
        let ov = self.obj_overflow_lines;
        let to = self.obj_time_over_lines;
        self.obj_overflow_lines = 0;
        self.obj_time_over_lines = 0;
        format!("OBJ: overflow_lines={} time_over_lines={}", ov, to)
    }

    // Build per-line OBJ pipeline: pick first 32 overlapping sprites and precompute tile starts
    pub(crate) fn prepare_line_obj_pipeline(&mut self, scanline: u16) {
        self.line_sprites.clear();
        for bucket in &mut self.line_sprites_by_priority {
            bucket.clear();
        }
        self.sprite_tile_entry_counts.fill(0);
        self.sprite_tile_budget_remaining = 34;
        self.sprite_draw_disabled = false;
        self.sprite_overflow = false;
        self.sprite_time_over = false;
        self.sprite_timeover_first_idx = 0;
        self.sprites_on_line_count = 0;

        if self.oam_dirty {
            self.rebuild_oam_cache();
        }

        // OAM dump on scanline 0 (opt-in via DUMP_OAM_FRAME)
        if scanline == 0 {
            use std::sync::OnceLock;
            static DUMP_FRAME: OnceLock<Option<u64>> = OnceLock::new();
            let target = *DUMP_FRAME.get_or_init(|| {
                std::env::var("DUMP_OAM_FRAME")
                    .ok()
                    .and_then(|v| v.parse::<u64>().ok())
            });
            if let Some(target_frame) = target {
                if self.frame == target_frame {
                    eprintln!("[OAM-DUMP] frame={} oam_eval_base={} OBSEL: name_base=0x{:04X} name_sel_gap=0x{:04X} sprite_size={}",
                        self.frame, self.oam_eval_base, self.sprite_name_base, self.sprite_name_select_gap_words(), self.sprite_size);
                    for i in 0..128usize {
                        let y = self.sprite_cached_y[i];
                        let x = self.sprite_cached_x_signed[i];
                        let tile = self.sprite_cached_tile[i];
                        let large = self.sprite_cached_size_large[i];
                        let attr = self.sprite_cached_attr[i];
                        // Only dump sprites in visible area or nearby
                        if y < 224 || y > 192 {
                            // y>192 wraps to top
                            let pal = (attr >> 1) & 0x07;
                            let pri = (attr >> 4) & 0x03;
                            eprintln!("[OAM-DUMP] #{:3} x={:4} y={:3} tile=0x{:03X} large={} pal={} pri={}",
                                i, x, y, tile, large as u8, pal, pri);
                        }
                    }
                }
            }
        }

        // Gather sprites in rotated OAM order (starting at oam_eval_base), cap at 32 like hardware.
        //
        // SNESdev/Super Famicom wiki behavior:
        // - Range over (bit6): set if there are >32 sprites on a scanline, but *off-screen* sprites do not count.
        //   Only sprites with -size < X < 256 are considered in the range test.
        // - Time over (bit7): set if there are >34 8x8 tiles on a scanline, evaluated from the 32-range list.
        //   Only tiles with -8 < X < 256 are counted.
        // - Special case: if X == -256 (raw 256), it is treated as X == 0 for range/time evaluation.
        //
        // We approximate this by applying the horizontal inclusion rules and counting tiles across.
        let mut count_seen = 0u8;
        let mut in_range_total = 0u16;
        let obj_line = self.obj_line_for_scanline(scanline);
        let (small_w, small_h) = self.get_sprite_pixel_size(&SpriteSize::Small);
        let (large_w, large_h) = self.get_sprite_pixel_size(&SpriteSize::Large);
        for n in 0..128u16 {
            let i = ((self.oam_eval_base as u16 + n) & 0x7F) as usize;
            let y = self.sprite_cached_y[i];
            // Note: Y is 8-bit and wraps on hardware (e.g., 0xFE shows at top).
            // Determine size
            let size_bit = self.sprite_cached_size_large[i];
            let size = if size_bit {
                SpriteSize::Large
            } else {
                SpriteSize::Small
            };
            let (sprite_w, sprite_h) = if size_bit {
                (large_w, large_h)
            } else {
                (small_w, small_h)
            };
            // Y is 8-bit and wraps; test overlap via wrapped subtraction.
            let dy_lines = self.obj_sprite_dy(obj_line, y);
            let sprite_height_lines = self.obj_sprite_height_lines(sprite_h as u16);
            if dy_lines >= sprite_height_lines {
                continue;
            }

            // Range stage horizontal inclusion: -size < X < 256 (X is signed 9-bit).
            let x_raw = self.sprite_cached_x_raw[i];
            let mut x = self.sprite_cached_x_signed[i];
            // Bug: treat X == -256 as X == 0 for range/time evaluation.
            if x == -256 {
                x = 0;
            }
            // Only partially-on-screen sprites count towards range overflow.
            if x <= -(sprite_w as i16) {
                continue;
            }
            // x is signed (-256..255), so x < 256 always holds; keep the check for clarity.
            if x >= 256 {
                continue;
            }

            in_range_total = in_range_total.saturating_add(1);

            // Pull rest of fields
            let attr = self.sprite_cached_attr[i];
            let x = x_raw;
            let tile = self.sprite_cached_tile[i];
            let palette = (attr >> 1) & 0x07;
            let priority = (attr >> 4) & 0x03;
            let flip_x = (attr & 0x40) != 0;
            let flip_y = (attr & 0x80) != 0;
            if in_range_total == 33 && crate::debug_flags::trace_burnin_obj() {
                println!(
                    "[BURNIN-OBJ][OVERFLOW33] frame={} line={} idx={} x_raw={} y={} dy={} size={:?}",
                    self.frame,
                    scanline,
                    i,
                    x,
                    y,
                    dy_lines,
                    size
                );
            }

            if count_seen < 32 {
                let idx = self.line_sprites.len();
                let line_rel_y = self.obj_sprite_rel_y(dy_lines);
                self.line_sprites.push(self.build_sprite_data(
                    x, y, tile, palette, priority, flip_x, flip_y, size, sprite_w, sprite_h,
                    line_rel_y,
                ));
                if (priority as usize) < self.line_sprites_by_priority.len() {
                    self.line_sprites_by_priority[priority as usize].push(idx);
                }
                count_seen = count_seen.saturating_add(1);
            }
            // Do not break early; keep scanning to detect overflow realistcally
        }

        self.sprites_on_line_count = in_range_total.min(0x3F) as u8;

        // Sprite overflow flag/metric
        self.sprite_overflow = in_range_total > 32;
        if self.sprite_overflow {
            if !self.sprite_overflow_latched && crate::debug_flags::trace_burnin_obj() {
                println!(
                    "[BURNIN-OBJ][LATCH] range_over set: frame={} line={} overlapped_total={} oam_base={}",
                    self.frame, scanline, in_range_total, self.oam_eval_base
                );
            }
            self.sprite_overflow_latched = true;
            self.obj_overflow_lines = self.obj_overflow_lines.saturating_add(1);
        }

        // Time-over (tile overflow) evaluation for this scanline.
        // Count up to 34 tiles across the 32-sprite range list in OAM order.
        // When exceeded, drop this sprite and any later ones (approximation).
        let mut tiles_seen: u16 = 0;
        self.sprite_timeover_first_idx = 0;
        'time_eval: for (idx, s) in self.line_sprites.iter().enumerate() {
            let mut sx = s.x_signed;
            if sx == -256 {
                sx = 0;
            }
            let tiles_across = (s.width as i16) / 8;
            for k in 0..tiles_across {
                let tx = sx + k * 8;
                // Only tiles with -8 < X < 256 are counted.
                if tx <= -8 || tx >= 256 {
                    continue;
                }
                // Dot-level approximation:
                // charge this tile on the first visible pixel where it can appear.
                let entry_x = tx.max(0) as usize;
                self.sprite_tile_entry_counts[entry_x] =
                    self.sprite_tile_entry_counts[entry_x].saturating_add(1);
                tiles_seen = tiles_seen.saturating_add(1);
                if tiles_seen > 34 {
                    self.sprite_time_over = true;
                    self.sprite_timeover_first_idx = (idx.min(255)) as u8;
                    if !self.sprite_time_over_latched && crate::debug_flags::trace_burnin_obj() {
                        println!(
                            "[BURNIN-OBJ][LATCH] time_over set: frame={} line={} tiles_seen={} first_idx={}",
                            self.frame, scanline, tiles_seen, self.sprite_timeover_first_idx
                        );
                    }
                    self.sprite_time_over_latched = true;
                    self.obj_time_over_lines = self.obj_time_over_lines.saturating_add(1);
                    break 'time_eval;
                }
            }
        }
    }

    pub(crate) fn rebuild_oam_cache(&mut self) {
        for i in 0..128usize {
            let oam_offset = i * 4;
            if oam_offset + 3 >= self.oam.len() {
                break;
            }
            let y = self.oam[oam_offset + 1];
            let x_lo = self.oam[oam_offset] as u16;
            let tile_lo = self.oam[oam_offset + 2] as u16;
            let attr = self.oam[oam_offset + 3];
            let high_table_offset = 0x200 + (i / 4);
            if high_table_offset >= self.oam.len() {
                break;
            }
            let high_table_byte = self.oam[high_table_offset];
            let bit_shift = (i % 4) * 2;
            let high_bits = (high_table_byte >> bit_shift) & 0x03;
            let x_raw = x_lo | (((high_bits & 0x01) as u16) << 8);
            let tile = tile_lo | (((attr & 0x01) as u16) << 8);

            self.sprite_cached_y[i] = y;
            self.sprite_cached_x_raw[i] = x_raw;
            self.sprite_cached_x_signed[i] = Self::sprite_x_signed(x_raw);
            self.sprite_cached_tile[i] = tile;
            self.sprite_cached_attr[i] = attr;
            self.sprite_cached_size_large[i] = (high_bits & 0x02) != 0;
        }
        self.oam_dirty = false;
    }

    // Consume time budget on first pixel of each 8px tile; disable OBJ for rest of line when exhausted
    #[allow(dead_code)]
    pub(crate) fn update_obj_time_over_at_x(&mut self, x: u16) {
        if x == 0 {
            self.sprite_tile_budget_remaining = 34;
            self.sprite_draw_disabled = false;
        }
        if self.sprite_draw_disabled || !self.sprite_time_over || x >= 256 {
            return;
        }
        // Fetch budget is consumed on tile boundaries only.
        if (x & 7) != 0 {
            return;
        }
        let consume = self.sprite_tile_entry_counts[x as usize] as i16;
        if consume <= 0 {
            return;
        }
        self.sprite_tile_budget_remaining -= consume;
        if self.sprite_tile_budget_remaining < 0 {
            self.sprite_draw_disabled = true;
        }
    }

    // サブスクリーン用スプライト描画（簡易版）
    #[allow(dead_code)]
    pub(crate) fn get_sub_sprite_pixel(&self, x: u16, y: u16) -> (u32, u8) {
        let enabled = (self.sub_screen_designation & 0x10) != 0;
        let (color, pr, _math_allowed) = self.get_sprite_pixel_common(x, y, enabled, false);
        (color, pr)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ppu::Ppu;

    fn sprite_test_ppu() -> Ppu {
        let mut ppu = Ppu::new();
        // Tile 0 row 0: leftmost pixel = color 1.
        ppu.vram[0] = 0x80;
        ppu.vram[1] = 0x00;
        ppu.vram[16] = 0x00;
        ppu.vram[17] = 0x00;
        ppu.line_sprites = vec![
            ppu.build_sprite_data(0, 0, 0, 4, 0, false, false, SpriteSize::Small, 8, 8, 0),
            ppu.build_sprite_data(0, 0, 0, 4, 1, false, false, SpriteSize::Small, 8, 8, 0),
        ];
        ppu
    }

    fn set_oam_sprite(ppu: &mut Ppu, index: usize, x_raw: u16, y: u8, tile: u8, attr: u8) {
        let base = index * 4;
        ppu.oam[base] = x_raw as u8;
        ppu.oam[base + 1] = y;
        ppu.oam[base + 2] = tile;
        ppu.oam[base + 3] = attr;

        let high_table_offset = 0x200 + (index / 4);
        let bit_shift = (index % 4) * 2;
        let mut high = ppu.oam[high_table_offset];
        high &= !(0x03 << bit_shift);
        high |= (((x_raw >> 8) as u8) & 0x01) << bit_shift;
        ppu.oam[high_table_offset] = high;
        ppu.oam_dirty = true;
    }

    fn hide_unused_oam_sprites(ppu: &mut Ppu) {
        for index in 0..128usize {
            ppu.oam[index * 4 + 1] = 0xF0;
        }
        ppu.oam_dirty = true;
    }

    #[test]
    fn time_over_keeps_already_fetched_earlier_sprites_visible() {
        let mut ppu = sprite_test_ppu();
        ppu.sprite_time_over = true;
        ppu.sprite_draw_disabled = true;
        ppu.sprite_timeover_first_idx = 1;

        let (color, priority, _math_allowed) = ppu.get_sprite_pixel_common(0, 0, true, true);
        assert_ne!(color, 0);
        assert_eq!(priority, 0);
    }

    #[test]
    fn time_over_drops_sprites_at_and_after_cutoff_index() {
        let mut ppu = sprite_test_ppu();
        ppu.line_sprites[0].tile = 1;
        ppu.sprite_time_over = true;
        ppu.sprite_draw_disabled = true;
        ppu.sprite_timeover_first_idx = 1;

        let (color, _priority, _math_allowed) = ppu.get_sprite_pixel_common(0, 0, true, true);
        assert_eq!(color, 0);
    }

    #[test]
    fn priority_rotation_changes_frontmost_obj_order() {
        let mut ppu = Ppu::new();
        ppu.vram[0] = 0x80;
        hide_unused_oam_sprites(&mut ppu);
        set_oam_sprite(&mut ppu, 0, 0, 0, 0, 0x00);
        set_oam_sprite(&mut ppu, 1, 0, 0, 0, 0x10);

        ppu.prepare_line_obj_pipeline(0);
        assert_eq!(ppu.sprites_on_line_count, 2);
        let (_color, priority, _math_allowed) = ppu.get_sprite_pixel_common(0, 0, true, true);
        assert_eq!(priority, 0);

        ppu.oam_priority_rotation_enabled = true;
        ppu.oam_eval_base = 1;
        ppu.prepare_line_obj_pipeline(0);
        assert_eq!(ppu.sprites_on_line_count, 2);
        let (_color, priority, _math_allowed) = ppu.get_sprite_pixel_common(0, 0, true, true);
        assert_eq!(priority, 1);
    }

    #[test]
    fn x_minus_256_counts_for_range_evaluation_but_stays_offscreen() {
        let mut ppu = Ppu::new();
        ppu.vram[0] = 0x80;
        hide_unused_oam_sprites(&mut ppu);
        set_oam_sprite(&mut ppu, 0, 0x100, 0, 0, 0x00);

        ppu.prepare_line_obj_pipeline(0);

        assert_eq!(ppu.sprites_on_line_count, 1);
        assert_eq!(ppu.line_sprites.len(), 1);

        let (color, _priority, _math_allowed) = ppu.get_sprite_pixel_common(0, 0, true, true);
        assert_eq!(
            color, 0,
            "X=-256 should count for range, but not render on-screen"
        );
    }

    #[test]
    fn fully_offscreen_left_sprite_does_not_count_toward_range_limit() {
        let mut ppu = Ppu::new();
        ppu.vram[0] = 0x80;
        hide_unused_oam_sprites(&mut ppu);
        set_oam_sprite(&mut ppu, 0, 0x1F8, 0, 0, 0x00); // -8: fully off the left edge
        set_oam_sprite(&mut ppu, 1, 0x000, 0, 0, 0x10); // visible

        ppu.prepare_line_obj_pipeline(0);

        assert_eq!(ppu.sprites_on_line_count, 1);
        assert!(!ppu.sprite_overflow);
        assert_eq!(ppu.line_sprites.len(), 1);
        assert_eq!(ppu.line_sprites[0].x, 0);
    }

    #[test]
    fn oam_data_read_advances_priority_rotation_base() {
        let mut ppu = Ppu::new();
        ppu.vram[0] = 0x80;
        hide_unused_oam_sprites(&mut ppu);
        set_oam_sprite(&mut ppu, 0, 0, 0, 0, 0x00);
        set_oam_sprite(&mut ppu, 1, 0, 0, 0, 0x10);

        ppu.write(0x02, 0x00);
        ppu.write(0x03, 0x80);
        ppu.prepare_line_obj_pipeline(0);
        let (_color, priority, _math_allowed) = ppu.get_sprite_pixel_common(0, 0, true, true);
        assert_eq!(priority, 0);

        for _ in 0..4 {
            let _ = ppu.read(0x38);
        }
        assert_eq!(ppu.oam_eval_base, 1);

        ppu.prepare_line_obj_pipeline(0);
        let (_color, priority, _math_allowed) = ppu.get_sprite_pixel_common(0, 0, true, true);
        assert_eq!(priority, 1);
    }

    #[test]
    fn oam_data_write_advances_priority_rotation_base() {
        let mut ppu = Ppu::new();
        hide_unused_oam_sprites(&mut ppu);
        ppu.screen_display = 0x80;

        ppu.write(0x02, 0x00);
        ppu.write(0x03, 0x80);
        assert_eq!(ppu.oam_eval_base, 0);

        for value in [0x00, 0x00, 0x00, 0x10] {
            ppu.write(0x04, value);
        }
        assert_eq!(ppu.oam_eval_base, 1);
    }

    #[test]
    fn oam_data_read_is_blocked_during_active_display() {
        let mut ppu = Ppu::new();
        hide_unused_oam_sprites(&mut ppu);
        set_oam_sprite(&mut ppu, 0, 0, 0, 0xAA, 0x00);
        ppu.write(0x02, 0x00);
        ppu.write(0x03, 0x00);
        ppu.screen_display = 0x00;
        ppu.scanline = 0;
        ppu.cycle = 32;
        ppu.v_blank = false;
        ppu.h_blank = false;

        let value = ppu.read(0x38);
        assert_eq!(value, 0);
        assert_eq!(ppu.oam_internal_addr, 0);
    }

    #[test]
    fn oam_data_read_works_in_forced_blank() {
        let mut ppu = Ppu::new();
        hide_unused_oam_sprites(&mut ppu);
        set_oam_sprite(&mut ppu, 0, 0, 0x5A, 0, 0x00);
        ppu.screen_display = 0x80;
        ppu.write(0x02, 0x00);
        ppu.write(0x03, 0x00);

        let value = ppu.read(0x38);
        assert_eq!(value, 0x00);
        let value = ppu.read(0x38);
        assert_eq!(value, 0x5A);
    }

    #[test]
    fn cgram_read_is_blocked_during_active_display() {
        let mut ppu = Ppu::new();
        ppu.cgram[0] = 0x34;
        ppu.cgram[1] = 0x12;
        ppu.screen_display = 0x00;
        ppu.scanline = 0;
        ppu.cycle = 32;
        ppu.v_blank = false;
        ppu.h_blank = false;

        let lo = ppu.read(0x3B);
        let hi = ppu.read(0x3B);
        assert_eq!(lo, 0);
        assert_eq!(hi, 0);
        assert!(!ppu.cgram_read_second);
        assert_eq!(ppu.cgram_addr, 0);
    }

    #[test]
    fn cgram_read_works_in_vblank() {
        let mut ppu = Ppu::new();
        ppu.cgram[0] = 0x34;
        ppu.cgram[1] = 0x92;
        ppu.v_blank = true;

        let lo = ppu.read(0x3B);
        let hi = ppu.read(0x3B);
        assert_eq!(lo, 0x34);
        assert_eq!(hi, 0x12);
        assert_eq!(ppu.cgram_addr, 1);
    }
}
