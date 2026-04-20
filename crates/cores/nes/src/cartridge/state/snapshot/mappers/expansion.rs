use crate::cartridge::state::conversion::snapshot_vrc6_state;
use crate::cartridge::state::types::*;
use crate::cartridge::Cartridge;

pub(super) struct ExpansionSnapshotStates {
    pub(super) mapper246: Option<Mapper246State>,
    pub(super) sunsoft3: Option<Sunsoft3State>,
    pub(super) sunsoft4: Option<Sunsoft4State>,
    pub(super) taito_tc0190: Option<TaitoTc0190State>,
    pub(super) taito_x1005: Option<TaitoX1005State>,
    pub(super) taito_x1017: Option<TaitoX1017State>,
    pub(super) vrc6: Option<Vrc6State>,
}

impl Cartridge {
    pub(super) fn snapshot_expansion_mapper_states(&self) -> ExpansionSnapshotStates {
        let mapper246 = self.mappers.mapper246.as_ref().map(|m| Mapper246State {
            prg_banks: m.prg_banks,
            chr_banks: m.chr_banks,
        });
        let sunsoft3 = self.mappers.sunsoft3.as_ref().map(|m| Sunsoft3State {
            chr_banks: m.chr_banks,
            prg_bank: m.prg_bank,
            irq_counter: m.irq_counter,
            irq_enabled: m.irq_enabled,
            irq_pending: m.irq_pending.get(),
            irq_write_high: m.irq_write_high,
        });
        let sunsoft4 = self.mappers.sunsoft4.as_ref().map(|m| Sunsoft4State {
            chr_banks: m.chr_banks,
            nametable_banks: m.nametable_banks,
            control: m.control,
            prg_bank: m.prg_bank,
            prg_ram_enabled: m.prg_ram_enabled,
        });
        let taito_tc0190 = self
            .mappers
            .taito_tc0190
            .as_ref()
            .map(|m| TaitoTc0190State {
                prg_banks: m.prg_banks,
                chr_banks: m.chr_banks,
                irq_latch: m.irq_latch,
                irq_counter: m.irq_counter,
                irq_reload: m.irq_reload,
                irq_enabled: m.irq_enabled,
                irq_pending: m.irq_pending.get(),
                irq_delay: m.irq_delay,
            });
        let taito_x1005 = self.mappers.taito_x1005.as_ref().map(|m| TaitoX1005State {
            prg_banks: m.prg_banks,
            chr_banks: m.chr_banks,
            ram_enabled: m.ram_enabled,
        });
        let taito_x1017 = self.mappers.taito_x1017.as_ref().map(|m| TaitoX1017State {
            prg_banks: m.prg_banks,
            chr_banks: m.chr_banks,
            ram_enabled: m.ram_enabled,
            chr_invert: m.chr_invert,
        });
        let vrc6 = self.mappers.vrc6.as_ref().map(snapshot_vrc6_state);

        ExpansionSnapshotStates {
            mapper246,
            sunsoft3,
            sunsoft4,
            taito_tc0190,
            taito_x1005,
            taito_x1017,
            vrc6,
        }
    }
}
