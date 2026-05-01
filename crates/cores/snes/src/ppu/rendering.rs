use super::Ppu;
mod bg_fetch;
mod bg_modes;
mod color_math;
mod composite;
mod line_state;
mod mode7;
mod screen_pixel;
mod superfx_direct;
mod trace;

impl Ppu {
    #[allow(dead_code)]
    pub(crate) fn render_scanline(&mut self) {
        if crate::debug_flags::boot_verbose() {
            // Debug scanline rendering
            static mut SCANLINE_DEBUG_COUNT: u32 = 0;
            unsafe {
                SCANLINE_DEBUG_COUNT += 1;
                if SCANLINE_DEBUG_COUNT <= 10 || SCANLINE_DEBUG_COUNT.is_multiple_of(1000) {
                    println!(
                        "🖼️ SCANLINE RENDER[{}]: line={}, brightness={}, forced_blank={}",
                        SCANLINE_DEBUG_COUNT,
                        self.scanline,
                        self.brightness,
                        (self.screen_display & 0x80) != 0
                    );
                }
            }
        }

        // 画面表示が有効でなくても、テストパターンを表示
        let y = self.scanline as usize;

        if crate::debug_flags::boot_verbose() {
            static mut SCANLINE_CHECK_COUNT: u32 = 0;
            unsafe {
                SCANLINE_CHECK_COUNT += 1;
                if SCANLINE_CHECK_COUNT <= 5 {
                    println!(
                        "🔍 SCANLINE CHECK: y={}, scanline={}, condition y >= 239: {}",
                        y,
                        self.scanline,
                        y >= 239
                    );
                }
            }
        }

        // Scanline 0 is not visible on real hardware (overscan area)
        if y == 0 || y > 239 {
            return;
        }
        let fb_y = y - 1; // map scanline 1 -> fb row 0

        // Render pixels for scanline y

        // Use game-provided screen designation as-is.

        // Debug: Check main screen designation during rendering
        if crate::debug_flags::render_verbose() && !crate::debug_flags::quiet() {
            static mut RENDER_DEBUG_COUNT: u32 = 0;
            unsafe {
                if RENDER_DEBUG_COUNT < 10 {
                    RENDER_DEBUG_COUNT += 1;
                    let effective = self.effective_main_screen_designation();
                    println!("🎬 RENDER[{}]: y={} main_screen=0x{:02X} effective=0x{:02X} last_nonzero=0x{:02X} bg_mode={} brightness={} forced_blank={}",
                        RENDER_DEBUG_COUNT, y, self.main_screen_designation, effective,
                        self.main_screen_designation_last_nonzero, self.bg_mode,
                        self.brightness, (self.screen_display & 0x80) != 0);
                }
            }
        }

        // CRITICAL DEBUG: Verify we reach this point
        // Process 256 pixels for this scanline

        // Debug: Report pixel loop entry
        if crate::debug_flags::boot_verbose() {
            static mut PIXEL_LOOP_REPORTED: bool = false;
            unsafe {
                if !PIXEL_LOOP_REPORTED {
                    println!("🖼️ PIXEL LOOP: Starting pixel rendering for line {}", y);
                    PIXEL_LOOP_REPORTED = true;
                }
            }
        }

        // Render all 256 pixels
        let boot_verbose = crate::debug_flags::boot_verbose();
        for x in 0..256 {
            // メインスクリーンとサブスクリーンを個別に描画（レイヤID付き）
            let (mut main_color, mut main_layer_id, mut main_obj_math_allowed) =
                self.render_main_screen_pixel_with_layer(x as u16, y as u16);
            let _main_transparent = main_color == 0;
            if main_color == 0 {
                main_color = self.cgram_to_rgb(0);
                main_layer_id = 5;
                main_obj_math_allowed = true;
            }
            let (sub_color, _sub_layer_id, sub_transparent, _sub_obj_math_allowed) =
                self.render_sub_screen_pixel_with_layer(x as u16, y as u16);

            let final_color = if self.pseudo_hires {
                // Main screen pixel with normal color math
                let main_mixed = self.apply_color_math_screens(
                    main_color,
                    sub_color,
                    main_layer_id,
                    main_obj_math_allowed,
                    x as u16,
                    y as u16,
                    sub_transparent,
                );
                // Sub screen pixel as-is (even subpixel in 512px output)
                let sub_pixel = if sub_transparent {
                    self.cgram_to_rgb(0)
                } else {
                    sub_color
                };
                Self::average_rgb(sub_pixel, main_mixed)
            } else {
                // カラー演算適用（対象レイヤに限定）
                self.apply_color_math_screens(
                    main_color,
                    sub_color,
                    main_layer_id,
                    main_obj_math_allowed,
                    x as u16,
                    y as u16,
                    sub_transparent,
                )
            };

            let pixel_offset = fb_y * 256 + x;

            // 画面の明度（INIDISP）を適用
            let final_brightness_color = self.apply_brightness(final_color);
            self.render_framebuffer[pixel_offset] = final_brightness_color;
            self.render_subscreen_buffer[pixel_offset] = sub_color;

            // Debug: all boot_verbose checks hoisted out of hot loop
            if boot_verbose {
                static mut RENDER_SCANLINE_CALLS: u32 = 0;
                static mut REAL_GRAPHICS_SHOWN: bool = false;
                static mut WHITE_PIXEL_DEBUG: u32 = 0;
                static mut FRAMEBUFFER_DEBUG_COUNT: u32 = 0;
                unsafe {
                    RENDER_SCANLINE_CALLS += 1;
                    if !REAL_GRAPHICS_SHOWN && x == 0 && y == 0 {
                        println!(
                            "🎮 RENDER_SCANLINE[{}]: x={}, y={}, first final_color=0x{:08X}",
                            RENDER_SCANLINE_CALLS, x, y, final_color
                        );
                        REAL_GRAPHICS_SHOWN = true;
                    } else if RENDER_SCANLINE_CALLS <= 100 && x == 0 {
                        println!(
                            "📺 SCANLINE PIXEL[{}]: y={}, first_final_color=0x{:08X}",
                            RENDER_SCANLINE_CALLS, y, final_color
                        );
                    }
                    if final_brightness_color != 0xFF000000 {
                        WHITE_PIXEL_DEBUG += 1;
                        if WHITE_PIXEL_DEBUG <= 10 {
                            println!(
                                "🖼️ FRAMEBUFFER[{}]: pos={} final=0x{:08X} (brightness={})",
                                WHITE_PIXEL_DEBUG,
                                pixel_offset,
                                final_brightness_color,
                                self.brightness
                            );
                        }
                    }
                    FRAMEBUFFER_DEBUG_COUNT += 1;
                    if FRAMEBUFFER_DEBUG_COUNT <= 5 {
                        println!(
                            "🖼️ FRAMEBUFFER[{}]: pos={} final=0x{:08X} (brightness={})",
                            FRAMEBUFFER_DEBUG_COUNT,
                            pixel_offset,
                            final_brightness_color,
                            self.brightness
                        );
                    }
                }
            }
        }
    }

