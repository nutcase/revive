use super::Emulator;

impl Emulator {
    /// Apply SuperFX-specific rendering workarounds
    pub fn apply_superfx_workarounds(&mut self) {
        if self.bus.is_superfx_active() {
            self.bus.get_ppu_mut().superfx_bypass_bg1_window =
                Self::superfx_bypass_bg1_window_enabled();
        }
    }

    pub(super) fn maybe_show_startup_test_pattern(&mut self, enabled: bool) {
        if !enabled {
            return;
        }

        self.bus.get_ppu_mut().force_test_pattern();
        let frame_delay = std::time::Duration::from_millis(16);
        for _ in 0..120 {
            self.render();
            if !self.headless {
                std::thread::sleep(frame_delay);
            }
        }
    }

    pub(super) fn maybe_apply_boot_test_pattern(&mut self, enabled: bool) {
        if !enabled || self.frame_count < 150 {
            return;
        }

        let non_black = {
            let fb = self.bus.get_ppu().get_framebuffer();
            fb.iter()
                .filter(|&&px| px != 0xFF000000 && px != 0x00000000)
                .count()
        };
        if non_black == 0 {
            println!("VISUAL FALLBACK: Applying PPU test pattern (BOOT_TEST_PATTERN=1)");
            self.bus.get_ppu_mut().force_test_pattern();
        }
    }

    // Force unblank regardless of game state (debug/compat aid)
    pub(super) fn maybe_force_unblank(&mut self) {
        let Some((force_always, from, to)) = crate::debug_flags::boot_force_unblank_config() else {
            return;
        };

        if self.frame_count < from || self.frame_count > to {
            return;
        }

        let (imp, vwl, vwh, cg, oam) = self.bus.get_ppu().get_init_counters();
        let forced_blank = {
            let ppu = self.bus.get_ppu();
            ppu.is_forced_blank() || ppu.current_brightness() == 0
        };

        if !forced_blank && !force_always {
            return;
        }

        // Require minimal activity before unblanking to avoid premature intervention
        let has_activity = (vwl + vwh) > 100 || cg > 0 || oam > 0;
        if !force_always && !has_activity && self.frame_count < 200 {
            return;
        }

        let ppu = self.bus.get_ppu_mut();
        if crate::debug_flags::boot_verbose() || crate::debug_flags::compat() {
            println!(
                "🔆 FORCE-UNBLANK: frame={} (imp={} VRAM L/H={}/{} CGRAM={} OAM={})",
                self.frame_count, imp, vwl, vwh, cg, oam
            );
        }

        self.boot_fallback_applied = true;

        // Enable BG1 and set brightness to max (unblank). Write directly to fields to bypass IGNORE_INIDISP_CPU.
        ppu.screen_display = 0x0F;
        ppu.brightness = 0x0F;
        ppu.write(0x2C, 0x01); // TM: BG1 on
                               // Disable color math to avoid unintended global gray (halve/add) on fallback frames
        ppu.write(0x30, 0x00); // CGWSEL: clear
        ppu.write(0x31, 0x00); // CGADSUB: no layers selected
                               // Reset fixed color to black
        ppu.write(0x32, 0x00); // component=0 (no-op/blue=0)
        ppu.write(0x32, 0x20); // set green=0 (component=010) with intensity 0
        ppu.write(0x32, 0x40); // set red=0   (component=100) with intensity 0
                               // If CGRAM is still empty, inject minimal palette for visibility
        if ppu.cgram_usage() == 0 {
            ppu.write(0x21, 0x00); // CGADD=0
            ppu.write(0x22, 0xFF);
            ppu.write(0x22, 0x7F); // White
            ppu.write(0x22, 0x00);
            ppu.write(0x22, 0x7C); // Blue
            ppu.write(0x22, 0x1F);
            ppu.write(0x22, 0x00); // Red
            ppu.write(0x22, 0xE0);
            ppu.write(0x22, 0x03); // Green
        }

        // If somehow brightness is still 0 while forced blank is off, bump it.
        if ppu.current_brightness() == 0 && !ppu.is_forced_blank() {
            ppu.screen_display = 0x0F;
            ppu.brightness = 0x0F;
        }
    }

