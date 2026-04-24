use super::*;

impl Default for YmOperator {
    fn default() -> Self {
        Self {
            detune: 0,
            mul: 1,
            tl: 0,
            key_scale: 0,
            am_enable: false,
            ssg_eg: 0,
            ssg_invert: false,
            ssg_hold_active: false,
            attack_rate: 31,
            decay_rate: 0,
            sustain_rate: 0,
            sustain_level: 0,
            // Keep default release short to avoid lingering notes when a game
            // hasn't initialized operator envelopes yet.
            release_rate: 15,
            key_on: false,
            reg_key_on: false,
            csm_key_on: false,
            phase: 0,
            envelope_phase: YmEnvelopePhase::Off,
            envelope_level: 1023,
            last_output: 0,
        }
    }
}

impl Default for YmChannel {
    fn default() -> Self {
        Self {
            fnum: 0x200,
            block: 4,
            special_fnum: [0x200; 3],
            special_block: [4; 3],
            algorithm: 0,
            feedback: 0,
            feedback_sample: 0,
            feedback_sample_prev: 0,
            pan_left: true,
            pan_right: true,
            ams: 0,
            fms: 0,
            operators: [YmOperator::default(); 4],
        }
    }
}

impl Default for Ym2612 {
    fn default() -> Self {
        Self {
            addr_port0: 0,
            addr_port1: 0,
            regs: [[0; 256]; 2],
            writes: 0,
            dac_data_writes: 0,
            busy_z80_cycles: 0,
            timer_status: 0,
            timer_control: 0,
            timer_a_value: 0,
            timer_b_value: 0,
            timer_clock_accumulator: 0,
            timer_a_elapsed_ym_cycles: 0,
            timer_b_elapsed_ym_cycles: 0,
            dac_enabled: false,
            dac_output: 0,
            dac_enabled_pending: None,
            dac_output_pending: None,
            lfo_enabled: false,
            lfo_rate: 0,
            lfo_counter: 0,
            lfo_timer: 0,
            // Start near an EG boundary so low/mid attack rates begin moving
            // promptly after the first key-on instead of stalling for a long
            // initial counter phase.
            eg_counter: 127,
            hw_sample_frac: 0,
            hw_eg_divider: 0,
            last_hw_output: (0, 0),
            csm_key_on_active: false,
            csm_key_on_rendered: false,
            test_register: 0,
            channels: [YmChannel::default(); 6],
        }
    }
}

impl Ym2612 {
    // YM2612 BUSY stays asserted for roughly 32 master clocks after a write.
    // Converting directly from master-clock cycles matches observed software
    // pacing better than using OPN internal divider cycles.
    const BUSY_DURATION_MASTER_CYCLES: u64 = 32;
    const MASTER_CLOCK_HZ: u64 = 7_670_454;
    const Z80_CLOCK_HZ: u64 = 3_579_545;
    const YM2612_DIVIDER: u64 = 6;
    const BUSY_DURATION_Z80_CYCLES: u32 = ((Self::BUSY_DURATION_MASTER_CYCLES * Self::Z80_CLOCK_HZ
        + (Self::MASTER_CLOCK_HZ - 1))
        / Self::MASTER_CLOCK_HZ) as u32;
    // YM2612 DAC raw 8-bit data maps to a signed range centered at 0x80.
    // Keep output in a moderate range relative to FM mix to avoid clipping.
    const DAC_OUTPUT_SHIFT: i16 = 6;
    // DAC pending output stores a 1-bit ordering tag in bit0.
    // Actual DAC output values are multiples of 64, so bit0 is unused.
    const DAC_PENDING_ORDER_MASK: i16 = 0x0001;

    pub(super) fn write_port(&mut self, port: u8, value: u8) {
        self.write_port_internal(port, value, false);
    }

    pub(super) fn write_port_from_z80(&mut self, port: u8, value: u8) {
        self.write_port_internal(port, value, true);
    }

    fn write_port_internal(&mut self, port: u8, value: u8, from_z80: bool) {
        match port & 0x03 {
            0 => {
                self.addr_port0 = value;
                // YM2612 asserts BUSY for a short window after address writes too.
                self.arm_busy();
            }
            1 => {
                let reg = self.addr_port0;
                self.regs[0][reg as usize] = value;
                self.apply_write(0, reg, value, from_z80);
                self.writes += 1;
                self.arm_busy();
            }
            2 => {
                self.addr_port1 = value;
                self.arm_busy();
            }
            3 => {
                let reg = self.addr_port1;
                self.regs[1][reg as usize] = value;
                self.apply_write(1, reg, value, from_z80);
                self.writes += 1;
                self.arm_busy();
            }
            _ => {}
        }
    }

    fn arm_busy(&mut self) {
        self.busy_z80_cycles = Self::BUSY_DURATION_Z80_CYCLES;
    }

