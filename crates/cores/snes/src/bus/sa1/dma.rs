use crate::cartridge::sa1::Sa1;

use crate::bus::Bus;

impl Bus {
    pub fn process_sa1_dma(&mut self) {
        if !self.is_sa1_active() {
            return;
        }

        // Check for pending normal DMA
        if self.sa1.is_dma_pending() {
            if crate::debug_flags::trace_sa1_dma() {
                println!(
                    "SA1_DMA: Normal DMA pending src=0x{:06X} dest=0x{:06X} len=0x{:04X}",
                    self.sa1.registers.dma_source,
                    self.sa1.registers.dma_dest,
                    self.sa1.registers.dma_length
                );
            }
            self.run_sa1_normal_dma_copy();
            let irq_fired = {
                let sa1 = &mut self.sa1;
                sa1.complete_dma()
            };
            if irq_fired && crate::debug_flags::trace_sa1_dma() {
                println!("SA1_DMA: Normal DMA complete, IRQ fired to S-CPU");
            }
        }

        // Check for pending CC-DMA
        if self.sa1.is_ccdma_pending() || self.sa1.registers.ccdma_buffer_ready {
            let typ = if self.sa1.registers.ccdma_buffer_ready && self.sa1.registers.brf_pos >= 16 {
                2
            } else {
                self.sa1.ccdma_type().unwrap_or(0)
            };
            if crate::debug_flags::trace_sa1_ccdma() {
                self.sa1.log_ccdma_state("process_begin");
                self.sa1.trace_ccdma_transfer("begin");
                println!(
                    "SA1_CCDMA: Processing CC-DMA src=0x{:06X} dest=0x{:06X} len=0x{:04X}",
                    self.sa1.registers.dma_source,
                    self.sa1.registers.dma_dest,
                    self.sa1.registers.dma_length
                );
            }

            if !self.perform_sa1_ccdma() {
                return;
            }

            if typ == 1 {
                self.sa1.registers.ccdma_pending = false;
                self.sa1.registers.ccdma_buffer_ready = false;
                self.sa1.registers.handshake_state = 2;
                if crate::debug_flags::trace_sa1_ccdma() {
                    self.sa1.log_ccdma_state("process_wait_end");
                    println!("SA1_CCDMA: Type1 conversion complete, waiting for $2231 bit7");
                }
                return;
            }

            let irq_fired = self.sa1.complete_ccdma();
            if crate::debug_flags::trace_sa1_ccdma() {
                self.sa1.log_ccdma_state("process_complete");
                self.sa1.trace_ccdma_transfer("complete");
                if irq_fired {
                    println!("SA1_CCDMA: CC-DMA complete, IRQ fired to S-CPU");
                } else {
                    println!("SA1_CCDMA: CC-DMA complete, no IRQ");
                }
            }
        }
    }

