use super::super::super::BandaiFcg;
use super::types::{BandaiEepromKind, BandaiEepromPhase};

impl BandaiFcg {
    pub(in crate::cartridge) fn configure_mapper(&mut self, mapper: u16, has_battery: bool) {
        self.eeprom_kind = if mapper == 159 {
            BandaiEepromKind::X24C01
        } else if mapper == 16 && has_battery {
            BandaiEepromKind::C24C02
        } else {
            BandaiEepromKind::None
        };
    }

    pub(super) fn eeprom_start(&mut self) {
        if self.eeprom_kind == BandaiEepromKind::None {
            return;
        }
        self.eeprom_phase = BandaiEepromPhase::ReceivingControl;
        self.eeprom_shift = 0;
        self.eeprom_bits = 0;
        self.eeprom_data_out = true;
    }

    pub(super) fn eeprom_stop(&mut self) {
        if self.eeprom_kind == BandaiEepromKind::None {
            return;
        }
        self.eeprom_phase = BandaiEepromPhase::Idle;
        self.eeprom_data_out = true;
        self.eeprom_shift = 0;
        self.eeprom_bits = 0;
    }

    pub(super) fn eeprom_begin_send(&mut self, byte: u8) {
        self.eeprom_phase = BandaiEepromPhase::Sending { byte, bit_index: 7 };
        self.eeprom_data_out = (byte & 0x80) != 0;
    }
}
