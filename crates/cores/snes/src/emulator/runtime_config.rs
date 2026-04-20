#[derive(Clone, Copy, Debug)]
pub(in crate::emulator) struct EmulatorRuntimeConfig {
    pub(in crate::emulator) headless_requested: bool,
    pub(in crate::emulator) headless_max_frames: u64,
    pub(in crate::emulator) ignore_sram: bool,
    pub(in crate::emulator) audio_off: bool,
    pub(in crate::emulator) external_audio_output: bool,
    pub(in crate::emulator) multitap: bool,
    pub(in crate::emulator) srm_autosave_every: Option<u64>,
    pub(in crate::emulator) adaptive_timing: bool,
    pub(in crate::emulator) disable_frame_skip: bool,
    pub(in crate::emulator) max_frame_skip: u8,
    pub(in crate::emulator) frame_skip_threshold: f64,
    pub(in crate::emulator) apu_step_batch: u32,
    pub(in crate::emulator) apu_step_force: u32,
    pub(in crate::emulator) fast_mode: bool,
    pub(in crate::emulator) sa1_batch_cpu: u16,
}

impl EmulatorRuntimeConfig {
    pub(in crate::emulator) fn from_env(sa1_active: bool) -> Self {
        let fast_mode = Self::read_loose_bool_env("FAST_MODE", sa1_active);
        let sa1_batch_cpu = Self::read_positive_u16_env("SA1_BATCH_CPU").unwrap_or_else(|| {
            if !sa1_active {
                1
            } else if fast_mode {
                64
            } else {
                1
            }
        });

        Self {
            headless_requested: Self::read_strict_bool_env("HEADLESS", false),
            headless_max_frames: Self::read_u64_env("HEADLESS_FRAMES", 300),
            ignore_sram: Self::read_strict_bool_env("IGNORE_SRAM", false),
            audio_off: Self::read_strict_bool_env("NO_AUDIO", false),
            external_audio_output: Self::read_external_audio_output_env(),
            multitap: Self::read_strict_bool_env("MULTITAP", false),
            srm_autosave_every: Self::read_positive_u64_env("SRAM_AUTOSAVE_FRAMES"),
            adaptive_timing: Self::read_loose_bool_env("ADAPTIVE_TIMING", true),
            disable_frame_skip: Self::read_strict_bool_env("DISABLE_FRAME_SKIP", false),
            max_frame_skip: Self::read_u8_env("MAX_FRAME_SKIP", 4).min(10),
            frame_skip_threshold: Self::read_f64_env("FRAME_SKIP_THRESHOLD", 0.95, |v| {
                v > 0.0 && v < 1.0
            }),
            apu_step_batch: Self::read_positive_u32_env("APU_STEP_BATCH").unwrap_or(32),
            apu_step_force: Self::read_positive_u32_env("APU_STEP_FORCE").unwrap_or(2048),
            fast_mode,
            sa1_batch_cpu,
        }
    }

    pub(in crate::emulator) fn read_strict_bool_env(name: &str, default: bool) -> bool {
        std::env::var(name)
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(default)
    }

    pub(in crate::emulator) fn read_loose_bool_env(name: &str, default: bool) -> bool {
        std::env::var(name)
            .map(|v| !(v == "0" || v.eq_ignore_ascii_case("false")))
            .unwrap_or(default)
    }

    pub(in crate::emulator) fn read_u64_env(name: &str, default: u64) -> u64 {
        std::env::var(name)
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(default)
    }

    pub(in crate::emulator) fn read_u32_env(name: &str, default: u32) -> u32 {
        std::env::var(name)
            .ok()
            .and_then(|v| v.parse::<u32>().ok())
            .unwrap_or(default)
    }

    pub(in crate::emulator) fn read_u8_env(name: &str, default: u8) -> u8 {
        std::env::var(name)
            .ok()
            .and_then(|v| v.parse::<u8>().ok())
            .unwrap_or(default)
    }

    pub(in crate::emulator) fn read_positive_u64_env(name: &str) -> Option<u64> {
        std::env::var(name)
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .filter(|&v| v > 0)
    }

    pub(in crate::emulator) fn read_positive_u32_env(name: &str) -> Option<u32> {
        std::env::var(name)
            .ok()
            .and_then(|v| v.parse::<u32>().ok())
            .filter(|&v| v > 0)
    }

    pub(in crate::emulator) fn read_positive_u16_env(name: &str) -> Option<u16> {
        std::env::var(name)
            .ok()
            .and_then(|v| v.parse::<u16>().ok())
            .filter(|&v| v > 0)
    }

    pub(in crate::emulator) fn read_f64_env(
        name: &str,
        default: f64,
        valid: impl Fn(f64) -> bool,
    ) -> f64 {
        std::env::var(name)
            .ok()
            .and_then(|v| v.parse::<f64>().ok())
            .filter(|&v| valid(v))
            .unwrap_or(default)
    }

    fn read_external_audio_output_env() -> bool {
        std::env::var("AUDIO_BACKEND")
            .map(|v| {
                let backend = v.trim().to_ascii_lowercase();
                matches!(
                    backend.as_str(),
                    "sdl" | "sdl_callback" | "sdl-callback" | "callback"
                )
            })
            .unwrap_or(false)
    }
}

