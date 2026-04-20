//! S-DD1 chip emulation (data decompression + extended ROM mapping).
//!
//! Used by Street Fighter Zero 2 / Alpha 2 and Star Ocean.
//! Provides:
//! 1. Extended ROM mapping for banks $C0-$FF (configurable 1 MB pages)
//! 2. On-the-fly DMA decompression (Golomb-Rice coding with context prediction)
//! 3. Registers at $4800-$4807
//!
//! Decompression algorithm reference: bsnes by byuu.

/// S-DD1 chip state.
pub struct Sdd1 {
    /// $4800 (W): which DMA channels have decompression enabled (bitmask)
    pub(crate) dma_enable: u8,
    /// $4801 (W): DMA intercept status (which channels to intercept)
    pub(crate) xfer_enable: u8,

    /// $4804-$4807: bank mapping for $Cx/$Dx/$Ex/$Fx (default 0,1,2,3)
    bank_map: [u8; 4],

    /// Per-channel DMA address/size shadows (snooped from $43x2-$43x6 writes)
    dma_addr: [u32; 8],
    dma_size: [u16; 8],

    /// Decompressor state (active during DMA interception)
    decomp: Option<Sdd1Decomp>,
    dma_ready: bool,
}

impl Sdd1 {
    pub fn new() -> Self {
        Self {
            dma_enable: 0,
            xfer_enable: 0,
            bank_map: [0, 1, 2, 3],
            dma_addr: [0; 8],
            dma_size: [0; 8],
            decomp: None,
            dma_ready: false,
        }
    }

    /// Read S-DD1 register ($4800-$4807).
    pub fn read_register(&self, addr: u16) -> u8 {
        match addr {
            0x4800 => self.dma_enable,
            0x4801 => self.xfer_enable,
            0x4802 | 0x4803 => 0x00,
            0x4804 => self.bank_map[0],
            0x4805 => self.bank_map[1],
            0x4806 => self.bank_map[2],
            0x4807 => self.bank_map[3],
            _ => 0xFF,
        }
    }

    /// Write S-DD1 register ($4800-$4807).
    pub fn write_register(&mut self, addr: u16, value: u8) {
        if crate::debug_flags::trace_sdd1() {
            eprintln!("[SDD1] write ${:04X} = {:02X}", addr, value);
        }
        match addr {
            0x4800 => self.dma_enable = value,
            0x4801 => self.xfer_enable = value,
            0x4802 | 0x4803 => {}
            0x4804 => self.bank_map[0] = value & 0x07,
            0x4805 => self.bank_map[1] = value & 0x07,
            0x4806 => self.bank_map[2] = value & 0x07,
            0x4807 => self.bank_map[3] = value & 0x07,
            _ => {}
        }
    }

    /// Snoop DMA register writes ($43x0-$43xF) to track per-channel addresses/sizes.
    pub fn snoop_dma_write(&mut self, addr: u16, value: u8) {
        let channel = ((addr >> 4) & 7) as usize;
        match addr & 0x0F {
            // $43x2: A1TnL (source address low)
            0x02 => {
                self.dma_addr[channel] = (self.dma_addr[channel] & 0x00FF_FF00) | (value as u32);
            }
            // $43x3: A1TnH (source address high)
            0x03 => {
                self.dma_addr[channel] =
                    (self.dma_addr[channel] & 0x00FF_00FF) | ((value as u32) << 8);
            }
            // $43x4: A1Bn (source bank)
            0x04 => {
                self.dma_addr[channel] =
                    (self.dma_addr[channel] & 0x0000_FFFF) | ((value as u32) << 16);
            }
            // $43x5: DASn low (transfer size low)
            0x05 => {
                self.dma_size[channel] = (self.dma_size[channel] & 0xFF00) | (value as u16);
            }
            // $43x6: DASn high (transfer size high)
            0x06 => {
                self.dma_size[channel] = (self.dma_size[channel] & 0x00FF) | ((value as u16) << 8);
            }
            _ => {}
        }
    }

