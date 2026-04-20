use std::cell::RefCell;
use std::collections::VecDeque;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU32, Ordering};

const BIOS_BASE: u32 = 0x0000_0000;
const EWRAM_BASE: u32 = 0x0200_0000;
const IWRAM_BASE: u32 = 0x0300_0000;
const IO_BASE: u32 = 0x0400_0000;
const PRAM_BASE: u32 = 0x0500_0000;
const VRAM_BASE: u32 = 0x0600_0000;
const OAM_BASE: u32 = 0x0700_0000;
const ROM0_BASE: u32 = 0x0800_0000;
const ROM1_BASE: u32 = 0x0A00_0000;
const ROM2_BASE: u32 = 0x0C00_0000;
const EEPROM_BASE: u32 = 0x0D00_0000;
const SRAM_BASE: u32 = 0x0E00_0000;

const BIOS_SIZE: usize = 16 * 1024;
const EWRAM_SIZE: usize = 256 * 1024;
const IWRAM_SIZE: usize = 32 * 1024;
const IWRAM_EXEC_PAGE_SHIFT: usize = 8; // 256-byte pages.
const IWRAM_EXEC_PAGE_SIZE: usize = 1 << IWRAM_EXEC_PAGE_SHIFT;
const IWRAM_EXEC_TRACK_PAGES: usize = IWRAM_SIZE / IWRAM_EXEC_PAGE_SIZE;
const IWRAM_EXEC_BITMAP_WORDS: usize = IWRAM_EXEC_TRACK_PAGES / 64;
const IO_SIZE: usize = 0x400;
const SCANLINE_IO_SNAPSHOT_SIZE: usize = 0x56; // IO 0x00..0x55 (DISPCNT through BLDY)
const GBA_VISIBLE_LINES: usize = 160;
const PRAM_SIZE: usize = 1024;
const VRAM_SIZE: usize = 96 * 1024;
const OAM_SIZE: usize = 1024;
const SRAM_SIZE: usize = 64 * 1024;
const OAM_OBJ_STRIDE: usize = 8;
const VRAM_MIRROR_START: usize = 0x18_000;
const VRAM_MIRROR_BASE: usize = 0x10_000;
const BG_BITMAP_VRAM_SNAPSHOT_SIZE: usize = 0x14_000;
const OBJ_VRAM_SNAPSHOT_SIZE: usize = VRAM_SIZE - VRAM_MIRROR_BASE;
const LEGACY_MISSING_SCANLINE_SNAPSHOT_SIZE: usize = (PRAM_SIZE * GBA_VISIBLE_LINES)
    + GBA_VISIBLE_LINES
    + (BG_BITMAP_VRAM_SNAPSHOT_SIZE * GBA_VISIBLE_LINES)
    + GBA_VISIBLE_LINES
    + (OBJ_VRAM_SNAPSHOT_SIZE * GBA_VISIBLE_LINES)
    + GBA_VISIBLE_LINES
    + (OAM_SIZE * GBA_VISIBLE_LINES)
    + GBA_VISIBLE_LINES;
const LEGACY_SCANLINE_SNAPSHOT_TOLERANCE: usize = 256;
const EEPROM_512_SIZE: usize = 512;
const EEPROM_8K_SIZE: usize = 8 * 1024;
const EEPROM_TAG: &[u8] = b"EEPROM_V";
const SRAM_TAG: &[u8] = b"SRAM_V";
const FLASH_TAG: &[u8] = b"FLASH_V";
const FLASH512_TAG: &[u8] = b"FLASH512_V";
const FLASH1M_TAG: &[u8] = b"FLASH1M_V";
const TRACE_LIMIT_DEFAULT: u32 = 64;

