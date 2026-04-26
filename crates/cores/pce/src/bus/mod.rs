use crate::psg::Psg;
use crate::vce::Vce;
use crate::vdc::{
    DMA_CTRL_DST_DEC, DMA_CTRL_SRC_DEC, FRAME_HEIGHT, FRAME_WIDTH, VDC_CTRL_ENABLE_BACKGROUND,
    VDC_CTRL_ENABLE_BACKGROUND_LEGACY, VDC_CTRL_ENABLE_SPRITES, VDC_CTRL_ENABLE_SPRITES_LEGACY,
    VDC_DMA_WORD_CYCLES, VDC_VBLANK_INTERVAL, Vdc,
};

// Re-export VDC status constants for external consumers (examples, etc.)
// These were originally `pub const` in this file.
pub use crate::vdc::{
    VDC_STATUS_BUSY, VDC_STATUS_CR, VDC_STATUS_DS, VDC_STATUS_DV, VDC_STATUS_OR, VDC_STATUS_RCR,
    VDC_STATUS_VBL,
};

pub const PAGE_SIZE: usize = 0x2000; // 8 KiB per bank
const NUM_BANKS: usize = 8;
const RAM_SIZE: usize = PAGE_SIZE * NUM_BANKS;
const IO_REG_SIZE: usize = PAGE_SIZE; // full hardware page
pub const IRQ_DISABLE_IRQ2: u8 = 0x01;
pub const IRQ_DISABLE_IRQ1: u8 = 0x02;
pub const IRQ_DISABLE_TIMER: u8 = 0x04;
pub const IRQ_REQUEST_IRQ2: u8 = 0x01;
pub const IRQ_REQUEST_IRQ1: u8 = 0x02;
pub const IRQ_REQUEST_TIMER: u8 = 0x04;
const TIMER_CONTROL_START: u8 = 0x01;
const HW_TIMER_BASE: usize = 0x0C00;
const HW_JOYPAD_BASE: usize = 0x1000;
const HW_IRQ_BASE: usize = 0x1400;
const HW_CPU_CTRL_BASE: usize = 0x1C00;
const BRAM_PAGE: u8 = 0xF7;
const BRAM_SIZE: usize = 0x0800;
const BRAM_PAGE_DUMP_SIZE: usize = PAGE_SIZE;
pub(super) const BRAM_FORMAT_HEADER: [u8; 8] = [0x48, 0x55, 0x42, 0x4D, 0x00, 0x88, 0x10, 0x80];
const BRAM_LOCK_PORT: usize = 0x1803;
const BRAM_UNLOCK_PORT: usize = 0x1807;
const MASTER_CLOCK_HZ: u32 = 7_159_090;
const PSG_CLOCK_HZ: u32 = MASTER_CLOCK_HZ / 2;
const AUDIO_SAMPLE_RATE: u32 = 44_100;
const DEFAULT_DISPLAY_HEIGHT: usize = 224;
const LARGE_HUCARD_MAPPER_THRESHOLD_PAGES: usize = 0x100; // 2 MiB / 8 KiB
const LARGE_HUCARD_MAPPER_WINDOW_PAGES: usize = 0x40; // 512 KiB / 8 KiB

mod access;
mod env;
mod font;
mod io;
mod mapping;
mod render;
mod runtime;
mod state;
mod types;

use self::types::TransientU64;
use self::types::{
    BankMapping, ControlRegister, IoPort, PaletteFlickerEvent, Timer, TransientBool, TransientBram,
    TransientPaletteFlicker, TransientSpriteVramSnapshot, TransientUsize, VdcPort,
};
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AudioDiagnostics {
    pub master_clock_hz: u32,
    pub sample_rate_hz: u32,
    pub total_phi_cycles: u64,
    pub generated_samples: u64,
    pub drained_samples: u64,
    pub drain_calls: u64,
    pub pending_bus_samples: usize,
    pub phi_remainder: u64,
}

