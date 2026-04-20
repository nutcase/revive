use std::cell::Cell;

mod irq;
mod prg;
mod ram;

#[derive(Debug, Clone)]
pub(in crate::cartridge) struct Vrc3 {
    pub(in crate::cartridge) irq_reload: u16,
    pub(in crate::cartridge) irq_counter: u16,
    pub(in crate::cartridge) irq_enable_on_ack: bool,
    pub(in crate::cartridge) irq_enabled: bool,
    pub(in crate::cartridge) irq_mode_8bit: bool,
    pub(in crate::cartridge) irq_pending: Cell<bool>,
}

impl Vrc3 {
    pub(in crate::cartridge) fn new() -> Self {
        Self {
            irq_reload: 0,
            irq_counter: 0,
            irq_enable_on_ack: false,
            irq_enabled: false,
            irq_mode_8bit: false,
            irq_pending: Cell::new(false),
        }
    }
}
