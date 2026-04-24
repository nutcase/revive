mod dma;
mod mode4;
mod plane;
mod ports;
mod sprites;

use crate::debug_flags;

pub const FRAME_WIDTH: usize = 320;
pub const FRAME_HEIGHT: usize = 240;
const FRAME_WIDTH_32_CELL: usize = 256;
const FRAME_HEIGHT_28_CELL: usize = 224;

const VRAM_SIZE: usize = 0x10000;
const CRAM_COLORS: usize = 64;
const VSRAM_WORDS: usize = 40;
const TILE_SIZE_BYTES: usize = 32;
const REG_COUNT: usize = 0x20;
const REG_MODE_SET_1: usize = 0;
const REG_MODE_SET_2: usize = 1;
const REG_PLANE_A_NAMETABLE: usize = 2;
const REG_WINDOW_NAMETABLE: usize = 3;
const REG_PLANE_B_NAMETABLE: usize = 4;
const REG_SPRITE_TABLE: usize = 5;
const REG_BACKGROUND_COLOR: usize = 7;
const REG_H_INTERRUPT_COUNTER: usize = 10;
const REG_HSCROLL_TABLE: usize = 13;
const REG_WINDOW_HPOS: usize = 17;
const REG_WINDOW_VPOS: usize = 18;
const REG_PLANE_SIZE: usize = 16;
const REG_AUTO_INCREMENT: usize = 15;
const REG_DMA_LENGTH_LOW: usize = 19;
const REG_DMA_LENGTH_HIGH: usize = 20;
const REG_DMA_SOURCE_LOW: usize = 21;
const REG_DMA_SOURCE_MID: usize = 22;
const REG_DMA_SOURCE_HIGH: usize = 23;
const STATUS_BASE: u16 = 0x3400;
const STATUS_FIFO_EMPTY: u16 = 0x0200;
const STATUS_FIFO_FULL: u16 = 0x0100;
const STATUS_HBLANK: u16 = 0x0004;
const STATUS_VBLANK: u16 = 0x0008;
const STATUS_ODD_FRAME: u16 = 0x0010;
const STATUS_DMA_BUSY: u16 = 0x0002;
const STATUS_SPRITE_COLLISION: u16 = 0x0020;
const STATUS_SPRITE_OVERFLOW: u16 = 0x0040;

/// Master clock cycles per byte for DMA Fill during active display.
const DMA_FILL_CYCLES_PER_BYTE_ACTIVE: u64 = 4;
/// Master clock cycles per byte for DMA Fill during blanking.
const DMA_FILL_CYCLES_PER_BYTE_BLANK: u64 = 2;
/// Master clock cycles per byte for DMA Copy during active display.
const DMA_COPY_CYCLES_PER_BYTE_ACTIVE: u64 = 8;
/// Master clock cycles per byte for DMA Copy during blanking.
const DMA_COPY_CYCLES_PER_BYTE_BLANK: u64 = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq, bincode::Encode, bincode::Decode)]
struct DmaFillState {
    remaining_words: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, bincode::Encode, bincode::Decode)]
struct DmaFillActive {
    fill_byte: u8,
    fill_word: bool,
    lane_no_xor: bool,
    increment: u16,
    remaining: usize,
    cycle_carry: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, bincode::Encode, bincode::Decode)]
struct DmaCopyActive {
    source_addr: u16,
    increment: u16,
    remaining: usize,
    cycle_carry: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, bincode::Encode, bincode::Decode)]
pub(crate) enum DmaTarget {
    Vram,
    Cram,
    Vsram,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, bincode::Encode, bincode::Decode)]
pub(crate) struct BusDmaRequest {
    pub source_addr: u32,
    pub dest_addr: u16,
    pub auto_increment: u16,
    pub words: usize,
    pub target: DmaTarget,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, bincode::Encode, bincode::Decode)]
enum AccessMode {
    VramRead,
    #[default]
    VramWrite,
    CramRead,
    CramWrite,
    VsramRead,
    VsramWrite,
    Unsupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, bincode::Encode, bincode::Decode)]
struct PlaneSample {
    color_index: usize,
    opaque: bool,
    priority_high: bool,
}

#[derive(Debug, Clone, Copy, Default)]
struct PlaneTileCache {
    valid: bool,
    tile_x: usize,
    sample_y: usize,
    color_base: usize,
    priority_high: bool,
    pixels: [u8; 8],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, bincode::Encode, bincode::Decode)]