/// Memory bus exposing an 8x8 KiB banked window into linear RAM/ROM data.
/// This mirrors the HuC6280 page architecture and provides simple helpers
/// for experimenting with bank switching.
#[derive(Clone, bincode::Encode, bincode::Decode)]
pub struct Bus {
    ram: Vec<u8>,
    rom: Vec<u8>,
    large_hucard_mapper: bool,
    large_hucard_latch: u8,
    large_hucard_bank_mask: u8,
    banks: [BankMapping; NUM_BANKS],
    mpr: [u8; NUM_BANKS],
    st_ports: [u8; 3],
    io: [u8; IO_REG_SIZE],
    io_port: IoPort,
    interrupt_disable: u8,
    interrupt_request: u8,
    timer: Timer,
    vdc: Vdc,
    psg: Psg,
    vce: Vce,
    audio_phi_accumulator: u64,
    audio_psg_accumulator: TransientU64,
    audio_buffer: Vec<i16>,
    audio_total_phi_cycles: TransientU64,
    audio_total_generated_samples: TransientU64,
    audio_total_drained_samples: TransientU64,
    audio_total_drain_calls: TransientU64,
    cpu_vdc_vce_penalty_cycles: TransientU64,
    cpu_high_speed_hint: TransientBool,
    vce_palette_flicker: TransientPaletteFlicker,
    sprite_vram_snapshot: TransientSpriteVramSnapshot,
    framebuffer: Vec<u32>,
    frame_ready: bool,
    cart_ram: Vec<u8>,
    bram: TransientBram,
    bram_unlocked: TransientBool,
    video_output_enabled: TransientBool,
    current_display_width: usize,
    current_display_height: usize,
    current_display_x_offset: TransientUsize,
    current_display_y_offset: usize,
    bg_opaque: Vec<bool>,
    bg_priority: Vec<bool>,
    sprite_line_counts: Vec<u8>,
    /// True while in a post-burst-mode transition: the VDC went through burst
    /// mode (both BG and SPR off) and is now in SPR-only mode.  During this
    /// window the game is setting up the new scene; rendering sprites would
    /// flash partially-loaded content.  Cleared when BG is re-enabled (scene
    /// ready).  This matches CRT behavior where the brief sprite flash during
    /// scene transitions is invisible due to phosphor response/blanking.
    /// Not serialized — transient render state, safe to default to false.
    burst_transition: TransientBool,
    #[cfg(feature = "trace_hw_writes")]
    last_pc_for_trace: Option<u16>,
    #[cfg(debug_assertions)]
    debug_force_ds_after: TransientU64,
    #[cfg(feature = "trace_hw_writes")]
    st0_lock_window: u8,
}

#[derive(Clone, bincode::Encode, bincode::Decode)]
pub(crate) struct CompatBusStateV1 {
    ram: Vec<u8>,
    rom: Vec<u8>,
    banks: [BankMapping; NUM_BANKS],
    mpr: [u8; NUM_BANKS],
    st_ports: [u8; 3],
    io: [u8; IO_REG_SIZE],
    io_port: IoPort,
    interrupt_disable: u8,
    interrupt_request: u8,
    timer: Timer,
    vdc: crate::vdc::CompatVdcStateV1,
    psg: Psg,
    vce: Vce,
    audio_phi_accumulator: u64,
    audio_psg_accumulator: TransientU64,
    audio_buffer: Vec<i16>,
    audio_total_phi_cycles: TransientU64,
    audio_total_generated_samples: TransientU64,
    audio_total_drained_samples: TransientU64,
    audio_total_drain_calls: TransientU64,
    cpu_vdc_vce_penalty_cycles: TransientU64,
    cpu_high_speed_hint: TransientBool,
    vce_palette_flicker: TransientPaletteFlicker,
    framebuffer: Vec<u32>,
    frame_ready: bool,
    cart_ram: Vec<u8>,
    bram: TransientBram,
    bram_unlocked: TransientBool,
    video_output_enabled: TransientBool,
    current_display_width: usize,
    current_display_height: usize,
    current_display_x_offset: usize,
    current_display_y_offset: usize,
    bg_opaque: Vec<bool>,
    bg_priority: Vec<bool>,
    sprite_line_counts: Vec<u8>,
    burst_transition: TransientBool,
    #[cfg(feature = "trace_hw_writes")]
    last_pc_for_trace: Option<u16>,
    #[cfg(debug_assertions)]
    debug_force_ds_after: TransientU64,
    #[cfg(feature = "trace_hw_writes")]
    st0_lock_window: u8,
}

