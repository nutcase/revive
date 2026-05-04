use super::*;
use crate::cpu::bus::CpuBus;

fn configure_dma_dest(bus: &mut Bus, channel: usize, dest: u8) {
    let ch = &mut bus.dma_controller.channels[channel];
    ch.dest_address = dest;
    ch.configured = true;
}

#[test]
fn strict_mdma_allows_cgram_during_active_hblank() {
    let mut bus = Bus::new(vec![]);
    configure_dma_dest(&mut bus, 0, 0x22);
    bus.ppu.screen_display = 0x00;
    bus.ppu.v_blank = false;
    bus.ppu.h_blank = true;
    bus.ppu.scanline = 10;
    bus.ppu.cycle = 300;

    let (now_mask, defer_mask) = bus.partition_mdma_mask_for_current_window(0x01, true);
    assert_eq!(now_mask, 0x01);
    assert_eq!(defer_mask, 0x00);
}

#[test]
fn strict_mdma_defers_oam_outside_vblank_even_in_hblank() {
    let mut bus = Bus::new(vec![]);
    configure_dma_dest(&mut bus, 0, 0x04);
    bus.ppu.screen_display = 0x00;
    bus.ppu.v_blank = false;
    bus.ppu.h_blank = true;
    bus.ppu.scanline = 10;
    bus.ppu.cycle = 300;

    let (now_mask, defer_mask) = bus.partition_mdma_mask_for_current_window(0x01, true);
    assert_eq!(now_mask, 0x00);
    assert_eq!(defer_mask, 0x01);
}

#[test]
fn strict_mdma_allows_oam_during_forced_blank() {
    let mut bus = Bus::new(vec![]);
    configure_dma_dest(&mut bus, 0, 0x04);
    bus.ppu.screen_display = 0x80;
    bus.ppu.v_blank = false;
    bus.ppu.h_blank = false;
    bus.ppu.scanline = 42;
    bus.ppu.cycle = 12;

    let (now_mask, defer_mask) = bus.partition_mdma_mask_for_current_window(0x01, true);
    assert_eq!(now_mask, 0x01);
    assert_eq!(defer_mask, 0x00);
}

#[test]
fn cpu_exec_trace_ring_keeps_latest_entries() {
    let mut bus = Bus::new(vec![]);
    for i in 0..(CPU_EXEC_TRACE_RING_LEN as u32 + 3) {
        bus.set_last_cpu_exec_pc(0x008000 + i);
    }

    assert_eq!(
        bus.debug_recent_cpu_exec_pcs().len(),
        CPU_EXEC_TRACE_RING_LEN
    );
    assert_eq!(bus.debug_recent_cpu_exec_pcs()[0], 0x008003);
    assert_eq!(
        *bus.debug_recent_cpu_exec_pcs().last().unwrap(),
        0x008000 + CPU_EXEC_TRACE_RING_LEN as u32 + 2
    );
}

#[test]
fn bus_superfx_r15_high_write_does_not_mutate_starfox_working_regs_immediately() {
    let rom = vec![0u8; 0x20_0000];
    let mut bus = Bus::new_with_mapper(rom, crate::cartridge::MapperType::SuperFx, 0x2000);
    let gsu = bus.superfx.as_mut().unwrap();
    gsu.debug_set_pbr(0x01);
    gsu.debug_set_rombr(0x14);
    gsu.debug_set_scmr(0x39);
    gsu.debug_set_reg(9, 0x2800);
    gsu.debug_set_reg(13, 0xB3DE);
    gsu.debug_set_reg(14, 0x6242);
    gsu.debug_set_reg(15, 0xB3E6);

    bus.write_u8(0x00_301E, 0x01);
    let gsu = bus.superfx.as_ref().unwrap();
    assert_eq!(gsu.debug_reg(15), 0xB301);
    assert_eq!(gsu.debug_reg(9), 0x2800);
    assert_eq!(gsu.debug_reg(13), 0xB3DE);
    assert_eq!(gsu.debug_reg(14), 0x6242);
    assert!(!gsu.running());

    bus.write_u8(0x00_301F, 0xB3);
    let gsu = bus.superfx.as_ref().unwrap();
    assert!(gsu.running());
    assert_eq!(gsu.debug_reg(15), 0xB301);
    assert_eq!(gsu.debug_reg(9), 0x2800);
    assert_eq!(gsu.debug_reg(13), 0xB3DE);
    assert_eq!(gsu.debug_reg(14), 0x6242);
}

