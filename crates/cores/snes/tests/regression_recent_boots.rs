use std::path::{Path, PathBuf};
use std::process::Command;

struct RegressionCase<'a> {
    name: &'a str,
    rom_path: &'a str,
    frames: u64,
    min_4200_writes: u64,
    min_mdma: u64,
    min_hdma: u64,
    min_vram_l: u64,
    min_oam: u64,
    min_vis370: u64,
}

#[test]
fn regression_recent_boots() {
    let emulator_bin = find_emulator_bin();

    let cases = [
        RegressionCase {
            // Regression: controller2 default-disconnect caused init stall.
            name: "super_punch_out_np",
            rom_path: "roms/Super Punch-Out!! (Japan) (NP).sfc",
            frames: 420,
            min_4200_writes: 2,
            min_mdma: 18,
            min_hdma: 80,
            min_vram_l: 15_000,
            min_oam: 1_500,
            min_vis370: 7_000,
        },
        RegressionCase {
            // Regression: Derby progression/timing broke around title/demo.
            name: "derby_stallion_iii_rev3",
            rom_path: "roms/Derby Stallion III (Japan) (Rev 3).sfc",
            frames: 420,
            min_4200_writes: 500,
            min_mdma: 50,
            min_hdma: 0,
            min_vram_l: 50_000,
            min_oam: 30_000,
            min_vis370: 30_000,
        },
    ];

    let missing_roms: Vec<&str> = cases
        .iter()
        .filter(|c| !Path::new(c.rom_path).exists())
        .map(|c| c.rom_path)
        .collect();
    if !missing_roms.is_empty() {
        eprintln!(
            "Skipping regression_recent_boots (missing ROMs): {}",
            missing_roms.join(", ")
        );
        return;
    }

    for case in cases {
        run_case(&emulator_bin, &case);
    }
}

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

fn run_case(emulator_bin: &str, case: &RegressionCase<'_>) {
    let output = Command::new(emulator_bin)
        .arg(case.rom_path)
        .env("HEADLESS", "1")
        .env("HEADLESS_FRAMES", case.frames.to_string())
        .env("HEADLESS_VIS_CHECK", "1")
        .env("IGNORE_SRAM", "1")
        .env("NO_SRAM_SAVE", "1")
        .env("QUIET", "1")
        .env("COMPAT_BOOT_FALLBACK", "0")
        .env("COMPAT_INJECT_MIN_PALETTE", "0")
        .env("COMPAT_PERIODIC_MIN_PALETTE", "0")
        .output()
        .expect("failed to launch emulator process");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let all = format!("{}\n{}", stdout, stderr);

    assert!(
        output.status.success(),
        "{}: emulator failed (status={:?})\n{}",
        case.name,
        output.status.code(),
        tail_lines(&all, 60)
    );

    let summary =
        find_last_line_starting_with(&all, "INIT summary: ").expect("INIT summary not found");
    let vis370 = find_last_line_starting_with(&all, "VISIBILITY: frame=370 ")
        .expect("VISIBILITY frame=370 not found");

    let w4200 = parse_u64_after(summary, "$4200 writes=").expect("cannot parse $4200 writes");
    let mdma = parse_u64_after(summary, "MDMAEN!=0=").expect("cannot parse MDMAEN count");
    let hdma = parse_u64_after(summary, "HDMAEN!=0=").expect("cannot parse HDMAEN count");
    let vram_l = parse_u64_after(summary, "VRAM L/H=").expect("cannot parse VRAM L");
    let oam = parse_u64_after(summary, "OAM=").expect("cannot parse OAM count");
    let vis_pixels =
        parse_u64_after(vis370, "non_black_pixels=").expect("cannot parse VISIBILITY pixels");

    assert!(
        w4200 >= case.min_4200_writes,
        "{}: $4200 writes={} < {}",
        case.name,
        w4200,
        case.min_4200_writes
    );
    assert!(
        mdma >= case.min_mdma,
        "{}: MDMAEN!=0={} < {}",
        case.name,
        mdma,
        case.min_mdma
    );
    assert!(
        hdma >= case.min_hdma,
        "{}: HDMAEN!=0={} < {}",
        case.name,
        hdma,
        case.min_hdma
    );
    assert!(
        vram_l >= case.min_vram_l,
        "{}: VRAM_L={} < {}",
        case.name,
        vram_l,
        case.min_vram_l
    );
    assert!(
        oam >= case.min_oam,
        "{}: OAM={} < {}",
        case.name,
        oam,
        case.min_oam
    );
    assert!(
        vis_pixels >= case.min_vis370,
        "{}: vis370={} < {}",
        case.name,
        vis_pixels,
        case.min_vis370
    );
}

fn find_last_line_starting_with<'a>(text: &'a str, prefix: &str) -> Option<&'a str> {
    text.lines().rev().find(|line| line.starts_with(prefix))
}

fn parse_u64_after(line: &str, marker: &str) -> Option<u64> {
    let start = line.find(marker)? + marker.len();
    let digits: String = line[start..]
        .chars()
        .take_while(|c| c.is_ascii_digit())
        .collect();
    if digits.is_empty() {
        None
    } else {
        digits.parse::<u64>().ok()
    }
}

fn tail_lines(text: &str, n: usize) -> String {
    let mut lines: Vec<&str> = text.lines().collect();
    if lines.len() > n {
        lines = lines.split_off(lines.len() - n);
    }
    lines.join("\n")
}