    #[allow(dead_code)]
    pub(crate) fn get_pixel_color(&mut self, x: u16, y: u16) -> u32 {
        // Respect forced blank: when set, output black regardless of scene state
        let mut forced_blank = (self.screen_display & 0x80) != 0;
        if self.force_display_active() || self.force_no_blank {
            forced_blank = false;
        }
        if forced_blank {
            return 0xFF000000;
        }

        if crate::debug_flags::boot_verbose() {
            static mut EMERGENCY_DEBUG_COUNT: u32 = 0;
            static mut PIXEL_CALL_COUNT: u32 = 0;
            unsafe {
                PIXEL_CALL_COUNT += 1;
                EMERGENCY_DEBUG_COUNT += 1;
                if EMERGENCY_DEBUG_COUNT <= 3 {
                    println!(
                        "🔍 GET_PIXEL_COLOR CALLED[{}]: x={}, y={}, forced_blank={}, brightness={}",
                        EMERGENCY_DEBUG_COUNT, x, y, forced_blank, self.brightness
                    );
                    println!(
                        "   📊 Total get_pixel_color calls: {} (from render_scanline)",
                        PIXEL_CALL_COUNT
                    );
                }
            }
        }

        // BGとスプライトの情報を取得 - Use main BG pixel function for proper graphics
        let enables = self.effective_main_screen_designation();
        let (bg_color, bg_priority, bg_id) = self.get_main_bg_pixel(x, y, enables);
        let (sprite_color, sprite_priority) = self.get_sprite_pixel(x, y);

        // Emergency test pattern removed - show actual graphics

        // Debug pixel color generation (first few pixels only)
        if crate::debug_flags::boot_verbose() {
            static mut PIXEL_DEBUG_COUNT: u32 = 0;
            unsafe {
                PIXEL_DEBUG_COUNT += 1;
                if PIXEL_DEBUG_COUNT <= 10 && x < 3 && y < 3 {
                    println!("🎨 PIXEL[{},{}]: bg_color=0x{:08X}, bg_priority={}, sprite_color=0x{:08X}, sprite_priority={}", 
                            x, y, bg_color, bg_priority, sprite_color, sprite_priority);
                    // Check if CGRAM has any non-black data for palette colors 1-15
                    let non_zero_colors = (1..16)
                        .map(|i| {
                            let addr = i * 2;
                            if addr + 1 < self.cgram.len() {
                                let color = ((self.cgram[addr + 1] as u16) << 8)
                                    | (self.cgram[addr] as u16);
                                color != 0
                            } else {
                                false
                            }
                        })
                        .filter(|&x| x)
                        .count();
                    if PIXEL_DEBUG_COUNT == 1 {
                        println!(
                            "🎨 CGRAM: Non-zero colors in palette 1-15: {}/15",
                            non_zero_colors
                        );
                        println!("🎨 PPU STATE: bg_mode={}, main_screen_designation=0x{:02X}, sub_screen_designation=0x{:02X}", 
                                self.bg_mode, self.main_screen_designation, self.sub_screen_designation);
                        println!("🎨 PPU STATE: screen_display=0x{:02X} (forced_blank={}), brightness={}", 
                                self.screen_display, (self.screen_display & 0x80) != 0, self.brightness);
                    }
                }
            }
        }

        // プライオリティベースの合成
        let (final_color, _lid) = self.composite_pixel_with_layer(
            bg_color,
            bg_priority,
            bg_id,
            sprite_color,
            sprite_priority,
        );

        if crate::debug_flags::boot_verbose() && x < 2 && y < 2 {
            println!(
                "🎨 COMPOSITE[{},{}]: final_color=0x{:08X}",
                x, y, final_color
            );
        }

        if final_color != 0 {
            let result = self.apply_brightness(final_color);
            if crate::debug_flags::boot_verbose() && x < 2 && y < 2 {
                println!(
                    "🎨 BRIGHT[{},{}]: final_color=0x{:08X} -> brightness_applied=0x{:08X}",
                    x, y, final_color, result
                );
            }
            return result;
        }

        // No emergency forcing. If nothing composites, use backdrop color (palette index 0)
        // バックドロップカラー（CGRAMの0番）を使用（代替色は使わない）
        let backdrop = self.cgram_to_rgb(0);
        let result = self.apply_brightness(backdrop);
        if crate::debug_flags::boot_verbose() && x < 2 && y < 2 {
            println!(
                "🎨 BACKDROP[{},{}]: backdrop=0x{:08X} -> brightness_applied=0x{:08X}",
                x, y, backdrop, result
            );
        }
        result
    }

    // get_sprite_pixel_common moved to sprites.rs

    // Helper: Get effective main screen designation for rendering
    #[inline]
    pub(crate) fn effective_main_screen_designation(&self) -> u8 {
        if let Some(v) = crate::debug_flags::debug_force_tm() {
            return v;
        }
        let mut designation = self.main_screen_designation;
        if self.should_suppress_starfox_title_bg1() {
            designation &= !0x01;
        }
        designation
    }

    #[inline]
    pub(crate) fn starfox_title_layout_active(&self) -> bool {
        self.bg_mode == 1
            && self.main_screen_designation == 0x07
            && self.sub_screen_designation == 0x07
            && self.tmw_mask == 0
            && self.tsw_mask == 0
            && self.cgwsel == 0x02
            && self.cgadsub == 0x50
            && self.bg1_hscroll == 0
            && self.bg1_vscroll == 0
            && self.bg2_hscroll == 0
            && self.bg2_vscroll == 0x0101
            && self.bg3_hscroll == 0x03FC
            && self.bg3_vscroll == 0x0009
            && self.bg1_tilemap_base == 0x2C00
            && self.bg2_tilemap_base == 0x7000
            && self.bg3_tilemap_base == 0x6800
            && self.bg1_tile_base == 0x3000
            && self.bg2_tile_base == 0x5000
            && self.bg3_tile_base == 0x7000
    }

