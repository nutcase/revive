use super::{Emulator, PPU_CLOCK_DIVIDER};

impl Emulator {
    /// 主要PPUレジスタの変遷を出力（回帰検出用）
    pub(super) fn maybe_dump_register_summary(&self, frame: u32) {
        // 環境変数で制御
        let enabled = std::env::var("DUMP_REGISTER_SUMMARY")
            .map(|v| v == "1" || v.to_lowercase() == "true")
            .unwrap_or(false);

        if !enabled {
            return;
        }

        // 特定フレームのみ出力（環境変数で指定可能）
        let target_frames: Vec<u32> = std::env::var("DUMP_REGISTER_FRAMES")
            .ok()
            .and_then(|s| {
                s.split(',')
                    .filter_map(|n| n.trim().parse::<u32>().ok())
                    .collect::<Vec<_>>()
                    .into()
            })
            .unwrap_or_else(|| vec![60, 120, 180, 240, 300, 360, 500, 1000]);

        if !target_frames.contains(&frame) {
            return;
        }

        let ppu = self.bus.get_ppu();
        println!("\n━━━━ REGISTER SUMMARY @ Frame {} ━━━━", frame);
        println!(
            "  INIDISP:    0x{:02X} (blank={} brightness={})",
            if ppu.is_forced_blank() {
                0x80 | ppu.current_brightness()
            } else {
                ppu.current_brightness()
            },
            if ppu.is_forced_blank() { "ON " } else { "OFF" },
            ppu.current_brightness()
        );
        println!(
            "  TM (main):  0x{:02X} (BG1={} BG2={} BG3={} BG4={} OBJ={})",
            ppu.get_main_screen_designation(),
            (ppu.get_main_screen_designation() & 0x01) != 0,
            (ppu.get_main_screen_designation() & 0x02) != 0,
            (ppu.get_main_screen_designation() & 0x04) != 0,
            (ppu.get_main_screen_designation() & 0x08) != 0,
            (ppu.get_main_screen_designation() & 0x10) != 0
        );
        println!("  BG mode:    {}", ppu.get_bg_mode());
        let setini = ppu.get_setini();
        println!(
            "  SETINI:     0x{:02X} (interlace={} obj_interlace={} pseudo_hires={} overscan={} extbg={})",
            setini,
            (setini & 0x01) != 0,
            (setini & 0x02) != 0,
            (setini & 0x08) != 0,
            (setini & 0x04) != 0,
            (setini & 0x40) != 0
        );

        let main_tm = ppu.get_main_screen_designation();
        if (main_tm & 0x01) != 0 {
            let (tile_base, map_base, tile_16, screen_size) = ppu.get_bg_config(1);
            let size_desc = match screen_size {
                0 => "32x32",
                1 => "64x32",
                2 => "32x64",
                3 => "64x64",
                _ => "???",
            };
            println!(
                "  BG1 config: tile_base=0x{:04X} map_base=0x{:04X} tile_size={} screen={}",
                tile_base,
                map_base,
                if tile_16 { "16x16" } else { "8x8" },
                size_desc
            );
        }

        if (main_tm & 0x08) != 0 {
            let (tile_base, map_base, tile_16, screen_size) = ppu.get_bg_config(4);
            let size_desc = match screen_size {
                0 => "32x32",
                1 => "64x32",
                2 => "32x64",
                3 => "64x64",
                _ => "???",
            };
            println!(
                "  BG4 config: tile_base=0x{:04X} map_base=0x{:04X} tile_size={} screen={}",
                tile_base,
                map_base,
                if tile_16 { "16x16" } else { "8x8" },
                size_desc
            );
        }

        // BG3 configuration
        if (main_tm & 0x04) != 0 {
            // BG3 enabled
            let (tile_base, map_base, tile_16, screen_size) = ppu.get_bg_config(3);
            let size_desc = match screen_size {
                0 => "32x32",
                1 => "64x32",
                2 => "32x64",
                3 => "64x64",
                _ => "???",
            };
            println!(
                "  BG3 config: tile_base=0x{:04X} map_base=0x{:04X} tile_size={} screen={}",
                tile_base,
                map_base,
                if tile_16 { "16x16" } else { "8x8" },
                size_desc
            );

            // Check actual data in tile and map regions
            let (tile_nonzero, tile_samples) = ppu.analyze_vram_region(tile_base, 512);
            let (map_nonzero, map_samples) = ppu.analyze_vram_region(map_base, 512);
            println!(
                "    └─ Tile data @ 0x{:04X}: {} nonzero bytes, samples: {:02X?}...",
                tile_base,
                tile_nonzero,
                &tile_samples[..tile_samples.len().min(8)]
            );
            println!("    └─ Map  data @ 0x{:04X}: {} nonzero bytes (512 words checked), samples: {:02X?}...",
                map_base, map_nonzero, &map_samples[..map_samples.len().min(8)]
            );
        }

        // VRAM analysis
        let (vram_nonzero, vram_unique, vram_samples) = ppu.analyze_vram_content();
        println!(
            "  VRAM usage: {}/{} bytes ({:.1}%)",
            vram_nonzero,
            65536,
            (vram_nonzero as f64 / 65536.0) * 100.0
        );
        if vram_nonzero > 0 {
            println!(
                "    └─ {} unique values, samples: {:?}...",
                vram_unique,
                &vram_samples[..vram_samples.len().min(5)]
            );

            // Show VRAM distribution by 4KB blocks
            let distribution = ppu.get_vram_distribution();
            println!("    └─ Distribution by 4KB blocks (word addresses):");
            for (word_addr, count) in distribution.iter() {
                println!("       0x{:04X}: {} bytes", word_addr, count);
            }
        }

        println!(
            "  CGRAM usage: {}/{} bytes ({:.1}%)",
            ppu.cgram_usage(),
            512,
            (ppu.cgram_usage() as f64 / 512.0) * 100.0
        );
        println!(
            "  OAM usage:  {}/{} bytes ({:.1}%)",
            ppu.oam_usage(),
            544,
            (ppu.oam_usage() as f64 / 544.0) * 100.0
        );

        // フレームバッファの統計
        let fb = ppu.get_framebuffer();
        let non_black = fb
            .iter()
            .take(256 * 239)
            .filter(|&&px| px != 0xFF000000 && px != 0x00000000)
            .count();
        println!(
            "  Non-black pixels: {} ({:.1}%)",
            non_black,
            (non_black as f64 / (256.0 * 239.0)) * 100.0
        );
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");
    }

