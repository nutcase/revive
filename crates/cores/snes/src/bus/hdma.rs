use super::Bus;

impl Bus {
    pub(super) fn enable_hdma_channels_now(&mut self, mask: u8) {
        let at_frame_head = self.ppu.scanline == 0 && !self.ppu.is_hblank();
        for i in 0..8usize {
            if (mask & (1 << i)) == 0 {
                continue;
            }

            let ch = &self.dma_controller.channels[i];
            if !ch.configured {
                continue;
            }

            let should_initialize = at_frame_head && !ch.hdma_initialized_this_frame;
            if ch.hdma_terminated && !should_initialize {
                continue;
            }

            if should_initialize {
                if crate::debug_flags::trace_hdma_all() {
                    println!(
                        "[HDMA-LATE-INIT] frame={} sl={} cyc={} ch{} src=0x{:06X}",
                        self.ppu.get_frame(),
                        self.ppu.scanline,
                        self.ppu.get_cycle(),
                        i,
                        ch.src_address
                    );
                }
                self.initialize_hdma_channel_for_frame(i);
                continue;
            }

            // $420C rising edges are live, but they must not reinitialize the HDMA
            // table pointer or line counter. Full table initialization happens at
            // frame start; this only resumes an already configured channel.
            let ch = &mut self.dma_controller.channels[i];
            ch.hdma_enabled = true;
            ch.hdma_terminated = false;
            ch.hdma_indirect = (ch.control & 0x40) != 0;
        }
    }

    fn initialize_hdma_channel_for_frame(&mut self, channel: usize) {
        let ch = &mut self.dma_controller.channels[channel];
        ch.hdma_enabled = true;
        ch.hdma_terminated = false;
        ch.hdma_initialized_this_frame = true;
        ch.hdma_line_counter = 0;
        ch.hdma_repeat_flag = false;
        ch.hdma_do_transfer = false;
        ch.hdma_indirect = (ch.control & 0x40) != 0;
        ch.hdma_indirect_addr = 0;
        ch.hdma_latched = [0; 4];
        ch.hdma_latched_len = 0;
        ch.hdma_table_addr = ch.src_address;
        // Mirror into readable HDMA state registers.
        ch.a2a = (ch.src_address & 0xFFFF) as u16;
        ch.nltr = 0x80; // reload flag set; counter will be loaded from the table
    }

    // Called when the PPU scanline counter wraps to 0 (start of a new frame).
    //
    // Hardware behavior: HDMA channels are re-initialized every frame. The table start
    // address (A1T/A1B) is copied into the current table address (A2A), and per-channel
    // processing resumes even if the channel terminated earlier due to a $00 line-count.
    pub fn on_frame_start(&mut self) {
        let mask = self.dma_controller.hdma_enable;
        if crate::debug_flags::trace_hdma_all() && mask != 0 {
            println!(
                "[HDMA-INIT] frame={} hdma_enable=0x{:02X}",
                self.ppu.get_frame(),
                mask
            );
        }
        for ch in &mut self.dma_controller.channels {
            ch.hdma_enabled = false;
            ch.hdma_terminated = false;
            ch.hdma_initialized_this_frame = false;
        }
        for i in 0..8usize {
            if (mask & (1 << i)) == 0 {
                continue;
            }
            let ch = &mut self.dma_controller.channels[i];
            // If a channel wasn't configured, leave it alone (prevents wandering reads).
            if !ch.configured {
                continue;
            }
            if crate::debug_flags::trace_hdma_all() {
                let unit = ch.control & 0x07;
                let indirect = (ch.control & 0x40) != 0;
                println!(
                    "[HDMA-INIT]   ch{} ctrl=0x{:02X} unit={} indirect={} dest=$21{:02X} src=0x{:06X} dasb=0x{:02X}",
                    i, ch.control, unit, indirect, ch.dest_address, ch.src_address, ch.dasb
                );
            }
            self.initialize_hdma_channel_for_frame(i);
        }
    }

