use super::super::super::Cartridge;
use super::super::types::{
    Mmc1State, Mmc2State, Mmc3State, Mmc5AudioState, Mmc5PulseState, Mmc5State,
};

impl Cartridge {
    pub(super) fn snapshot_mmc1_state(&self) -> Option<Mmc1State> {
        self.mappers.mmc1.as_ref().map(|m| Mmc1State {
            shift_register: m.shift_register,
            shift_count: m.shift_count,
            control: m.control,
            chr_bank_0: m.chr_bank_0,
            chr_bank_1: m.chr_bank_1,
            prg_bank: m.prg_bank,
            prg_ram_disable: m.prg_ram_disable,
        })
    }

    pub(super) fn snapshot_mmc2_state(&self) -> Option<Mmc2State> {
        self.mappers.mmc2.as_ref().map(|m| Mmc2State {
            prg_bank: m.prg_bank,
            chr_bank_0_fd: m.chr_bank_0_fd,
            chr_bank_0_fe: m.chr_bank_0_fe,
            chr_bank_1_fd: m.chr_bank_1_fd,
            chr_bank_1_fe: m.chr_bank_1_fe,
            latch_0: m.latch_0.get(),
            latch_1: m.latch_1.get(),
        })
    }

    pub(super) fn snapshot_mmc3_state(&self) -> Option<Mmc3State> {
        self.mappers.mmc3.as_ref().map(|m| Mmc3State {
            bank_select: m.bank_select,
            bank_registers: m.bank_registers,
            extra_bank_registers: m.extra_bank_registers,
            irq_latch: m.irq_latch,
            irq_counter: m.irq_counter,
            irq_reload: m.irq_reload,
            irq_enabled: m.irq_enabled,
            irq_pending: m.irq_pending.get(),
            prg_ram_enabled: m.prg_ram_enabled,
            prg_ram_write_protect: m.prg_ram_write_protect,
            irq_cycle_mode: m.irq_cycle_mode,
            irq_prescaler: m.irq_prescaler,
            irq_delay: m.irq_delay,
        })
    }

    pub(super) fn snapshot_mmc5_state(&self) -> Option<Mmc5State> {
        self.mappers.mmc5.as_ref().map(|m| Mmc5State {
            prg_mode: m.prg_mode,
            chr_mode: m.chr_mode,
            exram_mode: m.exram_mode,
            prg_ram_protect_1: m.prg_ram_protect_1,
            prg_ram_protect_2: m.prg_ram_protect_2,
            nametable_map: m.nametable_map,
            fill_tile: m.fill_tile,
            fill_attr: m.fill_attr,
            prg_ram_bank: m.prg_ram_bank,
            prg_banks: m.prg_banks,
            chr_upper: m.chr_upper,
            sprite_chr_banks: m.sprite_chr_banks,
            bg_chr_banks: m.bg_chr_banks,
            exram: m.exram.clone(),
            irq_scanline_compare: m.irq_scanline_compare,
            irq_enabled: m.irq_enabled,
            irq_pending: m.irq_pending.get(),
            in_frame: m.in_frame.get(),
            scanline_counter: m.scanline_counter.get(),
            multiplier_a: m.multiplier_a,
            multiplier_b: m.multiplier_b,
            split_control: m.split_control,
            split_scroll: m.split_scroll,
            split_bank: m.split_bank,
            ppu_ctrl: m.ppu_ctrl.get(),
            ppu_mask: m.ppu_mask.get(),
            cached_tile_x: m.cached_tile_x.get(),
            cached_tile_y: m.cached_tile_y.get(),
            cached_ext_palette: m.cached_ext_palette.get(),
            cached_ext_bank: m.cached_ext_bank.get(),
            ppu_data_uses_bg_banks: m.ppu_data_uses_bg_banks,
            audio: Mmc5AudioState {
                pulse1: Mmc5PulseState {
                    duty: m.pulse1.duty,
                    length_counter: m.pulse1.length_counter,
                    envelope_divider: m.pulse1.envelope_divider,
                    envelope_decay: m.pulse1.envelope_decay,
                    envelope_disable: m.pulse1.envelope_disable,
                    envelope_start: m.pulse1.envelope_start,
                    volume: m.pulse1.volume,
                    timer: m.pulse1.timer,
                    timer_reload: m.pulse1.timer_reload,
                    duty_counter: m.pulse1.duty_counter,
                    length_enabled: m.pulse1.length_enabled,
                },
                pulse2: Mmc5PulseState {
                    duty: m.pulse2.duty,
                    length_counter: m.pulse2.length_counter,
                    envelope_divider: m.pulse2.envelope_divider,
                    envelope_decay: m.pulse2.envelope_decay,
                    envelope_disable: m.pulse2.envelope_disable,
                    envelope_start: m.pulse2.envelope_start,
                    volume: m.pulse2.volume,
                    timer: m.pulse2.timer,
                    timer_reload: m.pulse2.timer_reload,
                    duty_counter: m.pulse2.duty_counter,
                    length_enabled: m.pulse2.length_enabled,
                },
                pulse1_enabled: m.pulse1_enabled,
                pulse2_enabled: m.pulse2_enabled,
                pcm_irq_enabled: m.pcm_irq_enabled,
                pcm_read_mode: m.pcm_read_mode,
                pcm_irq_pending: m.pcm_irq_pending.get(),
                pcm_dac: m.pcm_dac,
                audio_frame_accum: m.audio_frame_accum,
                audio_even_cycle: m.audio_even_cycle,
            },
        })
    }
}
