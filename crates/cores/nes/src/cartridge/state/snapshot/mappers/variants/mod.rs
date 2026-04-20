mod mmc3;
mod multicart;
mod simple;

use crate::cartridge::state::types::*;
use crate::cartridge::Cartridge;

pub(super) struct VariantSnapshotStates {
    pub(super) mapper233: Option<Mapper233State>,
    pub(super) mapper234: Option<Mapper234State>,
    pub(super) mapper235: Option<Mapper235State>,
    pub(super) mapper202: Option<Mapper202State>,
    pub(super) mapper37: Option<Mapper37State>,
    pub(super) mapper44: Option<Mapper44State>,
    pub(super) mapper103: Option<Mapper103State>,
    pub(super) mapper12: Option<Mapper12State>,
    pub(super) mapper114: Option<Mapper114State>,
    pub(super) mapper212: Option<Mapper212State>,
    pub(super) mapper47: Option<Mapper47State>,
    pub(super) mapper123: Option<Mapper123State>,
    pub(super) mapper115: Option<Mapper115State>,
    pub(super) mapper205: Option<Mapper205State>,
    pub(super) mapper226: Option<Mapper226State>,
    pub(super) mapper230: Option<Mapper230State>,
    pub(super) mapper228: Option<Mapper228State>,
    pub(super) mapper242: Option<Mapper242State>,
    pub(super) mapper243: Option<Mapper243State>,
    pub(super) mapper221: Option<Mapper221State>,
    pub(super) mapper191: Option<Mapper191State>,
    pub(super) mapper195: Option<Mapper195State>,
    pub(super) mapper208: Option<Mapper208State>,
    pub(super) mapper189: Option<Mapper189State>,
    pub(super) mapper185: Option<Mapper185State>,
    pub(super) mapper236: Option<Mapper236State>,
    pub(super) mapper227: Option<Mapper227State>,
}

impl Cartridge {
    pub(super) fn snapshot_variant_mapper_states(&self) -> VariantSnapshotStates {
        let multicart = self.snapshot_multicart_variant_states();
        let mmc3 = self.snapshot_mmc3_variant_states();
        let simple = self.snapshot_simple_variant_states();

        VariantSnapshotStates {
            mapper233: multicart.mapper233,
            mapper234: multicart.mapper234,
            mapper235: multicart.mapper235,
            mapper202: multicart.mapper202,
            mapper37: mmc3.mapper37,
            mapper44: mmc3.mapper44,
            mapper103: simple.mapper103,
            mapper12: mmc3.mapper12,
            mapper114: mmc3.mapper114,
            mapper212: multicart.mapper212,
            mapper47: mmc3.mapper47,
            mapper123: mmc3.mapper123,
            mapper115: mmc3.mapper115,
            mapper205: mmc3.mapper205,
            mapper226: multicart.mapper226,
            mapper230: multicart.mapper230,
            mapper228: multicart.mapper228,
            mapper242: multicart.mapper242,
            mapper243: multicart.mapper243,
            mapper221: multicart.mapper221,
            mapper191: mmc3.mapper191,
            mapper195: mmc3.mapper195,
            mapper208: mmc3.mapper208,
            mapper189: mmc3.mapper189,
            mapper185: simple.mapper185,
            mapper236: multicart.mapper236,
            mapper227: multicart.mapper227,
        }
    }
}