#[test]
fn hdmaen_rising_edge_enables_configured_channel_without_reinitialising_table() {
    let mut bus = Bus::new(vec![]);
    bus.ppu.scanline = 42;
    bus.ppu.cycle = 120;
    let ch = &mut bus.dma_controller.channels[1];
    ch.configured = true;
    ch.control = 0x40;
    ch.hdma_enabled = false;
    ch.hdma_terminated = false;
    ch.hdma_indirect = false;
    ch.hdma_table_addr = 0x12_3456;
    ch.hdma_line_counter = 0x23;
    ch.hdma_do_transfer = false;
    bus.dma_controller.hdma_enable = 0x00;

    bus.write_u8(0x420C, 0x02);

    let ch = &bus.dma_controller.channels[1];
    assert_eq!(bus.dma_controller.hdma_enable, 0x02);
    assert!(ch.hdma_enabled);
    assert!(!ch.hdma_terminated);
    assert!(ch.hdma_indirect);
    assert_eq!(ch.hdma_table_addr, 0x12_3456);
    assert_eq!(ch.hdma_line_counter, 0x23);
    assert!(!ch.hdma_do_transfer);
}

#[test]
fn hdmaen_rising_edge_before_first_hblank_initialises_frame_channel() {
    let mut bus = Bus::new(vec![]);
    bus.ppu.scanline = 0;
    bus.ppu.cycle = 225;
    bus.ppu.h_blank = false;

    let ch = &mut bus.dma_controller.channels[1];
    ch.configured = true;
    ch.control = 0x40;
    ch.src_address = 0x12_3456;
    ch.hdma_enabled = false;
    ch.hdma_terminated = false;
    ch.hdma_indirect = false;
    ch.hdma_indirect_addr = 0x7E_9999;
    ch.hdma_table_addr = 0x7E_2222;
    ch.hdma_line_counter = 0x23;
    ch.hdma_repeat_flag = true;
    ch.hdma_do_transfer = true;
    ch.a2a = 0x2222;
    ch.nltr = 0xA3;
    bus.dma_controller.hdma_enable = 0x00;

    bus.write_u8(0x420C, 0x02);

    let ch = &bus.dma_controller.channels[1];
    assert_eq!(bus.dma_controller.hdma_enable, 0x02);
    assert!(ch.hdma_enabled);
    assert!(!ch.hdma_terminated);
    assert!(ch.hdma_indirect);
    assert_eq!(ch.hdma_indirect_addr, 0);
    assert_eq!(ch.hdma_table_addr, 0x12_3456);
    assert_eq!(ch.hdma_line_counter, 0);
    assert!(!ch.hdma_repeat_flag);
    assert!(!ch.hdma_do_transfer);
    assert_eq!(ch.a2a, 0x3456);
    assert_eq!(ch.nltr, 0x80);
}

#[test]
fn hdmaen_rising_edge_does_not_restart_channel_terminated_this_frame() {
    let mut bus = Bus::new(vec![]);
    bus.ppu.scanline = 42;
    bus.ppu.cycle = 120;
    let ch = &mut bus.dma_controller.channels[1];
    ch.configured = true;
    ch.hdma_enabled = false;
    ch.hdma_terminated = true;
    bus.dma_controller.hdma_enable = 0x00;

    bus.write_u8(0x420C, 0x02);

    let ch = &bus.dma_controller.channels[1];
    assert!(!ch.hdma_enabled);
    assert!(ch.hdma_terminated);
}

#[test]
fn hdma_table_line_80_is_nonrepeat_128_lines() {
    let mut bus = Bus::new(vec![]);
    bus.wram[0] = 0x80;
    let ch = &mut bus.dma_controller.channels[0];
    ch.configured = true;
    ch.control = 0x00;
    ch.hdma_table_addr = 0x7E_0000;

    assert!(bus.load_hdma_entry(0));

    let ch = &bus.dma_controller.channels[0];
    assert_eq!(ch.hdma_line_counter, 128);
    assert!(!ch.hdma_repeat_flag);
    assert!(ch.hdma_do_transfer);
    assert_eq!(ch.hdma_table_addr, 0x7E_0001);
}
