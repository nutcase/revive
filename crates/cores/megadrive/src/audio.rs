// YM2612 hardware sine ROM: quarter-wave log-attenuation (256 entries, 12-bit 4.8 format).
// logsin[i] = round(-log2(sin((2*i+1) * PI / 1024)) * 256)
const LOG_SIN_TABLE: [u16; 256] = [
    2137, 1731, 1543, 1419, 1326, 1252, 1190, 1137,
    1091, 1050, 1013, 979, 949, 920, 894, 869,
    846, 825, 804, 785, 767, 749, 732, 717,
    701, 687, 672, 659, 646, 633, 621, 609,
    598, 587, 576, 566, 556, 546, 536, 527,
    518, 509, 501, 492, 484, 476, 468, 461,
    453, 446, 439, 432, 425, 418, 411, 405,
    399, 392, 386, 380, 375, 369, 363, 358,
    352, 347, 341, 336, 331, 326, 321, 316,
    311, 307, 302, 297, 293, 289, 284, 280,
    276, 271, 267, 263, 259, 255, 251, 248,
    244, 240, 236, 233, 229, 226, 222, 219,
    215, 212, 209, 205, 202, 199, 196, 193,
    190, 187, 184, 181, 178, 175, 172, 169,
    167, 164, 161, 159, 156, 153, 151, 148,
    146, 143, 141, 138, 136, 134, 131, 129,
    127, 125, 122, 120, 118, 116, 114, 112,
    110, 108, 106, 104, 102, 100, 98, 96,
    94, 92, 91, 89, 87, 85, 83, 82,
    80, 78, 77, 75, 74, 72, 70, 69,
    67, 66, 64, 63, 62, 60, 59, 57,
    56, 55, 53, 52, 51, 49, 48, 47,
    46, 45, 43, 42, 41, 40, 39, 38,
    37, 36, 35, 34, 33, 32, 31, 30,
    29, 28, 27, 26, 25, 24, 23, 23,
    22, 21, 20, 20, 19, 18, 17, 17,
    16, 15, 15, 14, 13, 13, 12, 12,
    11, 10, 10, 9, 9, 8, 8, 7,
    7, 7, 6, 6, 5, 5, 5, 4,
    4, 4, 3, 3, 3, 2, 2, 2,
    2, 1, 1, 1, 1, 1, 1, 1,
    0, 0, 0, 0, 0, 0, 0, 0,
];

// YM2612 hardware exponential ROM: converts 8-bit log mantissa to 10-bit linear.
// exp[i] = round((pow(2, (255-i)/256.0) - 1.0) * 1024)
const EXP_TABLE: [u16; 256] = [
    1018, 1013, 1007, 1002, 996, 991, 986, 980,
    975, 969, 964, 959, 953, 948, 942, 937,
    932, 927, 921, 916, 911, 906, 900, 895,
    890, 885, 880, 874, 869, 864, 859, 854,
    849, 844, 839, 834, 829, 824, 819, 814,
    809, 804, 799, 794, 789, 784, 779, 774,
    770, 765, 760, 755, 750, 745, 741, 736,
    731, 726, 722, 717, 712, 708, 703, 698,
    693, 689, 684, 680, 675, 670, 666, 661,
    657, 652, 648, 643, 639, 634, 630, 625,
    621, 616, 612, 607, 603, 599, 594, 590,
    585, 581, 577, 572, 568, 564, 560, 555,
    551, 547, 542, 538, 534, 530, 526, 521,
    517, 513, 509, 505, 501, 496, 492, 488,
    484, 480, 476, 472, 468, 464, 460, 456,
    452, 448, 444, 440, 436, 432, 428, 424,
    420, 416, 412, 409, 405, 401, 397, 393,
    389, 385, 382, 378, 374, 370, 367, 363,
    359, 355, 352, 348, 344, 340, 337, 333,
    329, 326, 322, 318, 315, 311, 308, 304,
    300, 297, 293, 290, 286, 283, 279, 276,
    272, 268, 265, 262, 258, 255, 251, 248,
    244, 241, 237, 234, 231, 227, 224, 220,
    217, 214, 210, 207, 204, 200, 197, 194,
    190, 187, 184, 181, 177, 174, 171, 168,
    164, 161, 158, 155, 152, 148, 145, 142,
    139, 136, 133, 130, 126, 123, 120, 117,
    114, 111, 108, 105, 102, 99, 96, 93,
    90, 87, 84, 81, 78, 75, 72, 69,
    66, 63, 60, 57, 54, 51, 48, 45,
    42, 40, 37, 34, 31, 28, 25, 22,
    20, 17, 14, 11, 8, 6, 3, 0,
];

