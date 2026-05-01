#![allow(static_mut_refs)]
// Logging controls (runtime via env — see debug_flags)
pub(crate) const IMPORTANT_WRITE_LIMIT: u32 = 10; // How many important writes to print
mod access;
mod framebuffer;
mod latch;
mod palette;
mod registers;
mod rendering;
mod sprites;
mod state_io;
mod superfx_bridge;
mod timing;
mod trace;
mod window;

pub(crate) use trace::{
    trace_cgram_write_config, trace_cgram_write_match, trace_sample_dot_config,
    trace_scanline_state_config, trace_vram_write_config, trace_vram_write_match,
};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct WindowLutConfig {
    pub(crate) window1_left: u8,
    pub(crate) window1_right: u8,
    pub(crate) window2_left: u8,
    pub(crate) window2_right: u8,
    pub(crate) window_bg_mask: [u8; 4],
    pub(crate) bg_window_logic: [u8; 4],
    pub(crate) window_obj_mask: u8,
    pub(crate) obj_window_logic: u8,
    pub(crate) window_color_mask: u8,
    pub(crate) color_window_logic: u8,
    pub(crate) tmw_mask: u8,
    pub(crate) tsw_mask: u8,
}

#[derive(Clone, Copy, Default)]
pub(crate) struct BgMapCache {
    pub(crate) valid: bool,
    pub(crate) tile_x: u16,
    pub(crate) tile_y: u16,
    pub(crate) map_entry: u16,
}

#[derive(Clone, Copy, Default)]
pub(crate) struct BgRowCache {
    pub(crate) valid: bool,
    pub(crate) tile_addr: u16,
    pub(crate) rel_y: u8,
    pub(crate) bpp: u8,
    pub(crate) row: [u8; 8],
}

pub struct Ppu {
    pub(crate) vram: Vec<u8>,
    pub(crate) cgram: Vec<u8>,
    pub(crate) cgram_rgb_cache: [u32; 256],
    pub(crate) oam: Vec<u8>,

    pub(crate) scanline: u16,
    // Current dot within the scanline (0..=340 approx). This is our dot counter.
    pub(crate) cycle: u16,
    pub(crate) frame: u64,
    // Latched H/V counters (set by reading $2137 or by WRIO latch via $4201 bit7 transition).
    pub(crate) hv_latched_h: u16,
    pub(crate) hv_latched_v: u16,
    // Pending external latch via WRIO ($4201 bit7 1->0). Fires after a 2-dot delay.
    pub(crate) wio_latch_pending_dots: u8,
    // Pending SLHV latch via $2137 read. Fires after a 1-dot delay.
    pub(crate) slhv_latch_pending_dots: u8,
    pub(crate) ophct_second: bool,
    pub(crate) opvct_second: bool,

    pub(crate) bg_mode: u8,
    // Mode 1 only: BG3 priority enable ($2105 bit3). Used by z-rank model.
    pub(crate) mode1_bg3_priority: bool,
    pub(crate) bg_mosaic: u8,
    pub(crate) mosaic_size: u8, // モザイクサイズ（1-16）

    pub(crate) bg1_tile_base: u16,
    pub(crate) bg2_tile_base: u16,
    pub(crate) bg3_tile_base: u16,
    pub(crate) bg4_tile_base: u16,

    pub(crate) bg1_tilemap_base: u16,
    pub(crate) bg2_tilemap_base: u16,
    pub(crate) bg3_tilemap_base: u16,
    pub(crate) bg4_tilemap_base: u16,

    pub(crate) bg1_hscroll: u16,
    pub(crate) bg1_vscroll: u16,
    pub(crate) bg2_hscroll: u16,
    pub(crate) bg2_vscroll: u16,
    pub(crate) bg3_hscroll: u16,
    pub(crate) bg3_vscroll: u16,
    pub(crate) bg4_hscroll: u16,
    pub(crate) bg4_vscroll: u16,

    // BG tile size flags (false=8x8, true=16x16)
    pub(crate) bg_tile_16: [bool; 4],
    // BG screen sizes: 0=32x32, 1=64x32, 2=32x64, 3=64x64
    pub(crate) bg_screen_size: [u8; 4],

    // Scroll register latches shared across BG1..BG4 ($210D..$2114).
    // See SNESdev wiki: BGnHOFS/BGnVOFS behavior uses shared latches.
    pub(crate) bgofs_latch: u8,
    pub(crate) bghofs_latch: u8,

    pub(crate) main_screen_designation: u8,
    pub(crate) main_screen_designation_last_nonzero: u8, // Remember last non-zero value for rendering
    pub(crate) sub_screen_designation: u8,
    pub(crate) tmw_mask: u8, // $212E: window mask enables for main screen (bits: BG1..BG4,OBJ)
    pub(crate) tsw_mask: u8, // $212F: window mask enables for sub screen

    pub(crate) screen_display: u8,
    pub(crate) brightness: u8,

    pub(crate) vram_addr: u16,
    pub(crate) vram_increment: u16,
    pub(crate) vram_mapping: u8,
    // VRAM read latch for $2139/$213A (VMDATAREAD)
    pub(crate) vram_read_buf_lo: u8,
    pub(crate) vram_read_buf_hi: u8,

    pub(crate) cgram_addr: u8,          // CGRAM word address (0..255)
    pub(crate) cgram_second: bool,      // false: next $2122 is low; true: next $2122 is high
    pub(crate) cgram_read_second: bool, // false: next $213B returns low; true: next returns high then increments
    pub(crate) cgram_latch_lo: u8,      // latched low byte (not committed until high arrives)
    pub(crate) oam_addr: u16,

    // スプライト関連の追加フィールド
    pub(crate) sprite_overflow: bool, // スプライトオーバーフローフラグ
    pub(crate) sprite_time_over: bool, // スプライトタイムオーバーフラグ
    // STAT77 flags are sticky until end of VBlank.
    pub(crate) sprite_overflow_latched: bool,
    pub(crate) sprite_time_over_latched: bool,
    #[allow(dead_code)]
    pub(crate) sprites_on_line_count: u8, // 現在のスキャンラインのスプライト数

    // スプライト関連
    pub(crate) sprite_size: u8,         // スプライトサイズ設定
    pub(crate) sprite_name_base: u16,   // スプライトタイル名ベースアドレス
    pub(crate) sprite_name_select: u16, // スプライト名テーブル選択

    // ウィンドウ関連
    pub(crate) window1_left: u8,        // Window 1の左端
    pub(crate) window1_right: u8,       // Window 1の右端
    pub(crate) window2_left: u8,        // Window 2の左端
    pub(crate) window2_right: u8,       // Window 2の右端
    pub(crate) window_bg_mask: [u8; 4], // BG1-4のウィンドウマスク設定
    pub(crate) window_obj_mask: u8,     // スプライトのウィンドウマスク設定
    pub(crate) window_color_mask: u8,   // カラーウィンドウマスク
    // Window logic (WBGLOG/WOBJLOG): 0=OR,1=AND,2=XOR,3=XNOR
    pub(crate) bg_window_logic: [u8; 4],
    pub(crate) obj_window_logic: u8,
    pub(crate) color_window_logic: u8,

    // カラー演算関連
    // Color math registers
    pub(crate) cgwsel: u8, // $2130: Color Window Select (gating + subscreen/fixed)
    pub(crate) cgadsub: u8, // $2131: Addition/Subtraction + halve + layer enables
    pub(crate) color_math_designation: u8, // legacy alias (CGADSUB layer mask)
    pub(crate) color_math_control: u8, // legacy alias (CGWSEL)
    pub(crate) fixed_color: u16, // 固定色データ（$2132）

    // Mode 7関連
    pub(crate) m7sel: u8,           // $211A: Mode 7 settings (repeat/fill/flip)
    pub(crate) mode7_matrix_a: i16, // Mode 7変換行列A ($211B)
    pub(crate) mode7_matrix_b: i16, // Mode 7変換行列B ($211C)
    pub(crate) mode7_matrix_c: i16, // Mode 7変換行列C ($211D)
    pub(crate) mode7_matrix_d: i16, // Mode 7変換行列D ($211E)
    pub(crate) mode7_center_x: i16, // Mode 7回転中心X ($211F) (13-bit signed)
    pub(crate) mode7_center_y: i16, // Mode 7回転中心Y ($2120) (13-bit signed)
    pub(crate) mode7_hofs: i16,     // $210D: M7HOFS (13-bit signed)
    pub(crate) mode7_vofs: i16,     // $210E: M7VOFS (13-bit signed)
    pub(crate) mode7_latch: u8,     // Shared latch for Mode 7 regs ($210D/$210E/$211B-$2120)
    pub(crate) mode7_mul_b: i8,     // Last 8-bit value written to M7B for $2134-$2136

    // Mode 7 乗算結果キャッシュ ($2134-$2136)
    pub(crate) mode7_mul_result: u32, // 24bit 有効（下位3バイト）

    // Present buffers (last completed frame). The emulator's main loop can overshoot a
    // PPU frame boundary (instruction granularity), which would otherwise partially
    // overwrite the top of the next frame before the host presents → visible tearing.
    pub(crate) framebuffer: Vec<u32>,
    pub(crate) subscreen_buffer: Vec<u32>, // サブスクリーン用バッファ
    // Render (back) buffers for the current in-progress frame.
    pub(crate) render_framebuffer: Vec<u32>,
    pub(crate) render_subscreen_buffer: Vec<u32>,
    pub(crate) brightness_simd_buf: [u32; 8],
    pub(crate) brightness_simd_len: u8,
    pub(crate) brightness_simd_start: usize,
    pub(crate) brightness_simd_factor: u8,
    // Headless高速化用: PPUのピクセル合成（フレームバッファ書き込み）を無効化できる。
    // 画面出力が不要なフレームをスキップし、最終フレームだけ描画する用途を想定。
    pub(crate) framebuffer_rendering_enabled: bool,
    // Per-line render cache (reduces per-pixel bit tests)
    pub(crate) line_main_enables: u8,
    pub(crate) line_sub_enables: u8,
    pub(crate) line_main_has_bg: bool,
    pub(crate) line_main_has_obj: bool,
    pub(crate) line_sub_has_bg: bool,
    pub(crate) line_sub_has_obj: bool,
    pub(crate) line_hires_out: bool,
    pub(crate) line_color_math_enabled: bool,
    pub(crate) line_need_subscreen: bool,

    // SETINI ($2133)
    pub(crate) setini: u8,
    pub(crate) pseudo_hires: bool,
    pub(crate) extbg: bool,
    pub(crate) interlace: bool,
    // H/V counter latch enable (mirrors $4201 bit7) and STAT78 latch flag.
    pub(crate) wio_latch_enable: bool,
    pub(crate) stat78_latch_flag: bool,
    // STAT78 "interlace field" bit (toggles every VBlank).
    pub(crate) interlace_field: bool,
    // SETINI bits
    pub(crate) overscan: bool,
    pub(crate) obj_interlace: bool,
    pub(crate) force_no_blank: bool,
    /// When true, bypass BG1 window masking. Used as a workaround for SuperFX
    /// games where the viewport metadata computation produces incorrect window positions.
    pub(crate) superfx_bypass_bg1_window: bool,
    /// When true, mode 2 BG1 may sample SuperFX buffers directly instead of the
    /// normal BG1 tile/tilemap path.
    pub(crate) superfx_authoritative_bg1_source: bool,
    /// Legacy Star Fox title-screen debug workaround. BG1 carries the title
    /// flight-line layer, so this must stay opt-in.
    pub(crate) starfox_title_suppress_bg1: bool,
    pub(crate) superfx_direct_buffer: Vec<u8>,
    pub(crate) superfx_direct_height: u16,
    pub(crate) superfx_direct_bpp: u8,
    pub(crate) superfx_direct_mode: u8,
    pub(crate) superfx_direct_default_x_offset: i32,
    pub(crate) superfx_direct_default_y_offset: i32,
    pub(crate) superfx_tile_buffer: Vec<u8>,
    pub(crate) superfx_tile_bpp: u8,
    pub(crate) superfx_tile_mode: u8,

