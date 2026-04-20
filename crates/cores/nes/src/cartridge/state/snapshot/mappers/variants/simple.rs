use crate::cartridge::state::types::*;
use crate::cartridge::Cartridge;

pub(super) struct SimpleVariantSnapshotStates {
    pub(super) mapper103: Option<Mapper103State>,
    pub(super) mapper185: Option<Mapper185State>,
}

impl Cartridge {
    pub(super) fn snapshot_simple_variant_states(&self) -> SimpleVariantSnapshotStates {
        let mapper103 = if self.mapper == 103 {
            Some(Mapper103State {
                prg_ram_disabled: self.mappers.simple.mapper103_prg_ram_disabled,
            })
        } else {
            None
        };
        let mapper185 = if self.mapper == 185 {
            Some(Mapper185State {
                disabled_reads: self.mappers.simple.mapper185_disabled_reads.get(),
            })
        } else {
            None
        };

        SimpleVariantSnapshotStates {
            mapper103,
            mapper185,
        }
    }
}
