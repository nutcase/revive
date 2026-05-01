use super::{
    full_address,
    memory::{add_cycles, read_u8_generic},
    CoreState,
};
use crate::{cpu::bus::CpuBus, cpu::StatusFlags};

#[inline(always)]
pub(super) fn set_flags_nz_8(state: &mut CoreState, value: u8) {
    state.p.set(StatusFlags::NEGATIVE, value & 0x80 != 0);
    state.p.set(StatusFlags::ZERO, value == 0);
}

#[inline(always)]
pub(super) fn set_flags_nz_16(state: &mut CoreState, value: u16) {
    state.p.set(StatusFlags::NEGATIVE, value & 0x8000 != 0);
    state.p.set(StatusFlags::ZERO, value == 0);
}

pub(super) fn apply_status_side_effects_after_pull(state: &mut CoreState, prev_p: StatusFlags) {
    // In emulation mode, M/X are forced to 1.
    if state.emulation_mode {
        state
            .p
            .insert(StatusFlags::MEMORY_8BIT | StatusFlags::INDEX_8BIT);
        return;
    }

    // If X flag changed 0->1 (16-bit -> 8-bit), high bytes of X/Y are cleared.
    let prev_x_16 = !prev_p.contains(StatusFlags::INDEX_8BIT);
    let new_x_16 = !state.p.contains(StatusFlags::INDEX_8BIT);
    if prev_x_16 && !new_x_16 {
        state.x &= 0x00FF;
        state.y &= 0x00FF;
    }
}

pub(super) fn rep_generic<T: CpuBus>(state: &mut CoreState, bus: &mut T) -> u8 {
    let mask = bus.read_u8(full_address(state, state.pc));
    state.pc = state.pc.wrapping_add(1);
    let new_flags = StatusFlags::from_bits_truncate(state.p.bits() & !mask);
    state.p = new_flags;
    // Emulation mode forces M/X=1; REP cannot clear them effectively.
    if state.emulation_mode {
        state
            .p
            .insert(StatusFlags::MEMORY_8BIT | StatusFlags::INDEX_8BIT);
    }
    if crate::debug_flags::trace_mflag() {
        println!(
            "[MFLAG] PC={:02X}:{:04X} REP #{:02X} -> P={:02X} emu={} A={:04X} X={:04X} Y={:04X}",
            state.pb,
            state.pc.wrapping_sub(1),
            mask,
            state.p.bits(),
            state.emulation_mode,
            state.a,
            state.x,
            state.y
        );
    }
    add_cycles(state, 3);
    3
}

pub(super) fn sep_generic<T: CpuBus>(state: &mut CoreState, bus: &mut T) -> u8 {
    let mask = read_u8_generic(state, bus);
    let prev_flags = state.p;
    let mut new_flags = StatusFlags::from_bits_truncate(prev_flags.bits() | mask);
    if state.emulation_mode {
        new_flags.insert(StatusFlags::MEMORY_8BIT | StatusFlags::INDEX_8BIT);
    }
    let prev_x_16 = !prev_flags.contains(StatusFlags::INDEX_8BIT) && !state.emulation_mode;
    let new_x_16 = !new_flags.contains(StatusFlags::INDEX_8BIT) && !state.emulation_mode;
    state.p = new_flags;
    if prev_x_16 && !new_x_16 {
        state.x &= 0x00FF;
        state.y &= 0x00FF;
    }
    // Accumulator upper byte (B) is preserved across M width changes.

    if crate::debug_flags::trace_mflag() {
        println!(
            "[MFLAG] PC={:02X}:{:04X} SEP #{:02X} -> P={:02X} emu={} A={:04X} X={:04X} Y={:04X}",
            state.pb,
            state.pc.wrapping_sub(1),
            mask,
            state.p.bits(),
            state.emulation_mode,
            state.a,
            state.x,
            state.y
        );
    }
    add_cycles(state, 2);
    3
}
