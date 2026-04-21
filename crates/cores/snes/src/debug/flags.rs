#![allow(dead_code)]

use std::sync::OnceLock;

#[cold]
#[inline(never)]
fn env_flag(key: &str, default: bool) -> bool {
    #[cfg(not(feature = "runtime-debug-flags"))]
    {
        let _ = key;
        default
    }
    #[cfg(feature = "runtime-debug-flags")]
    {
        std::env::var(key)
            .map(|v| matches!(v.as_str(), "1" | "true" | "TRUE" | "on" | "ON"))
            .unwrap_or(default)
    }
}

#[cold]
#[inline(never)]
fn env_present(key: &str) -> bool {
    #[cfg(not(feature = "runtime-debug-flags"))]
    {
        let _ = key;
        false
    }
    #[cfg(feature = "runtime-debug-flags")]
    {
        std::env::var_os(key).is_some()
    }
}

#[cold]
#[inline(never)]
fn env_u16(key: &str, default: u16) -> u16 {
    #[cfg(not(feature = "runtime-debug-flags"))]
    {
        let _ = key;
        default
    }
    #[cfg(feature = "runtime-debug-flags")]
    {
        std::env::var(key)
            .ok()
            .and_then(|v| v.parse::<u16>().ok())
            .unwrap_or(default)
    }
}

#[cold]
#[inline(never)]
fn env_u32(key: &str, default: u32) -> u32 {
    #[cfg(not(feature = "runtime-debug-flags"))]
    {
        let _ = key;
        default
    }
    #[cfg(feature = "runtime-debug-flags")]
    {
        std::env::var(key)
            .ok()
            .and_then(|v| v.parse::<u32>().ok())
            .unwrap_or(default)
    }
}

#[cold]
#[inline(never)]
fn env_u8_opt(key: &str) -> Option<u8> {
    #[cfg(not(feature = "runtime-debug-flags"))]
    {
        let _ = key;
        None
    }
    #[cfg(feature = "runtime-debug-flags")]
    {
        std::env::var(key).ok().and_then(|v| {
            let v = v.trim();
            u8::from_str_radix(v.trim_start_matches("0x"), 16)
                .ok()
                .or_else(|| v.parse().ok())
        })
    }
}

// --- Mode 7 z-rank tunables (i16 semantics but parsed as u16 then clamped) ---
#[cold]
#[inline(never)]
fn env_i16(key: &str, default: i16) -> i16 {
    #[cfg(not(feature = "runtime-debug-flags"))]
    {
        let _ = key;
        default
    }
    #[cfg(feature = "runtime-debug-flags")]
    {
        std::env::var(key)
            .ok()
            .and_then(|v| v.parse::<i32>().ok())
            .map(|n| n.clamp(i16::MIN as i32, i16::MAX as i32) as i16)
            .unwrap_or(default)
    }
}

// ============================================================
// Macros for repetitive flag definitions
// ============================================================

/// Boolean flag backed by `env_flag(key, default)`.
macro_rules! debug_flag {
    ($fn_name:ident, $env_key:literal, $default:expr) => {
        #[cfg(not(feature = "runtime-debug-flags"))]
        #[inline(always)]
        pub fn $fn_name() -> bool {
            $default
        }

        #[cfg(feature = "runtime-debug-flags")]
        pub fn $fn_name() -> bool {
            static ON: OnceLock<bool> = OnceLock::new();
            *ON.get_or_init(|| env_flag($env_key, $default))
        }
    };
}

/// Boolean flag backed by `env_present(key)` (true if env var exists).
macro_rules! debug_flag_present {
    ($fn_name:ident, $env_key:literal) => {
        #[cfg(not(feature = "runtime-debug-flags"))]
        #[inline(always)]
        pub fn $fn_name() -> bool {
            false
        }

        #[cfg(feature = "runtime-debug-flags")]
        pub fn $fn_name() -> bool {
            static ON: OnceLock<bool> = OnceLock::new();
            *ON.get_or_init(|| env_present($env_key))
        }
    };
}

/// u16 tunable backed by `env_u16(key, default)`.
macro_rules! debug_u16 {
    ($fn_name:ident, $env_key:literal, $default:expr) => {
        #[cfg(not(feature = "runtime-debug-flags"))]
        #[inline(always)]
        pub fn $fn_name() -> u16 {
            $default
        }

        #[cfg(feature = "runtime-debug-flags")]
        pub fn $fn_name() -> u16 {
            static V: OnceLock<u16> = OnceLock::new();
            *V.get_or_init(|| env_u16($env_key, $default))
        }
    };
}

/// u32 tunable backed by `env_u32(key, default)`.
macro_rules! debug_u32 {
    ($fn_name:ident, $env_key:literal, $default:expr) => {
        #[cfg(not(feature = "runtime-debug-flags"))]
        #[inline(always)]
        pub fn $fn_name() -> u32 {
            $default
        }

        #[cfg(feature = "runtime-debug-flags")]
        pub fn $fn_name() -> u32 {
            static V: OnceLock<u32> = OnceLock::new();
            *V.get_or_init(|| env_u32($env_key, $default))
        }
    };
}

