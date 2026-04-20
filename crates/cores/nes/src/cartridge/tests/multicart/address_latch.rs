use super::super::*;

#[test]
fn mapper_240_switches_prg_chr_and_exposes_prg_ram() {
    let mut cart = make_simple_bank_cart(240, 4, 4);
    cart.prg_ram = vec![0; 0x2000];

    cart.write_prg(0x4800, 0x21);
    assert_eq!(cart.read_prg(0x8000), 2);
    assert_eq!(cart.read_chr(0x0000), 0x41);

    cart.write_prg(0x4100, 0x32);
    assert_eq!(cart.read_prg(0x8000), 3);
    assert_eq!(cart.read_chr(0x0000), 0x42);

    cart.write_prg_ram(0x6000, 0xA5);
    assert_eq!(cart.read_prg_ram(0x6000), 0xA5);
}

#[test]
fn mapper_213_matches_mapper_58_address_latch_behavior() {
    let mut cart = make_uxrom_like_cart(213, 16, 8);

    cart.write_prg(0x80DA, 0);

    assert_eq!(cart.read_prg(0x8000), 2);
    assert_eq!(cart.read_prg(0xC000), 2);
    assert_eq!(cart.read_chr(0x0000), 0x73);
    assert_eq!(cart.mirroring(), Mirroring::Horizontal);

    let snapshot = cart.snapshot_state();
    cart.write_prg(0x8000, 0);
    cart.restore_state(&snapshot);

    assert_eq!(cart.read_prg(0x8000), 2);
    assert_eq!(cart.read_prg(0xC000), 2);
    assert_eq!(cart.read_chr(0x0000), 0x73);
}

#[test]
fn mapper_241_switches_32k_prg_from_4800_window_and_exposes_wram() {
    let mut cart = make_simple_bank_cart(241, 4, 1);
    cart.prg_ram = vec![0; 0x2000];

    cart.write_prg(0x4800, 0x02);
    assert_eq!(cart.read_prg(0x8000), 2);
    assert_eq!(cart.read_prg(0xFFFF), 2);
    assert_eq!(cart.read_chr(0x0000), 0x40);

    cart.write_prg_ram(0x6000, 0x5A);
    assert_eq!(cart.read_prg_ram(0x6000), 0x5A);
}

#[test]
fn mapper_200_latches_mirrored_16k_prg_chr_and_mirroring_from_address() {
    let mut cart = make_uxrom_like_cart(200, 16, 16);

    cart.write_prg(0x800B, 0);

    assert_eq!(cart.read_prg(0x8000), 11);
    assert_eq!(cart.read_prg(0xC000), 11);
    assert_eq!(cart.read_chr(0x0000), 0x7B);
    assert_eq!(cart.read_chr(0x1FFF), 0x7B);
    assert_eq!(cart.mirroring(), Mirroring::Horizontal);

    cart.write_prg(0x8002, 0);
    assert_eq!(cart.read_prg(0x8000), 2);
    assert_eq!(cart.mirroring(), Mirroring::Vertical);
}

#[test]
fn mapper_201_uses_low_address_byte_for_prg_and_chr_bank() {
    let mut cart = make_simple_bank_cart(201, 8, 8);

    cart.write_prg(0x80C5, 0);

    assert_eq!(cart.read_prg(0x8000), 5);
    assert_eq!(cart.read_prg(0xFFFF), 5);
    assert_eq!(cart.read_chr(0x0000), 0x45);
    assert_eq!(cart.read_chr(0x1FFF), 0x45);
}

#[test]
fn mapper_202_switches_between_mirrored_16k_and_32k_modes() {
    let mut cart = make_uxrom_like_cart(202, 16, 8);

    cart.write_prg(0x800C, 0);
    assert_eq!(cart.read_prg(0x8000), 6);
    assert_eq!(cart.read_prg(0xC000), 6);
    assert_eq!(cart.read_chr(0x0000), 0x76);
    assert_eq!(cart.mirroring(), Mirroring::Vertical);

    cart.write_prg(0x8009, 0);
    assert_eq!(cart.read_prg(0x8000), 8);
    assert_eq!(cart.read_prg(0xC000), 9);
    assert_eq!(cart.read_chr(0x0000), 0x74);
    assert_eq!(cart.mirroring(), Mirroring::Horizontal);

    let snapshot = cart.snapshot_state();
    cart.write_prg(0x8000, 0);
    cart.restore_state(&snapshot);

    assert_eq!(cart.read_prg(0x8000), 8);
    assert_eq!(cart.read_prg(0xC000), 9);
    assert_eq!(cart.mirroring(), Mirroring::Horizontal);
}

#[test]
fn mapper_203_uses_data_latch_for_mirrored_prg_and_chr_bank() {
    let mut cart = make_uxrom_like_cart(203, 16, 4);

    cart.write_prg(0x8000, 0x1D);

    assert_eq!(cart.read_prg(0x8000), 7);
    assert_eq!(cart.read_prg(0xC000), 7);
    assert_eq!(cart.read_chr(0x0000), 0x71);
    assert_eq!(cart.read_chr(0x1FFF), 0x71);
}

#[test]
fn mapper_229_switches_shared_bank_and_special_cases_bank_zero() {
    let mut cart = make_uxrom_like_cart(229, 16, 8);

    cart.write_prg(0x8020, 0);
    assert_eq!(cart.read_prg(0x8000), 0);
    assert_eq!(cart.read_prg(0xC000), 1);
    assert_eq!(cart.read_chr(0x0000), 0x70);
    assert_eq!(cart.mirroring(), Mirroring::Horizontal);

    cart.write_prg(0x8005, 0);
    assert_eq!(cart.read_prg(0x8000), 5);
    assert_eq!(cart.read_prg(0xC000), 5);
    assert_eq!(cart.read_chr(0x0000), 0x75);
    assert_eq!(cart.mirroring(), Mirroring::Vertical);
}

#[test]
fn mapper_212_switches_between_mirrored_16k_and_32k_modes() {
    let mut cart = make_uxrom_like_cart(212, 8, 8);

    cart.write_prg(0x800D, 0);
    assert_eq!(cart.read_prg(0x8000), 5);
    assert_eq!(cart.read_prg(0xC000), 5);
    assert_eq!(cart.read_chr(0x0000), 0x75);
    assert_eq!(cart.mirroring(), Mirroring::Horizontal);
    assert_eq!(cart.read_prg_ram(0x6000), 0x80);

    cart.write_prg(0xC006, 0);
    assert_eq!(cart.read_prg(0x8000), 6);
    assert_eq!(cart.read_prg(0xC000), 7);
    assert_eq!(cart.read_chr(0x0000), 0x76);
    assert_eq!(cart.mirroring(), Mirroring::Vertical);
}
