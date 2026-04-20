use super::Emulator;
use crate::cartridge::{Cartridge, CartridgeHeader, MapperType};
use crate::emulator::write_framebuffer_png;
use std::ffi::OsString;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::path::PathBuf;
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

fn make_test_emulator_inner() -> Emulator {
    let prev_headless = set_env_temp("HEADLESS", "1");
    let prev_headless_frames = set_env_temp("HEADLESS_FRAMES", "1");
    let prev_quiet = set_env_temp("QUIET", "1");

    let mut rom = vec![0u8; 0x10000];
    rom[0x7FFC] = 0x00;
    rom[0x7FFD] = 0x80;
    let cartridge = Cartridge {
        rom,
        header: CartridgeHeader {
            title: "TEST".to_string(),
            mapper_type: MapperType::LoRom,
            rom_size: 0x10000,
            ram_size: 0,
            country: 0,
            developer: 0,
            version: 0,
            checksum: 0,
            checksum_complement: 0xFFFF,
        },
        has_header: false,
    };

    let emulator = Emulator::new(cartridge, "TEST".to_string(), Option::<PathBuf>::None)
        .expect("failed to construct test emulator");

    restore_env("QUIET", prev_quiet);
    restore_env("HEADLESS_FRAMES", prev_headless_frames);
    restore_env("HEADLESS", prev_headless);

    emulator
}

fn make_test_emulator() -> Emulator {
    let _guard = env_lock().lock().unwrap();
    make_test_emulator_inner()
}

fn read_png_rgba(path: &Path) -> Result<(u32, u32, Vec<u8>), String> {
    let file = File::open(path).map_err(|e| format!("open {}: {}", path.display(), e))?;
    let decoder = png::Decoder::new(BufReader::new(file));
    let mut reader = decoder
        .read_info()
        .map_err(|e| format!("decode {}: {}", path.display(), e))?;
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader
        .next_frame(&mut buf)
        .map_err(|e| format!("frame {}: {}", path.display(), e))?;
    let bytes = &buf[..info.buffer_size()];
    let rgba = match info.color_type {
        png::ColorType::Rgba => bytes.to_vec(),
        png::ColorType::Rgb => {
            let mut out = Vec::with_capacity((info.width * info.height * 4) as usize);
            for rgb in bytes.chunks_exact(3) {
                out.extend_from_slice(&[rgb[0], rgb[1], rgb[2], 0xFF]);
            }
            out
        }
        png::ColorType::Grayscale => {
            let mut out = Vec::with_capacity((info.width * info.height * 4) as usize);
            for &g in bytes {
                out.extend_from_slice(&[g, g, g, 0xFF]);
            }
            out
        }
        png::ColorType::GrayscaleAlpha => {
            let mut out = Vec::with_capacity((info.width * info.height * 4) as usize);
            for ga in bytes.chunks_exact(2) {
                out.extend_from_slice(&[ga[0], ga[0], ga[0], ga[1]]);
            }
            out
        }
        png::ColorType::Indexed => {
            return Err(format!(
                "indexed PNG is not supported for {}",
                path.display()
            ));
        }
    };
    Ok((info.width, info.height, rgba))
}

