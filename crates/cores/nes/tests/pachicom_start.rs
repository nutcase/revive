use std::path::PathBuf;

use nes_emulator::Nes;

fn run_n_frames(nes: &mut Nes, frames: usize) {
    for _ in 0..frames {
        while !nes.step() {}
    }
}

fn non_black_pixels(buf: &[u8]) -> usize {
    buf.chunks_exact(3)
        .filter(|px| px.iter().any(|&c| c > 8))
        .count()
}

/// Pachicom (Japan) uses an `LDA $2002 / BPL -5` vblank wait after pressing
/// Start, with NMI enabled. An atomic-per-instruction CPU model misses the
/// vblank flag transition — the NMI handler consumes the 20-scanline vblank
/// window and clears the flag before the main loop can poll it, leaving the
/// screen permanently blanked. Covers the `$2002`-read timing fix in
/// `Bus::read` that pre-advances the PPU so the read sees mid-instruction
/// cycle timing, matching real hardware.
#[test]
fn pachicom_start_advances_past_title() {
    let rom_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .join("roms/nes/Pachicom (Japan).nes");

    if !rom_path.exists() {
        eprintln!("skipping: ROM not found at {}", rom_path.display());
        return;
    }

    let mut nes = Nes::new();
    nes.load_rom(rom_path.to_str().unwrap()).unwrap();

    run_n_frames(&mut nes, 240);

    nes.set_controller(0x08); // Start
    run_n_frames(&mut nes, 10);
    nes.set_controller(0x00);
    run_n_frames(&mut nes, 180);

    let nb = non_black_pixels(nes.get_frame_buffer());
    assert!(
        nb > 500,
        "screen stayed black after pressing Start (non-black pixels: {nb})"
    );
}
