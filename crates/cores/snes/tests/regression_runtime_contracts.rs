use snes_emulator::bus::Bus;
use snes_emulator::hud_toast::{draw_hud_toast, show_hud_toast, HudToast};
use snes_emulator::input::InputSystem;

#[test]
fn controller2_default_is_connected_for_joyser_reads() {
    let mut bus = Bus::new(vec![]);

    // Latch joypad state (4016: 1 -> 0), then read JOYSER1.
    bus.write_u8(0x4016, 1);
    bus.write_u8(0x4016, 0);
    let v = bus.read_u8(0x4017);

    // $4017 bits2-4 are fixed 1, and D0 should be 0 when no button is pressed
    // on a connected standard controller.
    assert_eq!(v & 0x1C, 0x1C);
    assert_eq!(
        v & 0x01,
        0,
        "controller2 default should behave as connected (not pull-up 1)"
    );
}

#[test]
fn controller2_connection_flag_roundtrips_via_input_savestate() {
    let mut input = InputSystem::new();

    // Default: connected -> first serial bit is B button state (0 with no input).
    input.write_strobe(1);
    input.write_strobe(0);
    assert_eq!(input.read_controller2(), 0);

    // Force controller2 to disconnected via save-state payload and verify pull-up behavior.
    let mut st = input.to_save_state();
    st.controller2_connected = false;
    input.load_from_save_state(&st);
    input.write_strobe(1);
    input.write_strobe(0);
    assert_eq!(input.read_controller2(), 1);
}

#[test]
fn rdnmi_bit7_clears_on_read_without_new_vblank_edge() {
    let mut bus = Bus::new(vec![]);

    let first = bus.read_u8(0x4210);
    let second = bus.read_u8(0x4210);

    // Version bits remain 0x02.
    assert_eq!(first & 0x02, 0x02);
    assert_eq!(second & 0x02, 0x02);
    // Read-clear behavior: second read should not keep bit7 asserted.
    assert_eq!(
        second & 0x80,
        0,
        "RDNMI bit7 must clear after read until next vblank edge"
    );
}

#[test]
fn hud_toast_draws_overlay_pixels() {
    let width = 64usize;
    let height = 64usize;
    let mut frame = vec![0u32; width * height];
    let mut toast: Option<HudToast> = None;

    show_hud_toast(&mut toast, "SAVE 1 OK");
    draw_hud_toast(&mut frame, width, height, &mut toast);

    assert!(
        toast.is_some(),
        "toast should still be alive right after show"
    );
    assert!(
        frame.iter().any(|&px| px != 0),
        "hud toast draw should modify framebuffer"
    );
}
