use super::debug::trace_nmi_suppress_enabled;
use super::Bus;

impl Bus {
    pub(super) fn read_io_register(&mut self, addr: u16) -> u8 {
        match addr {
            0x4016 => {
                // JOYSER0 ($4016): returns two bits (D0/D1) per read.
                // Standard controllers use only D0 (bit0). D1 (bit1) is used by multitap/etc.
                let d0 = self.input_system.read_controller1() & 1;
                let d1 = if self.input_system.is_multitap_enabled() {
                    self.input_system.read_controller3() & 1
                } else {
                    0
                };

                d0 | (d1 << 1)
            }
            0x4017 => {
                // JOYSER1 ($4017): returns two bits (D0/D1) per read plus fixed 1s in bits2-4.
                let d0 = self.input_system.read_controller2() & 1;
                let d1 = if self.input_system.is_multitap_enabled() {
                    self.input_system.read_controller4() & 1
                } else {
                    0
                };
                0x1C | d0 | (d1 << 1)
            }
            // 0x4210 - RDNMI: NMI flag and version
            0x4210 => {
                // 強制デバッグ: 常に 0x82 を返す（ループ脱出用）
                if crate::debug_flags::rdnmi_always_82() {
                    if crate::debug_flags::trace_4210() {
                        println!(
                            "[TRACE4210] read(force 0x82) PC={:06X} vblank={} nmi_en={}",
                            self.last_cpu_pc,
                            self.ppu.is_vblank(),
                            self.ppu.nmi_enabled
                        );
                    }
                    return 0x82;
                }
                // BIT $4210 ループ専用ハック: PC が 0x825B/0x8260/0x8263 のときは 0x82 を返す（VBlank判定なし）
                // 環境変数 RDNMI_FORCE_BITLOOP=1 で有効化
                if crate::debug_flags::rdnmi_force_bitloop()
                    && (self.last_cpu_pc == 0x00825B
                        || self.last_cpu_pc == 0x008260
                        || self.last_cpu_pc == 0x008263)
                {
                    // ラッチは一度クリアしておく
                    self.ppu.nmi_flag = false;
                    self.ppu.nmi_latched = false;
                    self.rdnmi_consumed = true;
                    return 0x82;
                }
                // BITループ専用ハック/ワンショットは現状なし（実機準拠）
                // デバッグ: 強制 0x82 を一度だけ返す (FORCE_4210_ONCE=1)
                use std::sync::atomic::{AtomicBool, Ordering};
                static FORCE_4210_ONCE_DONE: AtomicBool = AtomicBool::new(false);
                let force_once = crate::debug_flags::force_4210_once();
                if force_once && !FORCE_4210_ONCE_DONE.load(Ordering::Relaxed) {
                    FORCE_4210_ONCE_DONE.store(true, Ordering::Relaxed);
                    return 0x82;
                }
                // CPUテスト専用の強制 0x82 は環境変数 CPUTEST_FORCE_82 がある場合のみ
                if self.cpu_test_mode && crate::debug_flags::cputest_force_82() {
                    if crate::debug_flags::trace_4210() {
                        println!(
                            "[TRACE4210] read(cpu_test_mode force) PC={:06X} vblank={} nmi_en={}",
                            self.last_cpu_pc,
                            self.ppu.is_vblank(),
                            self.ppu.nmi_enabled
                        );
                    }
                    return 0x82;
                }

                // CPUテストHLE
                if crate::debug_flags::cpu_test_hle() {
                    let vblank = self.ppu.is_vblank();
                    let force = crate::debug_flags::cpu_test_hle_force();
                    let val = if force {
                        0x82 // 常時強制
                    } else if crate::debug_flags::cpu_test_hle_strict_vblank() {
                        if vblank {
                            0x82
                        } else {
                            0x02
                        }
                    } else {
                        0x82
                    };
                    if crate::debug_flags::trace_4210() {
                        println!(
                            "[TRACE4210] read(cpu_test_hle) PC={:06X} vblank={} nmi_en={} -> {:02X}",
                            self.last_cpu_pc,
                            vblank,
                            self.ppu.nmi_enabled,
                            val
                        );
                    }
                    return val;
                }

                // デフォルトはバージョン 0x02。bit7 は VBlank 発生ラッチ
                // または現在の VBlank level を返す。
                let mut value = 0x02;
                if crate::debug_flags::force_nmi_flag() {
                    self.ppu.nmi_flag = true;
                }
                static FORCE_RDNMI_ONCE_DONE: AtomicBool = AtomicBool::new(false);
                // 起動直後1回だけ強制で bit7 を立てる（環境変数がなくても CPU テスト時は実行）
                let force_once_env = crate::debug_flags::force_rdnmi_once();
                let force_once_auto =
                    self.cpu_test_mode && !FORCE_RDNMI_ONCE_DONE.load(Ordering::Relaxed);
                if (force_once_env || force_once_auto)
                    && !FORCE_RDNMI_ONCE_DONE.load(Ordering::Relaxed)
                {
                    FORCE_RDNMI_ONCE_DONE.store(true, Ordering::Relaxed);
                    self.ppu.nmi_flag = true;
                }

                let in_vblank = self.ppu.is_vblank();
                // 電源投入直後の特別扱いはしない（実機準拠）
                let sticky_power_on = false;
                if self.ppu.nmi_flag || in_vblank {
                    value |= 0x80;
                }
                if sticky_power_on {
                    value |= 0x80;
                    self.ppu.nmi_flag = true;
                }
                if crate::debug_flags::rdnmi_force_on() {
                    value |= 0x80;
                }
                if crate::debug_flags::rdnmi_force_vbl() && in_vblank {
                    value |= 0x80;
                }
                if crate::debug_flags::rdnmi_always_82() {
                    value = 0x82;
                }

                // CPUテスト時は16bit BIT対策で上位バイトにもbit7を複製
                if self.cpu_test_mode {
                    self.rdnmi_high_byte_for_test = if (value & 0x80) != 0 { 0x80 } else { 0x00 };
                }

                // 読み出しで VBlank edge ラッチはクリアする。ただし VBlank 中は
                // bit7 の level 表示が残るため、同じ VBlank 内の後続読み出しでも
                // bit7 を返す。
                let sticky_rdnmi = crate::debug_flags::rdnmi_sticky();
                if !sticky_rdnmi && !sticky_power_on {
                    self.ppu.nmi_flag = false;
                    if in_vblank {
                        self.ppu.rdnmi_read_in_vblank = true;
                    }
                    self.rdnmi_consumed = true;
                }

                if crate::debug_flags::trace_burnin_v224() {
                    let pc16 = (self.last_cpu_pc & 0xFFFF) as u16;
                    if (0x97D0..=0x98FF).contains(&pc16) {
                        use std::sync::atomic::{AtomicU8, Ordering};
                        static LAST: AtomicU8 = AtomicU8::new(0xFF);
                        let prev = LAST.swap(value, Ordering::Relaxed);
                        // Log only on NMI-flag (bit7) edges to avoid spamming tight loops.
                        if (prev ^ value) & 0x80 != 0 {
                            println!(
                                "[BURNIN-V224][RDNMI] PC={:06X} sl={} cyc={} vblank={} nmi_en={} {:02X}->{:02X}",
                                self.last_cpu_pc,
                                self.ppu.scanline,
                                self.ppu.get_cycle(),
                                self.ppu.is_vblank() as u8,
                                self.ppu.nmi_enabled as u8,
                                prev,
                                value
                            );
                        }
                    }
                }

                if crate::debug_flags::trace_4210() {
                    use std::sync::atomic::{AtomicU32, Ordering};
                    static COUNT: AtomicU32 = AtomicU32::new(0);
                    let n = COUNT.fetch_add(1, Ordering::Relaxed);
                    let interesting = self.ppu.is_vblank() || (value & 0x80) != 0 || n < 64;
                    if interesting {
                        println!(
                            "[TRACE4210] read#{} value=0x{:02X} (nmi_flag_after_clear={} vblank={} nmi_en={}) PC={:06X} scanline={} cycle={}",
                            n + 1,
                            value,
                            self.ppu.nmi_flag,
                            self.ppu.is_vblank(),
                            self.ppu.nmi_enabled,
                            self.last_cpu_pc,
                            self.ppu.scanline,
                            self.ppu.get_cycle()
                        );
                    }
                }
                value
            }
            // 0x4211 - TIMEUP: IRQ time-up (read/clear)
            0x4211 => {
                let v = if self.cpu_test_mode {
                    // 高バイトにもbit7を残し、BIT (16bit) でもVBlankを検出できるようにする。
                    self.rdnmi_high_byte_for_test
                } else if self.irq_pending {
                    0x80
                } else {
                    0x00
                };
                self.irq_pending = false; // reading clears
                v
            }
            // 0x4212 - HVBJOY: H/V-Blank and Joypad busy flags
            0x4212 => {
                // デバッグ: 強制値を返す（例: 0x80 なら VBlank=1, HBlank=0, JOYBUSY=0）
                if let Some(force) = crate::debug_flags::force_4212() {
                    return force;
                }
                let mut value = 0u8;
                if crate::debug_flags::cpu_test_hle_force() {
                    value = 0x80; // VBlank=1, HBlank=0, JOYBUSY=0
                } else {
                    if self.ppu.is_vblank() {
                        value |= 0x80;
                    }
                    if self.ppu.is_hblank() {
                        value |= 0x40;
                    }
                    // bit0 (JOYBUSY): set while auto-joypad is running
                    if self.joy_busy_counter > 0 {
                        value |= 0x01;
                    }
                }
                // Debug: log transitions of $4212 to confirm VBlank/HBlank visibility (opt-in)
                if crate::debug_flags::trace_4212_values() && !crate::debug_flags::quiet() {
                    use std::sync::atomic::{AtomicU32, AtomicU8, Ordering};
                    static LAST: AtomicU8 = AtomicU8::new(0xFF);
                    static COUNT: AtomicU32 = AtomicU32::new(0);
                    let prev = LAST.swap(value, Ordering::Relaxed);
                    // Log only when VBlank bit (bit7) toggles to avoid flooding with HBlank edges.
                    if (prev ^ value) & 0x80 != 0 {
                        let n = COUNT.fetch_add(1, Ordering::Relaxed);
                        if n < 64 {
                            println!(
                                "[4212] change#{:02} {:02X}->{:02X} vblank={} hblank={} joybusy={} scanline={} cycle={} PC={:06X}",
                                n + 1,
                                prev,
                                value,
                                self.ppu.is_vblank() as u8,
                                self.ppu.is_hblank() as u8,
                                (self.joy_busy_counter > 0) as u8,
                                self.ppu.scanline,
                                self.ppu.get_cycle(),
                                self.last_cpu_pc
                            );
                        }
                    }
                }
                // Debug: dump reads to see JOYBUSY behavior (opt-in)
                if crate::debug_flags::debug_joybusy() && !crate::debug_flags::quiet() {
                    use std::sync::atomic::{AtomicU32, Ordering};
                    static LOG_COUNT: AtomicU32 = AtomicU32::new(0);
                    let idx = LOG_COUNT.fetch_add(1, Ordering::Relaxed);
                    if idx < 128 {
                        println!(
                            "[JOYBUSY] read#{:03} value=0x{:02X} counter={} vblank={} hblank={} scanline={} cycle={}",
                            idx + 1,
                            value,
                            self.joy_busy_counter,
                            self.ppu.is_vblank() as u8,
                            self.ppu.is_hblank() as u8,
                            self.ppu.scanline,
                            self.ppu.get_cycle()
                        );
                    }
                }
                value
            }
            // 0x4213 - RDIO: Programmable I/O port readback
            0x4213 => {
                // Minimal behavior: return last value written to $4201.
                // Some hw ties bits to controller/expansion; we keep it simple for now.
                self.wio
            }
            // JOY1/2/3/4 data
            0x4218..=0x421F => {
                let idx = (addr - 0x4218) as usize;
                self.joy_data[idx]
            }
            // Hardware multiplication/division results
            // 0x4214/0x4215: Quotient (low/high)
            0x4214 => (self.div_quot & 0xFF) as u8,
            0x4215 => (self.div_quot >> 8) as u8,
            // 0x4216/0x4217: Multiplication result (if last op was MUL) or Division remainder
            0x4216 => (self.mul_result & 0xFF) as u8, // or div_rem low after DIV
            0x4217 => (self.mul_result >> 8) as u8,   // or div_rem high after DIV
            // $420B/$420C are write-only (W8). Reads return open bus.
            0x420B => self.mdr,
            0x420C => self.mdr,
            // APU registers readback
            0x2140..=0x217F => {
                let port = (addr & 0x3F) as u8;
                // デバッグ: APU_FORCE_PORT{0,1} で固定値を返す
                if port == 0x00 {
                    if let Some(v) = crate::debug_flags::apu_force_port0() {
                        return v;
                    }
                } else if port == 0x01 {
                    if let Some(v) = crate::debug_flags::apu_force_port1() {
                        return v;
                    }
                }
                // SMW APU HLE: 2140 reads echo連動で WRAM DMAバッファの内容を返す
                if self.smw_apu_hle && !self.smw_apu_hle_buf.is_empty() && !self.smw_apu_hle_done {
                    let idx = (self.smw_apu_hle_echo_idx as usize) % self.smw_apu_hle_buf.len();
                    let v = self.smw_apu_hle_buf[idx];
                    self.smw_apu_hle_echo_idx = self.smw_apu_hle_echo_idx.wrapping_add(1);
                    return v;
                }
                if let Ok(mut apu) = self.apu.lock() {
                    apu.sync_for_port_access(); // Catch up SPC700 before reading port
                    let v = apu.read_port(port & 0x03);
                    if crate::debug_flags::boot_verbose() {
                        static mut APU_RD_LOG: u32 = 0;
                        unsafe {
                            APU_RD_LOG += 1;
                            if APU_RD_LOG <= 16 {
                                println!("APU READ  port=0x{:02X} -> 0x{:02X}", port, v);
                            }
                        }
                    }
                    v
                } else {
                    0x00
                }
            }
            // SPC7110 registers ($4800-$484F)
            0x4800..=0x484F if self.spc7110.is_some() => {
                let rom = &self.rom as *const Vec<u8>;
                // SAFETY: read_register only reads from rom, does not mutate Bus.
                self.spc7110
                    .as_mut()
                    .unwrap()
                    .read_register(addr, unsafe { &*rom })
            }
            // S-DD1 registers ($4800-$4807)
            0x4800..=0x4807 if self.sdd1.is_some() => {
                self.sdd1.as_ref().unwrap().read_register(addr)
            }
            _ => self.mdr,
        }
    }

