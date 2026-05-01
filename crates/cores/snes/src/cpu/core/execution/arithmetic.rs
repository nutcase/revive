use super::super::{
    addressing::*,
    alu::{adc_generic, sbc_generic},
    memory::{add_cycles, read_u16_generic, read_u8_generic},
    CoreState,
};
use crate::{cpu::bus::CpuBus, cpu::StatusFlags};

pub(super) fn execute_arithmetic_opcode<T: CpuBus>(
    state: &mut CoreState,
    opcode: u8,
    bus: &mut T,
) -> Option<u8> {
    Some(match opcode {
        // Arithmetic operations
        0x69 => {
            // ADC immediate (supports decimal mode)
            let memory_8bit = state.emulation_mode || state.p.contains(StatusFlags::MEMORY_8BIT);
            let operand = if memory_8bit {
                read_u8_generic(state, bus) as u16
            } else {
                read_u16_generic(state, bus)
            };
            adc_generic(state, operand);
            let total_cycles: u8 = if memory_8bit { 2 } else { 3 };
            let already_accounted: u8 = if memory_8bit { 1 } else { 2 };
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x65 => {
            // ADC direct page
            let (addr, penalty) = read_direct_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = state.emulation_mode || state.p.contains(StatusFlags::MEMORY_8BIT);
            let operand = if memory_8bit {
                bus.read_u8(addr) as u16
            } else {
                bus.read_u16(addr)
            };
            adc_generic(state, operand);
            let base_cycles: u8 = if memory_8bit { 3 } else { 4 };
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x75 => {
            // ADC direct page,X
            let (addr, penalty) = read_direct_x_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = state.emulation_mode || state.p.contains(StatusFlags::MEMORY_8BIT);
            let operand = if memory_8bit {
                bus.read_u8(addr) as u16
            } else {
                bus.read_u16(addr)
            };
            adc_generic(state, operand);
            let base_cycles: u8 = if memory_8bit { 4 } else { 5 };
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x6D => {
            // ADC absolute
            let addr = read_absolute_address_generic(state, bus);
            let memory_8bit = state.emulation_mode || state.p.contains(StatusFlags::MEMORY_8BIT);
            let operand = if memory_8bit {
                bus.read_u8(addr) as u16
            } else {
                bus.read_u16(addr)
            };
            adc_generic(state, operand);
            let base_cycles: u8 = if memory_8bit { 4 } else { 5 };
            let already_accounted: u8 = 2;
            add_cycles(state, base_cycles.saturating_sub(already_accounted));
            base_cycles
        }

        0x7D => {
            // ADC absolute,X
            let (addr, penalty) = read_absolute_x_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = state.emulation_mode || state.p.contains(StatusFlags::MEMORY_8BIT);
            let operand = if memory_8bit {
                bus.read_u8(addr) as u16
            } else {
                bus.read_u16(addr)
            };
            adc_generic(state, operand);
            let base_cycles: u8 = if memory_8bit { 4 } else { 5 };
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 2 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x79 => {
            // ADC absolute,Y
            let (addr, penalty) = read_absolute_y_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = state.emulation_mode || state.p.contains(StatusFlags::MEMORY_8BIT);
            let operand = if memory_8bit {
                bus.read_u8(addr) as u16
            } else {
                bus.read_u16(addr)
            };
            adc_generic(state, operand);
            let base_cycles: u8 = if memory_8bit { 4 } else { 5 };
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 2 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x6F => {
            // ADC absolute long
            let addr = read_absolute_long_address_generic(state, bus);
            let memory_8bit = state.emulation_mode || state.p.contains(StatusFlags::MEMORY_8BIT);
            let operand = if memory_8bit {
                bus.read_u8(addr) as u16
            } else {
                bus.read_u16(addr)
            };
            adc_generic(state, operand);
            let base_cycles: u8 = if memory_8bit { 5 } else { 6 };
            let already_accounted: u8 = 3;
            add_cycles(state, base_cycles.saturating_sub(already_accounted));
            base_cycles
        }

        0x7F => {
            // ADC absolute long,X
            let addr = read_absolute_long_x_address_generic(state, bus);
            let memory_8bit = state.emulation_mode || state.p.contains(StatusFlags::MEMORY_8BIT);
            let operand = if memory_8bit {
                bus.read_u8(addr) as u16
            } else {
                bus.read_u16(addr)
            };
            adc_generic(state, operand);
            let base_cycles: u8 = if memory_8bit { 5 } else { 6 };
            let already_accounted: u8 = 3;
            add_cycles(state, base_cycles.saturating_sub(already_accounted));
            base_cycles
        }

        0x61 => {
            // ADC (dp,X)
            let (addr, penalty) = read_indirect_x_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = state.emulation_mode || state.p.contains(StatusFlags::MEMORY_8BIT);
            let operand = if memory_8bit {
                bus.read_u8(addr) as u16
            } else {
                bus.read_u16(addr)
            };
            adc_generic(state, operand);
            let base_cycles: u8 = if memory_8bit { 6 } else { 7 };
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x71 => {
            // ADC (dp),Y
            let (addr, penalty) = read_indirect_y_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = state.emulation_mode || state.p.contains(StatusFlags::MEMORY_8BIT);
            let operand = if memory_8bit {
                bus.read_u8(addr) as u16
            } else {
                bus.read_u16(addr)
            };
            adc_generic(state, operand);
            let base_cycles: u8 = if memory_8bit { 5 } else { 6 };
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x72 => {
            // ADC (dp)
            let (addr, penalty) = read_indirect_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = state.emulation_mode || state.p.contains(StatusFlags::MEMORY_8BIT);
            let operand = if memory_8bit {
                bus.read_u8(addr) as u16
            } else {
                bus.read_u16(addr)
            };
            adc_generic(state, operand);
            let base_cycles: u8 = if memory_8bit { 5 } else { 6 };
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x67 => {
            // ADC [dp]
            let (addr, penalty) = read_indirect_long_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = state.emulation_mode || state.p.contains(StatusFlags::MEMORY_8BIT);
            let operand = if memory_8bit {
                bus.read_u8(addr) as u16
            } else {
                bus.read_u16(addr)
            };
            adc_generic(state, operand);
            let base_cycles: u8 = if memory_8bit { 6 } else { 7 };
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x77 => {
            // ADC [dp],Y
            let (addr, penalty) = read_indirect_long_y_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = state.emulation_mode || state.p.contains(StatusFlags::MEMORY_8BIT);
            let operand = if memory_8bit {
                bus.read_u8(addr) as u16
            } else {
                bus.read_u16(addr)
            };
            adc_generic(state, operand);
            let base_cycles: u8 = if memory_8bit { 6 } else { 7 };
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x63 => {
            // ADC stack relative
            let addr = read_stack_relative_address_generic(state, bus);
            let memory_8bit = state.emulation_mode || state.p.contains(StatusFlags::MEMORY_8BIT);
            let operand = if memory_8bit {
                bus.read_u8(addr) as u16
            } else {
                bus.read_u16(addr)
            };
            adc_generic(state, operand);
            let base_cycles: u8 = if memory_8bit { 4 } else { 5 };
            let already_accounted: u8 = 1;
            add_cycles(state, base_cycles.saturating_sub(already_accounted));
            base_cycles
        }

        0x73 => {
            // ADC (sr,S),Y
            let (addr, penalty) = read_stack_relative_indirect_y_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = state.emulation_mode || state.p.contains(StatusFlags::MEMORY_8BIT);
            let operand = if memory_8bit {
                bus.read_u8(addr) as u16
            } else {
                bus.read_u16(addr)
            };
            adc_generic(state, operand);
            let base_cycles: u8 = if memory_8bit { 7 } else { 8 };
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0xE9 => {
            // SBC immediate (supports decimal mode)
            let memory_8bit = state.emulation_mode || state.p.contains(StatusFlags::MEMORY_8BIT);
            let operand = if memory_8bit {
                read_u8_generic(state, bus) as u16
            } else {
                read_u16_generic(state, bus)
            };
            sbc_generic(state, operand);
            let total_cycles: u8 = if memory_8bit { 2 } else { 3 };
            let already_accounted: u8 = if memory_8bit { 1 } else { 2 };
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0xE5 => {
            // SBC direct page
            let (addr, penalty) = read_direct_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = state.emulation_mode || state.p.contains(StatusFlags::MEMORY_8BIT);
            let operand = if memory_8bit {
                bus.read_u8(addr) as u16
            } else {
                bus.read_u16(addr)
            };
            sbc_generic(state, operand);
            let base_cycles: u8 = if memory_8bit { 3 } else { 4 };
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0xF5 => {
            // SBC direct page,X
            let (addr, penalty) = read_direct_x_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = state.emulation_mode || state.p.contains(StatusFlags::MEMORY_8BIT);
            let operand = if memory_8bit {
                bus.read_u8(addr) as u16
            } else {
                bus.read_u16(addr)
            };
            sbc_generic(state, operand);
            let base_cycles: u8 = if memory_8bit { 4 } else { 5 };
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0xED => {
            // SBC absolute
            let addr = read_absolute_address_generic(state, bus);
            let memory_8bit = state.emulation_mode || state.p.contains(StatusFlags::MEMORY_8BIT);
            let operand = if memory_8bit {
                bus.read_u8(addr) as u16
            } else {
                bus.read_u16(addr)
            };
            sbc_generic(state, operand);
            let base_cycles: u8 = if memory_8bit { 4 } else { 5 };
            let already_accounted: u8 = 2;
            add_cycles(state, base_cycles.saturating_sub(already_accounted));
            base_cycles
        }

        0xFD => {
            // SBC absolute,X
            let (addr, penalty) = read_absolute_x_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = state.emulation_mode || state.p.contains(StatusFlags::MEMORY_8BIT);
            let operand = if memory_8bit {
                bus.read_u8(addr) as u16
            } else {
                bus.read_u16(addr)
            };
            sbc_generic(state, operand);
            let base_cycles: u8 = if memory_8bit { 4 } else { 5 };
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 2 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0xF9 => {
            // SBC absolute,Y
            let (addr, penalty) = read_absolute_y_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = state.emulation_mode || state.p.contains(StatusFlags::MEMORY_8BIT);
            let operand = if memory_8bit {
                bus.read_u8(addr) as u16
            } else {
                bus.read_u16(addr)
            };
            sbc_generic(state, operand);
            let base_cycles: u8 = if memory_8bit { 4 } else { 5 };
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 2 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0xEF => {
            // SBC absolute long
            let addr = read_absolute_long_address_generic(state, bus);
            let memory_8bit = state.emulation_mode || state.p.contains(StatusFlags::MEMORY_8BIT);
            let operand = if memory_8bit {
                bus.read_u8(addr) as u16
            } else {
                bus.read_u16(addr)
            };
            sbc_generic(state, operand);
            let base_cycles: u8 = if memory_8bit { 5 } else { 6 };
            let already_accounted: u8 = 3;
            add_cycles(state, base_cycles.saturating_sub(already_accounted));
            base_cycles
        }

        0xFF => {
            // SBC absolute long,X
            let addr = read_absolute_long_x_address_generic(state, bus);
            let memory_8bit = state.emulation_mode || state.p.contains(StatusFlags::MEMORY_8BIT);
            let operand = if memory_8bit {
                bus.read_u8(addr) as u16
            } else {
                bus.read_u16(addr)
            };
            sbc_generic(state, operand);
            let base_cycles: u8 = if memory_8bit { 5 } else { 6 };
            let already_accounted: u8 = 3;
            add_cycles(state, base_cycles.saturating_sub(already_accounted));
            base_cycles
        }

        0xE1 => {
            // SBC (dp,X)
            let (addr, penalty) = read_indirect_x_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = state.emulation_mode || state.p.contains(StatusFlags::MEMORY_8BIT);
            let operand = if memory_8bit {
                bus.read_u8(addr) as u16
            } else {
                bus.read_u16(addr)
            };
            sbc_generic(state, operand);
            let base_cycles: u8 = if memory_8bit { 6 } else { 7 };
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0xF1 => {
            // SBC (dp),Y
            let (addr, penalty) = read_indirect_y_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = state.emulation_mode || state.p.contains(StatusFlags::MEMORY_8BIT);
            let operand = if memory_8bit {
                bus.read_u8(addr) as u16
            } else {
                bus.read_u16(addr)
            };
            sbc_generic(state, operand);
            let base_cycles: u8 = if memory_8bit { 5 } else { 6 };
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0xE7 => {
            // SBC [dp]
            let (addr, penalty) = read_indirect_long_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = state.emulation_mode || state.p.contains(StatusFlags::MEMORY_8BIT);
            let operand = if memory_8bit {
                bus.read_u8(addr) as u16
            } else {
                bus.read_u16(addr)
            };
            sbc_generic(state, operand);
            let base_cycles: u8 = if memory_8bit { 6 } else { 7 };
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0xF7 => {
            // SBC [dp],Y
            let (addr, penalty) = read_indirect_long_y_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = state.emulation_mode || state.p.contains(StatusFlags::MEMORY_8BIT);
            let operand = if memory_8bit {
                bus.read_u8(addr) as u16
            } else {
                bus.read_u16(addr)
            };
            sbc_generic(state, operand);
            let base_cycles: u8 = if memory_8bit { 6 } else { 7 };
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0xE3 => {
            // SBC stack relative
            let addr = read_stack_relative_address_generic(state, bus);
            let memory_8bit = state.emulation_mode || state.p.contains(StatusFlags::MEMORY_8BIT);
            let operand = if memory_8bit {
                bus.read_u8(addr) as u16
            } else {
                bus.read_u16(addr)
            };
            sbc_generic(state, operand);
            let base_cycles: u8 = if memory_8bit { 4 } else { 5 };
            let already_accounted: u8 = 1;
            add_cycles(state, base_cycles.saturating_sub(already_accounted));
            base_cycles
        }

        0xF3 => {
            // SBC (sr,S),Y
            let (addr, penalty) = read_stack_relative_indirect_y_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = state.emulation_mode || state.p.contains(StatusFlags::MEMORY_8BIT);
            let operand = if memory_8bit {
                bus.read_u8(addr) as u16
            } else {
                bus.read_u16(addr)
            };
            sbc_generic(state, operand);
            let base_cycles: u8 = if memory_8bit { 7 } else { 8 };
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0xF2 => {
            // SBC (dp)
            let (addr, penalty) = read_indirect_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = state.emulation_mode || state.p.contains(StatusFlags::MEMORY_8BIT);
            let operand = if memory_8bit {
                bus.read_u8(addr) as u16
            } else {
                bus.read_u16(addr)
            };
            sbc_generic(state, operand);
            let base_cycles: u8 = if memory_8bit { 5 } else { 6 };
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        _ => return None,
    })
}
