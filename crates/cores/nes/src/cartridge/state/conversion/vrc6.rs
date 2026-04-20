use crate::cartridge::state::types::{Vrc6PulseState, Vrc6SawState, Vrc6State};
use crate::cartridge::Vrc6;

pub(in crate::cartridge::state) fn snapshot_vrc6_state(m: &Vrc6) -> Vrc6State {
    Vrc6State {
        prg_bank_16k: m.prg_bank_16k,
        prg_bank_8k: m.prg_bank_8k,
        chr_banks: m.chr_banks,
        banking_control: m.banking_control,
        irq_latch: m.irq_latch,
        irq_counter: m.irq_counter,
        irq_enable_after_ack: m.irq_enable_after_ack,
        irq_enabled: m.irq_enabled,
        irq_cycle_mode: m.irq_cycle_mode,
        irq_prescaler: m.irq_prescaler,
        irq_pending: m.irq_pending.get(),
        audio_halt: m.audio_halt,
        audio_freq_shift: m.audio_freq_shift,
        pulse1: Vrc6PulseState {
            volume: m.pulse1.volume,
            duty: m.pulse1.duty,
            ignore_duty: m.pulse1.ignore_duty,
            period: m.pulse1.period,
            enabled: m.pulse1.enabled,
            step: m.pulse1.step,
            divider: m.pulse1.divider,
        },
        pulse2: Vrc6PulseState {
            volume: m.pulse2.volume,
            duty: m.pulse2.duty,
            ignore_duty: m.pulse2.ignore_duty,
            period: m.pulse2.period,
            enabled: m.pulse2.enabled,
            step: m.pulse2.step,
            divider: m.pulse2.divider,
        },
        saw: Vrc6SawState {
            rate: m.saw.rate,
            period: m.saw.period,
            enabled: m.saw.enabled,
            step: m.saw.step,
            divider: m.saw.divider,
            accumulator: m.saw.accumulator,
        },
    }
}

pub(in crate::cartridge::state) fn restore_vrc6_state(vrc6: &mut Vrc6, saved: &Vrc6State) {
    vrc6.prg_bank_16k = saved.prg_bank_16k;
    vrc6.prg_bank_8k = saved.prg_bank_8k;
    vrc6.chr_banks = saved.chr_banks;
    vrc6.banking_control = saved.banking_control;
    vrc6.irq_latch = saved.irq_latch;
    vrc6.irq_counter = saved.irq_counter;
    vrc6.irq_enable_after_ack = saved.irq_enable_after_ack;
    vrc6.irq_enabled = saved.irq_enabled;
    vrc6.irq_cycle_mode = saved.irq_cycle_mode;
    vrc6.irq_prescaler = saved.irq_prescaler;
    vrc6.irq_pending.set(saved.irq_pending);
    vrc6.audio_halt = saved.audio_halt;
    vrc6.audio_freq_shift = saved.audio_freq_shift;

    vrc6.pulse1.volume = saved.pulse1.volume;
    vrc6.pulse1.duty = saved.pulse1.duty;
    vrc6.pulse1.ignore_duty = saved.pulse1.ignore_duty;
    vrc6.pulse1.period = saved.pulse1.period;
    vrc6.pulse1.enabled = saved.pulse1.enabled;
    vrc6.pulse1.step = saved.pulse1.step;
    vrc6.pulse1.divider = saved.pulse1.divider;

    vrc6.pulse2.volume = saved.pulse2.volume;
    vrc6.pulse2.duty = saved.pulse2.duty;
    vrc6.pulse2.ignore_duty = saved.pulse2.ignore_duty;
    vrc6.pulse2.period = saved.pulse2.period;
    vrc6.pulse2.enabled = saved.pulse2.enabled;
    vrc6.pulse2.step = saved.pulse2.step;
    vrc6.pulse2.divider = saved.pulse2.divider;

    vrc6.saw.rate = saved.saw.rate;
    vrc6.saw.period = saved.saw.period;
    vrc6.saw.enabled = saved.saw.enabled;
    vrc6.saw.step = saved.saw.step;
    vrc6.saw.divider = saved.saw.divider;
    vrc6.saw.accumulator = saved.saw.accumulator;
}
