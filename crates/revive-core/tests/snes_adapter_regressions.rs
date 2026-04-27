use revive_core::{CoreInstance, SystemKind};
use std::collections::BTreeSet;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn rom_path() -> Option<PathBuf> {
    let rel = Path::new("roms/snes/SuperFormationSoccer.sfc");
    if rel.exists() {
        return Some(rel.to_path_buf());
    }

    let manifest_dir =
        PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR missing"));
    let candidates = [
        manifest_dir.join("../..").join(rel),
        manifest_dir.join("../../..").join(rel),
    ];

    candidates.into_iter().find(|path| path.exists())
}

struct EnvVarGuard {
    key: &'static str,
    previous: Option<OsString>,
}

impl Drop for EnvVarGuard {
    fn drop(&mut self) {
        if let Some(value) = self.previous.take() {
            std::env::set_var(self.key, value);
        } else {
            std::env::remove_var(self.key);
        }
    }
}

fn set_env_temp(key: &'static str, value: &str) -> EnvVarGuard {
    let previous = std::env::var_os(key);
    std::env::set_var(key, value);
    EnvVarGuard { key, previous }
}

fn assert_super_formation_soccer_produces_audio() {
    let Some(rom_path) = rom_path() else {
        eprintln!("Skipping snes_adapter_super_formation_soccer_produces_audio (missing ROM)");
        return;
    };

    let mut core = CoreInstance::load_rom_with_audio(&rom_path, Some(SystemKind::Snes), true)
        .expect("ROM should load");
    let mut audio = Vec::new();
    let mut peak = 0i16;
    let mut active_frames = 0usize;
    let mut nonzero_samples = 0usize;
    let mut abs_sum = 0u64;
    let mut sample_count = 0usize;
    let mut first_active_frame = None;
    let mut frame_stats = Vec::new();

    for frame_index in 0..1800 {
        core.step_frame().expect("frame should advance");
        core.drain_audio_i16(&mut audio);
        let frame_peak = audio.iter().map(|sample| sample.saturating_abs()).max();
        let frame_nonzero = audio.iter().filter(|&&sample| sample != 0).count();
        let frame_abs_sum: u64 = audio
            .iter()
            .map(|sample| sample.saturating_abs() as u64)
            .sum();
        if frame_peak.is_some_and(|sample| sample > 256) {
            active_frames += 1;
            first_active_frame.get_or_insert(frame_index);
        }
        for &sample in &audio {
            peak = peak.max(sample.saturating_abs());
            if sample != 0 {
                nonzero_samples += 1;
            }
        }
        abs_sum = abs_sum.saturating_add(frame_abs_sum);
        sample_count += audio.len();
        if frame_peak.is_some_and(|sample| sample != 0) {
            frame_stats.push((
                frame_index,
                frame_peak.unwrap_or(0),
                frame_nonzero,
                frame_abs_sum,
                audio.len(),
            ));
        }
        if active_frames >= 30 && std::env::var_os("SFS_AUDIO_DIAG_FULL").is_none() {
            break;
        }
    }

    if std::env::var_os("SFS_AUDIO_DIAG").is_some() {
        let mean_abs = if sample_count == 0 {
            0.0
        } else {
            abs_sum as f64 / sample_count as f64
        };
        eprintln!(
            "SFS audio diag: peak={peak}, active_frames={active_frames}, first_active_frame={first_active_frame:?}, nonzero_samples={nonzero_samples}, mean_abs={mean_abs:.2}, sample_count={sample_count}"
        );
        for (frame, frame_peak, frame_nonzero, frame_abs_sum, frame_len) in
            frame_stats.iter().take(48)
        {
            let frame_mean_abs = if *frame_nonzero == 0 {
                0.0
            } else {
                *frame_abs_sum as f64 / (*frame_len).max(1) as f64
            };
            eprintln!(
                "  frame={frame:4} peak={frame_peak:5} nonzero={frame_nonzero:4} mean_abs={frame_mean_abs:.2}"
            );
        }
    }

    assert!(
        peak > 256
            && active_frames >= 30
            && nonzero_samples >= 1024
            && first_active_frame.is_some_and(|frame| frame < 900),
        "Super Formation Soccer produced only silent, late, or near-silent audio (peak={peak}, active_frames={active_frames}, first_active_frame={first_active_frame:?}, nonzero_samples={nonzero_samples})"
    );
}

fn run_super_formation_soccer_case(audio_enabled: bool) {
    let Some(rom_path) = rom_path() else {
        eprintln!("Skipping snes_adapter_super_formation_soccer regression (missing ROM)");
        return;
    };

    let _guard = env_lock().lock().unwrap();
    let mut core =
        CoreInstance::load_rom_with_audio(&rom_path, Some(SystemKind::Snes), audio_enabled)
            .expect("ROM should load");
    let mut audio = Vec::new();
    let mut saw_non_black = false;

    for _ in 0..300 {
        core.step_frame().expect("frame should advance");
        core.drain_audio_i16(&mut audio);

        let frame = core.frame();
        saw_non_black = frame.data.chunks_exact(4).any(|px| px[..3] != [0, 0, 0]);
        if saw_non_black {
            break;
        }
    }

    assert!(
        saw_non_black,
        "Super Formation Soccer stayed fully black for 300 adapter frames"
    );
}

fn frame_signature(frame_bytes: &[u8]) -> u64 {
    frame_bytes.iter().fold(0xcbf29ce484222325, |acc, &byte| {
        acc.wrapping_mul(0x100000001b3) ^ u64::from(byte)
    })
}

#[test]
fn snes_adapter_super_formation_soccer_reaches_non_black_frame() {
    run_super_formation_soccer_case(true);
}

#[test]
fn snes_adapter_super_formation_soccer_reaches_non_black_frame_without_audio_backend() {
    run_super_formation_soccer_case(false);
}

#[test]
fn snes_adapter_super_formation_soccer_framebuffer_progresses() {
    let Some(rom_path) = rom_path() else {
        eprintln!(
            "Skipping snes_adapter_super_formation_soccer_framebuffer_progresses (missing ROM)"
        );
        return;
    };

    let _guard = env_lock().lock().unwrap();
    let mut core = CoreInstance::load_rom_with_audio(&rom_path, Some(SystemKind::Snes), false)
        .expect("ROM should load");
    let mut audio = Vec::new();
    let mut signatures = BTreeSet::new();

    for _ in 0..600 {
        core.step_frame().expect("frame should advance");
        core.drain_audio_i16(&mut audio);
        let frame = core.frame();
        signatures.insert(frame_signature(frame.data));
    }

    assert!(
        signatures.len() >= 2,
        "Super Formation Soccer adapter framebuffer never changed after boot"
    );
}

#[test]
fn snes_adapter_super_formation_soccer_produces_audio() {
    let _guard = env_lock().lock().unwrap();
    assert_super_formation_soccer_produces_audio();
}

#[test]
fn snes_adapter_super_formation_soccer_audio_ignores_boot_hle_env() {
    let _guard = env_lock().lock().unwrap();
    let _apu_boot_hle = set_env_temp("APU_BOOT_HLE", "1");
    assert_super_formation_soccer_produces_audio();
}