pub enum VideoStandard {
    Ntsc,
    Pal,
}

impl VideoStandard {
    fn total_lines(self) -> u64 {
        match self {
            Self::Ntsc => Vdp::NTSC_TOTAL_LINES,
            Self::Pal => Vdp::PAL_TOTAL_LINES,
        }
    }
}

/// A Vec wrapper that serializes as empty (scratch buffers need not be persisted).
#[derive(Debug, Clone, Default)]
struct ScratchBuf<T>(Vec<T>);

impl<T> std::ops::Deref for ScratchBuf<T> {
    type Target = Vec<T>;
    fn deref(&self) -> &Vec<T> {
        &self.0
    }
}
impl<T> std::ops::DerefMut for ScratchBuf<T> {
    fn deref_mut(&mut self) -> &mut Vec<T> {
        &mut self.0
    }
}
impl<T> bincode::Encode for ScratchBuf<T> {
    fn encode<E: bincode::enc::Encoder>(
        &self,
        _encoder: &mut E,
    ) -> Result<(), bincode::error::EncodeError> {
        Ok(())
    }
}
impl<C, T> bincode::Decode<C> for ScratchBuf<T> {
    fn decode<D: bincode::de::Decoder<Context = C>>(
        _decoder: &mut D,
    ) -> Result<Self, bincode::error::DecodeError> {
        Ok(ScratchBuf(Vec::new()))
    }
}
impl<'de, C, T> bincode::BorrowDecode<'de, C> for ScratchBuf<T> {
    fn borrow_decode<D: bincode::de::BorrowDecoder<'de, Context = C>>(
        _decoder: &mut D,
    ) -> Result<Self, bincode::error::DecodeError> {
        Ok(ScratchBuf(Vec::new()))
    }
}

#[derive(Debug, Clone, bincode::Encode, bincode::Decode)]
pub struct Vdp {
    video_standard: VideoStandard,
    frame_cycles: u64,
    frame_count: u64,
    sprite_collision: bool,
    sprite_overflow: bool,
    h_interrupt_pending: bool,
    h_interrupt_armed: bool,
    v_interrupt_pending: bool,
    h_interrupt_counter: u8,
    vram: [u8; VRAM_SIZE],
    cram: [u16; CRAM_COLORS],
    vsram: [u16; VSRAM_WORDS],
    frame_buffer: Vec<u8>,
    registers: [u8; REG_COUNT],
    line_registers: [[u8; REG_COUNT]; FRAME_HEIGHT],
    line_vsram: [[u16; VSRAM_WORDS]; FRAME_HEIGHT],
    line_hscroll: [[u16; 2]; FRAME_HEIGHT],
    line_cram: [[u16; CRAM_COLORS]; FRAME_HEIGHT],
    line_vram: Vec<[u8; VRAM_SIZE]>,
    line_vram_latch_enabled: bool,
    debug_line_latch_next: bool,
    control_latch: Option<u16>,
    access_addr: u16,
    access_mode: AccessMode,
    vram_read_buffer: u16,
    dma_fill_pending: Option<DmaFillState>,
    dma_fill_active: Option<DmaFillActive>,
    dma_copy_active: Option<DmaCopyActive>,
    dma_bus_pending: Option<BusDmaRequest>,
    dma_fill_ops: u64,
    dma_copy_ops: u64,
    /// VDP write FIFO: number of pending entries (0..=4).
    fifo_count: u8,
    /// Fractional cycle accumulator for FIFO drain timing.
    fifo_drain_carry: u64,
    /// Scratch buffer for per-pixel plane metadata (reused across frames).
    render_plane_meta: ScratchBuf<u8>,
    /// Scratch flags for sprite debug rendering options (not serialized).
    debug_sprite_flags: ScratchBuf<bool>,
    /// Scratch buffer for sprite pixel fill tracking (reused across frames).
    render_sprite_filled: ScratchBuf<bool>,
}

impl Default for Vdp {
    fn default() -> Self {
        Self::with_video_standard(VideoStandard::Ntsc)
    }
}

