use super::{Bus, CPU_EXEC_TRACE_RING_LEN};
use crate::bus::debug::{
    auto_press_a_frame, auto_press_a_stop_frame, auto_press_start_frame,
    trace_starfox_slow_profile_enabled,
};
use std::time::Instant;

impl Bus {
    #[inline]
    pub(super) fn on_cpu_bus_cycle(&mut self) {
        if !self.cpu_instr_active || self.dma_in_progress {
            return;
        }
        let profile_enabled = trace_starfox_slow_profile_enabled();
        let profile_start = profile_enabled.then(Instant::now);
        self.cpu_instr_bus_cycles = self.cpu_instr_bus_cycles.saturating_add(1);
        let extra = self.cpu_access_extra_master_cycles(self.last_cpu_bus_addr);
        self.cpu_instr_extra_master_cycles =
            self.cpu_instr_extra_master_cycles.saturating_add(extra);
        self.tick_cpu_cycles(1);
        if let Some(start) = profile_start {
            self.cpu_profile_bus_cycle_ns = self
                .cpu_profile_bus_cycle_ns
                .saturating_add(start.elapsed().as_nanos() as u64);
            self.cpu_profile_bus_cycle_count = self.cpu_profile_bus_cycle_count.saturating_add(1);
        }
    }

    #[inline]
    pub(super) fn take_apu_inline_cpu_cycles_for_current_access(&mut self) -> u8 {
        if !self.cpu_instr_active || self.dma_in_progress {
            return 0;
        }
        let elapsed = self.cpu_instr_bus_cycles.saturating_add(1);
        let delta = elapsed.saturating_sub(self.cpu_instr_apu_synced_bus_cycles);
        if delta != 0 {
            self.cpu_instr_apu_synced_bus_cycles = elapsed;
        }
        delta
    }

    #[inline]
    pub(super) fn cpu_instr_elapsed_master_cycles(&self) -> u64 {
        (self.cpu_instr_bus_cycles as u64) * 6 + self.cpu_instr_extra_master_cycles
    }

    #[inline]
    fn is_wram_address(&self, addr: u32) -> bool {
        let bank = ((addr >> 16) & 0xFF) as u8;
        let off = (addr & 0xFFFF) as u16;
        // WRAM direct: $7E:0000-$7F:FFFF
        if (0x7E..=0x7F).contains(&bank) {
            return true;
        }
        // WRAM mirror: $00-$3F/$80-$BF:0000-1FFF
        ((0x00..=0x3F).contains(&bank) || (0x80..=0xBF).contains(&bank)) && off < 0x2000
    }

    #[inline]
    pub(super) fn cpu_access_master_cycles(&self, addr: u32) -> u8 {
        // Reference: https://snes.nesdev.org/wiki/Timing
        let bank = ((addr >> 16) & 0xFF) as u8;
        let off = (addr & 0xFFFF) as u16;

        // JOYSER0/1: always 12 master clocks
        if ((0x00..=0x3F).contains(&bank) || (0x80..=0xBF).contains(&bank))
            && matches!(off, 0x4016 | 0x4017)
        {
            return 12;
        }

        // Most MMIO: 6 master clocks
        if ((0x00..=0x3F).contains(&bank) || (0x80..=0xBF).contains(&bank))
            && matches!(
                off,
                0x2100..=0x21FF | 0x4000..=0x41FF | 0x4200..=0x421F | 0x4300..=0x437F
            )
        {
            return 6;
        }

        // Internal WRAM: 8 master clocks
        if self.is_wram_address(addr) {
            return 8;
        }

        // ROM: 6 master clocks for FastROM ($80:0000+ with MEMSEL=1), otherwise 8.
        if self.is_rom_address(addr) {
            let fast = self.fastrom && (addr & 0x80_0000) != 0;
            return if fast { 6 } else { 8 };
        }

        // Default to 8 (safe/slow) for SRAM/unknown regions.
        8
    }

    #[inline]
    fn cpu_access_extra_master_cycles(&self, addr: u32) -> u64 {
        let mc = self.cpu_access_master_cycles(addr);
        mc.saturating_sub(6) as u64
    }

    #[inline]
    pub(super) fn add16_in_bank(addr: u32, delta: u32) -> u32 {
        let bank = addr & 0x00FF_0000;
        let lo = (addr & 0x0000_FFFF).wrapping_add(delta) & 0x0000_FFFF; // allow wrapping within 16-bit
        bank | lo
    }

