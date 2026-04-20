use super::*;
use crate::bus::PAGE_SIZE;
use crate::vdc::VDC_VBLANK_INTERVAL;
use hucard::{HUCARD_HEADER_SIZE, HUCARD_MAGIC_HI, HUCARD_MAGIC_LO, HUCARD_TYPE_PCE, HucardHeader};

#[test]
fn emulator_runs_simple_program() {
    let mut emu = Emulator::new();
    let program = [0xA9, 0x0F, 0x85, 0x10, 0x00];

    emu.load_program(0xC000, &program);
    emu.reset();
    emu.run_until_halt(Some(20));

    assert_eq!(emu.bus.read(0x0010), 0x0F);
    assert!(emu.cpu.halted);
}

#[test]
fn load_hucard_maps_reset_vector() {
    let mut rom = vec![0u8; PAGE_SIZE * 4];
    let vec_offset = PAGE_SIZE - 2;
    rom[vec_offset] = 0x34;
    rom[vec_offset + 1] = 0xE2;
    let entry = 0xE234usize - 0xE000usize;
    rom[entry] = 0xA9; // LDA #$99
    rom[entry + 1] = 0x99;
    rom[entry + 2] = 0x00; // BRK
    let entry_ptr = PAGE_SIZE - 8;
    rom[entry_ptr] = 0x34;
    rom[entry_ptr + 1] = 0xE2;
    let mut emu = Emulator::new();
    emu.load_hucard(&rom).unwrap();
    emu.reset();

    assert_eq!(emu.cpu.pc, 0xE234);
}

#[test]
fn load_hucard_falls_back_when_high_banks_empty() {
    let mut rom = vec![0u8; PAGE_SIZE * 16];
    let vec_offset = (15 * PAGE_SIZE) + (PAGE_SIZE - 2);
    rom[vec_offset] = 0x78;
    rom[vec_offset + 1] = 0xF6;
    let entry = (15 * PAGE_SIZE) + (0xF678 - 0xE000) as usize;
    rom[entry] = 0xA9; // LDA #$01
    rom[entry + 1] = 0x01;
    rom[entry + 2] = 0x00; // BRK
    let entry_ptr = (15 * PAGE_SIZE) + (PAGE_SIZE - 8);
    rom[entry_ptr] = 0x78;
    rom[entry_ptr + 1] = 0xF6;

    let mut emu = Emulator::new();
    emu.load_hucard(&rom).unwrap();
    emu.reset();

    assert_eq!(emu.cpu.pc, 0xF678);
}

#[test]
fn load_hucard_with_magic_griffin_header_sets_cart_ram() {
    let rom_pages = 4u16;
    let mut image = vec![0u8; HUCARD_HEADER_SIZE + (rom_pages as usize * PAGE_SIZE)];
    image[0] = (rom_pages & 0x00FF) as u8;
    image[1] = (rom_pages >> 8) as u8;
    image[2] = 0x84; // Mode 0 entry, 16 KiB backup RAM.
    image[8] = HUCARD_MAGIC_LO;
    image[9] = HUCARD_MAGIC_HI;
    image[10] = HUCARD_TYPE_PCE;

    let header = HucardHeader::parse(&image).unwrap();
    assert_eq!(header.flags, 0x84);
    assert_eq!(header.backup_ram_bytes(), 16 * 1024);

    let payload = &mut image[HUCARD_HEADER_SIZE..];
    let reset_offset = payload.len() - 2;
    payload[reset_offset] = 0x00;
    payload[reset_offset + 1] = 0x80;
    payload[0] = 0x00; // BRK to halt once execution reaches entry point.

    let mut emu = Emulator::new();
    emu.load_hucard(&image).unwrap();
    assert_eq!(emu.bus.cart_ram_size(), header.backup_ram_bytes());
    emu.reset();

    assert_eq!(emu.bus.cart_ram_size(), 16 * 1024);
    assert_eq!(emu.cpu.pc, 0x8000);
    assert_eq!(emu.bus.read_u16(0xFFFE), 0x8000);
    assert_eq!(emu.bus.read(0x8000), 0x00);
}

#[test]
fn backup_ram_round_trip_via_emulator_api() {
    let rom = vec![0xFF; PAGE_SIZE * 8];
    let mut emu = Emulator::new();
    emu.load_hucard(&rom).unwrap();
    assert!(emu.backup_ram().is_none());
    assert!(emu.load_backup_ram(&[]).is_err());

    // Configure backup RAM explicitly and exercise APIs.
    emu.bus.configure_cart_ram(PAGE_SIZE);
    let snapshot = vec![0xC3; PAGE_SIZE];
    emu.load_backup_ram(&snapshot).unwrap();
    assert_eq!(emu.save_backup_ram().unwrap()[0], 0xC3);
    assert_eq!(emu.bus.cart_ram().unwrap()[..16], snapshot[..16]);
}

#[test]
fn bram_round_trip_via_emulator_api() {
    let rom = vec![0xFF; PAGE_SIZE * 8];
    let mut emu = Emulator::new();
    emu.load_hucard(&rom).unwrap();

    assert_eq!(emu.bram().len(), 0x0800);
    assert!(emu.load_bram(&vec![0; 0x0400]).is_err());

    let snapshot = vec![0x5A; 0x0800];
    emu.load_bram(&snapshot).unwrap();
    assert_eq!(emu.save_bram()[0], 0x5A);

    emu.bus.set_mpr(0, 0xFF);
    emu.bus.set_mpr(2, 0xF7);
    emu.bus.write(0x1807, 0x80);
    assert_eq!(emu.bus.read(0x4000), 0x5A);
}

