use std::cell::Cell;

mod audio;
mod banking;
mod irq;

pub(in crate::cartridge) use audio::{Vrc6Pulse, Vrc6Saw};

#[derive(Debug, Clone)]
pub(in crate::cartridge) struct Vrc6 {
    pub(in crate::cartridge) prg_bank_16k: u8,
    pub(in crate::cartridge) prg_bank_8k: u8,
    pub(in crate::cartridge) chr_banks: [u8; 8],
    pub(in crate::cartridge) banking_control: u8,
    pub(in crate::cartridge) irq_latch: u8,
    pub(in crate::cartridge) irq_counter: u8,
    pub(in crate::cartridge) irq_enable_after_ack: bool,
    pub(in crate::cartridge) irq_enabled: bool,
    pub(in crate::cartridge) irq_cycle_mode: bool,
    pub(in crate::cartridge) irq_prescaler: i16,
    pub(in crate::cartridge) irq_pending: Cell<bool>,
    pub(in crate::cartridge) audio_halt: bool,
    pub(in crate::cartridge) audio_freq_shift: u8,
    pub(in crate::cartridge) pulse1: Vrc6Pulse,
    pub(in crate::cartridge) pulse2: Vrc6Pulse,
    pub(in crate::cartridge) saw: Vrc6Saw,
}

impl Vrc6 {
    pub(in crate::cartridge) fn new() -> Self {
        Self {
            prg_bank_16k: 0,
            prg_bank_8k: 0,
            chr_banks: [0; 8],
            banking_control: 0,
            irq_latch: 0,
            irq_counter: 0,
            irq_enable_after_ack: false,
            irq_enabled: false,
            irq_cycle_mode: false,
            irq_prescaler: 341,
            irq_pending: Cell::new(false),
            audio_halt: false,
            audio_freq_shift: 0,
            pulse1: Vrc6Pulse::new(),
            pulse2: Vrc6Pulse::new(),
            saw: Vrc6Saw::new(),
        }
    }
}