/// Optional u8 backed by `env_u8_opt(key)`.
macro_rules! debug_u8_opt {
    ($fn_name:ident, $env_key:literal) => {
        #[cfg(not(feature = "runtime-debug-flags"))]
        #[inline(always)]
        pub fn $fn_name() -> Option<u8> {
            None
        }

        #[cfg(feature = "runtime-debug-flags")]
        pub fn $fn_name() -> Option<u8> {
            static VAL: OnceLock<Option<u8>> = OnceLock::new();
            *VAL.get_or_init(|| env_u8_opt($env_key))
        }
    };
}

/// i16 tunable backed by `env_i16(key, default)`.
macro_rules! debug_i16 {
    ($fn_name:ident, $env_key:literal, $default:expr) => {
        #[cfg(not(feature = "runtime-debug-flags"))]
        #[inline(always)]
        pub fn $fn_name() -> i16 {
            $default
        }

        #[cfg(feature = "runtime-debug-flags")]
        pub fn $fn_name() -> i16 {
            static V: OnceLock<i16> = OnceLock::new();
            *V.get_or_init(|| env_i16($env_key, $default))
        }
    };
}

// ============================================================
// env_flag-backed boolean flags
// ============================================================

debug_flag!(dma, "DEBUG_DMA", false);
debug_flag!(dma_reg, "DEBUG_DMA_REG", false);
debug_flag_present!(trace_starfox_boot, "TRACE_STARFOX_BOOT");
debug_flag!(trace_apu_port_all, "TRACE_APU_PORT_ALL", false);
debug_flag!(trace_apu_port0, "TRACE_APU_PORT0", false);
debug_flag!(cpu_test_hle, "CPUTEST_HLE", false);
debug_flag!(cpu_test_hle_strict_vblank, "CPUTEST_HLE_STRICT_VBL", false);
debug_flag!(cpu_test_hle_force, "CPUTEST_HLE_FORCE", false);
debug_flag!(mapper, "DEBUG_MAPPER", false);
debug_flag!(ppu_write, "DEBUG_PPU_WRITE", false);
debug_flag!(boot_verbose, "DEBUG_BOOT", false);
debug_flag!(cgram_dma, "DEBUG_CGRAM_DMA", false);
debug_flag!(graphics_dma_verbose, "DEBUG_GRAPHICS_DMA", false);
debug_flag!(headless, "HEADLESS", false);
debug_flag!(trace_jsl, "TRACE_JSL", false);
debug_flag!(trace_rtl, "TRACE_RTL", false);
debug_flag!(sa1_force_irq_each_frame, "SA1_FORCE_IRQ_EACH_FRAME", false);
debug_flag!(sa1_force_irq_once, "SA1_FORCE_IRQ_ONCE", false);
debug_flag!(trace_sa1_wait, "TRACE_SA1_WAIT", false);
debug_flag!(apu_handshake_plus, "APU_HANDSHAKE_PLUS", false);
debug_flag!(trace_apu_handshake, "TRACE_APU_HANDSHAKE", false);
debug_flag!(trace_burnin_obj, "TRACE_BURNIN_OBJ", false);
debug_flag!(trace_burnin_obj_checks, "TRACE_BURNIN_OBJ_CHECKS", false);
// Keep S-CPU SlowROM opcode wait states on by default; set ENABLE_MEM_TIMING=0
// only when bisecting older timing behavior.
debug_flag!(mem_timing, "ENABLE_MEM_TIMING", true);
debug_flag!(compat, "DEBUG_COMPAT", false);
debug_flag!(trace_sa1_ccdma, "TRACE_SA1_CCDMA", false);
debug_flag!(trace_sa1_bwram_guard, "TRACE_SA1_BWRAM_GUARD", false);
debug_flag!(trace_sa1_iram_guard, "TRACE_SA1_IRAM_GUARD", false);
debug_flag!(trace_sa1_dma, "TRACE_SA1_DMA", false);
debug_flag!(trace_ppu_inidisp, "TRACE_PPU_INIDISP", false);
debug_flag!(trace_ppu_tm, "TRACE_PPU_TM", false);
debug_flag!(trace_ppu_scroll, "TRACE_PPU_SCROLL", false);
debug_flag!(trace_burnin_v224, "TRACE_BURNIN_V224", false);
debug_flag!(trace_burnin_ext_latch, "TRACE_BURNIN_EXT_LATCH", false);
debug_flag!(force_display, "FORCE_DISPLAY", false);
debug_flag!(ignore_inidisp_cpu, "IGNORE_INIDISP_CPU", false);
debug_flag!(block_inidisp_dma, "BLOCK_INIDISP_DMA", false);
debug_flag!(strict_ppu_timing, "STRICT_PPU_TIMING", false);
debug_flag!(dma_zero_is_zero, "DMA_ZERO_IS_ZERO", false);
debug_flag!(m7_mul_full16, "M7_MUL_FULL16", false);
debug_flag!(render_metrics, "DEBUG_RENDER_METRICS", false);
debug_flag!(timing_rejects, "DEBUG_TIMING_REJECTS", false);
debug_flag!(oam_gap_in_vblank, "OAM_GAP_IN_VBLANK", false);
debug_flag!(debug_reset_area, "DEBUG_RESET_AREA", false);
debug_flag!(debug_cgram_read, "DEBUG_CGRAM_READ", false);
debug_flag!(debug_bg_pixel, "DEBUG_BG_PIXEL", false);
debug_flag!(debug_render_dot, "DEBUG_RENDER_DOT", false);
debug_flag!(debug_suspicious_tile, "DEBUG_SUSPICIOUS_TILE", false);
debug_flag!(debug_stack_read, "DEBUG_STACK_READ", false);
debug_flag!(debug_pixel_found, "DEBUG_PIXEL_FOUND", false);
debug_flag!(debug_graphics_detected, "DEBUG_GRAPHICS_DETECTED", false);
debug_flag!(compat_rts_as_rtl_8d7f, "COMPAT_RTS_AS_RTL_8D7F", false);
debug_flag!(compat_mario_rts_as_rtl, "COMPAT_MARIO_RTS_AS_RTL", false);
debug_flag!(compat_mario_rts_fix, "COMPAT_MARIO_RTS_FIX", false);
debug_flag!(jmp8cbe_to_jsr, "JMP8CBE_TO_JSR", false);
debug_flag!(smw_force_bbaa, "SMW_FORCE_BBAA", false);

