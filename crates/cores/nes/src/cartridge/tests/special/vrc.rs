use super::super::*;

#[test]
fn mapper_21_uses_vrc4_irq_and_restores_state() {
    let mut cart = make_mapper21_cart();

    cart.write_prg(0x8000, 0x02);
    cart.write_prg(0x9000, 0x03);
    cart.write_prg(0xF000, 0x0E);
    cart.write_prg(0xF040, 0x0F);
    cart.write_prg(0xF004, 0x07);
    cart.clock_irq_counter_cycles(1);
    assert!(!cart.irq_pending());
    cart.clock_irq_counter_cycles(1);
    assert!(cart.irq_pending());

    let snapshot = cart.snapshot_state();
    cart.write_prg(0x8000, 0x00);
    cart.write_prg(0x9000, 0x00);
    cart.write_prg(0xF004, 0x00);
    cart.restore_state(&snapshot);

    assert_eq!(cart.read_prg(0x8000), 2);
    assert_eq!(cart.mirroring(), Mirroring::OneScreenUpper);
    cart.acknowledge_irq();
    assert!(!cart.irq_pending());
    cart.clock_irq_counter_cycles(1);
    assert!(!cart.irq_pending());
    cart.clock_irq_counter_cycles(1);
    assert!(cart.irq_pending());
}

#[test]
fn mapper_22_ignores_vrc4_irq_registers_and_restores_state() {
    let mut cart = make_mapper22_cart();

    cart.write_prg(0x8000, 0x02);
    cart.write_prg(0x9000, 0x01);
    cart.write_prg(0xF000, 0x0E);
    cart.write_prg(0xF002, 0x07);
    cart.clock_irq_counter_cycles(16);
    assert!(!cart.irq_pending());

    let snapshot = cart.snapshot_state();
    cart.write_prg(0x8000, 0x00);
    cart.write_prg(0x9000, 0x00);
    cart.write_prg_ram(0x6000, 0x00);
    cart.restore_state(&snapshot);

    assert_eq!(cart.read_prg(0x8000), 2);
    assert_eq!(cart.read_prg_ram(0x6000), 0x60);
    assert_eq!(cart.mirroring(), Mirroring::Horizontal);
    assert!(!cart.irq_pending());
}

#[test]
fn mapper_23_uses_vrc2_latch_and_vrc4_irq() {
    let mut cart = make_mapper23_cart();

    cart.write_prg_ram(0x6000, 0x01);
    assert_eq!(cart.read_prg_ram(0x6000), 0x61);
    assert_eq!(cart.read_prg_ram(0x7000), 0x70);

    cart.write_prg(0x9000, 0xFF);
    assert_eq!(cart.mirroring(), Mirroring::Horizontal);

    cart.write_prg(0xF000, 0x0E);
    cart.write_prg(0xF004, 0x0F);
    cart.write_prg(0xF008, 0x07);
    cart.clock_irq_counter_cycles(1);
    assert!(!cart.irq_pending());
    cart.clock_irq_counter_cycles(1);
    assert!(cart.irq_pending());

    cart.acknowledge_irq();
    assert!(!cart.irq_pending());

    let snapshot = cart.snapshot_state();
    cart.write_prg(0x9008, 0x03);
    cart.write_prg(0xF008, 0x00);
    cart.restore_state(&snapshot);

    assert_eq!(cart.read_prg_ram(0x6000), 0x61);
    cart.clock_irq_counter_cycles(1);
    assert!(!cart.irq_pending());
    cart.clock_irq_counter_cycles(1);
    assert!(cart.irq_pending());
}

