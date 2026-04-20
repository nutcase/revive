use crate::cartridge::state::types::*;
use crate::cartridge::Cartridge;

pub(super) struct SimpleSnapshotStates {
    pub(super) mapper34: Option<Mapper34State>,
    pub(super) mapper93: Option<Mapper93State>,
    pub(super) mapper184: Option<Mapper184State>,
    pub(super) vrc1: Option<Vrc1State>,
    pub(super) vrc2_vrc4: Option<Vrc2Vrc4State>,
    pub(super) mapper15: Option<Mapper15State>,
    pub(super) mapper72: Option<Mapper72State>,
}

impl Cartridge {
    pub(super) fn snapshot_simple_mapper_states(&self) -> SimpleSnapshotStates {
        let mapper34 = if self.mapper == 34 {
            Some(Mapper34State {
                nina001: self.mappers.simple.mapper34_nina001,
                chr_bank_1: self.chr_bank_1,
            })
        } else {
            None
        };

        let mapper93 = if self.mapper == 93 {
            Some(Mapper93State {
                chr_ram_enabled: self.mappers.simple.mapper93_chr_ram_enabled,
            })
        } else {
            None
        };

        let mapper184 = if self.mapper == 184 {
            Some(Mapper184State {
                chr_bank_1: self.chr_bank_1,
            })
        } else {
            None
        };

        let vrc1 = self.mappers.vrc1.as_ref().map(|v| Vrc1State {
            prg_banks: v.prg_banks,
            chr_bank_0: v.chr_bank_0,
            chr_bank_1: v.chr_bank_1,
        });
        let vrc2_vrc4 = self.mappers.vrc2_vrc4.as_ref().map(|v| Vrc2Vrc4State {
            prg_banks: v.prg_banks,
            chr_banks: v.chr_banks,
            wram_enabled: v.wram_enabled,
            prg_swap_mode: v.prg_swap_mode,
            vrc4_mode: v.vrc4_mode,
            latch: v.latch,
            irq_latch: v.irq_latch,
            irq_counter: v.irq_counter,
            irq_enable_after_ack: v.irq_enable_after_ack,
            irq_enabled: v.irq_enabled,
            irq_cycle_mode: v.irq_cycle_mode,
            irq_prescaler: v.irq_prescaler,
            irq_pending: v.irq_pending.get(),
        });

        let mapper15 = self.mappers.mapper15.as_ref().map(|m| Mapper15State {
            mode: m.mode,
            data: m.data,
        });
        let mapper72 = if matches!(self.mapper, 72 | 92) {
            Some(Mapper72State {
                last_command: self.chr_bank_1,
            })
        } else {
            None
        };

        SimpleSnapshotStates {
            mapper34,
            mapper93,
            mapper184,
            vrc1,
            vrc2_vrc4,
            mapper15,
            mapper72,
        }
    }
}