// ============================================================
// env_present-backed boolean flags
// ============================================================

debug_flag_present!(trace_vectors, "TRACE_VECTORS");
debug_flag_present!(trace_4212, "TRACE_4212");
debug_flag_present!(trace_sfr, "TRACE_SFR");
debug_flag_present!(trace_sfr_values, "TRACE_SFR_VALUES");
debug_flag_present!(trace_sfr_val, "TRACE_SFR_VAL");
debug_flag_present!(trace_mode7_regs, "TRACE_MODE7_REGS");
debug_flag_present!(trace_m7_scanline, "TRACE_M7_SCANLINE");
debug_flag_present!(trace_hdma_window, "TRACE_HDMA_WINDOW");
debug_flag_present!(trace_hdma_all, "TRACE_HDMA_ALL");
debug_flag_present!(force_no_blank, "FORCE_NO_BLANK");
debug_flag_present!(trace_bwram_sys, "TRACE_BWRAM_SYS");
debug_flag_present!(trace_sa1_reg, "TRACE_SA1_REG");
debug_flag_present!(trace_sa1_irq, "TRACE_SA1_IRQ");
debug_flag_present!(trace_burnin_dma_memory, "TRACE_BURNIN_DMA_MEMORY");
debug_flag_present!(trace_burnin_status, "TRACE_BURNIN_STATUS");
debug_flag_present!(trace_burnin_apu_status, "TRACE_BURNIN_APU_STATUS");
debug_flag_present!(trace_burnin_apu_prog, "TRACE_BURNIN_APU_PROG");
debug_flag_present!(trace_apu_port, "TRACE_APU_PORT");
debug_flag_present!(trace_ipl_xfer, "TRACE_IPL_XFER");
debug_flag_present!(trace_stack_write, "TRACE_STACK_WRITE");
debug_flag_present!(debug_stack_trace, "DEBUG_STACK_TRACE");
debug_flag_present!(trace_burnin_zp16, "TRACE_BURNIN_ZP16");
debug_flag_present!(trace_zp, "TRACE_ZP");
debug_flag_present!(trace_burnin_apu_cpu, "TRACE_BURNIN_APU_CPU");
debug_flag_present!(trace_burnin_apu_writes, "TRACE_BURNIN_APU_WRITES");
debug_flag_present!(trace_burnin_apu_check, "TRACE_BURNIN_APU_CHECK");
debug_flag_present!(trace_burnin_apu_f1, "TRACE_BURNIN_APU_F1");
debug_flag_present!(trace_burnin_apu_port1, "TRACE_BURNIN_APU_PORT1");
debug_flag_present!(trace_wram_stack_dma, "TRACE_WRAM_STACK_DMA");
debug_flag_present!(trace_wram_addr, "TRACE_WRAM_ADDR");
debug_flag_present!(trace_dma_reg_pc, "TRACE_DMA_REG_PC");
debug_flag_present!(trace_dma_addr, "TRACE_DMA_ADDR");
debug_flag_present!(trace_handshake, "TRACE_HANDSHAKE");
debug_flag_present!(trace_p_change, "TRACE_P_CHANGE");
debug_flag_present!(trace_irq, "TRACE_IRQ");
debug_flag_present!(trace_wai, "TRACE_WAI");
debug_flag_present!(watch_pc_flow, "WATCH_PC_FLOW");
debug_flag_present!(trace_cpu_suspicious_flow, "TRACE_CPU_SUSPICIOUS_FLOW");
debug_flag_present!(trace_smw_apu_loop, "TRACE_SMW_APU_LOOP");
debug_flag_present!(trace_brk, "TRACE_BRK");
debug_flag_present!(trace_sfs_smp_dp_i_read, "TRACE_SFS_SMP_DP_I_READ");
debug_flag_present!(trace_sfs_smp_write_dp, "TRACE_SFS_SMP_WRITE_DP");
debug_flag_present!(trace_apu_smp_pc, "TRACE_APU_SMP_PC");
debug_flag_present!(trace_apu_port_once, "TRACE_APU_PORT_ONCE");
debug_flag_present!(trace_sfs_apu_wait, "TRACE_SFS_APU_WAIT");
debug_flag_present!(trace_sfs_apu_wait_dump, "TRACE_SFS_APU_WAIT_DUMP");
debug_flag_present!(trace_sfs_apu_mismatch, "TRACE_SFS_APU_MISMATCH");
debug_flag_present!(trace_dma_reg, "TRACE_DMA_REG");

