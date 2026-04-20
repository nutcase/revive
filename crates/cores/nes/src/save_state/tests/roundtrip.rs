use crate::save_state::SaveState;

#[test]
fn save_state_round_trips_cpu_and_bus_timing_fields() {
    let state = sample_state();

    let encoded = bincode::serialize(&state).expect("serialize save state");
    let decoded: SaveState = bincode::deserialize(&encoded).expect("deserialize save state");

    assert_state_timing(&decoded);
}

#[test]
fn save_state_file_round_trips_current_version_wrapper() {
    let state = sample_state();
    let mut path = std::env::temp_dir();
    path.push(format!(
        "nes_current_state_{}.sav",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time")
            .as_nanos()
    ));

    state
        .save_to_file(path.to_str().expect("utf-8 path"))
        .unwrap();
    let decoded = SaveState::load_from_file(path.to_str().expect("utf-8 path")).unwrap();
    let _ = std::fs::remove_file(path);

    assert_eq!(SaveState::FORMAT_VERSION, 4);
    assert_state_timing(&decoded);
    assert_eq!(decoded.rom_filename(), "roundtrip");
    assert_eq!(decoded.timestamp(), 999);
}

fn sample_state() -> SaveState {
    SaveState {
        cpu_a: 0,
        cpu_x: 0,
        cpu_y: 0,
        cpu_pc: 0x8000,
        cpu_sp: 0xFD,
        cpu_status: 0x24,
        cpu_cycles: 42_123,
        ppu_control: 0,
        ppu_mask: 0,
        ppu_status: 0,
        ppu_oam_addr: 0,
        ppu_scroll_x: 0,
        ppu_scroll_y: 0,
        ppu_addr: 0,
        ppu_data_buffer: 0,
        ppu_w: false,
        ppu_t: 0,
        ppu_v: 0,
        ppu_x: 0,
        ppu_scanline: 10,
        ppu_cycle: 123,
        ppu_frame: 456,
        ppu_palette: [0; 32],
        ppu_nametable: vec![0; 2048],
        ppu_oam: vec![0; 256],
        ram: vec![0; 0x800],
        cartridge_prg_bank: 0,
        cartridge_chr_bank: 0,
        cartridge_state: None,
        apu_frame_counter: 0,
        apu_frame_interrupt: false,
        apu_state: None,
        rom_filename: "roundtrip".to_string(),
        timestamp: 999,
        cpu_halted: true,
        bus_dma_cycles: 7,
        bus_dma_in_progress: true,
        bus_dmc_stall_cycles: 3,
        ppu_frame_complete: true,
    }
}

fn assert_state_timing(decoded: &SaveState) {
    assert_eq!(decoded.cpu_cycles, 42_123);
    assert!(decoded.cpu_halted);
    assert_eq!(decoded.bus_dma_cycles, 7);
    assert!(decoded.bus_dma_in_progress);
    assert_eq!(decoded.bus_dmc_stall_cycles, 3);
    assert!(decoded.ppu_frame_complete);
}
