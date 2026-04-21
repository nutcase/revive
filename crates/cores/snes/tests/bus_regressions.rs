use snes_emulator::bus::Bus;
use snes_emulator::input::button;

#[test]
fn bus_joyser1_serial_order_matches_standard_pad_bits() {
    let mut bus = Bus::new(vec![]);
    {
        let input = bus.get_input_system_mut();
        input.controller1.set_buttons(button::B | button::START);
    }

    // Latch: 4016 1->0
    bus.write_u8(0x4016, 1);
    bus.write_u8(0x4016, 0);

    // Serial order: B, Y, Select, Start ...
    let b = bus.read_u8(0x4016) & 1;
    let y = bus.read_u8(0x4016) & 1;
    let select = bus.read_u8(0x4016) & 1;
    let start = bus.read_u8(0x4016) & 1;

    assert_eq!(b, 1);
    assert_eq!(y, 0);
    assert_eq!(select, 0);
    assert_eq!(start, 1);
}

#[test]
fn bus_rdnmi_bit7_reasserts_on_next_vblank_after_clear() {
    let mut bus = Bus::new(vec![]);

    // Power-on often has bit7=1; after read it should clear.
    let _ = bus.read_u8(0x4210);
    let cleared = bus.read_u8(0x4210);
    assert_eq!(cleared & 0x80, 0);

    // Advance PPU into vblank.
    for _ in 0..300 {
        bus.get_ppu_mut().step(341);
        if bus.get_ppu().is_vblank() {
            break;
        }
    }
    assert!(bus.get_ppu().is_vblank(), "failed to reach vblank in test");

    let v = bus.read_u8(0x4210);
    assert_ne!(v & 0x80, 0, "RDNMI bit7 must be set again on vblank");
}

#[test]
fn bus_rdnmi_bit7_remains_visible_during_current_vblank_after_read() {
    let mut bus = Bus::new(vec![]);

    let _ = bus.read_u8(0x4210);
    while !bus.get_ppu().is_vblank() {
        bus.get_ppu_mut().step(341);
    }

    let first = bus.read_u8(0x4210);
    let second = bus.read_u8(0x4210);
    assert_ne!(first & 0x80, 0);
    assert_ne!(
        second & 0x80,
        0,
        "RDNMI bit7 should remain visible while VBlank is active"
    );

    while bus.get_ppu().is_vblank() {
        bus.get_ppu_mut().step(341);
    }
    let after_vblank = bus.read_u8(0x4210);
    assert_eq!(
        after_vblank & 0x80,
        0,
        "a read during VBlank should still acknowledge the edge after VBlank ends"
    );
}

#[test]
fn bus_autojoy_joy1l_joy1h_layout_matches_hardware_bytes() {
    let mut bus = Bus::new(vec![]);
    {
        let input = bus.get_input_system_mut();
        input
            .controller1
            .set_buttons(button::B | button::RIGHT | button::A | button::L);
    }

    // Enable auto-joypad and emulate vblank start where latching occurs.
    bus.write_u8(0x4200, 0x01);
    bus.on_vblank_start();

    let joy1l = bus.read_u8(0x4218);
    let joy1h = bus.read_u8(0x4219);

    // JOY1L (low byte): A,X,L,R,0,0,0,0 => A(bit7) + L(bit5)
    assert_eq!(joy1l, 0xA0);
    // JOY1H (high byte): B,Y,Sel,Sta,Up,Dn,Lt,Rt => B(bit7) + Right(bit0)
    assert_eq!(joy1h, 0x81);
}

#[test]
fn bus_hirq_skips_missing_dot_on_short_ntsc_scanline() {
    let mut bus = Bus::new(vec![]);

    while !bus.get_ppu().is_vblank() {
        bus.get_ppu_mut().step(341);
    }
    while bus.get_ppu().get_scanline() < 240 {
        bus.get_ppu_mut().step(341);
    }
    assert_eq!(bus.get_ppu().get_scanline(), 240);

    bus.write_u8(0x4207, 0x50); // HTIME = 0x150 -> coarse +4 => dot 340
    bus.write_u8(0x4208, 0x01);
    bus.write_u8(0x4200, 0x10); // H-IRQ only

    bus.tick_timers_hv(339, 0, 240);

    let timeup = bus.read_u8(0x4211);
    assert_eq!(
        timeup & 0x80,
        0,
        "short scanline must not generate an H-IRQ on missing dot 340"
    );
}

