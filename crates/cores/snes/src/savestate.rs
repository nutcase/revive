use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{Read, Write};

#[derive(Serialize, Deserialize)]
pub struct SaveState {
    pub version: u32,
    pub timestamp: u64,
    pub cpu_state: CpuSaveState,
    pub ppu_state: PpuSaveState,
    pub apu_state: ApuSaveState,
    pub memory_state: MemoryState,
    pub input_state: InputSaveState,
    #[serde(default)]
    pub bus_state: BusSaveState,
    #[serde(default)]
    pub emulator_state: EmulatorSaveState,
    pub master_cycles: u64,
    pub frame_count: u64,
    pub rom_checksum: u32,
}

#[derive(Serialize, Deserialize)]
pub struct CpuSaveState {
    pub a: u16,
    pub x: u16,
    pub y: u16,
    pub sp: u16,
    pub dp: u16,
    pub db: u8,
    pub pb: u8,
    pub pc: u16,
    pub p: u8,
    pub emulation_mode: bool,
    pub cycles: u64,
    #[serde(default)]
    pub waiting_for_irq: bool,
    #[serde(default)]
    pub stopped: bool,
    #[serde(default)]
    pub deferred_fetch: Option<CpuDeferredFetchSaveState>,
}

#[derive(Serialize, Deserialize, Clone, Copy)]
pub struct CpuDeferredFetchSaveState {
    pub opcode: u8,
    pub memspeed_penalty: u8,
    pub pc_before: u16,
    pub full_addr: u32,
}

#[derive(Serialize, Deserialize)]
pub struct PpuSaveState {
    pub scanline: u16,
    pub dot: u16,
    pub frame_count: u64,
    pub vblank: bool,
    pub hblank: bool,
    #[serde(default)]
    pub hv_latched_h: Option<u16>,
    #[serde(default)]
    pub hv_latched_v: Option<u16>,
    #[serde(default)]
    pub wio_latch_pending_dots: Option<u8>,
    #[serde(default)]
    pub slhv_latch_pending_dots: Option<u8>,
    #[serde(default)]
    pub ophct_second: Option<bool>,
    #[serde(default)]
    pub opvct_second: Option<bool>,
    pub brightness: u8,
    pub forced_blank: bool,
    pub nmi_enabled: bool,
    pub nmi_pending: bool,
    #[serde(default)]
    pub nmi_latched: bool,
    #[serde(default)]
    pub rdnmi_read_in_vblank: bool,
    pub bg_mode: u8,
    pub mosaic_size: u8,
    #[serde(default)]
    pub main_screen_designation: Option<u8>,
    #[serde(default)]
    pub sub_screen_designation: Option<u8>,
    pub bg_enabled: [bool; 4],
    pub bg_priority: [u8; 4],
    pub bg_scroll_x: [u16; 4],
    pub bg_scroll_y: [u16; 4],
    pub bg_tilemap_address: [u16; 4],
    pub bg_character_address: [u16; 4],
    pub vram: Vec<u8>,
    pub cgram: Vec<u8>,
    pub oam: Vec<u8>,
    #[serde(default)]
    pub framebuffer: Vec<u32>,
    #[serde(default)]
    pub subscreen_buffer: Vec<u32>,
    #[serde(default)]
    pub render_framebuffer: Vec<u32>,
    #[serde(default)]
    pub render_subscreen_buffer: Vec<u32>,
    pub vram_address: u16,
    pub vram_increment: u16,
    #[serde(default)]
    pub vram_read_buf_lo: Option<u8>,
    #[serde(default)]
    pub vram_read_buf_hi: Option<u8>,
    pub cgram_address: u8,
    #[serde(default)]
    pub cgram_read_second: Option<bool>,
    pub oam_address: u16,
    #[serde(default)]
    pub main_screen_designation_last_nonzero: Option<u8>,
    #[serde(default)]
    pub oam_internal_address: Option<u16>,
    #[serde(default)]
    pub oam_priority_rotation_enabled: Option<bool>,
    #[serde(default)]
    pub oam_eval_base: Option<u8>,
    #[serde(default)]
    pub sprite_size: Option<u8>,
    #[serde(default)]
    pub sprite_name_base: Option<u16>,
    #[serde(default)]
    pub sprite_name_select: Option<u16>,