    pub fn hdma_scanline(&mut self) {
        // HDMAチャンネルのスキャンライン処理を実行
        for i in 0..8 {
            if !self.dma_controller.channels[i].hdma_enabled
                || self.dma_controller.channels[i].hdma_terminated
            {
                continue;
            }

            // 行カウンタが0なら新しいエントリをロード
            if self.dma_controller.channels[i].hdma_line_counter == 0 && !self.load_hdma_entry(i) {
                self.dma_controller.channels[i].hdma_terminated = true;
                continue;
            }

            // HDMA転送実行
            //
            // SNES HDMA line-counter semantics:
            // - repeat=0: transfer once, then pause for (count-1) scanlines (register value holds)
            // - repeat=1: transfer every scanline for `count` scanlines, consuming new data each line
            let do_transfer = self.dma_controller.channels[i].hdma_do_transfer;
            if do_transfer {
                self.perform_hdma_transfer(i);
                if !self.dma_controller.channels[i].hdma_repeat_flag {
                    self.dma_controller.channels[i].hdma_do_transfer = false;
                }
            }

            // 行カウンタをデクリメント
            let new_count = self.dma_controller.channels[i]
                .hdma_line_counter
                .saturating_sub(1);
            self.dma_controller.channels[i].hdma_line_counter = new_count;
            Self::mirror_hdma_nltr(&mut self.dma_controller.channels[i]);
            if new_count == 0 {
                // Next scanline will load a new entry (which re-enables do_transfer as appropriate).
                self.dma_controller.channels[i].hdma_do_transfer = false;
            } else if self.dma_controller.channels[i].hdma_repeat_flag {
                // repeat=1 transfers on every scanline while the counter is nonzero.
                self.dma_controller.channels[i].hdma_do_transfer = true;
            }
        }
    }

    // H-Blank開始タイミングで呼ばれる想定のHDMA処理
    pub fn hdma_hblank(&mut self) {
        // 実機はH-Blankの頭でHDMAを行う。ここではhdma_scanlineと同等処理を呼ぶ。
        self.hdma_scanline();
        self.hdma_lines_executed = self.hdma_lines_executed.saturating_add(1);
    }

    pub(super) fn load_hdma_entry(&mut self, channel: usize) -> bool {
        // 参照の衝突を避けるため、必要値を先に取り出す
        let table_addr = { self.dma_controller.channels[channel].hdma_table_addr };
        let control = { self.dma_controller.channels[channel].control };

        let line_info = self.read_u8(table_addr);
        if line_info == 0 {
            return false;
        }

        let repeat_flag = line_info != 0x80 && (line_info & 0x80) != 0;
        // HDMA line-count semantics per SNESdev:
        // - $00: terminate for the rest of the frame
        // - $01..$80: non-repeat, wait N scanlines
        // - $81..$FF: repeat, transfer every scanline for (N-$80) scanlines
        //
        // The low 7 bits encode the count, except $80 means 128 (not 0).
        let mut line_count = line_info & 0x7F;
        if line_count == 0 {
            line_count = 128;
        }
        let indirect = (control & 0x40) != 0; // bit6: indirect addressing

        // NOTE: HDMA tables live in the bank specified by A1Bn ($43x4) / src_address bank.
        // Indirect HDMA data blocks live in the bank specified by DASB ($43x7).
        {
            let ch = &mut self.dma_controller.channels[channel];
            ch.hdma_line_counter = line_count;
            ch.hdma_repeat_flag = repeat_flag;
            ch.hdma_do_transfer = true; // first line always transfers
            ch.hdma_indirect = indirect;
            ch.hdma_latched = [0; 4];
            ch.hdma_latched_len = 0;
            ch.nltr = line_info;
            // Advance table pointer past the line counter byte.
            ch.hdma_table_addr = Bus::add16_in_bank(table_addr, 1);
            ch.a2a = (ch.hdma_table_addr & 0xFFFF) as u16;
        }

        if indirect {
            let ptr = self.dma_controller.channels[channel].hdma_table_addr;
            let lo = self.read_u8(ptr) as u32;
            let hi = self.read_u8(Bus::add16_in_bank(ptr, 1)) as u32;
            let bank = self.dma_controller.channels[channel].dasb as u32;
            let ch = &mut self.dma_controller.channels[channel];
            ch.size = ((hi << 8) | lo) as u16;
            ch.hdma_indirect_addr = (bank << 16) | (hi << 8) | lo;
            // Advance table pointer past the 16-bit indirect address.
            ch.hdma_table_addr = Bus::add16_in_bank(ch.hdma_table_addr, 2);
            ch.a2a = (ch.hdma_table_addr & 0xFFFF) as u16;
        }

        if crate::debug_flags::trace_hdma_all() {
            use std::sync::atomic::{AtomicU32, Ordering};
            static ENTRY_CNT: AtomicU32 = AtomicU32::new(0);
            let n = ENTRY_CNT.fetch_add(1, Ordering::Relaxed);
            if n < 4096 {
                let ch = &self.dma_controller.channels[channel];
                println!(
                    "[HDMA-ENTRY] frame={} sl={} ch{} lines={} repeat={} indirect={} table=0x{:06X}{}",
                    self.ppu.get_frame(),
                    self.ppu.scanline,
                    channel,
                    line_count,
                    repeat_flag,
                    indirect,
                    table_addr,
                    if indirect {
                        format!(" iaddr=0x{:06X}", ch.hdma_indirect_addr)
                    } else {
                        String::new()
                    }
                );
            }
        }

        true
    }

