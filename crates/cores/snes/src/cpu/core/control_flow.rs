use super::{
    addressing::read_absolute_address_generic,
    full_address,
    memory::{add_cycles, read_u16_generic, read_u8_generic},
    stack::{
        pop_u16_generic, pop_u8_generic, push_u16_emulation_edge, push_u16_generic, push_u8_generic,
    },
    CoreState,
};
use crate::{cpu::bus::CpuBus, cpu::StatusFlags};

#[inline]
pub(super) fn is_suspicious_exec_target(pb: u8, pc: u16) -> bool {
    !matches!(pb, 0x00 | 0x7E | 0x7F) && pc < 0x8000
}

pub(super) fn trace_suspicious_control_flow(
    tag: &str,
    from_pb: u8,
    from_pc: u16,
    opcode: u8,
    to_pb: u8,
    to_pc: u16,
    sp_before: u16,
    extra: impl AsRef<str>,
) {
    if !crate::debug_flags::trace_cpu_suspicious_flow() || !is_suspicious_exec_target(to_pb, to_pc)
    {
        return;
    }
    use std::sync::atomic::{AtomicU32, Ordering};
    static COUNT: AtomicU32 = AtomicU32::new(0);
    if COUNT.fetch_add(1, Ordering::Relaxed) < 64 {
        println!(
            "[CPU-SUSP-{}] {:02X}:{:04X} op={:02X} -> {:02X}:{:04X} SP={:04X} {}",
            tag,
            from_pb,
            from_pc,
            opcode,
            to_pb,
            to_pc,
            sp_before,
            extra.as_ref()
        );
    }
}

pub(super) fn branch_if_generic<T: CpuBus>(
    state: &mut CoreState,
    bus: &mut T,
    condition: bool,
) -> u8 {
    let offset = read_u8_generic(state, bus) as i8;
    let pc_before = state.pc;
    if condition {
        let new_pc = state.pc.wrapping_add(offset as u16);
        state.pc = new_pc;
        let mut total_cycles = 3u8;
        if (pc_before & 0xFF00) != (new_pc & 0xFF00) {
            total_cycles = total_cycles.saturating_add(1);
        }
        // read_u8_generic already accounted for one cycle
        add_cycles(state, total_cycles.saturating_sub(1));
        if crate::debug_flags::debug_branch()
            && state.pb == 0x00
            && (0x8240..=0x82A0).contains(&pc_before)
        {
            println!(
                "[BRANCH] pc_before={:04X} pc_after={:04X} offset={:02X} P=0x{:02X} taken=true",
                pc_before,
                new_pc,
                offset as u8,
                state.p.bits()
            );
            if crate::debug_flags::exit_on_branch_neg() && state.p.contains(StatusFlags::NEGATIVE) {
                println!("[EXIT_ON_BRANCH_NEG] triggered");
                std::process::exit(0);
            }
        }
        total_cycles
    } else {
        // Not taken branch is 2 cycles total
        add_cycles(state, 1); // one more cycle beyond operand fetch
        if crate::debug_flags::debug_branch()
            && state.pb == 0x00
            && (0x8240..=0x82A0).contains(&pc_before)
        {
            println!(
                "[BRANCH] pc_before={:04X} pc_after={:04X} offset={:02X} P=0x{:02X} taken=false",
                pc_before,
                state.pc,
                offset as u8,
                state.p.bits()
            );
        }
        2
    }
}

pub(super) fn brl_generic<T: CpuBus>(state: &mut CoreState, bus: &mut T) -> u8 {
    let offset = read_u16_generic(state, bus) as i16;
    let old_pc = state.pc;
    let new_pc = state.pc.wrapping_add(offset as u16);
    state.pc = new_pc;
    let mut total_cycles = 4u8;
    if (old_pc & 0xFF00) != (new_pc & 0xFF00) {
        total_cycles = total_cycles.saturating_add(1);
    }
    // read_u16_generic already accounted for 3 cycles (2 for read + 1 for add below)
    add_cycles(state, total_cycles.saturating_sub(2));
    total_cycles
}

pub(super) fn per_generic<T: CpuBus>(state: &mut CoreState, bus: &mut T) -> u8 {
    let offset = read_u16_generic(state, bus) as i16;
    let value = state.pc.wrapping_add(offset as u16);
    if state.emulation_mode {
        push_u16_emulation_edge(state, bus, value);
    } else {
        push_u16_generic(state, bus, value);
    }
    let total_cycles: u8 = 6;
    // read_u16_generic accounted for 2 cycles, push_u16 added 2 cycles
    add_cycles(state, total_cycles.saturating_sub(4));
    total_cycles
}