// Hardware detune table from MAME: phase increment delta indexed by [dt_base (0-3)][keycode (0-31)].
// DT1 register bits [2:0]: values 4-7 negate the delta.
const DETUNE_TABLE: [[u8; 32]; 4] = [
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 1, 1, 1, 1, 1, 1, 1, 1, 2, 2, 2, 2, 2, 3, 3, 3, 4, 4, 4, 5, 5, 6, 6, 7, 8, 8, 8, 8],
    [1, 1, 1, 1, 2, 2, 2, 2, 2, 3, 3, 3, 4, 4, 4, 5, 5, 6, 6, 7, 8, 8, 9, 10, 11, 12, 13, 14, 16, 16, 16, 16],
    [2, 2, 2, 2, 2, 3, 3, 3, 4, 4, 4, 5, 5, 6, 6, 7, 8, 8, 9, 10, 11, 12, 13, 14, 16, 17, 19, 20, 22, 22, 22, 22],
];

// Envelope generator increment patterns from MAME (19 groups × 8 counter steps).
const EG_INC_TABLE: [[u16; 8]; 19] = [
    [0, 1, 0, 1, 0, 1, 0, 1], //  0: rates 00..11 pattern 0
    [0, 1, 0, 1, 1, 1, 0, 1], //  1: rates 00..11 pattern 1
    [0, 1, 1, 1, 0, 1, 1, 1], //  2: rates 00..11 pattern 2
    [0, 1, 1, 1, 1, 1, 1, 1], //  3: rates 00..11 pattern 3
    [1, 1, 1, 1, 1, 1, 1, 1], //  4: rate 12 pattern 0
    [1, 1, 1, 2, 1, 1, 1, 2], //  5: rate 12 pattern 1
    [1, 2, 1, 2, 1, 2, 1, 2], //  6: rate 12 pattern 2
    [1, 2, 2, 2, 1, 2, 2, 2], //  7: rate 12 pattern 3
    [2, 2, 2, 2, 2, 2, 2, 2], //  8: rate 13 pattern 0
    [2, 2, 2, 4, 2, 2, 2, 4], //  9: rate 13 pattern 1
    [2, 4, 2, 4, 2, 4, 2, 4], // 10: rate 13 pattern 2
    [2, 4, 4, 4, 2, 4, 4, 4], // 11: rate 13 pattern 3
    [4, 4, 4, 4, 4, 4, 4, 4], // 12: rate 14 pattern 0
    [4, 4, 4, 8, 4, 4, 4, 8], // 13: rate 14 pattern 1
    [4, 8, 4, 8, 4, 8, 4, 8], // 14: rate 14 pattern 2
    [4, 8, 8, 8, 4, 8, 8, 8], // 15: rate 14 pattern 3
    [8, 8, 8, 8, 8, 8, 8, 8], // 16: rate 15+
    [16, 16, 16, 16, 16, 16, 16, 16], // 17: maxed
    [0, 0, 0, 0, 0, 0, 0, 0], // 18: zero (no change)
];

// Maps effective rate (0-63) to EG_INC_TABLE row index.
const EG_RATE_SELECT: [u8; 64] = [
    18, 18, 2, 3, 0, 1, 2, 3,
    0, 1, 2, 3, 0, 1, 2, 3,
    0, 1, 2, 3, 0, 1, 2, 3,
    0, 1, 2, 3, 0, 1, 2, 3,
    0, 1, 2, 3, 0, 1, 2, 3,
    0, 1, 2, 3, 0, 1, 2, 3,
    4, 5, 6, 7, 8, 9, 10, 11,
    12, 13, 14, 15, 16, 17, 17, 17,
];

