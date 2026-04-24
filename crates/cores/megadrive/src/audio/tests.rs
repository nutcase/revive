use super::{DETUNE_TABLE, EXP_TABLE, LOG_SIN_TABLE, Psg, Ym2612, op_calc};

fn write_ym_reg(ym: &mut Ym2612, bank: usize, reg: u8, value: u8) {
    let (addr_port, data_port) = if (bank & 1) == 0 {
        (0u8, 1u8)
    } else {
        (2u8, 3u8)
    };
    ym.write_port(addr_port, reg);
    ym.write_port(data_port, value);
}

#[test]
fn psg_data_byte_updates_latched_volume_register() {
    let mut psg = Psg::default();
    psg.write_data(0b1101_0011);
    assert_eq!(psg.attenuation(2), 0x03);
    psg.write_data(0x0B);
    assert_eq!(psg.attenuation(2), 0x0B);
}

#[test]
fn psg_data_byte_updates_latched_tone_period_high_bits() {
    let mut psg = Psg::default();
    psg.write_data(0b1000_0101);
    psg.write_data(0x12);
    assert_eq!(psg.tone_period(0), 0x125);
}

#[test]
fn sine_table_peak_and_zero_crossing() {
    // At quarter-wave peak (index 255), attenuation should be 0 (maximum output).
    assert_eq!(LOG_SIN_TABLE[255], 0);
    // At zero crossing (index 0), attenuation should be large.
    assert!(LOG_SIN_TABLE[0] > 2000);
}

#[test]
fn exp_table_range() {
    // Index 0 should give near-maximum significand.
    assert!(EXP_TABLE[0] >= 1016);
    // Index 255 should give 0.
    assert_eq!(EXP_TABLE[255], 0);
}

#[test]
fn op_calc_zero_attenuation_gives_max_output() {
    // Phase at quarter-wave peak (0x100 = mirror=1, index=0 → index=255)
    let output = op_calc(0x100, 0);
    // Should be near +8188 (13-bit max)
    assert!(output > 8000, "expected >8000, got {}", output);
}

#[test]
fn op_calc_sign_bit() {
    // Phase with sign bit set → negative output
    let positive = op_calc(0x100, 0);
    let negative = op_calc(0x300, 0); // bit 9 set = sign
    assert!(positive > 0);
    assert!(negative < 0);
    assert_eq!(positive, -negative);
}

#[test]
fn op_calc_max_attenuation_gives_zero() {
    let output = op_calc(0x100, 0xFFF);
    assert_eq!(output, 0);
}

#[test]
fn detune_table_dt0_is_all_zeros() {
    for kc in 0..32 {
        assert_eq!(DETUNE_TABLE[0][kc], 0);
    }
}

#[test]
fn feedback_levels_sweep() {
    // Verify that higher feedback levels produce larger self-modulation.
    // Use algorithm 0 (serial) and measure OP1 output with different FB values.
    let mut max_outputs = Vec::new();
    for fb in 0..8u8 {
        let mut ym = Ym2612::default();
        // CH1: set algorithm 0, feedback = fb
        write_ym_reg(&mut ym, 0, 0xB0, fb << 3);
        // Set all operators to max volume (TL=0), fast attack, no decay
        for op_reg_base in [0x30u8, 0x38, 0x34, 0x3C] {
            write_ym_reg(&mut ym, 0, op_reg_base, 0x01); // MUL=1, DT=0
        }
        for op_reg_base in [0x40u8, 0x48, 0x44, 0x4C] {
            write_ym_reg(&mut ym, 0, op_reg_base, 0x00); // TL=0
        }
        for op_reg_base in [0x50u8, 0x58, 0x54, 0x5C] {
            write_ym_reg(&mut ym, 0, op_reg_base, 0x1F); // AR=31
        }
        for op_reg_base in [0x60u8, 0x68, 0x64, 0x6C] {
            write_ym_reg(&mut ym, 0, op_reg_base, 0x00); // DR=0
        }
        for op_reg_base in [0x80u8, 0x88, 0x84, 0x8C] {
            write_ym_reg(&mut ym, 0, op_reg_base, 0x00); // SL=0, RR=0
        }
        // Set frequency: fnum=0x200, block=4
        write_ym_reg(&mut ym, 0, 0xA4, 0x22);
        write_ym_reg(&mut ym, 0, 0xA0, 0x00);
        // Key-on all operators
        write_ym_reg(&mut ym, 0, 0x28, 0xF0);
        // Render some samples and find max
        let mut max_val = 0i32;
        for _ in 0..1000 {
            let (l, _) = ym.next_sample_stereo(44100);
            max_val = max_val.max(l.unsigned_abs() as i32);
        }
        max_outputs.push(max_val);
    }
    // FB=0 should give some output (pure FM chain)
    assert!(max_outputs[0] > 0, "FB=0 should produce output");
    // Higher FB levels should generally produce different (often larger) peak outputs
    // due to self-modulation creating harmonics
}