    /// Check if S-DD1 decompression should intercept a DMA read at `addr`.
    /// If so, returns the decompressed byte. Called from the DMA transfer loop.
    pub fn dma_read(&mut self, addr: u32, rom: &[u8], rom_size: usize) -> Option<u8> {
        if (self.dma_enable & self.xfer_enable) == 0 {
            return None;
        }
        for i in 0..8 {
            let mask = 1u8 << i;
            if (self.dma_enable & self.xfer_enable & mask) != 0 && addr == self.dma_addr[i] {
                if !self.dma_ready {
                    // Resolve ROM address through bank mapping
                    let rom_offset = self.resolve_rom_addr(addr);
                    if crate::debug_flags::trace_sdd1() {
                        eprintln!(
                            "[SDD1] decomp init: ch{} addr=0x{:06X} rom_offset=0x{:06X} size={}",
                            i, addr, rom_offset, self.dma_size[i]
                        );
                    }
                    self.decomp = Some(Sdd1Decomp::new(rom, rom_size, rom_offset));
                    self.dma_ready = true;
                }
                let data = if let Some(ref mut decomp) = self.decomp {
                    decomp.read()
                } else {
                    0
                };
                if self.dma_size[i] > 0 {
                    self.dma_size[i] -= 1;
                }
                if self.dma_size[i] == 0 {
                    self.dma_ready = false;
                    self.decomp = None;
                    self.xfer_enable &= !mask;
                }
                return Some(data);
            }
        }
        None
    }

    /// Resolve a $C0-$FF address to a ROM byte offset using the bank mapping.
    fn resolve_rom_addr(&self, addr: u32) -> usize {
        let bank = ((addr >> 16) & 0xFF) as u8;
        let offset = (addr & 0xFFFF) as u16;
        let page_idx = ((bank >> 4) & 0x03) as usize; // 0=Cx, 1=Dx, 2=Ex, 3=Fx
        let page = self.bank_map[page_idx] as usize;
        let local_bank = (bank & 0x0F) as usize;
        page * 0x10_0000 + local_bank * 0x10000 + offset as usize
    }

    /// Read a byte from banks $C0-$FF using the S-DD1 bank mapping.
    pub fn read_bank_c0_ff(&self, bank: u8, offset: u16, rom: &[u8], rom_size: usize) -> u8 {
        let page_idx = ((bank >> 4) & 0x03) as usize;
        let page = self.bank_map[page_idx] as usize;
        let local_bank = (bank & 0x0F) as usize;
        let rom_addr = page * 0x10_0000 + local_bank * 0x10000 + offset as usize;
        if rom_size == 0 {
            0xFF
        } else {
            rom[rom_addr % rom_size]
        }
    }
}

// =============================================================================
// S-DD1 Decompression Engine
// =============================================================================
// Implements Golomb-Rice coding with adaptive context-based prediction.
// Reference: bsnes decomp.cpp by byuu.

/// Probability Estimation Module (PEM) state entry.
#[derive(Clone, Copy)]
struct PemState {
    code_number: u8,
    next_if_mps: u8,
    next_if_lps: u8,
}

