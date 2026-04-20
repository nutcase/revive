use super::*;

#[test]
fn save_state_at_gsu_pc_range_requires_latched_hit_for_exact_filters() {
    let _guard = env_lock().lock().unwrap();
    let prev_frame = set_env_temp("TRACE_SUPERFX_EXEC_AT_FRAME", "");
    let prev_hit_index = set_env_temp("SAVE_STATE_AT_GSU_PC_HIT_INDEX", "");
    let prev_reg_eq = set_env_temp("SAVE_STATE_AT_GSU_REG_EQ", "");
    let prev_reg_write = set_env_temp("SAVE_STATE_AT_GSU_REG_WRITE", "");
    let prev_tail = set_env_temp("SAVE_STATE_AT_GSU_RECENT_EXEC_TAIL", "");

    assert!(!Emulator::save_state_at_gsu_pc_requires_latched_hit_env());

    std::env::set_var("SAVE_STATE_AT_GSU_REG_EQ", "R2=004B");
    assert!(Emulator::save_state_at_gsu_pc_requires_latched_hit_env());

    std::env::remove_var("SAVE_STATE_AT_GSU_REG_EQ");
    std::env::set_var("SAVE_STATE_AT_GSU_REG_WRITE", "R13=0000");
    assert!(Emulator::save_state_at_gsu_pc_requires_latched_hit_env());

    std::env::remove_var("SAVE_STATE_AT_GSU_REG_WRITE");
    std::env::set_var("SAVE_STATE_AT_GSU_RECENT_EXEC_TAIL", "01:AD44,01:AD46");
    assert!(Emulator::save_state_at_gsu_pc_requires_latched_hit_env());

    std::env::remove_var("SAVE_STATE_AT_GSU_RECENT_EXEC_TAIL");
    std::env::set_var("SAVE_STATE_AT_GSU_PC_HIT_INDEX", "2");
    assert!(Emulator::save_state_at_gsu_pc_requires_latched_hit_env());

    std::env::remove_var("SAVE_STATE_AT_GSU_PC_HIT_INDEX");
    std::env::set_var("TRACE_SUPERFX_EXEC_AT_FRAME", "164");
    assert!(Emulator::save_state_at_gsu_pc_requires_latched_hit_env());

    restore_env("TRACE_SUPERFX_EXEC_AT_FRAME", prev_frame);
    restore_env("SAVE_STATE_AT_GSU_PC_HIT_INDEX", prev_hit_index);
    restore_env("SAVE_STATE_AT_GSU_REG_EQ", prev_reg_eq);
    restore_env("SAVE_STATE_AT_GSU_REG_WRITE", prev_reg_write);
    restore_env("SAVE_STATE_AT_GSU_RECENT_EXEC_TAIL", prev_tail);
}
#[test]
fn gsu_only_should_not_save_final_state_for_exact_filters_without_hit() {
    let _guard = env_lock().lock().unwrap();
    let prev_hit_index = set_env_temp("SAVE_STATE_AT_GSU_PC_HIT_INDEX", "");
    let prev_reg_eq = set_env_temp("SAVE_STATE_AT_GSU_REG_EQ", "");
    let prev_reg_write = set_env_temp("SAVE_STATE_AT_GSU_REG_WRITE", "");
    let prev_tail = set_env_temp("SAVE_STATE_AT_GSU_RECENT_EXEC_TAIL", "");

    std::env::set_var("SAVE_STATE_AT_GSU_REG_EQ", "R2=004B");
    assert!(!Emulator::gsu_only_should_save_state(
        false, false, None, None,
    ));
    assert!(Emulator::gsu_only_should_save_state(
        false,
        false,
        Some((0x01, 0xD223)),
        None,
    ));

    std::env::remove_var("SAVE_STATE_AT_GSU_REG_EQ");
    std::env::set_var("SAVE_STATE_AT_GSU_REG_WRITE", "R13=0000");
    assert!(!Emulator::gsu_only_should_save_state(
        false, false, None, None,
    ));
    assert!(Emulator::gsu_only_should_save_state(
        false,
        false,
        Some((0x01, 0xD4D0)),
        None,
    ));

    std::env::remove_var("SAVE_STATE_AT_GSU_REG_WRITE");
    assert!(Emulator::gsu_only_should_save_state(
        false, false, None, None,
    ));

    restore_env("SAVE_STATE_AT_GSU_PC_HIT_INDEX", prev_hit_index);
    restore_env("SAVE_STATE_AT_GSU_REG_EQ", prev_reg_eq);
    restore_env("SAVE_STATE_AT_GSU_REG_WRITE", prev_reg_write);
    restore_env("SAVE_STATE_AT_GSU_RECENT_EXEC_TAIL", prev_tail);
}
#[test]
fn gsu_only_save_requires_latched_hit_for_range_and_ram_hooks() {
    let _guard = env_lock().lock().unwrap();
    let prev_hit_index = set_env_temp("SAVE_STATE_AT_GSU_PC_HIT_INDEX", "");
    let prev_reg_eq = set_env_temp("SAVE_STATE_AT_GSU_REG_EQ", "");
    let prev_reg_write = set_env_temp("SAVE_STATE_AT_GSU_REG_WRITE", "");
    let prev_tail = set_env_temp("SAVE_STATE_AT_GSU_RECENT_EXEC_TAIL", "");

    assert!(Emulator::gsu_only_save_requires_latched_hit_env(
        true, false
    ));
    assert!(Emulator::gsu_only_save_requires_latched_hit_env(
        false, true
    ));
    assert!(!Emulator::gsu_only_save_requires_latched_hit_env(
        false, false
    ));

    std::env::set_var("SAVE_STATE_AT_GSU_RECENT_EXEC_TAIL", "01:D383,01:D223");
    assert!(Emulator::gsu_only_save_requires_latched_hit_env(
        false, false
    ));

    restore_env("SAVE_STATE_AT_GSU_PC_HIT_INDEX", prev_hit_index);
    restore_env("SAVE_STATE_AT_GSU_REG_EQ", prev_reg_eq);
    restore_env("SAVE_STATE_AT_GSU_REG_WRITE", prev_reg_write);
    restore_env("SAVE_STATE_AT_GSU_RECENT_EXEC_TAIL", prev_tail);
}
#[test]
fn parse_star_fox_gsu_test_overrides_accepts_ram_word_writes() {
    let _guard = env_lock().lock().unwrap();
    let prev = set_env_temp("STARFOX_GSU_OVERRIDE_RAM_WORDS", "");
    std::env::set_var(
        "STARFOX_GSU_OVERRIDE_RAM_WORDS",
        "04C4=888C,021E=887F, 1AE0=FFF9",
    );

    let overrides = parse_star_fox_gsu_test_overrides();

    assert_eq!(
        overrides.ram_words,
        vec![(0x04C4, 0x888C), (0x021E, 0x887F), (0x1AE0, 0xFFF9)]
    );

    restore_env("STARFOX_GSU_OVERRIDE_RAM_WORDS", prev);
}
#[test]
fn apply_star_fox_gsu_test_overrides_writes_ram_words() {
    let mut emulator = make_test_emulator_inner();
    emulator.bus.superfx = Some(crate::cartridge::superfx::SuperFx::new(0x20_0000));
    let overrides = StarFoxGsuTestOverrides {
        ram_words: vec![(0x04C4, 0x888C), (0x021E, 0x887F)],
        ..Default::default()
    };

    apply_star_fox_gsu_test_overrides(&mut emulator, &overrides);

    let gsu = emulator.bus.superfx.as_ref().expect("missing SuperFX");
    assert_eq!(gsu.debug_read_ram_word_short(0x04C4), 0x888C);
    assert_eq!(gsu.debug_read_ram_word_short(0x021E), 0x887F);
}
#[test]
fn save_state_exact_capture_env_active_detects_gsu_and_ram_hooks() {
    let _guard = env_lock().lock().unwrap();
    let prev_cpu = set_env_temp("SAVE_STATE_AT_CPU_EXEC_PC", "");
    let prev_gsu = set_env_temp("SAVE_STATE_AT_GSU_PC_RANGE", "");
    let prev_reg_write = set_env_temp("SAVE_STATE_AT_GSU_REG_WRITE", "");
    let prev_ram = set_env_temp("SAVE_STATE_AT_SUPERFX_RAM_ADDRS", "");

    std::env::remove_var("SAVE_STATE_AT_CPU_EXEC_PC");
    std::env::remove_var("SAVE_STATE_AT_GSU_PC_RANGE");
    std::env::remove_var("SAVE_STATE_AT_GSU_REG_WRITE");
    std::env::remove_var("SAVE_STATE_AT_SUPERFX_RAM_ADDRS");
    assert!(!Emulator::save_state_exact_capture_env_active());

    std::env::set_var("SAVE_STATE_AT_GSU_PC_RANGE", "01:AF70-AF70");
    assert!(Emulator::save_state_exact_capture_env_active());

    std::env::remove_var("SAVE_STATE_AT_GSU_PC_RANGE");
    std::env::set_var("SAVE_STATE_AT_GSU_REG_WRITE", "R13=0000");
    assert!(Emulator::save_state_exact_capture_env_active());

    std::env::remove_var("SAVE_STATE_AT_GSU_REG_WRITE");
    std::env::set_var("SAVE_STATE_AT_SUPERFX_RAM_ADDRS", "0136");
    assert!(Emulator::save_state_exact_capture_env_active());

    restore_env("SAVE_STATE_AT_CPU_EXEC_PC", prev_cpu);
    restore_env("SAVE_STATE_AT_GSU_PC_RANGE", prev_gsu);
    restore_env("SAVE_STATE_AT_GSU_REG_WRITE", prev_reg_write);
    restore_env("SAVE_STATE_AT_SUPERFX_RAM_ADDRS", prev_ram);
}
#[test]
fn maybe_save_state_at_gsu_pc_range_requests_capture_stop_without_quit() {
    let _guard = env_lock().lock().unwrap();
    crate::shutdown::clear_for_tests();
    let save_path = std::env::temp_dir().join(format!(
        "codex_gsu_pc_range_capture_stop_{}.json",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&save_path);

    let prev_range = set_env_temp("SAVE_STATE_AT_GSU_PC_RANGE", "01:D040-D040");
    let prev_path = set_env_temp("SAVE_STATE_PATH", &save_path.to_string_lossy());
    let prev_quit = set_env_temp("SAVE_STATE_QUIT", "0");
    let prev_hit_index = set_env_temp("SAVE_STATE_AT_GSU_PC_HIT_INDEX", "1");

    let mut emulator = make_test_emulator_inner();
    emulator.bus.superfx = Some(crate::cartridge::superfx::SuperFx::new(0x20_0000));
    emulator
        .bus
        .superfx
        .as_mut()
        .expect("superfx")
        .debug_set_save_state_pc_hit(Some((0x01, 0xD040)));

    assert!(emulator.maybe_save_state_at_gsu_pc_range());
    assert!(save_path.exists());
    assert!(emulator.take_save_state_capture_stop_requested());
    assert!(!crate::shutdown::should_quit());

    let _ = std::fs::remove_file(&save_path);
    restore_env("SAVE_STATE_AT_GSU_PC_RANGE", prev_range);
    restore_env("SAVE_STATE_PATH", prev_path);
    restore_env("SAVE_STATE_QUIT", prev_quit);
    restore_env("SAVE_STATE_AT_GSU_PC_HIT_INDEX", prev_hit_index);
    crate::shutdown::clear_for_tests();
}
#[test]
fn run_frame_stops_before_timing_catchup_on_gsu_pc_range_capture() {
    let _guard = env_lock().lock().unwrap();
    crate::shutdown::clear_for_tests();
    let save_path = std::env::temp_dir().join(format!(
        "codex_gsu_pc_range_early_stop_{}.json",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&save_path);

    let prev_range = set_env_temp("SAVE_STATE_AT_GSU_PC_RANGE", "01:D040-D040");
    let prev_path = set_env_temp("SAVE_STATE_PATH", &save_path.to_string_lossy());
    let prev_quit = set_env_temp("SAVE_STATE_QUIT", "0");
    let prev_hit_index = set_env_temp("SAVE_STATE_AT_GSU_PC_HIT_INDEX", "1");

    let mut emulator = make_test_emulator_inner();
    emulator.bus.superfx = Some(crate::cartridge::superfx::SuperFx::new(0x20_0000));
    emulator
        .bus
        .superfx
        .as_mut()
        .expect("superfx")
        .debug_set_save_state_pc_hit(Some((0x01, 0xD040)));
    emulator.master_cycles = 1234;

    emulator.run_frame();

    assert!(save_path.exists());
    assert!(emulator.take_save_state_capture_stop_requested());
    assert_eq!(emulator.master_cycles, 1234);
    assert!(!crate::shutdown::should_quit());

    let _ = std::fs::remove_file(&save_path);
    restore_env("SAVE_STATE_AT_GSU_PC_RANGE", prev_range);
    restore_env("SAVE_STATE_PATH", prev_path);
    restore_env("SAVE_STATE_QUIT", prev_quit);
    restore_env("SAVE_STATE_AT_GSU_PC_HIT_INDEX", prev_hit_index);
    crate::shutdown::clear_for_tests();
}
#[test]
fn maybe_save_state_at_gsu_reg_write_requests_capture_stop_without_quit() {
    let _guard = env_lock().lock().unwrap();
    crate::shutdown::clear_for_tests();
    let save_path = std::env::temp_dir().join(format!(
        "codex_gsu_reg_write_capture_stop_{}.json",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&save_path);

    let prev_reg_write = set_env_temp("SAVE_STATE_AT_GSU_REG_WRITE", "R14=653A");
    let prev_path = set_env_temp("SAVE_STATE_PATH", &save_path.to_string_lossy());
    let prev_quit = set_env_temp("SAVE_STATE_QUIT", "0");

    let mut emulator = make_test_emulator_inner();
    emulator.bus.superfx = Some(crate::cartridge::superfx::SuperFx::new(0x20_0000));
    emulator
        .bus
        .superfx
        .as_mut()
        .expect("superfx")
        .debug_set_save_state_pc_hit(Some((0x01, 0xB30A)));

    assert!(emulator.maybe_save_state_at_gsu_reg_write());
    assert!(save_path.exists());
    assert!(emulator.take_save_state_capture_stop_requested());
    assert!(!crate::shutdown::should_quit());

    let _ = std::fs::remove_file(&save_path);
    restore_env("SAVE_STATE_AT_GSU_REG_WRITE", prev_reg_write);
    restore_env("SAVE_STATE_PATH", prev_path);
    restore_env("SAVE_STATE_QUIT", prev_quit);
    crate::shutdown::clear_for_tests();
}
#[test]
fn run_frame_stops_before_timing_catchup_on_gsu_reg_write_capture() {
    let _guard = env_lock().lock().unwrap();
    crate::shutdown::clear_for_tests();
    let save_path = std::env::temp_dir().join(format!(
        "codex_gsu_reg_write_early_stop_{}.json",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&save_path);

    let prev_reg_write = set_env_temp("SAVE_STATE_AT_GSU_REG_WRITE", "R14=653A");
    let prev_path = set_env_temp("SAVE_STATE_PATH", &save_path.to_string_lossy());
    let prev_quit = set_env_temp("SAVE_STATE_QUIT", "0");

    let mut emulator = make_test_emulator_inner();
    emulator.bus.superfx = Some(crate::cartridge::superfx::SuperFx::new(0x20_0000));
    emulator
        .bus
        .superfx
        .as_mut()
        .expect("superfx")
        .debug_set_save_state_pc_hit(Some((0x01, 0xB30A)));
    emulator.master_cycles = 1234;

    emulator.run_frame();

    assert!(save_path.exists());
    assert!(emulator.take_save_state_capture_stop_requested());
    assert_eq!(emulator.master_cycles, 1234);
    assert!(!crate::shutdown::should_quit());

    let _ = std::fs::remove_file(&save_path);
    restore_env("SAVE_STATE_AT_GSU_REG_WRITE", prev_reg_write);
    restore_env("SAVE_STATE_PATH", prev_path);
    restore_env("SAVE_STATE_QUIT", prev_quit);
    crate::shutdown::clear_for_tests();
}
#[test]
fn step_one_frame_inner_returns_true_after_save_state_quit_request() {
    let _guard = env_lock().lock().unwrap();
    crate::shutdown::clear_for_tests();
    let save_path = std::env::temp_dir().join(format!(
        "codex_save_state_quit_step_frame_{}.json",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&save_path);

    let prev_save_at = set_env_temp("SAVE_STATE_AT", "1");
    let prev_path = set_env_temp("SAVE_STATE_PATH", &save_path.to_string_lossy());
    let prev_quit = set_env_temp("SAVE_STATE_QUIT", "1");

    let mut emulator = make_test_emulator_inner();
    assert!(emulator.step_one_frame_inner());
    assert_eq!(emulator.frame_count(), 1);
    assert!(save_path.exists());
    assert!(crate::shutdown::should_quit());

    let _ = std::fs::remove_file(&save_path);
    restore_env("SAVE_STATE_AT", prev_save_at);
    restore_env("SAVE_STATE_PATH", prev_path);
    restore_env("SAVE_STATE_QUIT", prev_quit);
    crate::shutdown::clear_for_tests();
}
#[test]
fn save_state_at_superfx_ram_addr_accepts_byte_eq_filter() {
    let _guard = env_lock().lock().unwrap();
    let prev_range = set_env_temp("SAVE_STATE_AT_SUPERFX_RAM_ADDR_RANGE", "");
    let prev_addrs = set_env_temp("SAVE_STATE_AT_SUPERFX_RAM_ADDRS", "");
    let prev_byte_eq = set_env_temp("SAVE_STATE_AT_SUPERFX_RAM_BYTE_EQ", "");

    std::env::remove_var("SAVE_STATE_AT_SUPERFX_RAM_ADDR_RANGE");
    std::env::remove_var("SAVE_STATE_AT_SUPERFX_RAM_ADDRS");
    std::env::set_var("SAVE_STATE_AT_SUPERFX_RAM_BYTE_EQ", "0136=4B");

    let has_range = std::env::var("SAVE_STATE_AT_SUPERFX_RAM_ADDR_RANGE")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .is_some();
    let has_addrs = std::env::var("SAVE_STATE_AT_SUPERFX_RAM_ADDRS")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .is_some();
    let has_byte_eq = std::env::var("SAVE_STATE_AT_SUPERFX_RAM_BYTE_EQ")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .is_some();

    assert!(!has_range && !has_addrs && has_byte_eq);

    restore_env("SAVE_STATE_AT_SUPERFX_RAM_ADDR_RANGE", prev_range);
    restore_env("SAVE_STATE_AT_SUPERFX_RAM_ADDRS", prev_addrs);
    restore_env("SAVE_STATE_AT_SUPERFX_RAM_BYTE_EQ", prev_byte_eq);
}
#[test]
fn save_state_at_superfx_ram_addr_accepts_word_eq_filter() {
    let _guard = env_lock().lock().unwrap();
    let prev_range = set_env_temp("SAVE_STATE_AT_SUPERFX_RAM_ADDR_RANGE", "");
    let prev_addrs = set_env_temp("SAVE_STATE_AT_SUPERFX_RAM_ADDRS", "");
    let prev_word_eq = set_env_temp("SAVE_STATE_AT_SUPERFX_RAM_WORD_EQ", "");

    std::env::remove_var("SAVE_STATE_AT_SUPERFX_RAM_ADDR_RANGE");
    std::env::remove_var("SAVE_STATE_AT_SUPERFX_RAM_ADDRS");
    std::env::set_var("SAVE_STATE_AT_SUPERFX_RAM_WORD_EQ", "04C4=887F");

    let has_range = std::env::var("SAVE_STATE_AT_SUPERFX_RAM_ADDR_RANGE")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .is_some();
    let has_addrs = std::env::var("SAVE_STATE_AT_SUPERFX_RAM_ADDRS")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .is_some();
    let has_word_eq = std::env::var("SAVE_STATE_AT_SUPERFX_RAM_WORD_EQ")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .is_some();

    assert!(!has_range && !has_addrs && has_word_eq);

    restore_env("SAVE_STATE_AT_SUPERFX_RAM_ADDR_RANGE", prev_range);
    restore_env("SAVE_STATE_AT_SUPERFX_RAM_ADDRS", prev_addrs);
    restore_env("SAVE_STATE_AT_SUPERFX_RAM_WORD_EQ", prev_word_eq);
}
#[test]
fn maybe_save_state_at_superfx_ram_addr_requests_capture_stop_without_quit() {
    let _guard = env_lock().lock().unwrap();
    crate::shutdown::clear_for_tests();
    let save_path = std::env::temp_dir().join(format!(
        "codex_superfx_ram_capture_stop_{}.json",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&save_path);

    let prev_addrs = set_env_temp("SAVE_STATE_AT_SUPERFX_RAM_ADDRS", "01A0");
    let prev_path = set_env_temp("SAVE_STATE_PATH", &save_path.to_string_lossy());
    let prev_quit = set_env_temp("SAVE_STATE_QUIT", "0");

    let mut emulator = make_test_emulator_inner();
    emulator.bus.superfx = Some(crate::cartridge::superfx::SuperFx::new(0x20_0000));
    emulator
        .bus
        .superfx
        .as_mut()
        .expect("superfx")
        .debug_set_save_state_ram_addr_hit(Some((0x01, 0xB396, 0x01A0)));

    assert!(emulator.maybe_save_state_at_superfx_ram_addr());
    assert!(save_path.exists());
    assert!(emulator.take_save_state_capture_stop_requested());
    assert!(!crate::shutdown::should_quit());

    let _ = std::fs::remove_file(&save_path);
    restore_env("SAVE_STATE_AT_SUPERFX_RAM_ADDRS", prev_addrs);
    restore_env("SAVE_STATE_PATH", prev_path);
    restore_env("SAVE_STATE_QUIT", prev_quit);
    crate::shutdown::clear_for_tests();
}
#[test]
fn step_superfx_for_master_cycles_leaves_pending_save_hit_for_emulator_capture() {
    let _guard = env_lock().lock().unwrap();
    crate::shutdown::clear_for_tests();
    let save_path = std::env::temp_dir().join(format!(
        "codex_superfx_step_cycles_capture_{}.json",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&save_path);
    let prev_addrs = set_env_temp("SAVE_STATE_AT_SUPERFX_RAM_ADDRS", "021E");
    let prev_path = set_env_temp("SAVE_STATE_PATH", &save_path.to_string_lossy());
    let prev_quit = set_env_temp("SAVE_STATE_QUIT", "0");
    let mut emulator = make_test_emulator_inner();
    emulator.bus.mapper_type = crate::cartridge::MapperType::SuperFx;
    emulator.bus.superfx = Some(crate::cartridge::superfx::SuperFx::new(0x20_0000));
    emulator
        .bus
        .superfx
        .as_mut()
        .expect("superfx")
        .debug_set_save_state_ram_addr_hit(Some((0x01, 0xB396, 0x021E)));

    emulator.step_superfx_for_master_cycles(64);
    assert!(emulator.superfx_save_state_hit_pending());
    assert!(emulator.maybe_save_state_at_superfx_ram_addr());

    assert!(save_path.exists());
    assert!(emulator.take_save_state_capture_stop_requested());
    assert_eq!(
        emulator
            .bus
            .superfx
            .as_mut()
            .expect("superfx")
            .debug_take_save_state_ram_addr_hit(),
        None
    );
    let _ = std::fs::remove_file(&save_path);
    restore_env("SAVE_STATE_AT_SUPERFX_RAM_ADDRS", prev_addrs);
    restore_env("SAVE_STATE_PATH", prev_path);
    restore_env("SAVE_STATE_QUIT", prev_quit);
    crate::shutdown::clear_for_tests();
}