fn framebuffer_to_rgba_bytes(framebuffer: &[u32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(framebuffer.len() * 4);
    for &pixel in framebuffer {
        out.extend_from_slice(&[
            ((pixel >> 16) & 0xFF) as u8,
            ((pixel >> 8) & 0xFF) as u8,
            (pixel & 0xFF) as u8,
            ((pixel >> 24) & 0xFF) as u8,
        ]);
    }
    out
}

fn framebuffer_to_rgba_bytes_cropped(
    framebuffer: &[u32],
    src_width: usize,
    dst_width: usize,
    dst_height: usize,
) -> Vec<u8> {
    let mut out = Vec::with_capacity(dst_width * dst_height * 4);
    for y in 0..dst_height {
        let row_start = y * src_width;
        let row_end = row_start + dst_width;
        for &pixel in &framebuffer[row_start..row_end] {
            out.extend_from_slice(&[
                ((pixel >> 16) & 0xFF) as u8,
                ((pixel >> 8) & 0xFF) as u8,
                (pixel & 0xFF) as u8,
                ((pixel >> 24) & 0xFF) as u8,
            ]);
        }
    }
    out
}

fn mean_abs_rgba(lhs: &[u8], rhs: &[u8]) -> f64 {
    let mut total_abs = 0u64;
    for (l, r) in lhs.iter().zip(rhs.iter()) {
        total_abs += l.abs_diff(*r) as u64;
    }
    total_abs as f64 / lhs.len() as f64
}

fn weighted_startup_score(lhs: &[u8], rhs: &[u8]) -> f64 {
    let mut total = 0f64;
    let mut weight_sum = 0f64;
    for (lp, rp) in lhs.chunks_exact(4).zip(rhs.chunks_exact(4)) {
        let rr = rp[0] as f64;
        let rg = rp[1] as f64;
        let rb = rp[2] as f64;
        let ref_luma = (rr * 0.299 + rg * 0.587 + rb * 0.114) as f64;
        let lr = lp[0] as f64;
        let lg = lp[1] as f64;
        let lb = lp[2] as f64;
        let diff = (lr - rr).abs() + (lg - rg).abs() + (lb - rb).abs();
        let weight = if ref_luma > 8.0 { 12.0 } else { 1.0 };
        total += diff * weight;
        weight_sum += 3.0 * weight;
    }
    total / weight_sum.max(1.0)
}

fn save_rgba_png(path: &Path, rgba: &[u8], width: usize, height: usize) -> Result<(), String> {
    let file = File::create(path).map_err(|e| format!("create {}: {}", path.display(), e))?;
    let mut encoder = png::Encoder::new(file, width as u32, height as u32);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder
        .write_header()
        .map_err(|e| format!("png header {}: {}", path.display(), e))?;
    writer
        .write_image_data(rgba)
        .map_err(|e| format!("png data {}: {}", path.display(), e))?;
    writer
        .finish()
        .map_err(|e| format!("png finish {}: {}", path.display(), e))
}

fn resize_rgba_nearest(
    src: &[u8],
    src_w: usize,
    src_h: usize,
    dst_w: usize,
    dst_h: usize,
) -> Vec<u8> {
    let mut out = vec![0u8; dst_w * dst_h * 4];
    for y in 0..dst_h {
        let sy = y * src_h / dst_h;
        for x in 0..dst_w {
            let sx = x * src_w / dst_w;
            let src_idx = (sy * src_w + sx) * 4;
            let dst_idx = (y * dst_w + x) * 4;
            out[dst_idx..dst_idx + 4].copy_from_slice(&src[src_idx..src_idx + 4]);
        }
    }
    out
}

mod core;
mod starfox;
mod superfx_capture;
mod superfx_direct;

#[derive(Default)]
struct StarFoxGsuTestOverrides {
    regs: Vec<(usize, u16)>,
    ram_words: Vec<(u16, u16)>,
    pbr: Option<u8>,
    rombr: Option<u8>,
    scmr: Option<u8>,
    sfr: Option<u16>,
    src_reg: Option<u8>,
    dst_reg: Option<u8>,
    with_reg: Option<u8>,
    clear_pipe: bool,
    invoke_cpu_start: bool,
}
fn parse_star_fox_gsu_test_overrides() -> StarFoxGsuTestOverrides {
    let regs = (0usize..=15)
        .into_iter()
        .filter_map(|reg| {
            let name = format!("STARFOX_GSU_OVERRIDE_R{reg}");
            let value = std::env::var(&name).ok()?;
            let token = value
                .trim()
                .trim_start_matches("0x")
                .trim_start_matches("0X");
            let parsed = u16::from_str_radix(token, 16).ok()?;
            Some((reg, parsed))
        })
        .collect::<Vec<_>>();
    let ram_words = std::env::var("STARFOX_GSU_OVERRIDE_RAM_WORDS")
        .ok()
        .map(|raw| {
            raw.split(',')
                .filter_map(|entry| {
                    let (addr, value) = entry.split_once('=')?;
                    let addr = addr
                        .trim()
                        .trim_start_matches("0x")
                        .trim_start_matches("0X");
                    let value = value
                        .trim()
                        .trim_start_matches("0x")
                        .trim_start_matches("0X");
                    Some((
                        u16::from_str_radix(addr, 16).ok()?,
                        u16::from_str_radix(value, 16).ok()?,
                    ))
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let parse_u8_hex = |name: &str| {
        std::env::var(name).ok().and_then(|s| {
            let token = s.trim().trim_start_matches("0x").trim_start_matches("0X");
            u8::from_str_radix(token, 16).ok()
        })
    };
    let parse_u16_hex = |name: &str| {
        std::env::var(name).ok().and_then(|s| {
            let token = s.trim().trim_start_matches("0x").trim_start_matches("0X");
            u16::from_str_radix(token, 16).ok()
        })
    };
    StarFoxGsuTestOverrides {
        regs,
        ram_words,
        pbr: parse_u8_hex("STARFOX_GSU_OVERRIDE_PBR"),
        rombr: parse_u8_hex("STARFOX_GSU_OVERRIDE_ROMBR"),
        scmr: parse_u8_hex("STARFOX_GSU_OVERRIDE_SCMR"),
        sfr: parse_u16_hex("STARFOX_GSU_OVERRIDE_SFR"),
        src_reg: std::env::var("STARFOX_GSU_OVERRIDE_SRC")
            .ok()
            .and_then(|s| s.parse::<u8>().ok()),
        dst_reg: std::env::var("STARFOX_GSU_OVERRIDE_DST")
            .ok()
            .and_then(|s| s.parse::<u8>().ok()),
        with_reg: std::env::var("STARFOX_GSU_OVERRIDE_WITH")
            .ok()
            .and_then(|s| s.parse::<u8>().ok()),
        clear_pipe: std::env::var_os("STARFOX_GSU_OVERRIDE_CLEAR_PIPE").is_some(),
        invoke_cpu_start: std::env::var_os("STARFOX_GSU_INVOKE_CPU_START").is_some(),
    }
}
fn parse_trace_superfx_recent_regs() -> Vec<u8> {
    std::env::var("TRACE_SUPERFX_RECENT_REGS")
        .ok()
        .map(|raw| {
            raw.split(',')
                .filter_map(|token| {
                    let token = token.trim();
                    if token.is_empty() {
                        return None;
                    }
                    let token = token.strip_prefix('r').unwrap_or(token);
                    let token = token.strip_prefix('R').unwrap_or(token);
                    let parsed = token.parse::<u8>().ok().or_else(|| {
                        let token = token.trim_start_matches("0x").trim_start_matches("0X");
                        u8::from_str_radix(token, 16).ok()
                    })?;
                    Some(parsed & 0x0F)
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}
fn trace_superfx_recent_regs_limit() -> usize {
    std::env::var("TRACE_SUPERFX_RECENT_REGS_LIMIT")
        .ok()
        .and_then(|raw| raw.parse::<usize>().ok())
        .filter(|&n| n > 0)
        .unwrap_or(32)
}
fn apply_star_fox_gsu_test_overrides(emulator: &mut Emulator, overrides: &StarFoxGsuTestOverrides) {
    let rom = if overrides.invoke_cpu_start {
        Some(emulator.bus.rom.clone())
    } else {
        None
    };
    if let Some(gsu) = emulator.bus.superfx.as_mut() {
        for (addr, value) in &overrides.ram_words {
            gsu.debug_write_ram_word(*addr, *value);
        }
        for (reg, value) in &overrides.regs {
            gsu.debug_set_reg(*reg, *value);
        }
        if let Some(value) = overrides.pbr {
            gsu.debug_set_pbr(value);
        }
        if let Some(value) = overrides.rombr {
            gsu.debug_set_rombr(value);
        }
        if let Some(value) = overrides.scmr {
            gsu.debug_set_scmr(value);
        }
        if let Some(value) = overrides.sfr {
            gsu.debug_set_sfr(value);
        }
        if let Some(value) = overrides.src_reg {
            gsu.debug_set_src_reg(value);
        }
        if let Some(value) = overrides.dst_reg {
            gsu.debug_set_dst_reg(value);
        }
        if let Some(value) = overrides.with_reg {
            gsu.debug_set_with_reg(value);
        }
        if overrides.clear_pipe {
            gsu.debug_clear_pipe();
        }
        if let (true, Some(rom)) = (overrides.invoke_cpu_start, rom.as_ref()) {
            gsu.debug_prepare_cpu_start(rom);
        }
    }
}
fn step_one_frame_state_only_for_test(emulator: &mut Emulator) -> bool {
    crate::cartridge::superfx::set_trace_superfx_exec_frame(emulator.frame_count.wrapping_add(1));
    emulator.run_frame();
    if emulator.take_save_state_capture_stop_requested() {
        return true;
    }
    emulator.maybe_auto_unblank();
    emulator.maybe_force_unblank();
    emulator.maybe_inject_min_palette_periodic();
    emulator.frame_count = emulator.frame_count.wrapping_add(1);
    crate::shutdown::should_quit()
}
