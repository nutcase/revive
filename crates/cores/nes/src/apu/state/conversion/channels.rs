use crate::apu::channels::{DmcChannel, NoiseChannel, PulseChannel, TriangleChannel};

use super::super::{DmcState, NoiseChannelState, PulseChannelState, TriangleChannelState};

pub(super) fn snapshot_pulse_channel(channel: &PulseChannel) -> PulseChannelState {
    PulseChannelState {
        duty: channel.duty,
        length_counter: channel.length_counter,
        envelope_divider: channel.envelope_divider,
        envelope_decay: channel.envelope_decay,
        envelope_disable: channel.envelope_disable,
        envelope_start: channel.envelope_start,
        volume: channel.volume,
        sweep_enabled: channel.sweep_enabled,
        sweep_period: channel.sweep_period,
        sweep_negate: channel.sweep_negate,
        sweep_shift: channel.sweep_shift,
        sweep_reload: channel.sweep_reload,
        sweep_divider: channel.sweep_divider,
        timer: channel.timer,
        timer_reload: channel.timer_reload,
        duty_counter: channel.duty_counter,
        length_enabled: channel.length_enabled,
        is_pulse1: channel.is_pulse1,
    }
}

pub(super) fn restore_pulse_channel(channel: &mut PulseChannel, state: &PulseChannelState) {
    channel.duty = state.duty;
    channel.length_counter = state.length_counter;
    channel.envelope_divider = state.envelope_divider;
    channel.envelope_decay = state.envelope_decay;
    channel.envelope_disable = state.envelope_disable;
    channel.envelope_start = state.envelope_start;
    channel.volume = state.volume;
    channel.sweep_enabled = state.sweep_enabled;
    channel.sweep_period = state.sweep_period;
    channel.sweep_negate = state.sweep_negate;
    channel.sweep_shift = state.sweep_shift;
    channel.sweep_reload = state.sweep_reload;
    channel.sweep_divider = state.sweep_divider;
    channel.timer = state.timer;
    channel.timer_reload = state.timer_reload;
    channel.duty_counter = state.duty_counter;
    channel.length_enabled = state.length_enabled;
    channel.is_pulse1 = state.is_pulse1;
}

pub(super) fn snapshot_triangle_channel(channel: &TriangleChannel) -> TriangleChannelState {
    TriangleChannelState {
        linear_counter: channel.linear_counter,
        linear_reload: channel.linear_reload,
        linear_control: channel.linear_control,
        linear_reload_flag: channel.linear_reload_flag,
        length_counter: channel.length_counter,
        timer: channel.timer,
        timer_reload: channel.timer_reload,
        sequence_counter: channel.sequence_counter,
        length_enabled: channel.length_enabled,
    }
}

pub(super) fn restore_triangle_channel(
    channel: &mut TriangleChannel,
    state: &TriangleChannelState,
) {
    channel.linear_counter = state.linear_counter;
    channel.linear_reload = state.linear_reload;
    channel.linear_control = state.linear_control;
    channel.linear_reload_flag = state.linear_reload_flag;
    channel.length_counter = state.length_counter;
    channel.timer = state.timer;
    channel.timer_reload = state.timer_reload;
    channel.sequence_counter = state.sequence_counter;
    channel.length_enabled = state.length_enabled;
}

pub(super) fn snapshot_noise_channel(channel: &NoiseChannel) -> NoiseChannelState {
    NoiseChannelState {
        length_counter: channel.length_counter,
        envelope_divider: channel.envelope_divider,
        envelope_decay: channel.envelope_decay,
        envelope_disable: channel.envelope_disable,
        envelope_start: channel.envelope_start,
        volume: channel.volume,
        mode: channel.mode,
        timer: channel.timer,
        timer_reload: channel.timer_reload,
        shift_register: channel.shift_register,
        length_enabled: channel.length_enabled,
    }
}

pub(super) fn restore_noise_channel(channel: &mut NoiseChannel, state: &NoiseChannelState) {
    channel.length_counter = state.length_counter;
    channel.envelope_divider = state.envelope_divider;
    channel.envelope_decay = state.envelope_decay;
    channel.envelope_disable = state.envelope_disable;
    channel.envelope_start = state.envelope_start;
    channel.volume = state.volume;
    channel.mode = state.mode;
    channel.timer = state.timer;
    channel.timer_reload = state.timer_reload;
    channel.shift_register = state.shift_register;
    channel.length_enabled = state.length_enabled;
}

pub(super) fn snapshot_dmc_channel(channel: &DmcChannel) -> DmcState {
    DmcState {
        irq_enabled: channel.irq_enabled,
        irq_pending: channel.irq_pending,
        loop_flag: channel.loop_flag,
        timer: channel.timer,
        timer_reload: channel.timer_reload,
        output_level: channel.output_level,
        sample_address: channel.sample_address,
        sample_length: channel.sample_length,
        current_address: channel.current_address,
        bytes_remaining: channel.bytes_remaining,
        sample_buffer: channel.sample_buffer,
        shift_register: channel.shift_register,
        bits_remaining: channel.bits_remaining,
        silence: channel.silence,
        dma_delay: channel.dma_delay,
        pending_dma_stall_cycles: channel.pending_dma_stall_cycles,
    }
}

pub(super) fn restore_dmc_channel(channel: &mut DmcChannel, state: &DmcState) {
    channel.irq_enabled = state.irq_enabled;
    channel.irq_pending = state.irq_pending;
    channel.loop_flag = state.loop_flag;
    channel.timer = state.timer;
    channel.timer_reload = state.timer_reload;
    channel.output_level = state.output_level;
    channel.sample_address = state.sample_address;
    channel.sample_length = state.sample_length;
    channel.current_address = state.current_address;
    channel.bytes_remaining = state.bytes_remaining;
    channel.sample_buffer = state.sample_buffer;
    channel.shift_register = state.shift_register;
    channel.bits_remaining = state.bits_remaining;
    channel.silence = state.silence;
    channel.dma_delay = state.dma_delay;
    channel.pending_dma_stall_cycles = state.pending_dma_stall_cycles;
}
