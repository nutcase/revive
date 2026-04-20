use megadrive_core::audio::AudioBus;

#[test]
fn writes_ym2612_registers_via_address_and_data_ports() {
    let mut audio = AudioBus::new();

    audio.write_ym2612(0, 0x22);
    audio.write_ym2612(1, 0x0F);
    audio.write_ym2612(2, 0x2B);
    audio.write_ym2612(3, 0x80);

    assert_eq!(audio.ym2612().register(0, 0x22), 0x0F);
    assert_eq!(audio.ym2612().register(1, 0x2B), 0x80);
}

#[test]
fn ym2612_status_reports_busy_for_short_window_after_data_write() {
    let mut audio = AudioBus::new();
    audio.write_ym2612(0, 0x22);
    audio.write_ym2612(1, 0x0F);
    assert_eq!(audio.read_ym2612(0) & 0x80, 0x80);

    audio.step_z80_cycles(8);
    assert_eq!(audio.read_ym2612(0) & 0x80, 0x80);

    let mut cleared = false;
    for _ in 0..256 {
        audio.step_z80_cycles(1);
        if (audio.read_ym2612(0) & 0x80) == 0 {
            cleared = true;
            break;
        }
    }
    assert!(cleared, "busy flag should eventually clear");
}

#[test]
fn ym2612_status_reports_busy_after_address_write() {
    let mut audio = AudioBus::new();
    audio.write_ym2612(0, 0x22);
    assert_eq!(audio.read_ym2612(0) & 0x80, 0x80);

    let mut cleared = false;
    for _ in 0..256 {
        audio.step_z80_cycles(1);
        if (audio.read_ym2612(0) & 0x80) == 0 {
            cleared = true;
            break;
        }
    }
    assert!(cleared, "busy flag should clear after advancing YM time");
}

#[test]
fn ym2612_z80_data_write_is_not_dropped_while_busy() {
    let mut audio = AudioBus::new();

    audio.write_ym2612_from_z80(0, 0x22);
    // Keep BUSY asserted and immediately write data from Z80 side.
    assert_eq!(audio.read_ym2612(0) & 0x80, 0x80);
    audio.write_ym2612_from_z80(1, 0x0F);

    assert_eq!(audio.ym2612().register(0, 0x22), 0x0F);
}

#[test]
fn ym2612_timer_a_sets_status_bit0_when_enabled() {
    let mut audio = AudioBus::new();
    // Timer A = 1023 => shortest period in this model.
    audio.write_ym2612(0, 0x24);
    audio.write_ym2612(1, 0xFF);
    audio.write_ym2612(0, 0x25);
    audio.write_ym2612(1, 0x03);
    // Load + enable timer A.
    audio.write_ym2612(0, 0x27);
    audio.write_ym2612(1, 0x05);

    // Advance enough Z80 cycles to overflow timer A at least once.
    audio.step_z80_cycles(80);
    assert_ne!(audio.read_ym2612(0) & 0x01, 0);

    // Reset timer A status bit.
    audio.write_ym2612(0, 0x27);
    audio.write_ym2612(1, 0x15);
    assert_eq!(audio.read_ym2612(0) & 0x01, 0);
}

#[test]
fn ym2612_timer_a_reload_bit_restarts_counter_even_when_already_running() {
    let mut audio = AudioBus::new();
    // Timer A = 1023 => shortest period in this model.
    audio.write_ym2612(0, 0x24);
    audio.write_ym2612(1, 0xFF);
    audio.write_ym2612(0, 0x25);
    audio.write_ym2612(1, 0x03);
    // Load + enable timer A.
    audio.write_ym2612(0, 0x27);
    audio.write_ym2612(1, 0x05);

    // Not enough to overflow once yet.
    audio.step_z80_cycles(40);
    assert_eq!(audio.read_ym2612(0) & 0x01, 0);

    // Writing load bit again should reload/restart the counter.
    audio.write_ym2612(0, 0x27);
    audio.write_ym2612(1, 0x05);
    audio.step_z80_cycles(40);
    assert_eq!(
        audio.read_ym2612(0) & 0x01,
        0,
        "timer A should not overflow immediately after explicit reload"
    );

    audio.step_z80_cycles(40);
    assert_ne!(audio.read_ym2612(0) & 0x01, 0);
}

#[test]
fn ym2612_timer_b_sets_status_bit1_when_enabled() {
    let mut audio = AudioBus::new();
    // Timer B = 255 => shortest period in this model.
    audio.write_ym2612(0, 0x26);
    audio.write_ym2612(1, 0xFF);
    // Load + enable timer B.
    audio.write_ym2612(0, 0x27);
    audio.write_ym2612(1, 0x0A);

    // Advance enough Z80 cycles to overflow timer B at least once.
    audio.step_z80_cycles(1_200);
    assert_ne!(audio.read_ym2612(0) & 0x02, 0);

    // Reset timer B status bit.
    audio.write_ym2612(0, 0x27);
    audio.write_ym2612(1, 0x20);
    assert_eq!(audio.read_ym2612(0) & 0x02, 0);
}

#[test]
fn ym2612_timer_b_reload_bit_restarts_counter_even_when_already_running() {
    let mut audio = AudioBus::new();
    // Timer B = 255 => shortest period in this model.
    audio.write_ym2612(0, 0x26);
    audio.write_ym2612(1, 0xFF);
    // Load + enable timer B.
    audio.write_ym2612(0, 0x27);
    audio.write_ym2612(1, 0x0A);

    // Not enough to overflow once yet.
    audio.step_z80_cycles(450);
    assert_eq!(audio.read_ym2612(0) & 0x02, 0);

    // Writing load bit again should reload/restart the counter.
    audio.write_ym2612(0, 0x27);
    audio.write_ym2612(1, 0x0A);
    audio.step_z80_cycles(450);
    assert_eq!(
        audio.read_ym2612(0) & 0x02,
        0,
        "timer B should not overflow immediately after explicit reload"
    );

    audio.step_z80_cycles(700);
    assert_ne!(audio.read_ym2612(0) & 0x02, 0);
}

#[test]
fn captures_psg_writes() {
    let mut audio = AudioBus::new();
    audio.write_psg(0x9F);
    assert_eq!(audio.psg().last_data(), 0x9F);
    assert_eq!(audio.psg().writes(), 1);
}

#[test]
fn generates_silence_samples_without_psg_writes() {
    let mut audio = AudioBus::new();
    audio.step(2_000);
    assert!(audio.pending_samples() > 0);
    let samples = audio.drain_samples(64);
    assert!(samples.iter().all(|&s| s == 0));
}

#[test]
fn generates_nonzero_samples_after_psg_write() {
    let mut audio = AudioBus::new();
    // Use a normal tone period (>1) so the PSG toggles between polarities.
    audio.write_psg(0x80);
    audio.write_psg(0x02);
    audio.write_psg(0x90); // low attenuation -> larger amplitude
    audio.step(2_000);

    let samples = audio.drain_samples(64);
    assert!(!samples.is_empty());
    assert!(samples.iter().any(|&s| s > 0));
    assert!(samples.iter().any(|&s| s < 0));
}

