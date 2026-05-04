use crate::savestate::BusSaveState;

use super::Bus;

impl Bus {
    // --- Save-state helpers (WRAM/SRAM and simple IO) ---
    pub fn snapshot_memory(&self) -> (Vec<u8>, Vec<u8>) {
        (self.wram.clone(), self.sram.clone())
    }

    pub fn restore_memory(&mut self, wram: &[u8], sram: &[u8]) {
        if self.wram.len() == wram.len() {
            self.wram.copy_from_slice(wram);
        }
        if self.sram.len() == sram.len() {
            self.sram.copy_from_slice(sram);
            self.sram_dirty = false;
        }
    }

    // --- SRAM access/persistence helpers ---
    pub fn sram(&self) -> &[u8] {
        &self.sram
    }
    pub fn sram_mut(&mut self) -> &mut [u8] {
        &mut self.sram
    }
    pub fn sram_size(&self) -> usize {
        self.sram_size
    }
    pub fn is_sram_dirty(&self) -> bool {
        self.sram_dirty
    }
    pub fn clear_sram_dirty(&mut self) {
        self.sram_dirty = false;
    }

    pub fn get_input_system(&self) -> &crate::input::InputSystem {
        &self.input_system
    }

    #[allow(dead_code)]
    pub(crate) fn read_expansion(&mut self, _addr: u32) -> u8 {
        // Unmapped expansion/coprocessor windows read as open bus unless a mapper hooks them.
        self.mdr
    }

    #[allow(dead_code)]
    pub(crate) fn write_expansion(&mut self, _addr: u32, _value: u8) {
        // Unmapped expansion/coprocessor windows ignore writes.
    }

    pub fn get_ppu(&self) -> &crate::ppu::Ppu {
        &self.ppu
    }

    pub fn get_ppu_mut(&mut self) -> &mut crate::ppu::Ppu {
        &mut self.ppu
    }

    /// 現在のNMITIMEN値（$4200）を取得（デバッグ/フォールバック用）
    #[inline]
    #[allow(dead_code)]
    pub fn nmitimen(&self) -> u8 {
        self.nmitimen
    }

    pub fn to_save_state(&self) -> BusSaveState {
        let sa1_state = if self.is_sa1_active() {
            let cpu_state = self.sa1.cpu.get_state();
            Some(crate::savestate::Sa1SaveState {
                cpu_state: crate::savestate::CpuSaveState {
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
                },
                registers: self.sa1.registers.clone(),
                boot_vector_applied: self.sa1.boot_vector_applied,
                boot_pb: self.sa1.boot_pb,
                pending_reset: self.sa1.pending_reset,
                hold_reset: self.sa1.hold_reset,
                ipl_ran: self.sa1.ipl_ran,
                h_timer_accum: self.sa1.h_timer_accum,
                v_timer_accum: self.sa1.v_timer_accum,
                math_cycles_left: self.sa1.math_cycles_left,
                math_pending_result: self.sa1.math_pending_result,
                math_pending_overflow: self.sa1.math_pending_overflow,
                bwram: self.sa1_bwram.clone(),
                iram: self.sa1_iram.to_vec(),
                cycle_deficit: self.sa1_cycle_deficit,
                cycles_accum_frame: self.sa1_cycles_accum_frame,
                nmi_delay_active: self.sa1_nmi_delay_active,
            })
        } else {
            None
        };
        let spc7110_state = self.spc7110.as_ref().map(|spc| spc.save_data());
        let superfx_state = self.superfx.as_ref().map(|gsu| gsu.save_data());
        BusSaveState {
            nmitimen: self.nmitimen,
            wram_address: self.wram_address,
            mdr: self.mdr,
            mul_a: self.mul_a,
            mul_b: self.mul_b,
            mul_result: self.mul_result,
            div_a: self.div_a,
            div_b: self.div_b,
            div_quot: self.div_quot,
            div_rem: self.div_rem,
            mul_busy: self.mul_busy,
            mul_just_started: self.mul_just_started,
            mul_cycles_left: self.mul_cycles_left,
            mul_work_a: self.mul_work_a,
            mul_work_b: self.mul_work_b,
            mul_partial: self.mul_partial,
            div_busy: self.div_busy,
            div_just_started: self.div_just_started,
            div_cycles_left: self.div_cycles_left,
            div_work_dividend: self.div_work_dividend,
            div_work_divisor: self.div_work_divisor,
            div_work_quot: self.div_work_quot,
            div_work_rem: self.div_work_rem,
            div_work_bit: self.div_work_bit,
            cpu_instr_active: self.cpu_instr_active,
            cpu_instr_bus_cycles: self.cpu_instr_bus_cycles,
            cpu_instr_extra_master_cycles: self.cpu_instr_extra_master_cycles,
            irq_h_enabled: self.irq_h_enabled,
            irq_v_enabled: self.irq_v_enabled,
            irq_pending: self.irq_pending,
            irq_v_matched_line: self.irq_v_matched_line,
            h_timer: self.h_timer,
            v_timer: self.v_timer,
            h_timer_set: self.h_timer_set,
            v_timer_set: self.v_timer_set,
            joy_busy_counter: self.joy_busy_counter,
            joy_data: self.joy_data,
            joy_busy_scanlines: self.joy_busy_scanlines,
            pending_gdma_mask: self.pending_gdma_mask,
            pending_mdma_mask: self.pending_mdma_mask,
            mdma_started_after_opcode_fetch: self.mdma_started_after_opcode_fetch,
            rdnmi_consumed: self.rdnmi_consumed,
            rdnmi_high_byte_for_test: self.rdnmi_high_byte_for_test,
            pending_stall_master_cycles: self.pending_stall_master_cycles,
            smw_apu_hle: self.smw_apu_hle,
            smw_apu_hle_done: self.smw_apu_hle_done,
            smw_apu_hle_buf: self.smw_apu_hle_buf.clone(),
            smw_apu_hle_echo_idx: self.smw_apu_hle_echo_idx,
            wio: self.wio,
            fastrom: self.fastrom,
            dma_state: self.dma_controller.to_save_state(),
            spc7110_state,
            superfx_state,
            sa1_state,
        }
    }