    pub(crate) nmi_enabled: bool,
    pub(crate) nmi_flag: bool,
    pub(crate) nmi_latched: bool,
    /// 同一VBlank中にRDNMIが読まれたか。
    pub(crate) rdnmi_read_in_vblank: bool,

    pub(crate) v_blank: bool,
    pub(crate) h_blank: bool,

    // Lightweight VRAM write diagnostics (headless summaries)
    pub(crate) vram_write_buckets: [u32; 8], // counts per 0x1000-word region (0x0000..0x7000)
    pub(crate) vram_write_low_count: u32,
    pub(crate) vram_write_high_count: u32,
    pub(crate) vram_last_vmain: u8,
    // Strict timing: reject counters
    pub(crate) vram_rejects: u32,
    pub(crate) cgram_rejects: u32,
    pub(crate) oam_rejects: u32,
    // Gap-block counters (per summary interval)
    pub(crate) vram_gap_blocks: u32,
    pub(crate) cgram_gap_blocks: u32,
    pub(crate) oam_gap_blocks: u32,
    pub(crate) oam_data_gap_ticks: u16,
    // First per-frame rejection logs (to avoid spam when DEBUG_TIMING_REJECTS)
    pub(crate) last_reject_frame_vram: u64,
    pub(crate) last_reject_frame_cgram: u64,
    pub(crate) last_reject_frame_oam: u64,

    // Run-wide counters for headless init summary
    pub(crate) important_writes_count: u32,
    pub(crate) vram_writes_total_low: u64,
    pub(crate) vram_writes_total_high: u64,
    pub(crate) cgram_writes_total: u64,
    pub(crate) oam_writes_total: u64,
    // OAMDATA write latch (low table uses 16-bit word staging)
    pub(crate) oam_write_latch: u8,
    pub(crate) oam_dirty: bool,
    pub(crate) sprite_cached_y: [u8; 128],
    pub(crate) sprite_cached_x_raw: [u16; 128],
    pub(crate) sprite_cached_x_signed: [i16; 128],
    pub(crate) sprite_cached_tile: [u16; 128],
    pub(crate) sprite_cached_attr: [u8; 128],
    pub(crate) sprite_cached_size_large: [bool; 128],
    // $2103 bit7: priority rotation enable
    pub(crate) oam_priority_rotation_enabled: bool,
    // OBJ timing metrics per frame
    pub(crate) obj_overflow_lines: u32,
    pub(crate) obj_time_over_lines: u32,
    // OAM evaluation rotation base (sprite index 0..127). Derived from $2102/$2103.
    pub(crate) oam_eval_base: u8,

    // Dot-level OBJ pipeline state (per visible scanline)
    pub(crate) line_sprites: Vec<SpriteData>,
    // Per-priority sprite indices for the current scanline (preserve OAM order)
    pub(crate) line_sprites_by_priority: [Vec<usize>; 4],
    #[allow(dead_code)]
    pub(crate) sprite_tile_entry_counts: [u8; 256],
    #[allow(dead_code)]
    pub(crate) sprite_tile_budget_remaining: i16,
    #[allow(dead_code)]
    pub(crate) sprite_draw_disabled: bool,
    pub(crate) sprite_timeover_first_idx: u8, // first line_sprites index to drop when time-over hits (inclusive)

    // --- Dot-level window/color-math gating (per visible scanline) ---
    pub(crate) line_window_prepared: bool,
    pub(crate) line_window_cfg: Option<WindowLutConfig>,
    pub(crate) color_window_lut: [u8; 256], // 1: inside color window per $2125(COL)
    pub(crate) main_bg_window_lut: [[u8; 256]; 4], // 1: BG masked on main at x
    pub(crate) sub_bg_window_lut: [[u8; 256]; 4], // 1: BG masked on sub at x
    pub(crate) main_obj_window_lut: [u8; 256], // 1: OBJ masked on main at x
    pub(crate) sub_obj_window_lut: [u8; 256], // 1: OBJ masked on sub at x

    // --- BG tile row cache (per BG) ---
    pub(crate) bg_cache_dirty: bool,
    pub(crate) bg_map_cache: [BgMapCache; 4],
    pub(crate) bg_row_cache: [BgRowCache; 4],

    // --- Mode 2 offset-per-tile (BG3 OPT) cached per visible scanline ---
    // Index is tile-column on screen (0..32). Column 0 is never affected by OPT.
    pub(crate) mode2_opt_hscroll_lut: [[u16; 33]; 2], // [BG1/BG2][col] -> effective HOFS
    pub(crate) mode2_opt_vscroll_lut: [[u16; 33]; 2], // [BG1/BG2][col] -> effective VOFS

    // internal OAM byte address (internal_oamadd, 10-bit)
    pub(crate) oam_internal_addr: u16,

    // --- HBlank head HDMA phase guard ---
    // A tiny sub-window after HBlank starts where only HDMA should be active; MDMA is held off.
    pub(crate) hdma_head_busy_until: u16,

    // --- Latched (timed-commit) display-affecting registers ---
    // These are optionally used when STRICT_PPU_TIMING is enabled to apply
    // register effects at well-defined scanline boundaries instead of mid-line.
    pub(crate) latched_inidisp: Option<u8>, // mirrors $2100 (forced blank + brightness)
    pub(crate) latched_tm: Option<u8>,      // $212C main screen designation
    pub(crate) latched_ts: Option<u8>,      // $212D sub  screen designation
    pub(crate) latched_tmw: Option<u8>,     // $212E window mask enable (main)
    pub(crate) latched_tsw: Option<u8>,     // $212F window mask enable (sub)
    pub(crate) latched_cgwsel: Option<u8>,  // $2130 color window select
    pub(crate) latched_cgadsub: Option<u8>, // $2131 color math control
    pub(crate) latched_fixed_color: Option<u16>, // $2132 fixed color
    pub(crate) latched_setini: Option<u8>,  // $2133 SETINI (pseudo hires, EXTBG, interlace)
    // --- Latched control (address) registers for safe commit ---
    pub(crate) latched_vmadd_lo: Option<u8>, // $2116 VMADDL (low byte)
    pub(crate) latched_vmadd_hi: Option<u8>, // $2117 VMADDH (high byte)
    pub(crate) latched_cgadd: Option<u8>,    // $2121 CGADD
    pub(crate) latched_vmain: Option<u8>,    // $2115 VMAIN
    // Deferred effect for VMAIN (after commit)
    pub(crate) vmain_effect_pending: Option<u8>,
    pub(crate) vmain_effect_ticks: u16,
    // Deferred effect for CGADD
    pub(crate) cgadd_effect_pending: Option<u8>,
    pub(crate) cgadd_effect_ticks: u16,
    // Data write gap after VMAIN effect (MDMA/CPU only)
    pub(crate) vmain_data_gap_ticks: u16,
    // Data write gap after CGADD effect (MDMA/CPU only)
    pub(crate) cgram_data_gap_ticks: u16,
    pub(crate) latched_wbglog: Option<u8>, // $212A window logic BG1..BG4
    pub(crate) latched_wobjlog: Option<u8>, // $212B window logic OBJ/COL

    // --- Optional per-frame render metrics (for regression/debug) ---
    pub(crate) dbg_clip_inside: u64,
    pub(crate) dbg_clip_outside: u64,
    pub(crate) dbg_math_add: u64,
    pub(crate) dbg_math_sub: u64,
    pub(crate) dbg_math_add_half: u64,
    pub(crate) dbg_math_sub_half: u64,
    pub(crate) dbg_masked_bg: u64,
    pub(crate) dbg_masked_obj: u64,
    pub(crate) dbg_math_obj_add: u64,
    pub(crate) dbg_math_obj_sub: u64,
    pub(crate) dbg_math_obj_add_half: u64,
    pub(crate) dbg_math_obj_sub_half: u64,
    pub(crate) dbg_clip_obj_inside: u64,
    pub(crate) dbg_clip_obj_outside: u64,
    // Mode 7 metrics
    pub(crate) dbg_m7_wrap: u64,
    pub(crate) dbg_m7_clip: u64,
    pub(crate) dbg_m7_fill: u64,
    pub(crate) dbg_m7_bg1: u64,
    pub(crate) dbg_m7_bg2: u64,
    pub(crate) dbg_m7_edge: u64,
    // Window logic usage counters (optional)
    pub(crate) dbg_win_xor_applied: u64,
    pub(crate) dbg_win_xnor_applied: u64,
    // Color math blocked by CGADSUB counters
    pub(crate) dbg_math_blocked: u64,
    pub(crate) dbg_math_blocked_obj: u64,
    pub(crate) dbg_math_blocked_backdrop: u64,

    // Distinguish CPU vs MDMA vs HDMA register writes (0=CPU,1=MDMA,2=HDMA)
    pub(crate) write_ctx: u8,
    pub(crate) debug_dma_channel: Option<u8>, // active MDMA/HDMA channel for debug logs
    // burn-in-test.sfc: arm narrow VRAM clobber tracing after DMA MEMORY begins
    pub(crate) burnin_vram_trace_armed: bool,
    pub(crate) burnin_vram_trace_cnt_2118: u32,
    pub(crate) burnin_vram_trace_cnt_2119: u32,
}

#[derive(Debug, Clone)]
pub(crate) struct SpriteData {
    pub(crate) x: u16,
    pub(crate) x_signed: i16,
    pub(crate) y: u8,
    pub(crate) tile: u16,
    pub(crate) palette: u8,
    pub(crate) priority: u8,
    pub(crate) flip_x: bool,
    pub(crate) flip_y: bool,
    pub(crate) size: SpriteSize,
    pub(crate) width: u8,
    pub(crate) height: u8,
    pub(crate) line_rel_y: u8,
    pub(crate) line_tile_y: u8,
    pub(crate) line_pixel_y: u8,
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum SpriteSize {
    Small, // BGモードによって 8x8 または 16x16
    Large, // BGモードによって 16x16, 32x32, または 64x64
}

impl Ppu {
    // sprite_x_signed moved to sprites.rs

    // obj_interlace_active moved to sprites.rs

    // obj_line_for_scanline moved to sprites.rs

    // obj_sprite_dy moved to sprites.rs

    // obj_sprite_height_lines moved to sprites.rs

    // obj_sprite_rel_y moved to sprites.rs

    // sign_extend13 moved to registers.rs

    // mode7_combine moved to registers.rs

    // write_m7hofs moved to registers.rs

    // write_m7vofs moved to registers.rs

    // write_bghofs moved to registers.rs

    // write_bgvofs moved to registers.rs