#[test]
fn psg_latch_and_data_bytes_update_tone_period() {
    let mut audio = AudioBus::new();
    // Latch tone 0 low nibble = 0xA.
    audio.write_psg(0x8A);
    // Data byte sets high bits = 0x12.
    audio.write_psg(0x12);
    assert_eq!(audio.psg().tone_period(0), 0x12A);
}

#[test]
fn psg_noise_latch_updates_control_register() {
    let mut audio = AudioBus::new();
    // Latch noise control: white noise + clock mode 2.
    audio.write_psg(0xE6);
    assert_eq!(audio.psg().noise_control(), 0x06);
}

#[test]
fn ym2612_dac_outputs_pcm_when_enabled() {
    let mut audio = AudioBus::new();
    audio.write_ym2612(0, 0x2B);
    audio.write_ym2612(1, 0x80);
    audio.write_ym2612(0, 0x2A);
    audio.write_ym2612(1, 0xFF);
    audio.step(2_000);

    let samples = audio.drain_samples(64);
    assert!(!samples.is_empty());
    assert!(audio.ym2612().dac_enabled());
    assert!(samples.iter().all(|&s| s > 0));
}

#[test]
fn ym2612_dac_averages_subsample_write_timing() {
    let mut audio = AudioBus::new();
    audio.write_ym2612_from_z80(0, 0x2B);
    audio.write_ym2612_from_z80(1, 0x80);
    audio.step_z80_cycles(1);

    // Two different DAC levels inside one host sample interval.
    audio.write_ym2612_from_z80(0, 0x2A);
    audio.write_ym2612_from_z80(1, 0x00);
    audio.step_z80_cycles(2);
    audio.write_ym2612_from_z80(0, 0x2A);
    audio.write_ym2612_from_z80(1, 0xFF);
    audio.step_z80_cycles(2);

    // Exactly one 44.1kHz output frame worth of 68k time.
    audio.step(174);
    let samples = audio.drain_samples(2);
    assert_eq!(samples.len(), 2);
    // Without timing-aware DAC accumulation this would be near +8128.
    assert!(samples[0].abs() < 7000, "left={}", samples[0]);
    assert!(samples[1].abs() < 7000, "right={}", samples[1]);
}

#[test]
fn ym2612_dac_write_applies_after_elapsed_z80_slice() {
    let mut audio = AudioBus::new();
    audio.write_ym2612_from_z80(0, 0x2B);
    audio.write_ym2612_from_z80(1, 0x80);
    audio.step_z80_cycles(1);

    // Write max DAC value, but do not advance Z80 yet.
    audio.write_ym2612_from_z80(0, 0x2A);
    audio.write_ym2612_from_z80(1, 0xFF);
    // One output sample before any elapsed Z80 time should still be near silence.
    audio.step(174);
    let before = audio.drain_samples(2);
    assert_eq!(before.len(), 2);
    assert!(before[0].abs() < 64, "left_before={}", before[0]);
    assert!(before[1].abs() < 64, "right_before={}", before[1]);

    // First elapsed slice latches pending value; second slice accumulates it.
    audio.step_z80_cycles(4);
    audio.step_z80_cycles(4);
    audio.step(174);
    let after = audio.drain_samples(2);
    assert_eq!(after.len(), 2);
    assert!(after[0] > 1000, "left_after={}", after[0]);
    assert!(after[1] > 1000, "right_after={}", after[1]);
}

#[test]
fn ym2612_dac_write_contributes_within_elapsed_z80_slice() {
    let mut audio = AudioBus::new();
    audio.write_ym2612(0, 0x2B);
    audio.write_ym2612(1, 0x80);
    audio.write_ym2612(0, 0x2A);
    audio.write_ym2612(1, 0x80);

    // Clear any initial buffered output.
    audio.step(174);
    let _ = audio.drain_samples(2);

    // Z80-side DAC data write should contribute to this elapsed slice.
    audio.write_ym2612_from_z80(0, 0x2A);
    audio.write_ym2612_from_z80(1, 0xFF);
    audio.step_z80_cycles(8);
    audio.step(174);
    let samples = audio.drain_samples(2);
    assert_eq!(samples.len(), 2);
    assert!(samples[0] > 500, "left={}", samples[0]);
    assert!(samples[1] > 500, "right={}", samples[1]);
}

#[test]
fn ym2612_dac_pending_write_order_preserved_with_enable_toggle() {
    let mut audio = AudioBus::new();
    // Prime DAC output with a negative sample while DAC is disabled.
    audio.write_ym2612(0, 0x2A);
    audio.write_ym2612(1, 0x00);

    // Same Z80 slice: write new output first, then enable DAC.
    audio.write_ym2612_from_z80(0, 0x2A);
    audio.write_ym2612_from_z80(1, 0xFF);
    audio.write_ym2612_from_z80(0, 0x2B);
    audio.write_ym2612_from_z80(1, 0x80);
    audio.step_z80_cycles(16);
    audio.step(174);

    let samples = audio.drain_samples(2);
    assert_eq!(samples.len(), 2);
    // If ordering is reversed internally, stale negative DAC output leaks first.
    assert!(samples[0] > 0, "left={}", samples[0]);
    assert!(samples[1] > 0, "right={}", samples[1]);
}

#[test]
fn ym2612_dac_is_silent_when_disabled() {
    let mut audio = AudioBus::new();
    audio.write_ym2612(0, 0x2A);
    audio.write_ym2612(1, 0xFF);
    audio.step(2_000);

    let samples = audio.drain_samples(64);
    assert!(!samples.is_empty());
    assert!(samples.iter().all(|&s| s == 0));
}

#[test]
fn ym2612_dac_respects_channel6_pan() {
    let mut audio = AudioBus::new();
    // CH6 pan: left only (bank1 reg B6).
    audio.write_ym2612(2, 0xB6);
    audio.write_ym2612(3, 0x80);
    audio.write_ym2612(0, 0x2B);
    audio.write_ym2612(1, 0x80);
    audio.write_ym2612(0, 0x2A);
    audio.write_ym2612(1, 0xFF);
    audio.step(2_000);

    let samples = audio.drain_samples(128);
    assert!(!samples.is_empty());
    let mut left_nonzero = false;
    let mut right_nonzero = false;
    for pair in samples.chunks_exact(2) {
        if pair[0] != 0 {
            left_nonzero = true;
        }
        if pair[1] != 0 {
            right_nonzero = true;
        }
    }
    assert!(left_nonzero);
    assert!(!right_nonzero);
}