static TRACE_EEPROM_ENABLED: OnceLock<bool> = OnceLock::new();
static TRACE_LIMIT: OnceLock<u32> = OnceLock::new();
static TRACE_EEPROM_COUNT: AtomicU32 = AtomicU32::new(0);
static TRACE_EEPROM_DMA_ENABLED: OnceLock<bool> = OnceLock::new();
static TRACE_EEPROM_DMA_COUNT: AtomicU32 = AtomicU32::new(0);
static TRACE_MAIN_FLAGS_ENABLED: OnceLock<bool> = OnceLock::new();
static TRACE_MAIN_FLAGS_COUNT: AtomicU32 = AtomicU32::new(0);
static TRACE_SOUND_ENABLED: OnceLock<bool> = OnceLock::new();
static TRACE_SOUND_COUNT: AtomicU32 = AtomicU32::new(0);
static TRACE_SOUND_UNDERRUN_ENABLED: OnceLock<bool> = OnceLock::new();
static TRACE_SOUND_UNDERRUN_COUNT: AtomicU32 = AtomicU32::new(0);
static TRACE_FIFO_DMA_ENABLED: OnceLock<bool> = OnceLock::new();
static TRACE_FIFO_DMA_COUNT: AtomicU32 = AtomicU32::new(0);
static TRACE_IO_ENABLED: OnceLock<bool> = OnceLock::new();
static DIRECT_SOUND_INTERPOLATE: OnceLock<Option<bool>> = OnceLock::new();
static AUDIO_SANITIZE_FIFO_POINTER_WORDS: OnceLock<bool> = OnceLock::new();
static AUDIO_SANITIZE_FIFO_EXEC_PAGES: OnceLock<bool> = OnceLock::new();
static DISABLE_PSG: OnceLock<bool> = OnceLock::new();
static AUDIO_POST_FILTER_ENABLED: OnceLock<bool> = OnceLock::new();
static AUDIO_POST_FILTER_HPF_HZ: OnceLock<f32> = OnceLock::new();
static AUDIO_POST_FILTER_LPF_HZ: OnceLock<f32> = OnceLock::new();
static AUDIO_POST_FILTER_LPF_STAGES: OnceLock<u8> = OnceLock::new();
static AUDIO_FIFO_UNDERRUN_DECAY: OnceLock<bool> = OnceLock::new();
static AUDIO_NO_BIOS_SLEW_LIMIT: OnceLock<i32> = OnceLock::new();

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SaveType {
    None,
    Sram,
    Eeprom,
    Flash64K,
    Flash128K,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FlashVariant {
    None,
    Flash64K,
    Flash128K,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FlashMode {
    ReadArray,
    ReadId,
}

#[derive(Debug)]
struct EepromState {
    enabled: bool,
    addr_bits: Option<usize>,
    storage: Vec<u8>,
    command_bits: Vec<u8>,
    response_bits: Vec<u8>,
    response_index: usize,
    busy_reads: u8,
    busy_reads_config: u8,
    dma_write_hint_len: Option<usize>,
}

impl Default for EepromState {
    fn default() -> Self {
        Self {
            enabled: false,
            addr_bits: None,
            storage: vec![0xFF; EEPROM_512_SIZE],
            command_bits: Vec::new(),
            response_bits: Vec::new(),
            response_index: 0,
            busy_reads: 0,
            busy_reads_config: 0,
            dma_write_hint_len: None,
        }
    }
}

impl EepromState {
    fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        self.addr_bits = None;
        self.busy_reads = 0;
        self.busy_reads_config = std::env::var("GBA_EEPROM_BUSY_READS")
            .ok()
            .and_then(|value| value.trim().parse::<u8>().ok())
            .unwrap_or(0);
        if enabled {
            if let Ok(value) = std::env::var("GBA_EEPROM_ADDR_BITS") {
                let parsed = value.trim().parse::<usize>().ok();
                if matches!(parsed, Some(6 | 14)) {
                    self.configure_addr_bits(parsed.unwrap_or(6));
                }
            }
        }
        self.command_bits.clear();
        self.response_bits.clear();
        self.response_index = 0;
        self.busy_reads = 0;
        self.dma_write_hint_len = None;
    }

    fn reset_session(&mut self) {
        self.command_bits.clear();
        self.response_bits.clear();
        self.response_index = 0;
        self.dma_write_hint_len = None;
    }

    fn clear_storage(&mut self) {
        self.storage.fill(0xFF);
    }

    fn load_storage(&mut self, data: &[u8]) {
        if data.len() > EEPROM_512_SIZE {
            self.configure_addr_bits(14);
        } else {
            self.configure_addr_bits(6);
        }
        self.storage.fill(0xFF);
        let copy_len = data.len().min(self.storage.len());
        self.storage[..copy_len].copy_from_slice(&data[..copy_len]);
        self.reset_session();
    }

    fn storage_bytes(&self) -> Vec<u8> {
        self.storage.clone()
    }

    fn set_dma_write_hint(&mut self, bit_len: Option<usize>) {
        if bit_len.is_some() {
            // EEPROM commands are DMA-framed in most games. Reset stale partial state
            // so a new DMA block is parsed from bit 0.
            self.command_bits.clear();
        }
        self.dma_write_hint_len = bit_len.filter(|len| *len > 0);
    }

    fn read_bit(&mut self) -> u8 {
        if !self.enabled {
            return 1;
        }

        if self.response_index < self.response_bits.len() {
            let bit = self.response_bits[self.response_index];
            self.response_index += 1;
            return bit;
        }

        if self.busy_reads > 0 {
            self.busy_reads = self.busy_reads.saturating_sub(1);
            return 0;
        }

        1
    }

    fn write_bit(&mut self, bit: u8) {
        if !self.enabled {
            return;
        }

        let bit = bit & 1;
        if self.busy_reads > 0 {
            return;
        }
        if self.response_index < self.response_bits.len() {
            return;
        }

        self.command_bits.push(bit);
        if self.command_bits.len() == 1 {
            if self.command_bits[0] != 1 {
                self.command_bits.clear();
            }
            return;
        }

        if self.command_bits.len() > 81 {
            self.command_bits.clear();
            return;
        }

        match (self.command_bits[0] << 1) | self.command_bits[1] {
            0b10 => self.try_finalize_write_command(),
            0b11 => self.try_finalize_read_command(),
            _ => self.command_bits.clear(),
        }
    }

    fn try_finalize_write_command(&mut self) {
        let len = self.command_bits.len();
        if let Some(addr_bits) = self.addr_bits {
            let effective_addr_bits = self.effective_write_addr_bits(addr_bits);
            let expected = 2 + effective_addr_bits + 64 + 1;
            if len == expected {
                self.execute_write(effective_addr_bits);
            } else if len > expected {
                self.command_bits.clear();
            }
            return;
        }

        if let Some(addr_bits) = self.hinted_write_addr_bits() {
            let expected = 2 + addr_bits + 64 + 1;
            if len == expected {
                self.execute_write(addr_bits);
            } else if len > expected {
                self.command_bits.clear();
            }
            return;
        }

        if len == 73 {
            self.execute_write(6);
        } else if len == 81 {
            self.execute_write(14);
        }
    }

    fn try_finalize_read_command(&mut self) {
        let len = self.command_bits.len();
        if let Some(addr_bits) = self.addr_bits {
            let effective_addr_bits = self.effective_read_addr_bits(addr_bits);
            let expected = 2 + effective_addr_bits + 1;
            if len == expected {
                self.execute_read(effective_addr_bits);
            } else if len > expected {
                self.command_bits.clear();
            }
            return;
        }

        if let Some(addr_bits) = self.hinted_read_addr_bits() {
            let expected = 2 + addr_bits + 1;
            if len == expected {
                self.execute_read(addr_bits);
            } else if len > expected {
                self.command_bits.clear();
            }
            return;
        }

        if len == 9 {
            self.execute_read(6);
        } else if len == 17 {
            self.execute_read(14);
        }
    }

    fn execute_write(&mut self, addr_bits: usize) {
        self.configure_addr_bits(addr_bits);
        let address = bits_to_usize(&self.command_bits[2..(2 + addr_bits)]);
        let data_bits = &self.command_bits[(2 + addr_bits)..(2 + addr_bits + 64)];
        let block = self.map_block_index(address, addr_bits);
        let mut write_bytes = [0u8; 8];
        for (byte_index, slot) in write_bytes.iter_mut().enumerate() {
            let start = byte_index * 8;
            let end = start + 8;
            *slot = bits_to_u8(&data_bits[start..end]);
        }
        trace_eeprom_write(addr_bits, address, block, &write_bytes);
        let offset = block * 8;
        if offset + 8 <= self.storage.len() {
            for (byte_index, value) in write_bytes.iter().enumerate() {
                self.storage[offset + byte_index] = *value;
            }
        }
        self.command_bits.clear();
        self.busy_reads = self.busy_reads_config;
    }

    fn execute_read(&mut self, addr_bits: usize) {
        self.configure_addr_bits(addr_bits);
        let address = bits_to_usize(&self.command_bits[2..(2 + addr_bits)]);
        let block = self.map_block_index(address, addr_bits);
        let offset = block * 8;
        let mut read_bytes = [0xFFu8; 8];
        if offset + 8 <= self.storage.len() {
            read_bytes.copy_from_slice(&self.storage[offset..(offset + 8)]);
        }
        trace_eeprom_read(addr_bits, address, block, &read_bytes);

        self.response_bits.clear();
        self.response_bits.extend_from_slice(&[0, 0, 0, 0]);
        if offset + 8 <= self.storage.len() {
            for byte in &self.storage[offset..(offset + 8)] {
                for shift in (0..8).rev() {
                    self.response_bits.push((byte >> shift) & 1);
                }
            }
        } else {
            self.response_bits.extend(std::iter::repeat(1).take(64));
        }
        self.response_index = 0;
        self.command_bits.clear();
    }

    fn configure_addr_bits(&mut self, addr_bits: usize) {
        self.addr_bits = Some(addr_bits);
        if addr_bits > 6 && self.storage.len() != EEPROM_8K_SIZE {
            self.storage.resize(EEPROM_8K_SIZE, 0xFF);
        } else if addr_bits <= 6 && self.storage.len() < EEPROM_512_SIZE {
            self.storage.resize(EEPROM_512_SIZE, 0xFF);
        }
    }

    fn map_block_index(&self, address: usize, addr_bits: usize) -> usize {
        if self.storage.len() >= EEPROM_8K_SIZE {
            address & 0x03FF
        } else {
            address & ((1usize << addr_bits.min(6)) - 1)
        }
    }

    fn hinted_write_addr_bits(&self) -> Option<usize> {
        match self.dma_write_hint_len {
            Some(73) => Some(6),
            Some(81) => Some(14),
            _ => None,
        }
    }

    fn hinted_read_addr_bits(&self) -> Option<usize> {
        match self.dma_write_hint_len {
            Some(9) => Some(6),
            Some(17) => Some(14),
            _ => None,
        }
    }

    fn effective_write_addr_bits(&self, current: usize) -> usize {
        match (current, self.hinted_write_addr_bits()) {
            (6, Some(14)) => 14,
            _ => current,
        }
    }

    fn effective_read_addr_bits(&self, current: usize) -> usize {
        match (current, self.hinted_read_addr_bits()) {
            (6, Some(14)) => 14,
            _ => current,
        }
    }
}

#[derive(Debug)]
struct FlashState {
    variant: FlashVariant,
    mode: FlashMode,
    unlock_stage: u8,
    erase_armed: bool,
    program_armed: bool,
    bank_switch_armed: bool,
    bank: u8,
}

impl Default for FlashState {
    fn default() -> Self {
        Self {
            variant: FlashVariant::None,
            mode: FlashMode::ReadArray,
            unlock_stage: 0,
            erase_armed: false,
            program_armed: false,
            bank_switch_armed: false,
            bank: 0,
        }
    }
}

impl FlashState {
    fn configure(&mut self, variant: FlashVariant) {
        self.variant = variant;
        self.mode = FlashMode::ReadArray;
        self.unlock_stage = 0;
        self.erase_armed = false;
        self.program_armed = false;
        self.bank_switch_armed = false;
        self.bank = 0;
    }

    fn reset_session(&mut self) {
        self.mode = FlashMode::ReadArray;
        self.unlock_stage = 0;
        self.erase_armed = false;
        self.program_armed = false;
        self.bank_switch_armed = false;
        self.bank = 0;
    }

    fn manufacturer_id(&self) -> u8 {
        match self.variant {
            FlashVariant::Flash128K => 0x62,
            FlashVariant::Flash64K => 0x32,
            FlashVariant::None => 0xFF,
        }
    }

    fn device_id(&self) -> u8 {
        match self.variant {
            FlashVariant::Flash128K => 0x13,
            FlashVariant::Flash64K => 0x1B,
            FlashVariant::None => 0xFF,
        }
    }

    fn is_enabled(&self) -> bool {
        self.variant != FlashVariant::None
    }

    fn max_banks(&self) -> u8 {
        match self.variant {
            FlashVariant::Flash128K => 2,
            FlashVariant::Flash64K => 1,
            FlashVariant::None => 0,
        }
    }
}

const REG_DISPSTAT: usize = 0x0004;
const REG_VCOUNT: usize = 0x0006;
const REG_VCOUNT_HI: usize = REG_VCOUNT + 1;
const REG_SOUND1CNT_L: usize = 0x0060;
const REG_SOUND1CNT_H: usize = 0x0062;
const REG_SOUND1CNT_X: usize = 0x0064;
const REG_SOUND2CNT_L: usize = 0x0068;
const REG_SOUND2CNT_H: usize = 0x006C;
const REG_SOUND3CNT_L: usize = 0x0070;
const REG_SOUND3CNT_H: usize = 0x0072;
const REG_SOUND3CNT_X: usize = 0x0074;
const REG_SOUND4CNT_L: usize = 0x0078;
const REG_SOUND4CNT_H: usize = 0x007C;
const REG_SOUNDCNT_L: usize = 0x0080;
const REG_KEYINPUT: usize = 0x0130;
const REG_KEYINPUT_HI: usize = REG_KEYINPUT + 1;
const REG_KEYCNT: usize = 0x0132;
const REG_KEYCNT_HI: usize = REG_KEYCNT + 1;
const REG_IE: usize = 0x0200;
const REG_IF: usize = 0x0202;
const REG_IF_HI: usize = REG_IF + 1;
const REG_IME: usize = 0x0208;
const REG_IME_HI: usize = REG_IME + 1;
const REG_POSTFLG: usize = 0x0300;
const REG_SOUNDCNT_H: usize = 0x0082;
const REG_SOUNDCNT_X: usize = 0x0084;
const REG_SOUNDBIAS: usize = 0x0088;

const TIMER_COUNT: usize = 4;
const TIMER_BASE: usize = 0x0100;
const TIMER_STRIDE: usize = 4;
const TIMER_CTRL_OFFSET: usize = 2;

const DMA_SRC_OFFSETS: [usize; 4] = [0x00B0, 0x00BC, 0x00C8, 0x00D4];
const DMA_DST_OFFSETS: [usize; 4] = [0x00B4, 0x00C0, 0x00CC, 0x00D8];
const DMA_COUNT_OFFSETS: [usize; 4] = [0x00B8, 0x00C4, 0x00D0, 0x00DC];
const DMA_CTRL_OFFSETS: [usize; 4] = [0x00BA, 0x00C6, 0x00D2, 0x00DE];

const DMA_ENABLE: u16 = 1 << 15;
const DMA_IRQ_ENABLE: u16 = 1 << 14;
const DMA_TIMING_SHIFT: u16 = 12;
const DMA_TRANSFER_32BIT: u16 = 1 << 10;
const DMA_REPEAT: u16 = 1 << 9;
const DMA_DEST_MODE_SHIFT: u16 = 5;
const DMA_SOURCE_MODE_SHIFT: u16 = 7;

const DMA_TIMING_IMMEDIATE: u16 = 0;
const DMA_TIMING_VBLANK: u16 = 1;
const DMA_TIMING_HBLANK: u16 = 2;
const DMA_TIMING_SPECIAL: u16 = 3;

const FIFO_A_ADDR: u32 = 0x0400_00A0;
const FIFO_B_ADDR: u32 = 0x0400_00A4;
const SOUND_FIFO_CAPACITY: usize = 32;
// Hold the last latched sample briefly on FIFO underrun to absorb short DMA
// jitter without introducing zipper-like crackle.
const SOUND_FIFO_UNDERRUN_HOLD_SAMPLES: u16 = 64;
const AUDIO_SAMPLE_BUFFER_LIMIT: usize = 262_144;
const WAVE_RAM_START: usize = 0x0090;
const GBA_MASTER_CLOCK_HZ: u32 = 16_777_216;
const AUDIO_BASE_RATE_HZ_U32: u32 = 32_768;
const AUDIO_BASE_CYCLES_PER_SAMPLE: u32 = 512; // 16_777_216 / 32_768
const TIMER_PRESCALER_CYCLES: [u32; 4] = [1, 64, 256, 1024];
#[cfg(test)]
const PSG_FRAME_SEQ_TICK_SAMPLES: u16 = 64; // 32_768 / 512
const PSG_FRAME_SEQ_HZ: f32 = 512.0;
const PSG_NOISE_PHASE_BITS: u32 = 24;
const PSG_NOISE_PHASE_ONE: u32 = 1 << PSG_NOISE_PHASE_BITS;
const SOUND_BIAS_LEVEL_MASK: u16 = 0x03FF;
const SOUND_DAC_MIN: i32 = 0;
const SOUND_DAC_MAX: i32 = 0x03FF;
// Map centered 10-bit DAC units to signed 16-bit PCM.
const SOUND_PCM_SHIFT: u32 = 6;
// Direct Sound FIFO is signed 8-bit PCM. Route volume in SOUNDCNT_H is
// 50% or 100% of the base Direct Sound amplitude.
// Keep base gain conservative to avoid persistent clipping when both FIFO A/B
// are routed simultaneously (common in stereo engines such as Tetris Worlds).
const DIRECT_SOUND_SCALE_FULL: i16 = 2;
const DIRECT_SOUND_SCALE_HALF: i16 = 1;
const PSG_CHANNEL_FULL_SCALE: f32 = 128.0;
const AUDIO_POST_FILTER_MAX_STAGES: usize = 4;
const AUDIO_POST_FILTER_HPF_HZ_DEFAULT: f32 = 20.0;
const AUDIO_POST_FILTER_LPF_HZ_DEFAULT: f32 = 14_000.0;
const AUDIO_POST_FILTER_LPF_STAGES_DEFAULT: u8 = 1;
#[cfg(not(test))]
const AUDIO_NO_BIOS_SLEW_LIMIT_DEFAULT: i32 = 2048;

pub const IRQ_VBLANK: u16 = 1 << 0;
pub const IRQ_HBLANK: u16 = 1 << 1;
pub const IRQ_VCOUNT: u16 = 1 << 2;
pub const IRQ_TIMER0: u16 = 1 << 3;
pub const IRQ_TIMER1: u16 = 1 << 4;
pub const IRQ_TIMER2: u16 = 1 << 5;
pub const IRQ_TIMER3: u16 = 1 << 6;
pub const IRQ_KEYPAD: u16 = 1 << 12;

#[derive(Debug)]
pub struct GbaBus {
    bios: Vec<u8>,
    bios_loaded: bool,
    rom: Vec<u8>,
    rom_len: usize,
    rom_pow2: bool,
    rom_mask: usize,
    save_type: SaveType,
    eeprom: RefCell<EepromState>,
    flash: FlashState,
    ewram: Vec<u8>,
    iwram: Vec<u8>,
    io: Vec<u8>,
    wave_ram: [u8; 32],
    pram: Vec<u8>,
    vram: Vec<u8>,
    oam: Vec<u8>,
    sram: Vec<u8>,
    iwram_exec_bitmap: [u64; IWRAM_EXEC_BITMAP_WORDS],
    timer_reload: [u16; TIMER_COUNT],
    timer_counter: [u16; TIMER_COUNT],
    timer_control: [u16; TIMER_COUNT],
    fifo_a: VecDeque<i8>,
    fifo_b: VecDeque<i8>,
    last_fifo_a: i8,
    last_fifo_b: i8,
    fifo_a_underrun_streak: u16,
    fifo_b_underrun_streak: u16,
    direct_sound_a_latch: i16,
    direct_sound_b_latch: i16,
    direct_sound_a_prev_latch: i16,
    direct_sound_b_prev_latch: i16,
    direct_sound_a_cycles_since_latch: u32,
    direct_sound_b_cycles_since_latch: u32,
    direct_sound_a_latch_period_cycles: u32,
    direct_sound_b_latch_period_cycles: u32,
    audio_cycle_accum: u32,
    audio_samples: Vec<i16>,
    audio_post_filter_enabled: bool,
    audio_post_hpf_alpha: f32,
    audio_post_lpf_alpha: f32,
    audio_post_lpf_stages: u8,
    audio_post_filter_rate_hz: u32,
    audio_post_hpf_prev_in_l: f32,
    audio_post_hpf_prev_in_r: f32,
    audio_post_hpf_prev_out_l: f32,
    audio_post_hpf_prev_out_r: f32,
    audio_post_lpf_l: [f32; AUDIO_POST_FILTER_MAX_STAGES],
    audio_post_lpf_r: [f32; AUDIO_POST_FILTER_MAX_STAGES],
    psg_square1_phase: f32,
    psg_square2_phase: f32,
    psg_square1_on: bool,
    psg_square2_on: bool,
    psg_wave_phase: f32,
    psg_noise_phase: u32,
    psg_wave_on: bool,
    psg_wave_play_bank: u8,
    psg_wave_last_sample_index: u8,
    psg_wave_pending_writes: [Option<u8>; 32],
    psg_noise_on: bool,
    psg_noise_lfsr: u16,
    psg_frame_seq_accum: f32,
    psg_frame_seq_step: u8,
    psg_square1_length_ticks: u16,
    psg_square2_length_ticks: u16,
    psg_wave_length_ticks: u16,
    psg_noise_length_ticks: u16,
    psg_square1_shadow_freq: u16,
    psg_square1_sweep_counter: u8,
    psg_square1_volume: u8,
    psg_square2_volume: u8,
    psg_noise_volume: u8,
    psg_square1_env_period: u8,
    psg_square2_env_period: u8,
    psg_noise_env_period: u8,
    psg_square1_env_counter: u8,
    psg_square2_env_counter: u8,
    psg_noise_env_counter: u8,
    dma_internal_src: [u32; 4],
    dma_internal_dst: [u32; 4],
    dma_active: [bool; 4],
    scanline_io: Box<[[u8; SCANLINE_IO_SNAPSHOT_SIZE]; GBA_VISIBLE_LINES]>,
    scanline_io_valid: Box<[bool; GBA_VISIBLE_LINES]>,
    scanline_pram: Box<[[u8; PRAM_SIZE]; GBA_VISIBLE_LINES]>,
    scanline_pram_valid: Box<[bool; GBA_VISIBLE_LINES]>,
    scanline_bg_bitmap_vram: Vec<u8>,
    scanline_bg_bitmap_vram_valid: Box<[bool; GBA_VISIBLE_LINES]>,
    scanline_obj_vram: Vec<u8>,
    scanline_obj_vram_valid: Box<[bool; GBA_VISIBLE_LINES]>,
    scanline_oam: Box<[[u8; OAM_SIZE]; GBA_VISIBLE_LINES]>,
    scanline_oam_valid: Box<[bool; GBA_VISIBLE_LINES]>,
    pub(crate) pram_snapshot: Vec<u8>,
    pub(crate) vram_snapshot: Vec<u8>,
    pub(crate) oam_snapshot: Vec<u8>,
    render_snapshot_valid: bool,
}

impl Default for GbaBus {
    fn default() -> Self {
        let mut bus = Self {
            bios: vec![0; BIOS_SIZE],
            bios_loaded: false,
            rom: Vec::new(),
            rom_len: 0,
            rom_pow2: false,
            rom_mask: 0,
            save_type: SaveType::None,
            eeprom: RefCell::new(EepromState::default()),
            flash: FlashState::default(),
            ewram: vec![0; EWRAM_SIZE],
            iwram: vec![0; IWRAM_SIZE],
            io: vec![0; IO_SIZE],
            wave_ram: [0; 32],
            pram: vec![0; PRAM_SIZE],
            vram: vec![0; VRAM_SIZE],
            oam: vec![0; OAM_SIZE],
            sram: vec![0xFF; SRAM_SIZE],
            iwram_exec_bitmap: [0; IWRAM_EXEC_BITMAP_WORDS],
            timer_reload: [0; TIMER_COUNT],
            timer_counter: [0; TIMER_COUNT],
            timer_control: [0; TIMER_COUNT],
            fifo_a: VecDeque::with_capacity(SOUND_FIFO_CAPACITY),
            fifo_b: VecDeque::with_capacity(SOUND_FIFO_CAPACITY),
            last_fifo_a: 0,
            last_fifo_b: 0,
            fifo_a_underrun_streak: 0,
            fifo_b_underrun_streak: 0,
            direct_sound_a_latch: 0,
            direct_sound_b_latch: 0,
            direct_sound_a_prev_latch: 0,
            direct_sound_b_prev_latch: 0,
            direct_sound_a_cycles_since_latch: 0,
            direct_sound_b_cycles_since_latch: 0,
            direct_sound_a_latch_period_cycles: 1,
            direct_sound_b_latch_period_cycles: 1,
            audio_cycle_accum: 0,
            audio_samples: Vec::new(),
            audio_post_filter_enabled: false,
            audio_post_hpf_alpha: 0.0,
            audio_post_lpf_alpha: 0.0,
            audio_post_lpf_stages: 0,
            audio_post_filter_rate_hz: AUDIO_BASE_RATE_HZ_U32,
            audio_post_hpf_prev_in_l: 0.0,
            audio_post_hpf_prev_in_r: 0.0,
            audio_post_hpf_prev_out_l: 0.0,
            audio_post_hpf_prev_out_r: 0.0,
            audio_post_lpf_l: [0.0; AUDIO_POST_FILTER_MAX_STAGES],
            audio_post_lpf_r: [0.0; AUDIO_POST_FILTER_MAX_STAGES],
            psg_square1_phase: 0.0,
            psg_square2_phase: 0.0,
            psg_square1_on: false,
            psg_square2_on: false,
            psg_wave_phase: 0.0,
            psg_noise_phase: 0,
            psg_wave_on: false,
            psg_wave_play_bank: 0,
            psg_wave_last_sample_index: 0xFF,
            psg_wave_pending_writes: [None; 32],
            psg_noise_on: false,
            psg_noise_lfsr: 0x7FFF,
            psg_frame_seq_accum: 0.0,
            psg_frame_seq_step: 0,
            psg_square1_length_ticks: 0,
            psg_square2_length_ticks: 0,
            psg_wave_length_ticks: 0,
            psg_noise_length_ticks: 0,
            psg_square1_shadow_freq: 0,
            psg_square1_sweep_counter: 0,
            psg_square1_volume: 0,
            psg_square2_volume: 0,
            psg_noise_volume: 0,
            psg_square1_env_period: 0,
            psg_square2_env_period: 0,
            psg_noise_env_period: 0,
            psg_square1_env_counter: 0,
            psg_square2_env_counter: 0,
            psg_noise_env_counter: 0,
            dma_internal_src: [0; 4],
            dma_internal_dst: [0; 4],
            dma_active: [false; 4],
            scanline_io: Box::new([[0u8; SCANLINE_IO_SNAPSHOT_SIZE]; GBA_VISIBLE_LINES]),
            scanline_io_valid: Box::new([false; GBA_VISIBLE_LINES]),
            scanline_pram: Box::new([[0u8; PRAM_SIZE]; GBA_VISIBLE_LINES]),
            scanline_pram_valid: Box::new([false; GBA_VISIBLE_LINES]),
            scanline_bg_bitmap_vram: vec![0; BG_BITMAP_VRAM_SNAPSHOT_SIZE * GBA_VISIBLE_LINES],
            scanline_bg_bitmap_vram_valid: Box::new([false; GBA_VISIBLE_LINES]),
            scanline_obj_vram: vec![0; OBJ_VRAM_SNAPSHOT_SIZE * GBA_VISIBLE_LINES],
            scanline_obj_vram_valid: Box::new([false; GBA_VISIBLE_LINES]),
            scanline_oam: Box::new([[0u8; OAM_SIZE]; GBA_VISIBLE_LINES]),
            scanline_oam_valid: Box::new([false; GBA_VISIBLE_LINES]),
            pram_snapshot: vec![0; PRAM_SIZE],
            vram_snapshot: vec![0; VRAM_SIZE],
            oam_snapshot: vec![0; OAM_SIZE],
            render_snapshot_valid: false,
        };
        bus.refresh_audio_post_filter_config();
        bus.set_oam_default_hidden();
        bus
    }
}

impl GbaBus {
    pub fn load_rom(&mut self, rom: &[u8]) {
        self.rom.clear();
        self.rom.extend_from_slice(rom);
        self.rom_len = self.rom.len();
        self.rom_pow2 = self.rom_len.is_power_of_two();
        self.rom_mask = if self.rom_pow2 && self.rom_len != 0 {
            self.rom_len - 1
        } else {
            0
        };
        self.save_type = save_type_override_from_env().unwrap_or_else(|| detect_save_type(rom));
        self.configure_save_hardware();
        self.sram.fill(0xFF);
        self.iwram_exec_bitmap.fill(0);
        self.eeprom.borrow_mut().clear_storage();
        self.wave_ram = [0; 32];
        self.fifo_a.clear();
        self.fifo_b.clear();
        self.last_fifo_a = 0;
        self.last_fifo_b = 0;
        self.fifo_a_underrun_streak = 0;
        self.fifo_b_underrun_streak = 0;
        self.direct_sound_a_latch = 0;
        self.direct_sound_b_latch = 0;
        self.direct_sound_a_prev_latch = 0;
        self.direct_sound_b_prev_latch = 0;
        self.direct_sound_a_cycles_since_latch = 0;
        self.direct_sound_b_cycles_since_latch = 0;
        self.direct_sound_a_latch_period_cycles = 1;
        self.direct_sound_b_latch_period_cycles = 1;
        self.audio_cycle_accum = 0;
        self.audio_samples.clear();
        self.reset_audio_post_filter_state();
        self.psg_square1_phase = 0.0;
        self.psg_square2_phase = 0.0;
        self.psg_square1_on = false;
        self.psg_square2_on = false;
        self.psg_wave_phase = 0.0;
        self.psg_noise_phase = 0;
        self.psg_wave_on = false;
        self.psg_wave_play_bank = 0;
        self.psg_wave_last_sample_index = 0xFF;
        self.psg_wave_pending_writes = [None; 32];
        self.psg_noise_on = false;
        self.psg_noise_lfsr = 0x7FFF;
        self.psg_frame_seq_accum = 0.0;
        self.psg_frame_seq_step = 0;
        self.psg_square1_length_ticks = 0;
        self.psg_square2_length_ticks = 0;
        self.psg_wave_length_ticks = 0;
        self.psg_noise_length_ticks = 0;
        self.psg_square1_shadow_freq = 0;
        self.psg_square1_sweep_counter = 0;
        self.psg_square1_volume = 0;
        self.psg_square2_volume = 0;
        self.psg_noise_volume = 0;
        self.psg_square1_env_period = 0;
        self.psg_square2_env_period = 0;
        self.psg_noise_env_period = 0;
        self.psg_square1_env_counter = 0;
        self.psg_square2_env_counter = 0;
        self.psg_noise_env_counter = 0;
        self.refresh_audio_post_filter_config();
    }

    pub fn load_bios(&mut self, bios: &[u8]) {
        self.bios.clear();
        self.bios.extend_from_slice(bios);
        if self.bios.len() < BIOS_SIZE {
            self.bios.resize(BIOS_SIZE, 0);
        }
        self.bios_loaded = !bios.is_empty();
        self.refresh_audio_post_filter_config();
    }

    pub fn has_bios(&self) -> bool {
        self.bios_loaded
    }

    pub fn rom_bytes(&self) -> &[u8] {
        &self.rom
    }

    pub(crate) fn maybe_upgrade_legacy_state_payload(
        payload: &[u8],
        current_payload_len: usize,
    ) -> Option<Vec<u8>> {
        let insert_len = current_payload_len.checked_sub(payload.len())?;
        if insert_len.abs_diff(LEGACY_MISSING_SCANLINE_SNAPSHOT_SIZE)
            > LEGACY_SCANLINE_SNAPSHOT_TOLERANCE
        {
            return None;
        }

        let insert_at = legacy_scanline_snapshot_insert_offset(payload).ok()?;
        let mut upgraded = Vec::with_capacity(current_payload_len);
        upgraded.extend_from_slice(&payload[..insert_at]);
        upgraded.resize(insert_at + insert_len, 0);
        upgraded.extend_from_slice(&payload[insert_at..]);
        if upgraded.len() < current_payload_len {
            upgraded.resize(current_payload_len, 0);
        }
        Some(upgraded)
    }

    pub fn serialize_state(&self, w: &mut crate::state::StateWriter) {
        // 1. Memory regions
        w.write_slice(&self.ewram);
        w.write_slice(&self.iwram);
        w.write_slice(&self.io);
        w.write_slice(&self.wave_ram);
        w.write_slice(&self.pram);
        w.write_slice(&self.vram);
        w.write_slice(&self.oam);
        w.write_slice(&self.sram);

        // 2. iwram_exec_bitmap
        for &word in &self.iwram_exec_bitmap {
            w.write_u64(word);
        }

        // 3. Timer registers
        for &v in &self.timer_reload {
            w.write_u16(v);
        }
        for &v in &self.timer_counter {
            w.write_u16(v);
        }
        for &v in &self.timer_control {
            w.write_u16(v);
        }

        // 4. FIFO (length-prefixed)
        w.write_u32(self.fifo_a.len() as u32);
        for &s in &self.fifo_a {
            w.write_i8(s);
        }
        w.write_u32(self.fifo_b.len() as u32);
        for &s in &self.fifo_b {
            w.write_i8(s);
        }

        // 5. Audio scalars
        w.write_i8(self.last_fifo_a);
        w.write_i8(self.last_fifo_b);
        w.write_u16(self.fifo_a_underrun_streak);
        w.write_u16(self.fifo_b_underrun_streak);
        w.write_i16(self.direct_sound_a_latch);
        w.write_i16(self.direct_sound_b_latch);
        w.write_i16(self.direct_sound_a_prev_latch);
        w.write_i16(self.direct_sound_b_prev_latch);
        w.write_u32(self.direct_sound_a_cycles_since_latch);
        w.write_u32(self.direct_sound_b_cycles_since_latch);
        w.write_u32(self.direct_sound_a_latch_period_cycles);
        w.write_u32(self.direct_sound_b_latch_period_cycles);
        w.write_u32(self.audio_cycle_accum);

        // PSG state
        w.write_f32(self.psg_square1_phase);
        w.write_f32(self.psg_square2_phase);
        w.write_bool(self.psg_square1_on);
        w.write_bool(self.psg_square2_on);
        w.write_f32(self.psg_wave_phase);
        w.write_u32(self.psg_noise_phase);
        w.write_bool(self.psg_wave_on);
        w.write_u8(self.psg_wave_play_bank);
        w.write_u8(self.psg_wave_last_sample_index);
        for &opt in &self.psg_wave_pending_writes {
            w.write_bool(opt.is_some());
            w.write_u8(opt.unwrap_or(0));
        }
        w.write_bool(self.psg_noise_on);
        w.write_u16(self.psg_noise_lfsr);
        w.write_f32(self.psg_frame_seq_accum);
        w.write_u8(self.psg_frame_seq_step);
        w.write_u16(self.psg_square1_length_ticks);
        w.write_u16(self.psg_square2_length_ticks);
        w.write_u16(self.psg_wave_length_ticks);
        w.write_u16(self.psg_noise_length_ticks);
        w.write_u16(self.psg_square1_shadow_freq);
        w.write_u8(self.psg_square1_sweep_counter);
        w.write_u8(self.psg_square1_volume);
        w.write_u8(self.psg_square2_volume);
        w.write_u8(self.psg_noise_volume);
        w.write_u8(self.psg_square1_env_period);
        w.write_u8(self.psg_square2_env_period);
        w.write_u8(self.psg_noise_env_period);
        w.write_u8(self.psg_square1_env_counter);
        w.write_u8(self.psg_square2_env_counter);
        w.write_u8(self.psg_noise_env_counter);

        // 6. Audio post-filter state (not config — just running state)
        w.write_f32(self.audio_post_hpf_prev_in_l);
        w.write_f32(self.audio_post_hpf_prev_in_r);
        w.write_f32(self.audio_post_hpf_prev_out_l);
        w.write_f32(self.audio_post_hpf_prev_out_r);
        for &v in &self.audio_post_lpf_l {
            w.write_f32(v);
        }
        for &v in &self.audio_post_lpf_r {
            w.write_f32(v);
        }

        // 7. DMA internal state
        for &v in &self.dma_internal_src {
            w.write_u32(v);
        }
        for &v in &self.dma_internal_dst {
            w.write_u32(v);
        }
        for &v in &self.dma_active {
            w.write_bool(v);
        }

        // 8. Scanline IO snapshots
        for row in self.scanline_io.iter() {
            w.write_slice(row);
        }
        for &valid in self.scanline_io_valid.iter() {
            w.write_bool(valid);
        }
        for row in self.scanline_pram.iter() {
            w.write_slice(row);
        }
        for &valid in self.scanline_pram_valid.iter() {
            w.write_bool(valid);
        }
        for line in 0..GBA_VISIBLE_LINES {
            let start = line * BG_BITMAP_VRAM_SNAPSHOT_SIZE;
            let end = start + BG_BITMAP_VRAM_SNAPSHOT_SIZE;
            w.write_slice(&self.scanline_bg_bitmap_vram[start..end]);
        }
        for &valid in self.scanline_bg_bitmap_vram_valid.iter() {
            w.write_bool(valid);
        }
        for line in 0..GBA_VISIBLE_LINES {
            let start = line * OBJ_VRAM_SNAPSHOT_SIZE;
            let end = start + OBJ_VRAM_SNAPSHOT_SIZE;
            w.write_slice(&self.scanline_obj_vram[start..end]);
        }
        for &valid in self.scanline_obj_vram_valid.iter() {
            w.write_bool(valid);
        }
        for row in self.scanline_oam.iter() {
            w.write_slice(row);
        }
        for &valid in self.scanline_oam_valid.iter() {
            w.write_bool(valid);
        }

        // 9. Render snapshots
        w.write_vec_u8(&self.pram_snapshot);
        w.write_vec_u8(&self.vram_snapshot);
        w.write_vec_u8(&self.oam_snapshot);
        w.write_bool(self.render_snapshot_valid);

        // 10. Save type + EEPROM + Flash
        let save_type_tag: u8 = match self.save_type {
            SaveType::None => 0,
            SaveType::Sram => 1,
            SaveType::Eeprom => 2,
            SaveType::Flash64K => 3,
            SaveType::Flash128K => 4,
        };
        w.write_u8(save_type_tag);

        // EEPROM state
        let eeprom = self.eeprom.borrow();
        w.write_bool(eeprom.enabled);
        w.write_bool(eeprom.addr_bits.is_some());
        w.write_u32(eeprom.addr_bits.unwrap_or(0) as u32);
        w.write_vec_u8(&eeprom.storage);
        w.write_vec_u8(&eeprom.command_bits.iter().map(|&b| b).collect::<Vec<u8>>());
        w.write_vec_u8(&eeprom.response_bits.iter().map(|&b| b).collect::<Vec<u8>>());
        w.write_u32(eeprom.response_index as u32);
        w.write_u8(eeprom.busy_reads);
        w.write_u8(eeprom.busy_reads_config);
        w.write_bool(eeprom.dma_write_hint_len.is_some());
        w.write_u32(eeprom.dma_write_hint_len.unwrap_or(0) as u32);
        drop(eeprom);

        // Flash state
        let flash_variant_tag: u8 = match self.flash.variant {
            FlashVariant::None => 0,
            FlashVariant::Flash64K => 1,
            FlashVariant::Flash128K => 2,
        };
        w.write_u8(flash_variant_tag);
        let flash_mode_tag: u8 = match self.flash.mode {
            FlashMode::ReadArray => 0,
            FlashMode::ReadId => 1,
        };
        w.write_u8(flash_mode_tag);
        w.write_u8(self.flash.unlock_stage);
        w.write_bool(self.flash.erase_armed);
        w.write_bool(self.flash.program_armed);
        w.write_bool(self.flash.bank_switch_armed);
        w.write_u8(self.flash.bank);
    }

    pub fn deserialize_state(
        &mut self,
        r: &mut crate::state::StateReader,
    ) -> Result<(), &'static str> {
        // 1. Memory regions
        r.read_into_slice(&mut self.ewram)?;
        r.read_into_slice(&mut self.iwram)?;
        r.read_into_slice(&mut self.io)?;
        r.read_into_slice(&mut self.wave_ram)?;
        r.read_into_slice(&mut self.pram)?;
        r.read_into_slice(&mut self.vram)?;
        r.read_into_slice(&mut self.oam)?;
        r.read_into_slice(&mut self.sram)?;

        // 2. iwram_exec_bitmap
        for word in &mut self.iwram_exec_bitmap {
            *word = r.read_u64()?;
        }

        // 3. Timer registers
        for v in &mut self.timer_reload {
            *v = r.read_u16()?;
        }
        for v in &mut self.timer_counter {
            *v = r.read_u16()?;
        }
        for v in &mut self.timer_control {
            *v = r.read_u16()?;
        }

        // 4. FIFO
        let fifo_a_len = r.read_u32()? as usize;
        self.fifo_a.clear();
        for _ in 0..fifo_a_len {
            self.fifo_a.push_back(r.read_i8()?);
        }
        let fifo_b_len = r.read_u32()? as usize;
        self.fifo_b.clear();
        for _ in 0..fifo_b_len {
            self.fifo_b.push_back(r.read_i8()?);
        }

        // 5. Audio scalars
        self.last_fifo_a = r.read_i8()?;
        self.last_fifo_b = r.read_i8()?;
        self.fifo_a_underrun_streak = r.read_u16()?;
        self.fifo_b_underrun_streak = r.read_u16()?;
        self.direct_sound_a_latch = r.read_i16()?;
        self.direct_sound_b_latch = r.read_i16()?;
        self.direct_sound_a_prev_latch = r.read_i16()?;
        self.direct_sound_b_prev_latch = r.read_i16()?;
        self.direct_sound_a_cycles_since_latch = r.read_u32()?;
        self.direct_sound_b_cycles_since_latch = r.read_u32()?;
        self.direct_sound_a_latch_period_cycles = r.read_u32()?;
        self.direct_sound_b_latch_period_cycles = r.read_u32()?;
        self.audio_cycle_accum = r.read_u32()?;

        // PSG state
        self.psg_square1_phase = r.read_f32()?;
        self.psg_square2_phase = r.read_f32()?;
        self.psg_square1_on = r.read_bool()?;
        self.psg_square2_on = r.read_bool()?;
        self.psg_wave_phase = r.read_f32()?;
        self.psg_noise_phase = r.read_u32()?;
        self.psg_wave_on = r.read_bool()?;
        self.psg_wave_play_bank = r.read_u8()?;
        self.psg_wave_last_sample_index = r.read_u8()?;
        for slot in &mut self.psg_wave_pending_writes {
            let has = r.read_bool()?;
            let val = r.read_u8()?;
            *slot = if has { Some(val) } else { None };
        }
        self.psg_noise_on = r.read_bool()?;
        self.psg_noise_lfsr = r.read_u16()?;
        self.psg_frame_seq_accum = r.read_f32()?;
        self.psg_frame_seq_step = r.read_u8()?;
        self.psg_square1_length_ticks = r.read_u16()?;
        self.psg_square2_length_ticks = r.read_u16()?;
        self.psg_wave_length_ticks = r.read_u16()?;
        self.psg_noise_length_ticks = r.read_u16()?;
        self.psg_square1_shadow_freq = r.read_u16()?;
        self.psg_square1_sweep_counter = r.read_u8()?;
        self.psg_square1_volume = r.read_u8()?;
        self.psg_square2_volume = r.read_u8()?;
        self.psg_noise_volume = r.read_u8()?;
        self.psg_square1_env_period = r.read_u8()?;
        self.psg_square2_env_period = r.read_u8()?;
        self.psg_noise_env_period = r.read_u8()?;
        self.psg_square1_env_counter = r.read_u8()?;
        self.psg_square2_env_counter = r.read_u8()?;
        self.psg_noise_env_counter = r.read_u8()?;

        // 6. Audio post-filter state
        self.audio_post_hpf_prev_in_l = r.read_f32()?;
        self.audio_post_hpf_prev_in_r = r.read_f32()?;
        self.audio_post_hpf_prev_out_l = r.read_f32()?;
        self.audio_post_hpf_prev_out_r = r.read_f32()?;
        for v in &mut self.audio_post_lpf_l {
            *v = r.read_f32()?;
        }
        for v in &mut self.audio_post_lpf_r {
            *v = r.read_f32()?;
        }

        // 7. DMA internal state
        for v in &mut self.dma_internal_src {
            *v = r.read_u32()?;
        }
        for v in &mut self.dma_internal_dst {
            *v = r.read_u32()?;
        }
        for v in &mut self.dma_active {
            *v = r.read_bool()?;
        }

        // 8. Scanline IO snapshots
        for row in self.scanline_io.iter_mut() {
            r.read_into_slice(row)?;
        }
        for valid in self.scanline_io_valid.iter_mut() {
            *valid = r.read_bool()?;
        }
        for row in self.scanline_pram.iter_mut() {
            r.read_into_slice(row)?;
        }
        for valid in self.scanline_pram_valid.iter_mut() {
            *valid = r.read_bool()?;
        }
        for line in 0..GBA_VISIBLE_LINES {
            let start = line * BG_BITMAP_VRAM_SNAPSHOT_SIZE;
            let end = start + BG_BITMAP_VRAM_SNAPSHOT_SIZE;
            r.read_into_slice(&mut self.scanline_bg_bitmap_vram[start..end])?;
        }
        for valid in self.scanline_bg_bitmap_vram_valid.iter_mut() {
            *valid = r.read_bool()?;
        }
        for line in 0..GBA_VISIBLE_LINES {
            let start = line * OBJ_VRAM_SNAPSHOT_SIZE;
            let end = start + OBJ_VRAM_SNAPSHOT_SIZE;
            r.read_into_slice(&mut self.scanline_obj_vram[start..end])?;
        }
        for valid in self.scanline_obj_vram_valid.iter_mut() {
            *valid = r.read_bool()?;
        }
        for row in self.scanline_oam.iter_mut() {
            r.read_into_slice(row)?;
        }
        for valid in self.scanline_oam_valid.iter_mut() {
            *valid = r.read_bool()?;
        }

        // 9. Render snapshots
        self.pram_snapshot = r.read_vec_u8()?;
        self.vram_snapshot = r.read_vec_u8()?;
        self.oam_snapshot = r.read_vec_u8()?;
        self.render_snapshot_valid = r.read_bool()?;
        self.normalize_render_snapshots_after_deserialize();

        // 10. Save type + EEPROM + Flash
        self.save_type = match r.read_u8()? {
            0 => SaveType::None,
            1 => SaveType::Sram,
            2 => SaveType::Eeprom,
            3 => SaveType::Flash64K,
            4 => SaveType::Flash128K,
            _ => return Err("invalid save type in state"),
        };

        // EEPROM state
        let mut eeprom = self.eeprom.borrow_mut();
        eeprom.enabled = r.read_bool()?;
        let has_addr_bits = r.read_bool()?;
        let addr_bits_val = r.read_u32()? as usize;
        eeprom.addr_bits = if has_addr_bits {
            Some(addr_bits_val)
        } else {
            None
        };
        eeprom.storage = r.read_vec_u8()?;
        eeprom.command_bits = r.read_vec_u8()?;
        eeprom.response_bits = r.read_vec_u8()?;
        eeprom.response_index = r.read_u32()? as usize;
        eeprom.busy_reads = r.read_u8()?;
        eeprom.busy_reads_config = r.read_u8()?;
        let has_hint = r.read_bool()?;
        let hint_val = r.read_u32()? as usize;
        eeprom.dma_write_hint_len = if has_hint { Some(hint_val) } else { None };
        drop(eeprom);

        // Flash state
        self.flash.variant = match r.read_u8()? {
            0 => FlashVariant::None,
            1 => FlashVariant::Flash64K,
            2 => FlashVariant::Flash128K,
            _ => return Err("invalid flash variant in state"),
        };
        self.flash.mode = match r.read_u8()? {
            0 => FlashMode::ReadArray,
            1 => FlashMode::ReadId,
            _ => return Err("invalid flash mode in state"),
        };
        self.flash.unlock_stage = r.read_u8()?;
        self.flash.erase_armed = r.read_bool()?;
        self.flash.program_armed = r.read_bool()?;
        self.flash.bank_switch_armed = r.read_bool()?;
        self.flash.bank = r.read_u8()?;

        // Clear transient output buffer.
        self.audio_samples.clear();
        Ok(())
    }

    fn normalize_render_snapshots_after_deserialize(&mut self) {
        let pram_ok = self.pram_snapshot.len() == PRAM_SIZE;
        let vram_ok = self.vram_snapshot.len() == VRAM_SIZE;
        let oam_ok = self.oam_snapshot.len() == OAM_SIZE;
        if pram_ok && vram_ok && oam_ok {
            return;
        }

        self.pram_snapshot.resize(PRAM_SIZE, 0);
        self.vram_snapshot.resize(VRAM_SIZE, 0);
        self.oam_snapshot.resize(OAM_SIZE, 0);
        self.render_snapshot_valid = false;
    }

    pub fn snapshot_scanline_io(&mut self, line: u16) {
        let i = line as usize;
        if i < GBA_VISIBLE_LINES {
            self.scanline_io[i][..SCANLINE_IO_SNAPSHOT_SIZE]
                .copy_from_slice(&self.io[..SCANLINE_IO_SNAPSHOT_SIZE]);
            self.scanline_io_valid[i] = true;
        }
    }

    pub fn snapshot_scanline_pram(&mut self, line: u16) {
        let i = line as usize;
        if i < GBA_VISIBLE_LINES {
            self.scanline_pram[i][..PRAM_SIZE].copy_from_slice(&self.pram[..PRAM_SIZE]);
            self.scanline_pram_valid[i] = true;
        }
    }

    pub fn snapshot_scanline_obj_vram(&mut self, line: u16) {
        let i = line as usize;
        if i < GBA_VISIBLE_LINES {
            let start = i * OBJ_VRAM_SNAPSHOT_SIZE;
            let end = start + OBJ_VRAM_SNAPSHOT_SIZE;
            self.scanline_obj_vram[start..end]
                .copy_from_slice(&self.vram[VRAM_MIRROR_BASE..VRAM_SIZE]);
            self.scanline_obj_vram_valid[i] = true;
        }
    }

    pub fn snapshot_scanline_bg_bitmap_vram(&mut self, line: u16) {
        let i = line as usize;
        if i < GBA_VISIBLE_LINES {
            let start = i * BG_BITMAP_VRAM_SNAPSHOT_SIZE;
            let end = start + BG_BITMAP_VRAM_SNAPSHOT_SIZE;
            self.scanline_bg_bitmap_vram[start..end]
                .copy_from_slice(&self.vram[..BG_BITMAP_VRAM_SNAPSHOT_SIZE]);
            self.scanline_bg_bitmap_vram_valid[i] = true;
        }
    }

    pub fn snapshot_scanline_oam(&mut self, line: u16) {
        let i = line as usize;
        if i < GBA_VISIBLE_LINES {
            self.scanline_oam[i][..OAM_SIZE].copy_from_slice(&self.oam[..OAM_SIZE]);
            self.scanline_oam_valid[i] = true;
        }
    }

    /// Read a 16-bit IO register value for a specific scanline.
    /// Falls back to current IO state when the PPU has not captured a
    /// snapshot for this scanline (e.g. in unit tests that skip emulation).
    pub fn scanline_io_read16(&self, line: u32, io_offset: u32) -> u16 {
        let i = line as usize;
        let o = io_offset as usize;
        if i < GBA_VISIBLE_LINES && self.scanline_io_valid[i] && o + 1 < SCANLINE_IO_SNAPSHOT_SIZE {
            u16::from(self.scanline_io[i][o]) | (u16::from(self.scanline_io[i][o + 1]) << 8)
        } else {
            let lo = self.io[o % IO_SIZE] as u16;
            let hi = self.io[(o + 1) % IO_SIZE] as u16;
            lo | (hi << 8)
        }
    }

    pub fn scanline_pram_read16(&self, line: u32, offset: u32) -> u16 {
        let i = line as usize;
        let o = (offset as usize) & (PRAM_SIZE - 1);
        if i < GBA_VISIBLE_LINES && self.scanline_pram_valid[i] {
            let lo = self.scanline_pram[i][o] as u16;
            let hi = self.scanline_pram[i][(o + 1) & (PRAM_SIZE - 1)] as u16;
            lo | (hi << 8)
        } else {
            self.read_pram16(offset)
        }
    }

    pub fn scanline_obj_vram_read8(&self, line: u32, addr: u32) -> u8 {
        let i = line as usize;
        if i >= GBA_VISIBLE_LINES || !self.scanline_obj_vram_valid[i] {
            return self.read_vram8(addr);
        }

        let offset = (addr as usize).wrapping_sub(VRAM_BASE as usize);
        if (VRAM_MIRROR_BASE..VRAM_SIZE).contains(&offset) {
            self.scanline_obj_vram[i * OBJ_VRAM_SNAPSHOT_SIZE + (offset - VRAM_MIRROR_BASE)]
        } else if (VRAM_MIRROR_START..(VRAM_MIRROR_START + OBJ_VRAM_SNAPSHOT_SIZE))
            .contains(&offset)
        {
            self.scanline_obj_vram[i * OBJ_VRAM_SNAPSHOT_SIZE + (offset - VRAM_MIRROR_START)]
        } else {
            self.read_vram8(addr)
        }
    }

    pub fn scanline_bg_bitmap_vram_read8(&self, line: u32, addr: u32) -> u8 {
        let i = line as usize;
        if i >= GBA_VISIBLE_LINES || !self.scanline_bg_bitmap_vram_valid[i] {
            return self.read_vram8(addr);
        }

        let offset = (addr as usize).wrapping_sub(VRAM_BASE as usize);
        if offset < BG_BITMAP_VRAM_SNAPSHOT_SIZE {
            self.scanline_bg_bitmap_vram[i * BG_BITMAP_VRAM_SNAPSHOT_SIZE + offset]
        } else {
            self.read_vram8(addr)
        }
    }

    pub fn scanline_bg_bitmap_vram_read16(&self, line: u32, addr: u32) -> u16 {
        let lo = self.scanline_bg_bitmap_vram_read8(line, addr) as u16;
        let hi = self.scanline_bg_bitmap_vram_read8(line, addr + 1) as u16;
        lo | (hi << 8)
    }

    pub fn scanline_oam_read16(&self, line: u32, addr: u32) -> u16 {
        let i = line as usize;
        let o = (addr as usize).wrapping_sub(OAM_BASE as usize);
        if i < GBA_VISIBLE_LINES && self.scanline_oam_valid[i] && o + 1 < OAM_SIZE {
            u16::from(self.scanline_oam[i][o]) | (u16::from(self.scanline_oam[i][o + 1]) << 8)
        } else {
            self.read_oam16(addr)
        }
    }

    /// Snapshot PRAM, VRAM and OAM so the renderer can use the state
    /// from before the VBlank handler modifies them for the next frame.
    pub fn snapshot_render_state(&mut self) {
        self.pram_snapshot.copy_from_slice(&self.pram);
        self.vram_snapshot.copy_from_slice(&self.vram);
        self.oam_snapshot.copy_from_slice(&self.oam);
        self.render_snapshot_valid = true;
    }

    /// Clear the render snapshot so the renderer reads live PRAM/VRAM/OAM.
    pub fn invalidate_render_snapshot(&mut self) {
        self.render_snapshot_valid = false;
    }

    /// Read a 16-bit color from the palette snapshot captured at VBlank entry.
    /// Falls back to current PRAM when no snapshot has been taken (unit tests).
    pub fn read_pram16(&self, offset: u32) -> u16 {
        let src = if self.render_snapshot_valid {
            &self.pram_snapshot
        } else {
            &self.pram
        };
        let i = (offset as usize) & (PRAM_SIZE - 1);
        let lo = src[i] as u16;
        let hi = src[(i + 1) & (PRAM_SIZE - 1)] as u16;
        lo | (hi << 8)
    }

    /// Read an 8-bit value from the VRAM snapshot (addr is the full GBA address).
    pub fn read_vram8(&self, addr: u32) -> u8 {
        let src = if self.render_snapshot_valid {
            &self.vram_snapshot
        } else {
            &self.vram
        };
        let i = (addr as usize).wrapping_sub(VRAM_BASE as usize);
        if i < VRAM_SIZE {
            src[i]
        } else if i < VRAM_MIRROR_START + (VRAM_SIZE - VRAM_MIRROR_BASE) {
            src[VRAM_MIRROR_BASE + (i - VRAM_MIRROR_START)]
        } else {
            0
        }
    }

    /// Read a 16-bit value from the OAM snapshot (addr is the full GBA address).
    pub fn read_oam16(&self, addr: u32) -> u16 {
        let src = if self.render_snapshot_valid {
            &self.oam_snapshot
        } else {
            &self.oam
        };
        let i = (addr as usize).wrapping_sub(OAM_BASE as usize);
        if i + 1 < OAM_SIZE {
            let lo = src[i] as u16;
            let hi = src[i + 1] as u16;
            lo | (hi << 8)
        } else {
            0
        }
    }

    pub fn reset(&mut self) {
        self.ewram.fill(0);
        self.iwram.fill(0);
        self.io.fill(0);
        for row in self.scanline_io.iter_mut() {
            row.fill(0);
        }
        self.scanline_io_valid.fill(false);
        for row in self.scanline_pram.iter_mut() {
            row.fill(0);
        }
        self.scanline_pram_valid.fill(false);
        self.scanline_bg_bitmap_vram.fill(0);
        self.scanline_bg_bitmap_vram_valid.fill(false);
        self.scanline_obj_vram.fill(0);
        self.scanline_obj_vram_valid.fill(false);
        for row in self.scanline_oam.iter_mut() {
            row.fill(0);
        }
        self.scanline_oam_valid.fill(false);
        self.wave_ram = [0; 32];
        self.pram.fill(0);
        self.pram_snapshot.fill(0);
        self.vram.fill(0);
        self.vram_snapshot.fill(0);
        self.oam_snapshot.fill(0);
        self.render_snapshot_valid = false;
        self.set_oam_default_hidden();
        self.iwram_exec_bitmap.fill(0);
        self.timer_reload = [0; TIMER_COUNT];
        self.timer_counter = [0; TIMER_COUNT];
        self.timer_control = [0; TIMER_COUNT];
        self.fifo_a.clear();
        self.fifo_b.clear();
        self.last_fifo_a = 0;
        self.last_fifo_b = 0;
        self.fifo_a_underrun_streak = 0;
        self.fifo_b_underrun_streak = 0;
        self.direct_sound_a_latch = 0;
        self.direct_sound_b_latch = 0;
        self.direct_sound_a_prev_latch = 0;
        self.direct_sound_b_prev_latch = 0;
        self.direct_sound_a_cycles_since_latch = 0;
        self.direct_sound_b_cycles_since_latch = 0;
        self.direct_sound_a_latch_period_cycles = 1;
        self.direct_sound_b_latch_period_cycles = 1;
        self.audio_cycle_accum = 0;
        self.audio_samples.clear();
        self.reset_audio_post_filter_state();
        self.psg_square1_phase = 0.0;
        self.psg_square2_phase = 0.0;
        self.psg_square1_on = false;
        self.psg_square2_on = false;
        self.psg_wave_phase = 0.0;
        self.psg_noise_phase = 0;
        self.psg_wave_on = false;
        self.psg_wave_play_bank = 0;
        self.psg_wave_last_sample_index = 0xFF;
        self.psg_wave_pending_writes = [None; 32];
        self.psg_noise_on = false;
        self.psg_noise_lfsr = 0x7FFF;
        self.psg_frame_seq_accum = 0.0;
        self.psg_frame_seq_step = 0;
        self.psg_square1_length_ticks = 0;
        self.psg_square2_length_ticks = 0;
        self.psg_wave_length_ticks = 0;
        self.psg_noise_length_ticks = 0;
        self.psg_square1_shadow_freq = 0;
        self.psg_square1_sweep_counter = 0;
        self.psg_square1_volume = 0;
        self.psg_square2_volume = 0;
        self.psg_noise_volume = 0;
        self.psg_square1_env_period = 0;
        self.psg_square2_env_period = 0;
        self.psg_noise_env_period = 0;
        self.psg_square1_env_counter = 0;
        self.psg_square2_env_counter = 0;
        self.psg_noise_env_counter = 0;
        self.dma_internal_src = [0; 4];
        self.dma_internal_dst = [0; 4];
        self.dma_active = [false; 4];
        self.eeprom.borrow_mut().reset_session();
        self.flash.reset_session();

        if self.bios_loaded {
            // Cold boot with BIOS starts with POSTFLG cleared.
            self.io[REG_POSTFLG] = 0;
        } else {
            // No-BIOS bootstrap behaves as if BIOS already ran.
            self.io[REG_POSTFLG] = 1;
            // In no-BIOS mode, mimic the post-BIOS SOUNDBIAS center.
            self.write_io16_raw(REG_SOUNDBIAS, 0x0200);
        }
        self.refresh_audio_post_filter_config();
        // KEYINPUT defaults to "no key pressed" (all bits set).
        self.write_io16_raw(REG_KEYINPUT, 0x03FF);
    }

    pub fn set_keyinput_pressed_mask(&mut self, pressed_mask: u16) {
        let pressed = pressed_mask & 0x03FF;
        let register_value = 0x03FF & !pressed;
        self.write_io16_raw(REG_KEYINPUT, register_value);
        self.update_keypad_irq_condition();
    }

    pub fn has_backup(&self) -> bool {
        matches!(
            self.save_type,
            SaveType::Sram | SaveType::Eeprom | SaveType::Flash64K | SaveType::Flash128K
        )
    }

    pub fn backup_data(&self) -> Option<Vec<u8>> {
        match self.save_type {
            SaveType::Eeprom => Some(self.eeprom.borrow().storage_bytes()),
            SaveType::Sram | SaveType::Flash64K | SaveType::Flash128K => Some(self.sram.clone()),
            SaveType::None => None,
        }
    }

    pub fn take_audio_samples(&mut self) -> Vec<i16> {
        let mut out = Vec::new();
        self.take_audio_samples_into(&mut out);
        out
    }

    pub fn take_audio_samples_into(&mut self, out: &mut Vec<i16>) {
        out.clear();
        std::mem::swap(out, &mut self.audio_samples);
    }

    fn refresh_audio_post_filter_config(&mut self) {
        self.audio_post_filter_enabled = audio_post_filter_enabled();
        self.audio_post_lpf_stages = audio_post_filter_lpf_stages();
        self.refresh_audio_post_filter_coefficients(self.audio_sample_rate_hz());
        self.reset_audio_post_filter_state();
    }

    fn refresh_audio_post_filter_coefficients(&mut self, sample_rate_hz: u32) {
        self.audio_post_filter_rate_hz = sample_rate_hz.max(1);
        let rate = self.audio_post_filter_rate_hz as f32;
        self.audio_post_hpf_alpha = Self::high_pass_alpha(audio_post_filter_hpf_hz(), rate);
        self.audio_post_lpf_alpha = Self::low_pass_alpha(audio_post_filter_lpf_hz(), rate);
    }

    fn reset_audio_post_filter_state(&mut self) {
        self.audio_post_hpf_prev_in_l = 0.0;
        self.audio_post_hpf_prev_in_r = 0.0;
        self.audio_post_hpf_prev_out_l = 0.0;
        self.audio_post_hpf_prev_out_r = 0.0;
        self.audio_post_lpf_l = [0.0; AUDIO_POST_FILTER_MAX_STAGES];
        self.audio_post_lpf_r = [0.0; AUDIO_POST_FILTER_MAX_STAGES];
    }

    #[inline]
    fn high_pass_alpha(cutoff_hz: f32, sample_rate_hz: f32) -> f32 {
        if cutoff_hz <= 0.0 || sample_rate_hz <= 0.0 {
            return 1.0;
        }
        let rc = 1.0 / (2.0 * std::f32::consts::PI * cutoff_hz);
        let dt = 1.0 / sample_rate_hz;
        (rc / (rc + dt)).clamp(0.0, 1.0)
    }

    #[inline]
    fn low_pass_alpha(cutoff_hz: f32, sample_rate_hz: f32) -> f32 {
        if cutoff_hz <= 0.0 || sample_rate_hz <= 0.0 {
            return 1.0;
        }
        let rc = 1.0 / (2.0 * std::f32::consts::PI * cutoff_hz);
        let dt = 1.0 / sample_rate_hz;
        (dt / (rc + dt)).clamp(0.0, 1.0)
    }

    #[inline]
    fn decay_fifo_hold_sample(last: i8, streak: u16) -> i8 {
        if last == 0 {
            return 0;
        }
        let decay_num: i16 = match streak {
            0..=2 => 15,
            3..=8 => 14,
            9..=24 => 12,
            _ => 10,
        };
        let next = (i16::from(last) * decay_num) / 16;
        if next == 0 {
            0
        } else {
            next.clamp(i16::from(i8::MIN), i16::from(i8::MAX)) as i8
        }
    }

    #[inline]
    fn apply_audio_post_filter(&mut self, left: i16, right: i16) -> (i16, i16) {
        if !self.audio_post_filter_enabled {
            return (left, right);
        }

        let left_in = f32::from(left);
        let right_in = f32::from(right);

        let left_hp = self.audio_post_hpf_alpha
            * (self.audio_post_hpf_prev_out_l + left_in - self.audio_post_hpf_prev_in_l);
        let right_hp = self.audio_post_hpf_alpha
            * (self.audio_post_hpf_prev_out_r + right_in - self.audio_post_hpf_prev_in_r);
        self.audio_post_hpf_prev_in_l = left_in;
        self.audio_post_hpf_prev_in_r = right_in;
        self.audio_post_hpf_prev_out_l = left_hp;
        self.audio_post_hpf_prev_out_r = right_hp;

        let mut left_f = left_hp;
        let mut right_f = right_hp;
        let stages = usize::from(
            self.audio_post_lpf_stages
                .min(AUDIO_POST_FILTER_MAX_STAGES as u8),
        );
        for stage in 0..stages {
            self.audio_post_lpf_l[stage] +=
                self.audio_post_lpf_alpha * (left_f - self.audio_post_lpf_l[stage]);
            self.audio_post_lpf_r[stage] +=
                self.audio_post_lpf_alpha * (right_f - self.audio_post_lpf_r[stage]);
            left_f = self.audio_post_lpf_l[stage];
            right_f = self.audio_post_lpf_r[stage];
        }

        let left_out = left_f.round() as i32;
        let right_out = right_f.round() as i32;
        (
            left_out.clamp(i16::MIN as i32, i16::MAX as i32) as i16,
            right_out.clamp(i16::MIN as i32, i16::MAX as i32) as i16,
        )
    }

    pub fn load_backup_data(&mut self, data: &[u8]) {
        match self.save_type {
            SaveType::Eeprom => self.eeprom.borrow_mut().load_storage(data),
            SaveType::Sram | SaveType::Flash64K | SaveType::Flash128K => {
                self.sram.fill(0xFF);
                let copy_len = data.len().min(self.sram.len());
                self.sram[..copy_len].copy_from_slice(&data[..copy_len]);
            }
            SaveType::None => {}
        }
    }

    fn configure_save_hardware(&mut self) {
        let eeprom_enabled = self.save_type == SaveType::Eeprom;
        self.eeprom.borrow_mut().set_enabled(eeprom_enabled);
        self.flash.configure(match self.save_type {
            SaveType::Flash64K => FlashVariant::Flash64K,
            SaveType::Flash128K => FlashVariant::Flash128K,
            _ => FlashVariant::None,
        });

        let target_len = match self.save_type {
            SaveType::Flash128K => SRAM_SIZE * 2,
            SaveType::Flash64K | SaveType::Sram => SRAM_SIZE,
            _ => SRAM_SIZE,
        };
        if self.sram.len() != target_len {
            self.sram.resize(target_len, 0xFF);
        }
    }

    #[inline]
    fn read_rom_byte_offset(&self, offset: usize) -> u8 {
        if self.rom_len == 0 {
            return 0xFF;
        }
        let index = if self.rom_pow2 {
            offset & self.rom_mask
        } else {
            offset % self.rom_len
        };
        self.rom[index]
    }

    #[inline]
    fn read_rom16_window(&self, addr: u32, base: u32) -> u16 {
        let offset = (addr - base) as usize;
        let low = self.read_rom_byte_offset(offset) as u16;
        let high = self.read_rom_byte_offset(offset + 1) as u16;
        low | (high << 8)
    }

    #[inline]
    fn read_rom32_window(&self, addr: u32, base: u32) -> u32 {
        let offset = (addr - base) as usize;
        let b0 = self.read_rom_byte_offset(offset) as u32;
        let b1 = self.read_rom_byte_offset(offset + 1) as u32;
        let b2 = self.read_rom_byte_offset(offset + 2) as u32;
        let b3 = self.read_rom_byte_offset(offset + 3) as u32;
        b0 | (b1 << 8) | (b2 << 16) | (b3 << 24)
    }

    #[inline]
    pub fn fetch16_instr(&self, addr: u32) -> u16 {
        match addr {
            BIOS_BASE..=0x0000_3FFE => {
                let index = addr as usize;
                u16::from(self.bios[index]) | (u16::from(self.bios[index + 1]) << 8)
            }
            EWRAM_BASE..=0x02FF_FFFF => {
                let index = (addr as usize - EWRAM_BASE as usize) & (EWRAM_SIZE - 1);
                let low = self.ewram[index] as u16;
                let high = self.ewram[(index + 1) & (EWRAM_SIZE - 1)] as u16;
                low | (high << 8)
            }
            IWRAM_BASE..=0x03FF_FFFF => {
                let index = (addr as usize - IWRAM_BASE as usize) & (IWRAM_SIZE - 1);
                let low = self.iwram[index] as u16;
                let high = self.iwram[(index + 1) & (IWRAM_SIZE - 1)] as u16;
                low | (high << 8)
            }
            ROM0_BASE..=0x09FF_FFFE => self.read_rom16_window(addr, ROM0_BASE),
            ROM1_BASE..=0x0BFF_FFFE => self.read_rom16_window(addr, ROM1_BASE),
            ROM2_BASE..=0x0DFF_FFFE if !self.eeprom_active_addr(addr) => {
                self.read_rom16_window(addr, ROM2_BASE)
            }
            _ => self.read16(addr),
        }
    }

    pub fn note_exec_fetch(&mut self, addr: u32) {
        if !(IWRAM_BASE..=0x03FF_FFFF).contains(&addr) {
            return;
        }
        let offset = (addr as usize - IWRAM_BASE as usize) & (IWRAM_SIZE - 1);
        let page_index = (offset / IWRAM_EXEC_PAGE_SIZE).min(IWRAM_EXEC_TRACK_PAGES - 1);
        let start = page_index.saturating_sub(1);
        let end = (page_index + 1).min(IWRAM_EXEC_TRACK_PAGES - 1);
        for idx in start..=end {
            let bucket = idx / 64;
            let bit = idx % 64;
            self.iwram_exec_bitmap[bucket] |= 1u64 << bit;
        }
    }

    fn iwram_addr_recently_executed(&self, addr: u32) -> bool {
        if !(IWRAM_BASE..=0x03FF_FFFF).contains(&addr) {
            return false;
        }
        let offset = (addr as usize - IWRAM_BASE as usize) & (IWRAM_SIZE - 1);
        let page_index = (offset / IWRAM_EXEC_PAGE_SIZE).min(IWRAM_EXEC_TRACK_PAGES - 1);
        let bucket = page_index / 64;
        let bit = page_index % 64;
        (self.iwram_exec_bitmap[bucket] & (1u64 << bit)) != 0
    }

    #[inline]
    pub fn fetch32_instr(&self, addr: u32) -> u32 {
        match addr {
            BIOS_BASE..=0x0000_3FFC => {
                let index = addr as usize;
                let b0 = self.bios[index] as u32;
                let b1 = self.bios[index + 1] as u32;
                let b2 = self.bios[index + 2] as u32;
                let b3 = self.bios[index + 3] as u32;
                b0 | (b1 << 8) | (b2 << 16) | (b3 << 24)
            }
            EWRAM_BASE..=0x02FF_FFFF => {
                let index = (addr as usize - EWRAM_BASE as usize) & (EWRAM_SIZE - 1);
                let b0 = self.ewram[index] as u32;
                let b1 = self.ewram[(index + 1) & (EWRAM_SIZE - 1)] as u32;
                let b2 = self.ewram[(index + 2) & (EWRAM_SIZE - 1)] as u32;
                let b3 = self.ewram[(index + 3) & (EWRAM_SIZE - 1)] as u32;
                b0 | (b1 << 8) | (b2 << 16) | (b3 << 24)
            }
            IWRAM_BASE..=0x03FF_FFFF => {
                let index = (addr as usize - IWRAM_BASE as usize) & (IWRAM_SIZE - 1);
                let b0 = self.iwram[index] as u32;
                let b1 = self.iwram[(index + 1) & (IWRAM_SIZE - 1)] as u32;
                let b2 = self.iwram[(index + 2) & (IWRAM_SIZE - 1)] as u32;
                let b3 = self.iwram[(index + 3) & (IWRAM_SIZE - 1)] as u32;
                b0 | (b1 << 8) | (b2 << 16) | (b3 << 24)
            }
            ROM0_BASE..=0x09FF_FFFC => self.read_rom32_window(addr, ROM0_BASE),
            ROM1_BASE..=0x0BFF_FFFC => self.read_rom32_window(addr, ROM1_BASE),
            ROM2_BASE..=0x0DFF_FFFC if !self.eeprom_active_addr(addr) => {
                self.read_rom32_window(addr, ROM2_BASE)
            }
            _ => self.read32(addr),
        }
    }

    pub fn read8(&self, addr: u32) -> u8 {
        match addr {
            BIOS_BASE..=0x0000_3FFF => self.bios[addr as usize],
            EWRAM_BASE..=0x02FF_FFFF => {
                self.ewram[(addr as usize - EWRAM_BASE as usize) % EWRAM_SIZE]
            }
            IWRAM_BASE..=0x03FF_FFFF => {
                self.iwram[(addr as usize - IWRAM_BASE as usize) % IWRAM_SIZE]
            }
            IO_BASE..=0x0400_03FF => self.read_io8((addr as usize - IO_BASE as usize) % IO_SIZE),
            PRAM_BASE..=0x0500_03FF => self.pram[(addr as usize - PRAM_BASE as usize) % PRAM_SIZE],
            VRAM_BASE..=0x0601_FFFF => self.vram[vram_index(addr)],
            OAM_BASE..=0x0700_03FF => self.oam[(addr as usize - OAM_BASE as usize) % OAM_SIZE],
            ROM0_BASE..=0x09FF_FFFF => self.read_rom_window(addr, ROM0_BASE),
            ROM1_BASE..=0x0BFF_FFFF => self.read_rom_window(addr, ROM1_BASE),
            ROM2_BASE..=0x0DFF_FFFF => {
                if self.eeprom_active_addr(addr) {
                    self.eeprom.borrow_mut().read_bit()
                } else {
                    self.read_rom_window(addr, ROM2_BASE)
                }
            }
            SRAM_BASE..=0x0E00_FFFF => self.read_backup_byte(addr),
            _ => 0,
        }
    }

    pub fn read16(&self, addr: u32) -> u16 {
        if self.eeprom_active_addr(addr) {
            return u16::from(self.eeprom.borrow_mut().read_bit());
        }
        match addr {
            BIOS_BASE..=0x0000_3FFE => {
                let index = addr as usize;
                u16::from(self.bios[index]) | (u16::from(self.bios[index + 1]) << 8)
            }
            EWRAM_BASE..=0x02FF_FFFF => {
                let index = (addr as usize - EWRAM_BASE as usize) & (EWRAM_SIZE - 1);
                let low = self.ewram[index] as u16;
                let high = self.ewram[(index + 1) & (EWRAM_SIZE - 1)] as u16;
                low | (high << 8)
            }
            IWRAM_BASE..=0x03FF_FFFF => {
                let index = (addr as usize - IWRAM_BASE as usize) & (IWRAM_SIZE - 1);
                let low = self.iwram[index] as u16;
                let high = self.iwram[(index + 1) & (IWRAM_SIZE - 1)] as u16;
                low | (high << 8)
            }
            PRAM_BASE..=0x0500_03FF => {
                let index = (addr as usize - PRAM_BASE as usize) & (PRAM_SIZE - 1);
                let low = self.pram[index] as u16;
                let high = self.pram[(index + 1) & (PRAM_SIZE - 1)] as u16;
                low | (high << 8)
            }
            VRAM_BASE..=0x0601_FFFF => {
                let low = self.vram[vram_index(addr)] as u16;
                let high = self.vram[vram_index(addr.wrapping_add(1))] as u16;
                low | (high << 8)
            }
            OAM_BASE..=0x0700_03FF => {
                let index = (addr as usize - OAM_BASE as usize) & (OAM_SIZE - 1);
                let low = self.oam[index] as u16;
                let high = self.oam[(index + 1) & (OAM_SIZE - 1)] as u16;
                low | (high << 8)
            }
            ROM0_BASE..=0x09FF_FFFE => self.read_rom16_window(addr, ROM0_BASE),
            ROM1_BASE..=0x0BFF_FFFE => self.read_rom16_window(addr, ROM1_BASE),
            ROM2_BASE..=0x0DFF_FFFE => self.read_rom16_window(addr, ROM2_BASE),
            _ => {
                let low = self.read8(addr) as u16;
                let high = self.read8(addr.wrapping_add(1)) as u16;
                low | (high << 8)
            }
        }
    }

    pub fn read32(&self, addr: u32) -> u32 {
        if self.eeprom_active_addr(addr) {
            return u32::from(self.eeprom.borrow_mut().read_bit());
        }
        match addr {
            BIOS_BASE..=0x0000_3FFC => {
                let index = addr as usize;
                let b0 = self.bios[index] as u32;
                let b1 = self.bios[index + 1] as u32;
                let b2 = self.bios[index + 2] as u32;
                let b3 = self.bios[index + 3] as u32;
                b0 | (b1 << 8) | (b2 << 16) | (b3 << 24)
            }
            EWRAM_BASE..=0x02FF_FFFF => {
                let index = (addr as usize - EWRAM_BASE as usize) & (EWRAM_SIZE - 1);
                let b0 = self.ewram[index] as u32;
                let b1 = self.ewram[(index + 1) & (EWRAM_SIZE - 1)] as u32;
                let b2 = self.ewram[(index + 2) & (EWRAM_SIZE - 1)] as u32;
                let b3 = self.ewram[(index + 3) & (EWRAM_SIZE - 1)] as u32;
                b0 | (b1 << 8) | (b2 << 16) | (b3 << 24)
            }
            IWRAM_BASE..=0x03FF_FFFF => {
                let index = (addr as usize - IWRAM_BASE as usize) & (IWRAM_SIZE - 1);
                let b0 = self.iwram[index] as u32;
                let b1 = self.iwram[(index + 1) & (IWRAM_SIZE - 1)] as u32;
                let b2 = self.iwram[(index + 2) & (IWRAM_SIZE - 1)] as u32;
                let b3 = self.iwram[(index + 3) & (IWRAM_SIZE - 1)] as u32;
                b0 | (b1 << 8) | (b2 << 16) | (b3 << 24)
            }
            PRAM_BASE..=0x0500_03FF => {
                let index = (addr as usize - PRAM_BASE as usize) & (PRAM_SIZE - 1);
                let b0 = self.pram[index] as u32;
                let b1 = self.pram[(index + 1) & (PRAM_SIZE - 1)] as u32;
                let b2 = self.pram[(index + 2) & (PRAM_SIZE - 1)] as u32;
                let b3 = self.pram[(index + 3) & (PRAM_SIZE - 1)] as u32;
                b0 | (b1 << 8) | (b2 << 16) | (b3 << 24)
            }
            VRAM_BASE..=0x0601_FFFF => {
                let b0 = self.vram[vram_index(addr)] as u32;
                let b1 = self.vram[vram_index(addr.wrapping_add(1))] as u32;
                let b2 = self.vram[vram_index(addr.wrapping_add(2))] as u32;
                let b3 = self.vram[vram_index(addr.wrapping_add(3))] as u32;
                b0 | (b1 << 8) | (b2 << 16) | (b3 << 24)
            }
            OAM_BASE..=0x0700_03FF => {
                let index = (addr as usize - OAM_BASE as usize) & (OAM_SIZE - 1);
                let b0 = self.oam[index] as u32;
                let b1 = self.oam[(index + 1) & (OAM_SIZE - 1)] as u32;
                let b2 = self.oam[(index + 2) & (OAM_SIZE - 1)] as u32;
                let b3 = self.oam[(index + 3) & (OAM_SIZE - 1)] as u32;
                b0 | (b1 << 8) | (b2 << 16) | (b3 << 24)
            }
            ROM0_BASE..=0x09FF_FFFC => self.read_rom32_window(addr, ROM0_BASE),
            ROM1_BASE..=0x0BFF_FFFC => self.read_rom32_window(addr, ROM1_BASE),
            ROM2_BASE..=0x0DFF_FFFC => self.read_rom32_window(addr, ROM2_BASE),
            _ => {
                let b0 = self.read8(addr) as u32;
                let b1 = self.read8(addr.wrapping_add(1)) as u32;
                let b2 = self.read8(addr.wrapping_add(2)) as u32;
                let b3 = self.read8(addr.wrapping_add(3)) as u32;
                b0 | (b1 << 8) | (b2 << 16) | (b3 << 24)
            }
        }
    }

    pub fn write8(&mut self, addr: u32, value: u8) {
        trace_sound_write(addr, 1, u32::from(value));
        if self.eeprom_active_addr(addr) {
            self.eeprom.borrow_mut().write_bit(value);
            return;
        }
        if let Some(fifo_addr) = fifo_addr_from_io_addr(addr) {
            self.push_fifo_byte(fifo_addr, value);
        }

        match addr {
            EWRAM_BASE..=0x02FF_FFFF => {
                self.ewram[(addr as usize - EWRAM_BASE as usize) % EWRAM_SIZE] = value
            }
            IWRAM_BASE..=0x03FF_FFFF => {
                self.iwram[(addr as usize - IWRAM_BASE as usize) % IWRAM_SIZE] = value;
                trace_main_flags_write(addr, 1, u32::from(value));
            }
            IO_BASE..=0x0400_03FF => {
                let offset = (addr as usize - IO_BASE as usize) % IO_SIZE;
                self.write_io8(offset, value);
            }
            PRAM_BASE..=0x0500_03FF => duplicate_byte_write(&mut self.pram, PRAM_BASE, addr, value),
            VRAM_BASE..=0x0601_FFFF => {
                let aligned = vram_index(addr) & !1;
                self.vram[aligned] = value;
                self.vram[aligned + 1] = value;
            }
            OAM_BASE..=0x0700_03FF => duplicate_byte_write(&mut self.oam, OAM_BASE, addr, value),
            SRAM_BASE..=0x0E00_FFFF => self.write_backup_byte(addr, value),
            _ => {}
        }
    }

    pub fn write16(&mut self, addr: u32, value: u16) {
        trace_sound_write(addr, 2, u32::from(value));
        if self.eeprom_active_addr(addr) {
            self.eeprom.borrow_mut().write_bit(value as u8);
            return;
        }

        let low = (value & 0x00FF) as u8;
        let high = (value >> 8) as u8;
        if let Some(fifo_addr) = fifo_addr_from_io_addr(addr) {
            self.push_fifo_byte(fifo_addr, low);
            self.push_fifo_byte(fifo_addr, high);
        }
        match addr {
            EWRAM_BASE..=0x02FF_FFFF => {
                let index = (addr as usize - EWRAM_BASE as usize) % EWRAM_SIZE;
                self.ewram[index] = low;
                self.ewram[(index + 1) % EWRAM_SIZE] = high;
            }
            IWRAM_BASE..=0x03FF_FFFF => {
                let index = (addr as usize - IWRAM_BASE as usize) % IWRAM_SIZE;
                self.iwram[index] = low;
                self.iwram[(index + 1) % IWRAM_SIZE] = high;
                trace_main_flags_write(addr, 2, u32::from(value));
            }
            IO_BASE..=0x0400_03FF => {
                let offset = (addr as usize - IO_BASE as usize) % IO_SIZE;
                self.write_io8(offset, low);
                self.write_io8((offset + 1) % IO_SIZE, high);
            }
            PRAM_BASE..=0x0500_03FF => {
                let index = ((addr as usize - PRAM_BASE as usize) % PRAM_SIZE) & !1;
                self.pram[index] = low;
                self.pram[index + 1] = high;
            }
            VRAM_BASE..=0x0601_FFFF => {
                let index = vram_index(addr) & !1;
                self.vram[index] = low;
                self.vram[index + 1] = high;
            }
            OAM_BASE..=0x0700_03FF => {
                let index = ((addr as usize - OAM_BASE as usize) % OAM_SIZE) & !1;
                self.oam[index] = low;
                self.oam[index + 1] = high;
            }
            SRAM_BASE..=0x0E00_FFFF => {
                self.write_backup_byte(addr, low);
                self.write_backup_byte(addr.wrapping_add(1), high);
            }
            _ => {}
        }
    }

    pub fn write32(&mut self, addr: u32, value: u32) {
        trace_sound_write(addr, 4, value);
        if self.eeprom_active_addr(addr) {
            self.eeprom.borrow_mut().write_bit(value as u8);
            return;
        }

        self.write16(addr, (value & 0x0000_FFFF) as u16);
        self.write16(addr.wrapping_add(2), ((value >> 16) & 0xFFFF) as u16);
    }

    pub fn clear_ewram(&mut self) {
        self.ewram.fill(0);
    }

    pub fn clear_iwram(&mut self) {
        self.iwram.fill(0);
    }

    pub fn clear_pram(&mut self) {
        self.pram.fill(0);
    }

    pub fn clear_vram(&mut self) {
        self.vram.fill(0);
    }

    pub fn clear_oam(&mut self) {
        self.oam.fill(0);
    }

    pub fn request_irq(&mut self, irq_mask: u16) {
        let flags = self.read_io16_raw(REG_IF) | irq_mask;
        self.write_io16_raw(REG_IF, flags);
    }

    pub fn clear_irq(&mut self, irq_mask: u16) {
        let flags = self.read_io16_raw(REG_IF) & !irq_mask;
        self.write_io16_raw(REG_IF, flags);
    }

    pub fn pending_interrupts(&self) -> u16 {
        self.read_io16_raw(REG_IE) & self.read_io16_raw(REG_IF)
    }

    pub fn interrupts_master_enabled(&self) -> bool {
        (self.read_io16_raw(REG_IME) & 0x0001) != 0
    }

    fn update_keypad_irq_condition(&mut self) {
        let keycnt = self.read_io16_raw(REG_KEYCNT);
        // KEYCNT bit14: IRQ enable, bit15: 0=OR, 1=AND.
        if (keycnt & (1 << 14)) == 0 {
            return;
        }

        let mask = keycnt & 0x03FF;
        if mask == 0 {
            return;
        }

        let keyinput = self.read_io16_raw(REG_KEYINPUT) & 0x03FF;
        let pressed = (!keyinput) & 0x03FF;
        let and_mode = (keycnt & (1 << 15)) != 0;
        let condition_met = if and_mode {
            (pressed & mask) == mask
        } else {
            (pressed & mask) != 0
        };
        if condition_met {
            self.request_irq(IRQ_KEYPAD);
        }
    }

    pub(crate) fn current_audio_sample_rate_hz(&self) -> u32 {
        self.audio_sample_rate_hz()
    }

    pub fn dispstat(&self) -> u16 {
        self.read_io16_raw(REG_DISPSTAT)
    }

    pub fn timer_reload(&self, channel: usize) -> u16 {
        self.timer_reload[channel % TIMER_COUNT]
    }

    pub fn timer_control(&self, channel: usize) -> u16 {
        self.timer_control[channel % TIMER_COUNT]
    }

    pub fn timer_counter(&self, channel: usize) -> u16 {
        self.timer_counter[channel % TIMER_COUNT]
    }

    pub fn set_timer_counter(&mut self, channel: usize, value: u16) {
        let index = channel % TIMER_COUNT;
        self.timer_counter[index] = value;
        let base = timer_base_offset(index);
        self.io[base] = (value & 0x00FF) as u8;
        self.io[base + 1] = (value >> 8) as u8;
    }

    pub fn set_lcd_status(&mut self, vcount: u16, vblank: bool, hblank: bool, vcounter: bool) {
        self.write_io16_raw(REG_VCOUNT, vcount);

        let mut dispstat = self.read_io16_raw(REG_DISPSTAT) & !0x0007;
        if vblank {
            dispstat |= 1 << 0;
        }
        if hblank {
            dispstat |= 1 << 1;
        }
        if vcounter {
            dispstat |= 1 << 2;
        }
        self.write_io16_raw(REG_DISPSTAT, dispstat);
    }

    pub fn trigger_vblank_dma(&mut self) {
        self.trigger_dma_timing(DMA_TIMING_VBLANK);
    }

    pub fn trigger_hblank_dma(&mut self) {
        self.trigger_dma_timing(DMA_TIMING_HBLANK);
    }

    pub fn on_timer_overflow(&mut self, timer_channel: usize, overflows: u32) {
        if overflows == 0 {
            return;
        }

        for _ in 0..overflows {
            self.trigger_special_sound_dma(timer_channel);
        }
    }

    pub(crate) fn mix_audio_for_cycles(&mut self, cycles: u32) {
        let cycles_per_sample = self.audio_cycles_per_sample();
        let sample_rate_hz = self.audio_sample_rate_hz();
        if self.audio_post_filter_enabled && self.audio_post_filter_rate_hz != sample_rate_hz {
            self.refresh_audio_post_filter_coefficients(sample_rate_hz);
        }

        let mut remaining = cycles;
        while remaining != 0 {
            let until_sample = cycles_per_sample
                .saturating_sub(self.audio_cycle_accum)
                .max(1);
            let step = remaining.min(until_sample);
            remaining -= step;

            self.audio_cycle_accum = self.audio_cycle_accum.wrapping_add(step);
            self.direct_sound_a_cycles_since_latch =
                self.direct_sound_a_cycles_since_latch.saturating_add(step);
            self.direct_sound_b_cycles_since_latch =
                self.direct_sound_b_cycles_since_latch.saturating_add(step);
            if self.audio_cycle_accum >= cycles_per_sample {
                self.audio_cycle_accum -= cycles_per_sample;
                let master_on = (self.read_io16_raw(REG_SOUNDCNT_X) & 0x0080) != 0;
                let (direct_left, direct_right, direct_routed) = if master_on {
                    self.current_direct_sound_mix()
                } else {
                    (0, 0, false)
                };

                self.mix_audio_output_sample(
                    sample_rate_hz as f32,
                    direct_left,
                    direct_right,
                    direct_routed,
                );
            }
        }
    }

    #[inline]
    fn current_direct_sound_mix(&self) -> (i32, i32, bool) {
        let soundcnt_h = self.read_io16_raw(REG_SOUNDCNT_H);
        let direct_routed = (soundcnt_h & ((1 << 8) | (1 << 9) | (1 << 12) | (1 << 13))) != 0;
        if !direct_routed {
            return (0, 0, false);
        }

        let interpolate = direct_sound_interpolate_enabled(self.bios_loaded);
        let sample_a = if interpolate {
            Self::interpolate_direct_sound_sample(
                self.direct_sound_a_prev_latch,
                self.direct_sound_a_latch,
                self.direct_sound_a_cycles_since_latch,
                self.direct_sound_a_latch_period_cycles,
            )
        } else {
            self.direct_sound_a_latch
        };
        let sample_b = if interpolate {
            Self::interpolate_direct_sound_sample(
                self.direct_sound_b_prev_latch,
                self.direct_sound_b_latch,
                self.direct_sound_b_cycles_since_latch,
                self.direct_sound_b_latch_period_cycles,
            )
        } else {
            self.direct_sound_b_latch
        };

        let left_a = if (soundcnt_h & (1 << 9)) != 0 {
            i32::from(sample_a)
        } else {
            0
        };
        let right_a = if (soundcnt_h & (1 << 8)) != 0 {
            i32::from(sample_a)
        } else {
            0
        };
        let left_b = if (soundcnt_h & (1 << 13)) != 0 {
            i32::from(sample_b)
        } else {
            0
        };
        let right_b = if (soundcnt_h & (1 << 12)) != 0 {
            i32::from(sample_b)
        } else {
            0
        };

        (left_a + left_b, right_a + right_b, true)
    }

    #[inline]
    fn interpolate_direct_sound_sample(
        previous: i16,
        current: i16,
        cycles_since_latch: u32,
        latch_period_cycles: u32,
    ) -> i16 {
        if previous == current || latch_period_cycles <= 1 {
            return current;
        }
        let t = (cycles_since_latch.min(latch_period_cycles) as f32) / (latch_period_cycles as f32);
        let prev = previous as f32;
        let curr = current as f32;
        (prev + (curr - prev) * t).round() as i16
    }

    #[inline]
    fn direct_sound_timer_period_cycles(&self, timer_channel: usize) -> u32 {
        if timer_channel >= TIMER_COUNT {
            return self.audio_cycles_per_sample().max(1);
        }
        let control = self.timer_control(timer_channel);
        if (control & 0x0080) == 0 || (control & 0x0004) != 0 {
            return self.audio_cycles_per_sample().max(1);
        }
        let prescaler = TIMER_PRESCALER_CYCLES[(control & 0x0003) as usize];
        let reload = self.timer_reload(timer_channel);
        let ticks = (0x1_0000u32 - u32::from(reload)).max(1);
        ticks.saturating_mul(prescaler).max(1)
    }

    #[inline]
    fn soundbias_sample_rate_select(&self) -> u8 {
        ((self.read_io16_raw(REG_SOUNDBIAS) >> 14) & 0x3) as u8
    }

    #[inline]
    fn audio_cycles_per_sample(&self) -> u32 {
        AUDIO_BASE_CYCLES_PER_SAMPLE >> self.soundbias_sample_rate_select()
    }

    #[inline]
    fn audio_sample_rate_hz(&self) -> u32 {
        GBA_MASTER_CLOCK_HZ / self.audio_cycles_per_sample().max(1)
    }

    fn read_rom_window(&self, addr: u32, base: u32) -> u8 {
        let offset = (addr - base) as usize;
        self.read_rom_byte_offset(offset)
    }

    fn eeprom_active_addr(&self, addr: u32) -> bool {
        if !(EEPROM_BASE..=0x0DFF_FFFF).contains(&addr) {
            return false;
        }
        self.save_type == SaveType::Eeprom && self.eeprom.borrow().enabled
    }

    fn read_backup_byte(&self, addr: u32) -> u8 {
        match self.save_type {
            SaveType::Sram => {
                let index = (addr as usize - SRAM_BASE as usize) % self.sram.len();
                self.sram[index]
            }
            SaveType::Flash64K | SaveType::Flash128K => self.flash_read(addr),
            SaveType::None | SaveType::Eeprom => 0xFF,
        }
    }

    fn write_backup_byte(&mut self, addr: u32, value: u8) {
        match self.save_type {
            SaveType::Sram => {
                let index = (addr as usize - SRAM_BASE as usize) % self.sram.len();
                self.sram[index] = value;
            }
            SaveType::Flash64K | SaveType::Flash128K => self.flash_write(addr, value),
            SaveType::None | SaveType::Eeprom => {}
        }
    }

    fn flash_read(&self, addr: u32) -> u8 {
        if !self.flash.is_enabled() {
            return 0xFF;
        }

        let offset = ((addr - SRAM_BASE) & 0xFFFF) as usize;
        if self.flash.mode == FlashMode::ReadId {
            return match offset {
                0x0000 => self.flash.manufacturer_id(),
                0x0001 => self.flash.device_id(),
                _ => 0xFF,
            };
        }

        let bank_offset = usize::from(self.flash.bank) * SRAM_SIZE;
        let index = bank_offset + offset;
        self.sram.get(index).copied().unwrap_or(0xFF)
    }

    fn flash_write(&mut self, addr: u32, value: u8) {
        if !self.flash.is_enabled() {
            return;
        }

        let offset = ((addr - SRAM_BASE) & 0xFFFF) as usize;

        if value == 0xF0 {
            self.flash.mode = FlashMode::ReadArray;
            self.flash.unlock_stage = 0;
            self.flash.erase_armed = false;
            self.flash.program_armed = false;
            self.flash.bank_switch_armed = false;
            return;
        }

        if self.flash.program_armed {
            self.flash_program_byte(offset, value);
            self.flash.program_armed = false;
            return;
        }

        if self.flash.bank_switch_armed {
            if offset == 0 {
                let max_bank = self.flash.max_banks();
                if max_bank > 0 {
                    self.flash.bank = value % max_bank;
                }
            }
            self.flash.bank_switch_armed = false;
            return;
        }

        match self.flash.unlock_stage {
            0 => {
                if offset == 0x5555 && value == 0xAA {
                    self.flash.unlock_stage = 1;
                }
            }
            1 => {
                if offset == 0x2AAA && value == 0x55 {
                    self.flash.unlock_stage = 2;
                } else {
                    self.flash.unlock_stage = 0;
                }
            }
            _ => {
                self.flash.unlock_stage = 0;
                self.flash_handle_command(offset, value);
            }
        }
    }

    fn flash_handle_command(&mut self, offset: usize, value: u8) {
        if self.flash.erase_armed {
            self.flash.erase_armed = false;
            match value {
                0x10 if offset == 0x5555 => self.flash_chip_erase(),
                0x30 => self.flash_sector_erase(offset),
                _ => {}
            }
            return;
        }

        match value {
            0x90 => self.flash.mode = FlashMode::ReadId,
            0x80 => self.flash.erase_armed = true,
            0xA0 => self.flash.program_armed = true,
            0xB0 => {
                if self.flash.variant == FlashVariant::Flash128K {
                    self.flash.bank_switch_armed = true;
                }
            }
            0xF0 => self.flash.mode = FlashMode::ReadArray,
            _ => {}
        }
    }

    fn flash_program_byte(&mut self, offset: usize, value: u8) {
        let bank_offset = usize::from(self.flash.bank) * SRAM_SIZE;
        let index = bank_offset + offset;
        if let Some(slot) = self.sram.get_mut(index) {
            *slot = value;
        }
    }

    fn flash_chip_erase(&mut self) {
        self.sram.fill(0xFF);
    }

    fn flash_sector_erase(&mut self, offset: usize) {
        let sector_start = offset & !0x0FFF;
        let bank_offset = usize::from(self.flash.bank) * SRAM_SIZE;
        let start = bank_offset + sector_start;
        let end = start + 0x1000;
        if end <= self.sram.len() {
            self.sram[start..end].fill(0xFF);
        }
    }

    fn push_fifo_byte(&mut self, fifo_addr: u32, value: u8) {
        let fifo = if fifo_addr == FIFO_A_ADDR {
            &mut self.fifo_a
        } else {
            &mut self.fifo_b
        };
        if fifo.len() >= SOUND_FIFO_CAPACITY {
            let _ = fifo.pop_front();
        }
        fifo.push_back(value as i8);
    }

    fn pop_sound_fifo_sample(&mut self, fifo_addr: u32) -> i16 {
        if fifo_addr == FIFO_A_ADDR {
            if let Some(sample) = self.fifo_a.pop_front() {
                self.last_fifo_a = sample;
                self.fifo_a_underrun_streak = 0;
                i16::from(sample)
            } else if self.bios_loaded {
                trace_sound_underrun(fifo_addr, self.fifo_a.len(), self.fifo_b.len());
                i16::from(self.last_fifo_a)
            } else {
                trace_sound_underrun(fifo_addr, self.fifo_a.len(), self.fifo_b.len());
                self.fifo_a_underrun_streak = self.fifo_a_underrun_streak.saturating_add(1);
                if !audio_fifo_underrun_decay_enabled()
                    || self.fifo_a_underrun_streak <= SOUND_FIFO_UNDERRUN_HOLD_SAMPLES
                {
                    return i16::from(self.last_fifo_a);
                }
                let decay_streak = self
                    .fifo_a_underrun_streak
                    .saturating_sub(SOUND_FIFO_UNDERRUN_HOLD_SAMPLES);
                self.last_fifo_a = Self::decay_fifo_hold_sample(self.last_fifo_a, decay_streak);
                i16::from(self.last_fifo_a)
            }
        } else if let Some(sample) = self.fifo_b.pop_front() {
            self.last_fifo_b = sample;
            self.fifo_b_underrun_streak = 0;
            i16::from(sample)
        } else if self.bios_loaded {
            trace_sound_underrun(fifo_addr, self.fifo_a.len(), self.fifo_b.len());
            i16::from(self.last_fifo_b)
        } else {
            trace_sound_underrun(fifo_addr, self.fifo_a.len(), self.fifo_b.len());
            self.fifo_b_underrun_streak = self.fifo_b_underrun_streak.saturating_add(1);
            if !audio_fifo_underrun_decay_enabled()
                || self.fifo_b_underrun_streak <= SOUND_FIFO_UNDERRUN_HOLD_SAMPLES
            {
                return i16::from(self.last_fifo_b);
            }
            let decay_streak = self
                .fifo_b_underrun_streak
                .saturating_sub(SOUND_FIFO_UNDERRUN_HOLD_SAMPLES);
            self.last_fifo_b = Self::decay_fifo_hold_sample(self.last_fifo_b, decay_streak);
            i16::from(self.last_fifo_b)
        }
    }

    fn sound_fifo_len(&self, fifo_addr: u32) -> usize {
        if fifo_addr == FIFO_A_ADDR {
            self.fifo_a.len()
        } else {
            self.fifo_b.len()
        }
    }

    fn clear_sound_fifo(&mut self, fifo_addr: u32) {
        if fifo_addr == FIFO_A_ADDR {
            self.fifo_a.clear();
            self.last_fifo_a = 0;
            self.fifo_a_underrun_streak = 0;
            self.direct_sound_a_prev_latch = 0;
            self.direct_sound_a_latch = 0;
            self.direct_sound_a_cycles_since_latch = 0;
            self.direct_sound_a_latch_period_cycles = 1;
        } else {
            self.fifo_b.clear();
            self.last_fifo_b = 0;
            self.fifo_b_underrun_streak = 0;
            self.direct_sound_b_prev_latch = 0;
            self.direct_sound_b_latch = 0;
            self.direct_sound_b_cycles_since_latch = 0;
            self.direct_sound_b_latch_period_cycles = 1;
        }
    }

    fn wave_ram_selected_bank(&self) -> usize {
        if (self.io[REG_SOUND3CNT_L] & (1 << 6)) != 0 {
            1
        } else {
            0
        }
    }

    fn wave_ram_two_bank_mode(&self) -> bool {
        (self.io[REG_SOUND3CNT_L] & (1 << 5)) != 0
    }

    fn current_wave_playback_bank(&self) -> usize {
        let selected_bank = if self.psg_wave_on {
            usize::from(self.psg_wave_play_bank & 1)
        } else {
            self.wave_ram_selected_bank()
        };
        if !self.psg_wave_on || !self.wave_ram_two_bank_mode() {
            return selected_bank;
        }

        let sample_index = ((self.psg_wave_phase * 64.0_f32) as usize).min(63);
        if sample_index < 32 {
            selected_bank
        } else {
            selected_bank ^ 1
        }
    }

    fn wave_ram_access_bank(&self) -> usize {
        if self.psg_wave_on && (self.io[REG_SOUND3CNT_L] & 0x80) != 0 {
            self.current_wave_playback_bank() ^ 1
        } else {
            self.wave_ram_selected_bank() ^ 1
        }
    }

    fn wave_ram_io_index(&self, offset: usize) -> usize {
        let index = (offset - WAVE_RAM_START) & 0x0F;
        self.wave_ram_access_bank() * 16 + index
    }

    fn apply_wave_pending_writes(&mut self) {
        for (index, pending) in self.psg_wave_pending_writes.iter_mut().enumerate() {
            if let Some(value) = pending.take() {
                self.wave_ram[index] = value;
            }
        }
    }

    fn read_wave_ram_io8(&self, offset: usize) -> u8 {
        let index = self.wave_ram_io_index(offset);
        self.psg_wave_pending_writes[index].unwrap_or(self.wave_ram[index])
    }

    fn write_wave_ram_io8(&mut self, offset: usize, value: u8) {
        let wave_index = self.wave_ram_io_index(offset);
        if self.psg_wave_on && (self.io[REG_SOUND3CNT_L] & 0x80) != 0 {
            self.psg_wave_pending_writes[wave_index] = Some(value);
        } else {
            self.wave_ram[wave_index] = value;
        }
        self.io[offset % IO_SIZE] = value;
    }

    fn trigger_psg_square1(&mut self) {
        if (self.read_io16_raw(REG_SOUNDCNT_X) & 0x0080) == 0 {
            return;
        }
        let cnt_l = self.read_io16_raw(REG_SOUND1CNT_L);
        let cnt_h = self.read_io16_raw(REG_SOUND1CNT_H);
        let cnt_x = self.read_io16_raw(REG_SOUND1CNT_X);
        self.psg_square1_on = true;
        self.psg_square1_phase = 0.0;
        self.psg_square1_length_ticks = 64 - (cnt_h & 0x003F);
        self.psg_square1_volume = ((cnt_h >> 12) & 0x0F) as u8;
        self.psg_square1_env_period = ((cnt_h >> 8) & 0x7) as u8;
        self.psg_square1_env_counter = self.psg_square1_env_period;
        self.psg_square1_shadow_freq = cnt_x & 0x07FF;
        let sweep_period = ((cnt_l >> 4) & 0x7) as u8;
        self.psg_square1_sweep_counter = if sweep_period == 0 { 8 } else { sweep_period };

        let sweep_shift = (cnt_l & 0x7) as u8;
        let sweep_negate = (cnt_l & (1 << 3)) != 0;
        if sweep_shift != 0
            && self
                .compute_square1_sweep_frequency(sweep_negate, sweep_shift)
                .is_none()
        {
            self.psg_square1_on = false;
        }
    }

    fn trigger_psg_square2(&mut self) {
        if (self.read_io16_raw(REG_SOUNDCNT_X) & 0x0080) == 0 {
            return;
        }
        let cnt_l = self.read_io16_raw(REG_SOUND2CNT_L);
        self.psg_square2_on = true;
        self.psg_square2_phase = 0.0;
        self.psg_square2_length_ticks = 64 - (cnt_l & 0x003F);
        self.psg_square2_volume = ((cnt_l >> 12) & 0x0F) as u8;
        self.psg_square2_env_period = ((cnt_l >> 8) & 0x7) as u8;
        self.psg_square2_env_counter = self.psg_square2_env_period;
    }

    fn trigger_psg_wave(&mut self) {
        if (self.read_io16_raw(REG_SOUNDCNT_X) & 0x0080) == 0 {
            return;
        }
        self.apply_wave_pending_writes();
        let cnt_h = self.read_io16_raw(REG_SOUND3CNT_H);
        self.psg_wave_on = true;
        self.psg_wave_phase = 0.0;
        self.psg_wave_play_bank = self.wave_ram_selected_bank() as u8;
        self.psg_wave_last_sample_index = 0xFF;
        self.psg_wave_length_ticks = 256 - (cnt_h & 0x00FF);
    }

    fn trigger_psg_noise(&mut self) {
        if (self.read_io16_raw(REG_SOUNDCNT_X) & 0x0080) == 0 {
            return;
        }
        let cnt_l = self.read_io16_raw(REG_SOUND4CNT_L);
        self.psg_noise_on = true;
        self.psg_noise_phase = 0;
        self.psg_noise_lfsr = 0x7FFF;
        self.psg_noise_length_ticks = 64 - (cnt_l & 0x003F);
        self.psg_noise_volume = ((cnt_l >> 12) & 0x0F) as u8;
        self.psg_noise_env_period = ((cnt_l >> 8) & 0x7) as u8;
        self.psg_noise_env_counter = self.psg_noise_env_period;
    }

    fn advance_psg_timing_at_rate(&mut self, sample_rate_hz: f32) {
        if sample_rate_hz <= 0.0 {
            return;
        }

        self.psg_frame_seq_accum += PSG_FRAME_SEQ_HZ / sample_rate_hz;
        while self.psg_frame_seq_accum >= 1.0 {
            self.psg_frame_seq_accum -= 1.0;
            self.clock_psg_frame_sequencer();
        }
    }

    fn clock_psg_frame_sequencer(&mut self) {
        match self.psg_frame_seq_step {
            0 | 4 => {
                self.clock_psg_length_counters();
            }
            2 | 6 => {
                self.clock_psg_length_counters();
                self.clock_psg_square1_sweep();
            }
            7 => {
                self.clock_psg_envelopes();
            }
            _ => {}
        }
        self.psg_frame_seq_step = (self.psg_frame_seq_step + 1) & 0x7;
    }

    fn compute_square1_sweep_frequency(&self, negate: bool, shift: u8) -> Option<u16> {
        if shift == 0 {
            return Some(self.psg_square1_shadow_freq);
        }

        let delta = self.psg_square1_shadow_freq >> shift;
        let next = if negate {
            i32::from(self.psg_square1_shadow_freq) - i32::from(delta)
        } else {
            i32::from(self.psg_square1_shadow_freq) + i32::from(delta)
        };

        if (0..=2047).contains(&next) {
            Some(next as u16)
        } else {
            None
        }
    }

    fn clock_psg_square1_sweep(&mut self) {
        if !self.psg_square1_on {
            return;
        }

        let cnt_l = self.read_io16_raw(REG_SOUND1CNT_L);
        let sweep_period = ((cnt_l >> 4) & 0x7) as u8;
        let sweep_shift = (cnt_l & 0x7) as u8;
        let sweep_negate = (cnt_l & (1 << 3)) != 0;

        if self.psg_square1_sweep_counter > 0 {
            self.psg_square1_sweep_counter -= 1;
        }
        if self.psg_square1_sweep_counter != 0 {
            return;
        }
        self.psg_square1_sweep_counter = if sweep_period == 0 { 8 } else { sweep_period };

        if sweep_shift == 0 {
            return;
        }

        let Some(next_frequency) = self.compute_square1_sweep_frequency(sweep_negate, sweep_shift)
        else {
            self.psg_square1_on = false;
            self.refresh_psg_status_bits();
            return;
        };

        self.psg_square1_shadow_freq = next_frequency;
        let cnt_x = self.read_io16_raw(REG_SOUND1CNT_X);
        self.write_io16_raw(REG_SOUND1CNT_X, (cnt_x & !0x07FF) | next_frequency);

        if self
            .compute_square1_sweep_frequency(sweep_negate, sweep_shift)
            .is_none()
        {
            self.psg_square1_on = false;
            self.refresh_psg_status_bits();
        }
    }

    fn clock_psg_length_counters(&mut self) {
        let mut changed = false;

        if self.psg_square1_on && (self.read_io16_raw(REG_SOUND1CNT_X) & (1 << 14)) != 0 {
            self.psg_square1_length_ticks = self.psg_square1_length_ticks.saturating_sub(1);
            if self.psg_square1_length_ticks == 0 {
                self.psg_square1_on = false;
                changed = true;
            }
        }

        if self.psg_square2_on && (self.read_io16_raw(REG_SOUND2CNT_H) & (1 << 14)) != 0 {
            self.psg_square2_length_ticks = self.psg_square2_length_ticks.saturating_sub(1);
            if self.psg_square2_length_ticks == 0 {
                self.psg_square2_on = false;
                changed = true;
            }
        }

        if self.psg_wave_on && (self.read_io16_raw(REG_SOUND3CNT_X) & (1 << 14)) != 0 {
            self.psg_wave_length_ticks = self.psg_wave_length_ticks.saturating_sub(1);
            if self.psg_wave_length_ticks == 0 {
                self.psg_wave_on = false;
                self.psg_wave_phase = 0.0;
                self.psg_wave_last_sample_index = 0xFF;
                self.apply_wave_pending_writes();
                changed = true;
            }
        }

        if self.psg_noise_on && (self.read_io16_raw(REG_SOUND4CNT_H) & (1 << 14)) != 0 {
            self.psg_noise_length_ticks = self.psg_noise_length_ticks.saturating_sub(1);
            if self.psg_noise_length_ticks == 0 {
                self.psg_noise_on = false;
                changed = true;
            }
        }

        if changed {
            self.refresh_psg_status_bits();
        }
    }

    fn clock_psg_envelopes(&mut self) {
        if self.psg_square1_on {
            let cnt_h = self.read_io16_raw(REG_SOUND1CNT_H);
            if Self::clock_psg_envelope(
                self.psg_square1_env_period,
                &mut self.psg_square1_env_counter,
            ) {
                if (cnt_h & (1 << 11)) != 0 {
                    self.psg_square1_volume = (self.psg_square1_volume.saturating_add(1)).min(15);
                } else {
                    self.psg_square1_volume = self.psg_square1_volume.saturating_sub(1);
                }
            }
        }

        if self.psg_square2_on {
            let cnt_l = self.read_io16_raw(REG_SOUND2CNT_L);
            if Self::clock_psg_envelope(
                self.psg_square2_env_period,
                &mut self.psg_square2_env_counter,
            ) {
                if (cnt_l & (1 << 11)) != 0 {
                    self.psg_square2_volume = (self.psg_square2_volume.saturating_add(1)).min(15);
                } else {
                    self.psg_square2_volume = self.psg_square2_volume.saturating_sub(1);
                }
            }
        }

        if self.psg_noise_on {
            let cnt_l = self.read_io16_raw(REG_SOUND4CNT_L);
            if Self::clock_psg_envelope(self.psg_noise_env_period, &mut self.psg_noise_env_counter)
            {
                if (cnt_l & (1 << 11)) != 0 {
                    self.psg_noise_volume = (self.psg_noise_volume.saturating_add(1)).min(15);
                } else {
                    self.psg_noise_volume = self.psg_noise_volume.saturating_sub(1);
                }
            }
        }
    }

    fn clock_psg_envelope(period: u8, counter: &mut u8) -> bool {
        if period == 0 {
            return false;
        }

        if *counter == 0 {
            *counter = period;
        }

        if *counter > 1 {
            *counter -= 1;
            return false;
        }

        *counter = period;
        true
    }

    fn refresh_psg_status_bits(&mut self) {
        let mut soundcnt_x = self.read_io16_raw(REG_SOUNDCNT_X);
        soundcnt_x &= !0x000F;
        if self.psg_square1_on {
            soundcnt_x |= 1 << 0;
        }
        if self.psg_square2_on {
            soundcnt_x |= 1 << 1;
        }
        if self.psg_wave_on {
            soundcnt_x |= 1 << 2;
        }
        if self.psg_noise_on {
            soundcnt_x |= 1 << 3;
        }
        self.write_io16_raw(REG_SOUNDCNT_X, soundcnt_x);
    }

    fn mix_psg_sample_at_rate(&mut self, sample_rate_hz: f32) -> (i16, i16) {
        self.advance_psg_timing_at_rate(sample_rate_hz);

        let soundcnt_l = self.read_io16_raw(REG_SOUNDCNT_L);
        let soundcnt_h = self.read_io16_raw(REG_SOUNDCNT_H);
        let right_master = i32::from((soundcnt_l & 0x0007) as u8) + 1;
        let left_master = i32::from(((soundcnt_l >> 4) & 0x0007) as u8) + 1;
        let psg_ratio_num = match (soundcnt_h & 0x0003) as u8 {
            0 => 1, // 25%
            1 => 2, // 50%
            _ => 4, // 100% (and 3 treated as 100%)
        };

        let ch1 = self.next_psg_square_sample_at_rate(1, sample_rate_hz);
        let ch2 = self.next_psg_square_sample_at_rate(2, sample_rate_hz);
        let ch3 = self.next_psg_wave_sample_at_rate(sample_rate_hz);
        let ch4 = self.next_psg_noise_sample_at_rate(sample_rate_hz);

        let mut left = 0i32;
        let mut right = 0i32;

        if (soundcnt_l & (1 << 12)) != 0 {
            left += i32::from(ch1);
        }
        if (soundcnt_l & (1 << 8)) != 0 {
            right += i32::from(ch1);
        }
        if (soundcnt_l & (1 << 13)) != 0 {
            left += i32::from(ch2);
        }
        if (soundcnt_l & (1 << 9)) != 0 {
            right += i32::from(ch2);
        }
        if (soundcnt_l & (1 << 14)) != 0 {
            left += i32::from(ch3);
        }
        if (soundcnt_l & (1 << 10)) != 0 {
            right += i32::from(ch3);
        }
        if (soundcnt_l & (1 << 15)) != 0 {
            left += i32::from(ch4);
        }
        if (soundcnt_l & (1 << 11)) != 0 {
            right += i32::from(ch4);
        }

        left = (((left * left_master) / 8) * psg_ratio_num) / 4;
        right = (((right * right_master) / 8) * psg_ratio_num) / 4;

        (
            left.clamp(i16::MIN as i32, i16::MAX as i32) as i16,
            right.clamp(i16::MIN as i32, i16::MAX as i32) as i16,
        )
    }

    fn next_psg_square_sample_at_rate(&mut self, channel: u8, sample_rate_hz: f32) -> i16 {
        let (enabled, phase, duty, volume, freq_reg) = match channel {
            1 => {
                let enabled = self.psg_square1_on;
                let phase = self.psg_square1_phase;
                let cnt_h = self.read_io16_raw(REG_SOUND1CNT_H);
                let cnt_x = self.read_io16_raw(REG_SOUND1CNT_X);
                let duty = ((cnt_h >> 6) & 0x3) as u8;
                let volume = self.psg_square1_volume;
                let freq_reg = cnt_x & 0x07FF;
                (enabled, phase, duty, volume, freq_reg)
            }
            _ => {
                let enabled = self.psg_square2_on;
                let phase = self.psg_square2_phase;
                let cnt_l = self.read_io16_raw(REG_SOUND2CNT_L);
                let cnt_h = self.read_io16_raw(REG_SOUND2CNT_H);
                let duty = ((cnt_l >> 6) & 0x3) as u8;
                let volume = self.psg_square2_volume;
                let freq_reg = cnt_h & 0x07FF;
                (enabled, phase, duty, volume, freq_reg)
            }
        };

        if !enabled || volume == 0 || freq_reg >= 2048 {
            return 0;
        }

        let duty_cycle = match duty {
            0 => 0.125_f32,
            1 => 0.25_f32,
            2 => 0.5_f32,
            _ => 0.75_f32,
        };

        // Approximate PSG timing against the current output sample clock.
        let effective_rate = sample_rate_hz.max(1.0);
        let frequency_hz = 131_072.0_f32 / (2048 - freq_reg) as f32;
        let mut phase_next = phase + (frequency_hz / effective_rate);
        while phase_next >= 1.0_f32 {
            phase_next -= 1.0_f32;
        }
        if channel == 1 {
            self.psg_square1_phase = phase_next;
        } else {
            self.psg_square2_phase = phase_next;
        }

        let polarity = if phase_next < duty_cycle {
            1.0_f32
        } else {
            -1.0_f32
        };
        let amp = (f32::from(volume) / 15.0_f32) * PSG_CHANNEL_FULL_SCALE;
        (polarity * amp).round() as i16
    }

    #[cfg(test)]
    fn next_psg_wave_sample(&mut self) -> i16 {
        self.next_psg_wave_sample_at_rate(AUDIO_BASE_RATE_HZ_U32 as f32)
    }

    fn next_psg_wave_sample_at_rate(&mut self, sample_rate_hz: f32) -> i16 {
        if !self.psg_wave_on {
            return 0;
        }

        let cnt_l = self.read_io16_raw(REG_SOUND3CNT_L);
        if (cnt_l & 0x0080) == 0 {
            self.psg_wave_on = false;
            self.psg_wave_phase = 0.0;
            self.psg_wave_last_sample_index = 0xFF;
            self.apply_wave_pending_writes();
            self.refresh_psg_status_bits();
            return 0;
        }

        let cnt_h = self.read_io16_raw(REG_SOUND3CNT_H);
        let cnt_x = self.read_io16_raw(REG_SOUND3CNT_X);
        let volume_code = ((cnt_h >> 13) & 0x7) as u8;
        let freq_reg = cnt_x & 0x07FF;
        if volume_code == 0 || freq_reg >= 2048 {
            return 0;
        }

        let two_bank_mode = (cnt_l & (1 << 5)) != 0;
        let bank_select = usize::from(self.psg_wave_play_bank & 1);
        let sample_count = if two_bank_mode { 64.0_f32 } else { 32.0_f32 };

        let effective_rate = sample_rate_hz.max(1.0);
        let frequency_hz = 65_536.0_f32 / (2048 - freq_reg) as f32;
        let mut phase_next = self.psg_wave_phase + (frequency_hz / effective_rate);
        while phase_next >= 1.0_f32 {
            phase_next -= 1.0_f32;
        }
        self.psg_wave_phase = phase_next;

        let sample_index = ((phase_next * sample_count) as usize).min(sample_count as usize - 1);
        let sample_index_u8 = sample_index as u8;
        if self.psg_wave_last_sample_index != sample_index_u8 {
            self.apply_wave_pending_writes();
            self.psg_wave_last_sample_index = sample_index_u8;
        }
        let (bank, bank_sample_index) = if two_bank_mode {
            if sample_index < 32 {
                (bank_select, sample_index)
            } else {
                (bank_select ^ 1, sample_index - 32)
            }
        } else {
            (bank_select, sample_index)
        };

        let wave_byte = self.wave_ram[bank * 16 + (bank_sample_index / 2)];
        let nibble = if (bank_sample_index & 1) == 0 {
            wave_byte >> 4
        } else {
            wave_byte & 0x0F
        };
        let centered = i16::from(nibble) - 8;

        let scaled = match volume_code {
            1 => centered * 16, // 100%
            2 => centered * 8,  // 50%
            3 => centered * 4,  // 25%
            4 => centered * 12, // 75%
            _ => centered * 16,
        };
        scaled.clamp(i16::MIN, i16::MAX)
    }

    #[cfg(test)]
    fn next_psg_noise_sample(&mut self) -> i16 {
        self.next_psg_noise_sample_at_rate(AUDIO_BASE_RATE_HZ_U32 as f32)
    }

    fn next_psg_noise_sample_at_rate(&mut self, sample_rate_hz: f32) -> i16 {
        if !self.psg_noise_on {
            return 0;
        }

        let cnt_h = self.read_io16_raw(REG_SOUND4CNT_H);
        let volume = self.psg_noise_volume;
        if volume == 0 {
            return 0;
        }

        let r = (cnt_h & 0x7) as u32;
        let width7 = (cnt_h & (1 << 3)) != 0;
        let s = ((cnt_h >> 4) & 0xF) as u32;
        let divisor = if r == 0 { 8 } else { r * 16 };
        let denominator = divisor << (s + 1);
        let effective_rate = sample_rate_hz.max(1.0);
        let frequency_hz = 524_288.0_f32 / denominator as f32;
        let step =
            (((frequency_hz / effective_rate) * PSG_NOISE_PHASE_ONE as f32).round() as u32).max(1);

        let mut phase = self.psg_noise_phase.saturating_add(step);
        while phase >= PSG_NOISE_PHASE_ONE {
            phase -= PSG_NOISE_PHASE_ONE;
            let bit = (self.psg_noise_lfsr ^ (self.psg_noise_lfsr >> 1)) & 1;
            self.psg_noise_lfsr = (self.psg_noise_lfsr >> 1) | (bit << 14);
            if width7 {
                self.psg_noise_lfsr = (self.psg_noise_lfsr & !(1 << 6)) | (bit << 6);
            }
        }
        self.psg_noise_phase = phase;

        let polarity = if (self.psg_noise_lfsr & 1) == 0 {
            1.0_f32
        } else {
            -1.0_f32
        };
        let amp = (f32::from(volume) / 15.0_f32) * PSG_CHANNEL_FULL_SCALE;
        (polarity * amp).round() as i16
    }

    fn read_io8(&self, offset: usize) -> u8 {
        if let Some((channel, byte)) = timer_reload_byte(offset) {
            let value = self.timer_counter[channel];
            return if byte == 0 {
                (value & 0x00FF) as u8
            } else {
                (value >> 8) as u8
            };
        }

        if let Some((channel, byte)) = timer_control_byte(offset) {
            let value = self.timer_control[channel];
            return if byte == 0 { (value & 0x00FF) as u8 } else { 0 };
        }

        if (WAVE_RAM_START..(WAVE_RAM_START + 0x10)).contains(&offset) {
            return self.read_wave_ram_io8(offset);
        }

        self.io[offset % IO_SIZE]
    }

    fn write_io8(&mut self, offset: usize, value: u8) {
        let index = offset % IO_SIZE;

        if let Some((channel, byte)) = timer_reload_byte(index) {
            if byte == 0 {
                self.timer_reload[channel] =
                    (self.timer_reload[channel] & 0xFF00) | u16::from(value);
            } else {
                self.timer_reload[channel] =
                    (self.timer_reload[channel] & 0x00FF) | (u16::from(value) << 8);
            }
            return;
        }

        if let Some((channel, byte)) = timer_control_byte(index) {
            if byte == 0 {
                self.timer_control[channel] =
                    (self.timer_control[channel] & 0xFF00) | u16::from(value);
                self.io[index] = value;
            } else {
                self.timer_control[channel] &= 0x00FF;
                self.io[index] = 0;
            }
            return;
        }

        match index {
            idx if (WAVE_RAM_START..(WAVE_RAM_START + 0x10)).contains(&idx) => {
                self.write_wave_ram_io8(idx, value);
            }
            REG_DISPSTAT => {
                // Bits 0-2 are status and read-only from CPU writes.
                self.io[index] = (self.io[index] & 0x07) | (value & !0x07);
            }
            REG_KEYCNT => {
                self.io[index] = value;
                self.update_keypad_irq_condition();
            }
            REG_KEYCNT_HI => {
                // KEYCNT high byte uses bits0-1 (key mask 8-9), bit6 (IRQ enable), bit7 (AND).
                self.io[index] = value & 0xC3;
                self.update_keypad_irq_condition();
            }
            idx if idx == REG_SOUND1CNT_X + 1 => {
                self.io[index] = value;
                if (value & 0x80) != 0 {
                    self.trigger_psg_square1();
                }
            }
            idx if idx == REG_SOUND2CNT_H + 1 => {
                self.io[index] = value;
                if (value & 0x80) != 0 {
                    self.trigger_psg_square2();
                }
            }
            REG_SOUND3CNT_L => {
                self.io[index] = value;
                if (value & 0x80) == 0 {
                    self.apply_wave_pending_writes();
                    self.psg_wave_on = false;
                    self.psg_wave_phase = 0.0;
                    self.psg_wave_play_bank = self.wave_ram_selected_bank() as u8;
                    self.psg_wave_last_sample_index = 0xFF;
                    self.psg_wave_length_ticks = 0;
                }
            }
            idx if idx == REG_SOUND3CNT_X + 1 => {
                self.io[index] = value;
                let wave_dac_enabled = (self.io[REG_SOUND3CNT_L] & 0x80) != 0;
                if (value & 0x80) != 0 && wave_dac_enabled {
                    self.trigger_psg_wave();
                }
            }
            idx if idx == REG_SOUND4CNT_H + 1 => {
                self.io[index] = value;
                if (value & 0x80) != 0 {
                    self.trigger_psg_noise();
                }
            }
            REG_SOUNDCNT_X => {
                self.io[index] = value & 0x80;
                if (value & 0x80) == 0 {
                    self.apply_wave_pending_writes();
                    self.psg_square1_on = false;
                    self.psg_square2_on = false;
                    self.psg_wave_on = false;
                    self.psg_noise_on = false;
                    self.psg_square1_phase = 0.0;
                    self.psg_square2_phase = 0.0;
                    self.psg_wave_phase = 0.0;
                    self.psg_noise_phase = 0;
                    self.psg_wave_last_sample_index = 0xFF;
                    self.psg_square1_length_ticks = 0;
                    self.psg_square2_length_ticks = 0;
                    self.psg_wave_length_ticks = 0;
                    self.psg_noise_length_ticks = 0;
                    self.psg_wave_play_bank = 0;
                    self.psg_square1_shadow_freq = 0;
                    self.psg_square1_sweep_counter = 0;
                    self.psg_square1_volume = 0;
                    self.psg_square2_volume = 0;
                    self.psg_noise_volume = 0;
                    self.psg_square1_env_period = 0;
                    self.psg_square2_env_period = 0;
                    self.psg_noise_env_period = 0;
                    self.psg_square1_env_counter = 0;
                    self.psg_square2_env_counter = 0;
                    self.psg_noise_env_counter = 0;
                    self.psg_frame_seq_accum = 0.0;
                    self.psg_frame_seq_step = 0;
                    self.clear_sound_fifo(FIFO_A_ADDR);
                    self.clear_sound_fifo(FIFO_B_ADDR);
                    self.direct_sound_a_latch = 0;
                    self.direct_sound_b_latch = 0;
                }
            }
            idx if idx == REG_SOUNDCNT_X + 1 => {
                self.io[index] = 0;
            }
            idx if idx == REG_SOUNDCNT_H + 1 => {
                // SOUNDCNT_H upper byte:
                // bit3  (overall bit11): reset FIFO A
                // bit7  (overall bit15): reset FIFO B
                if (value & (1 << 3)) != 0 {
                    self.clear_sound_fifo(FIFO_A_ADDR);
                }
                if (value & (1 << 7)) != 0 {
                    self.clear_sound_fifo(FIFO_B_ADDR);
                }
                // Reset bits are write-only pulse controls and do not persist.
                self.io[index] = value & !((1 << 3) | (1 << 7));
            }
            REG_VCOUNT | REG_VCOUNT_HI | REG_KEYINPUT | REG_KEYINPUT_HI => {
                // Read-only registers.
            }
            REG_IF | REG_IF_HI => {
                // IF is write-1-to-clear.
                self.io[index] &= !value;
            }
            REG_IME => {
                self.io[index] = value & 0x01;
            }
            REG_IME_HI => {
                self.io[index] = 0;
            }
            _ => {
                // Optional IO write trace for render timing diagnostics.
                if trace_io_enabled()
                    && matches!(index, 0x00..=0x01 | 0x04..=0x05 | 0x40..=0x4B | 0x50..=0x55)
                {
                    let old = self.io[index];
                    if old != value {
                        let vcount = self.read_io16_raw(0x06);
                        eprintln!(
                            "[io-trace] vcount={} io[{:#04X}] {:#04X} -> {:#04X}",
                            vcount, index, old, value
                        );
                    }
                }
                self.io[index] = value;
            }
        }

        self.refresh_psg_status_bits();

        if let Some(channel) = dma_channel_from_ctrl_high(index) {
            self.try_start_dma(channel);
        }
    }

    fn read_io16_raw(&self, offset: usize) -> u16 {
        let lo = self.io[offset % IO_SIZE] as u16;
        let hi = self.io[(offset + 1) % IO_SIZE] as u16;
        lo | (hi << 8)
    }

    fn write_io16_raw(&mut self, offset: usize, value: u16) {
        self.io[offset % IO_SIZE] = (value & 0x00FF) as u8;
        self.io[(offset + 1) % IO_SIZE] = (value >> 8) as u8;
    }

    fn read_io32_raw(&self, offset: usize) -> u32 {
        let lo = self.read_io16_raw(offset) as u32;
        let hi = self.read_io16_raw(offset + 2) as u32;
        lo | (hi << 16)
    }

    #[allow(dead_code)]
    fn write_io32_raw(&mut self, offset: usize, value: u32) {
        self.write_io16_raw(offset, (value & 0xFFFF) as u16);
        self.write_io16_raw(offset + 2, (value >> 16) as u16);
    }

    fn try_start_dma(&mut self, channel: usize) {
        let control = self.read_io16_raw(DMA_CTRL_OFFSETS[channel]);
        let enable = (control & DMA_ENABLE) != 0;

        if !self.dma_active[channel] && enable {
            // 0→1 enable transition: load internal registers from IO latches.
            // On real GBA hardware DMASAD/DMADAD are write-only latches; the
            // DMA unit copies them into internal working registers only on
            // this transition.
            self.dma_internal_src[channel] = self.read_io32_raw(DMA_SRC_OFFSETS[channel]);
            self.dma_internal_dst[channel] = self.read_io32_raw(DMA_DST_OFFSETS[channel]);
        }
        self.dma_active[channel] = enable;

        if !enable {
            return;
        }
        if dma_timing(control) != DMA_TIMING_IMMEDIATE {
            return;
        }
        self.run_dma(channel, DMA_TIMING_IMMEDIATE);
    }

    fn trigger_dma_timing(&mut self, timing: u16) {
        for channel in 0..DMA_CTRL_OFFSETS.len() {
            let control = self.read_io16_raw(DMA_CTRL_OFFSETS[channel]);
            if (control & DMA_ENABLE) == 0 {
                continue;
            }
            if dma_timing(control) != timing {
                continue;
            }
            self.run_dma(channel, timing);
        }
    }

    fn trigger_special_sound_dma(&mut self, timer_channel: usize) {
        let soundcnt_h = self.read_io16_raw(REG_SOUNDCNT_H);
        let fifo_a_timer = if (soundcnt_h & (1 << 10)) != 0 { 1 } else { 0 };
        let fifo_b_timer = if (soundcnt_h & (1 << 14)) != 0 { 1 } else { 0 };

        if timer_channel == fifo_a_timer {
            let sample_a = self.pop_sound_fifo_sample(FIFO_A_ADDR);
            let sample_a = if (soundcnt_h & (1 << 2)) != 0 {
                sample_a.saturating_mul(DIRECT_SOUND_SCALE_FULL)
            } else {
                sample_a.saturating_mul(DIRECT_SOUND_SCALE_HALF)
            };
            self.direct_sound_a_prev_latch = self.direct_sound_a_latch;
            self.direct_sound_a_latch = sample_a;
            self.direct_sound_a_cycles_since_latch = 0;
            self.direct_sound_a_latch_period_cycles =
                self.direct_sound_timer_period_cycles(timer_channel);
            if self.sound_fifo_len(FIFO_A_ADDR) <= 16 {
                self.trigger_special_dma_for_fifo(FIFO_A_ADDR);
            }
        }
        if timer_channel == fifo_b_timer {
            let sample_b = self.pop_sound_fifo_sample(FIFO_B_ADDR);
            let sample_b = if (soundcnt_h & (1 << 3)) != 0 {
                sample_b.saturating_mul(DIRECT_SOUND_SCALE_FULL)
            } else {
                sample_b.saturating_mul(DIRECT_SOUND_SCALE_HALF)
            };
            self.direct_sound_b_prev_latch = self.direct_sound_b_latch;
            self.direct_sound_b_latch = sample_b;
            self.direct_sound_b_cycles_since_latch = 0;
            self.direct_sound_b_latch_period_cycles =
                self.direct_sound_timer_period_cycles(timer_channel);
            if self.sound_fifo_len(FIFO_B_ADDR) <= 16 {
                self.trigger_special_dma_for_fifo(FIFO_B_ADDR);
            }
        }
    }

    fn mix_audio_output_sample(
        &mut self,
        sample_rate_hz: f32,
        left_direct: i32,
        right_direct: i32,
        direct_routed: bool,
    ) {
        if (self.read_io16_raw(REG_SOUNDCNT_X) & 0x0080) == 0 {
            return;
        }

        let psg_active = !disable_psg()
            && (self.psg_square1_on
                || self.psg_square2_on
                || self.psg_wave_on
                || self.psg_noise_on);
        let (left_direct, right_direct) = if direct_routed {
            (left_direct, right_direct)
        } else {
            (0, 0)
        };

        let (psg_left, psg_right) = if psg_active {
            self.mix_psg_sample_at_rate(sample_rate_hz)
        } else {
            (0, 0)
        };
        let left = left_direct + i32::from(psg_left);
        let right = right_direct + i32::from(psg_right);
        let bias = i32::from(self.read_io16_raw(REG_SOUNDBIAS) & SOUND_BIAS_LEVEL_MASK);
        let left_pcm = Self::mixed_sample_to_pcm(left, bias);
        let right_pcm = Self::mixed_sample_to_pcm(right, bias);

        let left = left_pcm;
        let right = right_pcm;
        let (mut left, mut right) = self.apply_audio_post_filter(left, right);
        if !self.bios_loaded {
            let slew_limit = no_bios_audio_slew_limit();
            if slew_limit > 0 {
                let (prev_left, prev_right) = if self.audio_samples.len() >= 2 {
                    let len = self.audio_samples.len();
                    (self.audio_samples[len - 2], self.audio_samples[len - 1])
                } else {
                    (0, 0)
                };
                left = Self::clamp_slew(left, prev_left, slew_limit);
                right = Self::clamp_slew(right, prev_right, slew_limit);
            }
        }

        if self.audio_samples.len() + 2 >= AUDIO_SAMPLE_BUFFER_LIMIT {
            self.audio_samples.clear();
        }
        self.audio_samples.push(left);
        self.audio_samples.push(right);
    }

    #[inline]
    fn mixed_sample_to_pcm(mixed: i32, bias: i32) -> i16 {
        let clipped = (mixed + bias).clamp(SOUND_DAC_MIN, SOUND_DAC_MAX);
        let centered = clipped - bias;
        (centered << SOUND_PCM_SHIFT).clamp(i16::MIN as i32, i16::MAX as i32) as i16
    }

    #[inline]
    fn clamp_slew(current: i16, previous: i16, limit: i32) -> i16 {
        let current = i32::from(current);
        let previous = i32::from(previous);
        let delta = current - previous;
        if delta > limit {
            (previous + limit) as i16
        } else if delta < -limit {
            (previous - limit) as i16
        } else {
            current as i16
        }
    }

    fn trigger_special_dma_for_fifo(&mut self, fifo_addr: u32) {
        for channel in 1..=2 {
            let control = self.read_io16_raw(DMA_CTRL_OFFSETS[channel]);
            if (control & DMA_ENABLE) == 0 {
                continue;
            }
            if dma_timing(control) != DMA_TIMING_SPECIAL {
                continue;
            }

            let dest = self.dma_internal_dst[channel] & !0x3;
            if dest != fifo_addr {
                continue;
            }

            self.run_dma(channel, DMA_TIMING_SPECIAL);
        }
    }

    fn set_oam_default_hidden(&mut self) {
        self.oam.fill(0);
        for attr0_hi in self.oam[1..].iter_mut().step_by(OAM_OBJ_STRIDE) {
            // attr0 bit9=1 while bit8=0 marks OBJ as disabled.
            *attr0_hi = 0x02;
        }
    }

    fn run_dma(&mut self, channel: usize, trigger_timing: u16) {
        let dst_offset = DMA_DST_OFFSETS[channel];
        let count_offset = DMA_COUNT_OFFSETS[channel];
        let ctrl_offset = DMA_CTRL_OFFSETS[channel];

        // Use internal working registers instead of IO latches.
        // On real GBA hardware, DMA source/dest registers are write-only
        // latches; the DMA unit operates on separate internal copies that
        // are loaded from the latches only on a 0→1 enable transition.
        let mut source = self.dma_internal_src[channel];
        let mut dest = self.dma_internal_dst[channel];
        let raw_count = self.read_io16_raw(count_offset);
        let mut control = self.read_io16_raw(ctrl_offset);

        let mut transfer_words = (control & DMA_TRANSFER_32BIT) != 0;
        let max_count = if channel == 3 { 0x1_0000 } else { 0x4000 };
        let mut count = if raw_count == 0 {
            max_count
        } else {
            raw_count as u32
        };

        let source_mask = if channel == 0 {
            0x07FF_FFFF
        } else {
            0x0FFF_FFFF
        };
        let dest_mask = if channel == 3 {
            0x0FFF_FFFF
        } else {
            0x07FF_FFFF
        };
        source &= source_mask;
        dest &= dest_mask;

        let source_mode = ((control >> DMA_SOURCE_MODE_SHIFT) & 0x3) as u8;
        let mut dest_mode = ((control >> DMA_DEST_MODE_SHIFT) & 0x3) as u8;
        let repeat = (control & DMA_REPEAT) != 0;
        let timing = dma_timing(control);

        if timing != trigger_timing {
            return;
        }

        let fifo_special = timing == DMA_TIMING_SPECIAL
            && (channel == 1 || channel == 2)
            && ((dest & !0x3) == FIFO_A_ADDR || (dest & !0x3) == FIFO_B_ADDR);
        if fifo_special {
            // Sound FIFO request always transfers four 32-bit words.
            // Hardware treats FIFO destination as fixed, regardless of DMA dest mode bits.
            count = 4;
            transfer_words = true;
            dest_mode = 2;
        }
        let dest_eeprom = self.eeprom_active_addr(dest);
        let source_eeprom = self.eeprom_active_addr(source);
        trace_eeprom_dma(
            channel,
            source,
            dest,
            count,
            transfer_words,
            timing,
            dest_eeprom,
            source_eeprom,
        );
        let eeprom_dma_write = dest_eeprom && !source_eeprom;
        if eeprom_dma_write {
            self.eeprom
                .borrow_mut()
                .set_dma_write_hint(Some(count as usize));
        }
        let unit_size = if transfer_words { 4 } else { 2 };
        let mut transfer_index = 0u32;

        while count != 0 {
            if transfer_words {
                let mut value = self.read32(source & !3);
                if fifo_special
                    && !self.bios_loaded
                    && sanitize_fifo_pointer_words_enabled()
                    && looks_like_non_pcm_fifo_word(
                        value,
                        source & !3,
                        sanitize_fifo_exec_pages_enabled()
                            && self.iwram_addr_recently_executed(source & !3),
                    )
                {
                    value = 0;
                }
                if fifo_special {
                    trace_fifo_dma_word(channel, source & !3, dest & !3, transfer_index, value);
                }
                self.write32(dest & !3, value);
            } else {
                let value = self.read16(source & !1);
                self.write16(dest & !1, value);
            }

            source = dma_step_address(source, source_mode, unit_size, false);
            dest = dma_step_address(dest, dest_mode, unit_size, true);
            transfer_index = transfer_index.wrapping_add(1);
            count -= 1;
        }
        if eeprom_dma_write {
            self.eeprom.borrow_mut().set_dma_write_hint(None);
        }

        let keep_enabled = repeat && timing != DMA_TIMING_IMMEDIATE;
        if !keep_enabled {
            control &= !DMA_ENABLE;
        }

        // For repeat + dest_mode=3 (increment/reload), reload dest from
        // the IO latch so the game's original destination is restored.
        let dest_writeback = if keep_enabled && dest_mode == 3 {
            self.read_io32_raw(dst_offset) & dest_mask
        } else {
            dest
        };

        // Update internal working registers.
        self.dma_internal_src[channel] = source;
        self.dma_internal_dst[channel] = dest_writeback;

        // On real GBA hardware DMA source/dest registers (DMASAD/DMADAD)
        // are write-only latches: only the CPU can write them, the DMA
        // unit never writes back the advanced address.  We must NOT
        // update the IO register here; otherwise a disable → re-enable
        // cycle (as the m4a engine does each VBlank) would incorrectly
        // pick up the advanced address instead of the latch value the
        // game originally programmed.
        //
        // Only the control register is updated (to clear the enable bit
        // when a non-repeat DMA completes).
        self.write_io16_raw(ctrl_offset, control);

        // Track active state so the next 0→1 transition is detected.
        self.dma_active[channel] = (control & DMA_ENABLE) != 0;

        if (control & DMA_IRQ_ENABLE) != 0 {
            self.request_irq(1 << (8 + channel));
        }
    }
}

