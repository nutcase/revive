use super::super::{Cartridge, Mirroring};

impl Cartridge {
    pub fn on_reset(&mut self) {
        self.reset_simple_mapper_state();
        self.reset_multicart_mapper_state();
        self.reset_mmc5_frame_state();
    }

    fn reset_simple_mapper_state(&mut self) {
        match self.mapper {
            41 => {
                self.prg_bank = 0;
                self.chr_bank = 0;
                self.mappers.simple.mapper41_inner_bank = 0;
                self.mirroring = Mirroring::Vertical;
            }
            185 => {
                self.mappers.simple.mapper185_disabled_reads.set(2);
            }
            _ => {}
        }
    }

    fn reset_multicart_mapper_state(&mut self) {
        match self.mapper {
            59 => {
                self.mappers.multicart.mapper59_locked = false;
            }
            60 => {
                self.advance_mapper60_game();
            }
            63 => {
                self.mappers.multicart.mapper63_latch = 0;
                self.mirroring = Mirroring::Vertical;
            }
            230 => {
                self.mappers.multicart.mapper230_contra_mode =
                    !self.mappers.multicart.mapper230_contra_mode;
                if self.mappers.multicart.mapper230_contra_mode {
                    self.mirroring = Mirroring::Vertical;
                }
            }
            _ => {}
        }
    }

    fn reset_mmc5_frame_state(&self) {
        if self.mapper == 5 {
            self.mmc5_end_frame();
        }
    }
}
