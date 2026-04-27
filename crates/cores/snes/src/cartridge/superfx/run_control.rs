use super::*;

impl SuperFx {
    pub(super) fn apply_sfr_side_effects(&mut self, rom: &[u8]) {
        if (self.sfr & SFR_GO_BIT) == 0 {
            self.running = false;
            self.cbr = 0;
            self.cache_enabled = false;
            self.cache_valid_mask = 0;
        } else {
            self.start_execution(rom);
        }
    }

    pub(super) fn invoke_cpu_start(&mut self, rom: &[u8]) {
        self.flush_all_pixel_caches();

        if trace_superfx_start_enabled() {
            use std::sync::atomic::{AtomicU32, Ordering};
            static CNT: OnceLock<AtomicU32> = OnceLock::new();
            let n = CNT
                .get_or_init(|| AtomicU32::new(0))
                .fetch_add(1, Ordering::Relaxed);
            if n < 64 {
                println!(
                    "[SFX-START] R15={:04X} PBR={:02X} R14={:04X} ROMBR={:02X} RAMBR={:02X} SFR={:04X} SCMR={:02X} gram60={:02X}{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}",
                    self.regs[15],
                    self.pbr,
                    self.regs[14],
                    self.rombr,
                    self.rambr,
                    self.sfr,
                    self.scmr,
                    self.game_ram.get(0x60).copied().unwrap_or(0),
                    self.game_ram.get(0x61).copied().unwrap_or(0),
                    self.game_ram.get(0x62).copied().unwrap_or(0),
                    self.game_ram.get(0x63).copied().unwrap_or(0),
                    self.game_ram.get(0x64).copied().unwrap_or(0),
                    self.game_ram.get(0x65).copied().unwrap_or(0),
                    self.game_ram.get(0x66).copied().unwrap_or(0),
                    self.game_ram.get(0x67).copied().unwrap_or(0),
                );
            }
        }
        self.sfr |= SFR_GO_BIT;
        self.total_run_instructions = 0;
        self.pending_delay_pc = None;
        self.pending_delay_pbr = None;
        self.pending_delay_cache_base = None;
        if self.prepare_start_execution(rom) {
            if let Some(steps) = Self::cpu_start_immediate_steps() {
                self.run_steps(rom, steps);
            }
        }
    }

    pub(super) fn finish_noop_run(&mut self) {
        self.sync_ram_buffer();
        self.running = false;
        self.sfr &= !(SFR_GO_BIT | SFR_R_BIT);
        self.pending_delay_pc = None;
        self.pending_delay_pbr = None;
        self.pending_delay_cache_base = None;
        self.r14_modified = false;
        self.r15_modified = false;
        self.pipe = default_superfx_pipe();
        self.pipe_valid = false;
        if (self.cfgr & 0x80) == 0 {
            self.sfr |= SFR_IRQ_BIT;
        }
    }

    fn experimental_core_enabled() -> bool {
        std::env::var("EXPERIMENTAL_SUPERFX_CORE")
            .map(|value| value != "0" && !value.eq_ignore_ascii_case("false"))
            .unwrap_or(true)
    }

    pub(super) fn steps_per_cpu_cycle(&self) -> usize {
        // CLSR bit 0: 0 = 10.738 MHz (standard), 1 = 21.477 MHz (turbo/SuperFX2)
        let default = if (self.clsr & 0x01) != 0 {
            DEFAULT_SUPERFX_RATIO_FAST
        } else {
            DEFAULT_SUPERFX_RATIO_SLOW
        };
        let override_ratio = || {
            std::env::var("SUPERFX_CPU_RATIO")
                .ok()
                .and_then(|value| value.parse::<usize>().ok())
                .filter(|value| *value > 0)
        };
        if cfg!(test) {
            override_ratio().unwrap_or(default)
        } else {
            static VALUE: OnceLock<Option<usize>> = OnceLock::new();
            (*VALUE.get_or_init(override_ratio)).unwrap_or(default)
        }
    }

    fn startup_step_budget() -> usize {
        let override_budget = || {
            std::env::var("SUPERFX_STEP_BUDGET")
                .ok()
                .and_then(|value| value.parse::<usize>().ok())
                .filter(|value| *value > 0)
        };
        if cfg!(test) {
            override_budget().unwrap_or(DEFAULT_STARTUP_STEP_BUDGET)
        } else {
            static VALUE: OnceLock<Option<usize>> = OnceLock::new();
            (*VALUE.get_or_init(override_budget)).unwrap_or(DEFAULT_STARTUP_STEP_BUDGET)
        }
    }

    pub fn status_poll_step_budget() -> usize {
        let override_budget = || {
            std::env::var("SUPERFX_STATUS_POLL_BOOST")
                .ok()
                .and_then(|value| value.parse::<usize>().ok())
                .filter(|value| *value > 0)
        };
        if cfg!(test) {
            override_budget().unwrap_or(DEFAULT_SUPERFX_STATUS_POLL_STEP_BUDGET)
        } else {
            static VALUE: OnceLock<Option<usize>> = OnceLock::new();
            (*VALUE.get_or_init(override_budget)).unwrap_or(DEFAULT_SUPERFX_STATUS_POLL_STEP_BUDGET)
        }
    }

