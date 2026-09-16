//! Opt-in local integration test; copyrighted test data is never committed.
//! REVIVE_PS1_TEST_ROM=... cargo test -p revive-core --test ps1_local_disc -- --ignored
use revive_core::{CoreInstance, SystemKind, VirtualButton};
use std::{path::PathBuf, time::Instant};
#[test]
#[ignore = "requires a local PS1 ZIP containing CUE/BIN via REVIVE_PS1_TEST_ROM"]
fn zipped_disc_hle_audio_input_and_save_round_trip() {
    let mut source =
        PathBuf::from(std::env::var_os("REVIVE_PS1_TEST_ROM").expect("set REVIVE_PS1_TEST_ROM"));
    if source.is_relative() && !source.exists() {
        source = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join(source);
    }
    let temp = tempfile::Builder::new()
        .prefix("revive-ps1-check-")
        .tempdir()
        .unwrap();
    let stem = temp.path().file_name().unwrap();
    let rom = temp.path().join(stem).with_extension("zip");
    std::fs::copy(source, &rom).unwrap();
    let mut core = CoreInstance::load_rom(&rom, None).unwrap();
    assert_eq!(core.system(), SystemKind::PlayStation);
    assert_eq!(core.audio_spec().sample_rate_hz, 44_100);
    assert_eq!(core.audio_spec().channels, 2);
    assert!((49.0..61.0).contains(&core.frame_rate_hz()));
    let started = Instant::now();
    let mut audio = Vec::new();
    let mut nonzero = 0usize;
    for frame in 0..2400 {
        if frame == 1810 {
            core.set_button(1, VirtualButton::Start, true);
        }
        if frame == 1830 {
            core.set_button(1, VirtualButton::Start, false);
        }
        core.step_frame().unwrap();
        core.drain_audio_i16(&mut audio);
        nonzero += audio.iter().filter(|&&v| v != 0).count();
    }
    assert!(nonzero > 44_100, "game should produce audible samples");
    let frame = core.frame();
    assert_eq!(frame.data.len(), frame.width * frame.height * 4);
    assert!(
        frame
            .data
            .chunks_exact(4)
            .filter(|p| p[..3] != [0, 0, 0])
            .count()
            > 1000
    );
    let before = core.read_memory("ram").unwrap().to_vec();
    core.save_state_to_slot(9).unwrap();
    let original = before[0x1ff000];
    core.write_memory_byte("ram", 0x1ff000, original ^ 0xff);
    core.load_state_from_slot(9).unwrap();
    assert!(
        core.read_memory("ram").unwrap() == before,
        "RAM must round-trip through state load"
    );
    core.step_frame().unwrap();
    core.flush_persistent_save().unwrap();
    let saves = PathBuf::from("states/ps1").join(stem);
    let card = std::fs::read(saves.join("memory-card.mcd")).unwrap();
    assert_eq!(card.len(), 128 * 1024);
    drop(core);
    let mut core = CoreInstance::load_rom(&rom, None).unwrap();
    core.load_state_from_slot(9).unwrap();
    core.step_frame().unwrap();
    core.flush_persistent_save().unwrap();
    assert_eq!(std::fs::read(saves.join("memory-card.mcd")).unwrap(), card);
    drop(core);
    std::fs::remove_dir_all(saves).unwrap();
    println!(
        "PS1 ZIP/HLE 2400 frames + reopen/state/card: {:.2}s",
        started.elapsed().as_secs_f64()
    );
}