#[test]
fn ym2612_key_on_generates_nonzero_without_dac() {
    let mut audio = AudioBus::new();
    // CH1 FNUM/BLOCK
    audio.write_ym2612(0, 0xA0);
    audio.write_ym2612(1, 0x98);
    audio.write_ym2612(0, 0xA4);
    audio.write_ym2612(1, 0x22);
    // Key on CH1
    audio.write_ym2612(0, 0x28);
    audio.write_ym2612(1, 0xF0);
    audio.step(2_000);

    let samples = audio.drain_samples(128);
    assert!(!samples.is_empty());
    assert!(audio.ym2612().channel_key_on(0));
    assert!(samples.iter().any(|&s| s != 0));
}

#[test]
fn ym2612_key_on_accepts_any_slot_bits_in_simplified_model() {
    let mut audio = AudioBus::new();
    // CH1 FNUM/BLOCK
    audio.write_ym2612(0, 0xA0);
    audio.write_ym2612(1, 0x98);
    audio.write_ym2612(0, 0xA4);
    audio.write_ym2612(1, 0x22);

    // Set CH1 to algorithm 7 (all slots can contribute).
    audio.write_ym2612(0, 0xB0);
    audio.write_ym2612(1, 0x07);

    // Set only non-slot4 bits (0x70), channel 1.
    audio.write_ym2612(0, 0x28);
    audio.write_ym2612(1, 0x70);
    audio.step(2_000);

    let audible = audio.drain_samples(128);
    assert!(audio.ym2612().channel_key_on(0));
    assert!(audible.iter().any(|&s| s != 0));
}

#[test]
fn ym2612_key_on_bit_mapping_matches_operator_order() {
    let mut audio = AudioBus::new();
    // CH1 algorithm 7 => all operators can reach output.
    audio.write_ym2612(0, 0xB0);
    audio.write_ym2612(1, 0x07);
    // CH1 pitch
    audio.write_ym2612(0, 0xA0);
    audio.write_ym2612(1, 0x98);
    audio.write_ym2612(0, 0xA4);
    audio.write_ym2612(1, 0x22);

    // b5 only => OP2 only.
    audio.write_ym2612(0, 0x28);
    audio.write_ym2612(1, 0x20);

    assert!(audio.ym2612().channel_operator_key_on(0, 1)); // OP2
    assert!(!audio.ym2612().channel_operator_key_on(0, 2)); // OP3
    assert!(!audio.ym2612().channel_operator_key_on(0, 0)); // OP1
    assert!(!audio.ym2612().channel_operator_key_on(0, 3)); // OP4
}

#[test]
fn ym2612_repeated_key_on_while_held_does_not_retrigger_phase() {
    let mut audio = AudioBus::new();
    // CH1 setup.
    audio.write_ym2612(0, 0xA0);
    audio.write_ym2612(1, 0x98);
    audio.write_ym2612(0, 0xA4);
    audio.write_ym2612(1, 0x22);
    audio.write_ym2612(0, 0x4C);
    audio.write_ym2612(1, 0x00);
    // Slow-ish attack so envelope value is measurably >0 before retrigger.
    audio.write_ym2612(0, 0x5C);
    audio.write_ym2612(1, 0x08);

    // First key on.
    audio.write_ym2612(0, 0x28);
    audio.write_ym2612(1, 0xF0);
    audio.step(3_000);
    let before = audio.ym2612().channel_envelope_level(0);
    assert!(before > 0.0);

    // Repeated key-on while already held should not forcibly retrigger.
    audio.write_ym2612(0, 0x28);
    audio.write_ym2612(1, 0xF0);
    let after = audio.ym2612().channel_envelope_level(0);
    assert!(audio.ym2612().channel_key_on(0));
    assert!(
        after >= before,
        "expected no retrigger: before={before} after={after}"
    );
}

#[test]
fn ym2612_csm_mode_retriggers_channel3_from_timer_a_overflow() {
    let mut baseline = AudioBus::new();
    // CH3 (channel index 2) base pitch and audible carrier level.
    baseline.write_ym2612(0, 0xA2);
    baseline.write_ym2612(1, 0x98);
    baseline.write_ym2612(0, 0xA6);
    baseline.write_ym2612(1, 0x22);
    baseline.write_ym2612(0, 0x4E);
    baseline.write_ym2612(1, 0x00);
    // Timer A = shortest period.
    baseline.write_ym2612(0, 0x24);
    baseline.write_ym2612(1, 0xFF);
    baseline.write_ym2612(0, 0x25);
    baseline.write_ym2612(1, 0x03);
    // Load + enable timer A (no CSM).
    baseline.write_ym2612(0, 0x27);
    baseline.write_ym2612(1, 0x05);
    baseline.step_z80_cycles(400);
    baseline.step(4_000);
    let silent = baseline.drain_samples(256);
    assert!(!baseline.ym2612().channel_key_on(2));
    assert!(silent.iter().all(|&s| s == 0));

    let mut csm = AudioBus::new();
    csm.write_ym2612(0, 0xA2);
    csm.write_ym2612(1, 0x98);
    csm.write_ym2612(0, 0xA6);
    csm.write_ym2612(1, 0x22);
    csm.write_ym2612(0, 0x4E);
    csm.write_ym2612(1, 0x00);
    csm.write_ym2612(0, 0x24);
    csm.write_ym2612(1, 0xFF);
    csm.write_ym2612(0, 0x25);
    csm.write_ym2612(1, 0x03);
    // Load + enable timer A + CSM mode.
    csm.write_ym2612(0, 0x27);
    csm.write_ym2612(1, 0x85);
    csm.step_z80_cycles(400);
    csm.step(4_000);
    let audible = csm.drain_samples(256);
    assert!(csm.ym2612().channel_key_on(2));
    assert!(audible.iter().any(|&s| s != 0));
}

#[test]
fn ym2612_channel3_mode_11_does_not_enable_csm_autokey() {
    let mut audio = AudioBus::new();
    // CH3 (channel index 2) pitch and carrier level.
    audio.write_ym2612(0, 0xA2);
    audio.write_ym2612(1, 0x98);
    audio.write_ym2612(0, 0xA6);
    audio.write_ym2612(1, 0x22);
    audio.write_ym2612(0, 0x4E);
    audio.write_ym2612(1, 0x00);
    // Timer A = shortest period.
    audio.write_ym2612(0, 0x24);
    audio.write_ym2612(1, 0xFF);
    audio.write_ym2612(0, 0x25);
    audio.write_ym2612(1, 0x03);
    // mode=11b should not behave as CSM auto-key in this core.
    // (0xC0 mode bits + timer A load/enable/flag-enable bits 0x05)
    audio.write_ym2612(0, 0x27);
    audio.write_ym2612(1, 0xC5);

    audio.step_z80_cycles(400);
    audio.step(4_000);
    let samples = audio.drain_samples(256);
    assert!(!audio.ym2612().channel_key_on(2));
    assert!(samples.iter().all(|&s| s == 0));
}

