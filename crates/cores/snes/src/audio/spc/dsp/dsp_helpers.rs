pub fn multiply_volume(value: i32, volume: u8) -> i32 {
    (value * ((volume as i8) as i32)) >> 7
}

pub fn clamp(value: i32) -> i32 {
    if value < -32768 {
        return -32768;
    } else if value > 32767 {
        return 32767;
    }
    return value;
}

/// BRR decoder clamp: limits to 15-bit signed range so that the subsequent
/// `<< 1` (producing a 16-bit sample with bit 0 = 0) cannot overflow `i16`.
///
/// On SNES hardware, the BRR decode pipeline works at full 16-bit scale and
/// applies `sclamp<16>` followed by `& !1`.  This crate's decoder works at
/// half-scale (the shift step includes `>> 1`) and doubles at the end with
/// `(sample << 1) as i16`.  Using the normal `clamp()` (±32767) allows values
/// in 16384..32767 to wrap when shifted, producing loud pops.
///
/// Clamping to ±16383 here makes `<< 1` produce ±32766, matching the real
/// hardware output range of −32768..32766 (even values only).
pub fn clamp_brr(value: i32) -> i32 {
    if value < -16384 {
        return -16384;
    } else if value > 16383 {
        return 16383;
    }
    value
}