pub(super) fn jsr_generic<T: CpuBus>(state: &mut CoreState, bus: &mut T) -> u8 {
    let addr = read_absolute_address_generic(state, bus);
    if crate::debug_flags::trace_jsr_stack() {
        println!(
            "[JSR] PB={:02X} PC={:04X} SP={:04X} push_ret={:04X}",
            state.pb,
            state.pc.wrapping_sub(2),
            state.sp,
            state.pc
        );
    }
    if crate::debug_flags::trace_jsl() || crate::debug_flags::trace_pb_calls() {
        println!(
            "PB_CALL JSR from {:02X}:{:04X} PB={:02X} DB={:02X} SP={:04X} target={:04X}",
            state.pb,
            state.pc.wrapping_sub(2),
            state.pb,
            state.db,
            state.sp,
            addr
        );
    }
    push_u16_generic(state, bus, state.pc.wrapping_sub(1));
    state.pc = (addr & 0xFFFF) as u16;
    6
}

pub(super) fn rts_generic<T: CpuBus>(state: &mut CoreState, bus: &mut T) -> u8 {
    if crate::debug_flags::trace_rts_detail() {
        let base = if state.emulation_mode {
            0x0100u32 | state.sp as u32
        } else {
            state.sp as u32
        };
        let lo = bus.read_u8(base.wrapping_add(1));
        let hi = bus.read_u8(base.wrapping_add(2));
        println!(
            "[RTS-PEEK] PB={:02X} PC={:04X} SP={:04X} peek={:04X} (bytes={:02X} {:02X}) emu={}",
            state.pb,
            state.pc.wrapping_sub(1),
            state.sp,
            ((hi as u16) << 8) | lo as u16,
            lo,
            hi,
            state.emulation_mode
        );
    }
    if crate::debug_flags::trace_rts_pop() {
        // Peek return address before popping
        let sp = state.sp;
        let base = if state.emulation_mode {
            0x0100u32 | sp as u32
        } else {
            sp as u32
        };
        let pcl = bus.read_u8(base.wrapping_add(1));
        let pch = bus.read_u8(base.wrapping_add(2));
        // Dump top 8 bytes of stack for corruption trace
        let mut bytes = [0u8; 8];
        for (i, byte) in bytes.iter_mut().enumerate() {
            *byte = bus.read_u8(base.wrapping_add(i as u32 + 1));
        }
        println!(
            "[RTS] PB={:02X} PC={:04X} SP={:04X} -> ret {:02X}:{:02X}{:02X} stack={:?}",
            state.pb,
            state.pc.wrapping_sub(1),
            state.sp,
            state.pb,
            pch,
            pcl,
            bytes
        );
    }
    // COMPAT: 特定の RTS を RTL 相当で扱う（Mario 初期化の誤帰還防止）
    // 有効化: COMPAT_RTS_AS_RTL_8D7F=1 または COMPAT_MARIO_RTS_AS_RTL=1
    let cur_op_addr = state.pc.wrapping_sub(1); // 実行中オペコードのアドレス
                                                // SMW/Super Mario Collection 専用: 8D7FのRTSをRTL扱い＋スタック誤帰還を補正
    let compat_rts_as_rtl_8d7f = crate::debug_flags::compat_rts_as_rtl_8d7f()
        || crate::debug_flags::compat_mario_rts_as_rtl();
    if compat_rts_as_rtl_8d7f && state.pb == 0x00 && cur_op_addr == 0x8D7F {
        // RTL: pop 16-bit PC then bank
        let mut addr = pop_u16_generic(state, bus);
        let mut bank = pop_u8_generic(state, bus);

        // Mario specific stack corruption guard:
        // もし誤って 00:805F が積まれていた場合は、本来戻るべき 8CBA へ強制修正する。
        let compat_fix = crate::debug_flags::compat_mario_rts_fix();
        if compat_fix && addr == 0x805F && bank == 0x00 {
            addr = 0x8CBA;
            bank = 0x00;
        }

        if crate::debug_flags::trace_rts_addr() {
            println!(
                "[RTS->RTL] PB={:02X} popped={:02X}:{:04X} -> next={:02X}:{:04X} SP={:04X}",
                state.pb,
                bank,
                addr,
                bank,
                addr.wrapping_add(1),
                state.sp
            );
        }
        state.pb = bank;
        state.pc = addr.wrapping_add(1);
        6
    } else {
        let addr = pop_u16_generic(state, bus);
        if crate::debug_flags::trace_rts_addr() {
            println!(
                "[RTS-POP] PB={:02X} popped={:04X} -> next={:04X} SP={:04X}",
                state.pb,
                addr,
                addr.wrapping_add(1),
                state.sp
            );
        }
        state.pc = addr.wrapping_add(1);
        6
    }
}

