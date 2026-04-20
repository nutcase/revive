use crate::cartridge::state::types::*;
use crate::cartridge::Cartridge;

pub(super) struct MulticartSnapshotStates {
    pub(super) mapper58: Option<Mapper58State>,
    pub(super) mapper59: Option<Mapper59State>,
    pub(super) mapper60: Option<Mapper60State>,
    pub(super) mapper61: Option<Mapper61State>,
    pub(super) mapper63: Option<Mapper63State>,
    pub(super) mapper137: Option<Mapper137State>,
    pub(super) mapper142: Option<Mapper142State>,
    pub(super) mapper150: Option<Mapper150State>,
    pub(super) mapper225: Option<Mapper225State>,
    pub(super) mapper232: Option<Mapper232State>,
    pub(super) mapper41: Option<Mapper41State>,
}

impl Cartridge {
    pub(super) fn snapshot_multicart_mapper_states(&self) -> MulticartSnapshotStates {
        let mapper58 = if matches!(self.mapper, 58 | 213) {
            Some(Mapper58State {
                nrom128: self.mappers.multicart.mapper58_nrom128,
            })
        } else {
            None
        };
        let mapper59 = if self.mapper == 59 {
            Some(Mapper59State {
                latch: self.mappers.multicart.mapper59_latch,
                locked: self.mappers.multicart.mapper59_locked,
            })
        } else {
            None
        };
        let mapper60 = if self.mapper == 60 {
            Some(Mapper60State {
                game_select: self.mappers.multicart.mapper60_game_select,
            })
        } else {
            None
        };
        let mapper61 = if self.mapper == 61 {
            Some(Mapper61State {
                latch: self.mappers.multicart.mapper61_latch,
            })
        } else {
            None
        };
        let mapper63 = if self.mapper == 63 {
            Some(Mapper63State {
                latch: self.mappers.multicart.mapper63_latch,
            })
        } else {
            None
        };
        let mapper137 = if self.mapper == 137 {
            Some(Mapper137State {
                index: self.mappers.simple.mapper137_index,
                registers: self.mappers.simple.mapper137_registers,
            })
        } else {
            None
        };
        let mapper142 = if self.mapper == 142 {
            Some(Mapper142State {
                bank_select: self.mappers.simple.mapper142_bank_select,
                prg_banks: self.mappers.simple.mapper142_prg_banks,
            })
        } else {
            None
        };
        let mapper150 = if self.mapper == 150 {
            Some(Mapper150State {
                index: self.mappers.simple.mapper150_index,
                registers: self.mappers.simple.mapper150_registers,
            })
        } else {
            None
        };
        let mapper225 = if matches!(self.mapper, 225 | 255) {
            Some(Mapper225State {
                nrom128: self.mappers.multicart.mapper225_nrom128,
            })
        } else {
            None
        };
        let mapper232 = if self.mapper == 232 {
            Some(Mapper232State {
                outer_bank: self.mappers.multicart.mapper232_outer_bank,
            })
        } else {
            None
        };
        let mapper41 = if self.mapper == 41 {
            Some(Mapper41State {
                inner_bank: self.mappers.simple.mapper41_inner_bank,
            })
        } else {
            None
        };

        MulticartSnapshotStates {
            mapper58,
            mapper59,
            mapper60,
            mapper61,
            mapper63,
            mapper137,
            mapper142,
            mapper150,
            mapper225,
            mapper232,
            mapper41,
        }
    }
}
