use super::Ppu;

impl Ppu {
    pub(super) fn enter_vblank(&mut self) {
        self.v_blank = true;
        // STAT78 field flag toggles every VBlank.
        self.interlace_field = !self.interlace_field;
        self.rdnmi_read_in_vblank = false; // 新しいVBlankでリセット
                                           // RDNMIフラグ（$4210 bit7）はNMI許可に関わらずVBlank突入で立つ。
                                           // 読み出しでクリアされるが、VBlank中は常に再セットされる挙動に近づける。
        self.nmi_flag = true;
        // NMIパルスは許可時のみCPUへ届ける。ラッチを使って多重発火を防ぐ。
        if self.nmi_enabled && !self.nmi_latched {
            self.nmi_latched = true; // ensure one NMI per VBlank
        }
        // SNESdev: internal OAM address resets to OAMADD at VBlank start when display is enabled.
        if !self.is_forced_blank() {
            self.oam_internal_addr = (self.oam_addr & 0x01FF) << 1;
            self.refresh_oam_eval_base_from_internal_addr();
        }
        if crate::debug_flags::trace_vblank() {
            use std::sync::atomic::{AtomicU32, Ordering};
            static COUNT: AtomicU32 = AtomicU32::new(0);
            let n = COUNT.fetch_add(1, Ordering::Relaxed);
            if n < 8 {
                println!(
                    "[TRACE_VBLANK] frame={} scanline={} nmi_flag={} nmi_en={} latched={}",
                    self.frame, self.scanline, self.nmi_flag, self.nmi_enabled, self.nmi_latched
                );
            }
        }
    }

    pub(super) fn exit_vblank(&mut self) {
        self.v_blank = false;
        // RDNMIフラグ(bit7)は読み出しでクリアされるため、
        // VBlank終了時には下げない（未読なら次フレームまで保持する）。
        // NMIラッチはCPU側がNMIを処理した時点でクリアされるため、ここでは触らない。
        self.rdnmi_read_in_vblank = false;
        // STAT77 flags are reset at end of VBlank.
        self.sprite_overflow_latched = false;
        self.sprite_time_over_latched = false;
    }

    // Returns true if we're currently in the active display area (not V/HBlank)
    #[inline]
    pub(super) fn in_active_display(&self) -> bool {
        let vis_lines = self.get_visible_height();
        let v_vis = self.scanline < vis_lines;
        let h_vis = self.cycle >= self.first_visible_dot() && self.cycle < self.first_hblank_dot();
        v_vis && h_vis && !self.v_blank && !self.h_blank
    }

    #[inline]
    pub(super) fn is_vram_write_safe_dot(&self) -> bool {
        // VRAM data port ($2118/$2119) writes are only effective during:
        // - forced blank (INIDISP bit7), or
        // - VBlank, or
        // - a small HBlank window for DMA/HDMA (timing-sensitive titles rely on this)
        //
        // NOTE: Even when the write is ignored, VMADD still increments based on VMAIN. The
        // caller must apply the increment regardless of the return value here.
        self.can_write_vram_now()
    }

    #[inline]
    pub(crate) fn can_read_vram_now(&self) -> bool {
        // SNESdev wiki: VRAM reads via $2139/$213A are only valid during VBlank or forced blank.
        if (self.screen_display & 0x80) != 0 {
            return true;
        }
        let vblank_start = self.vblank_start_line();
        self.v_blank || self.scanline >= vblank_start
    }

    #[inline]
    pub(crate) fn can_read_oam_now(&self) -> bool {
        // OAM reads are only reliable during forced blank or VBlank.
        if (self.screen_display & 0x80) != 0 {
            return true;
        }
        let vblank_start = self.vblank_start_line();
        self.v_blank || self.scanline >= vblank_start
    }

    #[inline]
    pub(crate) fn can_read_cgram_now(&self) -> bool {
        // CGRAM reads follow the same basic access window as OAM on real hardware.
        if (self.screen_display & 0x80) != 0 {
            return true;
        }
        let vblank_start = self.vblank_start_line();
        self.v_blank || self.scanline >= vblank_start
    }

