use crate::cartridge::state::types::CartridgeState;
use crate::cartridge::{Cartridge, Mirroring, Sunsoft4};

impl Cartridge {
    pub(super) fn restore_expansion_mapper_states(&mut self, state: &CartridgeState) {
        if let (Some(ref mut sunsoft4), Some(saved)) =
            (self.mappers.sunsoft4.as_mut(), state.sunsoft4.as_ref())
        {
            sunsoft4.chr_banks = saved.chr_banks;
            sunsoft4.nametable_banks = saved.nametable_banks;
            sunsoft4.control = saved.control;
            sunsoft4.prg_bank = saved.prg_bank;
            sunsoft4.prg_ram_enabled = saved.prg_ram_enabled;
            sunsoft4.nametable_chr_rom = saved.control & 0x10 != 0;
            self.prg_bank = saved.prg_bank;
            self.chr_bank = saved.chr_banks[0];
            self.mirroring = Sunsoft4::decode_mirroring(saved.control);
        }
        if let (Some(ref mut sunsoft3), Some(saved)) =
            (self.mappers.sunsoft3.as_mut(), state.sunsoft3.as_ref())
        {
            sunsoft3.chr_banks = saved.chr_banks;
            sunsoft3.prg_bank = saved.prg_bank;
            sunsoft3.irq_counter = saved.irq_counter;
            sunsoft3.irq_enabled = saved.irq_enabled;
            sunsoft3.irq_pending.set(saved.irq_pending);
            sunsoft3.irq_write_high = saved.irq_write_high;
            self.prg_bank = saved.prg_bank;
            self.chr_bank = saved.chr_banks[0];
        }

        if let (Some(ref mut taito_tc0190), Some(saved)) = (
            self.mappers.taito_tc0190.as_mut(),
            state.taito_tc0190.as_ref(),
        ) {
            taito_tc0190.prg_banks = saved.prg_banks;
            taito_tc0190.chr_banks = saved.chr_banks;
            taito_tc0190.irq_latch = saved.irq_latch;
            taito_tc0190.irq_counter = saved.irq_counter;
            taito_tc0190.irq_reload = saved.irq_reload;
            taito_tc0190.irq_enabled = saved.irq_enabled;
            taito_tc0190.irq_pending.set(saved.irq_pending);
            taito_tc0190.irq_delay = saved.irq_delay;
            self.prg_bank = saved.prg_banks[0];
            self.chr_bank = saved.chr_banks[0];
        }

        if let (Some(ref mut taito_x1005), Some(saved)) = (
            self.mappers.taito_x1005.as_mut(),
            state.taito_x1005.as_ref(),
        ) {
            taito_x1005.prg_banks = saved.prg_banks;
            taito_x1005.chr_banks = saved.chr_banks;
            taito_x1005.ram_enabled = saved.ram_enabled;
            self.prg_bank = saved.prg_banks[0];
            self.chr_bank = saved.chr_banks[0];
            if self.mapper == 207 {
                let top = (saved.chr_banks[0] >> 7) & 1;
                let bottom = (saved.chr_banks[1] >> 7) & 1;
                self.mirroring = match (top, bottom) {
                    (0, 0) => Mirroring::OneScreenLower,
                    (1, 1) => Mirroring::OneScreenUpper,
                    (0, 1) => Mirroring::Horizontal,
                    (1, 0) => Mirroring::HorizontalSwapped,
                    _ => Mirroring::Horizontal,
                };
            }
        }

        if let (Some(ref mut taito_x1017), Some(saved)) = (
            self.mappers.taito_x1017.as_mut(),
            state.taito_x1017.as_ref(),
        ) {
            taito_x1017.prg_banks = saved.prg_banks;
            taito_x1017.chr_banks = saved.chr_banks;
            taito_x1017.ram_enabled = saved.ram_enabled;
            taito_x1017.chr_invert = saved.chr_invert;
            self.prg_bank = saved.prg_banks[0];
            self.chr_bank = saved.chr_banks[0];
        }
    }
}
