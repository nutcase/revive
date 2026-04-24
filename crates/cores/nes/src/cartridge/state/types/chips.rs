use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Namco163State {
    pub chr_banks: [u8; 12],
    pub prg_banks: [u8; 3],
    pub sound_disable: bool,
    pub chr_nt_disabled_low: bool,
    pub chr_nt_disabled_high: bool,
    pub wram_write_enable: bool,
    pub wram_write_protect: u8,
    pub internal_addr: u8,
    pub internal_auto_increment: bool,
    pub irq_counter: u16,
    pub irq_enabled: bool,
    pub irq_pending: bool,
    pub audio_delay: u8,
    pub audio_channel_index: u8,
    pub audio_outputs: [f32; 8],
    pub audio_current: f32,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mapper18State {
    pub prg_banks: [u8; 3],
    pub chr_banks: [u8; 8],
    pub prg_ram_enabled: bool,
    pub prg_ram_write_enabled: bool,
    pub irq_reload: u16,
    pub irq_counter: u16,
    pub irq_control: u8,
    pub irq_pending: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mapper210State {
    pub chr_banks: [u8; 8],
    pub prg_banks: [u8; 3],
    pub namco340: bool,
    pub prg_ram_enabled: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Fme7State {
    pub command: u8,
    pub chr_banks: [u8; 8],
    pub prg_banks: [u8; 3],
    pub prg_bank_6000: u8,
    pub prg_ram_enabled: bool,
    pub prg_ram_select: bool,
    pub irq_counter: u16,
    pub irq_counter_enabled: bool,
    pub irq_enabled: bool,
    pub irq_pending: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BandaiFcgState {
    pub chr_banks: [u8; 8],
    pub prg_bank: u8,
    pub irq_counter: u16,
    pub irq_latch: u16,
    pub irq_enabled: bool,
    pub irq_pending: bool,
    #[serde(default)]
    pub outer_prg_bank: u8,
    #[serde(default)]
    pub prg_ram_enabled: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Vrc1State {
    pub prg_banks: [u8; 3],
    pub chr_bank_0: u8,
    pub chr_bank_1: u8,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Vrc2Vrc4State {
    pub prg_banks: [u8; 2],
    pub chr_banks: [u16; 8],
    pub wram_enabled: bool,
    pub prg_swap_mode: bool,
    pub vrc4_mode: bool,
    pub latch: u8,
    pub irq_latch: u8,
    pub irq_counter: u8,
    pub irq_enable_after_ack: bool,
    pub irq_enabled: bool,
    pub irq_cycle_mode: bool,
    pub irq_prescaler: i16,
    pub irq_pending: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IremG101State {
    pub prg_banks: [u8; 2],
    pub chr_banks: [u8; 8],
    pub prg_mode: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IremH3001State {
    pub prg_banks: [u8; 2],
    pub chr_banks: [u8; 8],
    pub prg_mode: bool,
    pub irq_reload: u16,
    pub irq_counter: u16,
    pub irq_enabled: bool,
    pub irq_pending: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Vrc3State {
    pub irq_reload: u16,
    pub irq_counter: u16,
    pub irq_enable_on_ack: bool,
    pub irq_enabled: bool,
    pub irq_mode_8bit: bool,
    pub irq_pending: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Vrc6PulseState {
    pub volume: u8,
    pub duty: u8,
    pub ignore_duty: bool,
    pub period: u16,
    pub enabled: bool,
    pub step: u8,
    pub divider: u16,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Vrc6SawState {
    pub rate: u8,
    pub period: u16,
    pub enabled: bool,
    pub step: u8,
    pub divider: u16,
    pub accumulator: u8,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Vrc6State {
    pub prg_bank_16k: u8,
    pub prg_bank_8k: u8,
    pub chr_banks: [u8; 8],
    pub banking_control: u8,
    pub irq_latch: u8,
    pub irq_counter: u8,
    pub irq_enable_after_ack: bool,
    pub irq_enabled: bool,
    pub irq_cycle_mode: bool,
    pub irq_prescaler: i16,
    pub irq_pending: bool,
    pub audio_halt: bool,
    pub audio_freq_shift: u8,
    pub pulse1: Vrc6PulseState,
    pub pulse2: Vrc6PulseState,
    pub saw: Vrc6SawState,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Vrc7State {
    pub prg_banks: [u8; 3],
    pub chr_banks: [u8; 8],
    pub control: u8,
    pub wram_enabled: bool,
    pub audio_silenced: bool,
    pub irq_latch: u8,
    pub irq_counter: u8,
    pub irq_enable_after_ack: bool,
    pub irq_enabled: bool,
    pub irq_cycle_mode: bool,
    pub irq_prescaler: i16,
    pub irq_pending: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Sunsoft3State {
    pub chr_banks: [u8; 4],
    pub prg_bank: u8,
    pub irq_counter: u16,
    pub irq_enabled: bool,
    pub irq_pending: bool,
    pub irq_write_high: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Sunsoft4State {
    pub chr_banks: [u8; 4],
    pub nametable_banks: [u8; 2],
    pub control: u8,
    pub prg_bank: u8,
    pub prg_ram_enabled: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaitoTc0190State {
    pub prg_banks: [u8; 2],
    pub chr_banks: [u8; 6],
    #[serde(default)]
    pub irq_latch: u8,
    #[serde(default)]
    pub irq_counter: u8,
    #[serde(default)]
    pub irq_reload: bool,
    #[serde(default)]
    pub irq_enabled: bool,
    #[serde(default)]
    pub irq_pending: bool,
    #[serde(default)]
    pub irq_delay: u8,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaitoX1005State {
    pub prg_banks: [u8; 3],
    pub chr_banks: [u8; 6],
    pub ram_enabled: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaitoX1017State {
    pub prg_banks: [u8; 3],
    pub chr_banks: [u8; 6],
    pub ram_enabled: [bool; 3],
    pub chr_invert: bool,
}