    /// Perform SA-1 character conversion DMA (type 0/1): linear pixels -> SNES bitplane tiles.
    fn perform_sa1_ccdma(&mut self) -> bool {
        let src = self.sa1.registers.dma_source;
        let dest = self.sa1.registers.dma_dest;
        let len = self.sa1.registers.dma_length as usize;

        let typ = if self.sa1.registers.ccdma_buffer_ready && self.sa1.registers.brf_pos >= 16 {
            2
        } else {
            self.sa1.ccdma_type().unwrap_or(0)
        };
        if len == 0 && typ != 2 {
            return false;
        }
        let dcnt = self.sa1.registers.dma_control;
        let src_dev = dcnt & 0x03;
        let dst_dev = (dcnt >> 2) & 0x01;

        let depth_bits = self.sa1.ccdma_color_depth_bits().unwrap_or(4);

        let bytes_per_tile_src = match depth_bits {
            8 => 64,
            4 => 32,
            2 => 16,
            _ => 64,
        };
        let bytes_per_tile_dst = (depth_bits / 2) * 16;

        if typ == 2 {
            let tile_dst = dest.wrapping_add(self.sa1.registers.brf_tile_offset);
            let bytes_per_row_out = depth_bits as u32 * 2;
            for row in 0..2 {
                let start = row * 8;
                let mut line = [0u8; 8];
                line.copy_from_slice(&self.sa1.registers.brf[start..start + 8]);
                let row_off = (row as u32) * bytes_per_row_out;
                let mask = (1u8 << depth_bits) - 1;
                let mut out_idx = row_off;
                for plane in (0..depth_bits).step_by(2) {
                    let mut byte_lo = 0u8;
                    let mut byte_hi = 0u8;
                    for (x, &pixel) in line.iter().enumerate() {
                        let val = pixel & mask;
                        byte_lo |= ((val >> plane) & 1) << (7 - x);
                        byte_hi |= ((val >> (plane + 1)) & 1) << (7 - x);
                    }
                    self.sa1_write_u8(tile_dst.wrapping_add(out_idx), byte_lo);
                    self.sa1_write_u8(tile_dst.wrapping_add(out_idx + 1), byte_hi);
                    out_idx += 2;
                }
            }
            let adv = bytes_per_tile_dst as u32;
            self.sa1.registers.brf_tile_offset =
                self.sa1.registers.brf_tile_offset.wrapping_add(adv);

            self.sa1.registers.brf.fill(0);
            self.sa1.registers.brf_pos = 0;
            self.sa1.registers.ccdma_pending = false;
            self.sa1.registers.ccdma_buffer_ready = false;
            if crate::debug_flags::trace_sa1_ccdma() {
                println!(
                    "SA1_CCDMA(type2 convert depth={}bpp) dest=0x{:06X} brf_off=0x{:06X}",
                    depth_bits, tile_dst, self.sa1.registers.brf_tile_offset
                );
            }
            return true;
        } else {
            let tiles = len / bytes_per_tile_src;
            if tiles == 0 {
                return false;
            }
            let width_tiles = 1usize << (self.sa1.ccdma_virtual_width_shift().min(5) as usize);
            let width_pixels = width_tiles * 8;
            let pixels_per_byte = match depth_bits {
                8 => 1usize,
                4 => 2usize,
                2 => 4usize,
                _ => 1usize,
            };
            let bytes_per_line = width_pixels / pixels_per_byte;
            let mask = if depth_bits >= 8 {
                0xFF
            } else {
                (1u8 << depth_bits) - 1
            };
            let plane_pairs = (depth_bits / 2) as usize;
            let mut pix = [0u8; 64];
            let mut out = vec![0u8; bytes_per_tile_dst as usize];

            for t in 0..tiles {
                let tile_x = t % width_tiles;
                let tile_y = t / width_tiles;
                let tile_dst = dest.wrapping_add((t * bytes_per_tile_dst as usize) as u32);

                for py in 0..8usize {
                    let row = tile_y * 8 + py;
                    let row_start = row * bytes_per_line;
                    for px in 0..8usize {
                        let col = tile_x * 8 + px;
                        let byte_index = row_start + (col / pixels_per_byte);
                        let shift = ((col % pixels_per_byte) * depth_bits as usize) as u8;
                        let b =
                            self.sa1_dma_read_device(src_dev, src.wrapping_add(byte_index as u32));
                        pix[py * 8 + px] = (b >> shift) & mask;
                    }
                }

                out.fill(0);
                for y in 0..8usize {
                    let mut planes = [0u8; 8];
                    for x in 0..8usize {
                        let val = pix[y * 8 + x];
                        let bit = 7 - x;
                        for p in 0..depth_bits {
                            if ((val >> p) & 1) != 0 {
                                planes[p as usize] |= 1u8 << bit;
                            }
                        }
                    }
                    for pair in 0..plane_pairs {
                        let base = pair * 16;
                        out[base + y * 2] = planes[pair * 2];
                        out[base + y * 2 + 1] = planes[pair * 2 + 1];
                    }
                }

                for (i, &byte) in out.iter().enumerate() {
                    self.sa1_dma_write_device(dst_dev, tile_dst.wrapping_add(i as u32), byte);
                }
            }

            if crate::debug_flags::trace_sa1_ccdma() {
                println!(
                    "SA1_CCDMA(type{} convert depth={}bpp) tiles={} width_tiles={} src=0x{:06X} dest=0x{:06X}",
                    typ, depth_bits, tiles, width_tiles, src, dest
                );
            }
        }
        true
    }

