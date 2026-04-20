use crate::save_state::legacy::{CartridgeStateV1, LegacySaveState, SaveStateV1, SaveStateV2};

use super::helpers::load_serialized_save_state;

#[test]
fn deserialize_legacy_save_state_defaults_new_fields() {
    let legacy = LegacySaveState {
        cpu_a: 1,
        cpu_x: 2,
        cpu_y: 3,
        cpu_pc: 0x8000,
        cpu_sp: 0xFD,
        cpu_status: 0x24,
        cpu_cycles: 123,
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
        ppu_scanline: 0,
        ppu_cycle: 0,
        ppu_frame: 0,
        ppu_palette: [0; 32],
        ppu_nametable: vec![0; 2048],
        ppu_oam: vec![0; 256],
        ram: vec![0; 0x800],
        cartridge_prg_bank: 0,
        cartridge_chr_bank: 0,
        apu_frame_counter: 0,
        apu_frame_interrupt: false,
        rom_filename: "legacy".to_string(),
        timestamp: 0,
    };

    let decoded = load_serialized_save_state("legacy", &legacy);

    assert!(decoded.cartridge_state.is_none());
    assert!(decoded.apu_state.is_none());
    assert!(!decoded.cpu_halted);
    assert_eq!(decoded.bus_dma_cycles, 0);
    assert!(!decoded.bus_dma_in_progress);
    assert_eq!(decoded.bus_dmc_stall_cycles, 0);
    assert!(!decoded.ppu_frame_complete);
}

#[test]
fn deserialize_v2_save_state_defaults_apu_state() {
    let v2 = SaveStateV2 {
        cpu_a: 0x01,
        cpu_x: 0x02,
        cpu_y: 0x03,
        cpu_pc: 0x8000,
        cpu_sp: 0xFD,
        cpu_status: 0x24,
        cpu_cycles: 1234,
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
        ppu_scanline: 0,
        ppu_cycle: 0,
        ppu_frame: 0,
        ppu_palette: [0; 32],
        ppu_nametable: vec![0; 2048],
        ppu_oam: vec![0; 256],
        ram: vec![0; 0x800],
        cartridge_prg_bank: 0,
        cartridge_chr_bank: 0,
        cartridge_state: None,
        apu_frame_counter: 7,
        apu_frame_interrupt: true,
        rom_filename: "v2".to_string(),
        timestamp: 77,
    };

    let decoded = load_serialized_save_state("v2", &v2);

    assert_eq!(decoded.apu_frame_counter, 7);
    assert!(decoded.apu_frame_interrupt);
    assert!(decoded.apu_state.is_none());
    assert!(!decoded.cpu_halted);
    assert_eq!(decoded.bus_dma_cycles, 0);
    assert!(!decoded.bus_dma_in_progress);
    assert_eq!(decoded.bus_dmc_stall_cycles, 0);
    assert!(!decoded.ppu_frame_complete);
}

#[test]
fn deserialize_v1_save_state_without_mmc2() {
    use crate::cartridge::{Mirroring, Mmc1State};

    // Build a V1-era SaveState (CartridgeState without mmc2)
    let v1 = SaveStateV1 {
        cpu_a: 0x10,
        cpu_x: 0x20,
        cpu_y: 0x30,
        cpu_pc: 0xC000,
        cpu_sp: 0xFB,
        cpu_status: 0x24,
        cpu_cycles: 5000,
        ppu_control: 0x80,
        ppu_mask: 0x1E,
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
        ppu_scanline: 0,
        ppu_cycle: 0,
        ppu_frame: 100,
        ppu_palette: [0; 32],
        ppu_nametable: vec![0; 2048],
        ppu_oam: vec![0; 256],
        ram: vec![0; 0x800],
        cartridge_prg_bank: 3,
        cartridge_chr_bank: 5,
        cartridge_state: Some(CartridgeStateV1 {
            mapper: 1,
            mirroring: Mirroring::Vertical,
            prg_bank: 3,
            chr_bank: 5,
            prg_ram: vec![0xAA; 0x2000],
            chr_ram: vec![0; 0x2000],
            has_valid_save_data: true,
            mmc1: Some(Mmc1State {
                shift_register: 0x10,
                shift_count: 0,
                control: 0x0C,
                chr_bank_0: 2,
                chr_bank_1: 4,
                prg_bank: 3,
                prg_ram_disable: false,
            }),
        }),
        apu_frame_counter: 2,
        apu_frame_interrupt: false,
        rom_filename: "v1_test".to_string(),
        timestamp: 12345,
    };

    let decoded = load_serialized_save_state("v1", &v1);

    assert_eq!(decoded.cpu_a, 0x10);
    assert_eq!(decoded.apu_frame_counter, 2);
    assert_eq!(decoded.rom_filename, "v1_test");
    assert!(decoded.apu_state.is_none());
    assert!(!decoded.cpu_halted);
    assert_eq!(decoded.bus_dma_cycles, 0);
    assert!(!decoded.bus_dma_in_progress);
    assert_eq!(decoded.bus_dmc_stall_cycles, 0);
    assert!(!decoded.ppu_frame_complete);

    let cs = decoded
        .cartridge_state
        .expect("cartridge_state should exist");
    assert_eq!(cs.mapper, 1);
    assert_eq!(cs.prg_bank, 3);
    assert!(cs.mmc1.is_some());
    assert!(cs.mmc2.is_none());
}