impl Bus {
    pub fn new() -> Self {
        let mut bus = Self {
            ram: vec![0; RAM_SIZE],
            rom: Vec::new(),
            large_hucard_mapper: false,
            large_hucard_latch: 0,
            large_hucard_bank_mask: 0,
            banks: [BankMapping::Ram { base: 0 }; NUM_BANKS],
            mpr: [0; NUM_BANKS],
            st_ports: [0; 3],
            io: [0; IO_REG_SIZE],
            io_port: IoPort::new(),
            interrupt_disable: 0,
            interrupt_request: 0,
            timer: Timer::new(),
            vdc: Vdc::new(),
            psg: Psg::new(),
            vce: Vce::new(),
            audio_phi_accumulator: 0,
            audio_psg_accumulator: TransientU64(0),
            audio_buffer: Vec::new(),
            audio_total_phi_cycles: TransientU64(0),
            audio_total_generated_samples: TransientU64(0),
            audio_total_drained_samples: TransientU64(0),
            audio_total_drain_calls: TransientU64(0),
            cpu_vdc_vce_penalty_cycles: TransientU64(0),
            cpu_high_speed_hint: TransientBool(false),
            vce_palette_flicker: TransientPaletteFlicker::default(),
            sprite_vram_snapshot: TransientSpriteVramSnapshot::default(),
            framebuffer: vec![0; FRAME_WIDTH * FRAME_HEIGHT],
            frame_ready: false,
            cart_ram: Vec::new(),
            bram: TransientBram::default(),
            bram_unlocked: TransientBool(false),
            video_output_enabled: TransientBool(true),
            current_display_width: 256,
            current_display_height: DEFAULT_DISPLAY_HEIGHT,
            current_display_x_offset: TransientUsize(0),
            current_display_y_offset: 0,
            bg_opaque: vec![false; FRAME_WIDTH * FRAME_HEIGHT],
            bg_priority: vec![false; FRAME_WIDTH * FRAME_HEIGHT],
            sprite_line_counts: vec![0; FRAME_HEIGHT],
            burst_transition: TransientBool(false),
            #[cfg(feature = "trace_hw_writes")]
            last_pc_for_trace: None,
            #[cfg(debug_assertions)]
            debug_force_ds_after: TransientU64(0),
            #[cfg(feature = "trace_hw_writes")]
            st0_lock_window: 0,
        };

        // Power-on mapping: expose internal RAM in bank 0 for ZP/stack and
        // keep all banks backed by RAM. The HuCARD loader remaps banks 4–7
        // to ROM after parsing the image header.
        let ram_pages = RAM_SIZE / PAGE_SIZE;
        for index in 0..NUM_BANKS {
            let page = index % ram_pages;
            bus.mpr[index] = 0xF8u8.saturating_add(page as u8);
            bus.update_mpr(index);
        }
        // Keep the top bank pointing at RAM so the reset vector can be patched
        // when loading raw programs; HuCARD mapping will override this later.
        bus.mpr[NUM_BANKS - 1] = 0xF8;
        bus.update_mpr(NUM_BANKS - 1);

        if Self::env_force_mpr1_hardware() {
            bus.set_mpr(1, 0xFF);
        }
        // Allow overriding default pad input for BIOS waits.
        bus.io_port.input = Self::env_pad_default();
        // Optionally start timer running by default (debug aid).
        if Self::env_timer_default_start() {
            bus.timer.enabled = true;
            bus.timer.counter = bus.timer.reload;
            bus.timer.prescaler = 0;
        }

        if Self::env_force_title_scene() {
            bus.force_title_scene();
        }

        bus
    }

    #[cfg(feature = "trace_hw_writes")]
    fn log_hw_access(kind: &str, addr: u16, value: u8) {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static COUNT: AtomicUsize = AtomicUsize::new(0);
        let idx = COUNT.fetch_add(1, Ordering::Relaxed);
        if idx < 1_000_000 {
            eprintln!("{kind} {:04X} -> {:02X}", addr, value);
        }
    }
}

