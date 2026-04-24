use super::*;

impl SuperFx {
    pub fn debug_in_starfox_live_producer_loop(&self) -> bool {
        self.running
            && self.pbr == 0x01
            && self.rambr == 0x00
            && matches!(self.regs[13], 0xB384..=0xB3E6)
    }

    pub fn run_status_poll_until_go_clears_in_starfox_live_producer_loop(
        &mut self,
        rom: &[u8],
        max_steps: usize,
    ) {
        if !self.debug_in_starfox_live_producer_loop() || max_steps == 0 {
            return;
        }

        let chunk = Self::starfox_producer_poll_chunk();
        let mut remaining = max_steps;
        while self.running && remaining > 0 {
            if (self.observed_sfr_low() & (SFR_GO_BIT as u8)) == 0 {
                break;
            }
            if self.fast_forward_starfox_cached_delay_loop() {
                continue;
            }
            if let Some(consumed) = self.fast_forward_starfox_live_producer_store(rom, remaining) {
                remaining = remaining.saturating_sub(consumed);
                continue;
            }
            let steps = remaining.min(chunk);
            if self.can_direct_run_starfox_late_wait() {
                self.run_steps_direct_no_pipe(rom, steps);
            } else {
                self.run_steps(rom, steps);
            }
            remaining -= steps;
        }
    }

    pub fn debug_in_starfox_cached_delay_loop(&self) -> bool {
        self.running
            && self.pbr == 0x01
            && self.cache_enabled
            && self.cbr == 0x84F0
            && matches!(self.regs[11], 0x8609 | 0x8615)
            && self.regs[13] == 0x000B
            && matches!(self.regs[15], 0x000B..=0x000D)
    }

    pub fn debug_in_starfox_late_parser_loop(&self) -> bool {
        self.running && self.pbr == 0x01 && matches!(self.regs[13], 0xD1B4..=0xD4EB)
    }

    pub(super) fn starfox_table_has_head_word(&self, key: u16) -> bool {
        (0..0x012Cusize).any(|index| {
            let addr = 0x1AB8u16.wrapping_add((index as u16).wrapping_mul(0x000E));
            self.debug_read_ram_word_short(addr) == key
        })
    }

    pub(super) fn starfox_table_find_head_by_any_word(&self, key: u16) -> Option<u16> {
        (0..0x012Cusize).find_map(|index| {
            let base = 0x1AB8u16.wrapping_add((index as u16).wrapping_mul(0x000E));
            let head = self.debug_read_ram_word_short(base);
            (0..7usize).find_map(|field| {
                let addr = base.wrapping_add((field as u16).wrapping_mul(2));
                (self.debug_read_ram_word_short(addr) == key).then_some(head)
            })
        })
    }

    pub(super) fn maybe_force_starfox_late_search_key_from_match(&mut self) {
        if env_presence_cached("STARFOX_FORCE_LATE_SEARCH_KEY_FROM_ANY_TABLE_FIELD")
            && self.running
            && self.pbr == 0x01
            && self.regs[15] == 0xD47A
        {
            let current_key = self.regs[7];
            if !self.starfox_table_has_head_word(current_key) {
                if let Some(head) = self.starfox_table_find_head_by_any_word(current_key) {
                    self.regs[7] = head;
                    return;
                }
            }
        }

        if !env_presence_cached("STARFOX_FORCE_LATE_SEARCH_KEY_FROM_1AE2") {
            return;
        }
        if !self.running || self.pbr != 0x01 || self.regs[15] != 0xD47A {
            return;
        }

        let cursor = self.read_ram_word(0x1AE0);
        let match_key = self.read_ram_word(0x1AE2);
        if cursor != 0xFFF9 || match_key == 0 {
            return;
        }

        let current_key = self.regs[7];
        let has_head_match = (0..0x012Cusize).any(|index| {
            let addr = 0x1AB8u16.wrapping_add((index as u16).wrapping_mul(0x000E));
            self.read_ram_word(addr) == current_key
        });
        if has_head_match {
            return;
        }

        let match_has_head = (0..0x012Cusize).any(|index| {
            let addr = 0x1AB8u16.wrapping_add((index as u16).wrapping_mul(0x000E));
            self.read_ram_word(addr) == match_key
        });
        if !match_has_head {
            return;
        }

        self.regs[7] = match_key;
    }

    pub(super) fn maybe_force_starfox_parser_key_from_match_word(
        &self,
        addr: u16,
        value: u16,
    ) -> u16 {
        if env_presence_cached("STARFOX_FORCE_PARSER_KEY_FROM_ANY_TABLE_FIELD")
            && self.running
            && self.pbr == 0x01
            && self.current_exec_pbr == 0x01
            && self.current_exec_pc == 0xAD46
            && self.current_exec_opcode == 0xA0
            && addr == 0x0136
            && !self.starfox_table_has_head_word(value)
        {
            if let Some(head) = self.starfox_table_find_head_by_any_word(value) {
                return head;
            }
        }

        if !env_presence_cached("STARFOX_FORCE_PARSER_KEY_FROM_MATCH_WORD") {
            return value;
        }
        if !self.running
            || self.pbr != 0x01
            || self.current_exec_pbr != 0x01
            || self.current_exec_pc != 0xAD46
            || self.current_exec_opcode != 0xA0
            || addr != 0x0136
        {
            return value;
        }

        let cursor = self.debug_read_ram_word_short(0x1AE0);
        let match_key = self.debug_read_ram_word_short(0x1AE2);
        if cursor != 0xFFF9 || match_key == 0 || value == match_key {
            return value;
        }

        let value_has_head = self.starfox_table_has_head_word(value);
        let match_has_head = self.starfox_table_has_head_word(match_key);
        if value_has_head || !match_has_head {
            return value;
        }

        match_key
    }