/// 33-state evolution table for PEM.
static EVOLUTION_TABLE: [PemState; 33] = [
    PemState {
        code_number: 0,
        next_if_mps: 25,
        next_if_lps: 25,
    },
    PemState {
        code_number: 0,
        next_if_mps: 2,
        next_if_lps: 1,
    },
    PemState {
        code_number: 0,
        next_if_mps: 3,
        next_if_lps: 1,
    },
    PemState {
        code_number: 0,
        next_if_mps: 4,
        next_if_lps: 2,
    },
    PemState {
        code_number: 0,
        next_if_mps: 5,
        next_if_lps: 3,
    },
    PemState {
        code_number: 1,
        next_if_mps: 6,
        next_if_lps: 4,
    },
    PemState {
        code_number: 1,
        next_if_mps: 7,
        next_if_lps: 5,
    },
    PemState {
        code_number: 1,
        next_if_mps: 8,
        next_if_lps: 6,
    },
    PemState {
        code_number: 1,
        next_if_mps: 9,
        next_if_lps: 7,
    },
    PemState {
        code_number: 2,
        next_if_mps: 10,
        next_if_lps: 8,
    },
    PemState {
        code_number: 2,
        next_if_mps: 11,
        next_if_lps: 9,
    },
    PemState {
        code_number: 2,
        next_if_mps: 12,
        next_if_lps: 10,
    },
    PemState {
        code_number: 2,
        next_if_mps: 13,
        next_if_lps: 11,
    },
    PemState {
        code_number: 3,
        next_if_mps: 14,
        next_if_lps: 12,
    },
    PemState {
        code_number: 3,
        next_if_mps: 15,
        next_if_lps: 13,
    },
    PemState {
        code_number: 3,
        next_if_mps: 16,
        next_if_lps: 14,
    },
    PemState {
        code_number: 3,
        next_if_mps: 17,
        next_if_lps: 15,
    },
    PemState {
        code_number: 4,
        next_if_mps: 18,
        next_if_lps: 16,
    },
    PemState {
        code_number: 4,
        next_if_mps: 19,
        next_if_lps: 17,
    },
    PemState {
        code_number: 5,
        next_if_mps: 20,
        next_if_lps: 18,
    },
    PemState {
        code_number: 5,
        next_if_mps: 21,
        next_if_lps: 19,
    },
    PemState {
        code_number: 6,
        next_if_mps: 22,
        next_if_lps: 20,
    },
    PemState {
        code_number: 6,
        next_if_mps: 23,
        next_if_lps: 21,
    },
    PemState {
        code_number: 7,
        next_if_mps: 24,
        next_if_lps: 22,
    },
    PemState {
        code_number: 7,
        next_if_mps: 24,
        next_if_lps: 23,
    },
    PemState {
        code_number: 0,
        next_if_mps: 26,
        next_if_lps: 1,
    },
    PemState {
        code_number: 1,
        next_if_mps: 27,
        next_if_lps: 2,
    },
    PemState {
        code_number: 2,
        next_if_mps: 28,
        next_if_lps: 4,
    },
    PemState {
        code_number: 3,
        next_if_mps: 29,
        next_if_lps: 8,
    },
    PemState {
        code_number: 4,
        next_if_mps: 30,
        next_if_lps: 12,
    },
    PemState {
        code_number: 5,
        next_if_mps: 31,
        next_if_lps: 16,
    },
    PemState {
        code_number: 6,
        next_if_mps: 32,
        next_if_lps: 18,
    },
    PemState {
        code_number: 7,
        next_if_mps: 24,
        next_if_lps: 22,
    },
];