#[test]
fn ym2612_channel3_special_mode_uses_ym3438_slot_mapping() {
    let mut audio = AudioBus::new();
    // CH3 base pitch via A2/A6 (used by OP4 in special mode).
    audio.write_ym2612(0, 0xA2);
    audio.write_ym2612(1, 0x00);
    audio.write_ym2612(0, 0xA6);
    audio.write_ym2612(1, 0x22);

    // CH3 special frequencies:
    // OP3 <- A8/AC, OP1 <- A9/AD, OP2 <- AA/AE.
    // Choose clearly different values so mapping mistakes are visible.
    audio.write_ym2612(0, 0xA8);
    audio.write_ym2612(1, 0x80);
    audio.write_ym2612(0, 0xAC);
    audio.write_ym2612(1, 0x10);

    audio.write_ym2612(0, 0xA9);
    audio.write_ym2612(1, 0x00);
    audio.write_ym2612(0, 0xAD);
    audio.write_ym2612(1, 0x21);

    audio.write_ym2612(0, 0xAA);
    audio.write_ym2612(1, 0x80);
    audio.write_ym2612(0, 0xAE);
    audio.write_ym2612(1, 0x29);

    let normal_op1 = audio.ym2612().channel_operator_frequency_hz_debug(2, 0);
    let normal_op2 = audio.ym2612().channel_operator_frequency_hz_debug(2, 1);
    let normal_op3 = audio.ym2612().channel_operator_frequency_hz_debug(2, 2);
    let normal_op4 = audio.ym2612().channel_operator_frequency_hz_debug(2, 3);

    // Enable CH3 special mode (bit 6).
    audio.write_ym2612(0, 0x27);
    audio.write_ym2612(1, 0x40);

    let special_op1 = audio.ym2612().channel_operator_frequency_hz_debug(2, 0);
    let special_op2 = audio.ym2612().channel_operator_frequency_hz_debug(2, 1);
    let special_op3 = audio.ym2612().channel_operator_frequency_hz_debug(2, 2);
    let special_op4 = audio.ym2612().channel_operator_frequency_hz_debug(2, 3);

    // OP4 keeps base A2/A6 frequency in special mode.
    assert!((special_op4 / normal_op4 - 1.0).abs() < 0.01);
    // OP1-3 should now diverge from base frequency and from each other.
    assert!((special_op1 / normal_op1 - 1.0).abs() > 0.20);
    assert!((special_op2 / normal_op2 - 1.0).abs() > 0.20);
    assert!((special_op3 / normal_op3 - 1.0).abs() > 0.20);
    assert!(special_op3 < special_op1);
    assert!(special_op1 < special_op2);

    // Disabling special mode returns all operators to base pitch.
    audio.write_ym2612(0, 0x27);
    audio.write_ym2612(1, 0x00);
    let restored_op1 = audio.ym2612().channel_operator_frequency_hz_debug(2, 0);
    let restored_op2 = audio.ym2612().channel_operator_frequency_hz_debug(2, 1);
    let restored_op3 = audio.ym2612().channel_operator_frequency_hz_debug(2, 2);
    let restored_op4 = audio.ym2612().channel_operator_frequency_hz_debug(2, 3);
    assert!((restored_op1 / normal_op1 - 1.0).abs() < 0.01);
    assert!((restored_op2 / normal_op2 - 1.0).abs() < 0.01);
    assert!((restored_op3 / normal_op3 - 1.0).abs() < 0.01);
    assert!((restored_op4 / normal_op4 - 1.0).abs() < 0.01);
}

#[test]
fn ym2612_key_off_silences_channel() {
    let mut audio = AudioBus::new();
    audio.write_ym2612(0, 0xA0);
    audio.write_ym2612(1, 0xA0);
    audio.write_ym2612(0, 0xA4);
    audio.write_ym2612(1, 0x24);
    // CH1 OP4 SL/RR: fast release.
    audio.write_ym2612(0, 0x8C);
    audio.write_ym2612(1, 0x0F);
    audio.write_ym2612(0, 0x28);
    audio.write_ym2612(1, 0xF0);
    audio.step(2_000);
    let _ = audio.drain_samples(128);

    audio.write_ym2612(0, 0x28);
    audio.write_ym2612(1, 0x00);
    audio.step(2_000);
    let release_samples = audio.drain_samples(128);
    assert!(!release_samples.is_empty());
    assert!(release_samples.iter().any(|&s| s != 0));

    audio.step(120_000);
    let tail_samples = audio.drain_samples(4096);
    assert!(!tail_samples.is_empty());
    let tail_quiet = tail_samples.iter().rev().take(256).all(|&s| s == 0);
    assert!(tail_quiet);
}

#[test]
fn supports_runtime_output_sample_rate_configuration() {
    let mut audio = AudioBus::new();
    audio.set_output_sample_rate_hz(22_050);
    assert_eq!(audio.output_sample_rate_hz(), 22_050);

    audio.step(7_670_454);
    assert!((audio.pending_samples() as i32 - (22_050 * 2)).abs() <= 2);
}

#[test]
fn output_is_stereo_interleaved() {
    let mut audio = AudioBus::new();
    assert_eq!(audio.output_channels(), 2);
    audio.step(2_000);
    assert_eq!(audio.pending_samples() % 2, 0);
}

#[test]
fn ym_pan_register_routes_channel_to_left_only() {
    let mut audio = AudioBus::new();
    // CH1 FNUM/BLOCK
    audio.write_ym2612(0, 0xA0);
    audio.write_ym2612(1, 0x98);
    audio.write_ym2612(0, 0xA4);
    audio.write_ym2612(1, 0x22);
    // CH1 pan: left only
    audio.write_ym2612(0, 0xB4);
    audio.write_ym2612(1, 0x80);
    // Key on CH1
    audio.write_ym2612(0, 0x28);
    audio.write_ym2612(1, 0xF0);
    audio.step(2_000);

    let samples = audio.drain_samples(256);
    assert!(!samples.is_empty());
    let mut left_nonzero = false;
    let mut right_nonzero = false;
    for pair in samples.chunks_exact(2) {
        if pair[0] != 0 {
            left_nonzero = true;
        }
        if pair[1] != 0 {
            right_nonzero = true;
        }
    }
    assert!(left_nonzero);
    assert!(!right_nonzero);
}

#[test]
fn ym_carrier_multiple_register_scales_channel_frequency() {
    let mut audio = AudioBus::new();
    // CH1 FNUM/BLOCK = 0x300 @ block 4.
    audio.write_ym2612(0, 0xA0);
    audio.write_ym2612(1, 0x00);
    audio.write_ym2612(0, 0xA4);
    audio.write_ym2612(1, 0x23);

    let base_hz = audio.ym2612().channel_frequency_hz_debug(0);
    // CH1 OP4 DT/MUL (reg 0x3C) MUL=4.
    audio.write_ym2612(0, 0x3C);
    audio.write_ym2612(1, 0x04);

    assert_eq!(audio.ym2612().channel_carrier_mul(0), 0x04);
    let scaled_hz = audio.ym2612().channel_frequency_hz_debug(0);
    assert!((scaled_hz / base_hz - 4.0).abs() < 0.05);
}

