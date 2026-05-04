use super::Ppu;

impl Ppu {
    // デバッグ用：PPU状態を表示
    pub fn debug_ppu_state(&self) {
        println!("\n=== PPU Debug State ===");
        println!(
            "Scanline: {}, Cycle: {}, Frame: {}",
            self.scanline, self.cycle, self.frame
        );
        println!(
            "Mode: {} (BG3prio={}), SETINI=0x{:02X} (pseudo_hires={}, interlace={}, obj_interlace={}, overscan={}, extbg={})",
            self.bg_mode,
            self.mode1_bg3_priority,
            self.setini,
            self.pseudo_hires,
            self.interlace,
            self.obj_interlace,
            self.overscan,
            self.extbg
        );
        println!(
            "Main Screen: 0x{:02X}, Sub Screen: 0x{:02X}",
            self.main_screen_designation, self.sub_screen_designation
        );
        println!(
            "Color Math: CGWSEL=0x{:02X} CGADSUB=0x{:02X} fixed=0x{:04X}",
            self.cgwsel, self.cgadsub, self.fixed_color
        );
        println!(
            "Windows: W1=({}, {}) W2=({}, {}) W12SEL=0x{:02X} W34SEL=0x{:02X} WOBJSEL(obj=0x{:01X} col=0x{:01X}) WBGLOG=[{}, {}, {}, {}] WOBJLOG(obj={} col={}) TMW=0x{:02X} TSW=0x{:02X}",
            self.window1_left,
            self.window1_right,
            self.window2_left,
            self.window2_right,
            ((self.window_bg_mask[1] & 0x0F) << 4) | (self.window_bg_mask[0] & 0x0F),
            ((self.window_bg_mask[3] & 0x0F) << 4) | (self.window_bg_mask[2] & 0x0F),
            (self.window_obj_mask & 0x0F),
            (self.window_color_mask & 0x0F),
            self.bg_window_logic[0],
            self.bg_window_logic[1],
            self.bg_window_logic[2],
            self.bg_window_logic[3],
            self.obj_window_logic,
            self.color_window_logic,
            self.tmw_mask,
            self.tsw_mask
        );
        println!(
            "OAM: addr=0x{:03X} internal=0x{:03X} eval_base={} rotation={}",
            self.oam_addr,
            self.oam_internal_addr,
            self.oam_eval_base,
            self.oam_priority_rotation_enabled
        );
        println!("Screen Display: 0x{:02X}", self.screen_display);
        println!("NMI: enabled={}, flag={}", self.nmi_enabled, self.nmi_flag);

        // BGレイヤー設定
        println!(
            "BG1: tilemap=0x{:04X}, tile=0x{:04X}, scroll=({},{})",
            self.bg1_tilemap_base, self.bg1_tile_base, self.bg1_hscroll, self.bg1_vscroll
        );
        println!(
            "BG2: tilemap=0x{:04X}, tile=0x{:04X}, scroll=({},{})",
            self.bg2_tilemap_base, self.bg2_tile_base, self.bg2_hscroll, self.bg2_vscroll
        );
        println!(
            "BG3: tilemap=0x{:04X}, tile=0x{:04X}, scroll=({},{})",
            self.bg3_tilemap_base, self.bg3_tile_base, self.bg3_hscroll, self.bg3_vscroll
        );
        println!(
            "BG4: tilemap=0x{:04X}, tile=0x{:04X}, scroll=({},{})",
            self.bg4_tilemap_base, self.bg4_tile_base, self.bg4_hscroll, self.bg4_vscroll
        );
        println!(
            "BG tile16: [{},{},{},{}] screen_size: [{},{},{},{}]",
            self.bg_tile_16[0],
            self.bg_tile_16[1],
            self.bg_tile_16[2],
            self.bg_tile_16[3],
            self.bg_screen_size[0],
            self.bg_screen_size[1],
            self.bg_screen_size[2],
            self.bg_screen_size[3]
        );

        // スプライト設定
        println!(
            "Sprite: size={}, name_base=0x{:04X}, name_select=0x{:04X}",
            self.sprite_size, self.sprite_name_base, self.sprite_name_select
        );

        // VRAM/CGRAM状態
        let vram_used = self.vram.iter().filter(|&&b| b != 0).count();
        let cgram_used = self.cgram.iter().filter(|&&b| b != 0).count();
        println!(
            "VRAM: {}/{} bytes used, CGRAM: {}/{} bytes used",
            vram_used,
            self.vram.len(),
            cgram_used,
            self.cgram.len()
        );

        // 最初の8個のCGRAMエントリ表示（パレット0）
        print!("Palette 0: ");
        for i in 0..8 {
            let color = self.cgram_to_rgb(i);
            print!("${:06X} ", color & 0xFFFFFF);
        }
        println!();

        println!("=======================");
    }

    // テストパターンを強制表示（デバッグ用）
    pub fn force_test_pattern(&mut self) {
        println!("Forcing test pattern display...");

        // テストパターン表示のため基本的なPPU設定を上書き
        self.brightness = 15;
        self.main_screen_designation = 0x1F; // 全BGレイヤーとスプライトを有効
        self.screen_display = 0; // forced blank off (表示有効)

        // Dragon Quest III fix: Fill VRAM with test data
        for i in 0..0x8000 {
            self.vram[i] = if i < 0x4000 { 0x11 } else { 0x22 };
        }
        self.bg_cache_dirty = true;

        // Set up tilemap entries at high addresses (0xE000-0xFFFF range)
        let tilemap_start = 0x6000; // Start from 0xE000 & 0x7FFF = 0x6000
        for i in (tilemap_start..tilemap_start + 0x800).step_by(2) {
            if i + 1 < self.vram.len() {
                self.vram[i] = 0x01; // Tile ID low
                self.vram[i + 1] = 0x00; // Tile ID high + attributes
            }
        }

        // Set up tile data at 0x6000+ range
        let tile_start = 0x4000; // Start from 0xE000 & 0x7FFF = 0x6000
        for i in tile_start..tile_start + 0x100 {
            if i < self.vram.len() {
                self.vram[i] = 0xFF; // White tile data
            }
        }

        // Fill CGRAM with test colors
        // Palette 0: Background colors
        self.cgram[0] = 0x00;
        self.cgram[1] = 0x00; // Color 0: Black (transparent)
        self.cgram[2] = 0xFF;
        self.cgram[3] = 0x7F; // Color 1: White
        self.cgram[4] = 0x1F;
        self.cgram[5] = 0x00; // Color 2: Red
        self.cgram[6] = 0xE0;
        self.cgram[7] = 0x03; // Color 3: Green

        // Palette 1-7: Fill with distinct colors
        for palette in 1..8 {
            let base = palette * 16 * 2;
            for color in 0..16 {
                let addr = base + color * 2;
                if addr + 1 < self.cgram.len() {
                    // Create distinct colors for each palette
                    let r = ((palette * 4) & 0x1F) as u16;
                    let g = ((color * 2) & 0x1F) as u16;
                    let b = ((palette + color) & 0x1F) as u16;
                    let color_val = (b << 10) | (g << 5) | r;
                    self.cgram[addr] = (color_val & 0xFF) as u8;
                    self.cgram[addr + 1] = ((color_val >> 8) & 0x7F) as u8;
                }
            }
        }
        self.rebuild_cgram_rgb_cache();

        println!(
            "PPU: Test pattern applied (brightness={}, layers=0x{:02X}) with VRAM test data",
            self.brightness, self.main_screen_designation
        );
    }
}