    pub(super) fn has_scene_activity_behind_forced_blank(&mut self) -> bool {
        let sample_points = [
            (32u16, 32u16),
            (64, 64),
            (128, 64),
            (192, 64),
            (64, 112),
            (128, 112),
            (192, 112),
            (128, 160),
            (32, 176),
            (96, 176),
            (160, 176),
            (224, 176),
            (32, 192),
            (96, 192),
            (160, 192),
            (224, 192),
        ];
        let ppu = self.bus.get_ppu_mut();
        let enables = ppu.effective_main_screen_designation();
        if enables == 0 || ppu.get_bg_mode() == 0 {
            return false;
        }
        for (x, y) in sample_points {
            let (bg_color, _, _) = ppu.get_main_bg_pixel(x, y, enables);
            if bg_color != 0 {
                return true;
            }
            let (obj_color, _) = ppu.get_sprite_pixel(x, y);
            if obj_color != 0 {
                return true;
            }
        }
        false
    }

    // Auto-unblank helper gated by env controls. Runs at specific frame thresholds.
    pub(super) fn maybe_auto_unblank(&mut self) {
        let Some((env_enabled, trace_auto_unblank, threshold)) =
            crate::debug_flags::compat_auto_unblank_config()
        else {
            return;
        };
        if self.boot_fallback_applied {
            if trace_auto_unblank {
                eprintln!(
                    "[AUTO-UNBLANK] frame={} skip=boot_fallback_applied",
                    self.frame_count
                );
            }
            return;
        }
        let second: u64 = threshold.saturating_mul(2);
        let third: u64 = threshold.saturating_mul(3);

        // If the framebuffer already contains visible pixels, prefer unblanking.
        let (forced_blank, brightness, non_black_pixels, tm, bg_mode) = {
            let ppu = self.bus.get_ppu();
            let fb = ppu.get_framebuffer();
            let nb = fb
                .iter()
                .take(256 * 239)
                .filter(|&&px| px != 0xFF000000)
                .count();
            (
                (ppu.screen_display & 0x80) != 0,
                ppu.brightness,
                nb,
                ppu.get_main_screen_designation(),
                ppu.get_bg_mode(),
            )
        };
        if !forced_blank {
            return;
        }
        let scene_active = if self.frame_count >= threshold {
            self.has_scene_activity_behind_forced_blank()
        } else {
            false
        };
        let preserve_existing_scene =
            self.frame_count >= threshold && tm != 0 && (scene_active || non_black_pixels > 0);

        if trace_auto_unblank {
            eprintln!(
                "[AUTO-UNBLANK] frame={} forced_blank={} bright={} tm={:02X} mode={} non_black={} scene_active={} env={} preserve={} threshold={}",
                self.frame_count,
                forced_blank as u8,
                brightness,
                tm,
                bg_mode,
                non_black_pixels,
                scene_active as u8,
                env_enabled as u8,
                preserve_existing_scene as u8,
                threshold
            );
        }

        // Rebuilding from the final PPU register state loses per-scanline HDMA state.
        // Keep it behind the explicit compatibility fallback so normal frames stay faithful.
        if !env_enabled {
            if trace_auto_unblank && preserve_existing_scene {
                eprintln!(
                    "[AUTO-UNBLANK] frame={} skip=compat_fallback_disabled",
                    self.frame_count
                );
            }
            return;
        }

        if !preserve_existing_scene
            && !(self.frame_count == threshold
                || self.frame_count == second
                || self.frame_count == third)
        {
            return;
        }

        // Heuristics: plenty of VRAM writes and zero CGRAM writes yet
        let (_imp, vwl, vwh, cg, oam) = self.bus.get_ppu().get_init_counters();
        let vram_min: u64 = std::env::var("COMPAT_AUTO_UNBLANK_VRAM_MIN")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(4096);
        let cgram_max: u64 = std::env::var("COMPAT_AUTO_UNBLANK_CGRAM_MAX")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        let minimal_activity = (vwl + vwh) > 0 || cg > 0 || oam > 0;
        if !preserve_existing_scene && (cg > cgram_max || (vwl + vwh) <= vram_min) {
            // Heuristics did not pass. As a last resort, if we've waited long enough and
            // the display is still blank while there is at least some activity, unblank anyway.
            let late_fallback = self.frame_count >= third && minimal_activity;
            if !late_fallback {
                return;
            }
        }

        // Also require that the game touched OAM a bit (sprites prepped), but keep this lenient
        // Keep permissive: allow unblank even if framebuffer is still black
        if !preserve_existing_scene && oam == 0 && non_black_pixels == 0 && self.frame_count < third
        {
            return;
        }

        if crate::debug_flags::boot_verbose() || crate::debug_flags::compat() {
            if preserve_existing_scene {
                println!(
                    "COMPAT: Auto-unblank existing scene at frame {} (TM=0x{:02X}, non_black={}, brightness={}).",
                    self.frame_count, tm, non_black_pixels, brightness
                );
            } else {
                println!(
                    "COMPAT: Auto-unblank at frame {} (VRAM L/H={} / {}, CGRAM={}, OAM={}, non_black={} ; brightness={}).",
                    self.frame_count, vwl, vwh, cg, oam, non_black_pixels, brightness
                );
                println!("        Forcing INIDISP=0x0F, TM=BG1 (fallback)");
            }
        }
        let ppu_mut = self.bus.get_ppu_mut();
        if preserve_existing_scene {
            let target_brightness = if brightness == 0 {
                0x0F
            } else {
                brightness & 0x0F
            };
            ppu_mut.screen_display = target_brightness;
            ppu_mut.brightness = target_brightness & 0x0F;
            ppu_mut.latched_inidisp = Some(target_brightness);
            ppu_mut.rebuild_presented_framebuffer();
            return;
        }
        ppu_mut.write(0x2C, 0x01); // TM: BG1 on
        ppu_mut.write(0x00, 0x0F); // INIDISP: brightness 15, unblank
                                   // Also disable color math to avoid global gray when palette is not ready yet
        ppu_mut.write(0x30, 0x00); // CGWSEL: clear
        ppu_mut.write(0x31, 0x00); // CGADSUB: no layers selected
        ppu_mut.rebuild_presented_framebuffer();

        if crate::debug_flags::compat_inject_min_palette() {
            if crate::debug_flags::boot_verbose() || crate::debug_flags::compat() {
                println!("COMPAT: Injecting minimal CGRAM palette (fallback)");
            }
            ppu_mut.write(0x21, 0x00); // CGADD=0
                                       // Color 0: White (backdrop visible)
            ppu_mut.write(0x22, 0xFF);
            ppu_mut.write(0x22, 0x7F);
            // Color 1: Blue
            ppu_mut.write(0x22, 0x00);
            ppu_mut.write(0x22, 0x7C);
            // Color 2: Red
            ppu_mut.write(0x22, 0x1F);
            ppu_mut.write(0x22, 0x00);
            // Color 3: Green
            ppu_mut.write(0x22, 0xE0);
            ppu_mut.write(0x22, 0x03);
        }
        self.boot_fallback_applied = true;
    }