// cpu/core.rs instruction handler flags
debug_flag_present!(debug_fetch_pc, "DEBUG_FETCH_PC");
debug_flag_present!(trace_stack_guard, "TRACE_STACK_GUARD");
debug_flag_present!(debug_bit4210, "DEBUG_BIT4210");
debug_flag_present!(debug_bit4210_all, "DEBUG_BIT4210_ALL");
debug_flag_present!(exit_on_bit82, "EXIT_ON_BIT82");
debug_flag_present!(debug_branch, "DEBUG_BRANCH");
debug_flag_present!(exit_on_branch_neg, "EXIT_ON_BRANCH_NEG");
debug_flag_present!(trace_jsr_stack, "TRACE_JSR_STACK");
debug_flag_present!(trace_pb_calls, "TRACE_PB_CALLS");
debug_flag_present!(trace_rts_detail, "TRACE_RTS_DETAIL");
debug_flag_present!(trace_rts_pop, "TRACE_RTS_POP");
debug_flag_present!(trace_rts_addr, "TRACE_RTS_ADDR");
debug_flag_present!(trace_mflag, "TRACE_MFLAG");
debug_flag_present!(trace_sp_change, "TRACE_SP_CHANGE");
debug_flag_present!(trace_sta_long, "TRACE_STA_LONG");
debug_flag_present!(trace_nmi_take, "TRACE_NMI_TAKE");

// bus/mod.rs hot path flags
debug_flag_present!(trace_apu_u16, "TRACE_APU_U16");
debug_flag_present!(rdnmi_always_82, "RDNMI_ALWAYS_82");
debug_flag_present!(rdnmi_force_bitloop, "RDNMI_FORCE_BITLOOP");
debug_flag_present!(force_4210_once, "FORCE_4210_ONCE");
debug_flag_present!(cputest_force_82, "CPUTEST_FORCE_82");
debug_flag_present!(force_nmi_flag, "FORCE_NMI_FLAG");
debug_flag_present!(force_rdnmi_once, "FORCE_RDNMI_ONCE");
debug_flag_present!(rdnmi_force_on, "RDNMI_FORCE_ON");
debug_flag_present!(rdnmi_force_vbl, "RDNMI_FORCE_VBL");
debug_flag_present!(rdnmi_sticky, "RDNMI_STICKY");
debug_flag_present!(trace_4212_values, "TRACE_4212_VALUES");
debug_flag_present!(debug_joybusy, "DEBUG_JOYBUSY");
debug_flag_present!(force_mdma_now, "FORCE_MDMA_NOW");
debug_flag_present!(trace_hdma_enable, "TRACE_HDMA_ENABLE");
debug_flag_present!(trace_vblank, "TRACE_VBLANK");
debug_flag_present!(trace_vblank_pc, "TRACE_VBLANK_PC");
debug_flag_present!(trace_oam_reset, "TRACE_OAM_RESET");
debug_flag_present!(trace_autojoy, "TRACE_AUTOJOY");
debug_flag_present!(dma_probe, "DMA_PROBE");
debug_flag_present!(trace_dma_dest, "TRACE_DMA_DEST");
debug_flag_present!(trace_oam_dma, "TRACE_OAM_DMA");
debug_flag_present!(trace_dma_setup_once, "TRACE_DMA_SETUP_ONCE");
debug_flag_present!(trace_smw_apu_hle, "TRACE_SMW_APU_HLE");
debug_flag_present!(trace_burnin_ext_flow, "TRACE_BURNIN_EXT_FLOW");
debug_flag_present!(trace_4210, "TRACE_4210");
debug_flag_present!(debug_sa1_scheduler, "DEBUG_SA1_SCHEDULER");
debug_flag_present!(trace_sdd1, "TRACE_SDD1");