#[test]
fn mapper_26_uses_vrc6_irq_audio_and_restores_state() {
    fn reg_addr(reg: u16) -> u16 {
        (reg & !0x0003) | (((reg & 0x0001) << 1) | ((reg & 0x0002) >> 1))
    }

    let mut cart = make_mapper24_26_cart(26);

    cart.write_prg(reg_addr(0x8000), 0x02);
    cart.write_prg(reg_addr(0xB003), 0x8C);
    cart.write_prg_ram(0x6002, 0x77);

    cart.write_prg(reg_addr(0xF000), 0xFE);
    cart.write_prg(reg_addr(0xF001), 0x07);
    cart.clock_irq_counter_cycles(1);
    assert!(!cart.irq_pending());
    cart.clock_irq_counter_cycles(1);
    assert!(cart.irq_pending());

    cart.write_prg(reg_addr(0x9000), 0x8F);
    cart.write_prg(reg_addr(0x9001), 0x00);
    cart.write_prg(reg_addr(0x9002), 0x80);
    cart.write_prg(reg_addr(0x9003), 0x00);

    let mut non_zero = false;
    for _ in 0..16 {
        if cart.clock_expansion_audio().abs() > f32::EPSILON {
            non_zero = true;
            break;
        }
    }
    assert!(non_zero);

    let snapshot = cart.snapshot_state();

    cart.write_prg(reg_addr(0x8000), 0x00);
    cart.write_prg(reg_addr(0xB003), 0x00);
    cart.write_prg_ram(0x6002, 0x00);
    cart.write_prg(reg_addr(0x9002), 0x00);
    cart.acknowledge_irq();

    cart.restore_state(&snapshot);

    assert_eq!(cart.read_prg(0x8000), 0x04);
    assert_eq!(cart.read_prg_ram(0x6002), 0x77);
    assert_eq!(cart.mirroring(), Mirroring::OneScreenUpper);
    assert!(cart.irq_pending());

    let mut restored_non_zero = false;
    for _ in 0..16 {
        if cart.clock_expansion_audio().abs() > f32::EPSILON {
            restored_non_zero = true;
            break;
        }
    }
    assert!(restored_non_zero);
}

#[test]
fn mapper_24_vrc6_chr_ram_writes_use_selected_1k_banks() {
    let mut cart = make_mapper24_chr_ram_cart();

    cart.write_prg(0xD003, 0x07);
    cart.write_chr(0x0C02, 0x5A);
    assert_eq!(cart.read_chr(0x0C02), 0x5A);

    cart.write_prg(0xE000, 0x04);
    cart.write_chr(0x1001, 0xA5);
    assert_eq!(cart.read_chr(0x1001), 0xA5);

    cart.write_prg(0xD003, 0x03);
    assert_eq!(cart.read_chr(0x0C02), 0x00);

    cart.write_prg(0xD003, 0x07);
    assert_eq!(cart.read_chr(0x0C02), 0x5A);
}

#[test]
fn mapper_85_vrc7_switches_banks_irq_wram_and_restores_state() {
    let mut cart = make_vrc7_cart();
    fn write_audio_reg(cart: &mut Cartridge, register: u8, data: u8) {
        cart.write_prg(0x9010, register);
        cart.write_prg(0x9030, data);
    }
    fn audio_peak(cart: &mut Cartridge, cycles: usize) -> f32 {
        let mut peak = 0.0f32;
        for _ in 0..cycles {
            peak = peak.max(cart.clock_expansion_audio().abs());
        }
        peak
    }

    cart.write_prg(0x8000, 0x04);
    cart.write_prg(0x8010, 0x05);
    cart.write_prg(0x9000, 0x06);
    assert_eq!(cart.read_prg(0x8000), 0x04);
    assert_eq!(cart.read_prg(0xA000), 0x05);
    assert_eq!(cart.read_prg(0xC000), 0x06);
    assert_eq!(cart.read_prg(0xE000), 0x3F);

    cart.write_prg(0xA000, 0x03);
    cart.write_chr(0x0002, 0x5A);
    cart.write_prg(0xA000, 0x00);
    assert_eq!(cart.read_chr(0x0002), 0x00);
    cart.write_prg(0xA000, 0x03);
    assert_eq!(cart.read_chr(0x0002), 0x5A);

    cart.write_prg(0xE000, 0x83);
    assert_eq!(cart.mirroring(), Mirroring::OneScreenUpper);
    cart.write_prg_ram(0x6002, 0x77);
    assert_eq!(cart.read_prg_ram(0x6002), 0x77);

    cart.write_prg(0xE010, 0xFE);
    cart.write_prg(0xF000, 0x07);
    cart.clock_irq_counter_cycles(1);
    assert!(!cart.irq_pending());
    cart.clock_irq_counter_cycles(1);
    assert!(cart.irq_pending());

    write_audio_reg(&mut cart, 0x10, 0x80);
    write_audio_reg(&mut cart, 0x20, 0x19);
    write_audio_reg(&mut cart, 0x30, 0x10);
    assert!(audio_peak(&mut cart, 8000) > 0.001);

    let snapshot = cart.snapshot_state();
    cart.write_prg(0x8000, 0x00);
    cart.write_prg(0x8010, 0x01);
    cart.write_prg(0x9000, 0x02);
    cart.write_prg(0xE000, 0x40);
    cart.write_prg_ram(0x6002, 0x00);
    cart.acknowledge_irq();
    assert_eq!(audio_peak(&mut cart, 8000), 0.0);

    cart.restore_state(&snapshot);

    assert_eq!(cart.read_prg(0x8000), 0x04);
    assert_eq!(cart.read_prg(0xA000), 0x05);
    assert_eq!(cart.read_prg(0xC000), 0x06);
    assert_eq!(cart.read_prg_ram(0x6002), 0x77);
    assert_eq!(cart.mirroring(), Mirroring::OneScreenUpper);
    assert!(cart.irq_pending());
    assert!(audio_peak(&mut cart, 8000) > 0.001);
}

