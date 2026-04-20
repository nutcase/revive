use crate::cartridge::state::conversion::restore_vrc6_state;
use crate::cartridge::state::types::*;
use crate::cartridge::{BandaiFcg, Cartridge, Fme7, Namco163, Namco210, Vrc1, Vrc2Vrc4};

impl Cartridge {
    pub(super) fn restore_chip_mapper_states(&mut self, state: &CartridgeState) {
        if let (Some(ref mut namco163), Some(saved)) =
            (self.mappers.namco163.as_mut(), state.namco163.as_ref())
        {
            restore_namco163_state(namco163, saved);
        }
        if let (Some(ref mut mapper18), Some(saved)) = (
            self.mappers.jaleco_ss88006.as_mut(),
            state.mapper18.as_ref(),
        ) {
            mapper18.prg_banks = saved.prg_banks;
            mapper18.chr_banks = saved.chr_banks;
            mapper18.prg_ram_enabled = saved.prg_ram_enabled;
            mapper18.prg_ram_write_enabled = saved.prg_ram_write_enabled;
            mapper18.irq_reload = saved.irq_reload;
            mapper18.irq_counter = saved.irq_counter;
            mapper18.irq_control = saved.irq_control;
            mapper18.irq_pending.set(saved.irq_pending);
            self.prg_bank = saved.prg_banks[0];
            self.chr_bank = saved.chr_banks[0];
        }
        if let (Some(ref mut mapper210), Some(saved)) =
            (self.mappers.namco210.as_mut(), state.mapper210.as_ref())
        {
            restore_mapper210_state(mapper210, saved);
            self.prg_bank = saved.prg_banks[0];
            self.chr_bank = saved.chr_banks[0];
        }

        if let (Some(ref mut fme7), Some(saved)) = (self.mappers.fme7.as_mut(), state.fme7.as_ref())
        {
            restore_fme7_state(fme7, saved);
        }

        if let (Some(ref mut bandai), Some(saved)) =
            (self.mappers.bandai_fcg.as_mut(), state.bandai_fcg.as_ref())
        {
            restore_bandai_fcg_state(bandai, saved);
        }

        if let (Some(ref mut vrc1), Some(saved)) = (self.mappers.vrc1.as_mut(), state.vrc1.as_ref())
        {
            restore_vrc1_state(vrc1, saved);
        }
        if let (Some(ref mut vrc2_vrc4), Some(saved)) =
            (self.mappers.vrc2_vrc4.as_mut(), state.vrc2_vrc4.as_ref())
        {
            restore_vrc2_vrc4_state(vrc2_vrc4, saved);
            self.prg_bank = saved.prg_banks[0];
            self.chr_bank = saved.chr_banks[0] as u8;
        }
        if let (Some(ref mut vrc6), Some(saved)) = (self.mappers.vrc6.as_mut(), state.vrc6.as_ref())
        {
            restore_vrc6_state(vrc6, saved);
            self.prg_bank = saved.prg_bank_16k;
            self.chr_bank = saved.chr_banks[0];
            self.vrc6_apply_banking_control(saved.banking_control);
        }
    }
}

fn restore_namco163_state(namco163: &mut Namco163, saved: &Namco163State) {
    namco163.chr_banks = saved.chr_banks;
    namco163.prg_banks = saved.prg_banks;
    namco163.sound_disable = saved.sound_disable;
    namco163.chr_nt_disabled_low = saved.chr_nt_disabled_low;
    namco163.chr_nt_disabled_high = saved.chr_nt_disabled_high;
    namco163.wram_write_enable = saved.wram_write_enable;
    namco163.wram_write_protect = saved.wram_write_protect;
    namco163.internal_addr.set(saved.internal_addr);
    namco163.internal_auto_increment = saved.internal_auto_increment;
    namco163.irq_counter = saved.irq_counter;
    namco163.irq_enabled = saved.irq_enabled;
    namco163.irq_pending.set(saved.irq_pending);
    namco163.audio_delay = saved.audio_delay;
    namco163.audio_channel_index = saved.audio_channel_index;
    namco163.audio_outputs = saved.audio_outputs;
    namco163.audio_current = saved.audio_current;
}

fn restore_mapper210_state(mapper210: &mut Namco210, saved: &Mapper210State) {
    mapper210.chr_banks = saved.chr_banks;
    mapper210.prg_banks = saved.prg_banks;
    mapper210.namco340 = saved.namco340;
    mapper210.prg_ram_enabled = saved.prg_ram_enabled;
}

fn restore_fme7_state(fme7: &mut Fme7, saved: &Fme7State) {
    fme7.command = saved.command;
    fme7.chr_banks = saved.chr_banks;
    fme7.prg_banks = saved.prg_banks;
    fme7.prg_bank_6000 = saved.prg_bank_6000;
    fme7.prg_ram_enabled = saved.prg_ram_enabled;
    fme7.prg_ram_select = saved.prg_ram_select;
    fme7.irq_counter = saved.irq_counter;
    fme7.irq_counter_enabled = saved.irq_counter_enabled;
    fme7.irq_enabled = saved.irq_enabled;
    fme7.irq_pending.set(saved.irq_pending);
}

fn restore_bandai_fcg_state(bandai: &mut BandaiFcg, saved: &BandaiFcgState) {
    bandai.chr_banks = saved.chr_banks;
    bandai.prg_bank = saved.prg_bank;
    bandai.irq_counter = saved.irq_counter;
    bandai.irq_latch = saved.irq_latch;
    bandai.irq_enabled = saved.irq_enabled;
    bandai.irq_pending.set(saved.irq_pending);
    bandai.outer_prg_bank = saved.outer_prg_bank;
    bandai.prg_ram_enabled = saved.prg_ram_enabled;
}

fn restore_vrc1_state(vrc1: &mut Vrc1, saved: &Vrc1State) {
    vrc1.prg_banks = saved.prg_banks;
    vrc1.chr_bank_0 = saved.chr_bank_0;
    vrc1.chr_bank_1 = saved.chr_bank_1;
}

fn restore_vrc2_vrc4_state(vrc2_vrc4: &mut Vrc2Vrc4, saved: &Vrc2Vrc4State) {
    vrc2_vrc4.prg_banks = saved.prg_banks;
    vrc2_vrc4.chr_banks = saved.chr_banks;
    vrc2_vrc4.wram_enabled = saved.wram_enabled;
    vrc2_vrc4.prg_swap_mode = saved.prg_swap_mode;
    vrc2_vrc4.vrc4_mode = saved.vrc4_mode;
    vrc2_vrc4.latch = saved.latch;
    vrc2_vrc4.irq_latch = saved.irq_latch;
    vrc2_vrc4.irq_counter = saved.irq_counter;
    vrc2_vrc4.irq_enable_after_ack = saved.irq_enable_after_ack;
    vrc2_vrc4.irq_enabled = saved.irq_enabled;
    vrc2_vrc4.irq_cycle_mode = saved.irq_cycle_mode;
    vrc2_vrc4.irq_prescaler = saved.irq_prescaler;
    vrc2_vrc4.irq_pending.set(saved.irq_pending);
}