/// Golomb-Rice run_count lookup table (256 entries).
/// For a given codeword byte, gives the MPS run count (bit-reversed decoding).
#[rustfmt::skip]
static RUN_COUNT: [u8; 256] = [
    0x00, 0x00, 0x01, 0x00, 0x03, 0x01, 0x02, 0x00,
    0x07, 0x03, 0x05, 0x01, 0x06, 0x02, 0x04, 0x00,
    0x0f, 0x07, 0x0b, 0x03, 0x0d, 0x05, 0x09, 0x01,
    0x0e, 0x06, 0x0a, 0x02, 0x0c, 0x04, 0x08, 0x00,
    0x1f, 0x0f, 0x17, 0x07, 0x1b, 0x0b, 0x13, 0x03,
    0x1d, 0x0d, 0x15, 0x05, 0x19, 0x09, 0x11, 0x01,
    0x1e, 0x0e, 0x16, 0x06, 0x1a, 0x0a, 0x12, 0x02,
    0x1c, 0x0c, 0x14, 0x04, 0x18, 0x08, 0x10, 0x00,
    0x3f, 0x1f, 0x2f, 0x0f, 0x37, 0x17, 0x27, 0x07,
    0x3b, 0x1b, 0x2b, 0x0b, 0x33, 0x13, 0x23, 0x03,
    0x3d, 0x1d, 0x2d, 0x0d, 0x35, 0x15, 0x25, 0x05,
    0x39, 0x19, 0x29, 0x09, 0x31, 0x11, 0x21, 0x01,
    0x3e, 0x1e, 0x2e, 0x0e, 0x36, 0x16, 0x26, 0x06,
    0x3a, 0x1a, 0x2a, 0x0a, 0x32, 0x12, 0x22, 0x02,
    0x3c, 0x1c, 0x2c, 0x0c, 0x34, 0x14, 0x24, 0x04,
    0x38, 0x18, 0x28, 0x08, 0x30, 0x10, 0x20, 0x00,
    0x7f, 0x3f, 0x5f, 0x1f, 0x6f, 0x2f, 0x4f, 0x0f,
    0x77, 0x37, 0x57, 0x17, 0x67, 0x27, 0x47, 0x07,
    0x7b, 0x3b, 0x5b, 0x1b, 0x6b, 0x2b, 0x4b, 0x0b,
    0x73, 0x33, 0x53, 0x13, 0x63, 0x23, 0x43, 0x03,
    0x7d, 0x3d, 0x5d, 0x1d, 0x6d, 0x2d, 0x4d, 0x0d,
    0x75, 0x35, 0x55, 0x15, 0x65, 0x25, 0x45, 0x05,
    0x79, 0x39, 0x59, 0x19, 0x69, 0x29, 0x49, 0x09,
    0x71, 0x31, 0x51, 0x11, 0x61, 0x21, 0x41, 0x01,
    0x7e, 0x3e, 0x5e, 0x1e, 0x6e, 0x2e, 0x4e, 0x0e,
    0x76, 0x36, 0x56, 0x16, 0x66, 0x26, 0x46, 0x06,
    0x7a, 0x3a, 0x5a, 0x1a, 0x6a, 0x2a, 0x4a, 0x0a,
    0x72, 0x32, 0x52, 0x12, 0x62, 0x22, 0x42, 0x02,
    0x7c, 0x3c, 0x5c, 0x1c, 0x6c, 0x2c, 0x4c, 0x0c,
    0x74, 0x34, 0x54, 0x14, 0x64, 0x24, 0x44, 0x04,
    0x78, 0x38, 0x58, 0x18, 0x68, 0x28, 0x48, 0x08,
    0x70, 0x30, 0x50, 0x10, 0x60, 0x20, 0x40, 0x00,
];

/// Complete S-DD1 decompressor state.
pub struct Sdd1Decomp {
    // ROM access
    rom_ptr: *const u8,
    rom_size: usize,

    // Input Manager (IM)
    im_offset: usize,
    im_bit_count: u8,

    // Bits Generators (BG) — 8 instances, one per code_number
    bg_mps_count: [u8; 8],
    bg_lps_index: [bool; 8],

    // Probability Estimation Module (PEM) — 32 contexts
    pem_status: [u8; 32],
    pem_mps: [u8; 32],

    // Context Model (CM)
    cm_bitplanes_info: u8,
    cm_context_bits_info: u8,
    cm_bit_number: u16,
    cm_current_bitplane: u8,
    cm_previous_bitplane_bits: [u16; 8],

    // Output Logic (OL)
    ol_bitplanes_info: u8,
    ol_r0: u8,
    ol_r1: u8,
    ol_r2: u8,
}

impl Sdd1Decomp {
    pub fn new(rom: &[u8], rom_size: usize, offset: usize) -> Self {
        let mut d = Self {
            rom_ptr: rom.as_ptr(),
            rom_size,
            im_offset: 0,
            im_bit_count: 0,
            bg_mps_count: [0; 8],
            bg_lps_index: [false; 8],
            pem_status: [0; 32],
            pem_mps: [0; 32],
            cm_bitplanes_info: 0,
            cm_context_bits_info: 0,
            cm_bit_number: 0,
            cm_current_bitplane: 0,
            cm_previous_bitplane_bits: [0; 8],
            ol_bitplanes_info: 0,
            ol_r0: 0,
            ol_r1: 0,
            ol_r2: 0,
        };
        d.init(offset);
        d
    }

    fn init(&mut self, offset: usize) {
        // IM init
        self.im_offset = offset;
        self.im_bit_count = 4;

        // BG init (all 8)
        for i in 0..8 {
            self.bg_mps_count[i] = 0;
            self.bg_lps_index[i] = false;
        }

        // PEM init
        for i in 0..32 {
            self.pem_status[i] = 0;
            self.pem_mps[i] = 0;
        }

        // CM init — read header byte from ROM
        let header = self.rom_read(offset);
        self.cm_bitplanes_info = header & 0xC0;
        self.cm_context_bits_info = header & 0x30;
        self.cm_bit_number = 0;
        for i in 0..8 {
            self.cm_previous_bitplane_bits[i] = 0;
        }
        self.cm_current_bitplane = match self.cm_bitplanes_info {
            0x00 => 1,
            0x40 => 7,
            0x80 => 3,
            _ => 0, // 0xC0 (mode 7 / 1bpp)
        };

        // OL init
        self.ol_bitplanes_info = header & 0xC0;
        self.ol_r0 = 0x01;
    }