    // Centralized timing gates for graphics register writes.
    // These are coarse approximations meant to be refined over time.
    #[inline]
    pub(super) fn can_write_vram_now(&self) -> bool {
        if self.write_ctx == 2 {
            if (self.screen_display & 0x80) != 0 {
                return true;
            }
            let vblank_start = self.vblank_start_line();
            if self.v_blank || self.scanline >= vblank_start {
                return true;
            }
            if !self.h_blank {
                return false;
            }
            // HDMA: allow a narrower HBlank window
            let head = crate::debug_flags::vram_hdma_head();
            let tail = crate::debug_flags::vram_hdma_tail();
            return Self::hblank_window_open(
                self.cycle,
                self.first_hblank_dot(),
                self.last_dot_index(),
                head,
                tail,
                self.hdma_head_busy_until,
            );
        }
        self.can_write_vram_non_hdma_now()
    }

    #[inline]
    pub(super) fn can_write_cgram_now(&self) -> bool {
        if self.write_ctx == 2 {
            if (self.screen_display & 0x80) != 0 {
                return true;
            }
            let vblank_start = self.vblank_start_line();
            if self.v_blank || self.scanline >= vblank_start {
                return true;
            }
            if !self.h_blank {
                return false;
            }
            let head = crate::debug_flags::cgram_hdma_head();
            let tail = crate::debug_flags::cgram_hdma_tail();
            return Self::hblank_window_open(
                self.cycle,
                self.first_hblank_dot(),
                self.last_dot_index(),
                head,
                tail,
                self.hdma_head_busy_until,
            );
        }
        self.can_write_cgram_non_hdma_now()
    }

    #[inline]
    pub(super) fn can_write_oam_now(&self) -> bool {
        self.can_write_oam_non_hdma_now()
    }

    #[inline]
    pub(crate) fn can_write_vram_non_hdma_now(&self) -> bool {
        let strict = crate::debug_flags::strict_ppu_timing();
        if (self.screen_display & 0x80) != 0 {
            return true;
        }
        let vblank_start = self.vblank_start_line();
        if self.v_blank || self.scanline >= vblank_start {
            if strict {
                let head = crate::debug_flags::vram_vblank_head();
                let tail = crate::debug_flags::vram_vblank_tail();
                if !Self::vblank_window_open(
                    self.scanline,
                    self.cycle,
                    vblank_start,
                    self.last_scanline_index(),
                    self.last_dot_index(),
                    head,
                    tail,
                ) {
                    return false;
                }
            }
            return true;
        }
        if !self.h_blank {
            return false;
        }
        if self.vmain_data_gap_ticks > 0 {
            return false;
        }
        let head = crate::debug_flags::vram_mdma_head();
        let tail = crate::debug_flags::vram_mdma_tail();
        Self::hblank_window_open(
            self.cycle,
            self.first_hblank_dot(),
            self.last_dot_index(),
            head,
            tail,
            self.hdma_head_busy_until,
        )
    }

    #[inline]
    pub(crate) fn can_write_cgram_non_hdma_now(&self) -> bool {
        let strict = crate::debug_flags::strict_ppu_timing();
        if (self.screen_display & 0x80) != 0 {
            return true;
        }
        let vblank_start = self.vblank_start_line();
        if self.v_blank || self.scanline >= vblank_start {
            if strict {
                let head = crate::debug_flags::cgram_vblank_head();
                let tail = crate::debug_flags::cgram_vblank_tail();
                if !Self::vblank_window_open(
                    self.scanline,
                    self.cycle,
                    vblank_start,
                    self.last_scanline_index(),
                    self.last_dot_index(),
                    head,
                    tail,
                ) {
                    return false;
                }
            }
            return true;
        }
        if !self.h_blank {
            return false;
        }
        if self.cgram_data_gap_ticks > 0 {
            return false;
        }
        let head = crate::debug_flags::cgram_mdma_head();
        let tail = crate::debug_flags::cgram_mdma_tail();
        Self::hblank_window_open(
            self.cycle,
            self.first_hblank_dot(),
            self.last_dot_index(),
            head,
            tail,
            0,
        )
    }

