use snes_emulator::cartridge::Cartridge;
use snes_emulator::emulator::Emulator;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn set_env_temp(key: &str, value: &str) -> Option<OsString> {
    let prev = std::env::var_os(key);
    std::env::set_var(key, value);
    prev
}

fn restore_env(key: &str, prev: Option<OsString>) {
    if let Some(value) = prev {
        std::env::set_var(key, value);
    } else {
        std::env::remove_var(key);
    }
}

fn rom_path() -> Option<PathBuf> {
    let rel = Path::new("roms/snes/SuperFormationSoccer.sfc");
    if rel.exists() {
        return Some(rel.to_path_buf());
    }

    let manifest_dir =
        PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR missing"));
    let candidates = [
        manifest_dir.join("../../..").join(rel),
        manifest_dir.join("../../../..").join(rel),
    ];

    candidates.into_iter().find(|path| path.exists())
}

#[test]
fn super_formation_soccer_reaches_visible_output() {
    let Some(rom_path) = rom_path() else {
        eprintln!("Skipping super_formation_soccer_reaches_visible_output (missing ROM)");
        return;
    };

    let _guard = env_lock().lock().unwrap();
    let prev_headless = set_env_temp("HEADLESS", "1");
    let prev_headless_frames = set_env_temp("HEADLESS_FRAMES", "1");
    let prev_quiet = set_env_temp("QUIET", "1");
    let prev_ignore_sram = set_env_temp("IGNORE_SRAM", "1");
    let prev_no_sram_save = set_env_temp("NO_SRAM_SAVE", "1");
    let prev_headless_summary = set_env_temp("HEADLESS_SUMMARY", "0");

    let cart = Cartridge::load_from_file(&rom_path).expect("ROM should parse");
    let title = cart.header.title.clone();
    let mut emulator =
        Emulator::new(cart, title, Option::<PathBuf>::None).expect("emulator should construct");

    restore_env("HEADLESS_SUMMARY", prev_headless_summary);
    restore_env("NO_SRAM_SAVE", prev_no_sram_save);
    restore_env("IGNORE_SRAM", prev_ignore_sram);
    restore_env("QUIET", prev_quiet);
    restore_env("HEADLESS_FRAMES", prev_headless_frames);
    restore_env("HEADLESS", prev_headless);

    assert!(
        emulator.warmup_until_unblanked(300),
        "Super Formation Soccer did not reach unblanked output within 300 frames; frame={} pc={:06X} tm={:02X} mode={} inidisp={:02X}",
        emulator.frame_count(),
        emulator.current_cpu_pc(),
        emulator.current_tm(),
        emulator.current_bg_mode(),
        emulator.current_inidisp(),
    );
}

#[test]
fn super_formation_soccer_leaves_apu_upload_waits() {
    let Some(rom_path) = rom_path() else {
        eprintln!("Skipping super_formation_soccer_leaves_apu_upload_waits (missing ROM)");
        return;
    };

    let _guard = env_lock().lock().unwrap();
    let prev_headless = set_env_temp("HEADLESS", "1");
    let prev_headless_frames = set_env_temp("HEADLESS_FRAMES", "1");
    let prev_quiet = set_env_temp("QUIET", "1");
    let prev_ignore_sram = set_env_temp("IGNORE_SRAM", "1");
    let prev_no_sram_save = set_env_temp("NO_SRAM_SAVE", "1");
    let prev_headless_summary = set_env_temp("HEADLESS_SUMMARY", "0");

    let cart = Cartridge::load_from_file(&rom_path).expect("ROM should parse");
    let title = cart.header.title.clone();
    let mut emulator =
        Emulator::new(cart, title, Option::<PathBuf>::None).expect("emulator should construct");

    let frames = std::env::var("SFS_APU_WAIT_FRAMES")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(1600);
    for _ in 0..frames {
        emulator.step_one_frame();
    }

    let pc = emulator.current_cpu_pc();
    let frame = emulator.frame_count();
    let tm = emulator.current_tm();
    let mode = emulator.current_bg_mode();
    let inidisp = emulator.current_inidisp();

    restore_env("HEADLESS_SUMMARY", prev_headless_summary);
    restore_env("NO_SRAM_SAVE", prev_no_sram_save);
    restore_env("IGNORE_SRAM", prev_ignore_sram);
    restore_env("QUIET", prev_quiet);
    restore_env("HEADLESS_FRAMES", prev_headless_frames);
    restore_env("HEADLESS", prev_headless);

    assert!(
        !matches!(pc, 0x008858 | 0x00885B | 0x0088BD),
        "Super Formation Soccer remained in an APU port wait loop; frame={} pc={:06X} tm={:02X} mode={} inidisp={:02X}",
        frame,
        pc,
        tm,
        mode,
        inidisp,
    );
    assert_eq!(
        inidisp & 0x80,
        0,
        "Super Formation Soccer was still forced blank after APU uploads; frame={} pc={:06X}",
        frame,
        pc,
    );
}