    pub(super) fn maybe_keep_starfox_success_cursor_armed(&self, addr: u16, value: u16) -> u16 {
        if !env_presence_cached("STARFOX_KEEP_SUCCESS_CURSOR_ARMED")
            && !env_presence_cached("STARFOX_KEEP_SUCCESS_CONTEXT")
        {
            return value;
        }
        if !self.running
            || self.pbr != 0x01
            || self.current_exec_pbr != 0x01
            || self.current_exec_pc != 0xD1CC
            || addr != 0x1AE0
            || value != 0x0000
        {
            return value;
        }

        0xFFF9
    }

    pub(super) fn maybe_keep_starfox_success_branch_target(
        &self,
        index: usize,
        value: u16,
        pc: u16,
    ) -> u16 {
        let keep_success_branch = env_presence_cached("STARFOX_KEEP_SUCCESS_BRANCH_TARGET");
        let keep_success_context = env_presence_cached("STARFOX_KEEP_SUCCESS_CONTEXT");
        let force_success_b196 = env_presence_cached("STARFOX_FORCE_SUCCESS_BRANCH_TO_B196");
        if !keep_success_branch && !keep_success_context && !force_success_b196 {
            return value;
        }
        if !self.running
            || self.pbr != 0x01
            || self.current_exec_pbr != 0x01
            || index != 13
            || pc != 0xD4D0
            || value != 0x0000
            || self.regs[13] != 0xD1B4
        {
            return value;
        }

        if force_success_b196 {
            0xB196
        } else {
            0xD1B4
        }
    }

    pub(super) fn maybe_keep_starfox_success_search_context(
        &self,
        index: usize,
        value: u16,
        pc: u16,
    ) -> u16 {
        if !env_presence_cached("STARFOX_KEEP_SUCCESS_CONTEXT") {
            return value;
        }
        if !self.running || self.pbr != 0x01 || self.current_exec_pbr != 0x01 {
            return value;
        }
        if self.regs[7] != 0x004B || self.regs[13] != 0xD1B4 || value != 0x0000 {
            return value;
        }

        match (index, pc) {
            (9, 0xD4BB) => self.regs[9],
            (13, 0xD4D0) => self.regs[13],
            _ => value,
        }
    }

    pub(super) fn maybe_force_starfox_b30a_r14_seed(
        &self,
        index: usize,
        value: u16,
        pc: u16,
    ) -> u16 {
        let Some(forced) = env_u16("STARFOX_FORCE_B30A_R14_VALUE") else {
            return value;
        };
        let frame = env_u16("STARFOX_FORCE_B30A_R14_FRAME")
            .map(u32::from)
            .unwrap_or_else(current_trace_superfx_frame);
        if !self.running
            || self.pbr != 0x01
            || self.current_exec_pbr != 0x01
            || current_trace_superfx_frame() != frame
            || index != 14
            || pc != 0xB30A
        {
            return value;
        }

        forced
    }

    pub(super) fn maybe_force_starfox_b380_r12_seed(
        &self,
        index: usize,
        value: u16,
        pc: u16,
    ) -> u16 {
        let Some(forced) = env_u16("STARFOX_FORCE_B380_R12_VALUE") else {
            return value;
        };
        let frame = env_u16("STARFOX_FORCE_B380_R12_FRAME")
            .map(u32::from)
            .unwrap_or_else(current_trace_superfx_frame);
        if !self.running
            || self.pbr != 0x01
            || self.current_exec_pbr != 0x01
            || current_trace_superfx_frame() != frame
            || index != 12
            || pc != 0xB380
        {
            return value;
        }

        forced
    }

    pub(super) fn maybe_force_starfox_b384_preexec_live_state(&mut self, pc: u16) {
        let frame = env_u16("STARFOX_FORCE_B384_PREEXEC_FRAME")
            .map(u32::from)
            .unwrap_or_else(current_trace_superfx_frame);
        if !self.running
            || self.pbr != 0x01
            || self.current_exec_pbr != 0x01
            || current_trace_superfx_frame() != frame
            || !(0xB384..=0xB396).contains(&pc)
        {
            return;
        }

        if let Some(value) = env_u16("STARFOX_FORCE_B384_PREEXEC_R12_VALUE") {
            self.regs[12] = value;
        }
        if let Some(value) = env_u16("STARFOX_FORCE_B384_PREEXEC_R14_VALUE") {
            self.regs[14] = value;
        }
    }

    pub(super) fn maybe_null_starfox_ac98_continuation_word(
        &self,
        index: usize,
        value: u16,
        pc: u16,
    ) -> u16 {
        if !env_presence_cached("STARFOX_NULL_AC98_AFTER_SUCCESS") {
            return value;
        }
        if !self.running
            || self.pbr != 0x01
            || self.current_exec_pbr != 0x01
            || index != 1
            || pc != 0xAC98
            || value != 0x887F
        {
            return value;
        }
        let match_key = self.debug_read_ram_word_short(0x1AE2);
        let success_fragment = self.debug_read_ram_word_short(0x888C);
        if match_key == 0x004B && success_fragment == 0x4BFC {
            return 0x0000;
        }
        value
    }

