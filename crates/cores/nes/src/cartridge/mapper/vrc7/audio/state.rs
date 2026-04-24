use crate::cartridge::state::{Vrc7AudioState, Vrc7ChannelState, Vrc7OperatorState};

use super::operator::Vrc7Operator;
use super::{Vrc7Audio, Vrc7Channel, REGISTER_COUNT};

impl Vrc7Audio {
    pub(in crate::cartridge) fn snapshot_state(&self) -> Vrc7AudioState {
        Vrc7AudioState {
            register_select: self.register_select,
            registers: self.registers.to_vec(),
            update_accumulator: self.update_accumulator,
            last_output: self.last_output,
            channels: self.channels.map(|channel| Vrc7ChannelState {
                modulator: operator_state(channel.modulator),
                carrier: operator_state(channel.carrier),
                key_on: channel.key_on,
            }),
        }
    }

    pub(in crate::cartridge) fn restore_state(&mut self, state: &Vrc7AudioState) {
        self.register_select = state.register_select;
        self.registers = [0; REGISTER_COUNT];
        let len = state.registers.len().min(REGISTER_COUNT);
        self.registers[..len].copy_from_slice(&state.registers[..len]);
        self.update_accumulator = state.update_accumulator;
        self.last_output = state.last_output;
        self.channels = state.channels.map(|channel| Vrc7Channel {
            modulator: operator_from_state(channel.modulator),
            carrier: operator_from_state(channel.carrier),
            key_on: channel.key_on,
        });
    }
}

fn operator_state(operator: Vrc7Operator) -> Vrc7OperatorState {
    Vrc7OperatorState {
        phase: operator.phase,
        envelope: operator.envelope,
        state: operator.state,
        last_output: operator.last_output,
    }
}

fn operator_from_state(state: Vrc7OperatorState) -> Vrc7Operator {
    Vrc7Operator {
        phase: state.phase,
        envelope: state.envelope,
        state: state.state,
        last_output: state.last_output,
    }
}
