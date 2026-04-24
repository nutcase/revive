use std::sync::{
    atomic::{AtomicU32, Ordering},
    OnceLock,
};

use crate::debug_flags;

#[cfg(not(feature = "runtime-debug-flags"))]
#[inline(always)]
fn env_flag(_key: &str, default: bool) -> bool {
    default
}

#[cfg(feature = "runtime-debug-flags")]
fn env_flag(key: &str, default: bool) -> bool {
    std::env::var(key)
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(default)
}

#[cfg(not(feature = "runtime-debug-flags"))]
#[inline(always)]
fn env_present(_key: &str) -> bool {
    false
}

#[cfg(feature = "runtime-debug-flags")]
fn env_present(key: &str) -> bool {
    std::env::var_os(key).is_some()
}

#[cfg(not(feature = "runtime-debug-flags"))]
#[inline(always)]
fn env_u32_opt(_key: &str) -> Option<u32> {
    None
}

#[cfg(feature = "runtime-debug-flags")]
fn env_u32_opt(key: &str) -> Option<u32> {
    std::env::var(key).ok().and_then(|v| v.parse::<u32>().ok())
}

#[cfg(not(feature = "runtime-debug-flags"))]
#[inline(always)]
fn env_u64_opt(_key: &str) -> Option<u64> {
    None
}

#[cfg(feature = "runtime-debug-flags")]
fn env_u64_opt(key: &str) -> Option<u64> {
    std::env::var(key).ok().and_then(|v| {
        let trimmed = v.trim();
        trimmed.parse::<u64>().ok().filter(|&value| value > 0)
    })
}

pub(super) fn cpu_test_auto_exit_enabled() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("CPU_TEST_MODE").is_some())
}

pub(super) fn trace_cpu_sfx_ram_callers_enabled() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| env_present("TRACE_CPU_SFX_RAM_CALLERS"))
}

pub(super) fn trace_cpu_slow_read_ms() -> Option<u64> {
    static VAL: OnceLock<Option<u64>> = OnceLock::new();
    *VAL.get_or_init(|| env_u64_opt("TRACE_CPU_SLOW_READ_MS"))
}

pub(super) fn trace_nmi_suppress_enabled() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| env_present("TRACE_NMI_SUPPRESS"))
}

pub(super) fn trace_starfox_slow_profile_enabled() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| {
        env_flag("PERF_VERBOSE", false)
            || debug_flags::trace_starfox_gui_slow_ms() > 0
            || env_present("STARFOX_DIAG_PERF")
    })
}

pub(super) fn auto_press_a_frame() -> Option<u32> {
    static VAL: OnceLock<Option<u32>> = OnceLock::new();
    *VAL.get_or_init(|| env_u32_opt("AUTO_PRESS_A"))
}

pub(super) fn auto_press_a_stop_frame() -> Option<u32> {
    static VAL: OnceLock<Option<u32>> = OnceLock::new();
    *VAL.get_or_init(|| env_u32_opt("AUTO_PRESS_A_STOP"))
}

pub(super) fn auto_press_start_frame() -> Option<u32> {
    static VAL: OnceLock<Option<u32>> = OnceLock::new();
    *VAL.get_or_init(|| env_u32_opt("AUTO_PRESS_START"))
}

fn trace_sram_enabled() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| env_flag("TRACE_SRAM", false))
}

fn trace_sram_limit() -> u32 {
    static LIMIT: OnceLock<u32> = OnceLock::new();
    *LIMIT.get_or_init(|| env_u32_opt("TRACE_SRAM_LIMIT").unwrap_or(64))
}

pub(super) fn trace_sram(access: &str, bank: u32, offset: u16, idx: usize, value: u8) {
    if !trace_sram_enabled() {
        return;
    }
    static COUNT: AtomicU32 = AtomicU32::new(0);
    let n = COUNT.fetch_add(1, Ordering::Relaxed);
    if n < trace_sram_limit() {
        println!(
            "[SRAM {}] bank={:02X} off={:04X} idx=0x{:04X} val=0x{:02X}",
            access, bank, offset, idx, value
        );
    }
}