    #[inline]
    fn should_suppress_starfox_title_bg1(&self) -> bool {
        self.starfox_title_suppress_bg1 && self.starfox_title_layout_active()
    }

    // get_sprite_pixel moved to sprites.rs

    #[allow(dead_code)]
    pub(crate) fn get_main_bg_layers(&mut self, x: u16, y: u16) -> Vec<(u32, u8, u8)> {
        // Return all background layers with their colors, priorities, and layer IDs
        let mut bg_results = Vec::new();

        // Debug: Sample a few pixels to see what's being rendered
        static mut BG_PIXEL_DEBUG: u32 = 0;
        unsafe {
            if crate::debug_flags::debug_bg_pixel() && BG_PIXEL_DEBUG < 5 && x == 100 && y == 100 {
                BG_PIXEL_DEBUG += 1;
                println!(
                    "🎨 BG_PIXEL[{}] at ({},{}) mode={} effective=0x{:02X}",
                    BG_PIXEL_DEBUG,
                    x,
                    y,
                    self.bg_mode,
                    self.effective_main_screen_designation()
                );
            }
        }

        match self.bg_mode {
            0 => {
                // Mode 0: BG1-4 全て2bpp
                if self.effective_main_screen_designation() & 0x01 != 0
                    && !self.should_mask_bg(x, 0, true)
                {
                    let (color, priority) = self.render_bg_mode0_with_priority(x, y, 0);
                    unsafe {
                        if crate::debug_flags::debug_bg_pixel()
                            && BG_PIXEL_DEBUG <= 5
                            && x == 100
                            && y == 100
                        {
                            println!("  BG1: color=0x{:08X} priority={}", color, priority);
                        }
                    }
                    if color != 0 {
                        bg_results.push((color, priority, 0));
                    }
                }
                if self.effective_main_screen_designation() & 0x02 != 0
                    && !self.should_mask_bg(x, 1, true)
                {
                    let (color, priority) = self.render_bg_mode0_with_priority(x, y, 1);
                    unsafe {
                        if crate::debug_flags::debug_bg_pixel()
                            && BG_PIXEL_DEBUG <= 5
                            && x == 100
                            && y == 100
                        {
                            println!("  BG2: color=0x{:08X} priority={}", color, priority);
                        }
                    }
                    if color != 0 {
                        bg_results.push((color, priority, 1));
                    }
                }
                if self.effective_main_screen_designation() & 0x04 != 0
                    && !self.should_mask_bg(x, 2, true)
                {
                    let (color, priority) = self.render_bg_mode0_with_priority(x, y, 2);
                    if color != 0 {
                        bg_results.push((color, priority, 2));
                    }
                }
                if self.effective_main_screen_designation() & 0x08 != 0
                    && !self.should_mask_bg(x, 3, true)
                {
                    let (color, priority) = self.render_bg_mode0_with_priority(x, y, 3);
                    if color != 0 {
                        bg_results.push((color, priority, 3));
                    }
                }
            }
            1 => {
                // Mode 1: BG1/BG2は4bpp、BG3は2bpp
                if self.effective_main_screen_designation() & 0x01 != 0
                    && !self.should_mask_bg(x, 0, true)
                {
                    let (color, priority) = self.render_bg_4bpp_with_priority(x, y, 0);
                    if color != 0 {
                        bg_results.push((color, priority, 0));
                    }
                }
                if self.effective_main_screen_designation() & 0x02 != 0
                    && !self.should_mask_bg(x, 1, true)
                {
                    let (color, priority) = self.render_bg_4bpp_with_priority(x, y, 1);
                    if color != 0 {
                        bg_results.push((color, priority, 1));
                    }
                }
                if self.effective_main_screen_designation() & 0x04 != 0 {
                    let (color, priority) = self.render_bg_mode0_with_priority(x, y, 2);
                    if color != 0 {
                        bg_results.push((color, priority, 2));
                    }
                }
            }
            4 => {
                // Mode 4: BG1は8bpp、BG2は2bpp
                if self.effective_main_screen_designation() & 0x01 != 0
                    && !self.should_mask_bg(x, 0, true)
                {
                    let (color, priority) = self.render_bg_8bpp_with_priority(x, y, 0);
                    if color != 0 {
                        bg_results.push((color, priority, 0));
                    }
                }
                if self.effective_main_screen_designation() & 0x02 != 0
                    && !self.should_mask_bg(x, 1, true)
                {
                    let (color, priority) = self.render_bg_mode0_with_priority(x, y, 1);
                    if color != 0 {
                        bg_results.push((color, priority, 1));
                    }
                }
            }
            2 => {
                // Mode 2: BG1/BG2は4bpp（オフセットパータイル機能付き）
                if self.effective_main_screen_designation() & 0x01 != 0 {
                    let (color, priority) = self.render_bg_mode2_window_aware(x, y, 0, true);
                    if color != 0 {
                        bg_results.push((color, priority, 0));
                    }
                }
                if self.effective_main_screen_designation() & 0x02 != 0 {
                    let (color, priority) = self.render_bg_mode2_window_aware(x, y, 1, true);
                    if color != 0 {
                        bg_results.push((color, priority, 1));
                    }
                }
            }
            3 => {
                // Mode 3: BG1は8bpp、BG2は4bpp
                if self.effective_main_screen_designation() & 0x01 != 0
                    && !self.should_mask_bg(x, 0, true)
                {
                    let (color, priority) = self.render_bg_8bpp_with_priority(x, y, 0);
                    if color != 0 {
                        bg_results.push((color, priority, 0));
                    }
                }
                if self.effective_main_screen_designation() & 0x02 != 0
                    && !self.should_mask_bg(x, 1, true)
                {
                    let (color, priority) = self.render_bg_4bpp_with_priority(x, y, 1);
                    if color != 0 {
                        bg_results.push((color, priority, 1));
                    }
                }
            }
            5 => {
                // Mode 5: BG1は4bpp、BG2は2bpp（高解像度）
                // Note: Some games also use BG3 in Mode 5
                if self.effective_main_screen_designation() & 0x01 != 0
                    && !self.should_mask_bg(x, 0, true)
                {
                    let (color, priority) = self.render_bg_mode5_with_priority(x, y, 0);
                    if color != 0 {
                        bg_results.push((color, priority, 0));
                    }
                }
                if self.effective_main_screen_designation() & 0x02 != 0
                    && !self.should_mask_bg(x, 1, true)
                {
                    let (color, priority) = self.render_bg_mode5_with_priority(x, y, 1);
                    if color != 0 {
                        bg_results.push((color, priority, 1));
                    }
                }
                if self.effective_main_screen_designation() & 0x04 != 0
                    && !self.should_mask_bg(x, 2, true)
                {
                    let (color, priority) = self.render_bg_mode5_with_priority(x, y, 2);
                    if color != 0 {
                        bg_results.push((color, priority, 2));
                    }
                }
            }
            6 => {
                // Mode 6: BG1は4bpp（高解像度）
                if self.effective_main_screen_designation() & 0x01 != 0
                    && !self.should_mask_bg(x, 0, true)
                {
                    let (color, priority) = self.render_bg_mode6_with_priority(x, y, 0);
                    if color != 0 {
                        bg_results.push((color, priority, 0));
                    }
                }
            }
            7 => {
                let (c, p, lid) = self.render_mode7_with_layer(x, y);
                if c != 0 {
                    let id = if self.extbg { lid } else { 0 };
                    let en_bit = 1u8 << id;
                    if (self.effective_main_screen_designation() & en_bit) != 0
                        && !self.should_mask_bg(x, id, true)
                    {
                        bg_results.push((c, p, id));
                    }
                }
            }
            _ => {
                // Unknown mode, return empty
            }
        }

        bg_results
    }

