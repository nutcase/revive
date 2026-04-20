use super::Bus;

/// Cached env-var flag: returns `true` when the env var is set (`.is_ok()`).
macro_rules! env_bool {
    ($name:ident, $var:expr) => {
        #[inline]
        pub(crate) fn $name() -> bool {
            use std::sync::OnceLock;
            static V: OnceLock<bool> = OnceLock::new();
            *V.get_or_init(|| std::env::var($var).is_ok())
        }
    };
}

/// Cached env-var flag: returns `true` only when the env var is set to `"1"`.
macro_rules! env_bool_eq1 {
    ($name:ident, $var:expr) => {
        #[inline]
        pub(crate) fn $name() -> bool {
            use std::sync::OnceLock;
            static V: OnceLock<bool> = OnceLock::new();
            *V.get_or_init(|| matches!(std::env::var($var), Ok(v) if v == "1"))
        }
    };
}

/// Cached env-var parsed as `i32` (decimal) with a default.
macro_rules! env_i32 {
    ($name:ident, $var:expr, $default:expr) => {
        #[inline]
        pub(crate) fn $name() -> i32 {
            use std::sync::OnceLock;
            static V: OnceLock<i32> = OnceLock::new();
            *V.get_or_init(|| {
                std::env::var($var)
                    .ok()
                    .and_then(|s| s.parse::<i32>().ok())
                    .unwrap_or($default)
            })
        }
    };
}

/// Cached env-var parsed as `Option<usize>` (with optional `> 0` filter).
macro_rules! env_option_usize {
    ($name:ident, $var:expr) => {
        pub(crate) fn $name() -> Option<usize> {
            use std::sync::OnceLock;
            static V: OnceLock<Option<usize>> = OnceLock::new();
            *V.get_or_init(|| {
                std::env::var($var)
                    .ok()
                    .and_then(|s| s.parse::<usize>().ok())
            })
        }
    };
    ($name:ident, $var:expr, nonzero) => {
        pub(crate) fn $name() -> Option<usize> {
            use std::sync::OnceLock;
            static V: OnceLock<Option<usize>> = OnceLock::new();
            *V.get_or_init(|| {
                std::env::var($var)
                    .ok()
                    .and_then(|s| s.parse::<usize>().ok())
                    .filter(|&v| v > 0)
            })
        }
    };
}

impl Bus {
    env_bool!(env_force_mpr1_hardware, "PCE_FORCE_MPR1_HW");
    env_bool!(env_force_display_on, "PCE_FORCE_DISPLAY_ON");
    env_bool!(env_fold_io_02xx, "PCE_FOLD_IO_02XX");
    env_bool!(env_force_test_palette, "PCE_FORCE_TEST_PALETTE");
    env_bool!(env_vce_catchall, "PCE_VCE_CATCHALL");
    env_bool!(env_extreme_mirror, "PCE_VDC_EXTREME_MIRROR");
    env_bool!(env_vdc_ultra_mirror, "PCE_VDC_ULTRA_MIRROR");
    env_bool!(env_vdc_catchall, "PCE_VDC_CATCHALL");
    env_bool!(env_timer_default_start, "PCE_TIMER_DEFAULT_START");
    env_bool!(env_force_palette_every_frame, "PCE_FORCE_PALETTE");
    env_bool!(env_bg_bit_lsb, "PCE_BG_BIT_LSB");
    env_bool!(env_bg_swap_words, "PCE_BG_SWAP_WORDS");
    env_bool!(env_bg_swap_bytes, "PCE_BG_SWAP_BYTES");
    env_bool!(env_bg_plane_major, "PCE_BG_PLANE_MAJOR");
    env_bool!(env_bg_tile12, "PCE_BG_TILE12");
    env_bool!(env_bg_force_chr0_only, "PCE_BG_CHR0_ONLY");
    env_bool!(env_bg_force_chr1_only, "PCE_BG_CHR1_ONLY");
    env_bool!(env_bg_row_words, "PCE_BG_ROW_WORDS");
    env_bool!(env_bg_force_tile0_zero, "PCE_BG_TILE0_ZERO");
    env_bool!(env_bg_palette_zero_visible, "PCE_BG_PAL0_VISIBLE");
    env_bool!(env_sprite_reverse_priority, "PCE_SPR_REVERSE_PRIORITY");
    env_bool!(env_no_sprite_line_limit, "PCE_NO_SPR_LINE_LIMIT");
    env_bool!(env_sprite_pattern_raw_index, "PCE_SPR_PATTERN_RAW");
    env_bool!(env_sprite_row_interleaved, "PCE_SPR_ROW_INTERLEAVED");
    env_bool!(env_force_timer, "PCE_FORCE_TIMER");
    env_bool!(env_force_vdc_dsdv, "PCE_FORCE_VDC_DSDV");
    env_bool!(env_force_irq1, "PCE_FORCE_IRQ1");
    env_bool!(env_force_irq2, "PCE_FORCE_IRQ2");
    env_bool!(env_debug_bg_only, "PCE_DEBUG_BG_ONLY");
    env_bool!(env_debug_spr_only, "PCE_DEBUG_SPR_ONLY");
    env_bool!(env_force_cram_from_vram, "PCE_FORCE_CRAM_FROM_VRAM");