    // Periodically inject a minimal visible palette until the game loads enough CGRAM
    pub(super) fn maybe_inject_min_palette_periodic(&mut self) {
        if !crate::debug_flags::compat_periodic_min_palette() {
            return;
        }
        // Only when CGRAM is still tiny
        let need_help = { self.bus.get_ppu().cgram_usage() < 32 };
        if !need_help {
            return;
        }
        // Every 30 frames, inject a few colors
        if !self.frame_count.is_multiple_of(30) {
            return;
        }
        let ppu = self.bus.get_ppu_mut();
        // Ensure BG1 on and unblank, color math off
        ppu.write(0x2C, 0x01);
        ppu.write(0x00, 0x0F);
        ppu.write(0x30, 0x00);
        ppu.write(0x31, 0x00);
        // Inject colors 0..7
        ppu.write(0x21, 0x00);
        // 0: White, 1: Blue, 2: Red, 3: Green, 4..7: Gray steps
        // White
        ppu.write(0x22, 0xFF);
        ppu.write(0x22, 0x7F);
        // Blue
        ppu.write(0x22, 0x00);
        ppu.write(0x22, 0x7C);
        // Red
        ppu.write(0x22, 0x1F);
        ppu.write(0x22, 0x00);
        // Green
        ppu.write(0x22, 0xE0);
        ppu.write(0x22, 0x03);
        // Gray tones
        for lvl in [0x10u8, 0x20, 0x30, 0x3A] {
            ppu.write(0x22, lvl);
            ppu.write(0x22, 0x3F);
        }
    }
}
