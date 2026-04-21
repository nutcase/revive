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

    pub fn dma_fill_ops(&self) -> u64 {
        self.dma_fill_ops
    }

    pub fn dma_copy_ops(&self) -> u64 {
        self.dma_copy_ops
    }

    /// Returns true when any DMA operation is in progress or pending.
    pub fn dma_busy(&self) -> bool {
        self.dma_fill_pending.is_some()
            || self.dma_fill_active.is_some()
            || self.dma_copy_active.is_some()
            || self.dma_bus_pending.is_some()
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

    pub fn read_control_port(&mut self) -> u16 {
        // Reading status clears command latch.
        self.control_latch = None;
        let mut status = STATUS_BASE;
        if self.hblank_active() {
            status |= STATUS_HBLANK;
        }
        if self.vblank_active() {
            status |= STATUS_VBLANK;
        }
        if self.interlace_mode_enabled() && (self.frame_count & 1) != 0 {
            status |= STATUS_ODD_FRAME;
        }
        if self.fifo_count == 0 {
            status |= STATUS_FIFO_EMPTY;
        }
        if self.fifo_count >= 4 {
            status |= STATUS_FIFO_FULL;
        }
        if self.dma_busy() {
            status |= STATUS_DMA_BUSY;
        }
        if self.sprite_collision {
            status |= STATUS_SPRITE_COLLISION;
        }
        if self.sprite_overflow {
            status |= STATUS_SPRITE_OVERFLOW;
        }
        self.sprite_collision = false;
        self.sprite_overflow = false;
        status
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

    pub fn read_hv_counter(&self) -> u16 {
        let (line, line_start, line_end) = self.line_cycle_bounds_for_cycle(self.frame_cycles);
        let line_cycles = line_end.saturating_sub(line_start).max(1);
        let cycle_in_line = self.frame_cycles.saturating_sub(line_start);
        let v = self.v_counter_value_for_line(line);
        let h = self.h_counter_value(cycle_in_line, line_cycles);
        u16::from_be_bytes([v, h])
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

    pub fn write_control_port(&mut self, value: u16) {
        // Register set command: 10rrrddd dddddddd
        if self.control_latch.is_none() && (value & 0xC000) == 0x8000 {
            let reg = ((value >> 8) & 0x1F) as usize;
            let data = (value & 0x00FF) as u8;
            self.write_register(reg, data);
            return;
        }

        if let Some(first) = self.control_latch.take() {
            let command = ((first as u32) << 16) | value as u32;
            self.set_access_command(command);
        } else {
            self.control_latch = Some(value);
        }
    }

    pub fn read_data_port(&mut self) -> u16 {
        match self.access_mode {
            AccessMode::VramRead => {
                let value = self.vram_read_buffer;
                self.vram_read_buffer = {
                    let hi = self.vram[self.access_addr as usize];
                    let lo = self.vram[self.access_addr.wrapping_add(1) as usize];
                    u16::from_be_bytes([hi, lo])
                };
                self.advance_access_addr();
                value
            }
            AccessMode::VramWrite => {
                let hi = self.vram[self.access_addr as usize];
                let lo = self.vram[self.access_addr.wrapping_add(1) as usize];
                let value = u16::from_be_bytes([hi, lo]);
                self.advance_access_addr();
                value
            }
            AccessMode::CramRead | AccessMode::CramWrite => {
                let value = self.read_cram_u16((self.access_addr >> 1) as u8);
                self.advance_access_addr();
                value
            }
            AccessMode::VsramRead | AccessMode::VsramWrite => {
                let value = self.read_vsram_u16((self.access_addr >> 1) as u8);
                self.advance_access_addr();
                value
            }
            AccessMode::Unsupported => {
                self.advance_access_addr();
                0
            }
        }
    }

    pub fn write_data_port(&mut self, value: u16) {
        if let Some(fill) = self.dma_fill_pending.take() {
            let no_prewrite = debug_flags::dma_fill_no_prewrite();
            // DMA fill is triggered by a regular data-port write: apply the
            // initial write first, then stream fill bytes.
            if !no_prewrite {
                self.write_data_value(value);
                self.advance_access_addr();
            }
            let fill_byte = (value & 0x00FF) as u8;
            let fill_word = debug_flags::dma_fill_word();
            let lane_no_xor = debug_flags::dma_fill_lane_no_xor();
            self.dma_fill_active = Some(DmaFillActive {
                fill_byte,
                fill_word,
                lane_no_xor,
                increment: self.auto_increment(),
                remaining: fill.remaining_words,
                cycle_carry: 0,
            });
            return;
        }

        self.write_data_value(value);
        self.advance_access_addr();
        if self.fifo_count < 4 {
            self.fifo_count += 1;
        }
    }

    fn write_data_value(&mut self, value: u16) {
        match self.access_mode {
            AccessMode::VramWrite => {
                let addr = self.access_addr as usize;
                let [hi, lo] = value.to_be_bytes();
                self.vram[addr % VRAM_SIZE] = hi;
                self.vram[(addr + 1) % VRAM_SIZE] = lo;
                if self.frame_cycles == 0 {
                    self.reset_line_state();
                    self.capture_line_state(0);
                }
            }
            AccessMode::CramWrite => {
                let index = ((self.access_addr >> 1) as usize) % CRAM_COLORS;
                self.cram[index] = value & 0x0EEE;
                if self.frame_cycles == 0 {
                    self.reset_line_state();
                    self.capture_line_state(0);
                }
            }
            AccessMode::VsramWrite => {
                let index = ((self.access_addr >> 1) as usize) % VSRAM_WORDS;
                self.vsram[index] = value & 0x07FF;
                if self.frame_cycles == 0 {
                    self.reset_line_state();
                    self.capture_line_state(0);
                }
            }
            AccessMode::VramRead
            | AccessMode::CramRead
            | AccessMode::VsramRead
            | AccessMode::Unsupported => {}
        }
    }

    pub fn read_vram_u8(&self, addr: u16) -> u8 {
        self.vram[addr as usize]
    }

    pub fn write_vram_u8(&mut self, addr: u16, value: u8) {
        self.vram[addr as usize] = value;
        if self.frame_cycles == 0 {
            self.reset_line_state();
            self.capture_line_state(0);
        }
    }

    pub fn read_cram_u16(&self, index: u8) -> u16 {
        let i = (index as usize) % CRAM_COLORS;
        self.cram[i]
    }

    pub fn write_cram_u16(&mut self, index: u8, value: u16) {
        let i = (index as usize) % CRAM_COLORS;
        self.cram[i] = value & 0x0EEE;
        if self.frame_cycles == 0 {
            self.reset_line_state();
            self.capture_line_state(0);
        }
    }

    pub fn read_vsram_u16(&self, index: u8) -> u16 {
        let i = (index as usize) % VSRAM_WORDS;
        self.vsram[i]
    }

    pub fn write_vsram_u16(&mut self, index: u8, value: u16) {
        let i = (index as usize) % VSRAM_WORDS;
        self.vsram[i] = value & 0x07FF;
        if self.frame_cycles == 0 {
            self.reset_line_state();
            self.capture_line_state(0);
        }
    }

    pub(crate) fn take_bus_dma_request(&mut self) -> Option<BusDmaRequest> {
        self.dma_bus_pending.take()
    }

    pub(crate) fn complete_bus_dma(&mut self, next_source_addr: u32) {
        // Only update LOW/MID source registers; HIGH is frozen during transfer
        let encoded = (next_source_addr >> 1) & 0x007F_FFFF;
        self.registers[REG_DMA_SOURCE_LOW] = (encoded & 0xFF) as u8;
        self.registers[REG_DMA_SOURCE_MID] = ((encoded >> 8) & 0xFF) as u8;
        self.clear_dma_length();
    }

    fn advance_access_addr(&mut self) {
        let increment = self.auto_increment();
        self.access_addr = self.access_addr.wrapping_add(increment);
    }

    fn auto_increment(&self) -> u16 {
        self.registers[REG_AUTO_INCREMENT] as u16
    }

    fn write_register(&mut self, reg: usize, value: u8) {
        if reg < REG_COUNT {
            let masked = match reg {
                REG_MODE_SET_2 => value & 0x7F,
                REG_PLANE_A_NAMETABLE => value & 0x38,
                REG_WINDOW_NAMETABLE => value & 0x3E,
                REG_PLANE_B_NAMETABLE => value & 0x07,
                REG_SPRITE_TABLE => value & 0x7F,
                REG_BACKGROUND_COLOR => value & 0x3F,
                REG_HSCROLL_TABLE => value & 0x3F,
                REG_WINDOW_HPOS | REG_WINDOW_VPOS => value & 0x9F,
                REG_PLANE_SIZE => value & 0x33,
                REG_AUTO_INCREMENT => value,
                REG_DMA_LENGTH_LOW | REG_DMA_LENGTH_HIGH | REG_DMA_SOURCE_LOW
                | REG_DMA_SOURCE_MID | REG_DMA_SOURCE_HIGH => value,
                _ => value,
            };
            self.registers[reg] = masked;
            if self.frame_cycles == 0 {
                self.reset_line_state();
                self.capture_line_state(0);
            }
        }
    }

    fn set_access_command(&mut self, command: u32) {
        let code = ((command >> 30) as u8 & 0x3) | (((command >> 2) as u8) & 0x3C);
        let base_code = code & 0x1F;
        let dma_request = (code & 0x20) != 0;
        let address = (((command >> 16) & 0x3FFF) as u16) | (((command & 0x3) as u16) << 14);

        self.dma_fill_pending = None;
        self.dma_bus_pending = None;
        self.access_addr = address;
        self.access_mode = match base_code {
            0x00 => AccessMode::VramRead,
            0x01 => AccessMode::VramWrite,
            0x02 => AccessMode::CramRead,
            0x03 => AccessMode::CramWrite,
            0x04 => AccessMode::VsramRead,
            0x05 => AccessMode::VsramWrite,
            _ => AccessMode::Unsupported,
        };

        if self.access_mode == AccessMode::VramRead {
            // VRAM read setup prefetches into an internal read buffer and
            // advances the address once before the first data-port read.
            let hi = self.vram[self.access_addr as usize];
            let lo = self.vram[self.access_addr.wrapping_add(1) as usize];
            self.vram_read_buffer = u16::from_be_bytes([hi, lo]);
            self.advance_access_addr();
        }

        if dma_request && self.dma_enabled() {
            self.start_dma(base_code);
        }
    }

    fn dma_enabled(&self) -> bool {
        (self.registers[REG_MODE_SET_2] & 0x10) != 0
    }

    fn dma_mode(&self) -> u8 {
        let high = self.registers[REG_DMA_SOURCE_HIGH];
        if (high & 0x80) == 0 {
            // 68k bus transfer. In this mode, bit6 contributes to source address.
            (high >> 6) & 0x01
        } else {
            0b10 | ((high >> 6) & 0x01)
        }
    }

    fn dma_length(&self) -> usize {
        let len = ((self.registers[REG_DMA_LENGTH_HIGH] as usize) << 8)
            | self.registers[REG_DMA_LENGTH_LOW] as usize;
        if len == 0 {
            0x10000
        } else {
            len
        }
    }

    fn clear_dma_length(&mut self) {
        self.registers[REG_DMA_LENGTH_LOW] = 0;
        self.registers[REG_DMA_LENGTH_HIGH] = 0;
    }

    fn dma_source_addr(&self) -> u16 {
        ((self.registers[REG_DMA_SOURCE_MID] as u16) << 8)
            | self.registers[REG_DMA_SOURCE_LOW] as u16
    }

    fn set_dma_source_addr(&mut self, addr: u16) {
        self.registers[REG_DMA_SOURCE_LOW] = (addr & 0x00FF) as u8;
        self.registers[REG_DMA_SOURCE_MID] = (addr >> 8) as u8;
    }

    fn dma_bus_source_addr(&self) -> u32 {
        let encoded = ((self.registers[REG_DMA_SOURCE_HIGH] as u32 & 0x7F) << 16)
            | ((self.registers[REG_DMA_SOURCE_MID] as u32) << 8)
            | self.registers[REG_DMA_SOURCE_LOW] as u32;
        (encoded << 1) & 0x00FF_FFFE
    }

    fn start_dma(&mut self, base_code: u8) {
        // DMA writes are valid for VRAM/CRAM/VSRAM write targets.
        if !matches!(
            self.access_mode,
            AccessMode::VramWrite | AccessMode::CramWrite | AccessMode::VsramWrite
        ) {
            return;
        }

        match self.dma_mode() {
            // 68k bus -> VDP transfer.
            0b00 | 0b01 => {
                let target = match self.access_mode {
                    AccessMode::VramWrite => DmaTarget::Vram,
                    AccessMode::CramWrite => DmaTarget::Cram,
                    AccessMode::VsramWrite => DmaTarget::Vsram,
                    _ => return,
                };
                self.dma_bus_pending = Some(BusDmaRequest {
                    source_addr: self.dma_bus_source_addr(),
                    dest_addr: self.access_addr,
                    auto_increment: self.auto_increment(),
                    words: self.dma_length(),
                    target,
                });
            }
            // DMA fill: executes when the next data-port write provides fill value.
            0b10 => {
                if self.access_mode == AccessMode::VramWrite {
                    self.dma_fill_ops = self.dma_fill_ops.saturating_add(1);
                    self.dma_fill_pending = Some(DmaFillState {
                        remaining_words: self.dma_length(),
                    });
                }
            }
            // DMA copy: gradual VRAM-to-VRAM byte copy.
            0b11 => {
                if base_code == 0x01 && self.access_mode == AccessMode::VramWrite {
                    self.dma_copy_ops = self.dma_copy_ops.saturating_add(1);
                    self.dma_copy_active = Some(DmaCopyActive {
                        source_addr: self.dma_source_addr(),
                        increment: self.auto_increment(),
                        remaining: self.dma_length(),
                        cycle_carry: 0,
                    });
                    if self.frame_cycles == 0 {
                        self.complete_dma_copy_immediately();
                    }
                }
            }
            _ => {}
        }
    }

    fn complete_dma_copy_immediately(&mut self) {
        let Some(mut copy) = self.dma_copy_active.take() else {
            return;
        };

        for _ in 0..copy.remaining {
            let byte = self.vram[copy.source_addr as usize % VRAM_SIZE];
            self.vram[self.access_addr as usize % VRAM_SIZE] = byte;
            copy.source_addr = copy.source_addr.wrapping_add(1);
            self.access_addr = self.access_addr.wrapping_add(copy.increment);
        }

        self.set_dma_source_addr(copy.source_addr);
        self.clear_dma_length();
        self.reset_line_state();
        self.capture_line_state(0);
    }

    /// Advance in-progress DMA fill/copy by the given number of master clock
    /// cycles. Called from `step()` each time the frame cycle counter advances.
    fn step_dma(&mut self, cycles: u64) {
        let in_blank = self.vblank_active() || self.hblank_active();

        // --- DMA Fill ---
        if self.dma_fill_active.is_some() {
            let rate = if in_blank {
                DMA_FILL_CYCLES_PER_BYTE_BLANK
            } else {
                DMA_FILL_CYCLES_PER_BYTE_ACTIVE
            };
            self.dma_fill_active.as_mut().unwrap().cycle_carry += cycles;
            while {
                let fill = self.dma_fill_active.as_ref().unwrap();
                fill.cycle_carry >= rate && fill.remaining > 0
            } {
                {
                    let fill = self.dma_fill_active.as_mut().unwrap();
                    fill.cycle_carry -= rate;
                    if fill.fill_word {
                        let addr = self.access_addr as usize % VRAM_SIZE;
                        self.vram[addr] = fill.fill_byte;
                        self.vram[(addr + 1) % VRAM_SIZE] = fill.fill_byte;
                    } else {
                        let addr = if fill.lane_no_xor {
                            self.access_addr as usize
                        } else {
                            self.access_addr as usize ^ 0x0001
                        } % VRAM_SIZE;
                        self.vram[addr] = fill.fill_byte;
                    }
                    self.access_addr = self.access_addr.wrapping_add(fill.increment);
                    fill.remaining -= 1;
                }
                self.refresh_line0_latch_if_active();
            }
            if self.dma_fill_active.as_ref().unwrap().remaining == 0 {
                self.dma_fill_active = None;
                if self.frame_cycles == 0 {
                    self.reset_line_state();
                    self.capture_line_state(0);
                }
                self.clear_dma_length();
            }
        }

        // --- DMA Copy ---
        if self.dma_copy_active.is_some() {
            let rate = if in_blank {
                DMA_COPY_CYCLES_PER_BYTE_BLANK
            } else {
                DMA_COPY_CYCLES_PER_BYTE_ACTIVE
            };
            self.dma_copy_active.as_mut().unwrap().cycle_carry += cycles;
            while {
                let copy = self.dma_copy_active.as_ref().unwrap();
                copy.cycle_carry >= rate && copy.remaining > 0
            } {
                {
                    let copy = self.dma_copy_active.as_mut().unwrap();
                    copy.cycle_carry -= rate;
                    let byte = self.vram[copy.source_addr as usize % VRAM_SIZE];
                    self.vram[self.access_addr as usize % VRAM_SIZE] = byte;
                    copy.source_addr = copy.source_addr.wrapping_add(1);
                    self.access_addr = self.access_addr.wrapping_add(copy.increment);
                    copy.remaining -= 1;
                }
                self.refresh_line0_latch_if_active();
            }
            if self.dma_copy_active.as_ref().unwrap().remaining == 0 {
                let src = self.dma_copy_active.unwrap().source_addr;
                self.dma_copy_active = None;
                self.set_dma_source_addr(src);
                if self.frame_cycles == 0 {
                    self.reset_line_state();
                    self.capture_line_state(0);
                }
                self.clear_dma_length();
            }
        }
    }

    /// Complete any in-progress DMA fill or copy immediately. Useful for tests
    /// that need deterministic results without stepping the VDP clock.
    #[cfg(test)]
    pub(crate) fn flush_pending_dma(&mut self) {
        if let Some(fill) = self.dma_fill_active.take() {
            for _ in 0..fill.remaining {
                if fill.fill_word {
                    let addr = self.access_addr as usize % VRAM_SIZE;
                    self.vram[addr] = fill.fill_byte;
                    self.vram[(addr + 1) % VRAM_SIZE] = fill.fill_byte;
                } else {
                    let addr = if fill.lane_no_xor {
                        self.access_addr as usize
                    } else {
                        self.access_addr as usize ^ 0x0001
                    } % VRAM_SIZE;
                    self.vram[addr] = fill.fill_byte;
                }
                self.access_addr = self.access_addr.wrapping_add(fill.increment);
            }
            self.clear_dma_length();
        }

        if let Some(mut copy) = self.dma_copy_active.take() {
            for _ in 0..copy.remaining {
                let byte = self.vram[copy.source_addr as usize % VRAM_SIZE];
                self.vram[self.access_addr as usize % VRAM_SIZE] = byte;
                copy.source_addr = copy.source_addr.wrapping_add(1);
                self.access_addr = self.access_addr.wrapping_add(copy.increment);
            }
            self.set_dma_source_addr(copy.source_addr);
            self.clear_dma_length();
        }
    }

    #[cfg(test)]
    fn nametable_base(&self) -> usize {
        Self::nametable_base_from_regs(&self.registers)
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

    fn nametable_base_from_regs(regs: &[u8; REG_COUNT]) -> usize {
        ((regs[REG_PLANE_A_NAMETABLE] as usize & 0x38) << 10) % VRAM_SIZE
    }

    fn plane_b_nametable_base_from_regs(regs: &[u8; REG_COUNT]) -> usize {
        ((regs[REG_PLANE_B_NAMETABLE] as usize & 0x07) << 13) % VRAM_SIZE
    }

    fn hscroll_table_base_from_regs(regs: &[u8; REG_COUNT]) -> usize {
        ((regs[REG_HSCROLL_TABLE] as usize & 0x3F) << 10) % VRAM_SIZE
    }

    fn window_nametable_base_from_regs(regs: &[u8; REG_COUNT]) -> usize {
        let mask = if Self::h40_mode_from_regs(regs) {
            0x3C
        } else {
            0x3E
        };
        ((regs[REG_WINDOW_NAMETABLE] as usize & mask) << 10) % VRAM_SIZE
    }

    fn plane_tile_dimensions_from_regs(regs: &[u8; REG_COUNT]) -> (usize, usize) {
        let width_code = regs[REG_PLANE_SIZE] & 0x03;
        let height_code = (regs[REG_PLANE_SIZE] >> 4) & 0x03;
        (
            plane_size_code_to_tiles(width_code),
            plane_size_code_to_tiles(height_code),
        )
    }

    fn window_tile_dimensions_from_regs(regs: &[u8; REG_COUNT]) -> (usize, usize) {
        let width_tiles = if Self::h40_mode_from_regs(regs) {
            64
        } else {
            32
        };
        (width_tiles, 32)
    }

    fn sign_extend_11(value: u16) -> i16 {
        let masked = (value & 0x07FF) as i16;
        (masked << 5) >> 5
    }

    fn vscroll_index_for_x_from_regs(regs: &[u8; REG_COUNT], plane: usize, x: usize) -> usize {
        if (regs[11] & 0x04) == 0 {
            return plane;
        }
        ((x / 16) * 2 + plane) % VSRAM_WORDS
    }

    fn hscroll_word_index_for_line_from_regs(
        regs: &[u8; REG_COUNT],
        plane: usize,
        y: usize,
    ) -> usize {
        match regs[11] & 0x03 {
            // Full-screen scroll (and reserved mode treated as full-screen).
            0x00 | 0x01 => plane,
            // 8-line strips.
            0x02 => (y / 8) * 2 + plane,
            // Per-line scroll.
            0x03 => y * 2 + plane,
            _ => plane,
        }
    }

    #[inline(always)]
    #[allow(clippy::too_many_arguments)]
    fn sample_plane_pixel_cached(
        &self,
        cache: &mut PlaneTileCache,
        vram: &[u8; VRAM_SIZE],
        base: usize,
        sample_x: usize,
        sample_y: usize,
        plane_width_tiles: usize,
        plane_height_tiles: usize,
        use_64x32_paged_layout: bool,
        scroll_plane_layout: bool,
        plane_paged_layout: bool,
        plane_paged_xmajor: bool,
        interlace_mode_2: bool,
        interlace_field: usize,
    ) -> (Option<PlaneSample>, bool) {
        let tile_x = (sample_x / 8) % plane_width_tiles.max(1);
        let tile_y = (sample_y / 8) % plane_height_tiles.max(1);
        let in_tile_x = sample_x & 7;

        if !cache.valid || cache.tile_x != tile_x || cache.sample_y != sample_y {
            let name_addr = if scroll_plane_layout {
                self.scroll_plane_name_addr(
                    base,
                    tile_x,
                    tile_y,
                    plane_width_tiles,
                    plane_height_tiles,
                    use_64x32_paged_layout,
                    plane_paged_layout,
                    plane_paged_xmajor,
                )
            } else {
                base + (tile_y * plane_width_tiles + tile_x) * 2
            };
            let entry = read_u16_be_wrapped(vram, name_addr);
            let tile_index = (entry & 0x07FF) as usize;
            let palette_line = ((entry >> 13) & 0x3) as usize;
            let priority_high = (entry & 0x8000) != 0;
            let hflip = (entry & 0x0800) != 0;
            let vflip = (entry & 0x1000) != 0;
            let mut row_in_tile = sample_y & 7;
            if vflip {
                row_in_tile = 7 - row_in_tile;
            }

            let tile_stride = if interlace_mode_2 {
                TILE_SIZE_BYTES * 2
            } else {
                TILE_SIZE_BYTES
            };
            let row_in_tile = if interlace_mode_2 {
                (row_in_tile << 1) | (interlace_field & 1)
            } else {
                row_in_tile
            };
            let tile_row_addr = tile_index * tile_stride + row_in_tile * 4;
            for dst_x in 0..8 {
                let src_x = if hflip { 7 - dst_x } else { dst_x };
                let tile_byte = vram[(tile_row_addr + src_x / 2) % VRAM_SIZE];
                cache.pixels[dst_x] = if src_x & 1 == 0 {
                    tile_byte >> 4
                } else {
                    tile_byte & 0x0F
                };
            }

            cache.valid = true;
            cache.tile_x = tile_x;
            cache.sample_y = sample_y;
            cache.color_base = palette_line * 16;
            cache.priority_high = priority_high;
        }

        let pixel = cache.pixels[in_tile_x];
        if pixel == 0 {
            return (None, cache.priority_high);
        }

        (
            Some(PlaneSample {
                color_index: cache.color_base + pixel as usize,
                opaque: true,
                priority_high: cache.priority_high,
            }),
            cache.priority_high,
        )
    }

    fn scroll_plane_name_addr(
        &self,
        base: usize,
        tile_x: usize,
        tile_y: usize,
        plane_width_tiles: usize,
        plane_height_tiles: usize,
        use_64x32_paged_layout: bool,
        paged_layout: bool,
        paged_xmajor: bool,
    ) -> usize {
        let wrapped_x = tile_x % plane_width_tiles.max(1);
        let wrapped_y = tile_y % plane_height_tiles.max(1);
        // Some 128-cell maps require 64x32-cell paged addressing (2KB pages).
        if use_64x32_paged_layout {
            let page_width = plane_width_tiles.max(1).div_ceil(64);
            let page_height = plane_height_tiles.max(1).div_ceil(32);
            let page_x = wrapped_x / 64;
            let page_y = wrapped_y / 32;
            let in_page_x = wrapped_x & 63;
            let in_page_y = wrapped_y & 31;
            let page_index = if paged_xmajor {
                page_x * page_height + page_y
            } else {
                page_y * page_width + page_x
            };
            return base + page_index * 64 * 32 * 2 + (in_page_y * 64 + in_page_x) * 2;
        }
        // Optional diagnostic mode: force 32x32-cell paged probing.
        if paged_layout {
            let page_width = plane_width_tiles.max(1).div_ceil(32);
            let page_x = wrapped_x / 32;
            let page_y = wrapped_y / 32;
            let in_page_x = wrapped_x & 31;
            let in_page_y = wrapped_y & 31;
            let page_height = plane_height_tiles.max(1).div_ceil(32);
            let page_index = if paged_xmajor {
                page_x * page_height + page_y
            } else {
                page_y * page_width + page_x
            };
            return base + page_index * 32 * 32 * 2 + (in_page_y * 32 + in_page_x) * 2;
        }
        base + (wrapped_y * plane_width_tiles + wrapped_x) * 2
    }

    fn compose_plane_samples(
        &self,
        front: Option<PlaneSample>,
        back: Option<PlaneSample>,
        ignore_priority: bool,
    ) -> Option<PlaneSample> {
        if ignore_priority {
            return front.or(back);
        }
        match (front, back) {
            (Some(front), Some(back)) => {
                if front.priority_high != back.priority_high {
                    if front.priority_high {
                        Some(front)
                    } else {
                        Some(back)
                    }
                } else {
                    Some(front)
                }
            }
            (Some(front), None) => Some(front),
            (None, Some(back)) => Some(back),
            (None, None) => None,
        }
    }

    fn window_active_at(&self, regs: &[u8; REG_COUNT], x: usize, y: usize) -> bool {
        let active_height = Self::active_display_height_from_regs(regs);
        let active_width = Self::active_display_width_from_regs(regs);
        let hreg = regs[REG_WINDOW_HPOS];
        let vreg = regs[REG_WINDOW_VPOS];
        let hsplit = (((hreg & 0x1F) as usize) * 16).min(active_width);
        let vsplit = (((vreg & 0x1F) as usize) * 8).min(active_height);
        let vactive = if (vreg & 0x80) != 0 {
            y >= vsplit
        } else {
            y < vsplit
        };
        // When an explicit vertical split is defined (vsplit > 0) and
        // the line falls inside the vertical window region, the ENTIRE
        // line uses the window plane (horizontal split is ignored).
        // This matches real hardware behavior where vertical window
        // takes priority over horizontal window.
        if vsplit > 0 && vactive {
            return true;
        }
        let hactive = if (hreg & 0x80) != 0 {
            x >= hsplit
        } else {
            x < hsplit
        };
        hactive && vactive
    }

    fn comix_pretitle_vscroll_swap_active(regs: &[u8; REG_COUNT]) -> bool {
        // Comix Zone uses swapped A/B VSRAM sources during the early pre-title logo scene
        // (32x32 plane setup with per-line hscroll). Later title rollout uses normal mapping.
        regs[REG_PLANE_B_NAMETABLE] == 0x07
            && (regs[REG_MODE_SET_2] & 0x40) != 0
            && regs[REG_HSCROLL_TABLE] == 0x3C
            && regs[REG_PLANE_SIZE] == 0x01
            && regs[11] == 0x03
    }

    fn comix_title_roll_active(regs: &[u8; REG_COUNT], vsram: &[u16; VSRAM_WORDS]) -> bool {
        if !(regs[REG_PLANE_B_NAMETABLE] == 0x07
            && (regs[REG_MODE_SET_2] & 0x40) != 0
            && regs[REG_HSCROLL_TABLE] == 0x3C
            && regs[REG_PLANE_SIZE] == 0x11
            && regs[11] == 0x00
            && (regs[12] & 0x08) != 0)
        {
            return false;
        }
        // During the roll-down animation, VSRAM has non-zero scroll values
        // (the roll effect is driven by H-INT updating scroll per line).
        // On the static start menu, all VSRAM entries are zero.
        // Require at least one non-zero entry to avoid false-positive
        // sparse mask clipping on the start menu.
        vsram.iter().any(|&v| v != 0)
    }

    /// For the plane B nametable, compute the first pixel row whose nametable
    /// entries overlap with the HSCROLL table in VRAM.  Returns `None` if there
    /// is no overlap.
    fn plane_b_hscroll_overlap_pixel_row(regs: &[u8; REG_COUNT]) -> Option<usize> {
        let plane_b_base = Self::plane_b_nametable_base_from_regs(regs);
        let hscroll_base = Self::hscroll_table_base_from_regs(regs);
        let (plane_width_tiles, plane_height_tiles) = Self::plane_tile_dimensions_from_regs(regs);
        if plane_width_tiles == 0 {
            return None;
        }
        let row_bytes = plane_width_tiles * 2;
        let plane_size_bytes = row_bytes * plane_height_tiles;
        let plane_end = plane_b_base + plane_size_bytes;
        if hscroll_base >= plane_b_base && hscroll_base < plane_end {
            let overlap_tile_row = (hscroll_base - plane_b_base) / row_bytes;
            Some(overlap_tile_row * 8)
        } else {
            None
        }
    }

    fn render_frame(&mut self) {
        self.sprite_collision = false;
        self.sprite_overflow = false;

        // Mode 4 (SMS compatibility): active when Mode 5 bit is clear and
        // H40 mode is not set (H40 is a Mode 5-only feature).
        if !Self::mode5_enabled_from_regs(&self.registers)
            && !Self::h40_mode_from_regs(&self.registers)
        {
            self.render_frame_mode4();
            return;
        }

        let disable_plane_a = debug_flags::disable_plane_a();
        let disable_plane_b = debug_flags::disable_plane_b();
        let disable_window = debug_flags::disable_window() || debug_flags::force_window_off();
        let disable_sprites =
            debug_flags::disable_sprites() || debug_flags::force_disable_sprites();
        let invert_vscroll_a = debug_flags::invert_vscroll_a();
        let invert_vscroll_b = debug_flags::invert_vscroll_b();
        let debug_swap_vscroll_ab = debug_flags::vscroll_swap_ab();
        let plane_paged_layout = debug_flags::plane_paged();
        let plane_paged_layout_a = plane_paged_layout || debug_flags::plane_a_paged();
        let plane_paged_layout_b = plane_paged_layout || debug_flags::plane_b_paged();
        let plane_paged_xmajor = debug_flags::plane_paged_xmajor();
        let plane_paged_xmajor_a = plane_paged_xmajor || debug_flags::plane_a_paged_xmajor();
        let plane_paged_xmajor_b = plane_paged_xmajor || debug_flags::plane_b_paged_xmajor();
        let force_plane_live_vram = debug_flags::plane_live_vram();
        let use_plane_line_latch = self.line_vram_latch_enabled && debug_flags::plane_line_latch();
        let live_cram = debug_flags::live_cram();
        let line_offset = debug_flags::line_offset();
        let bottom_bg_mask = debug_flags::bottom_bg_mask();
        let hscroll_live = debug_flags::hscroll_live();
        let disable_64x32_paged = debug_flags::disable_64x32_paged();
        let disable_64x32_paged_a = debug_flags::disable_64x32_paged_a();
        let disable_64x32_paged_b = debug_flags::disable_64x32_paged_b();
        let debug_plane_a_64x32_paged = debug_flags::plane_a_64x32_paged();
        let debug_plane_b_64x32_paged = debug_flags::plane_b_64x32_paged();
        let disable_comix_roll_fix = debug_flags::disable_comix_roll_fix();
        let comix_roll_offset = debug_flags::comix_roll_y();
        let disable_comix_roll_sparse_mask = debug_flags::disable_comix_roll_sparse_mask();
        let ignore_plane_priority = debug_flags::ignore_plane_priority();
        let mut plane_meta = std::mem::take(&mut self.render_plane_meta);
        if plane_meta.len() != FRAME_WIDTH * FRAME_HEIGHT {
            plane_meta.resize(FRAME_WIDTH * FRAME_HEIGHT, 0);
        }
        plane_meta.fill(0);
        let mut line_plane_b_opaque_pixels = [0usize; FRAME_HEIGHT];
        let mut comix_title_roll_any = false;
        let mut comix_title_roll_active_height = 0usize;
        for y in 0..FRAME_HEIGHT {
            let line_idx = y
                .saturating_add_signed(line_offset)
                .min(FRAME_HEIGHT.saturating_sub(1));
            let regs = self
                .line_registers
                .get(line_idx)
                .copied()
                .unwrap_or(self.registers);
            let vsram = self.line_vsram.get(line_idx).copied().unwrap_or(self.vsram);
            let hscroll_words = self
                .line_hscroll
                .get(line_idx)
                .copied()
                .unwrap_or_else(|| self.current_line_hscroll_words(y, &regs));
            let hscroll_words = if hscroll_live {
                self.current_line_hscroll_words(line_idx, &regs)
            } else {
                hscroll_words
            };
            let cram = if live_cram {
                self.cram
            } else {
                self.line_cram.get(line_idx).copied().unwrap_or(self.cram)
            };
            let vram = if use_plane_line_latch && !force_plane_live_vram {
                self.line_vram.get(line_idx).unwrap_or(&self.vram)
            } else {
                &self.vram
            };
            let row = y * FRAME_WIDTH * 3;
            if !Self::display_enabled_from_regs(&regs) {
                self.frame_buffer[row..row + FRAME_WIDTH * 3].fill(0);
                continue;
            }
            let line_active_height = Self::active_display_height_from_regs(&regs);
            if y >= line_active_height {
                self.frame_buffer[row..row + FRAME_WIDTH * 3].fill(0);
                continue;
            }

            let line_active_width = Self::active_display_width_from_regs(&regs);
            let plane_a_base = Self::nametable_base_from_regs(&regs);
            let plane_b_base = Self::plane_b_nametable_base_from_regs(&regs);
            let window_base = Self::window_nametable_base_from_regs(&regs);
            let (plane_width_tiles, plane_height_tiles) =
                Self::plane_tile_dimensions_from_regs(&regs);
            let (window_width_tiles, window_height_tiles) =
                Self::window_tile_dimensions_from_regs(&regs);
            let auto_64x32_paged = plane_width_tiles > 64;
            let plane_a_uses_64x32_paged = !disable_64x32_paged
                && !disable_64x32_paged_a
                && (debug_plane_a_64x32_paged || auto_64x32_paged);
            let plane_b_uses_64x32_paged = !disable_64x32_paged
                && !disable_64x32_paged_b
                && (debug_plane_b_64x32_paged || auto_64x32_paged);
            let plane_width_px = plane_width_tiles * 8;
            let plane_height_px = plane_height_tiles * 8;
            let window_width_px = window_width_tiles * 8;
            let window_height_px = window_height_tiles * 8;
            let bg_color_index = Self::background_color_index_from_regs(&regs);
            let interlace_mode_2 = Self::interlace_mode_2_from_regs(&regs);
            let interlace_field = if interlace_mode_2 {
                (self.frame_count & 1) as usize
            } else {
                0
            };

            let a_hscroll =
                normalize_scroll(Self::sign_extend_11(hscroll_words[0]), plane_width_px);
            let b_hscroll =
                normalize_scroll(Self::sign_extend_11(hscroll_words[1]), plane_width_px);
            let comix_swap_fix_active = Self::comix_pretitle_vscroll_swap_active(&regs)
                || Self::comix_title_roll_active(&regs, &vsram);
            let comix_title_roll = comix_swap_fix_active
                && Self::comix_title_roll_active(&regs, &vsram)
                && !disable_comix_roll_fix;
            // Suppress plane B pixels whose nametable entries fall inside the
            // HSCROLL table region.  This is computed from the actual register
            // values each line (independent of comix_title_roll_active) so that
            // mid-frame register changes from H-INT don't create gaps.
            let comix_roll_overlap_limit = if comix_swap_fix_active {
                Self::plane_b_hscroll_overlap_pixel_row(&regs)
            } else {
                None
            };
            let swap_vscroll_ab =
                debug_swap_vscroll_ab || Self::comix_pretitle_vscroll_swap_active(&regs);
            if comix_title_roll {
                comix_title_roll_any = true;
                comix_title_roll_active_height = line_active_height;
            }
            let mut line_b_opaque = 0usize;
            let mut plane_a_tile_cache = PlaneTileCache::default();
            let mut plane_b_tile_cache = PlaneTileCache::default();
            let mut window_tile_cache = PlaneTileCache::default();

            for x in 0..FRAME_WIDTH {
                if x >= line_active_width {
                    let out = row + x * 3;
                    self.frame_buffer[out] = 0;
                    self.frame_buffer[out + 1] = 0;
                    self.frame_buffer[out + 2] = 0;
                    continue;
                }
                let (a_idx, b_idx) = if swap_vscroll_ab {
                    (1usize, 0usize)
                } else {
                    (0usize, 1usize)
                };
                let a_vscroll_raw = Self::sign_extend_11(
                    vsram[Self::vscroll_index_for_x_from_regs(&regs, a_idx, x) % VSRAM_WORDS],
                );
                let b_vscroll_raw = Self::sign_extend_11(
                    vsram[Self::vscroll_index_for_x_from_regs(&regs, b_idx, x) % VSRAM_WORDS],
                );
                let a_vscroll_raw = if interlace_mode_2 {
                    a_vscroll_raw >> 1
                } else {
                    a_vscroll_raw
                };
                let b_vscroll_raw = if interlace_mode_2 {
                    b_vscroll_raw >> 1
                } else {
                    b_vscroll_raw
                };
                let a_vscroll = normalize_scroll(a_vscroll_raw, plane_height_px);
                let b_vscroll = normalize_scroll(b_vscroll_raw, plane_height_px);
                let (plane_b, plane_b_raw_pri) = if disable_plane_b {
                    (None, false)
                } else {
                    let mut sample_y = if invert_vscroll_b {
                        (y + plane_height_px - b_vscroll) % plane_height_px
                    } else {
                        (y + b_vscroll) % plane_height_px
                    };
                    if comix_title_roll {
                        sample_y = (sample_y as isize + comix_roll_offset as isize)
                            .rem_euclid(plane_height_px as isize)
                            as usize;
                    }
                    if comix_roll_overlap_limit.map_or(false, |limit| sample_y >= limit) {
                        (None, false)
                    } else {
                        self.sample_plane_pixel_cached(
                            &mut plane_b_tile_cache,
                            vram,
                            plane_b_base,
                            (x + plane_width_px - b_hscroll) % plane_width_px,
                            sample_y,
                            plane_width_tiles,
                            plane_height_tiles,
                            plane_b_uses_64x32_paged,
                            true,
                            plane_paged_layout_b,
                            plane_paged_xmajor_b,
                            interlace_mode_2,
                            interlace_field,
                        )
                    }
                };
                if plane_b.is_some() {
                    line_b_opaque = line_b_opaque.saturating_add(1);
                }

                let (front_plane, front_raw_pri) =
                    if !disable_window && self.window_active_at(&regs, x, y) {
                        self.sample_plane_pixel_cached(
                            &mut window_tile_cache,
                            vram,
                            window_base,
                            x % window_width_px,
                            y % window_height_px,
                            window_width_tiles,
                            window_height_tiles,
                            false,
                            false,
                            false,
                            false,
                            interlace_mode_2,
                            interlace_field,
                        )
                    } else {
                        let sample_y = if invert_vscroll_a {
                            (y + plane_height_px - a_vscroll) % plane_height_px
                        } else {
                            (y + a_vscroll) % plane_height_px
                        };
                        self.sample_plane_pixel_cached(
                            &mut plane_a_tile_cache,
                            vram,
                            plane_a_base,
                            (x + plane_width_px - a_hscroll) % plane_width_px,
                            sample_y,
                            plane_width_tiles,
                            plane_height_tiles,
                            plane_a_uses_64x32_paged,
                            true,
                            plane_paged_layout_a,
                            plane_paged_xmajor_a,
                            interlace_mode_2,
                            interlace_field,
                        )
                    };
                let front_plane = if disable_plane_a { None } else { front_plane };

                let mut composed =
                    self.compose_plane_samples(front_plane, plane_b, ignore_plane_priority);
                if bottom_bg_mask && y >= line_active_height.saturating_sub(32) {
                    composed = None;
                }
                let color_index = composed
                    .map(|sample| sample.color_index)
                    .unwrap_or(bg_color_index);
                let color = cram[color_index % CRAM_COLORS];
                let (r, g, b) = md_color_to_rgb888(color);

                // Shadow/Highlight mode: if ANY plane has priority set at this
                // pixel (even if transparent), the pixel is at normal brightness.
                // Otherwise it is shadowed.
                let line_sh = Self::shadow_highlight_mode_from_regs(&regs);
                let any_plane_priority = front_raw_pri || plane_b_raw_pri;
                let (r, g, b) = if line_sh && !any_plane_priority {
                    (shadow_channel(r), shadow_channel(g), shadow_channel(b))
                } else {
                    (r, g, b)
                };

                let out = row + x * 3;
                self.frame_buffer[out] = r;
                self.frame_buffer[out + 1] = g;
                self.frame_buffer[out + 2] = b;

                let meta_index = y * FRAME_WIDTH + x;
                // Encode: bit 0 = opaque, bit 1 = composed pixel priority
                // (for sprite vs plane ordering), bits 2..7 = color_index.
                // Note: S/H uses any_plane_priority (OR of raw priorities)
                // which is computed separately above and not stored here.
                let ci = (color_index as u8) & 0x3F;
                let opaque = composed.map(|s| s.opaque).unwrap_or(false);
                let composed_pri = composed.map(|s| s.priority_high).unwrap_or(false);
                plane_meta[meta_index] = (opaque as u8) | ((composed_pri as u8) << 1) | (ci << 2);
            }
            if comix_title_roll && !disable_comix_roll_sparse_mask {
                line_plane_b_opaque_pixels[y] = line_b_opaque;
            }
        }

        if comix_title_roll_any && !disable_comix_roll_sparse_mask {
            let min_pixels = debug_flags::comix_roll_min_pixels();
            let run_required = debug_flags::comix_roll_min_run().max(1);
            let search_start = (comix_title_roll_active_height / 3).max(48);
            let search_end = comix_title_roll_active_height.min(FRAME_HEIGHT);
            let mut run = 0usize;
            let mut clip_start = None;
            for y in search_start..search_end {
                if line_plane_b_opaque_pixels[y] < min_pixels {
                    run = run.saturating_add(1);
                    if run >= run_required {
                        clip_start = Some(y + 1 - run_required);
                        break;
                    }
                } else {
                    run = 0;
                }
            }
            if let Some(start) = clip_start {
                for y in start..search_end {
                    let row = y * FRAME_WIDTH * 3;
                    self.frame_buffer[row..row + FRAME_WIDTH * 3].fill(0);
                    plane_meta[y * FRAME_WIDTH..(y + 1) * FRAME_WIDTH].fill(0);
                }
            }
        }

        if !disable_sprites {
            self.render_sprites(&plane_meta);
        }
        self.render_plane_meta = plane_meta;
    }

    fn render_sprites(&mut self, plane_meta: &[u8]) {
        let max_sat_sprites = if self.h40_mode() { 80usize } else { 64usize };
        let sat_use_line_latched = self.line_vram_latch_enabled
            && (self.debug_sat_flag(Self::DEBUG_SAT_LINE_LATCH_FLAG)
                || debug_flags::sat_line_latch());
        let sat_use_live = self.debug_sat_flag(Self::DEBUG_SAT_LIVE_FLAG)
            || debug_flags::sat_live()
            || !sat_use_line_latched;
        let sat_per_line =
            self.debug_sat_flag(Self::DEBUG_SAT_PER_LINE_FLAG) || debug_flags::sat_per_line();
        let sprite_x_offset = debug_flags::sprite_x_offset();
        let sprite_y_offset = debug_flags::sprite_y_offset();
        if sat_per_line {
            self.render_sprites_per_line(
                plane_meta,
                sat_use_live,
                sprite_x_offset,
                sprite_y_offset,
            );
            return;
        }
        let mut sprites_on_line = [0u8; FRAME_HEIGHT];
        let mut sprite_pixels_on_line = [0u16; FRAME_HEIGHT];
        let mut masked_line = [false; FRAME_HEIGHT];
        let mut sprite_filled = std::mem::take(&mut self.render_sprite_filled);
        if sprite_filled.len() != FRAME_WIDTH * FRAME_HEIGHT {
            sprite_filled.resize(FRAME_WIDTH * FRAME_HEIGHT, false);
        }
        sprite_filled.fill(false);
        let mut index = 0usize;

        for _ in 0..max_sat_sprites {
            let entry_addr = self.sprite_table_base() + index * 8;
            let (mut y_word, mut size_link, mut attr, mut x_word) = {
                // Use live SAT by default; line-latched SAT can be enabled for diagnostics.
                let sat_vram = if sat_use_live {
                    &self.vram
                } else {
                    self.line_vram.first().unwrap_or(&self.vram)
                };
                (
                    read_u16_be_wrapped(sat_vram, entry_addr),
                    read_u16_be_wrapped(sat_vram, entry_addr + 2),
                    read_u16_be_wrapped(sat_vram, entry_addr + 4),
                    read_u16_be_wrapped(sat_vram, entry_addr + 6),
                )
            };
            if sat_use_line_latched {
                let mut y = (y_word & 0x03FF) as i32 - 128;
                if self.interlace_mode_enabled() {
                    y >>= 1;
                }
                let line = y.clamp(0, (FRAME_HEIGHT - 1) as i32) as usize;
                let sat_vram = self.line_vram.get(line).unwrap_or(&self.vram);
                y_word = read_u16_be_wrapped(sat_vram, entry_addr);
                size_link = read_u16_be_wrapped(sat_vram, entry_addr + 2);
                attr = read_u16_be_wrapped(sat_vram, entry_addr + 4);
                x_word = read_u16_be_wrapped(sat_vram, entry_addr + 6);
            }

            self.draw_sprite(
                y_word,
                size_link,
                attr,
                x_word,
                plane_meta,
                &mut sprite_filled,
                &mut masked_line,
                &mut sprites_on_line,
                &mut sprite_pixels_on_line,
                sprite_x_offset,
                sprite_y_offset,
            );

            let link = (size_link & 0x007F) as usize;
            if link == 0 || link == index || link >= max_sat_sprites {
                break;
            }
            index = link;
        }
        self.render_sprite_filled = sprite_filled;
    }

    #[allow(clippy::too_many_arguments)]
    fn render_sprites_per_line(
        &mut self,
        plane_meta: &[u8],
        sat_use_live: bool,
        sprite_x_offset: i32,
        sprite_y_offset: i32,
    ) {
        let swap_size = debug_flags::sprite_swap_size();
        let sprite_pattern_line0 = self.sprite_pattern_line0_enabled();
        let sprite_row_major = debug_flags::sprite_row_major();
        let disable_mask_sprite = debug_flags::disable_sprite_mask();

        let mut sprite_filled = std::mem::take(&mut self.render_sprite_filled);
        if sprite_filled.len() != FRAME_WIDTH * FRAME_HEIGHT {
            sprite_filled.resize(FRAME_WIDTH * FRAME_HEIGHT, false);
        }
        sprite_filled.fill(false);
        let sat_base = self.sprite_table_base();
        for dy in 0..FRAME_HEIGHT {
            let regs = self
                .line_registers
                .get(dy)
                .copied()
                .unwrap_or(self.registers);
            let interlace_mode_2 = Self::interlace_mode_2_from_regs(&regs);
            let interlace_field = if interlace_mode_2 {
                (self.frame_count & 1) as usize
            } else {
                0
            };
            if !Self::display_enabled_from_regs(&regs) {
                continue;
            }
            let line_active_height = Self::active_display_height_from_regs(&regs);
            if dy >= line_active_height {
                continue;
            }
            let line_active_width = Self::active_display_width_from_regs(&regs);
            let (max_sprites_per_line, max_pixels_per_line) = if Self::h40_mode_from_regs(&regs) {
                (20usize, line_active_width)
            } else {
                (16usize, line_active_width)
            };
            let max_sat_sprites = if Self::h40_mode_from_regs(&regs) {
                80usize
            } else {
                64usize
            };
            let sat_vram = if sat_use_live {
                &self.vram
            } else {
                self.line_vram.get(dy).unwrap_or(&self.vram)
            };
            let pattern_vram = if sprite_pattern_line0 {
                if sat_use_live {
                    &self.vram
                } else {
                    self.line_vram.first().unwrap_or(&self.vram)
                }
            } else {
                if sat_use_live {
                    &self.vram
                } else {
                    self.line_vram.get(dy).unwrap_or(&self.vram)
                }
            };

            let mut masked = false;
            let mut line_sprites = 0usize;
            let mut line_pixels = 0usize;
            let mut index = 0usize;
            let mut visited = vec![false; max_sat_sprites];

            for _ in 0..max_sat_sprites {
                if index >= max_sat_sprites || visited[index] {
                    break;
                }
                visited[index] = true;
                let entry_addr = sat_base + index * 8;
                let y_word = read_u16_be_wrapped(sat_vram, entry_addr);
                let size_link = read_u16_be_wrapped(sat_vram, entry_addr + 2);
                let attr = read_u16_be_wrapped(sat_vram, entry_addr + 4);
                let x_word = read_u16_be_wrapped(sat_vram, entry_addr + 6);
                let link = (size_link & 0x007F) as usize;

                let x = (x_word & 0x01FF) as i32 - 128 + sprite_x_offset;
                let mut y = (y_word & 0x03FF) as i32 - 128 + sprite_y_offset;
                if interlace_mode_2 {
                    y >>= 1;
                }
                let is_mask_sprite = (x_word & 0x01FF) == 0 && !disable_mask_sprite;
                let (width_tiles, height_tiles) = if swap_size {
                    (
                        ((size_link >> 8) & 0x3) as usize + 1,
                        ((size_link >> 10) & 0x3) as usize + 1,
                    )
                } else {
                    (
                        ((size_link >> 10) & 0x3) as usize + 1,
                        ((size_link >> 8) & 0x3) as usize + 1,
                    )
                };
                let width_px = width_tiles * 8;
                let height_px = height_tiles * 8;
                let dy_i32 = dy as i32;
                let covered = dy_i32 >= y && dy_i32 < y + height_px as i32;
                if covered {
                    if is_mask_sprite {
                        masked = true;
                    } else if !masked {
                        if line_sprites >= max_sprites_per_line {
                            self.sprite_overflow = true;
                        } else {
                            line_sprites += 1;
                            let sprite_priority_high = (attr & 0x8000) != 0;
                            let tile_base = (attr & 0x07FF) as usize;
                            let palette_line = ((attr >> 13) & 0x3) as usize;
                            let hflip = (attr & 0x0800) != 0;
                            let vflip = (attr & 0x1000) != 0;
                            let line_shadow_highlight =
                                Self::shadow_highlight_mode_from_regs(&regs);
                            let sy = (dy_i32 - y) as usize;
                            let src_y = if vflip { height_px - 1 - sy } else { sy };
                            let tile_row = src_y / 8;
                            let in_tile_y = src_y & 7;
                            let in_tile_y = if interlace_mode_2 {
                                (in_tile_y << 1) | interlace_field
                            } else {
                                in_tile_y
                            };
                            let tile_stride = if interlace_mode_2 {
                                TILE_SIZE_BYTES * 2
                            } else {
                                TILE_SIZE_BYTES
                            };
                            for sx in 0..width_px {
                                if line_pixels >= max_pixels_per_line {
                                    self.sprite_overflow = true;
                                    break;
                                }
                                // Consume sprite dot budget including transparent/offscreen dots.
                                line_pixels += 1;

                                let src_x = if hflip { width_px - 1 - sx } else { sx };
                                let dx = x + sx as i32;
                                if !(0..line_active_width as i32).contains(&dx) {
                                    continue;
                                }
                                let tile_col = src_x / 8;
                                let in_tile_x = src_x & 7;
                                let tile_index = if sprite_row_major {
                                    tile_base + tile_row * width_tiles + tile_col
                                } else {
                                    tile_base + tile_col * height_tiles + tile_row
                                };
                                let tile_addr =
                                    tile_index * tile_stride + in_tile_y * 4 + in_tile_x / 2;
                                let tile_byte = pattern_vram[tile_addr % VRAM_SIZE];
                                let pixel = if in_tile_x & 1 == 0 {
                                    tile_byte >> 4
                                } else {
                                    tile_byte & 0x0F
                                };
                                if pixel == 0 {
                                    continue;
                                }

                                let meta_index = dy * FRAME_WIDTH + dx as usize;
                                let meta = plane_meta[meta_index];
                                let plane_opaque = (meta & 0x01) != 0;
                                let plane_priority_high = (meta & 0x02) != 0;
                                if !sprite_priority_high && plane_opaque && plane_priority_high {
                                    continue;
                                }

                                if line_shadow_highlight
                                    && palette_line == 3
                                    && (pixel == 14 || pixel == 15)
                                {
                                    let plane_ci = ((meta >> 2) & 0x3F) as usize;
                                    let plane_color = self.line_cram[dy][plane_ci % CRAM_COLORS];
                                    let (pr, pg, pb) = md_color_to_rgb888(plane_color);
                                    let out = meta_index * 3;
                                    if pixel == 15 {
                                        self.frame_buffer[out] = shadow_channel(pr);
                                        self.frame_buffer[out + 1] = shadow_channel(pg);
                                        self.frame_buffer[out + 2] = shadow_channel(pb);
                                    } else {
                                        if !plane_priority_high {
                                            self.frame_buffer[out] = pr;
                                            self.frame_buffer[out + 1] = pg;
                                            self.frame_buffer[out + 2] = pb;
                                        } else {
                                            self.frame_buffer[out] = highlight_channel(pr);
                                            self.frame_buffer[out + 1] = highlight_channel(pg);
                                            self.frame_buffer[out + 2] = highlight_channel(pb);
                                        }
                                    }
                                    continue;
                                }

                                let color_index = palette_line * 16 + pixel as usize;
                                let color = self.line_cram[dy][color_index % CRAM_COLORS];
                                let (r, g, b) = md_color_to_rgb888(color);
                                // S/H mode: high-priority sprite → normal,
                                // low-priority sprite → shadow.
                                // Both sprite & plane high priority → highlight.
                                let (r, g, b) = if line_shadow_highlight {
                                    if sprite_priority_high && plane_opaque && plane_priority_high {
                                        (
                                            highlight_channel(r),
                                            highlight_channel(g),
                                            highlight_channel(b),
                                        )
                                    } else if sprite_priority_high {
                                        (r, g, b)
                                    } else {
                                        (shadow_channel(r), shadow_channel(g), shadow_channel(b))
                                    }
                                } else {
                                    (r, g, b)
                                };
                                let out = meta_index * 3;
                                if sprite_filled[meta_index] {
                                    self.sprite_collision = true;
                                    continue;
                                }
                                self.frame_buffer[out] = r;
                                self.frame_buffer[out + 1] = g;
                                self.frame_buffer[out + 2] = b;
                                sprite_filled[meta_index] = true;
                            }
                        }
                    }
                }

                if link == 0 || link == index || link >= max_sat_sprites {
                    break;
                }
                index = link;
            }
        }
        self.render_sprite_filled = sprite_filled;
    }

    fn draw_sprite(
        &mut self,
        y_word: u16,
        size_link: u16,
        attr: u16,
        x_word: u16,
        plane_meta: &[u8],
        sprite_filled: &mut [bool],
        masked_line: &mut [bool; FRAME_HEIGHT],
        sprites_on_line: &mut [u8; FRAME_HEIGHT],
        sprite_pixels_on_line: &mut [u16; FRAME_HEIGHT],
        sprite_x_offset: i32,
        sprite_y_offset: i32,
    ) {
        // Sprite X coordinate is 9-bit (0..511), offset by 128.
        let x = (x_word & 0x01FF) as i32 - 128 + sprite_x_offset;
        let mut y = (y_word & 0x03FF) as i32 - 128 + sprite_y_offset;
        if self.interlace_mode_enabled() {
            y >>= 1;
        }
        let swap_size = debug_flags::sprite_swap_size();
        let (width_tiles, height_tiles) = if swap_size {
            (
                ((size_link >> 8) & 0x3) as usize + 1,
                ((size_link >> 10) & 0x3) as usize + 1,
            )
        } else {
            (
                ((size_link >> 10) & 0x3) as usize + 1,
                ((size_link >> 8) & 0x3) as usize + 1,
            )
        };
        let sprite_priority_high = (attr & 0x8000) != 0;
        let tile_base = (attr & 0x07FF) as usize;
        let palette_line = ((attr >> 13) & 0x3) as usize;
        let hflip = (attr & 0x0800) != 0;
        let vflip = (attr & 0x1000) != 0;
        let width_px = width_tiles * 8;
        let height_px = height_tiles * 8;
        let disable_mask_sprite = debug_flags::disable_sprite_mask();
        let is_mask_sprite = (x_word & 0x01FF) == 0 && !disable_mask_sprite;
        let sprite_pattern_line0 = self.sprite_pattern_line0_enabled();
        let sprite_row_major = debug_flags::sprite_row_major();

        for sy in 0..height_px {
            let src_y = if vflip { height_px - 1 - sy } else { sy };
            let dy = y + sy as i32;
            if !(0..FRAME_HEIGHT as i32).contains(&dy) {
                continue;
            }
            let dy_index = dy as usize;
            let regs = self
                .line_registers
                .get(dy_index)
                .copied()
                .unwrap_or(self.registers);
            if !Self::display_enabled_from_regs(&regs) {
                continue;
            }
            let line_active_height = Self::active_display_height_from_regs(&regs);
            if dy_index >= line_active_height {
                continue;
            }
            let line_active_width = Self::active_display_width_from_regs(&regs);
            let (line_max_sprites_per_line, line_max_pixels_per_line) =
                if Self::h40_mode_from_regs(&regs) {
                    (20usize, line_active_width)
                } else {
                    (16usize, line_active_width)
                };
            let interlace_mode_2 = Self::interlace_mode_2_from_regs(&regs);
            let interlace_field = if interlace_mode_2 {
                (self.frame_count & 1) as usize
            } else {
                0
            };
            let line_shadow_highlight = Self::shadow_highlight_mode_from_regs(&regs);
            if is_mask_sprite {
                masked_line[dy_index] = true;
                continue;
            }
            if masked_line[dy_index] {
                continue;
            }
            if sprites_on_line[dy_index] as usize >= line_max_sprites_per_line {
                self.sprite_overflow = true;
                continue;
            }
            sprites_on_line[dy_index] = sprites_on_line[dy_index].saturating_add(1);

            let tile_row = src_y / 8;
            let in_tile_y = src_y & 7;
            let in_tile_y = if interlace_mode_2 {
                (in_tile_y << 1) | interlace_field
            } else {
                in_tile_y
            };
            let tile_stride = if interlace_mode_2 {
                TILE_SIZE_BYTES * 2
            } else {
                TILE_SIZE_BYTES
            };
            for sx in 0..width_px {
                let src_x = if hflip { width_px - 1 - sx } else { sx };
                let dx = x + sx as i32;
                if sprite_pixels_on_line[dy_index] as usize >= line_max_pixels_per_line {
                    self.sprite_overflow = true;
                    break;
                }
                // VDP line sprite budget is consumed by visible sprite dots,
                // including transparent/offscreen pixels.
                sprite_pixels_on_line[dy_index] = sprite_pixels_on_line[dy_index].saturating_add(1);
                if !(0..line_active_width as i32).contains(&dx) {
                    continue;
                }

                let tile_col = src_x / 8;
                let in_tile_x = src_x & 7;
                let tile_index = if sprite_row_major {
                    // Diagnostic: row-major order.
                    tile_base + tile_row * width_tiles + tile_col
                } else {
                    // Sprite pattern index advances in column-major order on the MD VDP.
                    tile_base + tile_col * height_tiles + tile_row
                };
                let tile_addr = tile_index * tile_stride + in_tile_y * 4 + in_tile_x / 2;
                let tile_byte = {
                    let vram = if self.line_vram_latch_enabled {
                        if sprite_pattern_line0 {
                            self.line_vram.first().unwrap_or(&self.vram)
                        } else {
                            self.line_vram.get(dy_index).unwrap_or(&self.vram)
                        }
                    } else {
                        &self.vram
                    };
                    vram[tile_addr % VRAM_SIZE]
                };
                let pixel = if in_tile_x & 1 == 0 {
                    tile_byte >> 4
                } else {
                    tile_byte & 0x0F
                };
                if pixel == 0 {
                    continue;
                }

                let meta_index = dy as usize * FRAME_WIDTH + dx as usize;
                let meta = plane_meta[meta_index];
                let plane_opaque = (meta & 0x01) != 0;
                let plane_priority_high = (meta & 0x02) != 0;
                if !sprite_priority_high && plane_opaque && plane_priority_high {
                    continue;
                }

                if line_shadow_highlight && palette_line == 3 && (pixel == 14 || pixel == 15) {
                    // S/H control sprites modify brightness of the underlying
                    // plane pixel.  They are transparent — they do NOT occupy
                    // the sprite layer and do NOT trigger collision.
                    let plane_ci = ((meta >> 2) & 0x3F) as usize;
                    let plane_color = self.line_cram[dy_index][plane_ci % CRAM_COLORS];
                    let (pr, pg, pb) = md_color_to_rgb888(plane_color);
                    let out = meta_index * 3;
                    if pixel == 15 {
                        // Shadow control: always shadow the plane color.
                        self.frame_buffer[out] = shadow_channel(pr);
                        self.frame_buffer[out + 1] = shadow_channel(pg);
                        self.frame_buffer[out + 2] = shadow_channel(pb);
                    } else {
                        // Highlight control: shadow→normal, normal→highlight.
                        if !plane_priority_high {
                            // Was shadowed → restore to normal.
                            self.frame_buffer[out] = pr;
                            self.frame_buffer[out + 1] = pg;
                            self.frame_buffer[out + 2] = pb;
                        } else {
                            // Was normal → highlight.
                            self.frame_buffer[out] = highlight_channel(pr);
                            self.frame_buffer[out + 1] = highlight_channel(pg);
                            self.frame_buffer[out + 2] = highlight_channel(pb);
                        }
                    }
                    continue;
                }

                let color_index = palette_line * 16 + pixel as usize;
                let color = self.line_cram[dy_index][color_index % CRAM_COLORS];
                let (r, g, b) = md_color_to_rgb888(color);
                // S/H mode: high-priority sprite → normal,
                // low-priority → shadow, both high → highlight.
                let (r, g, b) = if line_shadow_highlight {
                    if sprite_priority_high && plane_opaque && plane_priority_high {
                        (
                            highlight_channel(r),
                            highlight_channel(g),
                            highlight_channel(b),
                        )
                    } else if sprite_priority_high {
                        (r, g, b)
                    } else {
                        (shadow_channel(r), shadow_channel(g), shadow_channel(b))
                    }
                } else {
                    (r, g, b)
                };
                let out = meta_index * 3;
                if sprite_filled[meta_index] {
                    self.sprite_collision = true;
                    continue;
                }
                self.frame_buffer[out] = r;
                self.frame_buffer[out + 1] = g;
                self.frame_buffer[out + 2] = b;
                sprite_filled[meta_index] = true;
            }
        }
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

    /// Convert a Mode 4 (SMS) CRAM byte to RGB888.
    /// Format: --BBGGRR (2 bits per channel), value range 0-3 mapped to 0/85/170/255.
    fn sms_cram_to_rgb888(cram_byte: u8) -> (u8, u8, u8) {
        let r = (cram_byte & 0x03) * 85;
        let g = ((cram_byte >> 2) & 0x03) * 85;
        let b = ((cram_byte >> 4) & 0x03) * 85;
        (r, g, b)
    }

    /// Decode a Mode 4 tile pixel from planar 4bpp data.
    /// Returns the 4-bit color index for the given pixel column (0=leftmost).
    fn sms_tile_pixel(vram: &[u8; VRAM_SIZE], tile_index: usize, row: usize, col: usize) -> u8 {
        let tile_addr = (tile_index * 32) + (row * 4);
        if tile_addr + 3 >= VRAM_SIZE {
            return 0;
        }
        let bit = 7 - col;
        let b0 = (vram[tile_addr] >> bit) & 1;
        let b1 = (vram[tile_addr + 1] >> bit) & 1;
        let b2 = (vram[tile_addr + 2] >> bit) & 1;
        let b3 = (vram[tile_addr + 3] >> bit) & 1;
        b0 | (b1 << 1) | (b2 << 2) | (b3 << 3)
    }

    /// Render a complete frame in Mode 4 (SMS compatibility mode).
    /// Resolution: 256x192, centered in the 320x240 frame buffer.
    fn render_frame_mode4(&mut self) {
        const MODE4_WIDTH: usize = 256;
        const MODE4_HEIGHT: usize = 192;
        const BORDER_X: usize = (FRAME_WIDTH - MODE4_WIDTH) / 2;
        const BORDER_Y: usize = (FRAME_HEIGHT - MODE4_HEIGHT) / 2;

        let regs = self.registers;

        // Backdrop color: palette 0, color 0 (CRAM index 0, low byte)
        let backdrop_byte = self.cram[0] as u8;
        let (bd_r, bd_g, bd_b) = Self::sms_cram_to_rgb888(backdrop_byte);

        // Nametable base address: reg 2 bits 3-1 * 0x800
        let nt_base = ((regs[2] as usize >> 1) & 0x07) * 0x800;

        // SAT base address: reg 5 bits 6-1 * 0x100
        let sat_base = ((regs[5] as usize >> 1) & 0x3F) * 0x100;

        // Sprite tile base offset: reg 6 bit 2 -> add 256 to tile index
        let sprite_tile_offset: usize = if (regs[6] & 0x04) != 0 { 256 } else { 0 };

        // Sprite size: reg 1 bit 1 -> 8x16 mode
        let sprites_8x16 = (regs[1] & 0x02) != 0;
        let sprite_height: usize = if sprites_8x16 { 16 } else { 8 };

        // Scroll values
        let hscroll_val = regs[8] as usize;
        let vscroll_val = regs[9] as usize;

        // Reg 0 flags
        let mask_left_column = (regs[0] & 0x20) != 0;
        let lock_top_hscroll = (regs[0] & 0x40) != 0;

        // Build sprite list: scan SAT Y table, stop at Y=0xD0 or 64 entries
        struct SpriteEntry {
            y: usize,
            x: usize,
            tile: usize,
        }
        let mut sprites: Vec<SpriteEntry> = Vec::with_capacity(64);
        for i in 0..64 {
            let y_byte = self.vram[(sat_base + i) % VRAM_SIZE];
            if y_byte == 0xD0 {
                break;
            }
            // Y position: sprite appears at line (y_byte + 1)
            let y = y_byte as usize;
            let xn_offset = sat_base + 0x80 + i * 2;
            let x = self.vram[xn_offset % VRAM_SIZE] as usize;
            let tile = self.vram[(xn_offset + 1) % VRAM_SIZE] as usize;
            sprites.push(SpriteEntry { y, x, tile });
        }

        // Fill entire frame buffer with backdrop first
        for i in 0..(FRAME_WIDTH * FRAME_HEIGHT) {
            let off = i * 3;
            self.frame_buffer[off] = bd_r;
            self.frame_buffer[off + 1] = bd_g;
            self.frame_buffer[off + 2] = bd_b;
        }

        // Render the 256x192 active area
        for screen_y in 0..MODE4_HEIGHT {
            // Collect sprites on this scanline (max 8)
            let mut line_sprites: Vec<&SpriteEntry> = Vec::with_capacity(8);
            for spr in &sprites {
                // Sprite Y is +1 offset: y_byte=0 means line 1
                let spr_top = spr.y.wrapping_add(1);
                if screen_y >= spr_top && screen_y < spr_top + sprite_height {
                    line_sprites.push(spr);
                    if line_sprites.len() >= 8 {
                        break;
                    }
                }
            }

            for screen_x in 0..MODE4_WIDTH {
                // --- Background ---
                // Determine effective scroll for this pixel
                let eff_hscroll = if lock_top_hscroll && screen_y < 16 {
                    0
                } else {
                    hscroll_val
                };

                let scrolled_x = (screen_x + MODE4_WIDTH - eff_hscroll) % MODE4_WIDTH;
                let scrolled_y = (screen_y + vscroll_val) % (28 * 8); // 224 pixel wrap

                let tile_col = scrolled_x / 8;
                let tile_row = scrolled_y / 8;
                let pixel_x_in_tile = scrolled_x % 8;
                let pixel_y_in_tile = scrolled_y % 8;

                let nt_addr = nt_base + (tile_row * 32 + tile_col) * 2;
                let nt_lo = self.vram[nt_addr % VRAM_SIZE];
                let nt_hi = self.vram[(nt_addr + 1) % VRAM_SIZE];
                let nt_word = (nt_hi as u16) << 8 | (nt_lo as u16);

                let bg_tile_index = (nt_word & 0x01FF) as usize;
                let bg_hflip = (nt_word & 0x0200) != 0;
                let bg_vflip = (nt_word & 0x0400) != 0;
                let bg_palette = if (nt_word & 0x0800) != 0 { 16 } else { 0 };
                let bg_priority = (nt_word & 0x1000) != 0;

                let eff_px = if bg_hflip {
                    7 - pixel_x_in_tile
                } else {
                    pixel_x_in_tile
                };
                let eff_py = if bg_vflip {
                    7 - pixel_y_in_tile
                } else {
                    pixel_y_in_tile
                };

                let bg_color_idx =
                    Self::sms_tile_pixel(&self.vram, bg_tile_index, eff_py, eff_px) as usize;
                let bg_opaque = bg_color_idx != 0;

                // --- Sprites ---
                let mut spr_color_idx: usize = 0;
                let mut spr_opaque = false;
                for spr in &line_sprites {
                    let spr_top = spr.y.wrapping_add(1);
                    let spr_x = if mask_left_column {
                        // Shift all sprites left by 8
                        spr.x.wrapping_sub(8)
                    } else {
                        spr.x
                    };
                    if screen_x >= spr_x && screen_x < spr_x + 8 {
                        let px = screen_x - spr_x;
                        let py = screen_y - spr_top;
                        let mut tile_idx = spr.tile + sprite_tile_offset;
                        let tile_row_in_spr;
                        if sprites_8x16 {
                            tile_idx &= !1; // Force bit 0 to 0
                            if py >= 8 {
                                tile_idx += 1;
                                tile_row_in_spr = py - 8;
                            } else {
                                tile_row_in_spr = py;
                            }
                        } else {
                            tile_row_in_spr = py;
                        }

                        let c = Self::sms_tile_pixel(&self.vram, tile_idx, tile_row_in_spr, px)
                            as usize;
                        if c != 0 {
                            spr_color_idx = c + 16; // Sprites always use palette 1
                            spr_opaque = true;
                            break;
                        }
                    }
                }

                // --- Priority compositing ---
                let final_color_idx = if bg_priority && bg_opaque {
                    bg_palette + bg_color_idx
                } else if spr_opaque {
                    spr_color_idx
                } else if bg_opaque {
                    bg_palette + bg_color_idx
                } else {
                    0 // backdrop
                };

                // Left column masking
                let masked = mask_left_column && screen_x < 8;

                let (r, g, b) = if masked {
                    (bd_r, bd_g, bd_b)
                } else {
                    let cram_byte = self.cram[final_color_idx % CRAM_COLORS] as u8;
                    Self::sms_cram_to_rgb888(cram_byte)
                };

                let fb_x = BORDER_X + screen_x;
                let fb_y = BORDER_Y + screen_y;
                let off = (fb_y * FRAME_WIDTH + fb_x) * 3;
                self.frame_buffer[off] = r;
                self.frame_buffer[off + 1] = g;
                self.frame_buffer[off + 2] = b;
            }
        }
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
