use super::super::*;

#[test]
fn snapshot_and_restore_keeps_mmc1_and_ram_state() {
    let mut cart = make_mmc1_cart();
    {
        let mmc1 = cart.mappers.mmc1.as_mut().unwrap();
        mmc1.shift_register = 0x1B;
        mmc1.shift_count = 3;
        mmc1.control = 0x12;
        mmc1.chr_bank_0 = 7;
        mmc1.chr_bank_1 = 9;
        mmc1.prg_bank = 5;
        mmc1.prg_ram_disable = true;
    }
    cart.prg_ram[0x10] = 0xAA;
    cart.chr_ram[0x20] = 0x55;

    let snapshot = cart.snapshot_state();

    cart.mirroring = Mirroring::Horizontal;
    cart.has_valid_save_data = false;
    cart.prg_ram.fill(0);
    cart.chr_ram.fill(0);
    {
        let mmc1 = cart.mappers.mmc1.as_mut().unwrap();
        mmc1.shift_register = 0x10;
        mmc1.shift_count = 0;
        mmc1.control = 0x0C;
        mmc1.chr_bank_0 = 0;
        mmc1.chr_bank_1 = 0;
        mmc1.prg_bank = 0;
        mmc1.prg_ram_disable = false;
    }

    cart.restore_state(&snapshot);

    assert_eq!(cart.mirroring, Mirroring::Vertical);
    assert!(cart.has_valid_save_data);
    assert_eq!(cart.prg_ram[0x10], 0xAA);
    assert_eq!(cart.chr_ram[0x20], 0x55);

    let mmc1 = cart.mappers.mmc1.as_ref().unwrap();
    assert_eq!(mmc1.shift_register, 0x1B);
    assert_eq!(mmc1.shift_count, 3);
    assert_eq!(mmc1.control, 0x12);
    assert_eq!(mmc1.chr_bank_0, 7);
    assert_eq!(mmc1.chr_bank_1, 9);
    assert_eq!(mmc1.prg_bank, 5);
    assert!(mmc1.prg_ram_disable);
}

#[test]
fn restore_state_ignores_mapper_mismatch() {
    let mut cart = make_mmc1_cart();
    let mut state = cart.snapshot_state();
    state.mapper = 2;

    cart.prg_ram[0] = 0x11;
    cart.restore_state(&state);
    assert_eq!(cart.prg_ram[0], 0x11);
}
