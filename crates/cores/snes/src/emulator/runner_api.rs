use super::{Emulator, SCREEN_HEIGHT, SCREEN_WIDTH};
use crate::shutdown;
use std::time::{Duration, Instant};

impl Emulator {
    // ── Public accessors for the SDL runner ────────────────────────────────

    fn step_one_frame_inner_with_render_and_audio(
        &mut self,
        render_frame: bool,
        emit_audio: bool,
    ) -> bool {
        let frame_start = Instant::now();
        let trace_slow_ms = crate::debug_flags::trace_starfox_gui_slow_ms();
        let trace_slow_frame =
            trace_slow_ms > 0 && self.rom_title().to_ascii_uppercase().contains("STAR FOX");
        let (before_cpu_pc, before_inidisp, before_tm, before_mode) = if trace_slow_frame {
            (
                self.current_cpu_pc(),
                self.current_inidisp(),
                self.current_tm(),
                self.current_bg_mode(),
            )
        } else {
            (0, 0, 0, 0)
        };
        self.bus
            .get_ppu_mut()
            .set_framebuffer_rendering_enabled(render_frame);
        crate::cartridge::superfx::set_trace_superfx_exec_frame(self.frame_count.wrapping_add(1));
        let run_frame_start = if trace_slow_frame {
            Some(Instant::now())
        } else {
            None
        };
        self.suppress_next_audio_output = !emit_audio;
        self.run_frame();
        self.suppress_next_audio_output = false;
        let run_frame_time = run_frame_start
            .map(|start| start.elapsed())
            .unwrap_or(Duration::ZERO);
        if self.take_save_state_capture_stop_requested() {
            return true;
        }
        if self.maybe_save_state_at_frame_anchor() {
            return true;
        }
        // Keep state-based diagnostics aligned with the main run loop.
        let compat_start = if trace_slow_frame {
            Some(Instant::now())
        } else {
            None
        };
        self.maybe_auto_unblank();
        self.maybe_force_unblank();
        self.maybe_inject_min_palette_periodic();
        let compat_time = compat_start
            .map(|start| start.elapsed())
            .unwrap_or(Duration::ZERO);
        let mut render_time = Duration::ZERO;
        if render_frame {
            let render_start = if trace_slow_frame {
                Some(Instant::now())
            } else {
                None
            };
            self.render();
            render_time = render_start
                .map(|start| start.elapsed())
                .unwrap_or(Duration::ZERO);
        }
        let frame_time = frame_start.elapsed();
        if trace_slow_frame && frame_time.as_millis() >= trace_slow_ms {
            eprintln!(
                "[STARFOX-FRAME-SLOW] frame={} cpu_pc={:06X}->{:06X} inidisp={:02X}->{:02X} tm={:02X}->{:02X} mode={}->{} run_ms={} compat_ms={} render_ms={} total_ms={}",
                self.frame_count,
                before_cpu_pc,
                self.current_cpu_pc(),
                before_inidisp,
                self.current_inidisp(),
                before_tm,
                self.current_tm(),
                before_mode,
                self.current_bg_mode(),
                run_frame_time.as_millis(),
                compat_time.as_millis(),
                render_time.as_millis(),
                frame_time.as_millis(),
            );
        }
        let _ = self.performance_stats.update(frame_time);
        self.frame_count = self.frame_count.wrapping_add(1);
        self.maybe_dump_framebuffer_at();
        self.maybe_dump_mem_at();
        self.maybe_save_state_at();
        if shutdown::should_quit() {
            return true;
        }
        self.maybe_dump_starfox_diag_at();
        self.maybe_autosave_sram();
        false
    }

    fn step_one_frame_inner_with_render(&mut self, render_frame: bool) -> bool {
        self.step_one_frame_inner_with_render_and_audio(render_frame, true)
    }

    pub(super) fn step_one_frame_inner(&mut self) -> bool {
        self.step_one_frame_inner_with_render(true)
    }