pub(super) fn jsl_generic<T: CpuBus>(state: &mut CoreState, bus: &mut T) -> u8 {
    let addr_lo = bus.read_u8(full_address(state, state.pc)) as u32;
    let addr_hi = bus.read_u8(full_address(state, state.pc + 1)) as u32;
    let addr_bank = bus.read_u8(full_address(state, state.pc + 2)) as u32;
    let target = addr_lo | (addr_hi << 8) | (addr_bank << 16);
    state.pc = state.pc.wrapping_add(3);

    if crate::debug_flags::trace_pb_calls() || crate::debug_flags::trace_jsl() {
        let op0 = addr_lo as u8;
        let op1 = addr_hi as u8;
        let op2 = addr_bank as u8;
        println!(
            "PB_CALL JSL from {:02X}:{:04X} PB={:02X} DB={:02X} SP={:04X} target={:06X} op=[{:02X} {:02X} {:02X}]",
            state.pb,
            state.pc.wrapping_sub(3),
            state.pb,
            state.db,
            state.sp,
            target,
            op0,
            op1,
            op2
        );
    }

    let ret = state.pc.wrapping_sub(1);
    if state.emulation_mode {
        // Undocumented emulation edge: JSL pushes 3 bytes using a 16-bit stack decrement.
        // This can write outside $0100-$01FF when SP starts at $0100.
        bus.write_u8(state.sp as u32, state.pb);
        state.sp = state.sp.wrapping_sub(1);
        add_cycles(state, 1);
        bus.write_u8(state.sp as u32, (ret >> 8) as u8);
        state.sp = state.sp.wrapping_sub(1);
        add_cycles(state, 1);
        bus.write_u8(state.sp as u32, (ret & 0xFF) as u8);
        state.sp = state.sp.wrapping_sub(1);
        add_cycles(state, 1);
        // Re-assert emulation-mode stack high byte after the sequence.
        state.sp = 0x0100 | (state.sp & 0x00FF);
    } else {
        push_u8_generic(state, bus, state.pb);
        push_u16_generic(state, bus, ret);
    }

    state.pb = (target >> 16) as u8;
    state.pc = (target & 0xFFFF) as u16;
    8
}

pub(super) fn rtl_generic<T: CpuBus>(state: &mut CoreState, bus: &mut T) -> u8 {
    let from_pb = state.pb;
    let from_pc = state.pc.wrapping_sub(1);
    let sp_before = state.sp;
    if crate::debug_flags::trace_pb_calls() || crate::debug_flags::trace_rtl() {
        // peek stack before pop
        let sp = state.sp;
        let sp_addr = if state.emulation_mode {
            0x0100u32 | (sp as u32)
        } else {
            sp as u32
        };
        let pcl = bus.read_u8(sp_addr.wrapping_add(1));
        let pch = bus.read_u8(sp_addr.wrapping_add(2));
        let pb = bus.read_u8(sp_addr.wrapping_add(3));
        println!(
            "PB_CALL RTL pull {:02X}:{:04X} SP={:04X} ret={:02X}:{:02X}{:02X}",
            state.pb, state.pc, state.sp, pb, pch, pcl
        );
    }

    let (addr, pb) = if state.emulation_mode {
        // Undocumented emulation edge: RTL pulls 3 bytes using a 16-bit stack increment.
        // This can read from $0200.. when SP starts at $01FF.
        state.sp = state.sp.wrapping_add(1);
        let lo = bus.read_u8(state.sp as u32) as u16;
        add_cycles(state, 1);
        state.sp = state.sp.wrapping_add(1);
        let hi = bus.read_u8(state.sp as u32) as u16;
        add_cycles(state, 1);
        state.sp = state.sp.wrapping_add(1);
        let pb = bus.read_u8(state.sp as u32);
        add_cycles(state, 1);
        // Re-assert emulation-mode stack high byte after the sequence.
        state.sp = 0x0100 | (state.sp & 0x00FF);
        ((hi << 8) | lo, pb)
    } else {
        (pop_u16_generic(state, bus), pop_u8_generic(state, bus))
    };
    state.pb = pb;
    state.pc = addr.wrapping_add(1);
    trace_suspicious_control_flow(
        "RTL",
        from_pb,
        from_pc,
        0x6B,
        state.pb,
        state.pc,
        sp_before,
        format!("popped={:02X}:{:04X}", pb, addr),
    );
    6
}