#[derive(Clone, Copy, Debug)]
pub(in crate::emulator) struct RunLoopConfig {
    pub(in crate::emulator) force_test_pattern: bool,
    pub(in crate::emulator) perf_stats: bool,
    pub(in crate::emulator) headless_fast_render: bool,
    pub(in crate::emulator) headless_fast_render_last: u64,
    pub(in crate::emulator) present_every: Option<u64>,
    pub(in crate::emulator) auto_present: bool,
    pub(in crate::emulator) headless_stats: bool,
    pub(in crate::emulator) mode7_test: bool,
    pub(in crate::emulator) headless_summary: bool,
}

impl RunLoopConfig {
    pub(in crate::emulator) fn from_env() -> Self {
        let perf_verbose = EmulatorRuntimeConfig::read_strict_bool_env("PERF_VERBOSE", false);
        let perf_stats = EmulatorRuntimeConfig::read_strict_bool_env("PERF_STATS", perf_verbose);
        Self {
            force_test_pattern: std::env::var("FORCE_TEST_PATTERN")
                .map(|v| v == "1")
                .unwrap_or(false),
            perf_stats,
            headless_fast_render: EmulatorRuntimeConfig::read_strict_bool_env(
                "HEADLESS_FAST_RENDER",
                false,
            ),
            headless_fast_render_last: EmulatorRuntimeConfig::read_u64_env(
                "HEADLESS_FAST_RENDER_LAST",
                1,
            ),
            present_every: EmulatorRuntimeConfig::read_positive_u64_env("PRESENT_EVERY"),
            auto_present: EmulatorRuntimeConfig::read_loose_bool_env("AUTO_PRESENT", true),
            headless_stats: EmulatorRuntimeConfig::read_strict_bool_env(
                "HEADLESS_STATS",
                perf_stats,
            ),
            mode7_test: EmulatorRuntimeConfig::read_strict_bool_env("MODE7_TEST", false),
            headless_summary: EmulatorRuntimeConfig::read_strict_bool_env("HEADLESS_SUMMARY", true),
        }
    }

    pub(in crate::emulator) fn headless_fast_render_from(self, headless_max_frames: u64) -> u64 {
        headless_max_frames.saturating_sub(self.headless_fast_render_last.max(1))
    }
}

#[derive(Clone, Copy, Debug)]
pub(in crate::emulator) struct FrameRunConfig {
    pub(in crate::emulator) max_iterations: u64,
    pub(in crate::emulator) stall_threshold: u32,
    pub(in crate::emulator) perf_verbose: bool,
    pub(in crate::emulator) batch_exec: bool,
    pub(in crate::emulator) batch_max: u16,
    pub(in crate::emulator) trace_exec: bool,
    pub(in crate::emulator) trace_pc_ffff: bool,
    pub(in crate::emulator) trace_pc_frame: bool,
    pub(in crate::emulator) trace_loop_cycles: bool,
    pub(in crate::emulator) trace_pc_ffff_once: bool,
    pub(in crate::emulator) smw_force_bbaa: bool,
}

impl FrameRunConfig {
    pub(in crate::emulator) fn from_env(frame_count: u32, fast_mode: bool) -> Self {
        let tracing_heavy = std::env::var_os("TRACE_4210").is_some()
            || std::env::var_os("WATCH_PC").is_some()
            || std::env::var_os("WATCH_PC_FLOW").is_some()
            || std::env::var_os("TRACE_BRANCH").is_some();
        let default_max = if tracing_heavy {
            50_000_000
        } else if frame_count <= 3 {
            5_000_000
        } else {
            1_000_000
        };

        Self {
            max_iterations: EmulatorRuntimeConfig::read_u64_env("LOOP_GUARD_MAX", default_max),
            stall_threshold: EmulatorRuntimeConfig::read_u32_env("TRACE_STALL", 0),
            perf_verbose: EmulatorRuntimeConfig::read_strict_bool_env("PERF_VERBOSE", false),
            batch_exec: EmulatorRuntimeConfig::read_strict_bool_env("CPU_BATCH", fast_mode),
            batch_max: EmulatorRuntimeConfig::read_positive_u16_env("CPU_BATCH_MAX")
                .unwrap_or(if fast_mode { 255 } else { 32 }),
            trace_exec: EmulatorRuntimeConfig::read_strict_bool_env("TRACE_EXEC", false),
            trace_pc_ffff: std::env::var_os("TRACE_PC_FFFF").is_some(),
            trace_pc_frame: std::env::var_os("TRACE_PC_FRAME").is_some(),
            trace_loop_cycles: std::env::var_os("TRACE_LOOP_CYCLES").is_some(),
            trace_pc_ffff_once: std::env::var_os("TRACE_PC_FFFF_ONCE").is_some(),
            smw_force_bbaa: crate::debug_flags::smw_force_bbaa(),
        }
    }
}

#[derive(Clone, Debug)]
pub(in crate::emulator) struct FrameStallTraceState {
    pub(in crate::emulator) pc: u32,
    pub(in crate::emulator) count: u32,
    pub(in crate::emulator) ring: [u32; 16],
    pub(in crate::emulator) ring_pos: usize,
    pub(in crate::emulator) last_diff: u32,
}

impl Default for FrameStallTraceState {
    fn default() -> Self {
        Self {
            pc: 0,
            count: 0,
            ring: [0; 16],
            ring_pos: 0,
            last_diff: 0,
        }
    }
}
