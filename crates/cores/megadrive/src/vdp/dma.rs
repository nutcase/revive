use super::*;

impl Vdp {
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

    pub(super) fn dma_enabled(&self) -> bool {
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
        if len == 0 { 0x10000 } else { len }
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

    pub(super) fn start_dma(&mut self, base_code: u8) {
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
    pub(super) fn step_dma(&mut self, cycles: u64) {
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
}