// Maps effective rate (0-63) to counter right-shift amount.
const EG_RATE_SHIFT: [u8; 64] = [
    0, 0, 11, 11, 10, 10, 10, 10,
    9, 9, 9, 9, 8, 8, 8, 8,
    7, 7, 7, 7, 6, 6, 6, 6,
    5, 5, 5, 5, 4, 4, 4, 4,
    3, 3, 3, 3, 2, 2, 2, 2,
    1, 1, 1, 1, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0,
];

// LFO timer increment (16.16 fixed-point) per hardware sample, indexed by LFO rate register.
// LFO counter is 7-bit (0-127). Increment = round(lfo_hz * 128 * 65536 / 53267).
const LFO_TIMER_INC: [u32; 8] = [627, 876, 948, 1003, 1083, 1517, 7575, 11370];

// LFO PM output table from GenPlusGX/Nuked: per-bit modulation values.
// Indexed by [7 FNUM bits (4-10) * 8 FMS depths][8 LFO steps].
// Each row stores the PM displacement contribution for one FNUM bit at one FMS depth.
const LFO_PM_OUTPUT: [[u8; 8]; 56] = [
    // FNUM bit 4: 0, 0, 0, 0, 0, 0, 0, 0
    [0,0,0,0,0,0,0,0], [0,0,0,0,0,0,0,0], [0,0,0,0,0,0,0,0], [0,0,0,0,0,0,0,0],
    [0,0,0,0,0,0,0,0], [0,0,0,0,0,0,0,0], [0,0,0,0,0,0,0,0],
    // FNUM bit 4, depth 7
    [0,0,0,0,1,1,1,1],
    // FNUM bit 5: 0..
    [0,0,0,0,0,0,0,0], [0,0,0,0,0,0,0,0], [0,0,0,0,0,0,0,0], [0,0,0,0,0,0,0,0],
    [0,0,0,0,0,0,0,0], [0,0,0,0,0,0,0,0],
    // FNUM bit 5, depth 6
    [0,0,0,0,1,1,1,1],
    // FNUM bit 5, depth 7
    [0,0,1,1,2,2,2,3],
    // FNUM bit 6
    [0,0,0,0,0,0,0,0], [0,0,0,0,0,0,0,0], [0,0,0,0,0,0,0,0], [0,0,0,0,0,0,0,0],
    [0,0,0,0,0,0,0,0],
    // FNUM bit 6, depth 5
    [0,0,0,0,1,1,1,1],
    // FNUM bit 6, depth 6
    [0,0,1,1,2,2,2,3],
    // FNUM bit 6, depth 7
    [0,0,2,3,4,4,5,6],
    // FNUM bit 7
    [0,0,0,0,0,0,0,0], [0,0,0,0,0,0,0,0], [0,0,0,0,0,0,0,0], [0,0,0,0,0,0,0,0],
    // FNUM bit 7, depth 4
    [0,0,0,0,1,1,1,1],
    // FNUM bit 7, depth 5
    [0,0,1,1,2,2,2,3],
    // FNUM bit 7, depth 6
    [0,0,2,3,4,4,5,6],
    // FNUM bit 7, depth 7
    [0,0,4,6,8,8,0x0A,0x0C],
    // FNUM bit 8
    [0,0,0,0,0,0,0,0], [0,0,0,0,0,0,0,0], [0,0,0,0,0,0,0,0],
    // FNUM bit 8, depth 3
    [0,0,0,0,1,1,1,1],
    // FNUM bit 8, depth 4
    [0,0,1,1,2,2,2,3],
    // FNUM bit 8, depth 5
    [0,0,2,3,4,4,5,6],
    // FNUM bit 8, depth 6
    [0,0,4,6,8,8,0x0A,0x0C],
    // FNUM bit 8, depth 7
    [0,0,8,0x0C,0x10,0x10,0x14,0x18],
    // FNUM bit 9
    [0,0,0,0,0,0,0,0], [0,0,0,0,0,0,0,0],
    // FNUM bit 9, depth 2
    [0,0,0,0,1,1,1,1],
    // FNUM bit 9, depth 3
    [0,0,1,1,2,2,2,3],
    // FNUM bit 9, depth 4
    [0,0,2,3,4,4,5,6],
    // FNUM bit 9, depth 5
    [0,0,4,6,8,8,0x0A,0x0C],
    // FNUM bit 9, depth 6
    [0,0,8,0x0C,0x10,0x10,0x14,0x18],
    // FNUM bit 9, depth 7
    [0,0,0x10,0x18,0x20,0x20,0x28,0x30],
    // FNUM bit 10
    [0,0,0,0,0,0,0,0],
    // FNUM bit 10, depth 1
    [0,0,0,0,1,1,1,1],
    // FNUM bit 10, depth 2
    [0,0,1,1,2,2,2,3],
    // FNUM bit 10, depth 3
    [0,0,2,3,4,4,5,6],
    // FNUM bit 10, depth 4
    [0,0,4,6,8,8,0x0A,0x0C],
    // FNUM bit 10, depth 5
    [0,0,8,0x0C,0x10,0x10,0x14,0x18],
    // FNUM bit 10, depth 6
    [0,0,0x10,0x18,0x20,0x20,0x28,0x30],
    // FNUM bit 10, depth 7
    [0,0,0x20,0x30,0x40,0x40,0x50,0x60],
];