    fn apply_write(&mut self, bank: usize, reg: u8, value: u8, from_z80: bool) {
        if let Some(channel) = self.decode_fnum_low_channel(bank, reg) {
            self.channels[channel].fnum = (self.channels[channel].fnum & 0x0700) | value as u16;
        } else if let Some(channel) = self.decode_fnum_high_channel(bank, reg) {
            self.channels[channel].fnum =
                (self.channels[channel].fnum & 0x00FF) | (((value & 0x07) as u16) << 8);
            self.channels[channel].block = (value >> 3) & 0x07;
        } else if let Some(slot) = self.decode_channel3_special_low(bank, reg) {
            let channel = &mut self.channels[2];
            channel.special_fnum[slot] = (channel.special_fnum[slot] & 0x0700) | value as u16;
        } else if let Some(slot) = self.decode_channel3_special_high(bank, reg) {
            let channel = &mut self.channels[2];
            channel.special_fnum[slot] =
                (channel.special_fnum[slot] & 0x00FF) | (((value & 0x07) as u16) << 8);
            channel.special_block[slot] = (value >> 3) & 0x07;
        } else if let Some(channel) = self.decode_pan_channel(bank, reg) {
            self.channels[channel].pan_left = (value & 0x80) != 0;
            self.channels[channel].pan_right = (value & 0x40) != 0;
            self.channels[channel].ams = (value >> 4) & 0x03;
            self.channels[channel].fms = value & 0x07;
        } else if let Some(channel) = self.decode_algorithm_channel(bank, reg) {
            self.channels[channel].algorithm = value & 0x07;
            self.channels[channel].feedback = (value >> 3) & 0x07;
        } else if let Some((channel, slot, param)) = Self::decode_operator_target(bank, reg) {
            let op = &mut self.channels[channel].operators[slot];
            match param {
                YmOperatorParam::Mul => {
                    op.detune = (value >> 4) & 0x07;
                    op.mul = value & 0x0F;
                }
                YmOperatorParam::Tl => {
                    op.tl = value & 0x7F;
                }
                YmOperatorParam::Attack => {
                    op.key_scale = (value >> 6) & 0x03;
                    op.attack_rate = value & 0x1F;
                }
                YmOperatorParam::Decay => {
                    op.am_enable = (value & 0x80) != 0;
                    op.decay_rate = value & 0x1F;
                }
                YmOperatorParam::SustainRate => {
                    op.sustain_rate = value & 0x1F;
                }
                YmOperatorParam::SustainRelease => {
                    op.sustain_level = (value >> 4) & 0x0F;
                    op.release_rate = value & 0x0F;
                }
                YmOperatorParam::SsgEg => {
                    op.ssg_eg = value & 0x0F;
                    if (op.ssg_eg & 0x08) == 0 {
                        op.ssg_invert = false;
                        op.ssg_hold_active = false;
                    }
                }
            }
        }

        if bank == 0 {
            match reg {
                0x21 => {
                    // Test register: bit 0 = LFO halt (Nuked OPN2), other bits = test modes.
                    // Most games write 0; store for reference.
                    self.test_register = value;
                }
                0x22 => {
                    self.lfo_enabled = (value & 0x08) != 0;
                    self.lfo_rate = value & 0x07;
                    // Nuked OPN2: LFO counter preserved when disabled (just stops advancing)
                }
                0x24 => {
                    self.timer_a_value = (self.timer_a_value & 0x0003) | ((value as u16) << 2);
                }
                0x25 => {
                    self.timer_a_value = (self.timer_a_value & 0x03FC) | ((value as u16) & 0x03);
                }
                0x26 => {
                    self.timer_b_value = value;
                }
                0x27 => {
                    self.timer_control = value;
                    if (value & 0x10) != 0 {
                        self.timer_status &= !0x01;
                    }
                    if (value & 0x20) != 0 {
                        self.timer_status &= !0x02;
                    }
                    // Timer load bits explicitly reload their counters even when
                    // the timer is already running.
                    if (value & 0x01) != 0 {
                        self.timer_a_elapsed_ym_cycles = 0;
                    }
                    if (value & 0x02) != 0 {
                        self.timer_b_elapsed_ym_cycles = 0;
                    }
                }
                0x28 => {
                    if let Some(channel) = Self::decode_keyon_channel(value) {
                        let mut reset_feedback = false;
                        let slot_mask = (value >> 4) & 0x0F;
                        for op_index in 0..4 {
                            let next_reg_key_on =
                                Self::keyon_slot_mask_targets_operator(slot_mask, op_index);
                            let op = &mut self.channels[channel].operators[op_index];
                            let old_key_on = op.key_on;
                            op.reg_key_on = next_reg_key_on;
                            let new_key_on = op.reg_key_on || op.csm_key_on;
                            if new_key_on && !old_key_on {
                                op.phase = 0;
                                op.last_output = 0;
                                op.envelope_phase = YmEnvelopePhase::Attack;
                                // Do not reset envelope_level — real HW continues from current level
                                // Nuked OPN2: normalize SSG-EG inverted level on key-on
                                if Self::ssg_eg_enabled(op) && (op.envelope_level & 0x200) != 0 {
                                    op.envelope_level =
                                        (0x200u16.wrapping_sub(op.envelope_level)) & 0x3FF;
                                }
                                op.ssg_invert = false;
                                op.ssg_hold_active = false;
                                if op_index == 0 {
                                    reset_feedback = true;
                                }
                            } else if !new_key_on && old_key_on {
                                // Nuked OPN2: normalize SSG-EG inverted level before Release
                                if Self::ssg_eg_enabled(op) && (op.envelope_level & 0x200) != 0 {
                                    op.envelope_level =
                                        (0x200u16.wrapping_sub(op.envelope_level)) & 0x3FF;
                                }
                                op.ssg_invert = false;
                                op.envelope_phase = if op.envelope_level < 1023 {
                                    YmEnvelopePhase::Release
                                } else {
                                    YmEnvelopePhase::Off
                                };
                                op.ssg_hold_active = false;
                            }
                            op.key_on = new_key_on;
                        }
                        if reset_feedback {
                            self.channels[channel].feedback_sample = 0;
                            self.channels[channel].feedback_sample_prev = 0;
                        }
                    }
                }
                0x2A => {
                    let centered = value as i16 - 0x80;
                    let output = centered << Self::DAC_OUTPUT_SHIFT;
                    if from_z80 {
                        let output_after_enable = self.dac_enabled_pending.is_some();
                        self.dac_output_pending =
                            Some(Self::encode_pending_dac_output(output, output_after_enable));
                    } else {
                        self.dac_output = output;
                    }
                    self.dac_data_writes += 1;
                }
                0x2B => {
                    let enabled = (value & 0x80) != 0;
                    if from_z80 {
                        self.dac_enabled_pending = Some(enabled);
                    } else {
                        self.set_dac_enabled(enabled);
                    }
                }
                _ => {}
            }
        }
    }

    fn decode_fnum_low_channel(&self, bank: usize, reg: u8) -> Option<usize> {
        if (0xA0..=0xA2).contains(&reg) {
            Some((bank & 1) * 3 + (reg as usize - 0xA0))
        } else {
            None
        }
    }

    fn decode_fnum_high_channel(&self, bank: usize, reg: u8) -> Option<usize> {
        if (0xA4..=0xA6).contains(&reg) {
            Some((bank & 1) * 3 + (reg as usize - 0xA4))
        } else {
            None
        }
    }

    fn decode_channel3_special_low(&self, bank: usize, reg: u8) -> Option<usize> {
        if bank == 0 && (0xA8..=0xAA).contains(&reg) {
            Some((reg - 0xA8) as usize)
        } else {
            None
        }
    }

    fn decode_channel3_special_high(&self, bank: usize, reg: u8) -> Option<usize> {
        if bank == 0 && (0xAC..=0xAE).contains(&reg) {
            Some((reg - 0xAC) as usize)
        } else {
            None
        }
    }

