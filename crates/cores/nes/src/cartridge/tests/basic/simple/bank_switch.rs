use super::super::super::*;

#[test]
fn mapper_11_switches_prg_and_chr_banks() {
    let mut cart = make_simple_bank_cart(11, 4, 16);

    cart.write_prg(0x8000, 0xA1);

    assert_eq!(cart.read_prg(0x8000), 1);
    assert_eq!(cart.read_prg(0xFFFF), 1);
    assert_eq!(cart.read_chr(0x0000), 0x4A);
    assert_eq!(cart.read_chr(0x1FFF), 0x4A);
}

#[test]
fn mapper_66_switches_prg_and_chr_banks() {
    let mut cart = make_simple_bank_cart(66, 4, 4);

    cart.write_prg(0x8000, 0x32);

    assert_eq!(cart.read_prg(0x8000), 3);
    assert_eq!(cart.read_prg(0xBFFF), 3);
    assert_eq!(cart.read_chr(0x0000), 0x42);
    assert_eq!(cart.read_chr(0x1FFF), 0x42);
}

#[test]
fn mapper_34_bnrom_switches_32k_prg_bank() {
    let mut cart = make_simple_bank_cart(34, 4, 1);
    cart.prg_rom[0] = 0xFF;

    cart.write_prg(0x8000, 0x02);

    assert_eq!(cart.read_prg(0x8000), 2);
    assert_eq!(cart.read_prg(0xFFFF), 2);
    assert_eq!(cart.read_chr(0x0000), 0x40);
}

#[test]
fn mapper_34_nina001_switches_prg_and_chr_halves() {
    let mut cart = make_nina001_cart();

    cart.write_prg_ram(0x7FFD, 0x03);
    cart.write_prg_ram(0x7FFE, 0x01);
    cart.write_prg_ram(0x7FFF, 0x02);

    assert_eq!(cart.read_prg(0x8000), 3);
    assert_eq!(cart.read_prg_ram(0x7FFD), 0x03);
    assert_eq!(cart.read_chr(0x0000), 0x51);
    assert_eq!(cart.read_chr(0x1000), 0x52);

    let snapshot = cart.snapshot_state();
    cart.prg_bank = 0;
    cart.chr_bank = 0;
    cart.chr_bank_1 = 1;
    cart.restore_state(&snapshot);

    assert_eq!(cart.prg_bank, 3);
    assert_eq!(cart.chr_bank, 1);
    assert_eq!(cart.chr_bank_1, 2);
}

#[test]
fn mapper_71_switches_low_prg_bank_and_mirroring() {
    let mut cart = make_camerica_cart();

    cart.write_prg(0xC000, 0x03);
    cart.write_prg(0x9000, 0x10);

    assert_eq!(cart.read_prg(0x8000), 3);
    assert_eq!(cart.read_prg(0xBFFF), 3);
    assert_eq!(cart.read_prg(0xC000), 7);
    assert_eq!(cart.mirroring(), Mirroring::OneScreenUpper);
}

#[test]
fn mapper_79_switches_32k_prg_and_chr_banks_via_low_address_latch() {
    let mut cart = make_simple_bank_cart(79, 2, 8);

    cart.write_prg(0x4100, 0x0B);

    assert_eq!(cart.read_prg(0x8000), 1);
    assert_eq!(cart.read_prg(0xFFFF), 1);
    assert_eq!(cart.read_chr(0x0000), 0x43);
}

#[test]
fn mapper_41_switches_outer_and_inner_chr_banks_with_reset() {
    let mut cart = make_simple_bank_cart(41, 8, 16);

    cart.write_prg_ram(0x600C, 0x00);
    cart.prg_rom[4 * 0x8000] = 0x03;
    cart.write_prg(0x8000, 0x03);

    assert_eq!(cart.prg_bank, 4);
    assert_eq!(cart.read_prg(0x8001), 4);
    assert_eq!(cart.read_chr(0x0000), 0x47);
    assert_eq!(cart.mirroring(), Mirroring::Vertical);

    let snapshot = cart.snapshot_state();

    cart.write_prg_ram(0x6008, 0x00);
    assert_eq!(cart.prg_bank, 0);
    assert_eq!(cart.read_chr(0x0000), 0x44);

    cart.restore_state(&snapshot);
    assert_eq!(cart.prg_bank, 4);
    assert_eq!(cart.read_chr(0x0000), 0x47);

    cart.on_reset();
    assert_eq!(cart.prg_bank, 0);
    assert_eq!(cart.read_chr(0x0000), 0x40);
    assert_eq!(cart.mirroring(), Mirroring::Vertical);
}

#[test]
fn mapper_57_switches_prg_chr_and_mirroring_from_address_latch() {
    let mut cart = make_mapper57_cart();

    cart.write_prg(0x8000, 0xAD);

    assert_eq!(cart.read_prg(0x8000), 5);
    assert_eq!(cart.read_prg(0xC000), 7);
    assert_eq!(cart.read_chr(0x0000), 0xA5);
    assert_eq!(cart.mirroring(), Mirroring::Horizontal);

    let snapshot = cart.snapshot_state();

    cart.write_prg(0x8800, 0x66);
    assert_eq!(cart.read_prg(0x8000), 1);
    assert_eq!(cart.read_chr(0x0000), 0xA2);
    assert_eq!(cart.mirroring(), Mirroring::Vertical);

    cart.restore_state(&snapshot);
    assert_eq!(cart.read_prg(0x8000), 5);
    assert_eq!(cart.read_chr(0x0000), 0xA5);
    assert_eq!(cart.mirroring(), Mirroring::Horizontal);
}

#[test]
fn mapper_99_switches_low_prg_chr_and_shared_ram() {
    let mut cart = make_mapper99_cart();

    assert_eq!(cart.read_prg(0x8000), 0);
    assert_eq!(cart.read_prg(0xA000), 1);
    assert_eq!(cart.read_prg(0xE000), 3);
    assert_eq!(cart.read_chr(0x0000), 0x90);

    cart.write_prg_low(0x4016, 0x04);
    assert_eq!(cart.read_prg(0x8000), 4);
    assert_eq!(cart.read_chr(0x0000), 0x91);
    assert_eq!(cart.read_prg(0xA000), 1);

    cart.write_prg_ram(0x6000, 0x12);
    cart.write_prg_ram(0x6800, 0x34);
    assert_eq!(cart.read_prg_ram(0x6000), 0x34);
    assert_eq!(cart.read_prg_ram(0x7800), 0x34);
}