    fn status_poll_sfr_change_chunk() -> usize {
        let override_chunk = || {
            std::env::var("SUPERFX_STATUS_POLL_SFR_CHANGE_CHUNK")
                .ok()
                .and_then(|value| value.parse::<usize>().ok())
                .filter(|value| *value > 0)
        };
        let default = || Self::status_poll_step_budget().clamp(1, 8_192);
        if cfg!(test) {
            override_chunk().unwrap_or_else(default)
        } else {
            static VALUE: OnceLock<Option<usize>> = OnceLock::new();
            (*VALUE.get_or_init(override_chunk)).unwrap_or_else(default)
        }
    }

    pub(super) fn starfox_producer_poll_chunk() -> usize {
        let override_chunk = || {
            std::env::var("SUPERFX_STATUS_POLL_STARFOX_PRODUCER_CHUNK")
                .ok()
                .and_then(|value| value.parse::<usize>().ok())
                .filter(|value| *value > 0)
        };
        if cfg!(test) {
            override_chunk().unwrap_or(262_144)
        } else {
            static VALUE: OnceLock<Option<usize>> = OnceLock::new();
            (*VALUE.get_or_init(override_chunk)).unwrap_or(262_144)
        }
    }

    fn cpu_start_immediate_steps() -> Option<usize> {
        let override_steps = || {
            std::env::var("SUPERFX_CPU_START_IMMEDIATE_STEPS")
                .ok()
                .and_then(|value| value.parse::<usize>().ok())
                .filter(|value| *value > 0)
        };
        if cfg!(test) {
            override_steps()
        } else {
            static VALUE: OnceLock<Option<usize>> = OnceLock::new();
            *VALUE.get_or_init(override_steps)
        }
    }

    pub fn run_status_poll_catchup(&mut self, rom: &[u8]) {
        if !self.running {
            return;
        }
        self.run_steps(rom, Self::status_poll_step_budget());
    }

    pub fn run_status_poll_catchup_steps(&mut self, rom: &[u8], steps: usize) {
        if !self.running || steps == 0 {
            return;
        }
        self.run_steps(rom, steps);
    }

    pub fn run_status_poll_until_stop(&mut self, rom: &[u8], max_steps: usize) {
        if !self.running || max_steps == 0 {
            return;
        }
        let chunk = Self::status_poll_step_budget().saturating_mul(16).max(1);
        let mut remaining = max_steps;
        while self.running && remaining > 0 {
            let steps = remaining.min(chunk);
            self.run_steps(rom, steps);
            remaining -= steps;
        }
    }

    pub fn run_status_poll_until_sfr_low_mask_changes(
        &mut self,
        rom: &[u8],
        initial_low: u8,
        mask: u8,
        max_steps: usize,
    ) {
        if !self.running || max_steps == 0 {
            return;
        }

        // Poll loops only care that the observed status bit changes before the
        // CPU re-reads $3030. Small chunks keep the stop condition tight
        // without turning a single busy-wait into millions of 1-step calls.
        let status_change_chunk = Self::status_poll_sfr_change_chunk();
        let mut remaining = max_steps;
        let initial_masked = initial_low & mask;
        while self.running && remaining > 0 {
            let current_low = self.observed_sfr_low();
            let changed = if mask == 0 {
                current_low != initial_low
            } else {
                (current_low & mask) != initial_masked
            };
            if changed {
                break;
            }
            if self.fast_forward_starfox_cached_delay_loop() {
                continue;
            }
            let steps = remaining.min(status_change_chunk);
            self.run_steps(rom, steps);
            remaining -= steps;
        }
    }

    pub fn run_status_poll_until_sfr_low_changes(
        &mut self,
        rom: &[u8],
        initial_low: u8,
        max_steps: usize,
    ) {
        self.run_status_poll_until_sfr_low_mask_changes(rom, initial_low, u8::MAX, max_steps);
    }

    pub(super) fn prepare_start_execution(&mut self, rom: &[u8]) -> bool {
        if !Self::experimental_core_enabled() || rom.is_empty() {
            self.finish_noop_run();
            return false;
        }
        if self.regs[15] < CACHE_RAM_SIZE as u16 && self.cache_ram.iter().any(|&byte| byte != 0) {
            self.cache_enabled = true;
            self.cache_valid_mask = u32::MAX;
        }
        self.running = true;
        self.pipe = default_superfx_pipe();
        self.pipe_valid = false;
        self.r14_modified = false;
        self.r15_modified = false;
        true
    }

    pub(super) fn start_execution(&mut self, rom: &[u8]) {
        if !self.prepare_start_execution(rom) {
            return;
        }
        self.run_steps(rom, Self::startup_step_budget());
    }

    pub fn run_for_cpu_cycles(&mut self, rom: &[u8], cpu_cycles: u8) {
        if !self.running || cpu_cycles == 0 {
            return;
        }
        let steps = (cpu_cycles as usize).saturating_mul(self.steps_per_cpu_cycle());
        self.run_steps(rom, steps);
    }
}
