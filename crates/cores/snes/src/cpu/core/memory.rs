use super::{full_address, CoreState};
use crate::cpu::bus::CpuBus;

#[inline(always)]
pub(super) fn add_cycles(state: &mut CoreState, cycles: u8) {
    state.cycles = state.cycles.wrapping_add(cycles as u64);
}

#[inline(always)]
pub(super) fn read_u8_generic<T: CpuBus>(state: &mut CoreState, bus: &mut T) -> u8 {
    let addr = full_address(state, state.pc);
    let value = bus.read_u8(addr);
    state.pc = state.pc.wrapping_add(1);
    add_cycles(state, 1);
    value
}

#[inline(always)]
pub(super) fn write_u8_generic<T: CpuBus>(bus: &mut T, addr: u32, value: u8) {
    bus.write_u8(addr, value);
}

#[inline(always)]
pub(super) fn read_u16_generic<T: CpuBus>(state: &mut CoreState, bus: &mut T) -> u16 {
    let addr = full_address(state, state.pc);
    let value = bus.read_u16(addr);
    state.pc = state.pc.wrapping_add(2);
    add_cycles(state, 2);
    value
}

#[inline(always)]
pub(super) fn read_u24_generic<T: CpuBus>(state: &mut CoreState, bus: &mut T) -> u32 {
    let addr = full_address(state, state.pc);
    let lo = bus.read_u8(addr) as u32;
    let mid = bus.read_u8(addr + 1) as u32;
    let hi = bus.read_u8(addr + 2) as u32;
    state.pc = state.pc.wrapping_add(3);
    add_cycles(state, 3);
    lo | (mid << 8) | (hi << 16)
}
