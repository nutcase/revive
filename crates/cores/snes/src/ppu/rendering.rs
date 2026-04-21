use super::{trace_sample_dot_config, BgMapCache, BgRowCache, Ppu};
use std::sync::OnceLock;
use wide::u32x8;

mod superfx_direct;

fn env_presence_flag(name: &'static str) -> bool {
    if cfg!(test) {
        return std::env::var_os(name).is_some();
    }

    match name {
        "BYPASS_OPT" => {
            static VALUE: OnceLock<bool> = OnceLock::new();
            *VALUE.get_or_init(|| std::env::var_os("BYPASS_OPT").is_some())
        }
        _ => std::env::var_os(name).is_some(),
    }
}

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

    // プライオリティベースのピクセル合成
    #[allow(dead_code)]
    pub(crate) fn composite_pixel(
        &self,
        bg_color: u32,
        bg_priority: u8,
        sprite_color: u32,
        sprite_priority: u8,
    ) -> u32 {
        let (final_color, _layer_id) = self.composite_pixel_with_layer(
            bg_color,
            bg_priority,
            0,
            sprite_color,
            sprite_priority,
        );
        // 画面間のカラー演算は apply_color_math_screens() で一括適用する。
        // ここではレイヤー合成のみ行う。
        final_color
    }

    pub(crate) fn composite_pixel_with_layer(
        &self,
        bg_color: u32,
        bg_priority: u8,
        bg_layer_id: u8,
        sprite_color: u32,
        sprite_priority: u8,
    ) -> (u32, u8) {
        // 透明なピクセルをチェック
        let sprite_transparent = self.is_transparent_pixel(sprite_color);
        let bg_transparent = self.is_transparent_pixel(bg_color);
        if sprite_transparent && bg_transparent {
            // Fully transparent: caller will fall back to backdrop color.
            return (0, 5);
        }
        if sprite_transparent {
            return (bg_color, bg_layer_id);
        }
        if bg_transparent {
            return (sprite_color, 4);
        }

        // Unified priority model via z-rank table.
        let z_obj = self.z_rank_for_obj(sprite_priority);
        let z_bg = self.z_rank_for_bg(bg_layer_id, bg_priority);
        if z_obj >= z_bg {
            (sprite_color, 4)
        } else {
            (bg_color, bg_layer_id)
        }
    }

    #[inline]
    pub(crate) fn z_rank_for_obj(&self, pr: u8) -> i16 {
        match self.bg_mode {
            7 => match pr {
                3 => crate::debug_flags::m7_z_obj3(),
                2 => crate::debug_flags::m7_z_obj2(),
                1 => crate::debug_flags::m7_z_obj1(),
                _ => crate::debug_flags::m7_z_obj0(),
            },
            _ => match pr {
                3 => 90,
                2 => 70,
                1 => 50,
                _ => 40,
            },
        }
    }

    #[inline]
    pub(crate) fn z_rank_for_bg(&self, layer: u8, pr: u8) -> i16 {
        match self.bg_mode {
            0 => {
                // Priority order for Mode 0 (front -> back):
                //   S3 1H 2H S2 1L 2L S1 3H 4H S0 3L 4L
                // Where:
                //   S3..S0 = OBJ priorities 3..0
                //   1..4   = BG1..BG4 (layer 0..3)
                //   H/L    = tile priority bit (pr=1/0)
                match (layer, pr >= 1) {
                    // BG1
                    (0, true) => 85,  // between S3(90) and S2(70)
                    (0, false) => 65, // between S2(70) and S1(50)
                    // BG2
                    (1, true) => 80,  // between BG1H and S2
                    (1, false) => 60, // between BG1L and S1
                    // BG3
                    (2, true) => 45,  // between S1(50) and S0(40)
                    (2, false) => 35, // below S0
                    // BG4
                    (3, true) => 42,  // between BG3H and S0
                    (3, false) => 30, // bottom-most BG
                    _ => 30,
                }
            }
            1 => {
                // Priority order for Mode 1 (front -> back):
                //   If BGMODE bit3 (BG3 priority) = 1:
                //     3H S3 1H 2H S2 1L 2L S1 S0 3L
                //   If BGMODE bit3 = 0:
                //     S3 1H 2H S2 1L 2L S1 3H S0 3L
                // (BG4 not present.)
                let bg3_slot_high = self.mode1_bg3_priority; // $2105 bit3
                match layer {
                    // BG3
                    2 => {
                        if pr >= 1 {
                            if bg3_slot_high {
                                95 // above S3
                            } else {
                                45 // between S1 and S0
                            }
                        } else {
                            35 // 3L below S0
                        }
                    }
                    // BG2
                    1 => {
                        if pr >= 1 {
                            80 // between S3 and S2
                        } else {
                            60 // between S2 and S1
                        }
                    }
                    // BG1 (default)
                    _ => {
                        if pr >= 1 {
                            85 // between S3 and S2, above BG2H
                        } else {
                            65 // between S2 and S1, above BG2L
                        }
                    }
                }
            }
            2..=4 => {
                // Priority order for Modes 2/3/4 (front -> back):
                //   S3 1H S2 2H S1 1L S0 2L
                match (layer, pr >= 1) {
                    // BG1
                    (0, true) => 80,  // between S3 and S2
                    (0, false) => 45, // between S1 and S0
                    // BG2
                    (1, true) => 60,  // between S2 and S1
                    (1, false) => 35, // below S0
                    _ => 35,
                }
            }
            5 | 6 => {
                // Mode 5/6: BG1 and BG2 have distinct priority slots.
                // Order (front->back) from SNESdev: OBJ3, BG1H, OBJ2, BG2H, OBJ1, BG1L, OBJ0, BG2L.
                // We map into the generic OBJ z-ranks (90/70/50/40) by placing BG ranks between them.
                match (layer, pr) {
                    (0, 1) => 80, // BG1 high
                    (1, 1) => 60, // BG2 high
                    (0, _) => 45, // BG1 low
                    _ => 35,      // BG2 low (and any other BG)
                }
            }
            7 => {
                // EXTBG z-ranks are tunable via env for precise ordering experiments
                if layer == 1 {
                    crate::debug_flags::m7_z_bg2()
                } else {
                    crate::debug_flags::m7_z_bg1()
                }
            }
            _ => {
                if pr >= 1 {
                    60
                } else {
                    40
                }
            }
        }
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

    #[inline]
    pub(crate) fn render_mode7_with_layer(&mut self, x: u16, y: u16) -> (u32, u8, u8) {
        // Mode 7: affine transform into 1024x1024 world; tiles: 8x8 8bpp, map: 128x128 bytes.
        // Helper: sample for a desired layer (0:BG1, 1:BG2 when EXTBG). Applies mosaic per layer.
        let sample_for_layer = |desired_layer: u8| -> (u32, u8, u8, bool, bool, bool, bool) {
            // Screen mosaic per layer
            let (mx, my) = self.apply_mosaic(x, y, desired_layer);
            // Apply flips around 255
            let sx = if (self.m7sel & 0x01) != 0 {
                255 - (mx as i32)
            } else {
                mx as i32
            };
            let sy = if (self.m7sel & 0x02) != 0 {
                255 - (my as i32)
            } else {
                my as i32
            };

            let (wx, wy) = self.mode7_world_xy_int(sx, sy);
            let repeat_off = (self.m7sel & 0x80) != 0; // R
            let fill_char0 = (self.m7sel & 0x40) != 0; // F (only when R=1)
            let inside = (0..1024).contains(&wx) && (0..1024).contains(&wy);
            let (ix, iy, outside, wrapped) = if inside {
                (wx, wy, false, false)
            } else if !repeat_off {
                // 1024 is a power of two, so masking matches Euclidean modulo for signed i32.
                (wx & 0x03FF, wy & 0x03FF, false, true)
            } else {
                (wx, wy, true, false)
            };

            if outside {
                if !fill_char0 {
                    return (0, 0, desired_layer, false, true, false, false);
                }
                let px = (ix & 7) as u8;
                let py = (iy & 7) as u8;
                if self.extbg {
                    let (c, pr, lid) = self.sample_mode7_for_layer(0, px, py, desired_layer);
                    return (c, pr, lid, false, true, true, false);
                } else {
                    let (c, pr) = self.sample_mode7_color_only(0, px, py);
                    return (c, pr, 0, false, true, true, false);
                }
            }

            // In-bounds or wrapped sampling
            let tile_x = (ix >> 3) & 0x7F; // 0..127
            let tile_y = (iy >> 3) & 0x7F; // 0..127
            let px = (ix & 7) as u8;
            let py = (iy & 7) as u8;
            // Mode 7 VRAM layout:
            // - Tilemap: low byte of VRAM words 0x0000..0x3FFF (128x128 bytes)
            // - Tile data: high byte of the same VRAM words (256 tiles * 64 bytes = 16384 bytes)
            let map_word = ((tile_y as usize) << 7) | (tile_x as usize);
            let map_index = map_word * 2;
            if map_index >= self.vram.len() {
                return (0, 0, desired_layer, wrapped, false, false, false);
            }
            let tile_id = self.vram[map_index] as u16;

            let edge = ix == 0 || ix == 1023 || iy == 0 || iy == 1023;
            if self.extbg {
                let (c, pr, lid) = self.sample_mode7_for_layer(tile_id, px, py, desired_layer);
                (c, pr, lid, wrapped, false, false, edge)
            } else {
                let (c, pr) = self.sample_mode7_color_only(tile_id, px, py);
                (c, pr, 0, wrapped, false, false, edge)
            }
        };

        if self.extbg {
            let (c2, p2, lid2, wrap2, clip2, fill2, edge2) = sample_for_layer(1);
            let (c1, p1, lid1, wrap1, clip1, fill1, edge1) = sample_for_layer(0);
            // Metrics
            if crate::debug_flags::render_metrics() {
                if wrap1 || wrap2 {
                    self.dbg_m7_wrap = self.dbg_m7_wrap.saturating_add(1);
                }
                if clip1 || clip2 {
                    self.dbg_m7_clip = self.dbg_m7_clip.saturating_add(1);
                }
                if fill1 || fill2 {
                    self.dbg_m7_fill = self.dbg_m7_fill.saturating_add(1);
                }
                if c1 != 0 {
                    self.dbg_m7_bg1 = self.dbg_m7_bg1.saturating_add(1);
                }
                if c2 != 0 {
                    self.dbg_m7_bg2 = self.dbg_m7_bg2.saturating_add(1);
                }
                if edge1 || edge2 {
                    self.dbg_m7_edge = self.dbg_m7_edge.saturating_add(1);
                }
            }
            // Prefer BG1 over BG2 when both present; actual sort happens in z-rank stage.
            if c1 != 0 {
                return (c1, p1, lid1);
            }
            if c2 != 0 {
                return (c2, p2, lid2);
            }
            (0, 0, 0)
        } else {
            let (c, p, lid, wrapped, clipped, filled, edge) = sample_for_layer(0);
            if crate::debug_flags::render_metrics() {
                if wrapped {
                    self.dbg_m7_wrap = self.dbg_m7_wrap.saturating_add(1);
                }
                if clipped {
                    self.dbg_m7_clip = self.dbg_m7_clip.saturating_add(1);
                }
                if filled {
                    self.dbg_m7_fill = self.dbg_m7_fill.saturating_add(1);
                }
                if c != 0 {
                    self.dbg_m7_bg1 = self.dbg_m7_bg1.saturating_add(1);
                }
                if edge {
                    self.dbg_m7_edge = self.dbg_m7_edge.saturating_add(1);
                }
            }
            (c, p, lid)
        }
    }

    // Render a single Mode 7 layer for EXTBG mode.
    // desired_layer: 0=BG1, 1=BG2
    #[inline]
    pub(crate) fn render_mode7_single_layer(
        &mut self,
        x: u16,
        y: u16,
        desired_layer: u8,
    ) -> (u32, u8, u8) {
        let (mx, my) = self.apply_mosaic(x, y, desired_layer);
        let sx = if (self.m7sel & 0x01) != 0 {
            255 - (mx as i32)
        } else {
            mx as i32
        };
        let sy = if (self.m7sel & 0x02) != 0 {
            255 - (my as i32)
        } else {
            my as i32
        };
        let (wx, wy) = self.mode7_world_xy_int(sx, sy);
        let repeat_off = (self.m7sel & 0x80) != 0;
        let fill_char0 = (self.m7sel & 0x40) != 0;
        let inside = (0..1024).contains(&wx) && (0..1024).contains(&wy);
        let (ix, iy, outside) = if inside {
            (wx, wy, false)
        } else if !repeat_off {
            (wx & 0x03FF, wy & 0x03FF, false)
        } else {
            (wx, wy, true)
        };
        if outside {
            if !fill_char0 {
                return (0, 0, desired_layer);
            }
            let px = (ix & 7) as u8;
            let py = (iy & 7) as u8;
            return self.sample_mode7_for_layer(0, px, py, desired_layer);
        }
        let tile_x = (ix >> 3) & 0x7F;
        let tile_y = (iy >> 3) & 0x7F;
        let px = (ix & 7) as u8;
        let py = (iy & 7) as u8;
        let map_word = ((tile_y as usize) << 7) | (tile_x as usize);
        let map_index = map_word * 2;
        if map_index >= self.vram.len() {
            return (0, 0, desired_layer);
        }
        let tile_id = self.vram[map_index] as u16;
        self.sample_mode7_for_layer(tile_id, px, py, desired_layer)
    }

    // Color only (legacy callers). Returns (ARGB, priority)
    // SNES Mode 7 tiles are 8x8, 8bpp, linear (64 bytes per tile).
    #[inline]
    pub(crate) fn sample_mode7_color_only(&self, tile_id: u16, px: u8, py: u8) -> (u32, u8) {
        // Mode 7 tile data is stored in the high byte of VRAM words 0x0000..0x3FFF.
        // Treating the high bytes as a contiguous byte array yields 256 tiles * 64 bytes.
        let data_word = ((tile_id as usize) << 6) | ((py as usize) << 3) | (px as usize); // 0..16383
        let addr = data_word * 2 + 1;
        if addr >= self.vram.len() {
            return (0, 0);
        }
        let color_index = self.vram[addr];
        if color_index == 0 {
            return (0, 0);
        }
        // Direct color mode (CGWSEL bit0) for 8bpp BGs; in Mode 7 there are no tilemap palette bits.
        let use_direct_color = (self.cgwsel & 0x01) != 0;
        let color = if use_direct_color {
            self.direct_color_to_rgb(0, color_index)
        } else {
            let palette_index = self.get_bg_palette_index(0, color_index, 8);
            self.cgram_to_rgb(palette_index)
        };
        (color, 1)
    }

    // Sample Mode 7 pixel for a specific layer in EXTBG mode.
    // desired_layer: 0=BG1, 1=BG2
    // BG1: uses full 8-bit color index, single priority level
    // BG2: uses lower 7 bits as color index, bit7 as priority (0 or 1)
    // Both layers sample from the SAME pixel data independently.
    #[inline]
    pub(crate) fn sample_mode7_for_layer(
        &self,
        tile_id: u16,
        px: u8,
        py: u8,
        desired_layer: u8,
    ) -> (u32, u8, u8) {
        let data_word = ((tile_id as usize) << 6) | ((py as usize) << 3) | (px as usize);
        let addr = data_word * 2 + 1;
        if addr >= self.vram.len() {
            return (0, 0, desired_layer);
        }
        let raw = self.vram[addr];
        if desired_layer == 0 {
            // BG1: full 8-bit color, single priority
            if raw == 0 {
                return (0, 0, 0);
            }
            let use_direct_color = (self.cgwsel & 0x01) != 0;
            let color = if use_direct_color {
                self.direct_color_to_rgb(0, raw)
            } else {
                let palette_index = self.get_bg_palette_index(0, raw, 8);
                self.cgram_to_rgb(palette_index)
            };
            (color, 1, 0)
        } else {
            // BG2: lower 7 bits as color, bit7 as priority
            let color_index = raw & 0x7F;
            let priority = (raw >> 7) & 1;
            if color_index == 0 {
                return (0, 0, 1);
            }
            let palette_index = self.get_bg_palette_index(0, color_index, 8);
            let color = self.cgram_to_rgb(palette_index);
            (color, priority, 1)
        }
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

    #[allow(dead_code)]
    pub(crate) fn apply_hires_enhancement(&self, color: u32) -> u32 {
        // 高解像度モード用の色調整（鮮明度向上）
        let r = ((color >> 16) & 0xFF) as u8;
        let g = ((color >> 8) & 0xFF) as u8;
        let b = (color & 0xFF) as u8;

        // 軽微な彩度向上
        let enhanced_r = ((r as u16 * 110 / 100).min(255)) as u8;
        let enhanced_g = ((g as u16 * 110 / 100).min(255)) as u8;
        let enhanced_b = ((b as u16 * 110 / 100).min(255)) as u8;

        0xFF000000 | ((enhanced_r as u32) << 16) | ((enhanced_g as u32) << 8) | (enhanced_b as u32)
    }

    pub(crate) fn apply_brightness(&self, color: u32) -> u32 {
        // Forced blank overrides everything (unless FORCE_DISPLAY or FORCE_NO_BLANK)
        if (self.screen_display & 0x80) != 0 && !self.force_display_active() && !self.force_no_blank
        {
            return 0xFF000000;
        }
        // Apply INIDISP brightness level (0..15). 15 = full.
        let factor = if self.force_display_active() {
            15
        } else {
            (self.brightness as u32).min(15)
        };
        if factor >= 15 {
            return (color & 0x00FFFFFF) | 0xFF000000;
        }
        let r = ((((color >> 16) & 0xFF) * factor / 15) & 0xFF) << 16;
        let g = ((((color >> 8) & 0xFF) * factor / 15) & 0xFF) << 8;
        let b = ((color & 0xFF) * factor / 15) & 0xFF;
        0xFF000000 | r | g | b
    }

    #[inline]
    pub(crate) fn apply_brightness_with_factor(color: u32, factor: u8) -> u32 {
        let factor = (factor as u32).min(15);
        if factor >= 15 {
            return (color & 0x00FF_FFFF) | 0xFF000000;
        }
        let r = ((((color >> 16) & 0xFF) * factor / 15) & 0xFF) << 16;
        let g = ((((color >> 8) & 0xFF) * factor / 15) & 0xFF) << 8;
        let b = ((color & 0xFF) * factor / 15) & 0xFF;
        0xFF000000 | r | g | b
    }

    #[inline]
    pub(crate) fn apply_brightness_simd_block(colors: [u32; 8], factor: u8) -> [u32; 8] {
        let factor = (factor as u32).min(15);
        if factor >= 15 {
            return colors.map(|c| (c & 0x00FF_FFFF) | 0xFF000000);
        }
        let v = u32x8::from(colors);
        let mask = u32x8::splat(0xFF);
        let r = (v >> u32x8::splat(16)) & mask;
        let g = (v >> u32x8::splat(8)) & mask;
        let b = v & mask;
        let f = u32x8::splat(factor);
        let recip = u32x8::splat(0x8889); // exact for 0..3825 when >> 19
        let shift = u32x8::splat(19);
        let r = ((r * f) * recip) >> shift;
        let g = ((g * f) * recip) >> shift;
        let b = ((b * f) * recip) >> shift;
        let out =
            u32x8::splat(0xFF000000) | (r * u32x8::splat(1 << 16)) | (g * u32x8::splat(1 << 8)) | b;
        out.into()
    }

    #[inline]
    pub(crate) fn flush_brightness_simd(&mut self) {
        let len = self.brightness_simd_len as usize;
        if len == 0 {
            return;
        }
        let start = self.brightness_simd_start;
        let factor = self.brightness_simd_factor;
        if start + len <= self.render_framebuffer.len() {
            if len == self.brightness_simd_buf.len() {
                let out = Self::apply_brightness_simd_block(self.brightness_simd_buf, factor);
                self.render_framebuffer[start..start + len].copy_from_slice(&out);
            } else {
                for i in 0..len {
                    let color = self.brightness_simd_buf[i];
                    self.render_framebuffer[start + i] =
                        Self::apply_brightness_with_factor(color, factor);
                }
            }
        }
        self.brightness_simd_len = 0;
    }

    #[inline]
    pub(crate) fn average_rgb(a: u32, b: u32) -> u32 {
        let ar = ((a >> 16) & 0xFF) as u16;
        let ag = ((a >> 8) & 0xFF) as u16;
        let ab = (a & 0xFF) as u16;
        let br = ((b >> 16) & 0xFF) as u16;
        let bg = ((b >> 8) & 0xFF) as u16;
        let bb = (b & 0xFF) as u16;
        let r = ((ar + br) / 2) as u32;
        let g = ((ag + bg) / 2) as u32;
        let bl = ((ab + bb) / 2) as u32;
        0xFF000000 | (r << 16) | (g << 8) | bl
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

    fn read_bg_tilemap_entry_word_at_pixel(&self, bg_num: u8, pixel_x: u16, pixel_y: u16) -> u16 {
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

    // カラー演算機能
    #[allow(dead_code)]
    pub(crate) fn apply_color_math(&self, main_color: u32, layer_id: u8) -> u32 {
        if !self.is_color_math_enabled(layer_id) {
            return main_color;
        }
        // Select subsource: CGWSEL bit1 (1=subscreen, 0=fixed)
        let sub_color = self.fixed_color_to_rgb();
        // Use CGADSUB for add/sub + halve
        let is_addition = (self.cgadsub & 0x80) == 0;
        let halve = (self.cgadsub & 0x40) != 0;
        self.blend_colors(main_color, sub_color, is_addition, halve)
    }

    pub(crate) fn is_color_math_enabled(&self, layer_id: u8) -> bool {
        let bit_mask = Self::color_math_layer_bit(layer_id);
        bit_mask != 0 && (self.cgadsub & bit_mask) != 0
    }

    #[inline]
    pub(crate) fn color_math_layer_bit(layer_id: u8) -> u8 {
        // レイヤーIDに対応するビットをチェック
        match layer_id {
            0 => 0x01, // BG1
            1 => 0x02, // BG2
            2 => 0x04, // BG3
            3 => 0x08, // BG4
            4 => 0x10, // Sprite
            5 => 0x20, // Backdrop
            _ => 0,
        }
    }

    pub(crate) fn fixed_color_to_rgb(&self) -> u32 {
        let r = (self.fixed_color & 0x1F) as u8;
        let g = ((self.fixed_color >> 5) & 0x1F) as u8;
        let b = ((self.fixed_color >> 10) & 0x1F) as u8;

        // 5bitから8bitに拡張
        let r = (r << 3) | (r >> 2);
        let g = (g << 3) | (g >> 2);
        let b = (b << 3) | (b >> 2);

        ((r as u32) << 16) | ((g as u32) << 8) | (b as u32) | 0xFF000000
    }

    pub(crate) fn blend_colors(
        &self,
        color1: u32,
        color2: u32,
        is_addition: bool,
        halve: bool,
    ) -> u32 {
        // Work in 5-bit space (SNES BGR555), then expand to 8-bit.
        //
        // SNES color math order (per docs):
        //   1) main +/- sub/fixed
        //   2) optional /2 (half color math)
        //   3) clamp to 0..31
        //
        // Doing saturating add before halving breaks Add+Half (it would cap at 31, then /2,
        // incorrectly darkening the result). Keep the full sum first, then halve, then clamp.
        let r1 = (((color1 >> 16) & 0xFF) as i16) >> 3;
        let g1 = (((color1 >> 8) & 0xFF) as i16) >> 3;
        let b1 = ((color1 & 0xFF) as i16) >> 3;

        let r2 = (((color2 >> 16) & 0xFF) as i16) >> 3;
        let g2 = (((color2 >> 8) & 0xFF) as i16) >> 3;
        let b2 = ((color2 & 0xFF) as i16) >> 3;

        let (mut r, mut g, mut b) = if is_addition {
            (r1 + r2, g1 + g2, b1 + b2)
        } else {
            (r1 - r2, g1 - g2, b1 - b2)
        };

        if halve {
            // Arithmetic shift matches hardware's divide-by-2 behavior for signed intermediates.
            r >>= 1;
            g >>= 1;
            b >>= 1;
        }

        r = r.clamp(0, 31);
        g = g.clamp(0, 31);
        b = b.clamp(0, 31);

        // Expand back to 8-bit (x<<3 | x>>2 format)
        let r8 = ((r as u32) << 3) | ((r as u32) >> 2);
        let g8 = ((g as u32) << 3) | ((g as u32) >> 2);
        let b8 = ((b as u32) << 3) | ((b as u32) >> 2);
        0xFF000000 | (r8 << 16) | (g8 << 8) | b8
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

    // Mode 7変換
    #[allow(dead_code)]
    pub(crate) fn mode7_transform(&self, screen_x: u16, screen_y: u16) -> (i32, i32) {
        // 画面座標を中心基準に変換
        let sx = screen_x as i32 - 128;
        let sy = screen_y as i32 - 128;

        // 回転中心からの相対座標
        let rel_x = sx - (self.mode7_center_x as i32);
        let rel_y = sy - (self.mode7_center_y as i32);

        // 変換行列適用 (固定小数点演算)
        let a = self.mode7_matrix_a as i32;
        let b = self.mode7_matrix_b as i32;
        let c = self.mode7_matrix_c as i32;
        let d = self.mode7_matrix_d as i32;

        let transformed_x = ((a * rel_x + b * rel_y) >> 8) + (self.mode7_center_x as i32);
        let transformed_y = ((c * rel_x + d * rel_y) >> 8) + (self.mode7_center_y as i32);

        (transformed_x, transformed_y)
    }

    // Mode 7 affine transform producing integer world pixels.
    #[inline]
    pub(crate) fn mode7_world_xy_int(&self, sx: i32, sy: i32) -> (i32, i32) {
        // Promote to i64 to avoid overflow in affine products.
        //
        // Mode 7 affine (SNESdev):
        //   [X]   [A B] [SX + HOFS - CX] + [CX]
        //   [Y] = [C D] [SY + VOFS - CY]   [CY]
        //
        // A..D are signed 8.8 fixed; SX/SY, HOFS/VOFS, CX/CY are signed integers.
        let a = self.mode7_matrix_a as i64;
        let b = self.mode7_matrix_b as i64;
        let c = self.mode7_matrix_c as i64;
        let d = self.mode7_matrix_d as i64;
        let cx = self.mode7_center_x as i64;
        let cy = self.mode7_center_y as i64;
        let hofs = self.mode7_hofs as i64;
        let vofs = self.mode7_vofs as i64;

        let dx = (sx as i64) + hofs - cx;
        let dy = (sy as i64) + vofs - cy;

        let x = ((a * dx + b * dy) >> 8) + cx;
        let y = ((c * dx + d * dy) >> 8) + cy;
        (x as i32, y as i32)
    }

    // メインスクリーン描画（レイヤID付き）共通処理
    pub(crate) fn render_main_screen_pixel_with_layer_internal(
        &mut self,
        x: u16,
        y: u16,
        enables: u8,
        has_bg: bool,
        has_obj: bool,
    ) -> (u32, u8, bool) {
        // BGとスプライトの情報を取得
        let (bg_color, bg_priority, bg_id) = if has_bg {
            self.get_main_bg_pixel(x, y, enables)
        } else {
            (0, 0, 0)
        };
        let (sprite_color, sprite_priority, sprite_math_allowed) = if has_obj {
            self.get_sprite_pixel_common(x, y, true, true)
        } else {
            (0, 0, true)
        };
        if let Some(cfg) = trace_sample_dot_config() {
            if self.frame == cfg.frame && x == cfg.x && y == cfg.y {
                let z_bg = self.z_rank_for_bg(bg_id, bg_priority);
                let z_obj = self.z_rank_for_obj(sprite_priority);
                println!(
                    "[TRACE_SAMPLE_DOT][MAIN] frame={} x={} y={} bg=(0x{:08X},id={},pr={},z={}) obj=(0x{:08X},pr={},z={},math={})",
                    self.frame,
                    x,
                    y,
                    bg_color,
                    bg_id,
                    bg_priority,
                    z_bg,
                    sprite_color,
                    sprite_priority,
                    z_obj,
                    sprite_math_allowed as u8
                );
            }
        }

        // プライオリティベースの合成（レイヤIDも取得）
        let (final_color, layer_id) = self.composite_pixel_with_layer(
            bg_color,
            bg_priority,
            bg_id,
            sprite_color,
            sprite_priority,
        );
        let obj_math_allowed = if layer_id == 4 {
            sprite_math_allowed
        } else {
            true
        };

        // NOTE: This function is called per pixel. Debug flags are OnceLock-cached
        // (single pointer read after init), and the outer check gates all inner work.
        {
            // Cache both flags in a single outer branch; avoids repeated OnceLock reads
            // inside the hot inner block.
            let rv = crate::debug_flags::render_verbose();
            let gd = crate::debug_flags::debug_graphics_detected();
            if rv || gd {
                static mut RENDER_DEBUG_COUNT: u32 = 0;
                static mut NON_BLACK_PIXELS: u32 = 0;
                static mut GRAPHICS_DETECTED_PRINTS: u32 = 0;
                let quiet = crate::debug_flags::quiet();

                unsafe {
                    RENDER_DEBUG_COUNT = RENDER_DEBUG_COUNT.saturating_add(1);
                    if final_color != 0xFF000000 {
                        NON_BLACK_PIXELS = NON_BLACK_PIXELS.saturating_add(1);
                    }

                    if rv && x == 10 && y == 10 {
                        static mut PIXEL_10_10_SHOWN: bool = false;
                        if !PIXEL_10_10_SHOWN && !quiet {
                            println!(
                                "🎯 PIXEL (10,10): bg_color=0x{:06X}, final_color=0x{:06X}, layer={}",
                                bg_color, final_color, layer_id
                            );
                            PIXEL_10_10_SHOWN = true;
                        }
                    }

                    if rv && !quiet && RENDER_DEBUG_COUNT.is_multiple_of(100000) {
                        println!(
                            "🖼️  RENDER STATS: {} pixels rendered, {} non-black ({:.1}%)",
                            RENDER_DEBUG_COUNT,
                            NON_BLACK_PIXELS,
                            (NON_BLACK_PIXELS as f32 / RENDER_DEBUG_COUNT as f32) * 100.0
                        );
                    }

                    if gd && !quiet && final_color != 0xFF000000 && GRAPHICS_DETECTED_PRINTS == 0 {
                        GRAPHICS_DETECTED_PRINTS = 1;
                        println!(
                            "🎨 GRAPHICS DETECTED: first non-black pixel 0x{:08X} at ({}, {}) layer={}",
                            final_color, x, y, layer_id
                        );
                    }
                }
            }
        }

        (final_color, layer_id, obj_math_allowed)
    }

    // メインスクリーン描画（レイヤID付き）
    pub(crate) fn render_main_screen_pixel_with_layer(
        &mut self,
        x: u16,
        y: u16,
    ) -> (u32, u8, bool) {
        let enables = self.effective_main_screen_designation();
        let has_bg = (enables & 0x0F) != 0;
        let has_obj = (enables & 0x10) != 0;
        self.render_main_screen_pixel_with_layer_internal(x, y, enables, has_bg, has_obj)
    }

    // Render path for active scanlines using cached per-line enables.
    pub(crate) fn render_main_screen_pixel_with_layer_cached(
        &mut self,
        x: u16,
        y: u16,
    ) -> (u32, u8, bool) {
        self.render_main_screen_pixel_with_layer_internal(
            x,
            y,
            self.line_main_enables,
            self.line_main_has_bg,
            self.line_main_has_obj,
        )
    }

    // メインスクリーン用BGの最前面色とその優先度を取得
    pub(crate) fn get_main_bg_pixel(&mut self, x: u16, y: u16, enables: u8) -> (u32, u8, u8) {
        // Hot path: avoid per-pixel heap allocations.

        let mut best: Option<(u32, u8, u8, i16)> = None;

        // Keep ordering consistent with the previous max_by_key:
        // (z_rank_for_bg(layer, pr), layer_id)
        macro_rules! consider {
            ($color:expr, $pr:expr, $id:expr) => {{
                let color: u32 = $color;
                if color == 0 {
                    // Transparent
                } else {
                    let pr: u8 = $pr;
                    let id: u8 = $id;
                    let z = self.z_rank_for_bg(id, pr);
                    let replace = match best {
                        None => true,
                        Some((_, _, best_id, best_z)) => {
                            z > best_z || (z == best_z && id > best_id)
                        }
                    };
                    if replace {
                        best = Some((color, pr, id, z));
                    }
                }
            }};
        }

        match self.bg_mode {
            0 => {
                // Mode 0: BG1-4 all 2bpp
                if (enables & 0x01) != 0 && !self.should_mask_bg(x, 0, true) {
                    let (c, p) = self.render_bg_mode0_with_priority(x, y, 0);
                    consider!(c, p, 0);
                }
                if (enables & 0x02) != 0 && !self.should_mask_bg(x, 1, true) {
                    let (c, p) = self.render_bg_mode0_with_priority(x, y, 1);
                    consider!(c, p, 1);
                }
                if (enables & 0x04) != 0 && !self.should_mask_bg(x, 2, true) {
                    let (c, p) = self.render_bg_mode0_with_priority(x, y, 2);
                    consider!(c, p, 2);
                }
                if (enables & 0x08) != 0 && !self.should_mask_bg(x, 3, true) {
                    let (c, p) = self.render_bg_mode0_with_priority(x, y, 3);
                    consider!(c, p, 3);
                }
            }
            1 => {
                // Mode 1: BG1/BG2 are 4bpp, BG3 is 2bpp
                if (enables & 0x01) != 0 && !self.should_mask_bg(x, 0, true) {
                    let (c, p) = self.render_bg_4bpp_with_priority(x, y, 0);
                    consider!(c, p, 0);
                }
                if (enables & 0x02) != 0 && !self.should_mask_bg(x, 1, true) {
                    let (c, p) = self.render_bg_4bpp_with_priority(x, y, 1);
                    consider!(c, p, 1);
                }
                if (enables & 0x04) != 0 && !self.should_mask_bg(x, 2, true) {
                    let (c, p) = self.render_bg_mode0_with_priority(x, y, 2);
                    consider!(c, p, 2);
                }
            }
            2 => {
                // Mode 2: BG1/BG2 are 4bpp with offset-per-tile
                if (enables & 0x01) != 0 {
                    let (c, p) = self.render_bg_mode2_window_aware(x, y, 0, true);
                    consider!(c, p, 0);
                }
                if (enables & 0x02) != 0 {
                    let (c, p) = self.render_bg_mode2_window_aware(x, y, 1, true);
                    consider!(c, p, 1);
                }
            }
            3 => {
                // Mode 3: BG1 is 8bpp, BG2 is 4bpp
                if (enables & 0x01) != 0 && !self.should_mask_bg(x, 0, true) {
                    let (c, p) = self.render_bg_8bpp_with_priority(x, y, 0);
                    consider!(c, p, 0);
                }
                if (enables & 0x02) != 0 && !self.should_mask_bg(x, 1, true) {
                    let (c, p) = self.render_bg_4bpp_with_priority(x, y, 1);
                    consider!(c, p, 1);
                }
            }
            4 => {
                // Mode 4: BG1 is 8bpp, BG2 is 2bpp
                if (enables & 0x01) != 0 && !self.should_mask_bg(x, 0, true) {
                    let (c, p) = self.render_bg_8bpp_with_priority(x, y, 0);
                    consider!(c, p, 0);
                }
                if (enables & 0x02) != 0 && !self.should_mask_bg(x, 1, true) {
                    let (c, p) = self.render_bg_mode0_with_priority(x, y, 1);
                    consider!(c, p, 1);
                }
            }
            5 => {
                // Mode 5: BG1 is 4bpp, BG2 is 2bpp (hires); some games also use BG3
                if (enables & 0x01) != 0 && !self.should_mask_bg(x, 0, true) {
                    let (c, p) = self.render_bg_mode5_with_priority(x, y, 0);
                    consider!(c, p, 0);
                }
                if (enables & 0x02) != 0 && !self.should_mask_bg(x, 1, true) {
                    let (c, p) = self.render_bg_mode5_with_priority(x, y, 1);
                    consider!(c, p, 1);
                }
                if (enables & 0x04) != 0 && !self.should_mask_bg(x, 2, true) {
                    let (c, p) = self.render_bg_mode5_with_priority(x, y, 2);
                    consider!(c, p, 2);
                }
            }
            6 => {
                // Mode 6: BG1 is 4bpp (hires)
                if (enables & 0x01) != 0 && !self.should_mask_bg(x, 0, true) {
                    let (c, p) = self.render_bg_mode6_with_priority(x, y, 0);
                    consider!(c, p, 0);
                }
            }
            7 => {
                if self.extbg {
                    // EXTBG: BG1 and BG2 are independent layers from the same Mode 7 tilemap.
                    // BG1: full 8-bit color, single priority.
                    // BG2: 7-bit color, bit7 as priority (0 or 1).
                    // Each layer is checked independently for enable and window masking.
                    for layer_id in 0..=1u8 {
                        let en_bit = 1u8 << layer_id;
                        if (enables & en_bit) != 0 && !self.should_mask_bg(x, layer_id, true) {
                            let (c, p, _) = self.render_mode7_single_layer(x, y, layer_id);
                            if c != 0 {
                                consider!(c, p, layer_id);
                            }
                        }
                    }
                } else if (enables & 0x01) != 0 {
                    let (c, p, _) = self.render_mode7_with_layer(x, y);
                    if c != 0 && !self.should_mask_bg(x, 0, true) {
                        consider!(c, p, 0);
                    }
                }
            }
            _ => {}
        }

        if let Some((best_color, best_pr, best_id, _)) = best {
            (best_color, best_pr, best_id)
        } else {
            (0, 0, 0)
        }
    }

    // サブスクリーン描画
    #[allow(dead_code)]
    pub(crate) fn render_sub_screen_pixel(&mut self, x: u16, y: u16) -> u32 {
        let enables = self.sub_screen_designation;
        let has_bg = (enables & 0x0F) != 0;
        let has_obj = (enables & 0x10) != 0;
        let (color, _lid, _transparent, _obj_math_allowed) =
            self.render_sub_screen_pixel_with_layer_internal(x, y, has_bg, has_obj);
        color
    }

    // サブスクリーン描画（レイヤID付き）
    // 戻り値:
    // - color: 合成後のRGBA
    // - layer_id: 最前面レイヤ（0..4、5=backdrop）
    // - transparent: サブスクリーンが完全透明で、fixed color ($2132) をbackdropとして返した場合
    pub(crate) fn render_sub_screen_pixel_with_layer_internal(
        &mut self,
        x: u16,
        y: u16,
        has_bg: bool,
        has_obj: bool,
    ) -> (u32, u8, bool, bool) {
        if !has_bg && !has_obj {
            // For color math, an empty subscreen behaves like a transparent source
            // that falls back to fixed color rather than forcing backdrop-black math.
            return (self.cgram_to_rgb(0), 5, true, true);
        }
        let (bg_color, bg_priority, bg_id) = if has_bg {
            self.get_sub_bg_pixel(x, y)
        } else {
            (0, 0, 0)
        };
        let (sprite_color, sprite_priority, sprite_math_allowed) =
            self.get_sprite_pixel_common(x, y, has_obj, false);
        let (final_color, layer_id) = self.composite_pixel_with_layer(
            bg_color,
            bg_priority,
            bg_id,
            sprite_color,
            sprite_priority,
        );
        let obj_math_allowed = if layer_id == 4 {
            sprite_math_allowed
        } else {
            true
        };
        if final_color != 0 {
            (final_color, layer_id, false, obj_math_allowed)
        } else {
            (self.cgram_to_rgb(0), 5, true, true)
        }
    }

    pub(crate) fn render_sub_screen_pixel_with_layer(
        &mut self,
        x: u16,
        y: u16,
    ) -> (u32, u8, bool, bool) {
        let enables = self.sub_screen_designation;
        let has_bg = (enables & 0x0F) != 0;
        let has_obj = (enables & 0x10) != 0;
        self.render_sub_screen_pixel_with_layer_internal(x, y, has_bg, has_obj)
    }

    pub(crate) fn render_sub_screen_pixel_with_layer_cached(
        &mut self,
        x: u16,
        y: u16,
    ) -> (u32, u8, bool, bool) {
        self.render_sub_screen_pixel_with_layer_internal(
            x,
            y,
            self.line_sub_has_bg,
            self.line_sub_has_obj,
        )
    }

    // サブスクリーン用BG描画（メインと同等のモード対応）
    pub(crate) fn get_sub_bg_pixel(&mut self, x: u16, y: u16) -> (u32, u8, u8) {
        // Hot path: avoid per-pixel heap allocations.
        let enables = self.sub_screen_designation;

        let mut best: Option<(u32, u8, u8, i16)> = None;

        // Keep ordering consistent with the previous max_by_key:
        // (z_rank_for_bg(layer, pr), layer_id)
        macro_rules! consider {
            ($color:expr, $pr:expr, $id:expr) => {{
                let color: u32 = $color;
                if color == 0 {
                    // Transparent
                } else {
                    let pr: u8 = $pr;
                    let id: u8 = $id;
                    let z = self.z_rank_for_bg(id, pr);
                    let replace = match best {
                        None => true,
                        Some((_, _, best_id, best_z)) => {
                            z > best_z || (z == best_z && id > best_id)
                        }
                    };
                    if replace {
                        best = Some((color, pr, id, z));
                    }
                }
            }};
        }

        match self.bg_mode {
            0 => {
                if (enables & 0x01) != 0 && !self.should_mask_bg(x, 0, false) {
                    let (c, p) = self.render_bg_mode0(x, y, 0);
                    consider!(c, p, 0);
                }
                if (enables & 0x02) != 0 && !self.should_mask_bg(x, 1, false) {
                    let (c, p) = self.render_bg_mode0(x, y, 1);
                    consider!(c, p, 1);
                }
                if (enables & 0x04) != 0 && !self.should_mask_bg(x, 2, false) {
                    let (c, p) = self.render_bg_mode0(x, y, 2);
                    consider!(c, p, 2);
                }
                if (enables & 0x08) != 0 && !self.should_mask_bg(x, 3, false) {
                    let (c, p) = self.render_bg_mode0(x, y, 3);
                    consider!(c, p, 3);
                }
            }
            1 => {
                if (enables & 0x01) != 0 && !self.should_mask_bg(x, 0, false) {
                    let (c, p) = self.render_bg_4bpp(x, y, 0);
                    consider!(c, p, 0);
                }
                if (enables & 0x02) != 0 && !self.should_mask_bg(x, 1, false) {
                    let (c, p) = self.render_bg_4bpp(x, y, 1);
                    consider!(c, p, 1);
                }
                if (enables & 0x04) != 0 && !self.should_mask_bg(x, 2, false) {
                    let (c, p) = self.render_bg_mode0(x, y, 2);
                    consider!(c, p, 2);
                }
            }
            2 => {
                if (enables & 0x01) != 0 {
                    let (c, p) = self.render_bg_mode2_window_aware(x, y, 0, false);
                    consider!(c, p, 0);
                }
                if (enables & 0x02) != 0 {
                    let (c, p) = self.render_bg_mode2_window_aware(x, y, 1, false);
                    consider!(c, p, 1);
                }
            }
            3 => {
                if (enables & 0x01) != 0 && !self.should_mask_bg(x, 0, false) {
                    let (c, p) = self.render_bg_8bpp(x, y, 0);
                    consider!(c, p, 0);
                }
                if (enables & 0x02) != 0 && !self.should_mask_bg(x, 1, false) {
                    let (c, p) = self.render_bg_4bpp(x, y, 1);
                    consider!(c, p, 1);
                }
            }
            4 => {
                if (enables & 0x01) != 0 && !self.should_mask_bg(x, 0, false) {
                    let (c, p) = self.render_bg_8bpp(x, y, 0);
                    consider!(c, p, 0);
                }
                if (enables & 0x02) != 0 && !self.should_mask_bg(x, 1, false) {
                    let (c, p) = self.render_bg_mode0(x, y, 1);
                    consider!(c, p, 1);
                }
            }
            5 => {
                if (enables & 0x01) != 0 && !self.should_mask_bg(x, 0, false) {
                    let (c, p) = self.render_bg_mode5(x, y, 0, false);
                    consider!(c, p, 0);
                }
                if (enables & 0x02) != 0 && !self.should_mask_bg(x, 1, false) {
                    let (c, p) = self.render_bg_mode5(x, y, 1, false);
                    consider!(c, p, 1);
                }
            }
            6 => {
                if (enables & 0x01) != 0 && !self.should_mask_bg(x, 0, false) {
                    let (c, p) = self.render_bg_mode6(x, y, 0, false);
                    consider!(c, p, 0);
                }
            }
            7 => {
                if self.extbg {
                    for layer_id in 0..=1u8 {
                        let en_bit = 1u8 << layer_id;
                        if (enables & en_bit) != 0 && !self.should_mask_bg(x, layer_id, false) {
                            let (c, p, _) = self.render_mode7_single_layer(x, y, layer_id);
                            if c != 0 {
                                consider!(c, p, layer_id);
                            }
                        }
                    }
                } else if (enables & 0x01) != 0 {
                    let (c, p, _) = self.render_mode7_with_layer(x, y);
                    if c != 0 && !self.should_mask_bg(x, 0, false) {
                        consider!(c, p, 0);
                    }
                }
            }
            _ => {}
        }

        if let Some((best_color, best_pr, best_id, _)) = best {
            (best_color, best_pr, best_id)
        } else {
            (0, 0, 0)
        }
    }

    // get_sub_sprite_pixel moved to sprites.rs

    // メイン・サブスクリーン間のカラー演算（対象レイヤ限定の簡易版）
    pub(crate) fn apply_color_math_screens(
        &mut self,
        main_color_in: u32,
        sub_color_in: u32,
        main_layer_id: u8,
        main_obj_math_allowed: bool,
        x: u16,
        y: u16,
        sub_transparent: bool,
    ) -> u32 {
        let force_display = self.force_display_active();
        // Forced blank produces black regardless of color math (unless FORCE_DISPLAY)
        if (self.screen_display & 0x80) != 0 && !force_display {
            return 0;
        }
        // Fast path: no window regions and no color-math enabled
        if (self.cgwsel & 0xF0) == 0 && (self.cgadsub & 0x3F) == 0 {
            return main_color_in;
        }

        // CGWSEL region types (MM / SS): 0=nowhere, 1=outside, 2=inside, 3=everywhere.
        let mm = (self.cgwsel >> 6) & 0x03; // main screen black region
        let ss = (self.cgwsel >> 4) & 0x03; // sub screen transparent region
        let layer_math_bit = Self::color_math_layer_bit(main_layer_id);
        let math_enabled = layer_math_bit != 0 && (self.cgadsub & layer_math_bit) != 0;
        let obj_math_blocked = main_layer_id == 4 && !main_obj_math_allowed;
        if mm == 0 && ss == 0 && (obj_math_blocked || !math_enabled) {
            return main_color_in;
        }

        // Color window W(x): 1=inside, 0=outside.
        // If the color window is disabled (WOBJSEL upper nibble = 0), do not apply MM/SS.
        let win = if mm == 0 && ss == 0 {
            false
        } else if self.window_color_mask == 0 {
            false
        } else if self.line_window_prepared {
            self.color_window_lut.get(x as usize).copied().unwrap_or(0) != 0
        } else {
            self.evaluate_window_mask(x, self.window_color_mask, self.color_window_logic)
        };

        let region_hit = |mode: u8, inside: bool| -> bool {
            match mode {
                0 => false,
                1 => !inside, // outside
                2 => inside,  // inside
                _ => true,    // everywhere
            }
        };

        // Apply main-screen black region *before* color math, but do not suppress color math.
        // (Real hardware still performs color math even when the main screen is replaced with black.)
        // NOTE: When no color windows are enabled (WOBJSEL upper nibble = 0), all pixels are
        // considered "outside" the window (win=false). The MM/SS region checks must still be
        // evaluated because modes 1 (outside window) and 3 (everywhere) apply even when no
        // windows are configured.
        let mut main_color = main_color_in;
        if region_hit(mm, win) && !force_display {
            if crate::debug_flags::render_metrics() {
                if mm == 1 {
                    self.dbg_clip_outside = self.dbg_clip_outside.saturating_add(1);
                    if main_layer_id == 4 {
                        self.dbg_clip_obj_outside = self.dbg_clip_obj_outside.saturating_add(1);
                    }
                } else if mm == 2 {
                    self.dbg_clip_inside = self.dbg_clip_inside.saturating_add(1);
                    if main_layer_id == 4 {
                        self.dbg_clip_obj_inside = self.dbg_clip_obj_inside.saturating_add(1);
                    }
                }
            }
            main_color = 0xFF000000;
        }

        // Mask color math via the sub-screen transparent region.
        // When active, output the (possibly black-clipped) main screen color unchanged.
        if region_hit(ss, win) && !force_display {
            if let Some(cfg) = trace_sample_dot_config() {
                if self.frame == cfg.frame && x == cfg.x && y == cfg.y {
                    println!(
                        "[TRACE_SAMPLE_DOT][MATH] frame={} x={} y={} mode=skip_by_ss win={} mm={} ss={} main_lid={} main=0x{:08X} sub=0x{:08X} sub_transparent={}",
                        self.frame,
                        x,
                        y,
                        win as u8,
                        mm,
                        ss,
                        main_layer_id,
                        main_color,
                        sub_color_in,
                        sub_transparent as u8
                    );
                }
            }
            return main_color;
        }

        // OBJ color math is disabled for palettes 0-3 on real hardware.
        if obj_math_blocked {
            return main_color;
        }

        // このメインレイヤにカラー演算が許可されているか
        if !math_enabled {
            if crate::debug_flags::render_metrics() {
                self.dbg_math_blocked = self.dbg_math_blocked.saturating_add(1);
                if main_layer_id == 4 {
                    self.dbg_math_blocked_obj = self.dbg_math_blocked_obj.saturating_add(1);
                }
                if main_layer_id == 5 {
                    self.dbg_math_blocked_backdrop =
                        self.dbg_math_blocked_backdrop.saturating_add(1);
                }
            }
            return main_color;
        }

        // Subsource is only used for color math; transparent main becomes backdrop earlier
        let use_sub_src = (self.cgwsel & 0x02) != 0; // 1=subscreen, 0=fixed
                                                     // NOTE: If the subscreen pixel is transparent, real hardware uses fixed color ($2132)
                                                     // as the addend, *and* disables half color math for that pixel.
        let sub_is_fixed_fallback = use_sub_src && sub_transparent;
        let sub_src = if use_sub_src && !sub_is_fixed_fallback {
            sub_color_in
        } else {
            self.fixed_color_to_rgb()
        };

        let src = sub_src;
        let src_is_fixed = !use_sub_src || sub_is_fixed_fallback;
        // Use CGADSUB for add/sub + halve
        let is_addition = (self.cgadsub & 0x80) == 0;
        let halve_flag = (self.cgadsub & 0x40) != 0;
        // In pseudo-hires, when actually blending main and sub (not fixed color),
        // halve the result to approximate 512px brightness. Skip when src is fixed color.
        let hires_halve =
            self.pseudo_hires && use_sub_src && !src_is_fixed && main_color != 0 && src != 0;
        let effective_halve = (halve_flag || hires_halve) && !sub_is_fixed_fallback;
        if let Some(cfg) = trace_sample_dot_config() {
            if self.frame == cfg.frame && x == cfg.x && y == cfg.y {
                println!(
                    "[TRACE_SAMPLE_DOT][MATH] frame={} x={} y={} mode=blend win={} mm={} ss={} main_lid={} main=0x{:08X} sub_in=0x{:08X} src=0x{:08X} use_sub={} sub_fixed_fallback={} sub_transparent={} math_enabled={} add={} halve={} hires_halve={} effective_halve={}",
                    self.frame,
                    x,
                    y,
                    win as u8,
                    mm,
                    ss,
                    main_layer_id,
                    main_color,
                    sub_color_in,
                    src,
                    use_sub_src as u8,
                    sub_is_fixed_fallback as u8,
                    sub_transparent as u8,
                    math_enabled as u8,
                    is_addition as u8,
                    halve_flag as u8,
                    hires_halve as u8,
                    effective_halve as u8
                );
            }
        }
        let out = self.blend_colors(main_color, src, is_addition, effective_halve);
        if crate::debug_flags::render_metrics() {
            if is_addition {
                if effective_halve {
                    self.dbg_math_add_half += 1;
                    if main_layer_id == 4 {
                        self.dbg_math_obj_add_half += 1;
                    }
                } else {
                    self.dbg_math_add += 1;
                    if main_layer_id == 4 {
                        self.dbg_math_obj_add += 1;
                    }
                }
            } else if effective_halve {
                self.dbg_math_sub_half += 1;
                if main_layer_id == 4 {
                    self.dbg_math_obj_sub_half += 1;
                }
            } else {
                self.dbg_math_sub += 1;
                if main_layer_id == 4 {
                    self.dbg_math_obj_sub += 1;
                }
            }
        }
        out
    }
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
