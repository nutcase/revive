use super::*;

#[test]
fn default_steps_per_cpu_cycle_tracks_clsr_speed_mode() {
    // CLSR bit 0: 0 = standard 10.738 MHz (SLOW), 1 = turbo 21.477 MHz (FAST)
    let standard = SuperFx::new(0x20_0000); // clsr=0 → standard speed
    let mut turbo = SuperFx::new(0x20_0000);
    turbo.clsr = 0x01; // clsr=1 → turbo speed

    assert_eq!(
        standard.steps_per_cpu_cycle(),
        super::DEFAULT_SUPERFX_RATIO_SLOW
    );
    assert_eq!(
        turbo.steps_per_cpu_cycle(),
        super::DEFAULT_SUPERFX_RATIO_FAST
    );
}

#[test]
fn superfx_cpu_ratio_env_overrides_default_speed() {
    let _guard = env_lock().lock().unwrap();
    let prev = std::env::var("SUPERFX_CPU_RATIO").ok();
    std::env::set_var("SUPERFX_CPU_RATIO", "7");

    let mut gsu = SuperFx::new(0x20_0000);
    gsu.clsr = 0x01;
    assert_eq!(gsu.steps_per_cpu_cycle(), 7);

    if let Some(value) = prev {
        std::env::set_var("SUPERFX_CPU_RATIO", value);
    } else {
        std::env::remove_var("SUPERFX_CPU_RATIO");
    }
}

#[test]
fn superfx_status_poll_boost_env_overrides_default_value() {
    let _guard = env_lock().lock().unwrap();
    let prev = std::env::var("SUPERFX_STATUS_POLL_BOOST").ok();
    std::env::set_var("SUPERFX_STATUS_POLL_BOOST", "96");

    assert_eq!(SuperFx::status_poll_step_budget(), 96);

    if let Some(value) = prev {
        std::env::set_var("SUPERFX_STATUS_POLL_BOOST", value);
    } else {
        std::env::remove_var("SUPERFX_STATUS_POLL_BOOST");
    }
}

#[test]
fn debug_in_starfox_cached_delay_loop_matches_expected_signature() {
    let mut gsu = SuperFx::new(0x20_0000);
    gsu.running = true;
    gsu.cache_enabled = true;
    gsu.pbr = 0x01;
    gsu.cbr = 0x84F0;
    gsu.regs[0] = 0x8EBC;
    gsu.regs[11] = 0x8615;
    gsu.regs[13] = 0x000B;
    gsu.regs[15] = 0x000C;

    assert!(gsu.debug_in_starfox_cached_delay_loop());

    gsu.regs[11] = 0x8609;
    assert!(gsu.debug_in_starfox_cached_delay_loop());

    gsu.regs[11] = 0x8614;
    assert!(!gsu.debug_in_starfox_cached_delay_loop());

    gsu.regs[11] = 0x8608;
    assert!(!gsu.debug_in_starfox_cached_delay_loop());
}

#[test]
fn debug_in_starfox_cached_delay_loop_ignores_r0_data_value() {
    let mut gsu = SuperFx::new(0x20_0000);
    gsu.running = true;
    gsu.cache_enabled = true;
    gsu.pbr = 0x01;
    gsu.cbr = 0x84F0;
    gsu.regs[0] = 0x0000;
    gsu.regs[11] = 0x8615;
    gsu.regs[13] = 0x000B;
    gsu.regs[15] = 0x000B;
    assert!(gsu.debug_in_starfox_cached_delay_loop());

    gsu.regs[0] = 0x8EBC;
    assert!(gsu.debug_in_starfox_cached_delay_loop());
}

