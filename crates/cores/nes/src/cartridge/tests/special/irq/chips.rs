use super::super::super::*;

#[test]
fn mapper_18_uses_irq_width_control_and_mirroring() {
    let mut cart = make_mapper18_cart();

    cart.write_prg(0xF002, 0x00);
    assert_eq!(cart.mirroring(), Mirroring::Horizontal);
    cart.write_prg(0xF002, 0x01);
    assert_eq!(cart.mirroring(), Mirroring::Vertical);
    cart.write_prg(0xF002, 0x02);
    assert_eq!(cart.mirroring(), Mirroring::OneScreenLower);
    cart.write_prg(0xF002, 0x03);
    assert_eq!(cart.mirroring(), Mirroring::OneScreenUpper);

    cart.write_prg(0xE000, 0x0);
    cart.write_prg(0xE001, 0x0);
    cart.write_prg(0xE002, 0x0);
    cart.write_prg(0xE003, 0x1);
    cart.write_prg(0xF000, 0);
    cart.write_prg(0xF001, 0x03);
    cart.clock_irq_counter_cycles(1);
    assert!(cart.irq_pending());

    cart.write_prg(0xF000, 0);
    assert!(!cart.irq_pending());

    cart.write_prg(0xE000, 0x2);
    cart.write_prg(0xE001, 0x0);
    cart.write_prg(0xE002, 0x0);
    cart.write_prg(0xE003, 0x0);
    cart.write_prg(0xF000, 0);
    cart.write_prg(0xF001, 0x09);
    cart.clock_irq_counter_cycles(2);
    assert!(!cart.irq_pending());
    cart.clock_irq_counter_cycles(1);
    assert!(cart.irq_pending());
}

#[test]
fn mapper_67_switches_chr_prg_mirroring_and_cycle_irq() {
    let mut cart = make_mapper67_cart();

    cart.write_prg(0x8800, 0x01);
    cart.write_prg(0x9800, 0x02);
    cart.write_prg(0xA800, 0x03);
    cart.write_prg(0xB800, 0x04);
    cart.write_prg(0xE800, 0x03);
    cart.write_prg(0xF800, 0x03);

    assert_eq!(cart.read_prg(0x8000), 3);
    assert_eq!(cart.read_prg(0xC000), 7);
    assert_eq!(cart.read_chr(0x0000), 0x71);
    assert_eq!(cart.read_chr(0x0800), 0x72);
    assert_eq!(cart.read_chr(0x1000), 0x73);
    assert_eq!(cart.read_chr(0x1800), 0x74);
    assert_eq!(cart.mirroring(), Mirroring::OneScreenUpper);

    cart.write_prg(0xC800, 0x00);
    cart.write_prg(0xC800, 0x02);
    cart.write_prg(0xD800, 0x10);
    cart.clock_irq_counter_cycles(2);
    assert!(!cart.irq_pending());
    cart.clock_irq_counter_cycles(1);
    assert!(cart.irq_pending());

    let snapshot = cart.snapshot_state();
    cart.write_prg(0x8000, 0x00);
    assert!(!cart.irq_pending());

    cart.restore_state(&snapshot);
    assert!(cart.irq_pending());
    assert_eq!(cart.read_prg(0x8000), 3);
    assert_eq!(cart.read_chr(0x1800), 0x74);
}

#[test]
fn mapper_65_switches_prg_chr_and_cycle_irq() {
    let mut cart = make_mapper65_cart();

    cart.write_prg(0x8000, 0x03);
    cart.write_prg(0xA000, 0x04);
    for index in 0..8 {
        cart.write_prg(0xB000 + index as u16, 0x08 + index as u8);
    }

    assert_eq!(cart.read_prg(0x8000), 3);
    assert_eq!(cart.read_prg(0xA000), 4);
    assert_eq!(cart.read_prg(0xC000), 14);
    assert_eq!(cart.read_prg(0xE000), 15);
    assert_eq!(cart.read_chr(0x0000), 0x98);
    assert_eq!(cart.read_chr(0x1C00), 0x9F);

    cart.write_prg(0x9000, 0x80);
    cart.write_prg(0x9001, 0x80);
    assert_eq!(cart.read_prg(0x8000), 14);
    assert_eq!(cart.read_prg(0xC000), 3);
    assert_eq!(cart.mirroring(), Mirroring::Horizontal);

    cart.write_prg(0x9005, 0x00);
    cart.write_prg(0x9006, 0x02);
    cart.write_prg(0x9004, 0x00);
    cart.write_prg(0x9003, 0x80);
    cart.clock_irq_counter_cycles(1);
    assert!(!cart.irq_pending());
    cart.clock_irq_counter_cycles(1);
    assert!(cart.irq_pending());

    let snapshot = cart.snapshot_state();
    cart.write_prg(0x9003, 0x00);
    assert!(!cart.irq_pending());

    cart.restore_state(&snapshot);
    assert!(cart.irq_pending());
    assert_eq!(cart.read_prg(0xC000), 3);
}

#[test]
fn mapper_159_uses_x24c01_eeprom_and_bandai_banks() {
    let mut cart = make_mapper159_cart();

    cart.write_prg(0x8000, 0x02);
    cart.write_prg(0x8001, 0x03);
    cart.write_prg(0x8008, 0x05);
    cart.write_prg(0x8009, 0x01);

    assert_eq!(cart.read_prg(0x8000), 5);
    assert_eq!(cart.read_prg(0xC000), 31);
    assert_eq!(cart.read_chr(0x0000), 0x82);
    assert_eq!(cart.read_chr(0x0400), 0x83);
    assert_eq!(cart.mirroring(), Mirroring::Horizontal);
    assert_eq!(cart.read_prg_ram(0x6000), 0x10);

    cart.write_prg(0x800B, 0x01);
    cart.write_prg(0x800C, 0x00);
    cart.write_prg(0x800A, 0x01);
    cart.clock_irq_counter_cycles(1);
    assert!(!cart.irq_pending());
    cart.clock_irq_counter_cycles(1);
    assert!(cart.irq_pending());

    let snapshot = cart.snapshot_state();
    cart.write_prg(0x8000, 0x00);
    cart.write_prg(0x8008, 0x00);
    cart.write_prg(0x8009, 0x00);
    cart.restore_state(&snapshot);

    assert_eq!(cart.read_prg(0x8000), 5);
    assert_eq!(cart.read_chr(0x0000), 0x82);
    assert_eq!(cart.mirroring(), Mirroring::Horizontal);
    assert!(cart.irq_pending());
}