    pub fn new() -> Self {
        let mut ppu = Self {
            vram: vec![0; 0x10000],
            cgram: vec![0; 0x200],
            cgram_rgb_cache: [0xFF000000; 256],
            oam: vec![0; 0x220],

            scanline: 0,
            cycle: 0,
            frame: 0,
            hv_latched_h: 0,
            hv_latched_v: 0,
            wio_latch_pending_dots: 0,
            slhv_latch_pending_dots: 0,
            ophct_second: false,
            opvct_second: false,

            bg_mode: 0,
            mode1_bg3_priority: false,
            bg_mosaic: 0,
            mosaic_size: 1,

            bg1_tile_base: 0,
            bg2_tile_base: 0,
            bg3_tile_base: 0,
            bg4_tile_base: 0,

            bg1_tilemap_base: 0,
            bg2_tilemap_base: 0,
            bg3_tilemap_base: 0,
            bg4_tilemap_base: 0,

            bg1_hscroll: 0,
            bg1_vscroll: 0,
            bg2_hscroll: 0,
            bg2_vscroll: 0,
            bg3_hscroll: 0,
            bg3_vscroll: 0,
            bg4_hscroll: 0,
            bg4_vscroll: 0,

            bg_tile_16: [false; 4],
            bg_screen_size: [0; 4],

            bgofs_latch: 0,
            bghofs_latch: 0,

            main_screen_designation: 0x1F, // 初期は全BG/Spriteレイヤー有効
            main_screen_designation_last_nonzero: 0x1F,
            sub_screen_designation: 0,
            tmw_mask: 0,
            tsw_mask: 0,

            screen_display: 0x80, // forced blank on by default (初期状態は画面非表示)
            brightness: 0,        // 初期明度を0に設定

            vram_addr: 0,
            vram_increment: 1,
            vram_mapping: 0,
            vram_read_buf_lo: 0,
            vram_read_buf_hi: 0,

            cgram_addr: 0,
            cgram_second: false,
            cgram_read_second: false,
            cgram_latch_lo: 0,
            oam_addr: 0,

            sprite_overflow: false,
            sprite_time_over: false,
            sprite_overflow_latched: false,
            sprite_time_over_latched: false,
            sprites_on_line_count: 0,

            // スプライト関連初期化
            sprite_size: 0,
            sprite_name_base: 0,
            sprite_name_select: 0,

            // ウィンドウ関連初期化
            window1_left: 0,
            window1_right: 0,
            window2_left: 0,
            window2_right: 0,
            window_bg_mask: [0; 4],
            window_obj_mask: 0,
            window_color_mask: 0,
            bg_window_logic: [0; 4],
            obj_window_logic: 0,
            color_window_logic: 0,

            // カラー演算関連初期化
            cgwsel: 0,
            cgadsub: 0,
            color_math_designation: 0,
            color_math_control: 0,
            fixed_color: 0,

            // Mode 7関連初期化（単位行列）
            m7sel: 0,
            mode7_matrix_a: 256, // 1.0 in fixed point (8.8)
            mode7_matrix_b: 0,
            mode7_matrix_c: 0,
            mode7_matrix_d: 256, // 1.0 in fixed point (8.8)
            mode7_center_x: 0,
            mode7_center_y: 0,
            mode7_hofs: 0,
            mode7_vofs: 0,
            mode7_latch: 0,
            mode7_mul_b: 0,
            mode7_mul_result: 0,

            framebuffer: vec![0; 256 * 239],
            subscreen_buffer: vec![0; 256 * 239],
            render_framebuffer: vec![0; 256 * 239],
            render_subscreen_buffer: vec![0; 256 * 239],
            brightness_simd_buf: [0; 8],
            brightness_simd_len: 0,
            brightness_simd_start: 0,
            brightness_simd_factor: 15,
            framebuffer_rendering_enabled: true,
            line_main_enables: 0,
            line_sub_enables: 0,
            line_main_has_bg: false,
            line_main_has_obj: false,
            line_sub_has_bg: false,
            line_sub_has_obj: false,
            line_hires_out: false,
            line_color_math_enabled: false,
            line_need_subscreen: false,

            setini: 0,
            pseudo_hires: false,
            extbg: false,
            interlace: false,
            wio_latch_enable: false,
            stat78_latch_flag: false,
            interlace_field: false,
            overscan: false,
            obj_interlace: false,
            force_no_blank: crate::debug_flags::force_no_blank(),
            superfx_bypass_bg1_window: false,
            superfx_authoritative_bg1_source: false,
            starfox_title_suppress_bg1: false,
            superfx_direct_buffer: Vec::new(),
            superfx_direct_height: 0,
            superfx_direct_bpp: 0,
            superfx_direct_mode: 0,
            superfx_direct_default_x_offset: -56,
            superfx_direct_default_y_offset: 0,
            superfx_tile_buffer: Vec::new(),
            superfx_tile_bpp: 0,
            superfx_tile_mode: 0,

            nmi_enabled: false,
            // 実機ではリセット直後に RDNMI フラグ(bit7)が1の状態から始まるため、初期値をtrueにしておく。
            nmi_flag: true,
            nmi_latched: false,
            rdnmi_read_in_vblank: false,

            v_blank: false,
            h_blank: false,

            vram_write_buckets: [0; 8],
            vram_write_low_count: 0,
            vram_write_high_count: 0,
            vram_last_vmain: 0,
            vram_rejects: 0,
            cgram_rejects: 0,
            oam_rejects: 0,
            vram_gap_blocks: 0,
            cgram_gap_blocks: 0,
            oam_gap_blocks: 0,
            oam_data_gap_ticks: 0,
            last_reject_frame_vram: u64::MAX,
            last_reject_frame_cgram: u64::MAX,
            last_reject_frame_oam: u64::MAX,

            important_writes_count: 0,
            vram_writes_total_low: 0,
            vram_writes_total_high: 0,
            cgram_writes_total: 0,
            oam_writes_total: 0,
            oam_write_latch: 0,
            oam_dirty: true,
            sprite_cached_y: [0; 128],
            sprite_cached_x_raw: [0; 128],
            sprite_cached_x_signed: [0; 128],
            sprite_cached_tile: [0; 128],
            sprite_cached_attr: [0; 128],
            sprite_cached_size_large: [false; 128],
            oam_priority_rotation_enabled: false,
            obj_overflow_lines: 0,
            obj_time_over_lines: 0,
            oam_eval_base: 0,
            line_sprites: Vec::new(),
            line_sprites_by_priority: [Vec::new(), Vec::new(), Vec::new(), Vec::new()],
            sprite_tile_entry_counts: [0; 256],
            sprite_tile_budget_remaining: 0,
            sprite_draw_disabled: false,
            sprite_timeover_first_idx: 0,
            line_window_prepared: false,
            line_window_cfg: None,
            color_window_lut: [0; 256],
            main_bg_window_lut: [[0; 256]; 4],
            sub_bg_window_lut: [[0; 256]; 4],
            main_obj_window_lut: [0; 256],
            sub_obj_window_lut: [0; 256],
            bg_cache_dirty: true,
            bg_map_cache: [BgMapCache::default(); 4],
            bg_row_cache: [BgRowCache::default(); 4],
            mode2_opt_hscroll_lut: [[0; 33]; 2],
            mode2_opt_vscroll_lut: [[0; 33]; 2],
            oam_internal_addr: 0,
            hdma_head_busy_until: 0,

            // Latched display regs (disabled by default)
            latched_inidisp: None,
            latched_tm: None,
            latched_ts: None,
            latched_tmw: None,
            latched_tsw: None,
            latched_cgwsel: None,
            latched_cgadsub: None,
            latched_fixed_color: None,
            latched_setini: None,
            latched_vmadd_lo: None,
            latched_vmadd_hi: None,
            latched_cgadd: None,
            latched_vmain: None,
            vmain_effect_pending: None,
            vmain_effect_ticks: 0,
            cgadd_effect_pending: None,
            cgadd_effect_ticks: 0,
            vmain_data_gap_ticks: 0,
            cgram_data_gap_ticks: 0,
            latched_wbglog: None,
            latched_wobjlog: None,

            dbg_clip_inside: 0,
            dbg_clip_outside: 0,
            dbg_math_add: 0,
            dbg_math_sub: 0,
            dbg_math_add_half: 0,
            dbg_math_sub_half: 0,
            dbg_masked_bg: 0,
            dbg_masked_obj: 0,
            dbg_math_obj_add: 0,
            dbg_math_obj_sub: 0,
            dbg_math_obj_add_half: 0,
            dbg_math_obj_sub_half: 0,
            dbg_clip_obj_inside: 0,
            dbg_clip_obj_outside: 0,
            dbg_m7_wrap: 0,
            dbg_m7_clip: 0,
            dbg_m7_fill: 0,
            dbg_m7_bg1: 0,
            dbg_m7_bg2: 0,
            dbg_m7_edge: 0,

            dbg_win_xor_applied: 0,
            dbg_win_xnor_applied: 0,
            dbg_math_blocked: 0,
            dbg_math_blocked_obj: 0,
            dbg_math_blocked_backdrop: 0,

            write_ctx: 0,
            debug_dma_channel: None,
            burnin_vram_trace_armed: false,
            burnin_vram_trace_cnt_2118: 0,
            burnin_vram_trace_cnt_2119: 0,
        };
        ppu.update_line_render_state();
        ppu
    }

    pub fn step(&mut self, cycles: u16) {
        // Per-CPU-cycle PPU stepping (approx 1 CPU cycle -> 1 PPU dot)
        let first_hblank = self.first_hblank_dot();
        let first_visible = self.first_visible_dot();
        let render_enabled = self.framebuffer_rendering_enabled;
        let mut vis_lines = self.get_visible_height();
        let mut vblank_start = self.vblank_start_line();
        for _ in 0..cycles {
            // Advance any deferred control effects before processing this dot
            self.tick_deferred_ctrl_effects();
            let x = self.cycle;
            let y = self.scanline;

            // Update HBlank state from dot counters.
            //
            // Official burn-in tests (HVBJOY/VH FLAG) expect $4212 bit6 (HBlank) to be set only
            // for the right-side blanking period. Do not treat the pre-visible dots as "HBlank"
            // for this flag.
            let hblank_now = x >= first_hblank;
            if hblank_now != self.h_blank {
                self.h_blank = hblank_now;
                if hblank_now && x == first_hblank {
                    // Entering right-side HBlank; guard a few dots at HBlank head for HDMA operations only.
                    let guard = crate::debug_flags::hblank_hdma_guard_dots();
                    self.hdma_head_busy_until = first_hblank.saturating_add(guard);
                }
            }

            // Start-of-line duties
            if x == 0 {
                // Commit latched regs at the beginning of each scanline
                self.commit_latched_display_regs();
                self.update_line_render_state();
                // Visible height depends on display regs (e.g., overscan) latched at line start.
                vis_lines = self.get_visible_height();
                vblank_start = self.vblank_start_line();
                if render_enabled && y < vis_lines {
                    // Prepare window LUTs at line start (OBJ list is prepared during previous HBlank)
                    self.prepare_line_window_luts();
                    self.prepare_line_opt_luts();
                    if self.line_sprites.is_empty() {
                        // Skip sprite evaluation if no sprites are present on this scanline.
                        self.line_main_has_obj = false;
                        self.line_sub_has_obj = false;
                    }
                    // Mode 7 flicker debug: log matrix state at scanline 50
                    if crate::debug_flags::trace_m7_scanline() && y == 50 && self.bg_mode == 7 {
                        eprintln!(
                            "[M7-SL50] frame={} A={} B={} C={} D={} HOFS={} VOFS={} CX={} CY={} latch=0x{:02X}",
                            self.frame,
                            self.mode7_matrix_a, self.mode7_matrix_b,
                            self.mode7_matrix_c, self.mode7_matrix_d,
                            self.mode7_hofs, self.mode7_vofs,
                            self.mode7_center_x, self.mode7_center_y,
                            self.mode7_latch
                        );
                    }
                }
            }

            // After guard period, commit any pending control registers (VMADD/CGADD)
            if self.h_blank && x == self.hdma_head_busy_until {
                self.commit_pending_ctrl_if_any();
            }

            // Visible pixel render (scanline 0 is not visible on real hardware)
            if !self.v_blank && y >= 1 && y <= vis_lines && x >= first_visible && x < first_hblank {
                let fb_x = (x - first_visible) as usize;
                if (y - 1) < 239 && render_enabled {
                    self.render_dot(fb_x, y as usize);
                }
            }

            // Advance dot; end-of-line at DOTS_PER_LINE
            let dots_per_line = self.dots_per_line();
            self.cycle += 1;
            if self.cycle >= dots_per_line {
                // End of scanline
                self.cycle = 0;
                self.h_blank = false; // dot 0 is not treated as HBlank for HVBJOY
                self.scanline = self.scanline.wrapping_add(1);

                // VBlank transitions
                // 通常: 可視領域終了の次のラインでVBlank突入
                if !self.v_blank && self.scanline == vblank_start {
                    if crate::debug_flags::boot_verbose() {
                        println!("📺 ENTERING VBLANK at scanline {}", self.scanline);
                    }
                    self.enter_vblank();
                } else if self.scanline == self.scanlines_per_frame() {
                    // NTSC frame end (coarse). Wrap to next frame.
                    if crate::debug_flags::boot_verbose() {
                        println!(
                            "📺 FRAME END: scanline {}, resetting to 0",
                            self.scanlines_per_frame()
                        );
                    }
                    // Present the last completed 256x224 frame before the next frame starts
                    // overwriting the top scanlines. This avoids visible tearing when the
                    // outer loop overshoots the boundary at instruction granularity.
                    if render_enabled {
                        std::mem::swap(&mut self.framebuffer, &mut self.render_framebuffer);
                        std::mem::swap(
                            &mut self.subscreen_buffer,
                            &mut self.render_subscreen_buffer,
                        );
                        // The next frame only redraws the visible region. Clear the back
                        // buffers here so overscan / non-rendered border pixels do not
                        // retain stale colors from older frames.
                        self.render_framebuffer.fill(0xFF000000);
                        self.render_subscreen_buffer.fill(0);
                    }
                    self.exit_vblank();
                    self.scanline = 0;
                    self.frame = self.frame.wrapping_add(1);
                    // Prepare first visible line sprites ahead (scanline 0)
                    self.prepare_line_obj_pipeline(0);
                } else {
                    // Prepare next visible scanline sprites during HBlank end
                    let ny = self.scanline;
                    if ny < vis_lines {
                        let vy = ny;
                        self.prepare_line_obj_pipeline(vy);
                    }
                }
            }

            // External HV latch via WRIO ($4201 bit7 1->0): latch occurs 1 dot later than $2137.
            // SLHV ($2137) latches 1 dot after the read, so WRIO latch uses 2 dots.
            if self.wio_latch_pending_dots > 0 {
                self.wio_latch_pending_dots = self.wio_latch_pending_dots.saturating_sub(1);
                if self.wio_latch_pending_dots == 0 {
                    self.latch_hv_counters();
                }
            }
            if self.slhv_latch_pending_dots > 0 {
                self.slhv_latch_pending_dots = self.slhv_latch_pending_dots.saturating_sub(1);
                if self.slhv_latch_pending_dots == 0 {
                    self.latch_hv_counters();
                }
            }
        }
    }