fn dma_channel_from_ctrl_high(offset: usize) -> Option<usize> {
    DMA_CTRL_OFFSETS
        .iter()
        .position(|control_offset| offset == control_offset + 1)
}

fn timer_base_offset(channel: usize) -> usize {
    TIMER_BASE + TIMER_STRIDE * channel
}

fn timer_reload_byte(offset: usize) -> Option<(usize, usize)> {
    if !(TIMER_BASE..(TIMER_BASE + TIMER_STRIDE * TIMER_COUNT)).contains(&offset) {
        return None;
    }

    let local = offset - TIMER_BASE;
    let channel = local / TIMER_STRIDE;
    let byte = local % TIMER_STRIDE;
    if byte <= 1 {
        Some((channel, byte))
    } else {
        None
    }
}

fn timer_control_byte(offset: usize) -> Option<(usize, usize)> {
    if !(TIMER_BASE..(TIMER_BASE + TIMER_STRIDE * TIMER_COUNT)).contains(&offset) {
        return None;
    }

    let local = offset - TIMER_BASE;
    let channel = local / TIMER_STRIDE;
    let byte = local % TIMER_STRIDE;
    if byte >= TIMER_CTRL_OFFSET {
        Some((channel, byte - TIMER_CTRL_OFFSET))
    } else {
        None
    }
}

