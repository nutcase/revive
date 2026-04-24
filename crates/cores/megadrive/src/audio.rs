mod bus;
mod psg;
#[cfg(test)]
mod tests;
mod ym2612;

// YM2612 hardware sine ROM: quarter-wave log-attenuation (256 entries, 12-bit 4.8 format).
// logsin[i] = round(-log2(sin((2*i+1) * PI / 1024)) * 256)
const LOG_SIN_TABLE: [u16; 256] = [
    2137, 1731, 1543, 1419, 1326, 1252, 1190, 1137, 1091, 1050, 1013, 979, 949, 920, 894, 869, 846,
    825, 804, 785, 767, 749, 732, 717, 701, 687, 672, 659, 646, 633, 621, 609, 598, 587, 576, 566,
    556, 546, 536, 527, 518, 509, 501, 492, 484, 476, 468, 461, 453, 446, 439, 432, 425, 418, 411,
    405, 399, 392, 386, 380, 375, 369, 363, 358, 352, 347, 341, 336, 331, 326, 321, 316, 311, 307,
    302, 297, 293, 289, 284, 280, 276, 271, 267, 263, 259, 255, 251, 248, 244, 240, 236, 233, 229,
    226, 222, 219, 215, 212, 209, 205, 202, 199, 196, 193, 190, 187, 184, 181, 178, 175, 172, 169,
    167, 164, 161, 159, 156, 153, 151, 148, 146, 143, 141, 138, 136, 134, 131, 129, 127, 125, 122,
    120, 118, 116, 114, 112, 110, 108, 106, 104, 102, 100, 98, 96, 94, 92, 91, 89, 87, 85, 83, 82,
    80, 78, 77, 75, 74, 72, 70, 69, 67, 66, 64, 63, 62, 60, 59, 57, 56, 55, 53, 52, 51, 49, 48, 47,
    46, 45, 43, 42, 41, 40, 39, 38, 37, 36, 35, 34, 33, 32, 31, 30, 29, 28, 27, 26, 25, 24, 23, 23,
    22, 21, 20, 20, 19, 18, 17, 17, 16, 15, 15, 14, 13, 13, 12, 12, 11, 10, 10, 9, 9, 8, 8, 7, 7,
    7, 6, 6, 5, 5, 5, 4, 4, 4, 3, 3, 3, 2, 2, 2, 2, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0,
];

// YM2612 hardware exponential ROM: converts 8-bit log mantissa to 10-bit linear.
// exp[i] = round((pow(2, (255-i)/256.0) - 1.0) * 1024)
const EXP_TABLE: [u16; 256] = [
    1018, 1013, 1007, 1002, 996, 991, 986, 980, 975, 969, 964, 959, 953, 948, 942, 937, 932, 927,
    921, 916, 911, 906, 900, 895, 890, 885, 880, 874, 869, 864, 859, 854, 849, 844, 839, 834, 829,
    824, 819, 814, 809, 804, 799, 794, 789, 784, 779, 774, 770, 765, 760, 755, 750, 745, 741, 736,
    731, 726, 722, 717, 712, 708, 703, 698, 693, 689, 684, 680, 675, 670, 666, 661, 657, 652, 648,
    643, 639, 634, 630, 625, 621, 616, 612, 607, 603, 599, 594, 590, 585, 581, 577, 572, 568, 564,
    560, 555, 551, 547, 542, 538, 534, 530, 526, 521, 517, 513, 509, 505, 501, 496, 492, 488, 484,
    480, 476, 472, 468, 464, 460, 456, 452, 448, 444, 440, 436, 432, 428, 424, 420, 416, 412, 409,
    405, 401, 397, 393, 389, 385, 382, 378, 374, 370, 367, 363, 359, 355, 352, 348, 344, 340, 337,
    333, 329, 326, 322, 318, 315, 311, 308, 304, 300, 297, 293, 290, 286, 283, 279, 276, 272, 268,
    265, 262, 258, 255, 251, 248, 244, 241, 237, 234, 231, 227, 224, 220, 217, 214, 210, 207, 204,
    200, 197, 194, 190, 187, 184, 181, 177, 174, 171, 168, 164, 161, 158, 155, 152, 148, 145, 142,
    139, 136, 133, 130, 126, 123, 120, 117, 114, 111, 108, 105, 102, 99, 96, 93, 90, 87, 84, 81,
    78, 75, 72, 69, 66, 63, 60, 57, 54, 51, 48, 45, 42, 40, 37, 34, 31, 28, 25, 22, 20, 17, 14, 11,
    8, 6, 3, 0,
];