    // render_scanline moved to rendering.rs

    // get_pixel_color moved to rendering.rs

    // composite_pixel moved to rendering.rs

    // composite_pixel_with_layer moved to rendering.rs

    // z_rank_for_obj moved to rendering.rs

    // z_rank_for_bg moved to rendering.rs

    // effective_main_screen_designation moved to rendering.rs

    // get_main_bg_layers moved to rendering.rs

    // get_bg_pixel moved to rendering.rs

    // render_bg_mode0_with_priority moved to rendering.rs

    // render_bg_4bpp_with_priority moved to rendering.rs

    // render_bg_8bpp_with_priority moved to rendering.rs

    // render_bg_mode2_with_priority moved to rendering.rs

    // render_bg_mode5_with_priority moved to rendering.rs

    // render_bg_mode6_with_priority moved to rendering.rs

    // sample_tile_2bpp moved to rendering.rs

    // sample_tile_4bpp moved to rendering.rs

    // render_bg_mode0 moved to rendering.rs

    // render_bg_mode1 moved to rendering.rs

    // render_bg_4bpp moved to rendering.rs

    // render_bg_4bpp_impl moved to rendering.rs

    // render_bg_mode4 moved to rendering.rs

    // render_bg_8bpp moved to rendering.rs

    // direct_color_to_rgb moved to rendering.rs

    // render_mode7_with_layer moved to rendering.rs

    // render_mode7_single_layer moved to rendering.rs

    // sample_mode7_color_only moved to rendering.rs

    // sample_mode7_for_layer moved to rendering.rs

    // render_bg_mode2 moved to rendering.rs

    // render_bg_mode5 moved to rendering.rs

    // render_bg_mode6 moved to rendering.rs

    // apply_hires_enhancement moved to rendering.rs

    // apply_brightness moved to rendering.rs

    // apply_brightness_with_factor moved to rendering.rs

    // apply_brightness_simd_block moved to rendering.rs

    // flush_brightness_simd moved to rendering.rs

    // average_rgb moved to rendering.rs

    // vram_remap_word_addr moved to registers.rs

    // reload_vram_read_latch moved to registers.rs

    // read moved to registers.rs

    // write moved to registers.rs

    /// デバッグ用: BG1 のタイルマップ／タイルベースアドレスを取得
    #[allow(dead_code)]
    pub fn dbg_bg1_bases(&self) -> (u16, u16) {
        (self.bg1_tilemap_base, self.bg1_tile_base)
    }

    #[allow(dead_code)]
    pub fn dbg_bg_bases(&self, bg: usize) -> (u16, u16) {
        match bg {
            0 => (self.bg1_tilemap_base, self.bg1_tile_base),
            1 => (self.bg2_tilemap_base, self.bg2_tile_base),
            2 => (self.bg3_tilemap_base, self.bg3_tile_base),
            _ => (self.bg4_tilemap_base, self.bg4_tile_base),
        }
    }

    /// デバッグ用: VRAM 関連レジスタを取得
    pub fn dbg_vram_regs(&self) -> (u16, u16, u8) {
        (self.vram_addr, self.vram_increment, self.vram_mapping)
    }

    // Raw memory accessors (headless debug dump)
    #[allow(dead_code)]
    pub fn get_vram(&self) -> &[u8] {
        &self.vram
    }

    #[allow(dead_code)]
    pub fn get_cgram(&self) -> &[u8] {
        &self.cgram
    }

    #[allow(dead_code)]
    pub fn get_oam(&self) -> &[u8] {
        &self.oam
    }

    // Convenience dumps (head portion) for debugging
    pub fn dump_vram_head(&self, n: usize) -> Vec<u8> {
        let cnt = n.min(self.vram.len());
        self.vram[..cnt].to_vec()
    }

    pub fn dump_cgram_head(&self, n: usize) -> Vec<u16> {
        let mut out = Vec::new();
        let cnt = n.min(16).min(self.cgram.len() / 2);
        for i in 0..cnt {
            let lo = self.cgram[i * 2] as u16;
            let hi = self.cgram[i * 2 + 1] as u16;
            out.push((hi << 8) | lo);
        }
        out
    }

    pub fn dump_oam_head(&self, n: usize) -> Vec<u8> {
        let cnt = n.min(self.oam.len());
        self.oam[..cnt].to_vec()
    }

    #[allow(dead_code)]
    pub fn get_subscreen_buffer(&self) -> &[u32] {
        &self.subscreen_buffer
    }

    // Debug helper: expose current OAM address and internal address.
    #[inline]
    pub fn dbg_oam_addrs(&self) -> (u16, u16) {
        (self.oam_addr, self.oam_internal_addr)
    }

    pub fn is_forced_blank(&self) -> bool {
        (self.screen_display & 0x80) != 0
    }

    pub fn current_brightness(&self) -> u8 {
        self.brightness & 0x0F
    }

    pub fn get_main_screen_designation(&self) -> u8 {
        self.main_screen_designation
    }

    pub fn get_bg_mode(&self) -> u8 {
        self.bg_mode
    }

    // Headless init counters summary
    pub fn get_init_counters(&self) -> (u32, u64, u64, u64, u64) {
        (
            self.important_writes_count,
            self.vram_writes_total_low,
            self.vram_writes_total_high,
            self.cgram_writes_total,
            self.oam_writes_total,
        )
    }

    // prepare_line_opt_luts moved to rendering.rs

    // update_line_render_state moved to rendering.rs

    // read_bg_tilemap_entry_word moved to rendering.rs

    // invalidate_bg_caches moved to rendering.rs

    // get_bg_map_entry_cached moved to rendering.rs

    // sample_bg_cached moved to rendering.rs

    // Summarize VRAM writes since last call, including FG mode info. Resets counters.
    pub fn take_vram_write_summary(&mut self) -> String {
        let mut parts: Vec<String> = Vec::new();
        let fg_mode = (self.vram_last_vmain >> 2) & 0x03;
        let inc = match self.vram_last_vmain & 0x03 {
            0 => 1,
            1 => 32,
            _ => 128,
        };
        let inc_on = if (self.vram_last_vmain & 0x80) != 0 {
            "HIGH"
        } else {
            "LOW"
        };
        parts.push(format!(
            "VMAIN fg={} inc={} inc_on_{}",
            fg_mode, inc, inc_on
        ));
        parts.push(format!(
            "writes: low={} high={}",
            self.vram_write_low_count, self.vram_write_high_count
        ));
        // Buckets 0..7 => 0x0000..0x7000 (word address)
        let mut bucket_strs: Vec<String> = Vec::new();
        for i in 0..8 {
            let base = i * 0x1000;
            let cnt = self.vram_write_buckets[i];
            if cnt > 0 {
                bucket_strs.push(format!("{:04X}-{:04X}:{}", base, base + 0x0FFF, cnt));
            }
        }
        if bucket_strs.is_empty() {
            parts.push("buckets: none".to_string());
        } else {
            parts.push(format!("buckets: {}", bucket_strs.join(", ")));
        }

        // Reject counters and concise gap blocks (timing tune)
        parts.push(format!(
            "rejects: vram={} cgram={} oam={}",
            self.vram_rejects, self.cgram_rejects, self.oam_rejects
        ));
        parts.push(format!(
            "gaps: vram={} cgram={} oam={}",
            self.vram_gap_blocks, self.cgram_gap_blocks, self.oam_gap_blocks
        ));

        // Reset counters
        self.vram_write_buckets = [0; 8];
        self.vram_write_low_count = 0;
        self.vram_write_high_count = 0;
        self.vram_rejects = 0;
        self.cgram_rejects = 0;
        self.oam_rejects = 0;
        self.vram_gap_blocks = 0;
        self.cgram_gap_blocks = 0;
        self.oam_gap_blocks = 0;

        parts.join(" | ")
    }

    // Summarize per-frame render metrics and reset counters
    pub fn take_render_metrics_summary(&mut self) -> String {
        if !crate::debug_flags::render_metrics() {
            return "RENDER_METRICS: off".to_string();
        }
        let s = format!(
            "RENDER_METRICS: clip_in={} clip_out={} add={} add/2={} sub={} sub/2={} masked_bg={} masked_obj={} obj_add={} obj_add/2={} obj_sub={} obj_sub/2={} obj_clip_in={} obj_clip_out={} win_xor={} win_xnor={} math_blocked={} math_blocked_obj={} math_blocked_bd={} m7_wrap={} m7_clip={} m7_fill={} m7_bg1={} m7_bg2={} m7_edge={}",
            self.dbg_clip_inside,
            self.dbg_clip_outside,
            self.dbg_math_add,
            self.dbg_math_add_half,
            self.dbg_math_sub,
            self.dbg_math_sub_half,
            self.dbg_masked_bg,
            self.dbg_masked_obj,
            self.dbg_math_obj_add,
            self.dbg_math_obj_add_half,
            self.dbg_math_obj_sub,
            self.dbg_math_obj_sub_half,
            self.dbg_clip_obj_inside,
            self.dbg_clip_obj_outside,
            self.dbg_win_xor_applied,
            self.dbg_win_xnor_applied,
            self.dbg_math_blocked,
            self.dbg_math_blocked_obj,
            self.dbg_math_blocked_backdrop,
            self.dbg_m7_wrap,
            self.dbg_m7_clip,
            self.dbg_m7_fill,
            self.dbg_m7_bg1,
            self.dbg_m7_bg2,
            self.dbg_m7_edge
        );
        self.dbg_clip_inside = 0;
        self.dbg_clip_outside = 0;
        self.dbg_math_add = 0;
        self.dbg_math_add_half = 0;
        self.dbg_math_sub = 0;
        self.dbg_math_sub_half = 0;
        self.dbg_masked_bg = 0;
        self.dbg_masked_obj = 0;
        self.dbg_math_obj_add = 0;
        self.dbg_math_obj_add_half = 0;
        self.dbg_math_obj_sub = 0;
        self.dbg_math_obj_sub_half = 0;
        self.dbg_clip_obj_inside = 0;
        self.dbg_clip_obj_outside = 0;
        self.dbg_win_xor_applied = 0;
        self.dbg_win_xnor_applied = 0;
        self.dbg_math_blocked = 0;
        self.dbg_math_blocked_obj = 0;
        self.dbg_math_blocked_backdrop = 0;
        self.dbg_m7_wrap = 0;
        self.dbg_m7_clip = 0;
        self.dbg_m7_fill = 0;
        self.dbg_m7_bg1 = 0;
        self.dbg_m7_bg2 = 0;
        self.dbg_m7_edge = 0;
        s
    }

