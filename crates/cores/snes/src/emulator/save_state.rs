use super::{Emulator, EmulatorRuntimeConfig, PerformanceStats};
use crate::savestate::*;
use std::path::PathBuf;

impl Emulator {
    pub(super) fn try_auto_load_state(&mut self) -> bool {
        if let Ok(path) = std::env::var("LOAD_STATE_PATH") {
            return self.try_auto_load_state_path(&path);
        }

        if let Ok(slot_str) = std::env::var("LOAD_STATE_SLOT") {
            return self.try_auto_load_state_slot(&slot_str);
        }

        false
    }

    pub(super) fn try_auto_load_state_path(&mut self, path: &str) -> bool {
        if path.trim().is_empty() {
            return false;
        }

        match SaveState::load_from_file(path) {
            Ok(save_state) => {
                let checksum_valid = save_state.validate_rom_checksum(self.rom_checksum);
                if checksum_valid {
                    self.load_save_state(save_state);
                    self.debug_apply_ppu_overrides_from_env();
                    true
                } else if EmulatorRuntimeConfig::read_strict_bool_env("FORCE_LOAD_STATE", false) {
                    println!("LOAD_STATE_PATH: checksum mismatch ignored (FORCE_LOAD_STATE=1)");
                    self.load_save_state(save_state);
                    self.debug_apply_ppu_overrides_from_env();
                    true
                } else {
                    eprintln!("LOAD_STATE_PATH: checksum mismatch, load aborted");
                    false
                }
            }
            Err(e) => {
                eprintln!("LOAD_STATE_PATH: failed to load: {}", e);
                false
            }
        }
    }

    pub(super) fn try_auto_load_state_slot(&mut self, slot_str: &str) -> bool {
        if let Ok(slot) = slot_str.trim().parse::<u8>() {
            match self.load_from_slot(slot) {
                Ok(_) => {
                    println!("Auto-loaded save state slot {}", slot);
                    true
                }
                Err(e) => {
                    eprintln!("LOAD_STATE_SLOT: failed to load slot {}: {}", slot, e);
                    false
                }
            }
        } else {
            if !slot_str.trim().is_empty() {
                eprintln!("LOAD_STATE_SLOT: invalid value '{}'", slot_str);
            }
            false
        }
    }

    pub(super) fn save_sram_if_dirty(&mut self) {
        let no_sram_save = std::env::var("NO_SRAM_SAVE")
            .map(|v| v == "1" || v.to_lowercase() == "true")
            .unwrap_or(false);
        if no_sram_save {
            return;
        }
        if let Some(ref path) = self.srm_path {
            if self.bus.is_sram_dirty() {
                if let Err(e) = std::fs::write(path, self.bus.sram()) {
                    eprintln!("Failed to save SRAM to {}: {}", path.display(), e);
                } else {
                    println!(
                        "SRAM saved to {} ({} bytes)",
                        path.display(),
                        self.bus.sram().len()
                    );
                    self.bus.clear_sram_dirty();
                }
            }
        }
    }

    pub(super) fn maybe_autosave_sram(&mut self) {
        if let (Some(every), Some(ref path)) = (self.srm_autosave_every, self.srm_path.as_ref()) {
            if self
                .frame_count
                .saturating_sub(self.srm_last_autosave_frame)
                >= every
                && self.bus.is_sram_dirty()
            {
                let tmp = {
                    let mut p = path.to_path_buf();
                    p.set_extension("srm.tmp");
                    p
                };
                let write_ok =
                    std::fs::write(&tmp, self.bus.sram()).and_then(|_| std::fs::rename(&tmp, path));
                match write_ok {
                    Ok(_) => {
                        println!(
                            "SRAM autosaved to {} (every {} frames)",
                            path.display(),
                            every
                        );
                        self.srm_last_autosave_frame = self.frame_count;
                        // Keep dirty true; we will still flush on exit.
                    }
                    Err(e) => eprintln!("SRAM autosave failed ({}): {}", path.display(), e),
                }
            }
        }
    }