impl Vdp {
    // Keep legacy NTSC timing to avoid regressions in existing emulation paths.
    const NTSC_CYCLES_PER_FRAME: u64 = 127_800;
    const NTSC_TOTAL_LINES: u64 = 262;
    const PAL_TOTAL_LINES: u64 = 313;
    const DEBUG_SAT_LINE_LATCH_FLAG: usize = 0;
    const DEBUG_SAT_LIVE_FLAG: usize = 1;
    const DEBUG_SAT_PER_LINE_FLAG: usize = 2;
    const DEBUG_SPRITE_PATTERN_LINE0_FLAG: usize = 3;
    const DEBUG_SPRITE_PATTERN_PER_LINE_FLAG: usize = 4;
    #[cfg(test)]
    const CYCLES_PER_FRAME: u64 = Self::NTSC_CYCLES_PER_FRAME;
    #[cfg(test)]
    const TOTAL_LINES: u64 = Self::NTSC_TOTAL_LINES;

    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_video_standard(video_standard: VideoStandard) -> Self {
        let mut registers = [0u8; REG_COUNT];
        registers[REG_MODE_SET_2] = 0x44; // Display enabled + Mode 5
        registers[REG_PLANE_A_NAMETABLE] = 0x30; // Plane A name table base: 0xC000
        registers[REG_SPRITE_TABLE] = 0x70; // Sprite attribute table base: 0xE000
        registers[REG_HSCROLL_TABLE] = 0x3C; // Horizontal scroll table base: 0xF000
        // Window off by default.
        // Keep window disabled by default.
        registers[REG_WINDOW_HPOS] = 0x00;
        registers[REG_WINDOW_VPOS] = 0x00;
        registers[REG_AUTO_INCREMENT] = 2; // Word access by default

        let mut vdp = Self {
            video_standard,
            frame_cycles: 0,
            frame_count: 0,
            sprite_collision: false,
            sprite_overflow: false,
            h_interrupt_pending: false,
            h_interrupt_armed: false,
            v_interrupt_pending: false,
            h_interrupt_counter: registers[REG_H_INTERRUPT_COUNTER],
            vram: [0; VRAM_SIZE],
            cram: [0; CRAM_COLORS],
            vsram: [0; VSRAM_WORDS],
            frame_buffer: vec![0; FRAME_WIDTH * FRAME_HEIGHT * 3],
            registers,
            line_registers: [[0; REG_COUNT]; FRAME_HEIGHT],
            line_vsram: [[0; VSRAM_WORDS]; FRAME_HEIGHT],
            line_hscroll: [[0; 2]; FRAME_HEIGHT],
            line_cram: [[0; CRAM_COLORS]; FRAME_HEIGHT],
            line_vram: vec![[0; VRAM_SIZE]; FRAME_HEIGHT],
            line_vram_latch_enabled: debug_flags::line_vram_latch(),
            debug_line_latch_next: debug_flags::line_latch_next(),
            control_latch: None,
            access_addr: 0,
            access_mode: AccessMode::default(),
            vram_read_buffer: 0,
            dma_fill_pending: None,
            dma_fill_active: None,
            dma_copy_active: None,
            dma_bus_pending: None,
            dma_fill_ops: 0,
            dma_copy_ops: 0,
            fifo_count: 0,
            fifo_drain_carry: 0,
            render_plane_meta: ScratchBuf(vec![0u8; FRAME_WIDTH * FRAME_HEIGHT]),
            debug_sprite_flags: ScratchBuf(vec![false; 5]),
            render_sprite_filled: ScratchBuf(vec![false; FRAME_WIDTH * FRAME_HEIGHT]),
        };
        vdp.reset_line_state();
        vdp.capture_line_state(0);
        vdp.on_scanline_start(0);
        vdp.render_frame();
        vdp
    }

    pub fn video_standard(&self) -> VideoStandard {
        self.video_standard
    }

    pub fn total_lines(&self) -> u64 {
        self.video_standard.total_lines()
    }

    pub fn refresh_runtime_debug_config_from_env(&mut self) {
        self.line_vram_latch_enabled = debug_flags::line_vram_latch();
        self.debug_line_latch_next = debug_flags::line_latch_next();
    }

