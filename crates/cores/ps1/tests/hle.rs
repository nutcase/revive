//! A tiny original MIPS program exercises HLE without retail firmware or ROMs.
use ps1_core::Emulator;
#[test]
fn hle_executes_homebrew_and_restores_state_and_memory_card() {
    let dir = tempfile::tempdir().unwrap();
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
    // Jump to self, delay-slot NOP. The video counters still advance normally.
    data[0x800..0x804].copy_from_slice(&0x08004000u32.to_le_bytes());
    std::fs::write(&exe, data).unwrap();
    let mut core = Emulator::load(&exe, dir.path()).unwrap();
    assert!(
        Emulator::load(&exe, dir.path()).is_err(),
        "native globals must have one owner"
    );
    assert_eq!(core.ram().len(), 2 * 1024 * 1024);
    assert_eq!(core.memory_card().len(), 128 * 1024);
    assert_eq!(&core.memory_card()[..2], b"MC");
    for _ in 0..3 {
        core.step_frame().unwrap();
    }
    let mut audio = Vec::new();
    core.drain_audio(&mut audio);
    assert!(!audio.is_empty());
    assert!(core.write_ram(0x1ff000, 42));
    let state = core.save_state().unwrap();
    assert!(core.load_state(&state[..100]).is_err());
    core.write_ram(0x1ff000, 7);
    core.load_state(&state).unwrap();
    assert_eq!(core.ram()[0x1ff000], 42);
    core.step_frame().unwrap();
    core.set_audio_enabled(false);
    core.step_frame().unwrap();
    core.drain_audio(&mut audio);
    assert!(audio.is_empty());
    assert!(!core.write_ram(2 * 1024 * 1024, 0));
    let mut card = core.memory_card().to_vec();
    card[0x4000] = 0x5a;
    assert!(core.load_memory_card(&card[..10]).is_err());
    core.load_memory_card(&card).unwrap();
    drop(core);
    let mut core = Emulator::load(&exe, dir.path()).unwrap();
    core.load_memory_card(&card).unwrap();
    assert_eq!(core.memory_card()[0x4000], 0x5a);
    core.step_frame().unwrap();
}