    pub(super) fn write_io_register(&mut self, addr: u16, value: u8) {
        match addr {
            // Controller ports
            0x4016 => {
                self.input_system.write_strobe(value);
            }
            // PPU/CPU communication
            0x4200 => {
                let pc = self.last_cpu_pc;
                // NMITIMEN - Interrupt Enable Register
                let mut actual_value = value;

                // SA-1 NMI delay: prevent NMI enable during SA-1 initialization
                if self.sa1_nmi_delay_active && (value & 0x80) != 0 {
                    actual_value = value & 0x7F; // Clear NMI enable bit
                    static mut NMI_DELAY_LOG_COUNT: u32 = 0;
                    unsafe {
                        NMI_DELAY_LOG_COUNT += 1;
                        if NMI_DELAY_LOG_COUNT <= 10 && crate::debug_flags::debug_sa1_scheduler() {
                            println!("SA-1 NMI delay: blocked $4200 NMI enable (value=0x{:02X} -> 0x{:02X})",
                                value, actual_value);
                        }
                    }
                }

                let prev_irq_enabled = (self.nmitimen & 0x30) != 0;
                self.nmitimen = actual_value;
                self.nmitimen_writes_count = self.nmitimen_writes_count.saturating_add(1);
                let prev_nmi_en = self.ppu.nmi_enabled;
                let nmi_en = (actual_value & 0x80) != 0;
                self.ppu.nmi_enabled = nmi_en;
                self.irq_h_enabled = (value & 0x10) != 0;
                self.irq_v_enabled = (value & 0x20) != 0;
                let new_irq_enabled = (actual_value & 0x30) != 0;
                // Reset HV shadow when enables change
                self.irq_v_matched_line = None;
                if prev_irq_enabled && !new_irq_enabled {
                    self.irq_pending = false;
                }
                self.recheck_irq_timer_match();
                // If NMI is enabled mid-VBlank, hardware may latch an NMI immediately *only if*
                // the NMI flag ($4210 bit7) is still set (i.e., the VBlank-edge has occurred and
                // has not yet been acknowledged via $4210 read).
                if nmi_en
                    && !prev_nmi_en
                    && self.ppu.is_vblank()
                    && self.ppu.nmi_flag
                    && !self.ppu.is_nmi_latched()
                {
                    // Suppress NMI re-latch when the NMI handler has already consumed most of
                    // VBlank.  On real hardware the handler finishes faster and a re-triggered
                    // NMI completes within VBlank; our slightly-slower CPU/DMA timing causes
                    // the second NMI to overrun into the active display, corrupting PPU state.
                    let remaining_vblank = 261u16.saturating_sub(self.ppu.scanline);
                    if remaining_vblank >= 6 {
                        self.ppu.latch_nmi_now();
                    }
                    if remaining_vblank < 6 && trace_nmi_suppress_enabled() {
                        eprintln!(
                            "[NMI-SUPPRESS] frame={} sl={} remaining={}",
                            self.ppu.get_frame(),
                            self.ppu.scanline,
                            remaining_vblank
                        );
                    }
                }
                // bit0: auto-joypad enable (ignored here)
                if crate::debug_flags::boot_verbose() && !crate::debug_flags::quiet() {
                    println!(
                        "$4200 NMITIMEN write: 0x{:02X} (NMI:{}, IRQ:{}, Auto-joypad:{}) PC={:06X}",
                        self.nmitimen,
                        (self.nmitimen & 0x80) != 0,
                        (self.nmitimen & 0x20) != 0,
                        (self.nmitimen & 0x01) != 0,
                        pc
                    );
                }
            }
            // WRIO - Joypad Programmable I/O Port; read back via $4213
            0x4201 => {
                // Bit7 ("a") is connected to the PPU latch line.
                // HV counter latch via WRIO: latching occurs on the 1->0 transition (writing 0),
                // and it latches 1 dot later than a $2137 read (see Super Famicom Dev Wiki "Timing").
                let prev = self.wio;
                self.wio = value;
                let prev_a = (prev & 0x80) != 0;
                let new_a = (value & 0x80) != 0;
                self.ppu.set_wio_latch_enable(new_a);
                if crate::debug_flags::trace_burnin_ext_latch() {
                    use std::sync::atomic::{AtomicU32, Ordering};
                    static CNT: AtomicU32 = AtomicU32::new(0);
                    let n = CNT.fetch_add(1, Ordering::Relaxed);
                    if n < 1024 {
                        println!(
                            "[BURNIN-EXT][WRIO] PC={:06X} $4201 <- {:02X} (prev={:02X}) sl={} cyc={}",
                            self.last_cpu_pc,
                            value,
                            prev,
                            self.ppu.scanline,
                            self.ppu.get_cycle()
                        );
                    }
                }
                if prev_a && !new_a {
                    self.ppu.request_wrio_hv_latch();
                }
            }
            0x4202 => {
                // WRMPYA - Multiplicand A (8-bit)
                self.mul_a = value;
            }
            0x4203 => {
                // WRMPYB - Multiplicand B (start 8x8 multiply)
                self.mul_b = value;
                // Any in-flight divide is aborted (single shared math unit behavior).
                self.div_busy = false;
                self.div_just_started = false;

                if self.mul_busy {
                    // Real hardware quirk: writing to WRMPYB again before the 8-cycle
                    // multiply has completed does *not* correctly restart the unit; the
                    // remaining cycles continue and the result becomes "corrupted".
                    // Model this by updating the internal multiplier shift register only.
                    self.mul_work_b = self.mul_b;
                } else {
                    // Start 8-cycle multiply; results ($4216/$4217) update while in-flight.
                    self.mul_busy = true;
                    self.mul_just_started = true;
                    self.mul_cycles_left = 8;
                    self.mul_work_a = self.mul_a as u16;
                    self.mul_work_b = self.mul_b;
                    self.mul_partial = 0;
                    self.mul_result = 0;
                }
            }
            0x4204 => {
                // WRDIVL - Dividend Low
                self.div_a = (self.div_a & 0xFF00) | (value as u16);
            }
            0x4205 => {
                // WRDIVH - Dividend High
                self.div_a = (self.div_a & 0x00FF) | ((value as u16) << 8);
            }
            0x4206 => {
                // WRDIVB - Divisor (start 16/8 divide)
                self.div_b = value;
                // Abort in-flight multiply (single shared math unit behavior).
                self.mul_busy = false;
                self.mul_just_started = false;

                if self.div_b == 0 {
                    // Division-by-zero special case.
                    self.div_quot = 0xFFFF;
                    self.div_rem = self.div_a;
                    self.mul_result = self.div_rem;
                    self.div_busy = false;
                    self.div_just_started = false;
                    self.div_cycles_left = 0;
                    self.div_work_dividend = 0;
                    self.div_work_divisor = 0;
                    self.div_work_quot = 0;
                    self.div_work_rem = 0;
                    self.div_work_bit = 0;
                } else {
                    // 16-cycle restoring division; results ($4214-$4217) update while in-flight.
                    self.div_busy = true;
                    self.div_just_started = true;
                    self.div_cycles_left = 16;
                    self.div_work_dividend = self.div_a;
                    self.div_work_divisor = self.div_b;
                    self.div_work_quot = 0;
                    self.div_work_rem = 0;
                    self.div_work_bit = 15;
                    self.div_quot = 0;
                    self.div_rem = 0;
                    self.mul_result = 0;
                }
            }
            0x4207 => {
                // HTIMEL - Horizontal Timer Low
                self.h_timer = (self.h_timer & 0xFF00) | (value as u16);
                self.h_timer_set = true;
                self.recheck_irq_timer_match();
            }
            0x4208 => {
                // HTIMEH - Horizontal Timer High
                self.h_timer = (self.h_timer & 0x00FF) | ((value as u16) << 8);
                self.h_timer_set = true;
                self.recheck_irq_timer_match();
            }
            0x4209 => {
                // VTIMEL - Vertical Timer Low
                self.v_timer = (self.v_timer & 0xFF00) | (value as u16);
                self.v_timer_set = true;
                self.recheck_irq_timer_match();
            }
            0x420A => {
                // VTIMEH - Vertical Timer High
                self.v_timer = (self.v_timer & 0x00FF) | ((value as u16) << 8);
                self.v_timer_set = true;
                self.recheck_irq_timer_match();
            }
            0x420B => {
                // MDMAEN - General DMA Enable
                if crate::debug_flags::trace_dma_reg_pc() {
                    println!(
                        "[DMA-EN-PC] PC={:06X} W $420B val={:02X}",
                        self.last_cpu_pc, value
                    );
                }
                self.dma_controller.write(addr, value);
                if value != 0 {
                    self.mdmaen_nonzero_count = self.mdmaen_nonzero_count.saturating_add(1);
                }

                // Debug/test mode: 強制的に即時MDMAを実行（タイミングゲート無視）
                // STRICT_PPU_TIMING などで defer されて実行されない疑いがある場合に使う。
                if crate::debug_flags::force_mdma_now() && value != 0 {
                    println!("[FORCE_MDMA_NOW] value=0x{:02X}", value);
                    for i in 0..8 {
                        if value & (1 << i) != 0 {
                            self.perform_dma_transfer(i as usize);
                        }
                    }
                    return;
                }

                let strict = crate::debug_flags::strict_ppu_timing();
                let (mut now_mask, defer_mask) =
                    self.partition_mdma_mask_for_current_window(value, strict);
                if defer_mask != 0 {
                    self.pending_gdma_mask |= defer_mask;
                }
                // Enhanced DMA monitoring for graphics transfers was removed to reduce log noise.
                // MDMAEN starts after the *next opcode fetch* (SNESdev timing note).
                // So here we only queue the channels; the actual transfer happens in
                // `CpuBus::opcode_memory_penalty()` for the S-CPU bus.
                for i in 0..8 {
                    if (now_mask & (1 << i)) != 0 && !self.dma_controller.channels[i].configured {
                        now_mask &= !(1 << i);
                    }
                }
                self.pending_mdma_mask |= now_mask;
                self.trace_starfox_boot_io("W", 0x420B, value);
            }
            0x420C => {
                // HDMAEN - H-blank DMA Enable
                let old_hdma_enable = self.dma_controller.hdma_enable;
                if crate::debug_flags::trace_hdma_enable() {
                    let frame = self.ppu.get_frame();
                    eprintln!(
                        "[HDMA-EN] frame={} PC={:06X} $420C <- {:02X}",
                        frame, self.last_cpu_pc, value
                    );
                }
                self.dma_controller.write(addr, value);
                let newly_enabled = value & !old_hdma_enable;
                if newly_enabled != 0 {
                    self.enable_hdma_channels_now(newly_enabled);
                }
                if value != 0 {
                    self.hdmaen_nonzero_count = self.hdmaen_nonzero_count.saturating_add(1);
                }
            }
            0x420D => {
                // MEMSEL - Memory Speed Control
                // bit0: 1=FastROM, 0=SlowROM. We store the bit for future timing use.
                self.fastrom = (value & 0x01) != 0;
            }
            // SPC7110 registers ($4800-$484F)
            0x4800..=0x484F if self.spc7110.is_some() => {
                let rom = &self.rom as *const Vec<u8>;
                // SAFETY: write_register only reads from rom, does not mutate Bus.
                self.spc7110
                    .as_mut()
                    .unwrap()
                    .write_register(addr, value, unsafe { &*rom });
            }
            // S-DD1 registers ($4800-$4807)
            0x4800..=0x4807 if self.sdd1.is_some() => {
                self.sdd1.as_mut().unwrap().write_register(addr, value);
            }
            _ => {
                // Unhandled CPU I/O holes: ignore writes.
            }
        }
    }
}