fn dma_timing(control: u16) -> u16 {
    (control >> DMA_TIMING_SHIFT) & 0x3
}

fn fifo_addr_from_io_addr(addr: u32) -> Option<u32> {
    let aligned = addr & !0x3;
    if aligned == FIFO_A_ADDR || aligned == FIFO_B_ADDR {
        Some(aligned)
    } else {
        None
    }
}

fn vram_index(addr: u32) -> usize {
    let offset = (addr as usize - VRAM_BASE as usize) & 0x1_FFFF;
    if offset < VRAM_MIRROR_START {
        offset
    } else {
        VRAM_MIRROR_BASE + (offset - VRAM_MIRROR_START)
    }
}

fn duplicate_byte_write(memory: &mut [u8], base: u32, addr: u32, value: u8) {
    let index = ((addr as usize - base as usize) % memory.len()) & !1;
    memory[index] = value;
    memory[index + 1] = value;
}

fn bits_to_usize(bits: &[u8]) -> usize {
    bits.iter()
        .fold(0usize, |acc, bit| (acc << 1) | usize::from(bit & 1))
}

fn bits_to_u8(bits: &[u8]) -> u8 {
    bits.iter().fold(0u8, |acc, bit| (acc << 1) | (bit & 1))
}

fn contains_tag(haystack: &[u8], needle: &[u8]) -> bool {
    if needle.is_empty() {
        return true;
    }
    haystack.windows(needle.len()).any(|chunk| chunk == needle)
}