// audio/spc/apu.rs I/O handler flags
debug_flag_present!(trace_burnin_smp_timer, "TRACE_BURNIN_SMP_TIMER");
debug_flag_present!(trace_sfs_apu_f4_read, "TRACE_SFS_APU_F4_READ");
debug_flag_present!(trace_burnin_apu_f4f7_reads, "TRACE_BURNIN_APU_F4F7_READS");
debug_flag_present!(trace_burnin_apu_f0_writes, "TRACE_BURNIN_APU_F0_WRITES");
debug_flag_present!(trace_burnin_apu_f1_writes, "TRACE_BURNIN_APU_F1_WRITES");
debug_flag_present!(trace_burnin_apu_f4_writes, "TRACE_BURNIN_APU_F4_WRITES");
debug_flag_present!(trace_burnin_apu_f5_writes, "TRACE_BURNIN_APU_F5_WRITES");
debug_flag_present!(trace_burnin_apu_dump_09e7, "TRACE_BURNIN_APU_DUMP_09E7");
debug_flag_present!(trace_sfs_apu_var81, "TRACE_SFS_APU_VAR81");
debug_flag_present!(trace_sfs_apu_var14, "TRACE_SFS_APU_VAR14");
debug_flag_present!(trace_burnin_apu_var2a, "TRACE_BURNIN_APU_VAR2A");
debug_flag_present!(trace_apu_bootstate, "TRACE_APU_BOOTSTATE");
debug_flag_present!(trace_apu_boot, "TRACE_APU_BOOT");
debug_flag_present!(trace_top_apu_diag, "TRACE_TOP_APU_DIAG");
debug_flag_present!(trace_top_spc_cmd, "TRACE_TOP_SPC_CMD");

// ppu/registers.rs flags
debug_flag_present!(trace_inidisp_cpu, "TRACE_INIDISP_CPU");
debug_flag_present!(debug_inidisp_dma, "DEBUG_INIDISP_DMA");
debug_flag_present!(force_max_brightness, "FORCE_MAX_BRIGHTNESS");

// ============================================================
// u16 tunables
// ============================================================

debug_u16!(vmadd_ctrl_head, "VMADD_CTRL_HEAD", 2);
debug_u16!(vmadd_ctrl_tail, "VMADD_CTRL_TAIL", 2);
debug_u16!(cgadd_ctrl_head, "CGADD_CTRL_HEAD", 2);
debug_u16!(cgadd_ctrl_tail, "CGADD_CTRL_TAIL", 2);
debug_u16!(cgadd_effect_delay_dots, "CGADD_EFFECT_DELAY", 1);
debug_u16!(vram_gap_after_vmain, "VRAM_DATA_GAP_AFTER_VMAIN", 2);
debug_u16!(cgram_gap_after_cgadd, "CGRAM_DATA_GAP_AFTER_CGADD", 2);
debug_u16!(vmain_effect_delay_dots, "VMAIN_EFFECT_DELAY", 1);
debug_u16!(hblank_hdma_guard_dots, "HBLANK_HDMA_GUARD", 6);
debug_u16!(vram_hdma_head, "VRAM_HDMA_HEAD", 0);
debug_u16!(vram_hdma_tail, "VRAM_HDMA_TAIL", 2);
debug_u16!(vram_mdma_head, "VRAM_MDMA_HEAD", 6);
debug_u16!(vram_mdma_tail, "VRAM_MDMA_TAIL", 9);
debug_u16!(cgram_hdma_head, "CGRAM_HDMA_HEAD", 4);
debug_u16!(cgram_hdma_tail, "CGRAM_HDMA_TAIL", 4);
debug_u16!(cgram_mdma_head, "CGRAM_MDMA_HEAD", 4);
debug_u16!(cgram_mdma_tail, "CGRAM_MDMA_TAIL", 4);
debug_u16!(oam_hdma_head, "OAM_HDMA_HEAD", 6);
debug_u16!(oam_hdma_tail, "OAM_HDMA_TAIL", 6);
debug_u16!(oam_gap_after_oamadd, "OAM_DATA_GAP_AFTER_OAMADD", 2);
debug_u16!(vram_vblank_head, "VRAM_VBLANK_HEAD", 0);
debug_u16!(vram_vblank_tail, "VRAM_VBLANK_TAIL", 0);
debug_u16!(cgram_vblank_head, "CGRAM_VBLANK_HEAD", 0);
debug_u16!(cgram_vblank_tail, "CGRAM_VBLANK_TAIL", 0);
debug_u16!(oam_vblank_head, "OAM_VBLANK_HEAD", 0);
debug_u16!(oam_vblank_tail, "OAM_VBLANK_TAIL", 0);

// ============================================================
// u32 tunables
// ============================================================

debug_u32!(trace_apu_handshake_limit, "TRACE_APU_HANDSHAKE_LIMIT", 256);

// ============================================================
// Optional u8 flags
// ============================================================

debug_u8_opt!(debug_force_tm, "DEBUG_FORCE_TM");
debug_u8_opt!(force_4212, "FORCE_4212");