    // Headless-only: render a full Mode 7 diagnostic frame without running CPU
    pub(super) fn run_mode7_diag_frame(&mut self) {
        let ppu = self.bus.get_ppu_mut();
        // One-time setup on first frame
        if self.frame_count == 0 {
            println!("MODE7_TEST: configuring PPU for Mode 7 diagnostic");
            // Forced blank during VRAM/CGRAM setup so writes are accepted regardless of timing.
            ppu.write(0x00, 0x8F);
            // Mode 7
            ppu.write(0x05, 0x07);
            // EXTBG on/off per env (default on)
            let extbg = std::env::var("M7_EXTBG")
                .map(|v| v == "1" || v.to_lowercase() == "true")
                .unwrap_or(true);
            ppu.write(0x33, if extbg { 0x40 } else { 0x00 });
            // Main screen: BG1, and BG2 as well when EXTBG is enabled.
            ppu.write(0x2C, if extbg { 0x03 } else { 0x01 });
            // M7SEL: from env flags; defaults R=1 (fill), F=1 (char0), flips off
            let r = std::env::var("M7_R")
                .map(|v| v == "1" || v.to_lowercase() == "true")
                .unwrap_or(true);
            let f = std::env::var("M7_F")
                .map(|v| v == "1" || v.to_lowercase() == "true")
                .unwrap_or(true);
            let flipx = std::env::var("M7_FLIPX")
                .map(|v| v == "1" || v.to_lowercase() == "true")
                .unwrap_or(false);
            let flipy = std::env::var("M7_FLIPY")
                .map(|v| v == "1" || v.to_lowercase() == "true")
                .unwrap_or(false);
            let mut m7sel: u8 = 0;
            if r {
                m7sel |= 0x80;
            }
            if f {
                m7sel |= 0x40;
            }
            if flipy {
                m7sel |= 0x02;
            }
            if flipx {
                m7sel |= 0x01;
            }
            ppu.write(0x1A, m7sel);
            // Matrix from angle/scale; Center at (128,128)
            let w16 = |ppu: &mut crate::ppu::Ppu, reg: u16, val: i16| {
                let lo = (val as u16 & 0x00FF) as u8;
                let hi = ((val as u16 >> 8) & 0xFF) as u8;
                ppu.write(reg, lo);
                ppu.write(reg, hi);
            };
            let scale = std::env::var("M7_SCALE")
                .ok()
                .and_then(|s| s.parse::<f32>().ok())
                .unwrap_or(1.25);
            let angle_deg = std::env::var("M7_ANGLE_DEG")
                .ok()
                .and_then(|s| s.parse::<f32>().ok())
                .unwrap_or(0.0);
            let theta = angle_deg.to_radians();
            let cos_t = theta.cos();
            let sin_t = theta.sin();
            let s256 = scale * 256.0;
            let a = (s256 * cos_t).round() as i16;
            let b = (s256 * -sin_t).round() as i16;
            let c = (s256 * sin_t).round() as i16;
            let d = (s256 * cos_t).round() as i16;
            w16(ppu, 0x1B, a); // A
            w16(ppu, 0x1C, b); // B
            w16(ppu, 0x1D, c); // C
            w16(ppu, 0x1E, d); // D
            w16(ppu, 0x1F, 128); // center X (13-bit signed integer)
            w16(ppu, 0x20, 128); // center Y (13-bit signed integer)
                                 // Palette: 256 entries gradient (covers both EXTBG and non-EXTBG cases)
            ppu.write(0x21, 0x00);
            for i in 0..256u16 {
                let r = (i >> 1) & 0x1F;
                let g = (i >> 1) & 0x1F;
                let b = i & 0x1F;
                let col = (r << 10) | (g << 5) | b;
                ppu.write(0x22, (col & 0xFF) as u8);
                ppu.write(0x22, ((col >> 8) as u8) & 0x7F);
            }

            // Mode 7 VRAM layout (words 0x0000..0x3FFF):
            // - Low byte: tilemap (128x128 bytes)
            // - High byte: tile data (256 tiles * 64 bytes)
            //
            // Fill tilemap with tile #1, and define tile #0/#1 with distinct gradients.
            // Configure VMAIN: increment after HIGH (bit7=1), inc=1
            ppu.write(0x15, 0x80);
            // VMADD = 0x0000 (word address)
            ppu.write(0x16, 0x00);
            ppu.write(0x17, 0x00);
            for w in 0..(128 * 128) {
                let lo = 0x01u8; // map: tile #1 everywhere
                let hi = if w < 64 {
                    // tile0: 0..63 (BG1)
                    w as u8
                } else if w < 128 {
                    // tile1: 128..191 (BG2 when EXTBG)
                    128u8.wrapping_add((w - 64) as u8)
                } else {
                    0u8
                };
                ppu.write(0x18, lo);
                ppu.write(0x19, hi); // increments word address
            }
            // Restore VMAIN to default (inc after LOW, inc=1)
            ppu.write(0x15, 0x00);
            // Unblank at full brightness
            ppu.write(0x00, 0x0F);
            println!(
                "MODE7_TEST: scale={:.2} angle_deg={:.1} EXTBG={} R={} F={} flips=({},{}) z:OBJ[3,2,1,0]=[{},{},{},{}] BG1={} BG2={}",
                scale, angle_deg, extbg, r, f, flipx, flipy,
                crate::debug_flags::m7_z_obj3(), crate::debug_flags::m7_z_obj2(),
                crate::debug_flags::m7_z_obj1(), crate::debug_flags::m7_z_obj0(),
                crate::debug_flags::m7_z_bg1(), crate::debug_flags::m7_z_bg2()
            );
        }
        // Step the PPU through exactly one current frame.
        let total_dots = self.bus.get_ppu().remaining_master_cycles_in_frame() / PPU_CLOCK_DIVIDER;
        for _ in 0..total_dots {
            self.bus.get_ppu_mut().step(1u16);
        }
    }
}