    fn cycles_per_frame(&self) -> u64 {
        match self.video_standard {
            VideoStandard::Ntsc => Self::NTSC_CYCLES_PER_FRAME,
            VideoStandard::Pal => {
                // Preserve per-line cadence relative to NTSC model.
                (Self::NTSC_CYCLES_PER_FRAME * Self::PAL_TOTAL_LINES + (Self::NTSC_TOTAL_LINES / 2))
                    / Self::NTSC_TOTAL_LINES
            }
        }
    }

    pub fn step(&mut self, cpu_cycles: u32) -> bool {
        let mut remaining = cpu_cycles as u64;
        let mut frame_ready = false;
        let cycles_per_frame = self.cycles_per_frame();

        while remaining > 0 {
            let until_frame_end = cycles_per_frame - self.frame_cycles;
            let advance = remaining.min(until_frame_end);
            let start = self.frame_cycles;
            let end = self.frame_cycles + advance;
            self.process_scanline_events(start, end);
            self.frame_cycles = end;
            self.step_fifo(advance);
            self.step_dma(advance);
            remaining -= advance;

            if self.frame_cycles >= cycles_per_frame {
                self.frame_cycles = 0;
                self.frame_count += 1;
                self.render_frame();
                self.h_interrupt_armed = false;
                self.reset_line_state();
                self.capture_line_state(0);
                self.on_scanline_start(0);
                frame_ready = true;
            }
        }

        frame_ready
    }

    pub fn frame_count(&self) -> u64 {
        self.frame_count
    }

    pub fn frame_buffer(&self) -> &[u8] {
        &self.frame_buffer
    }

    pub fn register(&self, index: usize) -> u8 {
        self.registers.get(index).copied().unwrap_or(0)
    }

    pub fn line_register(&self, line: usize, index: usize) -> u8 {
        if line < FRAME_HEIGHT && index < REG_COUNT {
            self.line_registers[line][index]
        } else {
            0
        }
    }

    pub fn line_vsram_u16(&self, line: usize, index: usize) -> u16 {
        if line < FRAME_HEIGHT && index < VSRAM_WORDS {
            self.line_vsram[line][index]
        } else {
            0
        }
    }

    pub fn line_hscroll_words(&self, line: usize) -> [u16; 2] {
        if line < FRAME_HEIGHT {
            self.line_hscroll[line]
        } else {
            [0; 2]
        }
    }

    pub fn line_vram_u8(&self, line: usize, addr: u16) -> u8 {
        if !self.line_vram_latch_enabled {
            self.vram[addr as usize % VRAM_SIZE]
        } else if line < FRAME_HEIGHT {
            self.line_vram[line][addr as usize % VRAM_SIZE]
        } else {
            0
        }
    }

    pub fn pending_interrupt_level(&self) -> Option<u8> {
        if self.v_interrupt_pending && self.v_interrupt_enabled() {
            Some(6)
        } else if self.h_interrupt_pending && self.h_interrupt_enabled() {
            Some(4)
        } else {
            None
        }
    }

    /// Returns true when the FIFO is full (4 entries).
    pub fn fifo_full(&self) -> bool {
        self.fifo_count >= 4
    }

    /// Whether the given absolute frame cycle falls in a blanking period
    /// (VBlank or HBlank).
    fn is_blanking_at(&self, cycle: u64) -> bool {
        let line = self
            .line_index_for_cycle(cycle)
            .min(self.total_lines().saturating_sub(1) as u64) as usize;
        if line >= self.active_display_height() {
            return true; // vblank
        }
        let (_, line_start, line_end) = self.line_cycle_bounds_for_cycle(cycle);
        let line_cycles = line_end.saturating_sub(line_start).max(1);
        let cycle_in_line = cycle.saturating_sub(line_start);
        let h = self.h_counter_value(cycle_in_line, line_cycles);
        if self.h40_mode() {
            h >= 0xB3 || h <= 0x05
        } else {
            h >= 0x93 || h <= 0x04
        }
    }

    /// Number of CPU wait cycles the 68K should stall if the FIFO was full
    /// when a data port write occurred.  Returns 0 if not full.
    pub fn fifo_wait_cycles(&self) -> u32 {
        if self.fifo_count >= 4 {
            let drain_interval: u64 = if self.is_blanking_at(self.frame_cycles) {
                8
            } else {
                18
            };
            // Account for cycles already accumulated toward the next drain
            drain_interval.saturating_sub(self.fifo_drain_carry) as u32
        } else {
            0
        }
    }