fn detect_save_type(rom: &[u8]) -> SaveType {
    if contains_tag(rom, FLASH1M_TAG) {
        SaveType::Flash128K
    } else if contains_tag(rom, FLASH512_TAG) || contains_tag(rom, FLASH_TAG) {
        SaveType::Flash64K
    } else if contains_tag(rom, EEPROM_TAG) {
        SaveType::Eeprom
    } else if contains_tag(rom, SRAM_TAG) {
        SaveType::Sram
    } else {
        SaveType::None
    }
}

fn save_type_override_from_env() -> Option<SaveType> {
    let raw = std::env::var("GBA_SAVE_TYPE").ok()?;
    let normalized = raw.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "none" => Some(SaveType::None),
        "sram" => Some(SaveType::Sram),
        "eeprom" => Some(SaveType::Eeprom),
        "flash64k" | "flash" => Some(SaveType::Flash64K),
        "flash128k" | "flash1m" => Some(SaveType::Flash128K),
        _ => None,
    }
}

fn env_flag(name: &str) -> bool {
    let value = match std::env::var(name) {
        Ok(value) => value,
        Err(_) => return false,
    };
    let lowered = value.trim().to_ascii_lowercase();
    !(lowered.is_empty()
        || lowered == "0"
        || lowered == "false"
        || lowered == "off"
        || lowered == "no")
}