#[test]
fn mapper_25_supports_vrc2c_battery_ram_and_vrc4d_irq() {
    let mut cart = make_mapper25_cart(true);

    cart.write_prg_ram(0x6002, 0x77);
    assert_eq!(cart.read_prg_ram(0x6002), 0x77);

    cart.write_prg(0x9000, 0x01);
    assert_eq!(cart.mirroring(), Mirroring::Horizontal);

    cart.write_prg(0xF000, 0x0E);
    cart.write_prg(0xF008, 0x0F);
    cart.write_prg(0xF004, 0x07);
    cart.clock_irq_counter_cycles(1);
    assert!(!cart.irq_pending());
    cart.clock_irq_counter_cycles(1);
    assert!(cart.irq_pending());

    cart.acknowledge_irq();
    assert!(!cart.irq_pending());

    let snapshot = cart.snapshot_state();
    cart.write_prg_ram(0x6002, 0x00);
    cart.write_prg(0xF004, 0x00);
    cart.restore_state(&snapshot);

    assert_eq!(cart.read_prg_ram(0x6002), 0x77);
    cart.clock_irq_counter_cycles(1);
    assert!(!cart.irq_pending());
    cart.clock_irq_counter_cycles(1);
    assert!(cart.irq_pending());
}

#[test]
fn mapper_73_switches_prg_and_handles_16bit_and_8bit_irq_modes() {
    let mut cart = make_vrc3_cart();

    cart.write_prg(0xF000, 0x03);
    assert_eq!(cart.read_prg(0x8000), 3);
    assert_eq!(cart.read_prg(0xC000), 7);

    cart.write_prg_ram(0x6000, 0xA5);
    assert_eq!(cart.read_prg_ram(0x6000), 0xA5);

    cart.write_prg(0x8000, 0x0E);
    cart.write_prg(0x9000, 0x0F);
    cart.write_prg(0xA000, 0x0F);
    cart.write_prg(0xB000, 0x0F);
    cart.write_prg(0xC000, 0x02);
    cart.clock_irq_counter_cycles(1);
    assert!(!cart.irq_pending());
    cart.clock_irq_counter_cycles(1);
    assert!(cart.irq_pending());

    let snapshot = cart.snapshot_state();
    cart.write_prg(0xD000, 0x00);
    assert!(!cart.irq_pending());

    cart.restore_state(&snapshot);
    assert!(cart.irq_pending());

    cart.write_prg(0xD000, 0x00);
    cart.write_prg(0x8000, 0x0E);
    cart.write_prg(0x9000, 0x0F);
    cart.write_prg(0xA000, 0x02);
    cart.write_prg(0xB000, 0x01);
    cart.write_prg(0xC000, 0x06);
    cart.clock_irq_counter_cycles(1);
    assert!(!cart.irq_pending());
    cart.clock_irq_counter_cycles(1);
    assert!(cart.irq_pending());
    assert_eq!(cart.mappers.vrc3.as_ref().unwrap().irq_counter, 0x12FE);
}
