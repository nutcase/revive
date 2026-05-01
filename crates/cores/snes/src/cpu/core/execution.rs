mod arithmetic;
mod logic;

use super::{
    addressing::*,
    alu::*,
    control_flow::*,
    flags::{
        apply_status_side_effects_after_pull, rep_generic, sep_generic, set_flags_nz_16,
        set_flags_nz_8,
    },
    full_address,
    memory::{add_cycles, read_u16_generic, read_u8_generic, write_u8_generic},
    stack::{
        pop_u16_emulation_edge, pop_u16_generic, pop_u8_generic, push_u16_emulation_edge,
        push_u16_generic, push_u8_generic,
    },
    CoreState,
};
use crate::{cpu::bus::CpuBus, cpu::StatusFlags};

pub fn execute_instruction_generic<T: CpuBus>(
    state: &mut CoreState,
    opcode: u8,
    bus: &mut T,
) -> u8 {
    // Debug: log first few iterations of the SMW APU upload loop to see what it waits for.
    if crate::debug_flags::trace_smw_apu_loop()
        && !crate::debug_flags::quiet()
        && state.pb == 0x00
        && (state.pc == 0x8BC5 || state.pc == 0x8BB2)
    {
        use std::sync::atomic::{AtomicU32, Ordering};
        static HITS: AtomicU32 = AtomicU32::new(0);
        let n = HITS.fetch_add(1, Ordering::Relaxed);
        if n < 32 {
            let p0 = bus.read_u8(0x2140);
            let p1 = bus.read_u8(0x2141);
            println!(
                "[SMW-APU-LOOP {:02}] p0={:02X} p1={:02X} X={:04X} Y={:04X}",
                n + 1,
                p0,
                p1,
                state.x,
                state.y
            );
        }
    }

    // PC watch hook (config: WATCH_PC=7DB6 or WATCH_PC=00:7DB6,7DC0,...)
    if let Some(list) = crate::debug_flags::watch_pc_list() {
        let full = ((state.pb as u32) << 16) | (state.pc as u32);
        if list.binary_search(&full).is_ok() || list.binary_search(&(state.pc as u32)).is_ok() {
            println!(
                "WATCH_PC hit at {:02X}:{:04X} A={:04X} X={:04X} Y={:04X} SP={:04X} D={:04X} DB={:02X} P={:02X}",
                state.pb,
                state.pc,
                state.a,
                state.x,
                state.y,
                state.sp,
                state.dp,
                state.db,
                state.p.bits()
            );
            // 直近のDPポインタ先頭16バイトをダンプして、間接参照の行方を追う
            let dbase = state.dp as u32;
            for i in 0..4u32 {
                let addr = dbase + i * 4;
                let b0 = bus.read_u8(addr);
                let b1 = bus.read_u8(addr + 1);
                let b2 = bus.read_u8(addr + 2);
                let b3 = bus.read_u8(addr + 3);
                println!(
                    "  DP+{:02X}: {:02X} {:02X} {:02X} {:02X}",
                    i * 4,
                    b0,
                    b1,
                    b2,
                    b3
                );
            }
        }
    }

    match opcode {
        0x04 | 0x05 | 0x15 | 0x0D | 0x1D | 0x19 | 0x0F | 0x1F | 0x01 | 0x11 | 0x12 | 0x13
        | 0x03 | 0x07 | 0x17 | 0x25 | 0x35 | 0x2D | 0x3D | 0x39 | 0x2F | 0x3F | 0x21 | 0x31
        | 0x32 | 0x33 | 0x23 | 0x27 | 0x37 | 0x45 | 0x55 | 0x4D | 0x5D | 0x59 | 0x4F | 0x5F
        | 0x41 | 0x51 | 0x52 | 0x53 | 0x43 | 0x47 | 0x57 | 0x0A | 0x2A | 0x4A | 0x6A | 0x06
        | 0x16 | 0x0E | 0x1E | 0x26 | 0x36 | 0x2E | 0x3E | 0x46 | 0x56 | 0x4E | 0x5E | 0x66
        | 0x76 | 0x6E | 0x7E | 0x89 | 0x24 | 0x34 | 0x2C | 0x3C | 0x29 | 0x49 | 0x09 => {
            logic::execute_logic_opcode(state, opcode, bus)
                .expect("logic opcode dispatch should match helper")
        }
        0x69 | 0x65 | 0x75 | 0x6D | 0x7D | 0x79 | 0x6F | 0x7F | 0x61 | 0x71 | 0x72 | 0x67
        | 0x77 | 0x63 | 0x73 | 0xE9 | 0xE5 | 0xF5 | 0xED | 0xFD | 0xF9 | 0xEF | 0xFF | 0xE1
        | 0xF1 | 0xE7 | 0xF7 | 0xE3 | 0xF3 | 0xF2 => {
            arithmetic::execute_arithmetic_opcode(state, opcode, bus)
                .expect("arithmetic opcode dispatch should match helper")
        }
        // Interrupt instructions - Essential for proper CPU operation
        0x00 => {
            let from_pb = state.pb;
            let from_pc = state.pc.wrapping_sub(1);
            let sp_before = state.sp;
            if crate::debug_flags::trace_brk() {
                println!(
                    "[BRK] at {:02X}:{:04X} A={:04X} X={:04X} Y={:04X} SP={:04X} P={:02X}",
                    from_pb,
                    state.pc,
                    state.a,
                    state.x,
                    state.y,
                    sp_before,
                    state.p.bits()
                );
            }
            if state.brk_is_nop {
                // Treat BRK as NOP (debug)
                add_cycles(state, 2);
                2
            } else {
                // BRK - Software Interrupt
                // BRK pushes PC+2 and status register, then jumps to BRK vector
                let next_pc = state.pc.wrapping_add(1); // BRK has a dummy operand byte
                state.pc = next_pc;

                // Push program bank (only in native mode)
                if !state.emulation_mode {
                    push_u8_generic(state, bus, state.pb);
                }

                // Push return address (PC after BRK + 1)
                push_u16_generic(state, bus, next_pc);

                // Push status register:
                // - Native mode: push P as-is (bits 4/5 are X/M)
                // - Emulation mode: push with B=1 and bit5 forced 1
                let status_to_push = if state.emulation_mode {
                    state.p.bits() | 0x30
                } else {
                    state.p.bits()
                };
                push_u8_generic(state, bus, status_to_push);

                // Set interrupt disable flag and clear decimal mode (65C816 behavior)
                state.p.insert(StatusFlags::IRQ_DISABLE);
                state.p.remove(StatusFlags::DECIMAL);

                // Jump to BRK vector
                let vector_addr = if state.emulation_mode { 0xFFFE } else { 0xFFE6 };
                let vector = bus.read_u16(vector_addr);
                state.pc = vector;

                // Interrupt vectors are always in bank 00
                state.pb = 0;
                trace_suspicious_control_flow(
                    "BRK",
                    from_pb,
                    from_pc,
                    0x00,
                    state.pb,
                    state.pc,
                    sp_before,
                    format!("vector={:04X} next_pc={:04X}", vector, next_pc),
                );

                add_cycles(state, if state.emulation_mode { 7 } else { 8 });
                if state.emulation_mode {
                    7
                } else {
                    8
                }
            }
        }

        0x02 => {
            // COP - Co-Processor Enable (software interrupt)
            let _signature = read_u8_generic(state, bus);
            let return_pc = state.pc;
            // - Native mode: push P as-is (bits 4/5 are X/M)
            // - Emulation mode: push with B=1 and bit5 forced 1 (6502-style)
            let pushed_status = if state.emulation_mode {
                state.p.bits() | 0x30
            } else {
                state.p.bits()
            };

            if state.emulation_mode {
                push_u16_generic(state, bus, return_pc);
                push_u8_generic(state, bus, pushed_status);
                let accounted = 1 + 3; // operand fetch + pushes (3 cycles)
                add_cycles(state, 7 - accounted);
            } else {
                push_u8_generic(state, bus, state.pb);
                push_u16_generic(state, bus, return_pc);
                push_u8_generic(state, bus, pushed_status);
                let accounted = 1 + 4; // operand fetch + pushes (4 cycles)
                add_cycles(state, 7 - accounted);
            }

            state.p.insert(StatusFlags::IRQ_DISABLE);
            state.p.remove(StatusFlags::DECIMAL);
            state.pb = 0;
            let vector_addr = if state.emulation_mode { 0xFFF4 } else { 0xFFE4 };
            let vector = bus.read_u16(vector_addr as u32);
            state.pc = vector;
            7
        }

        // Additional instructions needed by SA-1 test cases
        0x20 => jsr_generic(state, bus), // JSR absolute
        0x22 => jsl_generic(state, bus), // JSL long
        0x60 => rts_generic(state, bus), // RTS
        0x62 => per_generic(state, bus), // PER push effective relative address
        0x6B => rtl_generic(state, bus), // RTL
        0xC2 => rep_generic(state, bus), // REP
        0xE2 => sep_generic(state, bus), // SEP

        // Simple instructions that don't need bus access
        0xEA => {
            // NOP
            add_cycles(state, 2);
            2
        }
        0x18 => {
            // CLC
            state.p.remove(StatusFlags::CARRY);
            add_cycles(state, 2);
            2
        }

        0x1A => {
            // INC A
            if memory_is_8bit(state) {
                let value = ((state.a & 0xFF).wrapping_add(1)) as u8;
                state.a = (state.a & 0xFF00) | (value as u16);
                set_flags_nz_8(state, value);
            } else {
                state.a = state.a.wrapping_add(1);
                set_flags_nz_16(state, state.a);
            }
            add_cycles(state, 2);
            2
        }

        0x38 => {
            // SEC
            state.p.insert(StatusFlags::CARRY);
            add_cycles(state, 2);
            2
        }

        0x44 => {
            // MVP (Block Move Positive)
            // Operand order in object code: dest bank then src bank
            let dest_bank = read_u8_generic(state, bus);
            let src_bank = read_u8_generic(state, bus);
            // DBR becomes destination bank.
            state.db = dest_bank;
            let x_addr = if index_is_8bit(state) {
                state.x & 0x00FF
            } else {
                state.x
            };
            let y_addr = if index_is_8bit(state) {
                state.y & 0x00FF
            } else {
                state.y
            };
            let src_addr = ((src_bank as u32) << 16) | (x_addr as u32);
            let dest_addr = ((dest_bank as u32) << 16) | (y_addr as u32);
            let value = bus.read_u8(src_addr);
            bus.write_u8(dest_addr, value);
            if index_is_8bit(state) {
                state.x = (state.x & 0xFF00) | ((state.x as u8).wrapping_sub(1) as u16);
                state.y = (state.y & 0xFF00) | ((state.y as u8).wrapping_sub(1) as u16);
            } else {
                state.x = state.x.wrapping_sub(1);
                state.y = state.y.wrapping_sub(1);
            }
            state.a = state.a.wrapping_sub(1);
            if state.a != 0xFFFF {
                state.pc = state.pc.wrapping_sub(3);
            }
            let base_cycles: u8 = 7;
            let already_accounted: u8 = 2; // two immediate bytes already consumed
            add_cycles(state, base_cycles.saturating_sub(already_accounted));
            base_cycles
        }

        0x54 => {
            // MVN (Block Move Negative)
            // Operand order in object code: dest bank then src bank
            let dest_bank = read_u8_generic(state, bus);
            let src_bank = read_u8_generic(state, bus);
            // DBR becomes destination bank.
            state.db = dest_bank;
            let x_addr = if index_is_8bit(state) {
                state.x & 0x00FF
            } else {
                state.x
            };
            let y_addr = if index_is_8bit(state) {
                state.y & 0x00FF
            } else {
                state.y
            };
            let src_addr = ((src_bank as u32) << 16) | (x_addr as u32);
            let dest_addr = ((dest_bank as u32) << 16) | (y_addr as u32);
            let value = bus.read_u8(src_addr);
            bus.write_u8(dest_addr, value);
            if index_is_8bit(state) {
                state.x = (state.x & 0xFF00) | ((state.x as u8).wrapping_add(1) as u16);
                state.y = (state.y & 0xFF00) | ((state.y as u8).wrapping_add(1) as u16);
            } else {
                state.x = state.x.wrapping_add(1);
                state.y = state.y.wrapping_add(1);
            }
            state.a = state.a.wrapping_sub(1);
            if state.a != 0xFFFF {
                state.pc = state.pc.wrapping_sub(3);
            }
            let base_cycles: u8 = 7;
            let already_accounted: u8 = 2;
            add_cycles(state, base_cycles.saturating_sub(already_accounted));
            base_cycles
        }
        0x78 => {
            // SEI
            state.p.insert(StatusFlags::IRQ_DISABLE);
            add_cycles(state, 2);
            2
        }
        0xFB => {
            // XCE
            // C と E を入れ替える（C→新E, 旧E→新C）
            let old_emulation = state.emulation_mode;
            let new_emulation = state.p.contains(StatusFlags::CARRY);
            state.p.set(StatusFlags::CARRY, old_emulation);
            state.emulation_mode = new_emulation;
            if state.emulation_mode {
                // E=1 に入るときは M/X=1 を強制し、X/Y 上位をクリア、SP の上位バイトを 0x01 にする
                state
                    .p
                    .insert(StatusFlags::MEMORY_8BIT | StatusFlags::INDEX_8BIT);
                state.x &= 0x00FF;
                state.y &= 0x00FF;
                state.sp = (state.sp & 0x00FF) | 0x0100;
            }
            add_cycles(state, 2);
            2
        }

        // Jump instructions
        0x4C => {
            // JMP absolute
            let addr = read_u16_generic(state, bus);
            state.pc = addr;
            3
        }
        0x5C => {
            // JML long
            let addr_lo = bus.read_u8(full_address(state, state.pc)) as u32;
            let addr_hi = bus.read_u8(full_address(state, state.pc.wrapping_add(1))) as u32;
            let addr_bank = bus.read_u8(full_address(state, state.pc.wrapping_add(2))) as u32;
            let target = addr_lo | (addr_hi << 8) | (addr_bank << 16);
            state.pb = (target >> 16) as u8;
            state.pc = (target & 0xFFFF) as u16;
            add_cycles(state, 4);
            4
        }
        0x6C => {
            // JMP (addr)
            let ptr = read_u16_generic(state, bus);
            // Indirect pointer fetch is from bank 00 (not PB).
            // Also preserves the 6502 page-wrap bug when ptr ends in 0xFF.
            let lo = bus.read_u8(ptr as u32) as u16;
            let hi_addr = (ptr & 0xFF00) | (ptr.wrapping_add(1) & 0x00FF);
            let hi = bus.read_u8(hi_addr as u32) as u16;
            let target = lo | (hi << 8);
            state.pc = target;
            add_cycles(state, 5 - 2);
            5
        }
        0x7C => {
            // JMP (addr,X)
            let base = read_u16_generic(state, bus);
            let ptr = base.wrapping_add(state.x);
            let target = bus.read_u16(full_address(state, ptr));
            state.pc = target;
            add_cycles(state, 6 - 2);
            6
        }
        0xDC => {
            // JMP [addr]
            let ptr = read_u16_generic(state, bus);
            // Indirect long pointer fetch is from bank 00.
            let base = ptr as u32;
            let lo = bus.read_u8(base) as u32;
            let mid = bus.read_u8((ptr.wrapping_add(1)) as u32) as u32;
            let hi = bus.read_u8((ptr.wrapping_add(2)) as u32) as u32;
            let target = (hi << 16) | (mid << 8) | lo;
            state.pb = ((target >> 16) & 0xFF) as u8;
            state.pc = (target & 0xFFFF) as u16;
            add_cycles(state, 6 - 2);
            6
        }

        0xA1 => {
            // LDA (dp,X)
            let (addr, penalty) = read_indirect_x_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = memory_is_8bit(state);
            let value = read_operand_m(state, bus, addr, memory_8bit);
            if memory_8bit {
                state.a = (state.a & 0xFF00) | (value & 0xFF);
                set_flags_nz_8(state, value as u8);
            } else {
                state.a = value;
                set_flags_nz_16(state, value);
            }
            let base_cycles: u8 = 6;
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0xA3 => {
            // LDA stack relative
            let addr = read_stack_relative_address_generic(state, bus);
            let memory_8bit = memory_is_8bit(state);
            let value = read_operand_m(state, bus, addr, memory_8bit);
            if memory_8bit {
                state.a = (state.a & 0xFF00) | (value & 0xFF);
                set_flags_nz_8(state, value as u8);
            } else {
                state.a = value;
                set_flags_nz_16(state, value);
            }
            let base_cycles: u8 = 4;
            let already_accounted: u8 = 1;
            add_cycles(state, base_cycles.saturating_sub(already_accounted));
            base_cycles
        }

        0xA4 => {
            // LDY direct page
            let (addr, penalty) = read_direct_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            if index_is_8bit(state) {
                let value = bus.read_u8(addr) as u16;
                state.y = (state.y & 0xFF00) | value;
                set_flags_nz_8(state, value as u8);
            } else {
                let value = bus.read_u16(addr);
                state.y = value;
                set_flags_nz_16(state, value);
            }
            let base_cycles: u8 = if index_is_8bit(state) { 3 } else { 4 };
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0xA5 => {
            // LDA direct page
            let (addr, penalty) = read_direct_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = state.emulation_mode || state.p.contains(StatusFlags::MEMORY_8BIT);
            if memory_8bit {
                let value = bus.read_u8(addr) as u16;
                state.a = (state.a & 0xFF00) | value;
                set_flags_nz_8(state, value as u8);
            } else {
                let value = bus.read_u16(addr);
                state.a = value;
                set_flags_nz_16(state, value);
            }
            let base_cycles: u8 = if memory_8bit { 3 } else { 4 };
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty; // operand fetch + dp penalty
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0xA6 => {
            // LDX direct page
            let (addr, penalty) = read_direct_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            if index_is_8bit(state) {
                let value = bus.read_u8(addr) as u16;
                state.x = (state.x & 0xFF00) | value;
                set_flags_nz_8(state, value as u8);
            } else {
                let value = bus.read_u16(addr);
                state.x = value;
                set_flags_nz_16(state, value);
            }
            let base_cycles: u8 = if index_is_8bit(state) { 3 } else { 4 };
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0xA7 => {
            // LDA [dp]
            let (addr, penalty) = read_indirect_long_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = memory_is_8bit(state);
            let value = read_operand_m(state, bus, addr, memory_8bit);
            if memory_8bit {
                state.a = (state.a & 0xFF00) | (value & 0xFF);
                set_flags_nz_8(state, value as u8);
            } else {
                state.a = value;
                set_flags_nz_16(state, value);
            }
            let base_cycles: u8 = 6;
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0xA9 => {
            // LDA immediate
            let memory_8bit = memory_is_8bit(state);
            let value = if memory_8bit {
                read_u8_generic(state, bus) as u16
            } else {
                read_u16_generic(state, bus)
            };
            if memory_8bit {
                state.a = (state.a & 0xFF00) | (value & 0x00FF);
                set_flags_nz_8(state, value as u8);
            } else {
                state.a = value;
                set_flags_nz_16(state, value);
            }
            add_cycles(state, if memory_8bit { 2 } else { 3 });
            if memory_8bit {
                2
            } else {
                3
            }
        }
        0xAC => {
            // LDY absolute
            let addr = read_absolute_address_generic(state, bus);
            if index_is_8bit(state) {
                let value = bus.read_u8(addr) as u16;
                state.y = (state.y & 0xFF00) | value;
                set_flags_nz_8(state, value as u8);
            } else {
                let value = bus.read_u16(addr);
                state.y = value;
                set_flags_nz_16(state, value);
            }
            add_cycles(state, 4);
            4
        }
        0xAE => {
            // LDX absolute
            let addr = read_absolute_address_generic(state, bus);
            if index_is_8bit(state) {
                let value = bus.read_u8(addr) as u16;
                state.x = (state.x & 0xFF00) | value;
                set_flags_nz_8(state, value as u8);
            } else {
                let value = bus.read_u16(addr);
                state.x = value;
                set_flags_nz_16(state, value);
            }
            add_cycles(state, 4);
            4
        }
        0xA2 => {
            // LDX immediate
            let index_8bit = index_is_8bit(state);
            let value = if index_8bit {
                read_u8_generic(state, bus) as u16
            } else {
                read_u16_generic(state, bus)
            };
            if index_8bit {
                state.x = (state.x & 0xFF00) | (value & 0x00FF);
                set_flags_nz_8(state, value as u8);
            } else {
                state.x = value;
                set_flags_nz_16(state, value);
            }
            add_cycles(state, if index_8bit { 2 } else { 3 });
            if index_8bit {
                2
            } else {
                3
            }
        }
        0xA8 => {
            // TAY (Transfer Accumulator to Y)
            if index_is_8bit(state) {
                state.y = (state.y & 0xFF00) | (state.a & 0xFF);
                set_flags_nz_8(state, (state.y & 0xFF) as u8);
            } else {
                state.y = state.a;
                set_flags_nz_16(state, state.y);
            }
            add_cycles(state, 2);
            2
        }
        0xAA => {
            // TAX (Transfer Accumulator to X)
            if index_is_8bit(state) {
                state.x = (state.x & 0xFF00) | (state.a & 0xFF);
                set_flags_nz_8(state, (state.x & 0xFF) as u8);
            } else {
                state.x = state.a;
                set_flags_nz_16(state, state.x);
            }
            add_cycles(state, 2);
            2
        }
        0xA0 => {
            // LDY immediate
            let index_8bit = index_is_8bit(state);
            let value = if index_8bit {
                read_u8_generic(state, bus) as u16
            } else {
                read_u16_generic(state, bus)
            };
            if index_8bit {
                state.y = (state.y & 0xFF00) | (value & 0x00FF);
                set_flags_nz_8(state, value as u8);
            } else {
                state.y = value;
                set_flags_nz_16(state, value);
            }
            add_cycles(state, if index_8bit { 2 } else { 3 });
            if index_8bit {
                2
            } else {
                3
            }
        }

        // Store instructions
        0x8D => {
            // STA absolute
            let addr = read_absolute_address_generic(state, bus);
            if state.p.contains(StatusFlags::MEMORY_8BIT) {
                write_u8_generic(bus, addr, state.a as u8);
                add_cycles(state, 4);
                4
            } else {
                bus.write_u16(addr, state.a);
                add_cycles(state, 5);
                5
            }
        }
        0x8E => {
            // STX absolute
            let addr = read_absolute_address_generic(state, bus);
            if state.p.contains(StatusFlags::INDEX_8BIT) {
                write_u8_generic(bus, addr, state.x as u8);
                add_cycles(state, 4);
                4
            } else {
                bus.write_u16(addr, state.x);
                add_cycles(state, 5);
                5
            }
        }
        0x8C => {
            // STY absolute
            let addr = read_absolute_address_generic(state, bus);
            if state.p.contains(StatusFlags::INDEX_8BIT) {
                write_u8_generic(bus, addr, state.y as u8);
                add_cycles(state, 4);
                4
            } else {
                bus.write_u16(addr, state.y);
                add_cycles(state, 5);
                5
            }
        }

        // Stack operations - Critical for SA-1 function calls
        0x0B => {
            // PHD - Push Direct Page register
            if state.emulation_mode {
                push_u16_emulation_edge(state, bus, state.dp);
            } else {
                push_u16_generic(state, bus, state.dp);
            }
            add_cycles(state, 4);
            4
        }

        0x48 => {
            // PHA
            if state.p.contains(StatusFlags::MEMORY_8BIT) {
                push_u8_generic(state, bus, state.a as u8);
                add_cycles(state, 3);
                3
            } else {
                push_u16_generic(state, bus, state.a);
                add_cycles(state, 4);
                4
            }
        }
        0x4B => {
            // PHK - Push Program Bank
            push_u8_generic(state, bus, state.pb);
            add_cycles(state, 3);
            3
        }
        0x5A => {
            // PHY - Push Y register
            if index_is_8bit(state) {
                push_u8_generic(state, bus, (state.y & 0xFF) as u8);
                add_cycles(state, 3);
                3
            } else {
                push_u16_generic(state, bus, state.y);
                add_cycles(state, 4);
                4
            }
        }
        0x68 => {
            // PLA
            if state.p.contains(StatusFlags::MEMORY_8BIT) {
                state.a = (state.a & 0xFF00) | (pop_u8_generic(state, bus) as u16);
                set_flags_nz_8(state, state.a as u8);
                add_cycles(state, 4);
                4
            } else {
                state.a = pop_u16_generic(state, bus);
                set_flags_nz_16(state, state.a);
                add_cycles(state, 5);
                5
            }
        }
        0x8B => {
            // PHB - Push Data Bank register
            push_u8_generic(state, bus, state.db);
            add_cycles(state, 3);
            3
        }
        0xAB => {
            // PLB - Pull Data Bank register
            if state.emulation_mode {
                // Undocumented emulation edge: PLB can pull using a 16-bit stack increment.
                // This can read from $0200.. when SP starts at $01FF.
                state.sp = state.sp.wrapping_add(1);
                state.db = bus.read_u8(state.sp as u32);
                add_cycles(state, 1);
                // Re-assert emulation-mode stack high byte after the sequence.
                state.sp = 0x0100 | (state.sp & 0x00FF);
            } else {
                state.db = pop_u8_generic(state, bus);
            }
            set_flags_nz_8(state, state.db);
            add_cycles(state, 4);
            4
        }
        0xDA => {
            // PHX - Push X register
            if index_is_8bit(state) {
                push_u8_generic(state, bus, (state.x & 0xFF) as u8);
                add_cycles(state, 3);
                3
            } else {
                push_u16_generic(state, bus, state.x);
                add_cycles(state, 4);
                4
            }
        }
        0xFA => {
            // PLX - Pull X register
            if index_is_8bit(state) {
                let value = pop_u8_generic(state, bus);
                state.x = (state.x & 0xFF00) | (value as u16);
                set_flags_nz_8(state, value);
                add_cycles(state, 4);
                4
            } else {
                state.x = pop_u16_generic(state, bus);
                set_flags_nz_16(state, state.x);
                add_cycles(state, 5);
                5
            }
        }
        0xF4 => {
            // PEA
            let value = read_u16_generic(state, bus);
            if state.emulation_mode {
                push_u16_emulation_edge(state, bus, value);
            } else {
                push_u16_generic(state, bus, value);
            }
            add_cycles(state, 5);
            5
        }
        0x1B => {
            // TCS - Transfer Accumulator to Stack Pointer
            let old_sp = state.sp;
            state.sp = if state.emulation_mode {
                0x0100 | (state.a & 0x00FF)
            } else {
                state.a
            };
            if crate::debug_flags::trace_sp_change() {
                println!(
                    "SP CHANGE TCS PB={:02X} PC={:04X} {:04X}->{:04X}",
                    state.pb, state.pc, old_sp, state.sp
                );
            }
            add_cycles(state, 2);
            2
        }

        // Additional instruction coverage
        0xD8 => {
            // CLD - Clear Decimal Mode Flag
            state.p.remove(StatusFlags::DECIMAL);
            add_cycles(state, 2);
            2
        }
        0x7A => {
            // PLY - Pull Y from Stack
            if state.p.contains(StatusFlags::INDEX_8BIT) {
                // 8bitモード: 下位1バイトのみ読み、上位は必ずクリア
                state.y = pop_u8_generic(state, bus) as u16;
                set_flags_nz_8(state, state.y as u8);
                add_cycles(state, 4);
                4
            } else {
                state.y = pop_u16_generic(state, bus);
                set_flags_nz_16(state, state.y);
                add_cycles(state, 5);
                5
            }
        }

        0x7B => {
            // TDC - Transfer Direct Page register to Accumulator
            state.a = state.dp;
            // TDC transfers to the 16-bit accumulator (C) and sets N/Z based on the 16-bit value,
            // regardless of the M flag.
            set_flags_nz_16(state, state.a);
            add_cycles(state, 2);
            2
        }
        0xCE => {
            // DEC absolute - Decrement Absolute Memory
            let addr = read_absolute_address_generic(state, bus);
            if state.p.contains(StatusFlags::MEMORY_8BIT) {
                let value = bus.read_u8(addr).wrapping_sub(1);
                write_u8_generic(bus, addr, value);
                set_flags_nz_8(state, value);
                add_cycles(state, 6);
                6
            } else {
                let value = bus.read_u16(addr).wrapping_sub(1);
                bus.write_u16(addr, value);
                set_flags_nz_16(state, value);
                add_cycles(state, 7);
                7
            }
        }
        0xCB => {
            // WAI - Wait for Interrupt
            // Enter the "waiting for interrupt" state so the outer CPU
            // loop can stall until either IRQ or NMI arrives.
            // (Both S-CPU and SA-1 share this core.)
            if crate::debug_flags::trace_wai() {
                println!(
                    "[WAI] enter wait at {:02X}:{:04X} P={:02X}",
                    state.pb,
                    state.pc,
                    state.p.bits()
                );
            }
            state.waiting_for_irq = true;
            add_cycles(state, 3);
            3
        }

        0xDB => {
            // STP - Stop the processor until reset
            state.stopped = true;
            add_cycles(state, 3);
            3
        }
        0xCC => {
            // CPY absolute - Compare Y with Absolute Memory
            let addr = read_absolute_address_generic(state, bus);
            if state.p.contains(StatusFlags::INDEX_8BIT) {
                let value = bus.read_u8(addr);
                let result = (state.y as u8).wrapping_sub(value);
                state.p.set(StatusFlags::CARRY, (state.y as u8) >= value);
                state.p.set(StatusFlags::ZERO, result == 0);
                state.p.set(StatusFlags::NEGATIVE, (result & 0x80) != 0);
                add_cycles(state, 4);
                4
            } else {
                let value = bus.read_u16(addr);
                let result = state.y.wrapping_sub(value);
                state.p.set(StatusFlags::CARRY, state.y >= value);
                state.p.set(StatusFlags::ZERO, result == 0);
                state.p.set(StatusFlags::NEGATIVE, (result & 0x8000) != 0);
                add_cycles(state, 5);
                5
            }
        }
        0xC3 => {
            // CMP stack relative
            let addr = read_stack_relative_address_generic(state, bus);
            let operand = if memory_is_8bit(state) {
                bus.read_u8(addr) as u16
            } else {
                bus.read_u16(addr)
            };
            cmp_operand(state, operand);
            let base_cycles: u8 = if memory_is_8bit(state) { 4 } else { 5 };
            let already_accounted: u8 = 1;
            add_cycles(state, base_cycles.saturating_sub(already_accounted));
            base_cycles
        }

        // Second batch of instruction coverage
        0xB3 => {
            // LDA stack relative indirect indexed (sr,S),Y
            let (addr, penalty) = read_stack_relative_indirect_y_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = memory_is_8bit(state);
            let value = read_operand_m(state, bus, addr, memory_8bit);
            if memory_8bit {
                state.a = (state.a & 0xFF00) | (value & 0x00FF);
                set_flags_nz_8(state, value as u8);
            } else {
                state.a = value;
                set_flags_nz_16(state, value);
            }
            let base_cycles: u8 = if memory_8bit { 7 } else { 8 };
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }
        0xC4 => {
            // CPY direct page
            let addr = read_u8_generic(state, bus) as u32 + state.dp as u32;
            if state.p.contains(StatusFlags::INDEX_8BIT) {
                let value = bus.read_u8(addr);
                let result = (state.y as u8).wrapping_sub(value);
                state.p.set(StatusFlags::CARRY, (state.y as u8) >= value);
                state.p.set(StatusFlags::ZERO, result == 0);
                state.p.set(StatusFlags::NEGATIVE, (result & 0x80) != 0);
                add_cycles(state, 3);
                3
            } else {
                let value = bus.read_u16(addr);
                let result = state.y.wrapping_sub(value);
                state.p.set(StatusFlags::CARRY, state.y >= value);
                state.p.set(StatusFlags::ZERO, result == 0);
                state.p.set(StatusFlags::NEGATIVE, (result & 0x8000) != 0);
                add_cycles(state, 4);
                4
            }
        }
        0xDF => {
            // CMP long,X - Compare with Long Indexed X
            let addr_lo = bus.read_u8(full_address(state, state.pc)) as u32;
            let addr_hi = bus.read_u8(full_address(state, state.pc.wrapping_add(1))) as u32;
            let addr_bank = bus.read_u8(full_address(state, state.pc.wrapping_add(2))) as u32;
            state.pc = state.pc.wrapping_add(3);
            let addr = (addr_lo | (addr_hi << 8) | (addr_bank << 16)).wrapping_add(state.x as u32);

            if state.p.contains(StatusFlags::MEMORY_8BIT) {
                let value = bus.read_u8(addr);
                let result = (state.a as u8).wrapping_sub(value);
                state.p.set(StatusFlags::CARRY, (state.a as u8) >= value);
                state.p.set(StatusFlags::ZERO, result == 0);
                state.p.set(StatusFlags::NEGATIVE, (result & 0x80) != 0);
                add_cycles(state, 5);
                5
            } else {
                let value = bus.read_u16(addr);
                let result = state.a.wrapping_sub(value);
                state.p.set(StatusFlags::CARRY, state.a >= value);
                state.p.set(StatusFlags::ZERO, result == 0);
                state.p.set(StatusFlags::NEGATIVE, (result & 0x8000) != 0);
                add_cycles(state, 6);
                6
            }
        }
        0xF8 => {
            // SED - Set Decimal Mode Flag
            state.p.insert(StatusFlags::DECIMAL);
            add_cycles(state, 2);
            2
        }
        // Third batch of instruction coverage
        // Fourth batch: additional instruction coverage
        0xB6 => {
            // LDX direct page,Y
            let (addr, penalty) = read_direct_y_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            if index_is_8bit(state) {
                let value = bus.read_u8(addr);
                state.x = (state.x & 0xFF00) | (value as u16);
                set_flags_nz_8(state, value);
            } else {
                let value = bus.read_u16(addr);
                state.x = value;
                set_flags_nz_16(state, value);
            }
            let base_cycles: u8 = if index_is_8bit(state) { 4 } else { 5 };
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0xBE => {
            // LDX absolute,Y
            let (addr, penalty) = read_absolute_y_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            if index_is_8bit(state) {
                let value = bus.read_u8(addr);
                state.x = (state.x & 0xFF00) | (value as u16);
                set_flags_nz_8(state, value);
            } else {
                let value = bus.read_u16(addr);
                state.x = value;
                set_flags_nz_16(state, value);
            }
            let base_cycles: u8 = if index_is_8bit(state) { 4 } else { 5 };
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 2 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x58 => {
            // CLI - Clear Interrupt Disable
            state.p.remove(StatusFlags::IRQ_DISABLE);
            add_cycles(state, 2);
            2
        }

        0x08 => {
            // PHP - Push Processor Status
            let mut value = state.p.bits();
            if state.emulation_mode {
                // Emulation mode: push with B=1 and bit5 forced 1 (6502-compatible)
                value |= 0x30;
            }
            push_u8_generic(state, bus, value);
            add_cycles(state, 3);
            3
        }

        0xB4 => {
            // LDY direct page,X
            let (addr, penalty) = read_direct_x_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            if index_is_8bit(state) {
                let value = bus.read_u8(addr);
                state.y = (state.y & 0xFF00) | (value as u16);
                set_flags_nz_8(state, value);
            } else {
                let value = bus.read_u16(addr);
                state.y = value;
                set_flags_nz_16(state, value);
            }
            let base_cycles: u8 = if index_is_8bit(state) { 4 } else { 5 };
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x10 => branch_if_generic(state, bus, !state.p.contains(StatusFlags::NEGATIVE)),

        0xD5 => {
            // CMP direct page,X
            let (addr, penalty) = read_direct_x_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = memory_is_8bit(state);
            let operand = read_operand_m(state, bus, addr, memory_8bit);
            cmp_operand(state, operand);
            let base_cycles: u8 = if memory_8bit { 4 } else { 5 };
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0xD6 => {
            // DEC direct page,X
            let (addr, penalty) = read_direct_x_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            if memory_is_8bit(state) {
                let value = bus.read_u8(addr).wrapping_sub(1);
                bus.write_u8(addr, value);
                set_flags_nz_8(state, value);
            } else {
                let value = bus.read_u16(addr).wrapping_sub(1);
                bus.write_u16(addr, value);
                set_flags_nz_16(state, value);
            }
            let base_cycles: u8 = 6;
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0xB9 => {
            // LDA absolute,Y
            let (addr, penalty) = read_absolute_y_address_generic(state, bus);
            if state.p.contains(StatusFlags::MEMORY_8BIT) {
                let value = bus.read_u8(addr);
                state.a = (state.a & 0xFF00) | (value as u16);
                set_flags_nz_8(state, value);
                let base_cycles: u8 = 4;
                let total_cycles = base_cycles.saturating_add(penalty);
                let already_accounted: u8 = 2 + penalty;
                add_cycles(state, total_cycles.saturating_sub(already_accounted));
                total_cycles
            } else {
                let value = bus.read_u16(addr);
                state.a = value;
                set_flags_nz_16(state, value);
                let base_cycles: u8 = 5;
                let total_cycles = base_cycles.saturating_add(penalty);
                let already_accounted: u8 = 2 + penalty;
                add_cycles(state, total_cycles.saturating_sub(already_accounted));
                total_cycles
            }
        }

        0xBA => {
            // TSX - Transfer Stack Pointer to X
            if state.p.contains(StatusFlags::INDEX_8BIT) {
                let sp_low = state.sp as u8;
                state.x = (state.x & 0xFF00) | (sp_low as u16);
                set_flags_nz_8(state, sp_low);
            } else {
                state.x = state.sp;
                set_flags_nz_16(state, state.sp);
            }
            add_cycles(state, 2);
            2
        }

        0xBB => {
            // TYX - Transfer Y to X
            state.x = state.y;
            set_flags_index(state, state.x);
            add_cycles(state, 2);
            2
        }

        0xD4 => {
            // PEI - Push Effective Indirect Address
            let dp_offset = read_u8_generic(state, bus) as u32;
            let indirect_addr = (state.dp as u32 + dp_offset) & 0xFFFFFF;
            let effective_addr = bus.read_u16(indirect_addr);
            if state.emulation_mode {
                push_u16_emulation_edge(state, bus, effective_addr);
            } else {
                push_u16_generic(state, bus, effective_addr);
            }
            add_cycles(state, 6);
            6
        }

        // Fifth batch: more instruction coverage
        0x0C => {
            // TSB absolute - Test and Set Bits
            let addr = read_absolute_address_generic(state, bus);
            if state.p.contains(StatusFlags::MEMORY_8BIT) {
                let value = bus.read_u8(addr);
                let a_low = state.a as u8;
                state.p.set(StatusFlags::ZERO, (a_low & value) == 0);
                bus.write_u8(addr, value | a_low);
                add_cycles(state, 6);
                6
            } else {
                let value = bus.read_u16(addr);
                state.p.set(StatusFlags::ZERO, (state.a & value) == 0);
                bus.write_u16(addr, value | state.a);
                add_cycles(state, 8);
                8
            }
        }

        0xC1 => {
            // CMP indirect,X
            let (addr, penalty) = read_indirect_x_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let operand = if memory_is_8bit(state) {
                bus.read_u8(addr) as u16
            } else {
                bus.read_u16(addr)
            };
            cmp_operand(state, operand);
            let base_cycles: u8 = if memory_is_8bit(state) { 6 } else { 7 };
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0xC5 => {
            // CMP direct page
            let (addr, penalty) = read_direct_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            if memory_is_8bit(state) {
                let value = bus.read_u8(addr);
                let a_low = (state.a & 0xFF) as u8;
                let result = a_low.wrapping_sub(value);
                state.p.set(StatusFlags::CARRY, a_low >= value);
                state.p.set(StatusFlags::ZERO, result == 0);
                state.p.set(StatusFlags::NEGATIVE, (result & 0x80) != 0);
                let base_cycles: u8 = 3;
                let total_cycles = base_cycles.saturating_add(penalty);
                let already_accounted: u8 = 1 + penalty;
                add_cycles(state, total_cycles.saturating_sub(already_accounted));
                total_cycles
            } else {
                let value = bus.read_u16(addr);
                let result = state.a.wrapping_sub(value);
                state.p.set(StatusFlags::CARRY, state.a >= value);
                state.p.set(StatusFlags::ZERO, result == 0);
                state.p.set(StatusFlags::NEGATIVE, (result & 0x8000) != 0);
                let base_cycles: u8 = 4;
                let total_cycles = base_cycles.saturating_add(penalty);
                let already_accounted: u8 = 1 + penalty;
                add_cycles(state, total_cycles.saturating_sub(already_accounted));
                total_cycles
            }
        }

        0xC6 => {
            // DEC direct page
            let (addr, penalty) = read_direct_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            if memory_is_8bit(state) {
                let value = bus.read_u8(addr).wrapping_sub(1);
                bus.write_u8(addr, value);
                set_flags_nz_8(state, value);
            } else {
                let value = bus.read_u16(addr).wrapping_sub(1);
                bus.write_u16(addr, value);
                set_flags_nz_16(state, value);
            }
            let base_cycles: u8 = 5;
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0xC7 => {
            // CMP [dp]
            let (addr, penalty) = read_indirect_long_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            if memory_is_8bit(state) {
                let value = bus.read_u8(addr);
                let a_low = (state.a & 0xFF) as u8;
                let result = a_low.wrapping_sub(value);
                state.p.set(StatusFlags::CARRY, a_low >= value);
                state.p.set(StatusFlags::ZERO, result == 0);
                state.p.set(StatusFlags::NEGATIVE, (result & 0x80) != 0);
            } else {
                let value = bus.read_u16(addr);
                let result = state.a.wrapping_sub(value);
                state.p.set(StatusFlags::CARRY, state.a >= value);
                state.p.set(StatusFlags::ZERO, result == 0);
                state.p.set(StatusFlags::NEGATIVE, (result & 0x8000) != 0);
            }
            let base_cycles: u8 = 6;
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0xC8 => {
            // INY
            if index_is_8bit(state) {
                let value = ((state.y & 0xFF).wrapping_add(1)) as u8;
                state.y = (state.y & 0xFF00) | (value as u16);
                set_flags_nz_8(state, value);
            } else {
                state.y = state.y.wrapping_add(1);
                set_flags_nz_16(state, state.y);
            }
            add_cycles(state, 2);
            2
        }

        0xCA => {
            // DEX
            if index_is_8bit(state) {
                let value = ((state.x & 0xFF).wrapping_sub(1)) as u8;
                state.x = (state.x & 0xFF00) | (value as u16);
                set_flags_nz_8(state, value);
            } else {
                state.x = state.x.wrapping_sub(1);
                set_flags_nz_16(state, state.x);
            }
            add_cycles(state, 2);
            2
        }

        0xCD => {
            // CMP absolute
            let addr = read_absolute_address_generic(state, bus);
            let operand = if memory_is_8bit(state) {
                bus.read_u8(addr) as u16
            } else {
                bus.read_u16(addr)
            };
            cmp_operand(state, operand);
            let base_cycles: u8 = 4;
            let already_accounted: u8 = 3;
            add_cycles(state, base_cycles.saturating_sub(already_accounted));
            base_cycles
        }

        0xCF => {
            // CMP absolute long
            let addr = read_absolute_long_address_generic(state, bus);
            let operand = if memory_is_8bit(state) {
                bus.read_u8(addr) as u16
            } else {
                bus.read_u16(addr)
            };
            cmp_operand(state, operand);
            let base_cycles: u8 = 5;
            let already_accounted: u8 = 3;
            add_cycles(state, base_cycles.saturating_sub(already_accounted));
            base_cycles
        }

        0xD1 => {
            // CMP (dp),Y
            let (addr, penalty) = read_indirect_y_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let operand = if memory_is_8bit(state) {
                bus.read_u8(addr) as u16
            } else {
                bus.read_u16(addr)
            };
            cmp_operand(state, operand);
            let base_cycles: u8 = 5;
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0xD2 => {
            // CMP (dp)
            let (addr, penalty) = read_indirect_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let operand = if memory_is_8bit(state) {
                bus.read_u8(addr) as u16
            } else {
                bus.read_u16(addr)
            };
            cmp_operand(state, operand);
            let base_cycles: u8 = 5;
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0xD3 => {
            // CMP (sr,S),Y
            let (addr, penalty) = read_stack_relative_indirect_y_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let operand = if memory_is_8bit(state) {
                bus.read_u8(addr) as u16
            } else {
                bus.read_u16(addr)
            };
            cmp_operand(state, operand);
            let base_cycles: u8 = 7;
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0xD7 => {
            // CMP [dp],Y
            let (addr, penalty) = read_indirect_long_y_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let operand = if memory_is_8bit(state) {
                bus.read_u8(addr) as u16
            } else {
                bus.read_u16(addr)
            };
            cmp_operand(state, operand);
            let base_cycles: u8 = 6;
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0xD9 => {
            // CMP absolute,Y
            let (addr, penalty) = read_absolute_y_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let operand = if memory_is_8bit(state) {
                bus.read_u8(addr) as u16
            } else {
                bus.read_u16(addr)
            };
            cmp_operand(state, operand);
            let base_cycles: u8 = 4;
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 2 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0xDD => {
            // CMP absolute,X
            let (addr, penalty) = read_absolute_x_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let operand = if memory_is_8bit(state) {
                bus.read_u8(addr) as u16
            } else {
                bus.read_u16(addr)
            };
            cmp_operand(state, operand);
            let base_cycles: u8 = 4;
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 2 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0xDE => {
            // DEC absolute,X
            let (addr, penalty) = read_absolute_x_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            if memory_is_8bit(state) {
                let value = bus.read_u8(addr).wrapping_sub(1);
                bus.write_u8(addr, value);
                set_flags_nz_8(state, value);
            } else {
                let value = bus.read_u16(addr).wrapping_sub(1);
                bus.write_u16(addr, value);
                set_flags_nz_16(state, value);
            }
            let base_cycles: u8 = 7;
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 2 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0xE4 => {
            // CPX direct page
            let addr = (read_u8_generic(state, bus) as u32 + state.dp as u32) & 0xFFFFFF;
            if state.p.contains(StatusFlags::INDEX_8BIT) {
                let value = bus.read_u8(addr);
                let x_low = state.x as u8;
                let result = x_low.wrapping_sub(value);
                state.p.set(StatusFlags::CARRY, x_low >= value);
                state.p.set(StatusFlags::ZERO, result == 0);
                state.p.set(StatusFlags::NEGATIVE, (result & 0x80) != 0);
                add_cycles(state, 3);
                3
            } else {
                let value = bus.read_u16(addr);
                let result = state.x.wrapping_sub(value);
                state.p.set(StatusFlags::CARRY, state.x >= value);
                state.p.set(StatusFlags::ZERO, result == 0);
                state.p.set(StatusFlags::NEGATIVE, (result & 0x8000) != 0);
                add_cycles(state, 4);
                4
            }
        }

        0x2B => {
            // PLD - Pull Direct Page Register
            state.dp = if state.emulation_mode {
                pop_u16_emulation_edge(state, bus)
            } else {
                pop_u16_generic(state, bus)
            };
            set_flags_nz_16(state, state.dp);
            add_cycles(state, 5);
            5
        }

        0x40 => {
            // RTI - Return from Interrupt
            let from_pb = state.pb;
            let from_pc = state.pc.wrapping_sub(1);
            let sp_before = state.sp;
            let prev_p = state.p;
            if state.emulation_mode {
                let status = pop_u8_generic(state, bus);
                state.p = StatusFlags::from_bits_truncate(status);
                apply_status_side_effects_after_pull(state, prev_p);
                let lo = pop_u8_generic(state, bus) as u16;
                let hi = pop_u8_generic(state, bus) as u16;
                state.pc = (hi << 8) | lo;
                trace_suspicious_control_flow(
                    "RTI",
                    from_pb,
                    from_pc,
                    0x40,
                    state.pb,
                    state.pc,
                    sp_before,
                    format!("status={:02X} popped={:04X}", status, state.pc),
                );
                add_cycles(state, 6);
                6
            } else {
                let status = pop_u8_generic(state, bus);
                state.p = StatusFlags::from_bits_truncate(status);
                apply_status_side_effects_after_pull(state, prev_p);
                let lo = pop_u8_generic(state, bus) as u16;
                let hi = pop_u8_generic(state, bus) as u16;
                state.pc = (hi << 8) | lo;
                state.pb = pop_u8_generic(state, bus);
                trace_suspicious_control_flow(
                    "RTI",
                    from_pb,
                    from_pc,
                    0x40,
                    state.pb,
                    state.pc,
                    sp_before,
                    format!(
                        "status={:02X} popped={:02X}:{:04X}",
                        status, state.pb, state.pc
                    ),
                );
                add_cycles(state, 7);
                7
            }
        }

        0x30 => branch_if_generic(state, bus, state.p.contains(StatusFlags::NEGATIVE)),
        0x50 => branch_if_generic(state, bus, !state.p.contains(StatusFlags::OVERFLOW)),
        0x70 => branch_if_generic(state, bus, state.p.contains(StatusFlags::OVERFLOW)),
        0x80 => branch_if_generic(state, bus, true),
        0x82 => brl_generic(state, bus),
        0x90 => branch_if_generic(state, bus, !state.p.contains(StatusFlags::CARRY)),
        0xB0 => branch_if_generic(state, bus, state.p.contains(StatusFlags::CARRY)),
        0xD0 => branch_if_generic(state, bus, !state.p.contains(StatusFlags::ZERO)),
        0xF0 => branch_if_generic(state, bus, state.p.contains(StatusFlags::ZERO)),

        0xFE => {
            // INC absolute,X
            let (addr, penalty) = read_absolute_x_address_generic(state, bus);
            if state.p.contains(StatusFlags::MEMORY_8BIT) {
                let value = bus.read_u8(addr).wrapping_add(1);
                bus.write_u8(addr, value);
                set_flags_nz_8(state, value);
                let base_cycles: u8 = 7;
                let total_cycles = base_cycles.saturating_add(penalty);
                let already_accounted: u8 = 3 + penalty;
                add_cycles(state, total_cycles.saturating_sub(already_accounted));
                total_cycles
            } else {
                let value = bus.read_u16(addr).wrapping_add(1);
                bus.write_u16(addr, value);
                set_flags_nz_16(state, value);
                let base_cycles: u8 = 9;
                let total_cycles = base_cycles.saturating_add(penalty);
                let already_accounted: u8 = 3 + penalty;
                add_cycles(state, total_cycles.saturating_sub(already_accounted));
                total_cycles
            }
        }

        0x8F => {
            // STA long absolute
            let pc_before = state.pc;
            let addr_lo = read_u8_generic(state, bus) as u32;
            let addr_hi = read_u8_generic(state, bus) as u32;
            let addr_bank = read_u8_generic(state, bus) as u32;
            let full_addr = addr_lo | (addr_hi << 8) | (addr_bank << 16);

            if crate::debug_flags::trace_sta_long() {
                use std::sync::atomic::{AtomicU32, Ordering};
                static COUNT: AtomicU32 = AtomicU32::new(0);
                let n = COUNT.fetch_add(1, Ordering::Relaxed);
                if n < 64 {
                    println!(
                        "[STA_LONG] PB={:02X} PC={:04X} bytes={:02X} {:02X} {:02X} -> {:06X} A={:04X} M8={}",
                        state.pb,
                        pc_before,
                        addr_lo,
                        addr_hi,
                        addr_bank,
                        full_addr,
                        state.a,
                        state.p.contains(StatusFlags::MEMORY_8BIT)
                    );
                }
            }

            if state.p.contains(StatusFlags::MEMORY_8BIT) {
                bus.write_u8(full_addr, (state.a & 0xFF) as u8);
                add_cycles(state, 5);
                5
            } else {
                bus.write_u16(full_addr, state.a);
                add_cycles(state, 6);
                6
            }
        }

        0x42 => {
            // WDM (No operation on SA-1, but consume signature byte)
            read_u8_generic(state, bus); // Read and ignore signature byte
            add_cycles(state, 2);
            2
        }

        0x3A => {
            // DEC A (Decrement Accumulator)
            if state.p.contains(StatusFlags::MEMORY_8BIT) {
                state.a = ((state.a & 0xFF).wrapping_sub(1) & 0xFF) | (state.a & 0xFF00);
                set_flags_nz_8(state, (state.a & 0xFF) as u8);
            } else {
                state.a = state.a.wrapping_sub(1);
                set_flags_nz_16(state, state.a);
            }
            add_cycles(state, 2);
            2
        }

        0x3B => {
            // TSC - Transfer Stack Pointer to Accumulator
            state.a = state.sp;
            // TSC transfers to the 16-bit accumulator (C) and sets N/Z based on the 16-bit value,
            // regardless of the M flag.
            set_flags_nz_16(state, state.a);
            add_cycles(state, 2);
            2
        }

        0x9A => {
            // TXS (Transfer X to Stack Pointer)
            let old_sp = state.sp;
            if state.emulation_mode {
                state.sp = 0x0100 | (state.x & 0xFF);
            } else {
                state.sp = state.x;
            }
            if crate::debug_flags::trace_sp_change() {
                println!(
                    "SP CHANGE TXS PB={:02X} PC={:04X} {:04X}->{:04X}",
                    state.pb, state.pc, old_sp, state.sp
                );
            }
            add_cycles(state, 2);
            2
        }

        0x9B => {
            // TXY - Transfer X to Y
            state.y = state.x;
            set_flags_index(state, state.y);
            add_cycles(state, 2);
            2
        }

        // Missing opcodes used by some test/edge cases
        0x99 => {
            // STA absolute,Y
            let (addr, penalty) = read_absolute_y_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            write_a_generic(state, bus, addr);
            let base_cycles: u8 = 5;
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 2 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x9C => {
            // STZ absolute
            let addr = read_absolute_address_generic(state, bus);
            if memory_is_8bit(state) {
                bus.write_u8(addr, 0);
            } else {
                bus.write_u16(addr, 0);
            }
            let base_cycles: u8 = 4;
            let already_accounted: u8 = 3;
            add_cycles(state, base_cycles.saturating_sub(already_accounted));
            base_cycles
        }

        0x9D => {
            // STA absolute,X
            let (addr, penalty) = read_absolute_x_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            write_a_generic(state, bus, addr);
            let base_cycles: u8 = 5;
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 2 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x9E => {
            // STZ absolute,X
            let (addr, penalty) = read_absolute_x_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            if memory_is_8bit(state) {
                bus.write_u8(addr, 0);
            } else {
                bus.write_u16(addr, 0);
            }
            let base_cycles: u8 = 5;
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 2 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x9F => {
            // STA long absolute,X
            let addr_lo = read_u8_generic(state, bus) as u32;
            let addr_hi = read_u8_generic(state, bus) as u32;
            let addr_bank = read_u8_generic(state, bus) as u32;
            let full_addr =
                (addr_lo | (addr_hi << 8) | (addr_bank << 16)).wrapping_add(state.x as u32);

            if state.p.contains(StatusFlags::MEMORY_8BIT) {
                bus.write_u8(full_addr, (state.a & 0xFF) as u8);
            } else {
                bus.write_u16(full_addr, state.a);
            }
            add_cycles(state, 5);
            5
        }

        0xB1 => {
            // LDA (dp),Y
            let (addr, penalty) = read_indirect_y_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = memory_is_8bit(state);
            let value = read_operand_m(state, bus, addr, memory_8bit);
            if memory_8bit {
                state.a = (state.a & 0xFF00) | (value & 0xFF);
                set_flags_nz_8(state, value as u8);
            } else {
                state.a = value;
                set_flags_nz_16(state, value);
            }
            let base_cycles: u8 = 5;
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0xB2 => {
            // LDA (dp)
            let (addr, penalty) = read_indirect_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = memory_is_8bit(state);
            let value = read_operand_m(state, bus, addr, memory_8bit);
            if memory_8bit {
                state.a = (state.a & 0xFF00) | (value & 0xFF);
                set_flags_nz_8(state, value as u8);
            } else {
                state.a = value;
                set_flags_nz_16(state, value);
            }
            let base_cycles: u8 = 5;
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0xB7 => {
            // LDA [dp],Y
            let (addr, penalty) = read_indirect_long_y_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = memory_is_8bit(state);
            let value = read_operand_m(state, bus, addr, memory_8bit);
            if memory_8bit {
                state.a = (state.a & 0xFF00) | (value & 0xFF);
                set_flags_nz_8(state, value as u8);
            } else {
                state.a = value;
                set_flags_nz_16(state, value);
            }
            let base_cycles: u8 = 6;
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0xB5 => {
            // LDA direct page,X
            let (addr, penalty) = read_direct_x_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = memory_is_8bit(state);
            if memory_8bit {
                let value = bus.read_u8(addr) as u16;
                state.a = (state.a & 0xFF00) | value;
                set_flags_nz_8(state, value as u8);
            } else {
                let value = bus.read_u16(addr);
                state.a = value;
                set_flags_nz_16(state, value);
            }
            let base_cycles: u8 = if memory_8bit { 4 } else { 5 };
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x14 => {
            // TRB direct page
            let (addr, penalty) = read_direct_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = memory_is_8bit(state);
            if memory_8bit {
                let value = bus.read_u8(addr);
                let result = value & !(state.a as u8);
                bus.write_u8(addr, result);
                state
                    .p
                    .set(StatusFlags::ZERO, (value & (state.a as u8)) == 0);
            } else {
                let value = bus.read_u16(addr);
                let result = value & !state.a;
                bus.write_u16(addr, result);
                state.p.set(StatusFlags::ZERO, (value & state.a) == 0);
            }
            let base_cycles: u8 = if memory_8bit { 5 } else { 6 };
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x1C => {
            // TRB absolute
            let addr = read_absolute_address_generic(state, bus);
            if memory_is_8bit(state) {
                let value = bus.read_u8(addr);
                let a_low = (state.a & 0xFF) as u8;
                state.p.set(StatusFlags::ZERO, (value & a_low) == 0);
                bus.write_u8(addr, value & !a_low);
                let base_cycles: u8 = 6;
                let already_accounted: u8 = 3;
                add_cycles(state, base_cycles.saturating_sub(already_accounted));
                base_cycles
            } else {
                let value = bus.read_u16(addr);
                state.p.set(StatusFlags::ZERO, (value & state.a) == 0);
                bus.write_u16(addr, value & !state.a);
                let base_cycles: u8 = 8;
                let already_accounted: u8 = 3;
                add_cycles(state, base_cycles.saturating_sub(already_accounted));
                base_cycles
            }
        }

        0x88 => {
            // DEY
            if index_is_8bit(state) {
                let value = ((state.y & 0xFF).wrapping_sub(1)) as u8;
                state.y = (state.y & 0xFF00) | (value as u16);
                set_flags_nz_8(state, value);
            } else {
                state.y = state.y.wrapping_sub(1);
                set_flags_nz_16(state, state.y);
            }
            add_cycles(state, 2);
            2
        }

        0x8A => {
            // TXA (Transfer X to Accumulator)
            if state.p.contains(StatusFlags::MEMORY_8BIT) {
                state.a = (state.a & 0xFF00) | (state.x & 0xFF);
                set_flags_nz_8(state, (state.a & 0xFF) as u8);
            } else {
                state.a = state.x;
                set_flags_nz_16(state, state.a);
            }
            add_cycles(state, 2);
            2
        }

        0x98 => {
            // TYA (Transfer Y to Accumulator)
            if memory_is_8bit(state) {
                state.a = (state.a & 0xFF00) | (state.y & 0xFF);
                set_flags_nz_8(state, (state.a & 0xFF) as u8);
            } else {
                state.a = state.y;
                set_flags_nz_16(state, state.a);
            }
            add_cycles(state, 2);
            2
        }

        0xAD => {
            // LDA absolute
            let addr = read_absolute_address_generic(state, bus);
            if state.p.contains(StatusFlags::MEMORY_8BIT) {
                state.a = (state.a & 0xFF00) | (bus.read_u8(addr) as u16);
                set_flags_nz_8(state, (state.a & 0xFF) as u8);
            } else {
                state.a = bus.read_u16(addr);
                set_flags_nz_16(state, state.a);
            }
            add_cycles(state, 4);
            4
        }

        0xAF => {
            // LDA absolute long
            let addr = read_absolute_long_address_generic(state, bus);
            let memory_8bit = memory_is_8bit(state);
            let value = read_operand_m(state, bus, addr, memory_8bit);
            if memory_8bit {
                state.a = (state.a & 0xFF00) | (value & 0xFF);
                set_flags_nz_8(state, value as u8);
            } else {
                state.a = value;
                set_flags_nz_16(state, value);
            }
            let base_cycles: u8 = 5;
            let already_accounted: u8 = 3;
            add_cycles(state, base_cycles.saturating_sub(already_accounted));
            base_cycles
        }

        0xBD => {
            // LDA absolute,X with page-cross penalty
            let (addr, penalty) = read_absolute_x_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = state.emulation_mode || state.p.contains(StatusFlags::MEMORY_8BIT);
            if memory_8bit {
                let value = bus.read_u8(addr) as u16;
                state.a = (state.a & 0xFF00) | value;
                set_flags_nz_8(state, value as u8);
            } else {
                let value = bus.read_u16(addr);
                state.a = value;
                set_flags_nz_16(state, value);
            }
            let base_cycles: u8 = 4;
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 2 + penalty; // operand fetch (2) + penalty already applied
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0xBF => {
            // LDA absolute long,X
            let addr = read_absolute_long_x_address_generic(state, bus);
            let memory_8bit = memory_is_8bit(state);
            let value = read_operand_m(state, bus, addr, memory_8bit);
            if memory_8bit {
                state.a = (state.a & 0xFF00) | (value & 0xFF);
                set_flags_nz_8(state, value as u8);
            } else {
                state.a = value;
                set_flags_nz_16(state, value);
            }
            let base_cycles: u8 = 5;
            let already_accounted: u8 = 3;
            add_cycles(state, base_cycles.saturating_sub(already_accounted));
            base_cycles
        }

        0x28 => {
            // PLP (Pull Processor Status)
            let prev_p = state.p;
            let value = pop_u8_generic(state, bus);
            state.p = StatusFlags::from_bits_truncate(value);
            apply_status_side_effects_after_pull(state, prev_p);
            add_cycles(state, 4);
            4
        }

        0x5B => {
            // TCD (Transfer Accumulator to Direct Page)
            state.dp = state.a;
            set_flags_nz_16(state, state.dp);
            add_cycles(state, 2);
            2
        }

        0xBC => {
            // LDY absolute,X
            let (addr, penalty) = read_absolute_x_address_generic(state, bus);
            if state.p.contains(StatusFlags::INDEX_8BIT) {
                state.y = (state.y & 0xFF00) | (bus.read_u8(addr) as u16);
                set_flags_nz_8(state, (state.y & 0xFF) as u8);
                let base_cycles: u8 = 4;
                let total_cycles = base_cycles.saturating_add(penalty);
                let already_accounted: u8 = 2 + penalty;
                add_cycles(state, total_cycles.saturating_sub(already_accounted));
                total_cycles
            } else {
                state.y = bus.read_u16(addr);
                set_flags_nz_16(state, state.y);
                let base_cycles: u8 = 4; // LDX/LDY abs,X same timing both widths
                let total_cycles = base_cycles.saturating_add(penalty);
                let already_accounted: u8 = 2 + penalty;
                add_cycles(state, total_cycles.saturating_sub(already_accounted));
                total_cycles
            }
        }

        0x83 => {
            // STA stack relative,S
            let offset = read_u8_generic(state, bus) as u16;
            let addr = state.sp.wrapping_add(offset);
            if state.p.contains(StatusFlags::MEMORY_8BIT) {
                bus.write_u8(addr as u32, (state.a & 0xFF) as u8);
            } else {
                bus.write_u16(addr as u32, state.a);
            }
            add_cycles(state, 4);
            4
        }

        0x94 => {
            // STY zero page,X
            let (addr, penalty) = read_direct_x_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            write_y_generic(state, bus, addr);
            let base_cycles: u8 = 4;
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x96 => {
            // STX direct page,Y
            let (addr, penalty) = read_direct_y_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            write_x_generic(state, bus, addr);
            let base_cycles: u8 = if index_is_8bit(state) { 4 } else { 5 };
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        // Additional instructions for BW-RAM communication cases
        0x64 => {
            // STZ direct page
            let (addr, penalty) = read_direct_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            if memory_is_8bit(state) {
                bus.write_u8(addr, 0);
            } else {
                bus.write_u16(addr, 0);
            }
            let base_cycles: u8 = 3;
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x74 => {
            // STZ direct page,X
            let (addr, penalty) = read_direct_x_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            if memory_is_8bit(state) {
                bus.write_u8(addr, 0);
            } else {
                bus.write_u16(addr, 0);
            }
            let base_cycles: u8 = 4;
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x81 => {
            // STA (dp,X)
            let (addr, penalty) = read_indirect_x_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            write_a_generic(state, bus, addr);
            let base_cycles: u8 = 6;
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x87 => {
            // STA [dp]
            let (addr, penalty) = read_indirect_long_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            write_a_generic(state, bus, addr);
            let base_cycles: u8 = 6;
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x91 => {
            // STA (dp),Y
            let (addr, penalty) = read_indirect_y_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            write_a_generic(state, bus, addr);
            let base_cycles: u8 = 6;
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x92 => {
            // STA (dp)
            let (addr, penalty) = read_indirect_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            write_a_generic(state, bus, addr);
            let base_cycles: u8 = 5;
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x93 => {
            // STA (sr,S),Y
            let (addr, penalty) = read_stack_relative_indirect_y_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            write_a_generic(state, bus, addr);
            let base_cycles: u8 = 7;
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x95 => {
            // STA dp,X
            let (addr, penalty) = read_direct_x_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            write_a_generic(state, bus, addr);
            let base_cycles: u8 = 4;
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x97 => {
            // STA [dp],Y
            let (addr, penalty) = read_indirect_long_y_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            write_a_generic(state, bus, addr);
            let base_cycles: u8 = 6;
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x85 => {
            // STA direct page
            let (addr, penalty) = read_direct_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            write_a_generic(state, bus, addr);
            let base_cycles: u8 = if memory_is_8bit(state) { 3 } else { 4 };
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x86 => {
            // STX direct page
            let (addr, penalty) = read_direct_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            write_x_generic(state, bus, addr);
            let base_cycles: u8 = if index_is_8bit(state) { 3 } else { 4 };
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x84 => {
            // STY direct page
            let (addr, penalty) = read_direct_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            write_y_generic(state, bus, addr);
            let base_cycles: u8 = if index_is_8bit(state) { 3 } else { 4 };
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0xC9 => {
            // CMP immediate
            if state.p.contains(StatusFlags::MEMORY_8BIT) {
                let value = read_u8_generic(state, bus);
                let result = (state.a as u8).wrapping_sub(value);
                state
                    .p
                    .set(StatusFlags::CARRY, (state.a & 0xFF) >= value as u16);
                state.p.set(StatusFlags::ZERO, result == 0);
                state.p.set(StatusFlags::NEGATIVE, (result & 0x80) != 0);
            } else {
                let value_lo = read_u8_generic(state, bus) as u16;
                let value_hi = read_u8_generic(state, bus) as u16;
                let value = value_lo | (value_hi << 8);
                let result = state.a.wrapping_sub(value);
                state.p.set(StatusFlags::CARRY, state.a >= value);
                state.p.set(StatusFlags::ZERO, result == 0);
                state.p.set(StatusFlags::NEGATIVE, (result & 0x8000) != 0);
            }
            add_cycles(state, 2);
            2
        }

        0xE0 => {
            // CPX immediate
            if state.p.contains(StatusFlags::INDEX_8BIT) {
                let value = read_u8_generic(state, bus);
                let result = (state.x as u8).wrapping_sub(value);
                state
                    .p
                    .set(StatusFlags::CARRY, (state.x & 0xFF) >= value as u16);
                state.p.set(StatusFlags::ZERO, result == 0);
                state.p.set(StatusFlags::NEGATIVE, (result & 0x80) != 0);
            } else {
                let value_lo = read_u8_generic(state, bus) as u16;
                let value_hi = read_u8_generic(state, bus) as u16;
                let value = value_lo | (value_hi << 8);
                let result = state.x.wrapping_sub(value);
                state.p.set(StatusFlags::CARRY, state.x >= value);
                state.p.set(StatusFlags::ZERO, result == 0);
                state.p.set(StatusFlags::NEGATIVE, (result & 0x8000) != 0);
            }
            add_cycles(state, 2);
            2
        }

        0xE6 => {
            // INC direct page
            let (addr, penalty) = read_direct_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            if memory_is_8bit(state) {
                let value = bus.read_u8(addr).wrapping_add(1);
                bus.write_u8(addr, value);
                set_flags_nz_8(state, value);
            } else {
                let value = bus.read_u16(addr).wrapping_add(1);
                bus.write_u16(addr, value);
                set_flags_nz_16(state, value);
            }
            let base_cycles: u8 = 5;
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0xE8 => {
            // INX
            if index_is_8bit(state) {
                let value = ((state.x & 0xFF).wrapping_add(1)) as u8;
                state.x = (state.x & 0xFF00) | (value as u16);
                set_flags_nz_8(state, value);
            } else {
                state.x = state.x.wrapping_add(1);
                set_flags_nz_16(state, state.x);
            }
            add_cycles(state, 2);
            2
        }

        0xEB => {
            // XBA - Exchange B and A
            let low = (state.a & 0xFF) as u8;
            let high = (state.a >> 8) as u8;
            state.a = ((low as u16) << 8) | (high as u16);
            let new_low = (state.a & 0xFF) as u8;
            state.p.set(StatusFlags::ZERO, new_low == 0);
            state.p.set(StatusFlags::NEGATIVE, (new_low & 0x80) != 0);
            add_cycles(state, 3);
            3
        }

        0xEC => {
            // CPX absolute
            let addr = read_absolute_address_generic(state, bus);
            if index_is_8bit(state) {
                let value = bus.read_u8(addr);
                let result = (state.x as u8).wrapping_sub(value);
                state
                    .p
                    .set(StatusFlags::CARRY, (state.x & 0xFF) >= value as u16);
                state.p.set(StatusFlags::ZERO, result == 0);
                state.p.set(StatusFlags::NEGATIVE, (result & 0x80) != 0);
            } else {
                let value = bus.read_u16(addr);
                let result = state.x.wrapping_sub(value);
                state.p.set(StatusFlags::CARRY, state.x >= value);
                state.p.set(StatusFlags::ZERO, result == 0);
                state.p.set(StatusFlags::NEGATIVE, (result & 0x8000) != 0);
            }
            add_cycles(state, 4);
            4
        }

        0xEE => {
            // INC absolute
            let addr = read_absolute_address_generic(state, bus);
            if memory_is_8bit(state) {
                let value = bus.read_u8(addr).wrapping_add(1);
                bus.write_u8(addr, value);
                set_flags_nz_8(state, value);
            } else {
                let value = bus.read_u16(addr).wrapping_add(1);
                bus.write_u16(addr, value);
                set_flags_nz_16(state, value);
            }
            add_cycles(state, 6);
            6
        }

        0xF6 => {
            // INC direct page,X
            let (addr, penalty) = read_direct_x_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            if memory_is_8bit(state) {
                let value = bus.read_u8(addr).wrapping_add(1);
                bus.write_u8(addr, value);
                set_flags_nz_8(state, value);
            } else {
                let value = bus.read_u16(addr).wrapping_add(1);
                bus.write_u16(addr, value);
                set_flags_nz_16(state, value);
            }
            let base_cycles: u8 = 6;
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0xFC => {
            // JSR (addr,X)
            let base = read_u16_generic(state, bus);
            let addr = base.wrapping_add(state.x);
            // Indirect target fetch uses the current program bank (PB).
            let target = bus.read_u16(full_address(state, addr));
            let return_addr = state.pc.wrapping_sub(1);
            if state.emulation_mode {
                // Undocumented emulation edge: JSR (abs,X) uses a 16-bit stack decrement for the push.
                bus.write_u8(state.sp as u32, (return_addr >> 8) as u8);
                state.sp = state.sp.wrapping_sub(1);
                add_cycles(state, 1);
                bus.write_u8(state.sp as u32, (return_addr & 0xFF) as u8);
                state.sp = state.sp.wrapping_sub(1);
                add_cycles(state, 1);
                // Re-assert emulation-mode stack high byte after the sequence.
                state.sp = 0x0100 | (state.sp & 0x00FF);
            } else {
                push_u16_generic(state, bus, return_addr);
            }
            state.pc = target;
            let base_cycles: u8 = 8;
            let accounted: u8 = 2 + 2; // operand read + push
            add_cycles(state, base_cycles.saturating_sub(accounted));
            base_cycles
        }

        0xC0 => {
            // CPY immediate
            if state.p.contains(StatusFlags::INDEX_8BIT) {
                let value = read_u8_generic(state, bus);
                let result = (state.y as u8).wrapping_sub(value);
                state
                    .p
                    .set(StatusFlags::CARRY, (state.y & 0xFF) >= value as u16);
                state.p.set(StatusFlags::ZERO, result == 0);
                state.p.set(StatusFlags::NEGATIVE, (result & 0x80) != 0);
            } else {
                let value_lo = read_u8_generic(state, bus) as u16;
                let value_hi = read_u8_generic(state, bus) as u16;
                let value = value_lo | (value_hi << 8);
                let result = state.y.wrapping_sub(value);
                state.p.set(StatusFlags::CARRY, state.y >= value);
                state.p.set(StatusFlags::ZERO, result == 0);
                state.p.set(StatusFlags::NEGATIVE, (result & 0x8000) != 0);
            }
            add_cycles(state, 2);
            2
        }

        0xB8 => {
            // CLV (Clear Overflow)
            state.p.remove(StatusFlags::OVERFLOW);
            add_cycles(state, 2);
            2
        }
    }
}
