pub(super) const PSG_CLOCK_HZ: u32 = 7_159_090 / 2;
pub(super) const AUDIO_SAMPLE_RATE: u32 = 44_100;

pub(crate) const PSG_REG_COUNT: usize = 32;
pub(crate) const PSG_CHANNEL_COUNT: usize = 6;
pub(crate) const PSG_WAVE_SIZE: usize = 32;
pub(crate) const PSG_REG_CH_SELECT: usize = 0x00;
pub(crate) const PSG_REG_MAIN_BALANCE: usize = 0x01;
pub(crate) const PSG_REG_FREQ_LO: usize = 0x02;
pub(crate) const PSG_REG_FREQ_HI: usize = 0x03;
pub(crate) const PSG_REG_CH_CONTROL: usize = 0x04;
pub(crate) const PSG_REG_CH_BALANCE: usize = 0x05;
pub(crate) const PSG_REG_WAVE_DATA: usize = 0x06;
pub(crate) const PSG_REG_NOISE_CTRL: usize = 0x07;
pub(crate) const PSG_REG_LFO_FREQ: usize = 0x08;
pub(crate) const PSG_REG_LFO_CTRL: usize = 0x09;
pub(crate) const PSG_REG_TIMER_LO: usize = 0x18;
pub(crate) const PSG_REG_TIMER_HI: usize = 0x19;
pub(crate) const PSG_REG_TIMER_CTRL: usize = 0x1A;
pub(crate) const PSG_CTRL_ENABLE: u8 = 0x01;
pub(crate) const PSG_CTRL_IRQ_ENABLE: u8 = 0x02;
pub(super) const PSG_STATUS_IRQ: u8 = 0x80;
pub(crate) const PSG_CH_CTRL_VOLUME_MASK: u8 = 0x1F;
pub(crate) const PSG_CH_CTRL_DDA: u8 = 0x40;
pub(crate) const PSG_CH_CTRL_KEY_ON: u8 = 0x80;
pub(crate) const PSG_NOISE_ENABLE: u8 = 0x80;
pub(super) const PSG_NOISE_FREQ_MASK: u8 = 0x1F;
pub(super) const PSG_PHASE_FRAC_BITS: u32 = 12;
pub(super) const PSG_PERIOD_ENTRIES: usize = 0x1000;
// Keep headroom for 6 channels with signed 5-bit samples (-31..31) after
// attenuation.  170 keeps the theoretical full-scale mix below i16 clipping.
pub(super) const PSG_OUTPUT_GAIN: i32 = 170;

/// Logarithmic volume table (Mednafen-compatible).
/// Index = attenuation level (0 = full volume, 31 = silence).
/// Each step ≈ 1.5 dB: multiplier = 1.0 / pow(2, 0.25 * level).
/// Values are fixed-point with 16 fractional bits.
pub(super) fn psg_db_table() -> &'static [i32; 32] {
    static TABLE: std::sync::OnceLock<[i32; 32]> = std::sync::OnceLock::new();
    TABLE.get_or_init(|| {
        let mut table = [0i32; 32];
        for vl in 0..32 {
            if vl == 31 {
                table[vl] = 0; // muted
            } else if vl == 0 {
                table[vl] = 65536; // 1.0 in fixed-point
            } else {
                let multiplier = 1.0 / f64::powf(2.0, 0.25 * vl as f64);
                table[vl] = (multiplier * 65536.0) as i32;
            }
        }
        table
    })
}

/// Maps 4-bit balance register values (0-15) to 5-bit volume range (0-31).
/// 0 = muted (maps to 0), 15 = full volume (maps to 31).
pub(super) fn psg_balance_scale_tab() -> &'static [u8; 16] {
    static TABLE: std::sync::OnceLock<[u8; 16]> = std::sync::OnceLock::new();
    TABLE.get_or_init(|| {
        [
            0x00, 0x03, 0x05, 0x07, 0x09, 0x0B, 0x0D, 0x0F, 0x10, 0x13, 0x15, 0x17, 0x19, 0x1B,
            0x1D, 0x1F,
        ]
    })
}

#[inline]
pub(super) fn phase_step_for_period(period: u16) -> u32 {
    phase_step_table()[(period & 0x0FFF) as usize]
}

#[inline]
pub(super) fn phase_step_table() -> &'static [u32; PSG_PERIOD_ENTRIES] {
    static TABLE: std::sync::OnceLock<[u32; PSG_PERIOD_ENTRIES]> = std::sync::OnceLock::new();
    TABLE.get_or_init(|| {
        let mut table = [1u32; PSG_PERIOD_ENTRIES];
        for (period, slot) in table.iter_mut().enumerate() {
            let divider = if period == 0 {
                0x1000_u64
            } else {
                period as u64
            };
            *slot = ((((PSG_CLOCK_HZ as u64) << PSG_PHASE_FRAC_BITS)
                / (divider * AUDIO_SAMPLE_RATE as u64))
                .max(1)) as u32;
        }
        table
    })
}