// ============================================================
// i16 tunables (Mode 7 z-rank)
// ============================================================

debug_i16!(m7_z_obj3, "M7_Z_OBJ3", 90);
debug_i16!(m7_z_obj2, "M7_Z_OBJ2", 70);
debug_i16!(m7_z_obj1, "M7_Z_OBJ1", 50);
debug_i16!(m7_z_bg1, "M7_Z_BG1", 40);
debug_i16!(m7_z_obj0, "M7_Z_OBJ0", 20);
debug_i16!(m7_z_bg2, "M7_Z_BG2", 10);

// ============================================================
// Functions with custom logic (not macro-replaceable)
// ============================================================

#[cfg(not(feature = "runtime-debug-flags"))]
#[inline(always)]
pub fn trace_cpu_pc_range() -> Option<(u64, u64)> {
    None
}

#[cfg(feature = "runtime-debug-flags")]
pub fn trace_cpu_pc_range() -> Option<(u64, u64)> {
    static RANGE: OnceLock<Option<(u64, u64)>> = OnceLock::new();
    *RANGE.get_or_init(|| {
        let value = std::env::var("TRACE_CPU_PC_RANGE").ok()?;
        let (start, end) = value.split_once('-')?;
        Some((start.parse().ok()?, end.parse().ok()?))
    })
}

#[cfg(not(feature = "runtime-debug-flags"))]
#[inline(always)]
pub fn trace_scroll_frame() -> Option<u64> {
    None
}

#[cfg(feature = "runtime-debug-flags")]
pub fn trace_scroll_frame() -> Option<u64> {
    static FRAME: OnceLock<Option<u64>> = OnceLock::new();
    *FRAME.get_or_init(|| {
        std::env::var("TRACE_SCROLL_FRAME")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
    })
}

// Force APU port0/1 to fixed values (HLE debug: APU_PORT0_VAL/APU_PORT1_VAL)
pub fn apu_force_port0() -> Option<u8> {
    static VAL: OnceLock<Option<u8>> = OnceLock::new();
    *VAL.get_or_init(|| {
        std::env::var("APU_FORCE_PORT0").ok().and_then(|v| {
            u8::from_str_radix(v.trim_start_matches("0x"), 16)
                .ok()
                .or_else(|| v.parse().ok())
        })
    })
}

pub fn apu_force_port1() -> Option<u8> {
    static VAL: OnceLock<Option<u8>> = OnceLock::new();
    *VAL.get_or_init(|| {
        std::env::var("APU_FORCE_PORT1").ok().and_then(|v| {
            u8::from_str_radix(v.trim_start_matches("0x"), 16)
                .ok()
                .or_else(|| v.parse().ok())
        })
    })
}

// Force S-CPU data bank (DB) to a fixed value (debug: FORCE_DB=0x7E etc.)
pub fn force_db() -> Option<u8> {
    static VAL: OnceLock<Option<u8>> = OnceLock::new();
    *VAL.get_or_init(|| {
        std::env::var("FORCE_DB").ok().and_then(|v| {
            u8::from_str_radix(v.trim_start_matches("0x"), 16)
                .ok()
                .or_else(|| v.parse().ok())
        })
    })
}

pub fn quiet() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| {
        // Treat any non-zero / non-false value as quiet.
        // (run.sh uses QUIET=1/2/3 levels, so numeric values should enable quiet mode.)
        std::env::var("QUIET")
            .map(|v| {
                let v = v.trim().to_ascii_lowercase();
                !(v.is_empty() || v == "0" || v == "false" || v == "off")
            })
            .unwrap_or(false)
    })
}

// Extra-chatter for rendering/first frames. Alias of DEBUG_BOOT for now,
// but kept separate in case we want finer control later.
pub fn render_verbose() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| env_flag("DEBUG_RENDER", false) || env_flag("DEBUG_BOOT", false))
}

// CPU trace / verbose instruction logs (very noisy)
pub fn trace() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| env_flag("DEBUG_TRACE", false) || env_flag("DEBUG_BOOT", false))
}

#[allow(dead_code)]
pub fn trace_sa1_regs() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| env_flag("TRACE_SA1_REGS", false) || env_flag("TRACE_SA1_REG", false))
}

// Watch S-CPU PC for specific addresses (comma-separated hex, bank:addr or addr)
// Returns a sorted slice for binary_search at call sites.
pub fn watch_pc_list() -> Option<&'static [u32]> {
    static LIST: OnceLock<Option<Vec<u32>>> = OnceLock::new();
    LIST.get_or_init(|| {
        if let Ok(val) = std::env::var("WATCH_PC") {
            let mut pcs = Vec::new();
            for part in val.split(',') {
                let p = part.trim();
                if p.is_empty() {
                    continue;
                }
                if let Some((b, a)) = p.split_once(':') {
                    if let (Ok(bank), Ok(addr)) =
                        (u8::from_str_radix(b, 16), u16::from_str_radix(a, 16))
                    {
                        pcs.push(((bank as u32) << 16) | addr as u32);
                    }
                } else if let Ok(addr) = u32::from_str_radix(p, 16) {
                    pcs.push(addr);
                }
            }
            pcs.sort_unstable();
            pcs.dedup();
            Some(pcs)
        } else {
            None
        }
    })
    .as_deref()
}