    // --- Mode 7 ---
    #[serde(default)]
    pub mode7_matrix_a: Option<i16>,
    #[serde(default)]
    pub mode7_matrix_b: Option<i16>,
    #[serde(default)]
    pub mode7_matrix_c: Option<i16>,
    #[serde(default)]
    pub mode7_matrix_d: Option<i16>,
    #[serde(default)]
    pub mode7_center_x: Option<i16>,
    #[serde(default)]
    pub mode7_center_y: Option<i16>,
    #[serde(default)]
    pub mode7_hofs: Option<i16>,
    #[serde(default)]
    pub mode7_vofs: Option<i16>,
    #[serde(default)]
    pub mode7_latch: Option<u8>,
    #[serde(default)]
    pub m7sel: Option<u8>,

    // --- Color math ---
    #[serde(default)]
    pub cgwsel: Option<u8>,
    #[serde(default)]
    pub cgadsub: Option<u8>,
    #[serde(default)]
    pub fixed_color: Option<u16>,

    // --- Windows ---
    #[serde(default)]
    pub window1_left: Option<u8>,
    #[serde(default)]
    pub window1_right: Option<u8>,
    #[serde(default)]
    pub window2_left: Option<u8>,
    #[serde(default)]
    pub window2_right: Option<u8>,
    #[serde(default)]
    pub window_bg_mask: Option<[u8; 4]>,
    #[serde(default)]
    pub bg_window_logic: Option<[u8; 4]>,
    #[serde(default)]
    pub window_obj_mask: Option<u8>,
    #[serde(default)]
    pub obj_window_logic: Option<u8>,
    #[serde(default)]
    pub window_color_mask: Option<u8>,
    #[serde(default)]
    pub color_window_logic: Option<u8>,
    #[serde(default)]
    pub tmw_mask: Option<u8>,
    #[serde(default)]
    pub tsw_mask: Option<u8>,

    // --- Display settings ---
    #[serde(default)]
    pub setini: Option<u8>,
    #[serde(default)]
    pub pseudo_hires: Option<bool>,
    #[serde(default)]
    pub extbg: Option<bool>,
    #[serde(default)]
    pub overscan: Option<bool>,
    #[serde(default)]
    pub screen_display: Option<u8>,
    #[serde(default)]
    pub interlace: Option<bool>,
    #[serde(default)]
    pub obj_interlace: Option<bool>,
    #[serde(default)]
    pub force_no_blank: Option<bool>,

    // --- BG config ---
    #[serde(default)]
    pub bg_tile_16: Option<[bool; 4]>,
    #[serde(default)]
    pub bg_screen_size: Option<[u8; 4]>,
    #[serde(default)]
    pub mode1_bg3_priority: Option<bool>,

    // --- VRAM mapping ---
    #[serde(default)]
    pub vram_mapping: Option<u8>,

