use crate::cartridge::state::types::*;
use crate::cartridge::{BandaiFcg, Cartridge, Fme7, JalecoSs88006, Namco163, Namco210};

pub(super) struct ChipSnapshotStates {
    pub(super) namco163: Option<Namco163State>,
    pub(super) mapper18: Option<Mapper18State>,
    pub(super) mapper210: Option<Mapper210State>,
    pub(super) fme7: Option<Fme7State>,
    pub(super) bandai_fcg: Option<BandaiFcgState>,
}

impl Cartridge {
    pub(super) fn snapshot_chip_states(&self) -> ChipSnapshotStates {
        ChipSnapshotStates {
            namco163: self.mappers.namco163.as_ref().map(snapshot_namco163_state),
            mapper18: self
                .mappers
                .jaleco_ss88006
                .as_ref()
                .map(snapshot_mapper18_state),
            mapper210: self.mappers.namco210.as_ref().map(snapshot_mapper210_state),
            fme7: self.mappers.fme7.as_ref().map(snapshot_fme7_state),
            bandai_fcg: self
                .mappers
                .bandai_fcg
                .as_ref()
                .map(snapshot_bandai_fcg_state),
        }
    }
}

fn snapshot_namco163_state(n: &Namco163) -> Namco163State {
    Namco163State {
        chr_banks: n.chr_banks,
        prg_banks: n.prg_banks,
        sound_disable: n.sound_disable,
        chr_nt_disabled_low: n.chr_nt_disabled_low,
        chr_nt_disabled_high: n.chr_nt_disabled_high,
        wram_write_enable: n.wram_write_enable,
        wram_write_protect: n.wram_write_protect,
        internal_addr: n.internal_addr.get(),
        internal_auto_increment: n.internal_auto_increment,
        irq_counter: n.irq_counter,
        irq_enabled: n.irq_enabled,
        irq_pending: n.irq_pending.get(),
        audio_delay: n.audio_delay,
        audio_channel_index: n.audio_channel_index,
        audio_outputs: n.audio_outputs,
        audio_current: n.audio_current,
    }
}

fn snapshot_mapper18_state(m: &JalecoSs88006) -> Mapper18State {
    Mapper18State {
        prg_banks: m.prg_banks,
        chr_banks: m.chr_banks,
        prg_ram_enabled: m.prg_ram_enabled,
        prg_ram_write_enabled: m.prg_ram_write_enabled,
        irq_reload: m.irq_reload,
        irq_counter: m.irq_counter,
        irq_control: m.irq_control,
        irq_pending: m.irq_pending.get(),
    }
}

fn snapshot_mapper210_state(m: &Namco210) -> Mapper210State {
    Mapper210State {
        chr_banks: m.chr_banks,
        prg_banks: m.prg_banks,
        namco340: m.namco340,
        prg_ram_enabled: m.prg_ram_enabled,
    }
}

fn snapshot_fme7_state(f: &Fme7) -> Fme7State {
    Fme7State {
        command: f.command,
        chr_banks: f.chr_banks,
        prg_banks: f.prg_banks,
        prg_bank_6000: f.prg_bank_6000,
        prg_ram_enabled: f.prg_ram_enabled,
        prg_ram_select: f.prg_ram_select,
        irq_counter: f.irq_counter,
        irq_counter_enabled: f.irq_counter_enabled,
        irq_enabled: f.irq_enabled,
        irq_pending: f.irq_pending.get(),
    }
}

fn snapshot_bandai_fcg_state(b: &BandaiFcg) -> BandaiFcgState {
    BandaiFcgState {
        chr_banks: b.chr_banks,
        prg_bank: b.prg_bank,
        irq_counter: b.irq_counter,
        irq_latch: b.irq_latch,
        irq_enabled: b.irq_enabled,
        irq_pending: b.irq_pending.get(),
        outer_prg_bank: b.outer_prg_bank,
        prg_ram_enabled: b.prg_ram_enabled,
    }
}