    fn run_sa1_normal_dma_copy(&mut self) {
        if !self.sa1.dma_is_normal_public() || !self.sa1.registers.dma_pending {
            return;
        }
        let len = self.sa1.registers.dma_length as usize;
        if len == 0 {
            self.sa1.registers.dma_pending = false;
            return;
        }
        let dcnt = self.sa1.registers.dma_control;
        let src_dev = dcnt & 0x03;
        let dst_dev = (dcnt >> 2) & 0x01;

        let src_addr = self.sa1.registers.dma_source as usize;
        let dst_addr = self.sa1.registers.dma_dest as usize;

        let read_src = |bus: &mut Bus, idx: usize| -> u8 {
            match src_dev {
                0 => {
                    let addr = src_addr + idx;
                    let bank = (addr >> 16) as u32 & 0xFF;
                    let off = addr as u16;
                    let phys = bus.sa1_phys_addr(bank, off);
                    bus.rom.get(phys % bus.rom_size).copied().unwrap_or(0xFF)
                }
                1 => {
                    if bus.sa1_bwram.is_empty() {
                        0xFF
                    } else {
                        let bank = (src_addr >> 16) & 0xFF;
                        let off = src_addr & 0xFFFF;
                        let base = ((bank & 0x1F) << 16) | off;
                        let di = (base + idx) % bus.sa1_bwram.len();
                        bus.sa1_bwram[di]
                    }
                }
                2 => {
                    let base = src_addr & 0x7FF;
                    bus.sa1_iram[(base + idx) % bus.sa1_iram.len()]
                }
                _ => 0xFF,
            }
        };

        let write_dst = |bus: &mut Bus, idx: usize, val: u8| match dst_dev {
            0 => {
                let base = dst_addr & 0x7FF;
                let di = (base + idx) % bus.sa1_iram.len();
                if bus.iram_write_allowed_sa1(di as u16) {
                    bus.sa1_iram[di] = val;
                }
            }
            1 => {
                if !bus.sa1_bwram.is_empty() {
                    let bank = (dst_addr >> 16) & 0xFF;
                    let off = dst_addr & 0xFFFF;
                    let base = ((bank & 0x1F) << 16) | off;
                    let di = (base + idx) % bus.sa1_bwram.len();
                    if bus.bwram_write_allowed_sa1(di) {
                        bus.sa1_bwram[di] = val;
                    }
                }
            }
            _ => {}
        };

        for i in 0..len {
            let v = read_src(self, i);
            write_dst(self, i, v);
        }
        self.sa1.registers.dma_pending = false;
        self.sa1.registers.dma_control &= !0x80;
        self.sa1.registers.interrupt_pending |= Sa1::IRQ_DMA_FLAG;
        if (self.sa1.registers.interrupt_enable & Sa1::IRQ_LINE_BIT) != 0 {
            self.sa1.registers.interrupt_pending |= Sa1::IRQ_LINE_BIT;
        }
    }

    #[inline]
    fn sa1_dma_read_device(&self, dev: u8, addr: u32) -> u8 {
        match dev {
            0 => {
                let phys = self.sa1_phys_addr((addr >> 16) & 0xFF, addr as u16);
                self.rom.get(phys % self.rom_size).copied().unwrap_or(0xFF)
            }
            1 => {
                if self.sa1_bwram.is_empty() {
                    0xFF
                } else {
                    let idx = addr as usize % self.sa1_bwram.len();
                    self.sa1_bwram[idx]
                }
            }
            2 => {
                let off = (addr & 0x1FFF) as usize;
                self.sa1_iram
                    .get(off % self.sa1_iram.len())
                    .copied()
                    .unwrap_or(0xFF)
            }
            _ => 0xFF,
        }
    }

    #[inline]
    fn sa1_dma_write_device(&mut self, dev: u8, addr: u32, val: u8) {
        match dev {
            0 => {
                let off = (addr & 0x1FFF) as usize;
                if off < self.sa1_iram.len() && self.iram_write_allowed_sa1(off as u16) {
                    self.sa1_iram[off] = val;
                }
            }
            1 => {
                if !self.sa1_bwram.is_empty() {
                    let idx = addr as usize % self.sa1_bwram.len();
                    if self.bwram_write_allowed_sa1(idx) {
                        self.sa1_bwram[idx] = val;
                    }
                }
            }
            _ => {}
        }
    }
}