    pub(super) fn maybe_force_starfox_continuation_cursor_word(
        &self,
        addr: u16,
        value: u16,
    ) -> u16 {
        let forced_value = starfox_force_continuation_cursor_value();
        let force_match_fragment =
            env_presence_cached("STARFOX_FORCE_CONTINUATION_CURSOR_FROM_MATCH_FRAGMENT");
        let null_after_success = env_presence_cached("STARFOX_NULL_CONTINUATION_AFTER_SUCCESS");
        if !force_match_fragment && forced_value.is_none() && !null_after_success {
            return value;
        }
        if !self.running
            || self.pbr != 0x01
            || self.current_exec_pbr != 0x01
            || self.current_exec_pc != 0xACAD
            || addr != 0x04C4
        {
            return value;
        }
        if null_after_success {
            let cursor = self.debug_read_ram_word_short(0x1AE0);
            let match_key = self.debug_read_ram_word_short(0x1AE2);
            let success_fragment = self.debug_read_ram_word_short(0x888C);
            if cursor == 0xFFF9 && match_key == 0x004B && success_fragment == 0x4BFC {
                return 0x0000;
            }
        }
        if value != 0x887F {
            return value;
        }
        if let Some(forced_value) = forced_value {
            return forced_value;
        }
        0x888D
    }

    pub(super) fn maybe_force_starfox_continuation_ptr_byte(&self, addr: u16, value: u8) -> u8 {
        if !env_presence_cached("STARFOX_FORCE_CONTINUATION_PTR_FROM_MATCH_FRAGMENT") {
            return value;
        }
        if !self.running
            || self.pbr != 0x01
            || self.current_exec_pbr != 0x01
            || self.current_exec_pc != 0xB396
            || self.current_exec_opcode != 0x31
            || !matches!(addr, 0x021E | 0x021F)
        {
            return value;
        }

        // In the live path, 0x021E is finalized before 0x1AE2 has been re-armed
        // to 0x004B. Anchor this override to the already-produced success fragment.
        let success_fragment = self.debug_read_ram_word_short(0x888C);
        if success_fragment != 0x4BFC {
            return value;
        }

        let next_word = self.ram_word_after_byte_write(0x021E, addr, value);
        if next_word != 0x887F {
            return value;
        }

        match addr {
            0x021E => 0x8D,
            0x021F => 0x88,
            _ => value,
        }
    }

    pub(super) fn fast_forward_starfox_cached_delay_loop(&mut self) -> bool {
        if !self.debug_in_starfox_cached_delay_loop() || self.regs[15] != 0x000B {
            return false;
        }
        // 01:000B is the tight LOOP instruction that burns down R12 until it
        // reaches zero, then falls through to 01:000C. The status-poll helper
        // already special-cases this exact cached routine, so collapse the
        // counted loop in one step instead of iterating tens of thousands of
        // times during a single SFR poll.
        self.regs[12] = 0;
        self.update_sign_zero_flags(0);
        self.set_r15(0x000C);
        self.pipe = default_superfx_pipe();
        self.pipe_valid = false;
        self.clear_prefix_flags();
        self.maybe_force_starfox_late_search_key_from_match();

        true
    }

    pub(super) fn can_direct_run_starfox_late_wait(&self) -> bool {
        !trace_superfx_last_transfers_enabled()
            && !trace_superfx_pc_trace_enabled()
            && !trace_superfx_reg_flow_enabled()
            && !trace_superfx_profile_enabled()
            && !trace_superfx_start_enabled()
            && save_state_at_gsu_pc_range().is_none()
            && save_state_at_gsu_reg_write().is_none()
            && save_state_at_gsu_reg_eq().is_none()
            && save_state_at_gsu_recent_exec_tail().is_none()
            && save_state_at_superfx_ram_addr_config().is_none()
            && save_state_at_superfx_ram_byte_eq().is_none()
            && save_state_at_superfx_ram_word_eq().is_none()
    }

    pub(super) fn fast_forward_starfox_b4bf_rotate_loop(
        &mut self,
        step_budget_remaining: usize,
    ) -> Option<usize> {
        if !enable_experimental_starfox_fastpaths() {
            return None;
        }
        if !self.running
            || step_budget_remaining < 2
            || !self.can_direct_run_starfox_late_wait()
            || self.pbr != 0x01
            || self.rambr != 0x00
            || self.regs[3] == 0
        {
            return None;
        }

        let pc = self.regs[15];
        if !matches!(pc, 0xB4BA | 0xB4BF | 0xB4C0) {
            return None;
        }

        let loop_target = self.regs[13];
        if !matches!(loop_target, 0xB4BA | 0xB4BF) {
            return None;
        }

        let mut consumed = 0usize;

        if pc == 0xB4C0 {
            let next_r12 = self.regs[12].wrapping_sub(1);
            self.write_reg(12, next_r12);
            self.update_sign_zero_flags(next_r12);
            self.clear_prefix_flags();
            consumed += 1;

            if next_r12 == 0 {
                self.set_r15(0xB4C1);
                return Some(consumed);
            }

            self.set_r15(loop_target);
        } else if pc == 0xB4BA {
            // C3 / BEQ / NOP only gate entry into the rotate loop. When R3 is
            // non-zero, the loop body starts at B4BF.
            self.set_r15(0xB4BF);
            consumed += 3;
        }

        if step_budget_remaining <= consumed {
            return Some(consumed);
        }

        let iterations = usize::min(
            self.regs[12] as usize,
            (step_budget_remaining - consumed) / 2,
        );
        if iterations == 0 {
            return Some(consumed);
        }

        // Repeated ROL-through-carry over R4 is a rotate on the 17-bit ring
        // [carry, r4 bit0..bit15]. Collapse the hot B4BF/B4C0 loop by
        // rotating that ring and burning down R12 in one shot.
        let mask = (1u32 << 17) - 1;
        let shift = iterations % 17;
        let mut ring = u32::from(self.condition_carry_set() as u8) | (u32::from(self.regs[4]) << 1);
        if shift != 0 {
            ring = ((ring << shift) | (ring >> (17 - shift))) & mask;
        }

        let next_r4 = ((ring >> 1) & 0xFFFF) as u16;
        self.write_reg(4, next_r4);
        self.set_carry_flag((ring & 0x0001) != 0);

        let next_r12 = self.regs[12].wrapping_sub(iterations as u16);
        self.write_reg(12, next_r12);
        self.update_sign_zero_flags(next_r12);
        self.clear_prefix_flags();
        consumed += iterations * 2;

        if next_r12 == 0 {
            self.set_r15(0xB4C1);
        } else {
            self.set_r15(loop_target);
        }

        Some(consumed)
    }

