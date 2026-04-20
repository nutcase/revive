use super::super::super::BandaiFcg;
use super::types::{BandaiEepromKind, BandaiEepromNext, BandaiEepromPhase};

impl BandaiFcg {
    pub(super) fn eeprom_transition_after_ack(&mut self, next: BandaiEepromNext, storage: &[u8]) {
        self.eeprom_data_out = true;
        match next {
            BandaiEepromNext::ReceiveAddress => {
                self.eeprom_phase = BandaiEepromPhase::ReceivingAddress;
                self.eeprom_shift = 0;
                self.eeprom_bits = 0;
            }
            BandaiEepromNext::ReceiveData => {
                self.eeprom_phase = BandaiEepromPhase::ReceivingData;
                self.eeprom_shift = 0;
                self.eeprom_bits = 0;
            }
            BandaiEepromNext::SendData => {
                let byte = storage[self.eeprom_address as usize % storage.len()];
                self.eeprom_begin_send(byte);
            }
        }
    }

    pub(super) fn eeprom_process_received_byte(
        &mut self,
        byte: u8,
        storage: &mut [u8],
        dirty: &mut bool,
    ) {
        match self.eeprom_phase {
            BandaiEepromPhase::ReceivingControl => match self.eeprom_kind {
                BandaiEepromKind::C24C02 => {
                    if (byte >> 1) == 0x50 {
                        let next = if byte & 0x01 == 0 {
                            BandaiEepromNext::ReceiveAddress
                        } else {
                            BandaiEepromNext::SendData
                        };
                        self.eeprom_phase = BandaiEepromPhase::AckPending(next);
                    } else {
                        self.eeprom_phase = BandaiEepromPhase::Idle;
                    }
                }
                BandaiEepromKind::X24C01 => {
                    self.eeprom_address = byte >> 1;
                    let next = if byte & 0x01 == 0 {
                        BandaiEepromNext::ReceiveData
                    } else {
                        BandaiEepromNext::SendData
                    };
                    self.eeprom_phase = BandaiEepromPhase::AckPending(next);
                }
                BandaiEepromKind::None => {
                    self.eeprom_phase = BandaiEepromPhase::Idle;
                }
            },
            BandaiEepromPhase::ReceivingAddress => {
                self.eeprom_address = byte;
                self.eeprom_phase = BandaiEepromPhase::AckPending(BandaiEepromNext::ReceiveData);
            }
            BandaiEepromPhase::ReceivingData => {
                let index = self.eeprom_address as usize % storage.len();
                if storage[index] != byte {
                    storage[index] = byte;
                    *dirty = true;
                }
                self.eeprom_address = self.eeprom_address.wrapping_add(1);
                self.eeprom_phase = BandaiEepromPhase::AckPending(BandaiEepromNext::ReceiveData);
            }
            _ => {}
        }
    }
}
