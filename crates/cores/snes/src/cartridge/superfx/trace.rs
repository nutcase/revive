use super::{env_flag_enabled, GSU_REGISTER_COUNT, MAX_RECENT_REG_WRITES_PER_REG};
use std::sync::OnceLock;

pub(super) fn trace_superfx_sfr_enabled() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| env_flag_enabled("TRACE_SUPERFX_SFR"))
}

pub(super) fn trace_superfx_start_enabled() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| env_flag_enabled("TRACE_SUPERFX_START"))
}

pub(super) fn trace_superfx_unimpl_enabled() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| env_flag_enabled("TRACE_SUPERFX_UNIMPL"))
}

pub(super) fn trace_superfx_plot_enabled() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| env_flag_enabled("TRACE_SUPERFX_PLOT"))
}

pub(super) fn trace_superfx_profile_enabled() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| env_flag_enabled("TRACE_SUPERFX_PROFILE"))
}

pub(super) fn trace_superfx_pc_trace_enabled() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| env_flag_enabled("TRACE_SUPERFX_PC_TRACE"))
}

pub(super) fn trace_superfx_reg_flow_enabled() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| {
        env_flag_enabled("TRACE_SUPERFX_REG_FLOW") || env_flag_enabled("TRACE_SUPERFX_LAST_WRITERS")
    })
}

pub(super) fn trace_superfx_last_transfers_enabled() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| {
        cfg!(test)
            || env_flag_enabled("TRACE_SUPERFX_LAST_TRANSFERS")
            || trace_superfx_pc_trace_enabled()
            || trace_superfx_reg_flow_enabled()
            || trace_superfx_exec_at_frame().is_some()
    })
}

pub(super) fn trace_superfx_low_ram_writes_enabled() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| {
        cfg!(test)
            || env_flag_enabled("TRACE_SUPERFX_LOW_RAM")
            || std::env::var_os("TRACE_SUPERFX_LOW_RAM_WORDS").is_some()
            || trace_superfx_exec_at_frame().is_some()
    })
}

pub(super) fn trace_superfx_reg_flow_filter() -> &'static Option<[bool; GSU_REGISTER_COUNT]> {
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

pub(super) fn trace_superfx_reg_history_cap() -> usize {
    static CAP: OnceLock<usize> = OnceLock::new();
    *CAP.get_or_init(|| {
        std::env::var("TRACE_SUPERFX_REG_HISTORY_CAP")
            .ok()
            .and_then(|raw| raw.trim().parse::<usize>().ok())
            .filter(|&n| n > 0)
            .unwrap_or(MAX_RECENT_REG_WRITES_PER_REG)
    })
}

pub(super) fn trace_superfx_reg_flow_exclude_range() -> &'static Option<(u8, u16, u16)> {
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

pub(super) fn trace_superfx_getb_enabled() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| env_flag_enabled("TRACE_SUPERFX_GETB"))
}

pub(super) fn trace_superfx_screen_words_enabled() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| env_flag_enabled("TRACE_SUPERFX_SCREEN_WORDS"))
}

pub(super) fn trace_superfx_screen_bytes_enabled() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| env_flag_enabled("TRACE_SUPERFX_SCREEN_BYTES"))
}

pub(super) fn trace_superfx_stop_captures_enabled() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| env_flag_enabled("TRACE_SUPERFX_STOP_CAPTURES"))
}

pub(super) fn trace_superfx_pc_last_writers_enabled() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| env_flag_enabled("TRACE_SUPERFX_PC_LAST_WRITERS"))
}

pub(super) fn trace_superfx_exec_at_frame() -> Option<u32> {
    static VALUE: OnceLock<Option<u32>> = OnceLock::new();
    *VALUE.get_or_init(|| {
        std::env::var("TRACE_SUPERFX_EXEC_AT_FRAME")
            .ok()
            .and_then(|raw| raw.trim().parse::<u32>().ok())
    })
}

pub(super) fn trace_superfx_exec_frame_matches(frame: u64) -> bool {
    trace_superfx_exec_at_frame().is_none_or(|target| frame == u64::from(target))
}

pub(super) fn trace_superfx_pc_range_raw() -> &'static Option<(u8, u16, u16)> {
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

pub(super) fn trace_superfx_pc_range_post() -> &'static Option<(u8, u16, u16)> {
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

pub(super) fn trace_superfx_pc_range_raw_matches(pbr: u8, pc: u16) -> bool {
    trace_superfx_pc_range_raw()
        .as_ref()
        .is_some_and(|&(bank, start, end)| pbr == bank && pc >= start && pc <= end)
}