    pub(super) fn fast_forward_starfox_b4b1_prefix_to_rotate_loop(
        &mut self,
        step_budget_remaining: usize,
    ) -> Option<usize> {
        if !enable_experimental_starfox_fastpaths() {
            return None;
        }
        if !self.running
            || step_budget_remaining < 2
            || !self.can_direct_run_starfox_late_wait()
            || self.pbr != 0x01
            || self.rambr != 0x00
        {
            return None;
        }

        let mut pc = self.regs[15];
        if !(0xB4B1..=0xB4BE).contains(&pc) {
            return None;
        }

        let mut consumed = 0usize;
        loop {
            match pc {
                0xB4B1 => {
                    self.clear_prefix_flags();
                    consumed += 1;
                    pc = 0xB4B2;
                }
                0xB4B2 => {
                    let lhs = self.regs[0];
                    let rhs = self.regs[4];
                    let diff = i32::from(lhs) - i32::from(rhs);
                    let result = diff as u16;
                    let overflow = (((lhs ^ rhs) & (lhs ^ result)) & 0x8000) != 0;
                    self.set_carry_flag(diff >= 0);
                    self.set_overflow_flag(overflow);
                    self.update_sign_zero_flags(result);
                    self.write_reg(0, result);
                    self.clear_prefix_flags();
                    consumed += 1;
                    pc = 0xB4B3;
                }
                0xB4B3 => {
                    self.with_reg = 7;
                    self.src_reg = 7;
                    self.dst_reg = 7;
                    self.sfr |= SFR_B_BIT;
                    consumed += 1;
                    pc = 0xB4B4;
                }
                0xB4B4 => {
                    let value = self.regs[7];
                    self.write_reg(13, value);
                    self.sfr &= !SFR_B_BIT;
                    self.clear_prefix_flags();
                    consumed += 1;
                    pc = 0xB4B5;
                }
                0xB4B5 => {
                    self.with_reg = 2;
                    self.src_reg = 2;
                    self.dst_reg = 2;
                    self.sfr |= SFR_B_BIT;
                    consumed += 1;
                    pc = 0xB4B6;
                }
                0xB4B6 => {
                    let value = self.regs[2];
                    let result = value >> 1;
                    self.set_carry_flag((value & 0x0001) != 0);
                    self.write_reg(2, result);
                    self.update_sign_zero_flags(result);
                    self.clear_prefix_flags();
                    consumed += 1;
                    pc = 0xB4B7;
                }
                0xB4B7 => {
                    self.with_reg = 3;
                    self.src_reg = 3;
                    self.dst_reg = 3;
                    self.sfr |= SFR_B_BIT;
                    consumed += 1;
                    pc = 0xB4B8;
                }
                0xB4B8 => {
                    let value = self.regs[3];
                    let carry_in = u16::from(self.condition_carry_set()) << 15;
                    let result = (value >> 1) | carry_in;
                    self.set_carry_flag((value & 0x0001) != 0);
                    self.write_reg(3, result);
                    self.update_sign_zero_flags(result);
                    self.clear_prefix_flags();
                    consumed += 1;
                    pc = 0xB4B9;
                }
                0xB4B9 => {
                    self.src_reg = 2;
                    consumed += 1;
                    pc = 0xB4BA;
                }
                0xB4BA => {
                    let result = self.regs[2] | self.regs[3];
                    self.write_reg(0, result);
                    self.update_sign_zero_flags(result);
                    self.clear_prefix_flags();
                    consumed += 1;
                    pc = 0xB4BB;
                }
                0xB4BB => {
                    consumed += 2;
                    if self.condition_zero_set() {
                        self.set_r15(0xB4C3);
                        return Some(consumed);
                    }
                    pc = 0xB4BD;
                }
                0xB4BD => {
                    self.clear_prefix_flags();
                    consumed += 1;
                    pc = 0xB4BE;
                }
                0xB4BE => {
                    self.with_reg = 4;
                    self.src_reg = 4;
                    self.dst_reg = 4;
                    self.sfr |= SFR_B_BIT;
                    consumed += 1;
                    self.set_r15(0xB4BF);
                    return Some(consumed);
                }
                _ => return None,
            }

            if consumed >= step_budget_remaining {
                self.set_r15(pc);
                return Some(consumed);
            }
        }
    }

