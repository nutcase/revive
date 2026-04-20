mod conversion;

use super::Apu;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApuState {
    pub pulse1: PulseChannelState,
    pub pulse2: PulseChannelState,
    pub triangle: TriangleChannelState,
    pub noise: NoiseChannelState,
    pub dmc: DmcState,
    pub frame_counter: u16,
    pub cycle_count: u64,
    pub frame_mode: bool,
    pub irq_disable: bool,
    pub frame_irq: bool,
    pub pulse1_enabled: bool,
    pub pulse2_enabled: bool,
    pub triangle_enabled: bool,
    pub noise_enabled: bool,
    pub dmc_enabled: bool,
    pub sample_counter: f32,
    pub sample_accumulator: f32,
    pub sample_accumulator_count: u32,
    pub aa_filter1: LowPassFilterState,
    pub aa_filter2: LowPassFilterState,
    pub high_pass_90hz: HighPassFilterState,
    pub high_pass_440hz: HighPassFilterState,
    pub low_pass_14khz: LowPassFilterState,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PulseChannelState {
    pub duty: u8,
    pub length_counter: u8,
    pub envelope_divider: u8,
    pub envelope_decay: u8,
    pub envelope_disable: bool,
    pub envelope_start: bool,
    pub volume: u8,
    pub sweep_enabled: bool,
    pub sweep_period: u8,
    pub sweep_negate: bool,
    pub sweep_shift: u8,
    pub sweep_reload: bool,
    pub sweep_divider: u8,
    pub timer: u16,
    pub timer_reload: u16,
    pub duty_counter: u8,
    pub length_enabled: bool,
    pub is_pulse1: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriangleChannelState {
    pub linear_counter: u8,
    pub linear_reload: u8,
    pub linear_control: bool,
    pub linear_reload_flag: bool,
    pub length_counter: u8,
    pub timer: u16,
    pub timer_reload: u16,
    pub sequence_counter: u8,
    pub length_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoiseChannelState {
    pub length_counter: u8,
    pub envelope_divider: u8,
    pub envelope_decay: u8,
    pub envelope_disable: bool,
    pub envelope_start: bool,
    pub volume: u8,
    pub mode: bool,
    pub timer: u16,
    pub timer_reload: u16,
    pub shift_register: u16,
    pub length_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DmcState {
    pub irq_enabled: bool,
    pub irq_pending: bool,
    pub loop_flag: bool,
    pub timer: u16,
    pub timer_reload: u16,
    pub output_level: u8,
    pub sample_address: u16,
    pub sample_length: u16,
    pub current_address: u16,
    pub bytes_remaining: u16,
    pub sample_buffer: Option<u8>,
    pub shift_register: u8,
    pub bits_remaining: u8,
    pub silence: bool,
    #[serde(default)]
    pub dma_delay: u8,
    #[serde(default)]
    pub pending_dma_stall_cycles: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HighPassFilterState {
    pub prev_input: f32,
    pub prev_output: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LowPassFilterState {
    pub prev_output: f32,
}

impl Apu {
    pub fn snapshot_state(&self) -> ApuState {
        conversion::snapshot_apu_state(self)
    }

    pub fn restore_state(&mut self, state: &ApuState) {
        conversion::restore_apu_state(self, state);
    }

    pub fn restore_legacy_state(&mut self, frame_counter: u8, frame_irq: bool) {
        let ring = self.audio_ring.clone();
        *self = Apu::new();
        self.audio_ring = ring;
        self.frame_counter = frame_counter as u16;
        self.frame_irq = frame_irq;
    }
}