#[test]
fn fast_forward_starfox_cached_delay_loop_collapses_r12_to_zero() {
    let mut gsu = SuperFx::new(0x20_0000);
    gsu.running = true;
    gsu.cache_enabled = true;
    gsu.pbr = 0x01;
    gsu.cbr = 0x84F0;
    gsu.regs[0] = 0x0000;
    gsu.regs[11] = 0x8615;
    gsu.regs[12] = 0xBC8E;
    gsu.regs[13] = 0x000B;
    gsu.regs[15] = 0x000B;
    gsu.sfr = SFR_S_BIT;

    assert!(gsu.fast_forward_starfox_cached_delay_loop());
    assert_eq!(gsu.regs[12], 0x0000);
    assert_eq!(gsu.regs[15], 0x000C);
    assert!(!gsu.pipe_valid);
    assert!(gsu.sfr & SFR_Z_BIT != 0);
    assert!(gsu.sfr & SFR_S_BIT == 0);
}

#[test]
fn status_poll_late_wait_assist_can_exit_after_cached_delay_loop() {
    let mut gsu = SuperFx::new(0x20_0000);
    let rom = vec![0u8; 0x20_0000];
    gsu.running = true;
    gsu.cache_enabled = true;
    gsu.cache_valid_mask = u32::MAX;
    gsu.pbr = 0x01;
    gsu.cbr = 0x84F0;
    gsu.regs[0] = 0x0000;
    gsu.regs[11] = 0x8615;
    gsu.regs[12] = 0xBC8E;
    gsu.regs[13] = 0x000B;
    gsu.regs[15] = 0x000B;
    gsu.cache_ram[0x000C] = 0x00;

    gsu.run_status_poll_until_stop_with_starfox_late_wait_assist(&rom, 4);

    assert!(!gsu.running);
    assert_eq!(gsu.regs[12], 0x0000);
    assert_eq!(gsu.regs[15], 0x000E);
}

#[test]
fn status_poll_until_sfr_low_mask_changes_stops_after_go_bit_clears() {
    let mut gsu = SuperFx::new(0x20_0000);
    let rom = vec![0u8; 0x20_0000];
    gsu.running = true;
    gsu.cache_enabled = true;
    gsu.cache_valid_mask = u32::MAX;
    gsu.pbr = 0x01;
    gsu.cbr = 0x84F0;
    gsu.regs[0] = 0x0000;
    gsu.regs[11] = 0x8615;
    gsu.regs[12] = 0xBC8E;
    gsu.regs[13] = 0x000B;
    gsu.regs[15] = 0x000B;
    gsu.sfr = SFR_GO_BIT;
    gsu.cache_ram[0x000C] = 0x00;

    let initial_low = gsu.observed_sfr_low();
    assert_ne!(initial_low & (SFR_GO_BIT as u8), 0);

    gsu.run_status_poll_until_sfr_low_mask_changes(&rom, initial_low, SFR_GO_BIT as u8, 4);

    assert!(!gsu.running);
    assert_eq!(gsu.observed_sfr_low() & (SFR_GO_BIT as u8), 0);
    assert_eq!(gsu.regs[15], 0x000E);
}

#[test]
fn starfox_live_producer_wait_assist_can_run_until_stop() {
    let mut gsu = SuperFx::new(0x20_0000);
    let mut rom = vec![0u8; 0x20_0000];
    rom[0x01_B384] = 0x00;
    gsu.running = true;
    gsu.pbr = 0x01;
    gsu.rambr = 0x00;
    gsu.regs[13] = 0xB384;
    gsu.regs[15] = 0xB384;
    gsu.sfr = SFR_GO_BIT;

    gsu.run_status_poll_until_go_clears_in_starfox_live_producer_loop(&rom, 4);

    assert!(!gsu.running);
    assert_eq!(gsu.observed_sfr_low() & (SFR_GO_BIT as u8), 0);
}

#[test]
fn starfox_live_producer_wait_assist_stops_after_leaving_producer_band() {
    let mut gsu = SuperFx::new(0x20_0000);
    let rom = vec![0u8; 0x20_0000];
    gsu.running = true;
    gsu.pbr = 0x01;
    gsu.rambr = 0x00;
    gsu.regs[13] = 0xD1B4;
    gsu.regs[15] = 0xD1B4;
    gsu.sfr = SFR_GO_BIT;

    gsu.run_status_poll_until_go_clears_in_starfox_live_producer_loop(&rom, 8);

    assert!(gsu.running);
    assert_eq!(gsu.regs[13], 0xD1B4);
}