    // apply_color_math moved to rendering.rs

    // is_color_math_enabled moved to rendering.rs

    // fixed_color_to_rgb moved to rendering.rs

    // blend_colors moved to rendering.rs

    // apply_mosaic moved to rendering.rs

    // is_mosaic_enabled moved to rendering.rs

    // mode7_transform moved to rendering.rs

    // mode7_world_xy_int moved to rendering.rs

    // render_main_screen_pixel_with_layer_internal moved to rendering.rs

    // render_main_screen_pixel_with_layer moved to rendering.rs

    // render_main_screen_pixel_with_layer_cached moved to rendering.rs

    // get_main_bg_pixel moved to rendering.rs

    // render_sub_screen_pixel moved to rendering.rs

    // render_sub_screen_pixel_with_layer_internal moved to rendering.rs

    // render_sub_screen_pixel_with_layer moved to rendering.rs

    // render_sub_screen_pixel_with_layer_cached moved to rendering.rs

    // get_sub_bg_pixel moved to rendering.rs

    // apply_color_math_screens moved to rendering.rs

    pub fn nmi_pending(&self) -> bool {
        // CPU側へ通知するNMIリクエストは「ラッチ」(edge)で管理する。
        // nmi_flag は $4210(RDNMI) のbit7用で、読み出しでクリアされる。
        // NOTE: $4200 bit7 controls whether the edge is latched,
        // but once latched it should remain pending even if NMI is later disabled.
        self.nmi_latched
    }

    // Expose minimal NMI latch control for $4200 edge cases
    pub fn is_nmi_latched(&self) -> bool {
        self.nmi_latched
    }
    pub fn latch_nmi_now(&mut self) {
        self.nmi_latched = true;
    }

    pub fn get_scanline(&self) -> u16 {
        self.scanline
    }

    pub fn get_frame(&self) -> u64 {
        self.frame
    }

    // Accessors for HVB flags
    pub fn is_vblank(&self) -> bool {
        self.v_blank
    }

    pub fn is_hblank(&self) -> bool {
        self.h_blank
    }

    pub fn get_cycle(&self) -> u16 {
        self.cycle
    }

    pub(crate) fn dots_this_scanline(&self, scanline: u16) -> u16 {
        self.dots_per_scanline(scanline)
    }

    pub(crate) fn remaining_master_cycles_in_frame(&self) -> u64 {
        const MASTER_CYCLES_PER_DOT: u64 = 4;
        self.remaining_dots_in_frame() as u64 * MASTER_CYCLES_PER_DOT
    }

    // --- Write context control (called by Bus before/after DMA bursts) ---
    #[inline]
    pub fn begin_mdma_context(&mut self) {
        self.write_ctx = 1;
    }
    #[inline]
    pub fn end_mdma_context(&mut self) {
        self.write_ctx = 0;
        self.debug_dma_channel = None;
    }
    #[inline]
    pub fn begin_hdma_context(&mut self) {
        self.write_ctx = 2;
    }
    #[inline]
    pub fn end_hdma_context(&mut self) {
        self.write_ctx = 0;
        self.debug_dma_channel = None;
    }

    // Debug helper: mark which DMA channel is currently active
    #[inline]
    pub fn set_debug_dma_channel(&mut self, ch: Option<u8>) {
        self.debug_dma_channel = ch;
    }

    #[inline]
    pub fn arm_burnin_vram_trace(&mut self) {
        self.burnin_vram_trace_armed = true;
        self.burnin_vram_trace_cnt_2118 = 0;
        self.burnin_vram_trace_cnt_2119 = 0;
    }

    // Mark HBlank head guard window for HDMA operations
    pub fn on_hblank_start_guard(&mut self) {
        let hb = self.first_hblank_dot();
        const HDMA_HEAD_GUARD: u16 = 6;
        self.hdma_head_busy_until = hb.saturating_add(HDMA_HEAD_GUARD);
    }

    #[allow(dead_code)]
    pub fn clear_nmi(&mut self) {
        // NMIラッチだけを解除し、RDNMIフラグ（nmi_flag）は保持する。
        // 実機では $4210 読み出しでクリアされるため、CPU側のポーリングに委ねる。
        self.nmi_latched = false;
    }

    // Lightweight usage stats (counts non-zero bytes)
    pub fn vram_usage(&self) -> usize {
        self.vram.iter().filter(|&&b| b != 0).count()
    }

    /// Analyze VRAM content distribution
    pub fn analyze_vram_content(&self) -> (usize, usize, Vec<(usize, u8)>) {
        let mut nonzero_count = 0;
        let mut unique_values = std::collections::HashSet::new();
        let mut samples = Vec::new();

        for (i, &byte) in self.vram.iter().enumerate() {
            if byte != 0 {
                nonzero_count += 1;
                unique_values.insert(byte);
                if samples.len() < 20 {
                    samples.push((i, byte));
                }
            }
        }

        (nonzero_count, unique_values.len(), samples)
    }

    /// Analyze specific VRAM region (word address)
    pub fn analyze_vram_region(&self, word_addr: u16, word_count: usize) -> (usize, Vec<u8>) {
        // Apply VRAM mirroring: addresses 0x8000-0xFFFF mirror to 0x0000-0x7FFF
        let mirrored_addr = word_addr & 0x7FFF;
        let byte_start = (mirrored_addr as usize) * 2;
        let byte_end = (byte_start + word_count * 2).min(self.vram.len());
        let mut nonzero = 0;
        let mut samples = Vec::new();

        for i in byte_start..byte_end {
            if self.vram[i] != 0 {
                nonzero += 1;
                if samples.len() < 16 {
                    samples.push(self.vram[i]);
                }
            }
        }

        (nonzero, samples)
    }

    /// Get VRAM distribution by 4KB blocks
    pub fn get_vram_distribution(&self) -> Vec<(usize, usize)> {
        let block_size = 4096; // 4KB blocks
        let mut distribution = Vec::new();

        for block in 0..(self.vram.len() / block_size) {
            let start = block * block_size;
            let end = (start + block_size).min(self.vram.len());
            let nonzero = self.vram[start..end].iter().filter(|&&b| b != 0).count();
            if nonzero > 0 {
                distribution.push((block * block_size / 2, nonzero)); // word address
            }
        }

        distribution
    }

    pub fn cgram_usage(&self) -> usize {
        self.cgram.iter().filter(|&&b| b != 0).count()
    }

    /// Count non-zero color entries in CGRAM (each color is 2 bytes)
    #[allow(dead_code)]
    pub fn count_nonzero_colors(&self) -> usize {
        self.cgram
            .chunks_exact(2)
            .filter(|chunk| chunk[0] != 0 || chunk[1] != 0)
            .count()
    }

    /// Get BG configuration for debugging
    pub fn get_bg_config(&self, bg_num: u8) -> (u16, u16, bool, u8) {
        let index = (bg_num.saturating_sub(1)) as usize;
        if index >= 4 {
            return (0, 0, false, 0);
        }
        let tile_base = match bg_num {
            1 => self.bg1_tile_base,
            2 => self.bg2_tile_base,
            3 => self.bg3_tile_base,
            4 => self.bg4_tile_base,
            _ => 0,
        };
        let tilemap_base = match bg_num {
            1 => self.bg1_tilemap_base,
            2 => self.bg2_tilemap_base,
            3 => self.bg3_tilemap_base,
            4 => self.bg4_tilemap_base,
            _ => 0,
        };
        (
            tile_base,
            tilemap_base,
            self.bg_tile_16[index],
            self.bg_screen_size[index],
        )
    }

    pub fn get_setini(&self) -> u8 {
        self.setini
    }

    // write_cgram_color moved to palette.rs

    /// Write tilemap entry directly to VRAM (bypassing timing checks)
    #[allow(dead_code)]
    pub fn write_vram_word(&mut self, word_addr: u16, low_byte: u8, high_byte: u8) {
        // VRAM is 32KB words; wrap addresses the way hardware mirrors the 15-bit address.
        let addr = (word_addr as usize) & 0x7FFF; // 15-bit
        let byte_addr = addr * 2;
        if byte_addr + 1 < self.vram.len() {
            self.vram[byte_addr] = low_byte;
            self.vram[byte_addr + 1] = high_byte;
            self.bg_cache_dirty = true;
        }
    }

    pub fn oam_usage(&self) -> usize {
        self.oam.iter().filter(|&&b| b != 0).count()
    }

    // デバッグ用：PPU状態を表示
    pub fn debug_ppu_state(&self) {
        println!("\n=== PPU Debug State ===");
        println!(
            "Scanline: {}, Cycle: {}, Frame: {}",
            self.scanline, self.cycle, self.frame
        );
        println!(
            "Mode: {} (BG3prio={}), SETINI=0x{:02X} (pseudo_hires={}, interlace={}, obj_interlace={}, overscan={}, extbg={})",
            self.bg_mode,
            self.mode1_bg3_priority,
            self.setini,
            self.pseudo_hires,
            self.interlace,
            self.obj_interlace,
            self.overscan,
            self.extbg
        );
        println!(
            "Main Screen: 0x{:02X}, Sub Screen: 0x{:02X}",
            self.main_screen_designation, self.sub_screen_designation
        );
        println!(
            "Color Math: CGWSEL=0x{:02X} CGADSUB=0x{:02X} fixed=0x{:04X}",
            self.cgwsel, self.cgadsub, self.fixed_color
        );
        println!(
            "Windows: W1=({}, {}) W2=({}, {}) W12SEL=0x{:02X} W34SEL=0x{:02X} WOBJSEL(obj=0x{:01X} col=0x{:01X}) WBGLOG=[{}, {}, {}, {}] WOBJLOG(obj={} col={}) TMW=0x{:02X} TSW=0x{:02X}",
            self.window1_left,
            self.window1_right,
            self.window2_left,
            self.window2_right,
            ((self.window_bg_mask[1] & 0x0F) << 4) | (self.window_bg_mask[0] & 0x0F),
            ((self.window_bg_mask[3] & 0x0F) << 4) | (self.window_bg_mask[2] & 0x0F),
            (self.window_obj_mask & 0x0F),
            (self.window_color_mask & 0x0F),
            self.bg_window_logic[0],
            self.bg_window_logic[1],
            self.bg_window_logic[2],
            self.bg_window_logic[3],
            self.obj_window_logic,
            self.color_window_logic,
            self.tmw_mask,
            self.tsw_mask
        );
        println!(
            "OAM: addr=0x{:03X} internal=0x{:03X} eval_base={} rotation={}",
            self.oam_addr,
            self.oam_internal_addr,
            self.oam_eval_base,
            self.oam_priority_rotation_enabled
        );
        println!("Screen Display: 0x{:02X}", self.screen_display);
        println!("NMI: enabled={}, flag={}", self.nmi_enabled, self.nmi_flag);

        // BGレイヤー設定
        println!(
            "BG1: tilemap=0x{:04X}, tile=0x{:04X}, scroll=({},{})",
            self.bg1_tilemap_base, self.bg1_tile_base, self.bg1_hscroll, self.bg1_vscroll
        );
        println!(
            "BG2: tilemap=0x{:04X}, tile=0x{:04X}, scroll=({},{})",
            self.bg2_tilemap_base, self.bg2_tile_base, self.bg2_hscroll, self.bg2_vscroll
        );
        println!(
            "BG3: tilemap=0x{:04X}, tile=0x{:04X}, scroll=({},{})",
            self.bg3_tilemap_base, self.bg3_tile_base, self.bg3_hscroll, self.bg3_vscroll
        );
        println!(
            "BG4: tilemap=0x{:04X}, tile=0x{:04X}, scroll=({},{})",
            self.bg4_tilemap_base, self.bg4_tile_base, self.bg4_hscroll, self.bg4_vscroll
        );
        println!(
            "BG tile16: [{},{},{},{}] screen_size: [{},{},{},{}]",
            self.bg_tile_16[0],
            self.bg_tile_16[1],
            self.bg_tile_16[2],
            self.bg_tile_16[3],
            self.bg_screen_size[0],
            self.bg_screen_size[1],
            self.bg_screen_size[2],
            self.bg_screen_size[3]
        );

        // スプライト設定
        println!(
            "Sprite: size={}, name_base=0x{:04X}, name_select=0x{:04X}",
            self.sprite_size, self.sprite_name_base, self.sprite_name_select
        );

        // VRAM/CGRAM状態
        let vram_used = self.vram.iter().filter(|&&b| b != 0).count();
        let cgram_used = self.cgram.iter().filter(|&&b| b != 0).count();
        println!(
            "VRAM: {}/{} bytes used, CGRAM: {}/{} bytes used",
            vram_used,
            self.vram.len(),
            cgram_used,
            self.cgram.len()
        );

        // 最初の8個のCGRAMエントリ表示（パレット0）
        print!("Palette 0: ");
        for i in 0..8 {
            let color = self.cgram_to_rgb(i);
            print!("${:06X} ", color & 0xFFFFFF);
        }
        println!();

        println!("=======================");
    }