    #[allow(dead_code)]
    pub(crate) fn get_bg_pixel(&mut self, x: u16, y: u16) -> (u32, u8) {
        // Debug background layer enable status
        static mut BG_PIXEL_DEBUG: bool = false;
        unsafe {
            if !BG_PIXEL_DEBUG && x == 0 && y == 1 {
                println!(
                    "🎮 GET_BG_PIXEL: bg_mode={}, main_screen=0x{:02X}, bg_enables=[{},{},{},{}]",
                    self.bg_mode,
                    self.main_screen_designation,
                    self.effective_main_screen_designation() & 0x01 != 0,
                    self.effective_main_screen_designation() & 0x02 != 0,
                    self.effective_main_screen_designation() & 0x04 != 0,
                    self.effective_main_screen_designation() & 0x08 != 0
                );
                BG_PIXEL_DEBUG = true;
            }
        }

        // 全BGレイヤーの描画結果とプライオリティを取得
        let mut bg_results = Vec::new();

        match self.bg_mode {
            0 => {
                // Mode 0: BG1-4 全て2bpp
                if self.effective_main_screen_designation() & 0x01 != 0
                    && !self.should_mask_bg(x, 0, true)
                {
                    let (color, priority) = self.render_bg_mode0_with_priority(x, y, 0);
                    if color != 0 {
                        bg_results.push((color, priority, 0));
                    }
                }
                if self.effective_main_screen_designation() & 0x02 != 0
                    && !self.should_mask_bg(x, 1, true)
                {
                    let (color, priority) = self.render_bg_mode0_with_priority(x, y, 1);
                    if color != 0 {
                        bg_results.push((color, priority, 1));
                    }
                }
                if self.effective_main_screen_designation() & 0x04 != 0
                    && !self.should_mask_bg(x, 2, true)
                {
                    let (color, priority) = self.render_bg_mode0_with_priority(x, y, 2);
                    if color != 0 {
                        bg_results.push((color, priority, 2));
                    }
                }
                if self.effective_main_screen_designation() & 0x08 != 0
                    && !self.should_mask_bg(x, 3, true)
                {
                    let (color, priority) = self.render_bg_mode0_with_priority(x, y, 3);
                    if color != 0 {
                        bg_results.push((color, priority, 3));
                    }
                }
            }
            1 => {
                // Mode 1: BG1/BG2は4bpp、BG3は2bpp
                if self.effective_main_screen_designation() & 0x01 != 0
                    && !self.should_mask_bg(x, 0, true)
                {
                    let (color, priority) = self.render_bg_4bpp_with_priority(x, y, 0);
                    if color != 0 {
                        bg_results.push((color, priority, 0));
                    }
                }
                if self.effective_main_screen_designation() & 0x02 != 0
                    && !self.should_mask_bg(x, 1, true)
                {
                    let (color, priority) = self.render_bg_4bpp_with_priority(x, y, 1);
                    if color != 0 {
                        bg_results.push((color, priority, 1));
                    }
                }
                if self.effective_main_screen_designation() & 0x04 != 0 {
                    let (color, priority) = self.render_bg_mode0_with_priority(x, y, 2);
                    if color != 0 {
                        bg_results.push((color, priority, 2));
                    }
                }
            }
            4 => {
                // Mode 4: BG1は8bpp、BG2は2bpp
                if self.effective_main_screen_designation() & 0x01 != 0
                    && !self.should_mask_bg(x, 0, true)
                {
                    let (color, priority) = self.render_bg_8bpp_with_priority(x, y, 0);
                    if color != 0 {
                        bg_results.push((color, priority, 0));
                    }
                }
                if self.effective_main_screen_designation() & 0x02 != 0
                    && !self.should_mask_bg(x, 1, true)
                {
                    let (color, priority) = self.render_bg_mode0_with_priority(x, y, 1);
                    if color != 0 {
                        bg_results.push((color, priority, 1));
                    }
                }
            }
            2 => {
                // Mode 2: BG1/BG2は4bpp（オフセットパータイル機能付き）
                if self.effective_main_screen_designation() & 0x01 != 0 {
                    let (color, priority) = self.render_bg_mode2_window_aware(x, y, 0, true);
                    if color != 0 {
                        bg_results.push((color, priority, 0));
                    }
                }
                if self.effective_main_screen_designation() & 0x02 != 0 {
                    let (color, priority) = self.render_bg_mode2_window_aware(x, y, 1, true);
                    if color != 0 {
                        bg_results.push((color, priority, 1));
                    }
                }
            }
            3 => {
                // Mode 3: BG1は8bpp、BG2は4bpp
                if self.effective_main_screen_designation() & 0x01 != 0
                    && !self.should_mask_bg(x, 0, true)
                {
                    let (color, priority) = self.render_bg_8bpp_with_priority(x, y, 0);
                    if color != 0 {
                        bg_results.push((color, priority, 0));
                    }
                }
                if self.effective_main_screen_designation() & 0x02 != 0
                    && !self.should_mask_bg(x, 1, true)
                {
                    let (color, priority) = self.render_bg_4bpp_with_priority(x, y, 1);
                    if color != 0 {
                        bg_results.push((color, priority, 1));
                    }
                }
            }
            5 => {
                // Mode 5: BG1は4bpp、BG2は2bpp（高解像度）
                // Note: Some games also use BG3 in Mode 5
                if self.effective_main_screen_designation() & 0x01 != 0
                    && !self.should_mask_bg(x, 0, true)
                {
                    let (color, priority) = self.render_bg_mode5_with_priority(x, y, 0);
                    if color != 0 {
                        bg_results.push((color, priority, 0));
                    }
                }
                if self.effective_main_screen_designation() & 0x02 != 0
                    && !self.should_mask_bg(x, 1, true)
                {
                    let (color, priority) = self.render_bg_mode5_with_priority(x, y, 1);
                    if color != 0 {
                        bg_results.push((color, priority, 1));
                    }
                }
                if self.effective_main_screen_designation() & 0x04 != 0
                    && !self.should_mask_bg(x, 2, true)
                {
                    let (color, priority) = self.render_bg_mode5_with_priority(x, y, 2);
                    if color != 0 {
                        bg_results.push((color, priority, 2));
                    }
                }
            }
            6 => {
                // Mode 6: BG1は4bpp（高解像度）
                if self.effective_main_screen_designation() & 0x01 != 0
                    && !self.should_mask_bg(x, 0, true)
                {
                    let (color, priority) = self.render_bg_mode6_with_priority(x, y, 0);
                    if color != 0 {
                        bg_results.push((color, priority, 0));
                    }
                }
            }
            7 => {
                // Mode 7: BG1（EXTBG時はBG2相当もあり）
                if self.effective_main_screen_designation() & 0x01 != 0
                    && !self.should_mask_bg(x, 0, true)
                {
                    // Use with-layer sampler to decide BG1/BG2 based on color index bit7 when EXTBG
                    // Apply flips and outside handling same as render_bg_mode7
                    let sx = if (self.m7sel & 0x01) != 0 {
                        255 - (x as i32)
                    } else {
                        x as i32
                    };
                    let screen_y = y.saturating_add(1);
                    let sy = if (self.m7sel & 0x02) != 0 {
                        255 - (screen_y as i32)
                    } else {
                        screen_y as i32
                    };
                    let (wx, wy) = self.mode7_world_xy_int(sx, sy);
                    let repeat_off = (self.m7sel & 0x80) != 0;
                    let fill_char0 = (self.m7sel & 0x40) != 0;
                    let inside = (0..1024).contains(&wx) && (0..1024).contains(&wy);
                    let (ix, iy, outside) = if inside {
                        (wx, wy, false)
                    } else if !repeat_off {
                        (
                            ((wx % 1024) + 1024) % 1024,
                            ((wy % 1024) + 1024) % 1024,
                            false,
                        )
                    } else {
                        (wx, wy, true)
                    };
                    if outside {
                        if fill_char0 {
                            // Sample both BG1 and BG2 in EXTBG mode
                            for layer in 0..=1u8 {
                                let (col, pr, lid) = self.sample_mode7_for_layer(
                                    0,
                                    (ix & 7) as u8,
                                    (iy & 7) as u8,
                                    layer,
                                );
                                if col != 0 {
                                    bg_results.push((col, pr, lid));
                                }
                            }
                        }
                    } else {
                        let tile_x = (ix >> 3) & 0x7F;
                        let tile_y = (iy >> 3) & 0x7F;
                        let px = (ix & 7) as u8;
                        let py = (iy & 7) as u8;
                        let map_word = ((tile_y as usize) << 7) | (tile_x as usize);
                        let map_index = map_word * 2;
                        if map_index < self.vram.len() {
                            let tile_id = self.vram[map_index] as u16;
                            // Sample both BG1 and BG2 in EXTBG mode
                            for layer in 0..=1u8 {
                                let (col, pr, lid) =
                                    self.sample_mode7_for_layer(tile_id, px, py, layer);
                                if col != 0 {
                                    bg_results.push((col, pr, lid));
                                }
                            }
                        }
                    }
                }
            }
            _ => return (0, 0),
        }

        // プライオリティ順にソート（高い順）
        bg_results.sort_by(|a, b| {
            b.1.cmp(&a.1).then(b.2.cmp(&a.2)) // プライオリティ、BG番号の順
        });

        // 最も高いプライオリティのBGを返す
        bg_results
            .first()
            .map(|(color, priority, _)| (*color, *priority))
            .unwrap_or((0, 0))
    }