    fn decode_keyon_channel(value: u8) -> Option<usize> {
        match value & 0x07 {
            0 => Some(0),
            1 => Some(1),
            2 => Some(2),
            4 => Some(3),
            5 => Some(4),
            6 => Some(5),
            _ => None,
        }
    }

    fn keyon_slot_mask_targets_operator(slot_mask: u8, op_index: usize) -> bool {
        // YM2612 key-on bits: b4=OP1, b5=OP2, b6=OP3, b7=OP4.
        match op_index.min(3) {
            0 => (slot_mask & 0b0001) != 0, // OP1 (b4)
            1 => (slot_mask & 0b0010) != 0, // OP2 (b5)
            2 => (slot_mask & 0b0100) != 0, // OP3 (b6)
            _ => (slot_mask & 0b1000) != 0, // OP4 (b7)
        }
    }

    fn decode_pan_channel(&self, bank: usize, reg: u8) -> Option<usize> {
        if (0xB4..=0xB6).contains(&reg) {
            Some((bank & 1) * 3 + (reg as usize - 0xB4))
        } else {
            None
        }
    }

    fn decode_algorithm_channel(&self, bank: usize, reg: u8) -> Option<usize> {
        if (0xB0..=0xB2).contains(&reg) {
            Some((bank & 1) * 3 + (reg as usize - 0xB0))
        } else {
            None
        }
    }

    fn decode_operator_target(bank: usize, reg: u8) -> Option<(usize, usize, YmOperatorParam)> {
        let param = match reg & 0xF0 {
            0x30 => YmOperatorParam::Mul,
            0x40 => YmOperatorParam::Tl,
            0x50 => YmOperatorParam::Attack,
            0x60 => YmOperatorParam::Decay,
            0x70 => YmOperatorParam::SustainRate,
            0x80 => YmOperatorParam::SustainRelease,
            0x90 => YmOperatorParam::SsgEg,
            _ => return None,
        };

        let low = reg & 0x0F;
        if (low & 0x03) == 0x03 {
            return None;
        }
        let channel_in_bank = (low & 0x03) as usize;
        if channel_in_bank >= 3 {
            return None;
        }
        let slot_group = (low >> 2) as usize;
        let slot = match slot_group {
            0 => 0, // OP1
            1 => 2, // OP3
            2 => 1, // OP2
            3 => 3, // OP4
            _ => return None,
        };
        let channel = (bank & 1) * 3 + channel_in_bank;
        Some((channel, slot, param))
    }

    fn block_fnum_keycode(block: u8, fnum: u16) -> u8 {
        ((block & 0x07) << 2) | FN_NOTE[((fnum >> 7) & 0x0F) as usize]
    }

    #[allow(dead_code)]
    fn channel_keycode(channel: &YmChannel) -> u8 {
        Self::block_fnum_keycode(channel.block, channel.fnum)
    }

    fn channel3_special_mode_enabled(&self) -> bool {
        self.channel3_mode_bits() != 0 // CSM (0b10) also enables special frequencies
    }

    fn channel3_special_slot_for_operator(operator_index: usize) -> Option<usize> {
        match operator_index.min(3) {
            0 => Some(1), // OP1 <- A9/AD
            1 => Some(2), // OP2 <- AA/AE
            2 => Some(0), // OP3 <- A8/AC
            _ => None,    // OP4 uses channel FNUM/BLOCK (A2/A6)
        }
    }

    fn operator_fnum_block(
        channel: &YmChannel,
        operator_index: usize,
        channel3_special_mode: bool,
    ) -> (u16, u8) {
        if channel3_special_mode
            && let Some(slot) = Self::channel3_special_slot_for_operator(operator_index)
        {
            (channel.special_fnum[slot], channel.special_block[slot])
        } else {
            (channel.fnum, channel.block)
        }
    }

    #[allow(dead_code)]
    fn operator_keycode(
        channel: &YmChannel,
        operator_index: usize,
        channel3_special_mode: bool,
    ) -> u8 {
        let (fnum, block) =
            Self::operator_fnum_block(channel, operator_index, channel3_special_mode);
        Self::block_fnum_keycode(block, fnum)
    }

    /// Compute the raw phase increment from FNUM, BLOCK, MUL, and detune.
    /// Nuked OPN2: detune is applied to base frequency before MUL multiplication.
    fn compute_phase_inc(fnum: u16, block: u8, mul: u8, detune: u8, keycode: u8) -> u32 {
        let base = ((fnum as u32) << (block as u32)) >> 1;
        let dt_base = (detune & 0x03) as usize;
        let dt_delta = DETUNE_TABLE[dt_base][keycode as usize] as u32;
        let detuned = if (detune & 0x04) != 0 {
            base.saturating_sub(dt_delta)
        } else {
            base + dt_delta
        };
        if mul == 0 {
            detuned >> 1
        } else {
            detuned * (mul as u32)
        }
    }

    fn key_scale_rate_boost(keycode: u8, key_scale: u8) -> u8 {
        // Nuked OPN2: rks = kc >> (ks ^ 0x03)
        match key_scale & 0x03 {
            0 => keycode >> 3,
            1 => keycode >> 2,
            2 => keycode >> 1,
            _ => keycode,
        }
    }

    fn ssg_eg_enabled(op: &YmOperator) -> bool {
        (op.ssg_eg & 0x08) != 0
    }

    fn ssg_eg_output_level(op: &YmOperator) -> u16 {
        // GenPlusGX: SSG-EG output inversion disabled when key is off
        if !Self::ssg_eg_enabled(op) || !op.key_on {
            return op.envelope_level;
        }
        let attack_invert = (op.ssg_eg & 0x04) != 0;
        let invert = attack_invert ^ op.ssg_invert;
        if invert {
            (512u16.wrapping_sub(op.envelope_level)) & 0x3FF
        } else {
            op.envelope_level
        }
    }

