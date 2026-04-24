use super::super::BandaiFcg;
use crate::cartridge::{
    Cartridge, MapperRuntime, Mirroring, Mmc3VariantState, MulticartMapperState, SimpleMapperState,
};

const EEPROM_READ: u8 = 0x80;
const EEPROM_SDA: u8 = 0x40;
const EEPROM_SCL: u8 = 0x20;

fn make_bandai_eeprom_cart(mapper: u8, size: usize) -> Cartridge {
    let mapper_id = u16::from(mapper);
    let mut cart = Cartridge {
        prg_rom: vec![0; 0x8000],
        chr_rom: vec![0; 0x2000],
        chr_ram: vec![],
        prg_ram: vec![0xFF; size],
        has_valid_save_data: false,
        mapper: mapper_id,
        mirroring: Mirroring::Horizontal,
        has_battery: true,
        chr_bank: 0,
        chr_bank_1: 1,
        prg_bank: 0,
        mappers: MapperRuntime {
            simple: SimpleMapperState::new(mapper_id, false, true, false),
            multicart: MulticartMapperState::new(mapper_id, false),
            mmc3_variant: Mmc3VariantState::new(mapper_id, 0x2000),
            mmc1: None,
            mmc2: None,
            mmc3: None,
            mmc5: None,
            namco163: None,
            namco210: None,
            jaleco_ss88006: None,
            vrc2_vrc4: None,
            mapper40: None,
            mapper42: None,
            mapper43: None,
            mapper50: None,
            fme7: None,
            bandai_fcg: Some(BandaiFcg::new()),
            irem_g101: None,
            irem_h3001: None,
            vrc1: None,
            vrc3: None,
            vrc6: None,
            vrc7: None,
            mapper15: None,
            sunsoft3: None,
            sunsoft4: None,
            taito_tc0190: None,
            taito_x1005: None,
            taito_x1017: None,
            mapper246: None,
        },
    };
    if let Some(ref mut bandai) = cart.mappers.bandai_fcg {
        bandai.configure_mapper(mapper_id, true);
    }
    cart
}

fn drive(cart: &mut Cartridge, read: bool, sda: bool, scl: bool) {
    let mut data = 0;
    if read {
        data |= EEPROM_READ;
    }
    if sda {
        data |= EEPROM_SDA;
    }
    if scl {
        data |= EEPROM_SCL;
    }
    cart.write_prg_bandai(0x800D, data);
}

fn start(cart: &mut Cartridge) {
    drive(cart, false, true, false);
    drive(cart, false, true, true);
    drive(cart, false, false, true);
    drive(cart, false, false, false);
}

fn stop(cart: &mut Cartridge) {
    drive(cart, false, false, false);
    drive(cart, false, false, true);
    drive(cart, false, true, true);
    drive(cart, false, true, false);
}

fn write_bit(cart: &mut Cartridge, bit: bool) {
    drive(cart, false, bit, false);
    drive(cart, false, bit, true);
    drive(cart, false, bit, false);
}

fn read_bit(cart: &mut Cartridge) -> bool {
    drive(cart, true, true, false);
    drive(cart, true, true, true);
    let bit = cart.read_prg_ram_bandai(0x6000) & 0x10 != 0;
    drive(cart, true, true, false);
    bit
}

fn write_byte(cart: &mut Cartridge, byte: u8) -> bool {
    for shift in (0..8).rev() {
        write_bit(cart, ((byte >> shift) & 1) != 0);
    }
    !read_bit(cart)
}

fn read_byte(cart: &mut Cartridge, ack: bool) -> u8 {
    let mut byte = 0;
    for _ in 0..8 {
        byte = (byte << 1) | u8::from(read_bit(cart));
    }
    write_bit(cart, !ack);
    byte
}

fn write_byte_lsb(cart: &mut Cartridge, byte: u8) -> bool {
    for shift in 0..8 {
        write_bit(cart, ((byte >> shift) & 1) != 0);
    }
    !read_bit(cart)
}

fn read_byte_lsb(cart: &mut Cartridge, ack: bool) -> u8 {
    let mut byte = 0;
    for shift in 0..8 {
        byte |= u8::from(read_bit(cart)) << shift;
    }
    write_bit(cart, !ack);
    byte
}

#[test]
fn bandai_eeprom_round_trips_a_byte() {
    let mut cart = make_bandai_eeprom_cart(16, 256);

    start(&mut cart);
    assert!(write_byte(&mut cart, 0xA0));
    assert!(write_byte(&mut cart, 0x2A));
    assert!(write_byte(&mut cart, 0x5C));
    stop(&mut cart);

    start(&mut cart);
    assert!(write_byte(&mut cart, 0xA0));
    assert!(write_byte(&mut cart, 0x2A));
    start(&mut cart);
    assert!(write_byte(&mut cart, 0xA1));
    let value = read_byte(&mut cart, false);
    stop(&mut cart);

    assert_eq!(value, 0x5C);
    assert_eq!(cart.prg_ram[0x2A], 0x5C);
    assert!(cart.has_valid_save_data);
}

#[test]
fn bandai_eeprom_idle_line_reads_high() {
    let mut cart = make_bandai_eeprom_cart(16, 256);
    drive(&mut cart, true, true, true);
    assert_eq!(cart.read_prg_ram_bandai(0x6000) & 0x10, 0x10);
}

#[test]
fn bandai_x24c01_round_trips_a_byte() {
    let mut cart = make_bandai_eeprom_cart(159, 128);

    start(&mut cart);
    assert!(write_byte(&mut cart, 0x54));
    assert!(write_byte(&mut cart, 0x5C));
    stop(&mut cart);

    start(&mut cart);
    assert!(write_byte(&mut cart, 0x55));
    let value = read_byte(&mut cart, false);
    stop(&mut cart);

    assert_eq!(value, 0x5C);
    assert_eq!(cart.prg_ram[0x2A], 0x5C);
    assert!(cart.has_valid_save_data);
}

#[test]
fn bandai_x24c01_does_not_follow_lsb_first_assumption() {
    let mut cart = make_bandai_eeprom_cart(159, 128);

    start(&mut cart);
    assert!(write_byte_lsb(&mut cart, 0x54));
    assert!(write_byte_lsb(&mut cart, 0x5C));
    stop(&mut cart);

    start(&mut cart);
    assert!(write_byte_lsb(&mut cart, 0x55));
    let value = read_byte_lsb(&mut cart, false);
    stop(&mut cart);

    assert_ne!(value, 0x5C);
    assert!(cart.has_valid_save_data);
}