    // テストパターンを強制表示（デバッグ用）
    pub fn force_test_pattern(&mut self) {
        println!("Forcing test pattern display...");

        // テストパターン表示のため基本的なPPU設定を上書き
        self.brightness = 15;
        self.main_screen_designation = 0x1F; // 全BGレイヤーとスプライトを有効
        self.screen_display = 0; // forced blank off (表示有効)

        // Dragon Quest III fix: Fill VRAM with test data
        for i in 0..0x8000 {
            self.vram[i] = if i < 0x4000 { 0x11 } else { 0x22 };
        }
        self.bg_cache_dirty = true;

        // Set up tilemap entries at high addresses (0xE000-0xFFFF range)
        let tilemap_start = 0x6000; // Start from 0xE000 & 0x7FFF = 0x6000
        for i in (tilemap_start..tilemap_start + 0x800).step_by(2) {
            if i + 1 < self.vram.len() {
                self.vram[i] = 0x01; // Tile ID low
                self.vram[i + 1] = 0x00; // Tile ID high + attributes
            }
        }

        // Set up tile data at 0x6000+ range
        let tile_start = 0x4000; // Start from 0xE000 & 0x7FFF = 0x6000
        for i in tile_start..tile_start + 0x100 {
            if i < self.vram.len() {
                self.vram[i] = 0xFF; // White tile data
            }
        }

        // Fill CGRAM with test colors
        // Palette 0: Background colors
        self.cgram[0] = 0x00;
        self.cgram[1] = 0x00; // Color 0: Black (transparent)
        self.cgram[2] = 0xFF;
        self.cgram[3] = 0x7F; // Color 1: White
        self.cgram[4] = 0x1F;
        self.cgram[5] = 0x00; // Color 2: Red
        self.cgram[6] = 0xE0;
        self.cgram[7] = 0x03; // Color 3: Green

        // Palette 1-7: Fill with distinct colors
        for palette in 1..8 {
            let base = palette * 16 * 2;
            for color in 0..16 {
                let addr = base + color * 2;
                if addr + 1 < self.cgram.len() {
                    // Create distinct colors for each palette
                    let r = ((palette * 4) & 0x1F) as u16;
                    let g = ((color * 2) & 0x1F) as u16;
                    let b = ((palette + color) & 0x1F) as u16;
                    let color_val = (b << 10) | (g << 5) | r;
                    self.cgram[addr] = (color_val & 0xFF) as u8;
                    self.cgram[addr + 1] = ((color_val >> 8) & 0x7F) as u8;
                }
            }
        }
        self.rebuild_cgram_rgb_cache();

        println!(
            "PPU: Test pattern applied (brightness={}, layers=0x{:02X}) with VRAM test data",
            self.brightness, self.main_screen_designation
        );
    }
}

// --------------------------- tests ---------------------------
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_state_roundtrip_preserves_framebuffers() {
        let mut src = Ppu::new();
        src.framebuffer[0] = 0xFF112233;
        src.framebuffer[255] = 0xFF445566;
        src.subscreen_buffer[2] = 0xFF102030;
        src.subscreen_buffer[257] = 0xFF405060;
        src.render_framebuffer[1] = 0xFF778899;
        src.render_framebuffer[256] = 0xFFAABBCC;
        src.render_subscreen_buffer[3] = 0xFF0A0B0C;
        src.render_subscreen_buffer[258] = 0xFF0D0E0F;
        src.framebuffer_rendering_enabled = false;
        src.main_screen_designation_last_nonzero = 0x1F;
        src.vram_read_buf_lo = 0x12;
        src.vram_read_buf_hi = 0x34;
        src.cgram_read_second = true;
        src.interlace = true;
        src.obj_interlace = true;
        src.force_no_blank = true;
        src.superfx_bypass_bg1_window = true;
        src.superfx_authoritative_bg1_source = true;
        src.superfx_direct_buffer = vec![1, 2, 3, 4];
        src.superfx_direct_height = 160;
        src.superfx_direct_bpp = 4;
        src.superfx_direct_mode = 2;
        src.superfx_tile_buffer = vec![5, 6, 7, 8];
        src.superfx_tile_bpp = 4;
        src.superfx_tile_mode = 1;
        src.wio_latch_enable = true;
        src.stat78_latch_flag = true;
        src.interlace_field = true;
        src.sprite_overflow = true;
        src.sprite_time_over = true;
        src.sprite_overflow_latched = true;
        src.sprite_time_over_latched = true;

        let state = src.to_save_state();
        let mut dst = Ppu::new();
        dst.load_from_save_state(&state);

        assert_eq!(dst.framebuffer[0], 0xFF112233);
        assert_eq!(dst.framebuffer[255], 0xFF445566);
        assert_eq!(dst.subscreen_buffer[2], 0xFF102030);
        assert_eq!(dst.subscreen_buffer[257], 0xFF405060);
        assert_eq!(dst.render_framebuffer[1], 0xFF778899);
        assert_eq!(dst.render_framebuffer[256], 0xFFAABBCC);
        assert_eq!(dst.render_subscreen_buffer[3], 0xFF0A0B0C);
        assert_eq!(dst.render_subscreen_buffer[258], 0xFF0D0E0F);
        assert!(!dst.framebuffer_rendering_enabled);
        assert_eq!(dst.main_screen_designation_last_nonzero, 0x1F);
        assert_eq!(dst.vram_read_buf_lo, 0x12);
        assert_eq!(dst.vram_read_buf_hi, 0x34);
        assert!(dst.cgram_read_second);
        assert!(dst.interlace);
        assert!(dst.obj_interlace);
        assert!(dst.force_no_blank);
        assert!(dst.superfx_bypass_bg1_window);
        assert!(dst.superfx_authoritative_bg1_source);
        assert_eq!(dst.superfx_direct_buffer, vec![1, 2, 3, 4]);
        assert_eq!(dst.superfx_direct_height, 160);
        assert_eq!(dst.superfx_direct_bpp, 4);
        assert_eq!(dst.superfx_direct_mode, 2);
        assert_eq!(dst.superfx_tile_buffer, vec![5, 6, 7, 8]);
        assert_eq!(dst.superfx_tile_bpp, 4);
        assert_eq!(dst.superfx_tile_mode, 1);
        assert!(dst.wio_latch_enable);
        assert!(dst.stat78_latch_flag);
        assert!(dst.interlace_field);
        assert!(dst.sprite_overflow);
        assert!(dst.sprite_time_over);
        assert!(dst.sprite_overflow_latched);
        assert!(dst.sprite_time_over_latched);
    }

    #[test]
    fn cgram_rgb555_to_rgb888_mapping() {
        let mut ppu = Ppu::new();
        // RGB555 (SNES): bit0-4=R, 5-9=G, 10-14=B.
        ppu.write_cgram_color(0, 0x001F); // red
        ppu.write_cgram_color(1, 0x03E0); // green
        ppu.write_cgram_color(2, 0x7C00); // blue
        ppu.write_cgram_color(3, 0x7FFF); // white

        assert_eq!(ppu.cgram_to_rgb(0), 0xFFFF0000);
        assert_eq!(ppu.cgram_to_rgb(1), 0xFF00FF00);
        assert_eq!(ppu.cgram_to_rgb(2), 0xFF0000FF);
        assert_eq!(ppu.cgram_to_rgb(3), 0xFFFFFFFF);
    }

    #[test]
    fn coldata_updates_fixed_color_components() {
        let mut ppu = Ppu::new();
        // Set R=31, G=0, B=0
        ppu.write(0x32, 0x20 | 0x1F); // R enable + intensity
        ppu.write(0x32, 0x40 | 0x00); // G enable + intensity
        ppu.write(0x32, 0x80 | 0x00); // B enable + intensity
        assert_eq!(ppu.fixed_color_to_rgb(), 0xFFFF0000);

        // Set R=0, G=31, B=0
        ppu.write(0x32, 0x20 | 0x00);
        ppu.write(0x32, 0x40 | 0x1F);
        ppu.write(0x32, 0x80 | 0x00);
        assert_eq!(ppu.fixed_color_to_rgb(), 0xFF00FF00);

        // Set R=0, G=0, B=31
        ppu.write(0x32, 0x20 | 0x00);
        ppu.write(0x32, 0x40 | 0x00);
        ppu.write(0x32, 0x80 | 0x1F);
        assert_eq!(ppu.fixed_color_to_rgb(), 0xFF0000FF);
    }

    #[test]
    fn ntsc_odd_field_non_interlace_shortens_scanline_240() {
        let mut ppu = Ppu::new();
        ppu.interlace = false;
        ppu.interlace_field = true;
        ppu.scanline = 240;
        ppu.cycle = 339;

        ppu.step(1);

        assert_eq!(ppu.scanline, 241);
        assert_eq!(ppu.cycle, 0);
    }

    #[test]
    fn ntsc_even_field_interlace_adds_extra_scanline() {
        let mut ppu = Ppu::new();
        ppu.interlace = true;
        ppu.interlace_field = false;
        ppu.v_blank = true;
        ppu.scanline = 261;
        ppu.cycle = 340;

        ppu.step(1);

        assert_eq!(ppu.scanline, 262);
        assert_eq!(ppu.cycle, 0);
        assert_eq!(ppu.frame, 0);

        ppu.cycle = 340;
        ppu.step(1);

        assert_eq!(ppu.scanline, 0);
        assert_eq!(ppu.cycle, 0);
        assert_eq!(ppu.frame, 1);
    }

    #[test]
    fn forced_blank_allows_non_hdma_graphics_writes_outside_vblank() {
        let mut ppu = Ppu::new();
        ppu.screen_display = 0x80;
        ppu.v_blank = false;
        ppu.h_blank = false;
        ppu.scanline = 42;
        ppu.cycle = 12;

        assert!(ppu.can_write_vram_non_hdma_now());
        assert!(ppu.can_write_cgram_non_hdma_now());
        assert!(ppu.can_write_oam_non_hdma_now());
    }

    #[test]
    fn active_hblank_allows_non_hdma_vram_and_cgram_but_not_oam() {
        let mut ppu = Ppu::new();
        ppu.screen_display = 0x00;
        ppu.v_blank = false;
        ppu.h_blank = true;
        ppu.scanline = 10;
        ppu.cycle = ppu.first_hblank_dot().saturating_add(16);

        assert!(ppu.can_write_vram_non_hdma_now());
        assert!(ppu.can_write_cgram_non_hdma_now());
        assert!(!ppu.can_write_oam_non_hdma_now());
    }

    #[test]
    fn invalid_oam_write_does_not_change_latch_memory_or_internal_address() {
        let mut ppu = Ppu::new();
        ppu.screen_display = 0x00;
        ppu.scanline = 0;
        ppu.cycle = 32;
        ppu.v_blank = false;
        ppu.h_blank = false;
        ppu.oam_write_latch = 0xCC;
        ppu.oam_internal_addr = 0;
        ppu.oam[0] = 0x11;
        ppu.oam[1] = 0x22;

        ppu.write(0x04, 0x77);

        assert_eq!(ppu.oam_write_latch, 0xCC);
        assert_eq!(ppu.oam_internal_addr, 0);
        assert_eq!(ppu.oam[0], 0x11);
        assert_eq!(ppu.oam[1], 0x22);
    }

