use super::*;

#[test]
fn vblank_interrupt_becomes_pending_when_enabled() {
    let mut vdp = Vdp::new();
    // Register 1 = 0x60 (display enable + v-interrupt enable)
    vdp.write_control_port(0x8160);
    vdp.step(Vdp::CYCLES_PER_FRAME as u32);

    assert_eq!(vdp.pending_interrupt_level(), Some(6));
    vdp.acknowledge_interrupt(6);
    assert_eq!(vdp.pending_interrupt_level(), None);
}

#[test]
fn hblank_interrupt_becomes_pending_when_enabled() {
    let mut vdp = Vdp::new();
    // Register 0 bit4 enables H-INT. Register 10 = 0 triggers every line.
    vdp.write_control_port(0x8010);
    vdp.write_control_port(0x8A00);
    let cycles_per_line = (Vdp::CYCLES_PER_FRAME / Vdp::TOTAL_LINES) as u32;
    vdp.step(cycles_per_line * 2);

    assert_eq!(vdp.pending_interrupt_level(), Some(4));
    vdp.acknowledge_interrupt(4);
    assert_eq!(vdp.pending_interrupt_level(), None);
}

#[test]
fn hblank_interrupt_line_is_stable_across_frames() {
    let mut vdp = Vdp::new();
    // Enable H-INT and use a large line interval to surface frame-boundary drift.
    vdp.write_control_port(0x8010);
    vdp.write_control_port(0x8AB8);

    let mut first_hint_line_by_frame: [Option<u8>; 3] = [None, None, None];
    while vdp.frame_count() < 3 {
        vdp.step(1);
        if vdp.pending_interrupt_level() == Some(4) {
            let frame = vdp.frame_count() as usize;
            if frame < first_hint_line_by_frame.len() && first_hint_line_by_frame[frame].is_none() {
                let line = (vdp.read_hv_counter() >> 8) as u8;
                first_hint_line_by_frame[frame] = Some(line);
            }
            vdp.acknowledge_interrupt(4);
        }
    }

    let line_frame1 = first_hint_line_by_frame[1].expect("H-INT line for frame 1");
    let line_frame2 = first_hint_line_by_frame[2].expect("H-INT line for frame 2");
    assert_eq!(line_frame1, line_frame2);
}

#[test]
fn vblank_interrupt_has_priority_over_hblank_interrupt() {
    let mut vdp = Vdp::new();
    vdp.write_control_port(0x8010); // H-INT enable
    vdp.write_control_port(0x8160); // V-INT enable + display on
    vdp.write_control_port(0x8A00); // H-INT every line

    vdp.step(Vdp::CYCLES_PER_FRAME as u32);
    assert_eq!(vdp.pending_interrupt_level(), Some(6));
    vdp.acknowledge_interrupt(6);
    assert_eq!(vdp.pending_interrupt_level(), Some(4));
}

#[test]
fn hv_counter_changes_as_cycles_advance() {
    let mut vdp = Vdp::new();
    let before = vdp.read_hv_counter();
    vdp.step(1_000);
    let after = vdp.read_hv_counter();
    assert_ne!(before, after);
}