// Hardware fn_note table: maps (fnum >> 7) & 0x0F to note value for keycode calculation.
// Nuked OPN2: kcode = (block << 2) | fn_note[(fnum >> 7) & 0x0f]
const FN_NOTE: [u8; 16] = [0, 0, 0, 0, 0, 0, 0, 1, 2, 3, 3, 3, 3, 3, 3, 3];

// Phase accumulator uses 32-bit: top 20 bits = hardware phase, bottom 12 bits = fractional.
// Hardware sample rate: master_clock / (6 * 24) = 7670454 / 144 ≈ 53267 Hz.
const YM_HW_RATE: u64 = 7_670_454 / 144;
// EG cycles at HW_RATE / 3 ≈ 17756 Hz.

/// Hardware sine ROM + exponential ROM lookup.
/// `phase_10bit`: top 10 bits of the 20-bit phase (sign + mirror + 8-bit index).
/// `attenuation`: total attenuation in 4.8 fixed-point log2 domain.
/// Returns signed 14-bit value (approx ±8188).
fn op_calc(phase_10bit: u32, attenuation: u32) -> i32 {
    let sign = (phase_10bit >> 9) & 1;
    let mirror = (phase_10bit >> 8) & 1;
    let index = if mirror != 0 {
        255 - (phase_10bit & 0xFF)
    } else {
        phase_10bit & 0xFF
    } as usize;

    let log_sin = LOG_SIN_TABLE[index] as u32;
    let total = (log_sin + attenuation).min(0xFFF);

    let shift = total >> 8;
    let exp_index = (total & 0xFF) as usize;
    // 10-bit significand + implicit bit = 11 bits, left-shift 2 for 13-bit, then right-shift by exponent.
    let linear = ((EXP_TABLE[exp_index] as u32 | 0x400) << 2) >> shift;

    if sign != 0 {
        -(linear as i32)
    } else {
        linear as i32
    }
}

/// Look up EG increment for a given effective rate and global EG counter.
/// MAME/GenPlusGX: only fires when lower `shift` bits of counter are zero.
fn eg_increment(effective_rate: u8, eg_counter: u32) -> u16 {
    let rate = effective_rate.min(63) as usize;
    let shift = EG_RATE_SHIFT[rate] as u32;
    // Gate: only update when counter is aligned to shift boundary
    if shift > 0 && (eg_counter & ((1u32 << shift) - 1)) != 0 {
        return 0;
    }
    let row = EG_RATE_SELECT[rate] as usize;
    let step = ((eg_counter >> shift) & 7) as usize;
    EG_INC_TABLE[row][step]
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, bincode::Encode, bincode::Decode)]
enum YmEnvelopePhase {
    Off,
    Attack,
    Decay,
    Sustain,
    Release,
}

#[derive(Debug, Clone, Copy, bincode::Encode, bincode::Decode)]
struct YmOperator {
    detune: u8,
    mul: u8,
    tl: u8,
    key_scale: u8,
    am_enable: bool,
    ssg_eg: u8,
    ssg_invert: bool,
    ssg_hold_active: bool,
    attack_rate: u8,
    decay_rate: u8,
    sustain_rate: u8,
    sustain_level: u8,
    release_rate: u8,
    key_on: bool,       // composite: reg_key_on || csm_key_on
    reg_key_on: bool,   // key-on from register 0x28
    csm_key_on: bool,   // key-on from CSM Timer A overflow
    phase: u32,
    envelope_phase: YmEnvelopePhase,
    envelope_level: u16,
    last_output: i32,
}

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

