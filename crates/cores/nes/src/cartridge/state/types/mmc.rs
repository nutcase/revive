use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mmc1State {
    pub shift_register: u8,
    pub shift_count: u8,
    pub control: u8,
    pub chr_bank_0: u8,
    pub chr_bank_1: u8,
    pub prg_bank: u8,
    pub prg_ram_disable: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mmc2State {
    pub prg_bank: u8,
    pub chr_bank_0_fd: u8,
    pub chr_bank_0_fe: u8,
    pub chr_bank_1_fd: u8,
    pub chr_bank_1_fe: u8,
    pub latch_0: bool,
    pub latch_1: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mmc3State {
    pub bank_select: u8,
    pub bank_registers: [u8; 8],
    #[serde(default)]
    pub extra_bank_registers: [u8; 8],
    pub irq_latch: u8,
    pub irq_counter: u8,
    pub irq_reload: bool,
    pub irq_enabled: bool,
    pub irq_pending: bool,
    pub prg_ram_enabled: bool,
    pub prg_ram_write_protect: bool,
    #[serde(default)]
    pub irq_cycle_mode: bool,
    #[serde(default = "default_mmc3_irq_prescaler")]
    pub irq_prescaler: u8,
    #[serde(default)]
    pub irq_delay: u8,
}

fn default_mmc3_irq_prescaler() -> u8 {
    4
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Mmc5PulseState {
    pub duty: u8,
    pub length_counter: u8,
    pub envelope_divider: u8,
    pub envelope_decay: u8,
    pub envelope_disable: bool,
    pub envelope_start: bool,
    pub volume: u8,
    pub timer: u16,
    pub timer_reload: u16,
    pub duty_counter: u8,
    pub length_enabled: bool,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Mmc5AudioState {
    pub pulse1: Mmc5PulseState,
    pub pulse2: Mmc5PulseState,
    pub pulse1_enabled: bool,
    pub pulse2_enabled: bool,
    pub pcm_irq_enabled: bool,
    pub pcm_read_mode: bool,
    pub pcm_irq_pending: bool,
    pub pcm_dac: u8,
    pub audio_frame_accum: u32,
    pub audio_even_cycle: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mmc5State {
    pub prg_mode: u8,
    pub chr_mode: u8,
    pub exram_mode: u8,
    pub prg_ram_protect_1: u8,
    pub prg_ram_protect_2: u8,
    pub nametable_map: [u8; 4],
    pub fill_tile: u8,
    pub fill_attr: u8,
    pub prg_ram_bank: u8,
    pub prg_banks: [u8; 4],
    pub chr_upper: u8,
    pub sprite_chr_banks: [u8; 8],
    pub bg_chr_banks: [u8; 4],
    pub exram: Vec<u8>,
    pub irq_scanline_compare: u8,
    pub irq_enabled: bool,
    pub irq_pending: bool,
    pub in_frame: bool,
    pub scanline_counter: u8,
    pub multiplier_a: u8,
    pub multiplier_b: u8,
    pub split_control: u8,
    pub split_scroll: u8,
    pub split_bank: u8,
    pub ppu_ctrl: u8,
    pub ppu_mask: u8,
    pub cached_tile_x: u8,
    pub cached_tile_y: u8,
    pub cached_ext_palette: u8,
    pub cached_ext_bank: u8,
    #[serde(default)]
    pub ppu_data_uses_bg_banks: bool,
    #[serde(default)]
    pub audio: Mmc5AudioState,
}
