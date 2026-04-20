use std::cell::Cell;

mod chr;
mod eeprom;
mod irq;
mod prg;
mod ram;

use eeprom::{BandaiEepromKind, BandaiEepromPhase};

/// Bandai FCG / LZ93D50 (Mapper 16).
/// Used by Dragon Ball Z series and other Bandai games.
/// Features: 8x1KB CHR banking, 16KB PRG banking, CPU-cycle IRQ counter.
#[derive(Debug, Clone)]
pub(in crate::cartridge) struct BandaiFcg {
    pub(in crate::cartridge) chr_banks: [u8; 8],
    pub(in crate::cartridge) prg_bank: u8,
    pub(in crate::cartridge) outer_prg_bank: u8,
    pub(in crate::cartridge) irq_counter: u16,
    pub(in crate::cartridge) irq_latch: u16,
    pub(in crate::cartridge) irq_enabled: bool,
    pub(in crate::cartridge) irq_pending: Cell<bool>,
    pub(in crate::cartridge) prg_ram_enabled: bool,
    eeprom_kind: BandaiEepromKind,
    eeprom_phase: BandaiEepromPhase,
    eeprom_address: u8,
    eeprom_shift: u8,
    eeprom_bits: u8,
    eeprom_prev_scl: bool,
    eeprom_prev_sda: bool,
    eeprom_data_out: bool,
}

impl BandaiFcg {
    pub(in crate::cartridge) fn new() -> Self {
        BandaiFcg {
            chr_banks: [0; 8],
            prg_bank: 0,
            outer_prg_bank: 0,
            irq_counter: 0,
            irq_latch: 0,
            irq_enabled: false,
            irq_pending: Cell::new(false),
            prg_ram_enabled: false,
            eeprom_kind: BandaiEepromKind::C24C02,
            eeprom_phase: BandaiEepromPhase::Idle,
            eeprom_address: 0,
            eeprom_shift: 0,
            eeprom_bits: 0,
            eeprom_prev_scl: false,
            eeprom_prev_sda: true,
            eeprom_data_out: true,
        }
    }
}
