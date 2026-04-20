mod lifecycle;
mod transitions;
mod types;

use super::super::BandaiFcg;

pub(in crate::cartridge::mapper::bandai_fcg) use types::{BandaiEepromKind, BandaiEepromPhase};

impl BandaiFcg {
    pub(in crate::cartridge::mapper::bandai_fcg) fn eeprom_clock_control(
        &mut self,
        control: u8,
        storage: &mut [u8],
        dirty: &mut bool,
    ) {
        if self.eeprom_kind == BandaiEepromKind::None || storage.is_empty() {
            self.eeprom_data_out = true;
            return;
        }

        let read_enabled = (control & 0x80) != 0;
        let sda = if read_enabled {
            true
        } else {
            (control & 0x40) != 0
        };
        let scl = (control & 0x20) != 0;

        if self.eeprom_prev_scl && scl {
            if self.eeprom_prev_sda && !sda {
                self.eeprom_start();
            } else if !self.eeprom_prev_sda && sda {
                self.eeprom_stop();
            }
        }

        if !self.eeprom_prev_scl && scl {
            match self.eeprom_phase {
                BandaiEepromPhase::ReceivingControl
                | BandaiEepromPhase::ReceivingAddress
                | BandaiEepromPhase::ReceivingData => {
                    self.eeprom_shift = (self.eeprom_shift << 1) | u8::from(sda);
                    self.eeprom_bits += 1;
                    if self.eeprom_bits == 8 {
                        let byte = self.eeprom_shift;
                        self.eeprom_shift = 0;
                        self.eeprom_bits = 0;
                        self.eeprom_process_received_byte(byte, storage, dirty);
                    }
                }
                BandaiEepromPhase::Sending { byte, bit_index } => {
                    if bit_index == 0 {
                        self.eeprom_phase = BandaiEepromPhase::WaitAckPending;
                    } else {
                        self.eeprom_phase = BandaiEepromPhase::Sending {
                            byte,
                            bit_index: bit_index - 1,
                        };
                    }
                }
                BandaiEepromPhase::WaitAckPending => {}
                BandaiEepromPhase::WaitAck => {
                    if !sda {
                        self.eeprom_address = self.eeprom_address.wrapping_add(1);
                        let byte = storage[self.eeprom_address as usize % storage.len()];
                        self.eeprom_begin_send(byte);
                    } else {
                        self.eeprom_phase = BandaiEepromPhase::Idle;
                        self.eeprom_data_out = true;
                    }
                }
                BandaiEepromPhase::AckPending(_)
                | BandaiEepromPhase::AckLow(_)
                | BandaiEepromPhase::Idle => {}
            }
        }

        if self.eeprom_prev_scl && !scl {
            match self.eeprom_phase {
                BandaiEepromPhase::AckPending(next) => {
                    self.eeprom_phase = BandaiEepromPhase::AckLow(next);
                    self.eeprom_data_out = false;
                }
                BandaiEepromPhase::AckLow(next) => {
                    self.eeprom_transition_after_ack(next, storage);
                }
                BandaiEepromPhase::Sending { byte, bit_index } => {
                    self.eeprom_data_out = ((byte >> bit_index) & 0x01) != 0;
                }
                BandaiEepromPhase::WaitAckPending => {
                    self.eeprom_phase = BandaiEepromPhase::WaitAck;
                    self.eeprom_data_out = true;
                }
                _ => {}
            }
        }

        self.eeprom_prev_scl = scl;
        self.eeprom_prev_sda = sda;
    }
}