    /// Returns true if SSG-EG cycle fired (phase changed to Attack), in which case
    /// the caller should skip EG advance for this cycle (Nuked OPN2 behavior).
    fn advance_ssg_eg_cycle(op: &mut YmOperator) -> bool {
        if !Self::ssg_eg_enabled(op) || !op.key_on {
            return false;
        }
        // Nuked OPN2: SSG-EG cycle triggers when bit 9 of envelope level is set (>= 0x200).
        if op.envelope_phase == YmEnvelopePhase::Attack || op.envelope_level < 0x200 {
            return false;
        }

        let hold = (op.ssg_eg & 0x01) != 0;
        let alternate = (op.ssg_eg & 0x02) != 0;
        let attack = (op.ssg_eg & 0x04) != 0;
        let current_top = attack ^ op.ssg_invert;
        if hold {
            if alternate {
                op.ssg_invert = !op.ssg_invert;
            }
            if current_top {
                op.ssg_invert = attack;
            }
            // Hold latches the envelope at the endpoint reached by the SSG-EG
            // cycle rather than keeping the pre-cycle attenuation.
            op.envelope_level = if current_top { 0 } else { 1023 };
            op.ssg_hold_active = true;
            return false; // hold doesn't restart Attack
        }
        if alternate {
            op.ssg_invert = !op.ssg_invert;
        }
        if !hold && !alternate {
            op.phase = 0; // Nuked OPN2: phase reset on SSG-EG loop (non-alternate)
        }
        op.envelope_phase = YmEnvelopePhase::Attack;
        // Nuked OPN2: level is NOT reset on SSG-EG cycle
        true // cycle fired — skip EG advance this cycle
    }

    fn advance_envelope(op: &mut YmOperator, eg_counter: u32, keycode: u8) {
        if Self::ssg_eg_enabled(op) && op.ssg_hold_active && op.key_on {
            return;
        }

        // Nuked OPN2: ksv = kc >> (ks ^ 0x03), applied once to each rate
        let ksv = Self::key_scale_rate_boost(keycode, op.key_scale) as u16;

        match op.envelope_phase {
            YmEnvelopePhase::Off => {
                op.envelope_level = 1023;
            }
            YmEnvelopePhase::Attack => {
                // GenPlusGX: if base rate is 0, effective rate stays 0 regardless of KSR
                let effective_rate = if op.attack_rate == 0 {
                    0u8
                } else {
                    ((op.attack_rate as u16) * 2 + ksv).min(63) as u8
                };
                if effective_rate >= 62 {
                    op.envelope_level = 0;
                    op.envelope_phase = YmEnvelopePhase::Decay;
                } else {
                    let inc = eg_increment(effective_rate, eg_counter);
                    if inc > 0 {
                        let level = op.envelope_level as i32;
                        let delta = ((!level) * inc as i32) >> 4;
                        let new_level = (level + delta).max(0);
                        op.envelope_level = new_level as u16;
                    }
                    if op.envelope_level == 0 {
                        op.envelope_phase = YmEnvelopePhase::Decay;
                    }
                }
            }
            YmEnvelopePhase::Decay => {
                let sustain_target = if op.sustain_level >= 0x0F {
                    1023
                } else {
                    (op.sustain_level as u16) << 5
                };
                let effective_rate = if op.decay_rate == 0 {
                    0u8
                } else {
                    ((op.decay_rate as u16) * 2 + ksv).min(63) as u8
                };
                let inc = eg_increment(effective_rate, eg_counter);
                if inc > 0 {
                    op.envelope_level = (op.envelope_level + inc).min(1023);
                }
                if !Self::ssg_eg_enabled(op) && op.envelope_level >= sustain_target {
                    op.envelope_phase = YmEnvelopePhase::Sustain;
                }
            }
            YmEnvelopePhase::Sustain => {
                let effective_rate = if op.sustain_rate == 0 {
                    0u8
                } else {
                    ((op.sustain_rate as u16) * 2 + ksv).min(63) as u8
                };
                let inc = eg_increment(effective_rate, eg_counter);
                if inc > 0 {
                    op.envelope_level = (op.envelope_level + inc).min(1023);
                }
            }
            YmEnvelopePhase::Release => {
                // Nuked OPN2: release rate = RR * 4 + 2 + ksv
                let effective_rate = ((op.release_rate as u16) * 4 + 2 + ksv).min(63) as u8;
                let inc = eg_increment(effective_rate, eg_counter);
                if inc > 0 {
                    op.envelope_level = (op.envelope_level + inc).min(1023);
                }
                if op.envelope_level >= 1023 {
                    op.envelope_phase = YmEnvelopePhase::Off;
                }
            }
        }
    }

    fn operator_active(op: &YmOperator) -> bool {
        op.key_on || op.envelope_phase != YmEnvelopePhase::Off
    }

    fn channel_active(channel: &YmChannel) -> bool {
        channel.operators.iter().any(Self::operator_active)
    }

    /// Advance the LFO and return (am_value, pm_step, pm_sign).
    /// am_value: 0-126 triangle wave for amplitude modulation.
    /// pm_step: 0-7 magnitude step for phase modulation (8 steps per half-cycle).
    /// pm_sign: false=positive, true=negative.
    /// Advance LFO by one HW sample tick and return (am_value, pm_step, pm_sign).
    fn advance_lfo_hw(&mut self) -> (u8, u8, bool) {
        if !self.lfo_enabled {
            return (0, 0, false);
        }
        let lfo_inc = LFO_TIMER_INC[self.lfo_rate as usize & 7];
        self.lfo_timer = self.lfo_timer.wrapping_add(lfo_inc);
        while self.lfo_timer >= 65536 {
            self.lfo_timer -= 65536;
            self.lfo_counter = (self.lfo_counter + 1) & 0x7F;
        }

        let counter = self.lfo_counter;
        // AM: triangle wave 0→126→0
        let am = if counter < 64 {
            counter << 1
        } else {
            (127 - counter) << 1
        };
        // PM: step = (counter >> 2) & 7, sign from counter bit 6.
        let pm_step = (counter >> 2) & 7;
        let pm_sign = counter >= 64;
        (am, pm_step, pm_sign)
    }

    /// Compute PM displacement using the per-bit LFO_PM_OUTPUT table (GenPlusGX approach).
    /// fms: 0-7 depth, pm_step: 0-7 step within half-cycle, pm_sign: true = negative half.
    fn lfo_pm_displacement(fnum: u16, fms: u8, pm_step: u8, pm_sign: bool) -> i32 {
        if fms == 0 {
            return 0;
        }
        let step = (pm_step & 0x07) as usize;
        let fms_idx = (fms & 0x07) as usize;
        let mut displacement = 0i32;
        // Sum contributions from FNUM bits 4-10
        for bit in 0..7u32 {
            if (fnum >> (bit + 4)) & 1 != 0 {
                let row = (bit as usize) * 8 + fms_idx;
                displacement += LFO_PM_OUTPUT[row][step] as i32;
            }
        }
        if displacement == 0 {
            return 0;
        }
        if pm_sign { -displacement } else { displacement }
    }

