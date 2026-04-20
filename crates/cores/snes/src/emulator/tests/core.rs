use super::*;

#[test]
fn parse_hex_pc_range_inherits_bank_from_start_token() {
    assert_eq!(
        Emulator::parse_hex_pc_range("01:D13F-D148"),
        Some((0x01D13F, 0x01D148))
    );
    assert_eq!(
        Emulator::parse_hex_pc_range("01:D148-D13F"),
        Some((0x01D13F, 0x01D148))
    );
}
#[test]
fn parse_hex_pc_range_accepts_fully_qualified_tokens() {
    assert_eq!(
        Emulator::parse_hex_pc_range("01:D13F-01:D148"),
        Some((0x01D13F, 0x01D148))
    );
    assert_eq!(
        Emulator::parse_hex_pc_range("01D13F-01D148"),
        Some((0x01D13F, 0x01D148))
    );
}
#[test]
fn step_ppu_triggers_bus_vblank_start_only_when_ppu_enters_vblank() {
    let mut emulator = make_test_emulator_inner();
    emulator.bus.write_u8(0x4200, 0x01);

    {
        let ppu = emulator.bus.get_ppu_mut();
        ppu.scanline = 223;
        ppu.cycle = 340;
        ppu.v_blank = false;
        ppu.h_blank = true;
    }

    emulator.step_ppu(1, false);
    assert_eq!(emulator.bus.get_ppu().get_scanline(), 224);
    assert!(!emulator.bus.get_ppu().is_vblank());
    assert_eq!(emulator.bus.joy_busy_counter(), 0);

    emulator.step_ppu(341, false);
    assert_eq!(emulator.bus.get_ppu().get_scanline(), 225);
    assert!(emulator.bus.get_ppu().is_vblank());
    assert!(emulator.bus.joy_busy_counter() > 0);
}
#[test]
fn save_state_roundtrip_preserves_cpu_deferred_fetch() {
    let mut emulator = make_test_emulator_inner();
    emulator.cpu.set_state(crate::cpu::CpuState {
        a: 0x1234,
        x: 0x5678,
        y: 0x9ABC,
        sp: 0x02E8,
        dp: 0,
        db: 0,
        pb: 0x7E,
        pc: 0x3B36,
        p: 0x25,
        emulation_mode: false,
        cycles: 0x1122_3344,
        waiting_for_irq: false,
        stopped: false,
        deferred_fetch: Some(crate::cpu::core::DeferredFetchState {
            opcode: 0xAD,
            memspeed_penalty: 0,
            pc_before: 0x3B35,
            full_addr: 0x7E3B35,
        }),
    });

    let state = emulator.create_save_state();

    let mut restored = make_test_emulator();
    restored.load_save_state(state);

    let restored_state = restored.cpu.get_state();
    assert_eq!(restored_state.pb, 0x7E);
    assert_eq!(restored_state.pc, 0x3B36);
    assert_eq!(
        restored_state.deferred_fetch,
        Some(crate::cpu::core::DeferredFetchState {
            opcode: 0xAD,
            memspeed_penalty: 0,
            pc_before: 0x3B35,
            full_addr: 0x7E3B35,
        })
    );
}
#[test]
fn save_state_roundtrip_preserves_superfx_master_cycle_accum() {
    let mut emulator = make_test_emulator_inner();
    emulator.superfx_master_cycle_accum = 5;

    let state = emulator.create_save_state();

    let mut restored = make_test_emulator();
    restored.load_save_state(state);

    assert_eq!(restored.superfx_master_cycle_accum, 5);
}
#[test]
fn load_save_state_restores_superfx_trace_frame_counter() {
    let mut emulator = make_test_emulator_inner();
    emulator.frame_count = 123;
    crate::cartridge::superfx::set_trace_superfx_exec_frame(0);

    let state = emulator.create_save_state();

    let mut restored = make_test_emulator();
    restored.load_save_state(state);

    let trace_frame = crate::cartridge::superfx::debug_current_trace_superfx_exec_frame();
    assert_eq!(trace_frame, 123);
}
#[test]
fn step_ppu_applies_dram_refresh_mid_scanline() {
    let mut emulator = make_test_emulator();
    emulator.bus.get_ppu_mut().scanline = 32;
    emulator.bus.get_ppu_mut().cycle = 133;
    emulator.pending_stall_master_cycles = 0;

    emulator.step_ppu(2, true);

    assert_eq!(emulator.pending_stall_master_cycles, 40);
}
#[test]
fn step_ppu_does_not_add_dram_refresh_again_at_scanline_wrap() {
    let mut emulator = make_test_emulator();
    let last_dot = emulator.bus.get_ppu().dots_this_scanline(12) - 1;
    emulator.bus.get_ppu_mut().scanline = 12;
    emulator.bus.get_ppu_mut().cycle = last_dot;
    emulator.pending_stall_master_cycles = 0;

    emulator.step_ppu(1, true);

    assert_eq!(emulator.pending_stall_master_cycles, 0);
}