#[test]
fn ym_carrier_multiple_zero_uses_half_step() {
    let mut audio = AudioBus::new();
    // CH1 FNUM/BLOCK = 0x300 @ block 4.
    audio.write_ym2612(0, 0xA0);
    audio.write_ym2612(1, 0x00);
    audio.write_ym2612(0, 0xA4);
    audio.write_ym2612(1, 0x23);

    // MUL = 1 baseline.
    audio.write_ym2612(0, 0x3C);
    audio.write_ym2612(1, 0x01);
    let mul1_hz = audio.ym2612().channel_frequency_hz_debug(0);

    // MUL = 0 should be half of MUL = 1.
    audio.write_ym2612(0, 0x3C);
    audio.write_ym2612(1, 0x00);
    let mul0_hz = audio.ym2612().channel_frequency_hz_debug(0);

    assert!((mul0_hz / mul1_hz - 0.5).abs() < 0.05);
}

#[test]
fn ym_carrier_detune_changes_debug_frequency() {
    let mut audio = AudioBus::new();
    // CH1 FNUM/BLOCK = 0x300 @ block 4.
    audio.write_ym2612(0, 0xA0);
    audio.write_ym2612(1, 0x00);
    audio.write_ym2612(0, 0xA4);
    audio.write_ym2612(1, 0x23);

    // MUL=1, DT=0 baseline.
    audio.write_ym2612(0, 0x3C);
    audio.write_ym2612(1, 0x01);
    let base_hz = audio.ym2612().channel_frequency_hz_debug(0);
    assert_eq!(audio.ym2612().channel_carrier_detune(0), 0x00);

    // MUL=1, DT=3 (positive detune).
    audio.write_ym2612(0, 0x3C);
    audio.write_ym2612(1, 0x31);
    let plus_hz = audio.ym2612().channel_frequency_hz_debug(0);
    assert_eq!(audio.ym2612().channel_carrier_detune(0), 0x03);

    // MUL=1, DT=5 (negative detune).
    audio.write_ym2612(0, 0x3C);
    audio.write_ym2612(1, 0x51);
    let minus_hz = audio.ym2612().channel_frequency_hz_debug(0);
    assert_eq!(audio.ym2612().channel_carrier_detune(0), 0x05);

    assert!(plus_hz > base_hz, "plus_hz={plus_hz}, base_hz={base_hz}");
    assert!(minus_hz < base_hz, "minus_hz={minus_hz}, base_hz={base_hz}");
}

#[test]
fn ym_carrier_total_level_can_mute_channel_output() {
    let mut audio = AudioBus::new();
    audio.write_ym2612(0, 0xA0);
    audio.write_ym2612(1, 0x98);
    audio.write_ym2612(0, 0xA4);
    audio.write_ym2612(1, 0x22);

    // CH1 OP4 TL = 0 (loud)
    audio.write_ym2612(0, 0x4C);
    audio.write_ym2612(1, 0x00);
    audio.write_ym2612(0, 0x28);
    audio.write_ym2612(1, 0xF0);
    audio.step(2_000);
    let loud = audio.drain_samples(128);
    assert!(loud.iter().any(|&s| s != 0));

    // Key off before changing TL for deterministic restart.
    audio.write_ym2612(0, 0x28);
    audio.write_ym2612(1, 0x00);
    audio.step(200);
    let _ = audio.drain_samples(64);

    // CH1 OP4 TL = 127 (silent in this model)
    audio.write_ym2612(0, 0x4C);
    audio.write_ym2612(1, 0x7F);
    assert_eq!(audio.ym2612().channel_carrier_tl(0), 0x7F);
    audio.write_ym2612(0, 0x28);
    audio.write_ym2612(1, 0xF0);
    audio.step(2_000);
    let muted = audio.drain_samples(128);
    assert!(muted.iter().all(|&s| s == 0));
}

#[test]
fn psg_period_zero_matches_period_one_frequency_on_integrated_psg() {
    let mut audio = AudioBus::new();
    // Tone 0 period = 0x000 (low nibble + high bits).
    audio.write_psg(0x80);
    audio.write_psg(0x00);
    let f0 = audio.psg().tone_frequency_hz_debug(0);

    // Tone 0 period = 0x001.
    audio.write_psg(0x81);
    audio.write_psg(0x00);
    assert_eq!(audio.psg().tone_period(0), 0x001);
    let f1 = audio.psg().tone_frequency_hz_debug(0);
    assert!(
        (f0 - f1).abs() < 0.01,
        "period=0 should match period=1 frequency: f0={f0}, f1={f1}"
    );
}

#[test]
fn psg_rendered_tone_frequency_matches_divider_formula() {
    let mut audio = AudioBus::new();
    audio.set_output_sample_rate_hz(44_100);

    // CH0 tone period = 0x200.
    audio.write_psg(0x80);
    audio.write_psg(0x20);
    // CH0 volume = 0 (max loudness).
    audio.write_psg(0x90);

    // Generate ~1 second of audio.
    audio.step(7_670_454);
    let samples = audio.drain_samples(audio.pending_samples());
    let left: Vec<i16> = samples.iter().step_by(2).copied().collect();
    assert!(!left.is_empty());

    let mut falling_edges = 0usize;
    for pair in left.windows(2) {
        if pair[0] > 0 && pair[1] <= 0 {
            falling_edges += 1;
        }
    }

    let duration = left.len() as f32 / 44_100.0;
    let measured_hz = falling_edges as f32 / duration;
    let expected_hz = audio.psg().tone_frequency_hz_debug(0);
    let tolerance = expected_hz * 0.12 + 2.0;
    assert!(
        (measured_hz - expected_hz).abs() <= tolerance,
        "expected around {expected_hz:.2}Hz, got {measured_hz:.2}Hz"
    );
}

#[test]
fn ym_algorithm_and_feedback_registers_are_tracked() {
    let mut audio = AudioBus::new();
    // CH1 algorithm=5 feedback=6
    audio.write_ym2612(0, 0xB0);
    audio.write_ym2612(1, 0x35);

    assert_eq!(audio.ym2612().channel_algorithm_feedback(0), (0x05, 0x06));
}

#[test]
fn ym_pan_register_tracks_ams_and_fms_fields() {
    let mut audio = AudioBus::new();
    // CH1 pan L/R + AMS=3 + FMS=7
    audio.write_ym2612(0, 0xB4);
    audio.write_ym2612(1, 0xF7);
    assert_eq!(audio.ym2612().channel_ams_fms(0), (0x03, 0x07));
}

#[test]
fn ym_lfo_enable_and_rate_follow_register_22() {
    let mut audio = AudioBus::new();
    audio.write_ym2612(0, 0x22);
    audio.write_ym2612(1, 0x0D); // enable + rate=5
    assert!(audio.ym2612().lfo_enabled());
    assert_eq!(audio.ym2612().lfo_rate(), 0x05);

    audio.write_ym2612(0, 0x22);
    audio.write_ym2612(1, 0x00);
    assert!(!audio.ym2612().lfo_enabled());
}