    // take_obj_summary moved to sprites.rs

    // prepare_line_obj_pipeline moved to sprites.rs

    // rebuild_oam_cache moved to sprites.rs

    // update_obj_time_over_at_x moved to sprites.rs

    // Precompute per-x window masks for BG/OBJ and color window (dot gating)
    // prepare_line_window_luts moved to window.rs

    // get_sub_sprite_pixel moved to sprites.rs
}

#[cfg(test)]
mod tests {
    use super::Ppu;

    fn write_vram_entry(ppu: &mut Ppu, word_addr: u16, value: u16) {
        let idx = (word_addr as usize) * 2;
        ppu.vram[idx] = value as u8;
        ppu.vram[idx + 1] = (value >> 8) as u8;
    }

    fn write_superfx_4bpp_pixel(buffer: &mut [u8], x: usize, y: usize, color: u8) {
        let cn = ((x & 0xF8) << 1) + (x & 0xF8) + ((y & 0xF8) >> 3);
        let tile_base = cn * 32;
        let row = y & 7;
        let bit = 7 - (x & 7);
        let row01 = tile_base + row * 2;
        let row23 = tile_base + 16 + row * 2;
        if row23 + 1 >= buffer.len() {
            return;
        }
        let mask = 1u8 << bit;
        if (color & 0x01) != 0 {
            buffer[row01] |= mask;
        }
        if (color & 0x02) != 0 {
            buffer[row01 + 1] |= mask;
        }
        if (color & 0x04) != 0 {
            buffer[row23] |= mask;
        }
        if (color & 0x08) != 0 {
            buffer[row23 + 1] |= mask;
        }
    }