    #[cfg(feature = "trace_hw_writes")]
    env_bool!(env_trace_mpr, "PCE_TRACE_MPR");

    env_bool_eq1!(env_relax_io_mirror, "PCE_RELAX_IO_MIRROR");
    env_bool_eq1!(env_force_title_now, "PCE_FORCE_TITLE");
    env_bool_eq1!(env_vdc_force_hot_ports, "PCE_VDC_FORCE_HOT");
    env_bool_eq1!(env_force_title_scene, "PCE_FORCE_TITLE_SCENE");

    env_i32!(env_bg_y_bias, "PCE_BG_Y_BIAS", 0);

    env_option_usize!(env_bg_map_height_override, "PCE_BG_MAP_H_TILES", nonzero);
    env_option_usize!(env_bg_map_width_override, "PCE_BG_MAP_W_TILES", nonzero);
    env_option_usize!(env_sprite_max_entries, "PCE_SPR_MAX_ENTRIES");

    // Special env helpers with custom parsing (hex u8, hex-or-decimal i32)
    #[inline]
    pub(crate) fn env_pad_default() -> u8 {
        use std::sync::OnceLock;
        static V: OnceLock<u8> = OnceLock::new();
        *V.get_or_init(|| {
            std::env::var("PCE_PAD_DEFAULT")
                .ok()
                .and_then(|s| u8::from_str_radix(&s, 16).ok())
                .unwrap_or(0xFF)
        })
    }

    #[inline]
    pub(crate) fn env_irq_status_default() -> Option<u8> {
        use std::sync::OnceLock;
        static V: OnceLock<Option<u8>> = OnceLock::new();
        *V.get_or_init(|| {
            std::env::var("PCE_IRQ_STATUS_DEFAULT")
                .ok()
                .and_then(|s| u8::from_str_radix(&s, 16).ok())
        })
    }

    pub(crate) fn env_bg_map_base_bias() -> i32 {
        use std::sync::OnceLock;
        static V: OnceLock<i32> = OnceLock::new();
        *V.get_or_init(|| {
            std::env::var("PCE_BG_MAP_BASE_BIAS")
                .ok()
                .and_then(|s| {
                    i32::from_str_radix(&s, 16)
                        .ok()
                        .or_else(|| s.parse::<i32>().ok())
                })
                .unwrap_or(0)
        })
    }

    pub(crate) fn env_bg_tile_base_bias() -> i32 {
        use std::sync::OnceLock;
        static V: OnceLock<i32> = OnceLock::new();
        *V.get_or_init(|| {
            std::env::var("PCE_BG_TILE_BASE_BIAS")
                .ok()
                .and_then(|s| {
                    i32::from_str_radix(&s, 16)
                        .ok()
                        .or_else(|| s.parse::<i32>().ok())
                })
                .unwrap_or(0)
        })
    }

    pub(crate) fn env_route_02xx_hw() -> bool {
        use std::sync::OnceLock;
        static FLAG: OnceLock<bool> = OnceLock::new();
        *FLAG.get_or_init(|| match std::env::var("PCE_ROUTE_02XX_HW") {
            Ok(v) if v == "0" => false,
            _ => true, // default: route 0x0200–0x021F to hardware
        })
    }
}