    /// Advance the global EG counter by one HW sample tick.
    /// EG advances every 3 HW samples (HW_RATE / 3 = EG_RATE).
    fn advance_eg_counter_hw(&mut self) -> bool {
        self.hw_eg_divider += 1;
        if self.hw_eg_divider >= 3 {
            self.hw_eg_divider = 0;
            self.eg_counter = self.eg_counter.wrapping_add(1);
            if self.eg_counter == 0 {
                self.eg_counter = 1;
            } // Nuked OPN2: skip 0
            return true;
        }
        false
    }

    fn advance_operator_sample(
        op: &mut YmOperator,
        phase_inc: u32,
        phase_mod: i32,
        eg_counter: u32,
        keycode: u8,
        lfo_am: u8,
        channel_ams: u8,
        eg_tick: bool,
        eg_test: bool,
    ) -> i32 {
        if !Self::operator_active(op) {
            op.last_output = 0;
            return 0;
        }

        // Advance 20-bit phase accumulator directly at HW rate
        op.phase = op.phase.wrapping_add(phase_inc) & 0xFFFFF;

        // EG only advances on EG tick (every 3 HW samples)
        if eg_tick {
            // GenPlusGX: SSG-EG cycle processed before envelope advance.
            // Nuked OPN2: if cycle fires (phase→Attack), skip EG advance this cycle.
            let ssg_cycled = Self::advance_ssg_eg_cycle(op);
            if !ssg_cycled {
                Self::advance_envelope(op, eg_counter, keycode);
            }
        }

        // Nuked OPN2: test register bit 0 forces EG output to 0 (max volume)
        let eg_level = if eg_test {
            0
        } else {
            Self::ssg_eg_output_level(op)
        };
        let mut eg_out = eg_level as u32 + ((op.tl as u32) << 3);
        // AM modulation in 10-bit space
        if op.am_enable {
            let am_atten = match channel_ams {
                0 => (lfo_am as u32) >> 8, // Nuked OPN2: am_shift[0] = 8
                1 => (lfo_am as u32) >> 3,
                2 => (lfo_am as u32) >> 1,
                _ => lfo_am as u32,
            };
            eg_out += am_atten;
        }
        eg_out = eg_out.min(0x3FF); // 10-bit clamp
        let total_atten = eg_out << 2; // convert to 12-bit

        // Apply phase modulation to 20-bit phase, get 10-bit sine index
        let mod_shifted = (phase_mod as u32) << 10;
        let total_phase = op.phase.wrapping_add(mod_shifted);
        let sine_input = (total_phase >> 10) & 0x3FF;

        let sample = op_calc(sine_input, total_atten);
        op.last_output = sample;
        sample
    }

    pub fn writes(&self) -> u64 {
        self.writes
    }

    pub fn dac_data_writes(&self) -> u64 {
        self.dac_data_writes
    }

    pub fn active_channels(&self) -> usize {
        self.channels
            .iter()
            .enumerate()
            .filter(|(index, _)| {
                let dac_channel = *index == 5;
                !dac_channel || !self.dac_enabled
            })
            .filter(|(_, channel)| Self::channel_active(channel))
            .count()
    }

