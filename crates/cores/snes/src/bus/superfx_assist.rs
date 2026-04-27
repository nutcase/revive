use super::Bus;
use std::sync::OnceLock;

impl Bus {
    pub(super) fn matches_starfox_3030_go_busy_wait_in_wram(wram: &[u8], poll_pc: u32) -> bool {
        let bank = (poll_pc >> 16) as u8;
        if !matches!(bank, 0x7E | 0x7F) {
            return false;
        }

        let offset = (poll_pc & 0xFFFF) as usize;
        if offset + 5 >= 0x10000 {
            return false;
        }

        let base = if bank == 0x7F { 0x10000 } else { 0 };
        let start = base + offset;
        let bytes = &wram[start..start + 6];
        bytes[0] == 0xAD
            && bytes[1] == 0x30
            && bytes[2] == 0x30
            && bytes[3] == 0x29
            && bytes[4] == 0x20
            && bytes[5] == 0xD0
    }

    pub(super) fn starfox_status_poll_producer_budget() -> Option<usize> {
        static VALUE: OnceLock<Option<usize>> = OnceLock::new();
        *VALUE.get_or_init(|| {
            std::env::var("SUPERFX_STATUS_POLL_STARFOX_PRODUCER_BUDGET")
                .ok()
                .and_then(|raw| raw.trim().parse::<usize>().ok())
                .filter(|value| *value > 0)
        })
    }

    pub(super) fn disable_superfx_status_poll_assist_env() -> bool {
        if cfg!(test) {
            return std::env::var_os("DISABLE_SUPERFX_STATUS_POLL_ASSIST").is_some();
        }
        static VALUE: OnceLock<bool> = OnceLock::new();
        *VALUE.get_or_init(|| std::env::var_os("DISABLE_SUPERFX_STATUS_POLL_ASSIST").is_some())
    }

    pub(super) fn enable_superfx_status_poll_assist_env() -> bool {
        if cfg!(test) {
            return std::env::var_os("ENABLE_SUPERFX_STATUS_POLL_ASSIST").is_some();
        }
        static VALUE: OnceLock<bool> = OnceLock::new();
        *VALUE.get_or_init(|| std::env::var_os("ENABLE_SUPERFX_STATUS_POLL_ASSIST").is_some())
    }

    pub(super) fn disable_starfox_late_wait_assist_env() -> bool {
        if cfg!(test) {
            return std::env::var_os("DISABLE_STARFOX_LATE_WAIT_ASSIST").is_some();
        }
        static VALUE: OnceLock<bool> = OnceLock::new();
        *VALUE.get_or_init(|| std::env::var_os("DISABLE_STARFOX_LATE_WAIT_ASSIST").is_some())
    }

    pub(super) fn disable_superfx_status_poll_catchup_env() -> bool {
        if cfg!(test) {
            return std::env::var_os("DISABLE_SUPERFX_STATUS_POLL_CATCHUP").is_some();
        }
        static VALUE: OnceLock<bool> = OnceLock::new();
        *VALUE.get_or_init(|| std::env::var_os("DISABLE_SUPERFX_STATUS_POLL_CATCHUP").is_some())
    }

    pub(super) fn disable_superfx_status_poll_run_until_stop_env() -> bool {
        if cfg!(test) {
            return std::env::var_os("DISABLE_SUPERFX_STATUS_POLL_RUN_UNTIL_STOP").is_some();
        }
        static VALUE: OnceLock<bool> = OnceLock::new();
        *VALUE.get_or_init(|| {
            std::env::var_os("DISABLE_SUPERFX_STATUS_POLL_RUN_UNTIL_STOP").is_some()
        })
    }