    #[inline]
    fn current_hirq_dot(&self, scanline: u16) -> Option<u16> {
        let last_dot = self.ppu.dots_this_scanline(scanline).saturating_sub(1);
        if self.h_timer > last_dot {
            return None;
        }
        let h = self.h_timer.saturating_add(4);
        (h <= last_dot).then_some(h)
    }

    #[inline]
    fn htimer_condition_matches_current_dot(&self, scanline: u16, cycle: u16) -> bool {
        self.current_hirq_dot(scanline)
            .map(|h| cycle == h)
            .unwrap_or(false)
    }

    pub(crate) fn recheck_irq_timer_match(&mut self) {
        if !(self.irq_h_enabled || self.irq_v_enabled) {
            return;
        }

        let line = self.ppu.get_scanline();
        let cycle = self.ppu.get_cycle();
        let v_match = line == self.v_timer;

        match (self.irq_h_enabled, self.irq_v_enabled) {
            (true, true) => {
                self.irq_v_matched_line = if v_match { Some(line) } else { None };
                if v_match && self.htimer_condition_matches_current_dot(line, cycle) {
                    self.irq_pending = true;
                }
            }
            (true, false) => {
                self.irq_v_matched_line = None;
                if self.htimer_condition_matches_current_dot(line, cycle) {
                    self.irq_pending = true;
                }
            }
            (false, true) => {
                self.irq_v_matched_line = None;
            }
            _ => {}
        }
    }

    pub fn tick_timers(&mut self) {
        if !(self.irq_h_enabled || self.irq_v_enabled) {
            return;
        }
        let line = self.ppu.get_scanline();
        let v_match = line == self.v_timer;
        if self.irq_v_enabled && !self.irq_h_enabled {
            if v_match {
                self.irq_pending = true;
            }
        } else if self.irq_h_enabled && self.irq_v_enabled {
            self.irq_v_matched_line = if v_match { Some(line) } else { None };
        }
    }

    pub fn tick_timers_hv(&mut self, old_cycle: u16, new_cycle: u16, scanline: u16) {
        if !(self.irq_h_enabled || self.irq_v_enabled) {
            return;
        }

        let mut h_match = false;
        if let Some(h) = self.current_hirq_dot(scanline) {
            if old_cycle <= new_cycle {
                if old_cycle <= h && h < new_cycle {
                    h_match = true;
                }
            } else if old_cycle <= h || h < new_cycle {
                h_match = true;
            }
        }

        match (self.irq_h_enabled, self.irq_v_enabled) {
            (true, true) => {
                if h_match {
                    if let Some(vline) = self.irq_v_matched_line {
                        if vline == scanline {
                            self.irq_pending = true;
                        }
                    }
                }
            }
            (true, false) => {
                if h_match {
                    self.irq_pending = true;
                }
            }
            (false, true) => {}
            _ => {}
        }
    }

