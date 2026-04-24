use super::Bus;

#[cfg(feature = "runtime-debug-flags")]
use std::sync::atomic::{AtomicU32, Ordering};
#[cfg(feature = "runtime-debug-flags")]
use std::sync::OnceLock;

impl Bus {
    #[cfg(feature = "runtime-debug-flags")]
    #[inline]
    fn trace_starfox_boot_io_range() -> Option<(u32, u32)> {
        static CFG: OnceLock<Option<(u32, u32)>> = OnceLock::new();
        *CFG.get_or_init(|| {
            fn parse_u32_env(value: &str) -> Option<u32> {
                if let Some(hex) = value.strip_prefix("0x") {
                    u32::from_str_radix(hex, 16).ok()
                } else if let Some(hex) = value.strip_prefix("0X") {
                    u32::from_str_radix(hex, 16).ok()
                } else {
                    value.parse::<u32>().ok()
                }
            }

            let value = std::env::var("TRACE_STARFOX_IO_ADDR_RANGE").ok()?;
            if let Some((start, end)) = value.split_once('-') {
                let start_addr = parse_u32_env(start.trim())?;
                let end_addr = parse_u32_env(end.trim())?;
                Some((start_addr.min(end_addr), start_addr.max(end_addr)))
            } else {
                let addr = parse_u32_env(value.trim())?;
                Some((addr, addr))
            }
        })
    }

    #[cfg(feature = "runtime-debug-flags")]
    #[inline]
    fn trace_starfox_boot_ctrl_only() -> bool {
        static CFG: OnceLock<bool> = OnceLock::new();
        *CFG.get_or_init(|| std::env::var_os("TRACE_STARFOX_BOOT_CTRL_ONLY").is_some())
    }

    #[cfg(feature = "runtime-debug-flags")]
    #[inline]
    fn trace_wram_abs_target() -> Option<u32> {
        static CFG: OnceLock<Option<u32>> = OnceLock::new();
        *CFG.get_or_init(|| {
            let addr_str = std::env::var("TRACE_WRAM_ABS").ok()?;
            let addr = if let Some(hex) = addr_str.strip_prefix("0x") {
                u32::from_str_radix(hex, 16).ok()?
            } else if let Some(hex) = addr_str.strip_prefix("0X") {
                u32::from_str_radix(hex, 16).ok()?
            } else {
                addr_str.parse::<u32>().ok()?
            };
            Some(addr)
        })
    }

    #[cfg(feature = "runtime-debug-flags")]
    #[inline]
    fn trace_wram_abs_range() -> Option<(u32, u32)> {
        static CFG: OnceLock<Option<(u32, u32)>> = OnceLock::new();
        *CFG.get_or_init(|| {
            fn parse_u32_env(value: &str) -> Option<u32> {
                if let Some(hex) = value.strip_prefix("0x") {
                    u32::from_str_radix(hex, 16).ok()
                } else if let Some(hex) = value.strip_prefix("0X") {
                    u32::from_str_radix(hex, 16).ok()
                } else {
                    value.parse::<u32>().ok()
                }
            }

            let value = std::env::var("TRACE_WRAM_ABS_RANGE").ok()?;
            let (start, end) = value.split_once('-')?;
            let start_addr = parse_u32_env(start.trim())?;
            let end_addr = parse_u32_env(end.trim())?;
            Some((start_addr.min(end_addr), start_addr.max(end_addr)))
        })
    }

    #[cfg(feature = "runtime-debug-flags")]
    #[inline]
    fn trace_wram_abs_pcs_limit() -> Option<usize> {
        static CFG: OnceLock<Option<usize>> = OnceLock::new();
        *CFG.get_or_init(|| {
            std::env::var("TRACE_WRAM_ABS_PCS")
                .ok()
                .and_then(|value| value.trim().parse::<usize>().ok())
                .or(Some(8))
                .filter(|&n| n > 0)
        })
    }

    #[cfg(feature = "runtime-debug-flags")]
    #[inline]
    fn trace_ppu_reg_write_frame_range() -> Option<(u64, u64)> {
        static CFG: OnceLock<Option<(u64, u64)>> = OnceLock::new();
        *CFG.get_or_init(|| {
            let value = std::env::var("TRACE_PPU_REG_WRITE_FRAME").ok()?;
            if let Some((start, end)) = value.split_once('-') {
                let start_frame = start.trim().parse::<u64>().ok()?;
                let end_frame = end.trim().parse::<u64>().ok()?;
                Some((start_frame.min(end_frame), start_frame.max(end_frame)))
            } else {
                let frame = value.trim().parse::<u64>().ok()?;
                Some((frame, frame))
            }
        })
    }