    pub(super) fn superfx_status_poll_late_parser_budget() -> Option<usize> {
        if cfg!(test) {
            return std::env::var("SUPERFX_STATUS_POLL_LATE_PARSER_BUDGET")
                .ok()
                .and_then(|value| value.parse::<usize>().ok());
        }
        static VALUE: OnceLock<Option<usize>> = OnceLock::new();
        *VALUE.get_or_init(|| {
            std::env::var("SUPERFX_STATUS_POLL_LATE_PARSER_BUDGET")
                .ok()
                .and_then(|value| value.parse::<usize>().ok())
        })
    }

    pub(super) fn starfox_blocking_late_wait_assist_enabled() -> bool {
        static VALUE: OnceLock<bool> = OnceLock::new();
        *VALUE.get_or_init(|| {
            std::env::var("ENABLE_STARFOX_BLOCKING_LATE_WAIT_ASSIST")
                .ok()
                .map(|raw| raw != "0" && !raw.eq_ignore_ascii_case("false"))
                .unwrap_or(false)
        })
    }

    pub(super) fn is_starfox_late_3030_busy_wait_pc(poll_pc: u32) -> bool {
        matches!((poll_pc >> 16) as u8, 0x7E | 0x7F) && (poll_pc & 0xFFFF) == 0x4EFD
    }

    pub(super) fn apu_echo_wait_budget() -> usize {
        static VALUE: OnceLock<usize> = OnceLock::new();
        *VALUE.get_or_init(|| {
            std::env::var("APU_ECHO_WAIT_BUDGET")
                .or_else(|_| std::env::var("STARFOX_APU_ECHO_WAIT_BUDGET"))
                .ok()
                .and_then(|raw| raw.trim().parse::<usize>().ok())
                .filter(|value| *value > 0)
                .unwrap_or(4_096)
        })
    }

    pub(super) fn apu_echo_wait_assist_disabled() -> bool {
        static VALUE: OnceLock<bool> = OnceLock::new();
        *VALUE.get_or_init(|| std::env::var_os("DISABLE_APU_ECHO_WAIT_ASSIST").is_some())
    }

    pub(super) fn is_starfox_apu_echo_wait_pc(poll_pc: u32) -> bool {
        matches!(
            poll_pc,
            0x03B15E | 0x03B16E | 0x03B1AE | 0x03B1FE | 0x03B221..=0x03B236 | 0x03B262
        )
    }

    pub(super) fn is_starfox_apu_upload_write_high_pc(write_pc: u32) -> bool {
        matches!(
            write_pc,
            0x03B166 | 0x03B1FE | 0x03B22D..=0x03B22F | 0x03B25B
        )
    }

    #[cold]
    #[inline(never)]
    pub(super) fn trace_starfox_status_poll(
        frame: u64,
        scanline: u16,
        cycle: u16,
        cpu_pc: u32,
        mapper_type: crate::cartridge::MapperType,
        poll_pc: u32,
        streak: u16,
        is_wram_poll: bool,
        early_bootstrap: bool,
        cached_delay_loop: bool,
        catch_up_steps: usize,
        run_until_stop_steps: Option<usize>,
    ) {
        if !crate::debug_flags::trace_starfox_boot()
            || mapper_type != crate::cartridge::MapperType::SuperFx
        {
            return;
        }
        if catch_up_steps == 0 && run_until_stop_steps.is_none() {
            return;
        }
        use std::sync::atomic::{AtomicU32, Ordering};
        static COUNT: AtomicU32 = AtomicU32::new(0);
        let n = COUNT.fetch_add(1, Ordering::Relaxed);
        if n >= 512 {
            return;
        }
        println!(
            "[STARFOX-POLL] frame={} sl={} cyc={} cpu_pc={:06X} poll_pc={:06X} streak={} wram={} early={} cached_loop={} catch_up={} until_stop={}",
            frame,
            scanline,
            cycle,
            cpu_pc,
            poll_pc,
            streak,
            is_wram_poll as u8,
            early_bootstrap as u8,
            cached_delay_loop as u8,
            catch_up_steps,
            run_until_stop_steps.unwrap_or(0),
        );
    }
}