#[derive(Debug, Clone, Copy, bincode::Encode, bincode::Decode)]
struct YmChannel {
    fnum: u16,
    block: u8,
    special_fnum: [u16; 3],
    special_block: [u8; 3],
    algorithm: u8,
    feedback: u8,
    feedback_sample: i32,
    feedback_sample_prev: i32,
    pan_left: bool,
    pan_right: bool,
    ams: u8,
    fms: u8,
    operators: [YmOperator; 4],
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

#[derive(Debug, Clone, bincode::Encode, bincode::Decode)]
pub struct Ym2612 {
    addr_port0: u8,
    addr_port1: u8,
    regs: [[u8; 256]; 2],
    writes: u64,
    dac_data_writes: u64,
    busy_z80_cycles: u32,
    timer_status: u8,
    timer_control: u8,
    timer_a_value: u16,
    timer_b_value: u8,
    timer_clock_accumulator: u64,
    timer_a_elapsed_ym_cycles: u64,
    timer_b_elapsed_ym_cycles: u64,
    dac_enabled: bool,
    dac_output: i16,
    dac_enabled_pending: Option<bool>,
    dac_output_pending: Option<i16>,
    lfo_enabled: bool,
    lfo_rate: u8,
    lfo_counter: u8,
    lfo_timer: u32,
    eg_counter: u32,
    hw_sample_frac: u32,
    hw_eg_divider: u8,
    last_hw_output: (i16, i16),
    csm_key_on_active: bool,
    csm_key_on_rendered: bool,
    test_register: u8, // reg 0x21: test mode bits (normally 0)
    channels: [YmChannel; 6],
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, bincode::Encode, bincode::Decode)]
enum YmOperatorParam {
    Mul,
    Tl,
    Attack,
    Decay,
    SustainRate,
    SustainRelease,
    SsgEg,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum YmAlgorithmBus {
    M2,
    C1,
    C2,
    Mem,
    Out,
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

    fn write_port(&mut self, port: u8, value: u8) {
        self.write_port_internal(port, value, false);
    }

    fn write_port_from_z80(&mut self, port: u8, value: u8) {
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
                                    op.envelope_level = (0x200u16.wrapping_sub(op.envelope_level)) & 0x3FF;
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
        let (fnum, block) = Self::operator_fnum_block(channel, operator_index, channel3_special_mode);
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
        if mul == 0 { detuned >> 1 } else { detuned * (mul as u32) }
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
                let effective_rate =
                    ((op.release_rate as u16) * 4 + 2 + ksv).min(63) as u8;
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
        if pm_sign {
            -displacement
        } else {
            displacement
        }
    }

    /// Advance the global EG counter by one HW sample tick.
    /// EG advances every 3 HW samples (HW_RATE / 3 = EG_RATE).
    fn advance_eg_counter_hw(&mut self) -> bool {
        self.hw_eg_divider += 1;
        if self.hw_eg_divider >= 3 {
            self.hw_eg_divider = 0;
            self.eg_counter = self.eg_counter.wrapping_add(1);
            if self.eg_counter == 0 { self.eg_counter = 1; } // Nuked OPN2: skip 0
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
        let eg_level = if eg_test { 0 } else { Self::ssg_eg_output_level(op) };
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
            let (fnum, block) =
                Self::operator_fnum_block(channel, i, channel3_special_mode);
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
        let left = ((left_dac as i64 * 18000) >> 13)
            .clamp(i16::MIN as i64, i16::MAX as i64) as i16;
        let right = ((right_dac as i64 * 18000) >> 13)
            .clamp(i16::MIN as i64, i16::MAX as i64) as i16;
        (left, right)
    }

    /// Produce one output sample at the requested sample rate by running the
    /// internal HW clock at 53267 Hz (zero-order hold downsampling).
    /// Uses the most recent HW sample, avoiding box-filter averaging artifacts
    /// from variable sample counts (1 or 2 HW samples per output sample).
    fn next_sample_stereo(&mut self, sample_rate_hz: u32) -> (i16, i16) {
        let hw_rate = YM_HW_RATE as u32;
        self.hw_sample_frac += hw_rate;

        while self.hw_sample_frac >= sample_rate_hz {
            self.hw_sample_frac -= sample_rate_hz;
            self.last_hw_output = self.render_one_hw_sample();
        }

        self.last_hw_output
    }

    fn step_z80_cycles(&mut self, cycles: u32) {
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

    fn read_status(&self) -> u8 {
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

#[derive(Debug, Clone, bincode::Encode, bincode::Decode)]
pub struct Psg {
    last_data: u8,
    writes: u64,
    latched_channel: usize,
    latched_is_volume: bool,
    tone_period: [u16; 3],
    tone_output: [bool; 3],       // current output state (replaces tone_phase_high)
    tone_counter: [u16; 3],       // 10-bit downcounters (replaces tone_phase_acc)
    attenuation: [u8; 4],
    noise_control: u8,
    noise_lfsr: u16,
    noise_counter: u16,           // noise downcounter (replaces noise_phase_acc)
    sample_counter: u32,          // Bresenham counter for PSG clock vs output rate
}

// Pre-computed: round(8000.0 * 10^(-att*2/20)) for att=0..14, att=15→0
const PSG_VOLUME: [i16; 16] = [
    8000, 6355, 5048, 4009, 3184, 2529, 2009, 1596,
    1268, 1007, 800, 635, 505, 401, 318, 0,
];

impl Default for Psg {
    fn default() -> Self {
        Self {
            last_data: 0,
            writes: 0,
            latched_channel: 0,
            latched_is_volume: false,
            tone_period: [1, 1, 1],
            tone_output: [true, true, true],
            tone_counter: [1, 1, 1],
            attenuation: [0x0F; 4],
            noise_control: 0,
            noise_lfsr: 0x4000,
            noise_counter: 0x10,
            sample_counter: 0,
        }
    }
}

impl Psg {
    // SN76489 internal clock = master / 16
    const PSG_CLOCK_HZ: u32 = 3_579_545 / 16; // 223,721 Hz

    fn write_data(&mut self, value: u8) {
        self.last_data = value;
        self.writes += 1;
        if (value & 0x80) != 0 {
            self.latched_channel = ((value >> 5) & 0x3) as usize;
            self.latched_is_volume = (value & 0x10) != 0;
            let data = value & 0x0F;
            self.apply_latched_data(data);
            return;
        }

        if self.latched_is_volume {
            self.attenuation[self.latched_channel] = value & 0x0F;
        } else if self.latched_channel < 3 {
            let lo = self.tone_period[self.latched_channel] & 0x000F;
            let hi = ((value & 0x3F) as u16) << 4;
            self.tone_period[self.latched_channel] = lo | hi;
        }
    }

    pub fn last_data(&self) -> u8 {
        self.last_data
    }

    pub fn writes(&self) -> u64 {
        self.writes
    }

    pub fn tone_period(&self, channel: usize) -> u16 {
        self.tone_period[channel.min(2)]
    }

    pub fn attenuation(&self, channel: usize) -> u8 {
        self.attenuation[channel.min(3)]
    }

    pub fn noise_control(&self) -> u8 {
        self.noise_control
    }

    pub fn tone_frequency_hz_debug(&self, channel: usize) -> f32 {
        let raw_period = self.tone_period[channel.min(2)] & 0x03FF;
        let period = raw_period.max(1) as f32;
        3_579_545.0 / (32.0 * period)
    }

    fn apply_latched_data(&mut self, data: u8) {
        if self.latched_is_volume {
            self.attenuation[self.latched_channel] = data & 0x0F;
            return;
        }

        if self.latched_channel < 3 {
            let hi = self.tone_period[self.latched_channel] & 0x03F0;
            self.tone_period[self.latched_channel] = hi | data as u16;
        } else {
            self.noise_control = data & 0x07;
            self.noise_lfsr = 0x4000;
            self.noise_counter = Self::noise_period(data & 0x07, self.tone_period[2]);
        }
    }

    fn noise_period(noise_control: u8, tone3_period: u16) -> u16 {
        match noise_control & 0x03 {
            0x00 => 0x10,   // clock/512 → period 16
            0x01 => 0x20,   // clock/1024 → period 32
            0x02 => 0x40,   // clock/2048 → period 64
            _ => tone3_period.max(1), // use tone channel 3 period
        }
    }

    fn clock_noise_lfsr(&mut self) {
        let bit0 = self.noise_lfsr & 1;
        let feedback = if (self.noise_control & 0x04) != 0 {
            let bit3 = (self.noise_lfsr >> 3) & 1;
            bit0 ^ bit3
        } else {
            bit0
        };
        self.noise_lfsr = ((self.noise_lfsr >> 1) | (feedback << 14)) & 0x7FFF;
    }

    /// Advance PSG by one internal clock tick
    fn clock_tick(&mut self) {
        let noise_uses_tone3 = (self.noise_control & 0x03) == 0x03;

        // Advance tone counters
        for ch in 0..3 {
            if self.tone_counter[ch] > 0 {
                self.tone_counter[ch] -= 1;
            }
            if self.tone_counter[ch] == 0 {
                let period = (self.tone_period[ch] & 0x3FF).max(1);
                self.tone_counter[ch] = period;
                let was_high = self.tone_output[ch];
                self.tone_output[ch] = !self.tone_output[ch];
                // Noise channel clocked by tone3 falling edge
                if noise_uses_tone3 && ch == 2 && was_high && !self.tone_output[ch] {
                    self.clock_noise_lfsr();
                }
            }
        }

        // Advance noise counter (independent clock unless using tone3)
        if !noise_uses_tone3 {
            if self.noise_counter > 0 {
                self.noise_counter -= 1;
            }
            if self.noise_counter == 0 {
                self.noise_counter = Self::noise_period(self.noise_control, self.tone_period[2]);
                self.clock_noise_lfsr();
            }
        }
    }

    fn next_sample(&mut self, sample_rate_hz: u32) -> i16 {
        // Bresenham resampler: PSG_CLOCK_HZ → sample_rate_hz
        self.sample_counter += Self::PSG_CLOCK_HZ;
        while self.sample_counter >= sample_rate_hz {
            self.sample_counter -= sample_rate_hz;
            self.clock_tick();
        }

        // Mix using pre-computed integer volume table
        let mut mix = 0i32;
        for ch in 0..3 {
            let vol = PSG_VOLUME[self.attenuation[ch].min(15) as usize] as i32;
            mix += if self.tone_output[ch] { vol } else { -vol };
        }
        let noise_vol = PSG_VOLUME[self.attenuation[3].min(15) as usize] as i32;
        mix += if (self.noise_lfsr & 1) != 0 { noise_vol } else { -noise_vol };

        // Scale to match previous float output level (~1800.0 * amplitude)
        // PSG_VOLUME[0]=8000, old was 1.0*1800=1800, so scale by 1800/8000 ≈ 9/40
        ((mix * 9) / 40).clamp(i16::MIN as i32, i16::MAX as i32) as i16
    }
}

#[derive(Debug, Clone, bincode::Encode, bincode::Decode)]
pub struct AudioBus {
    ym2612: Ym2612,
    psg: Psg,
    ym_writes_from_68k: u64,
    ym_writes_from_z80: u64,
    psg_writes_from_68k: u64,
    psg_writes_from_z80: u64,
    cycles: u64,
    output_sample_rate_hz: u64,
    sample_accumulator: u64,
    sample_buffer: Vec<i16>,
}

impl AudioBus {
    const M68K_CLOCK_HZ: u64 = 7_670_454;
    const DEFAULT_OUTPUT_SAMPLE_RATE_HZ: u64 = 44_100;
    const OUTPUT_CHANNELS: u8 = 2;

    pub fn new() -> Self {
        Self::default()
    }

    pub fn output_sample_rate_hz(&self) -> u32 {
        self.output_sample_rate_hz as u32
    }

    pub fn output_channels(&self) -> u8 {
        Self::OUTPUT_CHANNELS
    }

    pub fn set_output_sample_rate_hz(&mut self, hz: u32) {
        self.output_sample_rate_hz = (hz as u64).clamp(8_000, 192_000);
    }

    pub fn read_ym2612(&self, port: u8) -> u8 {
        if (port & 0x01) == 0 {
            self.ym2612.read_status()
        } else {
            0xFF
        }
    }

    pub fn write_ym2612(&mut self, port: u8, value: u8) {
        self.ym_writes_from_68k += 1;
        self.ym2612.write_port(port, value);
    }

    pub fn write_ym2612_from_z80(&mut self, port: u8, value: u8) {
        self.ym_writes_from_z80 += 1;
        self.ym2612.write_port_from_z80(port, value);
    }

    pub fn write_psg(&mut self, value: u8) {
        self.psg_writes_from_68k += 1;
        self.psg.write_data(value);
    }

    pub fn write_psg_from_z80(&mut self, value: u8) {
        self.psg_writes_from_z80 += 1;
        self.psg.write_data(value);
    }

    pub fn step_z80_cycles(&mut self, z80_cycles: u32) {
        self.ym2612.step_z80_cycles(z80_cycles);
    }

    pub fn step(&mut self, m68k_cycles: u32) {
        self.cycles += m68k_cycles as u64;
        let sample_rate_hz = self.output_sample_rate_hz.max(1);
        self.sample_accumulator += m68k_cycles as u64 * sample_rate_hz;
        let produced = (self.sample_accumulator / Self::M68K_CLOCK_HZ) as usize;
        self.sample_accumulator %= Self::M68K_CLOCK_HZ;
        for _ in 0..produced {
            let psg_oversample_u64 = 4u64;
            let psg_oversample_i32 = 4i32;
            let mut psg_acc = 0i32;
            for _ in 0..psg_oversample_u64 {
                psg_acc += self
                    .psg
                    .next_sample((sample_rate_hz * psg_oversample_u64) as u32)
                    as i32;
            }
            let psg_sample = (psg_acc / psg_oversample_i32) * 2 / 5;
            let (ym_left, ym_right) = self.ym2612.next_sample_stereo(sample_rate_hz as u32);
            let left = (psg_sample + ym_left as i32).clamp(i16::MIN as i32, i16::MAX as i32) as i16;
            let right =
                (psg_sample + ym_right as i32).clamp(i16::MIN as i32, i16::MAX as i32) as i16;
            self.sample_buffer.push(left);
            self.sample_buffer.push(right);
        }
    }

    pub fn ym2612(&self) -> &Ym2612 {
        &self.ym2612
    }

    pub fn psg(&self) -> &Psg {
        &self.psg
    }

    pub fn ym_write_count(&self) -> u64 {
        self.ym2612.writes()
    }

    pub fn ym_dac_write_count(&self) -> u64 {
        self.ym2612.dac_data_writes()
    }

    pub fn psg_write_count(&self) -> u64 {
        self.psg.writes()
    }

    pub fn ym_writes_from_68k(&self) -> u64 {
        self.ym_writes_from_68k
    }

    pub fn ym_writes_from_z80(&self) -> u64 {
        self.ym_writes_from_z80
    }

    pub fn psg_writes_from_68k(&self) -> u64 {
        self.psg_writes_from_68k
    }

    pub fn psg_writes_from_z80(&self) -> u64 {
        self.psg_writes_from_z80
    }

    pub fn pending_samples(&self) -> usize {
        self.sample_buffer.len()
    }

    pub fn drain_samples(&mut self, max_samples: usize) -> Vec<i16> {
        let count = max_samples.min(self.sample_buffer.len());
        self.sample_buffer.drain(0..count).collect()
    }
}

impl Default for AudioBus {
    fn default() -> Self {
        Self {
            ym2612: Ym2612::default(),
            psg: Psg::default(),
            ym_writes_from_68k: 0,
            ym_writes_from_z80: 0,
            psg_writes_from_68k: 0,
            psg_writes_from_z80: 0,
            cycles: 0,
            output_sample_rate_hz: Self::DEFAULT_OUTPUT_SAMPLE_RATE_HZ,
            sample_accumulator: 0,
            sample_buffer: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        op_calc, Psg, Ym2612, LOG_SIN_TABLE, EXP_TABLE, DETUNE_TABLE,
    };

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
}