// Hardware detune table from MAME: phase increment delta indexed by [dt_base (0-3)][keycode (0-31)].
// DT1 register bits [2:0]: values 4-7 negate the delta.
const DETUNE_TABLE: [[u8; 32]; 4] = [
    [
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0,
    ],
    [
        0, 0, 0, 0, 1, 1, 1, 1, 1, 1, 1, 1, 2, 2, 2, 2, 2, 3, 3, 3, 4, 4, 4, 5, 5, 6, 6, 7, 8, 8,
        8, 8,
    ],
    [
        1, 1, 1, 1, 2, 2, 2, 2, 2, 3, 3, 3, 4, 4, 4, 5, 5, 6, 6, 7, 8, 8, 9, 10, 11, 12, 13, 14,
        16, 16, 16, 16,
    ],
    [
        2, 2, 2, 2, 2, 3, 3, 3, 4, 4, 4, 5, 5, 6, 6, 7, 8, 8, 9, 10, 11, 12, 13, 14, 16, 17, 19,
        20, 22, 22, 22, 22,
    ],
];

// Envelope generator increment patterns from MAME (19 groups × 8 counter steps).
const EG_INC_TABLE: [[u16; 8]; 19] = [
    [0, 1, 0, 1, 0, 1, 0, 1],         //  0: rates 00..11 pattern 0
    [0, 1, 0, 1, 1, 1, 0, 1],         //  1: rates 00..11 pattern 1
    [0, 1, 1, 1, 0, 1, 1, 1],         //  2: rates 00..11 pattern 2
    [0, 1, 1, 1, 1, 1, 1, 1],         //  3: rates 00..11 pattern 3
    [1, 1, 1, 1, 1, 1, 1, 1],         //  4: rate 12 pattern 0
    [1, 1, 1, 2, 1, 1, 1, 2],         //  5: rate 12 pattern 1
    [1, 2, 1, 2, 1, 2, 1, 2],         //  6: rate 12 pattern 2
    [1, 2, 2, 2, 1, 2, 2, 2],         //  7: rate 12 pattern 3
    [2, 2, 2, 2, 2, 2, 2, 2],         //  8: rate 13 pattern 0
    [2, 2, 2, 4, 2, 2, 2, 4],         //  9: rate 13 pattern 1
    [2, 4, 2, 4, 2, 4, 2, 4],         // 10: rate 13 pattern 2
    [2, 4, 4, 4, 2, 4, 4, 4],         // 11: rate 13 pattern 3
    [4, 4, 4, 4, 4, 4, 4, 4],         // 12: rate 14 pattern 0
    [4, 4, 4, 8, 4, 4, 4, 8],         // 13: rate 14 pattern 1
    [4, 8, 4, 8, 4, 8, 4, 8],         // 14: rate 14 pattern 2
    [4, 8, 8, 8, 4, 8, 8, 8],         // 15: rate 14 pattern 3
    [8, 8, 8, 8, 8, 8, 8, 8],         // 16: rate 15+
    [16, 16, 16, 16, 16, 16, 16, 16], // 17: maxed
    [0, 0, 0, 0, 0, 0, 0, 0],         // 18: zero (no change)
];

// Maps effective rate (0-63) to EG_INC_TABLE row index.
const EG_RATE_SELECT: [u8; 64] = [
    18, 18, 2, 3, 0, 1, 2, 3, 0, 1, 2, 3, 0, 1, 2, 3, 0, 1, 2, 3, 0, 1, 2, 3, 0, 1, 2, 3, 0, 1, 2,
    3, 0, 1, 2, 3, 0, 1, 2, 3, 0, 1, 2, 3, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15,
    16, 17, 17, 17,
];

// Maps effective rate (0-63) to counter right-shift amount.
const EG_RATE_SHIFT: [u8; 64] = [
    0, 0, 11, 11, 10, 10, 10, 10, 9, 9, 9, 9, 8, 8, 8, 8, 7, 7, 7, 7, 6, 6, 6, 6, 5, 5, 5, 5, 4, 4,
    4, 4, 3, 3, 3, 3, 2, 2, 2, 2, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0,
];

// LFO timer increment (16.16 fixed-point) per hardware sample, indexed by LFO rate register.
// LFO counter is 7-bit (0-127). Increment = round(lfo_hz * 128 * 65536 / 53267).
const LFO_TIMER_INC: [u32; 8] = [627, 876, 948, 1003, 1083, 1517, 7575, 11370];

