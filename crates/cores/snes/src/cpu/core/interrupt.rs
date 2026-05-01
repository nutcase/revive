use super::{add_cycles, stack::push_u8_generic, CoreState};
use crate::{cpu::bus::CpuBus, cpu::StatusFlags};

pub fn service_nmi<T: CpuBus>(state: &mut CoreState, bus: &mut T) -> u8 {
    let before = state.cycles;
    if crate::debug_flags::trace_nmi_take() {
        println!(
            "[TRACE_NMI_TAKE] at {:02X}:{:04X} A={:04X} X={:04X} Y={:04X} SP={:04X} D={:04X} DB={:02X} P={:02X}",
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
    }
    if state.emulation_mode {
        // Emulation mode: PCH, PCL, then status (bit5 forced 1, B cleared)
        push_u8_generic(state, bus, (state.pc >> 8) as u8);
        push_u8_generic(state, bus, (state.pc & 0xFF) as u8);
        push_u8_generic(state, bus, (state.p.bits() | 0x20) & !0x10);
        let vector = bus.read_u16(0x00FFFA);
        state.pc = vector;
        state.pb = 0;
    } else {
        // Native mode: PB, PCH, PCL, then status (bits 4/5 are X/M)
        push_u8_generic(state, bus, state.pb);
        push_u8_generic(state, bus, (state.pc >> 8) as u8);
        push_u8_generic(state, bus, (state.pc & 0xFF) as u8);
        push_u8_generic(state, bus, state.p.bits());
        let vector = bus.read_u16(0x00FFEA);
        state.pc = vector;
        state.pb = 0;
    }

    state.p.insert(StatusFlags::IRQ_DISABLE);
    state.p.remove(StatusFlags::DECIMAL);
    state.waiting_for_irq = false;
    bus.acknowledge_nmi();

    let consumed = state.cycles.wrapping_sub(before) as u8;
    let target = if state.emulation_mode { 7u8 } else { 8u8 };
    if consumed < target {
        add_cycles(state, target - consumed);
        target
    } else {
        consumed
    }
}

pub fn service_irq<T: CpuBus>(state: &mut CoreState, bus: &mut T) -> u8 {
    let before = state.cycles;
    if state.emulation_mode {
        push_u8_generic(state, bus, (state.pc >> 8) as u8);
        push_u8_generic(state, bus, (state.pc & 0xFF) as u8);
        push_u8_generic(state, bus, (state.p.bits() | 0x20) & !0x10);
        let vector = bus.read_u16(0x00FFFE);
        state.pc = vector;
        state.pb = 0;
    } else {
        push_u8_generic(state, bus, state.pb);
        push_u8_generic(state, bus, (state.pc >> 8) as u8);
        push_u8_generic(state, bus, (state.pc & 0xFF) as u8);
        push_u8_generic(state, bus, state.p.bits());
        let vector = bus.read_u16(0x00FFEE);
        state.pc = vector;
        state.pb = 0;
    }

    state.p.insert(StatusFlags::IRQ_DISABLE);
    state.p.remove(StatusFlags::DECIMAL);
    state.waiting_for_irq = false;

    if crate::debug_flags::trace_irq() {
        use std::sync::atomic::{AtomicU32, Ordering};
        static PRINT_COUNT: AtomicU32 = AtomicU32::new(0);
        if PRINT_COUNT.fetch_add(1, Ordering::Relaxed) < 16 {
            println!(
                "IRQ serviced → next PC {:02X}:{:04X} (emulation={})",
                state.pb, state.pc, state.emulation_mode
            );
        }
    }

    let consumed = state.cycles.wrapping_sub(before) as u8;
    let target = if state.emulation_mode { 7u8 } else { 8u8 };
    if consumed < target {
        add_cycles(state, target - consumed);
        target
    } else {
        consumed
    }
}