fn trace_eeprom_enabled() -> bool {
    *TRACE_EEPROM_ENABLED.get_or_init(|| env_flag("GBA_TRACE_EEPROM"))
}

fn trace_eeprom_dma_enabled() -> bool {
    *TRACE_EEPROM_DMA_ENABLED.get_or_init(|| env_flag("GBA_TRACE_EEPROM_DMA"))
}

fn trace_main_flags_enabled() -> bool {
    *TRACE_MAIN_FLAGS_ENABLED.get_or_init(|| env_flag("GBA_TRACE_MAIN_FLAGS"))
}

fn trace_sound_enabled() -> bool {
    *TRACE_SOUND_ENABLED.get_or_init(|| env_flag("GBA_TRACE_SOUND"))
}

fn trace_sound_underrun_enabled() -> bool {
    *TRACE_SOUND_UNDERRUN_ENABLED.get_or_init(|| env_flag("GBA_TRACE_SOUND_UNDERRUN"))
}

fn trace_fifo_dma_enabled() -> bool {
    *TRACE_FIFO_DMA_ENABLED.get_or_init(|| env_flag("GBA_TRACE_FIFO_DMA"))
}

fn trace_io_enabled() -> bool {
    *TRACE_IO_ENABLED.get_or_init(|| env_flag("GBA_TRACE_IO"))
}

fn direct_sound_interpolate_enabled(_has_bios: bool) -> bool {
    let configured = *DIRECT_SOUND_INTERPOLATE.get_or_init(|| {
        match std::env::var("GBA_DIRECT_SOUND_INTERPOLATE") {
            Ok(_) => Some(env_flag("GBA_DIRECT_SOUND_INTERPOLATE")),
            Err(_) => None,
        }
    });
    // Enable interpolation by default in all modes to smooth
    // Direct Sound FIFO staircase transitions that cause audible clicks.
    configured.unwrap_or(true)
}

fn sanitize_fifo_pointer_words_enabled() -> bool {
    *AUDIO_SANITIZE_FIFO_POINTER_WORDS.get_or_init(|| {
        // In no-BIOS mode, some titles can transiently stream pointer-like words
        // into DirectSound FIFO before the software mixer is fully settled.
        // Keep this enabled by default to suppress crackle from obvious non-PCM words.
        match std::env::var("GBA_AUDIO_SANITIZE_FIFO_POINTERS") {
            Ok(_) => env_flag("GBA_AUDIO_SANITIZE_FIFO_POINTERS"),
            Err(_) => true,
        }
    })
}

fn sanitize_fifo_exec_pages_enabled() -> bool {
    *AUDIO_SANITIZE_FIFO_EXEC_PAGES.get_or_init(|| {
        // Also suppress DMA words pulled from IWRAM pages that were executed as code.
        // This targets no-BIOS cases where sound DMA source pointers drift into
        // instruction regions and produce periodic high-pitched noise.
        match std::env::var("GBA_AUDIO_SANITIZE_EXEC_PAGES") {
            Ok(_) => env_flag("GBA_AUDIO_SANITIZE_EXEC_PAGES"),
            Err(_) => true,
        }
    })
}

fn looks_like_non_pcm_fifo_word(
    value: u32,
    _source_addr: u32,
    source_recently_executed: bool,
) -> bool {
    if value == 0 {
        return false;
    }

    if looks_like_rom_pointer_word(value)
        || looks_like_ram_pointer_word(value)
        || looks_like_io_vram_pointer_word(value)
    {
        return true;
    }

    if source_recently_executed {
        return true;
    }

    // Common control/sentinel words seen in broken no-BIOS sound streams.
    if value == 0x8000_0000 || value == 0xFFFF_FFFF {
        return true;
    }

    // Treat sparse control-style words (mostly 0x00/0xFF) as non-PCM.
    // Legitimate PCM can contain these occasionally, but this significantly
    // reduces harsh zipper/crackle in titles that DMA metadata by mistake.
    let bytes = value.to_le_bytes();
    let sparse_bytes = bytes
        .iter()
        .filter(|&&byte| byte == 0x00 || byte == 0xFF)
        .count();
    if sparse_bytes >= 3 && bytes.iter().any(|&byte| byte != 0x00 && byte != 0xFF) {
        return true;
    }

    // Four-letter ASCII tags (for example, "Smsh") are structure metadata.
    bytes.iter().all(u8::is_ascii_alphabetic)
}

fn looks_like_rom_pointer_word(value: u32) -> bool {
    (ROM0_BASE..=0x0DFF_FFFF).contains(&value)
}

fn looks_like_ram_pointer_word(value: u32) -> bool {
    (EWRAM_BASE..=0x03FF_FFFF).contains(&value)
}

fn looks_like_io_vram_pointer_word(value: u32) -> bool {
    (IO_BASE..=0x07FF_FFFF).contains(&value)
}

fn disable_psg() -> bool {
    *DISABLE_PSG.get_or_init(|| env_flag("GBA_DISABLE_PSG"))
}

fn audio_post_filter_enabled() -> bool {
    *AUDIO_POST_FILTER_ENABLED.get_or_init(|| {
        // Default ON: the GBA's 10-bit DAC output is followed by an analog
        // reconstruction filter on real hardware.  Without a digital low-pass
        // the quantised staircase waveform contains audible high-frequency
        // artifacts (perceived as "noise" or buzzing), especially at the
        // moderate amplitudes typical of in-game music.
        match std::env::var("GBA_AUDIO_POST_FILTER") {
            Ok(val) => val.trim() == "1" || val.trim().eq_ignore_ascii_case("true"),
            Err(_) => true,
        }
    })
}

fn audio_post_filter_hpf_hz() -> f32 {
    *AUDIO_POST_FILTER_HPF_HZ.get_or_init(|| {
        env_f32(
            "GBA_AUDIO_POST_FILTER_HPF_HZ",
            AUDIO_POST_FILTER_HPF_HZ_DEFAULT,
            1.0,
            2_000.0,
        )
    })
}

fn audio_post_filter_lpf_hz() -> f32 {
    *AUDIO_POST_FILTER_LPF_HZ.get_or_init(|| {
        env_f32(
            "GBA_AUDIO_POST_FILTER_LPF_HZ",
            AUDIO_POST_FILTER_LPF_HZ_DEFAULT,
            500.0,
            20_000.0,
        )
    })
}

fn audio_post_filter_lpf_stages() -> u8 {
    *AUDIO_POST_FILTER_LPF_STAGES.get_or_init(|| {
        std::env::var("GBA_AUDIO_POST_FILTER_LPF_STAGES")
            .ok()
            .and_then(|value| value.trim().parse::<u8>().ok())
            .map(|value| value.clamp(1, AUDIO_POST_FILTER_MAX_STAGES as u8))
            .unwrap_or(AUDIO_POST_FILTER_LPF_STAGES_DEFAULT)
    })
}

fn audio_fifo_underrun_decay_enabled() -> bool {
    *AUDIO_FIFO_UNDERRUN_DECAY.get_or_init(|| {
        match std::env::var("GBA_AUDIO_FIFO_UNDERRUN_DECAY") {
            Ok(_) => env_flag("GBA_AUDIO_FIFO_UNDERRUN_DECAY"),
            Err(_) => true,
        }
    })
}

fn no_bios_audio_slew_limit() -> i32 {
    *AUDIO_NO_BIOS_SLEW_LIMIT.get_or_init(|| {
        #[cfg(test)]
        let default_limit = 0;
        #[cfg(not(test))]
        let default_limit = AUDIO_NO_BIOS_SLEW_LIMIT_DEFAULT;

        std::env::var("GBA_AUDIO_NO_BIOS_SLEW_LIMIT")
            .ok()
            .and_then(|value| value.trim().parse::<i32>().ok())
            .map(|value| value.clamp(0, i16::MAX as i32))
            .unwrap_or(default_limit)
    })
}

fn env_f32(name: &str, default: f32, min: f32, max: f32) -> f32 {
    std::env::var(name)
        .ok()
        .and_then(|value| value.trim().parse::<f32>().ok())
        .filter(|value| value.is_finite())
        .map(|value| value.clamp(min, max))
        .unwrap_or(default)
}

fn legacy_scanline_snapshot_insert_offset(payload: &[u8]) -> Result<usize, &'static str> {
    let mut r = crate::state::StateReader::new(payload);

    r.read_slice(EWRAM_SIZE)?;
    r.read_slice(IWRAM_SIZE)?;
    r.read_slice(IO_SIZE)?;
    r.read_slice(32)?; // wave RAM
    r.read_slice(PRAM_SIZE)?;
    r.read_slice(VRAM_SIZE)?;
    r.read_slice(OAM_SIZE)?;
    r.read_slice(SRAM_SIZE)?;

    for _ in 0..IWRAM_EXEC_BITMAP_WORDS {
        r.read_u64()?;
    }
    for _ in 0..TIMER_COUNT {
        r.read_u16()?;
    }
    for _ in 0..TIMER_COUNT {
        r.read_u16()?;
    }
    for _ in 0..TIMER_COUNT {
        r.read_u16()?;
    }

    let fifo_a_len = r.read_u32()? as usize;
    r.read_slice(fifo_a_len)?;
    let fifo_b_len = r.read_u32()? as usize;
    r.read_slice(fifo_b_len)?;

    r.read_i8()?;
    r.read_i8()?;
    r.read_u16()?;
    r.read_u16()?;
    for _ in 0..4 {
        r.read_i16()?;
    }
    for _ in 0..5 {
        r.read_u32()?;
    }

    r.read_f32()?;
    r.read_f32()?;
    r.read_bool()?;
    r.read_bool()?;
    r.read_f32()?;
    r.read_u32()?;
    r.read_bool()?;
    r.read_u8()?;
    r.read_u8()?;
    for _ in 0..32 {
        r.read_bool()?;
        r.read_u8()?;
    }
    r.read_bool()?;
    r.read_u16()?;
    r.read_f32()?;
    r.read_u8()?;
    for _ in 0..4 {
        r.read_u16()?;
    }
    r.read_u16()?;
    r.read_u8()?;
    for _ in 0..9 {
        r.read_u8()?;
    }

    for _ in 0..4 {
        r.read_f32()?;
    }
    for _ in 0..AUDIO_POST_FILTER_MAX_STAGES {
        r.read_f32()?;
    }
    for _ in 0..AUDIO_POST_FILTER_MAX_STAGES {
        r.read_f32()?;
    }

    for _ in 0..4 {
        r.read_u32()?;
    }
    for _ in 0..4 {
        r.read_u32()?;
    }
    for _ in 0..4 {
        r.read_bool()?;
    }

    for _ in 0..GBA_VISIBLE_LINES {
        r.read_slice(SCANLINE_IO_SNAPSHOT_SIZE)?;
    }
    for _ in 0..GBA_VISIBLE_LINES {
        r.read_bool()?;
    }

    Ok(r.position())
}