#[test]
fn ym_lfo_does_not_am_modulate_channel_without_operator_am_enable() {
    fn configure_tone(audio: &mut AudioBus, lfo_enabled: bool) {
        // CH1 base tone.
        audio.write_ym2612(0, 0xA0);
        audio.write_ym2612(1, 0x98);
        audio.write_ym2612(0, 0xA4);
        audio.write_ym2612(1, 0x22);
        // CH1 algorithm 7 (all carriers) for stable output.
        audio.write_ym2612(0, 0xB0);
        audio.write_ym2612(1, 0x07);
        // Pan both + AMS=3 + FMS=0.
        audio.write_ym2612(0, 0xB4);
        audio.write_ym2612(1, 0xF0);
        // Ensure OP4 DR has AM-enable bit cleared.
        audio.write_ym2612(0, 0x6C);
        audio.write_ym2612(1, 0x00);
        if lfo_enabled {
            audio.write_ym2612(0, 0x22);
            audio.write_ym2612(1, 0x0F); // enable + high rate
        }
        // Key on CH1 all operators.
        audio.write_ym2612(0, 0x28);
        audio.write_ym2612(1, 0xF0);
    }

    let mut no_lfo = AudioBus::new();
    configure_tone(&mut no_lfo, false);
    no_lfo.step(6_000);
    let samples_no_lfo = no_lfo.drain_samples(256);

    let mut with_lfo = AudioBus::new();
    configure_tone(&mut with_lfo, true);
    with_lfo.step(6_000);
    let samples_with_lfo = with_lfo.drain_samples(256);

    assert_eq!(samples_no_lfo, samples_with_lfo);
}

#[test]
fn ym_feedback_setting_changes_waveform_after_key_on_restart() {
    let mut audio = AudioBus::new();

    // CH1 base pitch.
    audio.write_ym2612(0, 0xA0);
    audio.write_ym2612(1, 0x98);
    audio.write_ym2612(0, 0xA4);
    audio.write_ym2612(1, 0x22);
    // Ensure audible carrier level.
    audio.write_ym2612(0, 0x4C);
    audio.write_ym2612(1, 0x00);

    // Algorithm 0, feedback 0.
    audio.write_ym2612(0, 0xB0);
    audio.write_ym2612(1, 0x00);
    audio.write_ym2612(0, 0x28);
    audio.write_ym2612(1, 0xF0);
    audio.step(3_000);
    let baseline = audio.drain_samples(128);

    // Restart channel to reset phase, then raise feedback.
    audio.write_ym2612(0, 0x28);
    audio.write_ym2612(1, 0x00);
    audio.step(200);
    let _ = audio.drain_samples(64);
    audio.write_ym2612(0, 0xB0);
    audio.write_ym2612(1, 0x38); // algorithm 0 + max feedback (7)
    audio.write_ym2612(0, 0x28);
    audio.write_ym2612(1, 0xF0);
    audio.step(3_000);
    let feedback = audio.drain_samples(128);

    assert_eq!(baseline.len(), feedback.len());
    assert!(
        baseline != feedback,
        "feedback should alter sample stream for same note"
    );
}

#[test]
fn ym_alg1_op3_directly_contributes_to_o4_path() {
    fn configure_base(audio: &mut AudioBus) {
        // CH1 pitch.
        audio.write_ym2612(0, 0xA0);
        audio.write_ym2612(1, 0x98);
        audio.write_ym2612(0, 0xA4);
        audio.write_ym2612(1, 0x22);
        // ALG1 + no feedback.
        audio.write_ym2612(0, 0xB0);
        audio.write_ym2612(1, 0x01);
        // OP2 TL=127 (muted), OP4 TL=0 (audible).
        audio.write_ym2612(0, 0x48); // CH1 OP2 TL
        audio.write_ym2612(1, 0x7F);
        audio.write_ym2612(0, 0x4C); // CH1 OP4 TL
        audio.write_ym2612(1, 0x00);
        // OP1 TL=127 (remove OP1 influence), OP3 TL=0.
        audio.write_ym2612(0, 0x40); // CH1 OP1 TL
        audio.write_ym2612(1, 0x7F);
        audio.write_ym2612(0, 0x44); // CH1 OP3 TL
        audio.write_ym2612(1, 0x00);
    }

    let mut slow = AudioBus::new();
    configure_base(&mut slow);
    // OP3 MUL=1.
    slow.write_ym2612(0, 0x34); // CH1 OP3 DT/MUL
    slow.write_ym2612(1, 0x01);
    // Key on OP3+OP4 only.
    slow.write_ym2612(0, 0x28);
    slow.write_ym2612(1, 0xC0);
    slow.step(8_000);
    let samples_slow = slow.drain_samples(256);

    let mut fast = AudioBus::new();
    configure_base(&mut fast);
    // OP3 MUL=15 (large change if OP3 directly fed O4).
    fast.write_ym2612(0, 0x34); // CH1 OP3 DT/MUL
    fast.write_ym2612(1, 0x0F);
    fast.write_ym2612(0, 0x28);
    fast.write_ym2612(1, 0xC0);
    fast.step(8_000);
    let samples_fast = fast.drain_samples(256);

    assert_ne!(samples_slow, samples_fast);
}

#[test]
fn ym_alg2_op3_directly_contributes_to_o4_path() {
    fn configure_base(audio: &mut AudioBus) {
        // CH1 pitch.
        audio.write_ym2612(0, 0xA0);
        audio.write_ym2612(1, 0x98);
        audio.write_ym2612(0, 0xA4);
        audio.write_ym2612(1, 0x22);
        // ALG2 + no feedback.
        audio.write_ym2612(0, 0xB0);
        audio.write_ym2612(1, 0x02);
        // OP2 TL=127 (muted), OP4 TL=0 (audible).
        audio.write_ym2612(0, 0x48); // CH1 OP2 TL
        audio.write_ym2612(1, 0x7F);
        audio.write_ym2612(0, 0x4C); // CH1 OP4 TL
        audio.write_ym2612(1, 0x00);
        // OP1 TL=0 (this path remains), OP3 TL=0.
        audio.write_ym2612(0, 0x40); // CH1 OP1 TL
        audio.write_ym2612(1, 0x00);
        audio.write_ym2612(0, 0x44); // CH1 OP3 TL
        audio.write_ym2612(1, 0x00);
    }

    let mut slow = AudioBus::new();
    configure_base(&mut slow);
    // OP3 MUL=1.
    slow.write_ym2612(0, 0x34); // CH1 OP3 DT/MUL
    slow.write_ym2612(1, 0x01);
    // Key on OP1+OP3+OP4.
    slow.write_ym2612(0, 0x28);
    slow.write_ym2612(1, 0xD0);
    slow.step(8_000);
    let samples_slow = slow.drain_samples(256);

    let mut fast = AudioBus::new();
    configure_base(&mut fast);
    // OP3 MUL=15.
    fast.write_ym2612(0, 0x34); // CH1 OP3 DT/MUL
    fast.write_ym2612(1, 0x0F);
    fast.write_ym2612(0, 0x28);
    fast.write_ym2612(1, 0xD0);
    fast.step(8_000);
    let samples_fast = fast.drain_samples(256);

    assert_ne!(samples_slow, samples_fast);
}