    #[test]
    fn mode2_opt_hscroll_uses_full_13bit_value() {
        let mut ppu = Ppu::new();
        ppu.bg_mode = 2;
        ppu.bg3_tilemap_base = 0x1000;
        ppu.bg3_hscroll = 0;
        ppu.bg3_vscroll = 0;
        ppu.bg1_hscroll = 0x0005;
        ppu.bg2_hscroll = 0x0003;

        write_vram_entry(&mut ppu, 0x1000, 0x7234);
        write_vram_entry(&mut ppu, 0x1020, 0x3456);

        ppu.prepare_line_opt_luts();

        assert_eq!(ppu.mode2_opt_hscroll_lut[0][1], 0x1235);
        assert_eq!(ppu.mode2_opt_hscroll_lut[1][1], 0x1233);
    }

    #[test]
    fn mode2_opt_vscroll_uses_full_13bit_value() {
        let mut ppu = Ppu::new();
        ppu.bg_mode = 2;
        ppu.bg3_tilemap_base = 0x1000;
        ppu.bg3_hscroll = 0;
        ppu.bg3_vscroll = 0;
        ppu.bg1_vscroll = 0x0007;
        ppu.bg2_vscroll = 0x0009;

        write_vram_entry(&mut ppu, 0x1000, 0x2000);
        write_vram_entry(&mut ppu, 0x1020, 0x3456);

        ppu.prepare_line_opt_luts();

        assert_eq!(ppu.mode2_opt_vscroll_lut[0][1], 0x1456);
        assert_eq!(ppu.mode2_opt_vscroll_lut[1][1], 0x0009);
    }

    #[test]
    fn mode2_opt_rows_ignore_current_scanline() {
        let mut ppu = Ppu::new();
        ppu.bg_mode = 2;
        ppu.bg3_tilemap_base = 0x1000;
        ppu.bg3_hscroll = 0;
        ppu.bg3_vscroll = 0;
        ppu.bg1_hscroll = 0x0005;
        ppu.bg1_vscroll = 0x0007;
        ppu.scanline = 16;

        // Even on scanline 16, OPT still uses the row pair selected by BG3VOFS.
        write_vram_entry(&mut ppu, 0x1000, 0x3234);
        write_vram_entry(&mut ppu, 0x1020, 0x3456);
        write_vram_entry(&mut ppu, 0x1040, 0x2234);
        write_vram_entry(&mut ppu, 0x1060, 0x2678);

        ppu.prepare_line_opt_luts();

        assert_eq!(ppu.mode2_opt_hscroll_lut[0][1], 0x1235);
        assert_eq!(ppu.mode2_opt_vscroll_lut[0][1], 0x1456);
    }

    #[test]
    fn mode2_opt_rows_follow_bg3_vscroll_pair() {
        let mut ppu = Ppu::new();
        ppu.bg_mode = 2;
        ppu.bg3_tilemap_base = 0x1000;
        ppu.bg3_hscroll = 0;
        ppu.bg3_vscroll = 16;
        ppu.bg1_hscroll = 0x0005;
        ppu.bg1_vscroll = 0x0007;
        ppu.scanline = 0;

        write_vram_entry(&mut ppu, 0x1000, 0x3234);
        write_vram_entry(&mut ppu, 0x1020, 0x3456);
        write_vram_entry(&mut ppu, 0x1040, 0x2234);
        write_vram_entry(&mut ppu, 0x1060, 0x2678);

        ppu.prepare_line_opt_luts();

        assert_eq!(ppu.mode2_opt_hscroll_lut[0][1], 0x0235);
        assert_eq!(ppu.mode2_opt_vscroll_lut[0][1], 0x0678);
    }

    #[test]
    fn mode2_opt_column_tracks_layer_fine_scroll() {
        let mut ppu = Ppu::new();
        ppu.bg_mode = 2;
        ppu.bg1_hscroll = 0x0005;
        ppu.bg2_hscroll = 0x0003;

        assert_eq!(ppu.mode2_opt_column(0, 0), 0);
        assert_eq!(ppu.mode2_opt_column(2, 0), 0);
        assert_eq!(ppu.mode2_opt_column(3, 0), 1);
        assert_eq!(ppu.mode2_opt_column(10, 0), 1);
        assert_eq!(ppu.mode2_opt_column(11, 0), 2);

        assert_eq!(ppu.mode2_opt_column(0, 1), 0);
        assert_eq!(ppu.mode2_opt_column(4, 1), 0);
        assert_eq!(ppu.mode2_opt_column(5, 1), 1);
        assert_eq!(ppu.mode2_opt_column(12, 1), 1);
        assert_eq!(ppu.mode2_opt_column(13, 1), 2);
    }

    #[test]
    fn mode2_opt_lookup_respects_bg3_large_tiles() {
        let mut ppu = Ppu::new();
        ppu.bg_mode = 2;
        ppu.bg3_tilemap_base = 0x1000;
        ppu.bg3_hscroll = 0;
        ppu.bg3_vscroll = 0;
        ppu.bg_tile_16[2] = true;
        ppu.bg1_hscroll = 0x0005;

        write_vram_entry(&mut ppu, 0x1000, 0x3234);
        write_vram_entry(&mut ppu, 0x1001, 0x2AAA);
        write_vram_entry(&mut ppu, 0x1020, 0x0000);
        write_vram_entry(&mut ppu, 0x1021, 0x0000);

        ppu.prepare_line_opt_luts();

        assert_eq!(ppu.mode2_opt_hscroll_lut[0][1], 0x1235);
        assert_eq!(ppu.mode2_opt_hscroll_lut[0][2], 0x1235);
        assert_eq!(ppu.mode2_opt_hscroll_lut[0][3], 0x0AAD);
    }

    #[test]
    fn superfx_direct_default_x_offset_uses_startup_logo_offset_for_sparse_192_line_buffer() {
        let mut buffer = vec![0; 24_576];
        buffer[..900].fill(1);

        assert_eq!(
            Ppu::default_superfx_direct_x_offset(&buffer, 192, 4, 2, 135),
            -16
        );
        assert_eq!(
            Ppu::default_superfx_direct_y_offset(&buffer, 192, 4, 2, 135),
            -16
        );
    }

    #[test]
    fn superfx_direct_default_x_offset_centers_224px_scene_viewport_after_logo() {
        let mut buffer = vec![0; 24_576];
        write_superfx_4bpp_pixel(&mut buffer, 0, 16, 1);
        write_superfx_4bpp_pixel(&mut buffer, 223, 174, 1);

        assert_eq!(
            Ppu::default_superfx_direct_x_offset(&buffer, 192, 4, 2, 420),
            -16
        );
        assert_eq!(
            Ppu::default_superfx_direct_y_offset(&buffer, 192, 4, 2, 420),
            0
        );
    }

