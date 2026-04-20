use snes_emulator::bus::Bus;
use snes_emulator::cartridge::MapperType;
use snes_emulator::cpu::CpuState;

fn new_sa1_bus() -> Bus {
    Bus::new_with_mapper(vec![0; 0x20_0000], MapperType::Sa1, 0x2000)
}

#[test]
fn scpu_can_read_back_sa1_control_dma_and_brf_registers() {
    let mut bus = new_sa1_bus();

    bus.write_u8(0x002200, 0xA1);
    bus.write_u8(0x002201, 0x80);
    bus.write_u8(0x002203, 0x34);
    bus.write_u8(0x002204, 0x12);
    bus.write_u8(0x002220, 0x05);
    bus.write_u8(0x002221, 0x06);
    bus.write_u8(0x002222, 0x07);
    bus.write_u8(0x002223, 0x03);
    bus.write_u8(0x002230, 0x40);
    bus.write_u8(0x002231, 0x13);
    bus.write_u8(0x002232, 0x78);
    bus.write_u8(0x002233, 0x56);
    bus.write_u8(0x002234, 0x34);
    bus.write_u8(0x002235, 0xBC);
    bus.write_u8(0x002236, 0x9A);
    bus.write_u8(0x002237, 0x12);
    bus.write_u8(0x002238, 0xEF);
    bus.write_u8(0x002239, 0xCD);
    bus.write_u8(0x002240, 0x11);
    bus.write_u8(0x00224F, 0x99);

    assert_eq!(bus.read_u8(0x002200), 0xA1);
    assert_eq!(bus.read_u8(0x002201), 0x80);
    assert_eq!(bus.read_u8(0x002203), 0x34);
    assert_eq!(bus.read_u8(0x002204), 0x12);
    assert_eq!(bus.read_u8(0x002220), 0x05);
    assert_eq!(bus.read_u8(0x002221), 0x06);
    assert_eq!(bus.read_u8(0x002222), 0x07);
    assert_eq!(bus.read_u8(0x002223), 0x03);
    assert_eq!(bus.read_u8(0x002230), 0x40);
    assert_eq!(bus.read_u8(0x002231), 0x13);
    assert_eq!(bus.read_u8(0x002232), 0x78);
    assert_eq!(bus.read_u8(0x002233), 0x56);
    assert_eq!(bus.read_u8(0x002234), 0x34);
    assert_eq!(bus.read_u8(0x002235), 0xBC);
    assert_eq!(bus.read_u8(0x002236), 0x9A);
    assert_eq!(bus.read_u8(0x002237), 0x12);
    assert_eq!(bus.read_u8(0x002238), 0xEF);
    assert_eq!(bus.read_u8(0x002239), 0xCD);
    assert_eq!(bus.read_u8(0x002240), 0x11);
    assert_eq!(bus.read_u8(0x00224F), 0x99);
}

#[test]
fn scpu_can_read_sa1_status_hv_counter_and_version_registers() {
    let mut bus = new_sa1_bus();

    bus.write_u8(0x002201, 0xA0);
    {
        let sa1 = bus.sa1_mut();
        sa1.registers.control = 0x91;
        sa1.registers.scnt = 0x6A;
        sa1.registers.interrupt_pending = 0xA0;
        sa1.registers.timer_pending = 0x02;
    }

    bus.get_ppu_mut().step(341 * 2 + 17);

    let h = bus.get_ppu().get_cycle() & 0x01FF;
    let v = bus.get_ppu().get_scanline() & 0x01FF;

    assert_eq!(bus.read_u8(0x002300), 0xFA);
    assert_eq!(bus.read_u8(0x002301), 0xF1);
    assert_eq!(bus.read_u8(0x002302), (h & 0x00FF) as u8);
    assert_eq!(bus.read_u8(0x002303), ((h >> 8) & 0x01) as u8);
    assert_eq!(bus.read_u8(0x002304), (v & 0x00FF) as u8);
    assert_eq!(bus.read_u8(0x002305), ((v >> 8) & 0x01) as u8);
    assert_eq!(bus.read_u8(0x00230E), 0x23);
}