    /// Drain FIFO entries based on elapsed cycles.
    /// `advance` is how many cycles were just stepped; `frame_cycles` is
    /// already at the END of this window, so the start is `frame_cycles - advance`.
    fn step_fifo(&mut self, advance: u64) {
        if self.fifo_count == 0 {
            return;
        }
        self.fifo_drain_carry += advance;
        // Drain slots one at a time, evaluating the rate at the cycle where
        // each drain event occurs (= start of window + consumed cycles).
        let window_start = self.frame_cycles.saturating_sub(advance);
        let mut consumed = 0u64;
        while self.fifo_count > 0 {
            let drain_cycle = window_start + consumed;
            let drain_interval: u64 = if self.is_blanking_at(drain_cycle) {
                8
            } else {
                18
            };
            if self.fifo_drain_carry < drain_interval {
                break;
            }
            self.fifo_drain_carry -= drain_interval;
            consumed += drain_interval;
            self.fifo_count -= 1;
        }
        if self.fifo_count == 0 {
            self.fifo_drain_carry = 0;
        }
    }

    pub fn acknowledge_interrupt(&mut self, level: u8) {
        if level == 6 {
            self.v_interrupt_pending = false;
        } else if level == 4 {
            self.h_interrupt_pending = false;
        }
    }

    fn hblank_active(&self) -> bool {
        let (_, line_start, line_end) = self.line_cycle_bounds_for_cycle(self.frame_cycles);
        let line_cycles = line_end.saturating_sub(line_start).max(1);
        let cycle_in_line = self.frame_cycles.saturating_sub(line_start);
        let h = self.h_counter_value(cycle_in_line, line_cycles);
        if self.h40_mode() {
            h >= 0xB3 || h <= 0x05
        } else {
            h >= 0x93 || h <= 0x04
        }
    }

    fn current_line_index(&self) -> usize {
        self.line_index_for_cycle(self.frame_cycles)
            .min(self.total_lines().saturating_sub(1)) as usize
    }

    fn vblank_active(&self) -> bool {
        self.current_line_index() >= self.active_display_height()
    }

    fn hblank_start_cycle(&self, cycles_per_line: u64) -> u64 {
        // Compute the first cycle where the quantized H-counter reaches the
        // HBlank threshold. Equivalent to the previous scan loop but O(1).
        let threshold_step = if self.h40_mode() { 0xB3u64 } else { 0x93u64 };
        let steps = self.h_counter_step_count().max(1);
        let cycle = (threshold_step
            .saturating_mul(cycles_per_line)
            .saturating_add(steps - 1))
            / steps;
        cycle.min(cycles_per_line.saturating_sub(1))
    }

    fn h_counter_step_count(&self) -> u64 {
        if self.h40_mode() {
            // H40 progression: 00-B6, E4-FF
            0xB7 + (0xFF - 0xE4 + 1)
        } else {
            // H32 progression: 00-93, E9-FF
            0x94 + (0xFF - 0xE9 + 1)
        }
    }

    fn h_counter_value(&self, cycle_in_line: u64, line_cycles: u64) -> u8 {
        let steps = self.h_counter_step_count().max(1);
        let step = ((cycle_in_line * steps) / line_cycles).min(steps.saturating_sub(1));
        if self.h40_mode() {
            if step <= 0xB6 {
                step as u8
            } else {
                (0xE4 + (step - 0xB7)) as u8
            }
        } else if step <= 0x93 {
            step as u8
        } else {
            (0xE9 + (step - 0x94)) as u8
        }
    }

    fn v_counter_value_for_line(&self, line: u64) -> u8 {
        let line = line.min(self.total_lines().saturating_sub(1));
        let v30_mode = self.active_display_height() == FRAME_HEIGHT;
        match (self.video_standard, v30_mode) {
            // NTSC V28: 00-EA, E5-FF
            (VideoStandard::Ntsc, false) => {
                if line <= 0xEA {
                    line as u8
                } else {
                    (0xE5 + (line - 0xEB)) as u8
                }
            }
            // NTSC V30: treat as linear 8-bit wrap (00-FF, 00-05 for 262 lines).
            // This keeps timing stable without introducing invalid rolling-mode behavior.
            (VideoStandard::Ntsc, true) => line as u8,
            // PAL V28: 00-FF, 00-02, CA-FF
            (VideoStandard::Pal, false) => {
                if line <= 0xFF {
                    line as u8
                } else if line <= 0x102 {
                    (line - 0x100) as u8
                } else {
                    (0xCA + (line - 0x103)) as u8
                }
            }
            // PAL V30: 00-FF, 00-0A, D2-FF
            (VideoStandard::Pal, true) => {
                if line <= 0xFF {
                    line as u8
                } else if line <= 0x10A {
                    (line - 0x100) as u8
                } else {
                    (0xD2 + (line - 0x10B)) as u8
                }
            }
        }
    }