// Dump CPU ring buffer when S-CPU PC hits specific addresses (env: DUMP_ON_PC)
// Accepts comma-separated hex, bank:addr or addr (same format as WATCH_PC).
pub fn dump_on_pc_list() -> Option<&'static [u32]> {
    static LIST: OnceLock<Option<Vec<u32>>> = OnceLock::new();
    LIST.get_or_init(|| {
        if let Ok(val) = std::env::var("DUMP_ON_PC") {
            let mut pcs = Vec::new();
            for part in val.split(',') {
                let p = part.trim();
                if p.is_empty() {
                    continue;
                }
                if let Some((b, a)) = p.split_once(':') {
                    if let (Ok(bank), Ok(addr)) =
                        (u8::from_str_radix(b, 16), u16::from_str_radix(a, 16))
                    {
                        pcs.push(((bank as u32) << 16) | addr as u32);
                    }
                } else if let Ok(addr) = u32::from_str_radix(p, 16) {
                    pcs.push(addr);
                }
            }
            Some(pcs)
        } else {
            None
        }
    })
    .as_deref()
}

// Dump CPU ring buffer when the fetched opcode matches (env: DUMP_ON_OPCODE=DB)
pub fn dump_on_opcode() -> Option<u8> {
    static VAL: OnceLock<Option<u8>> = OnceLock::new();
    *VAL.get_or_init(|| {
        std::env::var("DUMP_ON_OPCODE").ok().and_then(|v| {
            u8::from_str_radix(v.trim_start_matches("0x"), 16)
                .ok()
                .or_else(|| v.parse().ok())
        })
    })
}

// Filter for ring buffer dumps (env: DUMP_ON_TEST_IDX=000C or 0x000C).
pub fn dump_on_test_idx() -> Option<u16> {
    static VAL: OnceLock<Option<u16>> = OnceLock::new();
    *VAL.get_or_init(|| {
        std::env::var("DUMP_ON_TEST_IDX").ok().and_then(|v| {
            u16::from_str_radix(v.trim_start_matches("0x"), 16)
                .ok()
                .or_else(|| v.parse().ok())
        })
    })
}

// Watch SA-1 PC (env: WATCH_SA1_PC, comma separated list, bank:addr or addr)
pub fn watch_sa1_pc_list() -> Option<Vec<u32>> {
    static LIST: OnceLock<Option<Vec<u32>>> = OnceLock::new();
    LIST.get_or_init(|| {
        if let Ok(val) = std::env::var("WATCH_SA1_PC") {
            let mut pcs = Vec::new();
            for part in val.split(',') {
                let p = part.trim();
                if p.is_empty() {
                    continue;
                }
                if let Some((b, a)) = p.split_once(':') {
                    if let (Ok(bank), Ok(addr)) =
                        (u8::from_str_radix(b, 16), u16::from_str_radix(a, 16))
                    {
                        pcs.push(((bank as u32) << 16) | addr as u32);
                    }
                } else if let Ok(addr) = u32::from_str_radix(p, 16) {
                    pcs.push(addr);
                }
            }
            Some(pcs)
        } else {
            None
        }
    })
    .clone() // Note: Vec is not Copy, so clone is necessary here
}

// Trace the first N SA-1 steps (env TRACE_SA1_STEPS=N)
pub fn trace_sa1_steps() -> Option<usize> {
    static VAL: OnceLock<Option<usize>> = OnceLock::new();
    *VAL.get_or_init(|| {
        std::env::var("TRACE_SA1_STEPS")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
    })
}

// Trace first N S-CPU instructions (env TRACE_PC_STEPS=N)
pub fn trace_pc_steps() -> Option<usize> {
    static VAL: OnceLock<Option<usize>> = OnceLock::new();
    *VAL.get_or_init(|| {
        std::env::var("TRACE_PC_STEPS")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
    })
}

// TRACE_PC_FILE: PCトレースを指定ファイルへ出力（標準出力がノイジーな場合の代替）
pub fn trace_pc_file() -> Option<String> {
    static VAL: OnceLock<Option<String>> = OnceLock::new();
    VAL.get_or_init(|| std::env::var("TRACE_PC_FILE").ok())
        .clone() // Note: String is not Copy, so clone is necessary here
}

// Trace SA-1 PC for first N instructions after a forced IRQ (env TRACE_SA1_WAKE_STEPS=N)
pub fn trace_sa1_wake_steps() -> Option<usize> {
    static VAL: OnceLock<Option<usize>> = OnceLock::new();
    *VAL.get_or_init(|| {
        std::env::var("TRACE_SA1_WAKE_STEPS")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
    })
}