    // --- Latches ---
    #[serde(default)]
    pub bgofs_latch: Option<u8>,
    #[serde(default)]
    pub bghofs_latch: Option<u8>,
    #[serde(default)]
    pub cgram_second: Option<bool>,
    #[serde(default)]
    pub cgram_latch_lo: Option<u8>,
    #[serde(default)]
    pub bg_mosaic: Option<u8>,
    #[serde(default)]
    pub mode7_mul_b: Option<i8>,
    #[serde(default)]
    pub mode7_mul_result: Option<u32>,
    #[serde(default)]
    pub oam_write_latch: Option<u8>,
    #[serde(default)]
    pub hdma_head_busy_until: Option<u16>,
    #[serde(default)]
    pub framebuffer_rendering_enabled: Option<bool>,
    #[serde(default)]
    pub superfx_bypass_bg1_window: Option<bool>,
    #[serde(default)]
    pub superfx_authoritative_bg1_source: Option<bool>,
    #[serde(default)]
    pub superfx_direct_buffer: Vec<u8>,
    #[serde(default)]
    pub superfx_direct_height: Option<u16>,
    #[serde(default)]
    pub superfx_direct_bpp: Option<u8>,
    #[serde(default)]
    pub superfx_direct_mode: Option<u8>,
    #[serde(default)]
    pub superfx_tile_buffer: Vec<u8>,
    #[serde(default)]
    pub superfx_tile_bpp: Option<u8>,
    #[serde(default)]
    pub superfx_tile_mode: Option<u8>,
    #[serde(default)]
    pub wio_latch_enable: Option<bool>,
    #[serde(default)]
    pub stat78_latch_flag: Option<bool>,
    #[serde(default)]
    pub interlace_field: Option<bool>,
    #[serde(default)]
    pub sprite_overflow: Option<bool>,
    #[serde(default)]
    pub sprite_time_over: Option<bool>,
    #[serde(default)]
    pub sprite_overflow_latched: Option<bool>,
    #[serde(default)]
    pub sprite_time_over_latched: Option<bool>,
    #[serde(default)]
    pub latched_inidisp: Option<u8>,
    #[serde(default)]
    pub latched_tm: Option<u8>,
    #[serde(default)]
    pub latched_ts: Option<u8>,
    #[serde(default)]
    pub latched_tmw: Option<u8>,
    #[serde(default)]
    pub latched_tsw: Option<u8>,
    #[serde(default)]
    pub latched_cgwsel: Option<u8>,
    #[serde(default)]
    pub latched_cgadsub: Option<u8>,
    #[serde(default)]
    pub latched_fixed_color: Option<u16>,
    #[serde(default)]
    pub latched_setini: Option<u8>,
    #[serde(default)]
    pub latched_vmadd_lo: Option<u8>,
    #[serde(default)]
    pub latched_vmadd_hi: Option<u8>,
    #[serde(default)]
    pub latched_cgadd: Option<u8>,
    #[serde(default)]
    pub latched_vmain: Option<u8>,
    #[serde(default)]
    pub vmain_effect_pending: Option<u8>,
    #[serde(default)]
    pub vmain_effect_ticks: Option<u16>,
    #[serde(default)]
    pub cgadd_effect_pending: Option<u8>,
    #[serde(default)]
    pub cgadd_effect_ticks: Option<u16>,
    #[serde(default)]
    pub vmain_data_gap_ticks: Option<u16>,
    #[serde(default)]
    pub cgram_data_gap_ticks: Option<u16>,
    #[serde(default)]
    pub latched_wbglog: Option<u8>,
    #[serde(default)]
    pub latched_wobjlog: Option<u8>,
}

#[derive(Serialize, Deserialize, Default)]
pub struct ApuSaveState {
    #[serde(default)]
    pub ram: Vec<u8>,
    #[serde(default)]
    pub ports: [u8; 4],
    #[serde(default)]
    pub dsp_registers: Vec<u8>,
    #[serde(default)]
    pub cycle_counter: u64,
    #[serde(default)]
    pub timers: Vec<TimerSaveState>,
    #[serde(default)]
    pub channels: Vec<SoundChannelSaveState>,
    #[serde(default)]
    pub master_volume_left: u8,
    #[serde(default)]
    pub master_volume_right: u8,
    #[serde(default)]
    pub echo_volume_left: u8,
    #[serde(default)]
    pub echo_volume_right: u8,
    #[serde(default)]
    pub smp_pc: u16,
    #[serde(default)]
    pub smp_a: u8,
    #[serde(default)]
    pub smp_x: u8,
    #[serde(default)]
    pub smp_y: u8,
    #[serde(default)]
    pub smp_psw: u8,
    #[serde(default)]
    pub smp_sp: u8,
    #[serde(default)]
    pub smp_stopped: bool,
    #[serde(default)]
    pub smp_cycle_count: i32,
    #[serde(default)]
    pub cpu_to_apu_ports: [u8; 4],
    #[serde(default)]
    pub apu_to_cpu_ports: [u8; 4],
    #[serde(default)]
    pub port_latch: [u8; 4],
    #[serde(default)]
    pub dsp_reg_address: u8,
    #[serde(default)]
    pub is_ipl_rom_enabled: bool,
    #[serde(default)]
    pub ipl_rom: Vec<u8>,
    #[serde(default)]
    pub boot_state: u8,
    #[serde(default)]
    pub boot_port0_echo: u8,
    #[serde(default)]
    pub cycle_accum: f64,
    #[serde(default)]
    pub pending_cpu_cycles: u32,
    #[serde(default)]
    pub pending_port_writes: Vec<[u8; 2]>,
    #[serde(default)]
    pub zero_write_seen: bool,
    #[serde(default)]
    pub last_port1: u8,
    #[serde(default)]
    pub upload_addr: u16,
    #[serde(default)]
    pub expected_index: u8,
    #[serde(default)]
    pub block_active: bool,
    #[serde(default)]
    pub pending_idx: Option<u8>,
    #[serde(default)]
    pub pending_cmd: Option<u8>,
    #[serde(default)]
    pub data_ready: bool,
    #[serde(default)]
    pub upload_done_count: u64,
    #[serde(default)]
    pub upload_bytes: u64,
    #[serde(default)]
    pub last_upload_idx: u8,
    #[serde(default)]
    pub end_zero_streak: u8,
    #[serde(default)]
    pub smw_hle_end_zero_streak: u8,
}