    fn line_cycle_bounds_for_cycle(&self, cycle: u64) -> (u64, u64, u64) {
        let line = self.line_index_for_cycle(cycle);
        self.line_cycle_bounds(line)
    }

    fn line_cycle_bounds(&self, line: u64) -> (u64, u64, u64) {
        let cycles_per_frame = self.cycles_per_frame();
        let total_lines = self.total_lines().max(1);
        let clamped_line = line.min(total_lines.saturating_sub(1));
        let start = (clamped_line * cycles_per_frame) / total_lines;
        let end = ((clamped_line + 1) * cycles_per_frame) / total_lines;
        (clamped_line, start, end.max(start + 1))
    }

    fn process_scanline_events(&mut self, start: u64, end: u64) {
        if end <= start {
            return;
        }
        let start_line = self.line_index_for_cycle(start);
        let end_line = self.line_index_for_cycle(end);
        let total_lines = self.total_lines();

        for line in start_line..=end_line {
            let (line_idx, line_start, line_end) = self.line_cycle_bounds(line);
            let line_cycles = line_end.saturating_sub(line_start).max(1);
            let hblank_start = line_start + self.hblank_start_cycle(line_cycles);
            if hblank_start > start && hblank_start <= end {
                // Capture state for the current line at HBlank, BEFORE firing
                // the H-INT.  By this point the previous line's H-INT handler
                // has had a full scanline (~488 CPU cycles) to complete, so
                // register changes (e.g. window position) are reflected.
                let capture_line = if self.debug_line_latch_next {
                    (line_idx as usize).saturating_add(1)
                } else {
                    line_idx as usize
                };
                if capture_line < FRAME_HEIGHT {
                    self.capture_line_state(capture_line);
                }
                self.on_hblank_start(line_idx as usize);
            }

            if line_end > start && line_end <= end {
                let next_line = line_idx + 1;
                if next_line < total_lines {
                    self.on_scanline_start(next_line as usize);
                }
            }
        }
    }

    fn line_index_for_cycle(&self, cycle: u64) -> u64 {
        let cycles_per_frame = self.cycles_per_frame();
        // Treat cycle==frame_end as the final scanline of the current frame.
        // Frame-start events for the next frame are handled by `step` after wrap.
        let clamped = cycle.min(cycles_per_frame.saturating_sub(1));
        (clamped * self.total_lines()) / cycles_per_frame
    }

    fn reset_line_state(&mut self) {
        for line in 0..FRAME_HEIGHT {
            self.line_registers[line] = self.registers;
            self.line_vsram[line] = self.vsram;
            self.line_hscroll[line] = self.current_line_hscroll_words(line, &self.registers);
            self.line_cram[line] = self.cram;
            if self.line_vram_latch_enabled {
                self.line_vram[line].copy_from_slice(&self.vram);
            }
        }
    }

    fn capture_line_state(&mut self, line: usize) {
        if line < FRAME_HEIGHT {
            self.line_registers[line] = self.registers;
            self.line_vsram[line] = self.vsram;
            self.line_hscroll[line] = self.current_line_hscroll_words(line, &self.registers);
            self.line_cram[line] = self.cram;
            if self.line_vram_latch_enabled {
                self.line_vram[line].copy_from_slice(&self.vram);
            }
        }
    }

    fn current_line_hscroll_words(&self, line: usize, regs: &[u8; REG_COUNT]) -> [u16; 2] {
        let hscroll_base = Self::hscroll_table_base_from_regs(regs);
        let a_idx = Self::hscroll_word_index_for_line_from_regs(regs, 0, line);
        let b_idx = Self::hscroll_word_index_for_line_from_regs(regs, 1, line);
        [
            read_u16_be_wrapped(&self.vram, hscroll_base + a_idx * 2),
            read_u16_be_wrapped(&self.vram, hscroll_base + b_idx * 2),
        ]
    }

