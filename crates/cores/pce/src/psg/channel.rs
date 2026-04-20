#[derive(Clone, Copy, bincode::Encode, bincode::Decode)]
pub(crate) struct PsgChannel {
    pub(crate) frequency: u16,
    pub(crate) phase_step: u32,
    pub(crate) control: u8,
    pub(crate) balance: u8,
    pub(crate) noise_control: u8,
    pub(crate) phase: u32,
    pub(crate) wave_pos: u8,
    pub(crate) wave_write_pos: u8,
    pub(crate) dda_sample: u8,
    pub(crate) noise_lfsr: u32, // 18-bit LFSR (HuC6280 reference)
    pub(crate) noise_phase: u32,
}

impl Default for PsgChannel {
    fn default() -> Self {
        Self {
            frequency: 0,
            phase_step: 1,
            control: 0,
            balance: 0xFF,
            noise_control: 0,
            phase: 0,
            wave_pos: 0,
            wave_write_pos: 0,
            dda_sample: 0x10,
            noise_lfsr: 1, // 18-bit LFSR initial value (Mednafen reference)
            noise_phase: 0,
        }
    }
}
