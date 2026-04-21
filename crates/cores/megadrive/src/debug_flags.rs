#![allow(dead_code)]

#[cfg(feature = "runtime-debug-flags")]
use std::sync::OnceLock;

#[cfg(feature = "runtime-debug-flags")]
#[cold]
#[inline(never)]
fn env_present(key: &str) -> bool {
    std::env::var_os(key).is_some()
}

#[cfg(feature = "runtime-debug-flags")]
#[cold]
#[inline(never)]
fn env_parse<T>(key: &str, default: T) -> T
where
    T: Copy + std::str::FromStr,
{
    std::env::var(key)
        .ok()
        .and_then(|value| value.parse::<T>().ok())
        .unwrap_or(default)
}

macro_rules! debug_present {
    ($fn_name:ident, $env_key:literal) => {
        #[cfg(not(feature = "runtime-debug-flags"))]
        #[inline(always)]
        pub(crate) fn $fn_name() -> bool {
            false
        }

        #[cfg(feature = "runtime-debug-flags")]
        pub(crate) fn $fn_name() -> bool {
            static VALUE: OnceLock<bool> = OnceLock::new();
            *VALUE.get_or_init(|| env_present($env_key))
        }
    };
}

macro_rules! debug_value {
    ($fn_name:ident, $env_key:literal, $ty:ty, $default:expr) => {
        #[cfg(not(feature = "runtime-debug-flags"))]
        #[inline(always)]
        pub(crate) fn $fn_name() -> $ty {
            $default
        }

        #[cfg(feature = "runtime-debug-flags")]
        pub(crate) fn $fn_name() -> $ty {
            static VALUE: OnceLock<$ty> = OnceLock::new();
            *VALUE.get_or_init(|| env_parse::<$ty>($env_key, $default))
        }
    };
}

debug_present!(line_vram_latch, "MEGADRIVE_DEBUG_LINE_VRAM_LATCH");
debug_present!(line_latch_next, "MEGADRIVE_DEBUG_LINE_LATCH_NEXT");
debug_present!(dma_fill_no_prewrite, "MEGADRIVE_DEBUG_DMA_FILL_NO_PREWRITE");
debug_present!(dma_fill_word, "MEGADRIVE_DEBUG_DMA_FILL_WORD");
debug_present!(dma_fill_lane_no_xor, "MEGADRIVE_DEBUG_DMA_FILL_LANE_NO_XOR");
debug_present!(vdp_byte_immediate, "MEGADRIVE_DEBUG_VDP_BYTE_IMMEDIATE");

debug_present!(
    sprite_pattern_per_line,
    "MEGADRIVE_DEBUG_SPRITE_PATTERN_PER_LINE"
);
debug_present!(sprite_pattern_line0, "MEGADRIVE_DEBUG_SPRITE_PATTERN_LINE0");

debug_present!(disable_plane_a, "MEGADRIVE_DEBUG_DISABLE_PLANE_A");
debug_present!(disable_plane_b, "MEGADRIVE_DEBUG_DISABLE_PLANE_B");
debug_present!(disable_window, "MEGADRIVE_DEBUG_DISABLE_WINDOW");
debug_present!(force_window_off, "FORCE_WINDOW_OFF");
debug_present!(disable_sprites, "MEGADRIVE_DEBUG_DISABLE_SPRITES");
debug_present!(force_disable_sprites, "DISABLE_SPRITES");
debug_present!(invert_vscroll_a, "MEGADRIVE_DEBUG_VSCROLL_INVERT_A");
debug_present!(invert_vscroll_b, "MEGADRIVE_DEBUG_VSCROLL_INVERT_B");
debug_present!(vscroll_swap_ab, "MEGADRIVE_DEBUG_VSCROLL_SWAP_AB");
debug_present!(plane_paged, "MEGADRIVE_DEBUG_PLANE_PAGED");
debug_present!(plane_a_paged, "MEGADRIVE_DEBUG_PLANE_A_PAGED");
debug_present!(plane_b_paged, "MEGADRIVE_DEBUG_PLANE_B_PAGED");
debug_present!(plane_paged_xmajor, "MEGADRIVE_DEBUG_PLANE_PAGED_XMAJOR");
debug_present!(plane_a_paged_xmajor, "MEGADRIVE_DEBUG_PLANE_A_PAGED_XMAJOR");
debug_present!(plane_b_paged_xmajor, "MEGADRIVE_DEBUG_PLANE_B_PAGED_XMAJOR");
debug_present!(plane_live_vram, "MEGADRIVE_DEBUG_PLANE_LIVE_VRAM");
debug_present!(plane_line_latch, "MEGADRIVE_DEBUG_PLANE_LINE_LATCH");
debug_present!(live_cram, "MEGADRIVE_DEBUG_LIVE_CRAM");
debug_value!(line_offset, "MEGADRIVE_DEBUG_LINE_OFFSET", isize, 0);
debug_present!(bottom_bg_mask, "MEGADRIVE_DEBUG_BOTTOM_BG_MASK");
debug_present!(hscroll_live, "MEGADRIVE_DEBUG_HSCROLL_LIVE");
debug_present!(disable_64x32_paged, "MEGADRIVE_DEBUG_DISABLE_64X32_PAGED");
debug_present!(
    disable_64x32_paged_a,
    "MEGADRIVE_DEBUG_DISABLE_64X32_PAGED_A"
);
debug_present!(
    disable_64x32_paged_b,
    "MEGADRIVE_DEBUG_DISABLE_64X32_PAGED_B"
);
debug_present!(plane_a_64x32_paged, "MEGADRIVE_DEBUG_PLANE_A_64X32_PAGED");
debug_present!(plane_b_64x32_paged, "MEGADRIVE_DEBUG_PLANE_B_64X32_PAGED");
debug_present!(
    disable_comix_roll_fix,
    "MEGADRIVE_DEBUG_DISABLE_COMIX_ROLL_FIX"
);
debug_value!(comix_roll_y, "MEGADRIVE_DEBUG_COMIX_ROLL_Y", i16, 0);
debug_present!(
    disable_comix_roll_sparse_mask,
    "MEGADRIVE_DEBUG_DISABLE_COMIX_ROLL_SPARSE_MASK"
);
debug_present!(
    ignore_plane_priority,
    "MEGADRIVE_DEBUG_IGNORE_PLANE_PRIORITY"
);
debug_value!(
    comix_roll_min_pixels,
    "MEGADRIVE_DEBUG_COMIX_ROLL_MIN_PIXELS",
    usize,
    192
);
debug_value!(
    comix_roll_min_run,
    "MEGADRIVE_DEBUG_COMIX_ROLL_MIN_RUN",
    usize,
    6
);

debug_present!(sat_line_latch, "MEGADRIVE_DEBUG_SAT_LINE_LATCH");
debug_present!(sat_live, "MEGADRIVE_DEBUG_SAT_LIVE");
debug_present!(sat_per_line, "MEGADRIVE_DEBUG_SAT_PER_LINE");
debug_value!(sprite_x_offset, "MEGADRIVE_DEBUG_SPRITE_X_OFFSET", i32, 0);
debug_value!(sprite_y_offset, "MEGADRIVE_DEBUG_SPRITE_Y_OFFSET", i32, 0);
debug_present!(sprite_swap_size, "MEGADRIVE_DEBUG_SPRITE_SWAP_SIZE");
debug_present!(sprite_row_major, "MEGADRIVE_DEBUG_SPRITE_ROW_MAJOR");
debug_present!(disable_sprite_mask, "MEGADRIVE_DEBUG_DISABLE_SPRITE_MASK");