    fn on_scanline_start(&mut self, line: usize) {
        let active_height = self.active_display_height();
        // V-INT condition is latched on VBlank entry; IRQ output is gated by
        // the current enable bit (register #1 bit 5).
        if line == active_height {
            self.v_interrupt_pending = true;
        }

        // Keep the H-INT counter reloaded throughout VBlank. This avoids
        // carrying partial countdown state across frames.
        if line >= active_height {
            self.h_interrupt_armed = false;
            self.h_interrupt_counter = self.registers[REG_H_INTERRUPT_COUNTER];
            return;
        }

        if self.h_interrupt_counter == 0 {
            // H-INT condition is latched by line counter timing; IRQ output is
            // gated by the current enable bit (register #0 bit 4).
            self.h_interrupt_armed = true;
            self.h_interrupt_counter = self.registers[REG_H_INTERRUPT_COUNTER];
        } else {
            self.h_interrupt_armed = false;
            self.h_interrupt_counter = self.h_interrupt_counter.wrapping_sub(1);
        }
    }

    fn on_hblank_start(&mut self, line: usize) {
        if line < self.active_display_height() && self.h_interrupt_armed {
            self.h_interrupt_pending = true;
            self.h_interrupt_armed = false;
        }
    }

    pub fn set_line_vram_latch_enabled_for_debug(&mut self, enabled: bool) {
        self.line_vram_latch_enabled = enabled;
        self.reset_line_state();
        self.capture_line_state(0);
    }

    pub fn set_sat_line_latch_for_debug(&mut self, enabled: bool) {
        self.set_debug_sprite_flag(Self::DEBUG_SAT_LINE_LATCH_FLAG, enabled);
    }

    pub fn set_sat_live_for_debug(&mut self, enabled: bool) {
        self.set_debug_sprite_flag(Self::DEBUG_SAT_LIVE_FLAG, enabled);
    }

    pub fn set_sat_per_line_for_debug(&mut self, enabled: bool) {
        self.set_debug_sprite_flag(Self::DEBUG_SAT_PER_LINE_FLAG, enabled);
    }

    pub fn set_sprite_pattern_line0_for_debug(&mut self, enabled: bool) {
        self.set_debug_sprite_flag(Self::DEBUG_SPRITE_PATTERN_LINE0_FLAG, enabled);
    }

    pub fn set_sprite_pattern_per_line_for_debug(&mut self, enabled: bool) {
        self.set_debug_sprite_flag(Self::DEBUG_SPRITE_PATTERN_PER_LINE_FLAG, enabled);
    }

    pub(crate) fn refresh_line0_latch_if_active(&mut self) {
        if self.line_vram_latch_enabled && self.line_index_for_cycle(self.frame_cycles) == 0 {
            self.capture_line_state(0);
        }
    }

    fn sprite_table_base(&self) -> usize {
        // In H40 mode the SAT base is 1KB aligned (bit0 ignored).
        let mask = if self.h40_mode() { 0x7E } else { 0x7F };
        ((self.registers[REG_SPRITE_TABLE] as usize & mask) << 9) % VRAM_SIZE
    }

    fn debug_sprite_flag(&self, index: usize) -> bool {
        self.debug_sprite_flags.get(index).copied().unwrap_or(false)
    }

    fn set_debug_sprite_flag(&mut self, index: usize, enabled: bool) {
        if self.debug_sprite_flags.len() <= index {
            self.debug_sprite_flags.resize(index + 1, false);
        }
        self.debug_sprite_flags[index] = enabled;
    }

    fn debug_sat_flag(&self, index: usize) -> bool {
        self.debug_sprite_flag(index)
    }

    fn sprite_pattern_line0_enabled(&self) -> bool {
        let force_per_line = self.debug_sprite_flag(Self::DEBUG_SPRITE_PATTERN_PER_LINE_FLAG)
            || debug_flags::sprite_pattern_per_line();
        !force_per_line
            && (self.debug_sprite_flag(Self::DEBUG_SPRITE_PATTERN_LINE0_FLAG)
                || debug_flags::sprite_pattern_line0())
    }

    fn h_interrupt_enabled(&self) -> bool {
        (self.registers[REG_MODE_SET_1] & 0x10) != 0
    }