    #[test]
    fn superfx_direct_y_offset_centers_forced_blank_192_line_buffer_even_when_dense() {
        let mut ppu = Ppu::new();
        ppu.screen_display = 0x80;
        let buffer = vec![0xFF; 24_576];

        ppu.set_superfx_direct_buffer(buffer, 192, 4, 2);

        assert_eq!(ppu.superfx_direct_default_y_offset, -16);
    }

    #[test]
    fn superfx_direct_y_offset_uses_scene_viewport_after_unblank() {
        let mut ppu = Ppu::new();
        ppu.screen_display = 0x0F;
        let buffer = vec![0xFF; 24_576];

        ppu.set_superfx_direct_buffer(buffer, 192, 4, 2);

        assert_eq!(ppu.superfx_direct_default_y_offset, 0);
    }

    #[test]
    fn superfx_direct_default_x_offset_stays_stable_for_sparse_later_scene_buffers() {
        let mut buffer = vec![0; 24_576];
        write_superfx_4bpp_pixel(&mut buffer, 112, 96, 1);
        write_superfx_4bpp_pixel(&mut buffer, 120, 104, 1);

        assert_eq!(
            Ppu::default_superfx_direct_x_offset(&buffer, 192, 4, 2, 900),
            -16
        );
    }

    #[test]
    fn mode2_bg1_prefers_authoritative_superfx_source_when_present() {
        let mut ppu = Ppu::new();
        ppu.screen_display = 0x0F;
        ppu.bg_mode = 2;
        ppu.set_superfx_authoritative_bg1_source(true);
        ppu.bg1_tilemap_base = 0x0000;
        ppu.bg1_tile_base = 0x0020;
        write_vram_entry(&mut ppu, 0x0007, 0x0001);
        // Standard BG1 path would yield palette index 1 at screen x=56,y=0.
        ppu.vram[0x60] = 0x80;
        ppu.write_cgram_color(1, 0x001F);
        // SuperFX direct path yields palette index 2 at the same dot.
        ppu.write_cgram_color(2, 0x03E0);
        let mut direct = vec![0x00; 32];
        direct[1] = 0x80;
        ppu.set_superfx_direct_buffer(direct, 192, 4, 2);

        let (color, priority) = ppu.render_bg_mode2(56, 0, 0);

        assert_eq!(color, ppu.cgram_to_rgb(2));
        assert_eq!(priority, 1);
    }

    #[test]
    fn mode2_bg1_ignores_superfx_direct_by_default() {
        let mut ppu = Ppu::new();
        ppu.screen_display = 0x0F;
        ppu.bg_mode = 2;
        ppu.bg1_tilemap_base = 0x0000;
        ppu.bg1_tile_base = 0x0020;
        write_vram_entry(&mut ppu, 0x0007, 0x0001);
        ppu.vram[0x60] = 0x80;
        ppu.write_cgram_color(1, 0x001F);
        ppu.write_cgram_color(2, 0x03E0);
        let mut direct = vec![0x00; 32];
        direct[1] = 0x80;
        ppu.set_superfx_direct_buffer(direct, 192, 4, 2);

        let (color, priority) = ppu.render_bg_mode2(56, 0, 0);

        assert_eq!(color, ppu.cgram_to_rgb(1));
        assert_eq!(priority, 0);
    }

    #[test]
    fn authoritative_superfx_bg1_bypasses_window_mask() {
        let mut ppu = Ppu::new();
        ppu.bg_mode = 2;
        ppu.set_superfx_authoritative_bg1_source(true);
        ppu.tmw_mask = 0x01;
        ppu.window_bg_mask[0] = 0x02;
        ppu.window1_left = 0;
        ppu.window1_right = 255;
        let mut direct = vec![0x00; 32];
        direct[0] = 0x80;
        ppu.set_superfx_direct_buffer(direct, 192, 4, 2);

        assert!(ppu.has_authoritative_superfx_bg1_source());
        assert!(!ppu.should_mask_bg(32, 0, true));
    }

    #[test]
    fn mode2_bg1_uses_superfx_direct_when_standard_source_is_transparent() {
        let mut ppu = Ppu::new();
        ppu.screen_display = 0x0F;
        ppu.bg_mode = 2;
        ppu.set_superfx_authoritative_bg1_source(true);
        ppu.bg1_tilemap_base = 0x0000;
        ppu.bg1_tile_base = 0x0020;
        write_vram_entry(&mut ppu, 0x0007, 0x0000);
        ppu.write_cgram_color(2, 0x001F);
        let mut direct = vec![0x00; 32];
        direct[1] = 0x80;
        ppu.set_superfx_direct_buffer(direct, 192, 4, 2);

        let (color, priority) = ppu.render_bg_mode2(56, 0, 0);

        assert_eq!(color, ppu.cgram_to_rgb(2));
        assert_eq!(priority, 1);
    }

    #[test]
    fn mode2_bg1_uses_superfx_direct_as_generic_fallback_when_standard_source_is_transparent() {
        let mut ppu = Ppu::new();
        ppu.screen_display = 0x0F;
        ppu.bg_mode = 2;
        ppu.bg1_tilemap_base = 0x0000;
        ppu.bg1_tile_base = 0x0020;
        write_vram_entry(&mut ppu, 0x0007, 0x0000);
        ppu.write_cgram_color(2, 0x001F);
        let mut direct = vec![0x00; 32];
        direct[1] = 0x80;
        ppu.set_superfx_direct_buffer(direct, 192, 4, 2);

        let (color, priority) = ppu.render_bg_mode2(56, 0, 0);

        assert_eq!(color, ppu.cgram_to_rgb(2));
        assert_eq!(priority, 1);
    }

    #[test]
    fn mode2_bg1_window_mask_still_allows_superfx_direct_generic_fallback() {
        let mut ppu = Ppu::new();
        ppu.screen_display = 0x0F;
        ppu.bg_mode = 2;
        ppu.bg1_tilemap_base = 0x0000;
        ppu.bg1_tile_base = 0x0020;
        write_vram_entry(&mut ppu, 0x0007, 0x0000);
        ppu.tmw_mask = 0x01;
        ppu.window_bg_mask[0] = 0x02;
        ppu.window1_left = 0;
        ppu.window1_right = 255;
        ppu.write_cgram_color(2, 0x001F);
        let mut direct = vec![0x00; 32];
        direct[1] = 0x80;
        ppu.set_superfx_direct_buffer(direct, 192, 4, 2);

        let (color, priority) = ppu.render_bg_mode2_window_aware(56, 0, 0, true);

        assert_eq!(color, ppu.cgram_to_rgb(2));
        assert_eq!(priority, 1);
    }