#[cfg(test)]
pub(crate) fn strip_newer_scanline_snapshots_for_legacy_test(payload: &[u8]) -> Vec<u8> {
    let insert_at = legacy_scanline_snapshot_insert_offset(payload)
        .expect("payload should contain scanline IO");
    let remove_end = insert_at + LEGACY_MISSING_SCANLINE_SNAPSHOT_SIZE;
    let mut stripped = Vec::with_capacity(payload.len() - LEGACY_MISSING_SCANLINE_SNAPSHOT_SIZE);
    stripped.extend_from_slice(&payload[..insert_at]);
    stripped.extend_from_slice(&payload[remove_end..]);
    stripped
}

fn trace_limit() -> u32 {
    *TRACE_LIMIT.get_or_init(|| {
        std::env::var("GBA_TRACE_LIMIT")
            .ok()
            .and_then(|value| value.parse::<u32>().ok())
            .filter(|value| *value > 0)
            .unwrap_or(TRACE_LIMIT_DEFAULT)
    })
}

fn trace_main_flags_write(addr: u32, width: u8, value: u32) {
    if !trace_main_flags_enabled() {
        return;
    }
    if !(0x0300_2B50..=0x0300_2B90).contains(&addr) {
        return;
    }

    let slot = TRACE_MAIN_FLAGS_COUNT.fetch_add(1, Ordering::Relaxed);
    if slot >= trace_limit() {
        return;
    }

    eprintln!(
        "[gba:trace:main-flags] slot={}/{} kind=write width={} addr={:#010X} value={:#010X}",
        slot + 1,
        trace_limit(),
        width,
        addr,
        value
    );
}

fn trace_sound_write(addr: u32, width: u8, value: u32) {
    if !trace_sound_enabled() {
        return;
    }

    let aligned = addr & !0x3;
    let in_sound_io = (0x0400_0060..=0x0400_00A7).contains(&addr);
    let is_fifo = aligned == FIFO_A_ADDR || aligned == FIFO_B_ADDR;
    if !(in_sound_io || is_fifo) {
        return;
    }

    let slot = TRACE_SOUND_COUNT.fetch_add(1, Ordering::Relaxed);
    if slot >= trace_limit() {
        return;
    }

    eprintln!(
        "[gba:trace:sound] slot={}/{} kind=write width={} addr={:#010X} value={:#010X}",
        slot + 1,
        trace_limit(),
        width,
        addr,
        value
    );
}

fn trace_sound_underrun(fifo_addr: u32, fifo_a_len: usize, fifo_b_len: usize) {
    if !trace_sound_underrun_enabled() {
        return;
    }
    let slot = TRACE_SOUND_UNDERRUN_COUNT.fetch_add(1, Ordering::Relaxed);
    if slot >= trace_limit() {
        return;
    }
    eprintln!(
        "[gba:trace:sound] slot={}/{} kind=fifo-underrun fifo={} fifo_a_len={} fifo_b_len={}",
        slot + 1,
        trace_limit(),
        if fifo_addr == FIFO_A_ADDR { "A" } else { "B" },
        fifo_a_len,
        fifo_b_len
    );
}

fn trace_fifo_dma_word(channel: usize, source: u32, dest: u32, transfer_index: u32, value: u32) {
    if !trace_fifo_dma_enabled() {
        return;
    }
    let slot = TRACE_FIFO_DMA_COUNT.fetch_add(1, Ordering::Relaxed);
    if slot >= trace_limit() {
        return;
    }
    let b0 = (value & 0xFF) as u8;
    let b1 = ((value >> 8) & 0xFF) as u8;
    let b2 = ((value >> 16) & 0xFF) as u8;
    let b3 = ((value >> 24) & 0xFF) as u8;
    eprintln!(
        "[gba:trace:fifo-dma] slot={}/{} ch={} idx={} src={:#010X} dst={:#010X} word={:#010X} bytes={:02X} {:02X} {:02X} {:02X}",
        slot + 1,
        trace_limit(),
        channel,
        transfer_index,
        source,
        dest,
        value,
        b0,
        b1,
        b2,
        b3
    );
}

fn trace_eeprom_write(addr_bits: usize, address: usize, block: usize, bytes: &[u8; 8]) {
    if !trace_eeprom_enabled() {
        return;
    }
    let slot = TRACE_EEPROM_COUNT.fetch_add(1, Ordering::Relaxed);
    if slot >= trace_limit() {
        return;
    }
    eprintln!(
        "[gba:trace:eeprom] slot={}/{} kind=write addr_bits={} raw_addr={:#06X} block={} byte_off={:#06X} data={:02X}{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}",
        slot + 1,
        trace_limit(),
        addr_bits,
        address,
        block,
        block * 8,
        bytes[0],
        bytes[1],
        bytes[2],
        bytes[3],
        bytes[4],
        bytes[5],
        bytes[6],
        bytes[7]
    );
}

fn trace_eeprom_read(addr_bits: usize, address: usize, block: usize, bytes: &[u8; 8]) {
    if !trace_eeprom_enabled() {
        return;
    }
    let slot = TRACE_EEPROM_COUNT.fetch_add(1, Ordering::Relaxed);
    if slot >= trace_limit() {
        return;
    }
    eprintln!(
        "[gba:trace:eeprom] slot={}/{} kind=read addr_bits={} raw_addr={:#06X} block={} byte_off={:#06X} data={:02X}{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}",
        slot + 1,
        trace_limit(),
        addr_bits,
        address,
        block,
        block * 8,
        bytes[0],
        bytes[1],
        bytes[2],
        bytes[3],
        bytes[4],
        bytes[5],
        bytes[6],
        bytes[7]
    );
}

fn trace_eeprom_dma(
    channel: usize,
    source: u32,
    dest: u32,
    count: u32,
    transfer_words: bool,
    timing: u16,
    dest_eeprom: bool,
    source_eeprom: bool,
) {
    if !trace_eeprom_dma_enabled() {
        return;
    }
    if !(dest_eeprom || source_eeprom) {
        return;
    }

    let slot = TRACE_EEPROM_DMA_COUNT.fetch_add(1, Ordering::Relaxed);
    if slot >= trace_limit() {
        return;
    }

    eprintln!(
        "[gba:trace:eeprom-dma] slot={}/{} ch={} src={:#010X} dst={:#010X} count={} unit={} timing={} src_eeprom={} dst_eeprom={}",
        slot + 1,
        trace_limit(),
        channel,
        source,
        dest,
        count,
        if transfer_words { 4 } else { 2 },
        timing,
        if source_eeprom { 1 } else { 0 },
        if dest_eeprom { 1 } else { 0 }
    );
}