    pub fn warmup_until_visible(&mut self, max_frames: u64) -> bool {
        for _ in 0..max_frames {
            self.bus
                .get_ppu_mut()
                .set_framebuffer_rendering_enabled(true);
            crate::cartridge::superfx::set_trace_superfx_exec_frame(
                self.frame_count.wrapping_add(1),
            );
            self.run_frame();
            if self.take_save_state_capture_stop_requested() {
                break;
            }
            if self.maybe_save_state_at_frame_anchor() {
                break;
            }
            self.maybe_auto_unblank();
            self.maybe_force_unblank();
            self.maybe_inject_min_palette_periodic();
            self.frame_count = self.frame_count.wrapping_add(1);
            if self.has_visible_output() {
                return true;
            }
            if shutdown::should_quit() {
                break;
            }
        }
        self.has_visible_output()
    }

    pub fn warmup_until_cpu_leaves_pc(&mut self, max_frames: u64, blocked_pc: u32) -> bool {
        for _ in 0..max_frames {
            self.bus
                .get_ppu_mut()
                .set_framebuffer_rendering_enabled(true);
            crate::cartridge::superfx::set_trace_superfx_exec_frame(
                self.frame_count.wrapping_add(1),
            );
            self.run_frame();
            if self.take_save_state_capture_stop_requested() {
                break;
            }
            if self.maybe_save_state_at_frame_anchor() {
                break;
            }
            self.maybe_auto_unblank();
            self.maybe_force_unblank();
            self.maybe_inject_min_palette_periodic();
            self.frame_count = self.frame_count.wrapping_add(1);
            if self.current_cpu_pc() != blocked_pc {
                return true;
            }
            if shutdown::should_quit() {
                break;
            }
        }
        self.current_cpu_pc() != blocked_pc
    }

    #[allow(dead_code)]
    /// Run exactly one video frame of emulation.
    pub fn step_one_frame(&mut self) {
        let _ = self.step_one_frame_inner();
    }

    pub fn step_one_frame_with_render(&mut self, render_frame: bool) {
        let _ = self.step_one_frame_inner_with_render(render_frame);
    }

    pub fn step_one_frame_with_render_and_audio(&mut self, render_frame: bool, emit_audio: bool) {
        let _ = self.step_one_frame_inner_with_render_and_audio(render_frame, emit_audio);
    }

    pub fn clear_audio_buffer(&mut self) {
        self.audio_system.clear_buffer();
    }

    pub fn audio_sample_rate(&self) -> u32 {
        self.audio_system.sample_rate()
    }

    pub fn audio_callback_source(&self) -> crate::audio::SnesAudioCallbackSource {
        self.audio_system.callback_source()
    }

    #[allow(dead_code)]
    /// Return the current PPU framebuffer (256×239, ARGB u32).
    pub fn framebuffer(&mut self) -> &[u32] {
        if self.populate_superfx_gui_fallback_framebuffer() {
            self.frame_buffer.as_slice()
        } else if self.populate_starfox_autocontrast_framebuffer() {
            self.frame_buffer.as_slice()
        } else {
            self.bus.get_ppu().get_framebuffer()
        }
    }

    pub fn has_visible_output(&self) -> bool {
        let ppu = self.bus.get_ppu();
        let tm = ppu.get_main_screen_designation();
        let bg_mode = ppu.get_bg_mode();
        let non_black = ppu
            .get_framebuffer()
            .iter()
            .any(|&p| (p & 0x00FF_FFFF) != 0);
        tm != 0 && bg_mode != 0 && non_black
    }

    pub fn has_starfox_title_output(&self) -> bool {
        let ppu = self.bus.get_ppu();
        if !ppu.starfox_title_layout_active() {
            return false;
        }
        ppu.get_framebuffer()
            .iter()
            .take(SCREEN_WIDTH * SCREEN_HEIGHT)
            .any(|&p| (p & 0x00FF_FFFF) != 0)
    }

    pub fn has_unblanked_output(&self) -> bool {
        let ppu = self.bus.get_ppu();
        let non_black = ppu
            .get_framebuffer()
            .iter()
            .any(|&p| (p & 0x00FF_FFFF) != 0);
        (!ppu.is_forced_blank()
            && ppu.current_brightness() != 0
            && ppu.get_main_screen_designation() != 0)
            || (ppu.get_main_screen_designation() != 0 && non_black)
    }