    fn render_channel_sample(
        channel: &mut YmChannel,
        eg_counter: u32,
        lfo_am: u8,
        lfo_pm_step: u8,
        lfo_pm_sign: bool,
        channel3_special_mode: bool,
        eg_tick: bool,
        eg_test: bool,
    ) -> i32 {
        // Compute phase increments and keycodes for each operator
        let mut op_phase_incs = [0u32; 4];
        let mut op_keycodes = [0u8; 4];
        for i in 0..4 {
            let (fnum, block) = Self::operator_fnum_block(channel, i, channel3_special_mode);
            let keycode = Self::block_fnum_keycode(block, fnum);
            op_keycodes[i] = keycode;

            // Apply PM to FNUM
            let pm_offset = Self::lfo_pm_displacement(fnum, channel.fms, lfo_pm_step, lfo_pm_sign);
            let modulated_fnum = ((fnum as i32) + pm_offset).clamp(0, 0x7FF) as u16;

            // At HW rate, raw phase inc is used directly (no scaling needed)
            op_phase_incs[i] = Self::compute_phase_inc(
                modulated_fnum,
                block,
                channel.operators[i].mul,
                channel.operators[i].detune,
                keycode,
            );
        }

        let alg = channel.algorithm & 0x07;
        let (connect1, connect2, connect3, mem_restore_to, special_alg5) = match alg {
            0 => (
                Some(YmAlgorithmBus::C1),
                Some(YmAlgorithmBus::Mem),
                Some(YmAlgorithmBus::C2),
                Some(YmAlgorithmBus::M2),
                false,
            ),
            1 => (
                Some(YmAlgorithmBus::Mem),
                Some(YmAlgorithmBus::Mem),
                Some(YmAlgorithmBus::C2),
                Some(YmAlgorithmBus::M2),
                false,
            ),
            2 => (
                Some(YmAlgorithmBus::C2),
                Some(YmAlgorithmBus::Mem),
                Some(YmAlgorithmBus::C2),
                Some(YmAlgorithmBus::M2),
                false,
            ),
            3 => (
                Some(YmAlgorithmBus::C1),
                Some(YmAlgorithmBus::Mem),
                Some(YmAlgorithmBus::C2),
                Some(YmAlgorithmBus::C2),
                false,
            ),
            4 => (
                Some(YmAlgorithmBus::C1),
                Some(YmAlgorithmBus::Out),
                Some(YmAlgorithmBus::C2),
                Some(YmAlgorithmBus::Mem),
                false,
            ),
            5 => (
                None,
                Some(YmAlgorithmBus::Out),
                Some(YmAlgorithmBus::Out),
                Some(YmAlgorithmBus::M2),
                true,
            ),
            6 => (
                Some(YmAlgorithmBus::C1),
                Some(YmAlgorithmBus::Out),
                Some(YmAlgorithmBus::Out),
                Some(YmAlgorithmBus::Mem),
                false,
            ),
            _ => (
                Some(YmAlgorithmBus::Out),
                Some(YmAlgorithmBus::Out),
                Some(YmAlgorithmBus::Out),
                Some(YmAlgorithmBus::Mem),
                false,
            ),
        };

        // GenPlusGX/MAME: buses are computed fresh each sample. Only mem (feedback_sample_prev)
        // has a 1-sample delay.
        let mut m2_bus = 0i32;
        let mut c1_bus = 0i32;
        let mut c2_bus = 0i32;
        let mut mem_bus = 0i32;
        let mut out_bus = 0i32;

        if let Some(destination) = mem_restore_to {
            Self::route_algorithm_bus(
                destination,
                channel.feedback_sample_prev,
                &mut m2_bus,
                &mut c1_bus,
                &mut c2_bus,
                &mut mem_bus,
                &mut out_bus,
            );
        }

        // OP1 feedback: Nuked OPN2: mod >> (10 - FB). FB=0 means no feedback.
        let op1_prev = channel.operators[0].last_output;
        let fb_phase_mod = if channel.feedback > 0 {
            let shift = (10 - channel.feedback.min(7)) as i32;
            (op1_prev + channel.feedback_sample) >> shift
        } else {
            0
        };

        let o1 = Self::advance_operator_sample(
            &mut channel.operators[0],
            op_phase_incs[0],
            fb_phase_mod,
            eg_counter,
            op_keycodes[0],
            lfo_am,
            channel.ams,
            eg_tick,
            eg_test,
        );
        channel.feedback_sample = op1_prev;

        // Route OP1 output to buses (immediate propagation within same sample)
        if special_alg5 {
            mem_bus += o1;
            c1_bus += o1;
            c2_bus += o1;
            m2_bus += o1;
        } else if let Some(destination) = connect1 {
            Self::route_algorithm_bus(
                destination,
                o1,
                &mut m2_bus,
                &mut c1_bus,
                &mut c2_bus,
                &mut mem_bus,
                &mut out_bus,
            );
        }

        // YM internal slot order is OP1 -> OP3 -> OP2 -> OP4.
        // Nuked OPN2: non-feedback operators apply >> 1 to modulation input
        let o3 = Self::advance_operator_sample(
            &mut channel.operators[2],
            op_phase_incs[2],
            m2_bus >> 1,
            eg_counter,
            op_keycodes[2],
            lfo_am,
            channel.ams,
            eg_tick,
            eg_test,
        );
        if let Some(destination) = connect3 {
            Self::route_algorithm_bus(
                destination,
                o3,
                &mut m2_bus,
                &mut c1_bus,
                &mut c2_bus,
                &mut mem_bus,
                &mut out_bus,
            );
        }

        let o2 = Self::advance_operator_sample(
            &mut channel.operators[1],
            op_phase_incs[1],
            c1_bus >> 1,
            eg_counter,
            op_keycodes[1],
            lfo_am,
            channel.ams,
            eg_tick,
            eg_test,
        );
        if let Some(destination) = connect2 {
            Self::route_algorithm_bus(
                destination,
                o2,
                &mut m2_bus,
                &mut c1_bus,
                &mut c2_bus,
                &mut mem_bus,
                &mut out_bus,
            );
        }

        let o4 = Self::advance_operator_sample(
            &mut channel.operators[3],
            op_phase_incs[3],
            c2_bus >> 1,
            eg_counter,
            op_keycodes[3],
            lfo_am,
            channel.ams,
            eg_tick,
            eg_test,
        );
        out_bus += o4;

        channel.feedback_sample_prev = mem_bus;
        // Nuked OPN2 / GenPlusGX: 14-bit channel output clipping
        out_bus.clamp(-8192, 8191)
    }

    fn route_algorithm_bus(
        destination: YmAlgorithmBus,
        sample: i32,
        m2_bus: &mut i32,
        c1_bus: &mut i32,
        c2_bus: &mut i32,
        mem_bus: &mut i32,
        out_bus: &mut i32,
    ) {
        match destination {
            YmAlgorithmBus::M2 => *m2_bus += sample,
            YmAlgorithmBus::C1 => *c1_bus += sample,
            YmAlgorithmBus::C2 => *c2_bus += sample,
            YmAlgorithmBus::Mem => *mem_bus += sample,
            YmAlgorithmBus::Out => *out_bus += sample,
        }
    }

    /// YM2612 9-bit DAC quantization: truncate lower 5 bits of the 14-bit
    /// accumulator value, simulating the real chip's DAC precision.
    fn dac_9bit_quantize(sample: i32) -> i32 {
        // Truncate toward zero: (sample / 32) * 32
        // This matches the hardware's bit truncation behavior.
        (sample >> 5) << 5
    }

    /// Render one HW sample at the internal 53267 Hz rate.
    fn render_one_hw_sample(&mut self) -> (i16, i16) {
        let (lfo_am, lfo_pm_step, lfo_pm_sign) = self.advance_lfo_hw();
        let eg_tick = self.advance_eg_counter_hw();
        let eg_counter = self.eg_counter;
        let eg_test = (self.test_register & 0x01) != 0;

        let mut left_sum = 0i32;
        let mut right_sum = 0i32;
        let channel3_special_mode = self.channel3_special_mode_enabled();
        if self.csm_key_on_active {
            self.csm_key_on_rendered = true;
        }

        for (index, channel) in self.channels.iter_mut().enumerate() {
            if index == 5 && self.dac_enabled {
                continue;
            }
            if !Self::channel_active(channel) {
                channel.feedback_sample = 0;
                channel.feedback_sample_prev = 0;
                continue;
            }
            let sample = Self::render_channel_sample(
                channel,
                eg_counter,
                lfo_am,
                lfo_pm_step,
                lfo_pm_sign,
                channel3_special_mode && index == 2,
                eg_tick,
                eg_test,
            );
            if channel.pan_left {
                left_sum += sample;
            }
            if channel.pan_right {
                right_sum += sample;
            }
        }

        // Add DAC output to the same 14-bit sum as FM channels
        if self.dac_enabled {
            let channel = &mut self.channels[5];
            let dac_sample = if channel.feedback_sample_prev > 0 {
                channel.feedback_sample / channel.feedback_sample_prev
            } else {
                self.dac_output as i32
            };
            channel.feedback_sample = 0;
            channel.feedback_sample_prev = 0;
            let dac_14bit = dac_sample.clamp(-8192, 8191);
            if channel.pan_left {
                left_sum += dac_14bit;
            }
            if channel.pan_right {
                right_sum += dac_14bit;
            }
        }

        // YM2612 9-bit DAC: the accumulated sum is converted through a 9-bit
        // (sign + 8 magnitude) DAC, truncating the lower 5 bits of the 14-bit value.
        // This produces the characteristic quantization noise of real hardware.
        let left_dac = Self::dac_9bit_quantize(left_sum);
        let right_dac = Self::dac_9bit_quantize(right_sum);

        // Scale quantized output to i16 range.
        let left = ((left_dac as i64 * 18000) >> 13).clamp(i16::MIN as i64, i16::MAX as i64) as i16;
        let right =
            ((right_dac as i64 * 18000) >> 13).clamp(i16::MIN as i64, i16::MAX as i64) as i16;
        (left, right)
    }