    pub(super) fn states_dir(&self) -> PathBuf {
        let rom_stem = self
            .srm_path
            .as_ref()
            .and_then(|p| p.file_stem())
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "unknown".to_string());
        PathBuf::from("states").join(rom_stem)
    }

    pub fn quick_save(&mut self) -> Result<(), String> {
        let dir = self.states_dir();
        std::fs::create_dir_all(&dir).map_err(|e| format!("Failed to create states dir: {}", e))?;
        let path = dir.join("quick.sav");
        let save_state = self.create_save_state();
        save_state.save_to_file(&path.to_string_lossy())
    }

    pub fn quick_load(&mut self) -> Result<(), String> {
        let path = self.states_dir().join("quick.sav");
        let save_state = SaveState::load_from_file(&path.to_string_lossy())?;

        if !save_state.validate_rom_checksum(self.rom_checksum) {
            return Err("Save state is from a different ROM".to_string());
        }

        self.load_save_state(save_state);
        Ok(())
    }

    #[allow(dead_code)]
    pub fn save_to_slot(&mut self, slot: u8) -> Result<(), String> {
        let dir = self.states_dir();
        std::fs::create_dir_all(&dir).map_err(|e| format!("Failed to create states dir: {}", e))?;
        let path = dir.join(format!("slot{}.sav", slot));
        let save_state = self.create_save_state();
        save_state.save_to_file(&path.to_string_lossy())
    }

    #[allow(dead_code)]
    pub fn load_from_slot(&mut self, slot: u8) -> Result<(), String> {
        let path = self.states_dir().join(format!("slot{}.sav", slot));
        let save_state = SaveState::load_from_file(&path.to_string_lossy())?;

        if !save_state.validate_rom_checksum(self.rom_checksum) {
            return Err("Save state is from a different ROM".to_string());
        }

        self.load_save_state(save_state);
        Ok(())
    }

    #[allow(dead_code)]
    /// Flush SRAM to disk (if path is set and SRAM is dirty).
    pub fn flush_sram(&mut self) {
        if let Some(ref path) = self.srm_path {
            if self.bus.sram_dirty {
                let sram = self.bus.sram();
                if let Err(e) = std::fs::write(path, sram) {
                    eprintln!("SRAM flush failed: {}", e);
                } else {
                    self.bus.sram_dirty = false;
                }
            }
        }
    }

    #[allow(dead_code)]
    /// Save full emulator state to an arbitrary file path.
    pub fn save_state_to_file(&mut self, path: &std::path::Path) -> Result<(), String> {
        let save_state = self.create_save_state();
        save_state.save_to_file(&path.to_string_lossy())
    }

    #[allow(dead_code)]
    /// Load full emulator state from an arbitrary file path.
    pub fn load_state_from_file(&mut self, path: &std::path::Path) -> Result<(), String> {
        let save_state = SaveState::load_from_file(&path.to_string_lossy())?;
        if !save_state.validate_rom_checksum(self.rom_checksum) {
            return Err("Save state is from a different ROM".to_string());
        }
        self.load_save_state(save_state);
        Ok(())
    }

    pub(super) fn create_save_state(&self) -> SaveState {
        let mut save_state = SaveState::new();

        // CPU state
        let cpu_state = self.cpu.get_state();
        save_state.cpu_state = CpuSaveState {
            a: cpu_state.a,
            x: cpu_state.x,
            y: cpu_state.y,
            sp: cpu_state.sp,
            dp: cpu_state.dp,
            db: cpu_state.db,
            pb: cpu_state.pb,
            pc: cpu_state.pc,
            p: cpu_state.p,
            emulation_mode: cpu_state.emulation_mode,
            cycles: cpu_state.cycles,
            waiting_for_irq: cpu_state.waiting_for_irq,
            stopped: cpu_state.stopped,
            deferred_fetch: cpu_state.deferred_fetch.map(|fetch| {
                crate::savestate::CpuDeferredFetchSaveState {
                    opcode: fetch.opcode,
                    memspeed_penalty: fetch.memspeed_penalty,
                    pc_before: fetch.pc_before,
                    full_addr: fetch.full_addr,
                }
            }),
        };

        // Set metadata
        save_state.master_cycles = self.master_cycles;
        save_state.frame_count = self.frame_count;
        save_state.rom_checksum = self.rom_checksum;

        // PPU/APU/Memory/Input
        save_state.ppu_state = self.bus.get_ppu().to_save_state();
        if let Ok(mut apu) = self.bus.get_apu_shared().lock() {
            save_state.apu_state = apu.to_save_state();
        }
        let (wram, sram) = self.bus.snapshot_memory();
        save_state.memory_state = crate::savestate::MemoryState { wram, sram };
        save_state.input_state = self.bus.get_input_system().to_save_state();
        save_state.bus_state = self.bus.to_save_state();
        save_state.emulator_state = EmulatorSaveState {
            pending_stall_master_cycles: self.pending_stall_master_cycles,
            ppu_cycle_accum: self.ppu_cycle_accum,
            // APU-internal pending cycles are stored inside ApuSaveState.
            apu_cycle_debt: self.apu_cycle_debt,
            apu_master_cycle_accum: self.apu_master_cycle_accum,
            superfx_master_cycle_accum: self.superfx_master_cycle_accum,
            apu_step_batch: self.apu_step_batch,
            apu_step_force: self.apu_step_force,
        };

        save_state
    }

    pub(super) fn load_save_state(&mut self, save_state: SaveState) {
        let save_version = save_state.version;
        // Restore CPU state
        let cpu_state = crate::cpu::CpuState {
            a: save_state.cpu_state.a,
            x: save_state.cpu_state.x,
            y: save_state.cpu_state.y,
            sp: save_state.cpu_state.sp,
            dp: save_state.cpu_state.dp,
            db: save_state.cpu_state.db,
            pb: save_state.cpu_state.pb,
            pc: save_state.cpu_state.pc,
            p: save_state.cpu_state.p,
            emulation_mode: save_state.cpu_state.emulation_mode,
            cycles: save_state.cpu_state.cycles,
            waiting_for_irq: save_state.cpu_state.waiting_for_irq,
            stopped: save_state.cpu_state.stopped,
            deferred_fetch: save_state.cpu_state.deferred_fetch.map(|fetch| {
                crate::cpu::core::DeferredFetchState {
                    opcode: fetch.opcode,
                    memspeed_penalty: fetch.memspeed_penalty,
                    pc_before: fetch.pc_before,
                    full_addr: fetch.full_addr,
                }
            }),
        };

        self.cpu.set_state(cpu_state);
        self.master_cycles = save_state.master_cycles;
        self.frame_count = save_state.frame_count;
        self.save_state_capture_stop_requested = false;
        crate::cartridge::superfx::set_trace_superfx_exec_frame(self.frame_count);

        // Restore PPU/APU/Memory/Input
        {
            let ppu = self.bus.get_ppu_mut();
            ppu.load_from_save_state(&save_state.ppu_state);
        }
        if let Ok(mut apu) = self.bus.get_apu_shared().lock() {
            apu.load_from_save_state(&save_state.apu_state);
        }
        self.bus
            .restore_memory(&save_state.memory_state.wram, &save_state.memory_state.sram);
        self.bus
            .get_input_system_mut()
            .load_from_save_state(&save_state.input_state);
        if save_version >= 2 {
            self.bus.load_from_save_state(&save_state.bus_state);
            self.pending_stall_master_cycles =
                save_state.emulator_state.pending_stall_master_cycles;
            self.ppu_cycle_accum = save_state.emulator_state.ppu_cycle_accum;
            self.apu_cycle_debt = save_state.emulator_state.apu_cycle_debt;
            self.apu_master_cycle_accum = save_state.emulator_state.apu_master_cycle_accum;
            self.superfx_master_cycle_accum = save_state.emulator_state.superfx_master_cycle_accum;
            self.apu_step_batch = save_state.emulator_state.apu_step_batch;
            self.apu_step_force = save_state.emulator_state.apu_step_force;
        }
        self.sa1_cycle_debt = 0;
        self.sync_superfx_direct_buffer();
        // Reset runtime performance counters so post-load stats are not mixed.
        self.performance_stats = PerformanceStats::new();
    }

    #[allow(dead_code)]
    pub fn get_save_info(&self, filename: &str) -> Result<SaveInfo, String> {
        let save_state = SaveState::load_from_file(filename)?;
        Ok(save_state.get_save_info())
    }
}
