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