    pub fn load_from_save_state(&mut self, st: &BusSaveState) {
        self.nmitimen = st.nmitimen;
        self.wram_address = st.wram_address;
        self.mdr = st.mdr;
        self.mul_a = st.mul_a;
        self.mul_b = st.mul_b;
        self.mul_result = st.mul_result;
        self.div_a = st.div_a;
        self.div_b = st.div_b;
        self.div_quot = st.div_quot;
        self.div_rem = st.div_rem;
        self.mul_busy = st.mul_busy;
        self.mul_just_started = st.mul_just_started;
        self.mul_cycles_left = st.mul_cycles_left;
        self.mul_work_a = st.mul_work_a;
        self.mul_work_b = st.mul_work_b;
        self.mul_partial = st.mul_partial;
        self.div_busy = st.div_busy;
        self.div_just_started = st.div_just_started;
        self.div_cycles_left = st.div_cycles_left;
        self.div_work_dividend = st.div_work_dividend;
        self.div_work_divisor = st.div_work_divisor;
        self.div_work_quot = st.div_work_quot;
        self.div_work_rem = st.div_work_rem;
        self.div_work_bit = st.div_work_bit;
        self.cpu_instr_active = st.cpu_instr_active;
        self.cpu_instr_bus_cycles = st.cpu_instr_bus_cycles;
        self.cpu_instr_extra_master_cycles = st.cpu_instr_extra_master_cycles;
        self.irq_h_enabled = st.irq_h_enabled;
        self.irq_v_enabled = st.irq_v_enabled;
        self.irq_pending = st.irq_pending;
        self.irq_v_matched_line = st.irq_v_matched_line;
        self.h_timer = st.h_timer;
        self.v_timer = st.v_timer;
        self.h_timer_set = st.h_timer_set;
        self.v_timer_set = st.v_timer_set;
        self.joy_busy_counter = st.joy_busy_counter;
        self.joy_data = st.joy_data;
        // Normalize auto-joy busy duration on load.
        // Old save states may carry legacy values (e.g. 8) that make input feel sluggish.
        self.joy_busy_scanlines = std::env::var("JOYBUSY_SCANLINES")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(3);
        self.joy_busy_counter = self.joy_busy_counter.min(self.joy_busy_scanlines);
        self.pending_gdma_mask = st.pending_gdma_mask;
        self.pending_mdma_mask = st.pending_mdma_mask;
        self.mdma_started_after_opcode_fetch = st.mdma_started_after_opcode_fetch;
        self.superfx_status_poll_pc = 0;
        self.superfx_status_poll_streak = 0;
        self.starfox_exact_wait_assist_frame = u64::MAX;
        self.rdnmi_consumed = st.rdnmi_consumed;
        self.rdnmi_high_byte_for_test = st.rdnmi_high_byte_for_test;
        self.pending_stall_master_cycles = st.pending_stall_master_cycles;
        self.smw_apu_hle = st.smw_apu_hle;
        self.smw_apu_hle_done = st.smw_apu_hle_done;
        self.smw_apu_hle_buf = st.smw_apu_hle_buf.clone();
        self.smw_apu_hle_echo_idx = st.smw_apu_hle_echo_idx;
        self.wio = st.wio;
        self.fastrom = st.fastrom;
        self.dma_controller.load_from_save_state(&st.dma_state);
        if let (Some(spc), Some(state)) = (self.spc7110.as_mut(), st.spc7110_state.as_ref()) {
            spc.load_data(state);
        }
        if let (Some(gsu), Some(state)) = (self.superfx.as_mut(), st.superfx_state.as_ref()) {
            gsu.load_data(state);
        }
        if self.is_sa1_active() {
            if let Some(sa1_state) = &st.sa1_state {
                self.sa1.cpu.set_state(crate::cpu::CpuState {
                    a: sa1_state.cpu_state.a,
                    x: sa1_state.cpu_state.x,
                    y: sa1_state.cpu_state.y,
                    sp: sa1_state.cpu_state.sp,
                    dp: sa1_state.cpu_state.dp,
                    db: sa1_state.cpu_state.db,
                    pb: sa1_state.cpu_state.pb,
                    pc: sa1_state.cpu_state.pc,
                    p: sa1_state.cpu_state.p,
                    emulation_mode: sa1_state.cpu_state.emulation_mode,
                    cycles: sa1_state.cpu_state.cycles,
                    waiting_for_irq: sa1_state.cpu_state.waiting_for_irq,
                    stopped: sa1_state.cpu_state.stopped,
                    deferred_fetch: sa1_state.cpu_state.deferred_fetch.map(|fetch| {
                        crate::cpu::core::DeferredFetchState {
                            opcode: fetch.opcode,
                            memspeed_penalty: fetch.memspeed_penalty,
                            pc_before: fetch.pc_before,
                            full_addr: fetch.full_addr,
                        }
                    }),
                });
                self.sa1.registers = sa1_state.registers.clone();
                self.sa1.boot_vector_applied = sa1_state.boot_vector_applied;
                self.sa1.boot_pb = sa1_state.boot_pb;
                self.sa1.pending_reset = sa1_state.pending_reset;
                self.sa1.hold_reset = sa1_state.hold_reset;
                self.sa1.ipl_ran = sa1_state.ipl_ran;
                self.sa1.h_timer_accum = sa1_state.h_timer_accum;
                self.sa1.v_timer_accum = sa1_state.v_timer_accum;
                self.sa1.math_cycles_left = sa1_state.math_cycles_left;
                self.sa1.math_pending_result = sa1_state.math_pending_result;
                self.sa1.math_pending_overflow = sa1_state.math_pending_overflow;
                self.sa1_bwram = sa1_state.bwram.clone();
                self.sa1_iram.fill(0);
                let copy_len = self.sa1_iram.len().min(sa1_state.iram.len());
                self.sa1_iram[..copy_len].copy_from_slice(&sa1_state.iram[..copy_len]);
                self.sa1_cycle_deficit = sa1_state.cycle_deficit;
                self.sa1_cycles_accum_frame = sa1_state.cycles_accum_frame;
                self.sa1_nmi_delay_active = sa1_state.nmi_delay_active;
            } else {
                self.sa1.math_cycles_left = 0;
                self.sa1.math_pending_result = 0;
                self.sa1.math_pending_overflow = false;
                self.sa1_cycle_deficit = 0;
                self.sa1_cycles_accum_frame = 0;
                self.sa1_nmi_delay_active = false;
            }
        } else {
            self.sa1.math_cycles_left = 0;
            self.sa1.math_pending_result = 0;
            self.sa1.math_pending_overflow = false;
            self.sa1_cycles_accum_frame = 0;
        }
    }
}