#[test]
fn ym_carrier_envelope_registers_are_tracked() {
    let mut audio = AudioBus::new();
    // CH1 OP4: AR/KS, DR/AM, SR, SL/RR
    audio.write_ym2612(0, 0x5C);
    audio.write_ym2612(1, 0x1F); // AR=31
    audio.write_ym2612(0, 0x6C);
    audio.write_ym2612(1, 0x0E); // DR=14
    audio.write_ym2612(0, 0x7C);
    audio.write_ym2612(1, 0x09); // SR=9
    audio.write_ym2612(0, 0x8C);
    audio.write_ym2612(1, 0xA7); // SL=10 RR=7

    assert_eq!(
        audio.ym2612().channel_envelope_params(0),
        (31, 14, 9, 10, 7)
    );
}

#[test]
fn ym_carrier_ssg_eg_register_is_tracked() {
    let mut audio = AudioBus::new();
    // CH1 OP4 SSG-EG.
    audio.write_ym2612(0, 0x9C);
    audio.write_ym2612(1, 0x0B);
    assert_eq!(audio.ym2612().channel_carrier_ssg_eg(0), 0x0B);
}

#[test]
fn ym_ssg_eg_repeat_keeps_tone_active_after_decay_floor() {
    fn configure_channel(audio: &mut AudioBus, ssg_eg: u8) {
        // CH1 base pitch.
        audio.write_ym2612(0, 0xA0);
        audio.write_ym2612(1, 0x98);
        audio.write_ym2612(0, 0xA4);
        audio.write_ym2612(1, 0x22);
        // Algorithm 0 so OP4 is the only output carrier.
        audio.write_ym2612(0, 0xB0);
        audio.write_ym2612(1, 0x00);
        // OP4 loud TL.
        audio.write_ym2612(0, 0x4C);
        audio.write_ym2612(1, 0x00);
        // Fast AR/DR and sustain floor at 0.
        audio.write_ym2612(0, 0x5C);
        audio.write_ym2612(1, 0x1F); // AR=31
        audio.write_ym2612(0, 0x6C);
        audio.write_ym2612(1, 0x1F); // DR=31
        audio.write_ym2612(0, 0x7C);
        audio.write_ym2612(1, 0x00); // SR=0
        audio.write_ym2612(0, 0x8C);
        audio.write_ym2612(1, 0xF0); // SL=15, RR=0
        audio.write_ym2612(0, 0x9C);
        audio.write_ym2612(1, ssg_eg & 0x0F);
        // Key on CH1 OP4 only.
        audio.write_ym2612(0, 0x28);
        audio.write_ym2612(1, 0x80);
    }

    fn tail_peak(samples: &[i16], tail_len: usize) -> u16 {
        samples
            .iter()
            .rev()
            .take(tail_len)
            .map(|s| s.unsigned_abs())
            .max()
            .unwrap_or(0)
    }

    let mut no_ssg = AudioBus::new();
    configure_channel(&mut no_ssg, 0x00);
    no_ssg.step(3_000_000);
    let no_ssg_samples = no_ssg.drain_samples(no_ssg.pending_samples());
    let no_ssg_tail_peak = tail_peak(&no_ssg_samples, 512);

    let mut repeat_ssg = AudioBus::new();
    configure_channel(&mut repeat_ssg, 0x08); // enable, repeat
    repeat_ssg.step(3_000_000);
    let repeat_ssg_samples = repeat_ssg.drain_samples(repeat_ssg.pending_samples());
    let repeat_ssg_tail_peak = tail_peak(&repeat_ssg_samples, 512);

    assert!(
        no_ssg_tail_peak < 8,
        "expected near-silent tail without SSG-EG, got peak={no_ssg_tail_peak}"
    );
    assert!(
        repeat_ssg_tail_peak > 32,
        "expected repeating SSG-EG tail to stay audible, got peak={repeat_ssg_tail_peak}"
    );
}

#[test]
fn ym_ssg_eg_hold_reaches_floor_even_with_high_sustain_level() {
    fn configure_channel(audio: &mut AudioBus, ssg_eg: u8) {
        // CH1 base pitch.
        audio.write_ym2612(0, 0xA0);
        audio.write_ym2612(1, 0x98);
        audio.write_ym2612(0, 0xA4);
        audio.write_ym2612(1, 0x22);
        // Algorithm 0 so OP4 is the only output carrier.
        audio.write_ym2612(0, 0xB0);
        audio.write_ym2612(1, 0x00);
        // OP4 loud TL.
        audio.write_ym2612(0, 0x4C);
        audio.write_ym2612(1, 0x00);
        // Fast AR/DR, but SL=0 and SR=0 (normally would stay loud forever).
        audio.write_ym2612(0, 0x5C);
        audio.write_ym2612(1, 0x1F); // AR=31
        audio.write_ym2612(0, 0x6C);
        audio.write_ym2612(1, 0x1F); // DR=31
        audio.write_ym2612(0, 0x7C);
        audio.write_ym2612(1, 0x00); // SR=0
        audio.write_ym2612(0, 0x8C);
        audio.write_ym2612(1, 0x00); // SL=0 RR=0
        audio.write_ym2612(0, 0x9C);
        audio.write_ym2612(1, ssg_eg & 0x0F);
        // Key on CH1 OP4 only.
        audio.write_ym2612(0, 0x28);
        audio.write_ym2612(1, 0x80);
    }

    fn tail_peak(samples: &[i16], tail_len: usize) -> u16 {
        samples
            .iter()
            .rev()
            .take(tail_len)
            .map(|s| s.unsigned_abs())
            .max()
            .unwrap_or(0)
    }

    let mut no_ssg = AudioBus::new();
    configure_channel(&mut no_ssg, 0x00);
    no_ssg.step(3_000_000);
    let no_ssg_env = no_ssg.ym2612().channel_envelope_level(0);
    let no_ssg_samples = no_ssg.drain_samples(no_ssg.pending_samples());
    let no_ssg_tail_peak = tail_peak(&no_ssg_samples, 512);

    let mut hold_ssg = AudioBus::new();
    configure_channel(&mut hold_ssg, 0x09); // enable + hold
    hold_ssg.step(3_000_000);
    let hold_ssg_env = hold_ssg.ym2612().channel_envelope_level(0);
    let hold_ssg_samples = hold_ssg.drain_samples(hold_ssg.pending_samples());
    let hold_ssg_tail_peak = tail_peak(&hold_ssg_samples, 512);

    // KS=0 still contributes a small rate boost (keycode >> 3) per Nuked
    // OPN2, so SR=0 slowly decays.  Envelope should remain audible though.
    assert!(no_ssg_env > 0.3, "no_ssg_env={no_ssg_env}");
    assert!(
        no_ssg_tail_peak > 128,
        "expected audible tail without SSG-EG hold, got peak={no_ssg_tail_peak}"
    );
    assert!(hold_ssg_env < 0.02, "hold_ssg_env={hold_ssg_env}");
    assert!(
        hold_ssg_tail_peak < 16,
        "expected near-silent tail with SSG-EG hold, got peak={hold_ssg_tail_peak}"
    );
}

