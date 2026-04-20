use crate::cartridge::state::types::CartridgeState;
use crate::cartridge::Cartridge;

impl Cartridge {
    pub(super) fn restore_irq_mapper_states(&mut self, state: &CartridgeState) {
        if let (Some(mapper246), Some(saved)) =
            (self.mappers.mapper246.as_mut(), state.mapper246.as_ref())
        {
            mapper246.prg_banks = saved.prg_banks;
            mapper246.chr_banks = saved.chr_banks;
        }
        if let (Some(ref mut mapper40), Some(saved)) =
            (self.mappers.mapper40.as_mut(), state.mapper40.as_ref())
        {
            mapper40.irq_counter = saved.irq_counter;
            mapper40.irq_enabled = saved.irq_enabled;
            mapper40.irq_pending.set(saved.irq_pending);
        }
        if let (Some(ref mut mapper42), Some(saved)) =
            (self.mappers.mapper42.as_mut(), state.mapper42.as_ref())
        {
            mapper42.irq_counter = saved.irq_counter;
            mapper42.irq_enabled = saved.irq_enabled;
            mapper42.irq_pending.set(saved.irq_pending);
        }
        if let (Some(ref mut mapper43), Some(saved)) =
            (self.mappers.mapper43.as_mut(), state.mapper43.as_ref())
        {
            mapper43.irq_counter = saved.irq_counter;
            mapper43.irq_enabled = saved.irq_enabled;
            mapper43.irq_pending.set(saved.irq_pending);
        }
        if let (Some(ref mut mapper50), Some(saved)) =
            (self.mappers.mapper50.as_mut(), state.mapper50.as_ref())
        {
            mapper50.irq_counter = saved.irq_counter;
            mapper50.irq_enabled = saved.irq_enabled;
            mapper50.irq_pending.set(saved.irq_pending);
        }
        if let (Some(ref mut g101), Some(saved)) =
            (self.mappers.irem_g101.as_mut(), state.irem_g101.as_ref())
        {
            g101.prg_banks = saved.prg_banks;
            g101.chr_banks = saved.chr_banks;
            g101.prg_mode = saved.prg_mode;
            self.prg_bank = saved.prg_banks[0];
            self.chr_bank = saved.chr_banks[0];
        }
        if let (Some(ref mut h3001), Some(saved)) =
            (self.mappers.irem_h3001.as_mut(), state.irem_h3001.as_ref())
        {
            h3001.prg_banks = saved.prg_banks;
            h3001.chr_banks = saved.chr_banks;
            h3001.prg_mode = saved.prg_mode;
            h3001.irq_reload = saved.irq_reload;
            h3001.irq_counter = saved.irq_counter;
            h3001.irq_enabled = saved.irq_enabled;
            h3001.irq_pending.set(saved.irq_pending);
            self.prg_bank = saved.prg_banks[0];
            self.chr_bank = saved.chr_banks[0];
        }
        if let (Some(ref mut vrc3), Some(saved)) = (self.mappers.vrc3.as_mut(), state.vrc3.as_ref())
        {
            vrc3.irq_reload = saved.irq_reload;
            vrc3.irq_counter = saved.irq_counter;
            vrc3.irq_enable_on_ack = saved.irq_enable_on_ack;
            vrc3.irq_enabled = saved.irq_enabled;
            vrc3.irq_mode_8bit = saved.irq_mode_8bit;
            vrc3.irq_pending.set(saved.irq_pending);
        }
    }
}