    #[test]
    fn invalid_cgram_write_does_not_stage_or_advance_address() {
        let mut ppu = Ppu::new();
        ppu.screen_display = 0x00;
        ppu.scanline = 0;
        ppu.cycle = 32;
        ppu.v_blank = false;
        ppu.h_blank = false;
        ppu.cgram_addr = 0;
        ppu.cgram_latch_lo = 0xAA;
        ppu.cgram_second = false;
        ppu.cgram[0] = 0x34;
        ppu.cgram[1] = 0x12;

        ppu.write(0x22, 0x56);

        assert_eq!(ppu.cgram_latch_lo, 0xAA);
        assert!(!ppu.cgram_second);
        assert_eq!(ppu.cgram_addr, 0);
        assert_eq!(ppu.cgram[0], 0x34);
        assert_eq!(ppu.cgram[1], 0x12);
    }

    #[test]
    fn pending_vmadd_commit_updates_vram_address_and_read_latch() {
        let mut ppu = Ppu::new();
        ppu.screen_display = 0x80;
        ppu.vram_mapping = 0x00;
        ppu.vram_addr = 0;
        ppu.latched_vmadd_lo = Some(0x02);
        ppu.latched_vmadd_hi = Some(0x00);
        ppu.vram[4] = 0xAB;
        ppu.vram[5] = 0xCD;

        ppu.commit_pending_ctrl_if_any();

        assert_eq!(ppu.vram_addr, 0x0002);
        assert!(ppu.latched_vmadd_lo.is_none());
        assert!(ppu.latched_vmadd_hi.is_none());
        assert_eq!(ppu.vram_read_buf_lo, 0xAB);
        assert_eq!(ppu.vram_read_buf_hi, 0xCD);
    }

    #[test]
    fn deferred_vmain_effect_updates_mapping_and_starts_gap() {
        let mut ppu = Ppu::new();
        ppu.vram_mapping = 0x00;
        ppu.vram_last_vmain = 0x00;
        ppu.vram_increment = 1;
        ppu.vmain_effect_pending = Some(0x81);
        ppu.vmain_effect_ticks = 1;
        ppu.vmain_data_gap_ticks = 0;

        ppu.tick_deferred_ctrl_effects();

        assert!(ppu.vmain_effect_pending.is_none());
        assert_eq!(ppu.vram_mapping, 0x81);
        assert_eq!(ppu.vram_last_vmain, 0x81);
        assert_eq!(ppu.vram_increment, 32);
        assert_eq!(
            ppu.vmain_data_gap_ticks,
            crate::debug_flags::vram_gap_after_vmain().saturating_sub(1)
        );
    }

    #[test]
    fn deferred_cgadd_effect_resets_staging_and_starts_gap() {
        let mut ppu = Ppu::new();
        ppu.cgram_addr = 0x10;
        ppu.cgram_second = true;
        ppu.cgram_read_second = true;
        ppu.cgadd_effect_pending = Some(0x3C);
        ppu.cgadd_effect_ticks = 1;
        ppu.cgram_data_gap_ticks = 0;

        ppu.tick_deferred_ctrl_effects();

        assert!(ppu.cgadd_effect_pending.is_none());
        assert_eq!(ppu.cgram_addr, 0x3C);
        assert!(!ppu.cgram_second);
        assert!(!ppu.cgram_read_second);
        assert_eq!(
            ppu.cgram_data_gap_ticks,
            crate::debug_flags::cgram_gap_after_cgadd()
        );
    }

    #[test]
    fn oamadd_low_write_updates_internal_addr_and_gap() {
        let mut ppu = Ppu::new();
        ppu.oam_addr = 0x100;
        ppu.oam_priority_rotation_enabled = true;
        ppu.oam_eval_base = 0;
        ppu.oam_data_gap_ticks = 0;

        ppu.write(0x02, 0x02);

        assert_eq!(ppu.oam_addr, 0x102);
        assert_eq!(ppu.oam_internal_addr, 0x204);
        assert_eq!(
            ppu.oam_eval_base,
            ((ppu.oam_internal_addr >> 2) & 0x7F) as u8
        );
        assert_eq!(
            ppu.oam_data_gap_ticks,
            crate::debug_flags::oam_gap_after_oamadd()
        );
    }

    #[test]
    fn oamadd_high_write_updates_rotation_mode_and_eval_base() {
        let mut ppu = Ppu::new();
        ppu.oam_addr = 0x002;
        ppu.oam_internal_addr = 0x004;
        ppu.oam_priority_rotation_enabled = false;
        ppu.oam_eval_base = 0;
        ppu.oam_data_gap_ticks = 0;

        ppu.write(0x03, 0x81);

        assert_eq!(ppu.oam_addr, 0x102);
        assert_eq!(ppu.oam_internal_addr, 0x204);
        assert!(ppu.oam_priority_rotation_enabled);
        assert_eq!(
            ppu.oam_eval_base,
            ((ppu.oam_internal_addr >> 2) & 0x7F) as u8
        );
        assert_eq!(
            ppu.oam_data_gap_ticks,
            crate::debug_flags::oam_gap_after_oamadd()
        );
    }

    #[test]
    fn oamadd_high_write_disabling_rotation_resets_eval_base() {
        let mut ppu = Ppu::new();
        ppu.oam_addr = 0x102;
        ppu.oam_internal_addr = 0x204;
        ppu.oam_priority_rotation_enabled = true;
        ppu.oam_eval_base = ((ppu.oam_internal_addr >> 2) & 0x7F) as u8;

        ppu.write(0x03, 0x01);

        assert_eq!(ppu.oam_addr, 0x102);
        assert_eq!(ppu.oam_internal_addr, 0x204);
        assert!(!ppu.oam_priority_rotation_enabled);
        assert_eq!(ppu.oam_eval_base, 0);
    }

    #[test]
    fn enter_vblank_resets_oam_internal_addr_when_display_enabled() {
        let mut ppu = Ppu::new();
        ppu.screen_display = 0x00;
        ppu.oam_addr = 0x102;
        ppu.oam_internal_addr = 0x000;
        ppu.oam_priority_rotation_enabled = true;
        ppu.oam_eval_base = 0;

        ppu.enter_vblank();

        assert!(ppu.v_blank);
        assert_eq!(ppu.oam_internal_addr, 0x204);
        assert_eq!(
            ppu.oam_eval_base,
            ((ppu.oam_internal_addr >> 2) & 0x7F) as u8
        );
    }

    #[test]
    fn enter_vblank_does_not_reset_oam_internal_addr_during_forced_blank() {
        let mut ppu = Ppu::new();
        ppu.screen_display = 0x80;
        ppu.oam_addr = 0x102;
        ppu.oam_internal_addr = 0x066;
        ppu.oam_priority_rotation_enabled = true;
        ppu.oam_eval_base = 0x19;

        ppu.enter_vblank();

        assert!(ppu.v_blank);
        assert_eq!(ppu.oam_internal_addr, 0x066);
        assert_eq!(ppu.oam_eval_base, 0x19);
    }

    #[test]
    fn forced_blank_deactivation_resets_oam_internal_addr() {
        let mut ppu = Ppu::new();
        ppu.oam_addr = 0x102;
        ppu.oam_internal_addr = 0x000;
        ppu.oam_priority_rotation_enabled = true;
        ppu.oam_eval_base = 0;

        ppu.maybe_reset_oam_on_inidisp(0x80, 0x00);

        assert_eq!(ppu.oam_internal_addr, 0x204);
        assert_eq!(
            ppu.oam_eval_base,
            ((ppu.oam_internal_addr >> 2) & 0x7F) as u8
        );
    }

    #[test]
    fn latched_inidisp_toggle_does_not_rebuild_and_clear_prior_scanlines() {
        let mut ppu = Ppu::new();
        ppu.screen_display = 0x0F;
        ppu.brightness = 0x0F;
        ppu.scanline = 120;
        ppu.cycle = 0;
        ppu.render_framebuffer[0] = 0xFF12_3456;
        ppu.render_subscreen_buffer[0] = 0x0000_00AA;
        ppu.latched_inidisp = Some(0x80);

        ppu.commit_latched_display_regs();

        assert_eq!(ppu.screen_display, 0x80);
        assert_eq!(ppu.brightness, 0x00);
        assert_eq!(ppu.render_framebuffer[0], 0xFF12_3456);
        assert_eq!(ppu.render_subscreen_buffer[0], 0x0000_00AA);
    }

    #[test]
    fn immediate_inidisp_toggle_does_not_rebuild_and_clear_prior_scanlines() {
        let mut ppu = Ppu::new();
        ppu.screen_display = 0x0F;
        ppu.brightness = 0x0F;
        ppu.scanline = 120;
        ppu.cycle = 128;
        ppu.render_framebuffer[0] = 0xFF12_3456;
        ppu.render_subscreen_buffer[0] = 0x0000_00AA;

        ppu.write(0x00, 0x80);

        assert_eq!(ppu.screen_display, 0x80);
        assert_eq!(ppu.brightness, 0x00);
        assert_eq!(ppu.render_framebuffer[0], 0xFF12_3456);
        assert_eq!(ppu.render_subscreen_buffer[0], 0x0000_00AA);
    }

    #[test]
    fn enter_vblank_sets_nmi_flag_and_resets_rdnmi_consumed_state() {
        let mut ppu = Ppu::new();
        ppu.nmi_enabled = false;
        ppu.nmi_flag = false;
        ppu.nmi_latched = false;
        ppu.rdnmi_read_in_vblank = true;

        ppu.enter_vblank();

        assert!(ppu.v_blank);
        assert!(ppu.nmi_flag);
        assert!(!ppu.nmi_latched);
        assert!(!ppu.rdnmi_read_in_vblank);
    }

    #[test]
    fn enter_vblank_latches_nmi_only_when_enabled() {
        let mut ppu = Ppu::new();
        ppu.nmi_enabled = true;
        ppu.nmi_flag = false;
        ppu.nmi_latched = false;

        ppu.enter_vblank();
        assert!(ppu.nmi_flag);
        assert!(ppu.nmi_latched);

        let mut ppu = Ppu::new();
        ppu.nmi_enabled = false;
        ppu.nmi_flag = false;
        ppu.nmi_latched = false;

        ppu.enter_vblank();
        assert!(ppu.nmi_flag);
        assert!(!ppu.nmi_latched);
    }

    #[test]
    fn clear_nmi_only_clears_latch_not_rdnmi_flag() {
        let mut ppu = Ppu::new();
        ppu.nmi_flag = true;
        ppu.nmi_latched = true;

        ppu.clear_nmi();

        assert!(ppu.nmi_flag);
        assert!(!ppu.nmi_latched);
    }

    #[test]
    fn stat78_read_reports_and_clears_latch_flag() {
        let mut ppu = Ppu::new();
        ppu.interlace_field = true;
        ppu.stat78_latch_flag = true;
        ppu.ophct_second = true;
        ppu.opvct_second = true;

        let value = ppu.read(0x3F);

        assert_eq!(value & 0xC0, 0xC0);
        assert_eq!(value & 0x0F, 0x03);
        assert!(!ppu.stat78_latch_flag);
        assert!(!ppu.ophct_second);
        assert!(!ppu.opvct_second);
    }

    #[test]
    fn enter_vblank_toggles_interlace_field_each_time() {
        let mut ppu = Ppu::new();
        ppu.interlace_field = false;

        ppu.enter_vblank();
        assert!(ppu.interlace_field);

        ppu.v_blank = false;
        ppu.enter_vblank();
        assert!(!ppu.interlace_field);
    }

    #[test]
    fn exit_vblank_clears_sprite_latches_but_keeps_nmi_flag() {
        let mut ppu = Ppu::new();
        ppu.v_blank = true;
        ppu.nmi_flag = true;
        ppu.nmi_latched = true;
        ppu.rdnmi_read_in_vblank = true;
        ppu.sprite_overflow_latched = true;
        ppu.sprite_time_over_latched = true;

        ppu.exit_vblank();

        assert!(!ppu.v_blank);
        assert!(ppu.nmi_flag);
        assert!(ppu.nmi_latched);
        assert!(!ppu.rdnmi_read_in_vblank);
        assert!(!ppu.sprite_overflow_latched);
        assert!(!ppu.sprite_time_over_latched);
    }