    pub(super) fn fast_forward_starfox_outer_packet_setup(
        &mut self,
        step_budget_remaining: usize,
    ) -> Option<usize> {
        if !enable_experimental_starfox_fastpaths() {
            return None;
        }
        if !self.running
            || step_budget_remaining < 4
            || !self.can_direct_run_starfox_late_wait()
            || self.pbr != 0x01
            || self.rambr != 0x00
        {
            return None;
        }

        let mut pc = self.regs[15];
        if !matches!(
            pc,
            0xB33D | 0xB347..=0xB34D | 0xB367 | 0xB37C..=0xB383
        ) {
            return None;
        }

        let mut consumed = 0usize;

        if pc == 0xB33D {
            if self.regs[4] == 0 {
                self.set_r15(0xB3C1);
                return Some(6);
            }
            consumed += 10;
            pc = 0xB37C;
        } else if pc == 0xB367 {
            if self.regs[4] == 0 {
                return None;
            }
            consumed += 5;
            pc = 0xB37C;
        } else if (0xB347..=0xB34D).contains(&pc) {
            consumed += 5;
            pc = 0xB37C;
        }

        if pc <= 0xB37D {
            self.with_reg = 4;
            self.src_reg = 4;
            self.dst_reg = 4;
            self.sfr |= SFR_B_BIT;
            consumed += if pc == 0xB37C { 2 } else { 1 };
            pc = 0xB37E;
        }

        if pc <= 0xB37E {
            self.sfr = (self.sfr & !SFR_B_BIT) | SFR_ALT2_BIT;
            consumed += 1;
            pc = 0xB37F;
        }

        if pc <= 0xB37F {
            let lhs = self.regs[4];
            let rhs = 7u16;
            let sum = i32::from(lhs) + i32::from(rhs);
            let result = sum as u16;
            let overflow = ((!(lhs ^ rhs) & (rhs ^ result)) & 0x8000) != 0;
            self.set_carry_flag(sum >= 0x1_0000);
            self.set_overflow_flag(overflow);
            self.write_reg(4, result);
            self.update_sign_zero_flags(result);
            self.clear_prefix_flags();
            consumed += 1;
            pc = 0xB380;
        }

        if pc <= 0xB381 {
            self.write_reg(12, 0x0008);
            self.update_sign_zero_flags(0x0008);
            self.clear_prefix_flags();
            consumed += if pc == 0xB380 { 2 } else { 1 };
            pc = 0xB382;
        }

        if pc <= 0xB382 {
            self.with_reg = 15;
            self.src_reg = 15;
            self.dst_reg = 15;
            self.sfr |= SFR_B_BIT;
            consumed += 1;
            pc = 0xB383;
        }

        if pc <= 0xB383 {
            self.write_reg(13, 0xB384);
            self.sfr &= !SFR_B_BIT;
            self.clear_prefix_flags();
            consumed += 1;
        }

        self.set_r15(0xB384);
        Some(consumed)
    }

    pub(super) fn run_steps_direct_no_pipe(&mut self, rom: &[u8], step_budget: usize) {
        if !self.running || step_budget == 0 {
            return;
        }

        let mut steps = 0usize;
        let mut instruction_count = 0usize;
        self.pipe_valid = false;

        while self.running && steps < step_budget {
            if let Some(consumed_steps) =
                self.fast_forward_starfox_outer_packet_setup(step_budget - steps)
            {
                instruction_count += consumed_steps;
                steps += consumed_steps;
                continue;
            }

            if let Some(consumed_steps) =
                self.fast_forward_starfox_b4b1_prefix_to_rotate_loop(step_budget - steps)
            {
                instruction_count += consumed_steps;
                steps += consumed_steps;
                continue;
            }

            if let Some(consumed_steps) =
                self.fast_forward_starfox_b4bf_rotate_loop(step_budget - steps)
            {
                instruction_count += consumed_steps;
                steps += consumed_steps;
                continue;
            }

            if let Some(consumed_steps) =
                self.fast_forward_starfox_live_producer_store(rom, step_budget - steps)
            {
                instruction_count += consumed_steps;
                steps += consumed_steps;
                continue;
            }

            if self.pending_delay_pc.is_some()
                || self.pending_delay_pbr.is_some()
                || self.pending_delay_cache_base.is_some()
            {
                self.apply_pending_delay_transfer();
            }

            let pc = self.regs[15];
            let exec_pbr = self.pbr;
            let Some(opcode) = self.read_program_rom_byte(rom, exec_pbr, pc) else {
                self.trace_abort("direct-fetch", pc, 0xFF);
                self.finish_noop_run();
                return;
            };
            self.advance_r15_after_fetch();
            self.current_exec_pbr = exec_pbr;
            self.current_exec_pc = pc;
            self.current_exec_opcode = opcode;
            if starfox_b384_preexec_debug_override_enabled() {
                self.maybe_force_starfox_b384_preexec_live_state(pc);
            }

            if !self.execute_opcode(opcode, rom, pc) {
                self.total_run_instructions += instruction_count as u64;
                self.finish_noop_run();
                return;
            }

            self.pipe_valid = false;
            instruction_count += 1;
            steps += self.last_opcode_cycles;
        }

        self.total_run_instructions += instruction_count as u64;
    }