    #[cfg(feature = "runtime-debug-flags")]
    #[inline]
    fn trace_ppu_reg_write_mask() -> u64 {
        static CFG: OnceLock<u64> = OnceLock::new();
        *CFG.get_or_init(|| {
            let Some(value) = std::env::var("TRACE_PPU_REGS").ok() else {
                return 0;
            };
            let mut mask = 0u64;
            for raw in value.split(',') {
                let token = raw.trim();
                if token.is_empty() {
                    continue;
                }
                let reg = if let Some(hex) = token.strip_prefix("0x") {
                    u8::from_str_radix(hex, 16).ok()
                } else if let Some(hex) = token.strip_prefix("0X") {
                    u8::from_str_radix(hex, 16).ok()
                } else {
                    token.parse::<u8>().ok()
                };
                let Some(reg) = reg else {
                    continue;
                };
                if reg <= 0x3F {
                    mask |= 1u64 << reg;
                }
            }
            mask
        })
    }

    #[cfg(not(feature = "runtime-debug-flags"))]
    #[inline(always)]
    pub(super) fn trace_ppu_reg_write(&self, _reg: u8, _value: u8) {}

    #[cfg(feature = "runtime-debug-flags")]
    #[cold]
    #[inline(never)]
    pub(super) fn trace_ppu_reg_write(&self, reg: u8, value: u8) {
        let Some((frame_min, frame_max)) = Self::trace_ppu_reg_write_frame_range() else {
            return;
        };
        let frame = self.ppu.get_frame();
        if frame < frame_min || frame > frame_max {
            return;
        }
        let mask = Self::trace_ppu_reg_write_mask();
        if mask != 0 && (mask & (1u64 << reg)) == 0 {
            return;
        }
        eprintln!(
            "[PPU-REG-W] frame={} sl={} cyc={} PC={:06X} reg=$21{:02X} val={:02X} TM={:02X} TS={:02X}",
            frame,
            self.ppu.scanline,
            self.ppu.get_cycle(),
            self.last_cpu_pc,
            reg,
            value,
            self.ppu.main_screen_designation,
            self.ppu.sub_screen_designation
        );
    }

    #[cfg(not(feature = "runtime-debug-flags"))]
    #[inline(always)]
    pub(super) fn trace_wram_abs_write(&self, _source: &str, _abs: u32, _value: u8) {}

    #[cfg(feature = "runtime-debug-flags")]
    #[cold]
    #[inline(never)]
    pub(super) fn trace_wram_abs_write(&self, source: &str, abs: u32, value: u8) {
        let exact_match = Self::trace_wram_abs_target().is_some_and(|target| abs == target);
        let range_match =
            Self::trace_wram_abs_range().is_some_and(|(start, end)| abs >= start && abs <= end);
        if !(exact_match || range_match) {
            return;
        }
        let recent = Self::trace_wram_abs_pcs_limit().map(|limit| {
            self.recent_cpu_exec_pcs
                .iter()
                .rev()
                .take(limit)
                .map(|pc| format!("{pc:06X}"))
                .collect::<Vec<_>>()
                .join(">")
        });
        if let Some(recent) = recent {
            println!(
                "[TRACE_WRAM_ABS] {} frame={} sl={} cyc={} pc={:06X} exec=[{}] addr=0x{:06X} val=0x{:02X} A={:04X} X={:04X} Y={:04X} DB={:02X} PB={:02X} P={:02X}",
                source,
                self.ppu.get_frame(),
                self.ppu.scanline,
                self.ppu.get_cycle(),
                self.last_cpu_pc,
                recent,
                abs,
                value,
                self.last_cpu_a,
                self.last_cpu_x,
                self.last_cpu_y,
                self.last_cpu_db,
                self.last_cpu_pb,
                self.last_cpu_p
            );
        } else {
            println!(
                "[TRACE_WRAM_ABS] {} frame={} sl={} cyc={} pc={:06X} addr=0x{:06X} val=0x{:02X} A={:04X} X={:04X} Y={:04X} DB={:02X} PB={:02X} P={:02X}",
                source,
                self.ppu.get_frame(),
                self.ppu.scanline,
                self.ppu.get_cycle(),
                self.last_cpu_pc,
                abs,
                value,
                self.last_cpu_a,
                self.last_cpu_x,
                self.last_cpu_y,
                self.last_cpu_db,
                self.last_cpu_pb,
                self.last_cpu_p
            );
        }
    }

    #[cfg(not(feature = "runtime-debug-flags"))]
    #[inline(always)]
    pub(super) fn trace_starfox_boot_io(&self, _kind: &str, _addr: u32, _value: u8) {}

