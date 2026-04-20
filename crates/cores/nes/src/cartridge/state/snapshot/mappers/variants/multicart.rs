use crate::cartridge::state::types::*;
use crate::cartridge::Cartridge;

pub(super) struct MulticartVariantSnapshotStates {
    pub(super) mapper233: Option<Mapper233State>,
    pub(super) mapper234: Option<Mapper234State>,
    pub(super) mapper235: Option<Mapper235State>,
    pub(super) mapper202: Option<Mapper202State>,
    pub(super) mapper212: Option<Mapper212State>,
    pub(super) mapper226: Option<Mapper226State>,
    pub(super) mapper230: Option<Mapper230State>,
    pub(super) mapper228: Option<Mapper228State>,
    pub(super) mapper242: Option<Mapper242State>,
    pub(super) mapper243: Option<Mapper243State>,
    pub(super) mapper221: Option<Mapper221State>,
    pub(super) mapper236: Option<Mapper236State>,
    pub(super) mapper227: Option<Mapper227State>,
}

impl Cartridge {
    pub(super) fn snapshot_multicart_variant_states(&self) -> MulticartVariantSnapshotStates {
        let mapper233 = if self.mapper == 233 {
            Some(Mapper233State {
                nrom128: self.mappers.multicart.mapper233_nrom128,
            })
        } else {
            None
        };
        let mapper234 = if self.mapper == 234 {
            Some(Mapper234State {
                reg0: self.mappers.multicart.mapper234_reg0,
                reg1: self.mappers.multicart.mapper234_reg1,
            })
        } else {
            None
        };
        let mapper235 = if self.mapper == 235 {
            Some(Mapper235State {
                nrom128: self.mappers.multicart.mapper235_nrom128,
            })
        } else {
            None
        };
        let mapper202 = if self.mapper == 202 {
            Some(Mapper202State {
                mode_32k: self.mappers.multicart.mapper202_32k_mode,
            })
        } else {
            None
        };
        let mapper212 = if self.mapper == 212 {
            Some(Mapper212State {
                mode_32k: self.mappers.multicart.mapper212_32k_mode,
            })
        } else {
            None
        };
        let mapper226 = if self.mapper == 226 {
            Some(Mapper226State {
                nrom128: self.mappers.multicart.mapper226_nrom128,
            })
        } else {
            None
        };
        let mapper230 = if self.mapper == 230 {
            Some(Mapper230State {
                contra_mode: self.mappers.multicart.mapper230_contra_mode,
                nrom128: self.mappers.multicart.mapper230_nrom128,
            })
        } else {
            None
        };
        let mapper228 = if self.mapper == 228 {
            Some(Mapper228State {
                chip_select: self.mappers.multicart.mapper228_chip_select,
                nrom128: self.mappers.multicart.mapper228_nrom128,
            })
        } else {
            None
        };
        let mapper242 = if self.mapper == 242 {
            Some(Mapper242State {
                latch: self.mappers.multicart.mapper242_latch,
            })
        } else {
            None
        };
        let mapper243 = if self.mapper == 243 {
            Some(Mapper243State {
                index: self.mappers.multicart.mapper243_index,
                registers: self.mappers.multicart.mapper243_registers,
            })
        } else {
            None
        };
        let mapper221 = if self.mapper == 221 {
            Some(Mapper221State {
                mode: self.mappers.multicart.mapper221_mode,
                outer_bank: self.mappers.multicart.mapper221_outer_bank,
                chr_write_protect: self.mappers.multicart.mapper221_chr_write_protect,
            })
        } else {
            None
        };
        let mapper236 = if self.mapper == 236 {
            Some(Mapper236State {
                mode: self.mappers.multicart.mapper236_mode,
                outer_bank: self.mappers.multicart.mapper236_outer_bank,
            })
        } else {
            None
        };
        let mapper227 = if self.mapper == 227 {
            Some(Mapper227State {
                latch: self.mappers.multicart.mapper227_latch,
            })
        } else {
            None
        };

        MulticartVariantSnapshotStates {
            mapper233,
            mapper234,
            mapper235,
            mapper202,
            mapper212,
            mapper226,
            mapper230,
            mapper228,
            mapper242,
            mapper243,
            mapper221,
            mapper236,
            mapper227,
        }
    }
}