// LFO PM output table from GenPlusGX/Nuked: per-bit modulation values.
// Indexed by [7 FNUM bits (4-10) * 8 FMS depths][8 LFO steps].
// Each row stores the PM displacement contribution for one FNUM bit at one FMS depth.
const LFO_PM_OUTPUT: [[u8; 8]; 56] = [
    // FNUM bit 4: 0, 0, 0, 0, 0, 0, 0, 0
    [0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0],
    // FNUM bit 4, depth 7
    [0, 0, 0, 0, 1, 1, 1, 1],
    // FNUM bit 5: 0..
    [0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0],
    // FNUM bit 5, depth 6
    [0, 0, 0, 0, 1, 1, 1, 1],
    // FNUM bit 5, depth 7
    [0, 0, 1, 1, 2, 2, 2, 3],
    // FNUM bit 6
    [0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0],
    // FNUM bit 6, depth 5
    [0, 0, 0, 0, 1, 1, 1, 1],
    // FNUM bit 6, depth 6
    [0, 0, 1, 1, 2, 2, 2, 3],
    // FNUM bit 6, depth 7
    [0, 0, 2, 3, 4, 4, 5, 6],
    // FNUM bit 7
    [0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0],
    // FNUM bit 7, depth 4
    [0, 0, 0, 0, 1, 1, 1, 1],
    // FNUM bit 7, depth 5
    [0, 0, 1, 1, 2, 2, 2, 3],
    // FNUM bit 7, depth 6
    [0, 0, 2, 3, 4, 4, 5, 6],
    // FNUM bit 7, depth 7
    [0, 0, 4, 6, 8, 8, 0x0A, 0x0C],
    // FNUM bit 8
    [0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0],
    // FNUM bit 8, depth 3
    [0, 0, 0, 0, 1, 1, 1, 1],
    // FNUM bit 8, depth 4
    [0, 0, 1, 1, 2, 2, 2, 3],
    // FNUM bit 8, depth 5
    [0, 0, 2, 3, 4, 4, 5, 6],
    // FNUM bit 8, depth 6
    [0, 0, 4, 6, 8, 8, 0x0A, 0x0C],
    // FNUM bit 8, depth 7
    [0, 0, 8, 0x0C, 0x10, 0x10, 0x14, 0x18],
    // FNUM bit 9
    [0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0],
    // FNUM bit 9, depth 2
    [0, 0, 0, 0, 1, 1, 1, 1],
    // FNUM bit 9, depth 3
    [0, 0, 1, 1, 2, 2, 2, 3],
    // FNUM bit 9, depth 4
    [0, 0, 2, 3, 4, 4, 5, 6],
    // FNUM bit 9, depth 5
    [0, 0, 4, 6, 8, 8, 0x0A, 0x0C],
    // FNUM bit 9, depth 6
    [0, 0, 8, 0x0C, 0x10, 0x10, 0x14, 0x18],
    // FNUM bit 9, depth 7
    [0, 0, 0x10, 0x18, 0x20, 0x20, 0x28, 0x30],
    // FNUM bit 10
    [0, 0, 0, 0, 0, 0, 0, 0],
    // FNUM bit 10, depth 1
    [0, 0, 0, 0, 1, 1, 1, 1],
    // FNUM bit 10, depth 2
    [0, 0, 1, 1, 2, 2, 2, 3],
    // FNUM bit 10, depth 3
    [0, 0, 2, 3, 4, 4, 5, 6],
    // FNUM bit 10, depth 4
    [0, 0, 4, 6, 8, 8, 0x0A, 0x0C],
    // FNUM bit 10, depth 5
    [0, 0, 8, 0x0C, 0x10, 0x10, 0x14, 0x18],
    // FNUM bit 10, depth 6
    [0, 0, 0x10, 0x18, 0x20, 0x20, 0x28, 0x30],
    // FNUM bit 10, depth 7
    [0, 0, 0x20, 0x30, 0x40, 0x40, 0x50, 0x60],
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
    key_on: bool,     // composite: reg_key_on || csm_key_on
    reg_key_on: bool, // key-on from register 0x28
    csm_key_on: bool, // key-on from CSM Timer A overflow
    phase: u32,
    envelope_phase: YmEnvelopePhase,
    envelope_level: u16,
    last_output: i32,
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

#[derive(Debug, Clone, bincode::Encode, bincode::Decode)]
pub struct Psg {
    last_data: u8,
    writes: u64,
    latched_channel: usize,
    latched_is_volume: bool,
    tone_period: [u16; 3],
    tone_output: [bool; 3], // current output state (replaces tone_phase_high)
    tone_counter: [u16; 3], // 10-bit downcounters (replaces tone_phase_acc)
    attenuation: [u8; 4],
    noise_control: u8,
    noise_lfsr: u16,
    noise_counter: u16,  // noise downcounter (replaces noise_phase_acc)
    sample_counter: u32, // Bresenham counter for PSG clock vs output rate
}

// Pre-computed: round(8000.0 * 10^(-att*2/20)) for att=0..14, att=15→0
const PSG_VOLUME: [i16; 16] = [
    8000, 6355, 5048, 4009, 3184, 2529, 2009, 1596, 1268, 1007, 800, 635, 505, 401, 318, 0,
];

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