    /// Produce one output sample at the requested sample rate by running the
    /// internal HW clock at 53267 Hz (zero-order hold downsampling).
    /// Uses the most recent HW sample, avoiding box-filter averaging artifacts
    /// from variable sample counts (1 or 2 HW samples per output sample).
    pub(super) fn next_sample_stereo(&mut self, sample_rate_hz: u32) -> (i16, i16) {
        let hw_rate = YM_HW_RATE as u32;
        self.hw_sample_frac += hw_rate;

        while self.hw_sample_frac >= sample_rate_hz {
            self.hw_sample_frac -= sample_rate_hz;
            self.last_hw_output = self.render_one_hw_sample();
        }

        self.last_hw_output
    }

    pub(super) fn step_z80_cycles(&mut self, cycles: u32) {
        if self.csm_key_on_active && self.csm_key_on_rendered {
            self.trigger_csm_channel3_key_off();
            self.csm_key_on_active = false;
            self.csm_key_on_rendered = false;
        }
        self.busy_z80_cycles = self.busy_z80_cycles.saturating_sub(cycles);
        if cycles > 0 {
            let pending_enabled = self.dac_enabled_pending.take();
            let pending_output = self
                .dac_output_pending
                .take()
                .map(Self::decode_pending_dac_output);
            let pending_count = pending_enabled.is_some() as u8 + pending_output.is_some() as u8;
            if pending_count == 0 {
                self.accumulate_dac_cycles(cycles);
            } else {
                let total_cycles = cycles as u64;
                let mut stage_index = 0u64;
                let stage_count = pending_count as u64 + 1;
                let mut consumed_cycles = 0u32;
                match (pending_enabled, pending_output) {
                    (Some(enabled), Some((output, output_after_enable))) => {
                        if output_after_enable {
                            stage_index += 1;
                            let boundary = ((stage_index * total_cycles) / stage_count) as u32;
                            let span = boundary.saturating_sub(consumed_cycles);
                            self.accumulate_dac_cycles(span);
                            consumed_cycles = boundary;
                            self.set_dac_enabled(enabled);

                            stage_index += 1;
                            let boundary = ((stage_index * total_cycles) / stage_count) as u32;
                            let span = boundary.saturating_sub(consumed_cycles);
                            self.accumulate_dac_cycles(span);
                            consumed_cycles = boundary;
                            self.dac_output = output;
                        } else {
                            stage_index += 1;
                            let boundary = ((stage_index * total_cycles) / stage_count) as u32;
                            let span = boundary.saturating_sub(consumed_cycles);
                            self.accumulate_dac_cycles(span);
                            consumed_cycles = boundary;
                            self.dac_output = output;

                            stage_index += 1;
                            let boundary = ((stage_index * total_cycles) / stage_count) as u32;
                            let span = boundary.saturating_sub(consumed_cycles);
                            self.accumulate_dac_cycles(span);
                            consumed_cycles = boundary;
                            self.set_dac_enabled(enabled);
                        }
                    }
                    (Some(enabled), None) => {
                        stage_index += 1;
                        let boundary = ((stage_index * total_cycles) / stage_count) as u32;
                        let span = boundary.saturating_sub(consumed_cycles);
                        self.accumulate_dac_cycles(span);
                        consumed_cycles = boundary;
                        self.set_dac_enabled(enabled);
                    }
                    (None, Some((output, _))) => {
                        stage_index += 1;
                        let boundary = ((stage_index * total_cycles) / stage_count) as u32;
                        let span = boundary.saturating_sub(consumed_cycles);
                        self.accumulate_dac_cycles(span);
                        consumed_cycles = boundary;
                        self.dac_output = output;
                    }
                    (None, None) => {}
                }
                self.accumulate_dac_cycles(cycles.saturating_sub(consumed_cycles));
            }
        }

        let ym_cycle_divisor = Self::Z80_CLOCK_HZ * Self::YM2612_DIVIDER;
        self.timer_clock_accumulator += (cycles as u64) * Self::MASTER_CLOCK_HZ;
        let ym_cycles = self.timer_clock_accumulator / ym_cycle_divisor;
        self.timer_clock_accumulator %= ym_cycle_divisor;
        if ym_cycles == 0 {
            return;
        }

        if (self.timer_control & 0x01) != 0 {
            let period = self.timer_a_period_ym_cycles();
            self.timer_a_elapsed_ym_cycles += ym_cycles;
            while self.timer_a_elapsed_ym_cycles >= period {
                self.timer_a_elapsed_ym_cycles -= period;
                if (self.timer_control & 0x04) != 0 {
                    self.timer_status |= 0x01;
                }
                if self.csm_mode_enabled() {
                    self.trigger_csm_channel3_key_on();
                    self.csm_key_on_active = true;
                    self.csm_key_on_rendered = false;
                }
            }
        }
        if (self.timer_control & 0x02) != 0 {
            let period = self.timer_b_period_ym_cycles();
            self.timer_b_elapsed_ym_cycles += ym_cycles;
            while self.timer_b_elapsed_ym_cycles >= period {
                self.timer_b_elapsed_ym_cycles -= period;
                if (self.timer_control & 0x08) != 0 {
                    self.timer_status |= 0x02;
                }
            }
        }
    }

    fn csm_mode_enabled(&self) -> bool {
        self.channel3_mode_bits() == 0b10
    }

    fn channel3_mode_bits(&self) -> u8 {
        (self.timer_control >> 6) & 0x03
    }

    fn trigger_csm_channel3_key_on(&mut self) {
        let ch3 = &mut self.channels[2];
        ch3.feedback_sample = 0;
        ch3.feedback_sample_prev = 0;
        for op in &mut ch3.operators {
            let was_on = op.key_on;
            op.csm_key_on = true;
            op.key_on = true; // composite = reg_key_on || csm_key_on
            if !was_on {
                op.phase = 0;
                op.last_output = 0;
                op.envelope_phase = YmEnvelopePhase::Attack;
                if Self::ssg_eg_enabled(op) && (op.envelope_level & 0x200) != 0 {
                    op.envelope_level = (0x200u16.wrapping_sub(op.envelope_level)) & 0x3FF;
                }
                op.ssg_invert = false;
                op.ssg_hold_active = false;
            }
        }
    }

