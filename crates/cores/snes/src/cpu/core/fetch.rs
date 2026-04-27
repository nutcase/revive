use super::{full_address, CoreState, FetchResult};
use crate::cpu::bus::CpuBus;

pub fn fetch_opcode(state: &mut CoreState, bus: &mut crate::bus::Bus) -> FetchResult {
    let pc_before = state.pc;
    let full_addr = full_address(state, pc_before);
    let opcode = apply_opcode_compat_patch(full_addr, bus.read_u8(full_addr));
    let mut memspeed_penalty = 0;
    if crate::debug_flags::mem_timing() && bus.is_rom_address(full_addr) && !bus.is_fastrom() {
        memspeed_penalty = 2;
    }
    state.pc = state.pc.wrapping_add(1);
    FetchResult {
        opcode,
        memspeed_penalty,
        pc_before,
        full_addr,
    }
}

// Generic version for SA-1 using CpuBus trait
pub fn fetch_opcode_generic<T: CpuBus>(state: &mut CoreState, bus: &mut T) -> FetchResult {
    let pc_before = state.pc;
    let full_addr = full_address(state, pc_before);
    let opcode = apply_opcode_compat_patch(full_addr, bus.read_u8(full_addr));
    let memspeed_penalty = bus.opcode_memory_penalty(full_addr);
    if crate::debug_flags::debug_fetch_pc()
        && state.pb == 0x00
        && state.pc >= 0x8240
        && state.pc <= 0x8270
    {
        println!(
            "[FETCH] PB={:02X} PC={:04X} OPCODE={:02X}",
            state.pb, state.pc, opcode
        );
    }
    state.pc = state.pc.wrapping_add(1);
    FetchResult {
        opcode,
        memspeed_penalty,
        pc_before,
        full_addr,
    }
}

#[inline(always)]
fn apply_opcode_compat_patch(full_addr: u32, opcode: u8) -> u8 {
    // Optional ROM-specific compat hook kept behind an explicit flag.
    if crate::debug_flags::jmp8cbe_to_jsr() && full_addr == 0x008CBE {
        0x20 // JSR absolute
    } else {
        opcode
    }
}