    #[test]
    fn slhv_read_latches_hv_counters_one_dot_later() {
        let mut ppu = Ppu::new();
        ppu.scanline = 0x34;
        ppu.cycle = 0x56;
        ppu.hv_latched_h = 0;
        ppu.hv_latched_v = 0;
        ppu.stat78_latch_flag = false;

        let value = ppu.read(0x37);
        assert_eq!(value, 0);
        assert_eq!(ppu.slhv_latch_pending_dots, 1);
        assert_eq!(ppu.hv_latched_h, 0);
        assert_eq!(ppu.hv_latched_v, 0);

        ppu.step(1);

        assert_eq!(ppu.hv_latched_h, 0x57);
        assert_eq!(ppu.hv_latched_v, 0x34);
        assert!(ppu.stat78_latch_flag);
        assert_eq!(ppu.slhv_latch_pending_dots, 0);
    }

    #[test]
    fn ophct_and_opvct_reads_toggle_low_then_high_bit() {
        let mut ppu = Ppu::new();
        ppu.hv_latched_h = 0x123;
        ppu.hv_latched_v = 0x0AB;

        assert_eq!(ppu.read(0x3C), 0x23);
        assert_eq!(ppu.read(0x3C), 0x01);
        assert_eq!(ppu.read(0x3D), 0xAB);
        assert_eq!(ppu.read(0x3D), 0x00);
    }

    #[test]
    fn latch_hv_counters_resets_ophct_and_opvct_selectors() {
        let mut ppu = Ppu::new();
        ppu.hv_latched_h = 0x155;
        ppu.hv_latched_v = 0x1AA;

        let _ = ppu.read(0x3C);
        let _ = ppu.read(0x3D);
        assert!(ppu.ophct_second);
        assert!(ppu.opvct_second);

        ppu.scanline = 0x12;
        ppu.cycle = 0x34;
        ppu.latch_hv_counters();

        assert!(!ppu.ophct_second);
        assert!(!ppu.opvct_second);
        assert_eq!(ppu.read(0x3C), 0x34);
        assert_eq!(ppu.read(0x3D), 0x12);
    }

    #[test]
    fn ophct_opvct_reads_realize_pending_slhv_latch_immediately() {
        let mut ppu = Ppu::new();
        ppu.scanline = 0x34;
        ppu.cycle = 0x56;
        ppu.hv_latched_h = 0;
        ppu.hv_latched_v = 0;

        let _ = ppu.read(0x37);

        assert_eq!(ppu.read(0x3D), 0x34);
        assert_eq!(ppu.slhv_latch_pending_dots, 0);
        assert!(ppu.stat78_latch_flag);
    }

    #[test]
    fn invalid_vram_low_write_keeps_memory_unchanged_and_increments_only_in_low_mode() {
        let mut ppu = Ppu::new();
        ppu.screen_display = 0x00;
        ppu.scanline = 0;
        ppu.cycle = 32;
        ppu.v_blank = false;
        ppu.h_blank = false;
        ppu.vram_increment = 1;
        ppu.vram_addr = 0;
        ppu.vram[0] = 0x00;

        ppu.vram_mapping = 0x00;
        ppu.write(0x18, 0x12);
        assert_eq!(ppu.vram[0], 0x00);
        assert_eq!(ppu.vram_addr, 1);

        ppu.vram_addr = 0;
        ppu.vram_mapping = 0x80;
        ppu.write(0x18, 0x34);
        assert_eq!(ppu.vram[0], 0x00);
        assert_eq!(ppu.vram_addr, 0);
    }

    #[test]
    fn invalid_vram_high_write_keeps_memory_unchanged_and_increments_only_in_high_mode() {
        let mut ppu = Ppu::new();
        ppu.screen_display = 0x00;
        ppu.scanline = 0;
        ppu.cycle = 32;
        ppu.v_blank = false;
        ppu.h_blank = false;
        ppu.vram_increment = 1;
        ppu.vram_addr = 0;
        ppu.vram[1] = 0x00;

        ppu.vram_mapping = 0x80;
        ppu.write(0x19, 0x12);
        assert_eq!(ppu.vram[1], 0x00);
        assert_eq!(ppu.vram_addr, 1);

        ppu.vram_addr = 0;
        ppu.vram_mapping = 0x00;
        ppu.write(0x19, 0x34);
        assert_eq!(ppu.vram[1], 0x00);
        assert_eq!(ppu.vram_addr, 0);
    }

    #[test]
    fn vblank_window_blocks_first_scanline_before_head_guard() {
        assert!(!Ppu::vblank_window_open(225, 3, 225, 261, 340, 4, 0));
        assert!(Ppu::vblank_window_open(225, 4, 225, 261, 340, 4, 0));
    }

    #[test]
    fn vblank_window_blocks_last_scanline_after_tail_guard() {
        assert!(Ppu::vblank_window_open(261, 336, 225, 261, 340, 0, 4));
        assert!(!Ppu::vblank_window_open(261, 337, 225, 261, 340, 0, 4));
    }

    #[test]
    fn vblank_window_stays_closed_before_vblank_begins() {
        assert!(!Ppu::vblank_window_open(224, 100, 225, 261, 340, 0, 0));
    }

    #[test]
    fn hblank_window_blocks_before_head_guard() {
        assert!(!Ppu::hblank_window_open(281, 278, 340, 4, 0, 0));
        assert!(Ppu::hblank_window_open(282, 278, 340, 4, 0, 0));
    }

    #[test]
    fn hblank_window_blocks_after_tail_guard() {
        assert!(Ppu::hblank_window_open(336, 278, 340, 0, 4, 0));
        assert!(!Ppu::hblank_window_open(337, 278, 340, 0, 4, 0));
    }

    #[test]
    fn hblank_window_respects_busy_until_guard() {
        assert!(!Ppu::hblank_window_open(289, 278, 340, 4, 0, 290));
        assert!(Ppu::hblank_window_open(290, 278, 340, 4, 0, 290));
    }

    #[test]
    fn cgram_non_hdma_write_requires_actual_hblank_cycle() {
        let mut ppu = Ppu::new();
        ppu.screen_display = 0x00;
        ppu.v_blank = false;
        ppu.h_blank = true;
        ppu.scanline = 10;
        ppu.cgram_data_gap_ticks = 0;

        ppu.cycle = ppu.first_hblank_dot().saturating_sub(1);
        assert!(!ppu.can_write_cgram_non_hdma_now());

        ppu.cycle = ppu.first_hblank_dot();
        assert!(!ppu.can_write_cgram_non_hdma_now());

        ppu.cycle = ppu.first_hblank_dot() + crate::debug_flags::cgram_mdma_head();
        assert!(ppu.can_write_cgram_non_hdma_now());
    }

    #[test]
    fn cgram_non_hdma_write_respects_gap_ticks_inside_hblank() {
        let mut ppu = Ppu::new();
        ppu.screen_display = 0x00;
        ppu.v_blank = false;
        ppu.h_blank = true;
        ppu.scanline = 10;
        ppu.cycle = ppu.first_hblank_dot();
        ppu.cgram_data_gap_ticks = 1;

        assert!(!ppu.can_write_cgram_non_hdma_now());
    }

    #[test]
    fn cgram_non_hdma_write_respects_tail_guard() {
        let mut ppu = Ppu::new();
        ppu.screen_display = 0x00;
        ppu.v_blank = false;
        ppu.h_blank = true;
        ppu.scanline = 10;
        ppu.cgram_data_gap_ticks = 0;

        ppu.cycle = ppu
            .last_dot_index()
            .saturating_sub(crate::debug_flags::cgram_mdma_tail());
        assert!(ppu.can_write_cgram_non_hdma_now());

        ppu.cycle = ppu
            .last_dot_index()
            .saturating_sub(crate::debug_flags::cgram_mdma_tail())
            + 1;
        assert!(!ppu.can_write_cgram_non_hdma_now());
    }

    #[test]
    fn oam_vblank_write_window_blocks_before_head_guard() {
        assert!(!Ppu::oam_vblank_write_window_open(
            225, 3, 225, 261, 340, 4, 0, false, 0
        ));
        assert!(Ppu::oam_vblank_write_window_open(
            225, 4, 225, 261, 340, 4, 0, false, 0
        ));
    }

    #[test]
    fn oam_vblank_write_window_blocks_after_tail_guard() {
        assert!(Ppu::oam_vblank_write_window_open(
            261, 336, 225, 261, 340, 0, 4, false, 0
        ));
        assert!(!Ppu::oam_vblank_write_window_open(
            261, 337, 225, 261, 340, 0, 4, false, 0
        ));
    }

    #[test]
    fn oam_vblank_write_window_respects_gap_block() {
        assert!(!Ppu::oam_vblank_write_window_open(
            230, 100, 225, 261, 340, 0, 0, true, 1
        ));
        assert!(Ppu::oam_vblank_write_window_open(
            230, 100, 225, 261, 340, 0, 0, true, 0
        ));
    }

    #[test]
    fn vram_non_hdma_write_respects_head_guard() {
        let mut ppu = Ppu::new();
        ppu.screen_display = 0x00;
        ppu.v_blank = false;
        ppu.h_blank = true;
        ppu.scanline = 10;
        ppu.vmain_data_gap_ticks = 0;
        ppu.hdma_head_busy_until = 0;

        ppu.cycle = ppu
            .first_hblank_dot()
            .saturating_add(crate::debug_flags::vram_mdma_head())
            .saturating_sub(1);
        assert!(!ppu.can_write_vram_non_hdma_now());

        ppu.cycle = ppu
            .first_hblank_dot()
            .saturating_add(crate::debug_flags::vram_mdma_head());
        assert!(ppu.can_write_vram_non_hdma_now());
    }

    #[test]
    fn vram_non_hdma_write_respects_busy_until_guard() {
        let mut ppu = Ppu::new();
        ppu.screen_display = 0x00;
        ppu.v_blank = false;
        ppu.h_blank = true;
        ppu.scanline = 10;
        ppu.vmain_data_gap_ticks = 0;
        ppu.hdma_head_busy_until = ppu
            .first_hblank_dot()
            .saturating_add(crate::debug_flags::vram_mdma_head())
            .saturating_add(5);

        ppu.cycle = ppu.hdma_head_busy_until.saturating_sub(1);
        assert!(!ppu.can_write_vram_non_hdma_now());

        ppu.cycle = ppu.hdma_head_busy_until;
        assert!(ppu.can_write_vram_non_hdma_now());
    }

    #[test]
    fn vram_non_hdma_write_respects_gap_ticks_inside_hblank() {
        let mut ppu = Ppu::new();
        ppu.screen_display = 0x00;
        ppu.v_blank = false;
        ppu.h_blank = true;
        ppu.scanline = 10;
        ppu.hdma_head_busy_until = 0;
        ppu.cycle = ppu
            .first_hblank_dot()
            .saturating_add(crate::debug_flags::vram_mdma_head());
        ppu.vmain_data_gap_ticks = 1;

        assert!(!ppu.can_write_vram_non_hdma_now());
    }

    #[test]
    fn vram_non_hdma_write_respects_tail_guard() {
        let mut ppu = Ppu::new();
        ppu.screen_display = 0x00;
        ppu.v_blank = false;
        ppu.h_blank = true;
        ppu.scanline = 10;
        ppu.vmain_data_gap_ticks = 0;
        ppu.hdma_head_busy_until = 0;

        ppu.cycle = ppu
            .last_dot_index()
            .saturating_sub(crate::debug_flags::vram_mdma_tail());
        assert!(ppu.can_write_vram_non_hdma_now());

        ppu.cycle = ppu
            .last_dot_index()
            .saturating_sub(crate::debug_flags::vram_mdma_tail())
            + 1;
        assert!(!ppu.can_write_vram_non_hdma_now());
    }
}