    pub(super) fn fast_forward_starfox_live_producer_store(
        &mut self,
        rom: &[u8],
        step_budget_remaining: usize,
    ) -> Option<usize> {
        if !enable_experimental_starfox_fastpaths() {
            return None;
        }
        if !self.running
            || step_budget_remaining < 8
            || !self.can_direct_run_starfox_late_wait()
            || self.pbr != 0x01
            || self.rambr != 0x00
            || !matches!(
                self.regs[13],
                0xB37F | 0xB380 | 0xB384 | 0xB392 | 0xB39D | 0xB3B8
            )
            || !matches!(
                self.regs[15],
                0xB37F
                    | 0xB380
                    | 0xB384
                    | 0xB389
                    | 0xB38A
                    | 0xB38B
                    | 0xB38C
                    | 0xB38D
                    | 0xB38E
                    | 0xB38F
                    | 0xB390
                    | 0xB391
                    | 0xB392
                    | 0xB39D..=0xB3B8
            )
        {
            return None;
        }

        let mut consumed = 0usize;
        let mut pc = self.regs[15];

        if pc == 0xB380 {
            self.write_reg(12, 0x0008);
            self.update_sign_zero_flags(0x0008);
            self.clear_prefix_flags();
            self.regs[13] = 0xB384;
            self.set_r15(0xB384);
            pc = 0xB384;
            consumed += 3;
        }

        loop {
            match pc {
                0xB37F => {
                    let lhs = self.regs[0];
                    let rhs = self.regs[7];
                    let sum = u32::from(lhs) + u32::from(rhs);
                    let result = sum as u16;
                    let overflow = ((!(lhs ^ rhs) & (rhs ^ result)) & 0x8000) != 0;
                    self.write_reg(0, result);
                    self.set_carry_flag(sum >= 0x1_0000);
                    self.set_overflow_flag(overflow);
                    self.update_sign_zero_flags(result);
                    self.clear_prefix_flags();

                    self.write_reg(12, 0x0008);
                    self.update_sign_zero_flags(0x0008);
                    self.clear_prefix_flags();
                    self.regs[13] = 0xB384;
                    self.set_r15(0xB384);
                    consumed += 4;
                    pc = 0xB384;
                }
                0xB384..=0xB391 => {
                    if pc <= 0xB384 {
                        self.with_reg = 2;
                        self.src_reg = 2;
                        self.dst_reg = 2;
                        self.sfr |= SFR_B_BIT;
                        consumed += 1;
                        pc = 0xB385;
                    }

                    if pc <= 0xB385 {
                        let value = self.regs[2];
                        let result = value >> 1;
                        self.set_carry_flag((value & 0x0001) != 0);
                        self.write_reg(2, result);
                        self.update_sign_zero_flags(result);
                        self.clear_prefix_flags();
                        consumed += 1;
                        pc = 0xB386;
                    }

                    if pc <= 0xB386 {
                        self.with_reg = 3;
                        self.src_reg = 3;
                        self.dst_reg = 3;
                        self.sfr |= SFR_B_BIT;
                        consumed += 1;
                        pc = 0xB387;
                    }

                    if pc <= 0xB387 {
                        let value = self.regs[3];
                        let carry_in = u16::from(self.condition_carry_set()) << 15;
                        let result = (value >> 1) | carry_in;
                        self.set_carry_flag((value & 0x0001) != 0);
                        self.write_reg(3, result);
                        self.update_sign_zero_flags(result);
                        self.clear_prefix_flags();
                        consumed += 1;
                        pc = 0xB388;
                    }

                    if pc <= 0xB388 {
                        self.src_reg = 2;
                        consumed += 1;
                        pc = 0xB389;
                    }

                    if pc <= 0xB389 {
                        let result = self.regs[2] | self.regs[3];
                        self.write_reg(0, result);
                        self.update_sign_zero_flags(result);
                        self.clear_prefix_flags();
                        consumed += 1;
                        pc = 0xB38A;
                    }

                    if pc <= 0xB38B {
                        consumed += 2;
                        if self.condition_zero_set() {
                            self.set_r15(0xB39D);
                            pc = 0xB39D;
                            continue;
                        }
                        pc = 0xB38C;
                    }

                    if pc <= 0xB38C {
                        self.clear_prefix_flags();
                        consumed += 1;
                        pc = 0xB38D;
                    }

                    if pc <= 0xB38D {
                        self.with_reg = 6;
                        self.src_reg = 6;
                        self.dst_reg = 6;
                        self.sfr |= SFR_B_BIT;
                        consumed += 1;
                        pc = 0xB38E;
                    }

                    if pc <= 0xB38E {
                        let value = self.regs[6];
                        let carry_in = u16::from(self.condition_carry_set());
                        let result = (value << 1) | carry_in;
                        self.set_carry_flag((value & 0x8000) != 0);
                        self.write_reg(6, result);
                        self.update_sign_zero_flags(result);
                        self.clear_prefix_flags();
                        consumed += 1;
                        pc = 0xB38F;
                    }

                    if pc <= 0xB38F {
                        self.with_reg = 5;
                        self.src_reg = 5;
                        self.dst_reg = 5;
                        self.sfr |= SFR_B_BIT;
                        consumed += 1;
                        pc = 0xB390;
                    }

                    if pc <= 0xB390 {
                        let value = self.regs[5];
                        let carry_in = u16::from(self.condition_carry_set());
                        let result = (value << 1) | carry_in;
                        self.set_carry_flag((value & 0x8000) != 0);
                        self.write_reg(5, result);
                        self.update_sign_zero_flags(result);
                        self.clear_prefix_flags();
                        consumed += 1;
                    }

                    let next_r12 = self.regs[12].wrapping_sub(1);
                    self.write_reg(12, next_r12);
                    self.update_sign_zero_flags(next_r12);
                    self.clear_prefix_flags();
                    consumed += 1;

                    if next_r12 != 0 {
                        if consumed.saturating_add(8) > step_budget_remaining {
                            self.set_r15(self.regs[13]);
                            return Some(consumed);
                        }
                        self.set_r15(self.regs[13]);
                        pc = self.regs[13];
                        continue;
                    }

                    self.set_r15(0xB392);
                    pc = 0xB392;
                }
                0xB39D..=0xB3B7 => loop {
                    match pc {
                        0xB39D => {
                            self.clear_prefix_flags();
                            consumed += 1;
                            pc = 0xB39E;
                        }
                        0xB39E | 0xB3A1 | 0xB3A5 | 0xB3A8 => {
                            let next_r14 = self.regs[14].wrapping_sub(1);
                            self.write_reg(14, next_r14);
                            self.update_sign_zero_flags(next_r14);
                            self.clear_prefix_flags();
                            consumed += 1;
                            pc = pc.wrapping_add(1);
                        }
                        0xB39F => {
                            self.dst_reg = 3;
                            consumed += 1;
                            pc = 0xB3A0;
                        }
                        0xB3A0 | 0xB3A4 | 0xB3A7 | 0xB3AB => {
                            let byte = self.read_data_rom_byte(rom)?;
                            let src_value = self.reg(self.src_reg);
                            let result = match self.alt_mode() {
                                0 => byte as u16,
                                1 => ((byte as u16) << 8) | (src_value & 0x00FF),
                                2 => (src_value & 0xFF00) | byte as u16,
                                3 => byte as i8 as i16 as u16,
                                _ => unreachable!(),
                            };
                            self.write_reg(self.dst_reg as usize, result);
                            self.update_sign_zero_flags(result);
                            self.clear_prefix_flags();
                            consumed += 1;
                            pc = pc.wrapping_add(1);
                        }
                        0xB3A2 => {
                            self.with_reg = 3;
                            self.src_reg = 3;
                            self.dst_reg = 3;
                            self.sfr |= SFR_B_BIT;
                            consumed += 1;
                            pc = 0xB3A3;
                        }
                        0xB3A3 | 0xB3AA => {
                            self.sfr = (self.sfr & !SFR_B_BIT) | SFR_ALT1_BIT;
                            consumed += 1;
                            pc = pc.wrapping_add(1);
                        }
                        0xB3A6 => {
                            self.dst_reg = 2;
                            consumed += 1;
                            pc = 0xB3A7;
                        }
                        0xB3A9 | 0xB3AF => {
                            self.with_reg = 2;
                            self.src_reg = 2;
                            self.dst_reg = 2;
                            self.sfr |= SFR_B_BIT;
                            consumed += 1;
                            pc = pc.wrapping_add(1);
                        }
                        0xB3AC => {
                            self.write_reg(0, 0x0001);
                            self.update_sign_zero_flags(0x0001);
                            self.clear_prefix_flags();
                            consumed += 2;
                            pc = 0xB3AE;
                        }
                        0xB3AE | 0xB3B0 | 0xB3B2 => {
                            let reg = self.src_reg as usize;
                            let value = self.reg(reg as u8);
                            let carry_in = u16::from(self.condition_carry_set()) << 15;
                            let result = (value >> 1) | carry_in;
                            self.set_carry_flag((value & 0x0001) != 0);
                            self.write_reg(reg, result);
                            self.update_sign_zero_flags(result);
                            self.clear_prefix_flags();
                            consumed += 1;
                            pc = pc.wrapping_add(1);
                        }
                        0xB3B1 => {
                            self.with_reg = 3;
                            self.src_reg = 3;
                            self.dst_reg = 3;
                            self.sfr |= SFR_B_BIT;
                            consumed += 1;
                            pc = 0xB3B2;
                        }
                        0xB3B3 => {
                            self.with_reg = 6;
                            self.src_reg = 6;
                            self.dst_reg = 6;
                            self.sfr |= SFR_B_BIT;
                            consumed += 1;
                            pc = 0xB3B4;
                        }
                        0xB3B4 | 0xB3B6 => {
                            let reg = self.src_reg as usize;
                            let value = self.reg(reg as u8);
                            let carry_in = u16::from(self.condition_carry_set());
                            let result = (value << 1) | carry_in;
                            self.set_carry_flag((value & 0x8000) != 0);
                            self.write_reg(reg, result);
                            self.update_sign_zero_flags(result);
                            self.clear_prefix_flags();
                            consumed += 1;
                            pc = pc.wrapping_add(1);
                        }
                        0xB3B5 => {
                            self.with_reg = 5;
                            self.src_reg = 5;
                            self.dst_reg = 5;
                            self.sfr |= SFR_B_BIT;
                            consumed += 1;
                            pc = 0xB3B6;
                        }
                        0xB3B7 => {
                            let next_r12 = self.regs[12].wrapping_sub(1);
                            self.write_reg(12, next_r12);
                            self.update_sign_zero_flags(next_r12);
                            self.clear_prefix_flags();
                            consumed += 1;

                            if next_r12 != 0 {
                                let loop_target = self.regs[13];
                                if consumed.saturating_add(7) > step_budget_remaining {
                                    self.set_r15(loop_target);
                                    return Some(consumed);
                                }
                                self.set_r15(loop_target);
                                pc = loop_target;
                                continue;
                            }

                            self.set_r15(0xB3B8);
                            pc = 0xB3B8;
                        }
                        _ => break,
                    }

                    if !(0xB39D..=0xB3B7).contains(&pc) {
                        break;
                    }
                },
                0xB392..=0xB39C | 0xB3B8 => {
                    if pc == 0xB3B8 {
                        pc = 0xB392;
                    }

                    if pc <= 0xB392 {
                        self.clear_prefix_flags();
                        consumed += 1;
                        pc = 0xB393;
                    }

                    if pc <= 0xB393 {
                        let next_r1 = self.regs[1].wrapping_sub(1);
                        self.write_reg(1, next_r1);
                        self.update_sign_zero_flags(next_r1);
                        self.clear_prefix_flags();
                        consumed += 1;
                        pc = 0xB394;
                    }

                    if pc <= 0xB394 {
                        self.src_reg = 6;
                        consumed += 1;
                        pc = 0xB395;
                    }

                    if pc <= 0xB395 {
                        self.sfr = (self.sfr & !SFR_B_BIT) | SFR_ALT1_BIT;
                        consumed += 1;
                        pc = 0xB396;
                    }

                    if pc <= 0xB396 {
                        self.write_ram_byte(self.regs[1], self.regs[6] as u8);
                        self.clear_prefix_flags();
                        consumed += 1;
                        pc = 0xB397;
                    }

                    if pc <= 0xB397 {
                        let next_r4 = self.regs[4].wrapping_sub(1);
                        self.write_reg(4, next_r4);
                        self.update_sign_zero_flags(next_r4);
                        self.clear_prefix_flags();
                        consumed += 1;
                        pc = 0xB398;
                    }

                    if pc <= 0xB399 {
                        consumed += 2;
                        if self.regs[4] != 0 {
                            if consumed.saturating_add(4) > step_budget_remaining {
                                self.set_r15(0xB380);
                                return Some(consumed);
                            }

                            let lhs = self.regs[0];
                            let rhs = self.regs[7];
                            let sum = u32::from(lhs) + u32::from(rhs);
                            let result = sum as u16;
                            let overflow = ((!(lhs ^ rhs) & (rhs ^ result)) & 0x8000) != 0;
                            self.write_reg(0, result);
                            self.set_carry_flag(sum >= 0x1_0000);
                            self.set_overflow_flag(overflow);
                            self.update_sign_zero_flags(result);
                            self.clear_prefix_flags();

                            self.write_reg(12, 0x0008);
                            self.update_sign_zero_flags(0x0008);
                            self.clear_prefix_flags();
                            self.regs[13] = 0xB384;
                            self.set_r15(0xB384);
                            consumed += 4;
                            pc = 0xB384;
                            continue;
                        }
                    }

                    self.set_r15(0xB3C0);
                    consumed += 3;
                    return Some(consumed);
                }
                _ => return None,
            }
        }
    }