impl From<CompatBusStateV1> for Bus {
    fn from(value: CompatBusStateV1) -> Self {
        Self {
            ram: value.ram,
            rom: value.rom,
            large_hucard_mapper: false,
            large_hucard_latch: 0,
            large_hucard_bank_mask: 0,
            banks: value.banks,
            mpr: value.mpr,
            st_ports: value.st_ports,
            io: value.io,
            io_port: value.io_port,
            interrupt_disable: value.interrupt_disable,
            interrupt_request: value.interrupt_request,
            timer: value.timer,
            vdc: value.vdc.into(),
            psg: value.psg,
            vce: value.vce,
            audio_phi_accumulator: value.audio_phi_accumulator,
            audio_psg_accumulator: value.audio_psg_accumulator,
            audio_buffer: value.audio_buffer,
            audio_total_phi_cycles: value.audio_total_phi_cycles,
            audio_total_generated_samples: value.audio_total_generated_samples,
            audio_total_drained_samples: value.audio_total_drained_samples,
            audio_total_drain_calls: value.audio_total_drain_calls,
            cpu_vdc_vce_penalty_cycles: value.cpu_vdc_vce_penalty_cycles,
            cpu_high_speed_hint: value.cpu_high_speed_hint,
            vce_palette_flicker: value.vce_palette_flicker,
            sprite_vram_snapshot: TransientSpriteVramSnapshot::default(),
            framebuffer: value.framebuffer,
            frame_ready: value.frame_ready,
            cart_ram: value.cart_ram,
            bram: value.bram,
            bram_unlocked: value.bram_unlocked,
            video_output_enabled: value.video_output_enabled,
            current_display_width: value.current_display_width,
            current_display_height: value.current_display_height,
            current_display_x_offset: TransientUsize(value.current_display_x_offset),
            current_display_y_offset: value.current_display_y_offset,
            bg_opaque: value.bg_opaque,
            bg_priority: value.bg_priority,
            sprite_line_counts: value.sprite_line_counts,
            burst_transition: value.burst_transition,
            #[cfg(feature = "trace_hw_writes")]
            last_pc_for_trace: value.last_pc_for_trace,
            #[cfg(debug_assertions)]
            debug_force_ds_after: value.debug_force_ds_after,
            #[cfg(feature = "trace_hw_writes")]
            st0_lock_window: value.st0_lock_window,
        }
    }
}

#[cfg(test)]
impl Bus {
    pub(crate) fn compat_state_v1(&self) -> CompatBusStateV1 {
        CompatBusStateV1 {
            ram: self.ram.clone(),
            rom: self.rom.clone(),
            banks: self.banks,
            mpr: self.mpr,
            st_ports: self.st_ports,
            io: self.io,
            io_port: self.io_port,
            interrupt_disable: self.interrupt_disable,
            interrupt_request: self.interrupt_request,
            timer: self.timer,
            vdc: self.vdc.compat_state_v1(),
            psg: self.psg.clone(),
            vce: self.vce.clone(),
            audio_phi_accumulator: self.audio_phi_accumulator,
            audio_psg_accumulator: self.audio_psg_accumulator,
            audio_buffer: self.audio_buffer.clone(),
            audio_total_phi_cycles: self.audio_total_phi_cycles,
            audio_total_generated_samples: self.audio_total_generated_samples,
            audio_total_drained_samples: self.audio_total_drained_samples,
            audio_total_drain_calls: self.audio_total_drain_calls,
            cpu_vdc_vce_penalty_cycles: self.cpu_vdc_vce_penalty_cycles,
            cpu_high_speed_hint: self.cpu_high_speed_hint,
            vce_palette_flicker: self.vce_palette_flicker.clone(),
            framebuffer: self.framebuffer.clone(),
            frame_ready: self.frame_ready,
            cart_ram: self.cart_ram.clone(),
            bram: self.bram.clone(),
            bram_unlocked: self.bram_unlocked,
            video_output_enabled: self.video_output_enabled,
            current_display_width: self.current_display_width,
            current_display_height: self.current_display_height,
            current_display_x_offset: *self.current_display_x_offset,
            current_display_y_offset: self.current_display_y_offset,
            bg_opaque: self.bg_opaque.clone(),
            bg_priority: self.bg_priority.clone(),
            sprite_line_counts: self.sprite_line_counts.clone(),
            burst_transition: self.burst_transition,
            #[cfg(feature = "trace_hw_writes")]
            last_pc_for_trace: self.last_pc_for_trace,
            #[cfg(debug_assertions)]
            debug_force_ds_after: self.debug_force_ds_after,
            #[cfg(feature = "trace_hw_writes")]
            st0_lock_window: self.st0_lock_window,
        }
    }
}

#[cfg(test)]
mod tests;