#[test]
fn bus_hirq_enable_midscanline_at_match_sets_timeup_immediately() {
    let mut bus = Bus::new(vec![]);

    bus.get_ppu_mut().step(8);
    assert_eq!(bus.get_ppu().get_scanline(), 0);
    assert_eq!(bus.get_ppu().get_cycle(), 8);

    bus.write_u8(0x4207, 0x04);
    bus.write_u8(0x4208, 0x00);
    bus.write_u8(0x4200, 0x10);

    let timeup = bus.read_u8(0x4211);
    assert_eq!(
        timeup & 0x80,
        0x80,
        "enabling H-IRQ at the current match point must assert TIMEUP immediately"
    );
}

#[test]
fn bus_virq_enable_on_matching_line_after_line_start_waits_for_next_line_match() {
    let mut bus = Bus::new(vec![]);

    bus.get_ppu_mut().step(341 + 8);
    assert_eq!(bus.get_ppu().get_scanline(), 1);
    assert_eq!(bus.get_ppu().get_cycle(), 8);

    bus.write_u8(0x4209, 0x01);
    bus.write_u8(0x420A, 0x00);
    bus.write_u8(0x4200, 0x20);

    let timeup = bus.read_u8(0x4211);
    assert_eq!(
        timeup & 0x80,
        0,
        "enabling V-IRQ after the matching line has already started must not retrigger midline"
    );
}

#[test]
fn bus_virq_reenable_after_timeup_clear_does_not_retrigger_same_scanline() {
    let mut bus = Bus::new(vec![]);

    bus.write_u8(0x4209, 0x01);
    bus.write_u8(0x420A, 0x00);
    bus.write_u8(0x4200, 0x20);

    bus.get_ppu_mut().step(341);
    bus.tick_timers();
    assert_eq!(bus.get_ppu().get_scanline(), 1);

    assert_eq!(bus.read_u8(0x4211) & 0x80, 0x80);

    bus.get_ppu_mut().step(8);
    assert_eq!(bus.get_ppu().get_scanline(), 1);
    bus.write_u8(0x4200, 0x20);

    assert_eq!(
        bus.read_u8(0x4211) & 0x80,
        0,
        "restoring NMITIMEN after clearing TIMEUP must not schedule a duplicate V-IRQ"
    );
}

#[test]
fn bus_hv_irq_reprogramming_htime_after_current_dot_does_not_retrigger_old_vline() {
    let mut bus = Bus::new(vec![]);

    bus.write_u8(0x4207, 0xA0);
    bus.write_u8(0x4208, 0x00);
    bus.write_u8(0x4209, 0x16);
    bus.write_u8(0x420A, 0x00);
    bus.write_u8(0x4200, 0x30);

    bus.get_ppu_mut().step(22 * 341 + 285);
    assert_eq!(bus.get_ppu().get_scanline(), 22);
    assert_eq!(bus.get_ppu().get_cycle(), 285);
    let _ = bus.read_u8(0x4211);

    bus.write_u8(0x4207, 0xA8);

    let timeup = bus.read_u8(0x4211);
    assert_eq!(
        timeup & 0x80,
        0,
        "moving HTIME to a dot that already passed must not retrigger the old matching V line"
    );
}

#[test]
fn bus_unmapped_io_reads_preserve_open_bus_value() {
    let mut bus = Bus::new(vec![]);

    bus.write_u8(0x000000, 0x5A);
    assert_eq!(bus.read_u8(0x000000), 0x5A);

    assert_eq!(bus.read_u8(0x00420E), 0x5A);
    assert_eq!(bus.read_u8(0x002181), 0x5A);
    assert_eq!(bus.read_u8(0x002184), 0x5A);
    assert_eq!(bus.read_u8(0x002200), 0x5A);
    assert_eq!(bus.read_u8(0x00440A), 0x5A);
}

#[test]
fn bus_invalid_oam_read_returns_open_bus_without_advancing_address() {
    let mut bus = Bus::new(vec![]);

    // Program OAM data while forced blank is active.
    bus.write_u8(0x002102, 0x00);
    bus.write_u8(0x002103, 0x00);
    bus.write_u8(0x002104, 0x12);
    bus.write_u8(0x002104, 0x34);
    bus.write_u8(0x002102, 0x00);
    bus.write_u8(0x002103, 0x00);

    // Prime MDR/open-bus value.
    bus.write_u8(0x000000, 0x5A);
    assert_eq!(bus.read_u8(0x000000), 0x5A);

    // Enter active display.
    bus.write_u8(0x002100, 0x00);
    bus.get_ppu_mut().step(32);

    assert_eq!(bus.read_u8(0x002138), 0x5A);

    // Re-enter forced blank; address should still point at the first byte.
    bus.write_u8(0x002100, 0x80);
    assert_eq!(bus.read_u8(0x002138), 0x12);
    assert_eq!(bus.read_u8(0x002138), 0x34);
}