    fn v_interrupt_enabled(&self) -> bool {
        (self.registers[REG_MODE_SET_2] & 0x20) != 0
    }

    fn h40_mode(&self) -> bool {
        Self::h40_mode_from_regs(&self.registers)
    }

    fn interlace_mode_enabled(&self) -> bool {
        Self::interlace_mode_2_from_regs(&self.registers)
    }

    fn interlace_mode_2_from_regs(regs: &[u8; REG_COUNT]) -> bool {
        // Reg 12 bits 2:1 select interlace mode.
        // Mode 3 (binary 11, often called interlace mode 2) doubles tile row
        // addressing and toggles odd/even field each frame.
        ((regs[12] >> 1) & 0x03) == 0x03
    }

    fn shadow_highlight_mode_from_regs(regs: &[u8; REG_COUNT]) -> bool {
        // Mode register 12 bit 3 enables shadow/highlight processing.
        (regs[12] & 0x08) != 0
    }

    fn active_display_height(&self) -> usize {
        Self::active_display_height_from_regs(&self.registers)
    }

    fn display_enabled_from_regs(regs: &[u8; REG_COUNT]) -> bool {
        (regs[REG_MODE_SET_2] & 0x40) != 0
    }

    fn mode5_enabled_from_regs(regs: &[u8; REG_COUNT]) -> bool {
        (regs[REG_MODE_SET_2] & 0x04) != 0
    }

    fn h40_mode_from_regs(regs: &[u8; REG_COUNT]) -> bool {
        (regs[12] & 0x01) != 0
    }

    fn active_display_height_from_regs(regs: &[u8; REG_COUNT]) -> usize {
        if (regs[REG_MODE_SET_2] & 0x08) != 0 {
            FRAME_HEIGHT
        } else {
            FRAME_HEIGHT_28_CELL
        }
    }

    fn active_display_width_from_regs(regs: &[u8; REG_COUNT]) -> usize {
        if Self::h40_mode_from_regs(regs) {
            FRAME_WIDTH
        } else {
            FRAME_WIDTH_32_CELL
        }
    }

    fn background_color_index_from_regs(regs: &[u8; REG_COUNT]) -> usize {
        let bg = regs[REG_BACKGROUND_COLOR];
        let palette = ((bg >> 4) & 0x3) as usize;
        let color = (bg & 0x0F) as usize;
        palette * 16 + color
    }
}

fn read_u16_be_wrapped(vram: &[u8; VRAM_SIZE], addr: usize) -> u16 {
    let hi = vram[addr % VRAM_SIZE];
    let lo = vram[(addr + 1) % VRAM_SIZE];
    u16::from_be_bytes([hi, lo])
}

#[cfg(test)]
fn encode_md_color(r: u8, g: u8, b: u8) -> u16 {
    let r = (r & 0x7) as u16;
    let g = (g & 0x7) as u16;
    let b = (b & 0x7) as u16;
    (b << 9) | (g << 5) | (r << 1)
}

fn md_color_to_rgb888(color: u16) -> (u8, u8, u8) {
    let r = ((color >> 1) & 0x7) as u8;
    let g = ((color >> 5) & 0x7) as u8;
    let b = ((color >> 9) & 0x7) as u8;
    (r * 36, g * 36, b * 36)
}

pub(crate) fn shadow_channel(channel: u8) -> u8 {
    // S/H mode uses a 4-bit DAC: shadow output = L (vs normal 2L).
    // This is exactly half the normal brightness.
    channel >> 1
}

pub(crate) fn highlight_channel(channel: u8) -> u8 {
    // S/H mode highlight output = 2L+1 (vs normal 2L).
    // One DAC step (252/14 = 18) above normal brightness.
    (channel as u16 + 18).min(255) as u8
}

fn plane_size_code_to_tiles(code: u8) -> usize {
    match code & 0x3 {
        0x0 => 32,
        0x1 => 64,
        0x3 => 128,
        _ => 32,
    }
}

fn normalize_scroll(value: i16, size: usize) -> usize {
    let size = size as i32;
    let mut wrapped = value as i32 % size;
    if wrapped < 0 {
        wrapped += size;
    }
    wrapped as usize
}

#[cfg(test)]
#[path = "tests/vdp_tests.rs"]
mod tests;
