#[cfg(feature = "runtime-debug-flags")]
use std::sync::OnceLock;

pub(super) fn disable_authoritative_superfx_bg1_source() -> bool {
    #[cfg(not(feature = "runtime-debug-flags"))]
    {
        false
    }
    #[cfg(feature = "runtime-debug-flags")]
    {
        static ON: OnceLock<bool> = OnceLock::new();
        *ON.get_or_init(|| std::env::var_os("DISABLE_AUTHORITATIVE_SUPERFX_BG1_SOURCE").is_some())
    }
}

#[derive(Clone, Copy)]
pub(crate) struct TraceScanlineStateConfig {
    pub(crate) frame_min: u64,
    pub(crate) frame_max: u64,
    pub(crate) y_min: u16,
    pub(crate) y_max: u16,
}

#[cfg(not(feature = "runtime-debug-flags"))]
#[inline(always)]
pub(crate) fn trace_scanline_state_config() -> Option<TraceScanlineStateConfig> {
    None
}

#[cfg(feature = "runtime-debug-flags")]
pub(crate) fn trace_scanline_state_config() -> Option<TraceScanlineStateConfig> {
    static CFG: OnceLock<Option<TraceScanlineStateConfig>> = OnceLock::new();
    *CFG.get_or_init(|| {
        std::env::var_os("TRACE_SCANLINE_STATE")?;

        fn env_u64(key: &str, default: u64) -> u64 {
            std::env::var(key)
                .ok()
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or(default)
        }

        fn env_u16(key: &str, default: u16) -> u16 {
            std::env::var(key)
                .ok()
                .and_then(|v| v.parse::<u16>().ok())
                .unwrap_or(default)
        }

        Some(TraceScanlineStateConfig {
            frame_min: env_u64("TRACE_SCANLINE_FRAME_MIN", 0),
            frame_max: env_u64("TRACE_SCANLINE_FRAME_MAX", u64::MAX),
            y_min: env_u16("TRACE_SCANLINE_Y_MIN", 0),
            y_max: env_u16("TRACE_SCANLINE_Y_MAX", u16::MAX),
        })
    })
}

#[derive(Clone, Copy)]
pub(crate) struct TraceSampleDotConfig {
    pub(crate) frame: u64,
    pub(crate) x: u16,
    pub(crate) y: u16,
}

#[cfg(not(feature = "runtime-debug-flags"))]
#[inline(always)]
pub(crate) fn trace_sample_dot_config() -> Option<TraceSampleDotConfig> {
    None
}

#[cfg(feature = "runtime-debug-flags")]
pub(crate) fn trace_sample_dot_config() -> Option<TraceSampleDotConfig> {
    static CFG: OnceLock<Option<TraceSampleDotConfig>> = OnceLock::new();
    *CFG.get_or_init(|| {
        std::env::var_os("TRACE_SAMPLE_DOT")?;

        fn env_u64(key: &str, default: u64) -> u64 {
            std::env::var(key)
                .ok()
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or(default)
        }

        fn env_u16(key: &str, default: u16) -> u16 {
            std::env::var(key)
                .ok()
                .and_then(|v| v.parse::<u16>().ok())
                .unwrap_or(default)
        }

        Some(TraceSampleDotConfig {
            frame: env_u64("TRACE_SAMPLE_DOT_FRAME", 0),
            x: env_u16("TRACE_SAMPLE_DOT_X", 0),
            y: env_u16("TRACE_SAMPLE_DOT_Y", 0),
        })
    })
}

#[derive(Clone, Copy)]
pub(crate) struct TraceVramWriteConfig {
    pub(crate) start_addr: u16,
    pub(crate) end_addr: u16,
    pub(crate) frame_min: u64,
    pub(crate) frame_max: u64,
}

#[derive(Clone, Copy)]
pub(crate) struct TraceCgramWriteConfig {
    pub(crate) addr: u8,
    pub(crate) frame_min: u64,
    pub(crate) frame_max: u64,
}

#[cfg(not(feature = "runtime-debug-flags"))]
#[inline(always)]
pub(crate) fn trace_vram_write_config() -> Option<TraceVramWriteConfig> {
    None
}

#[cfg(feature = "runtime-debug-flags")]
pub(crate) fn trace_vram_write_config() -> Option<TraceVramWriteConfig> {
    static CFG: OnceLock<Option<TraceVramWriteConfig>> = OnceLock::new();
    *CFG.get_or_init(|| {
        fn parse_u16_env(value: &str) -> Option<u16> {
            if let Some(hex) = value.strip_prefix("0x") {
                u16::from_str_radix(hex, 16).ok()
            } else if let Some(hex) = value.strip_prefix("0X") {
                u16::from_str_radix(hex, 16).ok()
            } else {
                value.parse::<u16>().ok()
            }
        }

        let (start_addr, end_addr) = if let Ok(range) = std::env::var("TRACE_VRAM_ADDR_RANGE") {
            let (start, end) = range.split_once('-')?;
            let start_addr = parse_u16_env(start.trim())?;
            let end_addr = parse_u16_env(end.trim())?;
            (start_addr.min(end_addr), start_addr.max(end_addr))
        } else {
            let addr_str = std::env::var("TRACE_VRAM_ADDR").ok()?;
            let addr = parse_u16_env(&addr_str)?;
            (addr, addr)
        };
        let frame_min = std::env::var("TRACE_VRAM_FRAME_MIN")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(0);
        let frame_max = std::env::var("TRACE_VRAM_FRAME_MAX")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(u64::MAX);
        Some(TraceVramWriteConfig {
            start_addr,
            end_addr,
            frame_min,
            frame_max,
        })
    })
}

#[inline]
pub(crate) fn trace_vram_write_match(cfg: TraceVramWriteConfig, addr: u16, frame: u64) -> bool {
    frame >= cfg.frame_min
        && frame <= cfg.frame_max
        && addr >= cfg.start_addr
        && addr <= cfg.end_addr
}

#[cfg(not(feature = "runtime-debug-flags"))]
#[inline(always)]
pub(crate) fn trace_cgram_write_config() -> Option<TraceCgramWriteConfig> {
    None
}

#[cfg(feature = "runtime-debug-flags")]
pub(crate) fn trace_cgram_write_config() -> Option<TraceCgramWriteConfig> {
    static CFG: OnceLock<Option<TraceCgramWriteConfig>> = OnceLock::new();
    *CFG.get_or_init(|| {
        let addr_str = std::env::var("TRACE_CGRAM_ADDR").ok()?;
        let addr = if let Some(hex) = addr_str.strip_prefix("0x") {
            u8::from_str_radix(hex, 16).ok()?
        } else if let Some(hex) = addr_str.strip_prefix("0X") {
            u8::from_str_radix(hex, 16).ok()?
        } else {
            addr_str.parse::<u8>().ok()?
        };
        let frame_min = std::env::var("TRACE_CGRAM_FRAME_MIN")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(0);
        let frame_max = std::env::var("TRACE_CGRAM_FRAME_MAX")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(u64::MAX);
        Some(TraceCgramWriteConfig {
            addr,
            frame_min,
            frame_max,
        })
    })
}

pub(crate) fn trace_cgram_write_match(cfg: TraceCgramWriteConfig, frame: u64, addr: u8) -> bool {
    frame >= cfg.frame_min && frame <= cfg.frame_max && addr == cfg.addr
}
