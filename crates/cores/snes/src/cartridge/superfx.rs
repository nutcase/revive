use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::OnceLock;

const GSU_REGISTER_COUNT: usize = 16;
const CACHE_RAM_SIZE: usize = 0x200;
const GAME_RAM_SIZE: usize = 0x2_0000;

#[derive(Clone, Copy)]
struct PixelCache {
    offset: u16,
    bitpend: u8,
    data: [u8; 8],
}

impl Default for PixelCache {
    fn default() -> Self {
        Self {
            offset: u16::MAX,
            bitpend: 0,
            data: [0; 8],
        }
    }
}
const DEFAULT_STARTUP_STEP_BUDGET: usize = 524_288;
// Calibrated for the current instruction-step SuperFX core. GUI real-time speed
// is controlled by frontend pacing; lowering these stalls Star Fox's bootstrap.
const DEFAULT_SUPERFX_RATIO_FAST: usize = 12;
const DEFAULT_SUPERFX_RATIO_SLOW: usize = 6;
// Status-poll catch-up works best with smaller, more frequent slices.
// Large bursts tend to keep the GSU inside long cached loops without letting
// the CPU/PPU side observe forward progress at the right cadence.
const DEFAULT_SUPERFX_STATUS_POLL_STEP_BUDGET: usize = 2_048;
const MAX_RECENT_REG_WRITES: usize = 4_096;
const MAX_RECENT_REG_WRITES_PER_REG: usize = 4_096;

const SFR_Z_BIT: u16 = 0x0002;
const SFR_CY_BIT: u16 = 0x0004;
const SFR_S_BIT: u16 = 0x0008;
const SFR_OV_BIT: u16 = 0x0010;
const SFR_GO_BIT: u16 = 0x0020;
const SFR_R_BIT: u16 = 0x0040;
const SFR_ALT2_BIT: u16 = 0x0100;
const SFR_ALT1_BIT: u16 = 0x0200;
const SFR_B_BIT: u16 = 0x1000;
const SFR_IRQ_BIT: u16 = 0x8000;

const SCMR_RON_BIT: u8 = 0x10;
const SCMR_RAN_BIT: u8 = 0x08;

static TRACE_SUPERFX_EXEC_FRAME: AtomicU32 = AtomicU32::new(0);

enum TraceSuperfxRamAddrConfig {
    Range { start_addr: u16, end_addr: u16 },
    List(Vec<u16>),
}

fn env_flag_enabled(name: &'static str) -> bool {
    std::env::var_os(name).is_some()
}

macro_rules! cached_env_presence {
    ($name:literal) => {{
        static ON: OnceLock<bool> = OnceLock::new();
        *ON.get_or_init(|| std::env::var_os($name).is_some())
    }};
}

macro_rules! cached_env_u16 {
    ($name:literal) => {{
        static VALUE: OnceLock<Option<u16>> = OnceLock::new();
        *VALUE.get_or_init(|| env_u16_direct($name))
    }};
}

#[inline]
fn env_presence_cached(name: &'static str) -> bool {
    if cfg!(test) {
        return std::env::var_os(name).is_some();
    }

    match name {
        "STARFOX_FORCE_LATE_SEARCH_KEY_FROM_ANY_TABLE_FIELD" => {
            cached_env_presence!("STARFOX_FORCE_LATE_SEARCH_KEY_FROM_ANY_TABLE_FIELD")
        }
        "STARFOX_FORCE_LATE_SEARCH_KEY_FROM_1AE2" => {
            cached_env_presence!("STARFOX_FORCE_LATE_SEARCH_KEY_FROM_1AE2")
        }
        "STARFOX_FORCE_PARSER_KEY_FROM_ANY_TABLE_FIELD" => {
            cached_env_presence!("STARFOX_FORCE_PARSER_KEY_FROM_ANY_TABLE_FIELD")
        }
        "STARFOX_FORCE_PARSER_KEY_FROM_MATCH_WORD" => {
            cached_env_presence!("STARFOX_FORCE_PARSER_KEY_FROM_MATCH_WORD")
        }
        "STARFOX_KEEP_SUCCESS_CURSOR_ARMED" => {
            cached_env_presence!("STARFOX_KEEP_SUCCESS_CURSOR_ARMED")
        }
        "STARFOX_KEEP_SUCCESS_CONTEXT" => cached_env_presence!("STARFOX_KEEP_SUCCESS_CONTEXT"),
        "STARFOX_KEEP_SUCCESS_BRANCH_TARGET" => {
            cached_env_presence!("STARFOX_KEEP_SUCCESS_BRANCH_TARGET")
        }
        "STARFOX_FORCE_SUCCESS_BRANCH_TO_B196" => {
            cached_env_presence!("STARFOX_FORCE_SUCCESS_BRANCH_TO_B196")
        }
        "STARFOX_NULL_AC98_AFTER_SUCCESS" => {
            cached_env_presence!("STARFOX_NULL_AC98_AFTER_SUCCESS")
        }
        "STARFOX_FORCE_CONTINUATION_CURSOR_FROM_MATCH_FRAGMENT" => {
            cached_env_presence!("STARFOX_FORCE_CONTINUATION_CURSOR_FROM_MATCH_FRAGMENT")
        }
        "STARFOX_NULL_CONTINUATION_AFTER_SUCCESS" => {
            cached_env_presence!("STARFOX_NULL_CONTINUATION_AFTER_SUCCESS")
        }
        "STARFOX_FORCE_CONTINUATION_PTR_FROM_MATCH_FRAGMENT" => {
            cached_env_presence!("STARFOX_FORCE_CONTINUATION_PTR_FROM_MATCH_FRAGMENT")
        }
        "TRACE_PLOT_COUNT" => cached_env_presence!("TRACE_PLOT_COUNT"),
        "TRACE_SUPERFX_IWT" => cached_env_presence!("TRACE_SUPERFX_IWT"),
        "TRACE_SUPERFX_R0_WRITES" => cached_env_presence!("TRACE_SUPERFX_R0_WRITES"),
        "TRACE_SUPERFX_R4_WRITES" => cached_env_presence!("TRACE_SUPERFX_R4_WRITES"),
        "TRACE_SUPERFX_R7_WRITES" => cached_env_presence!("TRACE_SUPERFX_R7_WRITES"),
        "TRACE_SUPERFX_R9_WRITES" => cached_env_presence!("TRACE_SUPERFX_R9_WRITES"),
        "TRACE_SUPERFX_R10_WRITES" => cached_env_presence!("TRACE_SUPERFX_R10_WRITES"),
        "TRACE_SUPERFX_R12_WRITES" => cached_env_presence!("TRACE_SUPERFX_R12_WRITES"),
        "TRACE_SFX_RAM_WRITES" => cached_env_presence!("TRACE_SFX_RAM_WRITES"),
        "TRACE_CACHE_FLUSH" => cached_env_presence!("TRACE_CACHE_FLUSH"),
        "TRACE_GRAM_LINEAR_W" => cached_env_presence!("TRACE_GRAM_LINEAR_W"),
        _ => std::env::var_os(name).is_some(),
    }
}

fn trace_superfx_sfr_enabled() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| env_flag_enabled("TRACE_SUPERFX_SFR"))
}

fn trace_superfx_start_enabled() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| env_flag_enabled("TRACE_SUPERFX_START"))
}

fn trace_superfx_unimpl_enabled() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| env_flag_enabled("TRACE_SUPERFX_UNIMPL"))
}

fn trace_superfx_plot_enabled() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| env_flag_enabled("TRACE_SUPERFX_PLOT"))
}

fn trace_superfx_profile_enabled() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| env_flag_enabled("TRACE_SUPERFX_PROFILE"))
}

fn enable_experimental_starfox_fastpaths() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| env_flag_enabled("ENABLE_EXPERIMENTAL_STARFOX_FASTPATHS"))
}

fn trace_superfx_pc_trace_enabled() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| env_flag_enabled("TRACE_SUPERFX_PC_TRACE"))
}

fn trace_superfx_reg_flow_enabled() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| {
        env_flag_enabled("TRACE_SUPERFX_REG_FLOW") || env_flag_enabled("TRACE_SUPERFX_LAST_WRITERS")
    })
}

fn trace_superfx_last_transfers_enabled() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| {
        cfg!(test)
            || env_flag_enabled("TRACE_SUPERFX_LAST_TRANSFERS")
            || trace_superfx_pc_trace_enabled()
            || trace_superfx_reg_flow_enabled()
            || trace_superfx_exec_at_frame().is_some()
    })
}

fn trace_superfx_low_ram_writes_enabled() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| {
        cfg!(test)
            || env_flag_enabled("TRACE_SUPERFX_LOW_RAM")
            || std::env::var_os("TRACE_SUPERFX_LOW_RAM_WORDS").is_some()
            || trace_superfx_exec_at_frame().is_some()
    })
}

fn trace_superfx_reg_flow_filter() -> &'static Option<[bool; GSU_REGISTER_COUNT]> {
    static FILTER: OnceLock<Option<[bool; GSU_REGISTER_COUNT]>> = OnceLock::new();
    FILTER.get_or_init(|| {
        let raw = std::env::var("TRACE_SUPERFX_REG_FILTER").ok()?;
        let mut regs = [false; GSU_REGISTER_COUNT];
        let mut any = false;
        for part in raw.split(',') {
            let part = part.trim();
            if part.is_empty() {
                continue;
            }
            let part = part.strip_prefix('r').unwrap_or(part);
            let Ok(reg) = part.parse::<usize>() else {
                continue;
            };
            if reg < GSU_REGISTER_COUNT {
                regs[reg] = true;
                any = true;
            }
        }
        any.then_some(regs)
    })
}

fn trace_superfx_reg_history_cap() -> usize {
    static CAP: OnceLock<usize> = OnceLock::new();
    *CAP.get_or_init(|| {
        std::env::var("TRACE_SUPERFX_REG_HISTORY_CAP")
            .ok()
            .and_then(|raw| raw.trim().parse::<usize>().ok())
            .filter(|&n| n > 0)
            .unwrap_or(MAX_RECENT_REG_WRITES_PER_REG)
    })
}

fn trace_superfx_reg_flow_exclude_range() -> &'static Option<(u8, u16, u16)> {
    static RANGE: OnceLock<Option<(u8, u16, u16)>> = OnceLock::new();
    RANGE.get_or_init(|| {
        let raw = std::env::var("TRACE_SUPERFX_REG_EXCLUDE_RANGE").ok()?;
        let (bank, range) = raw.split_once(':')?;
        let bank = u8::from_str_radix(bank.trim_start_matches("0x"), 16).ok()?;
        let (start, end) = range.split_once('-')?;
        let start = u16::from_str_radix(start.trim_start_matches("0x"), 16).ok()?;
        let end = u16::from_str_radix(end.trim_start_matches("0x"), 16).ok()?;
        Some((bank & 0x7F, start.min(end), start.max(end)))
    })
}

fn trace_superfx_getb_enabled() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| env_flag_enabled("TRACE_SUPERFX_GETB"))
}

fn trace_superfx_screen_words_enabled() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| env_flag_enabled("TRACE_SUPERFX_SCREEN_WORDS"))
}

fn trace_superfx_screen_bytes_enabled() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| env_flag_enabled("TRACE_SUPERFX_SCREEN_BYTES"))
}

fn trace_superfx_stop_captures_enabled() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| env_flag_enabled("TRACE_SUPERFX_STOP_CAPTURES"))
}

fn trace_superfx_pc_last_writers_enabled() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| env_flag_enabled("TRACE_SUPERFX_PC_LAST_WRITERS"))
}

fn trace_superfx_exec_at_frame() -> Option<u32> {
    static VALUE: OnceLock<Option<u32>> = OnceLock::new();
    *VALUE.get_or_init(|| {
        std::env::var("TRACE_SUPERFX_EXEC_AT_FRAME")
            .ok()
            .and_then(|raw| raw.trim().parse::<u32>().ok())
    })
}

fn save_state_at_superfx_ram_addr_hit_index() -> u32 {
    static VALUE: OnceLock<u32> = OnceLock::new();
    *VALUE.get_or_init(|| {
        std::env::var("SAVE_STATE_AT_SUPERFX_RAM_ADDR_HIT_INDEX")
            .ok()
            .and_then(|raw| raw.trim().parse::<u32>().ok())
            .filter(|&n| n > 0)
            .unwrap_or(1)
    })
}

fn trace_superfx_exec_frame_matches(frame: u64) -> bool {
    trace_superfx_exec_at_frame().is_none_or(|target| frame == u64::from(target))
}

pub fn set_trace_superfx_exec_frame(frame: u64) {
    TRACE_SUPERFX_EXEC_FRAME.store(frame.min(u64::from(u32::MAX)) as u32, Ordering::Relaxed);
}

#[inline]
fn current_trace_superfx_frame() -> u32 {
    TRACE_SUPERFX_EXEC_FRAME.load(Ordering::Relaxed)
}

fn trace_superfx_matches_current_frame(name: &'static str) -> bool {
    let Some(target) = std::env::var(name)
        .ok()
        .and_then(|raw| raw.trim().parse::<u32>().ok())
    else {
        return true;
    };
    current_trace_superfx_frame() == target
}

fn trace_superfx_pc_range_raw() -> &'static Option<(u8, u16, u16)> {
    static RANGE: OnceLock<Option<(u8, u16, u16)>> = OnceLock::new();
    RANGE.get_or_init(|| {
        let raw = std::env::var("TRACE_SUPERFX_PC_RANGE_RAW").ok()?;
        let (bank, range) = raw.split_once(':')?;
        let bank = u8::from_str_radix(bank.trim_start_matches("0x"), 16).ok()?;
        let (start, end) = range.split_once('-')?;
        let start = u16::from_str_radix(start.trim_start_matches("0x"), 16).ok()?;
        let end = u16::from_str_radix(end.trim_start_matches("0x"), 16).ok()?;
        Some((bank & 0x7F, start.min(end), start.max(end)))
    })
}

fn trace_superfx_pc_range_post() -> &'static Option<(u8, u16, u16)> {
    static RANGE: OnceLock<Option<(u8, u16, u16)>> = OnceLock::new();
    RANGE.get_or_init(|| {
        let raw = std::env::var("TRACE_SUPERFX_PC_RANGE_POST").ok()?;
        let (bank, range) = raw.split_once(':')?;
        let bank = u8::from_str_radix(bank.trim_start_matches("0x"), 16).ok()?;
        let (start, end) = range.split_once('-')?;
        let start = u16::from_str_radix(start.trim_start_matches("0x"), 16).ok()?;
        let end = u16::from_str_radix(end.trim_start_matches("0x"), 16).ok()?;
        Some((bank & 0x7F, start.min(end), start.max(end)))
    })
}

fn trace_superfx_pc_range_raw_matches(pbr: u8, pc: u16) -> bool {
    trace_superfx_pc_range_raw()
        .as_ref()
        .is_some_and(|&(bank, start, end)| pbr == bank && pc >= start && pc <= end)
}

fn save_state_at_gsu_pc_range() -> &'static Option<(u8, u16, u16)> {
    static RANGE: OnceLock<Option<(u8, u16, u16)>> = OnceLock::new();
    RANGE.get_or_init(|| {
        let raw = std::env::var("SAVE_STATE_AT_GSU_PC_RANGE").ok()?;
        let (bank, range) = raw.split_once(':')?;
        let bank = u8::from_str_radix(bank.trim_start_matches("0x"), 16).ok()?;
        let (start, end) = range.split_once('-')?;
        let start = u16::from_str_radix(start.trim_start_matches("0x"), 16).ok()?;
        let end = u16::from_str_radix(end.trim_start_matches("0x"), 16).ok()?;
        Some((bank & 0x7F, start.min(end), start.max(end)))
    })
}

fn save_state_at_gsu_pc_hit_index() -> u32 {
    static INDEX: OnceLock<u32> = OnceLock::new();
    *INDEX.get_or_init(|| {
        std::env::var("SAVE_STATE_AT_GSU_PC_HIT_INDEX")
            .ok()
            .and_then(|raw| raw.trim().parse::<u32>().ok())
            .filter(|&value| value > 0)
            .unwrap_or(1)
    })
}

fn parse_gsu_exec_pc(raw: &str) -> Option<(u8, u16)> {
    let (bank, pc) = raw.split_once(':')?;
    let bank = u8::from_str_radix(
        bank.trim()
            .trim_start_matches("0x")
            .trim_start_matches("0X"),
        16,
    )
    .ok()?;
    let pc = u16::from_str_radix(
        pc.trim().trim_start_matches("0x").trim_start_matches("0X"),
        16,
    )
    .ok()?;
    Some((bank & 0x7F, pc))
}

fn parse_save_state_gsu_recent_exec_tail(raw: &str) -> Option<Vec<(u8, u16)>> {
    let mut items = Vec::new();
    for part in raw.split(',').map(str::trim).filter(|s| !s.is_empty()) {
        items.push(parse_gsu_exec_pc(part)?);
    }
    (!items.is_empty()).then_some(items)
}

fn recent_exec_trace_ends_with(trace: &[SuperFxExecTrace], tail: &[(u8, u16)]) -> bool {
    if tail.len() > trace.len() {
        return false;
    }
    trace[trace.len() - tail.len()..]
        .iter()
        .zip(tail.iter())
        .all(|(entry, &(pbr, pc))| entry.pbr == pbr && entry.pc == pc)
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct SaveStateGsuRegEq {
    reg: u8,
    value: u16,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct SaveStateSuperfxRamByteEq {
    addr: u16,
    value: u8,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct SaveStateSuperfxRamWordEq {
    addr: u16,
    value: u16,
}

fn parse_save_state_gsu_reg_eq(raw: &str) -> Option<Vec<SaveStateGsuRegEq>> {
    let mut items = Vec::new();
    for part in raw.split(',').map(str::trim).filter(|s| !s.is_empty()) {
        let (reg_raw, value_raw) = part.split_once('=').or_else(|| part.split_once(':'))?;
        let reg_raw = reg_raw.trim();
        let reg_raw = reg_raw
            .strip_prefix('r')
            .or_else(|| reg_raw.strip_prefix('R'))
            .unwrap_or(reg_raw);
        let reg = reg_raw.parse::<usize>().ok()?;
        if reg >= GSU_REGISTER_COUNT {
            return None;
        }
        let value_raw = value_raw.trim();
        let value = parse_trace_u16_env(value_raw).or_else(|| {
            u16::from_str_radix(
                value_raw.trim_start_matches("0x").trim_start_matches("0X"),
                16,
            )
            .ok()
        })?;
        items.push(SaveStateGsuRegEq {
            reg: reg as u8,
            value,
        });
    }
    (!items.is_empty()).then_some(items)
}

fn save_state_at_gsu_reg_eq() -> &'static Option<Vec<SaveStateGsuRegEq>> {
    static VALUE: OnceLock<Option<Vec<SaveStateGsuRegEq>>> = OnceLock::new();
    VALUE.get_or_init(|| {
        let raw = std::env::var("SAVE_STATE_AT_GSU_REG_EQ").ok()?;
        parse_save_state_gsu_reg_eq(&raw)
    })
}

fn save_state_at_gsu_reg_write() -> &'static Option<Vec<SaveStateGsuRegEq>> {
    static VALUE: OnceLock<Option<Vec<SaveStateGsuRegEq>>> = OnceLock::new();
    VALUE.get_or_init(|| {
        let raw = std::env::var("SAVE_STATE_AT_GSU_REG_WRITE").ok()?;
        parse_save_state_gsu_reg_eq(&raw)
    })
}

fn save_state_at_gsu_reg_eq_matches(gsu: &SuperFx) -> bool {
    save_state_at_gsu_reg_eq().as_ref().is_none_or(|items| {
        items
            .iter()
            .all(|item| gsu.regs[item.reg as usize] == item.value)
    })
}

fn save_state_at_gsu_recent_exec_tail() -> &'static Option<Vec<(u8, u16)>> {
    static VALUE: OnceLock<Option<Vec<(u8, u16)>>> = OnceLock::new();
    VALUE.get_or_init(|| {
        let raw = std::env::var("SAVE_STATE_AT_GSU_RECENT_EXEC_TAIL").ok()?;
        parse_save_state_gsu_recent_exec_tail(&raw)
    })
}

fn save_state_at_gsu_recent_exec_tail_matches(gsu: &SuperFx) -> bool {
    save_state_at_gsu_recent_exec_tail()
        .as_ref()
        .is_none_or(|tail| recent_exec_trace_ends_with(&gsu.recent_exec_trace, tail))
}

fn parse_save_state_superfx_ram_byte_eq(raw: &str) -> Option<Vec<SaveStateSuperfxRamByteEq>> {
    let mut items = Vec::new();
    for part in raw.split(',').map(str::trim).filter(|s| !s.is_empty()) {
        let (addr_raw, value_raw) = part.split_once('=').or_else(|| part.split_once(':'))?;
        let addr = parse_trace_u16_env(addr_raw.trim())?;
        let value_raw = value_raw.trim();
        let value = u8::from_str_radix(
            value_raw.trim_start_matches("0x").trim_start_matches("0X"),
            16,
        )
        .ok()?;
        items.push(SaveStateSuperfxRamByteEq { addr, value });
    }
    (!items.is_empty()).then_some(items)
}

fn save_state_at_superfx_ram_byte_eq() -> &'static Option<Vec<SaveStateSuperfxRamByteEq>> {
    static VALUE: OnceLock<Option<Vec<SaveStateSuperfxRamByteEq>>> = OnceLock::new();
    VALUE.get_or_init(|| {
        let raw = std::env::var("SAVE_STATE_AT_SUPERFX_RAM_BYTE_EQ").ok()?;
        parse_save_state_superfx_ram_byte_eq(&raw)
    })
}

fn save_state_at_superfx_ram_byte_eq_matches(addr: u16, value: u8) -> bool {
    save_state_at_superfx_ram_byte_eq()
        .as_ref()
        .is_none_or(|items| {
            items
                .iter()
                .any(|item| item.addr == addr && item.value == value)
        })
}

fn parse_save_state_superfx_ram_word_eq(raw: &str) -> Option<Vec<SaveStateSuperfxRamWordEq>> {
    let mut items = Vec::new();
    for part in raw.split(',').map(str::trim).filter(|s| !s.is_empty()) {
        let (addr_raw, value_raw) = part.split_once('=').or_else(|| part.split_once(':'))?;
        let addr = parse_trace_u16_env(addr_raw.trim())?;
        let value = parse_trace_u16_env(value_raw.trim())?;
        items.push(SaveStateSuperfxRamWordEq { addr, value });
    }
    (!items.is_empty()).then_some(items)
}

fn save_state_at_superfx_ram_word_eq() -> &'static Option<Vec<SaveStateSuperfxRamWordEq>> {
    static VALUE: OnceLock<Option<Vec<SaveStateSuperfxRamWordEq>>> = OnceLock::new();
    VALUE.get_or_init(|| {
        let raw = std::env::var("SAVE_STATE_AT_SUPERFX_RAM_WORD_EQ").ok()?;
        parse_save_state_superfx_ram_word_eq(&raw)
    })
}

fn save_state_at_superfx_ram_addr_config() -> &'static Option<TraceSuperfxRamAddrConfig> {
    static VALUE: OnceLock<Option<TraceSuperfxRamAddrConfig>> = OnceLock::new();
    VALUE.get_or_init(|| {
        if let Ok(range) = std::env::var("SAVE_STATE_AT_SUPERFX_RAM_ADDR_RANGE") {
            let (start, end) = range.split_once('-')?;
            let start_addr = parse_trace_u16_env(start.trim())?;
            let end_addr = parse_trace_u16_env(end.trim())?;
            Some(TraceSuperfxRamAddrConfig::Range {
                start_addr: start_addr.min(end_addr),
                end_addr: start_addr.max(end_addr),
            })
        } else {
            let value = std::env::var("SAVE_STATE_AT_SUPERFX_RAM_ADDRS").ok()?;
            let addrs = value
                .split(',')
                .filter_map(|raw| {
                    let raw = raw.trim();
                    if raw.is_empty() {
                        None
                    } else {
                        parse_trace_u16_env(raw)
                    }
                })
                .collect::<Vec<_>>();
            (!addrs.is_empty()).then_some(TraceSuperfxRamAddrConfig::List(addrs))
        }
    })
}

fn save_state_at_superfx_ram_addr_matches(addr: u16) -> bool {
    save_state_at_superfx_ram_addr_config()
        .as_ref()
        .is_some_and(|cfg| match cfg {
            TraceSuperfxRamAddrConfig::Range {
                start_addr,
                end_addr,
            } => addr >= *start_addr && addr <= *end_addr,
            TraceSuperfxRamAddrConfig::List(addrs) => addrs.contains(&addr),
        })
}

fn parse_env_u16(raw: &str) -> Option<u16> {
    let trimmed = raw.trim();
    if let Some(hex) = trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"))
    {
        u16::from_str_radix(hex, 16).ok()
    } else if trimmed
        .bytes()
        .any(|b| matches!(b, b'a'..=b'f' | b'A'..=b'F'))
    {
        u16::from_str_radix(trimmed, 16).ok()
    } else {
        trimmed.parse::<u16>().ok()
    }
}

fn env_u16_direct(name: &'static str) -> Option<u16> {
    std::env::var(name).ok().and_then(|raw| parse_env_u16(&raw))
}

fn env_u16(name: &'static str) -> Option<u16> {
    if cfg!(test) {
        return env_u16_direct(name);
    }

    match name {
        "STARFOX_FORCE_B30A_R14_VALUE" => cached_env_u16!("STARFOX_FORCE_B30A_R14_VALUE"),
        "STARFOX_FORCE_B30A_R14_FRAME" => cached_env_u16!("STARFOX_FORCE_B30A_R14_FRAME"),
        "STARFOX_FORCE_B380_R12_VALUE" => cached_env_u16!("STARFOX_FORCE_B380_R12_VALUE"),
        "STARFOX_FORCE_B380_R12_FRAME" => cached_env_u16!("STARFOX_FORCE_B380_R12_FRAME"),
        "STARFOX_FORCE_B384_PREEXEC_FRAME" => {
            cached_env_u16!("STARFOX_FORCE_B384_PREEXEC_FRAME")
        }
        "STARFOX_FORCE_B384_PREEXEC_R12_VALUE" => {
            cached_env_u16!("STARFOX_FORCE_B384_PREEXEC_R12_VALUE")
        }
        "STARFOX_FORCE_B384_PREEXEC_R14_VALUE" => {
            cached_env_u16!("STARFOX_FORCE_B384_PREEXEC_R14_VALUE")
        }
        _ => env_u16_direct(name),
    }
}

fn any_env_present(names: &[&'static str]) -> bool {
    names.iter().any(|name| std::env::var_os(name).is_some())
}

fn starfox_reg_write_debug_override_enabled() -> bool {
    if cfg!(test) {
        return true;
    }
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| {
        any_env_present(&[
            "STARFOX_FORCE_B30A_R14_VALUE",
            "STARFOX_FORCE_B30A_R14_FRAME",
            "STARFOX_FORCE_B380_R12_VALUE",
            "STARFOX_FORCE_B380_R12_FRAME",
            "STARFOX_KEEP_SUCCESS_BRANCH_TARGET",
            "STARFOX_KEEP_SUCCESS_CONTEXT",
            "STARFOX_FORCE_SUCCESS_BRANCH_TO_B196",
            "STARFOX_NULL_AC98_AFTER_SUCCESS",
        ])
    })
}

fn starfox_b384_preexec_debug_override_enabled() -> bool {
    if cfg!(test) {
        return true;
    }
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| {
        any_env_present(&[
            "STARFOX_FORCE_B384_PREEXEC_FRAME",
            "STARFOX_FORCE_B384_PREEXEC_R12_VALUE",
            "STARFOX_FORCE_B384_PREEXEC_R14_VALUE",
        ])
    })
}

fn starfox_ram_write_debug_override_enabled() -> bool {
    if cfg!(test) {
        return true;
    }
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| {
        any_env_present(&[
            "STARFOX_FORCE_LATE_SEARCH_KEY_FROM_ANY_TABLE_FIELD",
            "STARFOX_FORCE_LATE_SEARCH_KEY_FROM_1AE2",
            "STARFOX_FORCE_PARSER_KEY_FROM_ANY_TABLE_FIELD",
            "STARFOX_FORCE_PARSER_KEY_FROM_MATCH_WORD",
            "STARFOX_KEEP_SUCCESS_CURSOR_ARMED",
            "STARFOX_KEEP_SUCCESS_CONTEXT",
            "STARFOX_FORCE_CONTINUATION_CURSOR_FROM_MATCH_FRAGMENT",
            "STARFOX_FORCE_CONTINUATION_CURSOR_VALUE",
            "STARFOX_NULL_CONTINUATION_AFTER_SUCCESS",
            "STARFOX_FORCE_CONTINUATION_PTR_FROM_MATCH_FRAGMENT",
        ])
    })
}

fn trace_superfx_reg_write_prints_enabled() -> bool {
    if cfg!(test) {
        return any_env_present(&[
            "TRACE_SUPERFX_R0_WRITES",
            "TRACE_SUPERFX_R4_WRITES",
            "TRACE_SUPERFX_R7_WRITES",
            "TRACE_SUPERFX_R9_WRITES",
            "TRACE_SUPERFX_R10_WRITES",
            "TRACE_SUPERFX_R12_WRITES",
        ]);
    }
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| {
        any_env_present(&[
            "TRACE_SUPERFX_R0_WRITES",
            "TRACE_SUPERFX_R4_WRITES",
            "TRACE_SUPERFX_R7_WRITES",
            "TRACE_SUPERFX_R9_WRITES",
            "TRACE_SUPERFX_R10_WRITES",
            "TRACE_SUPERFX_R12_WRITES",
        ])
    })
}

fn env_u8(name: &'static str) -> Option<u8> {
    std::env::var(name).ok().and_then(|raw| {
        let trimmed = raw.trim();
        if let Some(hex) = trimmed
            .strip_prefix("0x")
            .or_else(|| trimmed.strip_prefix("0X"))
        {
            u8::from_str_radix(hex, 16).ok()
        } else if trimmed
            .bytes()
            .any(|b| matches!(b, b'a'..=b'f' | b'A'..=b'F'))
        {
            u8::from_str_radix(trimmed, 16).ok()
        } else {
            trimmed.parse::<u8>().ok()
        }
    })
}

fn starfox_force_continuation_cursor_value() -> Option<u16> {
    fn parse() -> Option<u16> {
        std::env::var("STARFOX_FORCE_CONTINUATION_CURSOR_VALUE")
            .ok()
            .and_then(|raw| {
                let token = raw.trim().trim_start_matches("0x").trim_start_matches("0X");
                u16::from_str_radix(token, 16)
                    .ok()
                    .or_else(|| raw.trim().parse::<u16>().ok())
            })
    }

    if cfg!(test) {
        parse()
    } else {
        static VALUE: OnceLock<Option<u16>> = OnceLock::new();
        *VALUE.get_or_init(parse)
    }
}

fn superfx_screen_buffer_stop_pc_filter() -> Option<u16> {
    static VALUE: OnceLock<Option<u16>> = OnceLock::new();
    *VALUE.get_or_init(|| env_u16("SUPERFX_SCREEN_BUFFER_STOP_PC"))
}

fn superfx_tile_snapshot_pc_filter() -> Option<u16> {
    static VALUE: OnceLock<Option<u16>> = OnceLock::new();
    *VALUE.get_or_init(|| {
        std::env::var("SUPERFX_TILE_SNAPSHOT_PC")
            .ok()
            .and_then(|raw| u16::from_str_radix(raw.trim_start_matches("0x"), 16).ok())
    })
}

fn superfx_tile_snapshot_rev_index() -> usize {
    static VALUE: OnceLock<usize> = OnceLock::new();
    *VALUE.get_or_init(|| {
        std::env::var("SUPERFX_TILE_SNAPSHOT_REV_INDEX")
            .ok()
            .and_then(|raw| raw.parse::<usize>().ok())
            .unwrap_or(0)
    })
}

fn superfx_screen_buffer_capture_pc_filter() -> Option<u16> {
    static VALUE: OnceLock<Option<u16>> = OnceLock::new();
    *VALUE.get_or_init(|| env_u16("SUPERFX_SCREEN_BUFFER_CAPTURE_PC"))
}

fn superfx_screen_buffer_capture_pbr_filter() -> Option<u8> {
    static VALUE: OnceLock<Option<u8>> = OnceLock::new();
    *VALUE.get_or_init(|| env_u8("SUPERFX_SCREEN_BUFFER_CAPTURE_PBR"))
}

fn trace_superfx_tile_captures_enabled() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| env_flag_enabled("TRACE_SUPERFX_TILE_CAPTURES"))
}

fn trace_superfx_display_captures_enabled() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| env_flag_enabled("TRACE_SUPERFX_DISPLAY_CAPTURES"))
}

fn superfx_screen_buffer_stop_pbr_filter() -> Option<u8> {
    static VALUE: OnceLock<Option<u8>> = OnceLock::new();
    *VALUE.get_or_init(|| env_u8("SUPERFX_SCREEN_BUFFER_STOP_PBR"))
}

fn superfx_dma_uses_latest_stop_snapshot() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| {
        std::env::var("SUPERFX_DMA_USE_LATEST_STOP")
            .map(|v| v != "0" && !v.eq_ignore_ascii_case("false"))
            .unwrap_or(true)
    })
}

fn superfx_direct_uses_tile_snapshot() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| {
        std::env::var("SUPERFX_DIRECT_USE_TILE")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false)
    })
}

fn env_usize(name: &'static str) -> Option<usize> {
    std::env::var(name).ok().and_then(|raw| {
        let trimmed = raw.trim();
        if let Some(hex) = trimmed
            .strip_prefix("0x")
            .or_else(|| trimmed.strip_prefix("0X"))
        {
            usize::from_str_radix(hex, 16).ok()
        } else {
            trimmed.parse::<usize>().ok()
        }
    })
}

fn trace_superfx_screen_idx_min() -> Option<usize> {
    static VALUE: OnceLock<Option<usize>> = OnceLock::new();
    *VALUE.get_or_init(|| env_usize("TRACE_SUPERFX_SCREEN_IDX_MIN"))
}

fn trace_superfx_screen_idx_max() -> Option<usize> {
    static VALUE: OnceLock<Option<usize>> = OnceLock::new();
    *VALUE.get_or_init(|| env_usize("TRACE_SUPERFX_SCREEN_IDX_MAX"))
}

fn trace_superfx_screen_idx_matches(idx: usize) -> bool {
    if let Some(min) = trace_superfx_screen_idx_min() {
        if idx < min {
            return false;
        }
    }
    if let Some(max) = trace_superfx_screen_idx_max() {
        if idx > max {
            return false;
        }
    }
    true
}

fn parse_trace_u16_env(value: &str) -> Option<u16> {
    if let Some(hex) = value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
    {
        u16::from_str_radix(hex, 16).ok()
    } else {
        value.trim().parse::<u16>().ok()
    }
}

fn trace_superfx_ram_addr_config() -> &'static Option<TraceSuperfxRamAddrConfig> {
    static VALUE: OnceLock<Option<TraceSuperfxRamAddrConfig>> = OnceLock::new();
    VALUE.get_or_init(|| {
        if let Ok(range) = std::env::var("TRACE_SUPERFX_RAM_ADDR_RANGE") {
            let (start, end) = range.split_once('-')?;
            let start_addr = parse_trace_u16_env(start.trim())?;
            let end_addr = parse_trace_u16_env(end.trim())?;
            Some(TraceSuperfxRamAddrConfig::Range {
                start_addr: start_addr.min(end_addr),
                end_addr: start_addr.max(end_addr),
            })
        } else {
            let value = std::env::var("TRACE_SUPERFX_RAM_ADDRS").ok()?;
            let addrs = value
                .split(',')
                .filter_map(|raw| {
                    let raw = raw.trim();
                    if raw.is_empty() {
                        None
                    } else {
                        parse_trace_u16_env(raw)
                    }
                })
                .collect::<Vec<_>>();
            (!addrs.is_empty()).then_some(TraceSuperfxRamAddrConfig::List(addrs))
        }
    })
}

fn trace_superfx_ram_addr_matches(addr: u16) -> bool {
    trace_superfx_ram_addr_config()
        .as_ref()
        .is_some_and(|cfg| match cfg {
            TraceSuperfxRamAddrConfig::Range {
                start_addr,
                end_addr,
            } => addr >= *start_addr && addr <= *end_addr,
            TraceSuperfxRamAddrConfig::List(addrs) => addrs.contains(&addr),
        })
}

pub fn debug_trace_superfx_ram_addr_matches(addr: usize) -> bool {
    trace_superfx_ram_addr_matches(addr as u16)
}

pub fn debug_trace_superfx_ram_addr_matches_for_frame(addr: usize, frame: u64) -> bool {
    trace_superfx_exec_frame_matches(frame) && trace_superfx_ram_addr_matches(addr as u16)
}

pub fn debug_current_trace_superfx_exec_frame() -> u32 {
    current_trace_superfx_frame()
}

#[derive(Clone, Serialize, Deserialize)]
pub struct SuperFxSaveData {
    pub regs: [u16; GSU_REGISTER_COUNT],
    pub sfr: u16,
    #[serde(default)]
    pub shadow_sign: Option<u16>,
    #[serde(default)]
    pub shadow_zero: Option<u16>,
    #[serde(default)]
    pub shadow_carry: Option<bool>,
    #[serde(default)]
    pub shadow_overflow: Option<bool>,
    #[serde(default)]
    pub running: bool,
    #[serde(default)]
    pub src_reg: u8,
    #[serde(default)]
    pub dst_reg: u8,
    #[serde(default)]
    pub with_reg: u8,
    pub pbr: u8,
    pub rombr: u8,
    pub rambr: u8,
    pub cbr: u16,
    #[serde(default)]
    pub cache_enabled: bool,
    #[serde(default)]
    pub cache_valid_mask: u32,
    pub scbr: u8,
    pub scmr: u8,
    pub cfgr: u8,
    pub clsr: u8,
    pub bramr: u8,
    pub vcr: u8,
    #[serde(default)]
    pub colr: u8,
    #[serde(default)]
    pub por: u8,
    #[serde(default)]
    pub last_ram_addr: u16,
    #[serde(default)]
    pub ram_buffer_pending: bool,
    #[serde(default)]
    pub ram_buffer_pending_bank: u8,
    #[serde(default)]
    pub ram_buffer_pending_addr: u16,
    #[serde(default)]
    pub ram_buffer_pending_data: u8,
    #[serde(default)]
    pub rom_buffer: u8,
    #[serde(default)]
    pub rom_buffer_valid: bool,
    #[serde(default)]
    pub rom_buffer_pending: bool,
    #[serde(default)]
    pub rom_buffer_pending_bank: u8,
    #[serde(default)]
    pub rom_buffer_pending_addr: u16,
    #[serde(default)]
    pub pending_delay_pc: Option<u16>,
    #[serde(default)]
    pub pending_delay_pbr: Option<u8>,
    #[serde(default)]
    pub pending_delay_cache_base: Option<u16>,
    #[serde(default)]
    pub r14_modified: bool,
    #[serde(default)]
    pub r15_modified: bool,
    #[serde(default = "default_superfx_pipe")]
    pub pipe: u8,
    #[serde(default)]
    pub pipe_valid: bool,
    #[serde(default)]
    pub pipe_pc: Option<u16>,
    #[serde(default)]
    pub pipe_pbr: Option<u8>,
    #[serde(default)]
    pub tile_snapshot: Vec<u8>,
    #[serde(default)]
    pub tile_snapshot_valid: bool,
    #[serde(default)]
    pub tile_snapshot_scbr: u8,
    #[serde(default)]
    pub tile_snapshot_height: u16,
    #[serde(default)]
    pub tile_snapshot_bpp: u8,
    #[serde(default)]
    pub tile_snapshot_mode: u8,
    #[serde(default)]
    pub tile_snapshot_pc: u16,
    #[serde(default)]
    pub tile_snapshot_pbr: u8,
    #[serde(default)]
    pub latest_stop_snapshot: Vec<u8>,
    #[serde(default)]
    pub latest_stop_snapshot_valid: bool,
    #[serde(default)]
    pub latest_stop_scbr: u8,
    #[serde(default)]
    pub latest_stop_height: u16,
    #[serde(default)]
    pub latest_stop_bpp: u8,
    #[serde(default)]
    pub latest_stop_mode: u8,
    #[serde(default)]
    pub latest_stop_pc: u16,
    #[serde(default)]
    pub latest_stop_pbr: u8,
    #[serde(default)]
    pub display_snapshot: Vec<u8>,
    #[serde(default)]
    pub display_snapshot_valid: bool,
    #[serde(default)]
    pub display_snapshot_scbr: u8,
    #[serde(default)]
    pub display_snapshot_height: u16,
    #[serde(default)]
    pub display_snapshot_bpp: u8,
    #[serde(default)]
    pub display_snapshot_mode: u8,
    #[serde(default)]
    recent_stop_snapshots: Vec<StopSnapshot>,
    #[serde(default)]
    recent_tile_snapshots: Vec<StopSnapshot>,
    #[serde(default = "default_superfx_last_reg_writes")]
    pub last_nontrivial_reg_writes: [Option<SuperFxRegWrite>; GSU_REGISTER_COUNT],
    #[serde(default = "default_superfx_recent_reg_writes")]
    pub recent_nontrivial_reg_writes: [Vec<SuperFxRegWrite>; GSU_REGISTER_COUNT],
    #[serde(default = "default_superfx_last_reg_writes")]
    pub last_reg_writes: [Option<SuperFxRegWrite>; GSU_REGISTER_COUNT],
    #[serde(default = "default_superfx_recent_reg_writes")]
    pub recent_reg_writes_by_reg: [Vec<SuperFxRegWrite>; GSU_REGISTER_COUNT],
    #[serde(default)]
    pub recent_reg_writes: Vec<SuperFxRegWrite>,
    #[serde(default = "default_superfx_last_low_ram_writes")]
    pub last_low_ram_writes: Vec<Option<SuperFxRamWrite>>,
    pub cache_ram: Vec<u8>,
    pub game_ram: Vec<u8>,
}

const fn default_superfx_pipe() -> u8 {
    0x01
}

fn default_superfx_last_reg_writes() -> [Option<SuperFxRegWrite>; GSU_REGISTER_COUNT] {
    std::array::from_fn(|_| None)
}

fn default_superfx_recent_reg_writes() -> [Vec<SuperFxRegWrite>; GSU_REGISTER_COUNT] {
    std::array::from_fn(|_| Vec::new())
}

fn default_superfx_last_low_ram_writes() -> Vec<Option<SuperFxRamWrite>> {
    vec![None; 0x200]
}

impl Default for SuperFxSaveData {
    fn default() -> Self {
        Self {
            regs: [0; GSU_REGISTER_COUNT],
            sfr: 0,
            shadow_sign: None,
            shadow_zero: None,
            shadow_carry: None,
            shadow_overflow: None,
            running: false,
            src_reg: 0,
            dst_reg: 0,
            with_reg: 0,
            pbr: 0,
            rombr: 0,
            rambr: 0,
            cbr: 0,
            cache_enabled: false,
            cache_valid_mask: 0,
            scbr: 0,
            scmr: 0,
            cfgr: 0,
            clsr: 0,
            bramr: 0,
            vcr: 0,
            colr: 0,
            por: 0,
            last_ram_addr: 0,
            ram_buffer_pending: false,
            ram_buffer_pending_bank: 0,
            ram_buffer_pending_addr: 0,
            ram_buffer_pending_data: 0,
            rom_buffer: 0,
            rom_buffer_valid: false,
            rom_buffer_pending: false,
            rom_buffer_pending_bank: 0,
            rom_buffer_pending_addr: 0,
            pending_delay_pc: None,
            pending_delay_pbr: None,
            pending_delay_cache_base: None,
            r14_modified: false,
            r15_modified: false,
            pipe: default_superfx_pipe(),
            pipe_valid: false,
            pipe_pc: None,
            pipe_pbr: None,
            tile_snapshot: Vec::new(),
            tile_snapshot_valid: false,
            tile_snapshot_scbr: 0,
            tile_snapshot_height: 0,
            tile_snapshot_bpp: 0,
            tile_snapshot_mode: 0,
            tile_snapshot_pc: 0,
            tile_snapshot_pbr: 0,
            latest_stop_snapshot: Vec::new(),
            latest_stop_snapshot_valid: false,
            latest_stop_scbr: 0,
            latest_stop_height: 0,
            latest_stop_bpp: 0,
            latest_stop_mode: 0,
            latest_stop_pc: 0,
            latest_stop_pbr: 0,
            display_snapshot: Vec::new(),
            display_snapshot_valid: false,
            display_snapshot_scbr: 0,
            display_snapshot_height: 0,
            display_snapshot_bpp: 0,
            display_snapshot_mode: 0,
            recent_stop_snapshots: Vec::new(),
            recent_tile_snapshots: Vec::new(),
            last_nontrivial_reg_writes: default_superfx_last_reg_writes(),
            recent_nontrivial_reg_writes: default_superfx_recent_reg_writes(),
            last_reg_writes: default_superfx_last_reg_writes(),
            recent_reg_writes_by_reg: default_superfx_recent_reg_writes(),
            recent_reg_writes: Vec::new(),
            last_low_ram_writes: default_superfx_last_low_ram_writes(),
            cache_ram: vec![0; CACHE_RAM_SIZE],
            game_ram: vec![0; GAME_RAM_SIZE],
        }
    }
}

#[derive(Clone, Debug)]
pub struct SuperFxPcTransfer {
    pub opcode: u8,
    pub pbr: u8,
    pub from_pc: u16,
    pub next_pc: u16,
    pub to_pc: u16,
    pub rombr: u8,
    pub src_reg: u8,
    pub dst_reg: u8,
    pub r12: u16,
    pub r13: u16,
    pub sfr: u16,
    pub repeats: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SuperFxRegWrite {
    pub opcode: u8,
    pub pbr: u8,
    pub pc: u16,
    pub reg: u8,
    pub old_value: u16,
    pub new_value: u16,
    pub src_reg: u8,
    pub dst_reg: u8,
    pub sfr: u16,
    pub repeats: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SuperFxRamWrite {
    pub opcode: u8,
    pub pbr: u8,
    pub pc: u16,
    pub addr: u16,
    pub old_value: u8,
    pub new_value: u8,
    pub src_reg: u8,
    pub dst_reg: u8,
    pub sfr: u16,
    pub r10: u16,
    pub r12: u16,
    pub r14: u16,
    pub r15: u16,
    pub repeats: u32,
}

#[derive(Clone, Debug)]
pub struct SuperFxExecTrace {
    pub opcode: u8,
    pub pbr: u8,
    pub pc: u16,
    pub src_reg: u8,
    pub dst_reg: u8,
    pub sfr: u16,
    pub r0: u16,
    pub r1: u16,
    pub r2: u16,
    pub r3: u16,
    pub r4: u16,
    pub r5: u16,
    pub r6: u16,
    pub r11: u16,
    pub r12: u16,
    pub r13: u16,
    pub r14: u16,
    pub r15: u16,
}

#[derive(Clone, Serialize, Deserialize)]
struct StopSnapshot {
    data: Vec<u8>,
    scbr: u8,
    height: u16,
    bpp: u8,
    mode: u8,
    pc: u16,
    pbr: u8,
}

#[derive(Clone)]
pub struct SuperFx {
    regs: [u16; GSU_REGISTER_COUNT],
    sfr: u16,
    shadow_sign: u16,
    shadow_zero: u16,
    shadow_carry: bool,
    shadow_overflow: bool,
    running: bool,
    src_reg: u8,
    dst_reg: u8,
    with_reg: u8,
    pbr: u8,
    rombr: u8,
    rambr: u8,
    cbr: u16,
    cache_enabled: bool,
    cache_valid_mask: u32,
    scbr: u8,
    scmr: u8,
    cfgr: u8,
    clsr: u8,
    bramr: u8,
    vcr: u8,
    colr: u8,
    por: u8,
    last_ram_addr: u16,
    ram_buffer_pending: bool,
    ram_buffer_pending_bank: u8,
    ram_buffer_pending_addr: u16,
    ram_buffer_pending_data: u8,
    rom_buffer: u8,
    rom_buffer_valid: bool,
    rom_buffer_pending: bool,
    rom_buffer_pending_bank: u8,
    rom_buffer_pending_addr: u16,
    pending_delay_pc: Option<u16>,
    pending_delay_pbr: Option<u8>,
    pending_delay_cache_base: Option<u16>,
    r14_modified: bool,
    r15_modified: bool,
    pipe: u8,
    pipe_valid: bool,
    pipe_pc: u16,
    pipe_pbr: u8,
    cache_ram: [u8; CACHE_RAM_SIZE],
    game_ram: Vec<u8>,
    pixelcache: [PixelCache; 2],
    in_cache_flush: bool,
    /// Snapshot of tile buffer saved after B301 renderer completes.
    /// DMA reads from this instead of live Game RAM to prevent
    /// post-render buffer clear from erasing polygon data.
    tile_snapshot: Vec<u8>,
    tile_snapshot_valid: bool,
    tile_snapshot_scbr: u8,
    tile_snapshot_height: u16,
    tile_snapshot_bpp: u8,
    tile_snapshot_mode: u8,
    tile_snapshot_pc: u16,
    tile_snapshot_pbr: u8,
    latest_stop_snapshot: Vec<u8>,
    latest_stop_snapshot_valid: bool,
    latest_stop_scbr: u8,
    latest_stop_height: u16,
    latest_stop_bpp: u8,
    latest_stop_mode: u8,
    latest_stop_pc: u16,
    latest_stop_pbr: u8,
    display_snapshot: Vec<u8>,
    display_snapshot_valid: bool,
    display_snapshot_scbr: u8,
    display_snapshot_height: u16,
    display_snapshot_bpp: u8,
    display_snapshot_mode: u8,
    debug_pc_snapshot: Option<StopSnapshot>,
    recent_stop_snapshots: Vec<StopSnapshot>,
    recent_tile_snapshots: Vec<StopSnapshot>,
    #[cfg(test)]
    rom_bank_mask: usize,
    exec_profile: [u32; 256],
    exec_profile_by_alt: [[u32; 256]; 4],
    last_opcode_cycles: usize,
    recent_pc_transfers: Vec<SuperFxPcTransfer>,
    recent_reg_writes: Vec<SuperFxRegWrite>,
    last_reg_writes: [Option<SuperFxRegWrite>; GSU_REGISTER_COUNT],
    recent_reg_writes_by_reg: [Vec<SuperFxRegWrite>; GSU_REGISTER_COUNT],
    last_nontrivial_reg_writes: [Option<SuperFxRegWrite>; GSU_REGISTER_COUNT],
    recent_nontrivial_reg_writes: [Vec<SuperFxRegWrite>; GSU_REGISTER_COUNT],
    recent_low_ram_writes: Vec<SuperFxRamWrite>,
    last_low_ram_writes: [Option<SuperFxRamWrite>; 0x200],
    recent_exec_trace: Vec<SuperFxExecTrace>,
    current_exec_pbr: u8,
    current_exec_pc: u16,
    current_exec_opcode: u8,
    save_state_pc_hit: Option<(u8, u16)>,
    save_state_pc_hit_count: u32,
    save_state_ram_addr_hit: Option<(u8, u16, u16)>,
    save_state_ram_addr_hit_count: u32,
    total_run_instructions: u64,
}

impl SuperFx {
    fn latest_stop_snapshot_matches_filters(&self) -> bool {
        let pbr_ok =
            superfx_screen_buffer_stop_pbr_filter().is_none_or(|pbr| pbr == self.latest_stop_pbr);
        let pc_ok =
            superfx_screen_buffer_stop_pc_filter().is_none_or(|pc| pc == self.latest_stop_pc);
        pbr_ok && pc_ok
    }

    fn selected_screen_snapshot(&self) -> Option<(&[u8], u8, u16, u8, u8)> {
        let filter_pbr = superfx_screen_buffer_stop_pbr_filter();
        let filter_pc = superfx_screen_buffer_stop_pc_filter();
        if filter_pbr.is_some() || filter_pc.is_some() {
            if let Some(snapshot) = self.recent_stop_snapshots.iter().rev().find(|snapshot| {
                filter_pbr.is_none_or(|pbr| pbr == snapshot.pbr)
                    && filter_pc.is_none_or(|pc| pc == snapshot.pc)
            }) {
                return Some((
                    snapshot.data.as_slice(),
                    snapshot.scbr,
                    snapshot.height,
                    snapshot.bpp,
                    snapshot.mode,
                ));
            }
        }
        if let Some(snapshot) = self.debug_pc_snapshot.as_ref() {
            return Some((
                snapshot.data.as_slice(),
                snapshot.scbr,
                snapshot.height,
                snapshot.bpp,
                snapshot.mode,
            ));
        }
        if self.latest_stop_snapshot_valid && self.latest_stop_snapshot_matches_filters() {
            return Some((
                self.latest_stop_snapshot.as_slice(),
                self.latest_stop_scbr,
                self.latest_stop_height,
                self.latest_stop_bpp,
                self.latest_stop_mode,
            ));
        }
        None
    }

    fn display_screen_snapshot(&self) -> Option<(&[u8], u8, u16, u8, u8)> {
        self.display_snapshot_valid.then_some((
            self.display_snapshot.as_slice(),
            self.display_snapshot_scbr,
            self.display_snapshot_height,
            self.display_snapshot_bpp,
            self.display_snapshot_mode,
        ))
    }

    pub fn capture_display_snapshot_for_dma(&mut self, addr: usize, len: usize) -> bool {
        if len == 0 || self.game_ram.is_empty() {
            return false;
        }

        let selected = self
            .selected_screen_snapshot()
            .map(|(snapshot, scbr, height, bpp, mode)| (snapshot.to_vec(), scbr, height, bpp, mode))
            .or_else(|| {
                let len = self.screen_buffer_len()?;
                let start = self.screen_base_addr();
                let end = start.checked_add(len)?.min(self.game_ram.len());
                let height = self.effective_screen_height()? as u16;
                let bpp = self.bits_per_pixel()? as u8;
                let mode = self.effective_screen_layout_mode();
                (start < end).then(|| {
                    (
                        self.game_ram[start..end].to_vec(),
                        self.scbr,
                        height,
                        bpp,
                        mode,
                    )
                })
            });

        let Some((snapshot, scbr, height, bpp, mode)) = selected else {
            return false;
        };
        if snapshot.is_empty() {
            return false;
        }

        let dma_start = addr % self.game_ram.len();
        let dma_end = dma_start.saturating_add(len);
        let snapshot_start = (scbr as usize) << 10;
        let snapshot_end = snapshot_start.saturating_add(snapshot.len());
        if dma_start >= snapshot_end || dma_end <= snapshot_start {
            return false;
        }

        let metadata_changed = !self.display_snapshot_valid
            || self.display_snapshot_scbr != scbr
            || self.display_snapshot_height != height
            || self.display_snapshot_bpp != bpp
            || self.display_snapshot_mode != mode
            || self.display_snapshot.len() != snapshot.len();
        if metadata_changed {
            self.display_snapshot = vec![0; snapshot.len()];
        }

        let copy_start = dma_start.max(snapshot_start);
        let copy_end = dma_end.min(snapshot_end);
        let copy_len = copy_end.saturating_sub(copy_start);
        if copy_len == 0 {
            return false;
        }
        let rel = copy_start - snapshot_start;
        self.display_snapshot[rel..rel + copy_len].copy_from_slice(&snapshot[rel..rel + copy_len]);
        self.display_snapshot_valid = true;
        self.display_snapshot_scbr = scbr;
        self.display_snapshot_height = height;
        self.display_snapshot_bpp = bpp;
        self.display_snapshot_mode = mode;
        if trace_superfx_display_captures_enabled() {
            let nonzero = self
                .display_snapshot
                .iter()
                .filter(|&&byte| byte != 0)
                .count();
            eprintln!(
                "[SFX-DISPLAY-CAPTURE] frame={} dma={:05X}+{} copy={:05X}+{} scbr={:02X} h={} bpp={} mode={} len={} nonzero={}",
                current_trace_superfx_frame(),
                dma_start,
                len,
                copy_start,
                copy_len,
                scbr,
                height,
                bpp,
                mode,
                self.display_snapshot.len(),
                nonzero
            );
        }
        true
    }

    fn maybe_capture_debug_screen_snapshot(&mut self, pc: u16) {
        let Some(filter_pc) = superfx_screen_buffer_capture_pc_filter() else {
            return;
        };
        if pc != filter_pc {
            return;
        }
        if superfx_screen_buffer_capture_pbr_filter().is_some_and(|pbr| pbr != self.pbr) {
            return;
        }
        let Some(len) = self.screen_buffer_len() else {
            return;
        };
        let Some(height) = self.effective_screen_height() else {
            return;
        };
        let Some(bpp) = self.bits_per_pixel() else {
            return;
        };
        let start = self.screen_base_addr();
        let end = start.saturating_add(len).min(self.game_ram.len());
        if start >= end {
            return;
        }
        self.debug_pc_snapshot = Some(StopSnapshot {
            data: self.game_ram[start..end].to_vec(),
            scbr: self.scbr,
            height: height as u16,
            bpp: bpp as u8,
            mode: self.scmr & 0x03,
            pc,
            pbr: self.pbr,
        });
    }

    fn selected_tile_snapshot(&self) -> Option<(&[u8], u16, u8, u8)> {
        if let Some(pc) = superfx_tile_snapshot_pc_filter() {
            let rev_index = superfx_tile_snapshot_rev_index();
            if let Some(snapshot) = self
                .recent_tile_snapshots
                .iter()
                .rev()
                .filter(|snapshot| snapshot.pc == pc)
                .nth(rev_index)
            {
                return Some((
                    snapshot.data.as_slice(),
                    snapshot.height,
                    snapshot.bpp,
                    snapshot.mode,
                ));
            }
        }
        if self.tile_snapshot_valid {
            return Some((
                self.tile_snapshot.as_slice(),
                self.tile_snapshot_height,
                self.tile_snapshot_bpp,
                self.tile_snapshot_mode,
            ));
        }
        None
    }

    pub fn new(rom_size: usize) -> Self {
        let vcr = if rom_size > 0x10_0000 { 0x04 } else { 0x01 };
        #[cfg(test)]
        let rom_bank_mask = {
            let num_banks = rom_size.div_ceil(0x8000);
            num_banks.next_power_of_two().saturating_sub(1).max(1)
        };
        Self {
            regs: [0; GSU_REGISTER_COUNT],
            sfr: 0,
            shadow_sign: 0,
            shadow_zero: 0,
            shadow_carry: false,
            shadow_overflow: false,
            running: false,
            src_reg: 0,
            dst_reg: 0,
            with_reg: 0,
            pbr: 0,
            rombr: 0,
            rambr: 0,
            cbr: 0,
            cache_enabled: false,
            cache_valid_mask: 0,
            scbr: 0,
            scmr: 0,
            cfgr: 0,
            clsr: 0,
            bramr: 0,
            vcr,
            colr: 0,
            por: 0,
            last_ram_addr: 0,
            ram_buffer_pending: false,
            ram_buffer_pending_bank: 0,
            ram_buffer_pending_addr: 0,
            ram_buffer_pending_data: 0,
            rom_buffer: 0,
            rom_buffer_valid: false,
            rom_buffer_pending: false,
            rom_buffer_pending_bank: 0,
            rom_buffer_pending_addr: 0,
            pending_delay_pc: None,
            pending_delay_pbr: None,
            pending_delay_cache_base: None,
            r14_modified: false,
            r15_modified: false,
            pipe: default_superfx_pipe(),
            pipe_valid: false,
            pipe_pc: 0,
            pipe_pbr: 0,
            cache_ram: [0; CACHE_RAM_SIZE],
            game_ram: vec![0; GAME_RAM_SIZE],
            pixelcache: [PixelCache::default(); 2],
            in_cache_flush: false,
            tile_snapshot: Vec::new(),
            tile_snapshot_valid: false,
            tile_snapshot_scbr: 0,
            tile_snapshot_height: 0,
            tile_snapshot_bpp: 0,
            tile_snapshot_mode: 0,
            tile_snapshot_pc: 0,
            tile_snapshot_pbr: 0,
            latest_stop_snapshot: Vec::new(),
            latest_stop_snapshot_valid: false,
            latest_stop_scbr: 0,
            latest_stop_height: 0,
            latest_stop_bpp: 0,
            latest_stop_mode: 0,
            latest_stop_pc: 0,
            latest_stop_pbr: 0,
            display_snapshot: Vec::new(),
            display_snapshot_valid: false,
            display_snapshot_scbr: 0,
            display_snapshot_height: 0,
            display_snapshot_bpp: 0,
            display_snapshot_mode: 0,
            debug_pc_snapshot: None,
            recent_stop_snapshots: Vec::new(),
            recent_tile_snapshots: Vec::new(),
            #[cfg(test)]
            rom_bank_mask,
            last_opcode_cycles: 1,
            exec_profile: [0; 256],
            exec_profile_by_alt: [[0; 256]; 4],
            recent_pc_transfers: Vec::new(),
            recent_reg_writes: Vec::new(),
            last_reg_writes: std::array::from_fn(|_| None),
            recent_reg_writes_by_reg: std::array::from_fn(|_| Vec::new()),
            last_nontrivial_reg_writes: std::array::from_fn(|_| None),
            recent_nontrivial_reg_writes: std::array::from_fn(|_| Vec::new()),
            recent_low_ram_writes: Vec::new(),
            last_low_ram_writes: std::array::from_fn(|_| None),
            recent_exec_trace: Vec::new(),
            current_exec_pbr: 0,
            current_exec_pc: 0,
            current_exec_opcode: 0,
            save_state_pc_hit: None,
            save_state_pc_hit_count: 0,
            save_state_ram_addr_hit: None,
            save_state_ram_addr_hit_count: 0,
            total_run_instructions: 0,
        }
    }

    #[inline]
    pub fn scpu_irq_asserted(&self) -> bool {
        (self.sfr & SFR_IRQ_BIT) != 0
    }

    #[inline]
    pub fn running(&self) -> bool {
        self.running
    }

    #[inline]
    pub fn cpu_has_rom_access(&self) -> bool {
        !self.running || (self.scmr & SCMR_RON_BIT) == 0
    }

    #[inline]
    pub fn cpu_has_ram_access(&self) -> bool {
        !self.running || (self.scmr & SCMR_RAN_BIT) == 0
    }

    #[inline]
    pub fn backup_ram_write_enabled(&self) -> bool {
        (self.bramr & 0x01) != 0
    }

    pub fn save_data(&self) -> SuperFxSaveData {
        SuperFxSaveData {
            regs: self.regs,
            sfr: self.sfr,
            shadow_sign: Some(self.shadow_sign),
            shadow_zero: Some(self.shadow_zero),
            shadow_carry: Some(self.shadow_carry),
            shadow_overflow: Some(self.shadow_overflow),
            running: self.running,
            src_reg: self.src_reg,
            dst_reg: self.dst_reg,
            with_reg: self.with_reg,
            pbr: self.pbr,
            rombr: self.rombr,
            rambr: self.rambr,
            cbr: self.cbr,
            cache_enabled: self.cache_enabled,
            cache_valid_mask: self.cache_valid_mask,
            scbr: self.scbr,
            scmr: self.scmr,
            cfgr: self.cfgr,
            clsr: self.clsr,
            bramr: self.bramr,
            vcr: self.vcr,
            colr: self.colr,
            por: self.por,
            last_ram_addr: self.last_ram_addr,
            ram_buffer_pending: self.ram_buffer_pending,
            ram_buffer_pending_bank: self.ram_buffer_pending_bank,
            ram_buffer_pending_addr: self.ram_buffer_pending_addr,
            ram_buffer_pending_data: self.ram_buffer_pending_data,
            rom_buffer: self.rom_buffer,
            rom_buffer_valid: self.rom_buffer_valid,
            rom_buffer_pending: self.rom_buffer_pending,
            rom_buffer_pending_bank: self.rom_buffer_pending_bank,
            rom_buffer_pending_addr: self.rom_buffer_pending_addr,
            pending_delay_pc: self.pending_delay_pc,
            pending_delay_pbr: self.pending_delay_pbr,
            pending_delay_cache_base: self.pending_delay_cache_base,
            r14_modified: self.r14_modified,
            r15_modified: self.r15_modified,
            pipe: self.pipe,
            pipe_valid: self.pipe_valid,
            pipe_pc: self.pipe_valid.then_some(self.pipe_pc),
            pipe_pbr: self.pipe_valid.then_some(self.pipe_pbr),
            tile_snapshot: self.tile_snapshot.clone(),
            tile_snapshot_valid: self.tile_snapshot_valid,
            tile_snapshot_scbr: self.tile_snapshot_scbr,
            tile_snapshot_height: self.tile_snapshot_height,
            tile_snapshot_bpp: self.tile_snapshot_bpp,
            tile_snapshot_mode: self.tile_snapshot_mode,
            tile_snapshot_pc: self.tile_snapshot_pc,
            tile_snapshot_pbr: self.tile_snapshot_pbr,
            latest_stop_snapshot: self.latest_stop_snapshot.clone(),
            latest_stop_snapshot_valid: self.latest_stop_snapshot_valid,
            latest_stop_scbr: self.latest_stop_scbr,
            latest_stop_height: self.latest_stop_height,
            latest_stop_bpp: self.latest_stop_bpp,
            latest_stop_mode: self.latest_stop_mode,
            latest_stop_pc: self.latest_stop_pc,
            latest_stop_pbr: self.latest_stop_pbr,
            display_snapshot: self.display_snapshot.clone(),
            display_snapshot_valid: self.display_snapshot_valid,
            display_snapshot_scbr: self.display_snapshot_scbr,
            display_snapshot_height: self.display_snapshot_height,
            display_snapshot_bpp: self.display_snapshot_bpp,
            display_snapshot_mode: self.display_snapshot_mode,
            recent_stop_snapshots: self.recent_stop_snapshots.clone(),
            recent_tile_snapshots: self.recent_tile_snapshots.clone(),
            last_reg_writes: self.last_reg_writes.clone(),
            recent_reg_writes_by_reg: self.recent_reg_writes_by_reg.clone(),
            last_nontrivial_reg_writes: self.last_nontrivial_reg_writes.clone(),
            recent_nontrivial_reg_writes: self.recent_nontrivial_reg_writes.clone(),
            recent_reg_writes: self.recent_reg_writes.clone(),
            last_low_ram_writes: self.last_low_ram_writes.to_vec(),
            cache_ram: self.cache_ram.to_vec(),
            game_ram: self.game_ram.clone(),
        }
    }

    pub fn load_data(&mut self, state: &SuperFxSaveData) {
        self.regs = state.regs;
        self.sfr = state.sfr;
        self.shadow_sign = state.shadow_sign.unwrap_or_else(|| {
            if (self.sfr & SFR_S_BIT) != 0 {
                0x8000
            } else {
                0
            }
        });
        self.shadow_zero =
            state
                .shadow_zero
                .unwrap_or_else(|| if (self.sfr & SFR_Z_BIT) != 0 { 0 } else { 1 });
        self.shadow_carry = state.shadow_carry.unwrap_or((self.sfr & SFR_CY_BIT) != 0);
        self.shadow_overflow = state
            .shadow_overflow
            .unwrap_or((self.sfr & SFR_OV_BIT) != 0);
        self.running = state.running;
        self.src_reg = state.src_reg & 0x0F;
        self.dst_reg = state.dst_reg & 0x0F;
        self.with_reg = state.with_reg & 0x0F;
        self.pbr = state.pbr;
        self.rombr = state.rombr;
        self.rambr = state.rambr;
        self.cbr = state.cbr;
        self.cache_enabled = state.cache_enabled;
        self.cache_valid_mask = state.cache_valid_mask;
        self.scbr = state.scbr;
        self.scmr = state.scmr;
        self.cfgr = state.cfgr;
        self.clsr = state.clsr;
        self.bramr = state.bramr;
        self.vcr = state.vcr;
        self.colr = state.colr;
        self.por = state.por;
        self.last_ram_addr = state.last_ram_addr;
        self.ram_buffer_pending = state.ram_buffer_pending;
        self.ram_buffer_pending_bank = state.ram_buffer_pending_bank;
        self.ram_buffer_pending_addr = state.ram_buffer_pending_addr;
        self.ram_buffer_pending_data = state.ram_buffer_pending_data;
        self.rom_buffer = state.rom_buffer;
        self.rom_buffer_valid = state.rom_buffer_valid;
        self.rom_buffer_pending = state.rom_buffer_pending;
        self.rom_buffer_pending_bank = state.rom_buffer_pending_bank;
        self.rom_buffer_pending_addr = state.rom_buffer_pending_addr;
        self.pending_delay_pc = state.pending_delay_pc;
        self.pending_delay_pbr = state.pending_delay_pbr;
        self.pending_delay_cache_base = state.pending_delay_cache_base;
        self.r14_modified = state.r14_modified;
        self.r15_modified = state.r15_modified;
        self.pipe = state.pipe;
        self.pipe_valid = state.pipe_valid;
        self.pipe_pc = state.pipe_pc.unwrap_or_else(|| {
            if self.r15_modified {
                self.regs[15]
            } else {
                self.regs[15].wrapping_sub(1)
            }
        });
        self.pipe_pbr = state.pipe_pbr.unwrap_or(self.pbr);
        self.tile_snapshot = state.tile_snapshot.clone();
        self.tile_snapshot_valid = state.tile_snapshot_valid;
        self.tile_snapshot_scbr = state.tile_snapshot_scbr;
        self.tile_snapshot_height = state.tile_snapshot_height;
        self.tile_snapshot_bpp = state.tile_snapshot_bpp;
        self.tile_snapshot_mode = state.tile_snapshot_mode;
        self.tile_snapshot_pc = state.tile_snapshot_pc;
        self.tile_snapshot_pbr = state.tile_snapshot_pbr;
        self.latest_stop_snapshot = state.latest_stop_snapshot.clone();
        self.latest_stop_snapshot_valid = state.latest_stop_snapshot_valid;
        self.latest_stop_scbr = state.latest_stop_scbr;
        self.latest_stop_height = state.latest_stop_height;
        self.latest_stop_bpp = state.latest_stop_bpp;
        self.latest_stop_mode = state.latest_stop_mode;
        self.latest_stop_pc = state.latest_stop_pc;
        self.latest_stop_pbr = state.latest_stop_pbr;
        self.display_snapshot = state.display_snapshot.clone();
        self.display_snapshot_valid = state.display_snapshot_valid;
        self.display_snapshot_scbr = state.display_snapshot_scbr;
        self.display_snapshot_height = state.display_snapshot_height;
        self.display_snapshot_bpp = state.display_snapshot_bpp;
        self.display_snapshot_mode = state.display_snapshot_mode;
        self.recent_stop_snapshots = state.recent_stop_snapshots.clone();
        self.recent_tile_snapshots = state.recent_tile_snapshots.clone();
        self.last_reg_writes = state.last_reg_writes.clone();
        self.recent_reg_writes_by_reg = state.recent_reg_writes_by_reg.clone();
        self.last_nontrivial_reg_writes = state.last_nontrivial_reg_writes.clone();
        self.recent_nontrivial_reg_writes = state.recent_nontrivial_reg_writes.clone();
        self.recent_reg_writes = state.recent_reg_writes.clone();
        self.last_low_ram_writes.fill(None);
        for (dst, src) in self
            .last_low_ram_writes
            .iter_mut()
            .zip(state.last_low_ram_writes.iter().cloned())
        {
            *dst = src;
        }
        self.cache_ram.fill(0);
        let cache_len = self.cache_ram.len().min(state.cache_ram.len());
        self.cache_ram[..cache_len].copy_from_slice(&state.cache_ram[..cache_len]);
        self.game_ram = state.game_ram.clone();
        if self.game_ram.is_empty() {
            self.game_ram.resize(GAME_RAM_SIZE, 0);
        }
        self.exec_profile = [0; 256];
        self.exec_profile_by_alt = [[0; 256]; 4];
        self.recent_pc_transfers.clear();
        self.recent_low_ram_writes.clear();
        self.recent_exec_trace.clear();
        self.current_exec_pbr = 0;
        self.current_exec_pc = 0;
        self.current_exec_opcode = 0;
        self.save_state_pc_hit = None;
        self.save_state_pc_hit_count = 0;
        self.save_state_ram_addr_hit = None;
        self.save_state_ram_addr_hit_count = 0;
        if !self.rom_buffer_valid && !self.rom_buffer_pending {
            self.schedule_rom_buffer_reload();
        }
    }

    fn apply_pending_delay_transfer(&mut self) {
        if let Some(pbr) = self.pending_delay_pbr.take() {
            self.pbr = pbr & 0x7F;
        }
        if let Some(cbr) = self.pending_delay_cache_base.take() {
            self.cbr = cbr & 0xFFF0;
            self.cache_enabled = true;
            self.cache_valid_mask = 0;
        }
        if let Some(pc) = self.pending_delay_pc.take() {
            self.set_r15(pc);
        }
    }

    fn set_r15(&mut self, value: u16) {
        self.regs[15] = value;
        self.r15_modified = true;
    }

    fn advance_r15_after_fetch(&mut self) {
        self.regs[15] = self.regs[15].wrapping_add(1);
        self.r15_modified = false;
    }

    pub fn read_register(&mut self, offset: u16, mdr: u8) -> u8 {
        match offset {
            0x3000..=0x301F => {
                let reg_index = ((offset - 0x3000) / 2) as usize;
                let word = self.regs[reg_index];
                if (offset & 1) == 0 {
                    word as u8
                } else {
                    (word >> 8) as u8
                }
            }
            0x3030 => {
                let value = self.sfr as u8;
                if trace_superfx_sfr_enabled() {
                    use std::sync::atomic::{AtomicU32, Ordering};
                    static CNT: OnceLock<AtomicU32> = OnceLock::new();
                    let n = CNT
                        .get_or_init(|| AtomicU32::new(0))
                        .fetch_add(1, Ordering::Relaxed);
                    if n < 32 {
                        println!(
                            "[SFX-SFR] raw={:04X} running={} read_low={:02X}",
                            self.sfr, self.running as u8, value
                        );
                    }
                }
                value
            }
            0x3100..=0x32FF => self.cache_ram[(offset - 0x3100) as usize],
            0x3031 => {
                let value = (self.sfr >> 8) as u8;
                self.sfr &= !SFR_IRQ_BIT;
                value
            }
            0x3033 => self.bramr & 0x01,
            0x3034 => self.pbr,
            0x3036 => self.rombr & 0x7F,
            0x3038 => self.scbr,
            0x3039 => self.clsr & 0x01,
            0x303A => self.scmr & 0x3F,
            0x303B => self.vcr,
            0x303C => self.rambr & 0x03,
            0x303E => ((self.cbr & 0xFFF0) as u8) | (mdr & 0x0F),
            0x303F => (self.cbr >> 8) as u8,
            _ => mdr,
        }
    }

    #[inline]
    pub fn observed_sfr_low(&self) -> u8 {
        self.sfr as u8
    }

    pub fn write_register(&mut self, offset: u16, value: u8) {
        self.write_register_with_rom(offset, value, &[]);
    }

    pub fn write_register_with_rom(&mut self, offset: u16, value: u8, rom: &[u8]) {
        match offset {
            0x3000..=0x301F => {
                let reg_index = ((offset - 0x3000) / 2) as usize;
                let mut word = self.regs[reg_index];
                if (offset & 1) == 0 {
                    word = (word & 0xFF00) | value as u16;
                } else {
                    word = (word & 0x00FF) | ((value as u16) << 8);
                }
                self.write_reg(reg_index, word);
                if reg_index == 14 {
                    // bsnes updates the ROM buffer pipeline immediately on any
                    // CPU-side R14 write. Preserve that pending reload across
                    // the later GO/start path instead of clearing it via
                    // prepare_start_execution().
                    self.schedule_rom_buffer_reload();
                    self.r14_modified = false;
                }
                if reg_index == 15 && (offset & 1) != 0 {
                    self.invoke_cpu_start(rom);
                }
            }
            0x3100..=0x32FF => {
                self.cache_write(offset, value);
            }
            0x3030 => {
                self.sfr = (self.sfr & 0xFF00) | value as u16;
                self.sync_condition_flags_from_sfr();
                self.apply_sfr_side_effects(rom);
            }
            0x3031 => {
                self.sfr = (self.sfr & 0x00FF) | ((value as u16) << 8);
                self.sync_condition_flags_from_sfr();
                self.apply_sfr_side_effects(rom);
            }
            0x3033 => self.bramr = value & 0x01,
            0x3034 => {
                self.pbr = value & 0x7F;
                self.cache_valid_mask = 0;
            }
            0x3037 => self.cfgr = value & 0x80,
            0x3038 => self.scbr = value,
            0x3039 => self.clsr = value & 0x01,
            0x303A => self.scmr = value & 0x3F,
            _ => {}
        }
    }

    fn apply_sfr_side_effects(&mut self, rom: &[u8]) {
        if (self.sfr & SFR_GO_BIT) == 0 {
            self.running = false;
            self.cbr = 0;
            self.cache_enabled = false;
            self.cache_valid_mask = 0;
        } else {
            self.start_execution(rom);
        }
    }

    fn invoke_cpu_start(&mut self, rom: &[u8]) {
        self.flush_all_pixel_caches();

        if trace_superfx_start_enabled() {
            use std::sync::atomic::{AtomicU32, Ordering};
            static CNT: OnceLock<AtomicU32> = OnceLock::new();
            let n = CNT
                .get_or_init(|| AtomicU32::new(0))
                .fetch_add(1, Ordering::Relaxed);
            if n < 64 {
                println!(
                    "[SFX-START] R15={:04X} PBR={:02X} R14={:04X} ROMBR={:02X} RAMBR={:02X} SFR={:04X} SCMR={:02X} gram60={:02X}{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}",
                    self.regs[15],
                    self.pbr,
                    self.regs[14],
                    self.rombr,
                    self.rambr,
                    self.sfr,
                    self.scmr,
                    self.game_ram.get(0x60).copied().unwrap_or(0),
                    self.game_ram.get(0x61).copied().unwrap_or(0),
                    self.game_ram.get(0x62).copied().unwrap_or(0),
                    self.game_ram.get(0x63).copied().unwrap_or(0),
                    self.game_ram.get(0x64).copied().unwrap_or(0),
                    self.game_ram.get(0x65).copied().unwrap_or(0),
                    self.game_ram.get(0x66).copied().unwrap_or(0),
                    self.game_ram.get(0x67).copied().unwrap_or(0),
                );
            }
        }
        self.sfr |= SFR_GO_BIT;
        self.total_run_instructions = 0;
        self.pending_delay_pc = None;
        self.pending_delay_pbr = None;
        self.pending_delay_cache_base = None;
        if self.prepare_start_execution(rom) {
            if let Some(steps) = Self::cpu_start_immediate_steps() {
                self.run_steps(rom, steps);
            }
        }
    }

    fn finish_noop_run(&mut self) {
        self.sync_ram_buffer();
        self.running = false;
        self.sfr &= !(SFR_GO_BIT | SFR_R_BIT);
        self.pending_delay_pc = None;
        self.pending_delay_pbr = None;
        self.pending_delay_cache_base = None;
        self.r14_modified = false;
        self.r15_modified = false;
        self.pipe = default_superfx_pipe();
        self.pipe_valid = false;
        if (self.cfgr & 0x80) == 0 {
            self.sfr |= SFR_IRQ_BIT;
        }
    }

    fn experimental_core_enabled() -> bool {
        std::env::var("EXPERIMENTAL_SUPERFX_CORE")
            .map(|value| value != "0" && !value.eq_ignore_ascii_case("false"))
            .unwrap_or(true)
    }

    fn steps_per_cpu_cycle(&self) -> usize {
        // CLSR bit 0: 0 = 10.738 MHz (standard), 1 = 21.477 MHz (turbo/SuperFX2)
        let default = if (self.clsr & 0x01) != 0 {
            DEFAULT_SUPERFX_RATIO_FAST
        } else {
            DEFAULT_SUPERFX_RATIO_SLOW
        };
        let override_ratio = || {
            std::env::var("SUPERFX_CPU_RATIO")
                .ok()
                .and_then(|value| value.parse::<usize>().ok())
                .filter(|value| *value > 0)
        };
        if cfg!(test) {
            override_ratio().unwrap_or(default)
        } else {
            static VALUE: OnceLock<Option<usize>> = OnceLock::new();
            (*VALUE.get_or_init(override_ratio)).unwrap_or(default)
        }
    }

    fn startup_step_budget() -> usize {
        let override_budget = || {
            std::env::var("SUPERFX_STEP_BUDGET")
                .ok()
                .and_then(|value| value.parse::<usize>().ok())
                .filter(|value| *value > 0)
        };
        if cfg!(test) {
            override_budget().unwrap_or(DEFAULT_STARTUP_STEP_BUDGET)
        } else {
            static VALUE: OnceLock<Option<usize>> = OnceLock::new();
            (*VALUE.get_or_init(override_budget)).unwrap_or(DEFAULT_STARTUP_STEP_BUDGET)
        }
    }

    pub fn status_poll_step_budget() -> usize {
        let override_budget = || {
            std::env::var("SUPERFX_STATUS_POLL_BOOST")
                .ok()
                .and_then(|value| value.parse::<usize>().ok())
                .filter(|value| *value > 0)
        };
        if cfg!(test) {
            override_budget().unwrap_or(DEFAULT_SUPERFX_STATUS_POLL_STEP_BUDGET)
        } else {
            static VALUE: OnceLock<Option<usize>> = OnceLock::new();
            (*VALUE.get_or_init(override_budget)).unwrap_or(DEFAULT_SUPERFX_STATUS_POLL_STEP_BUDGET)
        }
    }

    fn status_poll_sfr_change_chunk() -> usize {
        let override_chunk = || {
            std::env::var("SUPERFX_STATUS_POLL_SFR_CHANGE_CHUNK")
                .ok()
                .and_then(|value| value.parse::<usize>().ok())
                .filter(|value| *value > 0)
        };
        let default = || Self::status_poll_step_budget().clamp(1, 8_192);
        if cfg!(test) {
            override_chunk().unwrap_or_else(default)
        } else {
            static VALUE: OnceLock<Option<usize>> = OnceLock::new();
            (*VALUE.get_or_init(override_chunk)).unwrap_or_else(default)
        }
    }

    fn starfox_producer_poll_chunk() -> usize {
        let override_chunk = || {
            std::env::var("SUPERFX_STATUS_POLL_STARFOX_PRODUCER_CHUNK")
                .ok()
                .and_then(|value| value.parse::<usize>().ok())
                .filter(|value| *value > 0)
        };
        if cfg!(test) {
            override_chunk().unwrap_or(262_144)
        } else {
            static VALUE: OnceLock<Option<usize>> = OnceLock::new();
            (*VALUE.get_or_init(override_chunk)).unwrap_or(262_144)
        }
    }

    fn cpu_start_immediate_steps() -> Option<usize> {
        let override_steps = || {
            std::env::var("SUPERFX_CPU_START_IMMEDIATE_STEPS")
                .ok()
                .and_then(|value| value.parse::<usize>().ok())
                .filter(|value| *value > 0)
        };
        if cfg!(test) {
            override_steps()
        } else {
            static VALUE: OnceLock<Option<usize>> = OnceLock::new();
            *VALUE.get_or_init(override_steps)
        }
    }

    pub fn run_status_poll_catchup(&mut self, rom: &[u8]) {
        if !self.running {
            return;
        }
        self.run_steps(rom, Self::status_poll_step_budget());
    }

    pub fn run_status_poll_catchup_steps(&mut self, rom: &[u8], steps: usize) {
        if !self.running || steps == 0 {
            return;
        }
        self.run_steps(rom, steps);
    }

    pub fn run_status_poll_until_stop(&mut self, rom: &[u8], max_steps: usize) {
        if !self.running || max_steps == 0 {
            return;
        }
        let chunk = Self::status_poll_step_budget().saturating_mul(16).max(1);
        let mut remaining = max_steps;
        while self.running && remaining > 0 {
            let steps = remaining.min(chunk);
            self.run_steps(rom, steps);
            remaining -= steps;
        }
    }

    pub fn run_status_poll_until_sfr_low_mask_changes(
        &mut self,
        rom: &[u8],
        initial_low: u8,
        mask: u8,
        max_steps: usize,
    ) {
        if !self.running || max_steps == 0 {
            return;
        }

        // Poll loops only care that the observed status bit changes before the
        // CPU re-reads $3030. Small chunks keep the stop condition tight
        // without turning a single busy-wait into millions of 1-step calls.
        let status_change_chunk = Self::status_poll_sfr_change_chunk();
        let mut remaining = max_steps;
        let initial_masked = initial_low & mask;
        while self.running && remaining > 0 {
            let current_low = self.observed_sfr_low();
            let changed = if mask == 0 {
                current_low != initial_low
            } else {
                (current_low & mask) != initial_masked
            };
            if changed {
                break;
            }
            if self.fast_forward_starfox_cached_delay_loop() {
                continue;
            }
            let steps = remaining.min(status_change_chunk);
            self.run_steps(rom, steps);
            remaining -= steps;
        }
    }

    pub fn run_status_poll_until_sfr_low_changes(
        &mut self,
        rom: &[u8],
        initial_low: u8,
        max_steps: usize,
    ) {
        self.run_status_poll_until_sfr_low_mask_changes(rom, initial_low, u8::MAX, max_steps);
    }

    pub fn debug_in_starfox_live_producer_loop(&self) -> bool {
        self.running
            && self.pbr == 0x01
            && self.rambr == 0x00
            && matches!(self.regs[13], 0xB384..=0xB3E6)
    }

    pub fn run_status_poll_until_go_clears_in_starfox_live_producer_loop(
        &mut self,
        rom: &[u8],
        max_steps: usize,
    ) {
        if !self.debug_in_starfox_live_producer_loop() || max_steps == 0 {
            return;
        }

        let chunk = Self::starfox_producer_poll_chunk();
        let mut remaining = max_steps;
        while self.running && remaining > 0 {
            if (self.observed_sfr_low() & (SFR_GO_BIT as u8)) == 0 {
                break;
            }
            if self.fast_forward_starfox_cached_delay_loop() {
                continue;
            }
            if let Some(consumed) = self.fast_forward_starfox_live_producer_store(rom, remaining) {
                remaining = remaining.saturating_sub(consumed);
                continue;
            }
            let steps = remaining.min(chunk);
            if self.can_direct_run_starfox_late_wait() {
                self.run_steps_direct_no_pipe(rom, steps);
            } else {
                self.run_steps(rom, steps);
            }
            remaining -= steps;
        }
    }

    pub fn debug_in_starfox_cached_delay_loop(&self) -> bool {
        self.running
            && self.pbr == 0x01
            && self.cache_enabled
            && self.cbr == 0x84F0
            && matches!(self.regs[11], 0x8609 | 0x8615)
            && self.regs[13] == 0x000B
            && matches!(self.regs[15], 0x000B..=0x000D)
    }

    pub fn debug_in_starfox_late_parser_loop(&self) -> bool {
        self.running && self.pbr == 0x01 && matches!(self.regs[13], 0xD1B4..=0xD4EB)
    }

    fn starfox_table_has_head_word(&self, key: u16) -> bool {
        (0..0x012Cusize).any(|index| {
            let addr = 0x1AB8u16.wrapping_add((index as u16).wrapping_mul(0x000E));
            self.debug_read_ram_word_short(addr) == key
        })
    }

    fn starfox_table_find_head_by_any_word(&self, key: u16) -> Option<u16> {
        (0..0x012Cusize).find_map(|index| {
            let base = 0x1AB8u16.wrapping_add((index as u16).wrapping_mul(0x000E));
            let head = self.debug_read_ram_word_short(base);
            (0..7usize).find_map(|field| {
                let addr = base.wrapping_add((field as u16).wrapping_mul(2));
                (self.debug_read_ram_word_short(addr) == key).then_some(head)
            })
        })
    }

    fn maybe_force_starfox_late_search_key_from_match(&mut self) {
        if env_presence_cached("STARFOX_FORCE_LATE_SEARCH_KEY_FROM_ANY_TABLE_FIELD")
            && self.running
            && self.pbr == 0x01
            && self.regs[15] == 0xD47A
        {
            let current_key = self.regs[7];
            if !self.starfox_table_has_head_word(current_key) {
                if let Some(head) = self.starfox_table_find_head_by_any_word(current_key) {
                    self.regs[7] = head;
                    return;
                }
            }
        }

        if !env_presence_cached("STARFOX_FORCE_LATE_SEARCH_KEY_FROM_1AE2") {
            return;
        }
        if !self.running || self.pbr != 0x01 || self.regs[15] != 0xD47A {
            return;
        }

        let cursor = self.read_ram_word(0x1AE0);
        let match_key = self.read_ram_word(0x1AE2);
        if cursor != 0xFFF9 || match_key == 0 {
            return;
        }

        let current_key = self.regs[7];
        let has_head_match = (0..0x012Cusize).any(|index| {
            let addr = 0x1AB8u16.wrapping_add((index as u16).wrapping_mul(0x000E));
            self.read_ram_word(addr) == current_key
        });
        if has_head_match {
            return;
        }

        let match_has_head = (0..0x012Cusize).any(|index| {
            let addr = 0x1AB8u16.wrapping_add((index as u16).wrapping_mul(0x000E));
            self.read_ram_word(addr) == match_key
        });
        if !match_has_head {
            return;
        }

        self.regs[7] = match_key;
    }

    fn maybe_force_starfox_parser_key_from_match_word(&self, addr: u16, value: u16) -> u16 {
        if env_presence_cached("STARFOX_FORCE_PARSER_KEY_FROM_ANY_TABLE_FIELD")
            && self.running
            && self.pbr == 0x01
            && self.current_exec_pbr == 0x01
            && self.current_exec_pc == 0xAD46
            && self.current_exec_opcode == 0xA0
            && addr == 0x0136
            && !self.starfox_table_has_head_word(value)
        {
            if let Some(head) = self.starfox_table_find_head_by_any_word(value) {
                return head;
            }
        }

        if !env_presence_cached("STARFOX_FORCE_PARSER_KEY_FROM_MATCH_WORD") {
            return value;
        }
        if !self.running
            || self.pbr != 0x01
            || self.current_exec_pbr != 0x01
            || self.current_exec_pc != 0xAD46
            || self.current_exec_opcode != 0xA0
            || addr != 0x0136
        {
            return value;
        }

        let cursor = self.debug_read_ram_word_short(0x1AE0);
        let match_key = self.debug_read_ram_word_short(0x1AE2);
        if cursor != 0xFFF9 || match_key == 0 || value == match_key {
            return value;
        }

        let value_has_head = self.starfox_table_has_head_word(value);
        let match_has_head = self.starfox_table_has_head_word(match_key);
        if value_has_head || !match_has_head {
            return value;
        }

        match_key
    }

    fn maybe_keep_starfox_success_cursor_armed(&self, addr: u16, value: u16) -> u16 {
        if !env_presence_cached("STARFOX_KEEP_SUCCESS_CURSOR_ARMED")
            && !env_presence_cached("STARFOX_KEEP_SUCCESS_CONTEXT")
        {
            return value;
        }
        if !self.running
            || self.pbr != 0x01
            || self.current_exec_pbr != 0x01
            || self.current_exec_pc != 0xD1CC
            || addr != 0x1AE0
            || value != 0x0000
        {
            return value;
        }

        0xFFF9
    }

    fn maybe_keep_starfox_success_branch_target(&self, index: usize, value: u16, pc: u16) -> u16 {
        let keep_success_branch = env_presence_cached("STARFOX_KEEP_SUCCESS_BRANCH_TARGET");
        let keep_success_context = env_presence_cached("STARFOX_KEEP_SUCCESS_CONTEXT");
        let force_success_b196 = env_presence_cached("STARFOX_FORCE_SUCCESS_BRANCH_TO_B196");
        if !keep_success_branch && !keep_success_context && !force_success_b196 {
            return value;
        }
        if !self.running
            || self.pbr != 0x01
            || self.current_exec_pbr != 0x01
            || index != 13
            || pc != 0xD4D0
            || value != 0x0000
            || self.regs[13] != 0xD1B4
        {
            return value;
        }

        if force_success_b196 {
            0xB196
        } else {
            0xD1B4
        }
    }

    fn maybe_keep_starfox_success_search_context(&self, index: usize, value: u16, pc: u16) -> u16 {
        if !env_presence_cached("STARFOX_KEEP_SUCCESS_CONTEXT") {
            return value;
        }
        if !self.running || self.pbr != 0x01 || self.current_exec_pbr != 0x01 {
            return value;
        }
        if self.regs[7] != 0x004B || self.regs[13] != 0xD1B4 || value != 0x0000 {
            return value;
        }

        match (index, pc) {
            (9, 0xD4BB) => self.regs[9],
            (13, 0xD4D0) => self.regs[13],
            _ => value,
        }
    }

    fn maybe_force_starfox_b30a_r14_seed(&self, index: usize, value: u16, pc: u16) -> u16 {
        let Some(forced) = env_u16("STARFOX_FORCE_B30A_R14_VALUE") else {
            return value;
        };
        let frame = env_u16("STARFOX_FORCE_B30A_R14_FRAME")
            .map(u32::from)
            .unwrap_or_else(current_trace_superfx_frame);
        if !self.running
            || self.pbr != 0x01
            || self.current_exec_pbr != 0x01
            || current_trace_superfx_frame() != frame
            || index != 14
            || pc != 0xB30A
        {
            return value;
        }

        forced
    }

    fn maybe_force_starfox_b380_r12_seed(&self, index: usize, value: u16, pc: u16) -> u16 {
        let Some(forced) = env_u16("STARFOX_FORCE_B380_R12_VALUE") else {
            return value;
        };
        let frame = env_u16("STARFOX_FORCE_B380_R12_FRAME")
            .map(u32::from)
            .unwrap_or_else(current_trace_superfx_frame);
        if !self.running
            || self.pbr != 0x01
            || self.current_exec_pbr != 0x01
            || current_trace_superfx_frame() != frame
            || index != 12
            || pc != 0xB380
        {
            return value;
        }

        forced
    }

    fn maybe_force_starfox_b384_preexec_live_state(&mut self, pc: u16) {
        let frame = env_u16("STARFOX_FORCE_B384_PREEXEC_FRAME")
            .map(u32::from)
            .unwrap_or_else(current_trace_superfx_frame);
        if !self.running
            || self.pbr != 0x01
            || self.current_exec_pbr != 0x01
            || current_trace_superfx_frame() != frame
            || !(0xB384..=0xB396).contains(&pc)
        {
            return;
        }

        if let Some(value) = env_u16("STARFOX_FORCE_B384_PREEXEC_R12_VALUE") {
            self.regs[12] = value;
        }
        if let Some(value) = env_u16("STARFOX_FORCE_B384_PREEXEC_R14_VALUE") {
            self.regs[14] = value;
        }
    }

    fn maybe_null_starfox_ac98_continuation_word(&self, index: usize, value: u16, pc: u16) -> u16 {
        if !env_presence_cached("STARFOX_NULL_AC98_AFTER_SUCCESS") {
            return value;
        }
        if !self.running
            || self.pbr != 0x01
            || self.current_exec_pbr != 0x01
            || index != 1
            || pc != 0xAC98
            || value != 0x887F
        {
            return value;
        }
        let match_key = self.debug_read_ram_word_short(0x1AE2);
        let success_fragment = self.debug_read_ram_word_short(0x888C);
        if match_key == 0x004B && success_fragment == 0x4BFC {
            return 0x0000;
        }
        value
    }

    fn maybe_force_starfox_continuation_cursor_word(&self, addr: u16, value: u16) -> u16 {
        let forced_value = starfox_force_continuation_cursor_value();
        let force_match_fragment =
            env_presence_cached("STARFOX_FORCE_CONTINUATION_CURSOR_FROM_MATCH_FRAGMENT");
        let null_after_success = env_presence_cached("STARFOX_NULL_CONTINUATION_AFTER_SUCCESS");
        if !force_match_fragment && forced_value.is_none() && !null_after_success {
            return value;
        }
        if !self.running
            || self.pbr != 0x01
            || self.current_exec_pbr != 0x01
            || self.current_exec_pc != 0xACAD
            || addr != 0x04C4
        {
            return value;
        }
        if null_after_success {
            let cursor = self.debug_read_ram_word_short(0x1AE0);
            let match_key = self.debug_read_ram_word_short(0x1AE2);
            let success_fragment = self.debug_read_ram_word_short(0x888C);
            if cursor == 0xFFF9 && match_key == 0x004B && success_fragment == 0x4BFC {
                return 0x0000;
            }
        }
        if value != 0x887F {
            return value;
        }
        if let Some(forced_value) = forced_value {
            return forced_value;
        }
        0x888D
    }

    fn maybe_force_starfox_continuation_ptr_byte(&self, addr: u16, value: u8) -> u8 {
        if !env_presence_cached("STARFOX_FORCE_CONTINUATION_PTR_FROM_MATCH_FRAGMENT") {
            return value;
        }
        if !self.running
            || self.pbr != 0x01
            || self.current_exec_pbr != 0x01
            || self.current_exec_pc != 0xB396
            || self.current_exec_opcode != 0x31
            || !matches!(addr, 0x021E | 0x021F)
        {
            return value;
        }

        // In the live path, 0x021E is finalized before 0x1AE2 has been re-armed
        // to 0x004B. Anchor this override to the already-produced success fragment.
        let success_fragment = self.debug_read_ram_word_short(0x888C);
        if success_fragment != 0x4BFC {
            return value;
        }

        let next_word = self.ram_word_after_byte_write(0x021E, addr, value);
        if next_word != 0x887F {
            return value;
        }

        match addr {
            0x021E => 0x8D,
            0x021F => 0x88,
            _ => value,
        }
    }

    fn fast_forward_starfox_cached_delay_loop(&mut self) -> bool {
        if !self.debug_in_starfox_cached_delay_loop() || self.regs[15] != 0x000B {
            return false;
        }
        // 01:000B is the tight LOOP instruction that burns down R12 until it
        // reaches zero, then falls through to 01:000C. The status-poll helper
        // already special-cases this exact cached routine, so collapse the
        // counted loop in one step instead of iterating tens of thousands of
        // times during a single SFR poll.
        self.regs[12] = 0;
        self.update_sign_zero_flags(0);
        self.set_r15(0x000C);
        self.pipe = default_superfx_pipe();
        self.pipe_valid = false;
        self.clear_prefix_flags();
        self.maybe_force_starfox_late_search_key_from_match();

        true
    }

    fn can_direct_run_starfox_late_wait(&self) -> bool {
        !trace_superfx_last_transfers_enabled()
            && !trace_superfx_pc_trace_enabled()
            && !trace_superfx_reg_flow_enabled()
            && !trace_superfx_profile_enabled()
            && !trace_superfx_start_enabled()
            && save_state_at_gsu_pc_range().is_none()
            && save_state_at_gsu_reg_write().is_none()
            && save_state_at_gsu_reg_eq().is_none()
            && save_state_at_gsu_recent_exec_tail().is_none()
            && save_state_at_superfx_ram_addr_config().is_none()
            && save_state_at_superfx_ram_byte_eq().is_none()
            && save_state_at_superfx_ram_word_eq().is_none()
    }

    fn fast_forward_starfox_b4bf_rotate_loop(
        &mut self,
        step_budget_remaining: usize,
    ) -> Option<usize> {
        if !enable_experimental_starfox_fastpaths() {
            return None;
        }
        if !self.running
            || step_budget_remaining < 2
            || !self.can_direct_run_starfox_late_wait()
            || self.pbr != 0x01
            || self.rambr != 0x00
            || self.regs[3] == 0
        {
            return None;
        }

        let pc = self.regs[15];
        if !matches!(pc, 0xB4BA | 0xB4BF | 0xB4C0) {
            return None;
        }

        let loop_target = self.regs[13];
        if !matches!(loop_target, 0xB4BA | 0xB4BF) {
            return None;
        }

        let mut consumed = 0usize;

        if pc == 0xB4C0 {
            let next_r12 = self.regs[12].wrapping_sub(1);
            self.write_reg(12, next_r12);
            self.update_sign_zero_flags(next_r12);
            self.clear_prefix_flags();
            consumed += 1;

            if next_r12 == 0 {
                self.set_r15(0xB4C1);
                return Some(consumed);
            }

            self.set_r15(loop_target);
        } else if pc == 0xB4BA {
            // C3 / BEQ / NOP only gate entry into the rotate loop. When R3 is
            // non-zero, the loop body starts at B4BF.
            self.set_r15(0xB4BF);
            consumed += 3;
        }

        if step_budget_remaining <= consumed {
            return Some(consumed);
        }

        let iterations = usize::min(
            self.regs[12] as usize,
            (step_budget_remaining - consumed) / 2,
        );
        if iterations == 0 {
            return Some(consumed);
        }

        // Repeated ROL-through-carry over R4 is a rotate on the 17-bit ring
        // [carry, r4 bit0..bit15]. Collapse the hot B4BF/B4C0 loop by
        // rotating that ring and burning down R12 in one shot.
        let mask = (1u32 << 17) - 1;
        let shift = iterations % 17;
        let mut ring = u32::from(self.condition_carry_set() as u8) | (u32::from(self.regs[4]) << 1);
        if shift != 0 {
            ring = ((ring << shift) | (ring >> (17 - shift))) & mask;
        }

        let next_r4 = ((ring >> 1) & 0xFFFF) as u16;
        self.write_reg(4, next_r4);
        self.set_carry_flag((ring & 0x0001) != 0);

        let next_r12 = self.regs[12].wrapping_sub(iterations as u16);
        self.write_reg(12, next_r12);
        self.update_sign_zero_flags(next_r12);
        self.clear_prefix_flags();
        consumed += iterations * 2;

        if next_r12 == 0 {
            self.set_r15(0xB4C1);
        } else {
            self.set_r15(loop_target);
        }

        Some(consumed)
    }

    fn fast_forward_starfox_b4b1_prefix_to_rotate_loop(
        &mut self,
        step_budget_remaining: usize,
    ) -> Option<usize> {
        if !enable_experimental_starfox_fastpaths() {
            return None;
        }
        if !self.running
            || step_budget_remaining < 2
            || !self.can_direct_run_starfox_late_wait()
            || self.pbr != 0x01
            || self.rambr != 0x00
        {
            return None;
        }

        let mut pc = self.regs[15];
        if !(0xB4B1..=0xB4BE).contains(&pc) {
            return None;
        }

        let mut consumed = 0usize;
        loop {
            match pc {
                0xB4B1 => {
                    self.clear_prefix_flags();
                    consumed += 1;
                    pc = 0xB4B2;
                }
                0xB4B2 => {
                    let lhs = self.regs[0];
                    let rhs = self.regs[4];
                    let diff = i32::from(lhs) - i32::from(rhs);
                    let result = diff as u16;
                    let overflow = (((lhs ^ rhs) & (lhs ^ result)) & 0x8000) != 0;
                    self.set_carry_flag(diff >= 0);
                    self.set_overflow_flag(overflow);
                    self.update_sign_zero_flags(result);
                    self.write_reg(0, result);
                    self.clear_prefix_flags();
                    consumed += 1;
                    pc = 0xB4B3;
                }
                0xB4B3 => {
                    self.with_reg = 7;
                    self.src_reg = 7;
                    self.dst_reg = 7;
                    self.sfr |= SFR_B_BIT;
                    consumed += 1;
                    pc = 0xB4B4;
                }
                0xB4B4 => {
                    let value = self.regs[7];
                    self.write_reg(13, value);
                    self.sfr &= !SFR_B_BIT;
                    self.clear_prefix_flags();
                    consumed += 1;
                    pc = 0xB4B5;
                }
                0xB4B5 => {
                    self.with_reg = 2;
                    self.src_reg = 2;
                    self.dst_reg = 2;
                    self.sfr |= SFR_B_BIT;
                    consumed += 1;
                    pc = 0xB4B6;
                }
                0xB4B6 => {
                    let value = self.regs[2];
                    let result = value >> 1;
                    self.set_carry_flag((value & 0x0001) != 0);
                    self.write_reg(2, result);
                    self.update_sign_zero_flags(result);
                    self.clear_prefix_flags();
                    consumed += 1;
                    pc = 0xB4B7;
                }
                0xB4B7 => {
                    self.with_reg = 3;
                    self.src_reg = 3;
                    self.dst_reg = 3;
                    self.sfr |= SFR_B_BIT;
                    consumed += 1;
                    pc = 0xB4B8;
                }
                0xB4B8 => {
                    let value = self.regs[3];
                    let carry_in = u16::from(self.condition_carry_set()) << 15;
                    let result = (value >> 1) | carry_in;
                    self.set_carry_flag((value & 0x0001) != 0);
                    self.write_reg(3, result);
                    self.update_sign_zero_flags(result);
                    self.clear_prefix_flags();
                    consumed += 1;
                    pc = 0xB4B9;
                }
                0xB4B9 => {
                    self.src_reg = 2;
                    consumed += 1;
                    pc = 0xB4BA;
                }
                0xB4BA => {
                    let result = self.regs[2] | self.regs[3];
                    self.write_reg(0, result);
                    self.update_sign_zero_flags(result);
                    self.clear_prefix_flags();
                    consumed += 1;
                    pc = 0xB4BB;
                }
                0xB4BB => {
                    consumed += 2;
                    if self.condition_zero_set() {
                        self.set_r15(0xB4C3);
                        return Some(consumed);
                    }
                    pc = 0xB4BD;
                }
                0xB4BD => {
                    self.clear_prefix_flags();
                    consumed += 1;
                    pc = 0xB4BE;
                }
                0xB4BE => {
                    self.with_reg = 4;
                    self.src_reg = 4;
                    self.dst_reg = 4;
                    self.sfr |= SFR_B_BIT;
                    consumed += 1;
                    self.set_r15(0xB4BF);
                    return Some(consumed);
                }
                _ => return None,
            }

            if consumed >= step_budget_remaining {
                self.set_r15(pc);
                return Some(consumed);
            }
        }
    }

    fn fast_forward_starfox_outer_packet_setup(
        &mut self,
        step_budget_remaining: usize,
    ) -> Option<usize> {
        if !enable_experimental_starfox_fastpaths() {
            return None;
        }
        if !self.running
            || step_budget_remaining < 4
            || !self.can_direct_run_starfox_late_wait()
            || self.pbr != 0x01
            || self.rambr != 0x00
        {
            return None;
        }

        let mut pc = self.regs[15];
        if !matches!(
            pc,
            0xB33D | 0xB347..=0xB34D | 0xB367 | 0xB37C..=0xB383
        ) {
            return None;
        }

        let mut consumed = 0usize;

        if pc == 0xB33D {
            if self.regs[4] == 0 {
                self.set_r15(0xB3C1);
                return Some(6);
            }
            consumed += 10;
            pc = 0xB37C;
        } else if pc == 0xB367 {
            if self.regs[4] == 0 {
                return None;
            }
            consumed += 5;
            pc = 0xB37C;
        } else if (0xB347..=0xB34D).contains(&pc) {
            consumed += 5;
            pc = 0xB37C;
        }

        if pc <= 0xB37D {
            self.with_reg = 4;
            self.src_reg = 4;
            self.dst_reg = 4;
            self.sfr |= SFR_B_BIT;
            consumed += if pc == 0xB37C { 2 } else { 1 };
            pc = 0xB37E;
        }

        if pc <= 0xB37E {
            self.sfr = (self.sfr & !SFR_B_BIT) | SFR_ALT2_BIT;
            consumed += 1;
            pc = 0xB37F;
        }

        if pc <= 0xB37F {
            let lhs = self.regs[4];
            let rhs = 7u16;
            let sum = i32::from(lhs) + i32::from(rhs);
            let result = sum as u16;
            let overflow = ((!(lhs ^ rhs) & (rhs ^ result)) & 0x8000) != 0;
            self.set_carry_flag(sum >= 0x1_0000);
            self.set_overflow_flag(overflow);
            self.write_reg(4, result);
            self.update_sign_zero_flags(result);
            self.clear_prefix_flags();
            consumed += 1;
            pc = 0xB380;
        }

        if pc <= 0xB381 {
            self.write_reg(12, 0x0008);
            self.update_sign_zero_flags(0x0008);
            self.clear_prefix_flags();
            consumed += if pc == 0xB380 { 2 } else { 1 };
            pc = 0xB382;
        }

        if pc <= 0xB382 {
            self.with_reg = 15;
            self.src_reg = 15;
            self.dst_reg = 15;
            self.sfr |= SFR_B_BIT;
            consumed += 1;
            pc = 0xB383;
        }

        if pc <= 0xB383 {
            self.write_reg(13, 0xB384);
            self.sfr &= !SFR_B_BIT;
            self.clear_prefix_flags();
            consumed += 1;
        }

        self.set_r15(0xB384);
        Some(consumed)
    }

    fn run_steps_direct_no_pipe(&mut self, rom: &[u8], step_budget: usize) {
        if !self.running || step_budget == 0 {
            return;
        }

        let mut steps = 0usize;
        let mut instruction_count = 0usize;
        self.pipe_valid = false;

        while self.running && steps < step_budget {
            if let Some(consumed_steps) =
                self.fast_forward_starfox_outer_packet_setup(step_budget - steps)
            {
                instruction_count += consumed_steps;
                steps += consumed_steps;
                continue;
            }

            if let Some(consumed_steps) =
                self.fast_forward_starfox_b4b1_prefix_to_rotate_loop(step_budget - steps)
            {
                instruction_count += consumed_steps;
                steps += consumed_steps;
                continue;
            }

            if let Some(consumed_steps) =
                self.fast_forward_starfox_b4bf_rotate_loop(step_budget - steps)
            {
                instruction_count += consumed_steps;
                steps += consumed_steps;
                continue;
            }

            if let Some(consumed_steps) =
                self.fast_forward_starfox_live_producer_store(rom, step_budget - steps)
            {
                instruction_count += consumed_steps;
                steps += consumed_steps;
                continue;
            }

            if self.pending_delay_pc.is_some()
                || self.pending_delay_pbr.is_some()
                || self.pending_delay_cache_base.is_some()
            {
                self.apply_pending_delay_transfer();
            }

            let pc = self.regs[15];
            let exec_pbr = self.pbr;
            let Some(opcode) = self.read_program_rom_byte(rom, exec_pbr, pc) else {
                self.trace_abort("direct-fetch", pc, 0xFF);
                self.finish_noop_run();
                return;
            };
            self.advance_r15_after_fetch();
            self.current_exec_pbr = exec_pbr;
            self.current_exec_pc = pc;
            self.current_exec_opcode = opcode;
            if starfox_b384_preexec_debug_override_enabled() {
                self.maybe_force_starfox_b384_preexec_live_state(pc);
            }

            if !self.execute_opcode(opcode, rom, pc) {
                self.total_run_instructions += instruction_count as u64;
                self.finish_noop_run();
                return;
            }

            self.pipe_valid = false;
            instruction_count += 1;
            steps += self.last_opcode_cycles;
        }

        self.total_run_instructions += instruction_count as u64;
    }

    fn fast_forward_starfox_live_producer_store(
        &mut self,
        rom: &[u8],
        step_budget_remaining: usize,
    ) -> Option<usize> {
        if !enable_experimental_starfox_fastpaths() {
            return None;
        }
        if !self.running
            || step_budget_remaining < 8
            || !self.can_direct_run_starfox_late_wait()
            || self.pbr != 0x01
            || self.rambr != 0x00
            || !matches!(
                self.regs[13],
                0xB37F | 0xB380 | 0xB384 | 0xB392 | 0xB39D | 0xB3B8
            )
            || !matches!(
                self.regs[15],
                0xB37F
                    | 0xB380
                    | 0xB384
                    | 0xB389
                    | 0xB38A
                    | 0xB38B
                    | 0xB38C
                    | 0xB38D
                    | 0xB38E
                    | 0xB38F
                    | 0xB390
                    | 0xB391
                    | 0xB392
                    | 0xB39D..=0xB3B8
            )
        {
            return None;
        }

        let mut consumed = 0usize;
        let mut pc = self.regs[15];

        if pc == 0xB380 {
            self.write_reg(12, 0x0008);
            self.update_sign_zero_flags(0x0008);
            self.clear_prefix_flags();
            self.regs[13] = 0xB384;
            self.set_r15(0xB384);
            pc = 0xB384;
            consumed += 3;
        }

        loop {
            match pc {
                0xB37F => {
                    let lhs = self.regs[0];
                    let rhs = self.regs[7];
                    let sum = u32::from(lhs) + u32::from(rhs);
                    let result = sum as u16;
                    let overflow = ((!(lhs ^ rhs) & (rhs ^ result)) & 0x8000) != 0;
                    self.write_reg(0, result);
                    self.set_carry_flag(sum >= 0x1_0000);
                    self.set_overflow_flag(overflow);
                    self.update_sign_zero_flags(result);
                    self.clear_prefix_flags();

                    self.write_reg(12, 0x0008);
                    self.update_sign_zero_flags(0x0008);
                    self.clear_prefix_flags();
                    self.regs[13] = 0xB384;
                    self.set_r15(0xB384);
                    consumed += 4;
                    pc = 0xB384;
                }
                0xB384..=0xB391 => {
                    if pc <= 0xB384 {
                        self.with_reg = 2;
                        self.src_reg = 2;
                        self.dst_reg = 2;
                        self.sfr |= SFR_B_BIT;
                        consumed += 1;
                        pc = 0xB385;
                    }

                    if pc <= 0xB385 {
                        let value = self.regs[2];
                        let result = value >> 1;
                        self.set_carry_flag((value & 0x0001) != 0);
                        self.write_reg(2, result);
                        self.update_sign_zero_flags(result);
                        self.clear_prefix_flags();
                        consumed += 1;
                        pc = 0xB386;
                    }

                    if pc <= 0xB386 {
                        self.with_reg = 3;
                        self.src_reg = 3;
                        self.dst_reg = 3;
                        self.sfr |= SFR_B_BIT;
                        consumed += 1;
                        pc = 0xB387;
                    }

                    if pc <= 0xB387 {
                        let value = self.regs[3];
                        let carry_in = u16::from(self.condition_carry_set()) << 15;
                        let result = (value >> 1) | carry_in;
                        self.set_carry_flag((value & 0x0001) != 0);
                        self.write_reg(3, result);
                        self.update_sign_zero_flags(result);
                        self.clear_prefix_flags();
                        consumed += 1;
                        pc = 0xB388;
                    }

                    if pc <= 0xB388 {
                        self.src_reg = 2;
                        consumed += 1;
                        pc = 0xB389;
                    }

                    if pc <= 0xB389 {
                        let result = self.regs[2] | self.regs[3];
                        self.write_reg(0, result);
                        self.update_sign_zero_flags(result);
                        self.clear_prefix_flags();
                        consumed += 1;
                        pc = 0xB38A;
                    }

                    if pc <= 0xB38B {
                        consumed += 2;
                        if self.condition_zero_set() {
                            self.set_r15(0xB39D);
                            pc = 0xB39D;
                            continue;
                        }
                        pc = 0xB38C;
                    }

                    if pc <= 0xB38C {
                        self.clear_prefix_flags();
                        consumed += 1;
                        pc = 0xB38D;
                    }

                    if pc <= 0xB38D {
                        self.with_reg = 6;
                        self.src_reg = 6;
                        self.dst_reg = 6;
                        self.sfr |= SFR_B_BIT;
                        consumed += 1;
                        pc = 0xB38E;
                    }

                    if pc <= 0xB38E {
                        let value = self.regs[6];
                        let carry_in = u16::from(self.condition_carry_set());
                        let result = (value << 1) | carry_in;
                        self.set_carry_flag((value & 0x8000) != 0);
                        self.write_reg(6, result);
                        self.update_sign_zero_flags(result);
                        self.clear_prefix_flags();
                        consumed += 1;
                        pc = 0xB38F;
                    }

                    if pc <= 0xB38F {
                        self.with_reg = 5;
                        self.src_reg = 5;
                        self.dst_reg = 5;
                        self.sfr |= SFR_B_BIT;
                        consumed += 1;
                        pc = 0xB390;
                    }

                    if pc <= 0xB390 {
                        let value = self.regs[5];
                        let carry_in = u16::from(self.condition_carry_set());
                        let result = (value << 1) | carry_in;
                        self.set_carry_flag((value & 0x8000) != 0);
                        self.write_reg(5, result);
                        self.update_sign_zero_flags(result);
                        self.clear_prefix_flags();
                        consumed += 1;
                    }

                    let next_r12 = self.regs[12].wrapping_sub(1);
                    self.write_reg(12, next_r12);
                    self.update_sign_zero_flags(next_r12);
                    self.clear_prefix_flags();
                    consumed += 1;

                    if next_r12 != 0 {
                        if consumed.saturating_add(8) > step_budget_remaining {
                            self.set_r15(self.regs[13]);
                            return Some(consumed);
                        }
                        self.set_r15(self.regs[13]);
                        pc = self.regs[13];
                        continue;
                    }

                    self.set_r15(0xB392);
                    pc = 0xB392;
                }
                0xB39D..=0xB3B7 => loop {
                    match pc {
                        0xB39D => {
                            self.clear_prefix_flags();
                            consumed += 1;
                            pc = 0xB39E;
                        }
                        0xB39E | 0xB3A1 | 0xB3A5 | 0xB3A8 => {
                            let next_r14 = self.regs[14].wrapping_sub(1);
                            self.write_reg(14, next_r14);
                            self.update_sign_zero_flags(next_r14);
                            self.clear_prefix_flags();
                            consumed += 1;
                            pc = pc.wrapping_add(1);
                        }
                        0xB39F => {
                            self.dst_reg = 3;
                            consumed += 1;
                            pc = 0xB3A0;
                        }
                        0xB3A0 | 0xB3A4 | 0xB3A7 | 0xB3AB => {
                            let byte = self.read_data_rom_byte(rom)?;
                            let src_value = self.reg(self.src_reg);
                            let result = match self.alt_mode() {
                                0 => byte as u16,
                                1 => ((byte as u16) << 8) | (src_value & 0x00FF),
                                2 => (src_value & 0xFF00) | byte as u16,
                                3 => byte as i8 as i16 as u16,
                                _ => unreachable!(),
                            };
                            self.write_reg(self.dst_reg as usize, result);
                            self.update_sign_zero_flags(result);
                            self.clear_prefix_flags();
                            consumed += 1;
                            pc = pc.wrapping_add(1);
                        }
                        0xB3A2 => {
                            self.with_reg = 3;
                            self.src_reg = 3;
                            self.dst_reg = 3;
                            self.sfr |= SFR_B_BIT;
                            consumed += 1;
                            pc = 0xB3A3;
                        }
                        0xB3A3 | 0xB3AA => {
                            self.sfr = (self.sfr & !SFR_B_BIT) | SFR_ALT1_BIT;
                            consumed += 1;
                            pc = pc.wrapping_add(1);
                        }
                        0xB3A6 => {
                            self.dst_reg = 2;
                            consumed += 1;
                            pc = 0xB3A7;
                        }
                        0xB3A9 | 0xB3AF => {
                            self.with_reg = 2;
                            self.src_reg = 2;
                            self.dst_reg = 2;
                            self.sfr |= SFR_B_BIT;
                            consumed += 1;
                            pc = pc.wrapping_add(1);
                        }
                        0xB3AC => {
                            self.write_reg(0, 0x0001);
                            self.update_sign_zero_flags(0x0001);
                            self.clear_prefix_flags();
                            consumed += 2;
                            pc = 0xB3AE;
                        }
                        0xB3AE | 0xB3B0 | 0xB3B2 => {
                            let reg = self.src_reg as usize;
                            let value = self.reg(reg as u8);
                            let carry_in = u16::from(self.condition_carry_set()) << 15;
                            let result = (value >> 1) | carry_in;
                            self.set_carry_flag((value & 0x0001) != 0);
                            self.write_reg(reg, result);
                            self.update_sign_zero_flags(result);
                            self.clear_prefix_flags();
                            consumed += 1;
                            pc = pc.wrapping_add(1);
                        }
                        0xB3B1 => {
                            self.with_reg = 3;
                            self.src_reg = 3;
                            self.dst_reg = 3;
                            self.sfr |= SFR_B_BIT;
                            consumed += 1;
                            pc = 0xB3B2;
                        }
                        0xB3B3 => {
                            self.with_reg = 6;
                            self.src_reg = 6;
                            self.dst_reg = 6;
                            self.sfr |= SFR_B_BIT;
                            consumed += 1;
                            pc = 0xB3B4;
                        }
                        0xB3B4 | 0xB3B6 => {
                            let reg = self.src_reg as usize;
                            let value = self.reg(reg as u8);
                            let carry_in = u16::from(self.condition_carry_set());
                            let result = (value << 1) | carry_in;
                            self.set_carry_flag((value & 0x8000) != 0);
                            self.write_reg(reg, result);
                            self.update_sign_zero_flags(result);
                            self.clear_prefix_flags();
                            consumed += 1;
                            pc = pc.wrapping_add(1);
                        }
                        0xB3B5 => {
                            self.with_reg = 5;
                            self.src_reg = 5;
                            self.dst_reg = 5;
                            self.sfr |= SFR_B_BIT;
                            consumed += 1;
                            pc = 0xB3B6;
                        }
                        0xB3B7 => {
                            let next_r12 = self.regs[12].wrapping_sub(1);
                            self.write_reg(12, next_r12);
                            self.update_sign_zero_flags(next_r12);
                            self.clear_prefix_flags();
                            consumed += 1;

                            if next_r12 != 0 {
                                let loop_target = self.regs[13];
                                if consumed.saturating_add(7) > step_budget_remaining {
                                    self.set_r15(loop_target);
                                    return Some(consumed);
                                }
                                self.set_r15(loop_target);
                                pc = loop_target;
                                continue;
                            }

                            self.set_r15(0xB3B8);
                            pc = 0xB3B8;
                        }
                        _ => break,
                    }

                    if !(0xB39D..=0xB3B7).contains(&pc) {
                        break;
                    }
                },
                0xB392..=0xB39C | 0xB3B8 => {
                    if pc == 0xB3B8 {
                        pc = 0xB392;
                    }

                    if pc <= 0xB392 {
                        self.clear_prefix_flags();
                        consumed += 1;
                        pc = 0xB393;
                    }

                    if pc <= 0xB393 {
                        let next_r1 = self.regs[1].wrapping_sub(1);
                        self.write_reg(1, next_r1);
                        self.update_sign_zero_flags(next_r1);
                        self.clear_prefix_flags();
                        consumed += 1;
                        pc = 0xB394;
                    }

                    if pc <= 0xB394 {
                        self.src_reg = 6;
                        consumed += 1;
                        pc = 0xB395;
                    }

                    if pc <= 0xB395 {
                        self.sfr = (self.sfr & !SFR_B_BIT) | SFR_ALT1_BIT;
                        consumed += 1;
                        pc = 0xB396;
                    }

                    if pc <= 0xB396 {
                        self.write_ram_byte(self.regs[1], self.regs[6] as u8);
                        self.clear_prefix_flags();
                        consumed += 1;
                        pc = 0xB397;
                    }

                    if pc <= 0xB397 {
                        let next_r4 = self.regs[4].wrapping_sub(1);
                        self.write_reg(4, next_r4);
                        self.update_sign_zero_flags(next_r4);
                        self.clear_prefix_flags();
                        consumed += 1;
                        pc = 0xB398;
                    }

                    if pc <= 0xB399 {
                        consumed += 2;
                        if self.regs[4] != 0 {
                            if consumed.saturating_add(4) > step_budget_remaining {
                                self.set_r15(0xB380);
                                return Some(consumed);
                            }

                            let lhs = self.regs[0];
                            let rhs = self.regs[7];
                            let sum = u32::from(lhs) + u32::from(rhs);
                            let result = sum as u16;
                            let overflow = ((!(lhs ^ rhs) & (rhs ^ result)) & 0x8000) != 0;
                            self.write_reg(0, result);
                            self.set_carry_flag(sum >= 0x1_0000);
                            self.set_overflow_flag(overflow);
                            self.update_sign_zero_flags(result);
                            self.clear_prefix_flags();

                            self.write_reg(12, 0x0008);
                            self.update_sign_zero_flags(0x0008);
                            self.clear_prefix_flags();
                            self.regs[13] = 0xB384;
                            self.set_r15(0xB384);
                            consumed += 4;
                            pc = 0xB384;
                            continue;
                        }
                    }

                    self.set_r15(0xB3C0);
                    consumed += 3;
                    return Some(consumed);
                }
                _ => return None,
            }
        }
    }

    pub fn run_status_poll_until_starfox_cached_delay_loop_exit(
        &mut self,
        rom: &[u8],
        max_steps: usize,
    ) {
        if !self.running || max_steps == 0 || !self.debug_in_starfox_cached_delay_loop() {
            return;
        }
        const STARFOX_DELAY_LOOP_FOLLOWUP_STEPS: usize = 1;

        let mut remaining = max_steps;
        while self.running && remaining > 0 {
            if self.fast_forward_starfox_cached_delay_loop() {
                // Keep chewing through the later Star Fox cached routine until
                // it either leaves the delay loop signature or exhausts the
                // poll budget. The caller is already in a busy-wait on $3030,
                // so stopping after an arbitrary small cycle count just turns
                // the same loop into many expensive polls.
            }
            let steps = remaining.min(STARFOX_DELAY_LOOP_FOLLOWUP_STEPS);
            if self.can_direct_run_starfox_late_wait() {
                self.run_steps_direct_no_pipe(rom, steps);
            } else {
                self.run_steps(rom, steps);
            }
            remaining -= steps;
        }
    }

    pub fn run_status_poll_until_stop_with_starfox_late_wait_assist(
        &mut self,
        rom: &[u8],
        max_steps: usize,
    ) {
        if !self.running || max_steps == 0 {
            return;
        }
        // The Star Fox late wait bounces in and out of the cached 01:000B
        // delay loop. Large chunks let it re-enter the loop and burn tens of
        // thousands of raw iterations before we can collapse it again.
        let starfox_late_wait_chunk = Self::status_poll_step_budget().saturating_mul(16).max(1);

        let mut remaining = max_steps;
        while self.running && remaining > 0 {
            if self.fast_forward_starfox_cached_delay_loop() {
                continue;
            }
            if let Some(consumed) = self.fast_forward_starfox_live_producer_store(rom, remaining) {
                remaining = remaining.saturating_sub(consumed);
                continue;
            }
            let steps = remaining.min(starfox_late_wait_chunk);
            if self.can_direct_run_starfox_late_wait() {
                self.run_steps_direct_no_pipe(rom, steps);
            } else {
                self.run_steps(rom, steps);
            }
            remaining -= steps;
        }
    }

    fn fast_forward_simple_store_inc_loop(
        &mut self,
        rom: &[u8],
        step_budget_remaining: usize,
    ) -> Option<usize> {
        if !self.running
            || step_budget_remaining < 4
            || !self.pipe_valid
            || self.pending_delay_pc.is_some()
            || self.pending_delay_pbr.is_some()
            || self.pending_delay_cache_base.is_some()
            || self.alt_mode() != 0
        {
            return None;
        }

        let pc = self.pipe_pc;
        let pbr = self.pipe_pbr;
        let opcode = self.pipe;
        if !(0x30..=0x3B).contains(&opcode) {
            return None;
        }
        let addr_reg = (opcode & 0x0F) as usize;
        if matches!(addr_reg, 12 | 13 | 15) || self.regs[12] <= 1 || self.regs[13] != pc {
            return None;
        }

        let Some(op1) = self.read_program_source_byte(rom, pbr, pc.wrapping_add(1)) else {
            return None;
        };
        let Some(op2) = self.read_program_source_byte(rom, pbr, pc.wrapping_add(2)) else {
            return None;
        };
        let Some(op3) = self.read_program_source_byte(rom, pbr, pc.wrapping_add(3)) else {
            return None;
        };
        let inc_opcode = 0xD0 | (addr_reg as u8);
        if op1 != inc_opcode || op2 != 0x3C || op3 != inc_opcode {
            return None;
        }

        let max_taken_iterations = self.regs[12].saturating_sub(1) as usize;
        let iterations = max_taken_iterations.min(step_budget_remaining / 4);
        if iterations == 0 {
            return None;
        }

        for _ in 0..iterations {
            let addr = self.regs[addr_reg];
            let src_value = self.reg(self.src_reg);
            self.write_ram_buffer_word(addr, src_value);

            let after_first_inc = addr.wrapping_add(1);
            self.write_reg(addr_reg, after_first_inc);

            let next_r12 = self.regs[12].wrapping_sub(1);
            self.write_reg(12, next_r12);

            let after_delay_inc = self.regs[addr_reg].wrapping_add(1);
            self.write_reg(addr_reg, after_delay_inc);
        }

        self.update_sign_zero_flags(self.regs[addr_reg]);
        self.clear_prefix_flags();
        self.set_r15(self.regs[13]);
        self.pipe = default_superfx_pipe();
        self.pipe_valid = false;
        self.pending_delay_pc = None;
        self.pending_delay_pbr = None;
        self.pending_delay_cache_base = None;

        Some(iterations * 4)
    }

    #[cfg(test)]
    pub(crate) fn debug_run_steps(&mut self, rom: &[u8], step_budget: usize) {
        self.run_steps(rom, step_budget);
    }

    #[cfg(test)]
    pub(crate) fn debug_set_reg(&mut self, index: usize, value: u16) {
        self.write_reg(index, value);
    }

    #[cfg(test)]
    pub(crate) fn debug_set_pbr(&mut self, value: u8) {
        self.pbr = value & 0x7F;
    }

    #[cfg(test)]
    pub(crate) fn debug_set_rombr(&mut self, value: u8) {
        self.rombr = value & (self.rom_bank_mask as u8);
    }

    #[cfg(test)]
    pub(crate) fn debug_set_scmr(&mut self, value: u8) {
        self.scmr = value & 0x3F;
    }

    #[cfg(test)]
    pub(crate) fn debug_set_sfr(&mut self, value: u16) {
        self.sfr = value;
    }

    #[cfg(test)]
    pub(crate) fn debug_set_src_reg(&mut self, value: u8) {
        self.src_reg = value & 0x0F;
    }

    #[cfg(test)]
    pub(crate) fn debug_set_dst_reg(&mut self, value: u8) {
        self.dst_reg = value & 0x0F;
    }

    #[cfg(test)]
    pub(crate) fn debug_set_with_reg(&mut self, value: u8) {
        self.with_reg = value & 0x0F;
    }

    #[cfg(test)]
    pub(crate) fn debug_clear_pipe(&mut self) {
        self.pipe = default_superfx_pipe();
        self.pipe_valid = false;
        self.pipe_pc = 0;
        self.pipe_pbr = self.pbr;
        self.r14_modified = false;
        self.r15_modified = false;
    }

    fn rewind_pipe_to_instruction_boundary(&mut self, exec_pbr: u8, pc: u16, opcode: u8) {
        self.pbr = exec_pbr & 0x7F;
        self.pipe = opcode;
        self.pipe_valid = true;
        self.pipe_pc = pc;
        self.pipe_pbr = self.pbr;
        // Match the normal pipelined state immediately after fetching `opcode`.
        self.regs[15] = pc.wrapping_add(1);
        self.r14_modified = false;
        self.r15_modified = false;
    }

    #[cfg(test)]
    pub(crate) fn debug_invoke_cpu_start(&mut self, rom: &[u8]) {
        self.invoke_cpu_start(rom);
    }

    #[cfg(test)]
    pub(crate) fn debug_prepare_cpu_start(&mut self, rom: &[u8]) {
        let _ = self.prepare_start_execution(rom);
    }

    fn prepare_start_execution(&mut self, rom: &[u8]) -> bool {
        if !Self::experimental_core_enabled() || rom.is_empty() {
            self.finish_noop_run();
            return false;
        }
        if self.regs[15] < CACHE_RAM_SIZE as u16 && self.cache_ram.iter().any(|&byte| byte != 0) {
            self.cache_enabled = true;
            self.cache_valid_mask = u32::MAX;
        }
        self.running = true;
        self.pipe = default_superfx_pipe();
        self.pipe_valid = false;
        self.r14_modified = false;
        self.r15_modified = false;
        true
    }

    fn start_execution(&mut self, rom: &[u8]) {
        if !self.prepare_start_execution(rom) {
            return;
        }
        self.run_steps(rom, Self::startup_step_budget());
    }

    pub fn run_for_cpu_cycles(&mut self, rom: &[u8], cpu_cycles: u8) {
        if !self.running || cpu_cycles == 0 {
            return;
        }
        let steps = (cpu_cycles as usize).saturating_mul(self.steps_per_cpu_cycle());
        self.run_steps(rom, steps);
    }

    fn run_steps(&mut self, rom: &[u8], step_budget: usize) {
        if rom.is_empty() {
            self.finish_noop_run();
            return;
        }

        self.sfr |= SFR_GO_BIT | SFR_R_BIT;
        self.running = true;

        let detailed_trace = trace_superfx_reg_flow_enabled()
            || trace_superfx_pc_trace_enabled()
            || trace_superfx_profile_enabled();

        let mut steps = 0usize;
        let mut instruction_count = 0usize;
        while self.running && steps < step_budget {
            if !self.pipe_valid {
                if self.prime_pipe(rom).is_none() {
                    self.trace_abort("prime-pipe", self.regs[15], 0xFF);
                    self.finish_noop_run();
                    return;
                }
            }

            if !detailed_trace {
                if let Some(consumed_steps) =
                    self.fast_forward_simple_store_inc_loop(rom, step_budget - steps)
                {
                    instruction_count += consumed_steps;
                    steps += consumed_steps;
                    continue;
                }
            }

            if self.pending_delay_pc.is_some()
                || self.pending_delay_pbr.is_some()
                || self.pending_delay_cache_base.is_some()
            {
                self.apply_pending_delay_transfer();
            }

            self.last_opcode_cycles = 1;
            let pc = self.pipe_pc;
            let exec_pbr = self.pipe_pbr;
            let opcode = self.pipe;
            if self.prefetch_pipe(rom).is_none() {
                self.trace_abort("fetch-pipe", pc, opcode);
                self.finish_noop_run();
                return;
            }

            self.current_exec_pbr = exec_pbr;
            self.current_exec_pc = pc;
            self.current_exec_opcode = opcode;
            if starfox_b384_preexec_debug_override_enabled() {
                self.maybe_force_starfox_b384_preexec_live_state(pc);
            }
            self.trace_exec_watch(exec_pbr, pc, opcode);
            self.push_recent_exec_trace(exec_pbr, pc, opcode);
            let frame_matches =
                trace_superfx_exec_frame_matches(u64::from(current_trace_superfx_frame()));
            if let Some((bank, start, end)) = *save_state_at_gsu_pc_range() {
                if frame_matches
                    && exec_pbr == bank
                    && pc >= start
                    && pc <= end
                    && save_state_at_gsu_reg_eq_matches(self)
                    && save_state_at_gsu_recent_exec_tail_matches(self)
                {
                    self.save_state_pc_hit_count = self.save_state_pc_hit_count.saturating_add(1);
                    if self.save_state_pc_hit.is_none()
                        && self.save_state_pc_hit_count >= save_state_at_gsu_pc_hit_index()
                    {
                        self.rewind_pipe_to_instruction_boundary(exec_pbr, pc, opcode);
                        self.save_state_pc_hit = Some((exec_pbr, pc));
                        self.total_run_instructions += instruction_count as u64;
                        return;
                    }
                }
            }

            if !self.execute_opcode(opcode, rom, pc) {
                if opcode == 0x00 {
                    if self.r15_modified {
                        self.r15_modified = false;
                    } else {
                        self.regs[15] = self.regs[15].wrapping_add(1);
                    }
                    instruction_count += 1;
                    steps += self.last_opcode_cycles;
                }
                self.total_run_instructions += instruction_count as u64;
                if trace_superfx_start_enabled() {
                    eprintln!(
                        "[SFX-STOP] steps={} instructions={} total={} r15={:04X} pbr={:02X} budget={}",
                        steps,
                        instruction_count,
                        self.total_run_instructions,
                        self.regs[15],
                        self.pbr,
                        step_budget
                    );
                }
                self.finish_noop_run();
                return;
            }
            if self.r15_modified {
                self.r15_modified = false;
            } else {
                self.regs[15] = self.regs[15].wrapping_add(1);
            }
            instruction_count += 1;
            steps += self.last_opcode_cycles;

            if self.save_state_pc_hit.is_some()
                && save_state_at_gsu_reg_write().is_some()
                && save_state_at_gsu_pc_range().is_none()
            {
                self.total_run_instructions += instruction_count as u64;
                return;
            }

            // Diagnostic save hooks must stop immediately after the matching store.
            // Otherwise the rest of the current run slice keeps mutating registers,
            // which makes RAM-hit exact captures unusable.
            if self.save_state_ram_addr_hit.is_some() {
                self.total_run_instructions += instruction_count as u64;
                return;
            }

            if detailed_trace {
                self.record_profile(opcode);
                if trace_superfx_pc_trace_enabled() {
                    let total = self.total_run_instructions + instruction_count as u64;
                    if total <= 500 || (total.is_multiple_of(100_000)) {
                        eprintln!(
                            "[SFX-PC] #{} pc={:04X} op={:02X} R0={:04X} R1={:04X} R2={:04X} R3={:04X} R4={:04X} R5={:04X} R6={:04X} R9={:04X} R12={:04X} SFR={:04X}",
                            total,
                            pc, opcode,
                            self.regs[0], self.regs[1], self.regs[2], self.regs[3],
                            self.regs[4], self.regs[5], self.regs[6], self.regs[9], self.regs[12], self.sfr
                        );
                    }
                }
            }
        }
        self.total_run_instructions += instruction_count as u64;
        self.sync_ram_buffer();
        if self.running && trace_superfx_start_enabled() && step_budget >= 1024 {
            eprintln!(
                "[SFX-BUDGET] steps={} instr={} total={} r15={:04X} pbr={:02X} budget={} scmr={:02X}",
                steps, instruction_count, self.total_run_instructions, self.regs[15], self.pbr, step_budget, self.scmr
            );
        }
    }

    fn note_pc_transfer(&mut self, opcode: u8, pc: u16, to_pc: u16) {
        self.record_pc_transfer(opcode, pc, self.regs[15], to_pc);
    }

    fn execute_opcode(&mut self, opcode: u8, rom: &[u8], pc: u16) -> bool {
        let ok = self.execute_opcode_internal(opcode, rom, pc, false);
        if ok && self.r14_modified && self.refresh_rom_buffer_if_needed(rom).is_none() {
            self.trace_abort("refresh-r14-rom-buffer", pc, opcode);
            self.finish_noop_run();
            return false;
        }
        if ok && !self.running {
            self.sync_ram_buffer();
        }
        ok
    }

    fn execute_opcode_internal(
        &mut self,
        opcode: u8,
        rom: &[u8],
        pc: u16,
        _in_delay_slot: bool,
    ) -> bool {
        self.current_exec_pc = pc;
        self.current_exec_opcode = opcode;
        self.maybe_capture_debug_screen_snapshot(pc);
        if let Some((bank, start, end)) = *trace_superfx_pc_range_raw() {
            if self.current_exec_pbr == bank && pc >= start && pc <= end {
                eprintln!(
                    "[SFX-PC-RAW] {:02X}:{:04X} op={:02X} src=r{}({:04X}) dst=r{}({:04X}) r0={:04X} r1={:04X} r2={:04X} r3={:04X} r4={:04X} r5={:04X} r6={:04X} r7={:04X} r8={:04X} r9={:04X} r10={:04X} r11={:04X} r12={:04X} r13={:04X} r14={:04X} r15={:04X} sfr={:04X}",
                    self.current_exec_pbr,
                    pc,
                    opcode,
                    self.src_reg,
                    self.reg(self.src_reg),
                    self.dst_reg,
                    self.reg(self.dst_reg),
                    self.regs[0],
                    self.regs[1],
                    self.regs[2],
                    self.regs[3],
                    self.regs[4],
                    self.regs[5],
                    self.regs[6],
                    self.regs[7],
                    self.regs[8],
                    self.regs[9],
                    self.regs[10],
                    self.regs[11],
                    self.regs[12],
                    self.regs[13],
                    self.regs[14],
                    self.regs[15],
                    self.sfr
                );
                if trace_superfx_pc_last_writers_enabled() {
                    eprintln!(
                        "[SFX-PC-LAST] {:02X}:{:04X} r0={:?} r1={:?} r2={:?} r3={:?} r4={:?} r6={:?} r10={:?} r11={:?} r12={:?} r14={:?}",
                        self.current_exec_pbr,
                        pc,
                        self.debug_last_reg_write_excluding(
                            0,
                            &[
                                0x03, 0xC0, 0xC1, 0xC2, 0xC3, 0xC4, 0xC5, 0xC6, 0xC7, 0xC8,
                                0xC9, 0xCA, 0xCB, 0xCC, 0xCD, 0xCE, 0xCF,
                            ],
                        ),
                        self.debug_last_reg_write_excluding(1, &[0xD1, 0xE1]),
                        self.debug_last_nontrivial_reg_write(2),
                        self.debug_last_nontrivial_reg_write(3),
                        self.debug_last_reg_write_excluding(4, &[0x04, 0xE4]),
                        self.debug_last_nontrivial_reg_write(6),
                        self.debug_last_nontrivial_reg_write(10),
                        self.debug_last_nontrivial_reg_write(11),
                        self.debug_last_nontrivial_reg_write(12),
                        self.debug_last_nontrivial_reg_write(14)
                    );
                }
            }
        }
        match opcode {
            0x00 => {
                self.sync_ram_buffer();
                // STOP: flush pixel caches and update CBR before halting
                self.flush_all_pixel_caches();
                if self.scbr > 0 {
                    let scbr_base = (self.scbr as usize) << 10;
                    let tile_size = self.screen_buffer_len().unwrap_or(0);
                    let height = self.effective_screen_height().unwrap_or(0) as u16;
                    let bpp = self.bits_per_pixel().unwrap_or(0) as u8;
                    let mode = self.effective_screen_layout_mode();
                    let end = scbr_base.saturating_add(tile_size).min(self.game_ram.len());
                    if scbr_base < end {
                        let snapshot = self.game_ram[scbr_base..end].to_vec();
                        self.latest_stop_snapshot = snapshot.clone();
                        self.latest_stop_snapshot_valid = true;
                        self.latest_stop_scbr = self.scbr;
                        self.latest_stop_height = height;
                        self.latest_stop_bpp = bpp;
                        self.latest_stop_mode = mode;
                        self.latest_stop_pc = pc;
                        self.latest_stop_pbr = self.pbr;
                        if self.recent_stop_snapshots.len() >= 64 {
                            self.recent_stop_snapshots.remove(0);
                        }
                        self.recent_stop_snapshots.push(StopSnapshot {
                            data: snapshot.clone(),
                            scbr: self.scbr,
                            height,
                            bpp,
                            mode,
                            pc,
                            pbr: self.pbr,
                        });
                        if trace_superfx_stop_captures_enabled() {
                            let nonzero = snapshot.iter().filter(|&&b| b != 0).count();
                            let mut hasher = DefaultHasher::new();
                            snapshot.hash(&mut hasher);
                            eprintln!(
                                "[SFX-STOP-CAPTURE] pbr={:02X} pc={:04X} scbr={:02X} scmr={:02X} len={} nonzero={} hash={:016X} dma_capture={}",
                                self.pbr,
                                pc,
                                self.scbr,
                                self.scmr,
                                snapshot.len(),
                                nonzero,
                                hasher.finish(),
                                if self.pbr == 0x01 && (0xB300..=0xB400).contains(&pc) { 1 } else { 0 }
                            );
                        }
                    }
                }
                // Snapshot tile buffer after main renderer completes.
                // B301 renderer stops near B3E5. Snapshot preserves polygon data
                // before B0CB buffer clear erases it.
                if self.scbr > 0 && self.pbr == 0x01 && pc >= 0xB300 && pc <= 0xB400 {
                    let scbr_base = (self.scbr as usize) << 10;
                    let tile_size = self.screen_buffer_len().unwrap_or(0);
                    let height = self.effective_screen_height().unwrap_or(0) as u16;
                    let bpp = self.bits_per_pixel().unwrap_or(0) as u8;
                    let mode = self.effective_screen_layout_mode();
                    let end = scbr_base.saturating_add(tile_size).min(self.game_ram.len());
                    if scbr_base < end {
                        self.tile_snapshot = self.game_ram[scbr_base..end].to_vec();
                        self.tile_snapshot_valid = true;
                        self.tile_snapshot_scbr = self.scbr;
                        self.tile_snapshot_height = height;
                        self.tile_snapshot_bpp = bpp;
                        self.tile_snapshot_mode = mode;
                        self.tile_snapshot_pc = pc;
                        self.tile_snapshot_pbr = self.pbr;
                        if self.recent_tile_snapshots.len() >= 64 {
                            self.recent_tile_snapshots.remove(0);
                        }
                        self.recent_tile_snapshots.push(StopSnapshot {
                            data: self.tile_snapshot.clone(),
                            scbr: self.scbr,
                            height,
                            bpp,
                            mode,
                            pc,
                            pbr: self.pbr,
                        });
                        if trace_superfx_tile_captures_enabled() {
                            let nonzero = self.tile_snapshot.iter().filter(|&&b| b != 0).count();
                            let mut hasher = DefaultHasher::new();
                            self.tile_snapshot.hash(&mut hasher);
                            eprintln!(
                                "[SFX-TILE-CAPTURE] pbr={:02X} pc={:04X} scbr={:02X} scmr={:02X} len={} nonzero={} hash={:016X}",
                                self.pbr,
                                pc,
                                self.scbr,
                                self.scmr,
                                self.tile_snapshot.len(),
                                nonzero,
                                hasher.finish(),
                            );
                        }
                    }
                }
                self.por = 0;
                self.clear_prefix_flags();
                self.cbr = self.regs[15] & 0xFFF0;
                self.cache_valid_mask = 0;
                self.finish_noop_run();
                return false;
            }
            0x01 => self.clear_prefix_flags(),
            0x02 => {
                // CACHE follows the prefetched R15 window, matching snes9x's `fx_cache`.
                // The later Star Fox cached routine at 01:84FB executes from cache page
                // 000B, which requires CBR=84F0 after CACHE at 01:84EE.
                let next_cbr = self.regs[15] & 0xFFF0;
                if !self.cache_enabled || self.cbr != next_cbr {
                    self.cbr = next_cbr;
                    self.cache_valid_mask = 0;
                }
                self.cache_enabled = true;
                self.clear_prefix_flags();
            }
            0x03 => {
                let value = self.reg(self.src_reg);
                let result = value >> 1;
                self.set_carry_flag((value & 0x0001) != 0);
                self.write_dest_exec(result, opcode, pc);
                self.update_sign_zero_flags(result);
                self.clear_prefix_flags();
            }
            0x04 => {
                let value = self.reg(self.src_reg);
                let carry_in = u16::from(self.condition_carry_set());
                let result = (value << 1) | carry_in;
                self.set_carry_flag((value & 0x8000) != 0);
                self.write_dest_exec(result, opcode, pc);
                self.update_sign_zero_flags(result);
                self.clear_prefix_flags();
            }
            0x05..=0x0F => {
                let rel = match self.fetch_opcode(rom) {
                    Some(value) => value as i8 as i16,
                    None => {
                        self.trace_abort("branch-fetch", pc, opcode);
                        self.finish_noop_run();
                        return false;
                    }
                };
                let taken = match opcode {
                    0x05 => true,
                    0x06 => self.condition_sign_set() == self.condition_overflow_set(),
                    0x07 => self.condition_sign_set() != self.condition_overflow_set(),
                    0x08 => !self.condition_zero_set(),
                    0x09 => self.condition_zero_set(),
                    0x0A => !self.condition_sign_set(),
                    0x0B => self.condition_sign_set(),
                    0x0C => !self.condition_carry_set(),
                    0x0D => self.condition_carry_set(),
                    0x0E => !self.condition_overflow_set(),
                    0x0F => self.condition_overflow_set(),
                    _ => unreachable!(),
                };
                if taken {
                    let to_pc = self.reg(15).wrapping_add_signed(rel);
                    self.note_pc_transfer(opcode, pc, to_pc);
                    self.set_r15(to_pc);
                }
            }
            0x10..=0x1F => {
                let reg = opcode & 0x0F;
                if (self.sfr & SFR_B_BIT) != 0 {
                    let value = self.reg(self.src_reg);
                    self.write_reg_exec(reg as usize, value, opcode, pc);
                    self.sfr &= !SFR_B_BIT;
                    self.clear_prefix_flags();
                } else {
                    self.dst_reg = reg;
                }
            }
            0x20..=0x2F => {
                let reg = opcode & 0x0F;
                self.with_reg = reg;
                self.src_reg = reg;
                self.dst_reg = reg;
                self.sfr |= SFR_B_BIT;
            }
            0x30..=0x3B => {
                let addr_reg = opcode & 0x0F;
                let addr = self.reg(addr_reg);
                let src_value = self.reg(self.src_reg);
                match self.alt_mode() {
                    0 => {
                        self.write_ram_buffer_word(addr, src_value);
                    }
                    1 => {
                        let value = src_value as u8;
                        self.write_ram_buffer_byte(addr, value);
                    }
                    2 => {
                        // ALT2 undefined for STW range; fall back to STW
                        self.write_ram_buffer_word(addr, src_value);
                    }
                    3 => {
                        // ALT3 undefined for STB range; fall back to STB
                        let value = src_value as u8;
                        self.write_ram_buffer_byte(addr, value);
                    }
                    _ => unreachable!(),
                }
                self.clear_prefix_flags();
            }
            0x3C => {
                // LOOP: decrement full 16-bit R12.
                // bsnes keeps the already-prefetched sequential byte live, so
                // a taken LOOP still exposes one delay-slot instruction.
                let next_r12 = self.regs[12].wrapping_sub(1);
                self.write_reg_exec(12, next_r12, opcode, pc);
                let zero = self.regs[12] == 0;
                if !zero {
                    self.note_pc_transfer(opcode, pc, self.regs[13]);
                    self.set_r15(self.regs[13]);
                }
                // S = bit 15 of R12, Z = R12 == 0
                self.update_sign_zero_flags(self.regs[12]);
                self.clear_prefix_flags();
            }
            0x3D => {
                self.sfr = (self.sfr & !SFR_B_BIT) | SFR_ALT1_BIT;
            }
            0x3E => {
                self.sfr = (self.sfr & !SFR_B_BIT) | SFR_ALT2_BIT;
            }
            0x3F => {
                self.sfr = (self.sfr & !SFR_B_BIT) | SFR_ALT1_BIT | SFR_ALT2_BIT;
            }
            0x40..=0x4B => {
                let addr = self.reg(opcode & 0x0F);
                let value = if self.alt_mode() == 1 {
                    self.read_ram_byte(addr) as u16
                } else {
                    self.read_ram_word(addr)
                };
                self.write_dest_exec(value, opcode, pc);
                self.clear_prefix_flags();
            }
            0x4C => {
                if matches!(self.alt_mode(), 1 | 3) {
                    // RPIX: flush pixel caches before reading
                    self.flush_all_pixel_caches();
                    let value = self.read_plot_pixel(self.regs[1], self.regs[2]) as u16;
                    self.write_dest_exec(value, opcode, pc);
                    if self.bits_per_pixel() == Some(8) {
                        self.set_zero_flag(value == 0);
                    }
                } else {
                    // PLOT: write pixel
                    if env_presence_cached("TRACE_PLOT_COUNT") {
                        use std::sync::atomic::{AtomicU64, Ordering};
                        static TOTAL: AtomicU64 = AtomicU64::new(0);
                        let t = TOTAL.fetch_add(1, Ordering::Relaxed);
                        // Show first 30 PLOT calls to see coordinate patterns
                        if t < 30 {
                            eprintln!(
                                "[PLOT] #{} x={} y={} c={} r15={:04X}",
                                t, self.regs[1], self.regs[2], self.colr, self.regs[15]
                            );
                        }
                    }
                    self.plot_pixel(self.regs[1], self.regs[2], self.colr);
                    self.regs[1] = self.regs[1].wrapping_add(1);
                }
                self.clear_prefix_flags();
            }
            0x4D => {
                let value = self.reg(self.src_reg).rotate_left(8);
                self.write_dest_exec(value, opcode, pc);
                self.update_sign_zero_flags(value);
                self.clear_prefix_flags();
            }
            0x4E => {
                if matches!(self.alt_mode(), 1 | 3) {
                    self.por = self.reg(self.src_reg) as u8;
                } else {
                    let color = self.reg(self.src_reg) as u8;
                    self.colr = self.apply_color(color);
                }
                self.clear_prefix_flags();
            }
            0x4F => {
                let value = !self.reg(self.src_reg);
                self.write_dest_exec(value, opcode, pc);
                self.update_sign_zero_flags(value);
                self.clear_prefix_flags();
            }
            0x50..=0x5F => {
                let lhs = self.reg(self.src_reg);
                let rhs = match self.alt_mode() {
                    2 | 3 => (opcode & 0x0F) as u16,
                    _ => self.reg(opcode & 0x0F),
                };
                let carry_in =
                    i32::from(matches!(self.alt_mode(), 1 | 3) && self.condition_carry_set());
                // snes9x uses SUSEX16 here: promote uint16 to int32 without sign-extending,
                // then derive carry from the 17th bit of the unsigned sum.
                let lhs_i32 = i32::from(lhs);
                let rhs_i32 = if matches!(self.alt_mode(), 2 | 3) {
                    i32::from((opcode & 0x0F) as u16)
                } else {
                    i32::from(rhs)
                };
                let sum = lhs_i32 + rhs_i32 + carry_in;
                let result = sum as u16;
                let overflow = ((!(lhs ^ rhs) & (rhs ^ result)) & 0x8000) != 0;
                self.set_carry_flag(sum >= 0x1_0000);
                self.set_overflow_flag(overflow);
                self.write_dest_exec(result, opcode, pc);
                self.update_sign_zero_flags(result);
                self.clear_prefix_flags();
            }
            0x60..=0x6F => {
                let lhs = self.reg(self.src_reg);
                let alt = self.alt_mode();
                let rhs = if alt == 2 {
                    (opcode & 0x0F) as u16
                } else {
                    self.reg(opcode & 0x0F)
                };
                // snes9x uses SUSEX16 here as well: promote uint16 to int32
                // without sign-extending, then derive carry from diff >= 0.
                let lhs_i32 = i32::from(lhs);
                let rhs_i32 = if alt == 2 {
                    i32::from((opcode & 0x0F) as u16)
                } else {
                    i32::from(rhs)
                };
                let borrow = i32::from(alt == 1 && !self.condition_carry_set());
                let diff = lhs_i32 - rhs_i32 - borrow;
                let result = diff as u16;
                let overflow = (((lhs ^ rhs) & (lhs ^ result)) & 0x8000) != 0;
                self.set_carry_flag(diff >= 0);
                self.set_overflow_flag(overflow);
                self.update_sign_zero_flags(result);
                if alt != 3 {
                    self.write_dest_exec(result, opcode, pc);
                }
                self.clear_prefix_flags();
            }
            0x70..=0x7F => {
                if opcode == 0x70 {
                    // MERGE: combine high bytes of R7 (→ result high) and R8 (→ result low)
                    let value = (self.regs[7] & 0xFF00) | ((self.regs[8] >> 8) & 0x00FF);
                    self.write_dest_exec(value, opcode, pc);
                    // Flags per bsnes: bitmask-based
                    let s = (value & 0x8080) != 0;
                    let z = (value & 0xF0F0) == 0;
                    let cy = (value & 0xE0E0) != 0;
                    let ov = (value & 0xC0C0) != 0;
                    self.set_sign_flag(s);
                    self.set_zero_flag(z);
                    self.set_carry_flag(cy);
                    self.set_overflow_flag(ov);
                    self.clear_prefix_flags();
                } else {
                    let lhs = self.reg(self.src_reg);
                    let rhs = if matches!(self.alt_mode(), 2 | 3) {
                        (opcode & 0x0F) as u16
                    } else {
                        self.reg(opcode & 0x0F)
                    };
                    let result = if matches!(self.alt_mode(), 1 | 3) {
                        lhs & !rhs
                    } else {
                        lhs & rhs
                    };
                    self.write_dest_exec(result, opcode, pc);
                    self.update_sign_zero_flags(result);
                    self.clear_prefix_flags();
                }
            }
            0x80..=0x8F => {
                let lhs = self.reg(self.src_reg);
                let rhs = if matches!(self.alt_mode(), 2 | 3) {
                    (opcode & 0x0F) as u16
                } else {
                    self.reg(opcode & 0x0F)
                };
                let product = if matches!(self.alt_mode(), 1 | 3) {
                    (lhs as u32 & 0x00FF) * (rhs as u32 & 0x00FF)
                } else {
                    ((lhs as i8 as i16 as i32) * (rhs as i8 as i16 as i32)) as u32
                };
                let result = product as u16;
                self.write_dest_exec(result, opcode, pc);
                self.update_sign_zero_flags(result);
                self.clear_prefix_flags();
            }
            0xA0..=0xAF => {
                let reg = (opcode & 0x0F) as usize;
                let imm = match self.fetch_opcode(rom) {
                    Some(value) => value,
                    None => {
                        self.trace_abort("ibt-fetch", pc, opcode);
                        self.finish_noop_run();
                        return false;
                    }
                };
                let trace_lms = trace_superfx_getb_enabled()
                    || trace_superfx_pc_range_raw_matches(self.pbr, pc);
                match self.alt_mode() {
                    1 | 3 => {
                        let addr = (imm as u16) << 1;
                        let value = self.read_ram_word_short(addr);
                        if trace_lms {
                            use std::sync::atomic::{AtomicU32, Ordering};
                            static COUNT: OnceLock<AtomicU32> = OnceLock::new();
                            let n = COUNT
                                .get_or_init(|| AtomicU32::new(0))
                                .fetch_add(1, Ordering::Relaxed);
                            if n < 256 {
                                println!(
                                    "[SFX-LMS] pc={:02X}:{:04X} reg=r{} imm={:02X} addr={:04X} value={:04X}",
                                    self.pbr, pc, reg, imm, addr, value
                                );
                            }
                        }
                        self.write_reg_exec(reg, value, opcode, pc);
                    }
                    2 => {
                        let addr = (imm as u16) << 1;
                        if trace_lms {
                            use std::sync::atomic::{AtomicU32, Ordering};
                            static COUNT: OnceLock<AtomicU32> = OnceLock::new();
                            let n = COUNT
                                .get_or_init(|| AtomicU32::new(0))
                                .fetch_add(1, Ordering::Relaxed);
                            if n < 256 {
                                println!(
                                    "[SFX-SMS] pc={:02X}:{:04X} reg=r{} imm={:02X} addr={:04X} value={:04X}",
                                    self.pbr,
                                    pc,
                                    reg,
                                    imm,
                                    addr,
                                    self.regs[reg]
                                );
                            }
                        }
                        self.write_ram_buffer_word_short(addr, self.regs[reg]);
                    }
                    _ => {
                        let value = imm as i8 as i16 as u16;
                        self.write_reg_exec(reg, value, opcode, pc);
                    }
                }
                self.clear_prefix_flags();
            }
            0xB0..=0xBF => {
                let reg = opcode & 0x0F;
                if (self.sfr & SFR_B_BIT) != 0 {
                    // MOVES copies the full word and sets flags from the moved datum.
                    // Overflow follows bit 7 while sign/zero follow the 16-bit result.
                    let value = self.reg(reg);
                    self.write_dest_exec(value, opcode, pc);
                    self.update_sign_zero_flags(value);
                    self.set_overflow_flag((value & 0x0080) != 0);
                    self.clear_prefix_flags();
                } else {
                    self.src_reg = reg;
                }
            }
            0xC0 => {
                let value = (self.reg(self.src_reg) >> 8) & 0x00FF;
                self.write_dest_exec(value, opcode, pc);
                self.update_sign_zero_flags(value << 8);
                self.clear_prefix_flags();
            }
            0xC1..=0xCF => {
                let lhs = self.reg(self.src_reg);
                let rhs = if matches!(self.alt_mode(), 2 | 3) {
                    (opcode & 0x0F) as u16
                } else {
                    self.reg(opcode & 0x0F)
                };
                let result = if matches!(self.alt_mode(), 1 | 3) {
                    lhs ^ rhs
                } else {
                    lhs | rhs
                };
                self.write_dest_exec(result, opcode, pc);
                self.update_sign_zero_flags(result);
                self.clear_prefix_flags();
            }
            0xD0..=0xDE => {
                let reg = (opcode & 0x0F) as usize;
                let value = self.regs[reg].wrapping_add(1);
                self.write_reg_exec(reg, value, opcode, pc);
                self.update_sign_zero_flags(self.regs[reg]);
                self.clear_prefix_flags();
            }
            0xDF => {
                let src_reg = self.src_reg;
                let src_value = self.reg(src_reg);
                match self.alt_mode() {
                    0 => {
                        let value = match self.read_data_rom_byte(rom) {
                            Some(value) => value,
                            None => {
                                self.trace_abort("getc", pc, opcode);
                                self.finish_noop_run();
                                return false;
                            }
                        };
                        self.colr = self.apply_color(value);
                    }
                    2 => {
                        self.sync_ram_buffer();
                        self.rambr = (src_value & 0x03) as u8;
                        if trace_superfx_getb_enabled() {
                            use std::sync::atomic::{AtomicU32, Ordering};
                            static COUNT: OnceLock<AtomicU32> = OnceLock::new();
                            let n = COUNT
                                .get_or_init(|| AtomicU32::new(0))
                                .fetch_add(1, Ordering::Relaxed);
                            if n < 256 {
                                println!(
                                    "[SFX-RAMB] pc={:02X}:{:04X} src=r{}({:04X}) rambr={:02X}",
                                    self.pbr, pc, src_reg, src_value, self.rambr
                                );
                            }
                        }
                    }
                    3 => {
                        if self.rom_buffer_pending && self.fill_rom_buffer(rom).is_none() {
                            self.trace_abort("romb-sync", pc, opcode);
                            self.finish_noop_run();
                            return false;
                        }
                        self.rombr = (src_value & 0x7F) as u8;
                        if trace_superfx_getb_enabled() {
                            use std::sync::atomic::{AtomicU32, Ordering};
                            static COUNT: OnceLock<AtomicU32> = OnceLock::new();
                            let n = COUNT
                                .get_or_init(|| AtomicU32::new(0))
                                .fetch_add(1, Ordering::Relaxed);
                            if n < 256 {
                                println!(
                                    "[SFX-ROMB] pc={:02X}:{:04X} src=r{}({:04X}) rombr={:02X}",
                                    self.pbr, pc, src_reg, src_value, self.rombr
                                );
                            }
                        }
                    }
                    1 => {
                        // ALT1 + GETC undefined; fall back to GETC behavior
                        let value = match self.read_data_rom_byte(rom) {
                            Some(value) => value,
                            None => {
                                self.trace_abort("getc-alt1", pc, opcode);
                                self.finish_noop_run();
                                return false;
                            }
                        };
                        self.colr = self.apply_color(value);
                    }
                    _ => unreachable!(),
                }
                self.clear_prefix_flags();
            }
            0xE0..=0xEE => {
                let reg = (opcode & 0x0F) as usize;
                let value = self.regs[reg].wrapping_sub(1);
                self.write_reg_exec(reg, value, opcode, pc);
                self.update_sign_zero_flags(self.regs[reg]);
                self.clear_prefix_flags();
            }
            0xEF => {
                let r14_before = self.regs[14];
                let value = match self.read_data_rom_byte(rom) {
                    Some(value) => value,
                    None => {
                        self.trace_abort("getb", pc, opcode);
                        self.finish_noop_run();
                        return false;
                    }
                };
                let alt_mode = self.alt_mode();
                let src_reg = self.src_reg;
                let dst_reg = self.dst_reg;
                let src_value = self.reg(src_reg);
                let result = match alt_mode {
                    0 => value as u16,
                    1 => {
                        let low = src_value & 0x00FF;
                        ((value as u16) << 8) | low
                    }
                    2 => {
                        let high = src_value & 0xFF00;
                        high | value as u16
                    }
                    3 => value as i8 as i16 as u16,
                    _ => unreachable!(),
                };
                self.write_dest_exec(result, opcode, pc);
                if trace_superfx_getb_enabled() {
                    use std::sync::atomic::{AtomicU32, Ordering};
                    static COUNT: OnceLock<AtomicU32> = OnceLock::new();
                    let n = COUNT
                        .get_or_init(|| AtomicU32::new(0))
                        .fetch_add(1, Ordering::Relaxed);
                    if n < 256 {
                        println!(
                            "[SFX-GETB] pc={:02X}:{:04X} alt={} rombr={:02X} r14={:04X}->{:04X} byte={:02X} src=r{}({:04X}) dst=r{} result={:04X}",
                            self.pbr,
                            pc,
                            alt_mode,
                            self.rombr,
                            r14_before,
                            self.regs[14],
                            value,
                            src_reg,
                            src_value,
                            dst_reg,
                            result
                        );
                    }
                }
                self.clear_prefix_flags();
            }
            0xF0..=0xFF => {
                let reg = (opcode & 0x0F) as usize;
                let lo = match self.fetch_opcode(rom) {
                    Some(value) => value,
                    None => {
                        self.trace_abort("iwt-lo", pc, opcode);
                        self.finish_noop_run();
                        return false;
                    }
                };
                let hi = match self.fetch_opcode(rom) {
                    Some(value) => value,
                    None => {
                        self.trace_abort("iwt-hi", pc, opcode);
                        self.finish_noop_run();
                        return false;
                    }
                };
                let addr_or_imm = u16::from_le_bytes([lo, hi]);
                if env_presence_cached("TRACE_SUPERFX_IWT") && reg == 12 {
                    use std::sync::atomic::{AtomicU32, Ordering};
                    static COUNT: OnceLock<AtomicU32> = OnceLock::new();
                    let n = COUNT
                        .get_or_init(|| AtomicU32::new(0))
                        .fetch_add(1, Ordering::Relaxed);
                    if n < 128 {
                        println!(
                            "[SFX-IWT] pc={:02X}:{:04X} op={:02X} alt={} reg=r{} lo={:02X} hi={:02X} word={:04X} src=r{} dst=r{} sfr={:04X}",
                            self.pbr,
                            pc,
                            opcode,
                            self.alt_mode(),
                            reg,
                            lo,
                            hi,
                            addr_or_imm,
                            self.src_reg,
                            self.dst_reg,
                            self.sfr
                        );
                    }
                }
                match self.alt_mode() {
                    0 => {
                        // IWT Rn, #xxxx — load immediate word
                        if reg == 15 {
                            self.write_reg_exec(15, addr_or_imm, opcode, pc);
                        } else {
                            self.write_reg_exec(reg, addr_or_imm, opcode, pc);
                        }
                        self.clear_prefix_flags();
                    }
                    1 | 3 => {
                        // LM Rn, (xxxx) — load word from RAM
                        let value = self.read_ram_word(addr_or_imm);
                        if reg == 15 {
                            self.write_reg_exec(15, value, opcode, pc);
                        } else {
                            self.write_reg_exec(reg, value, opcode, pc);
                        }
                        self.clear_prefix_flags();
                    }
                    2 => {
                        // SM (xxxx), Rn — store word to RAM
                        self.write_ram_buffer_word(addr_or_imm, self.regs[reg]);
                        self.clear_prefix_flags();
                    }
                    _ => unreachable!(),
                }
            }
            0x90 => {
                let value = self.reg(self.src_reg);
                self.write_ram_buffer_word(self.last_ram_addr, value);
                self.clear_prefix_flags();
            }
            0x91..=0x94 => {
                // LINK #n: under this core's prefetch model regs[15] is already
                // one byte past the sequential next byte when execute_opcode runs.
                // Subtract 1 so the effective return address matches snes9x's
                // FX_LINK_I, where R11 = R15 + n with R15 still on the next byte.
                let link = self.reg(15).wrapping_add((opcode & 0x0F) as u16);
                self.write_reg_exec(11, link, opcode, pc);
                self.clear_prefix_flags();
            }
            0x95 => {
                let value = self.reg(self.src_reg) as u8 as i8 as i16 as u16;
                self.write_dest_exec(value, opcode, pc);
                self.update_sign_zero_flags(value);
                self.clear_prefix_flags();
            }
            0x96 => {
                let value = self.reg(self.src_reg);
                let result = if matches!(self.alt_mode(), 1 | 3) {
                    let signed = value as i16;
                    if signed == -1 {
                        0
                    } else {
                        (signed >> 1) as u16
                    }
                } else {
                    ((value as i16) >> 1) as u16
                };
                self.set_carry_flag((value & 0x0001) != 0);
                self.write_dest_exec(result, opcode, pc);
                self.update_sign_zero_flags(result);
                self.clear_prefix_flags();
            }
            0x97 => {
                let value = self.reg(self.src_reg);
                let carry_in = u16::from(self.condition_carry_set()) << 15;
                let result = (value >> 1) | carry_in;
                self.set_carry_flag((value & 0x0001) != 0);
                self.write_dest_exec(result, opcode, pc);
                self.update_sign_zero_flags(result);
                self.clear_prefix_flags();
            }
            0x98..=0x9D => {
                let target_reg = opcode - 0x90;
                if matches!(self.alt_mode(), 1 | 3) {
                    // LJMP Rn: bank comes from Rn, target address comes from SREG.
                    let bank = (self.reg(target_reg) & 0xFF) as u8;
                    let target = self.reg(self.src_reg);
                    self.note_pc_transfer(opcode, pc, target);
                    self.pbr = bank & 0x7F;
                    self.cbr = target & 0xFFF0;
                    self.cache_enabled = true;
                    self.cache_valid_mask = 0;
                    self.set_r15(target);
                } else {
                    let target = self.reg(target_reg);
                    self.note_pc_transfer(opcode, pc, target);
                    self.set_r15(target);
                }
                self.clear_prefix_flags();
            }
            0x9E => {
                let value = self.reg(self.src_reg) & 0x00FF;
                self.write_dest_exec(value, opcode, pc);
                self.update_sign_zero_flags(value << 8);
                self.clear_prefix_flags();
            }
            0x9F => {
                // FMULT (alt0) / LMULT (alt1): signed 16x16 → 32-bit multiply
                let lhs = self.reg(self.src_reg) as i16 as i32;
                let rhs = self.regs[6] as i16 as i32;
                let product = lhs * rhs;
                // FMULT and LMULT both use product >> 16 (per snes9x/ares)
                let high = ((product >> 16) & 0xFFFF) as u16;
                let low = (product & 0xFFFF) as u16;
                if matches!(self.alt_mode(), 1 | 3) {
                    self.write_reg_exec(4, low, opcode, pc);
                }
                self.write_dest_exec(high, opcode, pc);
                self.update_sign_zero_flags(high);
                // snes9x: GSU.vCarry = (c >> 15) & 1
                self.set_carry_flag(((product >> 15) & 1) != 0);
                self.set_overflow_flag(false);
                self.clear_prefix_flags();
            }
        }

        if let Some((bank, start, end)) = *trace_superfx_pc_range_post() {
            if self.current_exec_pbr == bank && pc >= start && pc <= end {
                eprintln!(
                    "[SFX-PC-POST] {:02X}:{:04X} op={:02X} src=r{}({:04X}) dst=r{}({:04X}) r0={:04X} r1={:04X} r2={:04X} r3={:04X} r4={:04X} r5={:04X} r6={:04X} r7={:04X} r8={:04X} r9={:04X} r10={:04X} r11={:04X} r12={:04X} r13={:04X} r14={:04X} r15={:04X} cbr={:04X} sfr={:04X}",
                    self.current_exec_pbr,
                    pc,
                    opcode,
                    self.src_reg,
                    self.reg(self.src_reg),
                    self.dst_reg,
                    self.reg(self.dst_reg),
                    self.regs[0],
                    self.regs[1],
                    self.regs[2],
                    self.regs[3],
                    self.regs[4],
                    self.regs[5],
                    self.regs[6],
                    self.regs[7],
                    self.regs[8],
                    self.regs[9],
                    self.regs[10],
                    self.regs[11],
                    self.regs[12],
                    self.regs[13],
                    self.regs[14],
                    self.regs[15],
                    self.cbr,
                    self.sfr
                );
            }
        }

        true
    }
    fn trace_abort(&self, reason: &str, pc: u16, opcode: u8) {
        if !trace_superfx_unimpl_enabled() {
            return;
        }
        use std::sync::atomic::{AtomicU32, Ordering};
        static CNT: OnceLock<AtomicU32> = OnceLock::new();
        let n = CNT
            .get_or_init(|| AtomicU32::new(0))
            .fetch_add(1, Ordering::Relaxed);
        if n < 128 {
            println!(
                "[SFX-ABORT] reason={} pc={:02X}:{:04X} opcode={:02X} s={:04X} d={:04X} src={} dst={} sfr={:04X} rombr={:02X} rambr={:02X}",
                reason,
                self.pbr,
                pc,
                opcode,
                self.reg(self.src_reg),
                self.reg(self.dst_reg),
                self.src_reg,
                self.dst_reg,
                self.sfr,
                self.rombr,
                self.rambr
            );
        }
    }

    fn trace_exec_watch(&self, exec_pbr: u8, pc: u16, opcode: u8) {
        static WATCH: OnceLock<Option<(u8, u16, u16)>> = OnceLock::new();
        static COUNT: OnceLock<std::sync::atomic::AtomicU32> = OnceLock::new();

        let watch = WATCH.get_or_init(|| {
            let raw = std::env::var("TRACE_SUPERFX_EXEC_RANGE").ok()?;
            let (bank, range) = raw.split_once(':')?;
            let bank = u8::from_str_radix(bank.trim_start_matches("0x"), 16).ok()?;
            let (start, end) = range.split_once('-')?;
            let start = u16::from_str_radix(start.trim_start_matches("0x"), 16).ok()?;
            let end = u16::from_str_radix(end.trim_start_matches("0x"), 16).ok()?;
            Some((bank & 0x7F, start.min(end), start.max(end)))
        });
        let Some((bank, start, end)) = *watch else {
            return;
        };
        if let Some(frame) = trace_superfx_exec_at_frame() {
            if TRACE_SUPERFX_EXEC_FRAME.load(Ordering::Relaxed) != frame {
                return;
            }
        }
        if exec_pbr != bank || pc < start || pc > end {
            return;
        }

        let count = COUNT
            .get_or_init(|| std::sync::atomic::AtomicU32::new(0))
            .fetch_add(1, Ordering::Relaxed);
        if count >= 256 {
            return;
        }

        println!(
            "[SFX-EXEC] {:02X}:{:04X} op={:02X} alt={} src=r{}({:04X}) dst=r{}({:04X}) r0={:04X} r1={:04X} r2={:04X} r3={:04X} r4={:04X} r5={:04X} r6={:04X} r7={:04X} r8={:04X} r9={:04X} r10={:04X} r11={:04X} r12={:04X} r13={:04X} r14={:04X} r15={:04X} sfr={:04X}",
            exec_pbr,
            pc,
            opcode,
            self.alt_mode(),
            self.src_reg,
            self.reg(self.src_reg),
            self.dst_reg,
            self.reg(self.dst_reg),
            self.regs[0],
            self.regs[1],
            self.regs[2],
            self.regs[3],
            self.regs[4],
            self.regs[5],
            self.regs[6],
            self.regs[7],
            self.regs[8],
            self.regs[9],
            self.regs[10],
            self.regs[11],
            self.regs[12],
            self.regs[13],
            self.regs[14],
            self.regs[15],
            self.sfr
        );
    }

    fn write_reg(&mut self, index: usize, value: u16) {
        self.regs[index] = value;
        if index == 15 {
            self.r15_modified = true;
        }
        if index == 14 {
            // bsnes updates the ROM buffer after the instruction completes if
            // R14 changed. Mark it dirty here and commit the buffer update
            // from execute_opcode()/read_data_rom_byte().
            self.r14_modified = true;
        }
    }

    fn write_reg_exec(&mut self, index: usize, value: u16, opcode: u8, pc: u16) {
        let value = if starfox_reg_write_debug_override_enabled() {
            let value = self.maybe_force_starfox_b30a_r14_seed(index, value, pc);
            let value = self.maybe_force_starfox_b380_r12_seed(index, value, pc);
            let value = self.maybe_keep_starfox_success_branch_target(index, value, pc);
            let value = self.maybe_keep_starfox_success_search_context(index, value, pc);
            self.maybe_null_starfox_ac98_continuation_word(index, value, pc)
        } else {
            value
        };
        let old_value = self.regs[index];
        if index == 15 && old_value != value {
            self.record_pc_transfer(opcode, pc, self.regs[15], value);
        }
        self.write_reg(index, value);
        if self.save_state_pc_hit.is_none()
            && trace_superfx_exec_frame_matches(u64::from(current_trace_superfx_frame()))
            && save_state_at_gsu_reg_write().as_ref().is_some_and(|items| {
                items
                    .iter()
                    .any(|item| item.reg as usize == index && item.value == self.regs[index])
            })
        {
            self.save_state_pc_hit = Some((self.current_exec_pbr, pc));
        }
        let trace_reg_write_prints = trace_superfx_reg_write_prints_enabled();
        if trace_reg_write_prints && index == 0 && env_presence_cached("TRACE_SUPERFX_R0_WRITES") {
            use std::sync::atomic::{AtomicU32, Ordering};
            static COUNT: OnceLock<AtomicU32> = OnceLock::new();
            let interesting = self.regs[index] >= 0x0100 || old_value != self.regs[index];
            if interesting {
                let n = COUNT
                    .get_or_init(|| AtomicU32::new(0))
                    .fetch_add(1, Ordering::Relaxed);
                if n < 1024 {
                    println!(
                        "[SFX-R0] pbr={:02X} pc={:04X} op={:02X} alt={} old={:04X} new={:04X} src=r{}({:04X}) dst=r{}({:04X}) sfr={:04X}",
                        self.pbr,
                        pc,
                        opcode,
                        self.alt_mode(),
                        old_value,
                        self.regs[index],
                        self.src_reg,
                        self.reg(self.src_reg),
                        self.dst_reg,
                        self.reg(self.dst_reg),
                        self.sfr
                    );
                }
            }
        }
        if trace_reg_write_prints && index == 4 && env_presence_cached("TRACE_SUPERFX_R4_WRITES") {
            use std::sync::atomic::{AtomicU32, Ordering};
            static COUNT: OnceLock<AtomicU32> = OnceLock::new();
            let interesting = self.regs[index] >= 0x0100 || old_value != self.regs[index];
            if interesting {
                let n = COUNT
                    .get_or_init(|| AtomicU32::new(0))
                    .fetch_add(1, Ordering::Relaxed);
                if n < 1024 {
                    println!(
                        "[SFX-R4] pbr={:02X} pc={:04X} op={:02X} alt={} old={:04X} new={:04X} src=r{}({:04X}) dst=r{}({:04X}) sfr={:04X}",
                        self.pbr,
                        pc,
                        opcode,
                        self.alt_mode(),
                        old_value,
                        self.regs[index],
                        self.src_reg,
                        self.reg(self.src_reg),
                        self.dst_reg,
                        self.reg(self.dst_reg),
                        self.sfr
                    );
                }
            }
        }
        if trace_reg_write_prints && index == 7 && env_presence_cached("TRACE_SUPERFX_R7_WRITES") {
            use std::sync::atomic::{AtomicU32, Ordering};
            static COUNT: OnceLock<AtomicU32> = OnceLock::new();
            let interesting = self.regs[index] >= 0x0100 || old_value != self.regs[index];
            if interesting {
                let n = COUNT
                    .get_or_init(|| AtomicU32::new(0))
                    .fetch_add(1, Ordering::Relaxed);
                if n < 1024 {
                    println!(
                        "[SFX-R7] pbr={:02X} pc={:04X} op={:02X} alt={} old={:04X} new={:04X} src=r{}({:04X}) dst=r{}({:04X}) sfr={:04X}",
                        self.pbr,
                        pc,
                        opcode,
                        self.alt_mode(),
                        old_value,
                        self.regs[index],
                        self.src_reg,
                        self.reg(self.src_reg),
                        self.dst_reg,
                        self.reg(self.dst_reg),
                        self.sfr
                    );
                }
            }
        }
        if trace_reg_write_prints && index == 9 && env_presence_cached("TRACE_SUPERFX_R9_WRITES") {
            use std::sync::atomic::{AtomicU32, Ordering};
            static COUNT: OnceLock<AtomicU32> = OnceLock::new();
            let interesting = self.regs[index] >= 0x0100 || old_value != self.regs[index];
            if interesting {
                let n = COUNT
                    .get_or_init(|| AtomicU32::new(0))
                    .fetch_add(1, Ordering::Relaxed);
                if n < 1024 {
                    println!(
                        "[SFX-R9] pbr={:02X} pc={:04X} op={:02X} alt={} old={:04X} new={:04X} src=r{}({:04X}) dst=r{}({:04X}) sfr={:04X}",
                        self.pbr,
                        pc,
                        opcode,
                        self.alt_mode(),
                        old_value,
                        self.regs[index],
                        self.src_reg,
                        self.reg(self.src_reg),
                        self.dst_reg,
                        self.reg(self.dst_reg),
                        self.sfr
                    );
                }
            }
        }
        if trace_reg_write_prints && index == 10 && env_presence_cached("TRACE_SUPERFX_R10_WRITES")
        {
            use std::sync::atomic::{AtomicU32, Ordering};
            static COUNT: OnceLock<AtomicU32> = OnceLock::new();
            let interesting = self.regs[index] >= 0x0100 || old_value != self.regs[index];
            if interesting {
                let n = COUNT
                    .get_or_init(|| AtomicU32::new(0))
                    .fetch_add(1, Ordering::Relaxed);
                if n < 1024 {
                    println!(
                        "[SFX-R10] pbr={:02X} pc={:04X} op={:02X} alt={} old={:04X} new={:04X} src=r{}({:04X}) dst=r{}({:04X}) sfr={:04X}",
                        self.pbr,
                        pc,
                        opcode,
                        self.alt_mode(),
                        old_value,
                        self.regs[index],
                        self.src_reg,
                        self.reg(self.src_reg),
                        self.dst_reg,
                        self.reg(self.dst_reg),
                        self.sfr
                    );
                }
            }
        }
        if trace_reg_write_prints && index == 12 && env_presence_cached("TRACE_SUPERFX_R12_WRITES")
        {
            use std::sync::atomic::{AtomicU32, Ordering};
            static COUNT: OnceLock<AtomicU32> = OnceLock::new();
            let interesting =
                self.regs[index] >= 0x0100 || !matches!(pc, 0xB380 | 0xB391 | 0xB3B7 | 0xB4C0);
            if interesting {
                let n = COUNT
                    .get_or_init(|| AtomicU32::new(0))
                    .fetch_add(1, Ordering::Relaxed);
                if n < 512 {
                    println!(
                    "[SFX-R12] pbr={:02X} pc={:04X} op={:02X} alt={} old={:04X} new={:04X} src=r{}({:04X}) dst=r{}({:04X}) sfr={:04X}",
                    self.pbr,
                    pc,
                    opcode,
                    self.alt_mode(),
                    old_value,
                    self.regs[index],
                    self.src_reg,
                    self.reg(self.src_reg),
                    self.dst_reg,
                    self.reg(self.dst_reg),
                    self.sfr
                );
                }
            }
        }
        self.record_reg_write(opcode, pc, index as u8, old_value, self.regs[index]);
    }

    fn schedule_rom_buffer_reload(&mut self) {
        self.rom_buffer_valid = false;
        self.rom_buffer_pending = true;
        self.rom_buffer_pending_bank = self.rombr & 0x7F;
        self.rom_buffer_pending_addr = self.regs[14];
    }

    fn refresh_rom_buffer_if_needed(&mut self, rom: &[u8]) -> Option<()> {
        if self.r14_modified {
            self.schedule_rom_buffer_reload();
            self.fill_rom_buffer(rom)?;
            self.r14_modified = false;
            return Some(());
        }
        if self.rom_buffer_pending || !self.rom_buffer_valid {
            self.fill_rom_buffer(rom)?;
        }
        Some(())
    }

    fn fill_rom_buffer(&mut self, rom: &[u8]) -> Option<u8> {
        if !self.rom_buffer_pending {
            self.schedule_rom_buffer_reload();
        }
        let value = self.read_data_source_byte(
            rom,
            self.rom_buffer_pending_bank,
            self.rom_buffer_pending_addr,
        )?;
        self.rom_buffer = value;
        self.rom_buffer_valid = true;
        self.rom_buffer_pending = false;
        Some(value)
    }

    fn fetch_opcode(&mut self, rom: &[u8]) -> Option<u8> {
        if self.running && self.pipe_valid {
            return self.consume_pipe_byte(rom);
        }
        self.pipe_pc = self.regs[15];
        self.pipe_pbr = self.pbr;
        let value = self.read_program_rom_byte(rom, self.pbr, self.regs[15])?;
        self.advance_r15_after_fetch();
        Some(value)
    }

    fn prime_pipe(&mut self, rom: &[u8]) -> Option<()> {
        self.pipe_pc = self.regs[15];
        self.pipe_pbr = self.pbr;
        self.pipe = self.read_program_rom_byte(rom, self.pbr, self.regs[15])?;
        self.pipe_valid = true;
        self.advance_r15_after_fetch();
        Some(())
    }

    fn prefetch_pipe(&mut self, rom: &[u8]) -> Option<()> {
        self.pipe_pc = self.regs[15];
        self.pipe_pbr = self.pbr;
        self.pipe = self.read_program_rom_byte(rom, self.pbr, self.regs[15])?;
        self.pipe_valid = true;
        self.r15_modified = false;
        Some(())
    }

    fn consume_pipe_byte(&mut self, rom: &[u8]) -> Option<u8> {
        let value = self.pipe;
        self.regs[15] = self.regs[15].wrapping_add(1);
        self.pipe_pc = self.regs[15];
        self.pipe_pbr = self.pbr;
        self.pipe = self.read_program_rom_byte(rom, self.pbr, self.regs[15])?;
        self.pipe_valid = true;
        self.r15_modified = false;
        Some(value)
    }

    fn read_bus_mapped_byte(&self, rom: &[u8], bank: u8, addr: u16) -> Option<u8> {
        let bank = bank & 0x7F;
        let full_addr = ((bank as usize) << 16) | addr as usize;

        if (full_addr & 0xE0_0000) == 0x60_0000 {
            return self.game_ram.get(full_addr % self.game_ram.len()).copied();
        }
        if rom.is_empty() {
            return None;
        }

        // Match bsnes SuperFX bus mapping:
        // - $00-$3F:0000-FFFF => 32KB mirrored LoROM pages
        // - $40-$5F:0000-FFFF => linear 64KB ROM windows
        let offset = if (full_addr & 0xE0_0000) == 0x40_0000 {
            full_addr
        } else {
            ((full_addr & 0x3F_0000) >> 1) | (full_addr & 0x7FFF)
        };
        rom.get(offset % rom.len()).copied()
    }

    fn read_program_source_byte(&self, rom: &[u8], bank: u8, addr: u16) -> Option<u8> {
        self.read_bus_mapped_byte(rom, bank, addr)
    }

    fn read_data_source_byte(&self, rom: &[u8], bank: u8, addr: u16) -> Option<u8> {
        self.read_bus_mapped_byte(rom, bank, addr)
    }

    fn cache_offset_for_addr(&self, addr: u16) -> Option<usize> {
        let offset = addr.wrapping_sub(self.cbr) as usize;
        (offset < CACHE_RAM_SIZE).then_some(offset)
    }

    fn fill_cache_line(&mut self, rom: &[u8], bank: u8, addr: u16) {
        let Some(offset) = self.cache_offset_for_addr(addr) else {
            return;
        };
        let line_start_offset = offset & !0x0F;
        let line_index = line_start_offset >> 4;
        let line_start_addr = self.cbr.wrapping_add(line_start_offset as u16);
        for i in 0..16 {
            let cache_idx = line_start_offset + i;
            self.cache_ram[cache_idx] = self
                .read_program_source_byte(rom, bank, line_start_addr.wrapping_add(i as u16))
                .unwrap_or(0);
        }
        self.cache_valid_mask |= 1u32 << line_index;
    }

    fn read_program_rom_byte(&mut self, rom: &[u8], bank: u8, addr: u16) -> Option<u8> {
        if (bank & 0x60) == 0x60 {
            self.sync_ram_buffer();
        }
        if let Some(offset) = self.cache_offset_for_addr(addr) {
            let line_index = offset >> 4;
            if (self.cache_valid_mask & (1u32 << line_index)) == 0 {
                self.fill_cache_line(rom, bank, addr);
            }
            return Some(self.cache_ram[offset]);
        }
        self.read_program_source_byte(rom, bank, addr)
    }

    fn read_data_rom_byte(&mut self, rom: &[u8]) -> Option<u8> {
        // GETB/GETC read from the ROM buffer without modifying R14.
        // Match bsnes more closely: R14 writes only mark the buffer dirty
        // during instruction execution, and the buffer is refreshed once the
        // instruction completes or on demand before the next GETB/GETC read.
        self.refresh_rom_buffer_if_needed(rom)?;
        Some(self.rom_buffer)
    }

    fn ram_addr_with_bank(&self, bank: u8, addr: u16) -> Option<usize> {
        if self.game_ram.is_empty() {
            None
        } else {
            let bank_base = ((bank & 0x03) as usize) << 16;
            Some((bank_base + addr as usize) % self.game_ram.len())
        }
    }

    fn ram_addr(&self, addr: u16) -> Option<usize> {
        self.ram_addr_with_bank(self.rambr, addr)
    }

    fn peek_ram_byte(&self, addr: u16) -> u8 {
        self.ram_addr(addr)
            .map(|idx| self.game_ram[idx])
            .unwrap_or(0xFF)
    }

    fn sync_ram_buffer(&mut self) {
        if !self.ram_buffer_pending {
            return;
        }
        let bank = self.ram_buffer_pending_bank;
        let addr = self.ram_buffer_pending_addr;
        let data = self.ram_buffer_pending_data;
        self.ram_buffer_pending = false;
        self.write_ram_byte_immediate_with_bank(bank, addr, data);
    }

    fn ram_word_after_byte_write(&self, word_addr: u16, touched_addr: u16, value: u8) -> u16 {
        let lo_addr = word_addr;
        let hi_addr = word_addr ^ 1;
        let lo = if touched_addr == lo_addr {
            value
        } else {
            self.peek_ram_byte(lo_addr)
        };
        let hi = if touched_addr == hi_addr {
            value
        } else {
            self.peek_ram_byte(hi_addr)
        };
        u16::from_le_bytes([lo, hi])
    }

    fn read_ram_byte_raw(&mut self, addr: u16) -> u8 {
        self.last_ram_addr = addr;
        let value = self.peek_ram_byte(addr);
        if trace_superfx_exec_frame_matches(u64::from(current_trace_superfx_frame()))
            && trace_superfx_ram_addr_matches(addr)
        {
            eprintln!(
                "[SFX-RAM-R] f={} pc={:02X}:{:04X} op={:02X} r15={:04X} rambr={:02X} addr={:04X} -> {:02X} src=r{} dst=r{} r12={:04X} r13={:04X} r14={:04X}",
                current_trace_superfx_frame(),
                self.current_exec_pbr,
                self.current_exec_pc,
                self.current_exec_opcode,
                self.regs[15],
                self.rambr,
                addr,
                value,
                self.src_reg,
                self.dst_reg,
                self.regs[12],
                self.regs[13],
                self.regs[14],
            );
        }
        value
    }

    fn read_ram_byte(&mut self, addr: u16) -> u8 {
        self.sync_ram_buffer();
        self.read_ram_byte_raw(addr)
    }

    fn read_ram_word(&mut self, addr: u16) -> u16 {
        self.last_ram_addr = addr;
        let lo = self.read_ram_byte(addr);
        let hi = self.read_ram_byte(addr ^ 1);
        self.last_ram_addr = addr;
        u16::from_le_bytes([lo, hi])
    }

    fn read_ram_word_short(&mut self, addr: u16) -> u16 {
        self.last_ram_addr = addr;
        let lo = self.read_ram_byte(addr);
        let hi = self.read_ram_byte(addr.wrapping_add(1));
        self.last_ram_addr = addr;
        u16::from_le_bytes([lo, hi])
    }

    fn write_ram_byte_immediate_with_bank(&mut self, bank: u8, addr: u16, value: u8) {
        let value = if starfox_ram_write_debug_override_enabled() {
            self.maybe_force_starfox_continuation_ptr_byte(addr, value)
        } else {
            value
        };
        self.last_ram_addr = addr;
        self.trace_screen_byte_write(addr, value);
        let frame_matches =
            trace_superfx_exec_frame_matches(u64::from(current_trace_superfx_frame()));
        if frame_matches && trace_superfx_ram_addr_matches(addr) {
            eprintln!(
                "[SFX-RAM-W-ADDR] f={} pc={:02X}:{:04X} op={:02X} r15={:04X} rambr={:02X} addr={:04X} <- {:02X} src=r{}({:04X}) dst=r{}({:04X}) r12={:04X} r13={:04X} r14={:04X}",
                current_trace_superfx_frame(),
                self.current_exec_pbr,
                self.current_exec_pc,
                self.current_exec_opcode,
                self.regs[15],
                self.rambr,
                addr,
                value,
                self.src_reg,
                self.reg(self.src_reg),
                self.dst_reg,
                self.reg(self.dst_reg),
                self.regs[12],
                self.regs[13],
                self.regs[14],
            );
        }
        let save_word_eq_matches =
            save_state_at_superfx_ram_word_eq()
                .as_ref()
                .is_none_or(|items| {
                    items.iter().any(|item| {
                        let watched_addr = item.addr;
                        let touched = addr == watched_addr || addr == (watched_addr ^ 1);
                        touched
                            && self.ram_word_after_byte_write(watched_addr, addr, value)
                                == item.value
                    })
                });
        if frame_matches
            && save_state_at_superfx_ram_addr_matches(addr)
            && save_state_at_superfx_ram_byte_eq_matches(addr, value)
            && save_word_eq_matches
        {
            self.save_state_ram_addr_hit_count =
                self.save_state_ram_addr_hit_count.saturating_add(1);
            if self.save_state_ram_addr_hit.is_none()
                && self.save_state_ram_addr_hit_count >= save_state_at_superfx_ram_addr_hit_index()
            {
                self.save_state_ram_addr_hit =
                    Some((self.current_exec_pbr, self.current_exec_pc, addr));
            }
        }
        if env_presence_cached("TRACE_SFX_RAM_WRITES") {
            use std::sync::atomic::{AtomicU32, Ordering};
            static TOTAL: AtomicU32 = AtomicU32::new(0);
            static NZ: AtomicU32 = AtomicU32::new(0);
            static DETAIL: AtomicU32 = AtomicU32::new(0);
            let t = TOTAL.fetch_add(1, Ordering::Relaxed);
            if value != 0 {
                let n = NZ.fetch_add(1, Ordering::Relaxed);
                if n < 32 {
                    let d = DETAIL.fetch_add(1, Ordering::Relaxed);
                    if d < 32 {
                        eprintln!(
                            "[SFX-RAM-W] pbr={:02X} r15={:04X} rambr={:02X} addr={:04X} <- {:02X} (nz#{} total={})",
                            self.pbr, self.regs[15], self.rambr, addr, value, n, t
                        );
                    }
                }
            }
            if t > 0 && t.is_multiple_of(1_000_000) {
                let nz_count = NZ.load(Ordering::Relaxed);
                eprintln!(
                    "[SFX-RAM-W-SUMMARY] total_writes={} non_zero_writes={}",
                    t, nz_count
                );
            }
        }
        let old_value = self
            .ram_addr_with_bank(bank, addr)
            .map(|idx| self.game_ram[idx])
            .unwrap_or(0xFF);
        self.record_low_ram_write(addr, old_value, value);
        if let Some(idx) = self.ram_addr_with_bank(bank, addr) {
            self.game_ram[idx] = value;
        }
    }

    fn write_ram_byte(&mut self, addr: u16, value: u8) {
        self.write_ram_byte_immediate_with_bank(self.rambr, addr, value);
    }

    fn write_ram_buffer_byte(&mut self, addr: u16, value: u8) {
        let value = if starfox_ram_write_debug_override_enabled() {
            self.maybe_force_starfox_continuation_ptr_byte(addr, value)
        } else {
            value
        };
        self.sync_ram_buffer();
        self.last_ram_addr = addr;
        self.ram_buffer_pending = true;
        self.ram_buffer_pending_bank = self.rambr & 0x03;
        self.ram_buffer_pending_addr = addr;
        self.ram_buffer_pending_data = value;
    }

    fn write_ram_word(&mut self, addr: u16, value: u16) {
        let value = if starfox_ram_write_debug_override_enabled() {
            let value = self.maybe_force_starfox_parser_key_from_match_word(addr, value);
            let value = self.maybe_keep_starfox_success_cursor_armed(addr, value);
            self.maybe_force_starfox_continuation_cursor_word(addr, value)
        } else {
            value
        };
        self.last_ram_addr = addr;
        self.trace_screen_word_write(addr, value);
        self.write_ram_byte(addr, value as u8);
        self.write_ram_byte(addr ^ 1, (value >> 8) as u8);
        self.last_ram_addr = addr;
    }

    fn write_ram_buffer_word(&mut self, addr: u16, value: u16) {
        let value = if starfox_ram_write_debug_override_enabled() {
            let value = self.maybe_force_starfox_parser_key_from_match_word(addr, value);
            let value = self.maybe_keep_starfox_success_cursor_armed(addr, value);
            self.maybe_force_starfox_continuation_cursor_word(addr, value)
        } else {
            value
        };
        self.last_ram_addr = addr;
        self.trace_screen_word_write(addr, value);
        self.write_ram_buffer_byte(addr, value as u8);
        self.write_ram_buffer_byte(addr ^ 1, (value >> 8) as u8);
        self.last_ram_addr = addr;
    }

    #[cfg(test)]
    fn write_ram_word_short(&mut self, addr: u16, value: u16) {
        let value = if starfox_ram_write_debug_override_enabled() {
            let value = self.maybe_force_starfox_parser_key_from_match_word(addr, value);
            let value = self.maybe_keep_starfox_success_cursor_armed(addr, value);
            self.maybe_force_starfox_continuation_cursor_word(addr, value)
        } else {
            value
        };
        self.last_ram_addr = addr;
        self.trace_screen_word_write(addr, value);
        self.write_ram_byte(addr, value as u8);
        self.write_ram_byte(addr.wrapping_add(1), (value >> 8) as u8);
        self.last_ram_addr = addr;
    }

    fn write_ram_buffer_word_short(&mut self, addr: u16, value: u16) {
        let value = if starfox_ram_write_debug_override_enabled() {
            let value = self.maybe_force_starfox_parser_key_from_match_word(addr, value);
            let value = self.maybe_keep_starfox_success_cursor_armed(addr, value);
            self.maybe_force_starfox_continuation_cursor_word(addr, value)
        } else {
            value
        };
        self.last_ram_addr = addr;
        self.trace_screen_word_write(addr, value);
        self.write_ram_buffer_byte(addr, value as u8);
        self.write_ram_buffer_byte(addr.wrapping_add(1), (value >> 8) as u8);
        self.last_ram_addr = addr;
    }

    #[cfg(test)]
    fn screen_height(&self) -> Option<usize> {
        self.screen_height_for_mode(self.screen_height_mode())
    }

    fn effective_screen_height(&self) -> Option<usize> {
        self.screen_height_for_mode(self.effective_screen_layout_mode())
    }

    fn screen_height_for_mode(&self, mode: u8) -> Option<usize> {
        match mode {
            0 => Some(128),
            1 => Some(160),
            2 => Some(192),
            3 => Some(256),
            _ => unreachable!(),
        }
    }

    fn screen_height_mode(&self) -> u8 {
        (((self.scmr >> 5) & 0x01) << 1) | ((self.scmr >> 2) & 0x01)
    }

    fn effective_screen_layout_mode(&self) -> u8 {
        if (self.por & 0x10) != 0 {
            3
        } else {
            self.screen_height_mode()
        }
    }

    fn bits_per_pixel(&self) -> Option<usize> {
        match self.scmr & 0x03 {
            0 => Some(2),
            1 => Some(4),
            2 => Some(4),
            3 => Some(8),
            _ => None,
        }
    }

    fn screen_base_addr(&self) -> usize {
        (self.scbr as usize) << 10
    }

    fn screen_buffer_len(&self) -> Option<usize> {
        let height = self.effective_screen_height()?;
        let bpp = self.bits_per_pixel()?;
        let bytes_per_tile = match bpp {
            2 => 16,
            4 => 32,
            8 => 64,
            _ => return None,
        };
        Some(32 * (height / 8) * bytes_per_tile)
    }

    fn trace_screen_word_write(&self, addr: u16, value: u16) {
        if !trace_superfx_screen_words_enabled() {
            return;
        }
        if !trace_superfx_matches_current_frame("TRACE_SUPERFX_SCREEN_WORDS_AT_FRAME") {
            return;
        }
        let Some(idx) = self.ram_addr(addr) else {
            return;
        };
        let Some(len) = self.screen_buffer_len() else {
            return;
        };
        let start = self.screen_base_addr();
        let end = start.saturating_add(len);
        if idx < start || idx >= end {
            return;
        }
        if !trace_superfx_screen_idx_matches(idx) {
            return;
        }
        use std::sync::atomic::{AtomicU32, Ordering};
        static COUNT: OnceLock<AtomicU32> = OnceLock::new();
        let n = COUNT
            .get_or_init(|| AtomicU32::new(0))
            .fetch_add(1, Ordering::Relaxed);
        let capped =
            trace_superfx_screen_idx_min().is_none() && trace_superfx_screen_idx_max().is_none();
        if capped && n >= 128 {
            return;
        }
        println!(
            "[SFX-SCREEN-W] pc={:02X}:{:04X} op={:02X} pbr={:02X} r15={:04X} rambr={:02X} scbr={:02X} scmr={:02X} addr_reg=r{}({:04X}) addr={:04X} idx={:05X} off={:05X} odd={} value={:04X} src=r{}({:04X}) dst=r{}({:04X})",
            self.current_exec_pbr,
            self.current_exec_pc,
            self.current_exec_opcode,
            self.current_exec_pbr,
            self.regs[15],
            self.rambr,
            self.scbr,
            self.scmr,
            self.current_exec_opcode & 0x0F,
            self.reg(self.current_exec_opcode & 0x0F),
            addr,
            idx,
            idx - start,
            (addr & 1) != 0,
            value,
            self.src_reg,
            self.reg(self.src_reg),
            self.dst_reg,
            self.reg(self.dst_reg),
        );
    }

    fn trace_screen_byte_write(&self, addr: u16, value: u8) {
        if !trace_superfx_screen_bytes_enabled() {
            return;
        }
        let Some(idx) = self.ram_addr(addr) else {
            return;
        };
        let Some(len) = self.screen_buffer_len() else {
            return;
        };
        let start = self.screen_base_addr();
        let end = start.saturating_add(len);
        if idx < start || idx >= end {
            return;
        }
        if !trace_superfx_screen_idx_matches(idx) {
            return;
        }
        use std::sync::atomic::{AtomicU32, Ordering};
        static COUNT: OnceLock<AtomicU32> = OnceLock::new();
        let n = COUNT
            .get_or_init(|| AtomicU32::new(0))
            .fetch_add(1, Ordering::Relaxed);
        let capped =
            trace_superfx_screen_idx_min().is_none() && trace_superfx_screen_idx_max().is_none();
        if capped && n >= 256 {
            return;
        }
        println!(
            "[SFX-SCREEN-B] pc={:02X}:{:04X} op={:02X} pbr={:02X} r15={:04X} rambr={:02X} scbr={:02X} scmr={:02X} addr_reg=r{}({:04X}) addr={:04X} idx={:05X} off={:05X} value={:02X} src=r{}({:04X}) dst=r{}({:04X}) r10={:04X} r11={:04X}",
            self.current_exec_pbr,
            self.current_exec_pc,
            self.current_exec_opcode,
            self.current_exec_pbr,
            self.regs[15],
            self.rambr,
            self.scbr,
            self.scmr,
            self.current_exec_opcode & 0x0F,
            self.reg(self.current_exec_opcode & 0x0F),
            addr,
            idx,
            idx - start,
            value,
            self.src_reg,
            self.reg(self.src_reg),
            self.dst_reg,
            self.reg(self.dst_reg),
            self.regs[10],
            self.regs[11],
        );
    }

    fn tile_pixel_addr(&self, x: u16, y: u16) -> Option<(usize, usize, usize)> {
        let height = self.effective_screen_height()?;
        let bpp = self.bits_per_pixel()?;
        let x = (x as u8) as usize;
        let y = (y as u8) as usize;
        if y >= height {
            return None;
        }
        let row_in_tile = y & 7;
        let bit = 7 - (x & 7);
        let cn = match height {
            128 => ((x & 0xF8) << 1) + ((y & 0xF8) >> 3),
            160 => ((x & 0xF8) << 1) + ((x & 0xF8) >> 1) + ((y & 0xF8) >> 3),
            192 => ((x & 0xF8) << 1) + (x & 0xF8) + ((y & 0xF8) >> 3),
            256 => ((y & 0x80) << 2) + ((x & 0x80) << 1) + ((y & 0x78) << 1) + ((x & 0x78) >> 3),
            _ => return None,
        };
        let bytes_per_tile = match bpp {
            2 => 16,
            4 => 32,
            8 => 64,
            _ => return None,
        };
        Some((
            self.screen_base_addr() + cn * bytes_per_tile,
            row_in_tile,
            bit,
        ))
    }

    fn flush_pixel_cache(&mut self, cache_index: usize) {
        let cache = self.pixelcache[cache_index];
        if cache.bitpend == 0 {
            return;
        }
        self.in_cache_flush = true;
        if env_presence_cached("TRACE_CACHE_FLUSH") {
            use std::sync::atomic::{AtomicU64, Ordering};
            static CNT: AtomicU64 = AtomicU64::new(0);
            let n = CNT.fetch_add(1, Ordering::Relaxed);
            if n < 20 {
                let nz_data = cache.data.iter().filter(|&&d| d != 0).count();
                eprintln!(
                    "[FLUSH] #{} offset={} bitpend={:02X} nz_data={} data={:?}",
                    n, cache.offset, cache.bitpend, nz_data, cache.data
                );
            }
        }

        let x = (cache.offset << 3) as u16;
        let y = (cache.offset >> 5) as u16;

        let bpp = match self.bits_per_pixel() {
            Some(v) => v,
            None => {
                self.pixelcache[cache_index].bitpend = 0;
                return;
            }
        };

        let Some((tile_base, row, _)) = self.tile_pixel_addr(x, y) else {
            self.pixelcache[cache_index].bitpend = 0;
            return;
        };
        let addr_base = tile_base + row * 2;

        for n in 0..bpp {
            let byte_offset = ((n >> 1) << 4) + (n & 1);
            let addr = (addr_base + byte_offset) as u16;

            // Build the data byte from pixel cache
            let mut data: u8 = 0;
            for p in 0..8u8 {
                if cache.data[p as usize] & (1 << n) != 0 {
                    data |= 1 << p;
                }
            }

            // If not all 8 pixels are pending, merge with existing RAM data
            if cache.bitpend != 0xFF {
                let existing = self
                    .ram_addr(addr)
                    .map(|idx| self.game_ram[idx])
                    .unwrap_or(0);
                data = (existing & !cache.bitpend) | (data & cache.bitpend);
            }

            self.write_ram_byte(addr, data);
        }

        self.in_cache_flush = false;
        self.pixelcache[cache_index].bitpend = 0;
    }

    fn flush_all_pixel_caches(&mut self) {
        self.flush_pixel_cache(1);
        self.flush_pixel_cache(0);
    }

    fn plot_pixel(&mut self, x: u16, y: u16, color: u8) {
        let x = x as u8;
        let y = y as u8;
        // bsnes: transparency is checked before dithering and differs for 8bpp.
        if (self.por & 0x01) == 0 {
            let transparent = match self.bits_per_pixel() {
                Some(8) if (self.por & 0x08) == 0 => color == 0,
                _ => (color & 0x0F) == 0,
            };
            if transparent {
                return;
            }
        }
        // Dithering
        let color = if (self.por & 0x02) != 0 && self.bits_per_pixel() != Some(8) {
            if (x ^ y) & 1 != 0 {
                (color >> 4) & 0x0F
            } else {
                color & 0x0F
            }
        } else {
            color
        };
        let height = match self.effective_screen_height() {
            Some(value) => value as u16,
            None => return,
        };
        if u16::from(y) >= height {
            return;
        }
        let offset = ((u16::from(y) << 5) | (u16::from(x) >> 3)) as u16;
        if offset != self.pixelcache[0].offset {
            self.flush_pixel_cache(1);
            self.pixelcache[1] = self.pixelcache[0];
            self.pixelcache[0].bitpend = 0;
            self.pixelcache[0].offset = offset;
            self.pixelcache[0].data = [0; 8];
        }
        let cache_x = ((x & 7) ^ 7) as usize;
        self.pixelcache[0].data[cache_x] = color;
        self.pixelcache[0].bitpend |= 1 << cache_x;
        let tile = self.tile_pixel_addr(u16::from(x), u16::from(y));
        self.trace_plot(
            "plot",
            u16::from(x),
            u16::from(y),
            color,
            tile.map(|(base, _, _)| base),
            tile.map(|(_, row, _)| row),
            tile.map(|(_, _, bit)| bit),
        );
        if self.pixelcache[0].bitpend == 0xFF {
            self.flush_pixel_cache(1);
            self.pixelcache[1] = self.pixelcache[0];
            self.pixelcache[0].bitpend = 0;
            self.pixelcache[0].data = [0; 8];
        }
    }

    fn read_plot_pixel(&mut self, x: u16, y: u16) -> u8 {
        self.flush_all_pixel_caches();
        let Some((tile_base, row, bit)) = self.tile_pixel_addr(x, y) else {
            return 0;
        };
        let bpp = match self.bits_per_pixel() {
            Some(value) => value,
            None => return 0,
        };
        let plane_pairs = bpp / 2;
        let mut color = 0u8;
        for pair in 0..plane_pairs {
            let pair_base = tile_base + pair * 16 + row * 2;
            let low = self
                .ram_addr(pair_base as u16)
                .map(|idx| self.game_ram[idx])
                .unwrap_or(0);
            let high = self
                .ram_addr((pair_base + 1) as u16)
                .map(|idx| self.game_ram[idx])
                .unwrap_or(0);
            color |= ((low >> bit) & 0x01) << (pair * 2);
            color |= ((high >> bit) & 0x01) << (pair * 2 + 1);
        }
        self.trace_plot("rpix", x, y, color, Some(tile_base), Some(row), Some(bit));
        color
    }

    #[allow(dead_code, clippy::too_many_arguments)]
    fn trace_plot(
        &self,
        kind: &str,
        x: u16,
        y: u16,
        color: u8,
        tile_base: Option<usize>,
        row: Option<usize>,
        bit: Option<usize>,
    ) {
        if !trace_superfx_plot_enabled() {
            return;
        }
        if !trace_superfx_matches_current_frame("TRACE_SUPERFX_PLOT_AT_FRAME") {
            return;
        }
        use std::sync::atomic::{AtomicU32, Ordering};
        static COUNT: AtomicU32 = AtomicU32::new(0);
        let n = COUNT.fetch_add(1, Ordering::Relaxed);
        if n >= 128 {
            return;
        }
        eprintln!(
            "[SFX-PLOT] kind={} pbr={:02X} r15={:04X} rambr={:02X} scbr={:02X} scmr={:02X} por={:02X} colr={:02X} xy=({}, {}) color={:02X} tile_base={:?} row={:?} bit={:?}",
            kind,
            self.pbr,
            self.regs[15],
            self.rambr,
            self.scbr,
            self.scmr,
            self.por,
            self.colr,
            x,
            y,
            color,
            tile_base,
            row,
            bit
        );
    }

    fn record_profile(&mut self, opcode: u8) {
        if !trace_superfx_profile_enabled() {
            return;
        }
        let opcode = opcode as usize;
        let alt = self.alt_mode() as usize;
        self.exec_profile[opcode] = self.exec_profile[opcode].saturating_add(1);
        self.exec_profile_by_alt[alt][opcode] =
            self.exec_profile_by_alt[alt][opcode].saturating_add(1);
    }

    fn record_pc_transfer(&mut self, opcode: u8, from_pc: u16, next_pc: u16, to_pc: u16) {
        if !trace_superfx_last_transfers_enabled() {
            return;
        }
        if let Some(last) = self.recent_pc_transfers.last_mut() {
            if last.opcode == opcode
                && last.pbr == self.pbr
                && last.from_pc == from_pc
                && last.next_pc == next_pc
                && last.to_pc == to_pc
                && last.rombr == self.rombr
                && last.src_reg == self.src_reg
                && last.dst_reg == self.dst_reg
                && last.r13 == self.regs[13]
                && last.sfr == self.sfr
            {
                last.repeats = last.repeats.saturating_add(1);
                return;
            }
        }
        if self.recent_pc_transfers.len() >= 64 {
            self.recent_pc_transfers.remove(0);
        }
        self.recent_pc_transfers.push(SuperFxPcTransfer {
            opcode,
            pbr: self.pbr,
            from_pc,
            next_pc,
            to_pc,
            rombr: self.rombr,
            src_reg: self.src_reg,
            dst_reg: self.dst_reg,
            r12: self.regs[12],
            r13: self.regs[13],
            sfr: self.sfr,
            repeats: 1,
        });
    }

    fn record_reg_write(&mut self, opcode: u8, pc: u16, reg: u8, old_value: u16, new_value: u16) {
        if !trace_superfx_reg_flow_enabled() || old_value == new_value {
            return;
        }
        if let Some(frame) = trace_superfx_exec_at_frame() {
            if current_trace_superfx_frame() != frame {
                return;
            }
        }
        let reg = reg & 0x0F;
        if let Some(filter) = trace_superfx_reg_flow_filter().as_ref() {
            if !filter[reg as usize] {
                return;
            }
        }
        if let Some((bank, start, end)) = *trace_superfx_reg_flow_exclude_range() {
            if self.pbr == bank && pc >= start && pc <= end {
                return;
            }
        }
        if let Some(last) = self.recent_reg_writes.last_mut() {
            if last.opcode == opcode
                && last.pbr == self.pbr
                && last.pc == pc
                && last.reg == reg
                && last.src_reg == self.src_reg
                && last.dst_reg == self.dst_reg
            {
                last.new_value = new_value;
                last.sfr = self.sfr;
                last.repeats = last.repeats.saturating_add(1);
                let tracked = last.clone();
                self.last_reg_writes[reg as usize] = Some(tracked.clone());
                Self::push_reg_write_history(
                    &mut self.recent_reg_writes_by_reg[reg as usize],
                    tracked.clone(),
                );
                if !Self::is_trivial_reg_write_for_diagnostic(reg, opcode) {
                    self.last_nontrivial_reg_writes[reg as usize] = Some(tracked.clone());
                    Self::push_nontrivial_reg_write_history(
                        &mut self.recent_nontrivial_reg_writes[reg as usize],
                        tracked,
                    );
                }
                return;
            }
        }
        if self.recent_reg_writes.len() >= MAX_RECENT_REG_WRITES {
            self.recent_reg_writes.remove(0);
        }
        let write = SuperFxRegWrite {
            opcode,
            pbr: self.pbr,
            pc,
            reg,
            old_value,
            new_value,
            src_reg: self.src_reg,
            dst_reg: self.dst_reg,
            sfr: self.sfr,
            repeats: 1,
        };
        self.last_reg_writes[reg as usize] = Some(write.clone());
        Self::push_reg_write_history(
            &mut self.recent_reg_writes_by_reg[reg as usize],
            write.clone(),
        );
        if !Self::is_trivial_reg_write_for_diagnostic(reg, opcode) {
            self.last_nontrivial_reg_writes[reg as usize] = Some(write.clone());
            Self::push_nontrivial_reg_write_history(
                &mut self.recent_nontrivial_reg_writes[reg as usize],
                write.clone(),
            );
        }
        self.recent_reg_writes.push(write);
    }

    pub fn debug_reg(&self, index: usize) -> u16 {
        self.regs[index & 0x0F]
    }

    pub fn debug_pbr(&self) -> u8 {
        self.pbr
    }

    pub fn debug_rombr(&self) -> u8 {
        self.rombr
    }

    pub fn debug_rambr(&self) -> u8 {
        self.rambr
    }

    pub fn debug_cbr(&self) -> u16 {
        self.cbr
    }

    pub fn debug_scbr(&self) -> u8 {
        self.scbr
    }

    pub fn debug_sfr(&self) -> u16 {
        self.sfr
    }

    pub fn debug_scmr(&self) -> u8 {
        self.scmr
    }

    pub fn debug_cfgr(&self) -> u8 {
        self.cfgr
    }

    pub fn debug_colr(&self) -> u8 {
        self.colr
    }

    pub fn debug_por(&self) -> u8 {
        self.por
    }

    pub fn debug_src_reg(&self) -> u8 {
        self.src_reg
    }

    pub fn debug_dst_reg(&self) -> u8 {
        self.dst_reg
    }

    pub fn debug_top_profile(&self, limit: usize) -> Vec<(u8, u32)> {
        let mut items = self
            .exec_profile
            .iter()
            .enumerate()
            .filter_map(|(opcode, &count)| (count != 0).then_some((opcode as u8, count)))
            .collect::<Vec<_>>();
        items.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        items.truncate(limit);
        items
    }

    pub fn debug_top_profile_by_alt(&self, limit: usize) -> Vec<(u8, u8, u32)> {
        let mut items = Vec::new();
        for alt in 0..4u8 {
            for opcode in 0..=0xFFu8 {
                let count = self.exec_profile_by_alt[alt as usize][opcode as usize];
                if count != 0 {
                    items.push((alt, opcode, count));
                }
            }
        }
        items.sort_by(|a, b| {
            b.2.cmp(&a.2)
                .then_with(|| a.0.cmp(&b.0))
                .then_with(|| a.1.cmp(&b.1))
        });
        items.truncate(limit);
        items
    }

    pub fn debug_nonzero_game_ram(&self) -> usize {
        self.game_ram.iter().filter(|&&value| value != 0).count()
    }

    pub fn debug_nonzero_game_ram_range(&self) -> Option<(usize, usize)> {
        let mut first = None;
        let mut last = None;
        for (idx, &value) in self.game_ram.iter().enumerate() {
            if value != 0 {
                first.get_or_insert(idx);
                last = Some(idx);
            }
        }
        first.zip(last)
    }

    pub fn debug_nonzero_screen_region(&self) -> usize {
        let Some(height) = self.effective_screen_height() else {
            return 0;
        };
        let Some(bpp) = self.bits_per_pixel() else {
            return 0;
        };
        let bytes_per_tile = match bpp {
            2 => 16,
            4 => 32,
            8 => 64,
            _ => return 0,
        };
        let screen_bytes = 32 * (height / 8) * bytes_per_tile;
        let start = self.screen_base_addr();
        let end = start.saturating_add(screen_bytes).min(self.game_ram.len());
        self.game_ram[start..end]
            .iter()
            .filter(|&&value| value != 0)
            .count()
    }

    pub fn debug_recent_pc_transfers(&self) -> &[SuperFxPcTransfer] {
        &self.recent_pc_transfers
    }

    pub fn debug_recent_reg_writes(&self) -> &[SuperFxRegWrite] {
        &self.recent_reg_writes
    }

    pub fn debug_recent_reg_writes_for_reg(&self, reg: u8, limit: usize) -> Vec<SuperFxRegWrite> {
        let reg = reg & 0x0F;
        let limit = limit.max(1);
        let mut items = if !self.recent_reg_writes_by_reg[reg as usize].is_empty() {
            self.recent_reg_writes_by_reg[reg as usize]
                .iter()
                .rev()
                .take(limit)
                .cloned()
                .collect::<Vec<_>>()
        } else {
            self.recent_reg_writes
                .iter()
                .rev()
                .filter(|write| write.reg == reg)
                .take(limit)
                .cloned()
                .collect::<Vec<_>>()
        };
        items.reverse();
        items
    }

    pub fn debug_last_reg_write(&self, reg: u8) -> Option<SuperFxRegWrite> {
        let reg = reg & 0x0F;
        self.last_reg_writes[reg as usize].clone().or_else(|| {
            self.recent_reg_writes
                .iter()
                .rev()
                .find(|w| w.reg == reg)
                .cloned()
        })
    }

    pub fn debug_last_reg_write_excluding(
        &self,
        reg: u8,
        excluded_opcodes: &[u8],
    ) -> Option<SuperFxRegWrite> {
        let reg = reg & 0x0F;
        self.recent_reg_writes
            .iter()
            .rev()
            .find(|write| write.reg == reg && !excluded_opcodes.contains(&write.opcode))
            .cloned()
    }

    pub fn debug_last_nontrivial_reg_write(&self, reg: u8) -> Option<SuperFxRegWrite> {
        self.last_nontrivial_reg_writes[(reg & 0x0F) as usize].clone()
    }

    pub fn debug_recent_nontrivial_reg_writes(&self, reg: u8) -> &[SuperFxRegWrite] {
        &self.recent_nontrivial_reg_writes[(reg & 0x0F) as usize]
    }

    pub fn debug_recent_low_ram_writes(&self) -> &[SuperFxRamWrite] {
        &self.recent_low_ram_writes
    }

    pub fn debug_last_low_ram_write(&self, addr: u16) -> Option<SuperFxRamWrite> {
        (addr < 0x200)
            .then(|| self.last_low_ram_writes[addr as usize].clone())
            .flatten()
    }

    pub fn debug_read_ram_word_short(&self, addr: u16) -> u16 {
        let lo = self
            .ram_addr(addr)
            .map(|idx| self.game_ram[idx])
            .unwrap_or(0xFF);
        let hi = self
            .ram_addr(addr.wrapping_add(1))
            .map(|idx| self.game_ram[idx])
            .unwrap_or(0xFF);
        u16::from_le_bytes([lo, hi])
    }

    pub fn debug_write_ram_word(&mut self, addr: u16, value: u16) {
        self.write_ram_word(addr, value);
    }

    pub fn debug_read_program_source_byte(&self, rom: &[u8], bank: u8, addr: u16) -> Option<u8> {
        self.read_program_source_byte(rom, bank, addr)
    }

    pub fn debug_read_data_source_byte(&self, rom: &[u8], bank: u8, addr: u16) -> Option<u8> {
        self.read_data_source_byte(rom, bank, addr)
    }

    pub fn debug_recent_exec_trace(&self) -> &[SuperFxExecTrace] {
        &self.recent_exec_trace
    }

    pub fn debug_latest_stop_snapshot_meta(&self) -> Option<(u16, u8, u8, u16, u8, u8, usize)> {
        self.latest_stop_snapshot_valid.then_some((
            self.latest_stop_pc,
            self.latest_stop_pbr,
            self.latest_stop_scbr,
            self.latest_stop_height,
            self.latest_stop_bpp,
            self.latest_stop_mode,
            self.latest_stop_snapshot.len(),
        ))
    }

    pub fn debug_selected_screen_snapshot_meta(&self) -> Option<(u16, u8, u8, u16, u8, u8, usize)> {
        let filter_pbr = superfx_screen_buffer_stop_pbr_filter();
        let filter_pc = superfx_screen_buffer_stop_pc_filter();
        if filter_pbr.is_some() || filter_pc.is_some() {
            if let Some(snapshot) = self.recent_stop_snapshots.iter().rev().find(|snapshot| {
                filter_pbr.is_none_or(|pbr| pbr == snapshot.pbr)
                    && filter_pc.is_none_or(|pc| pc == snapshot.pc)
            }) {
                return Some((
                    snapshot.pc,
                    snapshot.pbr,
                    snapshot.scbr,
                    snapshot.height,
                    snapshot.bpp,
                    snapshot.mode,
                    snapshot.data.len(),
                ));
            }
        }
        if let Some(snapshot) = self.debug_pc_snapshot.as_ref() {
            return Some((
                snapshot.pc,
                snapshot.pbr,
                snapshot.scbr,
                snapshot.height,
                snapshot.bpp,
                snapshot.mode,
                snapshot.data.len(),
            ));
        }
        self.debug_latest_stop_snapshot_meta()
    }

    pub fn debug_selected_tile_snapshot_meta(&self) -> Option<(u16, u8, u8, u16, u8, u8, usize)> {
        if let Some(pc) = superfx_tile_snapshot_pc_filter() {
            let rev_index = superfx_tile_snapshot_rev_index();
            if let Some(snapshot) = self
                .recent_tile_snapshots
                .iter()
                .rev()
                .filter(|snapshot| snapshot.pc == pc)
                .nth(rev_index)
            {
                return Some((
                    snapshot.pc,
                    snapshot.pbr,
                    snapshot.scbr,
                    snapshot.height,
                    snapshot.bpp,
                    snapshot.mode,
                    snapshot.data.len(),
                ));
            }
        }
        self.tile_snapshot_valid.then_some((
            self.tile_snapshot_pc,
            self.tile_snapshot_pbr,
            self.tile_snapshot_scbr,
            self.tile_snapshot_height,
            self.tile_snapshot_bpp,
            self.tile_snapshot_mode,
            self.tile_snapshot.len(),
        ))
    }

    pub fn debug_recent_stop_snapshot_metas(
        &self,
        limit: usize,
    ) -> Vec<(u16, u8, u8, u16, u8, u8, usize)> {
        self.recent_stop_snapshots
            .iter()
            .rev()
            .take(limit)
            .map(|snapshot| {
                (
                    snapshot.pc,
                    snapshot.pbr,
                    snapshot.scbr,
                    snapshot.height,
                    snapshot.bpp,
                    snapshot.mode,
                    snapshot.data.len(),
                )
            })
            .collect()
    }

    pub fn debug_current_exec_pbr(&self) -> u8 {
        self.current_exec_pbr
    }

    pub fn debug_current_exec_pc(&self) -> u16 {
        self.current_exec_pc
    }

    pub fn debug_take_save_state_pc_hit(&mut self) -> Option<(u8, u16)> {
        self.save_state_pc_hit.take()
    }

    #[cfg(test)]
    pub(crate) fn debug_set_save_state_pc_hit(&mut self, hit: Option<(u8, u16)>) {
        self.save_state_pc_hit = hit;
    }

    pub fn debug_take_save_state_ram_addr_hit(&mut self) -> Option<(u8, u16, u16)> {
        self.save_state_ram_addr_hit.take()
    }

    pub fn debug_has_pending_save_state_hit(&self) -> bool {
        self.save_state_pc_hit.is_some() || self.save_state_ram_addr_hit.is_some()
    }

    #[cfg(test)]
    pub(crate) fn debug_set_save_state_ram_addr_hit(&mut self, hit: Option<(u8, u16, u16)>) {
        self.save_state_ram_addr_hit = hit;
    }

    fn is_trivial_reg_write_for_diagnostic(reg: u8, opcode: u8) -> bool {
        match reg & 0x0F {
            4 => opcode == 0xE4,
            12 => opcode == 0x3C,
            14 => opcode == 0xEE,
            _ => false,
        }
    }

    fn push_nontrivial_reg_write_history(
        history: &mut Vec<SuperFxRegWrite>,
        write: SuperFxRegWrite,
    ) {
        if let Some(last) = history.last_mut() {
            if last.opcode == write.opcode
                && last.pbr == write.pbr
                && last.pc == write.pc
                && last.reg == write.reg
                && last.src_reg == write.src_reg
                && last.dst_reg == write.dst_reg
            {
                last.new_value = write.new_value;
                last.sfr = write.sfr;
                last.repeats = write.repeats;
                return;
            }
        }
        if history.len() >= trace_superfx_reg_history_cap() {
            history.remove(0);
        }
        history.push(write);
    }

    fn push_reg_write_history(history: &mut Vec<SuperFxRegWrite>, write: SuperFxRegWrite) {
        if let Some(last) = history.last_mut() {
            if last.opcode == write.opcode
                && last.pbr == write.pbr
                && last.pc == write.pc
                && last.reg == write.reg
                && last.src_reg == write.src_reg
                && last.dst_reg == write.dst_reg
            {
                last.new_value = write.new_value;
                last.sfr = write.sfr;
                last.repeats = write.repeats;
                return;
            }
        }
        if history.len() >= trace_superfx_reg_history_cap() {
            history.remove(0);
        }
        history.push(write);
    }

    fn record_low_ram_write(&mut self, addr: u16, old_value: u8, new_value: u8) {
        if addr >= 0x200 || !trace_superfx_low_ram_writes_enabled() {
            return;
        }
        let write = SuperFxRamWrite {
            opcode: self.current_exec_opcode,
            pbr: self.current_exec_pbr,
            pc: self.current_exec_pc,
            addr,
            old_value,
            new_value,
            src_reg: self.src_reg,
            dst_reg: self.dst_reg,
            sfr: self.sfr,
            r10: self.regs[10],
            r12: self.regs[12],
            r14: self.regs[14],
            r15: self.regs[15],
            repeats: 1,
        };
        if let Some(last) = self.recent_low_ram_writes.last_mut() {
            if last.opcode == write.opcode
                && last.pbr == write.pbr
                && last.pc == write.pc
                && last.addr == write.addr
                && last.old_value == write.old_value
                && last.new_value == write.new_value
                && last.src_reg == write.src_reg
                && last.dst_reg == write.dst_reg
                && last.sfr == write.sfr
                && last.r10 == write.r10
                && last.r12 == write.r12
                && last.r14 == write.r14
                && last.r15 == write.r15
            {
                last.repeats = last.repeats.saturating_add(1);
                self.last_low_ram_writes[addr as usize] = Some(last.clone());
                return;
            }
        }
        if self.recent_low_ram_writes.len() >= 64 {
            self.recent_low_ram_writes.remove(0);
        }
        self.recent_low_ram_writes.push(write.clone());
        self.last_low_ram_writes[addr as usize] = Some(write);
    }

    fn reg(&self, index: u8) -> u16 {
        let index = (index & 0x0F) as usize;
        self.regs[index]
    }

    fn write_dest_exec(&mut self, value: u16, opcode: u8, pc: u16) {
        let index = (self.dst_reg & 0x0F) as usize;
        self.write_reg_exec(index, value, opcode, pc);
    }

    fn alt_mode(&self) -> u8 {
        (((self.sfr & SFR_ALT2_BIT) != 0) as u8) << 1 | (((self.sfr & SFR_ALT1_BIT) != 0) as u8)
    }

    fn clear_prefix_flags(&mut self) {
        self.sfr &= !(SFR_ALT1_BIT | SFR_ALT2_BIT | SFR_B_BIT);
        self.src_reg = 0;
        self.dst_reg = 0;
        self.with_reg = 0;
    }

    fn push_recent_exec_trace(&mut self, exec_pbr: u8, pc: u16, opcode: u8) {
        if !trace_superfx_reg_flow_enabled() {
            return;
        }
        if let Some((bank, start, end)) = *trace_superfx_reg_flow_exclude_range() {
            if exec_pbr == bank && pc >= start && pc <= end {
                return;
            }
        }
        if self.recent_exec_trace.len() >= 64 {
            self.recent_exec_trace.remove(0);
        }
        self.recent_exec_trace.push(SuperFxExecTrace {
            opcode,
            pbr: exec_pbr,
            pc,
            src_reg: self.src_reg,
            dst_reg: self.dst_reg,
            sfr: self.sfr,
            r0: self.regs[0],
            r1: self.regs[1],
            r2: self.regs[2],
            r3: self.regs[3],
            r4: self.regs[4],
            r5: self.regs[5],
            r6: self.regs[6],
            r11: self.regs[11],
            r12: self.regs[12],
            r13: self.regs[13],
            r14: self.regs[14],
            r15: self.regs[15],
        });
    }

    fn sync_condition_flags_from_sfr(&mut self) {
        self.shadow_sign = if (self.sfr & SFR_S_BIT) != 0 {
            0x8000
        } else {
            0
        };
        self.shadow_zero = if (self.sfr & SFR_Z_BIT) != 0 { 0 } else { 1 };
        self.shadow_carry = (self.sfr & SFR_CY_BIT) != 0;
        self.shadow_overflow = (self.sfr & SFR_OV_BIT) != 0;
    }

    fn condition_sign_set(&self) -> bool {
        (self.shadow_sign & 0x8000) != 0
    }

    fn condition_zero_set(&self) -> bool {
        self.shadow_zero == 0
    }

    fn condition_carry_set(&self) -> bool {
        self.shadow_carry
    }

    fn condition_overflow_set(&self) -> bool {
        self.shadow_overflow
    }

    fn set_sign_flag(&mut self, set: bool) {
        self.shadow_sign = if set { 0x8000 } else { 0 };
        if set {
            self.sfr |= SFR_S_BIT;
        } else {
            self.sfr &= !SFR_S_BIT;
        }
    }

    fn set_zero_flag(&mut self, set: bool) {
        self.shadow_zero = if set { 0 } else { 1 };
        if set {
            self.sfr |= SFR_Z_BIT;
        } else {
            self.sfr &= !SFR_Z_BIT;
        }
    }

    fn update_sign_zero_flags(&mut self, value: u16) {
        self.shadow_sign = value;
        self.shadow_zero = value;
        if value == 0 {
            self.sfr |= SFR_Z_BIT;
        } else {
            self.sfr &= !SFR_Z_BIT;
        }
        if (value & 0x8000) != 0 {
            self.sfr |= SFR_S_BIT;
        } else {
            self.sfr &= !SFR_S_BIT;
        }
    }

    fn set_carry_flag(&mut self, set: bool) {
        self.shadow_carry = set;
        if set {
            self.sfr |= SFR_CY_BIT;
        } else {
            self.sfr &= !SFR_CY_BIT;
        }
    }

    fn apply_color(&self, source: u8) -> u8 {
        if (self.por & 0x04) != 0 {
            return (self.colr & 0xF0) | (source >> 4);
        }
        if (self.por & 0x08) != 0 {
            return (self.colr & 0xF0) | (source & 0x0F);
        }
        source
    }

    fn set_overflow_flag(&mut self, set: bool) {
        self.shadow_overflow = set;
        if set {
            self.sfr |= SFR_OV_BIT;
        } else {
            self.sfr &= !SFR_OV_BIT;
        }
    }

    pub fn cache_read(&self, offset: u16) -> u8 {
        self.cache_ram[((offset - 0x3100) as usize) & (CACHE_RAM_SIZE - 1)]
    }

    pub fn cache_write(&mut self, offset: u16, value: u8) {
        let idx = ((offset - 0x3100) as usize) & (CACHE_RAM_SIZE - 1);
        self.cache_ram[idx] = value;
        self.cache_valid_mask |= 1u32 << (idx >> 4);
    }

    pub fn game_ram_slice(&self) -> &[u8] {
        &self.game_ram
    }

    pub fn screen_buffer_snapshot(&self) -> Option<(Vec<u8>, u16, u8, u8)> {
        if let Some((snapshot, _scbr, height, bpp, mode)) = self.selected_screen_snapshot() {
            return Some((snapshot.to_vec(), height, bpp, mode));
        }
        let len = self.screen_buffer_len()?;
        let start = self.screen_base_addr();
        let end = start.checked_add(len)?.min(self.game_ram.len());
        let height = self.effective_screen_height()? as u16;
        let bpp = self.bits_per_pixel()? as u8;
        let mode = self.effective_screen_layout_mode();
        (start < end).then(|| (self.game_ram[start..end].to_vec(), height, bpp, mode))
    }

    pub fn screen_buffer_live(&self) -> Option<(Vec<u8>, u16, u8, u8)> {
        let height = self.effective_screen_height()? as u16;
        let bpp = self.bits_per_pixel()? as u8;
        let mode = self.effective_screen_layout_mode();
        let len = self.screen_buffer_len()?;
        let start = self.screen_base_addr();
        let end = start.checked_add(len)?.min(self.game_ram.len());
        (start < end).then(|| (self.game_ram[start..end].to_vec(), height, bpp, mode))
    }

    pub fn screen_buffer_display_snapshot(&self) -> Option<(Vec<u8>, u16, u8, u8)> {
        if superfx_direct_uses_tile_snapshot() {
            if let Some((snapshot, height, bpp, mode)) = self.selected_tile_snapshot() {
                return Some((snapshot.to_vec(), height, bpp, mode));
            }
        }
        if superfx_screen_buffer_stop_pbr_filter().is_none()
            && superfx_screen_buffer_stop_pc_filter().is_none()
            && self.debug_pc_snapshot.is_none()
        {
            if let Some((snapshot, _scbr, height, bpp, mode)) = self.display_screen_snapshot() {
                return Some((snapshot.to_vec(), height, bpp, mode));
            }
        }
        self.selected_screen_snapshot()
            .map(|(snapshot, _scbr, height, bpp, mode)| (snapshot.to_vec(), height, bpp, mode))
    }

    pub fn tile_buffer_snapshot(&self) -> Option<(Vec<u8>, u16, u8, u8)> {
        self.selected_tile_snapshot()
            .map(|(snapshot, height, bpp, mode)| (snapshot.to_vec(), height, bpp, mode))
    }

    pub fn game_ram_read_linear(&self, addr: usize) -> u8 {
        if self.game_ram.is_empty() {
            return 0xFF;
        }
        let real_addr = addr % self.game_ram.len();
        // For DMA reads from the active screen buffer, prefer the latest captured STOP
        // snapshot. Do not silently fall back to tile_snapshot here: B3E4 tile captures
        // can hold tilemap-like staging data rather than final pixel graphics.
        let use_latest = superfx_dma_uses_latest_stop_snapshot();
        if use_latest {
            if let Some((snapshot, scbr, _height, _bpp, _mode)) = self.selected_screen_snapshot() {
                let scbr_base = (scbr as usize) << 10;
                if real_addr >= scbr_base && real_addr < scbr_base + snapshot.len() {
                    return snapshot[real_addr - scbr_base];
                }
            }
        }
        self.game_ram[real_addr]
    }

    pub fn game_ram_write_linear(&mut self, addr: usize, value: u8) {
        if !self.game_ram.is_empty() {
            let idx = addr % self.game_ram.len();
            if env_presence_cached("TRACE_GRAM_LINEAR_W") && value != 0 {
                use std::sync::atomic::{AtomicU32, Ordering};
                static CNT: AtomicU32 = AtomicU32::new(0);
                let n = CNT.fetch_add(1, Ordering::Relaxed);
                if n < 16 {
                    eprintln!(
                        "[GRAM-LINEAR-W] addr={:05X} <- {:02X} (nz#{})",
                        idx, value, n
                    );
                }
            }
            self.game_ram[idx] = value;
        }
    }

    pub fn game_ram_window_addr(&self, offset: u16) -> usize {
        let bank_base = (self.rambr as usize) << 16;
        bank_base + (offset as usize - 0x6000)
    }

    pub fn cpu_rom_addr(bank: u8, offset: u16) -> Option<usize> {
        match bank {
            0x00..=0x3F | 0x80..=0xBF if offset >= 0x8000 => {
                let rom_bank = (bank & 0x3F) as usize;
                Some(rom_bank * 0x8000 + (offset as usize - 0x8000))
            }
            0x40..=0x5F | 0xC0..=0xFF => {
                let rom_bank = (bank & 0x1F) as usize;
                Some(rom_bank * 0x10000 + offset as usize)
            }
            _ => None,
        }
    }

    pub fn illegal_rom_read_value(offset: u16) -> u8 {
        match offset & 0x000F {
            0x4 => 0x04,
            0xA => 0x08,
            0xE => 0x0C,
            0x0 | 0x2 | 0x6 | 0x8 | 0xC => 0x00,
            _ => 0x01,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        parse_save_state_gsu_reg_eq, SaveStateGsuRegEq, StopSnapshot, SuperFx, SuperFxRamWrite,
        SuperFxRegWrite, SFR_ALT1_BIT, SFR_ALT2_BIT, SFR_B_BIT, SFR_CY_BIT, SFR_GO_BIT,
        SFR_IRQ_BIT, SFR_OV_BIT, SFR_S_BIT, SFR_Z_BIT,
    };
    use std::sync::{Mutex, OnceLock};

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    #[test]
    fn cpu_write_r15_finishes_and_raises_irq_when_unmasked() {
        let mut gsu = SuperFx::new(0x20_0000);
        gsu.write_register(0x301E, 0x34);
        gsu.write_register(0x301F, 0x12);

        assert_eq!(gsu.read_register(0x301E, 0), 0x34);
        assert_eq!(gsu.read_register(0x301F, 0), 0x12);
        assert!(!gsu.running());
        assert!(gsu.scpu_irq_asserted());
        let _ = gsu.read_register(0x3031, 0);
        assert!(!gsu.scpu_irq_asserted());
    }

    #[test]
    fn cpu_write_r15_low_byte_does_not_start_until_high_byte_arrives() {
        let mut gsu = SuperFx::new(0x20_0000);

        gsu.write_register(0x301E, 0x34);
        assert_eq!(gsu.read_register(0x301E, 0), 0x34);
        assert!(!gsu.running());
        assert!(!gsu.scpu_irq_asserted());

        gsu.write_register(0x301F, 0x12);
        assert!(gsu.scpu_irq_asserted());
        assert_eq!(gsu.read_register(0x301E, 0), 0x34);
        assert_eq!(gsu.read_register(0x301F, 0), 0x12);
    }

    #[test]
    fn cpu_write_r15_high_byte_arms_execution_without_consuming_steps() {
        let mut gsu = SuperFx::new(0x20_0000);
        let rom = vec![0x00u8; 0x20_0000];

        gsu.write_register_with_rom(0x301E, 0x00, &rom);
        gsu.write_register_with_rom(0x301F, 0x00, &rom);

        assert!(gsu.running());
        assert_ne!(gsu.read_register(0x3030, 0) & (SFR_GO_BIT as u8), 0);
        assert_eq!(gsu.debug_reg(15), 0x0000);
    }

    #[test]
    fn starfox_like_cpu_write_r15_does_not_mutate_working_regs_immediately() {
        let mut gsu = SuperFx::new(0x20_0000);
        let rom = vec![0x00u8; 0x20_0000];

        gsu.pbr = 0x01;
        gsu.rombr = 0x14;
        gsu.scmr = 0x39;
        gsu.write_reg(9, 0x2800);
        gsu.write_reg(13, 0xB3DE);
        gsu.write_reg(14, 0x6242);
        gsu.write_reg(15, 0xB3E6);

        gsu.write_register_with_rom(0x301E, 0x01, &rom);
        assert_eq!(gsu.debug_reg(15), 0xB301);
        assert_eq!(gsu.debug_reg(9), 0x2800);
        assert_eq!(gsu.debug_reg(13), 0xB3DE);
        assert_eq!(gsu.debug_reg(14), 0x6242);

        gsu.write_register_with_rom(0x301F, 0xB3, &rom);
        assert!(gsu.running());
        assert_eq!(gsu.debug_reg(15), 0xB301);
        assert_eq!(gsu.debug_reg(9), 0x2800);
        assert_eq!(gsu.debug_reg(13), 0xB3DE);
        assert_eq!(gsu.debug_reg(14), 0x6242);
    }

    #[test]
    fn starting_execution_keeps_last_completed_tile_snapshot() {
        let mut gsu = SuperFx::new(0x20_0000);
        let mut rom = vec![0u8; 0x20_0000];
        rom[0x0000] = 0x00;
        gsu.regs[15] = 0x8000;
        gsu.tile_snapshot = vec![0xAA; 32];
        gsu.tile_snapshot_valid = true;

        gsu.start_execution(&rom);

        assert!(gsu.tile_snapshot_valid);
        assert_eq!(gsu.tile_snapshot[0], 0xAA);
    }

    #[test]
    fn screen_buffer_snapshot_uses_captured_stop_metadata() {
        let mut gsu = SuperFx::new(0x20_0000);
        gsu.latest_stop_snapshot = vec![0x11; 32];
        gsu.latest_stop_snapshot_valid = true;
        gsu.latest_stop_scbr = 0x0B;
        gsu.latest_stop_height = 192;
        gsu.latest_stop_bpp = 4;
        gsu.latest_stop_mode = 2;
        gsu.latest_stop_pc = 0xFBE4;
        gsu.latest_stop_pbr = 0x06;
        gsu.scbr = 0x00;
        gsu.scmr = 0x00;

        let (buffer, height, bpp, mode) = gsu.screen_buffer_snapshot().unwrap();
        assert_eq!(buffer.len(), 32);
        assert_eq!(height, 192);
        assert_eq!(bpp, 4);
        assert_eq!(mode, 2);
    }

    #[test]
    fn game_ram_read_linear_uses_captured_snapshot_base_address() {
        let mut gsu = SuperFx::new(0x20_0000);
        gsu.latest_stop_snapshot = vec![0x22; 32];
        gsu.latest_stop_snapshot_valid = true;
        gsu.latest_stop_scbr = 0x0B;
        gsu.latest_stop_height = 192;
        gsu.latest_stop_bpp = 4;
        gsu.latest_stop_mode = 2;
        gsu.latest_stop_pc = 0xFBE4;
        gsu.latest_stop_pbr = 0x06;
        gsu.scbr = 0x00;
        gsu.scmr = 0x00;

        assert_eq!(gsu.game_ram_read_linear((0x0Busize << 10) + 7), 0x22);
    }

    #[test]
    fn screen_buffer_display_snapshot_requires_captured_stop_by_default() {
        let mut gsu = SuperFx::new(0x20_0000);
        gsu.tile_snapshot = vec![0xAA; 32];
        gsu.tile_snapshot_valid = true;
        gsu.tile_snapshot_height = 192;
        gsu.tile_snapshot_bpp = 4;
        gsu.tile_snapshot_mode = 2;
        gsu.scbr = 0x0B;
        gsu.scmr = 0x21;
        let live_base = (gsu.scbr as usize) << 10;
        gsu.game_ram[live_base..live_base + 32].fill(0x11);

        assert!(gsu.screen_buffer_display_snapshot().is_none());
    }

    #[test]
    fn display_snapshot_updates_on_dma_and_survives_later_stop_snapshots() {
        let mut gsu = SuperFx::new(0x20_0000);
        gsu.latest_stop_snapshot = vec![0x22; 32];
        gsu.latest_stop_snapshot_valid = true;
        gsu.latest_stop_scbr = 0x0B;
        gsu.latest_stop_height = 192;
        gsu.latest_stop_bpp = 4;
        gsu.latest_stop_mode = 2;
        gsu.latest_stop_pc = 0xD6E6;
        gsu.latest_stop_pbr = 0x01;

        assert!(gsu.capture_display_snapshot_for_dma((0x0Busize << 10) + 4, 8));

        gsu.latest_stop_snapshot = vec![0x33; 32];
        gsu.latest_stop_pc = 0x829D;

        let (buffer, height, bpp, mode) = gsu.screen_buffer_display_snapshot().unwrap();
        let mut expected = vec![0; 32];
        expected[4..12].fill(0x22);
        assert_eq!(buffer, expected);
        assert_eq!(height, 192);
        assert_eq!(bpp, 4);
        assert_eq!(mode, 2);

        assert!(gsu.capture_display_snapshot_for_dma((0x0Busize << 10) + 16, 8));
        let (buffer, _height, _bpp, _mode) = gsu.screen_buffer_display_snapshot().unwrap();
        expected[16..24].fill(0x33);
        assert_eq!(buffer, expected);
    }

    #[test]
    fn save_data_roundtrip_preserves_display_and_tile_snapshots() {
        let mut src = SuperFx::new(0x20_0000);
        src.tile_snapshot = vec![0xAA; 32];
        src.tile_snapshot_valid = true;
        src.tile_snapshot_scbr = 0x0B;
        src.tile_snapshot_height = 192;
        src.tile_snapshot_bpp = 4;
        src.tile_snapshot_mode = 2;
        src.tile_snapshot_pc = 0xB301;
        src.tile_snapshot_pbr = 0x01;
        src.latest_stop_snapshot = vec![0x55; 64];
        src.latest_stop_snapshot_valid = true;
        src.latest_stop_scbr = 0x0C;
        src.latest_stop_height = 160;
        src.latest_stop_bpp = 4;
        src.latest_stop_mode = 1;
        src.latest_stop_pc = 0xFBE4;
        src.latest_stop_pbr = 0x06;
        src.display_snapshot = vec![0x77; 16];
        src.display_snapshot_valid = true;
        src.display_snapshot_scbr = 0x0D;
        src.display_snapshot_height = 128;
        src.display_snapshot_bpp = 2;
        src.display_snapshot_mode = 0;
        src.recent_stop_snapshots.push(StopSnapshot {
            data: vec![0x11; 48],
            scbr: 0x0B,
            height: 192,
            bpp: 4,
            mode: 2,
            pc: 0xB3E4,
            pbr: 0x01,
        });
        src.recent_tile_snapshots.push(StopSnapshot {
            data: vec![0x22; 48],
            scbr: 0x0B,
            height: 192,
            bpp: 4,
            mode: 2,
            pc: 0xB3E4,
            pbr: 0x01,
        });

        let state = src.save_data();
        let mut dst = SuperFx::new(0x20_0000);
        dst.load_data(&state);

        assert_eq!(dst.tile_snapshot, vec![0xAA; 32]);
        assert!(dst.tile_snapshot_valid);
        assert_eq!(dst.tile_snapshot_scbr, 0x0B);
        assert_eq!(dst.tile_snapshot_height, 192);
        assert_eq!(dst.tile_snapshot_bpp, 4);
        assert_eq!(dst.tile_snapshot_mode, 2);
        assert_eq!(dst.tile_snapshot_pc, 0xB301);
        assert_eq!(dst.tile_snapshot_pbr, 0x01);
        assert_eq!(dst.latest_stop_snapshot, vec![0x55; 64]);
        assert!(dst.latest_stop_snapshot_valid);
        assert_eq!(dst.latest_stop_scbr, 0x0C);
        assert_eq!(dst.latest_stop_height, 160);
        assert_eq!(dst.latest_stop_bpp, 4);
        assert_eq!(dst.latest_stop_mode, 1);
        assert_eq!(dst.latest_stop_pc, 0xFBE4);
        assert_eq!(dst.latest_stop_pbr, 0x06);
        assert_eq!(dst.display_snapshot, vec![0x77; 16]);
        assert!(dst.display_snapshot_valid);
        assert_eq!(dst.display_snapshot_scbr, 0x0D);
        assert_eq!(dst.display_snapshot_height, 128);
        assert_eq!(dst.display_snapshot_bpp, 2);
        assert_eq!(dst.display_snapshot_mode, 0);
        assert_eq!(dst.recent_stop_snapshots.len(), 1);
        assert_eq!(dst.recent_stop_snapshots[0].pc, 0xB3E4);
        assert_eq!(dst.recent_stop_snapshots[0].data, vec![0x11; 48]);
        assert_eq!(dst.recent_tile_snapshots.len(), 1);
        assert_eq!(dst.recent_tile_snapshots[0].pc, 0xB3E4);
        assert_eq!(dst.recent_tile_snapshots[0].data, vec![0x22; 48]);
    }

    #[test]
    fn save_data_roundtrip_preserves_last_nontrivial_reg_writes() {
        let mut src = SuperFx::new(0x20_0000);
        src.last_nontrivial_reg_writes[4] = Some(SuperFxRegWrite {
            opcode: 0xA4,
            pbr: 0x01,
            pc: 0xD04D,
            reg: 4,
            old_value: 0x1234,
            new_value: 0x0000,
            src_reg: 0,
            dst_reg: 0,
            sfr: 0x0068,
            repeats: 1,
        });
        src.last_nontrivial_reg_writes[9] = Some(SuperFxRegWrite {
            opcode: 0x19,
            pbr: 0x01,
            pc: 0xD055,
            reg: 9,
            old_value: 0x0E22,
            new_value: 0x0000,
            src_reg: 4,
            dst_reg: 4,
            sfr: 0x1060,
            repeats: 1,
        });

        let state = src.save_data();
        let mut dst = SuperFx::new(0x20_0000);
        dst.load_data(&state);

        let r4 = dst.debug_last_nontrivial_reg_write(4).expect("r4 write");
        assert_eq!(r4.pc, 0xD04D);
        assert_eq!(r4.new_value, 0x0000);
        let r9 = dst.debug_last_nontrivial_reg_write(9).expect("r9 write");
        assert_eq!(r9.pc, 0xD055);
        assert_eq!(r9.old_value, 0x0E22);
        assert_eq!(r9.new_value, 0x0000);
    }

    #[test]
    fn save_data_roundtrip_preserves_recent_nontrivial_reg_writes() {
        let mut src = SuperFx::new(0x20_0000);
        src.recent_nontrivial_reg_writes[0] = vec![
            SuperFxRegWrite {
                opcode: 0xF0,
                pbr: 0x01,
                pc: 0xD069,
                reg: 0,
                old_value: 0x0100,
                new_value: 0x00BF,
                src_reg: 0,
                dst_reg: 0,
                sfr: 0x1060,
                repeats: 1,
            },
            SuperFxRegWrite {
                opcode: 0xA0,
                pbr: 0x01,
                pc: 0x8191,
                reg: 0,
                old_value: 0x0080,
                new_value: 0x0100,
                src_reg: 0,
                dst_reg: 0,
                sfr: 0x0068,
                repeats: 1,
            },
        ];
        src.recent_nontrivial_reg_writes[9] = vec![SuperFxRegWrite {
            opcode: 0x19,
            pbr: 0x01,
            pc: 0xD055,
            reg: 9,
            old_value: 0x0E22,
            new_value: 0x0000,
            src_reg: 4,
            dst_reg: 4,
            sfr: 0x1060,
            repeats: 1,
        }];

        let state = src.save_data();
        let mut dst = SuperFx::new(0x20_0000);
        dst.load_data(&state);

        let r0 = dst.debug_recent_nontrivial_reg_writes(0);
        assert_eq!(r0.len(), 2);
        assert_eq!(r0[0].pc, 0xD069);
        assert_eq!(r0[0].new_value, 0x00BF);
        assert_eq!(r0[1].pc, 0x8191);
        assert_eq!(r0[1].old_value, 0x0080);
        assert_eq!(r0[1].new_value, 0x0100);

        let r9 = dst.debug_recent_nontrivial_reg_writes(9);
        assert_eq!(r9.len(), 1);
        assert_eq!(r9[0].pc, 0xD055);
        assert_eq!(r9[0].new_value, 0x0000);
    }

    #[test]
    fn save_data_roundtrip_preserves_recent_reg_writes() {
        let mut src = SuperFx::new(0x20_0000);
        src.last_reg_writes[14] = Some(SuperFxRegWrite {
            opcode: 0xAE,
            pbr: 0x01,
            pc: 0xB30A,
            reg: 14,
            old_value: 0xF144,
            new_value: 0x34B6,
            src_reg: 0,
            dst_reg: 0,
            sfr: 0x0062,
            repeats: 1,
        });
        src.recent_reg_writes_by_reg[14] = vec![src.last_reg_writes[14].clone().unwrap()];
        src.recent_reg_writes = vec![
            SuperFxRegWrite {
                opcode: 0xAE,
                pbr: 0x01,
                pc: 0xB30A,
                reg: 14,
                old_value: 0xF144,
                new_value: 0x34B6,
                src_reg: 0,
                dst_reg: 0,
                sfr: 0x0062,
                repeats: 1,
            },
            SuperFxRegWrite {
                opcode: 0x04,
                pbr: 0x01,
                pc: 0xB4BF,
                reg: 4,
                old_value: 0x0003,
                new_value: 0x0001,
                src_reg: 4,
                dst_reg: 4,
                sfr: 0x1068,
                repeats: 1,
            },
        ];

        let state = src.save_data();
        let mut dst = SuperFx::new(0x20_0000);
        dst.load_data(&state);

        let writes = dst.debug_recent_reg_writes();
        assert_eq!(writes.len(), 2);
        assert_eq!(writes[0].pc, 0xB30A);
        assert_eq!(writes[0].reg, 14);
        assert_eq!(writes[0].new_value, 0x34B6);
        assert_eq!(writes[1].pc, 0xB4BF);
        assert_eq!(writes[1].reg, 4);
        assert_eq!(writes[1].new_value, 0x0001);
        assert_eq!(dst.debug_last_reg_write(14).unwrap().new_value, 0x34B6);
        let r14 = dst.debug_recent_reg_writes_for_reg(14, 4);
        assert_eq!(r14.len(), 1);
        assert_eq!(r14[0].pc, 0xB30A);
    }

    #[test]
    fn debug_recent_reg_writes_for_reg_filters_and_limits() {
        let mut gsu = SuperFx::new(0x20_0000);
        gsu.recent_reg_writes = vec![
            SuperFxRegWrite {
                opcode: 0x10,
                pbr: 0x01,
                pc: 0x8000,
                reg: 1,
                old_value: 0x0000,
                new_value: 0x0001,
                src_reg: 0,
                dst_reg: 1,
                sfr: 0,
                repeats: 1,
            },
            SuperFxRegWrite {
                opcode: 0x11,
                pbr: 0x01,
                pc: 0x8001,
                reg: 2,
                old_value: 0x0000,
                new_value: 0x0002,
                src_reg: 0,
                dst_reg: 2,
                sfr: 0,
                repeats: 1,
            },
            SuperFxRegWrite {
                opcode: 0x12,
                pbr: 0x01,
                pc: 0x8002,
                reg: 1,
                old_value: 0x0001,
                new_value: 0x0003,
                src_reg: 0,
                dst_reg: 1,
                sfr: 0,
                repeats: 1,
            },
            SuperFxRegWrite {
                opcode: 0x13,
                pbr: 0x01,
                pc: 0x8003,
                reg: 1,
                old_value: 0x0003,
                new_value: 0x0004,
                src_reg: 0,
                dst_reg: 1,
                sfr: 0,
                repeats: 1,
            },
        ];

        let writes = gsu.debug_recent_reg_writes_for_reg(1, 2);

        assert_eq!(writes.len(), 2);
        assert_eq!(writes[0].pc, 0x8002);
        assert_eq!(writes[1].pc, 0x8003);
        assert!(writes.iter().all(|write| write.reg == 1));
    }

    #[test]
    fn save_data_roundtrip_preserves_last_low_ram_writes() {
        let mut src = SuperFx::new(0x20_0000);
        src.last_low_ram_writes[0x28] = Some(SuperFxRamWrite {
            opcode: 0x90,
            pbr: 0x01,
            pc: 0xCF70,
            addr: 0x0028,
            old_value: 0x78,
            new_value: 0x74,
            src_reg: 0,
            dst_reg: 0,
            sfr: 0x0064,
            r10: 0x04CA,
            r12: 0x0000,
            r14: 0xC8EC,
            r15: 0xCF72,
            repeats: 1,
        });
        src.last_low_ram_writes[0x29] = Some(SuperFxRamWrite {
            opcode: 0x90,
            pbr: 0x01,
            pc: 0xCF70,
            addr: 0x0029,
            old_value: 0x7A,
            new_value: 0x7A,
            src_reg: 0,
            dst_reg: 0,
            sfr: 0x0064,
            r10: 0x04CA,
            r12: 0x0000,
            r14: 0xC8EC,
            r15: 0xCF72,
            repeats: 1,
        });

        let state = src.save_data();
        let mut dst = SuperFx::new(0x20_0000);
        dst.load_data(&state);

        let lo = dst
            .debug_last_low_ram_write(0x0028)
            .expect("low byte write");
        assert_eq!(lo.pc, 0xCF70);
        assert_eq!(lo.new_value, 0x74);
        assert_eq!(lo.r10, 0x04CA);

        let hi = dst
            .debug_last_low_ram_write(0x0029)
            .expect("high byte write");
        assert_eq!(hi.pc, 0xCF70);
        assert_eq!(hi.new_value, 0x7A);
        assert_eq!(hi.r15, 0xCF72);
    }

    #[test]
    fn game_ram_read_linear_does_not_fall_back_to_tile_snapshot_by_default() {
        let mut gsu = SuperFx::new(0x20_0000);
        gsu.tile_snapshot = vec![0xAA; 32];
        gsu.tile_snapshot_valid = true;
        gsu.tile_snapshot_scbr = 0x0B;
        gsu.scbr = 0x0B;
        gsu.scmr = 0x21;
        let live_addr = (0x0Busize << 10) + 7;
        gsu.game_ram[live_addr] = 0x11;

        assert_eq!(gsu.game_ram_read_linear(live_addr), 0x11);
    }

    #[test]
    fn illegal_rom_pattern_matches_documented_dummy_values() {
        assert_eq!(SuperFx::illegal_rom_read_value(0x8000), 0x00);
        assert_eq!(SuperFx::illegal_rom_read_value(0x8004), 0x04);
        assert_eq!(SuperFx::illegal_rom_read_value(0x800A), 0x08);
        assert_eq!(SuperFx::illegal_rom_read_value(0x800E), 0x0C);
        assert_eq!(SuperFx::illegal_rom_read_value(0x8001), 0x01);
    }

    #[test]
    fn cpu_rom_addr_maps_e0_ff_banks_like_c0_df_mirrors() {
        assert_eq!(SuperFx::cpu_rom_addr(0xC2, 0x8515), Some(0x28515));
        assert_eq!(SuperFx::cpu_rom_addr(0xE2, 0x8515), Some(0x28515));
        assert_eq!(SuperFx::cpu_rom_addr(0xFF, 0xFFFF), Some(0x1FFFFF));
    }

    #[test]
    fn writing_sfr_go_directly_triggers_noop_completion() {
        let mut gsu = SuperFx::new(0x20_0000);
        gsu.write_register(0x3031, 0x00);
        gsu.write_register(0x3030, 0x20);

        assert!(!gsu.running());
        assert!((gsu.sfr & SFR_IRQ_BIT) != 0);
    }

    #[test]
    fn sfr_low_reflects_natural_go_clear_after_run_has_completed() {
        let mut gsu = SuperFx::new(0x20_0000);
        gsu.write_register(0x301E, 0x30);
        gsu.write_register(0x301F, 0xB3);

        assert!(!gsu.running());
        assert_eq!(gsu.read_register(0x3030, 0xFF) & (SFR_GO_BIT as u8), 0);
    }

    #[test]
    fn sfr_low_reports_raw_sfr_bits_even_when_execution_has_stopped() {
        let mut gsu = SuperFx::new(0x20_0000);
        gsu.sfr = 0x0030;
        gsu.running = false;

        assert_eq!(gsu.read_register(0x3030, 0xFF), 0x30);
    }

    #[test]
    fn read_data_rom_byte_reads_from_buffer_without_modifying_r14() {
        // GETB reads from the ROM buffer without auto-incrementing R14.
        // R14 must be managed explicitly by the program.
        let mut gsu = SuperFx::new(0x20_0000);
        let mut rom = vec![0u8; 0x20_0000];
        rom[0x1_0000] = 0x34;

        gsu.rombr = 0x02;
        gsu.write_reg(14, 0x0000);

        // First read refreshes the buffer from current ROMB/R14.
        assert_eq!(gsu.read_data_rom_byte(&rom), Some(0x34));
        // Second read returns same data (R14 unchanged, buffer unchanged)
        assert_eq!(gsu.read_data_rom_byte(&rom), Some(0x34));
        assert_eq!(gsu.rombr, 0x02);
        assert_eq!(gsu.regs[14], 0x0000); // R14 not modified
    }

    #[test]
    fn write_reg_r14_triggers_rom_buffer_reload() {
        // R14 writes mark the ROM buffer dirty; the next GETB/GETC read
        // refreshes from the new address.
        let mut gsu = SuperFx::new(0x20_0000);
        let mut rom = vec![0u8; 0x20_0000];
        rom[0x0A_7141] = 0x20;
        rom[0x0A_7142] = 0x6F;

        gsu.rombr = 0x14;
        gsu.write_reg(14, 0xF141);
        assert_eq!(gsu.read_data_rom_byte(&rom), Some(0x20));

        // DEC R14 triggers reload from new address
        gsu.write_reg(14, 0xF142);
        assert_eq!(gsu.read_data_rom_byte(&rom), Some(0x6F));
        assert_eq!(gsu.regs[14], 0xF142); // R14 not modified by read
    }

    #[test]
    fn write_reg_r14_uses_current_rombr_when_buffer_refreshes() {
        // Match bsnes more closely: the ROM buffer is refreshed after the
        // instruction using the current ROMB/R14, not a bank captured at write time.
        let mut gsu = SuperFx::new(0x20_0000);
        let mut rom = vec![0u8; 0x20_0000];
        rom[0x0A_7141] = 0x20;
        rom[0x0B_F141] = 0x33;

        gsu.rombr = 0x14;
        gsu.write_reg(14, 0xF141);
        gsu.rombr = 0x17; // change rombr AFTER write_reg

        // Read uses the current ROMB at refresh time.
        assert_eq!(gsu.read_data_rom_byte(&rom), Some(0x33));
    }

    #[test]
    fn cpu_write_r14_preserves_pending_rom_reload_into_start_execution() {
        let mut gsu = SuperFx::new(0x20_0000);
        let mut rom = vec![0u8; 0x20_0000];
        rom[0x0A_7141] = 0x20;

        gsu.rombr = 0x14;
        gsu.write_register_with_rom(0x301C, 0x41, &rom);
        gsu.write_register_with_rom(0x301D, 0xF1, &rom);

        assert!(gsu.rom_buffer_pending);
        assert!(!gsu.rom_buffer_valid);
        assert_eq!(gsu.rom_buffer_pending_bank, 0x14);
        assert_eq!(gsu.rom_buffer_pending_addr, 0xF141);

        gsu.debug_prepare_cpu_start(&rom);

        assert_eq!(gsu.read_data_rom_byte(&rom), Some(0x20));
    }

    #[test]
    fn cpu_write_pbr_invalidates_cache_lines() {
        let mut gsu = SuperFx::new(0x20_0000);
        gsu.cache_enabled = true;
        gsu.cache_valid_mask = u32::MAX;

        gsu.write_register_with_rom(0x3034, 0x21, &[]);

        assert_eq!(gsu.pbr, 0x21);
        assert_eq!(gsu.cache_valid_mask, 0);
    }

    #[test]
    fn read_data_rom_byte_uses_bsnes_lorom_mapping_for_low_banks() {
        let mut gsu = SuperFx::new(0x10_0000);
        let mut rom = vec![0u8; 0x10_0000];
        rom[0x0A_56C1] = 0x1F;
        rom[0x0A_56C0] = 0xD5;

        gsu.rombr = 0x14;
        gsu.write_reg(14, 0x56C1);
        assert_eq!(gsu.read_data_rom_byte(&rom), Some(0x1F));

        gsu.write_reg(14, 0x56C0);
        assert_eq!(gsu.read_data_rom_byte(&rom), Some(0xD5));
    }

    #[test]
    fn rombr_write_clears_alt3_before_following_iwt_table_setup() {
        let mut gsu = SuperFx::new(0x20_0000);
        let mut rom = vec![0u8; 0x20_0000];
        rom[0x0000..0x000A].copy_from_slice(&[
            0xA0, 0x14, // IBT R0,#14
            0x3F, // ALT3
            0xDF, // ROMBR = R0, then clear prefix flags
            0xFB, 0xB8, 0x1A, // IWT R11,#1AB8
            0xFC, 0x2C, 0x01, // IWT R12,#012C
        ]);

        // If ALT3 leaked past DF, FB/FC would behave as LM and read these words instead.
        gsu.write_ram_word(0x1AB8, 0x9C09);
        gsu.write_ram_word(0x012C, 0x004B);
        gsu.regs[15] = 0x8000;
        gsu.running = true;

        gsu.run_steps(&rom, 16);

        assert_eq!(gsu.debug_rombr(), 0x14);
        assert_eq!(gsu.regs[11], 0x1AB8);
        assert_eq!(gsu.regs[12], 0x012C);
    }

    #[test]
    fn parse_save_state_gsu_reg_eq_accepts_register_constraints() {
        let parsed = parse_save_state_gsu_reg_eq("r2=004B,7:236E").expect("expected constraints");
        assert_eq!(
            parsed,
            vec![
                SaveStateGsuRegEq {
                    reg: 2,
                    value: 0x004B,
                },
                SaveStateGsuRegEq {
                    reg: 7,
                    value: 0x236E,
                },
            ]
        );

        let parsed_upper =
            parse_save_state_gsu_reg_eq("R2=004B,R7=236E").expect("expected uppercase constraints");
        assert_eq!(
            parsed_upper,
            vec![
                SaveStateGsuRegEq {
                    reg: 2,
                    value: 0x004B,
                },
                SaveStateGsuRegEq {
                    reg: 7,
                    value: 0x236E,
                },
            ]
        );
    }

    #[test]
    fn parse_save_state_superfx_ram_byte_eq_accepts_addr_value_pairs() {
        let parsed = super::parse_save_state_superfx_ram_byte_eq("0x0136=4B,0x0137:00")
            .expect("expected filters");
        assert_eq!(
            parsed,
            vec![
                super::SaveStateSuperfxRamByteEq {
                    addr: 0x0136,
                    value: 0x4B,
                },
                super::SaveStateSuperfxRamByteEq {
                    addr: 0x0137,
                    value: 0x00,
                },
            ]
        );

        assert!(super::parse_save_state_superfx_ram_byte_eq("0136=123").is_none());
    }

    #[test]
    fn parse_save_state_gsu_recent_exec_tail_accepts_pc_pairs() {
        let parsed = super::parse_save_state_gsu_recent_exec_tail("01:AF77,01:ACA6")
            .expect("expected recent exec tail");
        assert_eq!(parsed, vec![(0x01, 0xAF77), (0x01, 0xACA6)]);
    }

    #[test]
    fn recent_exec_trace_ends_with_matches_tail_sequence() {
        let trace = vec![
            super::SuperFxExecTrace {
                opcode: 0x11,
                pbr: 0x01,
                pc: 0xAF71,
                src_reg: 0,
                dst_reg: 0,
                sfr: 0,
                r0: 0,
                r1: 0,
                r2: 0,
                r3: 0,
                r4: 0,
                r5: 0,
                r6: 0,
                r11: 0,
                r12: 0,
                r13: 0,
                r14: 0,
                r15: 0,
            },
            super::SuperFxExecTrace {
                opcode: 0xFF,
                pbr: 0x01,
                pc: 0xAF77,
                src_reg: 0,
                dst_reg: 0,
                sfr: 0,
                r0: 0,
                r1: 0,
                r2: 0,
                r3: 0,
                r4: 0,
                r5: 0,
                r6: 0,
                r11: 0,
                r12: 0,
                r13: 0,
                r14: 0,
                r15: 0,
            },
            super::SuperFxExecTrace {
                opcode: 0x60,
                pbr: 0x01,
                pc: 0xACA6,
                src_reg: 0,
                dst_reg: 0,
                sfr: 0,
                r0: 0,
                r1: 0,
                r2: 0,
                r3: 0,
                r4: 0,
                r5: 0,
                r6: 0,
                r11: 0,
                r12: 0,
                r13: 0,
                r14: 0,
                r15: 0,
            },
        ];

        assert!(super::recent_exec_trace_ends_with(
            &trace,
            &[(0x01, 0xAF77), (0x01, 0xACA6)]
        ));
        assert!(!super::recent_exec_trace_ends_with(
            &trace,
            &[(0x01, 0xAF71), (0x01, 0xACA6)]
        ));
    }

    #[test]
    fn save_state_gsu_reg_eq_matches_current_register_values() {
        let mut gsu = SuperFx::new(0x20_0000);
        gsu.write_reg(2, 0x004B);
        gsu.write_reg(7, 0x236E);

        let items = vec![
            SaveStateGsuRegEq {
                reg: 2,
                value: 0x004B,
            },
            SaveStateGsuRegEq {
                reg: 7,
                value: 0x236E,
            },
        ];
        assert!(items
            .iter()
            .all(|item| gsu.debug_reg(item.reg as usize) == item.value));

        gsu.write_reg(7, 0x004B);
        assert_ne!(gsu.debug_reg(7), 0x236E);
    }

    #[test]
    fn getb_preserves_sign_and_zero_flags() {
        let mut gsu = SuperFx::new(0x20_0000);
        let mut rom = vec![0u8; 0x20_0000];
        rom[0x0000] = 0x5A;

        gsu.dst_reg = 3;
        gsu.sfr |= super::SFR_S_BIT | super::SFR_Z_BIT;
        gsu.rombr = 0x00;
        gsu.write_reg(14, 0x8000);

        assert!(gsu.execute_opcode(0xEF, &rom, 0x8000));
        assert_eq!(gsu.regs[3], 0x005A);
        assert_ne!(gsu.sfr & super::SFR_S_BIT, 0);
        assert_ne!(gsu.sfr & super::SFR_Z_BIT, 0);
    }

    #[test]
    fn getbh_preserves_sign_and_zero_flags() {
        let mut gsu = SuperFx::new(0x20_0000);
        let mut rom = vec![0u8; 0x20_0000];
        rom[0x0000] = 0x5A;

        gsu.src_reg = 4;
        gsu.dst_reg = 5;
        gsu.regs[4] = 0x0034;
        gsu.sfr |= super::SFR_ALT1_BIT | super::SFR_S_BIT;
        gsu.rombr = 0x00;
        gsu.write_reg(14, 0x8000);

        assert!(gsu.execute_opcode(0xEF, &rom, 0x8000));
        assert_eq!(gsu.regs[5], 0x5A34);
        assert_ne!(gsu.sfr & super::SFR_S_BIT, 0);
        assert_eq!(gsu.sfr & super::SFR_Z_BIT, 0);
    }

    #[test]
    fn getb_write_to_r14_marks_rom_buffer_dirty_until_next_read() {
        let mut gsu = SuperFx::new(0x20_0000);
        let mut rom = vec![0u8; 0x20_0000];
        rom[0x0000] = 0x34;
        rom[0x0034] = 0x77;

        gsu.dst_reg = 14;
        gsu.rombr = 0x00;
        gsu.write_reg(14, 0x8000);

        assert!(gsu.execute_opcode(0xEF, &rom, 0x8000));
        assert_eq!(gsu.regs[14], 0x0034);
        assert!(gsu.rom_buffer_valid);
        assert!(!gsu.rom_buffer_pending);
        assert_eq!(gsu.read_data_rom_byte(&rom), Some(0x77));
    }

    #[test]
    fn last_reg_write_excluding_skips_trivial_opcode() {
        let mut gsu = SuperFx::new(0x20_0000);
        gsu.recent_reg_writes.push(super::SuperFxRegWrite {
            opcode: 0xAC,
            pbr: 0x01,
            pc: 0xB380,
            reg: 12,
            old_value: 0,
            new_value: 8,
            src_reg: 0,
            dst_reg: 12,
            sfr: 0,
            repeats: 1,
        });
        gsu.recent_reg_writes.push(super::SuperFxRegWrite {
            opcode: 0x3C,
            pbr: 0x01,
            pc: 0xB391,
            reg: 12,
            old_value: 8,
            new_value: 7,
            src_reg: 0,
            dst_reg: 0,
            sfr: 0,
            repeats: 1,
        });

        let last = gsu
            .debug_last_reg_write_excluding(12, &[0x3C])
            .expect("expected non-trivial writer");
        assert_eq!(last.opcode, 0xAC);
        assert_eq!(last.pc, 0xB380);
        assert_eq!(last.new_value, 8);
    }

    #[test]
    fn last_reg_write_excluding_returns_none_when_only_excluded_opcodes_exist() {
        let mut gsu = SuperFx::new(0x20_0000);
        gsu.recent_reg_writes.push(super::SuperFxRegWrite {
            opcode: 0xE4,
            pbr: 0x01,
            pc: 0xB397,
            reg: 4,
            old_value: 2,
            new_value: 1,
            src_reg: 0,
            dst_reg: 0,
            sfr: 0,
            repeats: 1,
        });

        assert!(gsu.debug_last_reg_write_excluding(4, &[0xE4]).is_none());
    }

    #[test]
    fn record_low_ram_write_tracks_last_writer_for_address() {
        let mut gsu = SuperFx::new(0x20_0000);
        gsu.current_exec_pbr = 0x01;
        gsu.current_exec_pc = 0x8619;
        gsu.current_exec_opcode = 0xA0;
        gsu.src_reg = 0;
        gsu.dst_reg = 0;
        gsu.sfr = 0x0166;

        gsu.record_low_ram_write(0x0032, 0x00, 0x8E);
        gsu.record_low_ram_write(0x0032, 0x00, 0x8E);
        let write = gsu
            .debug_last_low_ram_write(0x0032)
            .expect("expected low RAM writer");
        assert_eq!(write.pc, 0x8619);
        assert_eq!(write.new_value, 0x8E);
        assert_eq!(write.r10, 0);
        assert_eq!(write.r12, 0);
        assert_eq!(write.r14, 0);
        assert_eq!(write.r15, 0);
        assert_eq!(write.repeats, 2);
        assert!(gsu.debug_last_low_ram_write(0x0100).is_none());
    }

    #[test]
    fn record_low_ram_write_tracks_upper_short_ram_addresses() {
        let mut gsu = SuperFx::new(0x20_0000);
        gsu.current_exec_pbr = 0x01;
        gsu.current_exec_pc = 0xD1A8;
        gsu.current_exec_opcode = 0xA2;
        gsu.src_reg = 0;
        gsu.dst_reg = 0;
        gsu.sfr = 0x0064;
        gsu.regs[10] = 0x04C8;
        gsu.regs[12] = 0x012C;
        gsu.regs[14] = 0x083B;
        gsu.regs[15] = 0xD1AB;

        gsu.record_low_ram_write(0x01FE, 0x00, 0xA8);
        gsu.record_low_ram_write(0x01FF, 0x00, 0xBC);

        let lo = gsu
            .debug_last_low_ram_write(0x01FE)
            .expect("expected upper short RAM low byte write");
        assert_eq!(lo.pc, 0xD1A8);
        assert_eq!(lo.new_value, 0xA8);

        let hi = gsu
            .debug_last_low_ram_write(0x01FF)
            .expect("expected upper short RAM high byte write");
        assert_eq!(hi.pc, 0xD1A8);
        assert_eq!(hi.new_value, 0xBC);
    }

    #[test]
    fn load_data_clears_transient_exec_state() {
        let source = SuperFx::new(0x20_0000);
        let state = source.save_data();

        let mut restored = SuperFx::new(0x20_0000);
        restored.current_exec_pbr = 0x01;
        restored.current_exec_pc = 0xD040;
        restored.current_exec_opcode = 0x19;
        restored.save_state_pc_hit = Some((0x01, 0xD040));
        restored.save_state_pc_hit_count = 3;
        restored.recent_exec_trace.push(super::SuperFxExecTrace {
            opcode: 0x19,
            pbr: 0x01,
            pc: 0xD040,
            src_reg: 4,
            dst_reg: 4,
            sfr: 0x1066,
            r0: 0,
            r1: 0,
            r2: 0,
            r3: 0,
            r4: 0,
            r5: 0,
            r6: 0,
            r11: 0,
            r12: 0,
            r13: 0,
            r14: 0,
            r15: 0,
        });

        restored.load_data(&state);

        assert_eq!(restored.debug_current_exec_pbr(), 0);
        assert_eq!(restored.debug_current_exec_pc(), 0);
        assert_eq!(restored.current_exec_opcode, 0);
        assert!(restored.debug_take_save_state_pc_hit().is_none());
        assert_eq!(restored.save_state_pc_hit_count, 0);
        assert!(restored.recent_exec_trace.is_empty());
    }

    #[test]
    fn rewind_pipe_to_instruction_boundary_restores_current_opcode() {
        let mut gsu = SuperFx::new(0x20_0000);
        gsu.regs[15] = 0x8123;
        gsu.pbr = 0x05;
        gsu.pipe = 0x11;
        gsu.pipe_valid = true;
        gsu.pipe_pc = 0x8122;
        gsu.pipe_pbr = 0x05;

        gsu.rewind_pipe_to_instruction_boundary(0x01, 0x8000, 0x0D);

        assert!(gsu.pipe_valid);
        assert_eq!(gsu.pipe_pbr, 0x01);
        assert_eq!(gsu.pipe_pc, 0x8000);
        assert_eq!(gsu.pipe, 0x0D);
        assert_eq!(gsu.regs[15], 0x8001);
    }

    #[test]
    fn record_reg_write_tracks_last_nontrivial_writer_per_register() {
        let mut gsu = SuperFx::new(0x20_0000);
        gsu.last_nontrivial_reg_writes[12] = Some(super::SuperFxRegWrite {
            opcode: 0xAC,
            pbr: 0x01,
            pc: 0xB380,
            reg: 12,
            old_value: 0,
            new_value: 8,
            src_reg: 0,
            dst_reg: 12,
            sfr: 0,
            repeats: 1,
        });

        let last = gsu
            .debug_last_nontrivial_reg_write(12)
            .expect("expected tracked writer");
        assert_eq!(last.opcode, 0xAC);
        assert_eq!(last.pc, 0xB380);
        assert_eq!(last.new_value, 8);
    }

    #[test]
    fn push_nontrivial_reg_write_history_coalesces_same_writer() {
        let mut history = Vec::new();
        let write = super::SuperFxRegWrite {
            opcode: 0x04,
            pbr: 0x01,
            pc: 0xB4BF,
            reg: 4,
            old_value: 2,
            new_value: 4,
            src_reg: 4,
            dst_reg: 4,
            sfr: 0,
            repeats: 1,
        };
        super::SuperFx::push_nontrivial_reg_write_history(&mut history, write.clone());
        let mut updated = write.clone();
        updated.new_value = 8;
        updated.repeats = 3;
        super::SuperFx::push_nontrivial_reg_write_history(&mut history, updated);

        assert_eq!(history.len(), 1);
        assert_eq!(history[0].new_value, 8);
        assert_eq!(history[0].repeats, 3);
    }

    #[test]
    fn cpu_writes_do_not_override_read_only_rombr_and_rambr() {
        let mut gsu = SuperFx::new(0x20_0000);
        gsu.rombr = 0x12;
        gsu.rambr = 0x01;

        gsu.write_register(0x3036, 0xB2);
        gsu.write_register(0x303C, 0x03);

        assert_eq!(gsu.debug_rombr(), 0x12);
        assert_eq!(gsu.debug_rambr(), 0x01);
    }

    #[test]
    fn read_only_rambr_register_exposes_low_two_bits() {
        let mut gsu = SuperFx::new(0x20_0000);
        gsu.rambr = 0x03;

        assert_eq!(gsu.read_register(0x303C, 0x00), 0x03);
    }

    #[test]
    fn cpu_writes_do_not_override_read_only_cbr() {
        let mut gsu = SuperFx::new(0x20_0000);
        gsu.cbr = 0x34B0;
        gsu.cache_valid_mask = u32::MAX;

        gsu.write_register(0x303E, 0xBE);
        gsu.write_register(0x303F, 0x34);

        assert_eq!(gsu.debug_cbr(), 0x34B0);
        assert_eq!(gsu.cache_valid_mask, u32::MAX);
    }

    #[test]
    fn cpu_cache_window_marks_written_line_valid() {
        let mut gsu = SuperFx::new(0x20_0000);

        gsu.write_register(0x3112, 0x5A);

        assert_eq!(gsu.cache_read(0x3112), 0x5A);
        assert_ne!(gsu.cache_valid_mask & (1 << 1), 0);
    }

    #[test]
    fn ram_word_access_keeps_last_ram_addr_at_word_base() {
        let mut gsu = SuperFx::new(0x20_0000);

        gsu.write_ram_word(0x1234, 0xBEEF);
        assert_eq!(gsu.last_ram_addr, 0x1234);

        let value = gsu.read_ram_word(0x1234);
        assert_eq!(value, 0xBEEF);
        assert_eq!(gsu.last_ram_addr, 0x1234);
    }

    #[test]
    fn direct_ram_word_access_uses_xor_one_for_high_byte() {
        let mut gsu = SuperFx::new(0x20_0000);

        gsu.write_ram_word(0x1235, 0xBEEF);

        assert_eq!(gsu.game_ram[0x1235], 0xEF);
        assert_eq!(gsu.game_ram[0x1234], 0xBE);
        assert_eq!(gsu.read_ram_word(0x1235), 0xBEEF);
    }

    #[test]
    fn short_ram_word_access_uses_plus_one_for_high_byte() {
        let mut gsu = SuperFx::new(0x20_0000);

        gsu.write_ram_word_short(0x1235, 0xBEEF);

        assert_eq!(gsu.game_ram[0x1235], 0xEF);
        assert_eq!(gsu.game_ram[0x1236], 0xBE);
        assert_eq!(gsu.read_ram_word_short(0x1235), 0xBEEF);
    }

    #[test]
    fn buffered_ram_word_write_defers_final_byte_until_sync() {
        let mut gsu = SuperFx::new(0x20_0000);

        gsu.write_ram_buffer_word(0x1234, 0xBEEF);

        assert_eq!(gsu.game_ram[0x1234], 0xEF);
        assert_eq!(gsu.game_ram[0x1235], 0x00);
        assert!(gsu.ram_buffer_pending);
        assert_eq!(gsu.ram_buffer_pending_addr, 0x1235);
        assert_eq!(gsu.ram_buffer_pending_data, 0xBE);

        gsu.sync_ram_buffer();

        assert_eq!(gsu.game_ram[0x1235], 0xBE);
        assert!(!gsu.ram_buffer_pending);
    }

    #[test]
    fn ramb_flushes_pending_buffer_before_bank_switch() {
        let mut gsu = SuperFx::new(0x20_0000);
        let rom = vec![0u8; 0x20_0000];

        gsu.write_ram_buffer_byte(0x0010, 0xAA);
        gsu.src_reg = 0;
        gsu.regs[0] = 0x0001;
        gsu.sfr = SFR_ALT2_BIT;

        assert!(gsu.execute_opcode_internal(0xDF, &rom, 0x8000, false));

        assert_eq!(gsu.game_ram[0x0010], 0xAA);
        assert_eq!(gsu.rambr, 0x01);
        assert!(!gsu.ram_buffer_pending);
    }

    #[test]
    fn sbk_stores_back_to_base_of_last_word_access() {
        let mut gsu = SuperFx::new(0x20_0000);
        let rom = vec![0u8; 0x20_0000];
        gsu.write_ram_word(0x2000, 0x1122);
        gsu.write_ram_word(0x2002, 0x3344);
        gsu.src_reg = 1;
        gsu.regs[1] = 0xA1B2;

        let _ = gsu.read_ram_word(0x2000);
        assert!(gsu.execute_opcode_internal(0x90, &rom, 0x8000, false));

        assert_eq!(gsu.read_ram_word(0x2000), 0xA1B2);
        assert_eq!(gsu.read_ram_word(0x2002), 0x3344);
    }

    #[test]
    fn read_program_rom_byte_uses_high_32k_rom_windows() {
        let mut gsu = SuperFx::new(0x20_0000);
        let mut rom = vec![0u8; 0x20_0000];
        rom[0x8001] = 0xA5;

        assert_eq!(gsu.read_program_rom_byte(&rom, 0x01, 0x8001), Some(0xA5));
        assert_eq!(gsu.read_program_rom_byte(&rom, 0x01, 0x0001), Some(0xA5));
    }

    #[test]
    fn read_program_rom_byte_prefers_cache_window_over_rom() {
        let mut gsu = SuperFx::new(0x20_0000);
        let mut rom = vec![0u8; 0x20_0000];
        rom[0x0034] = 0x11;
        gsu.cache_enabled = true;
        gsu.cache_ram[0x34] = 0xA5;
        gsu.cache_valid_mask = u32::MAX;

        assert_eq!(gsu.read_program_rom_byte(&rom, 0x00, 0x0034), Some(0xA5));
    }

    #[test]
    fn cache_opcode_invalidates_lines_and_refills_on_demand() {
        let mut gsu = SuperFx::new(0x20_0000);
        let mut rom = vec![0u8; 0x20_0000];
        rom[0x0230] = 0xA5;
        rom[0x0235] = 0x5A;
        gsu.pbr = 0x00;
        gsu.regs[15] = 0x8235;

        assert!(gsu.execute_opcode_internal(0x02, &rom, 0x8234, false));
        assert!(gsu.cache_enabled);
        assert_eq!(gsu.cbr, 0x8230);
        assert_eq!(gsu.cache_valid_mask, 0);
        assert_eq!(gsu.read_program_rom_byte(&rom, 0x00, 0x8235), Some(0x5A));
        assert_ne!(gsu.cache_valid_mask & 1, 0);
        assert_eq!(gsu.cache_ram[0x00], 0xA5);
        assert_eq!(gsu.cache_ram[0x05], 0x5A);
    }

    #[test]
    fn cache_opcode_keeps_valid_lines_when_base_is_unchanged() {
        let mut gsu = SuperFx::new(0x20_0000);
        let rom = vec![0u8; 0x20_0000];
        gsu.pbr = 0x00;
        gsu.regs[15] = 0x8235;
        gsu.cbr = 0x8230;
        gsu.cache_enabled = true;
        gsu.cache_valid_mask = 0x1234_5678;
        gsu.cache_ram[0x35] = 0xA5;

        assert!(gsu.execute_opcode_internal(0x02, &rom, 0x8234, false));
        assert!(gsu.cache_enabled);
        assert_eq!(gsu.cbr, 0x8230);
        assert_eq!(gsu.cache_valid_mask, 0x1234_5678);
        assert_eq!(gsu.cache_ram[0x35], 0xA5);
    }

    #[test]
    fn cache_opcode_uses_prefetched_r15_window_at_16byte_boundary() {
        let mut gsu = SuperFx::new(0x20_0000);
        let rom = vec![0u8; 0x20_0000];
        gsu.pbr = 0x01;
        // snes9x's fx_cache uses R15, and under the pipelined core that points at the
        // prefetched stream. Star Fox later executes 01:84FB from cache page 000B,
        // which requires CACHE at 01:84EE to land on 0x84F0.
        gsu.regs[15] = 0x84F0;

        assert!(gsu.execute_opcode_internal(0x02, &rom, 0x84EE, false));
        assert!(gsu.cache_enabled);
        assert_eq!(gsu.cbr, 0x84F0);
    }

    #[test]
    fn cache_fetch_uses_cbr_relative_window() {
        let mut gsu = SuperFx::new(0x20_0000);
        let mut rom = vec![0u8; 0x20_0000];
        rom[0x1230] = 0x9A;
        rom[0x1231] = 0xBC;
        gsu.cache_enabled = true;
        gsu.cbr = 0x9230;

        assert_eq!(gsu.read_program_rom_byte(&rom, 0x00, 0x9230), Some(0x9A));
        assert_eq!(gsu.read_program_rom_byte(&rom, 0x00, 0x9231), Some(0xBC));
        assert_eq!(gsu.cache_ram[0x00], 0x9A);
        assert_eq!(gsu.cache_ram[0x01], 0xBC);
    }

    #[test]
    fn read_program_rom_byte_uses_rom_outside_cache_window() {
        let mut gsu = SuperFx::new(0x20_0000);
        let mut rom = vec![0u8; 0x20_0000];
        rom[0x2234] = 0x5A;
        gsu.cbr = 0x1200;
        gsu.cache_enabled = true;
        gsu.cache_ram[0x34] = 0xA5;
        gsu.cache_valid_mask = u32::MAX;

        assert_eq!(gsu.read_program_rom_byte(&rom, 0x00, 0xA234), Some(0x5A));
    }

    #[test]
    fn read_program_rom_byte_reads_program_ram_banks() {
        let rom = vec![0u8; 0x20_0000];
        let mut gsu = SuperFx::new(0x20_0000);
        gsu.game_ram[0x1234] = 0xA5;
        gsu.game_ram[0x1_1234] = 0x5A;

        assert_eq!(gsu.read_program_rom_byte(&rom, 0x70, 0x1234), Some(0xA5));
        assert_eq!(gsu.read_program_rom_byte(&rom, 0x71, 0x1234), Some(0x5A));
    }

    #[test]
    fn read_program_rom_byte_wraps_32k_rom_banks_through_rom_size() {
        let mut gsu = SuperFx::new(0x20_0000);
        let mut rom = vec![0u8; 0x10_000];
        rom[0x0000] = 0x5A;
        rom[0x8000] = 0xA5;

        assert_eq!(gsu.read_program_rom_byte(&rom, 0x00, 0x8000), Some(0x5A));
        assert_eq!(gsu.read_program_rom_byte(&rom, 0x01, 0x8000), Some(0xA5));
        assert_eq!(gsu.read_program_rom_byte(&rom, 0x02, 0x8000), Some(0x5A));
    }

    #[test]
    fn default_steps_per_cpu_cycle_tracks_clsr_speed_mode() {
        // CLSR bit 0: 0 = standard 10.738 MHz (SLOW), 1 = turbo 21.477 MHz (FAST)
        let standard = SuperFx::new(0x20_0000); // clsr=0 → standard speed
        let mut turbo = SuperFx::new(0x20_0000);
        turbo.clsr = 0x01; // clsr=1 → turbo speed

        assert_eq!(
            standard.steps_per_cpu_cycle(),
            super::DEFAULT_SUPERFX_RATIO_SLOW
        );
        assert_eq!(
            turbo.steps_per_cpu_cycle(),
            super::DEFAULT_SUPERFX_RATIO_FAST
        );
    }

    #[test]
    fn superfx_cpu_ratio_env_overrides_default_speed() {
        let _guard = env_lock().lock().unwrap();
        let prev = std::env::var("SUPERFX_CPU_RATIO").ok();
        std::env::set_var("SUPERFX_CPU_RATIO", "7");

        let mut gsu = SuperFx::new(0x20_0000);
        gsu.clsr = 0x01;
        assert_eq!(gsu.steps_per_cpu_cycle(), 7);

        if let Some(value) = prev {
            std::env::set_var("SUPERFX_CPU_RATIO", value);
        } else {
            std::env::remove_var("SUPERFX_CPU_RATIO");
        }
    }

    #[test]
    fn superfx_status_poll_boost_env_overrides_default_value() {
        let _guard = env_lock().lock().unwrap();
        let prev = std::env::var("SUPERFX_STATUS_POLL_BOOST").ok();
        std::env::set_var("SUPERFX_STATUS_POLL_BOOST", "96");

        assert_eq!(SuperFx::status_poll_step_budget(), 96);

        if let Some(value) = prev {
            std::env::set_var("SUPERFX_STATUS_POLL_BOOST", value);
        } else {
            std::env::remove_var("SUPERFX_STATUS_POLL_BOOST");
        }
    }

    #[test]
    fn debug_in_starfox_cached_delay_loop_matches_expected_signature() {
        let mut gsu = SuperFx::new(0x20_0000);
        gsu.running = true;
        gsu.cache_enabled = true;
        gsu.pbr = 0x01;
        gsu.cbr = 0x84F0;
        gsu.regs[0] = 0x8EBC;
        gsu.regs[11] = 0x8615;
        gsu.regs[13] = 0x000B;
        gsu.regs[15] = 0x000C;

        assert!(gsu.debug_in_starfox_cached_delay_loop());

        gsu.regs[11] = 0x8609;
        assert!(gsu.debug_in_starfox_cached_delay_loop());

        gsu.regs[11] = 0x8614;
        assert!(!gsu.debug_in_starfox_cached_delay_loop());

        gsu.regs[11] = 0x8608;
        assert!(!gsu.debug_in_starfox_cached_delay_loop());
    }

    #[test]
    fn debug_in_starfox_cached_delay_loop_ignores_r0_data_value() {
        let mut gsu = SuperFx::new(0x20_0000);
        gsu.running = true;
        gsu.cache_enabled = true;
        gsu.pbr = 0x01;
        gsu.cbr = 0x84F0;
        gsu.regs[0] = 0x0000;
        gsu.regs[11] = 0x8615;
        gsu.regs[13] = 0x000B;
        gsu.regs[15] = 0x000B;
        assert!(gsu.debug_in_starfox_cached_delay_loop());

        gsu.regs[0] = 0x8EBC;
        assert!(gsu.debug_in_starfox_cached_delay_loop());
    }

    #[test]
    fn fast_forward_starfox_cached_delay_loop_collapses_r12_to_zero() {
        let mut gsu = SuperFx::new(0x20_0000);
        gsu.running = true;
        gsu.cache_enabled = true;
        gsu.pbr = 0x01;
        gsu.cbr = 0x84F0;
        gsu.regs[0] = 0x0000;
        gsu.regs[11] = 0x8615;
        gsu.regs[12] = 0xBC8E;
        gsu.regs[13] = 0x000B;
        gsu.regs[15] = 0x000B;
        gsu.sfr = SFR_S_BIT;

        assert!(gsu.fast_forward_starfox_cached_delay_loop());
        assert_eq!(gsu.regs[12], 0x0000);
        assert_eq!(gsu.regs[15], 0x000C);
        assert!(!gsu.pipe_valid);
        assert!(gsu.sfr & SFR_Z_BIT != 0);
        assert!(gsu.sfr & SFR_S_BIT == 0);
    }

    #[test]
    fn status_poll_late_wait_assist_can_exit_after_cached_delay_loop() {
        let mut gsu = SuperFx::new(0x20_0000);
        let rom = vec![0u8; 0x20_0000];
        gsu.running = true;
        gsu.cache_enabled = true;
        gsu.cache_valid_mask = u32::MAX;
        gsu.pbr = 0x01;
        gsu.cbr = 0x84F0;
        gsu.regs[0] = 0x0000;
        gsu.regs[11] = 0x8615;
        gsu.regs[12] = 0xBC8E;
        gsu.regs[13] = 0x000B;
        gsu.regs[15] = 0x000B;
        gsu.cache_ram[0x000C] = 0x00;

        gsu.run_status_poll_until_stop_with_starfox_late_wait_assist(&rom, 4);

        assert!(!gsu.running);
        assert_eq!(gsu.regs[12], 0x0000);
        assert_eq!(gsu.regs[15], 0x000E);
    }

    #[test]
    fn status_poll_until_sfr_low_mask_changes_stops_after_go_bit_clears() {
        let mut gsu = SuperFx::new(0x20_0000);
        let rom = vec![0u8; 0x20_0000];
        gsu.running = true;
        gsu.cache_enabled = true;
        gsu.cache_valid_mask = u32::MAX;
        gsu.pbr = 0x01;
        gsu.cbr = 0x84F0;
        gsu.regs[0] = 0x0000;
        gsu.regs[11] = 0x8615;
        gsu.regs[12] = 0xBC8E;
        gsu.regs[13] = 0x000B;
        gsu.regs[15] = 0x000B;
        gsu.sfr = SFR_GO_BIT;
        gsu.cache_ram[0x000C] = 0x00;

        let initial_low = gsu.observed_sfr_low();
        assert_ne!(initial_low & (SFR_GO_BIT as u8), 0);

        gsu.run_status_poll_until_sfr_low_mask_changes(&rom, initial_low, SFR_GO_BIT as u8, 4);

        assert!(!gsu.running);
        assert_eq!(gsu.observed_sfr_low() & (SFR_GO_BIT as u8), 0);
        assert_eq!(gsu.regs[15], 0x000E);
    }

    #[test]
    fn starfox_live_producer_wait_assist_can_run_until_stop() {
        let mut gsu = SuperFx::new(0x20_0000);
        let mut rom = vec![0u8; 0x20_0000];
        rom[0x01_B384] = 0x00;
        gsu.running = true;
        gsu.pbr = 0x01;
        gsu.rambr = 0x00;
        gsu.regs[13] = 0xB384;
        gsu.regs[15] = 0xB384;
        gsu.sfr = SFR_GO_BIT;

        gsu.run_status_poll_until_go_clears_in_starfox_live_producer_loop(&rom, 4);

        assert!(!gsu.running);
        assert_eq!(gsu.observed_sfr_low() & (SFR_GO_BIT as u8), 0);
    }

    #[test]
    fn starfox_live_producer_wait_assist_stops_after_leaving_producer_band() {
        let mut gsu = SuperFx::new(0x20_0000);
        let rom = vec![0u8; 0x20_0000];
        gsu.running = true;
        gsu.pbr = 0x01;
        gsu.rambr = 0x00;
        gsu.regs[13] = 0xD1B4;
        gsu.regs[15] = 0xD1B4;
        gsu.sfr = SFR_GO_BIT;

        gsu.run_status_poll_until_go_clears_in_starfox_live_producer_loop(&rom, 8);

        assert!(gsu.running);
        assert_eq!(gsu.regs[13], 0xD1B4);
    }

    #[test]
    fn and_opcode_uses_plain_and_without_alt1() {
        let mut gsu = SuperFx::new(0x20_0000);
        let rom = vec![0u8; 0x20_0000];
        gsu.src_reg = 8;
        gsu.dst_reg = 8;
        gsu.regs[8] = 0x00F3;
        gsu.regs[7] = 0x00CC;

        assert!(gsu.execute_opcode_internal(0x77, &rom, 0x8000, false));
        assert_eq!(gsu.regs[8], 0x00C0);
    }

    #[test]
    fn bic_opcode_is_selected_by_alt1() {
        let mut gsu = SuperFx::new(0x20_0000);
        let rom = vec![0u8; 0x20_0000];
        gsu.src_reg = 8;
        gsu.dst_reg = 8;
        gsu.regs[8] = 0x00F3;
        gsu.regs[7] = 0x00CC;
        gsu.sfr |= SFR_ALT1_BIT;

        assert!(gsu.execute_opcode_internal(0x77, &rom, 0x8000, false));
        assert_eq!(gsu.regs[8], 0x0033);
    }

    #[test]
    fn fmult_writes_upper_product_word_and_sets_carry_from_lower_word() {
        let mut gsu = SuperFx::new(0x20_0000);
        let rom = vec![0u8; 0x20_0000];
        gsu.src_reg = 5;
        gsu.dst_reg = 2;
        gsu.regs[5] = 0x4AAA;
        gsu.regs[6] = 0xDAAB;

        assert!(gsu.execute_opcode_internal(0x9F, &rom, 0x8000, false));
        // FMULT: (product << 1) >> 16. product = 0xF51CA38E
        // (0xF51CA38E << 1) = 0xEA39471C >> 16 = 0xEA39
        // FMULT: product >> 16 (per snes9x/ares)
        assert_eq!(gsu.regs[2], 0xF51C);
        assert_ne!(gsu.sfr & SFR_S_BIT, 0);
        assert_eq!(gsu.sfr & SFR_Z_BIT, 0);
        assert_ne!(gsu.sfr & SFR_CY_BIT, 0);
    }

    #[test]
    fn lmult_writes_upper_product_word_and_keeps_low_word_in_r4() {
        let mut gsu = SuperFx::new(0x20_0000);
        let rom = vec![0u8; 0x20_0000];
        gsu.src_reg = 9;
        gsu.dst_reg = 8;
        gsu.regs[9] = 0xB556;
        gsu.regs[6] = 0xDAAB;
        gsu.sfr |= SFR_ALT1_BIT;

        assert!(gsu.execute_opcode_internal(0x9F, &rom, 0x8000, false));
        assert_eq!(gsu.regs[8], 0x0AE3);
        assert_eq!(gsu.regs[4], 0x5C72);
        assert_eq!(gsu.sfr & SFR_S_BIT, 0);
        assert_eq!(gsu.sfr & SFR_Z_BIT, 0);
        assert_eq!(gsu.sfr & SFR_CY_BIT, 0);
    }

    #[test]
    fn iwt_r15_works_for_ff_opcode() {
        let mut gsu = SuperFx::new(0x20_0000);
        let mut rom = vec![0u8; 0x20_0000];
        rom[0x0000] = 0xFF;
        rom[0x0001] = 0x34;
        rom[0x0002] = 0x92;
        rom[0x0003] = 0x01;
        rom[0x1236] = 0x00;

        gsu.pbr = 0x00;
        gsu.regs[15] = 0x8000;
        gsu.running = true;
        gsu.sfr |= SFR_GO_BIT;
        gsu.run_steps(&rom, 9);

        assert!(!gsu.running());
        assert_eq!(gsu.regs[15], 0x9236);
    }

    #[test]
    fn branch_run_steps_executes_delay_slot_using_target_stream_for_immediate_fetch() {
        let mut gsu = SuperFx::new(0x20_0000);
        let mut rom = vec![0u8; 0x20_0000];
        rom[0x0000] = 0x05; // BRA
        rom[0x0001] = 0x03; // target = 0x8005 after delay-slot prefetch
        rom[0x0002] = 0xAC; // delay slot: IBT R12, #imm
        rom[0x0003] = 0x11; // should be ignored
        rom[0x0005] = 0x22; // target-stream immediate
        rom[0x0006] = 0x00; // STOP at target

        gsu.pbr = 0x00;
        gsu.regs[15] = 0x8000;
        gsu.running = true;
        gsu.sfr |= SFR_GO_BIT;

        gsu.run_steps(&rom, 4);

        assert_eq!(gsu.regs[12], 0x0022);
        assert!(!gsu.running());
        assert_eq!(gsu.regs[15], 0x8008);
    }

    #[test]
    fn jmp_r11_run_steps_executes_delay_slot_using_target_stream_for_immediate_fetch() {
        let mut gsu = SuperFx::new(0x20_0000);
        let mut rom = vec![0u8; 0x20_0000];
        rom[0x0000] = 0x9B; // JMP R11
        rom[0x0001] = 0xAC; // delay slot: IBT R12, #imm
        rom[0x0002] = 0x11; // should be ignored
        rom[0x1234] = 0x22; // target-stream immediate
        rom[0x1235] = 0x00; // STOP at target fallthrough

        gsu.pbr = 0x00;
        gsu.regs[11] = 0x9234;
        gsu.regs[15] = 0x8000;
        gsu.running = true;
        gsu.sfr |= SFR_GO_BIT;

        gsu.run_steps(&rom, 4);

        assert_eq!(gsu.regs[12], 0x0022);
        assert!(!gsu.running());
        assert_eq!(
            gsu.debug_recent_pc_transfers()
                .last()
                .map(|t| (t.opcode, t.from_pc, t.to_pc)),
            Some((0x9B, 0x8000, 0x9234))
        );
    }

    #[test]
    fn iwt_r15_run_steps_executes_delay_slot_before_transfer() {
        let mut gsu = SuperFx::new(0x20_0000);
        let mut rom = vec![0u8; 0x20_0000];
        rom[0x0000] = 0xFF; // IWT R15, $9234
        rom[0x0001] = 0x34;
        rom[0x0002] = 0x92;
        rom[0x0003] = 0xD0; // delay slot: INC R0
        rom[0x1234] = 0x00; // STOP at target

        gsu.pbr = 0x00;
        gsu.regs[15] = 0x8000;
        gsu.running = true;
        gsu.sfr |= SFR_GO_BIT;

        gsu.run_steps(&rom, 4);

        assert_eq!(gsu.regs[0], 0x0001);
        assert!(!gsu.running());
        assert_eq!(gsu.regs[15], 0x9236);
    }

    #[test]
    fn with_r15_move_to_r8_uses_logical_execution_time_r15() {
        let mut gsu = SuperFx::new(0x20_0000);
        let mut rom = vec![0u8; 0x20_0000];
        rom[0x0000] = 0x2F; // WITH R15
        rom[0x0001] = 0x18; // MOVE R8, R15
        rom[0x0002] = 0x00; // STOP

        gsu.pbr = 0x00;
        gsu.regs[15] = 0x8000;
        gsu.running = true;
        gsu.sfr |= SFR_GO_BIT;

        gsu.run_steps(&rom, 4);

        // snes9x FETCHPIPE executes MOVE with R15 pointing at the next byte
        // already in the pipe, not one byte past it.
        assert_eq!(gsu.regs[8], 0x8002);
        assert!(!gsu.running());
    }

    #[test]
    fn to_b_form_resets_selectors_before_following_opcode() {
        let mut gsu = SuperFx::new(0x20_0000);
        let mut rom = vec![0u8; 0x20_0000];
        rom[0x0000] = 0x27; // WITH R7
        rom[0x0001] = 0x1D; // TO R13 (B-form copy from R7)
        rom[0x0002] = 0x69; // SUB R9 -> must fall back to default R0 after CLRFLAGS
        rom[0x0003] = 0x00; // STOP

        gsu.pbr = 0x00;
        gsu.regs[15] = 0x8000;
        gsu.regs[0] = 0x1234;
        gsu.regs[7] = 0x0003;
        gsu.regs[9] = 0x0001;
        gsu.running = true;
        gsu.sfr |= SFR_GO_BIT;

        gsu.run_steps(&rom, 4);

        assert_eq!(gsu.regs[13], 0x0003);
        assert_eq!(gsu.regs[7], 0x0003);
        assert_eq!(gsu.regs[0], 0x1233);
        assert!(!gsu.running());
    }

    #[test]
    fn to_r15_getb_run_steps_switches_immediately_to_target_stream() {
        let mut gsu = SuperFx::new(0x20_0000);
        let mut rom = vec![0u8; 0x20_0000];
        rom[0x0000] = 0x1F; // TO R15
        rom[0x0001] = 0xEF; // GETB -> R15
        rom[0x0002] = 0xD0; // one stale old-stream byte still executes
        rom[0x0003] = 0xD1; // but execution must not skip ahead to this byte
        rom[0x0004] = 0x05; // ROM data byte read by GETB
        rom[0x0005] = 0x00; // target stream STOP
        rom[0x0006] = 0x01; // target+1: NOP

        gsu.pbr = 0x00;
        gsu.rombr = 0x00;
        gsu.regs[14] = 0x8004;
        gsu.rom_buffer_pending = true;
        gsu.rom_buffer_valid = false;
        gsu.rom_buffer_pending_bank = 0x00;
        gsu.rom_buffer_pending_addr = 0x8004;
        gsu.cbr = 0x8000;
        gsu.cache_enabled = true;
        gsu.cache_valid_mask = u32::MAX;
        gsu.cache_ram[0x0000] = 0x1F; // TO R15
        gsu.cache_ram[0x0001] = 0xEF; // GETB -> R15
        gsu.cache_ram[0x0002] = 0xD0; // stale old-stream byte still executes
        gsu.cache_ram[0x0003] = 0xD1; // but execution must not advance to this byte
        gsu.cache_ram[0x0005] = 0x00; // STOP at target stream
        gsu.cache_ram[0x0006] = 0x01; // NOP after STOP target
        gsu.regs[15] = 0x8000;
        gsu.running = true;
        gsu.sfr |= SFR_GO_BIT;

        gsu.run_steps(&rom, 4);

        // FX_STEP fetches one sequential byte before GETB writes R15, so the
        // immediate next old-stream byte executes once. Execution must then
        // resume from the target stream, not from the second old-stream byte.
        assert_eq!(gsu.regs[0], 0x0001);
        assert_eq!(gsu.regs[1], 0x0000);
        assert!(!gsu.running());
        assert_eq!(gsu.regs[15], 0x0007);
    }

    #[test]
    fn run_steps_keeps_gsu_running_when_slice_budget_is_exhausted() {
        let mut gsu = SuperFx::new(0x20_0000);
        let mut rom = vec![0u8; 0x20_0000];
        rom[0x0000] = 0xD0;
        rom[0x0001] = 0xD0;
        rom[0x0002] = 0x00;

        gsu.pbr = 0x00;
        gsu.regs[15] = 0x8000;
        gsu.running = true;
        gsu.sfr |= SFR_GO_BIT;

        gsu.run_steps(&rom, 2);
        assert!(gsu.running());
        assert_ne!(gsu.read_register(0x3030, 0) & (SFR_GO_BIT as u8), 0);
        assert_eq!(gsu.regs[15], 0x8003);

        gsu.run_steps(&rom, 2);
        assert!(!gsu.running());
        assert_eq!(gsu.read_register(0x3030, 0) & (SFR_GO_BIT as u8), 0);
    }

    #[test]
    fn loop_decrements_full_r12_and_branches_when_nonzero() {
        let mut gsu = SuperFx::new(0x20_0000);
        let mut rom = vec![0u8; 0x20_0000];
        rom[0x0001] = 0x01;
        gsu.regs[12] = 0xFF02;
        gsu.regs[13] = 0x1234;
        gsu.regs[15] = 0x8001;

        assert!(gsu.execute_opcode_internal(0x3C, &rom, 0x0000, false));
        assert_eq!(gsu.regs[12], 0xFF01);
        assert_eq!(gsu.regs[15], 0x1234);
    }

    #[test]
    fn loop_stops_when_r12_reaches_zero() {
        let mut gsu = SuperFx::new(0x20_0000);
        let mut rom = vec![0u8; 0x20_0000];
        rom[0x0001] = 0x01; // delay slot NOP
        gsu.regs[12] = 0x0001;
        gsu.regs[13] = 0x1234;
        gsu.regs[15] = 0x8001;

        assert!(gsu.execute_opcode_internal(0x3C, &rom, 0x0000, false));
        assert_eq!(gsu.regs[12], 0x0000);
        assert_eq!(gsu.regs[15], 0x8001);
        assert_ne!(gsu.sfr & SFR_Z_BIT, 0);
    }

    #[test]
    fn loop_clears_prefix_flags() {
        let mut gsu = SuperFx::new(0x20_0000);
        let rom = vec![0u8; 0x20_0000];
        gsu.regs[12] = 0x0002;
        gsu.regs[13] = 0x1234;
        gsu.regs[15] = 0x8001;
        gsu.sfr |= super::SFR_ALT1_BIT | super::SFR_B_BIT;

        assert!(gsu.execute_opcode_internal(0x3C, &rom, 0x0000, false));
        assert_eq!(gsu.regs[15], 0x1234);
        assert_eq!(gsu.sfr & super::SFR_ALT1_BIT, 0);
        assert_eq!(gsu.sfr & super::SFR_B_BIT, 0);
    }

    #[test]
    fn loop_run_steps_executes_prefetched_delay_slot_when_branching() {
        let mut gsu = SuperFx::new(0x20_0000);
        let mut rom = vec![0u8; 0x20_0000];
        rom[0x0000] = 0x3C; // LOOP
        rom[0x0001] = 0xD0; // delay slot: INC R0
        rom[0x0002] = 0x00; // STOP

        gsu.pbr = 0x00;
        gsu.regs[12] = 0x0002;
        gsu.regs[13] = 0x8000;
        gsu.regs[15] = 0x8000;
        gsu.running = true;
        gsu.sfr |= SFR_GO_BIT;

        gsu.run_steps(&rom, 2);

        assert_eq!(gsu.regs[0], 0x0001);
        assert_eq!(gsu.regs[12], 0x0001);
        assert!(gsu.running());
        assert_eq!(gsu.regs[15], 0x8001);
    }

    #[test]
    fn branch_taken_uses_opcode_pc_plus_one_as_base() {
        let mut gsu = SuperFx::new(0x20_0000);
        let mut rom = vec![0u8; 0x20_0000];
        rom[0x0001] = 0x01;
        gsu.regs[15] = 0x8001;

        assert!(gsu.execute_opcode_internal(0x05, &rom, 0x8000, false));
        assert_eq!(gsu.regs[15], 0x8003);
    }

    #[test]
    fn branch_preserves_with_and_alt_prefix_state() {
        let mut gsu = SuperFx::new(0x20_0000);
        let mut rom = vec![0u8; 0x20_0000];
        rom[0x0001] = 0x01;
        gsu.regs[15] = 0x8001;
        gsu.src_reg = 6;
        gsu.dst_reg = 7;
        gsu.with_reg = 5;
        gsu.sfr |= SFR_B_BIT | SFR_ALT1_BIT | super::SFR_ALT2_BIT;

        assert!(gsu.execute_opcode_internal(0x05, &rom, 0x8000, false));
        assert_eq!(gsu.regs[15], 0x8003);
        assert_eq!(gsu.src_reg, 6);
        assert_eq!(gsu.dst_reg, 7);
        assert_eq!(gsu.with_reg, 5);
        assert_ne!(gsu.sfr & SFR_B_BIT, 0);
        assert_ne!(gsu.sfr & SFR_ALT1_BIT, 0);
        assert_ne!(gsu.sfr & super::SFR_ALT2_BIT, 0);
    }

    #[test]
    fn blt_takes_branch_when_sign_and_overflow_match() {
        let mut gsu = SuperFx::new(0x20_0000);
        let mut rom = vec![0u8; 0x20_0000];
        rom[0x0001] = 0x01;
        gsu.regs[15] = 0x8001;
        gsu.sfr = SFR_S_BIT | SFR_OV_BIT;
        gsu.sync_condition_flags_from_sfr();

        assert!(gsu.execute_opcode_internal(0x06, &rom, 0x8000, false));
        assert_eq!(gsu.regs[15], 0x8003);
    }

    #[test]
    fn bge_takes_branch_when_sign_and_overflow_differ() {
        let mut gsu = SuperFx::new(0x20_0000);
        let mut rom = vec![0u8; 0x20_0000];
        rom[0x0001] = 0x01;
        gsu.regs[15] = 0x8001;
        gsu.sfr = SFR_S_BIT;
        gsu.sync_condition_flags_from_sfr();

        assert!(gsu.execute_opcode_internal(0x07, &rom, 0x8000, false));
        assert_eq!(gsu.regs[15], 0x8003);
    }

    #[test]
    fn branch_does_not_execute_delay_slot() {
        let mut gsu = SuperFx::new(0x20_0000);
        let mut rom = vec![0u8; 0x20_0000];
        rom[0x0001] = 0x01;
        rom[0x0002] = 0xD0;
        gsu.regs[0] = 0x1234;
        gsu.regs[15] = 0x8001;

        assert!(gsu.execute_opcode_internal(0x05, &rom, 0x8000, false));
        assert_eq!(gsu.regs[0], 0x1234);
        assert_eq!(gsu.regs[15], 0x8003);
    }

    #[test]
    fn branch_taken_preserves_with_prefix_for_target_instruction() {
        let mut gsu = SuperFx::new(0x20_0000);
        let mut rom = vec![0u8; 0x20_0000];
        gsu.regs[0] = 0x1234;
        gsu.regs[2] = 0xABCD;
        gsu.src_reg = 0;
        gsu.dst_reg = 0;
        gsu.sfr |= SFR_B_BIT;
        gsu.regs[15] = 0x8001;
        rom[0x0001] = 0x01; // BRA +1

        assert!(gsu.execute_opcode_internal(0x05, &rom, 0x8000, false));
        assert_ne!(gsu.sfr & SFR_B_BIT, 0);

        assert!(gsu.execute_opcode_internal(0x12, &rom, 0x8002, false));

        assert_eq!(gsu.regs[2], 0x1234);
        assert_eq!(gsu.sfr & SFR_B_BIT, 0);
    }

    #[test]
    fn branch_not_taken_preserves_with_prefix_for_fallthrough_instruction() {
        let mut gsu = SuperFx::new(0x20_0000);
        let mut rom = vec![0u8; 0x20_0000];
        gsu.regs[0] = 0x1234;
        gsu.regs[2] = 0xABCD;
        gsu.src_reg = 0;
        gsu.dst_reg = 0;
        gsu.sfr |= SFR_B_BIT;
        gsu.regs[15] = 0x8001;
        rom[0x0001] = 0x00; // BEQ +0 (not taken because Z is clear)

        assert!(gsu.execute_opcode_internal(0x09, &rom, 0x8000, false));
        assert_ne!(gsu.sfr & SFR_B_BIT, 0);

        assert!(gsu.execute_opcode_internal(0x12, &rom, 0x8002, false));

        assert_eq!(gsu.regs[2], 0x1234);
        assert_eq!(gsu.sfr & SFR_B_BIT, 0);
    }

    #[test]
    fn branch_taken_preserves_alt_prefix_for_target_instruction() {
        let mut gsu = SuperFx::new(0x20_0000);
        let mut rom = vec![0u8; 0x20_0000];
        gsu.sfr |= SFR_ALT1_BIT;
        gsu.regs[15] = 0x8001;
        rom[0x0001] = 0x01; // BRA +1
        rom[0x0003] = 0x12; // immediate for IBT
        gsu.write_ram_word_short(0x24, 0xBEEF);

        assert!(gsu.execute_opcode_internal(0x05, &rom, 0x8000, false));
        assert_eq!(gsu.alt_mode(), 1);

        gsu.regs[15] = 0x8003;
        assert!(gsu.execute_opcode_internal(0xA0, &rom, 0x8002, false));

        assert_eq!(gsu.regs[0], 0xBEEF);
    }

    #[test]
    fn rol_uses_carry_in() {
        let mut gsu = SuperFx::new(0x20_0000);
        let rom = vec![0u8; 0x20_0000];
        gsu.src_reg = 4;
        gsu.dst_reg = 4;
        gsu.regs[4] = 0x0160;
        gsu.sfr |= SFR_CY_BIT;
        gsu.sync_condition_flags_from_sfr();

        assert!(gsu.execute_opcode_internal(0x04, &rom, 0x0000, false));
        assert_eq!(gsu.regs[4], 0x02C1);
    }

    #[test]
    fn div2_alt1_turns_negative_one_into_zero() {
        let mut gsu = SuperFx::new(0x20_0000);
        let rom = vec![0u8; 0x20_0000];
        gsu.src_reg = 4;
        gsu.dst_reg = 4;
        gsu.regs[4] = 0xFFFF;
        gsu.sfr |= SFR_ALT1_BIT;

        assert!(gsu.execute_opcode_internal(0x96, &rom, 0x0000, false));
        assert_eq!(gsu.regs[4], 0x0000);
        assert_ne!(gsu.sfr & SFR_Z_BIT, 0);
    }

    #[test]
    fn sub_carry_uses_unsigned_16bit_diff_rule() {
        let mut gsu = SuperFx::new(0x20_0000);
        let rom = vec![0u8; 0x20_0000];
        gsu.src_reg = 4;
        gsu.dst_reg = 4;
        gsu.regs[4] = 0x8000;
        gsu.regs[7] = 0x0001;

        assert!(gsu.execute_opcode_internal(0x67, &rom, 0x0000, false));
        assert_eq!(gsu.regs[4], 0x7FFF);
        assert_ne!(gsu.sfr & SFR_CY_BIT, 0);
    }

    #[test]
    fn link_four_sets_r11_to_return_after_delayed_jump_sequence() {
        let mut gsu = SuperFx::new(0x20_0000);
        gsu.regs[15] = 0xB33B;

        assert!(gsu.execute_opcode(0x94, &[], 0xB33A));
        assert_eq!(gsu.regs[11], 0xB33F);
    }

    #[test]
    fn jmp_9b_targets_r11() {
        let mut gsu = SuperFx::new(0x20_0000);
        let mut rom = vec![0u8; 0x20_0000];
        rom[0x0001] = 0x01;
        gsu.regs[11] = 0x3456;
        gsu.regs[9] = 0x1234;
        gsu.regs[15] = 0x8001;

        assert!(gsu.execute_opcode_internal(0x9B, &rom, 0x8000, false));
        assert_eq!(gsu.regs[15], 0x3456);
    }

    #[test]
    fn iwt_r15_records_pc_transfer_history() {
        let mut gsu = SuperFx::new(0x20_0000);
        let mut rom = vec![0u8; 0x20_0000];
        rom[0x0000] = 0x34;
        rom[0x0001] = 0x92;
        gsu.regs[15] = 0x8000;

        assert!(gsu.execute_opcode_internal(0xFF, &rom, 0x8000, false));
        let transfer = gsu.debug_recent_pc_transfers().last().unwrap();
        assert_eq!(transfer.opcode, 0xFF);
        assert_eq!(transfer.from_pc, 0x8000);
        assert_eq!(transfer.to_pc, 0x9234);
    }

    #[test]
    fn jmp_9b_records_pc_transfer_history() {
        let mut gsu = SuperFx::new(0x20_0000);
        let mut rom = vec![0u8; 0x20_0000];
        rom[0x0001] = 0x01;
        gsu.regs[11] = 0x3456;
        gsu.regs[15] = 0x8001;

        assert!(gsu.execute_opcode_internal(0x9B, &rom, 0x8000, false));
        let transfer = gsu.debug_recent_pc_transfers().last().unwrap();
        assert_eq!(transfer.opcode, 0x9B);
        assert_eq!(transfer.from_pc, 0x8000);
        assert_eq!(transfer.to_pc, 0x3456);
    }

    #[test]
    fn reg15_read_uses_written_value_when_modified_under_pipe() {
        let mut gsu = SuperFx::new(0x20_0000);
        gsu.running = true;
        gsu.pipe_valid = true;
        gsu.regs[15] = 0x8002;
        gsu.r15_modified = false;
        assert_eq!(gsu.reg(15), 0x8002);

        gsu.write_reg(15, 0x9234);
        assert_eq!(gsu.reg(15), 0x9234);
    }

    #[test]
    fn sm_uses_opcode_register_not_source_selector() {
        let mut gsu = SuperFx::new(0x20_0000);
        let mut rom = vec![0u8; 0x20_0000];
        rom[0x0002] = 0x12;

        gsu.src_reg = 1;
        gsu.regs[1] = 0x1111;
        gsu.regs[6] = 0xBEEF;
        gsu.sfr |= super::SFR_ALT2_BIT;
        gsu.pipe = 0x34;
        gsu.pipe_valid = true;
        gsu.running = true;
        gsu.regs[15] = 0x8001;
        gsu.cache_enabled = false;

        assert!(gsu.execute_opcode_internal(0xF6, &rom, 0x8000, false));
        assert_eq!(gsu.read_ram_word(0x1234), 0xBEEF);
    }

    #[test]
    fn alt3_f6_uses_lm_not_iwt() {
        let mut gsu = SuperFx::new(0x20_0000);
        let mut rom = vec![0u8; 0x20_0000];
        rom[0x0002] = 0x12;
        gsu.write_ram_word(0x1234, 0xCAFE);
        gsu.sfr |= super::SFR_ALT1_BIT | super::SFR_ALT2_BIT;
        gsu.pipe = 0x34;
        gsu.pipe_valid = true;
        gsu.running = true;
        gsu.regs[15] = 0x8001;
        gsu.cache_enabled = false;

        assert!(gsu.execute_opcode_internal(0xF6, &rom, 0x8000, false));
        assert_eq!(gsu.regs[6], 0xCAFE);
    }

    #[test]
    fn alt3_96_uses_div2() {
        let mut gsu = SuperFx::new(0x20_0000);
        let rom = vec![0u8; 0x20_0000];
        gsu.src_reg = 1;
        gsu.dst_reg = 2;
        gsu.regs[1] = 0xFFFF;
        gsu.sfr |= super::SFR_ALT1_BIT | super::SFR_ALT2_BIT;

        assert!(gsu.execute_opcode_internal(0x96, &rom, 0x8000, false));
        assert_eq!(gsu.regs[2], 0x0000);
    }

    #[test]
    fn alt3_9b_uses_ljmp() {
        let mut gsu = SuperFx::new(0x20_0000);
        let rom = vec![0u8; 0x20_0000];
        gsu.src_reg = 1;
        gsu.regs[1] = 0x9234;
        gsu.regs[11] = 0x0005;
        gsu.sfr |= super::SFR_ALT1_BIT | super::SFR_ALT2_BIT;

        assert!(gsu.execute_opcode_internal(0x9B, &rom, 0x8000, false));
        assert_eq!(gsu.pbr, 0x05);
        assert_eq!(gsu.regs[15], 0x9234);
    }

    #[test]
    fn alt3_9f_uses_lmult() {
        let mut gsu = SuperFx::new(0x20_0000);
        let rom = vec![0u8; 0x20_0000];
        gsu.src_reg = 9;
        gsu.dst_reg = 8;
        gsu.regs[9] = 0xB556;
        gsu.regs[6] = 0xDAAB;
        gsu.sfr |= super::SFR_ALT1_BIT | super::SFR_ALT2_BIT;

        assert!(gsu.execute_opcode_internal(0x9F, &rom, 0x8000, false));
        assert_eq!(gsu.regs[8], 0x0AE3);
        assert_eq!(gsu.regs[4], 0x5C72);
    }

    #[test]
    fn stw_uses_current_source_register_value() {
        let mut gsu = SuperFx::new(0x20_0000);
        gsu.src_reg = 13;
        gsu.regs[13] = 0x1234;
        gsu.regs[1] = 0x0010;

        assert!(gsu.execute_opcode(0x31, &[], 0x8000));
        assert_eq!(gsu.game_ram[0x0010], 0x34);
        assert_eq!(gsu.game_ram[0x0011], 0x12);
    }

    #[test]
    fn stb_stores_low_byte_of_source_register() {
        let mut gsu = SuperFx::new(0x20_0000);
        gsu.src_reg = 5;
        gsu.regs[5] = 0xABCD;
        gsu.regs[1] = 0x0020;
        gsu.sfr |= super::SFR_ALT1_BIT;

        assert!(gsu.execute_opcode(0x31, &[], 0x8000));
        assert_eq!(gsu.game_ram[0x0020], 0xCD);
        assert_eq!(gsu.game_ram[0x0021], 0x00);
    }

    #[test]
    fn ldw_loads_word_into_destination_register() {
        let mut gsu = SuperFx::new(0x20_0000);
        gsu.dst_reg = 7;
        gsu.regs[1] = 0x0010;
        gsu.game_ram[0x0010] = 0x78;
        gsu.game_ram[0x0011] = 0x56;

        assert!(gsu.execute_opcode(0x41, &[], 0x8000));
        assert_eq!(gsu.regs[7], 0x5678);
    }

    #[test]
    fn ldb_zero_extends_byte_into_destination_register() {
        let mut gsu = SuperFx::new(0x20_0000);
        gsu.dst_reg = 4;
        gsu.regs[1] = 0x0030;
        gsu.game_ram[0x0030] = 0x9A;
        gsu.sfr |= super::SFR_ALT1_BIT;

        assert!(gsu.execute_opcode(0x41, &[], 0x8000));
        assert_eq!(gsu.regs[4], 0x009A);
    }

    #[test]
    fn cmp_updates_sign_and_zero_flags_without_writing_destination() {
        let mut gsu = SuperFx::new(0x20_0000);
        gsu.src_reg = 9;
        gsu.dst_reg = 9;
        gsu.regs[9] = 0x0016;
        gsu.regs[1] = 0x0029;
        gsu.sfr |= super::SFR_ALT1_BIT | super::SFR_ALT2_BIT;

        assert!(gsu.execute_opcode(0x61, &[], 0x8000));
        assert_eq!(gsu.regs[9], 0x0016);
        assert_eq!(gsu.sfr & super::SFR_Z_BIT, 0);
        assert_ne!(gsu.sfr & super::SFR_S_BIT, 0);
        assert_eq!(gsu.sfr & super::SFR_CY_BIT, 0);
    }

    #[test]
    fn with_then_to_performs_move() {
        let mut gsu = SuperFx::new(0x20_0000);
        gsu.regs[3] = 0x1357;
        gsu.src_reg = 9;
        gsu.dst_reg = 1;

        assert!(gsu.execute_opcode(0x23, &[], 0x8000));
        assert_eq!(gsu.debug_src_reg(), 3);
        assert_eq!(gsu.debug_dst_reg(), 3);
        assert!(gsu.execute_opcode(0x11, &[], 0x8001));
        assert_eq!(gsu.regs[3], 0x1357);
        assert_eq!(gsu.regs[1], 0x1357);
        assert_eq!(gsu.debug_src_reg(), 0);
        assert_eq!(gsu.debug_dst_reg(), 0);
    }

    #[test]
    fn with_then_from_performs_move_without_touching_flags() {
        let mut gsu = SuperFx::new(0x20_0000);
        gsu.regs[2] = 0x0000;
        gsu.dst_reg = 1;
        gsu.sfr |= super::SFR_S_BIT;

        assert!(gsu.execute_opcode(0x21, &[], 0x8000));
        assert!(gsu.execute_opcode(0xB2, &[], 0x8001));
        assert_eq!(gsu.regs[1], 0x0000);
        assert_eq!(gsu.debug_src_reg(), 0);
        assert_eq!(gsu.debug_dst_reg(), 0);
        // MOVES updates sign/zero flags based on the copied value (0 → Z set, S clear)
        assert_ne!(gsu.sfr & super::SFR_Z_BIT, 0);
        assert_eq!(gsu.sfr & super::SFR_S_BIT, 0);
    }

    #[test]
    fn moves_sets_overflow_from_low_byte_bit7() {
        let mut gsu = SuperFx::new(0x20_0000);
        gsu.dst_reg = 1;
        gsu.regs[2] = 0x0080;
        gsu.sfr |= super::SFR_B_BIT;

        assert!(gsu.execute_opcode(0xB2, &[], 0x8000));
        assert_eq!(gsu.regs[1], 0x0080);
        assert_ne!(gsu.sfr & super::SFR_OV_BIT, 0);
        assert_eq!(gsu.sfr & super::SFR_S_BIT, 0);
        assert_eq!(gsu.sfr & super::SFR_Z_BIT, 0);
    }

    #[test]
    fn moves_zero_flag_tracks_full_word_value() {
        let mut gsu = SuperFx::new(0x20_0000);
        gsu.dst_reg = 1;
        gsu.regs[2] = 0x0200;
        gsu.sfr |= super::SFR_B_BIT;

        assert!(gsu.execute_opcode(0xB2, &[], 0x8000));
        assert_eq!(gsu.regs[1], 0x0200);
        assert_eq!(gsu.sfr & super::SFR_Z_BIT, 0);
        assert_eq!(gsu.sfr & super::SFR_S_BIT, 0);
    }

    #[test]
    fn with_immediately_updates_source_and_destination_registers() {
        let mut gsu = SuperFx::new(0x20_0000);
        gsu.src_reg = 5;
        gsu.dst_reg = 6;
        gsu.regs[3] = 0x0001;
        gsu.sfr |= super::SFR_CY_BIT;
        gsu.sync_condition_flags_from_sfr();

        assert!(gsu.execute_opcode(0x23, &[], 0x8000));
        assert_eq!(gsu.debug_src_reg(), 3);
        assert_eq!(gsu.debug_dst_reg(), 3);
        assert!(gsu.execute_opcode(0x04, &[], 0x8001));

        assert_eq!(gsu.regs[3], 0x0003);
        assert_eq!(gsu.regs[6], 0x0000);
        assert_eq!(gsu.debug_src_reg(), 0);
        assert_eq!(gsu.debug_dst_reg(), 0);
        assert_eq!(gsu.sfr & super::SFR_B_BIT, 0);
    }

    #[test]
    fn hib_uses_high_byte_for_sign_and_zero_flags() {
        let mut gsu = SuperFx::new(0x20_0000);
        gsu.src_reg = 4;
        gsu.dst_reg = 5;
        gsu.regs[4] = 0x8001;

        assert!(gsu.execute_opcode(0xC0, &[], 0x8000));
        assert_eq!(gsu.regs[5], 0x0080);
        assert_ne!(gsu.sfr & super::SFR_S_BIT, 0);
        assert_eq!(gsu.sfr & super::SFR_Z_BIT, 0);
    }

    #[test]
    fn lob_uses_low_byte_for_sign_and_zero_flags() {
        let mut gsu = SuperFx::new(0x20_0000);
        gsu.src_reg = 4;
        gsu.dst_reg = 5;
        gsu.regs[4] = 0x0080;

        assert!(gsu.execute_opcode(0x9E, &[], 0x8000));
        assert_eq!(gsu.regs[5], 0x0080);
        assert_ne!(gsu.sfr & super::SFR_S_BIT, 0);
        assert_eq!(gsu.sfr & super::SFR_Z_BIT, 0);
    }

    #[test]
    fn with_mode_consumes_selectors_then_resets_them_to_r0() {
        let mut gsu = SuperFx::new(0x20_0000);
        gsu.regs[3] = 0x0012;
        gsu.regs[0] = 0x0004;

        assert!(gsu.execute_opcode(0x23, &[], 0x8000));
        assert!(gsu.execute_opcode(0xD3, &[], 0x8001));
        assert!(gsu.execute_opcode(0x12, &[], 0x8002));
        assert!(gsu.execute_opcode(0x03, &[], 0x8003));

        assert_eq!(gsu.regs[3], 0x0013);
        assert_eq!(gsu.regs[2], 0x0002);
        assert_eq!(gsu.regs[0], 0x0004);
        assert_eq!(gsu.debug_src_reg(), 0);
        assert_eq!(gsu.debug_dst_reg(), 0);
        assert_eq!(gsu.sfr & super::SFR_B_BIT, 0);
    }

    #[test]
    fn to_and_from_prefixes_apply_to_next_opcode_then_reset_to_r0() {
        let mut gsu = SuperFx::new(0x20_0000);
        gsu.regs[0] = 0x0002;
        gsu.regs[1] = 0x0002;

        assert!(gsu.execute_opcode(0x11, &[], 0x8000));
        assert!(gsu.execute_opcode(0x03, &[], 0x8001));
        assert_eq!(gsu.regs[1], 0x0001);
        assert_eq!(gsu.debug_dst_reg(), 0);

        gsu.regs[1] = 0x0006;
        gsu.regs[0] = 0x0008;
        assert!(gsu.execute_opcode(0xB1, &[], 0x8002));
        assert!(gsu.execute_opcode(0x03, &[], 0x8003));
        assert_eq!(gsu.regs[0], 0x0003);
        assert_eq!(gsu.regs[1], 0x0006);
        assert_eq!(gsu.debug_src_reg(), 0);
        assert_eq!(gsu.debug_dst_reg(), 0);
    }

    #[test]
    fn to_r15_prefix_does_not_skip_past_the_next_prefetched_byte() {
        let mut gsu = SuperFx::new(0x20_0000);
        gsu.regs[15] = 0x8001;

        assert!(gsu.execute_opcode(0x1F, &[], 0x8000));
        assert_eq!(gsu.debug_dst_reg(), 15);
        assert_eq!(gsu.regs[15], 0x8001);
    }

    #[test]
    fn alt_prefixes_clear_b_but_keep_register_selectors() {
        let mut gsu = SuperFx::new(0x20_0000);

        assert!(gsu.execute_opcode(0x26, &[], 0x8000));
        assert_ne!(gsu.sfr & super::SFR_B_BIT, 0);
        assert_eq!(gsu.debug_src_reg(), 6);
        assert_eq!(gsu.debug_dst_reg(), 6);

        assert!(gsu.execute_opcode(0x3D, &[], 0x8001));
        assert_eq!(gsu.sfr & super::SFR_B_BIT, 0);
        assert_ne!(gsu.sfr & super::SFR_ALT1_BIT, 0);
        assert_eq!(gsu.debug_src_reg(), 6);
        assert_eq!(gsu.debug_dst_reg(), 6);
    }

    #[test]
    fn alt1_preserves_existing_alt2_bit() {
        let mut gsu = SuperFx::new(0x20_0000);
        gsu.sfr |= super::SFR_ALT2_BIT | super::SFR_B_BIT;

        assert!(gsu.execute_opcode(0x3D, &[], 0x8000));
        assert_eq!(gsu.sfr & super::SFR_B_BIT, 0);
        assert_ne!(gsu.sfr & super::SFR_ALT1_BIT, 0);
        assert_ne!(gsu.sfr & super::SFR_ALT2_BIT, 0);
        assert_eq!(gsu.alt_mode(), 3);
    }

    #[test]
    fn alt2_preserves_existing_alt1_bit() {
        let mut gsu = SuperFx::new(0x20_0000);
        gsu.sfr |= super::SFR_ALT1_BIT | super::SFR_B_BIT;

        assert!(gsu.execute_opcode(0x3E, &[], 0x8000));
        assert_eq!(gsu.sfr & super::SFR_B_BIT, 0);
        assert_ne!(gsu.sfr & super::SFR_ALT1_BIT, 0);
        assert_ne!(gsu.sfr & super::SFR_ALT2_BIT, 0);
        assert_eq!(gsu.alt_mode(), 3);
    }

    #[test]
    fn loop_clears_prefix_flags_and_resets_selectors() {
        let mut gsu = SuperFx::new(0x20_0000);
        gsu.regs[12] = 0x0002;
        gsu.regs[13] = 0x9000;

        assert!(gsu.execute_opcode(0x24, &[], 0x8000));
        assert_eq!(gsu.debug_src_reg(), 4);
        assert_eq!(gsu.debug_dst_reg(), 4);

        assert!(gsu.execute_opcode(0x3C, &[], 0x8001));
        assert_eq!(gsu.regs[12], 0x0001);
        assert_eq!(gsu.regs[15], 0x9000);
        assert_eq!(gsu.debug_src_reg(), 0);
        assert_eq!(gsu.debug_dst_reg(), 0);
        assert_eq!(gsu.sfr & super::SFR_B_BIT, 0);
        assert_eq!(gsu.sfr & super::SFR_ALT1_BIT, 0);
        assert_eq!(gsu.sfr & super::SFR_ALT2_BIT, 0);
    }

    #[test]
    fn ibt_sign_extends_immediate_when_alt0() {
        let mut gsu = SuperFx::new(0x20_0000);
        let mut rom = vec![0u8; 0x20_0000];
        rom[0x0000] = 0x80;
        gsu.regs[15] = 0x8000;

        assert!(gsu.execute_opcode_internal(0xA3, &rom, 0x0000, false));
        assert_eq!(gsu.regs[3], 0xFF80);
    }

    #[test]
    fn lms_loads_word_from_ram_word_address_immediate() {
        let mut gsu = SuperFx::new(0x20_0000);
        let mut rom = vec![0u8; 0x20_0000];
        rom[0x0000] = 0x12;
        gsu.sfr |= super::SFR_ALT1_BIT;
        gsu.regs[15] = 0x8000;
        gsu.write_ram_word(0x0024, 0xBEEF);

        assert!(gsu.execute_opcode_internal(0xA1, &rom, 0x0000, false));
        assert_eq!(gsu.regs[1], 0xBEEF);
    }

    #[test]
    fn alt3_a0_uses_lms_not_ibt() {
        let mut gsu = SuperFx::new(0x20_0000);
        let mut rom = vec![0u8; 0x20_0000];
        rom[0x0000] = 0x12;
        gsu.sfr |= super::SFR_ALT1_BIT | super::SFR_ALT2_BIT;
        gsu.regs[15] = 0x8000;
        gsu.write_ram_word(0x0024, 0xBEEF);

        assert!(gsu.execute_opcode_internal(0xA1, &rom, 0x0000, false));
        assert_eq!(gsu.regs[1], 0xBEEF);
    }

    #[test]
    fn sms_stores_word_to_ram_word_address_immediate() {
        let mut gsu = SuperFx::new(0x20_0000);
        let mut rom = vec![0u8; 0x20_0000];
        rom[0x0000] = 0x12;
        gsu.sfr |= super::SFR_ALT2_BIT;
        gsu.regs[15] = 0x8000;
        gsu.regs[1] = 0xBEEF;

        assert!(gsu.execute_opcode_internal(0xA1, &rom, 0x0000, false));
        assert_eq!(gsu.read_ram_word(0x0024), 0xBEEF);
    }

    #[test]
    fn ramb_uses_low_two_bits_of_source_register() {
        let mut gsu = SuperFx::new(0x20_0000);
        let rom = vec![0u8; 0x20_0000];
        gsu.src_reg = 5;
        gsu.regs[5] = 0x0003;
        gsu.sfr |= super::SFR_ALT2_BIT;

        assert!(gsu.execute_opcode_internal(0xDF, &rom, 0x0000, false));
        assert_eq!(gsu.debug_rambr(), 0x03);
    }

    #[test]
    fn plot_writes_snes_tile_bitplanes_into_game_ram() {
        let mut gsu = SuperFx::new(0x20_0000);
        gsu.scmr = 0x00;
        gsu.scbr = 0x00;
        gsu.colr = 0x03;
        gsu.regs[1] = 0;
        gsu.regs[2] = 0;

        assert!(gsu.execute_opcode(0x4C, &[], 0x8000));
        // Flush pixel caches to write to RAM
        gsu.flush_all_pixel_caches();
        assert_eq!(gsu.game_ram[0], 0x80);
        assert_eq!(gsu.game_ram[1], 0x80);
    }

    #[test]
    fn plot_uses_low_8_bits_of_x_coordinate() {
        let mut gsu = SuperFx::new(0x20_0000);
        gsu.scmr = 0x00;
        gsu.scbr = 0x00;
        gsu.colr = 0x03;
        gsu.regs[1] = 0x0100;
        gsu.regs[2] = 0;

        assert!(gsu.execute_opcode(0x4C, &[], 0x8000));
        gsu.flush_all_pixel_caches();

        assert_eq!(gsu.game_ram[0], 0x80);
        assert_eq!(gsu.game_ram[1], 0x80);
    }

    #[test]
    fn pixel_cache_flush_writes_leftmost_pixel_to_msb() {
        let mut gsu = SuperFx::new(0x20_0000);
        gsu.scmr = 0x00;
        gsu.scbr = 0x00;
        gsu.pixelcache[0].offset = 0;
        gsu.pixelcache[0].bitpend = 0x80;
        gsu.pixelcache[0].data[7] = 0x01;

        gsu.flush_pixel_cache(0);

        assert_eq!(gsu.game_ram[0], 0x80);
        assert_eq!(gsu.pixelcache[0].bitpend, 0);
    }

    #[test]
    fn mode2_screen_mode_uses_4bpp_layout_like_bsnes() {
        let mut gsu = SuperFx::new(0x20_0000);
        gsu.scmr = 0x02;
        gsu.scbr = 0x00;
        gsu.colr = 0x0F;
        gsu.regs[1] = 0;
        gsu.regs[2] = 0;

        assert!(gsu.execute_opcode(0x4C, &[], 0x8000));
        gsu.flush_all_pixel_caches();
        assert_eq!(gsu.game_ram[0x00], 0x80);
        assert_eq!(gsu.game_ram[0x01], 0x80);
        assert_eq!(gsu.game_ram[0x10], 0x80);
        assert_eq!(gsu.game_ram[0x11], 0x80);
        assert_eq!(gsu.bits_per_pixel(), Some(4));
    }

    #[test]
    fn height_mode_3_uses_obj_layout_storage() {
        let mut gsu = SuperFx::new(0x20_0000);
        gsu.scmr = 0x24;

        assert_eq!(gsu.screen_height(), Some(256));
        assert_eq!(gsu.screen_buffer_len(), Some(32 * 32 * 16));
        assert_eq!(gsu.tile_pixel_addr(0, 128), Some((0x2000, 0, 7)));
    }

    #[test]
    fn por_obj_mode_does_not_override_screen_height() {
        let mut gsu = SuperFx::new(0x20_0000);
        gsu.scmr = 0x01; // 4bpp, 128-line base
        gsu.por = 0x10; // OBJ flag affects plot/rpix layout, not nominal SCMR geometry

        assert_eq!(gsu.screen_height(), Some(128));
        assert_eq!(gsu.screen_buffer_len(), Some(32 * 32 * 32));
        assert_eq!(gsu.tile_pixel_addr(0, 127), Some((0x1E00, 7, 7)));
        assert_eq!(gsu.tile_pixel_addr(0, 255), Some((0x5E00, 7, 7)));
    }

    #[test]
    fn por_obj_mode_matches_obj_layout_tile_addressing() {
        let mut gsu = SuperFx::new(0x20_0000);
        gsu.scmr = 0x00; // 2bpp, 128-line nominal
        gsu.por = 0x10; // OBJ plot layout

        assert_eq!(gsu.screen_height(), Some(128));
        assert_eq!(gsu.screen_buffer_len(), Some(32 * 32 * 16));
        assert_eq!(gsu.tile_pixel_addr(0, 0), Some((0x0000, 0, 7)));
        assert_eq!(gsu.tile_pixel_addr(128, 0), Some((0x1000, 0, 7)));
        assert_eq!(gsu.tile_pixel_addr(0, 128), Some((0x2000, 0, 7)));
        assert_eq!(gsu.tile_pixel_addr(128, 128), Some((0x3000, 0, 7)));
    }

    #[test]
    fn rpix_preserves_colr_and_returns_pixel_in_destination_register() {
        let mut gsu = SuperFx::new(0x20_0000);
        gsu.scmr = 0x01;
        gsu.scbr = 0x00;
        gsu.regs[1] = 0;
        gsu.regs[2] = 0;
        gsu.dst_reg = 4;
        gsu.regs[4] = 0x8EBC;
        gsu.colr = 0xC3;
        gsu.sfr |= super::SFR_ALT1_BIT;
        gsu.plot_pixel(0, 0, 0x0A);

        assert!(gsu.execute_opcode(0x4C, &[], 0x8000));
        assert_eq!(gsu.colr, 0xC3);
        assert_eq!(gsu.regs[4], 0x000A);
    }

    #[test]
    fn cpu_access_is_not_blocked_by_scmr_when_gsu_is_idle() {
        let mut gsu = SuperFx::new(0x20_0000);
        gsu.scmr = super::SCMR_RON_BIT | super::SCMR_RAN_BIT;
        gsu.running = false;

        assert!(gsu.cpu_has_rom_access());
        assert!(gsu.cpu_has_ram_access());
    }

    #[test]
    fn cpu_access_is_blocked_by_scmr_bits_while_gsu_is_running() {
        let mut gsu = SuperFx::new(0x20_0000);
        gsu.scmr = super::SCMR_RON_BIT | super::SCMR_RAN_BIT;
        gsu.running = true;

        assert!(!gsu.cpu_has_rom_access());
        assert!(!gsu.cpu_has_ram_access());
    }

    #[test]
    fn merge_combines_high_bytes_of_r7_and_r8() {
        let mut gsu = SuperFx::new(0x20_0000);
        let rom = vec![0u8; 0x20_0000];
        gsu.src_reg = 0;
        gsu.dst_reg = 3;
        gsu.regs[7] = 0xAB12;
        gsu.regs[8] = 0xCD34;

        assert!(gsu.execute_opcode_internal(0x70, &rom, 0x8000, false));
        // Result = R7 high (0xAB) << 8 | R8 high (0xCD)
        assert_eq!(gsu.regs[3], 0xABCD);
    }

    #[test]
    fn merge_sets_flags_from_result_bit_masks() {
        let mut gsu = SuperFx::new(0x20_0000);
        let rom = vec![0u8; 0x20_0000];
        gsu.dst_reg = 0;
        gsu.regs[7] = 0x8000;
        gsu.regs[8] = 0x8000;

        assert!(gsu.execute_opcode_internal(0x70, &rom, 0x8000, false));
        assert_eq!(gsu.regs[0], 0x8080);
        // bsnes: S=(0x8080&0x8080)!=0 → true, Z=(0x8080&0xF0F0)==0 → false
        // CY=(0x8080&0xE0E0)!=0 → true, OV=(0x8080&0xC0C0)!=0 → true
        assert_ne!(gsu.sfr & SFR_S_BIT, 0);
        assert_eq!(gsu.sfr & SFR_Z_BIT, 0);
        assert_ne!(gsu.sfr & SFR_CY_BIT, 0);
        assert_ne!(gsu.sfr & super::SFR_OV_BIT, 0);
    }

    #[test]
    fn add_carry_matches_unsigned_reference_for_wraparound_sum() {
        let mut gsu = SuperFx::new(0x20_0000);
        let rom = vec![0u8; 0x20_0000];
        gsu.src_reg = 1;
        gsu.dst_reg = 2;
        gsu.regs[1] = 0xFFFF;
        gsu.regs[3] = 0x0001;

        assert!(gsu.execute_opcode_internal(0x53, &rom, 0x8000, false));
        assert_eq!(gsu.regs[2], 0x0000);
        assert_ne!(gsu.sfr & SFR_CY_BIT, 0);
        assert_ne!(gsu.sfr & SFR_Z_BIT, 0);
    }

    #[test]
    fn adc_carry_matches_unsigned_reference_for_wraparound_sum() {
        let mut gsu = SuperFx::new(0x20_0000);
        let rom = vec![0u8; 0x20_0000];
        gsu.src_reg = 4;
        gsu.dst_reg = 5;
        gsu.regs[4] = 0xFFFF;
        gsu.sfr |= super::SFR_ALT1_BIT | super::SFR_ALT2_BIT | super::SFR_CY_BIT;
        gsu.sync_condition_flags_from_sfr();

        assert!(gsu.execute_opcode_internal(0x50, &rom, 0x8000, false));
        assert_eq!(gsu.regs[5], 0x0000);
        assert_ne!(gsu.sfr & super::SFR_CY_BIT, 0);
        assert_ne!(gsu.sfr & super::SFR_Z_BIT, 0);
    }

    #[test]
    fn add_sets_carry_for_later_starfox_feedback_values() {
        let mut gsu = SuperFx::new(0x20_0000);
        let rom = vec![0u8; 0x20_0000];
        gsu.src_reg = 2;
        gsu.dst_reg = 1;
        gsu.regs[2] = 0x9528;
        gsu.regs[3] = 0xB2A0;

        assert!(gsu.execute_opcode_internal(0x53, &rom, 0x8000, false));
        assert_eq!(gsu.regs[1], 0x47C8);
        assert_ne!(gsu.sfr & SFR_CY_BIT, 0);
    }

    #[test]
    fn sub_carry_matches_unsigned_reference_for_wraparound_diff() {
        let mut gsu = SuperFx::new(0x20_0000);
        let rom = vec![0u8; 0x20_0000];
        gsu.src_reg = 0;
        gsu.dst_reg = 1;
        gsu.regs[0] = 0xFFFE;
        gsu.regs[4] = 0x7FFF;

        assert!(gsu.execute_opcode_internal(0x64, &rom, 0x8000, false));
        assert_eq!(gsu.regs[1], 0x7FFF);
        assert_ne!(gsu.sfr & super::SFR_CY_BIT, 0);
        assert_eq!(gsu.sfr & super::SFR_S_BIT, 0);
    }

    #[test]
    fn b414_helper_builds_r4_and_copies_it_to_r6_on_non_equal_path() {
        let mut gsu = SuperFx::new(0x20_0000);
        let mut rom = vec![0u8; 0x20_0000];
        rom[0x0000..0x0013].copy_from_slice(&[
            0xA0, 0x03, // IBT R0,#03
            0x3F, // ALT3
            0x64, // CMP R4
            0x09, 0x1F, // BEQ +0x1F (not taken)
            0x01, // NOP
            0xA0, 0x02, // IBT R0,#02
            0x3F, // ALT3
            0x64, // CMP R4
            0x09, 0x09, // BEQ +0x09 (not taken)
            0x01, // NOP
            0x24, // WITH R4
            0x3E, // ALT2
            0x55, // ADD #5 => R4 = 6
            0x24, // WITH R4
            0x16, // TO R6 (B-form) => R6 = 6
        ]);
        gsu.regs[4] = 0x0001;
        gsu.regs[6] = 0x0003;
        gsu.regs[15] = 0x8000;
        gsu.running = true;

        gsu.run_steps(&rom, 15);

        assert_eq!(gsu.regs[4], 0x0006);
        assert_eq!(gsu.regs[6], 0x0006);
        assert_eq!(gsu.debug_src_reg(), 0);
        assert_eq!(gsu.debug_dst_reg(), 0);
        assert_eq!(gsu.sfr & super::SFR_B_BIT, 0);
    }

    #[test]
    fn b37d_helper_builds_r4_r12_and_r13_for_b384_loop() {
        let mut gsu = SuperFx::new(0x20_0000);
        let mut rom = vec![0u8; 0x20_0000];
        rom[0x0000..0x0008].copy_from_slice(&[
            0x24, // WITH R4
            0x3E, // ALT2
            0x57, // ADD #7 => R4 = 7
            0xAC, 0x08, // IBT R12,#08
            0x2F, // WITH R15
            0x1D, // TO R13 => R13 = R15
            0x22, // WITH R2
        ]);
        gsu.regs[4] = 0x0000;
        gsu.regs[15] = 0x8000;
        gsu.running = true;

        gsu.run_steps(&rom, 8);

        assert_eq!(gsu.regs[4], 0x0007);
        assert_eq!(gsu.regs[12], 0x0008);
        assert_eq!(gsu.regs[13], 0x8007);
    }

    #[test]
    fn merge_flags_when_upper_bits_clear() {
        let mut gsu = SuperFx::new(0x20_0000);
        let rom = vec![0u8; 0x20_0000];
        gsu.dst_reg = 0;
        gsu.regs[7] = 0x12FF;
        gsu.regs[8] = 0x34FF;

        assert!(gsu.execute_opcode_internal(0x70, &rom, 0x8000, false));
        assert_eq!(gsu.regs[0], 0x1234);
        // bsnes: S=(0x1234&0x8080)!=0 → false, Z=(0x1234&0xF0F0)==0 → false
        // CY=(0x1234&0xE0E0)!=0 → true(0x0020), OV=(0x1234&0xC0C0)!=0 → false
        assert_eq!(gsu.sfr & SFR_S_BIT, 0);
        assert_eq!(gsu.sfr & SFR_Z_BIT, 0);
        assert_ne!(gsu.sfr & SFR_CY_BIT, 0);
        assert_eq!(gsu.sfr & super::SFR_OV_BIT, 0);
    }

    #[test]
    fn stw_alt2_falls_back_to_word_store() {
        let mut gsu = SuperFx::new(0x20_0000);
        gsu.src_reg = 5;
        gsu.regs[5] = 0xBEEF;
        gsu.regs[1] = 0x0040;
        gsu.sfr |= super::SFR_ALT2_BIT;

        assert!(gsu.execute_opcode(0x31, &[], 0x8000));
        assert_eq!(gsu.read_ram_word(0x0040), 0xBEEF);
    }

    #[test]
    fn loop_taken_keeps_prefetched_delay_slot_visible() {
        let mut gsu = SuperFx::new(0x20_0000);
        let mut rom = vec![0u8; 0x20_0000];
        rom[0x0000] = 0x31; // STW [R1], R0
        rom[0x0001] = 0xD1; // INC R1
        rom[0x0002] = 0x3C; // LOOP
        rom[0x0003] = 0x00; // STOP

        gsu.pbr = 0x00;
        gsu.regs[15] = 0x8000;
        gsu.regs[13] = 0x8000;
        gsu.regs[12] = 10;
        gsu.regs[1] = 0x0010;
        gsu.regs[0] = 0xA55A;
        gsu.src_reg = 0;
        gsu.running = true;

        gsu.run_steps(&rom, 30);

        assert!(!gsu.running());
        assert_eq!(gsu.regs[12], 9);
        assert_eq!(gsu.regs[1], 0x0011);
        assert_eq!(gsu.game_ram[0x0010], 0x5A);
        assert_eq!(gsu.game_ram[0x0011], 0xA5);
    }

    #[test]
    fn simple_store_inc_loop_fast_path_collapses_taken_iterations() {
        let mut gsu = SuperFx::new(0x20_0000);
        let mut rom = vec![0u8; 0x20_0000];
        rom[0x0000] = 0x31; // STW [R1], R0
        rom[0x0001] = 0xD1; // INC R1
        rom[0x0002] = 0x3C; // LOOP
        rom[0x0003] = 0xD1; // delay slot: INC R1
        rom[0x0004] = 0x00; // STOP

        gsu.pbr = 0x00;
        gsu.regs[15] = 0x8000;
        gsu.regs[13] = 0x8000;
        gsu.regs[12] = 0x0004;
        gsu.regs[1] = 0x0010;
        gsu.regs[0] = 0xA55A;
        gsu.src_reg = 0;
        gsu.running = true;

        assert_eq!(gsu.prime_pipe(&rom), Some(()));
        assert_eq!(gsu.fast_forward_simple_store_inc_loop(&rom, 64), Some(12));

        assert_eq!(gsu.regs[12], 0x0001);
        assert_eq!(gsu.regs[1], 0x0016);
        assert_eq!(gsu.regs[15], 0x8000);
        assert!(!gsu.pipe_valid);
        assert_eq!(gsu.read_ram_word(0x0010), 0xA55A);
        assert_eq!(gsu.read_ram_word(0x0012), 0xA55A);
        assert_eq!(gsu.read_ram_word(0x0014), 0xA55A);
        assert_eq!(gsu.sfr & SFR_Z_BIT, 0);
        assert_eq!(gsu.sfr & SFR_S_BIT, 0);
    }

    #[test]
    fn simple_store_inc_loop_fast_path_rejoins_generic_execution() {
        let mut gsu = SuperFx::new(0x20_0000);
        let mut rom = vec![0u8; 0x20_0000];
        rom[0x0000] = 0x31; // STW [R1], R0
        rom[0x0001] = 0xD1; // INC R1
        rom[0x0002] = 0x3C; // LOOP
        rom[0x0003] = 0xD1; // delay slot: INC R1
        rom[0x0004] = 0x00; // STOP

        gsu.pbr = 0x00;
        gsu.regs[15] = 0x8000;
        gsu.regs[13] = 0x8000;
        gsu.regs[12] = 0x0004;
        gsu.regs[1] = 0x0010;
        gsu.regs[0] = 0xA55A;
        gsu.src_reg = 0;
        gsu.running = true;

        assert_eq!(gsu.prime_pipe(&rom), Some(()));
        assert_eq!(gsu.fast_forward_simple_store_inc_loop(&rom, 64), Some(12));

        gsu.run_steps(&rom, 16);

        assert!(!gsu.running());
        assert_eq!(gsu.regs[12], 0x0000);
        assert_eq!(gsu.regs[1], 0x0018);
        assert_eq!(gsu.read_ram_word(0x0010), 0xA55A);
        assert_eq!(gsu.read_ram_word(0x0012), 0xA55A);
        assert_eq!(gsu.read_ram_word(0x0014), 0xA55A);
        assert_eq!(gsu.read_ram_word(0x0016), 0xA55A);
    }

    #[test]
    fn run_steps_auto_uses_simple_store_inc_loop_fast_path() {
        let mut gsu = SuperFx::new(0x20_0000);
        let mut rom = vec![0u8; 0x20_0000];
        rom[0x0000] = 0x31; // STW [R1], R0
        rom[0x0001] = 0xD1; // INC R1
        rom[0x0002] = 0x3C; // LOOP
        rom[0x0003] = 0xD1; // delay slot: INC R1
        rom[0x0004] = 0x00; // STOP

        gsu.pbr = 0x00;
        gsu.regs[15] = 0x8000;
        gsu.regs[13] = 0x8000;
        gsu.regs[12] = 0x0100;
        gsu.regs[1] = 0x0010;
        gsu.regs[0] = 0xA55A;
        gsu.src_reg = 0;
        gsu.running = true;

        gsu.run_steps(&rom, 64);

        assert!(gsu.running());
        assert_eq!(gsu.regs[12], 0x00F0);
        assert_eq!(gsu.regs[1], 0x0030);
        assert!(gsu.debug_recent_pc_transfers().is_empty());
        assert_eq!(gsu.read_ram_word(0x0010), 0xA55A);
        assert_eq!(gsu.read_ram_word(0x0012), 0xA55A);
        assert_eq!(gsu.read_ram_word(0x002E), 0xA55A);
    }

    #[test]
    fn run_steps_b48b_helper_copies_bytes_and_returns_via_r8() {
        let mut gsu = SuperFx::new(0x20_0000);
        let mut rom = vec![0u8; 0x20_0000];

        rom[0x0000] = 0xE6; // DEC R6
        rom[0x0001] = 0xB1; // FROM R1
        rom[0x0002] = 0x54; // ADD R4
        rom[0x0003] = 0xE0; // DEC R0
        rom[0x0004] = 0x3D; // ALT1
        rom[0x0005] = 0x40; // LDB (R0)
        rom[0x0006] = 0xE1; // DEC R1
        rom[0x0007] = 0x3D; // ALT1
        rom[0x0008] = 0x31; // STB (R1)
        rom[0x0009] = 0xE6; // DEC R6
        rom[0x000A] = 0x0A; // BPL
        rom[0x000B] = 0xF5; // -> B1
        rom[0x000C] = 0x01; // NOP
        rom[0x000D] = 0x98; // JMP R8
        rom[0x000E] = 0x01; // NOP
        rom[0x1000] = 0x00; // STOP

        gsu.pbr = 0x00;
        gsu.regs[15] = 0x8000;
        gsu.regs[13] = 0x8000;
        gsu.regs[1] = 0x0012;
        gsu.regs[4] = 0x0002;
        gsu.regs[6] = 0x0001;
        gsu.regs[8] = 0x9000;
        gsu.game_ram[0x0013] = 0xAB;
        gsu.running = true;

        gsu.run_steps(&rom, 20);

        assert_eq!(gsu.game_ram[0x0011], 0xAB);
        assert_eq!(gsu.regs[1], 0x0011);
        assert_eq!(gsu.regs[6], 0xFFFF);
        assert_eq!(
            gsu.debug_recent_pc_transfers()
                .last()
                .map(|t| (t.from_pc, t.to_pc)),
            Some((0x800D, 0x9000))
        );
    }

    #[test]
    fn stb_alt3_falls_back_to_byte_store() {
        let mut gsu = SuperFx::new(0x20_0000);
        gsu.src_reg = 5;
        gsu.regs[5] = 0xABCD;
        gsu.regs[1] = 0x0050;
        gsu.sfr |= super::SFR_ALT1_BIT | super::SFR_ALT2_BIT;

        assert!(gsu.execute_opcode(0x31, &[], 0x8000));
        assert_eq!(gsu.game_ram[0x0050], 0xCD);
        assert_eq!(gsu.game_ram[0x0051], 0x00);
    }

    #[test]
    fn lms_r14_loads_little_endian_word_from_short_ram() {
        let mut gsu = SuperFx::new(0x20_0000);
        let mut rom = vec![0u8; 0x20_0000];
        rom[0x0001] = 0x31;
        gsu.game_ram[0x0062] = 0x96;
        gsu.game_ram[0x0063] = 0xD8;
        gsu.regs[15] = 0x8001;
        gsu.sfr |= super::SFR_ALT1_BIT;

        assert!(gsu.execute_opcode(0xAE, &rom, 0x8000));
        assert_eq!(gsu.regs[14], 0xD896);
    }

    #[test]
    fn lms_r9_loads_little_endian_word_from_short_ram() {
        let mut gsu = SuperFx::new(0x20_0000);
        let mut rom = vec![0u8; 0x20_0000];
        rom[0x0001] = 0x16;
        gsu.game_ram[0x002C] = 0x00;
        gsu.game_ram[0x002D] = 0x40;
        gsu.regs[15] = 0x8001;
        gsu.sfr |= super::SFR_ALT1_BIT;

        assert!(gsu.execute_opcode(0xA9, &rom, 0x8000));
        assert_eq!(gsu.regs[9], 0x4000);
    }

    fn write_test_data_rom_byte(rom: &mut [u8], bank: u8, addr: u16, value: u8) {
        let bank = bank & 0x7F;
        let full_addr = ((bank as usize) << 16) | addr as usize;
        let offset = if (full_addr & 0xE0_0000) == 0x40_0000 {
            full_addr
        } else {
            ((full_addr & 0x3F_0000) >> 1) | (full_addr & 0x7FFF)
        };
        rom[offset % rom.len()] = value;
    }

    fn make_b301_packet_transform_state(r14_packet: u16) -> (SuperFx, Vec<u8>) {
        let mut gsu = SuperFx::new(0x20_0000);
        let mut rom = vec![0u8; 0x20_0000];

        let program = [
            0x3D, 0xA0, 0x32, 0x3F, 0xDF, 0x60, 0x3E, 0xDF, 0x3D, 0xAE, 0x31, 0xEE, 0x11, 0xEF,
            0xEE, 0x21, 0x3D, 0xEF, 0xEE, 0xEE, 0xEE, 0x13, 0xEF, 0xEE, 0x23, 0x3D, 0xEF, 0xEE,
            0x12, 0xEF, 0xEE, 0x22, 0x3D, 0xEF, 0x3D, 0xA9, 0x16,
        ];
        let program_base = 0x01usize * 0x8000 + (0xB301usize & 0x7FFF);
        rom[program_base..program_base + program.len()].copy_from_slice(&program);

        write_test_data_rom_byte(&mut rom, 0x14, 0xD895, 0x00);
        write_test_data_rom_byte(&mut rom, 0x14, 0xD894, 0x20);
        write_test_data_rom_byte(&mut rom, 0x14, 0xD891, 0x06);
        write_test_data_rom_byte(&mut rom, 0x14, 0xD890, 0x02);

        write_test_data_rom_byte(&mut rom, 0x14, 0xD5A9, 0x00);
        write_test_data_rom_byte(&mut rom, 0x14, 0xD5A8, 0x0A);
        write_test_data_rom_byte(&mut rom, 0x14, 0xD5A5, 0x12);
        write_test_data_rom_byte(&mut rom, 0x14, 0xD5A4, 0x00);

        gsu.pbr = 0x01;
        gsu.rombr = 0x00;
        gsu.regs[0] = 0x0824;
        gsu.regs[1] = 0x5191;
        gsu.regs[2] = 0x5190;
        gsu.regs[3] = 0x0700;
        gsu.regs[4] = 0x0000;
        gsu.regs[5] = 0xC7F8;
        gsu.regs[6] = 0x0153;
        gsu.regs[7] = 0xB4B6;
        gsu.regs[8] = 0xB337;
        gsu.regs[9] = 0x2800;
        gsu.regs[10] = 0x0000;
        gsu.regs[11] = 0xB33E;
        gsu.regs[12] = 0x0000;
        gsu.regs[13] = 0xB3DE;
        gsu.write_reg(14, 0x6242);
        gsu.regs[15] = 0xB301;
        gsu.game_ram[0x0062] = (r14_packet & 0x00FF) as u8;
        gsu.game_ram[0x0063] = (r14_packet >> 8) as u8;
        gsu.game_ram[0x0064] = 0x14;
        gsu.game_ram[0x002C] = 0x00;
        gsu.game_ram[0x002D] = 0x40;
        gsu.debug_prepare_cpu_start(&rom);

        (gsu, rom)
    }

    #[test]
    fn b301_packet_transform_consumes_d896_stream() {
        let (mut gsu, rom) = make_b301_packet_transform_state(0xD896);

        gsu.run_steps(&rom, 34);

        assert_eq!(gsu.regs[1], 0x2000);
        assert_eq!(gsu.regs[2], 0x0000);
        assert_eq!(gsu.regs[3], 0x0206);
        assert_eq!(gsu.regs[9], 0x4000);
        assert_eq!(gsu.regs[14], 0xD88E);
        assert_eq!(gsu.regs[15], 0xB327);
    }

    #[test]
    fn b301_packet_transform_consumes_d5aa_stream() {
        let (mut gsu, rom) = make_b301_packet_transform_state(0xD5AA);

        gsu.run_steps(&rom, 34);

        assert_eq!(gsu.regs[1], 0x0A00);
        assert_eq!(gsu.regs[2], 0x0000);
        assert_eq!(gsu.regs[3], 0x0012);
        assert_eq!(gsu.regs[9], 0x4000);
        assert_eq!(gsu.regs[14], 0xD5A2);
        assert_eq!(gsu.regs[15], 0xB327);
    }

    #[test]
    fn d4b4_success_tail_loads_record_fields_into_r9_r13_and_r6() {
        let mut gsu = SuperFx::new(0x20_0000);
        let mut rom = vec![0u8; 0x20_0000];
        let base = 0x01usize * 0x8000 + (0xD4B4usize & 0x7FFF);
        rom[base..base + 44].copy_from_slice(&[
            0xA2, 0x08, 0x22, 0x5B, 0x12, 0x42, 0x22, 0x19, 0x52, 0x90, 0x3D, 0xA4, 0x14, 0x12,
            0x54, 0xA0, 0x05, 0x5B, 0x3D, 0x40, 0x95, 0xA6, 0x0A, 0x26, 0x5B, 0x16, 0x46, 0x26,
            0x1D, 0x56, 0x90, 0x3D, 0xA4, 0x15, 0x16, 0x54, 0xA5, 0x01, 0x25, 0x5B, 0x15, 0x3D,
            0x45, 0x00,
        ]);

        gsu.pbr = 0x01;
        gsu.rombr = 0x14;
        gsu.regs[0] = 0x0031;
        gsu.regs[1] = 0xE2DA;
        gsu.regs[2] = 0x004B;
        gsu.regs[3] = 0x0000;
        gsu.regs[4] = 0xE2D3;
        gsu.regs[5] = 0x004B;
        gsu.regs[6] = 0x00FC;
        gsu.regs[7] = 0x004B;
        gsu.regs[8] = 0x0003;
        gsu.regs[9] = 0x004A;
        gsu.regs[10] = 0x04C8;
        gsu.regs[11] = 0x1AD6;
        gsu.regs[12] = 0x0129;
        gsu.regs[13] = 0xD1B4;
        gsu.write_reg(14, 0x8A9D);
        gsu.regs[15] = 0xD4B4;
        gsu.game_ram[0x0028] = 0x2B;
        gsu.game_ram[0x0029] = 0x01;
        gsu.game_ram[0x002A] = 0x0A;
        gsu.game_ram[0x002B] = 0x61;
        gsu.game_ram[0x1ADB] = 0xF9;
        gsu.game_ram[0x1ADE] = 0x32;
        gsu.game_ram[0x1ADF] = 0x00;
        gsu.game_ram[0x1AE0] = 0xF9;
        gsu.game_ram[0x1AE1] = 0xFF;
        gsu.running = true;

        gsu.run_steps(&rom, 64);

        assert_eq!(gsu.regs[2], 0x018E);
        assert_eq!(gsu.regs[6], 0x60FC);
        assert_eq!(gsu.regs[9], 0x0032);
        assert_eq!(gsu.regs[13], 0xFFF9);
        assert_eq!(gsu.regs[15], 0xD4E1);
    }

    #[test]
    fn d4b4_success_tail_rearms_match_word_after_loading_zero_cursor() {
        let mut gsu = SuperFx::new(0x20_0000);
        let mut rom = vec![0u8; 0x20_0000];
        let base = 0x01usize * 0x8000 + (0xD4B4usize & 0x7FFF);
        rom[base..base + 44].copy_from_slice(&[
            0xA2, 0x08, 0x22, 0x5B, 0x12, 0x42, 0x22, 0x19, 0x52, 0x90, 0x3D, 0xA4, 0x14, 0x12,
            0x54, 0xA0, 0x05, 0x5B, 0x3D, 0x40, 0x95, 0xA6, 0x0A, 0x26, 0x5B, 0x16, 0x46, 0x26,
            0x1D, 0x56, 0x90, 0x3D, 0xA4, 0x15, 0x16, 0x54, 0xA5, 0x01, 0x25, 0x5B, 0x15, 0x3D,
            0x45, 0x00,
        ]);

        gsu.pbr = 0x01;
        gsu.rombr = 0x14;
        gsu.regs[0] = 0x0031;
        gsu.regs[1] = 0xE2DA;
        gsu.regs[2] = 0x004B;
        gsu.regs[3] = 0x0000;
        gsu.regs[4] = 0xE2D3;
        gsu.regs[5] = 0x004B;
        gsu.regs[6] = 0x00FC;
        gsu.regs[7] = 0x004B;
        gsu.regs[8] = 0x0003;
        gsu.regs[9] = 0x004A;
        gsu.regs[10] = 0x04C8;
        gsu.regs[11] = 0x1AD6;
        gsu.regs[12] = 0x0129;
        gsu.regs[13] = 0xD1B4;
        gsu.write_reg(14, 0x8A7B);
        gsu.regs[15] = 0xD4B4;
        gsu.game_ram[0x0028] = 0x2B;
        gsu.game_ram[0x0029] = 0x01;
        gsu.game_ram[0x002A] = 0x0A;
        gsu.game_ram[0x002B] = 0x61;
        gsu.game_ram[0x1ADB] = 0xF9;
        gsu.game_ram[0x1ADE] = 0x32;
        gsu.game_ram[0x1ADF] = 0x00;
        gsu.game_ram[0x1AE0] = 0x00;
        gsu.game_ram[0x1AE1] = 0x00;
        gsu.running = true;

        gsu.run_steps(&rom, 64);

        assert_eq!(gsu.regs[13], 0x0000);
        assert_eq!(gsu.read_ram_word(0x1AE0), 0xFFF9);
        assert_eq!(gsu.regs[15], 0xD4E1);
    }

    #[test]
    fn d496_success_prelude_builds_continuation_fields_before_tail() {
        let mut gsu = SuperFx::new(0x20_0000);
        let mut rom = vec![0u8; 0x20_0000];
        let base = 0x01usize * 0x8000 + (0xD496usize & 0x7FFF);
        rom[base..base + 33].copy_from_slice(&[
            0x2B, 0x3E, 0x6C, 0xA0, 0x03, 0x5B, 0x3D, 0x40, 0x95, 0xA1, 0x06, 0x21, 0x5B, 0x11,
            0x41, 0x21, 0x13, 0x51, 0x90, 0x3D, 0xA4, 0x13, 0x11, 0x54, 0xA0, 0x04, 0x5B, 0x3D,
            0x40, 0x95, 0xA2, 0x08, 0x00,
        ]);

        gsu.pbr = 0x01;
        gsu.rombr = 0x14;
        gsu.regs[0] = 0x0000;
        gsu.regs[1] = 0x00FC;
        gsu.regs[2] = 0x004B;
        gsu.regs[3] = 0x2B14;
        gsu.regs[4] = 0x0006;
        gsu.regs[5] = 0x004B;
        gsu.regs[6] = 0x00FC;
        gsu.regs[7] = 0x004B;
        gsu.regs[8] = 0x0003;
        gsu.regs[9] = 0x004A;
        gsu.regs[10] = 0x04C8;
        gsu.regs[11] = 0x1AE2;
        gsu.regs[12] = 0x0129;
        gsu.regs[13] = 0xD1B4;
        gsu.write_reg(14, 0x8A7B);
        gsu.regs[15] = 0xD496;
        gsu.game_ram[0x0026] = 0xD3;
        gsu.game_ram[0x0027] = 0xE2;
        gsu.game_ram[0x1AD9] = 0x07;
        gsu.game_ram[0x1ADC] = 0x00;
        gsu.game_ram[0x1ADD] = 0x00;
        gsu.game_ram[0x1ADA] = 0x32;
        gsu.running = true;

        gsu.run_steps(&rom, 27);

        assert_eq!(gsu.regs[1], 0xE2DA);
        assert_eq!(gsu.regs[2], 0x0008);
        assert_eq!(gsu.regs[3], 0x0000);
        assert_eq!(gsu.regs[4], 0xE2D3);
        assert_eq!(gsu.regs[9], 0x004A);
        assert_eq!(gsu.regs[11], 0x1AD6);
        assert_eq!(gsu.regs[13], 0xD1B4);
        assert_eq!(gsu.regs[15], 0xD4B7);
    }

    #[test]
    fn af70_continuation_stream_jumps_to_target_when_pointer_is_nonzero() {
        let mut gsu = SuperFx::new(0x20_0000);
        let mut rom = vec![0u8; 0x20_0000];
        rom[0x0000..0x000D].copy_from_slice(&[
            0x11, // TO R1
            0x4A, // LDW [R10] -> R1
            0x11, // TO R1
            0x41, // LDW [R1] -> R1
            0x21, // WITH R1
            0xB1, // MOVES R1 -> R1 (updates Z)
            0x09, 0x05, // BEQ +5 (not taken)
            0x01, // NOP
            0xFF, 0x00, 0x90, // IWT R15,#9000
            0x01, // stale old-stream byte executes once before the transfer
        ]);
        rom[0x1000] = 0x00; // STOP at $9000

        gsu.pbr = 0x00;
        gsu.regs[10] = 0x04C4;
        gsu.regs[15] = 0x8000;
        gsu.running = true;
        gsu.sfr |= SFR_GO_BIT;
        gsu.write_ram_word(0x04C4, 0x887F);
        gsu.write_ram_word(0x887F, 0x29E3);

        gsu.run_steps(&rom, 11);

        assert_eq!(gsu.regs[1], 0x29E3);
        assert!(!gsu.running());
        assert_eq!(gsu.regs[15], 0x9002);
        assert_eq!(gsu.sfr & SFR_Z_BIT, 0);
    }

    #[test]
    fn af70_continuation_stream_branches_locally_when_pointer_is_zero() {
        let mut gsu = SuperFx::new(0x20_0000);
        let mut rom = vec![0u8; 0x20_0000];
        rom[0x0000..0x000E].copy_from_slice(&[
            0x11, // TO R1
            0x4A, // LDW [R10] -> R1
            0x11, // TO R1
            0x41, // LDW [R1] -> R1
            0x21, // WITH R1
            0xB1, // MOVES R1 -> R1 (updates Z)
            0x09, 0x05, // BEQ +5 (taken)
            0x01, // skipped NOP
            0xFF, 0x00, 0x90, // skipped IWT R15,#9000
            0x00, // padding at $800C
            0x00, // STOP at branch target $800D
        ]);

        gsu.pbr = 0x00;
        gsu.regs[10] = 0x04C4;
        gsu.regs[15] = 0x8000;
        gsu.running = true;
        gsu.sfr |= SFR_GO_BIT;
        gsu.write_ram_word(0x04C4, 0x887F);
        gsu.write_ram_word(0x887F, 0x0000);

        gsu.run_steps(&rom, 7);

        assert_eq!(gsu.regs[1], 0x0000);
        assert!(gsu.running());
        assert_eq!(gsu.regs[15], 0x800D);
        assert_ne!(gsu.sfr & SFR_Z_BIT, 0);
    }

    #[test]
    fn d1d0_success_fragment_branches_after_writing_record_when_r8_is_not_one() {
        let mut gsu = SuperFx::new(0x20_0000);
        let mut rom = vec![0u8; 0x20_0000];
        rom[0x0000..0x0012].copy_from_slice(&[
            0x53, // ADD R3 -> R7
            0xB5, // FROM R5
            0x37, // STW [R7], R5
            0xB6, // FROM R6
            0x3D, // ALT1
            0x33, // STB [R3], R6
            0xA0, 0x01, // IBT R0,#1
            0x3F, // ALT3
            0x68, // CMP R8
            0x08, 0x05, // BNE +5 (taken)
            0x01, // skipped NOP
            0xFF, 0xFD, 0xD3, // skipped IWT R15,#D3FD
            0x00, // padding at $8010
            0x00, // STOP at branch target $8011
        ]);

        gsu.pbr = 0x00;
        gsu.regs[3] = 0x1AD6;
        gsu.regs[5] = 0x004B;
        gsu.regs[6] = 0x00FC;
        gsu.regs[7] = 0x000C;
        gsu.regs[8] = 0x0003;
        gsu.regs[15] = 0x8000;
        gsu.src_reg = 7;
        gsu.dst_reg = 7;
        gsu.running = true;
        gsu.sfr |= SFR_GO_BIT;

        gsu.run_steps(&rom, 14);

        assert_eq!(gsu.regs[7], 0x1AE2);
        assert_eq!(gsu.read_ram_word(0x1AE2), 0x004B);
        assert_eq!(gsu.game_ram[0x1AD6], 0xFC);
        assert!(!gsu.running());
        assert_eq!(gsu.regs[15], 0x8013);
    }

    #[test]
    fn d1d0_success_fragment_falls_through_to_iwt_when_r8_is_one() {
        let mut gsu = SuperFx::new(0x20_0000);
        let mut rom = vec![0u8; 0x20_0000];
        rom[0x0000..0x0011].copy_from_slice(&[
            0x53, // ADD R3 -> R7
            0xB5, // FROM R5
            0x37, // STW [R7], R5
            0xB6, // FROM R6
            0x3D, // ALT1
            0x33, // STB [R3], R6
            0xA0, 0x01, // IBT R0,#1
            0x3F, // ALT3
            0x68, // CMP R8
            0x08, 0x05, // BNE +5 (not taken)
            0x01, // NOP
            0xFF, 0xFD, 0xD3, // IWT R15,#D3FD
            0xD0, // delay slot: INC R0
        ]);
        rom[0x53FD] = 0x00; // STOP at $D3FD

        gsu.pbr = 0x00;
        gsu.regs[3] = 0x1AD6;
        gsu.regs[5] = 0x004B;
        gsu.regs[6] = 0x00FC;
        gsu.regs[7] = 0x000C;
        gsu.regs[8] = 0x0001;
        gsu.regs[15] = 0x8000;
        gsu.src_reg = 7;
        gsu.dst_reg = 7;
        gsu.running = true;
        gsu.sfr |= SFR_GO_BIT;

        gsu.run_steps(&rom, 14);

        assert_eq!(gsu.regs[7], 0x1AE2);
        assert_eq!(gsu.read_ram_word(0x1AE2), 0x004B);
        assert_eq!(gsu.game_ram[0x1AD6], 0xFC);
        assert_eq!(gsu.regs[0], 0x0002);
        assert!(!gsu.running());
        assert_eq!(gsu.regs[15], 0xD3FF);
    }

    #[test]
    fn d1d0_success_fragment_dispatches_to_d316_when_r8_is_three() {
        let mut gsu = SuperFx::new(0x20_0000);
        let mut rom = vec![0u8; 0x20_0000];
        rom[0x0000..0x0027].copy_from_slice(&[
            0x53, // ADD R3 -> R7
            0xB5, // FROM R5
            0x37, // STW [R7], R5
            0xB6, // FROM R6
            0x3D, // ALT1
            0x33, // STB [R3], R6
            0xA0, 0x01, // IBT R0,#1
            0x3F, // ALT3
            0x68, // CMP R8
            0x08, 0x05, // BNE +5
            0x01, // NOP
            0xFF, 0xFD, 0xD3, // IWT R15,#D3FD
            0x01, // delay slot
            0xA0, 0x02, // IBT R0,#2
            0x3F, // ALT3
            0x68, // CMP R8
            0x08, 0x05, // BNE +5
            0x01, // NOP
            0xFF, 0x87, 0xD3, // IWT R15,#D387
            0x01, // delay slot
            0xA0, 0x03, // IBT R0,#3
            0x3F, // ALT3
            0x68, // CMP R8
            0x08, 0x05, // BNE +5 (not taken)
            0x01, // NOP
            0xFF, 0x16, 0xD3, // IWT R15,#D316
            0xD0, // delay slot: INC R0
        ]);
        rom[0x5316] = 0x00; // STOP at $D316

        gsu.pbr = 0x00;
        gsu.regs[3] = 0x1AD6;
        gsu.regs[5] = 0x004B;
        gsu.regs[6] = 0x00FC;
        gsu.regs[7] = 0x000C;
        gsu.regs[8] = 0x0003;
        gsu.regs[15] = 0x8000;
        gsu.src_reg = 7;
        gsu.dst_reg = 7;
        gsu.running = true;
        gsu.sfr |= SFR_GO_BIT;

        gsu.run_steps(&rom, 24);

        assert_eq!(gsu.regs[7], 0x1AE2);
        assert_eq!(gsu.read_ram_word(0x1AE2), 0x004B);
        assert_eq!(gsu.game_ram[0x1AD6], 0xFC);
        assert_eq!(gsu.regs[0], 0x0004);
        assert!(!gsu.running());
        assert_eq!(gsu.regs[15], 0xD318);
    }

    fn run_d316_success_fragment_sample(
        r2_start: u16,
        r3_start: u16,
        r4_start: u16,
        r9_start: u16,
        r12_start: u16,
        expected_r0: u16,
        expected_r2: u16,
        expected_r4: u16,
    ) {
        let mut gsu = SuperFx::new(0x20_0000);
        let mut rom = vec![0u8; 0x20_0000];
        let base = 0x01usize * 0x8000;
        rom[base..base + 43].copy_from_slice(&[
            0x60, // SUB R0 -> R0
            0xA7, 0x01, // IBT R7,#1
            0x27, // WITH R7
            0x53, // ADD R3 -> R7
            0xB0, // FROM R0
            0x3D, // ALT1
            0x37, // STB [R7], R0
            0xB2, // FROM R2
            0x4D, // SWAP
            0x97, // ROR
            0x52, // ADD R2
            0x12, // TO R2
            0x3D, // ALT1
            0x52, // ADC R2
            0xA0, 0x0F, // IBT R0,#0F
            0x72, // AND R2
            0xA7, 0x07, // IBT R7,#7
            0x67, // SUB R7
            0xA7, 0x03, // IBT R7,#3
            0x27, // WITH R7
            0x53, // ADD R3 -> R7
            0xB0, // FROM R0
            0x3D, // ALT1
            0x37, // STB [R7], R0
            0x20, // WITH R0
            0xB0, // MOVES R0
            0x0A, 0x03, // BPL +3
            0x01, // NOP
            0x4F, // NOT
            0xD0, // INC R0
            0x20, // WITH R0
            0x14, // TO R4
            0xB2, // FROM R2
            0x4D, // SWAP
            0x97, // ROR
            0x52, // ADD R2
            0x12, // TO R2
            0x00, // STOP before the next ALT1/ADC stage
        ]);

        gsu.pbr = 0x01;
        gsu.regs[0] = 0x0003;
        gsu.regs[1] = 0x00FC;
        gsu.regs[2] = r2_start;
        gsu.regs[3] = r3_start;
        gsu.regs[4] = r4_start;
        gsu.regs[5] = 0x004B;
        gsu.regs[6] = 0x00FC;
        gsu.regs[7] = r3_start.wrapping_add(0x000C);
        gsu.regs[8] = 0x0003;
        gsu.regs[9] = r9_start;
        gsu.regs[10] = 0x04C8;
        gsu.regs[11] = 0xAD88;
        gsu.regs[12] = r12_start;
        gsu.regs[13] = 0xD1B4;
        gsu.regs[14] = 0x8A7B;
        gsu.regs[15] = 0x8000;
        gsu.src_reg = 0;
        gsu.dst_reg = 0;
        gsu.running = true;
        gsu.sfr = 0x0066 | SFR_GO_BIT;
        gsu.game_ram[r3_start as usize + 1] = 0xA2;
        gsu.game_ram[r3_start as usize + 3] = 0xFF;

        gsu.run_steps(&rom, 64);

        assert_eq!(gsu.regs[0], expected_r0);
        assert_eq!(gsu.regs[2], expected_r2);
        assert_eq!(gsu.regs[4], expected_r4);
        assert_eq!(gsu.regs[7], r3_start.wrapping_add(3));
        assert_eq!(gsu.game_ram[r3_start as usize + 1], 0x00);
        assert_eq!(gsu.game_ram[r3_start as usize + 3], expected_r4 as u8);
        assert!(!gsu.running());
        assert_eq!(gsu.regs[15], 0x802C);
    }

    #[test]
    fn d316_success_fragment_matches_live_trace_for_88ed_sample() {
        run_d316_success_fragment_sample(
            0x88ED, 0x1AD6, 0x00FB, 0x004E, 0x0129, 0xD7E2, 0x889E, 0x0007,
        );
    }

    #[test]
    fn d316_success_fragment_matches_live_trace_for_1a60_sample() {
        run_d316_success_fragment_sample(
            0x1A60, 0x1D3E, 0x000E, 0x004D, 0x00FD, 0xCB7F, 0x64CD, 0x0006,
        );
    }

    #[test]
    fn d316_success_fragment_matches_live_trace_for_65a3_sample() {
        run_d316_success_fragment_sample(
            0x65A3, 0x1F0C, 0x000D, 0x004C, 0x00DC, 0x9906, 0x1CF8, 0x0001,
        );
    }

    #[test]
    fn late_search_key_override_uses_match_word_when_raw_key_is_missing() {
        let prev = std::env::var_os("STARFOX_FORCE_LATE_SEARCH_KEY_FROM_1AE2");
        std::env::set_var("STARFOX_FORCE_LATE_SEARCH_KEY_FROM_1AE2", "1");

        let mut gsu = SuperFx::new(0x20_0000);
        gsu.running = true;
        gsu.pbr = 0x01;
        gsu.regs[7] = 0x5A89;
        gsu.regs[15] = 0xD47A;
        gsu.write_ram_word(0x1AE0, 0xFFF9);
        gsu.write_ram_word(0x1AE2, 0x004B);
        gsu.write_ram_word(0x1AB8, 0x004B);

        gsu.maybe_force_starfox_late_search_key_from_match();
        assert_eq!(gsu.regs[7], 0x004B);

        if let Some(value) = prev {
            std::env::set_var("STARFOX_FORCE_LATE_SEARCH_KEY_FROM_1AE2", value);
        } else {
            std::env::remove_var("STARFOX_FORCE_LATE_SEARCH_KEY_FROM_1AE2");
        }
    }

    #[test]
    fn parser_key_override_uses_match_word_when_ad46_writes_missing_head_key() {
        let _guard = env_lock().lock().unwrap();
        let prev = std::env::var_os("STARFOX_FORCE_PARSER_KEY_FROM_MATCH_WORD");
        std::env::set_var("STARFOX_FORCE_PARSER_KEY_FROM_MATCH_WORD", "1");

        let mut gsu = SuperFx::new(0x20_0000);
        gsu.running = true;
        gsu.pbr = 0x01;
        gsu.current_exec_pbr = 0x01;
        gsu.current_exec_pc = 0xAD46;
        gsu.current_exec_opcode = 0xA0;
        gsu.write_ram_word(0x1AE0, 0xFFF9);
        gsu.write_ram_word(0x1AE2, 0x004B);
        gsu.write_ram_word(0x1AB8, 0x004B);

        gsu.write_ram_word(0x0136, 0x5ECF);

        assert_eq!(gsu.debug_read_ram_word_short(0x0136), 0x004B);

        if let Some(value) = prev {
            std::env::set_var("STARFOX_FORCE_PARSER_KEY_FROM_MATCH_WORD", value);
        } else {
            std::env::remove_var("STARFOX_FORCE_PARSER_KEY_FROM_MATCH_WORD");
        }
    }

    #[test]
    fn parser_key_override_can_promote_any_table_field_to_record_head() {
        let _guard = env_lock().lock().unwrap();
        let prev = std::env::var_os("STARFOX_FORCE_PARSER_KEY_FROM_ANY_TABLE_FIELD");
        std::env::set_var("STARFOX_FORCE_PARSER_KEY_FROM_ANY_TABLE_FIELD", "1");

        let mut gsu = SuperFx::new(0x20_0000);
        gsu.running = true;
        gsu.pbr = 0x01;
        gsu.current_exec_pbr = 0x01;
        gsu.current_exec_pc = 0xAD46;
        gsu.current_exec_opcode = 0xA0;
        gsu.write_ram_word_short(0x1AB8, 0x004B);
        gsu.write_ram_word_short(0x1AB8 + 12, 0x5A89);

        gsu.write_ram_word_short(0x0136, 0x5A89);
        assert_eq!(gsu.debug_read_ram_word_short(0x0136), 0x004B);

        if let Some(value) = prev {
            std::env::set_var("STARFOX_FORCE_PARSER_KEY_FROM_ANY_TABLE_FIELD", value);
        } else {
            std::env::remove_var("STARFOX_FORCE_PARSER_KEY_FROM_ANY_TABLE_FIELD");
        }
    }

    #[test]
    fn late_search_key_override_can_promote_any_table_field_to_record_head() {
        let prev = std::env::var_os("STARFOX_FORCE_LATE_SEARCH_KEY_FROM_ANY_TABLE_FIELD");
        std::env::set_var("STARFOX_FORCE_LATE_SEARCH_KEY_FROM_ANY_TABLE_FIELD", "1");

        let mut gsu = SuperFx::new(0x20_0000);
        gsu.running = true;
        gsu.pbr = 0x01;
        gsu.regs[7] = 0x5A89;
        gsu.regs[15] = 0xD47A;
        gsu.write_ram_word_short(0x1AB8, 0x004B);
        gsu.write_ram_word_short(0x1AB8 + 12, 0x5A89);

        gsu.maybe_force_starfox_late_search_key_from_match();
        assert_eq!(gsu.regs[7], 0x004B);

        if let Some(value) = prev {
            std::env::set_var("STARFOX_FORCE_LATE_SEARCH_KEY_FROM_ANY_TABLE_FIELD", value);
        } else {
            std::env::remove_var("STARFOX_FORCE_LATE_SEARCH_KEY_FROM_ANY_TABLE_FIELD");
        }
    }

    #[test]
    fn continuation_ptr_override_redirects_887f_write_to_match_fragment() {
        let _guard = env_lock().lock().unwrap();
        let prev = std::env::var_os("STARFOX_FORCE_CONTINUATION_PTR_FROM_MATCH_FRAGMENT");
        std::env::set_var("STARFOX_FORCE_CONTINUATION_PTR_FROM_MATCH_FRAGMENT", "1");

        let mut gsu = SuperFx::new(0x20_0000);
        gsu.running = true;
        gsu.pbr = 0x01;
        gsu.current_exec_pbr = 0x01;
        gsu.current_exec_pc = 0xB396;
        gsu.current_exec_opcode = 0x31;
        gsu.write_ram_word_short(0x888C, 0x4BFC);
        gsu.game_ram[0x021F] = 0x88;

        gsu.write_ram_byte(0x021E, 0x7F);

        assert_eq!(gsu.debug_read_ram_word_short(0x021E), 0x888D);

        if let Some(value) = prev {
            std::env::set_var("STARFOX_FORCE_CONTINUATION_PTR_FROM_MATCH_FRAGMENT", value);
        } else {
            std::env::remove_var("STARFOX_FORCE_CONTINUATION_PTR_FROM_MATCH_FRAGMENT");
        }
    }

    #[test]
    fn continuation_cursor_override_redirects_04c4_887f_to_match_fragment() {
        let _guard = env_lock().lock().unwrap();
        let prev = std::env::var_os("STARFOX_FORCE_CONTINUATION_CURSOR_FROM_MATCH_FRAGMENT");
        std::env::set_var("STARFOX_FORCE_CONTINUATION_CURSOR_FROM_MATCH_FRAGMENT", "1");

        let mut gsu = SuperFx::new(0x20_0000);
        gsu.running = true;
        gsu.pbr = 0x01;
        gsu.current_exec_pbr = 0x01;
        gsu.current_exec_pc = 0xACAD;
        gsu.write_ram_word_short(0x888C, 0x4BFC);

        gsu.write_ram_word(0x04C4, 0x887F);

        assert_eq!(gsu.debug_read_ram_word_short(0x04C4), 0x888D);

        if let Some(value) = prev {
            std::env::set_var(
                "STARFOX_FORCE_CONTINUATION_CURSOR_FROM_MATCH_FRAGMENT",
                value,
            );
        } else {
            std::env::remove_var("STARFOX_FORCE_CONTINUATION_CURSOR_FROM_MATCH_FRAGMENT");
        }
    }

    #[test]
    fn continuation_cursor_override_accepts_explicit_env_value() {
        let _guard = env_lock().lock().unwrap();
        let prev = std::env::var_os("STARFOX_FORCE_CONTINUATION_CURSOR_VALUE");
        std::env::set_var("STARFOX_FORCE_CONTINUATION_CURSOR_VALUE", "8890");

        let mut gsu = SuperFx::new(0x20_0000);
        gsu.running = true;
        gsu.pbr = 0x01;
        gsu.current_exec_pbr = 0x01;
        gsu.current_exec_pc = 0xACAD;

        gsu.write_ram_word(0x04C4, 0x887F);

        assert_eq!(gsu.debug_read_ram_word_short(0x04C4), 0x8890);

        if let Some(value) = prev {
            std::env::set_var("STARFOX_FORCE_CONTINUATION_CURSOR_VALUE", value);
        } else {
            std::env::remove_var("STARFOX_FORCE_CONTINUATION_CURSOR_VALUE");
        }
    }

    #[test]
    fn continuation_cursor_override_can_null_stream_after_success_fragment() {
        let _guard = env_lock().lock().unwrap();
        let prev = std::env::var_os("STARFOX_NULL_CONTINUATION_AFTER_SUCCESS");
        std::env::set_var("STARFOX_NULL_CONTINUATION_AFTER_SUCCESS", "1");

        let mut gsu = SuperFx::new(0x20_0000);
        gsu.running = true;
        gsu.pbr = 0x01;
        gsu.current_exec_pbr = 0x01;
        gsu.current_exec_pc = 0xACAD;
        gsu.write_ram_word_short(0x1AE0, 0xFFF9);
        gsu.write_ram_word_short(0x1AE2, 0x004B);
        gsu.write_ram_word_short(0x888C, 0x4BFC);

        assert_eq!(
            gsu.maybe_force_starfox_continuation_cursor_word(0x04C4, 0x29E3),
            0x0000
        );

        if let Some(value) = prev {
            std::env::set_var("STARFOX_NULL_CONTINUATION_AFTER_SUCCESS", value);
        } else {
            std::env::remove_var("STARFOX_NULL_CONTINUATION_AFTER_SUCCESS");
        }
    }

    #[test]
    fn success_cursor_override_keeps_1ae0_armed_at_d1cc() {
        let _guard = env_lock().lock().unwrap();
        let prev = std::env::var_os("STARFOX_KEEP_SUCCESS_CURSOR_ARMED");
        std::env::set_var("STARFOX_KEEP_SUCCESS_CURSOR_ARMED", "1");

        let mut gsu = SuperFx::new(0x20_0000);
        gsu.running = true;
        gsu.pbr = 0x01;
        gsu.current_exec_pbr = 0x01;
        gsu.current_exec_pc = 0xD1CC;

        gsu.write_ram_word(0x1AE0, 0x0000);

        assert_eq!(gsu.debug_read_ram_word_short(0x1AE0), 0xFFF9);

        if let Some(value) = prev {
            std::env::set_var("STARFOX_KEEP_SUCCESS_CURSOR_ARMED", value);
        } else {
            std::env::remove_var("STARFOX_KEEP_SUCCESS_CURSOR_ARMED");
        }
    }

    #[test]
    fn success_branch_target_override_keeps_r13_at_d1b4() {
        let _guard = env_lock().lock().unwrap();
        let prev = std::env::var_os("STARFOX_KEEP_SUCCESS_BRANCH_TARGET");
        std::env::set_var("STARFOX_KEEP_SUCCESS_BRANCH_TARGET", "1");

        let mut gsu = SuperFx::new(0x20_0000);
        gsu.running = true;
        gsu.pbr = 0x01;
        gsu.current_exec_pbr = 0x01;
        gsu.regs[13] = 0xD1B4;

        gsu.write_reg_exec(13, 0x0000, 0x1D, 0xD4D0);

        assert_eq!(gsu.regs[13], 0xD1B4);

        if let Some(value) = prev {
            std::env::set_var("STARFOX_KEEP_SUCCESS_BRANCH_TARGET", value);
        } else {
            std::env::remove_var("STARFOX_KEEP_SUCCESS_BRANCH_TARGET");
        }
    }

    #[test]
    fn success_branch_target_override_can_redirect_tail_to_b196() {
        let _guard = env_lock().lock().unwrap();
        let prev = std::env::var_os("STARFOX_FORCE_SUCCESS_BRANCH_TO_B196");
        std::env::set_var("STARFOX_FORCE_SUCCESS_BRANCH_TO_B196", "1");

        let mut gsu = SuperFx::new(0x20_0000);
        gsu.running = true;
        gsu.pbr = 0x01;
        gsu.current_exec_pbr = 0x01;
        gsu.regs[13] = 0xD1B4;

        gsu.write_reg_exec(13, 0x0000, 0x1D, 0xD4D0);

        assert_eq!(gsu.regs[13], 0xB196);

        if let Some(value) = prev {
            std::env::set_var("STARFOX_FORCE_SUCCESS_BRANCH_TO_B196", value);
        } else {
            std::env::remove_var("STARFOX_FORCE_SUCCESS_BRANCH_TO_B196");
        }
    }

    #[test]
    fn b30a_r14_seed_override_applies_only_on_matching_frame_and_pc() {
        let _guard = env_lock().lock().unwrap();
        let prev_value = std::env::var_os("STARFOX_FORCE_B30A_R14_VALUE");
        let prev_frame = std::env::var_os("STARFOX_FORCE_B30A_R14_FRAME");
        std::env::set_var("STARFOX_FORCE_B30A_R14_VALUE", "0x0000");
        std::env::set_var("STARFOX_FORCE_B30A_R14_FRAME", "163");

        let mut gsu = SuperFx::new(0x20_0000);
        gsu.running = true;
        gsu.pbr = 0x01;
        gsu.current_exec_pbr = 0x01;

        super::set_trace_superfx_exec_frame(163);
        assert_eq!(
            gsu.maybe_force_starfox_b30a_r14_seed(14, 0xD896, 0xB30A),
            0x0000
        );
        assert_eq!(
            gsu.maybe_force_starfox_b30a_r14_seed(13, 0xD896, 0xB30A),
            0xD896
        );
        assert_eq!(
            gsu.maybe_force_starfox_b30a_r14_seed(14, 0xD896, 0xB30B),
            0xD896
        );

        super::set_trace_superfx_exec_frame(164);
        assert_eq!(
            gsu.maybe_force_starfox_b30a_r14_seed(14, 0xD896, 0xB30A),
            0xD896
        );

        if let Some(value) = prev_value {
            std::env::set_var("STARFOX_FORCE_B30A_R14_VALUE", value);
        } else {
            std::env::remove_var("STARFOX_FORCE_B30A_R14_VALUE");
        }
        if let Some(value) = prev_frame {
            std::env::set_var("STARFOX_FORCE_B30A_R14_FRAME", value);
        } else {
            std::env::remove_var("STARFOX_FORCE_B30A_R14_FRAME");
        }
    }

    #[test]
    fn b380_r12_seed_override_applies_only_on_matching_frame_and_pc() {
        let _guard = env_lock().lock().unwrap();
        let prev_value = std::env::var_os("STARFOX_FORCE_B380_R12_VALUE");
        let prev_frame = std::env::var_os("STARFOX_FORCE_B380_R12_FRAME");
        std::env::set_var("STARFOX_FORCE_B380_R12_VALUE", "6");
        std::env::set_var("STARFOX_FORCE_B380_R12_FRAME", "163");

        let mut gsu = SuperFx::new(0x20_0000);
        gsu.running = true;
        gsu.pbr = 0x01;
        gsu.current_exec_pbr = 0x01;

        super::set_trace_superfx_exec_frame(163);
        assert_eq!(
            gsu.maybe_force_starfox_b380_r12_seed(12, 0x0008, 0xB380),
            0x0006
        );
        assert_eq!(
            gsu.maybe_force_starfox_b380_r12_seed(11, 0x0008, 0xB380),
            0x0008
        );
        assert_eq!(
            gsu.maybe_force_starfox_b380_r12_seed(12, 0x0008, 0xB381),
            0x0008
        );

        super::set_trace_superfx_exec_frame(164);
        assert_eq!(
            gsu.maybe_force_starfox_b380_r12_seed(12, 0x0008, 0xB380),
            0x0008
        );

        if let Some(value) = prev_value {
            std::env::set_var("STARFOX_FORCE_B380_R12_VALUE", value);
        } else {
            std::env::remove_var("STARFOX_FORCE_B380_R12_VALUE");
        }
        if let Some(value) = prev_frame {
            std::env::set_var("STARFOX_FORCE_B380_R12_FRAME", value);
        } else {
            std::env::remove_var("STARFOX_FORCE_B380_R12_FRAME");
        }
    }

    #[test]
    fn b384_preexec_live_state_override_applies_only_on_matching_frame_and_pc() {
        let _guard = env_lock().lock().unwrap();
        let prev_r12 = std::env::var_os("STARFOX_FORCE_B384_PREEXEC_R12_VALUE");
        let prev_r14 = std::env::var_os("STARFOX_FORCE_B384_PREEXEC_R14_VALUE");
        let prev_frame = std::env::var_os("STARFOX_FORCE_B384_PREEXEC_FRAME");
        std::env::set_var("STARFOX_FORCE_B384_PREEXEC_R12_VALUE", "6");
        std::env::set_var("STARFOX_FORCE_B384_PREEXEC_R14_VALUE", "0");
        std::env::set_var("STARFOX_FORCE_B384_PREEXEC_FRAME", "163");

        let mut gsu = SuperFx::new(0x20_0000);
        gsu.running = true;
        gsu.pbr = 0x01;
        gsu.current_exec_pbr = 0x01;
        gsu.regs[12] = 0x0008;
        gsu.regs[14] = 0xCD92;

        super::set_trace_superfx_exec_frame(163);
        gsu.maybe_force_starfox_b384_preexec_live_state(0xB384);
        assert_eq!(gsu.regs[12], 0x0006);
        assert_eq!(gsu.regs[14], 0x0000);

        gsu.regs[12] = 0x0008;
        gsu.regs[14] = 0xCD92;
        gsu.maybe_force_starfox_b384_preexec_live_state(0xB388);
        assert_eq!(gsu.regs[12], 0x0006);
        assert_eq!(gsu.regs[14], 0x0000);

        gsu.regs[12] = 0x0008;
        gsu.regs[14] = 0xCD92;
        gsu.maybe_force_starfox_b384_preexec_live_state(0xB396);
        assert_eq!(gsu.regs[12], 0x0006);
        assert_eq!(gsu.regs[14], 0x0000);

        gsu.regs[12] = 0x0008;
        gsu.regs[14] = 0xCD92;
        gsu.maybe_force_starfox_b384_preexec_live_state(0xB397);
        assert_eq!(gsu.regs[12], 0x0008);
        assert_eq!(gsu.regs[14], 0xCD92);

        super::set_trace_superfx_exec_frame(164);
        gsu.maybe_force_starfox_b384_preexec_live_state(0xB384);
        assert_eq!(gsu.regs[12], 0x0008);
        assert_eq!(gsu.regs[14], 0xCD92);

        if let Some(value) = prev_r12 {
            std::env::set_var("STARFOX_FORCE_B384_PREEXEC_R12_VALUE", value);
        } else {
            std::env::remove_var("STARFOX_FORCE_B384_PREEXEC_R12_VALUE");
        }
        if let Some(value) = prev_r14 {
            std::env::set_var("STARFOX_FORCE_B384_PREEXEC_R14_VALUE", value);
        } else {
            std::env::remove_var("STARFOX_FORCE_B384_PREEXEC_R14_VALUE");
        }
        if let Some(value) = prev_frame {
            std::env::set_var("STARFOX_FORCE_B384_PREEXEC_FRAME", value);
        } else {
            std::env::remove_var("STARFOX_FORCE_B384_PREEXEC_FRAME");
        }
    }

    #[test]
    fn success_context_override_keeps_r9_and_r13_when_success_tail_zeroes_them() {
        let _guard = env_lock().lock().unwrap();
        let prev = std::env::var_os("STARFOX_KEEP_SUCCESS_CONTEXT");
        std::env::set_var("STARFOX_KEEP_SUCCESS_CONTEXT", "1");

        let mut gsu = SuperFx::new(0x20_0000);
        gsu.running = true;
        gsu.pbr = 0x01;
        gsu.current_exec_pbr = 0x01;
        gsu.regs[7] = 0x004B;
        gsu.regs[9] = 0x004A;
        gsu.regs[13] = 0xD1B4;

        gsu.write_reg_exec(9, 0x0000, 0x19, 0xD4BB);
        gsu.write_reg_exec(13, 0x0000, 0x1D, 0xD4D0);

        assert_eq!(gsu.regs[9], 0x004A);
        assert_eq!(gsu.regs[13], 0xD1B4);

        if let Some(value) = prev {
            std::env::set_var("STARFOX_KEEP_SUCCESS_CONTEXT", value);
        } else {
            std::env::remove_var("STARFOX_KEEP_SUCCESS_CONTEXT");
        }
    }

    #[test]
    fn ac98_override_can_null_success_continuation_word_before_bad_parser_handoff() {
        let _guard = env_lock().lock().unwrap();
        let prev = std::env::var_os("STARFOX_NULL_AC98_AFTER_SUCCESS");
        std::env::set_var("STARFOX_NULL_AC98_AFTER_SUCCESS", "1");

        let mut gsu = SuperFx::new(0x20_0000);
        gsu.running = true;
        gsu.pbr = 0x01;
        gsu.current_exec_pbr = 0x01;
        gsu.write_ram_word_short(0x1AE2, 0x004B);
        gsu.write_ram_word_short(0x888C, 0x4BFC);

        gsu.write_reg_exec(1, 0x887F, 0xF1, 0xAC98);

        assert_eq!(gsu.regs[1], 0x0000);

        if let Some(value) = prev {
            std::env::set_var("STARFOX_NULL_AC98_AFTER_SUCCESS", value);
        } else {
            std::env::remove_var("STARFOX_NULL_AC98_AFTER_SUCCESS");
        }
    }

    #[test]
    fn d31a_success_fragment_clears_record_plus_one_flag_byte() {
        let mut gsu = SuperFx::new(0x20_0000);
        let mut rom = vec![0u8; 0x20_0000];
        rom[0x0000..0x0008].copy_from_slice(&[
            0xA7, 0x01, // IBT R7,#1
            0x27, // WITH R7
            0x53, // ADD R3 -> R7
            0xB0, // FROM R0
            0x3D, // ALT1
            0x37, // STB [R7], R0
            0x00, // STOP
        ]);

        gsu.pbr = 0x00;
        gsu.regs[0] = 0x0000;
        gsu.regs[3] = 0x1AD6;
        gsu.regs[7] = 0x000C;
        gsu.regs[15] = 0x8000;
        gsu.game_ram[0x1AD7] = 0xA2;
        gsu.running = true;
        gsu.sfr |= SFR_GO_BIT;

        for _ in 0..16 {
            if !gsu.running() {
                break;
            }
            gsu.run_steps(&rom, 1);
        }

        assert_eq!(gsu.regs[7], 0x1AD7);
        assert_eq!(gsu.game_ram[0x1AD7], 0x00);
        assert!(!gsu.running());
    }

    #[test]
    fn d4d8_success_tail_sets_low_bit_on_zero_flag_byte() {
        let mut gsu = SuperFx::new(0x20_0000);
        let mut rom = vec![0u8; 0x20_0000];
        rom[0x0000..0x0015].copy_from_slice(&[
            0xA5, 0x01, // IBT R5,#1
            0x25, // WITH R5
            0x5B, // ADD R11 -> R5
            0x15, // TO R5
            0x3D, // ALT1
            0x45, // LDB [R5] -> R5
            0xB5, // FROM R5
            0x3E, // ALT2
            0xC1, // OR #1 -> R0
            0xA4, 0x01, // IBT R4,#1
            0x24, // WITH R4
            0x5B, // ADD R11 -> R4
            0xB0, // FROM R0
            0x3D, // ALT1
            0x34, // STB [R4], R0
            0xB5, // FROM R5
            0x3E, // ALT2
            0x72, // AND #2 -> R0
            0x00, // STOP
        ]);

        gsu.pbr = 0x00;
        gsu.regs[1] = 0xE2DA;
        gsu.regs[2] = 0x007D;
        gsu.regs[4] = 0xFC4B;
        gsu.regs[5] = 0x0001;
        gsu.regs[6] = 0xFC44;
        gsu.regs[7] = 0x004B;
        gsu.regs[8] = 0x0003;
        gsu.regs[10] = 0x04C8;
        gsu.regs[11] = 0x1AD6;
        gsu.regs[12] = 0x0129;
        gsu.regs[14] = 0x8A7B;
        gsu.regs[15] = 0x8000;
        gsu.game_ram[0x1AD7] = 0x00;
        gsu.running = true;
        gsu.sfr |= SFR_GO_BIT;

        for _ in 0..32 {
            if !gsu.running() {
                break;
            }
            gsu.run_steps(&rom, 1);
        }

        assert_eq!(gsu.game_ram[0x1AD7], 0x01);
        assert_eq!(gsu.regs[0], 0x0000);
        assert!(!gsu.running());
    }

    #[test]
    fn d4d8_success_tail_preserves_existing_flag_bits_before_setting_low_bit() {
        let mut gsu = SuperFx::new(0x20_0000);
        let mut rom = vec![0u8; 0x20_0000];
        rom[0x0000..0x0015].copy_from_slice(&[
            0xA5, 0x01, // IBT R5,#1
            0x25, // WITH R5
            0x5B, // ADD R11 -> R5
            0x15, // TO R5
            0x3D, // ALT1
            0x45, // LDB [R5] -> R5
            0xB5, // FROM R5
            0x3E, // ALT2
            0xC1, // OR #1 -> R0
            0xA4, 0x01, // IBT R4,#1
            0x24, // WITH R4
            0x5B, // ADD R11 -> R4
            0xB0, // FROM R0
            0x3D, // ALT1
            0x34, // STB [R4], R0
            0xB5, // FROM R5
            0x3E, // ALT2
            0x72, // AND #2 -> R0
            0x00, // STOP
        ]);

        gsu.pbr = 0x00;
        gsu.regs[1] = 0xE2DA;
        gsu.regs[2] = 0x007D;
        gsu.regs[4] = 0xFC4B;
        gsu.regs[5] = 0x0001;
        gsu.regs[6] = 0xFC44;
        gsu.regs[7] = 0x004B;
        gsu.regs[8] = 0x0003;
        gsu.regs[10] = 0x04C8;
        gsu.regs[11] = 0x1AD6;
        gsu.regs[12] = 0x0129;
        gsu.regs[14] = 0x8A7B;
        gsu.regs[15] = 0x8000;
        gsu.game_ram[0x1AD7] = 0xA2;
        gsu.running = true;
        gsu.sfr |= SFR_GO_BIT;

        for _ in 0..32 {
            if !gsu.running() {
                break;
            }
            gsu.run_steps(&rom, 1);
        }

        assert_eq!(gsu.game_ram[0x1AD7], 0xA3);
        assert_eq!(gsu.regs[0], 0x0002);
        assert!(!gsu.running());
    }

    #[test]
    fn stop_updates_cbr_and_clears_r_bit() {
        let mut gsu = SuperFx::new(0x20_0000);
        let mut rom = vec![0u8; 0x20_0000];
        rom[0x0000] = 0xD0; // INC R0
        rom[0x0001] = 0x00; // STOP
        gsu.pbr = 0x00;
        gsu.regs[15] = 0x8000;
        gsu.running = true;
        gsu.sfr |= SFR_GO_BIT;

        gsu.run_steps(&rom, 4);
        assert!(!gsu.running());
        // CBR should be updated to R15 & 0xFFF0 at STOP
        assert_eq!(gsu.cbr, gsu.regs[15] & 0xFFF0);
        // R_BIT should be cleared
        assert_eq!(gsu.sfr & super::SFR_R_BIT, 0);
        // GO bit should be cleared
        assert_eq!(gsu.sfr & SFR_GO_BIT, 0);
    }

    #[test]
    fn stop_clears_prefix_flags_and_plot_option_register() {
        let mut gsu = SuperFx::new(0x20_0000);
        let mut rom = vec![0u8; 0x20_0000];
        rom[0x0000] = 0x00; // STOP
        gsu.pbr = 0x00;
        gsu.regs[15] = 0x8000;
        gsu.running = true;
        gsu.sfr |= SFR_GO_BIT | SFR_ALT1_BIT | super::SFR_ALT2_BIT | super::SFR_B_BIT;
        gsu.src_reg = 6;
        gsu.dst_reg = 7;
        gsu.with_reg = 8;
        gsu.por = 0x1F;

        gsu.run_steps(&rom, 1);

        assert_eq!(
            gsu.sfr & (SFR_ALT1_BIT | super::SFR_ALT2_BIT | super::SFR_B_BIT),
            0
        );
        assert_eq!(gsu.src_reg, 0);
        assert_eq!(gsu.dst_reg, 0);
        assert_eq!(gsu.with_reg, 0);
        assert_eq!(gsu.por, 0);
    }

    #[test]
    fn sfr_r_bit_set_while_running() {
        let mut gsu = SuperFx::new(0x20_0000);
        let mut rom = vec![0u8; 0x20_0000];
        rom[0x0000] = 0xD0; // INC R0
        rom[0x0001] = 0xD0; // INC R0
        rom[0x0002] = 0x00; // STOP
        gsu.pbr = 0x00;
        gsu.regs[15] = 0x8000;
        gsu.running = true;
        gsu.sfr |= SFR_GO_BIT;

        // Run only 1 step - should still be running
        gsu.run_steps(&rom, 1);
        assert!(gsu.running());
        assert_ne!(gsu.sfr & super::SFR_R_BIT, 0);
    }

    #[test]
    fn run_steps_stops_immediately_after_ram_save_hit() {
        let mut gsu = SuperFx::new(0x20_0000);
        let mut rom = vec![0u8; 0x20_0000];
        rom[0x0000] = 0xD0; // INC R0
        rom[0x0001] = 0xD0; // INC R0
        rom[0x0002] = 0x00; // STOP
        gsu.pbr = 0x00;
        gsu.regs[15] = 0x8000;
        gsu.running = true;
        gsu.sfr |= SFR_GO_BIT;
        gsu.save_state_ram_addr_hit = Some((0x00, 0x8000, 0x0010));

        gsu.run_steps(&rom, 8);

        assert_eq!(gsu.regs[0], 1);
        assert!(gsu.running());
        assert_eq!(gsu.save_state_ram_addr_hit, Some((0x00, 0x8000, 0x0010)));
    }

    #[test]
    fn ram_word_after_byte_write_uses_pending_xor_paired_byte() {
        let mut gsu = SuperFx::new(0x20_0000);
        gsu.game_ram[0x021E] = 0x52;
        gsu.game_ram[0x021F] = 0x88;

        assert_eq!(gsu.read_ram_word(0x021E), 0x8852);
        assert_eq!(gsu.ram_word_after_byte_write(0x021E, 0x021E, 0x7F), 0x887F);
        assert_eq!(gsu.ram_word_after_byte_write(0x021E, 0x021F, 0x29), 0x2952);
    }

    #[test]
    fn plot_always_increments_r1() {
        let mut gsu = SuperFx::new(0x20_0000);
        gsu.scmr = 0x00; // 2bpp, 128h
        gsu.scbr = 0x00;
        gsu.colr = 0x01;
        gsu.por = 0x08;
        gsu.regs[1] = 10;
        gsu.regs[2] = 0;

        assert!(gsu.execute_opcode(0x4C, &[], 0x8000));
        assert_eq!(gsu.regs[1], 11);
    }

    #[test]
    fn apply_color_matches_shift_and_merge_bits() {
        let mut gsu = SuperFx::new(0x20_0000);
        gsu.colr = 0xA0;

        gsu.por = 0x04;
        assert_eq!(gsu.apply_color(0xBC), 0xAB);

        gsu.por = 0x08;
        assert_eq!(gsu.apply_color(0xBC), 0xAC);

        gsu.por = 0x0C;
        assert_eq!(gsu.apply_color(0xBC), 0xAB);
    }

    #[test]
    fn plot_dither_mode_selects_color_nibble_by_position() {
        let mut gsu = SuperFx::new(0x20_0000);
        gsu.scmr = 0x00; // 2bpp, 128h
        gsu.scbr = 0x04; // offset screen to avoid overlap
        gsu.por = 0x0A; // dither (bit 1) + merge low nibble (bit 3)
        gsu.colr = 0x31; // high=3, low=1

        // Even position (x+y=0): use low nibble (1)
        gsu.regs[1] = 0;
        gsu.regs[2] = 0;
        assert!(gsu.execute_opcode(0x4C, &[], 0x8000));

        // Odd position (x+y=1): use high nibble (3)
        gsu.regs[1] = 1;
        gsu.regs[2] = 0;
        assert!(gsu.execute_opcode(0x4C, &[], 0x8000));
    }

    #[test]
    fn color_opcode_respects_por_shift_and_merge_bits() {
        let mut gsu = SuperFx::new(0x20_0000);
        gsu.regs[0] = 0x00BC;
        gsu.src_reg = 0;

        gsu.colr = 0xA0;
        gsu.por = 0x04;
        assert!(gsu.execute_opcode(0x4E, &[], 0x8000));
        assert_eq!(gsu.colr, 0xAB);

        gsu.regs[0] = 0x00BC;
        gsu.colr = 0xA0;
        gsu.por = 0x08;
        assert!(gsu.execute_opcode(0x4E, &[], 0x8000));
        assert_eq!(gsu.colr, 0xAC);

        gsu.regs[0] = 0x00BC;
        gsu.colr = 0xA0;
        gsu.por = 0x0C;
        assert!(gsu.execute_opcode(0x4E, &[], 0x8000));
        assert_eq!(gsu.colr, 0xAB);
    }

    #[test]
    fn plot_8bpp_uses_full_byte_for_transparency_when_freezehigh_is_clear() {
        let mut gsu = SuperFx::new(0x20_0000);
        gsu.scmr = 0x03; // 8bpp
        gsu.scbr = 0x00;

        gsu.plot_pixel(0, 0, 0x10);
        gsu.flush_all_pixel_caches();
        assert_ne!(gsu.read_plot_pixel(0, 0), 0);

        let mut gsu = SuperFx::new(0x20_0000);
        gsu.scmr = 0x03; // 8bpp
        gsu.scbr = 0x00;
        gsu.por = 0x08; // freezehigh

        gsu.plot_pixel(0, 0, 0x10);
        gsu.flush_all_pixel_caches();
        assert_eq!(gsu.read_plot_pixel(0, 0), 0);
    }

    #[test]
    fn cmode_opcode_updates_plot_option_register() {
        let mut gsu = SuperFx::new(0x20_0000);
        gsu.regs[0] = 0x0010;
        gsu.src_reg = 0;
        gsu.sfr |= SFR_ALT1_BIT;

        assert!(gsu.execute_opcode(0x4E, &[], 0x8000));
        assert_eq!(gsu.por, 0x10);
        assert_eq!(gsu.screen_height(), Some(128));
    }

    #[test]
    fn alt3_cmode_opcode_updates_plot_option_register() {
        let mut gsu = SuperFx::new(0x20_0000);
        gsu.regs[0] = 0x0010;
        gsu.src_reg = 0;
        gsu.sfr |= SFR_ALT1_BIT | super::SFR_ALT2_BIT;

        assert!(gsu.execute_opcode(0x4E, &[], 0x8000));
        assert_eq!(gsu.por, 0x10);
        assert_eq!(gsu.screen_height(), Some(128));
    }

    #[test]
    fn alt3_rpix_reads_pixel() {
        let mut gsu = SuperFx::new(0x20_0000);
        gsu.dst_reg = 3;
        gsu.regs[1] = 0;
        gsu.regs[2] = 0;
        gsu.colr = 0x5A;
        gsu.sfr |= SFR_ALT1_BIT | super::SFR_ALT2_BIT;
        gsu.plot_pixel(0, 0, 0x0A);

        assert!(gsu.execute_opcode(0x4C, &[], 0x8000));
        assert_eq!(gsu.regs[3], 0x0002);
        assert_eq!(gsu.colr, 0x5A);
    }

    #[test]
    fn rpix_4bit_preserves_existing_sign_zero_flags() {
        let mut gsu = SuperFx::new(0x20_0000);
        gsu.scmr = 0x01; // 4bpp
        gsu.scbr = 0x00;
        gsu.dst_reg = 2;
        gsu.regs[1] = 0;
        gsu.regs[2] = 0;
        gsu.sfr |= SFR_ALT1_BIT | SFR_S_BIT | SFR_Z_BIT;
        gsu.plot_pixel(0, 0, 0x0A);

        assert!(gsu.execute_opcode(0x4C, &[], 0x8000));
        assert_eq!(gsu.regs[2], 0x000A);
        assert_eq!(gsu.sfr & SFR_S_BIT, SFR_S_BIT);
        assert_eq!(gsu.sfr & SFR_Z_BIT, SFR_Z_BIT);
    }

    #[test]
    fn rpix_8bit_zero_case_updates_zero_only_and_preserves_sign() {
        let mut gsu = SuperFx::new(0x20_0000);
        gsu.scmr = 0x03; // 8bpp
        gsu.scbr = 0x00;
        gsu.dst_reg = 2;
        gsu.regs[1] = 0;
        gsu.regs[2] = 0;
        gsu.sfr |= SFR_ALT1_BIT | SFR_S_BIT;
        gsu.plot_pixel(0, 0, 0x00);

        assert!(gsu.execute_opcode(0x4C, &[], 0x8000));
        assert_eq!(gsu.regs[2], 0x0000);
        assert_eq!(gsu.sfr & SFR_S_BIT, SFR_S_BIT);
        assert_eq!(gsu.sfr & SFR_Z_BIT, SFR_Z_BIT);
    }

    #[test]
    fn rom_bank_mask_adapts_to_rom_size() {
        // 1MB ROM = 32 banks of 32KB → mask = 31
        let gsu_1m = SuperFx::new(0x10_0000);
        assert_eq!(gsu_1m.rom_bank_mask, 31);

        // 2MB ROM = 64 banks → mask = 63
        let gsu_2m = SuperFx::new(0x20_0000);
        assert_eq!(gsu_2m.rom_bank_mask, 63);

        // 512KB ROM = 16 banks → mask = 15
        let gsu_512k = SuperFx::new(0x8_0000);
        assert_eq!(gsu_512k.rom_bank_mask, 15);
    }

    #[test]
    fn default_instruction_cycle_cost_is_one() {
        let mut gsu = SuperFx::new(0x20_0000);
        let rom = vec![0u8; 0x20_0000];
        gsu.src_reg = 0;
        gsu.regs[0] = 3;
        gsu.regs[1] = 5;

        assert!(gsu.execute_opcode_internal(0x81, &rom, 0x8000, false));
        assert_eq!(gsu.last_opcode_cycles, 1);
    }
}
