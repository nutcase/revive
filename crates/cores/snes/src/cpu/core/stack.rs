use super::add_cycles;
use crate::{cpu::bus::CpuBus, cpu::core::CoreState};

#[inline(always)]
pub(super) fn push_u8_generic<T: CpuBus>(state: &mut CoreState, bus: &mut T, value: u8) {
    let addr = if state.emulation_mode {
        0x0100 | (state.sp as u32)
    } else {
        state.sp as u32
    };
    if crate::debug_flags::trace_stack_guard() {
        println!(
            "STACK PUSH8 PB={:02X} PC={:04X} SP={:04X} -> [{:04X}]={:02X}",
            state.pb, state.pc, state.sp, addr, value
        );
    }
    bus.write_u8(addr, value);
    state.sp = if state.emulation_mode {
        0x0100 | ((state.sp.wrapping_sub(1)) & 0xFF)
    } else {
        state.sp.wrapping_sub(1)
    };
    add_cycles(state, 1);
}

#[inline(always)]
pub(super) fn push_u16_generic<T: CpuBus>(state: &mut CoreState, bus: &mut T, value: u16) {
    push_u8_generic(state, bus, (value >> 8) as u8);
    push_u8_generic(state, bus, (value & 0xFF) as u8);
}

// W65C816S datasheet note (emulation mode): some opcodes that push/pull 2+ bytes use a 16-bit
// stack increment/decrement sequence and can access outside $0100-$01FF when SP is near the edge.
// Examples: PEA/PEI/PER/PHD/PLD, JSL/RTL, JSR (abs,X).
#[inline]
pub(super) fn push_u16_emulation_edge<T: CpuBus>(state: &mut CoreState, bus: &mut T, value: u16) {
    bus.write_u8(state.sp as u32, (value >> 8) as u8);
    state.sp = state.sp.wrapping_sub(1);
    add_cycles(state, 1);
    bus.write_u8(state.sp as u32, (value & 0xFF) as u8);
    state.sp = state.sp.wrapping_sub(1);
    add_cycles(state, 1);
    // Re-assert emulation-mode stack high byte after the sequence.
    state.sp = 0x0100 | (state.sp & 0x00FF);
}

#[inline]
pub(super) fn pop_u16_emulation_edge<T: CpuBus>(state: &mut CoreState, bus: &mut T) -> u16 {
    state.sp = state.sp.wrapping_add(1);
    let lo = bus.read_u8(state.sp as u32) as u16;
    add_cycles(state, 1);
    state.sp = state.sp.wrapping_add(1);
    let hi = bus.read_u8(state.sp as u32) as u16;
    add_cycles(state, 1);
    // Re-assert emulation-mode stack high byte after the sequence.
    state.sp = 0x0100 | (state.sp & 0x00FF);
    (hi << 8) | lo
}

#[inline(always)]
pub(super) fn pop_u8_generic<T: CpuBus>(state: &mut CoreState, bus: &mut T) -> u8 {
    state.sp = if state.emulation_mode {
        0x0100 | ((state.sp.wrapping_add(1)) & 0xFF)
    } else {
        state.sp.wrapping_add(1)
    };
    let addr = if state.emulation_mode {
        0x0100 | (state.sp as u32)
    } else {
        state.sp as u32
    };
    add_cycles(state, 1);
    bus.read_u8(addr)
}

#[inline(always)]
pub(super) fn pop_u16_generic<T: CpuBus>(state: &mut CoreState, bus: &mut T) -> u16 {
    let lo = pop_u8_generic(state, bus) as u16;
    let hi = pop_u8_generic(state, bus) as u16;
    (hi << 8) | lo
}