#[test]
fn wai_unblocks_when_timer_irq_fires() {
    let program = [
        // Set MPR[0]=$FF for I/O access at $0000-$1FFF
        0xA9, 0xFF, // LDA #$FF
        0x53, 0x01, // TAM #$01 (MPR[0] = $FF)
        0xA9, 0x04, // LDA #$04 (timer reload)
        0x8D, 0x00, 0x0C, // STA $0C00
        0xA9, 0x01, // LDA #$01 (start timer)
        0x8D, 0x01, 0x0C, // STA $0C01
        0x58, // CLI
        0xCB, // WAI
        0x00, // BRK
        // IRQ handler immediately after the main routine:
        0xAD, 0x00, 0x40, // LDA $4000
        0x69, 0x01, // ADC #$01
        0x8D, 0x00, 0x40, // STA $4000
        0x40, // RTI
    ];

    let mut emu = Emulator::new();
    emu.load_program(0x8000, &program);
    emu.bus.write_u16(0xFFFA, 0x8011);
    emu.reset();

    emu.run_until_halt(Some(10_000));

    assert!(emu.bus.read(0x4000) > 0);
}

#[test]
fn load_state_accepts_legacy_truncated_payload() {
    let mut emu = Emulator::new();
    let program = [0xA9, 0x42, 0x00];
    emu.load_program(0xC000, &program);
    emu.reset();
    emu.run_until_halt(Some(32));

    let mut bytes = bincode::encode_to_vec(&emu, bincode::config::standard()).unwrap();
    bytes.pop();

    let path = std::env::temp_dir().join(format!(
        "pce_legacy_state_{}_{}.state",
        std::process::id(),
        emu.cycles()
    ));
    std::fs::write(&path, &bytes).unwrap();

    let mut restored = Emulator::new();
    let load_result = restored.load_state_from_file(&path);
    let _ = std::fs::remove_file(&path);

    assert!(load_result.is_ok(), "legacy-compatible load should succeed");
}

#[test]
fn load_state_accepts_previous_render_cache_layout() {
    let mut emu = Emulator::new();
    let program = [0xA9, 0x42, 0x8D, 0x00, 0x20, 0x00];
    emu.load_program(0xC000, &program);
    emu.reset();
    emu.run_until_halt(Some(64));

    emu.bus.write_u16(0x2002, 0xBEEF);
    emu.bus.set_mpr(3, 0xFB);
    emu.bus.write_st_port(0, 0x0A);
    emu.bus.write_st_port(1, 0x34);
    emu.bus.write_st_port(2, 0x12);
    emu.bus.write_st_port(0, 0x0B);
    emu.bus.write_st_port(1, 0x78);
    emu.bus.write_st_port(2, 0x56);

    let compat = CompatEmulatorStateV1 {
        cpu: emu.cpu.clone(),
        bus: emu.bus.compat_state_v1(),
        cycles: emu.cycles,
        audio_buffer: emu.audio_buffer.clone(),
        audio_batch_size: emu.audio_batch_size,
    };

    let bytes = bincode::encode_to_vec(&compat, bincode::config::standard()).unwrap();
    let path = std::env::temp_dir().join(format!(
        "pce_prev_layout_{}_{}.state",
        std::process::id(),
        emu.cycles()
    ));
    std::fs::write(&path, &bytes).unwrap();

    let mut restored = Emulator::new();
    let load_result = restored.load_state_from_file(&path);
    let _ = std::fs::remove_file(&path);

    assert!(load_result.is_ok(), "compat load should succeed");
    assert_eq!(restored.cpu.pc, emu.cpu.pc);
    assert_eq!(restored.cycles(), emu.cycles());
    assert_eq!(restored.bus.read(0x2002), 0xEF);
    assert_eq!(restored.bus.read(0x2003), 0xBE);
    assert_eq!(restored.bus.mpr(3), 0xFB);
    assert_eq!(restored.bus.vdc_register(0x0A), Some(0x1234));
    assert_eq!(restored.bus.vdc_register(0x0B), Some(0x5678));
}

#[test]
fn load_state_invalidates_current_render_cache_round_trip() {
    let mut emu = Emulator::new();
    emu.bus.write_st_port(0, 0x0A);
    emu.bus.write_st_port(1, 0x02);
    emu.bus.write_st_port(2, 0x02);
    emu.bus.write_st_port(0, 0x0B);
    emu.bus.write_st_port(1, 0x1F);
    emu.bus.write_st_port(2, 0x02);
    emu.bus.tick(40_000, false);

    assert!(emu.bus.vdc_scroll_line_valid(0));

    let path = std::env::temp_dir().join(format!(
        "pce_current_roundtrip_{}_{}.state",
        std::process::id(),
        emu.cycles()
    ));
    emu.save_state_to_file(&path).unwrap();

    let mut restored = Emulator::new();
    let load_result = restored.load_state_from_file(&path);
    let _ = std::fs::remove_file(&path);

    assert!(
        load_result.is_ok(),
        "current round-trip load should succeed"
    );
    assert_eq!(restored.bus.vdc_register(0x0A), Some(0x0202));
    assert_eq!(restored.bus.vdc_register(0x0B), Some(0x021F));
    assert!(
        !restored.bus.vdc_scroll_line_valid(0),
        "transient line cache must be invalidated after load"
    );
    assert!(
        restored.take_frame().is_none(),
        "stale serialized frame should not be presented after load"
    );

    let mut fresh_frame = false;
    for _ in 0..4 {
        restored.bus.tick(VDC_VBLANK_INTERVAL, false);
        if restored.take_frame().is_some() {
            fresh_frame = true;
            break;
        }
    }
    assert!(fresh_frame, "loaded emulator should produce a fresh frame");
}
