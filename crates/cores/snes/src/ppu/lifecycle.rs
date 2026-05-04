use super::{BgMapCache, BgRowCache, Ppu};

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
}
