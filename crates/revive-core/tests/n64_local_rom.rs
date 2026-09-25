//! Optional cartridge test. PS1 and N64 deliberately coexist in one process.
use revive_core::{CoreInstance, SystemKind, VirtualButton};
#[test]
#[ignore = "requires a local cartridge via REVIVE_N64_TEST_ROM"]
fn n64_and_ps1_are_isolated_and_n64_adapter_round_trips() {
    let path = std::fs::canonicalize(
        std::env::var_os("REVIVE_N64_TEST_ROM").expect("set REVIVE_N64_TEST_ROM"),
    )
    .unwrap();
    let dir = tempfile::tempdir().unwrap();
    // This integration test has its own process; don't write saves in the repo.
    let old_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(dir.path()).unwrap();
    let exe = dir.path().join("homebrew.exe");
    let mut data = vec![0u8; 0x1000];
    data[..8].copy_from_slice(b"PS-X EXE");
    for (offset, value) in [
        (0x10, 0x80010000u32),
        (0x18, 0x80010000),
        (0x1c, 0x800),
        (0x30, 0x801fff00),
    ] {
        data[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
    data[0x800..0x804].copy_from_slice(&0x08004000u32.to_le_bytes());
    std::fs::write(&exe, data).unwrap();
    let mut ps1 = ps1_core::Emulator::load(&exe, dir.path()).unwrap();
    let mut n64 = CoreInstance::load_rom(&path, None).unwrap();
    assert_eq!(n64.system(), SystemKind::Nintendo64);
    for _ in 0..120 {
        n64.step_frame().unwrap();
    }
    ps1.step_frame().unwrap();
    assert!(ps1.write_ram(123, 0x42));
    assert!(n64.write_memory_byte("rdram", 0x700000, 0x12));
    n64.set_stick(1, 1234, -5678);
    n64.set_button(1, VirtualButton::CUp, true);
    n64.save_state_to_slot(9).unwrap();
    n64.write_memory_byte("rdram", 0x700000, 0x34);
    n64.load_state_from_slot(9).unwrap();
    assert_eq!(n64.read_memory("rdram").unwrap()[0x700000], 0x12);
    // A successful deserialize must leave the CPU/coroutine runnable.
    let mut heard_audio = false;
    let mut audio = vec![];
    for _ in 0..180 {
        n64.step_frame().unwrap();
        n64.drain_audio_i16(&mut audio);
        heard_audio |= audio.iter().any(|&sample| sample != 0);
    }
    assert!(heard_audio);
    assert_eq!(n64.read_memory("rdram").unwrap()[0x700000], 0x12);
    assert_eq!(ps1.ram()[123], 0x42);
    let state_dir = std::path::Path::new("states/n64").join(path.file_stem().unwrap());
    let good_state = std::fs::read(state_dir.join("slot9.n64st")).unwrap();
    let bad_slot = state_dir.join("slot8.n64st");
    let before_rejection = n64.read_memory("rdram").unwrap().to_vec();
    for mutation in ["checksum", "foreign-rom", "truncated", "oversized"] {
        let mut bad_state = good_state.clone();
        match mutation {
            "checksum" => bad_state[100] ^= 1,
            "foreign-rom" => bad_state[8] ^= 1,
            "truncated" => bad_state.truncate(100),
            "oversized" => bad_state.push(0),
            _ => unreachable!(),
        }
        std::fs::write(&bad_slot, &bad_state).unwrap();
        assert!(n64.load_state_from_slot(8).is_err(), "{mutation}");
        assert_eq!(n64.read_memory("rdram").unwrap(), before_rejection);
        assert_eq!(std::fs::read(&bad_slot).unwrap(), bad_state);
    }
    n64.load_state_from_slot(9).unwrap();
    n64.step_frame().unwrap();
    assert!(!n64.write_memory_byte("rdram", 8 * 1024 * 1024, 1));
    assert!(!n64.write_memory_byte("invalid", 0, 1));
    n64.flush_persistent_save().unwrap();
    let save = std::path::Path::new("states/n64")
        .join(path.file_stem().unwrap())
        .join("cartridge.srm");
    let before = std::fs::read(&save).unwrap();
    assert!(!before.is_empty());
    drop(n64);
    let mut reopened = CoreInstance::load_rom_with_audio(&path, None, false).unwrap();
    reopened.step_frame().unwrap();
    reopened.flush_persistent_save().unwrap();
    assert_eq!(before, std::fs::read(&save).unwrap());
    reopened.drain_audio_i16(&mut audio);
    assert!(audio.is_empty());
    drop(reopened);
    // A damaged battery-save file must not be silently replaced on failed boot.
    let damaged = &before[..before.len() - 1];
    std::fs::write(&save, damaged).unwrap();
    assert!(CoreInstance::load_rom(&path, None).is_err());
    assert_eq!(std::fs::read(&save).unwrap(), damaged);
    std::fs::write(&save, &before).unwrap();
    let mut restored = CoreInstance::load_rom(&path, None).unwrap();
    restored.step_frame().unwrap();
    drop(restored);
    drop(ps1);
    std::env::set_current_dir(old_dir).unwrap();
}