    #[test]
    fn mode2_bg1_window_mask_does_not_leak_standard_tile_when_not_transparent() {
        let mut ppu = Ppu::new();
        ppu.screen_display = 0x0F;
        ppu.bg_mode = 2;
        ppu.bg1_tilemap_base = 0x0000;
        ppu.bg1_tile_base = 0x0020;
        write_vram_entry(&mut ppu, 0x0007, 0x0001);
        ppu.vram[0x60] = 0x80;
        ppu.write_cgram_color(1, 0x001F);
        ppu.write_cgram_color(2, 0x03E0);
        ppu.tmw_mask = 0x01;
        ppu.window_bg_mask[0] = 0x02;
        ppu.window1_left = 0;
        ppu.window1_right = 255;
        let mut direct = vec![0x00; 32];
        direct[1] = 0x80;
        ppu.set_superfx_direct_buffer(direct, 192, 4, 2);

        let (color, priority) = ppu.render_bg_mode2_window_aware(56, 0, 0, true);

        assert_eq!(color, 0);
        assert_eq!(priority, 0);
    }

    #[test]
    fn mode2_bg1_uses_superfx_tile_fallback_when_direct_buffer_is_missing() {
        let mut ppu = Ppu::new();
        ppu.bg_mode = 2;
        ppu.set_superfx_authoritative_bg1_source(true);
        ppu.bg1_tilemap_base = 0x0000;
        ppu.bg1_tile_base = 0x0020;
        write_vram_entry(&mut ppu, 0x0000, 0x0000);
        ppu.cgram[2] = 0x1F;
        ppu.cgram[3] = 0x00;
        ppu.cgram[4] = 0xE0;
        ppu.cgram[5] = 0x03;
        let mut tile = vec![0x00; 32];
        tile[1] = 0x80;
        ppu.set_superfx_tile_buffer(tile, 4, 2);

        let (color, priority) = ppu.render_bg_mode2(0, 64, 0);

        assert_eq!(color, ppu.cgram_to_rgb(2));
        assert_eq!(priority, 0);
    }

    #[test]
    fn authoritative_superfx_direct_keeps_zero_pixels_transparent() {
        let mut ppu = Ppu::new();
        ppu.screen_display = 0x0F;
        ppu.bg_mode = 2;
        ppu.set_superfx_authoritative_bg1_source(true);
        ppu.bg1_tile_base = 0x0020;
        ppu.vram[0x121] = 0x80;
        ppu.cgram[2] = 0x1F;
        ppu.cgram[3] = 0x00;
        ppu.set_superfx_direct_buffer(vec![0x00; 32], 192, 4, 2);
        ppu.set_superfx_tile_buffer(vec![0x00; 32], 4, 2);

        let (color, priority) = ppu.render_bg_mode2(56, 0, 0);

        assert_eq!(color, 0);
        assert_eq!(priority, 0);
    }

    #[test]
    fn mode2_bg1_falls_back_to_tile_snapshot_when_direct_buffer_pixel_is_zero() {
        let mut ppu = Ppu::new();
        ppu.screen_display = 0x0F;
        ppu.bg1_tile_base = 0x0020;
        ppu.vram[0x121] = 0x80;
        ppu.cgram[2] = 0x1F;
        ppu.cgram[3] = 0x00;
        ppu.set_superfx_direct_buffer(vec![0x00; 32], 192, 4, 2);
        ppu.set_superfx_tile_buffer(vec![0x00; 32], 4, 2);

        let (color, priority) = ppu.render_bg_superfx_direct(56, 0);

        assert_eq!(color, ppu.cgram_to_rgb(2));
        assert_eq!(priority, 0);
    }

    fn configure_starfox_title_mode1(ppu: &mut Ppu) {
        ppu.bg_mode = 1;
        ppu.main_screen_designation = 0x07;
        ppu.sub_screen_designation = 0x07;
        ppu.tmw_mask = 0;
        ppu.tsw_mask = 0;
        ppu.cgwsel = 0x02;
        ppu.cgadsub = 0x50;
        ppu.bg1_hscroll = 0;
        ppu.bg1_vscroll = 0;
        ppu.bg2_hscroll = 0;
        ppu.bg2_vscroll = 0x0101;
        ppu.bg3_hscroll = 0x03FC;
        ppu.bg3_vscroll = 0x0009;
        ppu.bg1_tilemap_base = 0x2C00;
        ppu.bg2_tilemap_base = 0x7000;
        ppu.bg3_tilemap_base = 0x6800;
        ppu.bg1_tile_base = 0x3000;
        ppu.bg2_tile_base = 0x5000;
        ppu.bg3_tile_base = 0x7000;
    }

    #[test]
    fn starfox_title_suppression_clears_bg1_only_for_title_layout() {
        let mut ppu = Ppu::new();
        configure_starfox_title_mode1(&mut ppu);
        ppu.set_starfox_title_bg1_suppression(true);

        assert_eq!(ppu.effective_main_screen_designation(), 0x06);
    }

    #[test]
    fn starfox_title_layout_keeps_bg1_when_suppression_is_disabled() {
        let mut ppu = Ppu::new();
        configure_starfox_title_mode1(&mut ppu);

        assert!(ppu.starfox_title_layout_active());
        assert_eq!(ppu.effective_main_screen_designation(), 0x07);
    }

    #[test]
    fn starfox_title_suppression_keeps_bg1_for_other_mode1_layouts() {
        let mut ppu = Ppu::new();
        configure_starfox_title_mode1(&mut ppu);
        ppu.bg1_vscroll = 1;
        ppu.set_starfox_title_bg1_suppression(true);

        assert_eq!(ppu.effective_main_screen_designation(), 0x07);
    }

    #[test]
    fn sub_screen_without_enabled_layers_are_marked_transparent() {
        let mut ppu = Ppu::new();
        ppu.cgram[0] = 0x1F;
        ppu.cgram[1] = 0x00;
        ppu.fixed_color = 0x7FFF;

        let (color, layer_id, transparent, obj_math_allowed) =
            ppu.render_sub_screen_pixel_with_layer_internal(0, 0, false, false);

        assert_eq!(color, ppu.cgram_to_rgb(0));
        assert_eq!(layer_id, 5);
        assert!(transparent);
        assert!(obj_math_allowed);
    }
}