    #[inline]
    pub(crate) fn can_write_oam_non_hdma_now(&self) -> bool {
        // OAM is writable only during VBlank or forced blank on real hardware.
        // We always enforce this basic rule; STRICT_PPU_TIMING further narrows
        // the safe window inside VBlank.
        // During forced blank (INIDISP bit7), OAM is accessible at any time.
        if (self.screen_display & 0x80) != 0 {
            return true;
        }
        let vblank_start = self.vblank_start_line();
        if self.v_blank || self.scanline >= vblank_start {
            let head = if crate::debug_flags::strict_ppu_timing() {
                crate::debug_flags::oam_vblank_head()
            } else {
                0
            };
            let tail = if crate::debug_flags::strict_ppu_timing() {
                crate::debug_flags::oam_vblank_tail()
            } else {
                0
            };
            if !Self::oam_vblank_write_window_open(
                self.scanline,
                self.cycle,
                vblank_start,
                self.last_scanline_index(),
                self.last_dot_index(),
                head,
                tail,
                crate::debug_flags::oam_gap_in_vblank(),
                self.oam_data_gap_ticks,
            ) {
                return false;
            }
            return true;
        }
        false
    }

    #[inline]
    pub(super) fn vblank_window_open(
        scanline: u16,
        cycle: u16,
        vblank_start: u16,
        last_scanline: u16,
        last_dot: u16,
        head: u16,
        tail: u16,
    ) -> bool {
        if scanline < vblank_start {
            return false;
        }
        if head > 0 && scanline == vblank_start && cycle < head {
            return false;
        }
        if tail > 0 && scanline == last_scanline && cycle > last_dot.saturating_sub(tail) {
            return false;
        }
        true
    }

    #[inline]
    #[allow(clippy::too_many_arguments)]
    pub(super) fn oam_vblank_write_window_open(
        scanline: u16,
        cycle: u16,
        vblank_start: u16,
        last_scanline: u16,
        last_dot: u16,
        head: u16,
        tail: u16,
        gap_enabled: bool,
        gap_ticks: u16,
    ) -> bool {
        Self::vblank_window_open(
            scanline,
            cycle,
            vblank_start,
            last_scanline,
            last_dot,
            head,
            tail,
        ) && !(gap_enabled && gap_ticks > 0)
    }

    #[inline]
    pub(super) fn hblank_window_open(
        cycle: u16,
        first_hblank_dot: u16,
        last_dot: u16,
        head: u16,
        tail: u16,
        busy_until: u16,
    ) -> bool {
        let start = first_hblank_dot.saturating_add(head).max(busy_until);
        cycle >= start && cycle <= last_dot.saturating_sub(tail)
    }

    // Apply any latched display-affecting registers at the start of a scanline.
    pub(super) fn commit_latched_display_regs(&mut self) {
        let mut any = false;
        if let Some(v) = self.latched_inidisp.take() {
            let prev_display = self.screen_display;
            self.screen_display = v;
            self.brightness = v & 0x0F;
            self.maybe_reset_oam_on_inidisp(prev_display, v);
            any = true;
        }
        if let Some(v) = self.latched_tm.take() {
            self.main_screen_designation = v;
            if v != 0 {
                self.main_screen_designation_last_nonzero = v;
            }
            any = true;
        }
        if let Some(v) = self.latched_ts.take() {
            self.sub_screen_designation = v;
            any = true;
        }
        if let Some(v) = self.latched_tmw.take() {
            self.tmw_mask = v & 0x1F;
            any = true;
        }
        if let Some(v) = self.latched_tsw.take() {
            self.tsw_mask = v & 0x1F;
            any = true;
        }
        if let Some(v) = self.latched_cgwsel.take() {
            self.cgwsel = v;
            self.color_math_control = v;
            any = true;
        }
        if let Some(v) = self.latched_cgadsub.take() {
            self.cgadsub = v;
            self.color_math_designation = v;
            any = true;
        }
        if let Some(v) = self.latched_fixed_color.take() {
            self.fixed_color = v;
            any = true;
        }
        if let Some(v) = self.latched_setini.take() {
            self.setini = v;
            self.pseudo_hires = (v & 0x08) != 0;
            self.extbg = (v & 0x40) != 0;
            self.overscan = (v & 0x04) != 0;
            self.obj_interlace = (v & 0x02) != 0;
            self.interlace = (v & 0x01) != 0;
            any = true;
        }
        if let Some(v) = self.latched_wbglog.take() {
            self.bg_window_logic[0] = v & 0x03;
            self.bg_window_logic[1] = (v >> 2) & 0x03;
            self.bg_window_logic[2] = (v >> 4) & 0x03;
            self.bg_window_logic[3] = (v >> 6) & 0x03;
            any = true;
        }
        if let Some(v) = self.latched_wobjlog.take() {
            self.obj_window_logic = v & 0x03;
            self.color_window_logic = (v >> 2) & 0x03;
            any = true;
        }
        if any && crate::debug_flags::boot_verbose() {
            println!("PPU: latched regs committed at line {}", self.scanline);
        }
    }