#[test]
fn sa1_cpu_arithmetic_registers_handle_mul_div_and_cumulative_sum() {
    let mut bus = new_sa1_bus();

    // Signed multiply: -2 * 3 = -6
    bus.sa1_write_u8(0x002250, 0x00);
    bus.sa1_write_u8(0x002251, 0xFE);
    bus.sa1_write_u8(0x002252, 0xFF);
    bus.sa1_write_u8(0x002253, 0x03);
    bus.sa1_write_u8(0x002254, 0x00);
    bus.sa1_mut().tick_timers(5);
    assert_eq!(bus.sa1_read_u8(0x002306), 0xFA);
    assert_eq!(bus.sa1_read_u8(0x002307), 0xFF);
    assert_eq!(bus.sa1_read_u8(0x002308), 0xFF);
    assert_eq!(bus.sa1_read_u8(0x002309), 0xFF);
    assert_eq!(bus.sa1_read_u8(0x00230B), 0x00);

    // Division: 7 / 3 = 2 remainder 1
    bus.sa1_write_u8(0x002250, 0x01);
    bus.sa1_write_u8(0x002251, 0x07);
    bus.sa1_write_u8(0x002252, 0x00);
    bus.sa1_write_u8(0x002253, 0x03);
    bus.sa1_write_u8(0x002254, 0x00);
    bus.sa1_mut().tick_timers(6);
    assert_eq!(bus.sa1_read_u8(0x002306), 0x02);
    assert_eq!(bus.sa1_read_u8(0x002307), 0x00);
    assert_eq!(bus.sa1_read_u8(0x002308), 0x01);
    assert_eq!(bus.sa1_read_u8(0x002309), 0x00);

    // Cumulative sum: clear, then 2*3 + (-1*4) = 2
    bus.sa1_write_u8(0x002250, 0x02);
    bus.sa1_write_u8(0x002251, 0x02);
    bus.sa1_write_u8(0x002252, 0x00);
    bus.sa1_write_u8(0x002253, 0x03);
    bus.sa1_write_u8(0x002254, 0x00);
    bus.sa1_mut().tick_timers(5);
    bus.sa1_write_u8(0x002251, 0xFF);
    bus.sa1_write_u8(0x002252, 0xFF);
    bus.sa1_write_u8(0x002253, 0x04);
    bus.sa1_write_u8(0x002254, 0x00);
    bus.sa1_mut().tick_timers(5);
    assert_eq!(bus.sa1_read_u8(0x002306), 0x02);
    assert_eq!(bus.sa1_read_u8(0x002307), 0x00);
    assert_eq!(bus.sa1_read_u8(0x002308), 0x00);
    assert_eq!(bus.sa1_read_u8(0x002309), 0x00);
    assert_eq!(bus.sa1_read_u8(0x00230A), 0x00);
    assert_eq!(bus.sa1_read_u8(0x00230B), 0x00);
}

#[test]
fn sa1_math_results_appear_after_operation_delay_and_div0_matches_hardware_shape() {
    let mut bus = new_sa1_bus();

    bus.sa1_write_u8(0x002250, 0x00);
    bus.sa1_write_u8(0x002251, 0x02);
    bus.sa1_write_u8(0x002252, 0x00);
    bus.sa1_write_u8(0x002253, 0x03);
    bus.sa1_write_u8(0x002254, 0x00);

    assert_eq!(bus.sa1_read_u8(0x002306), 0x00);
    bus.sa1_mut().tick_timers(4);
    assert_eq!(bus.sa1_read_u8(0x002306), 0x00);
    bus.sa1_mut().tick_timers(1);
    assert_eq!(bus.sa1_read_u8(0x002306), 0x06);

    bus.sa1_write_u8(0x002250, 0x01);
    bus.sa1_write_u8(0x002251, 0x07);
    bus.sa1_write_u8(0x002252, 0x00);
    bus.sa1_write_u8(0x002253, 0x00);
    bus.sa1_write_u8(0x002254, 0x00);

    bus.sa1_mut().tick_timers(5);
    assert_eq!(bus.sa1_read_u8(0x002306), 0x06);
    bus.sa1_mut().tick_timers(1);
    assert_eq!(bus.sa1_read_u8(0x002306), 0xFF);
    assert_eq!(bus.sa1_read_u8(0x002307), 0xFF);
    assert_eq!(bus.sa1_read_u8(0x002308), 0x07);
    assert_eq!(bus.sa1_read_u8(0x002309), 0x00);
}