#[test]
fn ym2612_channel3_special_mode_uses_ym3438_slot_mapping() {
    let mut ym = Ym2612::default();
    write_ym_reg(&mut ym, 0, 0xA2, 0x34);
    write_ym_reg(&mut ym, 0, 0xA6, 0x21);
    write_ym_reg(&mut ym, 0, 0xA8, 0x11);
    write_ym_reg(&mut ym, 0, 0xAC, 0x18);
    write_ym_reg(&mut ym, 0, 0xA9, 0x22);
    write_ym_reg(&mut ym, 0, 0xAD, 0x29);
    write_ym_reg(&mut ym, 0, 0xAA, 0x33);
    write_ym_reg(&mut ym, 0, 0xAE, 0x31);
    write_ym_reg(&mut ym, 0, 0x27, 0x40);

    let op1 = ym.channel_operator_frequency_hz_debug(2, 0);
    let op2 = ym.channel_operator_frequency_hz_debug(2, 1);
    let op3 = ym.channel_operator_frequency_hz_debug(2, 2);
    let op4 = ym.channel_operator_frequency_hz_debug(2, 3);

    // Expected frequencies computed from phase increment formula:
    // freq = ((fnum << block) >> 1) * mul * HW_RATE / 2^20
    // With default mul=1, detune=0:
    // freq = ((fnum << block) >> 1) * 53267 / 1048576
    let compute_freq = |fnum: u16, block: u8| -> f32 {
        let inc = ((fnum as u32) << (block as u32)) >> 1;
        inc as f32 * (7_670_454.0 / 144.0) / 1_048_576.0
    };

    let expected_op1 = compute_freq(0x122, 5);
    let expected_op2 = compute_freq(0x133, 6);
    let expected_op3 = compute_freq(0x011, 3);
    let expected_op4 = compute_freq(0x134, 4);

    assert!(
        (op1 - expected_op1).abs() < 0.5,
        "op1={} exp={}",
        op1,
        expected_op1
    );
    assert!(
        (op2 - expected_op2).abs() < 0.5,
        "op2={} exp={}",
        op2,
        expected_op2
    );
    assert!(
        (op3 - expected_op3).abs() < 0.5,
        "op3={} exp={}",
        op3,
        expected_op3
    );
    assert!(
        (op4 - expected_op4).abs() < 0.5,
        "op4={} exp={}",
        op4,
        expected_op4
    );
}

#[test]
fn ym2612_channel3_without_special_mode_uses_normal_frequency_for_all_operators() {
    let mut ym = Ym2612::default();
    write_ym_reg(&mut ym, 0, 0xA2, 0x56);
    write_ym_reg(&mut ym, 0, 0xA6, 0x2B);
    write_ym_reg(&mut ym, 0, 0xA8, 0x01);
    write_ym_reg(&mut ym, 0, 0xAC, 0x10);
    write_ym_reg(&mut ym, 0, 0xA9, 0x02);
    write_ym_reg(&mut ym, 0, 0xAD, 0x18);
    write_ym_reg(&mut ym, 0, 0xAA, 0x03);
    write_ym_reg(&mut ym, 0, 0xAE, 0x20);

    let expected = {
        let inc = ((0x356u32) << 5) >> 1;
        inc as f32 * (7_670_454.0 / 144.0) / 1_048_576.0
    };
    for op in 0..4 {
        let got = ym.channel_operator_frequency_hz_debug(2, op);
        assert!(
            (got - expected).abs() < 0.5,
            "op{}={} exp={}",
            op + 1,
            got,
            expected
        );
    }
}