    fn perform_hdma_transfer(&mut self, channel: usize) {
        // Mark write context so PPU can allow HDMA during HBlank appropriately
        self.ppu.begin_hdma_context();
        self.ppu.set_debug_dma_channel(Some(channel as u8));
        // 必要な情報を事前に取得して、借用を短く保つ
        let dest_base = { self.dma_controller.channels[channel].dest_address };
        let control = { self.dma_controller.channels[channel].control };
        let unit = control & 0x07;
        let len = Self::hdma_transfer_len(unit) as usize;
        let (src, indirect) = {
            let ch = &self.dma_controller.channels[channel];
            if ch.hdma_indirect {
                (ch.hdma_indirect_addr, true)
            } else {
                (ch.hdma_table_addr, false)
            }
        };

        if crate::debug_flags::trace_hdma_all() {
            use std::sync::atomic::{AtomicU32, Ordering};
            static XFER_CNT: AtomicU32 = AtomicU32::new(0);
            let n = XFER_CNT.fetch_add(1, Ordering::Relaxed);
            if n < 8192 {
                let repeat = self.dma_controller.channels[channel].hdma_repeat_flag;
                // Read transfer bytes for display (up to 4)
                let mut vals = [0u8; 4];
                for (j, val) in vals.iter_mut().enumerate().take(len) {
                    *val = self.read_u8(Bus::add16_in_bank(src, j as u32));
                }
                let val_str: String = (0..len)
                    .map(|j| format!("{:02X}", vals[j]))
                    .collect::<Vec<_>>()
                    .join(",");
                println!(
                    "[HDMA-XFER] frame={} sl={} ch{} dest=$21{:02X} unit={} repeat={} src=0x{:06X} vals=[{}]",
                    self.ppu.get_frame(),
                    self.ppu.scanline,
                    channel,
                    dest_base,
                    unit,
                    repeat,
                    src,
                    val_str
                );
            }
        }

        // Temporary trace: dump ch0 $210D writes for a specific frame
        let trace_frame = crate::debug_flags::trace_scroll_frame();
        let cur_frame = self.ppu.get_frame();

        // 書き込み（PPU writable or APU I/O）
        for i in 0..len {
            let data = self.read_u8(Bus::add16_in_bank(src, i as u32));
            let dest_off = Self::hdma_dest_offset(unit, dest_base, i as u8);
            let dest_addr = 0x2100u32 + dest_off as u32;
            // Trace BG1/BG2 scroll HDMA for specified frame
            if let Some(tf) = trace_frame {
                if cur_frame == tf && matches!(dest_off, 0x0D | 0x0E | 0x0F | 0x10) {
                    eprintln!(
                        "[SCROLL-HDMA] frame={} sl={} ch{} dest=$21{:02X} i={} val=0x{:02X} src=0x{:06X}",
                        cur_frame, self.ppu.scanline, channel, dest_off, i, data,
                        Bus::add16_in_bank(src, i as u32)
                    );
                }
            }
            if dest_off <= 0x33 || (0x40..=0x43).contains(&dest_off) {
                if (0x26..=0x29).contains(&dest_off) && crate::debug_flags::trace_hdma_window() {
                    use std::sync::atomic::{AtomicU32, Ordering};
                    static CNT: AtomicU32 = AtomicU32::new(0);
                    let n = CNT.fetch_add(1, Ordering::Relaxed);
                    if n < 2048 && !crate::debug_flags::quiet() {
                        println!(
                            "[HDMA-WIN] frame={} sl={} cyc={} ch{} dest=$21{:02X} val={:02X}",
                            self.ppu.get_frame(),
                            self.ppu.scanline,
                            self.ppu.get_cycle(),
                            channel,
                            dest_off,
                            data
                        );
                    }
                }
                self.write_u8(dest_addr, data);
                // Aggregate per-port stats for concise logs
                match dest_off {
                    0x15..=0x19 => {
                        // VRAM path (incl. VMAIN/VMADD*)
                        self.hdma_bytes_vram = self.hdma_bytes_vram.saturating_add(1);
                    }
                    0x21 | 0x22 => {
                        // CGRAM path
                        self.hdma_bytes_cgram = self.hdma_bytes_cgram.saturating_add(1);
                    }
                    0x04 => {
                        // OAMDATA
                        self.hdma_bytes_oam = self.hdma_bytes_oam.saturating_add(1);
                    }
                    0x26..=0x29 => {
                        // Window positions (WH0..WH3)
                        self.hdma_bytes_window = self.hdma_bytes_window.saturating_add(1);
                    }
                    _ => {}
                }
            }
        }
        // Advance source pointer after the transfer.
        if len != 0 {
            let ch = &mut self.dma_controller.channels[channel];
            if indirect {
                ch.hdma_indirect_addr = Bus::add16_in_bank(ch.hdma_indirect_addr, len as u32);
                ch.size = (ch.hdma_indirect_addr & 0xFFFF) as u16;
            } else {
                ch.hdma_table_addr = Bus::add16_in_bank(ch.hdma_table_addr, len as u32);
                ch.a2a = (ch.hdma_table_addr & 0xFFFF) as u16;
            }
        }
        self.ppu.end_hdma_context();
    }

