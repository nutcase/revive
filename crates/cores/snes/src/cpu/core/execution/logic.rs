use super::super::{
    addressing::*,
    alu::*,
    memory::{add_cycles, read_u16_generic, read_u8_generic},
    CoreState,
};
use crate::{cpu::bus::CpuBus, cpu::StatusFlags};

pub(super) fn execute_logic_opcode<T: CpuBus>(
    state: &mut CoreState,
    opcode: u8,
    bus: &mut T,
) -> Option<u8> {
    Some(match opcode {
        // ORA logical OR operations
        0x04 => {
            // TSB direct page
            let (addr, penalty) = read_direct_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = memory_is_8bit(state);
            if memory_8bit {
                let value = bus.read_u8(addr);
                let a_low = (state.a & 0xFF) as u8;
                state.p.set(StatusFlags::ZERO, (value & a_low) == 0);
                bus.write_u8(addr, value | a_low);
            } else {
                let value = bus.read_u16(addr);
                state.p.set(StatusFlags::ZERO, (value & state.a) == 0);
                bus.write_u16(addr, value | state.a);
            }
            let base_cycles: u8 = if memory_8bit { 5 } else { 6 };
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x05 => {
            // ORA direct page
            let (addr, penalty) = read_direct_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = memory_is_8bit(state);
            let operand = read_operand_m(state, bus, addr, memory_8bit);
            ora_operand(state, operand);
            let base_cycles: u8 = 3;
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x15 => {
            // ORA direct page,X
            let (addr, penalty) = read_direct_x_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = memory_is_8bit(state);
            let operand = read_operand_m(state, bus, addr, memory_8bit);
            ora_operand(state, operand);
            let base_cycles: u8 = 4;
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x0D => {
            // ORA absolute
            let addr = read_absolute_address_generic(state, bus);
            let memory_8bit = memory_is_8bit(state);
            let operand = read_operand_m(state, bus, addr, memory_8bit);
            ora_operand(state, operand);
            let base_cycles: u8 = 4;
            let already_accounted: u8 = 2;
            add_cycles(state, base_cycles.saturating_sub(already_accounted));
            base_cycles
        }

        0x1D => {
            // ORA absolute,X
            let (addr, penalty) = read_absolute_x_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = memory_is_8bit(state);
            let operand = read_operand_m(state, bus, addr, memory_8bit);
            ora_operand(state, operand);
            let base_cycles: u8 = 4;
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 2 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x19 => {
            // ORA absolute,Y
            let (addr, penalty) = read_absolute_y_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = memory_is_8bit(state);
            let operand = read_operand_m(state, bus, addr, memory_8bit);
            ora_operand(state, operand);
            let base_cycles: u8 = if memory_8bit { 4 } else { 5 };
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 2 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x0F => {
            // ORA absolute long
            let addr = read_absolute_long_address_generic(state, bus);
            let memory_8bit = memory_is_8bit(state);
            let operand = read_operand_m(state, bus, addr, memory_8bit);
            ora_operand(state, operand);
            let base_cycles: u8 = 5;
            let already_accounted: u8 = 3;
            add_cycles(state, base_cycles.saturating_sub(already_accounted));
            base_cycles
        }

        0x1F => {
            // ORA absolute long,X
            let addr = read_absolute_long_x_address_generic(state, bus);
            let memory_8bit = memory_is_8bit(state);
            let operand = read_operand_m(state, bus, addr, memory_8bit);
            ora_operand(state, operand);
            let base_cycles: u8 = 5;
            let already_accounted: u8 = 3;
            add_cycles(state, base_cycles.saturating_sub(already_accounted));
            base_cycles
        }

        0x01 => {
            // ORA (dp,X)
            let (addr, penalty) = read_indirect_x_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = memory_is_8bit(state);
            let operand = read_operand_m(state, bus, addr, memory_8bit);
            ora_operand(state, operand);
            let base_cycles: u8 = 6;
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x11 => {
            // ORA (dp),Y
            let (addr, penalty) = read_indirect_y_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = memory_is_8bit(state);
            let operand = read_operand_m(state, bus, addr, memory_8bit);
            ora_operand(state, operand);
            let base_cycles: u8 = 5;
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x12 => {
            // ORA (dp)
            let (addr, penalty) = read_indirect_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = memory_is_8bit(state);
            let operand = read_operand_m(state, bus, addr, memory_8bit);
            ora_operand(state, operand);
            let base_cycles: u8 = 5;
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x13 => {
            // ORA (sr,S),Y
            let (addr, penalty) = read_stack_relative_indirect_y_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = memory_is_8bit(state);
            let operand = read_operand_m(state, bus, addr, memory_8bit);
            ora_operand(state, operand);
            let base_cycles: u8 = 7;
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x03 => {
            // ORA stack relative
            let addr = read_stack_relative_address_generic(state, bus);
            let memory_8bit = memory_is_8bit(state);
            let operand = read_operand_m(state, bus, addr, memory_8bit);
            ora_operand(state, operand);
            let base_cycles: u8 = 4;
            let already_accounted: u8 = 1;
            add_cycles(state, base_cycles.saturating_sub(already_accounted));
            base_cycles
        }

        0x07 => {
            // ORA [dp]
            let (addr, penalty) = read_indirect_long_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = memory_is_8bit(state);
            let operand = read_operand_m(state, bus, addr, memory_8bit);
            ora_operand(state, operand);
            let base_cycles: u8 = 6;
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x17 => {
            // ORA [dp],Y
            let (addr, penalty) = read_indirect_long_y_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = memory_is_8bit(state);
            let operand = read_operand_m(state, bus, addr, memory_8bit);
            ora_operand(state, operand);
            let base_cycles: u8 = 6;
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        // Load/Store instructions - extended coverage
        0x25 => {
            // AND direct page
            let (addr, penalty) = read_direct_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = memory_is_8bit(state);
            let operand = read_operand_m(state, bus, addr, memory_8bit);
            and_operand(state, operand);
            let base_cycles: u8 = if memory_8bit { 3 } else { 4 };
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x35 => {
            // AND direct page,X
            let (addr, penalty) = read_direct_x_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = memory_is_8bit(state);
            let operand = read_operand_m(state, bus, addr, memory_8bit);
            and_operand(state, operand);
            let base_cycles: u8 = if memory_8bit { 4 } else { 5 };
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x2D => {
            // AND absolute
            let addr = read_absolute_address_generic(state, bus);
            let memory_8bit = memory_is_8bit(state);
            let operand = read_operand_m(state, bus, addr, memory_8bit);
            and_operand(state, operand);
            let base_cycles: u8 = if memory_8bit { 4 } else { 5 };
            let already_accounted: u8 = 2;
            add_cycles(state, base_cycles.saturating_sub(already_accounted));
            base_cycles
        }

        0x3D => {
            // AND absolute,X
            let (addr, penalty) = read_absolute_x_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = memory_is_8bit(state);
            let operand = read_operand_m(state, bus, addr, memory_8bit);
            and_operand(state, operand);
            let base_cycles: u8 = if memory_8bit { 4 } else { 5 };
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 2 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x39 => {
            // AND absolute,Y
            let (addr, penalty) = read_absolute_y_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = memory_is_8bit(state);
            let operand = read_operand_m(state, bus, addr, memory_8bit);
            and_operand(state, operand);
            let base_cycles: u8 = if memory_8bit { 4 } else { 5 };
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 2 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x2F => {
            // AND absolute long
            let addr = read_absolute_long_address_generic(state, bus);
            let memory_8bit = memory_is_8bit(state);
            let operand = read_operand_m(state, bus, addr, memory_8bit);
            and_operand(state, operand);
            let base_cycles: u8 = if memory_8bit { 5 } else { 6 };
            let already_accounted: u8 = 3;
            add_cycles(state, base_cycles.saturating_sub(already_accounted));
            base_cycles
        }

        0x3F => {
            // AND absolute long,X
            let addr = read_absolute_long_x_address_generic(state, bus);
            let memory_8bit = memory_is_8bit(state);
            let operand = read_operand_m(state, bus, addr, memory_8bit);
            and_operand(state, operand);
            let base_cycles: u8 = if memory_8bit { 5 } else { 6 };
            let already_accounted: u8 = 3;
            add_cycles(state, base_cycles.saturating_sub(already_accounted));
            base_cycles
        }

        0x21 => {
            // AND (dp,X)
            let (addr, penalty) = read_indirect_x_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = memory_is_8bit(state);
            let operand = read_operand_m(state, bus, addr, memory_8bit);
            and_operand(state, operand);
            let base_cycles: u8 = if memory_8bit { 6 } else { 7 };
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x31 => {
            // AND (dp),Y
            let (addr, penalty) = read_indirect_y_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = memory_is_8bit(state);
            let operand = read_operand_m(state, bus, addr, memory_8bit);
            and_operand(state, operand);
            let base_cycles: u8 = if memory_8bit { 5 } else { 6 };
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x32 => {
            // AND (dp)
            let (addr, penalty) = read_indirect_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = memory_is_8bit(state);
            let operand = read_operand_m(state, bus, addr, memory_8bit);
            and_operand(state, operand);
            let base_cycles: u8 = if memory_8bit { 5 } else { 6 };
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x33 => {
            // AND (sr,S),Y
            let (addr, penalty) = read_stack_relative_indirect_y_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = memory_is_8bit(state);
            let operand = read_operand_m(state, bus, addr, memory_8bit);
            and_operand(state, operand);
            let base_cycles: u8 = if memory_8bit { 7 } else { 8 };
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x23 => {
            // AND stack relative
            let addr = read_stack_relative_address_generic(state, bus);
            let memory_8bit = memory_is_8bit(state);
            let operand = read_operand_m(state, bus, addr, memory_8bit);
            and_operand(state, operand);
            let base_cycles: u8 = if memory_8bit { 4 } else { 5 };
            let already_accounted: u8 = 1;
            add_cycles(state, base_cycles.saturating_sub(already_accounted));
            base_cycles
        }

        0x27 => {
            // AND [dp]
            let (addr, penalty) = read_indirect_long_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = memory_is_8bit(state);
            let operand = read_operand_m(state, bus, addr, memory_8bit);
            and_operand(state, operand);
            let base_cycles: u8 = if memory_8bit { 6 } else { 7 };
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x37 => {
            // AND [dp],Y
            let (addr, penalty) = read_indirect_long_y_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = memory_is_8bit(state);
            let operand = read_operand_m(state, bus, addr, memory_8bit);
            and_operand(state, operand);
            let base_cycles: u8 = if memory_8bit { 6 } else { 7 };
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        // EOR logical exclusive OR operations
        0x45 => {
            // EOR direct page
            let (addr, penalty) = read_direct_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = memory_is_8bit(state);
            let operand = read_operand_m(state, bus, addr, memory_8bit);
            eor_operand(state, operand);
            let base_cycles: u8 = if memory_8bit { 3 } else { 4 };
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x55 => {
            // EOR direct page,X
            let (addr, penalty) = read_direct_x_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = memory_is_8bit(state);
            let operand = read_operand_m(state, bus, addr, memory_8bit);
            eor_operand(state, operand);
            let base_cycles: u8 = if memory_8bit { 4 } else { 5 };
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x4D => {
            // EOR absolute
            let addr = read_absolute_address_generic(state, bus);
            let memory_8bit = memory_is_8bit(state);
            let operand = read_operand_m(state, bus, addr, memory_8bit);
            eor_operand(state, operand);
            let base_cycles: u8 = if memory_8bit { 4 } else { 5 };
            let already_accounted: u8 = 2;
            add_cycles(state, base_cycles.saturating_sub(already_accounted));
            base_cycles
        }

        0x5D => {
            // EOR absolute,X
            let (addr, penalty) = read_absolute_x_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = memory_is_8bit(state);
            let operand = read_operand_m(state, bus, addr, memory_8bit);
            eor_operand(state, operand);
            let base_cycles: u8 = if memory_8bit { 4 } else { 5 };
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 2 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x59 => {
            // EOR absolute,Y
            let (addr, penalty) = read_absolute_y_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = memory_is_8bit(state);
            let operand = read_operand_m(state, bus, addr, memory_8bit);
            eor_operand(state, operand);
            let base_cycles: u8 = if memory_8bit { 4 } else { 5 };
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 2 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x4F => {
            // EOR absolute long
            let addr = read_absolute_long_address_generic(state, bus);
            let memory_8bit = memory_is_8bit(state);
            let operand = read_operand_m(state, bus, addr, memory_8bit);
            eor_operand(state, operand);
            let base_cycles: u8 = if memory_8bit { 5 } else { 6 };
            let already_accounted: u8 = 3;
            add_cycles(state, base_cycles.saturating_sub(already_accounted));
            base_cycles
        }

        0x5F => {
            // EOR absolute long,X
            let addr = read_absolute_long_x_address_generic(state, bus);
            let memory_8bit = memory_is_8bit(state);
            let operand = read_operand_m(state, bus, addr, memory_8bit);
            eor_operand(state, operand);
            let base_cycles: u8 = if memory_8bit { 5 } else { 6 };
            let already_accounted: u8 = 3;
            add_cycles(state, base_cycles.saturating_sub(already_accounted));
            base_cycles
        }

        0x41 => {
            // EOR (dp,X)
            let (addr, penalty) = read_indirect_x_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = memory_is_8bit(state);
            let operand = read_operand_m(state, bus, addr, memory_8bit);
            eor_operand(state, operand);
            let base_cycles: u8 = if memory_8bit { 6 } else { 7 };
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x51 => {
            // EOR (dp),Y
            let (addr, penalty) = read_indirect_y_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = memory_is_8bit(state);
            let operand = read_operand_m(state, bus, addr, memory_8bit);
            eor_operand(state, operand);
            let base_cycles: u8 = if memory_8bit { 5 } else { 6 };
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x52 => {
            // EOR (dp)
            let (addr, penalty) = read_indirect_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = memory_is_8bit(state);
            let operand = read_operand_m(state, bus, addr, memory_8bit);
            eor_operand(state, operand);
            let base_cycles: u8 = if memory_8bit { 5 } else { 6 };
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x53 => {
            // EOR (sr,S),Y
            let (addr, penalty) = read_stack_relative_indirect_y_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = memory_is_8bit(state);
            let operand = read_operand_m(state, bus, addr, memory_8bit);
            eor_operand(state, operand);
            let base_cycles: u8 = if memory_8bit { 7 } else { 8 };
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x43 => {
            // EOR stack relative
            let addr = read_stack_relative_address_generic(state, bus);
            let memory_8bit = memory_is_8bit(state);
            let operand = read_operand_m(state, bus, addr, memory_8bit);
            eor_operand(state, operand);
            let base_cycles: u8 = if memory_8bit { 4 } else { 5 };
            let already_accounted: u8 = 1;
            add_cycles(state, base_cycles.saturating_sub(already_accounted));
            base_cycles
        }

        0x47 => {
            // EOR [dp]
            let (addr, penalty) = read_indirect_long_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = memory_is_8bit(state);
            let operand = read_operand_m(state, bus, addr, memory_8bit);
            eor_operand(state, operand);
            let base_cycles: u8 = if memory_8bit { 6 } else { 7 };
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x57 => {
            // EOR [dp],Y
            let (addr, penalty) = read_indirect_long_y_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = memory_is_8bit(state);
            let operand = read_operand_m(state, bus, addr, memory_8bit);
            eor_operand(state, operand);
            let base_cycles: u8 = if memory_8bit { 6 } else { 7 };
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x0A => {
            // ASL accumulator
            if memory_is_8bit(state) {
                let result = asl8(state, (state.a & 0xFF) as u8);
                state.a = (state.a & 0xFF00) | (result as u16);
            } else {
                state.a = asl16(state, state.a);
            }
            add_cycles(state, 2);
            2
        }

        0x2A => {
            // ROL accumulator
            if memory_is_8bit(state) {
                let result = rol8(state, (state.a & 0xFF) as u8);
                state.a = (state.a & 0xFF00) | (result as u16);
            } else {
                state.a = rol16(state, state.a);
            }
            add_cycles(state, 2);
            2
        }

        0x4A => {
            // LSR accumulator
            if memory_is_8bit(state) {
                let result = lsr8(state, (state.a & 0xFF) as u8);
                state.a = (state.a & 0xFF00) | (result as u16);
            } else {
                state.a = lsr16(state, state.a);
            }
            add_cycles(state, 2);
            2
        }

        0x6A => {
            // ROR accumulator
            if memory_is_8bit(state) {
                let result = ror8(state, (state.a & 0xFF) as u8);
                state.a = (state.a & 0xFF00) | (result as u16);
            } else {
                state.a = ror16(state, state.a);
            }
            add_cycles(state, 2);
            2
        }

        0x06 => {
            // ASL direct page
            let (addr, penalty) = read_direct_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = memory_is_8bit(state);
            modify_memory(state, bus, addr, memory_8bit, asl8, asl16);
            let base_cycles: u8 = 5;
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x16 => {
            // ASL direct page,X
            let (addr, penalty) = read_direct_x_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = memory_is_8bit(state);
            modify_memory(state, bus, addr, memory_8bit, asl8, asl16);
            let base_cycles: u8 = 6;
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x0E => {
            // ASL absolute
            let addr = read_absolute_address_generic(state, bus);
            let memory_8bit = memory_is_8bit(state);
            modify_memory(state, bus, addr, memory_8bit, asl8, asl16);
            let base_cycles: u8 = 6;
            let already_accounted: u8 = 3;
            add_cycles(state, base_cycles.saturating_sub(already_accounted));
            base_cycles
        }

        0x1E => {
            // ASL absolute,X
            let (addr, penalty) = read_absolute_x_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = memory_is_8bit(state);
            modify_memory(state, bus, addr, memory_8bit, asl8, asl16);
            let base_cycles: u8 = 7;
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 3 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x26 => {
            // ROL direct page
            let (addr, penalty) = read_direct_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = memory_is_8bit(state);
            modify_memory(state, bus, addr, memory_8bit, rol8, rol16);
            let base_cycles: u8 = 5;
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x36 => {
            // ROL direct page,X
            let (addr, penalty) = read_direct_x_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = memory_is_8bit(state);
            modify_memory(state, bus, addr, memory_8bit, rol8, rol16);
            let base_cycles: u8 = 6;
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x2E => {
            // ROL absolute
            let addr = read_absolute_address_generic(state, bus);
            let memory_8bit = memory_is_8bit(state);
            modify_memory(state, bus, addr, memory_8bit, rol8, rol16);
            let base_cycles: u8 = 6;
            let already_accounted: u8 = 3;
            add_cycles(state, base_cycles.saturating_sub(already_accounted));
            base_cycles
        }

        0x3E => {
            // ROL absolute,X
            let (addr, penalty) = read_absolute_x_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = memory_is_8bit(state);
            modify_memory(state, bus, addr, memory_8bit, rol8, rol16);
            let base_cycles: u8 = 7;
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 3 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x46 => {
            // LSR direct page
            let (addr, penalty) = read_direct_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = memory_is_8bit(state);
            modify_memory(state, bus, addr, memory_8bit, lsr8, lsr16);
            let base_cycles: u8 = 5;
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x56 => {
            // LSR direct page,X
            let (addr, penalty) = read_direct_x_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = memory_is_8bit(state);
            modify_memory(state, bus, addr, memory_8bit, lsr8, lsr16);
            let base_cycles: u8 = 6;
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x4E => {
            // LSR absolute
            let addr = read_absolute_address_generic(state, bus);
            let memory_8bit = memory_is_8bit(state);
            modify_memory(state, bus, addr, memory_8bit, lsr8, lsr16);
            let base_cycles: u8 = 6;
            let already_accounted: u8 = 3;
            add_cycles(state, base_cycles.saturating_sub(already_accounted));
            base_cycles
        }

        0x5E => {
            // LSR absolute,X
            let (addr, penalty) = read_absolute_x_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = memory_is_8bit(state);
            modify_memory(state, bus, addr, memory_8bit, lsr8, lsr16);
            let base_cycles: u8 = 7;
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 3 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x66 => {
            // ROR direct page
            let (addr, penalty) = read_direct_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = memory_is_8bit(state);
            modify_memory(state, bus, addr, memory_8bit, ror8, ror16);
            let base_cycles: u8 = 5;
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x76 => {
            // ROR direct page,X
            let (addr, penalty) = read_direct_x_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = memory_is_8bit(state);
            modify_memory(state, bus, addr, memory_8bit, ror8, ror16);
            let base_cycles: u8 = 6;
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x6E => {
            // ROR absolute
            let addr = read_absolute_address_generic(state, bus);
            let memory_8bit = memory_is_8bit(state);
            modify_memory(state, bus, addr, memory_8bit, ror8, ror16);
            let base_cycles: u8 = 6;
            let already_accounted: u8 = 3;
            add_cycles(state, base_cycles.saturating_sub(already_accounted));
            base_cycles
        }

        0x7E => {
            // ROR absolute,X
            let (addr, penalty) = read_absolute_x_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = memory_is_8bit(state);
            modify_memory(state, bus, addr, memory_8bit, ror8, ror16);
            let base_cycles: u8 = 7;
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 3 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x89 => {
            // BIT immediate
            let memory_8bit = memory_is_8bit(state);
            let operand = if memory_8bit {
                read_u8_generic(state, bus) as u16
            } else {
                read_u16_generic(state, bus)
            };
            bit_operand_immediate(state, operand);
            let total_cycles: u8 = if memory_8bit { 2 } else { 3 };
            let already_accounted: u8 = if memory_8bit { 1 } else { 2 };
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x24 => {
            // BIT direct page
            let (addr, penalty) = read_direct_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = memory_is_8bit(state);
            let operand = read_operand_m(state, bus, addr, memory_8bit);
            bit_operand_memory(state, operand);
            let base_cycles: u8 = 3;
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x34 => {
            // BIT direct page,X
            let (addr, penalty) = read_direct_x_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = memory_is_8bit(state);
            let operand = read_operand_m(state, bus, addr, memory_8bit);
            bit_operand_memory(state, operand);
            let base_cycles: u8 = 4;
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x2C => {
            // BIT absolute
            let addr = read_absolute_address_generic(state, bus);
            let memory_8bit = memory_is_8bit(state);
            let operand = read_operand_m(state, bus, addr, memory_8bit);
            bit_operand_memory(state, operand);
            if addr == 0x004210 && crate::debug_flags::debug_bit4210() {
                let log_all = crate::debug_flags::debug_bit4210_all();
                let interesting = operand != 0x0002 || log_all;
                if interesting {
                    println!(
                        "[BIT4210] pc_next={:04X} A=0x{:04X} operand=0x{:04X} M8={} P_after=0x{:02X} (N={} V={} Z={})",
                        state.pc,
                        state.a,
                        operand,
                        memory_8bit,
                        state.p.bits(),
                        state.p.contains(StatusFlags::NEGATIVE) as u8,
                        state.p.contains(StatusFlags::OVERFLOW) as u8,
                        state.p.contains(StatusFlags::ZERO) as u8,
                    );
                }
            }
            let base_cycles: u8 = 4;
            let already_accounted: u8 = 3;
            add_cycles(state, base_cycles.saturating_sub(already_accounted));
            base_cycles
        }

        0x3C => {
            // BIT absolute,X
            let (addr, penalty) = read_absolute_x_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = memory_is_8bit(state);
            let operand = read_operand_m(state, bus, addr, memory_8bit);
            bit_operand_memory(state, operand);
            let base_cycles: u8 = 4;
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 3 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        // Logical operations
        0x29 => {
            // AND immediate
            let memory_8bit = memory_is_8bit(state);
            let operand = if memory_8bit {
                read_u8_generic(state, bus) as u16
            } else {
                read_u16_generic(state, bus)
            };
            and_operand(state, operand);
            let total_cycles: u8 = if memory_8bit { 2 } else { 3 };
            let already_accounted: u8 = if memory_8bit { 1 } else { 2 };
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }
        0x49 => {
            // EOR immediate
            let memory_8bit = memory_is_8bit(state);
            let operand = if memory_8bit {
                read_u8_generic(state, bus) as u16
            } else {
                read_u16_generic(state, bus)
            };
            eor_operand(state, operand);
            let total_cycles: u8 = if memory_8bit { 2 } else { 3 };
            let already_accounted: u8 = if memory_8bit { 1 } else { 2 };
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }
        0x09 => {
            // ORA immediate
            let memory_8bit = memory_is_8bit(state);
            let operand = if memory_8bit {
                read_u8_generic(state, bus) as u16
            } else {
                read_u16_generic(state, bus)
            };
            ora_operand(state, operand);
            let total_cycles: u8 = if memory_8bit { 2 } else { 3 };
            let already_accounted: u8 = if memory_8bit { 1 } else { 2 };
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        _ => return None,
    })
}