    /// Read next decompressed byte.
    pub fn read(&mut self) -> u8 {
        self.ol_decompress()
    }

    fn rom_read(&self, offset: usize) -> u8 {
        if self.rom_size == 0 {
            return 0;
        }
        let idx = offset % self.rom_size;
        // SAFETY: rom_ptr is valid for the lifetime of the decompressor,
        // guaranteed by the caller holding a reference to rom.
        unsafe { *self.rom_ptr.add(idx) }
    }

    // =========================================================================
    // Input Manager (IM)
    // =========================================================================

    fn im_get_codeword(&mut self, code_length: u8) -> u8 {
        // C++ promotes u8 to int for shifts; replicate by using u32 intermediates
        // to avoid Rust's wrapping_shl/shr modular behavior on u8.
        let mut codeword =
            ((self.rom_read(self.im_offset) as u32) << (self.im_bit_count as u32)) as u8;
        self.im_bit_count = self.im_bit_count.wrapping_add(1);
        if (codeword & 0x80) != 0 {
            let shift = 9u32 - self.im_bit_count as u32;
            codeword |= ((self.rom_read(self.im_offset + 1) as u32) >> shift) as u8;
            self.im_bit_count = self.im_bit_count.wrapping_add(code_length);
        }
        if (self.im_bit_count & 0x08) != 0 {
            self.im_offset += 1;
            self.im_bit_count &= 0x07;
        }
        codeword
    }

    // =========================================================================
    // Golomb-Code Decoder (GCD)
    // =========================================================================

    fn gcd_get_run_count(&mut self, code_number: u8) -> (u8, bool) {
        let codeword = self.im_get_codeword(code_number);
        if (codeword & 0x80) != 0 {
            let idx = (codeword >> (code_number ^ 0x07)) as usize;
            let mps_count = RUN_COUNT[idx & 0xFF];
            (mps_count, true)
        } else {
            let mps_count = 1u8 << code_number;
            (mps_count, false)
        }
    }

    // =========================================================================
    // Bits Generator (BG)
    // =========================================================================

    /// Returns (bit, end_of_run).
    fn bg_get_bit(&mut self, code_number: usize) -> (u8, bool) {
        if self.bg_mps_count[code_number] == 0 && !self.bg_lps_index[code_number] {
            let (mps, lps) = self.gcd_get_run_count(code_number as u8);
            self.bg_mps_count[code_number] = mps;
            self.bg_lps_index[code_number] = lps;
        }
        let bit;
        if self.bg_mps_count[code_number] != 0 {
            bit = 0;
            self.bg_mps_count[code_number] -= 1;
        } else {
            bit = 1;
            self.bg_lps_index[code_number] = false;
        }
        let end_of_run = self.bg_mps_count[code_number] == 0 && !self.bg_lps_index[code_number];
        (bit, end_of_run)
    }

    // =========================================================================
    // Probability Estimation Module (PEM)
    // =========================================================================

    fn pem_get_bit(&mut self, context: u8) -> u8 {
        let ctx = (context & 0x1F) as usize;
        let state_idx = self.pem_status[ctx] as usize;
        let state = &EVOLUTION_TABLE[state_idx];
        let code_number = state.code_number as usize;
        let current_mps = self.pem_mps[ctx];

        let (raw_bit, end_of_run) = self.bg_get_bit(code_number);

        if end_of_run {
            if raw_bit == 0 {
                // MPS consumed — move toward higher code_number
                self.pem_status[ctx] = state.next_if_mps;
            } else {
                // LPS consumed — move toward lower code_number, possibly toggle MPS
                self.pem_status[ctx] = state.next_if_lps;
                if state_idx < 2 {
                    self.pem_mps[ctx] ^= 0x01;
                }
            }
        }

        raw_bit ^ current_mps
    }