    pub fn on_vblank_start(&mut self) {
        if (self.nmitimen & 0x01) != 0 {
            self.input_system.write_strobe(1);
            self.input_system.write_strobe(0);

            let mt = self.input_system.is_multitap_enabled();
            let mut b1: u16 = 0;
            let mut b2: u16 = 0;
            let mut b3: u16 = 0;
            let mut b4: u16 = 0;
            for i in 0..16 {
                let bit_pos = 15 - i;
                b1 |= ((self.input_system.read_controller1() & 1) as u16) << bit_pos;
                b2 |= ((self.input_system.read_controller2() & 1) as u16) << bit_pos;
                if mt {
                    b3 |= ((self.input_system.read_controller3() & 1) as u16) << bit_pos;
                    b4 |= ((self.input_system.read_controller4() & 1) as u16) << bit_pos;
                }
            }

            self.joy_data[0] = (b1 & 0x00FF) as u8;
            self.joy_data[1] = ((b1 >> 8) & 0x00FF) as u8;
            self.joy_data[2] = (b2 & 0x00FF) as u8;
            self.joy_data[3] = ((b2 >> 8) & 0x00FF) as u8;
            self.joy_data[4] = (b3 & 0x00FF) as u8;
            self.joy_data[5] = ((b3 >> 8) & 0x00FF) as u8;
            self.joy_data[6] = (b4 & 0x00FF) as u8;
            self.joy_data[7] = ((b4 >> 8) & 0x00FF) as u8;
            if self.cpu_test_mode && crate::debug_flags::headless() {
                self.joy_data[0] = 0x00;
                self.joy_data[1] = 0x00;
            }
            if crate::debug_flags::headless() {
                let cur = self.ppu.get_frame();
                if let Some(start_frame) = auto_press_a_frame() {
                    let stop = auto_press_a_stop_frame().unwrap_or(u32::MAX);
                    let sf = start_frame as u64;
                    if cur >= sf && cur < stop as u64 {
                        let elapsed = cur - sf;
                        if elapsed < 2 || (elapsed % 30) < 2 {
                            self.joy_data[0] |= 0x80;
                        }
                    }
                }
                if let Some(start_frame) = auto_press_start_frame() {
                    if cur >= start_frame as u64 && cur < (start_frame as u64) + 2 {
                        self.joy_data[1] |= 0x10;
                    }
                }
                if let Ok(hex) = std::env::var("AUTO_PRESS_BUTTONS") {
                    if let Ok(mask) = u16::from_str_radix(hex.trim_start_matches("0x"), 16) {
                        let start = auto_press_a_stop_frame().unwrap_or(0) as u64;
                        if cur >= start {
                            self.joy_data[0] |= (mask & 0xFF) as u8;
                            self.joy_data[1] |= ((mask >> 8) & 0xFF) as u8;
                        }
                    }
                }
            }
            let mut busy = self.joy_busy_scanlines;
            if self.cpu_test_mode && busy < 8 {
                busy = 8;
            }
            self.joy_busy_counter = if crate::debug_flags::cpu_test_hle_force() {
                0
            } else if crate::debug_flags::cpu_test_hle() {
                32
            } else {
                busy
            };
            if crate::debug_flags::trace_autojoy() {
                println!(
                    "[AUTOJOY] latched b1=0x{:04X} b2=0x{:04X} busy={} scanline={} cycle={}",
                    b1,
                    b2,
                    self.joy_busy_counter,
                    self.ppu.scanline,
                    self.ppu.get_cycle()
                );
            }
        }
        if self.pending_gdma_mask != 0 {
            let mask = self.pending_gdma_mask;
            self.pending_gdma_mask = 0;
            for i in 0..8 {
                if mask & (1 << i) != 0 {
                    if !self.dma_controller.channels[i].configured {
                        continue;
                    }
                    self.perform_dma_transfer(i);
                }
            }
        }
    }

    pub fn on_scanline_advance(&mut self) {
        if self.joy_busy_counter > 0 {
            self.joy_busy_counter -= 1;
        }
        if let Some((start, end)) = crate::debug_flags::trace_cpu_pc_range() {
            let frame = self.ppu.get_frame();
            let sl = self.ppu.scanline;
            if frame >= start && frame <= end && sl == 100 {
                eprintln!(
                    "[CPU-PC] frame={} sl={} PC=0x{:06X} NMI_en={} INIDISP=0x{:02X}",
                    frame, sl, self.last_cpu_pc, self.ppu.nmi_enabled, self.ppu.screen_display,
                );
            }
        }
    }

    #[inline]
    pub fn take_pending_stall_master_cycles(&mut self) -> u64 {
        let v = self.pending_stall_master_cycles;
        self.pending_stall_master_cycles = 0;
        v
    }

    #[inline]
    pub fn take_last_instr_extra_master(&mut self) -> u64 {
        let v = self.last_instr_extra_master;
        self.last_instr_extra_master = 0;
        v
    }

    #[inline]
    pub(crate) fn add_pending_stall_master_cycles(&mut self, cycles: u64) {
        self.pending_stall_master_cycles = self.pending_stall_master_cycles.saturating_add(cycles);
    }

    pub(crate) fn push_recent_cpu_exec_pc(&mut self, pc24: u32) {
        if self
            .recent_cpu_exec_pcs
            .last()
            .is_some_and(|&last| last == pc24)
        {
            return;
        }
        if self.recent_cpu_exec_pcs.len() >= CPU_EXEC_TRACE_RING_LEN {
            self.recent_cpu_exec_pcs.remove(0);
        }
        self.recent_cpu_exec_pcs.push(pc24);
    }

    pub fn debug_recent_cpu_exec_pcs(&self) -> &[u32] {
        &self.recent_cpu_exec_pcs
    }
}