#[test]
fn bus_invalid_cgram_read_returns_open_bus_without_advancing_address() {
    let mut bus = Bus::new(vec![]);

    bus.write_u8(0x002121, 0x00);
    bus.write_u8(0x002122, 0x34);
    bus.write_u8(0x002122, 0x12);
    bus.write_u8(0x002121, 0x00);

    bus.write_u8(0x000000, 0xA5);
    assert_eq!(bus.read_u8(0x000000), 0xA5);

    bus.write_u8(0x002100, 0x00);
    bus.get_ppu_mut().step(32);

    assert_eq!(bus.read_u8(0x00213B), 0xA5);

    bus.write_u8(0x002100, 0x80);
    assert_eq!(bus.read_u8(0x00213B), 0x34);
    assert_eq!(bus.read_u8(0x00213B), 0x12);
}

#[test]
fn bus_slhv_read_returns_open_bus_and_still_latches_counters() {
    let mut bus = Bus::new(vec![]);

    bus.write_u8(0x000000, 0x5A);
    assert_eq!(bus.read_u8(0x000000), 0x5A);

    assert_eq!(bus.read_u8(0x002137), 0x5A);

    bus.get_ppu_mut().step(1);

    assert_eq!(bus.read_u8(0x00213C), 0x01);
    assert_eq!(bus.read_u8(0x00213D), 0x00);
}

#[test]
fn bus_invalid_vram_read_returns_open_bus_without_advancing_address() {
    let mut bus = Bus::new(vec![]);

    bus.write_u8(0x002100, 0x80);
    bus.write_u8(0x002115, 0x80);
    bus.write_u8(0x002116, 0x00);
    bus.write_u8(0x002117, 0x00);
    bus.write_u8(0x002118, 0x34);
    bus.write_u8(0x002119, 0x12);
    bus.write_u8(0x002116, 0x00);
    bus.write_u8(0x002117, 0x00);

    bus.write_u8(0x000000, 0xA5);
    assert_eq!(bus.read_u8(0x000000), 0xA5);

    bus.write_u8(0x002100, 0x00);
    bus.get_ppu_mut().step(32);

    assert_eq!(bus.read_u8(0x002139), 0xA5);
    assert_eq!(bus.read_u8(0x00213A), 0xA5);

    bus.write_u8(0x002100, 0x80);
    assert_eq!(bus.read_u8(0x002139), 0x34);
    assert_eq!(bus.read_u8(0x00213A), 0x12);
}

#[test]
fn bus_hdmaen_midframe_reenable_preserves_current_state_until_next_frame_start() {
    let mut bus = Bus::new(vec![]);

    // Configure channel 0 as a valid HDMA channel.
    bus.write_u8(0x4300, 0x00);
    bus.write_u8(0x4301, 0x18);
    bus.write_u8(0x4302, 0x34);
    bus.write_u8(0x4303, 0x12);
    bus.write_u8(0x4304, 0x56);

    bus.write_u8(0x420C, 0x01);
    bus.on_frame_start();

    assert_eq!(bus.read_u8(0x4308), 0x34);
    assert_eq!(bus.read_u8(0x4309), 0x12);
    assert_eq!(bus.read_u8(0x430A), 0x80);

    // Emulate an in-progress channel state that must survive a mid-frame 0->1 toggle.
    bus.write_u8(0x4308, 0x78);
    bus.write_u8(0x4309, 0x56);
    bus.write_u8(0x430A, 0x22);

    bus.write_u8(0x420C, 0x00);
    bus.write_u8(0x420C, 0x01);

    assert_eq!(bus.read_u8(0x4308), 0x78);
    assert_eq!(bus.read_u8(0x4309), 0x56);
    assert_eq!(bus.read_u8(0x430A), 0x22);

    bus.on_frame_start();

    assert_eq!(bus.read_u8(0x4308), 0x34);
    assert_eq!(bus.read_u8(0x4309), 0x12);
    assert_eq!(bus.read_u8(0x430A), 0x80);
}