    #[inline]
    pub(super) fn maybe_reset_oam_on_inidisp(&mut self, prev_display: u8, new_display: u8) {
        // OAM reset: when forced blank is deactivated, the internal OAM address reloads
        // from OAMADD (in addition to the standard VBlank-start reset when not blanked).
        let prev_blank = (prev_display & 0x80) != 0;
        let new_blank = (new_display & 0x80) != 0;
        if prev_blank && !new_blank {
            self.oam_internal_addr = (self.oam_addr & 0x01FF) << 1;
            self.refresh_oam_eval_base_from_internal_addr();
            if crate::debug_flags::trace_oam_reset() && !crate::debug_flags::quiet() {
                println!(
                    "[OAM-RESET] scanline={} frame={} oam_addr=0x{:03X} internal=0x{:03X}",
                    self.scanline, self.frame, self.oam_addr, self.oam_internal_addr
                );
            }
        }
    }

    #[inline]
    pub(crate) fn refresh_oam_eval_base_from_internal_addr(&mut self) {
        self.oam_eval_base = if self.oam_priority_rotation_enabled {
            ((self.oam_internal_addr >> 2) & 0x7F) as u8
        } else {
            0
        };
    }

    // Determine if it is safe to commit VMADD (VRAM address) now
    pub(super) fn can_commit_vmadd_now(&self) -> bool {
        if !crate::debug_flags::strict_ppu_timing() {
            return true;
        }
        // During forced blank (INIDISP bit7), VRAM control regs are writable at any time.
        if (self.screen_display & 0x80) != 0 {
            return true;
        }
        let vblank_start = self.vblank_start_line();
        if self.v_blank || self.scanline >= vblank_start {
            return true;
        }
        if !self.h_blank {
            return false;
        }
        let x = self.cycle;
        let hb = self.first_hblank_dot();
        let last = self.last_dot_index();
        let head = hb
            .saturating_add(crate::debug_flags::vmadd_ctrl_head())
            .max(self.hdma_head_busy_until);
        let tail = crate::debug_flags::vmadd_ctrl_tail();
        x >= head && x <= (last.saturating_sub(tail))
    }

    // Determine if it is safe to commit CGADD (CGRAM address) now
    pub(super) fn can_commit_cgadd_now(&self) -> bool {
        if !crate::debug_flags::strict_ppu_timing() {
            return true;
        }
        // During forced blank (INIDISP bit7), CGRAM control regs are writable at any time.
        if (self.screen_display & 0x80) != 0 {
            return true;
        }
        let vblank_start = self.vblank_start_line();
        if self.v_blank || self.scanline >= vblank_start {
            return true;
        }
        if !self.h_blank {
            return false;
        }
        let x = self.cycle;
        let hb = self.first_hblank_dot();
        let last = self.last_dot_index();
        let head = hb
            .saturating_add(crate::debug_flags::cgadd_ctrl_head())
            .max(self.hdma_head_busy_until);
        let tail = crate::debug_flags::cgadd_ctrl_tail();
        x >= head && x <= (last.saturating_sub(tail))
    }

    // Determine if it is safe to commit VMAIN (VRAM control) now
    pub(super) fn can_commit_vmain_now(&self) -> bool {
        // Reuse VMADD control margins
        self.can_commit_vmadd_now()
    }

