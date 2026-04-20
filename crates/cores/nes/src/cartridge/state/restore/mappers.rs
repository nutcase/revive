mod chips;
mod expansion;
mod irq;
mod multicart;
mod simple;

use super::super::super::Cartridge;
use super::super::types::CartridgeState;

impl Cartridge {
    pub(super) fn restore_mapper_states(&mut self, state: &CartridgeState) {
        self.restore_chip_mapper_states(state);
        self.restore_simple_mapper_states(state);
        self.restore_multicart_mapper_states(state);
        self.restore_irq_mapper_states(state);
        self.restore_expansion_mapper_states(state);
    }
}
