use super::*;

fn step_dmc(apu: &mut Apu, cycles: usize, sample_data: u8) {
    for _ in 0..cycles {
        if let Some((addr, _stall_cycles)) = apu.pull_dmc_sample_request() {
            assert_eq!(addr, 0xC000);
            apu.push_dmc_sample(sample_data);
        }
        apu.step();
    }
}

#[test]
fn dmc_fetches_sample_and_modulates_output() {
    let mut apu = Apu::new();
    apu.write_register(0x4010, 0x0F);
    apu.write_register(0x4011, 64);
    apu.write_register(0x4012, 0x00);
    apu.write_register(0x4013, 0x00);
    apu.write_register(0x4015, 0x10);

    assert_eq!(apu.pull_dmc_sample_request(), None);
    apu.step();
    assert_eq!(apu.pull_dmc_sample_request(), None);
    apu.step();
    assert_eq!(apu.pull_dmc_sample_request(), Some((0xC000, 3)));
    apu.push_dmc_sample(0xFF);

    let cycles = apu.dmc.timer as usize + (DMC_RATE_TABLE[15] as usize + 1) * 20;
    step_dmc(&mut apu, cycles, 0xFF);

    assert!(apu.dmc.output_level > 64);
    assert_eq!(apu.read_register(0x4015) & 0x10, 0);
}

#[test]
fn dmc_sets_irq_and_write_4015_clears_it() {
    let mut apu = Apu::new();
    apu.write_register(0x4010, 0x80);
    apu.write_register(0x4012, 0x00);
    apu.write_register(0x4013, 0x00);
    apu.write_register(0x4015, 0x10);

    assert_eq!(apu.pull_dmc_sample_request(), None);
    apu.step();
    assert_eq!(apu.pull_dmc_sample_request(), None);
    apu.step();
    assert_eq!(apu.pull_dmc_sample_request(), Some((0xC000, 3)));
    assert!(apu.dmc.irq_pending);
    apu.push_dmc_sample(0x00);

    let status = apu.read_register(0x4015);
    assert_eq!(status & 0x80, 0x80);
    assert!(apu.dmc.irq_pending);

    apu.write_register(0x4015, 0x00);
    assert!(!apu.dmc.irq_pending);
}

#[test]
fn apu_state_restores_dmc_progress() {
    let mut apu = Apu::new();
    apu.write_register(0x4010, 0x8F);
    apu.write_register(0x4011, 32);
    apu.write_register(0x4012, 0x00);
    apu.write_register(0x4013, 0x01);
    apu.write_register(0x4015, 0x10);

    assert_eq!(apu.pull_dmc_sample_request(), None);
    apu.step();
    assert_eq!(apu.pull_dmc_sample_request(), None);
    apu.step();
    assert_eq!(apu.pull_dmc_sample_request(), Some((0xC000, 3)));
    apu.push_dmc_sample(0xAA);
    let cycles = apu.dmc.timer as usize + (DMC_RATE_TABLE[15] as usize + 1) * 6;
    step_dmc(&mut apu, cycles, 0x55);

    let snapshot = apu.snapshot_state();
    let mut restored = Apu::new();
    restored.restore_state(&snapshot);

    for _ in 0..64 {
        let request = apu.pull_dmc_sample_request();
        assert_eq!(request, restored.pull_dmc_sample_request());
        if request.is_some() {
            apu.push_dmc_sample(0x33);
            restored.push_dmc_sample(0x33);
        }

        apu.step();
        restored.step();

        assert_eq!(restored.dmc.output_level, apu.dmc.output_level);
        assert_eq!(restored.dmc.timer, apu.dmc.timer);
        assert_eq!(restored.dmc.bytes_remaining, apu.dmc.bytes_remaining);
        assert_eq!(restored.dmc.sample_buffer, apu.dmc.sample_buffer);
        assert_eq!(restored.dmc.shift_register, apu.dmc.shift_register);
        assert_eq!(restored.dmc.bits_remaining, apu.dmc.bits_remaining);
        assert_eq!(restored.dmc.silence, apu.dmc.silence);
    }
}