    #[cfg(feature = "runtime-debug-flags")]
    #[cold]
    #[inline(never)]
    pub(super) fn trace_starfox_boot_io(&self, kind: &str, addr: u32, value: u8) {
        if !crate::debug_flags::trace_starfox_boot()
            || self.mapper_type != crate::cartridge::MapperType::SuperFx
        {
            return;
        }
        {
            static FRAME_RANGE: OnceLock<(u64, u64)> = OnceLock::new();
            let (frame_min, frame_max) = *FRAME_RANGE.get_or_init(|| {
                let frame_min = std::env::var("TRACE_STARFOX_FRAME_MIN")
                    .ok()
                    .and_then(|v| v.parse::<u64>().ok())
                    .unwrap_or(0);
                let frame_max = std::env::var("TRACE_STARFOX_FRAME_MAX")
                    .ok()
                    .and_then(|v| v.parse::<u64>().ok())
                    .unwrap_or(u64::MAX);
                (frame_min, frame_max)
            });
            let frame = self.ppu.get_frame();
            if frame < frame_min || frame > frame_max {
                return;
            }
        }
        if std::env::var_os("TRACE_STARFOX_BOOT_WRITES_ONLY").is_some() && kind != "W" {
            return;
        }
        if let Some((start_addr, end_addr)) = Self::trace_starfox_boot_io_range() {
            if addr < start_addr || addr > end_addr {
                return;
            }
        }
        if Self::trace_starfox_boot_ctrl_only()
            && !matches!(
                addr,
                0x2100
                    | 0x2101
                    | 0x2105
                    | 0x2107
                    | 0x2108
                    | 0x2109
                    | 0x210A
                    | 0x210B
                    | 0x210C
                    | 0x2115
                    | 0x2116
                    | 0x2117
                    | 0x212C
                    | 0x212D
                    | 0x3030
                    | 0x3031
            )
        {
            return;
        }
        let bg_regs_only = std::env::var_os("TRACE_STARFOX_BG_REGS").is_some();
        if bg_regs_only
            && !matches!(
                addr,
                0x2105 | 0x2107 | 0x2108 | 0x2109 | 0x210A | 0x210B | 0x210C | 0x212C
            )
        {
            return;
        }
        static COUNT: AtomicU32 = AtomicU32::new(0);
        let n = COUNT.fetch_add(1, Ordering::Relaxed);
        let limit = if Self::trace_starfox_boot_ctrl_only() || bg_regs_only {
            2048
        } else {
            512
        };
        if n >= limit {
            return;
        }
        let (gsu_running, gsu_sfr, gsu_scmr, gsu_pbr, gsu_rombr, gsu_r9, gsu_r13, gsu_r14, gsu_r15) =
            self.superfx
                .as_ref()
                .map(|gsu| {
                    (
                        gsu.running() as u8,
                        gsu.debug_sfr(),
                        gsu.debug_scmr(),
                        gsu.debug_pbr(),
                        gsu.debug_rombr(),
                        gsu.debug_reg(9),
                        gsu.debug_reg(13),
                        gsu.debug_reg(14),
                        gsu.debug_reg(15),
                    )
                })
                .unwrap_or((0, 0, 0, 0, 0, 0, 0, 0, 0));
        println!(
            "[STARFOX-BOOT] kind={} frame={} sl={} cyc={} pc={:06X} exec_pc={:06X} addr={:06X} val={:02X} inidisp={:02X} tm={:02X} gsu_running={} gsu_sfr={:04X} gsu_scmr={:02X} gsu_pbr={:02X} gsu_rombr={:02X} gsu_r9={:04X} gsu_r13={:04X} gsu_r14={:04X} gsu_r15={:04X}",
            kind,
            self.ppu.get_frame(),
            self.ppu.scanline,
            self.ppu.get_cycle(),
            self.last_cpu_pc,
            self.last_cpu_exec_pc,
            addr,
            value,
            self.ppu.screen_display,
            self.ppu.main_screen_designation,
            gsu_running,
            gsu_sfr,
            gsu_scmr,
            gsu_pbr,
            gsu_rombr,
            gsu_r9,
            gsu_r13,
            gsu_r14,
            gsu_r15,
        );
    }

    #[cfg(not(feature = "runtime-debug-flags"))]
    #[inline(always)]
    pub(super) fn trace_superfx_cache_upload(&self, _addr: u16, _value: u8) {}

    #[cfg(feature = "runtime-debug-flags")]
    #[cold]
    #[inline(never)]
    pub(super) fn trace_superfx_cache_upload(&self, addr: u16, value: u8) {
        static ENABLED: OnceLock<bool> = OnceLock::new();
        if !*ENABLED.get_or_init(|| std::env::var_os("TRACE_SUPERFX_CACHE_UPLOAD").is_some()) {
            return;
        }
        if self.mapper_type != crate::cartridge::MapperType::SuperFx {
            return;
        }

        static COUNT: AtomicU32 = AtomicU32::new(0);
        let n = COUNT.fetch_add(1, Ordering::Relaxed);
        if n >= 4096 {
            return;
        }

        println!(
            "[SFX-CACHE-UPLOAD] frame={} sl={} cyc={} pc={:06X} exec_pc={:06X} addr={:04X} val={:02X}",
            self.ppu.get_frame(),
            self.ppu.scanline,
            self.ppu.get_cycle(),
            self.last_cpu_pc,
            self.last_cpu_exec_pc,
            addr,
            value,
        );
    }
}