// Watch a single WRAM/BWRAM address on S-CPU side (env WATCH_ADDR like "7F:7DC0")
pub fn watch_addr() -> Option<u32> {
    static VAL: OnceLock<Option<u32>> = OnceLock::new();
    *VAL.get_or_init(|| {
        std::env::var("WATCH_ADDR").ok().and_then(|s| {
            if let Some((b, a)) = s.split_once(':') {
                let bank = u8::from_str_radix(b, 16).ok()?;
                let addr = u16::from_str_radix(a, 16).ok()?;
                Some(((bank as u32) << 16) | addr as u32)
            } else {
                u32::from_str_radix(&s, 16).ok()
            }
        })
    })
}

// Watch a single S-CPU write address (env WATCH_ADDR_W)
pub fn watch_addr_write() -> Option<u32> {
    static VAL: OnceLock<Option<u32>> = OnceLock::new();
    *VAL.get_or_init(|| {
        std::env::var("WATCH_ADDR_W").ok().and_then(|s| {
            if let Some((b, a)) = s.split_once(':') {
                let bank = u8::from_str_radix(b, 16).ok()?;
                let addr = u16::from_str_radix(a, 16).ok()?;
                Some(((bank as u32) << 16) | addr as u32)
            } else {
                u32::from_str_radix(&s, 16).ok()
            }
        })
    })
}

// Watch a single S-CPU read address (env WATCH_ADDR_R)
pub fn watch_addr_read() -> Option<u32> {
    static VAL: OnceLock<Option<u32>> = OnceLock::new();
    *VAL.get_or_init(|| {
        std::env::var("WATCH_ADDR_R").ok().and_then(|s| {
            if let Some((b, a)) = s.split_once(':') {
                let bank = u8::from_str_radix(b, 16).ok()?;
                let addr = u16::from_str_radix(a, 16).ok()?;
                Some(((bank as u32) << 16) | addr as u32)
            } else {
                u32::from_str_radix(&s, 16).ok()
            }
        })
    })
}

// WATCH_WRAM_W: watch WRAM writes (7E/7F banks) with simple logging
pub fn watch_wram_write() -> Option<u32> {
    static VAL: OnceLock<Option<u32>> = OnceLock::new();
    *VAL.get_or_init(|| {
        std::env::var("WATCH_WRAM_W").ok().and_then(|s| {
            if let Some((b, a)) = s.split_once(':') {
                let bank = u8::from_str_radix(b, 16).ok()?;
                let addr = u16::from_str_radix(a, 16).ok()?;
                Some(((bank as u32) << 16) | addr as u32)
            } else {
                u32::from_str_radix(&s, 16).ok()
            }
        })
    })
}

// WATCH_WRAM_W_FORCE: 指定アドレスへの書き込みを強制値に置き換える（デバッグ用）
// 形式: WATCH_WRAM_W_FORCE=7E:E95C:01
pub fn watch_wram_write_force() -> Option<(u32, u8)> {
    static VAL: OnceLock<Option<(u32, u8)>> = OnceLock::new();
    *VAL.get_or_init(|| {
        std::env::var("WATCH_WRAM_W_FORCE").ok().and_then(|s| {
            let mut parts = s.split(':');
            let b = parts.next()?;
            let a = parts.next()?;
            let v = parts.next()?;
            let bank = u8::from_str_radix(b, 16).ok()?;
            let addr = u16::from_str_radix(a, 16).ok()?;
            let val = u8::from_str_radix(v, 16).ok()?;
            Some((((bank as u32) << 16) | addr as u32, val))
        })
    })
}

// Force S-CPU IRQ for first N frames (env FORCE_SCPU_IRQ_FRAMES=N)
pub fn force_scpu_irq_frames() -> Option<u32> {
    static VAL: OnceLock<Option<u32>> = OnceLock::new();
    *VAL.get_or_init(|| {
        std::env::var("FORCE_SCPU_IRQ_FRAMES")
            .ok()
            .and_then(|v| v.parse::<u32>().ok())
    })
}

// Mode7: force product value for $2134-2136 (hex up to 6 hex digits)
pub fn force_m7_product() -> Option<u32> {
    static VAL: OnceLock<Option<u32>> = OnceLock::new();
    *VAL.get_or_init(|| {
        std::env::var("FORCE_M7_PRODUCT")
            .ok()
            .and_then(|s| u32::from_str_radix(s.trim_start_matches("0x"), 16).ok())
            .map(|v| v & 0x00FF_FFFF)
    })
}

// Priority model variant: 0 = legacy ad-hoc, 1 = unified z-rank
pub fn priority_model_variant() -> u8 {
    use std::sync::OnceLock;
    fn env_u8(key: &str, default: u8) -> u8 {
        std::env::var(key)
            .ok()
            .and_then(|v| v.parse::<u8>().ok())
            .unwrap_or(default)
    }
    static V: OnceLock<u8> = OnceLock::new();
    *V.get_or_init(|| env_u8("PRIORITY_MODEL", 1))
}