#[test]
fn sa1_cpu_variable_length_port_supports_fixed_and_auto_increment_modes() {
    let mut bus = Bus::new_with_mapper(
        vec![0x80, 0x00, 0x00, 0x00, 0xA0, 0x00, 0x00, 0x00],
        MapperType::Sa1,
        0x2000,
    );

    // Fixed mode at ROM C0:0000. First bit is 1, second bit is 0.
    bus.sa1_write_u8(0x002259, 0x00);
    bus.sa1_write_u8(0x00225A, 0x00);
    bus.sa1_write_u8(0x00225B, 0xC0);
    assert_eq!(bus.sa1_read_u8(0x00230C), 0x01);
    assert_eq!(bus.sa1_read_u8(0x00230D), 0x00);
    bus.sa1_write_u8(0x002258, 0x01);
    assert_eq!(bus.sa1_read_u8(0x00230C), 0x00);
    assert_eq!(bus.sa1_read_u8(0x00230D), 0x00);

    // Auto-increment mode at ROM C0:0004 with 1-bit fields: 1,0,1...
    bus.sa1_write_u8(0x002259, 0x04);
    bus.sa1_write_u8(0x00225A, 0x00);
    bus.sa1_write_u8(0x00225B, 0xC0);
    bus.sa1_write_u8(0x002258, 0x81);
    assert_eq!(bus.sa1_read_u8(0x00230C), 0x05);
    assert_eq!(bus.sa1_read_u8(0x00230D), 0x00);
    assert_eq!(bus.sa1_read_u8(0x00230C), 0x02);
    assert_eq!(bus.sa1_read_u8(0x00230D), 0x00);
    assert_eq!(bus.sa1_read_u8(0x00230C), 0x01);
    assert_eq!(bus.sa1_read_u8(0x00230D), 0x00);
}

#[test]
fn bus_save_state_preserves_sa1_cpu_registers_and_work_ram() {
    let mut bus = new_sa1_bus();

    bus.sa1_mut().cpu.set_state(CpuState {
        a: 0x1234,
        x: 0x5678,
        y: 0x9ABC,
        sp: 0x01EF,
        dp: 0x0020,
        db: 0x7E,
        pb: 0xC0,
        pc: 0x3456,
        p: 0xA5,
        emulation_mode: false,
        cycles: 0x112233,
        waiting_for_irq: true,
        stopped: false,
        deferred_fetch: None,
    });
    {
        let sa1 = bus.sa1_mut();
        sa1.registers.control = 0x91;
        sa1.registers.math_result = 0x0000_1234_5678;
        sa1.registers.varlen_current_bits = 17;
        sa1.registers.interrupt_pending = 0xA0;
        sa1.registers.brf[3] = 0xCC;
    }
    bus.sa1_bwram_slice_mut()[0x0123] = 0x5A;
    bus.sa1_iram_slice_mut()[0x0456] = 0xA5;

    let state = bus.to_save_state();
    let mut restored = new_sa1_bus();
    restored.load_from_save_state(&state);

    let cpu = restored.sa1().cpu.get_state();
    assert_eq!(cpu.a, 0x1234);
    assert_eq!(cpu.x, 0x5678);
    assert_eq!(cpu.y, 0x9ABC);
    assert_eq!(cpu.pc, 0x3456);
    assert_eq!(cpu.pb, 0xC0);
    assert!(cpu.waiting_for_irq);

    assert_eq!(restored.sa1().registers.control, 0x91);
    assert_eq!(restored.sa1().registers.math_result, 0x0000_1234_5678);
    assert_eq!(restored.sa1().registers.varlen_current_bits, 17);
    assert_eq!(restored.sa1().registers.interrupt_pending, 0xA0);
    assert_eq!(restored.sa1().registers.brf[3], 0xCC);
    assert_eq!(restored.sa1_bwram_slice()[0x0123], 0x5A);
    assert_eq!(restored.sa1_iram_slice()[0x0456], 0xA5);
}

#[test]
fn sa1_hle_ipl_fallback_seeds_iram_from_rom_before_stub_jump() {
    let mut rom = vec![0xFF; 0x20_0000];
    rom[0x7FFC] = 0x34;
    rom[0x7FFD] = 0x12;
    rom[0x7FEA] = 0x78;
    rom[0x7FEB] = 0x56;
    rom[0x7FEE] = 0xBC;
    rom[0x7FEF] = 0x9A;
    for i in 0..0x800usize {
        rom[i] = (i & 0xFF) as u8;
    }

    let mut bus = Bus::new_with_mapper(rom, MapperType::Sa1, 0x2000);
    bus.run_sa1_scheduler(0);

    let iram = bus.sa1_iram_slice();
    let stub = bus.sa1().registers.reset_vector as usize;
    assert_eq!(iram[0], 0x00);
    assert_eq!(iram[1], 0x01);
    assert_eq!(iram[2], 0x02);
    assert_eq!(iram[3], 0x03);
    assert_eq!(iram[4], 0x04);
    assert_eq!(iram[5], 0x05);
    assert_eq!(iram[stub], 0x5C);
    assert_eq!(iram[stub + 1], 0x34);
    assert_eq!(iram[stub + 2], 0x12);
    assert_eq!(iram[stub + 3], 0x00);
    assert_eq!(bus.sa1().registers.reset_vector, 0x07FC);
    assert_eq!(bus.sa1().registers.nmi_vector, 0x5678);
    assert_eq!(bus.sa1().registers.irq_vector, 0x9ABC);
    assert_eq!(bus.sa1().registers.interrupt_pending & 0xA0, 0xA0);
}

