use std::path::{Path, PathBuf};
use std::process::Command;

fn find_emulator_bin() -> String {
    if let Ok(path) = std::env::var("CARGO_BIN_EXE_snes_emulator") {
        return path;
    }

    let manifest_dir =
        std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is not set by cargo");
    let manifest_dir = PathBuf::from(manifest_dir);
    let search_roots = [manifest_dir.clone(), manifest_dir.join("../..")];
    if let Ok(profile) = std::env::var("SNES_PROFILE") {
        for root in &search_roots {
            let bin = root.join("target").join(&profile).join("snes_emulator");
            if bin.exists() {
                return bin.display().to_string();
            }
        }
    }

    for profile in ["release-fast", "release", "debug"] {
        for root in &search_roots {
            let bin = root.join("target").join(profile).join("snes_emulator");
            if bin.exists() {
                return bin.display().to_string();
            }
        }
    }

    panic!(
        "snes_emulator binary not found. Build it first: cargo build --profile release-fast --bin snes_emulator"
    );
}

fn run_cpu_test_rom(emulator_bin: &str, rom_path: &str, frames: u64) -> String {
    let out = Command::new(emulator_bin)
        .arg(rom_path)
        .env("CPU_TEST_MODE", "1")
        .env("HEADLESS", "1")
        .env("HEADLESS_FRAMES", frames.to_string())
        .env("QUIET", "1")
        .env("HEADLESS_VIS_CHECK", "0")
        .env("HEADLESS_SUMMARY", "0")
        .env("ALLOW_BAD_CHECKSUM", "1")
        .output()
        .expect("failed to launch emulator");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    let all = format!("{}\n{}", stdout, stderr);
    assert!(
        out.status.success(),
        "emulator failed with status={:?}\n{}",
        out.status.code(),
        all
    );
    all
}

#[test]
fn cputest_basic_rom_passes() {
    let rom = "roms/tests/cputest-basic.sfc";
    if !Path::new(rom).exists() {
        eprintln!("Skipping cputest_basic_rom_passes (missing ROM: {rom})");
        return;
    }

    let emulator_bin = find_emulator_bin();
    let all = run_cpu_test_rom(&emulator_bin, rom, 2200);

    assert!(
        all.contains("[CPUTEST] PASS"),
        "expected [CPUTEST] PASS signature, got:\n{}",
        all
    );
    assert!(
        !all.contains("[CPUTEST] FAIL"),
        "unexpected [CPUTEST] FAIL signature:\n{}",
        all
    );
}