#[derive(Serialize, Deserialize, Default)]
pub struct TimerSaveState {
    pub enabled: bool,
    pub target: u8,
    pub counter: u8,
    pub divider: u16,
    pub divider_target: u16,
}

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct SoundChannelSaveState {
    pub volume_left: u8,
    pub volume_right: u8,
    pub pitch: u16,
    pub sample_start: u16,
    pub sample_loop: u16,
    pub envelope: EnvelopeSaveState,
    pub enabled: bool,
    pub current_sample: u16,
    pub phase: u32,
    pub amplitude: i16,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct EnvelopeSaveState {
    pub attack_rate: u8,
    pub decay_rate: u8,
    pub sustain_level: u8,
    pub release_rate: u8,
    pub current_level: u16,
    pub state: u8, // EnvelopeState as u8
}

#[derive(Serialize, Deserialize)]
pub struct MemoryState {
    pub wram: Vec<u8>,
    pub sram: Vec<u8>,
}

#[derive(Serialize, Deserialize, Default)]
pub struct DmaChannelSaveState {
    pub control: u8,
    pub dest_address: u8,
    pub src_address: u32,
    pub size: u16,
    pub dasb: u8,
    pub a2a: u16,
    pub nltr: u8,
    pub unused: u8,
    pub hdma_table_addr: u32,
    pub hdma_line_counter: u8,
    pub hdma_repeat_flag: bool,
    pub hdma_do_transfer: bool,
    pub hdma_enabled: bool,
    pub hdma_terminated: bool,
    #[serde(default)]
    pub hdma_initialized_this_frame: bool,
    pub hdma_latched: [u8; 4],
    pub hdma_latched_len: u8,
    pub hdma_indirect: bool,
    pub hdma_indirect_addr: u32,
    pub configured: bool,
    pub cfg_ctrl: bool,
    pub cfg_dest: bool,
    pub cfg_src: bool,
    pub cfg_size: bool,
}

#[derive(Serialize, Deserialize, Default)]
pub struct DmaControllerSaveState {
    pub channels: [DmaChannelSaveState; 8],
    pub dma_enable: u8,
    pub hdma_enable: u8,
}

#[derive(Serialize, Deserialize, Default)]
pub struct BusSaveState {
    pub nmitimen: u8,
    pub wram_address: u32,
    pub mdr: u8,
    pub mul_a: u8,
    pub mul_b: u8,
    pub mul_result: u16,
    pub div_a: u16,
    pub div_b: u8,
    pub div_quot: u16,
    pub div_rem: u16,
    pub mul_busy: bool,
    pub mul_just_started: bool,
    pub mul_cycles_left: u8,
    pub mul_work_a: u16,
    pub mul_work_b: u8,
    pub mul_partial: u16,
    pub div_busy: bool,
    pub div_just_started: bool,
    pub div_cycles_left: u8,
    pub div_work_dividend: u16,
    pub div_work_divisor: u8,
    pub div_work_quot: u16,
    pub div_work_rem: u16,
    pub div_work_bit: i8,
    pub cpu_instr_active: bool,
    pub cpu_instr_bus_cycles: u8,
    pub cpu_instr_extra_master_cycles: u64,
    pub irq_h_enabled: bool,
    pub irq_v_enabled: bool,
    pub irq_pending: bool,
    pub irq_v_matched_line: Option<u16>,
    pub h_timer: u16,
    pub v_timer: u16,
    pub h_timer_set: bool,
    pub v_timer_set: bool,
    pub joy_busy_counter: u8,
    pub joy_data: [u8; 8],
    pub joy_busy_scanlines: u8,
    pub pending_gdma_mask: u8,
    pub pending_mdma_mask: u8,
    pub mdma_started_after_opcode_fetch: bool,
    pub rdnmi_consumed: bool,
    pub rdnmi_high_byte_for_test: u8,
    pub pending_stall_master_cycles: u64,
    pub smw_apu_hle: bool,
    pub smw_apu_hle_done: bool,
    pub smw_apu_hle_buf: Vec<u8>,
    pub smw_apu_hle_echo_idx: u32,
    pub wio: u8,
    pub fastrom: bool,
    pub dma_state: DmaControllerSaveState,
    #[serde(default)]
    pub spc7110_state: Option<crate::cartridge::spc7110::Spc7110SaveData>,
    #[serde(default)]
    pub superfx_state: Option<crate::cartridge::superfx::SuperFxSaveData>,
    #[serde(default)]
    pub sa1_state: Option<Sa1SaveState>,
}

#[derive(Serialize, Deserialize, Default)]
pub struct Sa1SaveState {
    #[serde(default)]
    pub cpu_state: CpuSaveState,
    #[serde(default)]
    pub registers: crate::cartridge::sa1::Registers,
    #[serde(default)]
    pub boot_vector_applied: bool,
    #[serde(default)]
    pub boot_pb: u8,
    #[serde(default)]
    pub pending_reset: bool,
    #[serde(default)]
    pub hold_reset: bool,
    #[serde(default)]
    pub ipl_ran: bool,
    #[serde(default)]
    pub h_timer_accum: u32,
    #[serde(default)]
    pub v_timer_accum: u32,
    #[serde(default)]
    pub math_cycles_left: u8,
    #[serde(default)]
    pub math_pending_result: u64,
    #[serde(default)]
    pub math_pending_overflow: bool,
    #[serde(default)]
    pub bwram: Vec<u8>,
    #[serde(default)]
    pub iram: Vec<u8>,
    #[serde(default)]
    pub cycle_deficit: i64,
    #[serde(default)]
    pub cycles_accum_frame: u64,
    #[serde(default)]
    pub nmi_delay_active: bool,
}

#[derive(Serialize, Deserialize, Default)]
pub struct EmulatorSaveState {
    pub pending_stall_master_cycles: u64,
    pub ppu_cycle_accum: u8,
    pub apu_cycle_debt: u32,
    pub apu_master_cycle_accum: u8,
    #[serde(default)]
    pub superfx_master_cycle_accum: u8,
    pub apu_step_batch: u32,
    pub apu_step_force: u32,
}

#[derive(Serialize, Deserialize, Default)]
pub struct InputSaveState {
    pub controller1_buttons: u16,
    pub controller2_buttons: u16,
    #[serde(default)]
    pub controller3_buttons: u16,
    #[serde(default)]
    pub controller4_buttons: u16,
    pub controller1_shift_register: u16,
    pub controller2_shift_register: u16,
    #[serde(default)]
    pub controller3_shift_register: u16,
    #[serde(default)]
    pub controller4_shift_register: u16,
    pub controller1_latched_buttons: u16,
    pub controller2_latched_buttons: u16,
    #[serde(default)]
    pub controller3_latched_buttons: u16,
    #[serde(default)]
    pub controller4_latched_buttons: u16,
    pub strobe: bool,
    #[serde(default)]
    pub multitap_enabled: bool,
    #[serde(default = "default_true")]
    pub controller1_connected: bool,
    #[serde(default)]
    pub controller2_connected: bool,
}

fn default_true() -> bool {
    true
}

impl SaveState {
    pub const CURRENT_VERSION: u32 = 2;

    pub fn new() -> Self {
        Self {
            version: Self::CURRENT_VERSION,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            cpu_state: CpuSaveState::default(),
            ppu_state: PpuSaveState::default(),
            apu_state: ApuSaveState::default(),
            memory_state: MemoryState::default(),
            input_state: InputSaveState::default(),
            bus_state: BusSaveState::default(),
            emulator_state: EmulatorSaveState::default(),
            master_cycles: 0,
            frame_count: 0,
            rom_checksum: 0,
        }
    }

    pub fn save_to_file(&self, filename: &str) -> Result<(), String> {
        let compressed_data = self.compress()?;
        let mut file =
            File::create(filename).map_err(|e| format!("Failed to create save file: {}", e))?;

        file.write_all(&compressed_data)
            .map_err(|e| format!("Failed to write save file: {}", e))?;

        Ok(())
    }

    pub fn load_from_file(filename: &str) -> Result<Self, String> {
        let mut file =
            File::open(filename).map_err(|e| format!("Failed to open save file: {}", e))?;

        let mut compressed_data = Vec::new();
        file.read_to_end(&mut compressed_data)
            .map_err(|e| format!("Failed to read save file: {}", e))?;

        Self::decompress(&compressed_data)
    }

    fn compress(&self) -> Result<Vec<u8>, String> {
        let json = serde_json::to_string(self)
            .map_err(|e| format!("Failed to serialize save state: {}", e))?;

        // Simple compression - store as JSON for now
        // In the future, we could add proper compression like zlib
        Ok(json.into_bytes())
    }

    fn decompress(data: &[u8]) -> Result<Self, String> {
        let json = String::from_utf8(data.to_vec())
            .map_err(|e| format!("Invalid save file format: {}", e))?;

        let save_state: SaveState = serde_json::from_str(&json)
            .map_err(|e| format!("Failed to deserialize save state: {}", e))?;

        if save_state.version > Self::CURRENT_VERSION {
            return Err(format!(
                "Save state version {} is not supported (current: {})",
                save_state.version,
                Self::CURRENT_VERSION
            ));
        }

        Ok(save_state)
    }

    pub fn validate_rom_checksum(&self, current_checksum: u32) -> bool {
        self.rom_checksum == current_checksum
    }

    #[allow(dead_code)]
    pub fn get_save_info(&self) -> SaveInfo {
        SaveInfo {
            version: self.version,
            timestamp: self.timestamp,
            frame_count: self.frame_count,
            rom_checksum: self.rom_checksum,
        }
    }
}

#[derive(Debug)]
#[allow(dead_code)]
pub struct SaveInfo {
    pub version: u32,
    pub timestamp: u64,
    pub frame_count: u64,
    pub rom_checksum: u32,
}

impl Default for CpuSaveState {
    fn default() -> Self {
        Self {
            a: 0,
            x: 0,
            y: 0,
            sp: 0x01FF,
            dp: 0,
            db: 0,
            pb: 0,
            pc: 0,
            p: 0x34, // IRQ_DISABLE | MEMORY_8BIT | INDEX_8BIT
            emulation_mode: true,
            cycles: 0,
            waiting_for_irq: false,
            stopped: false,
            deferred_fetch: None,
        }
    }
}

impl Default for PpuSaveState {
    fn default() -> Self {
        Self {
            scanline: 0,
            dot: 0,
            frame_count: 0,
            vblank: false,
            hblank: false,
            hv_latched_h: None,
            hv_latched_v: None,
            wio_latch_pending_dots: None,
            slhv_latch_pending_dots: None,
            ophct_second: None,
            opvct_second: None,
            brightness: 15,
            forced_blank: true,
            nmi_enabled: false,
            nmi_pending: false,
            nmi_latched: false,
            rdnmi_read_in_vblank: false,
            bg_mode: 0,
            mosaic_size: 1,
            main_screen_designation: None,
            sub_screen_designation: None,
            bg_enabled: [false; 4],
            bg_priority: [0; 4],
            bg_scroll_x: [0; 4],
            bg_scroll_y: [0; 4],
            bg_tilemap_address: [0; 4],
            bg_character_address: [0; 4],
            vram: vec![0; 0x10000],
            cgram: vec![0; 0x200],
            oam: vec![0; 0x220],
            framebuffer: vec![0; 256 * 239],
            subscreen_buffer: vec![0; 256 * 239],
            render_framebuffer: vec![0; 256 * 239],
            render_subscreen_buffer: vec![0; 256 * 239],
            vram_address: 0,
            vram_increment: 1,
            vram_read_buf_lo: None,
            vram_read_buf_hi: None,
            cgram_address: 0,
            cgram_read_second: None,
            oam_address: 0,
            main_screen_designation_last_nonzero: None,
            oam_internal_address: None,
            oam_priority_rotation_enabled: None,
            oam_eval_base: None,
            sprite_size: None,
            sprite_name_base: None,
            sprite_name_select: None,
            mode7_matrix_a: None,
            mode7_matrix_b: None,
            mode7_matrix_c: None,
            mode7_matrix_d: None,
            mode7_center_x: None,
            mode7_center_y: None,
            mode7_hofs: None,
            mode7_vofs: None,
            mode7_latch: None,
            m7sel: None,
            cgwsel: None,
            cgadsub: None,
            fixed_color: None,
            window1_left: None,
            window1_right: None,
            window2_left: None,
            window2_right: None,
            window_bg_mask: None,
            bg_window_logic: None,
            window_obj_mask: None,
            obj_window_logic: None,
            window_color_mask: None,
            color_window_logic: None,
            tmw_mask: None,
            tsw_mask: None,
            setini: None,
            pseudo_hires: None,
            extbg: None,
            overscan: None,
            screen_display: None,
            interlace: None,
            obj_interlace: None,
            force_no_blank: None,
            bg_tile_16: None,
            bg_screen_size: None,
            mode1_bg3_priority: None,
            vram_mapping: None,
            bgofs_latch: None,
            bghofs_latch: None,
            cgram_second: None,
            cgram_latch_lo: None,
            bg_mosaic: None,
            mode7_mul_b: None,
            mode7_mul_result: None,
            oam_write_latch: None,
            hdma_head_busy_until: None,
            framebuffer_rendering_enabled: None,
            superfx_bypass_bg1_window: None,
            superfx_authoritative_bg1_source: None,
            superfx_direct_buffer: Vec::new(),
            superfx_direct_height: None,
            superfx_direct_bpp: None,
            superfx_direct_mode: None,
            superfx_tile_buffer: Vec::new(),
            superfx_tile_bpp: None,
            superfx_tile_mode: None,
            wio_latch_enable: None,
            stat78_latch_flag: None,
            interlace_field: None,
            sprite_overflow: None,
            sprite_time_over: None,
            sprite_overflow_latched: None,
            sprite_time_over_latched: None,
            latched_inidisp: None,
            latched_tm: None,
            latched_ts: None,
            latched_tmw: None,
            latched_tsw: None,
            latched_cgwsel: None,
            latched_cgadsub: None,
            latched_fixed_color: None,
            latched_setini: None,
            latched_vmadd_lo: None,
            latched_vmadd_hi: None,
            latched_cgadd: None,
            latched_vmain: None,
            vmain_effect_pending: None,
            vmain_effect_ticks: None,
            cgadd_effect_pending: None,
            cgadd_effect_ticks: None,
            vmain_data_gap_ticks: None,
            cgram_data_gap_ticks: None,
            latched_wbglog: None,
            latched_wobjlog: None,
        }
    }
}

impl Default for EnvelopeSaveState {
    fn default() -> Self {
        Self {
            attack_rate: 0,
            decay_rate: 0,
            sustain_level: 0,
            release_rate: 0,
            current_level: 0,
            state: 3, // EnvelopeState::Release
        }
    }
}

impl Default for MemoryState {
    fn default() -> Self {
        Self {
            wram: vec![0; 0x20000],
            sram: vec![0; 0x8000],
        }
    }
}