    fn trigger_csm_channel3_key_off(&mut self) {
        let ch3 = &mut self.channels[2];
        for op in &mut ch3.operators {
            if !op.csm_key_on {
                continue;
            }
            op.csm_key_on = false;
            let new_key_on = op.reg_key_on; // reg 0x28 takes priority
            if !new_key_on && op.key_on {
                // Transition to key-off: apply SSG-EG normalization + Release
                if Self::ssg_eg_enabled(op) && (op.envelope_level & 0x200) != 0 {
                    op.envelope_level = (0x200u16.wrapping_sub(op.envelope_level)) & 0x3FF;
                }
                op.ssg_invert = false;
                op.envelope_phase = if op.envelope_level < 1023 {
                    YmEnvelopePhase::Release
                } else {
                    YmEnvelopePhase::Off
                };
                op.ssg_hold_active = false;
            }
            op.key_on = new_key_on;
        }
    }

    fn accumulate_dac_cycles(&mut self, cycles: u32) {
        if !self.dac_enabled || cycles == 0 {
            return;
        }
        let channel = &mut self.channels[5];
        channel.feedback_sample += self.dac_output as i32 * cycles as i32;
        channel.feedback_sample_prev += cycles as i32;
    }

    fn set_dac_enabled(&mut self, enabled: bool) {
        let was_enabled = self.dac_enabled;
        self.dac_enabled = enabled;
        if was_enabled != self.dac_enabled {
            self.channels[5].feedback_sample = 0;
            self.channels[5].feedback_sample_prev = 0;
        }
    }

    fn encode_pending_dac_output(output: i16, output_after_enable: bool) -> i16 {
        (output & !Self::DAC_PENDING_ORDER_MASK)
            | if output_after_enable {
                Self::DAC_PENDING_ORDER_MASK
            } else {
                0
            }
    }

    fn decode_pending_dac_output(tagged_output: i16) -> (i16, bool) {
        let output_after_enable = (tagged_output & Self::DAC_PENDING_ORDER_MASK) != 0;
        let output = tagged_output & !Self::DAC_PENDING_ORDER_MASK;
        (output, output_after_enable)
    }

    pub(super) fn read_status(&self) -> u8 {
        let mut status = self.timer_status & 0x03;
        if self.busy_z80_cycles > 0 {
            status |= 0x80;
        }
        status
    }

    fn timer_a_period_ym_cycles(&self) -> u64 {
        let value = (self.timer_a_value & 0x03FF) as u64;
        (1024 - value).max(1) * 18
    }

    fn timer_b_period_ym_cycles(&self) -> u64 {
        let value = self.timer_b_value as u64;
        (256 - value).max(1) * 288
    }

    pub fn register(&self, bank: usize, index: u8) -> u8 {
        self.regs[bank & 1][index as usize]
    }

    pub fn dac_enabled(&self) -> bool {
        self.dac_enabled
    }

    pub fn channel_key_on(&self, channel: usize) -> bool {
        self.channels[channel.min(5)]
            .operators
            .iter()
            .any(|op| op.key_on)
    }

    pub fn channel_operator_key_on(&self, channel: usize, operator: usize) -> bool {
        let channel = self.channels[channel.min(5)];
        channel.operators[operator.min(3)].key_on
    }

    pub fn lfo_enabled(&self) -> bool {
        self.lfo_enabled
    }

    pub fn lfo_rate(&self) -> u8 {
        self.lfo_rate
    }

    pub fn channel_frequency_hz_debug(&self, channel: usize) -> f32 {
        self.channel_operator_frequency_hz_debug(channel, 3)
    }

    pub fn channel_operator_frequency_hz_debug(&self, channel: usize, operator: usize) -> f32 {
        let channel_index = channel.min(5);
        let operator_index = operator.min(3);
        let ch = &self.channels[channel_index];
        let op = &ch.operators[operator_index];
        let is_ch3_special = channel_index == 2 && self.channel3_special_mode_enabled();
        let (fnum, block) = Self::operator_fnum_block(ch, operator_index, is_ch3_special);
        let keycode = Self::block_fnum_keycode(block, fnum);
        let raw_inc = Self::compute_phase_inc(fnum, block, op.mul, op.detune, keycode);
        // Frequency = phase_inc * HW_RATE / 2^20
        const HW_RATE_F: f32 = 7_670_454.0 / 144.0;
        raw_inc as f32 * HW_RATE_F / 1_048_576.0
    }

    pub fn channel_carrier_mul(&self, channel: usize) -> u8 {
        self.channels[channel.min(5)].operators[3].mul
    }

    pub fn channel_carrier_detune(&self, channel: usize) -> u8 {
        self.channels[channel.min(5)].operators[3].detune
    }

    pub fn channel_carrier_tl(&self, channel: usize) -> u8 {
        self.channels[channel.min(5)].operators[3].tl
    }

    pub fn channel_carrier_ssg_eg(&self, channel: usize) -> u8 {
        self.channels[channel.min(5)].operators[3].ssg_eg
    }

    pub fn channel_algorithm_feedback(&self, channel: usize) -> (u8, u8) {
        let channel = self.channels[channel.min(5)];
        (channel.algorithm, channel.feedback)
    }

    pub fn channel_ams_fms(&self, channel: usize) -> (u8, u8) {
        let channel = self.channels[channel.min(5)];
        (channel.ams, channel.fms)
    }

    pub fn channel_envelope_level(&self, channel: usize) -> f32 {
        let level = self.channels[channel.min(5)].operators[3].envelope_level;
        // Convert integer attenuation (0=max, 1023=silence) to float (1.0=max, 0.0=silence)
        1.0 - (level as f32 / 1023.0)
    }

    pub fn channel_envelope_params(&self, channel: usize) -> (u8, u8, u8, u8, u8) {
        let op = self.channels[channel.min(5)].operators[3];
        (
            op.attack_rate,
            op.decay_rate,
            op.sustain_rate,
            op.sustain_level,
            op.release_rate,
        )
    }

    pub fn channel_block_and_fnum(&self, channel: usize) -> (u8, u16) {
        let channel = self.channels[channel.min(5)];
        (channel.block, channel.fnum)
    }
}
