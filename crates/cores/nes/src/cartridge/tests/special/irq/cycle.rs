use super::super::super::*;

#[test]
fn mapper_40_switches_c000_bank_and_fixed_cycle_irq() {
    let mut cart = make_mapper40_cart();

    cart.write_prg(0xE000, 0x03);

    assert_eq!(cart.read_prg_ram(0x6000), 6);
    assert_eq!(cart.read_prg(0x8000), 4);
    assert_eq!(cart.read_prg(0xA000), 5);
    assert_eq!(cart.read_prg(0xC000), 3);
    assert_eq!(cart.read_prg(0xE000), 7);

    cart.write_prg(0xA000, 0x00);
    cart.clock_irq_counter_cycles(4095);
    assert!(!cart.irq_pending());
    cart.clock_irq_counter_cycles(1);
    assert!(cart.irq_pending());

    let snapshot = cart.snapshot_state();
    cart.write_prg(0x8000, 0x00);
    assert!(!cart.irq_pending());

    cart.restore_state(&snapshot);
    assert!(cart.irq_pending());
}

#[test]
fn mapper_50_uses_scrambled_c000_bank_and_fixed_cycle_irq() {
    let mut cart = make_mapper50_cart();

    cart.write_prg(0x4020, 0x07);

    assert_eq!(cart.read_prg_ram(0x6000), 15);
    assert_eq!(cart.read_prg(0x8000), 8);
    assert_eq!(cart.read_prg(0xA000), 9);
    assert_eq!(cart.read_prg(0xC000), 7);
    assert_eq!(cart.read_prg(0xE000), 11);

    cart.write_prg(0x4120, 0x01);
    cart.clock_irq_counter_cycles(4096);
    assert!(cart.irq_pending());

    let snapshot = cart.snapshot_state();
    cart.write_prg(0x4120, 0x00);
    assert!(!cart.irq_pending());

    cart.restore_state(&snapshot);
    assert!(cart.irq_pending());
}

#[test]
fn mapper_142_switches_four_8k_prg_slots_and_uses_vrc3_irq() {
    let mut cart = make_mapper142_cart();

    cart.write_prg(0xE000, 0x01);
    cart.write_prg(0xF000, 0x03);
    cart.write_prg(0xE000, 0x02);
    cart.write_prg(0xF000, 0x04);
    cart.write_prg(0xE000, 0x03);
    cart.write_prg(0xF000, 0x05);
    cart.write_prg(0xE000, 0x04);
    cart.write_prg(0xF000, 0x06);

    assert_eq!(cart.read_prg_ram(0x6000), 6);
    assert_eq!(cart.read_prg(0x8000), 3);
    assert_eq!(cart.read_prg(0xA000), 4);
    assert_eq!(cart.read_prg(0xC000), 5);
    assert_eq!(cart.read_prg(0xE000), 15);

    cart.write_chr(0x0123, 0xA5);
    assert_eq!(cart.read_chr(0x0123), 0xA5);

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
    cart.write_prg(0xE000, 0x01);
    cart.write_prg(0xF000, 0x00);
    assert!(!cart.irq_pending());
    assert_eq!(cart.read_prg(0x8000), 0);

    cart.restore_state(&snapshot);
    assert!(cart.irq_pending());
    assert_eq!(cart.read_prg(0x8000), 3);
    assert_eq!(cart.read_prg_ram(0x6000), 6);
}

#[test]
fn mapper_42_switches_low_prg_bank_and_counts_cycle_irq() {
    let mut cart = make_mapper42_cart();

    cart.write_prg(0xE000, 0x27);

    assert_eq!(cart.read_prg_ram(0x6000), 7);
    assert_eq!(cart.read_prg(0x8000), 12);
    assert_eq!(cart.read_prg(0xA000), 13);
    assert_eq!(cart.read_prg(0xC000), 14);
    assert_eq!(cart.read_prg(0xE000), 15);
    assert_eq!(cart.mirroring(), Mirroring::Vertical);

    cart.clock_irq_counter_cycles(24_575);
    assert!(!cart.irq_pending());
    cart.clock_irq_counter_cycles(1);
    assert!(cart.irq_pending());

    let snapshot = cart.snapshot_state();
    cart.write_prg(0xE000, 0x10);
    assert_eq!(cart.read_prg_ram(0x6000), 0);
    assert_eq!(cart.mirroring(), Mirroring::Horizontal);
    assert!(!cart.irq_pending());

    cart.restore_state(&snapshot);
    assert_eq!(cart.read_prg_ram(0x6000), 7);
    assert!(cart.irq_pending());
}

#[test]
fn mapper_43_maps_split_prg_layout_and_12bit_irq() {
    let mut cart = make_mapper43_cart();

    assert_eq!(cart.read_prg_low(0x5000), 0xF2);
    assert_eq!(cart.read_prg_ram(0x6000), 2);
    assert_eq!(cart.read_prg(0x8000), 1);
    assert_eq!(cart.read_prg(0xA000), 0);
    assert_eq!(cart.read_prg(0xE000), 0xE8);

    cart.write_prg(0x4022, 0x01);
    assert_eq!(cart.read_prg(0xC000), 3);
    cart.write_prg(0x4022, 0x05);
    assert_eq!(cart.read_prg(0xC000), 7);

    cart.write_prg(0x4122, 0x01);
    cart.clock_irq_counter_cycles(4095);
    assert!(!cart.irq_pending());
    cart.clock_irq_counter_cycles(1);
    assert!(cart.irq_pending());

    let snapshot = cart.snapshot_state();
    cart.write_prg(0x4122, 0x00);
    assert!(!cart.irq_pending());

    cart.restore_state(&snapshot);
    assert!(cart.irq_pending());
    assert_eq!(cart.read_prg(0xC000), 7);
}
