use super::*;

impl Vdp {
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

    pub fn read_hv_counter(&self) -> u16 {
        let (line, line_start, line_end) = self.line_cycle_bounds_for_cycle(self.frame_cycles);
        let line_cycles = line_end.saturating_sub(line_start).max(1);
        let cycle_in_line = self.frame_cycles.saturating_sub(line_start);
        let v = self.v_counter_value_for_line(line);
        let h = self.h_counter_value(cycle_in_line, line_cycles);
        u16::from_be_bytes([v, h])
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

    pub(crate) fn trigger_dma_fill_from_data_byte(&mut self, value: u8) -> bool {
        if self.dma_fill_pending.is_none() {
            return false;
        }

        self.write_data_port(u16::from_be_bytes([value, value]));
        true
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

    pub(super) fn advance_access_addr(&mut self) {
        let increment = self.auto_increment();
        self.access_addr = self.access_addr.wrapping_add(increment);
    }

    pub(super) fn auto_increment(&self) -> u16 {
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
}