#[test]
fn ym_attack_rate_affects_envelope_ramp_speed() {
    let mut slow = AudioBus::new();
    // CH1 pitch and carrier level.
    slow.write_ym2612(0, 0xA0);
    slow.write_ym2612(1, 0x98);
    slow.write_ym2612(0, 0xA4);
    slow.write_ym2612(1, 0x22);
    slow.write_ym2612(0, 0x4C);
    slow.write_ym2612(1, 0x00);
    // Slow AR=1, keep long sustain.
    slow.write_ym2612(0, 0x5C);
    slow.write_ym2612(1, 0x01);
    slow.write_ym2612(0, 0x6C);
    slow.write_ym2612(1, 0x00);
    slow.write_ym2612(0, 0x7C);
    slow.write_ym2612(1, 0x00);
    slow.write_ym2612(0, 0x8C);
    slow.write_ym2612(1, 0x00);
    slow.write_ym2612(0, 0x28);
    slow.write_ym2612(1, 0xF0);
    slow.step(5_000);
    let slow_env = slow.ym2612().channel_envelope_level(0);

    let mut fast = AudioBus::new();
    // Same setup, but fast AR=31.
    fast.write_ym2612(0, 0xA0);
    fast.write_ym2612(1, 0x98);
    fast.write_ym2612(0, 0xA4);
    fast.write_ym2612(1, 0x22);
    fast.write_ym2612(0, 0x4C);
    fast.write_ym2612(1, 0x00);
    fast.write_ym2612(0, 0x5C);
    fast.write_ym2612(1, 0x1F);
    fast.write_ym2612(0, 0x6C);
    fast.write_ym2612(1, 0x00);
    fast.write_ym2612(0, 0x7C);
    fast.write_ym2612(1, 0x00);
    fast.write_ym2612(0, 0x8C);
    fast.write_ym2612(1, 0x00);
    fast.write_ym2612(0, 0x28);
    fast.write_ym2612(1, 0xF0);
    fast.step(5_000);
    let fast_env = fast.ym2612().channel_envelope_level(0);

    assert!(
        fast_env > slow_env,
        "fast_env={fast_env}, slow_env={slow_env}"
    );
}

#[test]
fn ym_attack_rate_zero_is_not_instant_full_level() {
    let mut ar0 = AudioBus::new();
    // CH1 pitch and carrier level.
    ar0.write_ym2612(0, 0xA0);
    ar0.write_ym2612(1, 0x98);
    ar0.write_ym2612(0, 0xA4);
    ar0.write_ym2612(1, 0x22);
    ar0.write_ym2612(0, 0x4C);
    ar0.write_ym2612(1, 0x00);
    // AR=0 (very slow), DR/SR disabled.
    ar0.write_ym2612(0, 0x5C);
    ar0.write_ym2612(1, 0x00);
    ar0.write_ym2612(0, 0x6C);
    ar0.write_ym2612(1, 0x00);
    ar0.write_ym2612(0, 0x7C);
    ar0.write_ym2612(1, 0x00);
    ar0.write_ym2612(0, 0x8C);
    ar0.write_ym2612(1, 0x00);
    ar0.write_ym2612(0, 0x28);
    ar0.write_ym2612(1, 0xF0);
    ar0.step(12_000);
    let ar0_env = ar0.ym2612().channel_envelope_level(0);

    let mut ar31 = AudioBus::new();
    ar31.write_ym2612(0, 0xA0);
    ar31.write_ym2612(1, 0x98);
    ar31.write_ym2612(0, 0xA4);
    ar31.write_ym2612(1, 0x22);
    ar31.write_ym2612(0, 0x4C);
    ar31.write_ym2612(1, 0x00);
    ar31.write_ym2612(0, 0x5C);
    ar31.write_ym2612(1, 0x1F);
    ar31.write_ym2612(0, 0x6C);
    ar31.write_ym2612(1, 0x00);
    ar31.write_ym2612(0, 0x7C);
    ar31.write_ym2612(1, 0x00);
    ar31.write_ym2612(0, 0x8C);
    ar31.write_ym2612(1, 0x00);
    ar31.write_ym2612(0, 0x28);
    ar31.write_ym2612(1, 0xF0);
    ar31.step(12_000);
    let ar31_env = ar31.ym2612().channel_envelope_level(0);

    assert!(ar0_env < 0.3, "ar0_env={ar0_env}");
    assert!(
        ar31_env > 0.45 && ar31_env > ar0_env + 0.25,
        "ar31_env={ar31_env}, ar0_env={ar0_env}"
    );
}

#[test]
fn ym_release_rate_affects_envelope_decay_speed_after_keyoff() {
    let mut slow = AudioBus::new();
    slow.write_ym2612(0, 0xA0);
    slow.write_ym2612(1, 0x98);
    slow.write_ym2612(0, 0xA4);
    slow.write_ym2612(1, 0x22);
    slow.write_ym2612(0, 0x4C);
    slow.write_ym2612(1, 0x00);
    slow.write_ym2612(0, 0x8C);
    slow.write_ym2612(1, 0x00); // RR=0 (slow)
    slow.write_ym2612(0, 0x28);
    slow.write_ym2612(1, 0xF0);
    slow.step(4_000);
    let _ = slow.drain_samples(128);
    slow.write_ym2612(0, 0x28);
    slow.write_ym2612(1, 0x00);
    slow.step(20_000);
    let slow_env = slow.ym2612().channel_envelope_level(0);

    let mut fast = AudioBus::new();
    fast.write_ym2612(0, 0xA0);
    fast.write_ym2612(1, 0x98);
    fast.write_ym2612(0, 0xA4);
    fast.write_ym2612(1, 0x22);
    fast.write_ym2612(0, 0x4C);
    fast.write_ym2612(1, 0x00);
    fast.write_ym2612(0, 0x8C);
    fast.write_ym2612(1, 0x0F); // RR=15 (fast)
    fast.write_ym2612(0, 0x28);
    fast.write_ym2612(1, 0xF0);
    fast.step(4_000);
    let _ = fast.drain_samples(128);
    fast.write_ym2612(0, 0x28);
    fast.write_ym2612(1, 0x00);
    fast.step(20_000);
    let fast_env = fast.ym2612().channel_envelope_level(0);

    assert!(
        fast_env < slow_env,
        "fast_env={fast_env}, slow_env={slow_env}"
    );
}
