use crate::cartridge::state::types::*;
use crate::cartridge::Cartridge;

pub(super) struct Mmc3VariantSnapshotStates {
    pub(super) mapper37: Option<Mapper37State>,
    pub(super) mapper44: Option<Mapper44State>,
    pub(super) mapper12: Option<Mapper12State>,
    pub(super) mapper114: Option<Mapper114State>,
    pub(super) mapper47: Option<Mapper47State>,
    pub(super) mapper123: Option<Mapper123State>,
    pub(super) mapper115: Option<Mapper115State>,
    pub(super) mapper205: Option<Mapper205State>,
    pub(super) mapper191: Option<Mapper191State>,
    pub(super) mapper195: Option<Mapper195State>,
    pub(super) mapper208: Option<Mapper208State>,
    pub(super) mapper189: Option<Mapper189State>,
}

impl Cartridge {
    pub(super) fn snapshot_mmc3_variant_states(&self) -> Mmc3VariantSnapshotStates {
        let mapper37 = if self.mapper == 37 {
            Some(Mapper37State {
                outer_bank: self.mappers.mmc3_variant.mapper37_outer_bank,
            })
        } else {
            None
        };
        let mapper44 = if self.mapper == 44 {
            Some(Mapper44State {
                outer_bank: self.mappers.mmc3_variant.mapper44_outer_bank,
            })
        } else {
            None
        };
        let mapper12 = if self.mapper == 12 {
            Some(Mapper12State {
                chr_outer: self.mappers.mmc3_variant.mapper12_chr_outer,
            })
        } else {
            None
        };
        let mapper114 = if matches!(self.mapper, 114 | 182) {
            Some(Mapper114State {
                nrom_override: self.mappers.mmc3_variant.mapper114_override,
                chr_outer_bank: self.mappers.mmc3_variant.mapper114_chr_outer_bank,
            })
        } else {
            None
        };
        let mapper47 = if self.mapper == 47 {
            Some(Mapper47State {
                outer_bank: self.mappers.mmc3_variant.mapper47_outer_bank,
            })
        } else {
            None
        };
        let mapper123 = if self.mapper == 123 {
            Some(Mapper123State {
                nrom_override: self.mappers.mmc3_variant.mapper123_override,
            })
        } else {
            None
        };
        let mapper115 = if matches!(self.mapper, 115 | 248) {
            Some(Mapper115State {
                nrom_override: self.mappers.mmc3_variant.mapper115_override,
                chr_outer_bank: self.mappers.mmc3_variant.mapper115_chr_outer_bank,
            })
        } else {
            None
        };
        let mapper205 = if self.mapper == 205 {
            Some(Mapper205State {
                block: self.mappers.mmc3_variant.mapper205_block,
            })
        } else {
            None
        };
        let mapper191 = if self.mapper == 191 {
            Some(Mapper191State {
                outer_bank: self.mappers.mmc3_variant.mapper191_outer_bank,
            })
        } else {
            None
        };
        let mapper195 = if self.mapper == 195 {
            Some(Mapper195State {
                mode: self.mappers.mmc3_variant.mapper195_mode,
            })
        } else {
            None
        };
        let mapper208 = if self.mapper == 208 {
            Some(Mapper208State {
                protection_index: self.mappers.mmc3_variant.mapper208_protection_index,
                protection_regs: self.mappers.mmc3_variant.mapper208_protection_regs,
            })
        } else {
            None
        };
        let mapper189 = if self.mapper == 189 {
            Some(Mapper189State {
                prg_bank: self.mappers.mmc3_variant.mapper189_prg_bank,
            })
        } else {
            None
        };

        Mmc3VariantSnapshotStates {
            mapper37,
            mapper44,
            mapper12,
            mapper114,
            mapper47,
            mapper123,
            mapper115,
            mapper205,
            mapper191,
            mapper195,
            mapper208,
            mapper189,
        }
    }
}