#[test]
fn sa1_hle_ipl_stub_prefers_unused_ff_gap_over_low_iram() {
    let mut rom = vec![0xFF; 0x20_0000];
    rom[0x7FFC] = 0x34;
    rom[0x7FFD] = 0x12;
    rom[0x7FEA] = 0x78;
    rom[0x7FEB] = 0x56;
    rom[0x7FEE] = 0xBC;
    rom[0x7FEF] = 0x9A;
    rom[..0x800].fill(0x11);
    rom[0x0120..0x0124].fill(0xFF);

    let mut bus = Bus::new_with_mapper(rom, MapperType::Sa1, 0x2000);
    bus.run_sa1_scheduler(0);

    let iram = bus.sa1_iram_slice();
    let stub = bus.sa1().registers.reset_vector as usize;
    assert_eq!(stub, 0x0120);
    assert_eq!(iram[0], 0x11);
    assert_eq!(iram[1], 0x11);
    assert_eq!(iram[stub], 0x5C);
    assert_eq!(iram[stub + 1], 0x34);
    assert_eq!(iram[stub + 2], 0x12);
    assert_eq!(iram[stub + 3], 0x00);
}

#[test]
fn sa1_scheduler_defaults_to_three_to_one_scpu_ratio() {
    let mut bus = new_sa1_bus();

    // Seed vectors first; init_sa1_vectors_from_rom() rewrites CONTROL.
    bus.run_sa1_scheduler(0);

    // Keep the SA-1 CPU asleep so only scheduler accounting/timers advance.
    bus.sa1_mut().registers.control = 0x40;
    bus.reset_sa1_cycle_accum();

    bus.run_sa1_scheduler(5);

    assert_eq!(
        bus.take_sa1_cycle_accum(),
        15,
        "default SA-1 scheduler ratio should be 3 SA-1 cycles per S-CPU cycle"
    );
}

#[test]
fn sa1_type1_ccdma_waits_for_end_flag_before_completing_irq() {
    let mut bus = new_sa1_bus();

    for row in 0..8usize {
        let base = 0x0100 + row * 2;
        bus.sa1_iram_slice_mut()[base] = 0xE4;
        bus.sa1_iram_slice_mut()[base + 1] = 0xE4;
    }

    bus.write_u8(0x002230, 0xB2);
    bus.write_u8(0x002231, 0x02);
    bus.write_u8(0x002232, 0x00);
    bus.write_u8(0x002233, 0x01);
    bus.write_u8(0x002234, 0x00);
    bus.write_u8(0x002235, 0x00);
    bus.write_u8(0x002238, 0x10);
    bus.write_u8(0x002239, 0x00);
    bus.write_u8(0x002236, 0x02);
    bus.write_u8(0x002237, 0x00);

    assert!(bus.sa1().registers.ccdma_pending);

    bus.process_sa1_dma();

    assert_eq!(bus.sa1_iram_slice()[0x0200], 0x55);
    assert_eq!(bus.sa1_iram_slice()[0x0201], 0x33);
    assert_eq!(bus.sa1_iram_slice()[0x0202], 0x55);
    assert_eq!(bus.sa1_iram_slice()[0x0203], 0x33);
    assert!(!bus.sa1().registers.ccdma_pending);
    assert_eq!(bus.sa1().registers.handshake_state, 2);
    assert_eq!(bus.sa1().registers.interrupt_pending & 0x20, 0x00);

    bus.process_sa1_dma();
    assert_eq!(bus.sa1().registers.interrupt_pending & 0x20, 0x00);

    bus.write_u8(0x002231, 0x82);
    assert_eq!(bus.sa1().registers.interrupt_pending & 0x20, 0x20);
}