    pub fn run_status_poll_until_starfox_cached_delay_loop_exit(
        &mut self,
        rom: &[u8],
        max_steps: usize,
    ) {
        if !self.running || max_steps == 0 || !self.debug_in_starfox_cached_delay_loop() {
            return;
        }
        const STARFOX_DELAY_LOOP_FOLLOWUP_STEPS: usize = 1;

        let mut remaining = max_steps;
        while self.running && remaining > 0 {
            if self.fast_forward_starfox_cached_delay_loop() {
                // Keep chewing through the later Star Fox cached routine until
                // it either leaves the delay loop signature or exhausts the
                // poll budget. The caller is already in a busy-wait on $3030,
                // so stopping after an arbitrary small cycle count just turns
                // the same loop into many expensive polls.
            }
            let steps = remaining.min(STARFOX_DELAY_LOOP_FOLLOWUP_STEPS);
            if self.can_direct_run_starfox_late_wait() {
                self.run_steps_direct_no_pipe(rom, steps);
            } else {
                self.run_steps(rom, steps);
            }
            remaining -= steps;
        }
    }

    pub fn run_status_poll_until_stop_with_starfox_late_wait_assist(
        &mut self,
        rom: &[u8],
        max_steps: usize,
    ) {
        if !self.running || max_steps == 0 {
            return;
        }
        // The Star Fox late wait bounces in and out of the cached 01:000B
        // delay loop. Large chunks let it re-enter the loop and burn tens of
        // thousands of raw iterations before we can collapse it again.
        let starfox_late_wait_chunk = Self::status_poll_step_budget().saturating_mul(16).max(1);

        let mut remaining = max_steps;
        while self.running && remaining > 0 {
            if self.fast_forward_starfox_cached_delay_loop() {
                continue;
            }
            if let Some(consumed) = self.fast_forward_starfox_live_producer_store(rom, remaining) {
                remaining = remaining.saturating_sub(consumed);
                continue;
            }
            let steps = remaining.min(starfox_late_wait_chunk);
            if self.can_direct_run_starfox_late_wait() {
                self.run_steps_direct_no_pipe(rom, steps);
            } else {
                self.run_steps(rom, steps);
            }
            remaining -= steps;
        }
    }
}