fn dma_step_address(addr: u32, mode: u8, unit_size: u32, is_dest: bool) -> u32 {
    match mode {
        0 => addr.wrapping_add(unit_size),
        1 => addr.wrapping_sub(unit_size),
        2 => addr,
        3 => {
            if is_dest {
                addr.wrapping_add(unit_size)
            } else {
                // Prohibited for source: treat as increment.
                addr.wrapping_add(unit_size)
            }
        }
        _ => addr.wrapping_add(unit_size),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mix_audio_samples_for_test(bus: &mut GbaBus, sample_count: u32) {
        bus.mix_audio_for_cycles(sample_count.saturating_mul(AUDIO_BASE_CYCLES_PER_SAMPLE));
    }

    fn expected_pcm_from_mixed_units(mixed: i32) -> i16 {
        let clipped = (mixed + 0x0200).clamp(0, 0x03FF);
        let centered = clipped - 0x0200;
        (centered << SOUND_PCM_SHIFT) as i16
    }

    fn eeprom_write_bits(bus: &mut GbaBus, bits: &[u8]) {
        for bit in bits {
            bus.write16(EEPROM_BASE, u16::from(bit & 1));
        }
    }

    fn eeprom_write_bits_with_dma_hint(bus: &mut GbaBus, bits: &[u8], dma_count: usize) {
        bus.eeprom.borrow_mut().set_dma_write_hint(Some(dma_count));
        for bit in bits {
            bus.write16(EEPROM_BASE, u16::from(bit & 1));
        }
        bus.eeprom.borrow_mut().set_dma_write_hint(None);
    }

    fn eeprom_read_bits(bus: &GbaBus, count: usize) -> Vec<u8> {
        (0..count)
            .map(|_| (bus.read16(EEPROM_BASE) as u8) & 1)
            .collect()
    }

    fn bits_from_bytes(bytes: &[u8]) -> Vec<u8> {
        let mut bits = Vec::with_capacity(bytes.len() * 8);
        for byte in bytes {
            for shift in (0..8).rev() {
                bits.push((byte >> shift) & 1);
            }
        }
        bits
    }

    fn rom_with_tag(tag: &[u8]) -> Vec<u8> {
        let mut rom = vec![0; 0x200];
        rom[0x80..(0x80 + tag.len())].copy_from_slice(tag);
        rom
    }

    fn flash_unlock(bus: &mut GbaBus) {
        bus.write8(SRAM_BASE + 0x5555, 0xAA);
        bus.write8(SRAM_BASE + 0x2AAA, 0x55);
    }

    fn flash_command(bus: &mut GbaBus, command: u8) {
        flash_unlock(bus);
        bus.write8(SRAM_BASE + 0x5555, command);
    }

    #[test]
    fn rom_windows_are_mirrored() {
        let mut bus = GbaBus::default();
        bus.load_rom(&[0x11, 0x22, 0x33, 0x44]);

        assert_eq!(bus.read8(0x0800_0000), 0x11);
        assert_eq!(bus.read8(0x0A00_0001), 0x22);
        assert_eq!(bus.read8(0x0C00_0002), 0x33);
        assert_eq!(bus.read8(0x0800_0008), 0x11);
    }

    #[test]
    fn word_round_trip_in_iwram() {
        let mut bus = GbaBus::default();
        bus.write32(0x0300_0000, 0xDEAD_BEEF);
        assert_eq!(bus.read32(0x0300_0000), 0xDEAD_BEEF);
    }

    #[test]
    fn if_register_is_write_one_to_clear() {
        let mut bus = GbaBus::default();
        bus.reset();
        bus.request_irq(IRQ_VBLANK | IRQ_HBLANK);
        assert_eq!(bus.read16(0x0400_0202), IRQ_VBLANK | IRQ_HBLANK);

        bus.write16(0x0400_0202, IRQ_VBLANK);
        assert_eq!(bus.read16(0x0400_0202), IRQ_HBLANK);
    }

    #[test]
    fn keyinput_reflects_pressed_mask_with_active_low_bits() {
        let mut bus = GbaBus::default();
        bus.reset();
        assert_eq!(bus.read16(0x0400_0130), 0x03FF);

        // A + START pressed.
        bus.set_keyinput_pressed_mask((1 << 0) | (1 << 3));
        assert_eq!(bus.read16(0x0400_0130), 0x03F6);

        // No key pressed.
        bus.set_keyinput_pressed_mask(0);
        assert_eq!(bus.read16(0x0400_0130), 0x03FF);
    }

    #[test]
    fn reset_without_bios_uses_post_bios_defaults() {
        let mut bus = GbaBus::default();
        bus.reset();

        assert_eq!(bus.read8(0x0400_0300), 1);
        assert_eq!(bus.read16(0x0400_0088), 0x0200);
    }

    #[test]
    fn reset_with_bios_uses_cold_boot_defaults() {
        let mut bus = GbaBus::default();
        bus.load_bios(&[0x00]);
        bus.reset();

        assert_eq!(bus.read8(0x0400_0300), 0);
        assert_eq!(bus.read16(0x0400_0088), 0x0000);
    }

    #[test]
    fn keycnt_irq_or_mode_requests_keypad_irq_when_any_selected_key_is_pressed() {
        let mut bus = GbaBus::default();
        bus.reset();
        bus.write16(0x0400_0200, IRQ_KEYPAD); // IE: keypad irq on
        bus.write16(0x0400_0132, (1 << 14) | (1 << 0)); // irq enable + A key

        bus.set_keyinput_pressed_mask(1 << 1);
        assert_eq!(bus.read16(0x0400_0202) & IRQ_KEYPAD, 0);

        bus.set_keyinput_pressed_mask(1 << 0);
        assert_ne!(bus.read16(0x0400_0202) & IRQ_KEYPAD, 0);
    }

    #[test]
    fn keycnt_irq_and_mode_requires_all_selected_keys() {
        let mut bus = GbaBus::default();
        bus.reset();
        bus.write16(0x0400_0200, IRQ_KEYPAD); // IE: keypad irq on
        bus.write16(0x0400_0132, (1 << 15) | (1 << 14) | (1 << 0) | (1 << 3));

        bus.set_keyinput_pressed_mask(1 << 0);
        assert_eq!(bus.read16(0x0400_0202) & IRQ_KEYPAD, 0);

        bus.set_keyinput_pressed_mask((1 << 0) | (1 << 3));
        assert_ne!(bus.read16(0x0400_0202) & IRQ_KEYPAD, 0);
    }

    #[test]
    fn reset_hides_all_oam_objects() {
        let mut bus = GbaBus::default();
        bus.reset();

        for obj_index in 0..(OAM_SIZE / OAM_OBJ_STRIDE) {
            let attr0 = bus.read16(OAM_BASE + (obj_index as u32) * (OAM_OBJ_STRIDE as u32));
            assert_eq!(attr0 & (1 << 9), 1 << 9);
            assert_eq!(attr0 & (1 << 8), 0);
        }
    }

    #[test]
    fn vram_upper_32k_is_mirrored_to_0x06010000() {
        let mut bus = GbaBus::default();
        bus.reset();

        bus.write16(VRAM_BASE + 0x10_000, 0x1234);
        assert_eq!(bus.read16(VRAM_BASE + 0x18_000), 0x1234);

        bus.write16(VRAM_BASE + 0x1F_FFE, 0xBEEF);
        assert_eq!(bus.read16(VRAM_BASE + 0x17_FFE), 0xBEEF);
    }

    #[test]
    fn byte_writes_to_pram_and_oam_duplicate_across_halfword() {
        let mut bus = GbaBus::default();
        bus.reset();

        bus.write8(PRAM_BASE + 1, 0x5A);
        assert_eq!(bus.read16(PRAM_BASE), 0x5A5A);

        bus.write8(OAM_BASE + 5, 0x3C);
        assert_eq!(bus.read16(OAM_BASE + 4), 0x3C3C);
    }

    #[test]
    fn byte_write_to_mirrored_vram_updates_mirrored_halfword() {
        let mut bus = GbaBus::default();
        bus.reset();

        bus.write8(VRAM_BASE + 0x18_001, 0x66);
        assert_eq!(bus.read16(VRAM_BASE + 0x10_000), 0x6666);
        assert_eq!(bus.read16(VRAM_BASE + 0x18_000), 0x6666);
    }

    #[test]
    fn eeprom_write_then_read_round_trip_works_for_512b_protocol() {
        let mut bus = GbaBus::default();
        let rom = rom_with_tag(EEPROM_TAG);
        bus.load_rom(&rom);
        bus.reset();

        let payload = [0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC, 0xDE, 0xF0];
        let mut write_bits = vec![1, 0];
        write_bits.extend([0, 0, 0, 0, 0, 1]); // address=1 (6-bit)
        write_bits.extend(bits_from_bytes(&payload));
        write_bits.push(0); // stop
        eeprom_write_bits(&mut bus, &write_bits);

        let mut read_cmd = vec![1, 1];
        read_cmd.extend([0, 0, 0, 0, 0, 1]); // address=1
        read_cmd.push(0);
        eeprom_write_bits(&mut bus, &read_cmd);

        let bits = eeprom_read_bits(&bus, 68);
        assert_eq!(&bits[0..4], &[0, 0, 0, 0]);
        assert_eq!(&bits[4..], bits_from_bytes(&payload).as_slice());
    }

    #[test]
    fn eeprom_dma_hints_prevent_early_6bit_finalize_on_8k_commands() {
        let mut bus = GbaBus::default();
        let rom = rom_with_tag(EEPROM_TAG);
        bus.load_rom(&rom);
        bus.reset();

        let address = 0x0123usize;
        let payload = [0xDE, 0xAD, 0xBE, 0xEF, 0x55, 0xAA, 0xC3, 0x3C];

        let mut write_bits = vec![1, 0];
        for shift in (0..14).rev() {
            write_bits.push(((address >> shift) & 1) as u8);
        }
        write_bits.extend(bits_from_bytes(&payload));
        write_bits.push(0);
        eeprom_write_bits_with_dma_hint(&mut bus, &write_bits, 81);

        let mut read_cmd = vec![1, 1];
        for shift in (0..14).rev() {
            read_cmd.push(((address >> shift) & 1) as u8);
        }
        read_cmd.push(0);
        eeprom_write_bits_with_dma_hint(&mut bus, &read_cmd, 17);

        let bits = eeprom_read_bits(&bus, 68);
        assert_eq!(&bits[0..4], &[0, 0, 0, 0]);
        assert_eq!(&bits[4..], bits_from_bytes(&payload).as_slice());

        let dump = bus.backup_data().expect("eeprom backup should exist");
        assert_eq!(dump.len(), EEPROM_8K_SIZE);
    }

    #[test]
    fn eeprom_dma_hints_can_promote_stale_6bit_mode_loaded_from_small_save() {
        let mut bus = GbaBus::default();
        let rom = rom_with_tag(EEPROM_TAG);
        bus.load_rom(&rom);
        bus.reset();

        // Simulate a stale old save file that forced 512B/6-bit mode.
        bus.load_backup_data(&[0xFF; EEPROM_512_SIZE]);

        let address = 0x0123usize;
        let payload = [0xFE, 0xDC, 0xBA, 0x98, 0x10, 0x32, 0x54, 0x76];

        let mut write_bits = vec![1, 0];
        for shift in (0..14).rev() {
            write_bits.push(((address >> shift) & 1) as u8);
        }
        write_bits.extend(bits_from_bytes(&payload));
        write_bits.push(0);
        eeprom_write_bits_with_dma_hint(&mut bus, &write_bits, 81);

        let mut read_cmd = vec![1, 1];
        for shift in (0..14).rev() {
            read_cmd.push(((address >> shift) & 1) as u8);
        }
        read_cmd.push(0);
        eeprom_write_bits_with_dma_hint(&mut bus, &read_cmd, 17);

        let bits = eeprom_read_bits(&bus, 68);
        assert_eq!(&bits[0..4], &[0, 0, 0, 0]);
        assert_eq!(&bits[4..], bits_from_bytes(&payload).as_slice());

        let dump = bus.backup_data().expect("eeprom backup should exist");
        assert_eq!(dump.len(), EEPROM_8K_SIZE);
    }

    #[test]
    fn flash_id_mode_reports_expected_codes() {
        let mut bus = GbaBus::default();
        bus.load_rom(&rom_with_tag(FLASH1M_TAG));
        bus.reset();

        flash_command(&mut bus, 0x90);
        assert_eq!(bus.read8(SRAM_BASE), 0x62);
        assert_eq!(bus.read8(SRAM_BASE + 1), 0x13);

        bus.write8(SRAM_BASE, 0xF0);
        assert_eq!(bus.read8(SRAM_BASE), 0xFF);
    }

    #[test]
    fn flash_program_and_bank_switch_work_for_flash1m() {
        let mut bus = GbaBus::default();
        bus.load_rom(&rom_with_tag(FLASH1M_TAG));
        bus.reset();

        flash_command(&mut bus, 0xA0);
        bus.write8(SRAM_BASE + 0x0123, 0x12);
        assert_eq!(bus.read8(SRAM_BASE + 0x0123), 0x12);

        flash_command(&mut bus, 0xB0);
        bus.write8(SRAM_BASE, 1);
        flash_command(&mut bus, 0xA0);
        bus.write8(SRAM_BASE + 0x0123, 0x34);
        assert_eq!(bus.read8(SRAM_BASE + 0x0123), 0x34);

        flash_command(&mut bus, 0xB0);
        bus.write8(SRAM_BASE, 0);
        assert_eq!(bus.read8(SRAM_BASE + 0x0123), 0x12);
    }

    #[test]
    fn sram_backup_survives_soft_reset() {
        let mut bus = GbaBus::default();
        bus.load_rom(&rom_with_tag(SRAM_TAG));
        bus.reset();

        bus.write8(SRAM_BASE + 0x0020, 0x5A);
        bus.reset();

        assert_eq!(bus.read8(SRAM_BASE + 0x0020), 0x5A);
    }

    #[test]
    fn load_backup_data_populates_sram_save() {
        let mut bus = GbaBus::default();
        bus.load_rom(&rom_with_tag(SRAM_TAG));
        bus.reset();

        bus.load_backup_data(&[0x11, 0x22, 0x33]);
        let dump = bus.backup_data().expect("sram backup should exist");

        assert_eq!(&dump[0..4], &[0x11, 0x22, 0x33, 0xFF]);
    }

    #[test]
    fn load_backup_data_populates_eeprom_save() {
        let mut bus = GbaBus::default();
        bus.load_rom(&rom_with_tag(EEPROM_TAG));
        bus.reset();

        bus.load_backup_data(&[0xAA; EEPROM_8K_SIZE]);
        let dump = bus.backup_data().expect("eeprom backup should exist");

        assert_eq!(dump.len(), EEPROM_8K_SIZE);
        assert_eq!(dump[0], 0xAA);
        assert_eq!(dump[EEPROM_8K_SIZE - 1], 0xAA);
    }

    #[test]
    fn dma3_immediate_word_copy_runs_on_enable() {
        let mut bus = GbaBus::default();
        bus.reset();
        bus.write32(0x0300_0000, 0x1122_3344);
        bus.write32(0x0300_0004, 0x5566_7788);

        bus.write32(0x0400_00D4, 0x0300_0000); // DMA3SAD
        bus.write32(0x0400_00D8, 0x0200_0000); // DMA3DAD
        bus.write16(0x0400_00DC, 2); // DMA3CNT_L
        bus.write16(0x0400_00DE, 0x8400); // enable + 32-bit + immediate

        assert_eq!(bus.read32(0x0200_0000), 0x1122_3344);
        assert_eq!(bus.read32(0x0200_0004), 0x5566_7788);
        assert_eq!(bus.read16(0x0400_00DE) & 0x8000, 0);
    }

    #[test]
    fn dma3_vblank_starts_when_triggered() {
        let mut bus = GbaBus::default();
        bus.reset();
        bus.write32(0x0300_0000, 0xAABB_CCDD);

        bus.write32(0x0400_00D4, 0x0300_0000); // DMA3SAD
        bus.write32(0x0400_00D8, 0x0200_0000); // DMA3DAD
        bus.write16(0x0400_00DC, 1); // DMA3CNT_L
        bus.write16(0x0400_00DE, 0x9400); // enable + 32-bit + VBlank timing

        assert_eq!(bus.read32(0x0200_0000), 0);
        bus.trigger_vblank_dma();
        assert_eq!(bus.read32(0x0200_0000), 0xAABB_CCDD);
        assert_eq!(bus.read16(0x0400_00DE) & 0x8000, 0);
    }

    #[test]
    fn dma3_hblank_repeat_keeps_enabled_and_advances_source() {
        let mut bus = GbaBus::default();
        bus.reset();
        bus.write32(0x0300_0000, 0x1111_2222);
        bus.write32(0x0300_0004, 0x3333_4444);

        bus.write32(0x0400_00D4, 0x0300_0000); // DMA3SAD
        bus.write32(0x0400_00D8, 0x0200_0000); // DMA3DAD
        bus.write16(0x0400_00DC, 1); // DMA3CNT_L
        // enable + HBlank + repeat + 32-bit + dest fixed
        bus.write16(0x0400_00DE, 0xA640);

        bus.trigger_hblank_dma();
        assert_eq!(bus.read32(0x0200_0000), 0x1111_2222);
        assert_ne!(bus.read16(0x0400_00DE) & 0x8000, 0);

        bus.trigger_hblank_dma();
        assert_eq!(bus.read32(0x0200_0000), 0x3333_4444);
        assert_ne!(bus.read16(0x0400_00DE) & 0x8000, 0);
        // DMA source register is write-only on real GBA hardware; the
        // advanced address lives only in the internal working register,
        // verified indirectly by the second transfer reading 0x0300_0004.
    }

    #[test]
    fn dma1_special_fifo_triggers_on_timer_overflow() {
        let mut bus = GbaBus::default();
        bus.reset();
        bus.write32(0x0300_0000, 0xAAA0_0001);
        bus.write32(0x0300_0004, 0xAAA0_0002);
        bus.write32(0x0300_0008, 0xAAA0_0003);
        bus.write32(0x0300_000C, 0xAAA0_0004);

        bus.write32(0x0400_00BC, 0x0300_0000); // DMA1SAD
        bus.write32(0x0400_00C0, FIFO_A_ADDR); // DMA1DAD
        bus.write16(0x0400_00C4, 1); // ignored for FIFO
        bus.write16(0x0400_00C6, 0xB640); // enable + special + repeat + 32-bit + dest fixed

        assert_eq!(bus.read32(FIFO_A_ADDR), 0);
        bus.on_timer_overflow(0, 1);

        assert_eq!(bus.read32(FIFO_A_ADDR), 0xAAA0_0004);
        assert_ne!(bus.read16(0x0400_00C6) & 0x8000, 0);
        // DMA source is write-only; internal pointer advances but the
        // IO latch retains the CPU-written value.
        assert_eq!(bus.read32(0x0400_00BC), 0x0300_0000);
    }

    #[test]
    fn dma1_special_fifo_forces_fixed_destination_even_if_control_uses_increment() {
        let mut bus = GbaBus::default();
        bus.reset();
        bus.write32(0x0300_2000, 0xCCCC_0001);
        bus.write32(0x0300_2004, 0xCCCC_0002);
        bus.write32(0x0300_2008, 0xCCCC_0003);
        bus.write32(0x0300_200C, 0xCCCC_0004);

        bus.write32(0x0400_00BC, 0x0300_2000); // DMA1SAD
        bus.write32(0x0400_00C0, FIFO_A_ADDR); // DMA1DAD
        // enable + special + repeat + 32-bit, but dest mode=increment (0)
        bus.write16(0x0400_00C6, 0xB600);

        bus.on_timer_overflow(0, 1);

        assert_eq!(bus.read32(FIFO_A_ADDR), 0xCCCC_0004);
        // Dest and source registers are write-only; IO latch retains
        // the CPU-written values, not the DMA-advanced addresses.
        assert_eq!(bus.read32(0x0400_00C0), FIFO_A_ADDR);
        assert_eq!(bus.read32(0x0400_00BC), 0x0300_2000);
        assert_ne!(bus.read16(0x0400_00C6) & 0x8000, 0);
    }

    #[test]
    fn dma1_special_fifo_sanitizes_rom_pointer_words_without_bios() {
        let mut bus = GbaBus::default();
        bus.reset();
        bus.write32(0x0300_2400, 0x0836_2239);
        bus.write32(0x0300_2404, 0x0836_22A9);
        bus.write32(0x0300_2408, 0x0836_22C9);
        bus.write32(0x0300_240C, 0x0836_22E5);

        bus.write32(0x0400_00BC, 0x0300_2400); // DMA1SAD
        bus.write32(0x0400_00C0, FIFO_A_ADDR); // DMA1DAD
        bus.write16(0x0400_00C6, 0xB640); // enable + special + repeat + 32-bit + dest fixed

        bus.on_timer_overflow(0, 1);

        // Pointer-like words in ROM range are suppressed to avoid harsh crackle
        // in no-BIOS mode when software mixer state is not yet stable.
        assert_eq!(bus.read32(FIFO_A_ADDR), 0);
    }

    #[test]
    fn dma1_special_fifo_sanitizes_ram_pointer_words_without_bios() {
        let mut bus = GbaBus::default();
        bus.reset();
        bus.write32(0x0300_2500, 0x0300_29D0);
        bus.write32(0x0300_2504, 0x0300_1010);
        bus.write32(0x0300_2508, 0x0200_0020);
        bus.write32(0x0300_250C, 0x0300_12E0);

        bus.write32(0x0400_00BC, 0x0300_2500); // DMA1SAD
        bus.write32(0x0400_00C0, FIFO_A_ADDR); // DMA1DAD
        bus.write16(0x0400_00C6, 0xB640); // enable + special + repeat + 32-bit + dest fixed

        bus.on_timer_overflow(0, 1);
        assert_eq!(bus.read32(FIFO_A_ADDR), 0);
    }

    #[test]
    fn dma1_special_fifo_sanitizes_ascii_tag_words_without_bios() {
        let mut bus = GbaBus::default();
        bus.reset();
        bus.write32(0x0300_2600, 0x6873_6D53); // "Smsh"
        bus.write32(0x0300_2604, 0x6D75_7369); // "isum"
        bus.write32(0x0300_2608, 0x4D75_7369); // "isuM"
        bus.write32(0x0300_260C, 0x4461_7461); // "ataD"

        bus.write32(0x0400_00BC, 0x0300_2600); // DMA1SAD
        bus.write32(0x0400_00C0, FIFO_A_ADDR); // DMA1DAD
        bus.write16(0x0400_00C6, 0xB640); // enable + special + repeat + 32-bit + dest fixed

        bus.on_timer_overflow(0, 1);
        assert_eq!(bus.read32(FIFO_A_ADDR), 0);
    }

    #[test]
    fn dma1_special_fifo_respects_timer_select() {
        let mut bus = GbaBus::default();
        bus.reset();
        bus.write32(0x0300_1000, 0xBBBB_0001);
        bus.write32(0x0300_1004, 0xBBBB_0002);
        bus.write32(0x0300_1008, 0xBBBB_0003);
        bus.write32(0x0300_100C, 0xBBBB_0004);

        bus.write32(0x0400_00BC, 0x0300_1000); // DMA1SAD
        bus.write32(0x0400_00C0, FIFO_A_ADDR); // DMA1DAD
        bus.write16(0x0400_00C6, 0xB640); // enable + special + repeat + 32-bit + dest fixed
        // FIFO A uses timer1.
        bus.write16(0x0400_0082, 1 << 10);

        bus.on_timer_overflow(0, 1);
        assert_eq!(bus.read32(FIFO_A_ADDR), 0);
        // Source latch unchanged (write-only register).
        assert_eq!(bus.read32(0x0400_00BC), 0x0300_1000);

        bus.on_timer_overflow(1, 1);
        assert_eq!(bus.read32(FIFO_A_ADDR), 0xBBBB_0004);
        assert_eq!(bus.read32(0x0400_00BC), 0x0300_1000);
    }

    #[test]
    fn direct_sound_fifo_outputs_stereo_samples() {
        let mut bus = GbaBus::default();
        bus.reset();
        bus.audio_post_filter_enabled = false;

        // Master enable + Direct Sound A full volume to both speakers.
        bus.write16(0x0400_0084, 0x0080);
        bus.write16(0x0400_0082, (1 << 2) | (1 << 8) | (1 << 9));
        // FIFO A bytes: 0x80 (-128), 0x7F (+127), then zeros.
        bus.write32(FIFO_A_ADDR, 0x0000_7F80);

        bus.on_timer_overflow(0, 1);
        mix_audio_samples_for_test(&mut bus, 1);
        bus.on_timer_overflow(0, 1);
        mix_audio_samples_for_test(&mut bus, 1);
        let samples = bus.take_audio_samples();

        assert_eq!(samples.len(), 4);
        assert_eq!(samples[0], expected_pcm_from_mixed_units(-256));
        assert_eq!(samples[1], expected_pcm_from_mixed_units(-256));
        assert_eq!(samples[2], expected_pcm_from_mixed_units(254));
        assert_eq!(samples[3], expected_pcm_from_mixed_units(254));
    }

    #[test]
    fn soundbias_rate_bits_change_pcm_mix_cadence() {
        let mut bus = GbaBus::default();
        bus.reset();
        bus.audio_post_filter_enabled = false;

        // Master enable + Direct Sound A full volume to both speakers.
        bus.write16(0x0400_0084, 0x0080);
        bus.write16(0x0400_0082, (1 << 2) | (1 << 8) | (1 << 9));
        bus.write32(FIFO_A_ADDR, 0x7F7F_7F7F);
        bus.on_timer_overflow(0, 1);

        // 512 cycles @32kHz base => 1 stereo frame.
        mix_audio_samples_for_test(&mut bus, 1);
        let base_rate = bus.take_audio_samples();
        assert_eq!(base_rate.len(), 2);
        assert_eq!(base_rate[0], expected_pcm_from_mixed_units(254));
        assert_eq!(base_rate[1], expected_pcm_from_mixed_units(254));

        // SOUND_BIAS bits14-15 select PWM resolution/sample cadence.
        // bit14=1 => 65536Hz (256 cycles/sample), so 512 cycles output 2 frames.
        bus.write16(0x0400_0088, 0x4200);
        bus.write32(FIFO_A_ADDR, 0x7F7F_7F7F);
        bus.on_timer_overflow(0, 1);
        mix_audio_samples_for_test(&mut bus, 1);
        let double_rate = bus.take_audio_samples();
        assert_eq!(double_rate.len(), 4);
        assert_eq!(double_rate[0], expected_pcm_from_mixed_units(254));
        assert_eq!(double_rate[1], expected_pcm_from_mixed_units(254));
        assert_eq!(double_rate[2], expected_pcm_from_mixed_units(254));
        assert_eq!(double_rate[3], expected_pcm_from_mixed_units(254));
    }

    #[test]
    fn direct_sound_a_and_b_mix_and_clip_when_both_routed() {
        let mut bus = GbaBus::default();
        bus.reset();
        bus.audio_post_filter_enabled = false;

        // Master enable.
        bus.write16(0x0400_0084, 0x0080);
        // Direct Sound A/B full volume to both speakers, timer0.
        bus.write16(
            0x0400_0082,
            (1 << 2) | (1 << 3) | (1 << 8) | (1 << 9) | (1 << 12) | (1 << 13),
        );
        bus.write32(FIFO_A_ADDR, 0x0000_007F);
        bus.write32(FIFO_B_ADDR, 0x0000_007F);

        bus.on_timer_overflow(0, 1);
        mix_audio_samples_for_test(&mut bus, 1);
        let samples = bus.take_audio_samples();

        assert_eq!(samples.len(), 2);
        assert_eq!(samples[0], expected_pcm_from_mixed_units(508));
        assert_eq!(samples[1], expected_pcm_from_mixed_units(508));
    }

    #[test]
    fn unrelated_timer_does_not_inject_audio_when_direct_sound_is_routed() {
        let mut bus = GbaBus::default();
        bus.reset();
        bus.audio_post_filter_enabled = false;

        // Master enable + Direct Sound A full volume to both speakers (timer0).
        bus.write16(0x0400_0084, 0x0080);
        bus.write16(0x0400_0082, (1 << 2) | (1 << 8) | (1 << 9));
        bus.write32(FIFO_A_ADDR, 0x0000_007F);
        bus.on_timer_overflow(1, 1);
        mix_audio_samples_for_test(&mut bus, 1);
        let _ = bus.take_audio_samples();

        bus.on_timer_overflow(0, 1);
        mix_audio_samples_for_test(&mut bus, 1);
        let samples = bus.take_audio_samples();
        assert_eq!(samples.len(), 2);
        assert_eq!(samples[0], expected_pcm_from_mixed_units(254));
        assert_eq!(samples[1], expected_pcm_from_mixed_units(254));
    }

    #[test]
    fn soundcnt_h_fifo_reset_bit_clears_fifo_queue() {
        let mut bus = GbaBus::default();
        bus.reset();

        bus.write16(0x0400_0084, 0x0080);
        bus.write32(FIFO_A_ADDR, 0x0000_7F80);
        // Enable A to both speakers and pulse FIFO A reset (bit11).
        bus.write16(0x0400_0082, (1 << 2) | (1 << 8) | (1 << 9) | (1 << 11));

        bus.on_timer_overflow(0, 1);
        mix_audio_samples_for_test(&mut bus, 1);
        let samples = bus.take_audio_samples();

        assert_eq!(samples.len(), 2);
        assert_eq!(samples[0], 0);
        assert_eq!(samples[1], 0);
    }

    #[test]
    fn psg_square1_mix_produces_nonzero_when_routed() {
        let mut bus = GbaBus::default();
        bus.reset();

        bus.write16(0x0400_0084, 0x0080); // master enable
        // Left/Right master volume = 7, route square1 to both L/R.
        bus.write16(0x0400_0080, 0x1177);
        // duty=50%, initial envelope volume=15
        bus.write16(0x0400_0062, (2 << 6) | (15 << 12));
        // freq=1024, trigger on
        bus.write16(0x0400_0064, 0x8000 | 1024);

        mix_audio_samples_for_test(&mut bus, 1);
        let samples = bus.take_audio_samples();

        assert_eq!(samples.len(), 2);
        assert_ne!(samples[0], 0);
        assert_ne!(samples[1], 0);
    }

    #[test]
    fn psg_output_is_silenced_when_master_sound_is_off() {
        let mut bus = GbaBus::default();
        bus.reset();

        bus.write16(0x0400_0084, 0x0080); // master enable
        bus.write16(0x0400_0080, 0x1177);
        bus.write16(0x0400_0062, (2 << 6) | (15 << 12));
        bus.write16(0x0400_0064, 0x8000 | 1024);
        mix_audio_samples_for_test(&mut bus, 1);
        let _ = bus.take_audio_samples();

        bus.write16(0x0400_0084, 0x0000); // master off
        mix_audio_samples_for_test(&mut bus, 1);
        let samples = bus.take_audio_samples();

        assert!(samples.is_empty());
    }

    #[test]
    fn psg_volume_ratio_in_soundcnt_h_scales_output() {
        let mut bus = GbaBus::default();
        bus.reset();

        bus.write16(0x0400_0084, 0x0080); // master enable
        // route square1 to both L/R, master volume max
        bus.write16(0x0400_0080, 0x1177);
        bus.write16(0x0400_0062, (2 << 6) | (15 << 12));
        bus.write16(0x0400_0064, (1 << 15) | 1024);

        // PSG ratio=25%
        bus.write16(0x0400_0082, 0x0000);
        mix_audio_samples_for_test(&mut bus, 1);
        let low = bus.take_audio_samples();
        assert_eq!(low.len(), 2);

        // Retrigger and set PSG ratio=100%
        bus.write16(0x0400_0064, (1 << 15) | 1024);
        bus.write16(0x0400_0082, 0x0002);
        mix_audio_samples_for_test(&mut bus, 1);
        let high = bus.take_audio_samples();
        assert_eq!(high.len(), 2);

        let low_amp = i32::from(low[0].abs());
        let high_amp = i32::from(high[0].abs());
        assert!(high_amp >= low_amp * 3);
    }

    #[test]
    fn psg_square1_length_counter_turns_channel_off() {
        let mut bus = GbaBus::default();
        bus.reset();

        bus.write16(0x0400_0084, 0x0080); // master enable
        bus.write16(0x0400_0080, 0x1177); // route square1 to both L/R
        // length=1 tick (64-63), duty=50%, volume=15
        bus.write16(0x0400_0062, 63 | (2 << 6) | (15 << 12));
        // length enable + trigger
        bus.write16(0x0400_0064, (1 << 14) | (1 << 15) | 1024);
        assert!(bus.psg_square1_on);

        for _ in 0..PSG_FRAME_SEQ_TICK_SAMPLES {
            mix_audio_samples_for_test(&mut bus, 1);
        }

        assert!(!bus.psg_square1_on);
        assert_eq!(bus.read16(0x0400_0084) & 0x0001, 0);
    }

    #[test]
    fn psg_square1_envelope_steps_at_64hz() {
        let mut bus = GbaBus::default();
        bus.reset();

        bus.write16(0x0400_0084, 0x0080); // master enable
        bus.write16(0x0400_0080, 0x1177); // route square1 to both L/R
        // duty=50%, envelope period=1, decrease, initial volume=15
        bus.write16(0x0400_0062, (2 << 6) | (1 << 8) | (15 << 12));
        bus.write16(0x0400_0064, (1 << 15) | 1024);
        assert_eq!(bus.psg_square1_volume, 15);

        let first_envelope_tick_samples = usize::from(PSG_FRAME_SEQ_TICK_SAMPLES) * 8;
        for _ in 0..(first_envelope_tick_samples - 1) {
            mix_audio_samples_for_test(&mut bus, 1);
        }
        assert_eq!(bus.psg_square1_volume, 15);

        mix_audio_samples_for_test(&mut bus, 1);
        assert_eq!(bus.psg_square1_volume, 14);
    }

    #[test]
    fn psg_square1_sweep_updates_frequency_register() {
        let mut bus = GbaBus::default();
        bus.reset();

        bus.write16(0x0400_0084, 0x0080); // master enable
        // sweep: period=1, increase, shift=1
        bus.write16(0x0400_0060, (1 << 4) | 1);
        bus.write16(0x0400_0062, (2 << 6) | (15 << 12));
        bus.write16(0x0400_0064, (1 << 15) | 700);

        let first_sweep_tick_samples = usize::from(PSG_FRAME_SEQ_TICK_SAMPLES) * 3;
        for _ in 0..first_sweep_tick_samples {
            mix_audio_samples_for_test(&mut bus, 1);
        }

        assert_eq!(bus.read16(0x0400_0064) & 0x07FF, 1050);
        assert!(bus.psg_square1_on);
    }

    #[test]
    fn psg_square1_sweep_overflow_disables_channel() {
        let mut bus = GbaBus::default();
        bus.reset();

        bus.write16(0x0400_0084, 0x0080); // master enable
        // sweep: period=1, increase, shift=1 (overflow from high base frequency)
        bus.write16(0x0400_0060, (1 << 4) | 1);
        bus.write16(0x0400_0062, (2 << 6) | (15 << 12));
        bus.write16(0x0400_0064, (1 << 15) | 2040);

        let first_sweep_tick_samples = usize::from(PSG_FRAME_SEQ_TICK_SAMPLES) * 3;
        for _ in 0..first_sweep_tick_samples {
            mix_audio_samples_for_test(&mut bus, 1);
        }

        assert!(!bus.psg_square1_on);
        assert_eq!(bus.read16(0x0400_0084) & 0x0001, 0);
    }

    #[test]
    fn psg_noise_width7_mode_updates_bit6_from_feedback_bit() {
        let mut bus_15bit = GbaBus::default();
        bus_15bit.reset();
        bus_15bit.write16(0x0400_0084, 0x0080); // master enable
        bus_15bit.write16(0x0400_0078, 15 << 12); // envelope volume=15
        bus_15bit.write16(0x0400_007C, 1 << 15); // trigger, r=0, s=0, width15
        let _ = bus_15bit.next_psg_noise_sample();
        assert_eq!(bus_15bit.psg_noise_lfsr, 0x3FFF);

        let mut bus_7bit = GbaBus::default();
        bus_7bit.reset();
        bus_7bit.write16(0x0400_0084, 0x0080); // master enable
        bus_7bit.write16(0x0400_0078, 15 << 12); // envelope volume=15
        bus_7bit.write16(0x0400_007C, (1 << 15) | (1 << 3)); // trigger + width7
        let _ = bus_7bit.next_psg_noise_sample();
        assert_eq!(bus_7bit.psg_noise_lfsr, 0x3FBF);
    }

    #[test]
    fn psg_noise_low_frequency_waits_expected_samples_before_lfsr_step() {
        let mut bus = GbaBus::default();
        bus.reset();

        bus.write16(0x0400_0084, 0x0080); // master enable
        bus.write16(0x0400_0078, 15 << 12); // envelope volume=15
        // trigger, r=7, s=5 -> one LFSR clock every ~448 output samples
        bus.write16(0x0400_007C, (1 << 15) | (5 << 4) | 7);

        let divisor = 7u32 * 16;
        let denominator = divisor << (5 + 1);
        let step = (((16u64) << PSG_NOISE_PHASE_BITS) / u64::from(denominator)).max(1) as u32;
        let samples_per_step =
            (u64::from(PSG_NOISE_PHASE_ONE) + u64::from(step) - 1) / u64::from(step);

        let initial = bus.psg_noise_lfsr;
        for _ in 0..(samples_per_step - 1) {
            let _ = bus.next_psg_noise_sample();
        }
        assert_eq!(bus.psg_noise_lfsr, initial);

        let _ = bus.next_psg_noise_sample();
        assert_ne!(bus.psg_noise_lfsr, initial);
    }

    #[test]
    fn wave_ram_io_access_uses_opposite_of_selected_bank() {
        let mut bus = GbaBus::default();
        bus.reset();

        bus.write16(0x0400_0070, 0x0000); // selected bank 0 -> CPU accesses bank 1
        bus.write8(0x0400_0090, 0x12);
        bus.write16(0x0400_0070, 0x0040); // selected bank 1 -> CPU accesses bank 0
        bus.write8(0x0400_0090, 0x34);

        bus.write16(0x0400_0070, 0x0000); // CPU reads bank 1
        assert_eq!(bus.read8(0x0400_0090), 0x12);
        bus.write16(0x0400_0070, 0x0040); // CPU reads bank 0
        assert_eq!(bus.read8(0x0400_0090), 0x34);
    }

    #[test]
    fn psg_wave_32_sample_mode_respects_bank_select() {
        let mut bus = GbaBus::default();
        bus.reset();

        bus.write16(0x0400_0084, 0x0080); // master enable
        bus.write16(0x0400_0072, 1 << 13); // volume code 1 (100%)
        bus.write16(0x0400_0070, 0x0040); // selected bank 1 -> CPU writes bank 0
        bus.write8(0x0400_0090, 0xF0); // first sample high nibble = 15 (positive)
        bus.write16(0x0400_0070, 0x0000); // selected bank 0 -> CPU writes bank 1
        bus.write8(0x0400_0090, 0x00); // first sample high nibble = 0 (negative)

        bus.write16(0x0400_0070, 0x0080); // enable, 32-sample mode, bank 0
        bus.write16(0x0400_0074, 1 << 15); // trigger, freq=0
        let bank0_sample = bus.next_psg_wave_sample();

        bus.write16(0x0400_0070, 0x00C0); // enable, 32-sample mode, bank 1
        bus.write16(0x0400_0074, 1 << 15); // retrigger
        let bank1_sample = bus.next_psg_wave_sample();

        assert!(bank0_sample > 0);
        assert!(bank1_sample < 0);
    }

    #[test]
    fn psg_wave_64_sample_mode_reads_both_banks() {
        let mut bus = GbaBus::default();
        bus.reset();

        bus.write16(0x0400_0084, 0x0080); // master enable
        bus.write16(0x0400_0072, 1 << 13); // volume code 1 (100%)
        bus.write16(0x0400_0070, 0x0040); // selected bank 1 -> CPU writes bank 0
        bus.write8(0x0400_0090, 0xF0); // bank0 sample0 positive
        bus.write16(0x0400_0070, 0x0000); // selected bank 0 -> CPU writes bank 1
        bus.write8(0x0400_0090, 0x00); // bank1 sample0 negative

        // enable + two-bank(64 sample) mode + start from bank0
        bus.write16(0x0400_0070, 0x00A0);
        bus.write16(0x0400_0074, 1 << 15); // trigger, freq=0

        let first = bus.next_psg_wave_sample();
        let mut after_boundary = first;
        for _ in 0..520 {
            after_boundary = bus.next_psg_wave_sample();
        }

        assert!(first > 0);
        assert!(after_boundary < 0);
    }

    #[test]
    fn wave_ram_access_bank_flips_during_two_bank_playback() {
        let mut bus = GbaBus::default();
        bus.reset();

        bus.write16(0x0400_0084, 0x0080); // master enable
        bus.write16(0x0400_0072, 1 << 13); // volume code 1 (100%)

        bus.write16(0x0400_0070, 0x0040); // selected bank 1 -> CPU writes bank 0
        bus.write8(0x0400_0090, 0x11);
        bus.write16(0x0400_0070, 0x0000); // selected bank 0 -> CPU writes bank 1
        bus.write8(0x0400_0090, 0x22);

        // enable + two-bank mode + selected bank 0 for playback start
        bus.write16(0x0400_0070, 0x00A0);
        bus.write16(0x0400_0074, 1 << 15); // trigger

        // Early playback: bank0 is playing, so CPU should see bank1.
        assert_eq!(bus.read8(0x0400_0090), 0x22);

        for _ in 0..520 {
            let _ = bus.next_psg_wave_sample();
        }

        // Later in cycle: bank1 is playing, so CPU should see bank0.
        assert_eq!(bus.read8(0x0400_0090), 0x11);
    }

    #[test]
    fn wave_playback_bank_is_latched_until_retrigger() {
        let mut bus = GbaBus::default();
        bus.reset();

        bus.write16(0x0400_0084, 0x0080); // master enable
        bus.write16(0x0400_0072, 1 << 13); // volume code 1 (100%)

        // Fill bank0 with positive start sample, bank1 with negative.
        bus.write16(0x0400_0070, 0x0040); // selected bank1 -> CPU writes bank0
        bus.write8(0x0400_0090, 0xF0);
        bus.write16(0x0400_0070, 0x0000); // selected bank0 -> CPU writes bank1
        bus.write8(0x0400_0090, 0x00);

        bus.write16(0x0400_0070, 0x00A0); // enable + 64-sample + selected bank0
        bus.write16(0x0400_0074, 1 << 15); // trigger

        // Change selected bank while playing; current cycle should keep latched start bank.
        bus.write16(0x0400_0070, 0x00E0);
        let first = bus.next_psg_wave_sample();
        assert!(first > 0);

        let mut after_boundary = first;
        for _ in 0..520 {
            after_boundary = bus.next_psg_wave_sample();
        }
        assert!(after_boundary < 0);
    }

    #[test]
    fn wave_retrigger_resets_phase_and_uses_new_selected_bank() {
        let mut bus = GbaBus::default();
        bus.reset();

        bus.write16(0x0400_0084, 0x0080); // master enable
        bus.write16(0x0400_0072, 1 << 13); // volume code 1 (100%)

        // Fill bank0 positive, bank1 negative at sample0.
        bus.write16(0x0400_0070, 0x0040); // selected bank1 -> CPU writes bank0
        bus.write8(0x0400_0090, 0xF0);
        bus.write16(0x0400_0070, 0x0000); // selected bank0 -> CPU writes bank1
        bus.write8(0x0400_0090, 0x00);

        bus.write16(0x0400_0070, 0x00A0); // selected bank0
        bus.write16(0x0400_0074, 1 << 15); // trigger
        for _ in 0..520 {
            let _ = bus.next_psg_wave_sample();
        }

        // Retrigger from bank1 start: first output sample should now be negative.
        bus.write16(0x0400_0070, 0x00E0); // selected bank1
        bus.write16(0x0400_0074, 1 << 15); // retrigger
        let first_after_retrigger = bus.next_psg_wave_sample();
        assert!(first_after_retrigger < 0);
    }

    #[test]
    fn wave_ram_write_during_playback_commits_on_sample_boundary() {
        let mut bus = GbaBus::default();
        bus.reset();

        bus.write16(0x0400_0084, 0x0080); // master enable
        bus.write16(0x0400_0072, 1 << 13); // volume code 1 (100%)
        bus.write16(0x0400_0070, 0x0000); // selected bank0 -> CPU accesses bank1
        bus.write8(0x0400_0090, 0x00); // bank1[0] initial

        bus.write16(0x0400_0070, 0x00A0); // enable + 64-sample + selected bank0
        bus.write16(0x0400_0074, (1 << 15) | 2045); // trigger with fast phase advance

        bus.write8(0x0400_0090, 0xF0); // pending write to bank1[0]
        assert_eq!(bus.wave_ram[16], 0x00);
        assert_eq!(bus.read8(0x0400_0090), 0xF0);

        let _ = bus.next_psg_wave_sample();
        assert_eq!(bus.wave_ram[16], 0xF0);
    }

    #[test]
    fn wave_ram_pending_writes_flush_when_wave_is_disabled() {
        let mut bus = GbaBus::default();
        bus.reset();

        bus.write16(0x0400_0084, 0x0080); // master enable
        bus.write16(0x0400_0072, 1 << 13); // volume code 1 (100%)
        bus.write16(0x0400_0070, 0x0000); // selected bank0 -> CPU accesses bank1
        bus.write8(0x0400_0090, 0x00); // bank1[0] initial

        bus.write16(0x0400_0070, 0x00A0); // enable + 64-sample + selected bank0
        bus.write16(0x0400_0074, 1 << 15); // trigger

        bus.write8(0x0400_0090, 0xF0); // pending while playing
        assert_eq!(bus.wave_ram[16], 0x00);

        bus.write16(0x0400_0070, 0x0000); // disable wave DAC
        assert_eq!(bus.wave_ram[16], 0xF0);
    }
}