    // =========================================================================
    // Context Model (CM)
    // =========================================================================

    /// Context Model: exact port of bsnes CM::get_bit().
    /// Bitplane selection happens FIRST, then context computation, then PEM call.
    fn cm_get_bit(&mut self) -> u8 {
        // Step 1: Advance bitplane selection (bsnes does this at the top of get_bit)
        match self.cm_bitplanes_info {
            0x00 => {
                // 2bpp: alternate between bitplanes 0 and 1
                self.cm_current_bitplane ^= 0x01;
            }
            0x40 => {
                // 8bpp: alternate pair, advance every 128 bits
                self.cm_current_bitplane ^= 0x01;
                if (self.cm_bit_number & 0x7F) == 0 {
                    self.cm_current_bitplane = (self.cm_current_bitplane.wrapping_add(2)) & 0x07;
                }
            }
            0x80 => {
                // 4bpp: alternate pair, advance every 128 bits
                self.cm_current_bitplane ^= 0x01;
                if (self.cm_bit_number & 0x7F) == 0 {
                    self.cm_current_bitplane ^= 0x02;
                }
            }
            _ => {
                // 0xC0: 1bpp / Mode 7: cycle through 0-7
                self.cm_current_bitplane = (self.cm_bit_number & 0x07) as u8;
            }
        }

        let cur_bp = self.cm_current_bitplane as usize;
        let context_bits = self.cm_previous_bitplane_bits[cur_bp];

        // Step 2: Compute context — bitplane parity at bit 4, history bits below
        let mut current_context = (self.cm_current_bitplane & 0x01) << 4;
        match self.cm_context_bits_info {
            0x00 => {
                current_context |=
                    ((context_bits & 0x01C0) >> 5) as u8 | (context_bits & 0x0001) as u8;
            }
            0x10 => {
                current_context |=
                    ((context_bits & 0x0180) >> 5) as u8 | (context_bits & 0x0001) as u8;
            }
            0x20 => {
                current_context |=
                    ((context_bits & 0x00C0) >> 5) as u8 | (context_bits & 0x0001) as u8;
            }
            _ => {
                // 0x30
                current_context |=
                    ((context_bits & 0x0180) >> 5) as u8 | (context_bits & 0x0003) as u8;
            }
        }

        // Step 3: Get bit from PEM
        let bit = self.pem_get_bit(current_context);

        // Step 4: Update shift register for this bitplane
        self.cm_previous_bitplane_bits[cur_bp] =
            (self.cm_previous_bitplane_bits[cur_bp] << 1) | (bit as u16);

        // Step 5: Advance bit counter
        self.cm_bit_number = self.cm_bit_number.wrapping_add(1);

        bit
    }

    // =========================================================================
    // Output Logic (OL)
    // =========================================================================

    fn ol_decompress(&mut self) -> u8 {
        match self.ol_bitplanes_info {
            0x00 | 0x40 | 0x80 => {
                // 2bpp / 8bpp / 4bpp: interleaved bitplane pairs
                if self.ol_r0 == 0 {
                    self.ol_r0 = !self.ol_r0; // set to 0xFF (non-zero)
                    return self.ol_r2;
                }
                self.ol_r0 = 0x80;
                self.ol_r1 = 0;
                self.ol_r2 = 0;
                while self.ol_r0 != 0 {
                    if self.cm_get_bit() != 0 {
                        self.ol_r1 |= self.ol_r0;
                    }
                    if self.cm_get_bit() != 0 {
                        self.ol_r2 |= self.ol_r0;
                    }
                    self.ol_r0 >>= 1;
                }
                self.ol_r0 = 0; // signal to return r2 on next call
                self.ol_r1
            }
            _ => {
                // 0xC0: 1bpp / Mode 7 — single bitplane
                self.ol_r0 = 0x01;
                self.ol_r1 = 0;
                while self.ol_r0 != 0 {
                    if self.cm_get_bit() != 0 {
                        self.ol_r1 |= self.ol_r0;
                    }
                    self.ol_r0 <<= 1;
                }
                self.ol_r1
            }
        }
    }
}
