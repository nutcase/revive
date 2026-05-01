use super::{read_u16_generic, read_u24_generic, read_u8_generic};
use crate::{cpu::bus::CpuBus, cpu::core::CoreState};

#[inline(always)]
pub(super) fn read_absolute_address_generic<T: CpuBus>(state: &mut CoreState, bus: &mut T) -> u32 {
    let addr = read_u16_generic(state, bus);
    ((state.db as u32) << 16) | (addr as u32)
}

#[inline(always)]
pub(super) fn read_direct_address_generic<T: CpuBus>(
    state: &mut CoreState,
    bus: &mut T,
) -> (u32, u8) {
    let offset = read_u8_generic(state, bus) as u16;
    let penalty = if state.dp & 0x00FF != 0 { 1 } else { 0 };
    let addr = state.dp.wrapping_add(offset) as u32;
    (addr, penalty)
}

pub(super) fn read_direct_x_address_generic<T: CpuBus>(
    state: &mut CoreState,
    bus: &mut T,
) -> (u32, u8) {
    let offset = read_u8_generic(state, bus) as u16;
    let penalty = if state.dp & 0x00FF != 0 { 1 } else { 0 };
    let addr = if state.emulation_mode && (state.dp & 0x00FF) == 0 {
        // 6502-style wrap within the direct page when D is page-aligned in emulation mode.
        let low = offset.wrapping_add(state.x & 0x00FF) & 0x00FF;
        ((state.dp & 0xFF00) | low) as u32
    } else {
        state.dp.wrapping_add(offset).wrapping_add(state.x) as u32
    };
    (addr, penalty)
}

pub(super) fn read_direct_y_address_generic<T: CpuBus>(
    state: &mut CoreState,
    bus: &mut T,
) -> (u32, u8) {
    let offset = read_u8_generic(state, bus) as u16;
    let penalty = if state.dp & 0x00FF != 0 { 1 } else { 0 };
    let addr = if state.emulation_mode && (state.dp & 0x00FF) == 0 {
        // 6502-style wrap within the direct page when D is page-aligned in emulation mode.
        let low = offset.wrapping_add(state.y & 0x00FF) & 0x00FF;
        ((state.dp & 0xFF00) | low) as u32
    } else {
        state.dp.wrapping_add(offset).wrapping_add(state.y) as u32
    };
    (addr, penalty)
}

pub(super) fn read_absolute_x_address_generic<T: CpuBus>(
    state: &mut CoreState,
    bus: &mut T,
) -> (u32, u8) {
    let base = read_u16_generic(state, bus);
    let low_sum = (base & 0x00FF) as u32 + (state.x & 0x00FF) as u32;
    let penalty = if low_sum >= 0x100 { 1 } else { 0 };
    // Absolute,X uses DBR for the bank. Indexing is applied to the full 24-bit address
    // (carry can propagate into the bank). This matters for WRAM $7E/$7F crossings.
    let base_full = ((state.db as u32) << 16) | (base as u32);
    let addr = base_full.wrapping_add(state.x as u32);
    (addr, penalty)
}

pub(super) fn read_absolute_y_address_generic<T: CpuBus>(
    state: &mut CoreState,
    bus: &mut T,
) -> (u32, u8) {
    let base = read_u16_generic(state, bus);
    let low_sum = (base & 0x00FF) as u32 + (state.y & 0x00FF) as u32;
    let penalty = if low_sum >= 0x100 { 1 } else { 0 };
    // Absolute,Y uses DBR for the bank; indexing is applied to the full 24-bit address.
    let base_full = ((state.db as u32) << 16) | (base as u32);
    let addr = base_full.wrapping_add(state.y as u32);
    (addr, penalty)
}

pub(super) fn read_absolute_long_address_generic<T: CpuBus>(
    state: &mut CoreState,
    bus: &mut T,
) -> u32 {
    read_u24_generic(state, bus)
}

pub(super) fn read_absolute_long_x_address_generic<T: CpuBus>(
    state: &mut CoreState,
    bus: &mut T,
) -> u32 {
    let base = read_u24_generic(state, bus);
    base.wrapping_add(state.x as u32)
}

pub(super) fn read_indirect_address_generic<T: CpuBus>(
    state: &mut CoreState,
    bus: &mut T,
) -> (u32, u8) {
    // (dp) - Direct Page Indirect
    // Operand is an 8-bit direct-page offset, not a 16-bit pointer operand.
    let base = read_u8_generic(state, bus) as u16;
    let penalty = if (state.dp & 0x00FF) != 0 { 1 } else { 0 };
    let ptr = state.dp.wrapping_add(base);
    let lo = bus.read_u8(ptr as u32) as u16;
    // Undocumented: in emulation mode, only a page-aligned D register exhibits 6502-style
    // wrapping for (dp) pointer reads.
    let hi_addr = if state.emulation_mode && (state.dp & 0x00FF) == 0 {
        (state.dp & 0xFF00) | ((base.wrapping_add(1)) & 0x00FF)
    } else {
        ptr.wrapping_add(1)
    };
    let hi = bus.read_u8(hi_addr as u32) as u16;
    let full = ((state.db as u32) << 16) | ((hi << 8) | lo) as u32;
    (full, penalty)
}

