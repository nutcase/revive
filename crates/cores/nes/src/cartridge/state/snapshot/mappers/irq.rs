use crate::cartridge::state::types::*;
use crate::cartridge::Cartridge;

pub(super) struct IrqSnapshotStates {
    pub(super) mapper40: Option<Mapper40State>,
    pub(super) mapper42: Option<Mapper42State>,
    pub(super) mapper43: Option<Mapper43State>,
    pub(super) mapper50: Option<Mapper50State>,
    pub(super) irem_g101: Option<IremG101State>,
    pub(super) irem_h3001: Option<IremH3001State>,
    pub(super) vrc3: Option<Vrc3State>,
}

impl Cartridge {
    pub(super) fn snapshot_irq_mapper_states(&self) -> IrqSnapshotStates {
        let mapper40 = self.mappers.mapper40.as_ref().map(|m| Mapper40State {
            irq_counter: m.irq_counter,
            irq_enabled: m.irq_enabled,
            irq_pending: m.irq_pending.get(),
        });
        let mapper42 = self.mappers.mapper42.as_ref().map(|m| Mapper42State {
            irq_counter: m.irq_counter,
            irq_enabled: m.irq_enabled,
            irq_pending: m.irq_pending.get(),
        });
        let mapper43 = self.mappers.mapper43.as_ref().map(|m| Mapper43State {
            irq_counter: m.irq_counter,
            irq_enabled: m.irq_enabled,
            irq_pending: m.irq_pending.get(),
        });
        let mapper50 = self.mappers.mapper50.as_ref().map(|m| Mapper50State {
            irq_counter: m.irq_counter,
            irq_enabled: m.irq_enabled,
            irq_pending: m.irq_pending.get(),
        });
        let irem_g101 = self.mappers.irem_g101.as_ref().map(|g| IremG101State {
            prg_banks: g.prg_banks,
            chr_banks: g.chr_banks,
            prg_mode: g.prg_mode,
        });
        let irem_h3001 = self.mappers.irem_h3001.as_ref().map(|h| IremH3001State {
            prg_banks: h.prg_banks,
            chr_banks: h.chr_banks,
            prg_mode: h.prg_mode,
            irq_reload: h.irq_reload,
            irq_counter: h.irq_counter,
            irq_enabled: h.irq_enabled,
            irq_pending: h.irq_pending.get(),
        });
        let vrc3 = self.mappers.vrc3.as_ref().map(|v| Vrc3State {
            irq_reload: v.irq_reload,
            irq_counter: v.irq_counter,
            irq_enable_on_ack: v.irq_enable_on_ack,
            irq_enabled: v.irq_enabled,
            irq_mode_8bit: v.irq_mode_8bit,
            irq_pending: v.irq_pending.get(),
        });

        IrqSnapshotStates {
            mapper40,
            mapper42,
            mapper43,
            mapper50,
            irem_g101,
            irem_h3001,
            vrc3,
        }
    }
}
