mod channels;

use crate::apu::Apu;

use super::ApuState;
use channels::{
    restore_dmc_channel, restore_noise_channel, restore_pulse_channel, restore_triangle_channel,
    snapshot_dmc_channel, snapshot_noise_channel, snapshot_pulse_channel,
    snapshot_triangle_channel,
};

pub(super) fn snapshot_apu_state(apu: &Apu) -> ApuState {
    ApuState {
        pulse1: snapshot_pulse_channel(&apu.pulse1),
        pulse2: snapshot_pulse_channel(&apu.pulse2),
        triangle: snapshot_triangle_channel(&apu.triangle),
        noise: snapshot_noise_channel(&apu.noise),
        dmc: snapshot_dmc_channel(&apu.dmc),
        frame_counter: apu.frame_counter,
        cycle_count: apu.cycle_count,
        frame_mode: apu.frame_mode,
        irq_disable: apu.irq_disable,
        frame_irq: apu.frame_irq,
        pulse1_enabled: apu.pulse1_enabled,
        pulse2_enabled: apu.pulse2_enabled,
        triangle_enabled: apu.triangle_enabled,
        noise_enabled: apu.noise_enabled,
        dmc_enabled: apu.dmc_enabled,
        sample_counter: apu.sample_counter,
        sample_accumulator: apu.sample_accumulator,
        sample_accumulator_count: apu.sample_accumulator_count,
        aa_filter1: apu.aa_filter1.snapshot_state(),
        aa_filter2: apu.aa_filter2.snapshot_state(),
        high_pass_90hz: apu.high_pass_90hz.snapshot_state(),
        high_pass_440hz: apu.high_pass_440hz.snapshot_state(),
        low_pass_14khz: apu.low_pass_14khz.snapshot_state(),
    }
}

pub(super) fn restore_apu_state(apu: &mut Apu, state: &ApuState) {
    restore_pulse_channel(&mut apu.pulse1, &state.pulse1);
    restore_pulse_channel(&mut apu.pulse2, &state.pulse2);
    restore_triangle_channel(&mut apu.triangle, &state.triangle);
    restore_noise_channel(&mut apu.noise, &state.noise);
    restore_dmc_channel(&mut apu.dmc, &state.dmc);
    apu.frame_counter = state.frame_counter;
    apu.cycle_count = state.cycle_count;
    apu.frame_mode = state.frame_mode;
    apu.irq_disable = state.irq_disable;
    apu.frame_irq = state.frame_irq;
    apu.pulse1_enabled = state.pulse1_enabled;
    apu.pulse2_enabled = state.pulse2_enabled;
    apu.triangle_enabled = state.triangle_enabled;
    apu.noise_enabled = state.noise_enabled;
    apu.dmc_enabled = state.dmc_enabled;
    apu.sample_counter = state.sample_counter;
    apu.sample_accumulator = state.sample_accumulator;
    apu.sample_accumulator_count = state.sample_accumulator_count;
    apu.aa_filter1.restore_state(&state.aa_filter1);
    apu.aa_filter2.restore_state(&state.aa_filter2);
    apu.high_pass_90hz.restore_state(&state.high_pass_90hz);
    apu.high_pass_440hz.restore_state(&state.high_pass_440hz);
    apu.low_pass_14khz.restore_state(&state.low_pass_14khz);
    apu.output_buffer.clear();
    apu.expansion_audio = 0.0;
}
