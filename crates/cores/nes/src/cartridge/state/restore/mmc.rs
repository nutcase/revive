use super::super::super::Cartridge;
use super::super::types::CartridgeState;

impl Cartridge {
    pub(super) fn restore_mmc_states(&mut self, state: &CartridgeState) {
        if let (Some(ref mut mmc1), Some(saved)) = (self.mappers.mmc1.as_mut(), state.mmc1.as_ref())
        {
            mmc1.shift_register = saved.shift_register;
            mmc1.shift_count = saved.shift_count;
            mmc1.control = saved.control;
            mmc1.chr_bank_0 = saved.chr_bank_0;
            mmc1.chr_bank_1 = saved.chr_bank_1;
            mmc1.prg_bank = saved.prg_bank;
            mmc1.prg_ram_disable = saved.prg_ram_disable;
        }

        if let (Some(ref mut mmc2), Some(saved)) = (self.mappers.mmc2.as_mut(), state.mmc2.as_ref())
        {
            mmc2.prg_bank = saved.prg_bank;
            mmc2.chr_bank_0_fd = saved.chr_bank_0_fd;
            mmc2.chr_bank_0_fe = saved.chr_bank_0_fe;
            mmc2.chr_bank_1_fd = saved.chr_bank_1_fd;
            mmc2.chr_bank_1_fe = saved.chr_bank_1_fe;
            mmc2.latch_0.set(saved.latch_0);
            mmc2.latch_1.set(saved.latch_1);
        }

        if let (Some(ref mut mmc3), Some(saved)) = (self.mappers.mmc3.as_mut(), state.mmc3.as_ref())
        {
            mmc3.bank_select = saved.bank_select;
            mmc3.bank_registers = saved.bank_registers;
            mmc3.extra_bank_registers = saved.extra_bank_registers;
            mmc3.irq_latch = saved.irq_latch;
            mmc3.irq_counter = saved.irq_counter;
            mmc3.irq_reload = saved.irq_reload;
            mmc3.irq_enabled = saved.irq_enabled;
            mmc3.irq_pending.set(saved.irq_pending);
            mmc3.prg_ram_enabled = saved.prg_ram_enabled;
            mmc3.prg_ram_write_protect = saved.prg_ram_write_protect;
            mmc3.irq_cycle_mode = saved.irq_cycle_mode;
            mmc3.irq_prescaler = saved.irq_prescaler;
            mmc3.irq_delay = saved.irq_delay;
        }

        if let (Some(ref mut mmc5), Some(saved)) = (self.mappers.mmc5.as_mut(), state.mmc5.as_ref())
        {
            mmc5.prg_mode = saved.prg_mode;
            mmc5.chr_mode = saved.chr_mode;
            mmc5.exram_mode = saved.exram_mode;
            mmc5.prg_ram_protect_1 = saved.prg_ram_protect_1;
            mmc5.prg_ram_protect_2 = saved.prg_ram_protect_2;
            mmc5.nametable_map = saved.nametable_map;
            mmc5.fill_tile = saved.fill_tile;
            mmc5.fill_attr = saved.fill_attr;
            mmc5.prg_ram_bank = saved.prg_ram_bank;
            mmc5.prg_banks = saved.prg_banks;
            mmc5.chr_upper = saved.chr_upper;
            mmc5.sprite_chr_banks = saved.sprite_chr_banks;
            mmc5.bg_chr_banks = saved.bg_chr_banks;
            mmc5.exram.clone_from(&saved.exram);
            mmc5.irq_scanline_compare = saved.irq_scanline_compare;
            mmc5.irq_enabled = saved.irq_enabled;
            mmc5.irq_pending.set(saved.irq_pending);
            mmc5.in_frame.set(saved.in_frame);
            mmc5.scanline_counter.set(saved.scanline_counter);
            mmc5.multiplier_a = saved.multiplier_a;
            mmc5.multiplier_b = saved.multiplier_b;
            mmc5.split_control = saved.split_control;
            mmc5.split_scroll = saved.split_scroll;
            mmc5.split_bank = saved.split_bank;
            mmc5.ppu_ctrl.set(saved.ppu_ctrl);
            mmc5.ppu_mask.set(saved.ppu_mask);
            mmc5.cached_tile_x.set(saved.cached_tile_x);
            mmc5.cached_tile_y.set(saved.cached_tile_y);
            mmc5.cached_ext_palette.set(saved.cached_ext_palette);
            mmc5.cached_ext_bank.set(saved.cached_ext_bank);
            mmc5.ppu_data_uses_bg_banks = saved.ppu_data_uses_bg_banks;
            mmc5.pulse1.duty = saved.audio.pulse1.duty;
            mmc5.pulse1.length_counter = saved.audio.pulse1.length_counter;
            mmc5.pulse1.envelope_divider = saved.audio.pulse1.envelope_divider;
            mmc5.pulse1.envelope_decay = saved.audio.pulse1.envelope_decay;
            mmc5.pulse1.envelope_disable = saved.audio.pulse1.envelope_disable;
            mmc5.pulse1.envelope_start = saved.audio.pulse1.envelope_start;
            mmc5.pulse1.volume = saved.audio.pulse1.volume;
            mmc5.pulse1.timer = saved.audio.pulse1.timer;
            mmc5.pulse1.timer_reload = saved.audio.pulse1.timer_reload;
            mmc5.pulse1.duty_counter = saved.audio.pulse1.duty_counter;
            mmc5.pulse1.length_enabled = saved.audio.pulse1.length_enabled;
            mmc5.pulse2.duty = saved.audio.pulse2.duty;
            mmc5.pulse2.length_counter = saved.audio.pulse2.length_counter;
            mmc5.pulse2.envelope_divider = saved.audio.pulse2.envelope_divider;
            mmc5.pulse2.envelope_decay = saved.audio.pulse2.envelope_decay;
            mmc5.pulse2.envelope_disable = saved.audio.pulse2.envelope_disable;
            mmc5.pulse2.envelope_start = saved.audio.pulse2.envelope_start;
            mmc5.pulse2.volume = saved.audio.pulse2.volume;
            mmc5.pulse2.timer = saved.audio.pulse2.timer;
            mmc5.pulse2.timer_reload = saved.audio.pulse2.timer_reload;
            mmc5.pulse2.duty_counter = saved.audio.pulse2.duty_counter;
            mmc5.pulse2.length_enabled = saved.audio.pulse2.length_enabled;
            mmc5.pulse1_enabled = saved.audio.pulse1_enabled;
            mmc5.pulse2_enabled = saved.audio.pulse2_enabled;
            mmc5.pcm_irq_enabled = saved.audio.pcm_irq_enabled;
            mmc5.pcm_read_mode = saved.audio.pcm_read_mode;
            mmc5.pcm_irq_pending.set(saved.audio.pcm_irq_pending);
            mmc5.pcm_dac = saved.audio.pcm_dac;
            mmc5.audio_frame_accum = saved.audio.audio_frame_accum;
            mmc5.audio_even_cycle = saved.audio.audio_even_cycle;
        }
    }
}
