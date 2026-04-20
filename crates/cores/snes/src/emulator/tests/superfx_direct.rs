use super::*;

#[test]
fn star_fox_renders_non_black_pixels_by_170_frames() {
    let rom_path = Path::new("roms/Star Fox (Japan).sfc");
    if !rom_path.exists() {
        return;
    }

    let _guard = env_lock().lock().unwrap();
    let prev_headless = set_env_temp("HEADLESS", "1");
    let prev_headless_frames = set_env_temp("HEADLESS_FRAMES", "170");
    let prev_headless_fast_render = set_env_temp("HEADLESS_FAST_RENDER", "1");
    let prev_headless_fast_render_last = set_env_temp("HEADLESS_FAST_RENDER_LAST", "1");
    let prev_compat = set_env_temp("COMPAT_BOOT_FALLBACK", "1");
    let prev_quiet = set_env_temp("QUIET", "1");

    let cart = Cartridge::load_from_file(rom_path).expect("failed to load Star Fox ROM");
    let mut emulator = Emulator::new(cart, "Star Fox".to_string(), Option::<PathBuf>::None)
        .expect("failed to construct Star Fox emulator");
    emulator.run();

    let inidisp = emulator.bus.get_ppu().screen_display;
    let non_black = emulator
        .bus
        .get_ppu()
        .get_framebuffer()
        .iter()
        .filter(|&&p| (p & 0x00FF_FFFF) != 0)
        .count();

    restore_env("QUIET", prev_quiet);
    restore_env("COMPAT_BOOT_FALLBACK", prev_compat);
    restore_env("HEADLESS_FAST_RENDER_LAST", prev_headless_fast_render_last);
    restore_env("HEADLESS_FAST_RENDER", prev_headless_fast_render);
    restore_env("HEADLESS_FRAMES", prev_headless_frames);
    restore_env("HEADLESS", prev_headless);

    assert_eq!(
        inidisp & 0x80,
        0,
        "expected forced blank cleared by 170 frames, INIDISP={inidisp:02X}"
    );
    assert!(
        non_black > 0,
        "expected visible pixels by 170 frames, got non_black={non_black}"
    );
}
#[test]
fn star_fox_gui_framebuffer_fallback_uses_live_superfx_buffer_when_forced() {
    let rom_path = Path::new("roms/Star Fox (Japan).sfc");
    if !rom_path.exists() {
        return;
    }

    let _guard = env_lock().lock().unwrap();
    let prev_headless = set_env_temp("HEADLESS", "1");
    let prev_headless_frames = set_env_temp("HEADLESS_FRAMES", "120");
    let prev_headless_fast_render = set_env_temp("HEADLESS_FAST_RENDER", "1");
    let prev_headless_fast_render_last = set_env_temp("HEADLESS_FAST_RENDER_LAST", "1");
    let prev_fallback = set_env_temp("STARFOX_GUI_FALLBACK", "1");
    let prev_direct_only = set_env_temp("STARFOX_GUI_DIRECT_ONLY", "1");
    let prev_live = set_env_temp("SUPERFX_DIRECT_USE_LIVE", "1");
    let prev_quiet = set_env_temp("QUIET", "1");

    let cart = Cartridge::load_from_file(rom_path).expect("failed to load Star Fox ROM");
    let mut emulator = Emulator::new(cart, "Star Fox".to_string(), Option::<PathBuf>::None)
        .expect("failed to construct Star Fox emulator");
    emulator.run();

    let ppu_forced_blank = (emulator.bus.get_ppu().screen_display & 0x80) != 0;
    let ppu_non_black = emulator
        .bus
        .get_ppu()
        .get_framebuffer()
        .iter()
        .filter(|&&p| (p & 0x00FF_FFFF) != 0)
        .count();
    let gui_non_black = emulator
        .framebuffer()
        .iter()
        .filter(|&&p| (p & 0x00FF_FFFF) != 0)
        .count();

    restore_env("QUIET", prev_quiet);
    restore_env("SUPERFX_DIRECT_USE_LIVE", prev_live);
    restore_env("STARFOX_GUI_DIRECT_ONLY", prev_direct_only);
    restore_env("STARFOX_GUI_FALLBACK", prev_fallback);
    restore_env("HEADLESS_FAST_RENDER_LAST", prev_headless_fast_render_last);
    restore_env("HEADLESS_FAST_RENDER", prev_headless_fast_render);
    restore_env("HEADLESS_FRAMES", prev_headless_frames);
    restore_env("HEADLESS", prev_headless);

    assert!(
        ppu_forced_blank,
        "expected PPU to still be forced blank before the GUI fallback"
    );
    assert_eq!(
        ppu_non_black, 0,
        "expected regular PPU framebuffer to remain blank without compatibility auto-unblank"
    );
    assert!(
        gui_non_black > 0,
        "expected GUI framebuffer fallback to expose SuperFX pixels"
    );
}
#[test]
fn star_fox_gui_direct_y_offset_stays_centered_while_forced_blank() {
    let _guard = env_lock().lock().unwrap();
    let mut emulator = make_test_emulator_inner();
    emulator.rom_title = "STAR FOX".to_string();
    emulator.bus.get_ppu_mut().screen_display = 0x80;
    let buffer = vec![0xFF; 24_576];

    let offset = emulator.superfx_gui_direct_y_offset(&buffer, 192, 4, 2, 150);

    assert_eq!(offset, -16);
}
#[test]
fn sync_superfx_direct_buffer_clears_stale_bypass_flag_when_inactive() {
    let mut emulator = make_test_emulator();
    emulator.bus.get_ppu_mut().superfx_bypass_bg1_window = true;

    emulator.sync_superfx_direct_buffer();

    assert!(!emulator.bus.get_ppu().superfx_bypass_bg1_window);
}
#[test]
fn sync_superfx_direct_buffer_applies_bypass_flag_even_when_inactive() {
    let mut emulator = make_test_emulator();
    let _guard = env_lock().lock().unwrap();
    let prev = set_env_temp("SUPERFX_BYPASS_BG1_WINDOW", "1");

    emulator.sync_superfx_direct_buffer();

    restore_env("SUPERFX_BYPASS_BG1_WINDOW", prev);
    assert!(emulator.bus.get_ppu().superfx_bypass_bg1_window);
}
#[test]
fn sync_superfx_direct_buffer_enables_authoritative_bg1_for_starfox_by_default() {
    let _guard = env_lock().lock().unwrap();
    let prev = std::env::var_os("SUPERFX_AUTHORITATIVE_BG1_SOURCE");
    std::env::remove_var("SUPERFX_AUTHORITATIVE_BG1_SOURCE");
    let mut emulator = make_test_emulator_inner();
    emulator.rom_title = "STAR FOX".to_string();

    emulator.sync_superfx_direct_buffer();

    restore_env("SUPERFX_AUTHORITATIVE_BG1_SOURCE", prev);
    assert!(emulator.bus.get_ppu().superfx_authoritative_bg1_source);
}
#[test]
fn sync_superfx_direct_buffer_can_disable_starfox_authoritative_bg1() {
    let _guard = env_lock().lock().unwrap();
    let prev = set_env_temp("SUPERFX_AUTHORITATIVE_BG1_SOURCE", "0");
    let mut emulator = make_test_emulator_inner();
    emulator.rom_title = "STAR FOX".to_string();

    emulator.sync_superfx_direct_buffer();

    restore_env("SUPERFX_AUTHORITATIVE_BG1_SOURCE", prev);
    assert!(!emulator.bus.get_ppu().superfx_authoritative_bg1_source);
}
#[test]
fn sync_superfx_direct_buffer_prefers_tile_snapshot_when_requested() {
    let _guard = env_lock().lock().unwrap();
    let prev_tile = set_env_temp("SUPERFX_DIRECT_USE_TILE", "1");
    let prev_live = set_env_temp("SUPERFX_DIRECT_USE_LIVE", "1");

    let mut emulator = make_test_emulator_inner();
    emulator.bus.mapper_type = crate::cartridge::MapperType::SuperFx;
    let mut gsu = crate::cartridge::superfx::SuperFx::new(0x20_0000);
    let mut state = gsu.save_data();
    state.scbr = 0x00;
    state.scmr = 0x21;
    state.game_ram[0..32].fill(0x55);
    state.tile_snapshot = vec![0xAA; 32];
    state.tile_snapshot_valid = true;
    state.tile_snapshot_height = 192;
    state.tile_snapshot_bpp = 4;
    state.tile_snapshot_mode = 2;
    gsu.load_data(&state);
    emulator.bus.superfx = Some(gsu);

    emulator.sync_superfx_direct_buffer();

    let direct_buffer = emulator.bus.get_ppu().superfx_direct_buffer.clone();
    let direct_height = emulator.bus.get_ppu().superfx_direct_height;
    let direct_bpp = emulator.bus.get_ppu().superfx_direct_bpp;
    let direct_mode = emulator.bus.get_ppu().superfx_direct_mode;
    restore_env("SUPERFX_DIRECT_USE_LIVE", prev_live);
    restore_env("SUPERFX_DIRECT_USE_TILE", prev_tile);

    assert_eq!(direct_buffer, vec![0xAA; 32]);
    assert_eq!(direct_height, 192);
    assert_eq!(direct_bpp, 4);
    assert_eq!(direct_mode, 2);
}
#[test]
fn sync_superfx_direct_buffer_uses_display_snapshot_by_default_while_gsu_runs() {
    let _guard = env_lock().lock().unwrap();
    let prev_tile = std::env::var_os("SUPERFX_DIRECT_USE_TILE");
    let prev_live = std::env::var_os("SUPERFX_DIRECT_USE_LIVE");
    std::env::remove_var("SUPERFX_DIRECT_USE_TILE");
    std::env::remove_var("SUPERFX_DIRECT_USE_LIVE");

    let mut emulator = make_test_emulator_inner();
    emulator.bus.mapper_type = crate::cartridge::MapperType::SuperFx;
    let mut gsu = crate::cartridge::superfx::SuperFx::new(0x20_0000);
    let mut state = gsu.save_data();
    state.scbr = 0x00;
    state.scmr = 0x21;
    state.running = true;
    state.game_ram[0..32].fill(0x55);
    state.latest_stop_snapshot = vec![0xAA; 32];
    state.latest_stop_snapshot_valid = true;
    state.latest_stop_scbr = 0x00;
    state.latest_stop_height = 192;
    state.latest_stop_bpp = 4;
    state.latest_stop_mode = 2;
    gsu.load_data(&state);
    emulator.bus.superfx = Some(gsu);

    emulator.sync_superfx_direct_buffer();

    let direct_buffer = emulator.bus.get_ppu().superfx_direct_buffer.clone();
    restore_env("SUPERFX_DIRECT_USE_LIVE", prev_live);
    restore_env("SUPERFX_DIRECT_USE_TILE", prev_tile);

    assert_eq!(direct_buffer, vec![0xAA; 32]);
}
#[test]
fn sync_superfx_direct_buffer_can_debug_live_buffer_when_requested() {
    let _guard = env_lock().lock().unwrap();
    let prev_tile = std::env::var_os("SUPERFX_DIRECT_USE_TILE");
    let prev_live = set_env_temp("SUPERFX_DIRECT_USE_LIVE", "1");
    std::env::remove_var("SUPERFX_DIRECT_USE_TILE");

    let mut emulator = make_test_emulator_inner();
    emulator.bus.mapper_type = crate::cartridge::MapperType::SuperFx;
    let mut gsu = crate::cartridge::superfx::SuperFx::new(0x20_0000);
    let mut state = gsu.save_data();
    state.scbr = 0x00;
    state.scmr = 0x21;
    state.running = true;
    state.game_ram[0..32].fill(0x55);
    state.latest_stop_snapshot = vec![0xAA; 32];
    state.latest_stop_snapshot_valid = true;
    state.latest_stop_scbr = 0x00;
    state.latest_stop_height = 192;
    state.latest_stop_bpp = 4;
    state.latest_stop_mode = 2;
    gsu.load_data(&state);
    emulator.bus.superfx = Some(gsu);

    emulator.sync_superfx_direct_buffer();

    let direct_buffer = emulator.bus.get_ppu().superfx_direct_buffer.clone();
    restore_env("SUPERFX_DIRECT_USE_LIVE", prev_live);
    restore_env("SUPERFX_DIRECT_USE_TILE", prev_tile);

    assert_eq!(direct_buffer.len(), 24_576);
    assert_eq!(&direct_buffer[..32], &[0x55; 32]);
}
#[test]
fn superfx_gui_direct_pixel_addr_defaults_to_plot_layout() {
    let _guard = env_lock().lock().unwrap();
    let prev_row_major = std::env::var_os("SUPERFX_DIRECT_ROW_MAJOR");
    std::env::remove_var("SUPERFX_DIRECT_ROW_MAJOR");

    let default = Emulator::superfx_gui_direct_pixel_addr(8, 0, 192, 4, 2);

    restore_env("SUPERFX_DIRECT_ROW_MAJOR", prev_row_major);

    assert_eq!(default, Some((768, 0, 7)));
}
#[test]
fn superfx_gui_direct_pixel_addr_matches_ppu_layout_modes() {
    let _guard = env_lock().lock().unwrap();
    let prev_row_major = set_env_temp("SUPERFX_DIRECT_ROW_MAJOR", "1");

    let row_major = Emulator::superfx_gui_direct_pixel_addr(8, 0, 192, 4, 2);

    std::env::set_var("SUPERFX_DIRECT_ROW_MAJOR", "0");
    let superfx_layout = Emulator::superfx_gui_direct_pixel_addr(8, 0, 192, 4, 2);

    restore_env("SUPERFX_DIRECT_ROW_MAJOR", prev_row_major);

    assert_eq!(row_major, Some((32, 0, 7)));
    assert_eq!(superfx_layout, Some((768, 0, 7)));
}
#[test]
fn maybe_auto_unblank_keeps_existing_scene_when_compat_fallback_is_disabled() {
    let _guard = env_lock().lock().unwrap();
    let prev_compat = std::env::var_os("COMPAT_BOOT_FALLBACK");
    std::env::remove_var("COMPAT_BOOT_FALLBACK");

    let mut emulator = make_test_emulator_inner();
    emulator.frame_count = 170;

    let ppu = emulator.bus.get_ppu_mut();
    ppu.write(0x2C, 0x13);
    ppu.screen_display = 0x80;
    ppu.brightness = 0;
    ppu.framebuffer[0] = 0xFF112233;

    emulator.maybe_auto_unblank();

    let ppu = emulator.bus.get_ppu();
    assert_eq!(ppu.get_main_screen_designation(), 0x13);
    assert_eq!(ppu.screen_display, 0x80);
    assert_eq!(ppu.brightness, 0);
    assert_eq!(ppu.framebuffer[0], 0xFF112233);

    restore_env("COMPAT_BOOT_FALLBACK", prev_compat);
}
#[test]
fn maybe_auto_unblank_preserves_existing_scene_with_compat_fallback_enabled() {
    let _guard = env_lock().lock().unwrap();
    let prev_compat = set_env_temp("COMPAT_BOOT_FALLBACK", "1");

    let mut emulator = make_test_emulator_inner();
    emulator.frame_count = 170;

    let ppu = emulator.bus.get_ppu_mut();
    ppu.write(0x2C, 0x13);
    ppu.screen_display = 0x80;
    ppu.brightness = 0;
    ppu.framebuffer[0] = 0xFF112233;

    emulator.maybe_auto_unblank();

    let ppu = emulator.bus.get_ppu();
    assert_eq!(ppu.get_main_screen_designation(), 0x13);
    assert_eq!(ppu.screen_display & 0x80, 0);
    assert_ne!(ppu.brightness, 0);

    restore_env("COMPAT_BOOT_FALLBACK", prev_compat);
}