    fn mirror_hdma_nltr(ch: &mut crate::dma::DmaChannel) {
        ch.nltr = if ch.hdma_line_counter == 128 {
            if ch.hdma_repeat_flag {
                0x00
            } else {
                0x80
            }
        } else {
            (ch.hdma_line_counter & 0x7F) | if ch.hdma_repeat_flag { 0x80 } else { 0x00 }
        };
    }

    #[inline]
    fn hdma_transfer_len(unit: u8) -> u8 {
        match unit & 0x07 {
            0 => 1,
            1 => 2,
            2 => 2,
            3 => 4,
            4 => 4,
            5 => 4,
            6 => 2,
            7 => 4,
            _ => 1,
        }
    }

    #[inline]
    fn hdma_dest_offset(unit: u8, base: u8, index: u8) -> u8 {
        let i = index;
        match unit & 0x07 {
            0 => base,                            // A
            1 => base.wrapping_add(i & 1),        // A, B
            2 => base,                            // A, A
            3 => base.wrapping_add((i >> 1) & 1), // A, A, B, B
            4 => base.wrapping_add(i & 3),        // A, B, C, D
            5 => base.wrapping_add(i & 1),        // A,B,A,B (undocumented)
            6 => base,                            // A,A (undocumented)
            7 => base.wrapping_add((i >> 1) & 1), // A,A,B,B (undocumented)
            _ => base,
        }
    }
}