    pub fn warmup_until_unblanked(&mut self, max_frames: u64) -> bool {
        for _ in 0..max_frames {
            self.bus
                .get_ppu_mut()
                .set_framebuffer_rendering_enabled(true);
            crate::cartridge::superfx::set_trace_superfx_exec_frame(
                self.frame_count.wrapping_add(1),
            );
            self.run_frame();
            if self.take_save_state_capture_stop_requested() {
                break;
            }
            if self.maybe_save_state_at_frame_anchor() {
                break;
            }
            self.maybe_auto_unblank();
            self.maybe_force_unblank();
            self.maybe_inject_min_palette_periodic();
            self.frame_count = self.frame_count.wrapping_add(1);
            if self.has_unblanked_output() {
                return true;
            }
            if shutdown::should_quit() {
                break;
            }
        }
        self.has_unblanked_output()
    }

    pub fn frame_count(&self) -> u64 {
        self.frame_count
    }

    pub fn current_cpu_pc(&self) -> u32 {
        ((self.cpu.pb() as u32) << 16) | self.cpu.pc() as u32
    }

    pub fn current_inidisp(&self) -> u8 {
        self.bus.get_ppu().screen_display
    }

    pub fn current_tm(&self) -> u8 {
        self.bus.get_ppu().get_main_screen_designation()
    }

    pub fn current_bg_mode(&self) -> u8 {
        self.bus.get_ppu().get_bg_mode()
    }

    pub fn current_hidden_scene_activity(&mut self) -> bool {
        self.has_scene_activity_behind_forced_blank()
    }

    pub fn current_superfx_pc(&self) -> Option<u32> {
        let gsu = self.bus.superfx.as_ref()?;
        let pc = if gsu.debug_current_exec_pc() != 0 {
            ((gsu.debug_current_exec_pbr() as u32) << 16) | u32::from(gsu.debug_current_exec_pc())
        } else {
            ((gsu.debug_pbr() as u32) << 16) | u32::from(gsu.debug_reg(15))
        };
        Some(pc)
    }

    pub fn current_superfx_running(&self) -> Option<bool> {
        self.bus.superfx.as_ref().map(|gsu| gsu.running())
    }

    pub fn current_superfx_reg(&self, reg: usize) -> Option<u16> {
        self.bus.superfx.as_ref().map(|gsu| gsu.debug_reg(reg))
    }

    pub fn set_framebuffer_rendering_enabled(&mut self, enabled: bool) {
        self.bus
            .get_ppu_mut()
            .set_framebuffer_rendering_enabled(enabled);
    }

    pub fn debug_apply_ppu_overrides_from_env(&mut self) {
        let ppu = self.bus.get_ppu_mut();
        if let Ok(value) = std::env::var("DEBUG_OVERRIDE_CGWSEL") {
            if let Ok(parsed) = u8::from_str_radix(value.trim_start_matches("0x"), 16) {
                ppu.cgwsel = parsed;
            }
        }
        if let Ok(value) = std::env::var("DEBUG_OVERRIDE_CGADSUB") {
            if let Ok(parsed) = u8::from_str_radix(value.trim_start_matches("0x"), 16) {
                ppu.cgadsub = parsed;
            }
        }
        if let Ok(value) = std::env::var("DEBUG_OVERRIDE_FIXED_COLOR") {
            if let Ok(parsed) = u16::from_str_radix(value.trim_start_matches("0x"), 16) {
                ppu.fixed_color = parsed;
            }
        }
    }

    #[allow(dead_code)]
    pub fn wram(&self) -> &[u8] {
        &self.bus.wram
    }
    #[allow(dead_code)]
    pub fn wram_mut(&mut self) -> &mut [u8] {
        &mut self.bus.wram
    }
    #[allow(dead_code)]
    pub fn sram(&self) -> &[u8] {
        &self.bus.sram
    }
    #[allow(dead_code)]
    pub fn sram_mut(&mut self) -> &mut [u8] {
        &mut self.bus.sram
    }

    #[allow(dead_code)]
    pub fn set_key_states(&mut self, ks: &crate::input::KeyStates) {
        self.bus.get_input_system_mut().handle_key_input(ks);
    }

    #[allow(dead_code)]
    pub fn rom_title(&self) -> &str {
        &self.rom_title
    }
}
