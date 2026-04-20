use super::super::super::*;

#[test]
fn mapper_64_supports_scanline_and_cycle_irq_modes() {
    let mut cart = make_mapper64_cart();

    cart.write_prg(0xC000, 0x00);
    cart.write_prg(0xC001, 0x00);
    cart.write_prg(0xE001, 0x00);

    cart.clock_irq_counter();
    assert!(!cart.irq_pending());
    cart.clock_irq_counter_cycles(3);
    assert!(!cart.irq_pending());
    cart.clock_irq_counter_cycles(1);
    assert!(cart.irq_pending());

    cart.write_prg(0xE000, 0x00);
    assert!(!cart.irq_pending());

    cart.write_prg(0xC001, 0x01);
    cart.write_prg(0xE001, 0x00);
    cart.clock_irq_counter();
    cart.clock_irq_counter();
    assert!(!cart.irq_pending());

    cart.clock_irq_counter_cycles(7);
    assert!(!cart.irq_pending());

    let snapshot = cart.snapshot_state();
    cart.clock_irq_counter_cycles(1);
    assert!(cart.irq_pending());

    cart.write_prg(0xE000, 0x00);
    cart.restore_state(&snapshot);
    assert!(!cart.irq_pending());
    cart.clock_irq_counter_cycles(1);
    assert!(cart.irq_pending());
}

#[test]
fn mapper_182_aliases_114_and_uses_mmc3a_irq_behavior() {
    let mut cart = make_mapper114_cart(182);

    cart.write_prg(0xA001, 0x00);
    cart.write_prg(0xC001, 0x00);
    cart.write_prg(0xE001, 0x00);
    cart.clock_irq_counter();
    cart.clock_irq_counter();
    assert!(!cart.irq_pending());

    cart.write_prg(0xA001, 0x01);
    cart.write_prg(0xC001, 0x00);
    cart.write_prg(0xE001, 0x00);
    cart.clock_irq_counter();
    assert!(!cart.irq_pending());
    cart.clock_irq_counter();
    assert!(cart.irq_pending());

    cart.write_prg(0xE000, 0x00);
    assert!(!cart.irq_pending());
}
