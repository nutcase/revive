use super::{trace_sample_dot_config, Ppu};
mod bg_fetch;
mod color_math;
mod composite;
mod mode7;
mod screen_pixel;
mod superfx_direct;
mod trace;

use self::trace::env_presence_flag;

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

    // take_obj_summary moved to sprites.rs

    // prepare_line_obj_pipeline moved to sprites.rs

    // rebuild_oam_cache moved to sprites.rs

    // update_obj_time_over_at_x moved to sprites.rs

    // Precompute per-x window masks for BG/OBJ and color window (dot gating)
    // prepare_line_window_luts moved to window.rs

    // Prepare per-tile-column offset-per-tile (OPT) tables for Mode 2.
    //
    // Reference: https://snes.nesdev.org/wiki/Offset-per-tile
    pub(crate) fn prepare_line_opt_luts(&mut self) {
        if self.bg_mode != 2 {
            return;
        }

        // DEBUG: bypass OPT to test raw tile rendering
        if env_presence_flag("BYPASS_OPT") {
            for col in 0..=32usize {
                self.mode2_opt_hscroll_lut[0][col] = self.bg1_hscroll;
                self.mode2_opt_vscroll_lut[0][col] = self.bg1_vscroll;
                self.mode2_opt_hscroll_lut[1][col] = self.bg2_hscroll;
                self.mode2_opt_vscroll_lut[1][col] = self.bg2_vscroll;
            }
            return;
        }

        // Column 0 is never affected (per OPT rules).
        self.mode2_opt_hscroll_lut[0][0] = self.bg1_hscroll;
        self.mode2_opt_vscroll_lut[0][0] = self.bg1_vscroll;
        self.mode2_opt_hscroll_lut[1][0] = self.bg2_hscroll;
        self.mode2_opt_vscroll_lut[1][0] = self.bg2_vscroll;

        for col in 1..=32usize {
            // Match bsnes: the lookup coordinates are pixel-based and therefore
            // respect BG3's tile size instead of assuming 8x8 entries.
            let lookup_x = ((col as u16 - 1) * 8).wrapping_add(self.bg3_hscroll & !0x0007);
            let h_entry = self.read_bg_tilemap_entry_word_at_pixel(2, lookup_x, self.bg3_vscroll);
            let v_entry = self.read_bg_tilemap_entry_word_at_pixel(
                2,
                lookup_x,
                self.bg3_vscroll.wrapping_add(8),
            );

            // bit13 applies to BG1, bit14 applies to BG2.
            let bg1_apply = (h_entry & 0x2000) != 0;
            let bg2_apply = (h_entry & 0x4000) != 0;
            let bg1_apply_v = (v_entry & 0x2000) != 0;
            let bg2_apply_v = (v_entry & 0x4000) != 0;

            // OPT scroll values are 13-bit. The low 3 bits are ignored for horizontal
            // replacement, but vertical replacement uses the full 13-bit value.
            let h_val = h_entry & 0x1FFF;
            let v_val = v_entry & 0x1FFF;

            let bg1_fine = self.bg1_hscroll & 0x0007;
            let bg2_fine = self.bg2_hscroll & 0x0007;

            let bg1_h = if bg1_apply {
                (h_val & !0x0007) | bg1_fine
            } else {
                self.bg1_hscroll
            };
            let bg2_h = if bg2_apply {
                (h_val & !0x0007) | bg2_fine
            } else {
                self.bg2_hscroll
            };
            let bg1_v = if bg1_apply_v { v_val } else { self.bg1_vscroll };
            let bg2_v = if bg2_apply_v { v_val } else { self.bg2_vscroll };

            self.mode2_opt_hscroll_lut[0][col] = bg1_h;
            self.mode2_opt_vscroll_lut[0][col] = bg1_v;
            self.mode2_opt_hscroll_lut[1][col] = bg2_h;
            self.mode2_opt_vscroll_lut[1][col] = bg2_v;
        }
    }

    #[inline]
    pub(crate) fn update_line_render_state(&mut self) {
        let main = self.effective_main_screen_designation();
        let sub = self.sub_screen_designation;
        self.line_main_enables = main;
        self.line_sub_enables = sub;
        self.line_main_has_bg = (main & 0x0F) != 0;
        self.line_main_has_obj = (main & 0x10) != 0;
        self.line_sub_has_bg = (sub & 0x0F) != 0;
        self.line_sub_has_obj = (sub & 0x10) != 0;
        self.line_hires_out = self.pseudo_hires || matches!(self.bg_mode, 5 | 6);
        let color_mask = self.cgadsub & 0x3F;
        let use_sub_src = (self.cgwsel & 0x02) != 0;
        self.line_color_math_enabled = (self.cgwsel & 0xF0) != 0 || color_mask != 0;
        self.line_need_subscreen = self.line_hires_out || (use_sub_src && color_mask != 0);
    }

    // モザイク効果（座標そのものに適用。インターレースのY変換は呼び出し側で行う）
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