pub(super) fn read_indirect_x_address_generic<T: CpuBus>(
    state: &mut CoreState,
    bus: &mut T,
) -> (u32, u8) {
    let base = read_u8_generic(state, bus) as u16;
    let penalty = if state.dp & 0x00FF != 0 { 1 } else { 0 };
    let addr = if state.emulation_mode && (state.dp & 0x00FF) == 0 {
        // 6502-style wrap within the direct page when D is page-aligned in emulation mode.
        let low = base.wrapping_add(state.x & 0x00FF) & 0x00FF;
        (state.dp & 0xFF00) | low
    } else {
        state.dp.wrapping_add(base).wrapping_add(state.x)
    };
    let lo = bus.read_u8(addr as u32) as u16;
    // In emulation mode, indirect pointer reads wrap within the direct page (6502-style).
    let hi_addr = if state.emulation_mode {
        (addr & 0xFF00) | (addr.wrapping_add(1) & 0x00FF)
    } else {
        addr.wrapping_add(1)
    };
    let hi = bus.read_u8(hi_addr as u32) as u16;
    let full = ((state.db as u32) << 16) | ((hi << 8) | lo) as u32;
    (full, penalty)
}

pub(super) fn read_indirect_y_address_generic<T: CpuBus>(
    state: &mut CoreState,
    bus: &mut T,
) -> (u32, u8) {
    let base = read_u8_generic(state, bus) as u16;
    let mut penalty = 0u8;
    if state.dp & 0x00FF != 0 {
        penalty = penalty.saturating_add(1);
    }
    let addr = state.dp.wrapping_add(base);
    let lo = bus.read_u8(addr as u32) as u16;
    // Undocumented: in emulation mode, only a page-aligned D register exhibits 6502-style
    // wrapping for (dp),Y pointer reads.
    let hi_addr = if state.emulation_mode && (state.dp & 0x00FF) == 0 {
        (state.dp & 0xFF00) | ((base.wrapping_add(1)) & 0x00FF)
    } else {
        addr.wrapping_add(1)
    };
    let hi = bus.read_u8(hi_addr as u32) as u16;
    let base16 = (hi << 8) | lo;
    if ((base16 & 0x00FF) as u32) + (state.y & 0x00FF) as u32 >= 0x100 {
        penalty = penalty.saturating_add(1);
    }
    // (dp),Y uses DBR for the bank; indexing is applied to the full 24-bit address.
    let base_full = ((state.db as u32) << 16) | (base16 as u32);
    let full = base_full.wrapping_add(state.y as u32);
    (full, penalty)
}

pub(super) fn read_indirect_long_address_generic<T: CpuBus>(
    state: &mut CoreState,
    bus: &mut T,
) -> (u32, u8) {
    let pointer = read_u8_generic(state, bus) as u16;
    let mut penalty = 0u8;
    if state.dp & 0x00FF != 0 {
        penalty = penalty.saturating_add(1);
    }
    let addr = state.dp.wrapping_add(pointer);
    let lo = bus.read_u8(addr as u32) as u32;
    let mid = bus.read_u8(addr.wrapping_add(1) as u32) as u32;
    let hi = bus.read_u8(addr.wrapping_add(2) as u32) as u32;
    ((hi << 16) | (mid << 8) | lo, penalty)
}

pub(super) fn read_indirect_long_y_address_generic<T: CpuBus>(
    state: &mut CoreState,
    bus: &mut T,
) -> (u32, u8) {
    let pointer = read_u8_generic(state, bus) as u16;
    let mut penalty = 0u8;
    if state.dp & 0x00FF != 0 {
        penalty = penalty.saturating_add(1);
    }
    let addr = state.dp.wrapping_add(pointer);
    let lo = bus.read_u8(addr as u32) as u32;
    let mid = bus.read_u8(addr.wrapping_add(1) as u32) as u32;
    let hi = bus.read_u8(addr.wrapping_add(2) as u32) as u32;
    let full = (hi << 16) | (mid << 8) | lo;
    (full.wrapping_add(state.y as u32), penalty)
}

pub(super) fn read_stack_relative_address_generic<T: CpuBus>(
    state: &mut CoreState,
    bus: &mut T,
) -> u32 {
    let offset = read_u8_generic(state, bus) as u16;
    state.sp.wrapping_add(offset) as u32
}

pub(super) fn read_stack_relative_indirect_y_generic<T: CpuBus>(
    state: &mut CoreState,
    bus: &mut T,
) -> (u32, u8) {
    let offset = read_u8_generic(state, bus) as u16;
    let addr = state.sp.wrapping_add(offset);
    let lo = bus.read_u8(addr as u32) as u16;
    let hi = bus.read_u8(addr.wrapping_add(1) as u32) as u16;
    let base16 = (hi << 8) | lo;
    let mut penalty = 0u8;
    if ((base16 & 0x00FF) as u32) + (state.y & 0x00FF) as u32 >= 0x100 {
        penalty = penalty.saturating_add(1);
    }
    // (sr),Y uses DBR for the bank; indexing is applied to the full 24-bit address.
    let base_full = ((state.db as u32) << 16) | (base16 as u32);
    let full = base_full.wrapping_add(state.y as u32);
    (full, penalty)
}