    // Commit pending control registers if safe
    pub(super) fn commit_pending_ctrl_if_any(&mut self) {
        // VMADD
        if (self.latched_vmadd_lo.is_some() || self.latched_vmadd_hi.is_some())
            && self.can_commit_vmadd_now()
        {
            let mut changed = false;
            if let Some(lo) = self.latched_vmadd_lo.take() {
                self.vram_addr = (self.vram_addr & 0xFF00) | (lo as u16);
                changed = true;
            }
            if let Some(hi) = self.latched_vmadd_hi.take() {
                self.vram_addr = (self.vram_addr & 0x00FF) | ((hi as u16) << 8);
                changed = true;
            }
            if changed {
                // SNESdev wiki: On VMADD write, vram_latch = [VMADD]
                self.reload_vram_read_latch();
            }
        }
        // CGADD
        if self.latched_cgadd.is_some() && self.can_commit_cgadd_now() {
            if let Some(v) = self.latched_cgadd.take() {
                self.cgadd_effect_pending = Some(v);
                self.cgadd_effect_ticks = crate::debug_flags::cgadd_effect_delay_dots();
            }
        }
        // VMAIN
        if let Some(v) = self.latched_vmain.take() {
            if self.can_commit_vmain_now() {
                // Defer the visible effect by a small number of dots
                self.vmain_effect_pending = Some(v);
                self.vmain_effect_ticks = crate::debug_flags::vmain_effect_delay_dots();
            } else {
                // Put back if still unsafe
                self.latched_vmain = Some(v);
            }
        }
    }

    // Tick and apply deferred control effects (called each dot)
    pub(super) fn tick_deferred_ctrl_effects(&mut self) {
        if self.vmain_effect_pending.is_none()
            && self.cgadd_effect_pending.is_none()
            && self.vmain_data_gap_ticks == 0
            && self.oam_data_gap_ticks == 0
        {
            return;
        }
        if self.vmain_effect_pending.is_some() {
            if self.vmain_effect_ticks > 0 {
                self.vmain_effect_ticks -= 1;
            }
            if self.vmain_effect_ticks == 0 {
                if let Some(v) = self.vmain_effect_pending.take() {
                    self.vram_mapping = v;
                    self.vram_last_vmain = v;
                    // Update increment now that mapping took effect
                    match v & 0x03 {
                        0 => self.vram_increment = 1,
                        1 => self.vram_increment = 32,
                        2 | 3 => self.vram_increment = 128,
                        _ => {}
                    }
                    if crate::debug_flags::ppu_write() {
                        let inc = match v & 0x03 {
                            0 => 1,
                            1 => 32,
                            _ => 128,
                        };
                        let fg = (v >> 2) & 0x03;
                        let inc_on_high = (v & 0x80) != 0;
                        println!(
                            "VMAIN applied: 0x{:02X} (inc={}, FGmode={}, inc_on_{})",
                            v,
                            inc,
                            fg,
                            if inc_on_high { "HIGH" } else { "LOW" }
                        );
                    }
                    // Start a small MDMA/CPU gap after VMAIN effect
                    self.vmain_data_gap_ticks = crate::debug_flags::vram_gap_after_vmain();
                }
            }
        }
        if self.vmain_data_gap_ticks > 0 {
            self.vmain_data_gap_ticks -= 1;
        }
        if self.oam_data_gap_ticks > 0 {
            self.oam_data_gap_ticks -= 1;
        }
        if self.cgadd_effect_pending.is_some() {
            if self.cgadd_effect_ticks > 0 {
                self.cgadd_effect_ticks -= 1;
            }
            if self.cgadd_effect_ticks == 0 {
                if let Some(v) = self.cgadd_effect_pending.take() {
                    self.cgram_addr = v;
                    self.cgram_second = false;
                    self.cgram_read_second = false;
                    if crate::debug_flags::ppu_write() {
                        println!("CGADD applied: 0x{:02X}", v);
                    }
                    // Start a small MDMA/CPU gap after CGADD effect
                    self.cgram_data_gap_ticks = crate::debug_flags::cgram_gap_after_cgadd();
                }
            }
        }
    }
}
