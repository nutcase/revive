use super::{debug::trace_sram, Bus};
use crate::cartridge::mapper::MemoryMapper;

impl Bus {
    pub fn read_u8(&mut self, addr: u32) -> u8 {
        let bank = (addr >> 16) & 0xFF;
        let offset = (addr & 0xFFFF) as u16;

        // SA-1 vector override for S-CPU when SCNT selects SA-1 provided vectors.
        // SCNT bit6 -> use SIV (IRQ vector) instead of ROM $FFEE
        // SCNT bit5 -> use SNV (NMI vector) instead of ROM $FFEA
        if self.is_sa1_active() && bank == 0x00 {
            match offset {
                0xFFEA | 0xFFEB if (self.sa1.registers.scnt & 0x20) != 0 => {
                    let v = self.sa1.registers.snv;
                    return if offset & 1 == 0 {
                        (v & 0xFF) as u8
                    } else {
                        (v >> 8) as u8
                    };
                }
                0xFFEE | 0xFFEF if (self.sa1.registers.scnt & 0x40) != 0 => {
                    let v = self.sa1.registers.siv;
                    return if offset & 1 == 0 {
                        (v & 0xFF) as u8
                    } else {
                        (v >> 8) as u8
                    };
                }
                _ => {}
            }
        }

        // Debug: consolidated read-trace checks behind a single cached flag.
        if self.any_read_trace_active {
            Self::read_u8_trace(self, addr, bank, offset);
        }

        // SA-1 BW-RAM mapping for S-CPU in banks $40-$4F and high-speed mirror $60-$6F (full 64KB each)
        if self.is_sa1_active() && ((0x40..=0x4F).contains(&bank) || (0x60..=0x6F).contains(&bank))
        {
            if !self.sa1_bwram.is_empty() {
                let base = if (0x60..=0x6F).contains(&bank) {
                    (bank - 0x60) as usize
                } else {
                    (bank - 0x40) as usize
                };
                let idx = (base << 16) | offset as usize;
                return self.sa1_bwram[idx % self.sa1_bwram.len()];
            }
            return 0xFF;
        }

        let value = match bank {
            // Dragon Quest 3 special banks - highest priority
            0x03 | 0x24 if self.mapper_type == crate::cartridge::MapperType::DragonQuest3 => {
                return self.read_dq3_rom(bank, offset);
            }
            // System area banks (mirror in 80-BF)
            0x00..=0x3F | 0x80..=0xBF => {
                match offset {
                    // SA-1 I-RAM window for S-CPU (00:3000-37FF)
                    0x3000..=0x37FF if self.is_sa1_active() => {
                        let idx = (offset - 0x3000) as usize;
                        if idx < self.sa1_iram.len() {
                            return self.sa1_iram[idx];
                        }
                        return 0xFF;
                    }
                    // SA-1 registers window (banks 00-3F/80-BF)
                    0x2200..=0x23FF if self.is_sa1_active() => {
                        let reg = offset - 0x2200;
                        let v = self.read_sa1_register_scpu(reg);
                        if crate::debug_flags::trace_sa1_reg() {
                            println!("SA1 REG R {:02X}:{:04X} -> {:02X}", bank, offset, v);
                        }
                        if matches!(reg, 0x100 | 0x101) && crate::debug_flags::trace_sfr_val() {
                            use std::sync::atomic::{AtomicU32, Ordering};
                            static COUNT_SFR: AtomicU32 = AtomicU32::new(0);
                            let idx = COUNT_SFR.fetch_add(1, Ordering::Relaxed);
                            if idx < 32 {
                                println!(
                                    "[SFR READ] reg=0x{:04X} val=0x{:02X} enable=0x{:02X} pending=0x{:02X} CIE=0x{:02X} SIE=0x{:02X}",
                                    0x2200 + reg,
                                    v,
                                    self.sa1.registers.interrupt_enable,
                                    self.sa1.registers.interrupt_pending,
                                    self.sa1.registers.cie,
                                    self.sa1.registers.sie
                                );
                            }
                        }
                        return v;
                    }
                    // 0x0000-0x1FFF: WRAM (標準挙動に統一)
                    // Stack area (0x0100-0x01FF)
                    0x0100..=0x01FF => {
                        let value = self.wram[offset as usize];
                        // Debug stack reads returning 0xFF
                        if crate::debug_flags::debug_stack_read() {
                            static mut STACK_READ_COUNT: u32 = 0;
                            unsafe {
                                if value == 0xFF {
                                    STACK_READ_COUNT += 1;
                                    if STACK_READ_COUNT <= 20 {
                                        println!("STACK READ #{}: Reading 0xFF from stack 0x{:04X}, bank=0x{:02X}",
                                                 STACK_READ_COUNT, offset, bank);
                                    }
                                }
                            }
                        }
                        value
                    }
                    // Mirror WRAM in first 8KB (excluding stack area already handled above)
                    0x0000..=0x00FF | 0x0200..=0x1FFF => self.wram[offset as usize],
                    // $2000-$20FF is unmapped on real hardware (open bus)
                    0x2000..=0x20FF => self.mdr,
                    0x6000..=0x7FFF if self.is_sa1_active() => {
                        if let Some(idx) = self.sa1_bwram_addr(offset) {
                            let v = self.sa1_bwram[idx];
                            if crate::debug_flags::trace_bwram_sys() {
                                use std::sync::atomic::{AtomicU32, Ordering};
                                static COUNT_R: AtomicU32 = AtomicU32::new(0);
                                let n = COUNT_R.fetch_add(1, Ordering::Relaxed);
                                if n < 32 {
                                    println!(
                                "BWRAM SYS R bank={:02X} off={:04X} idx=0x{:05X} val={:02X}",
                                bank, offset, idx, v
                            );
                                }
                            }
                            return v;
                        }
                        if crate::debug_flags::trace_bwram_sys() {
                            use std::sync::atomic::{AtomicU32, Ordering};
                            static COUNT: AtomicU32 = AtomicU32::new(0);
                            let n = COUNT.fetch_add(1, Ordering::Relaxed);
                            if n < 32 {
                                println!(
                                    "BWRAM SYS R bank={:02X} off={:04X} (no-map) val=FF",
                                    bank, offset
                                );
                            }
                        }
                        0xFF
                    }
                    // PPU registers
                    0x2100..=0x213F => {
                        let ppu_reg = offset & 0xFF;
                        if matches!(ppu_reg, 0x39 | 0x3A)
                            && crate::debug_flags::trace_burnin_dma_memory()
                        {
                            let pc16 = (self.last_cpu_pc & 0xFFFF) as u16;
                            if (0xAE80..=0xAEEF).contains(&pc16) {
                                use std::sync::atomic::{AtomicU32, Ordering};
                                static CNT: AtomicU32 = AtomicU32::new(0);
                                let n = CNT.fetch_add(1, Ordering::Relaxed);
                                if n < 64 {
                                    let (vmadd, inc, vmain) = self.ppu.dbg_vram_regs();
                                    println!(
                                        "[BURNIN-DMAMEM][PPU-R] PC={:06X} ${:04X} VMADD={:04X} VMAIN={:02X} inc={} (pre)",
                                        self.last_cpu_pc, offset, vmadd, vmain, inc
                                    );
                                }
                            }
                        }
                        let v = match ppu_reg {
                            0x37 => {
                                // $2137 latches H/V counters as a side effect, but the returned byte
                                // is open bus on hardware.
                                // Latch at the current MMIO access point rather than an arbitrary
                                // later dot. The PPU only advances between instructions in this
                                // emulator, so project the latch by the elapsed bus time within
                                // the current instruction plus this read's access time.
                                let access_master =
                                    self.cpu_access_master_cycles(offset as u32) as u64;
                                let when = self
                                    .cpu_instr_elapsed_master_cycles()
                                    .saturating_add(access_master);
                                self.ppu.latch_hv_counters_after_master_cycles(when);
                                self.mdr
                            }
                            0x38 if !self.ppu.can_read_oam_now() => self.mdr,
                            0x39 | 0x3A if !self.ppu.can_read_vram_now() => self.mdr,
                            0x3B if !self.ppu.can_read_cgram_now() => self.mdr,
                            _ => self.ppu.read(ppu_reg),
                        };
                        if matches!(ppu_reg, 0x39 | 0x3A)
                            && crate::debug_flags::trace_burnin_dma_memory()
                        {
                            let pc16 = (self.last_cpu_pc & 0xFFFF) as u16;
                            if (0xAE80..=0xAEEF).contains(&pc16) {
                                use std::sync::atomic::{AtomicU32, Ordering};
                                static CNT: AtomicU32 = AtomicU32::new(0);
                                let n = CNT.fetch_add(1, Ordering::Relaxed);
                                if n < 64 {
                                    let (vmadd, inc, vmain) = self.ppu.dbg_vram_regs();
                                    println!(
                                        "[BURNIN-DMAMEM][PPU-R] PC={:06X} ${:04X} -> {:02X} VMADD={:04X} VMAIN={:02X} inc={} (post)",
                                        self.last_cpu_pc, offset, v, vmadd, vmain, inc
                                    );
                                }
                            }
                        }
                        if crate::debug_flags::trace_burnin_v224() {
                            let pc16 = (self.last_cpu_pc & 0xFFFF) as u16;
                            if (0x97D0..=0x98FF).contains(&pc16) {
                                match offset {
                                    0x2137 | 0x213D | 0x213F => {
                                        println!(
                                            "[BURNIN-V224][PPU-R] PC={:06X} ${:04X} -> {:02X} sl={} cyc={} vblank={} vis_h={}",
                                            self.last_cpu_pc,
                                            offset,
                                            v,
                                            self.ppu.scanline,
                                            self.ppu.get_cycle(),
                                            self.ppu.is_vblank() as u8,
                                            self.ppu.get_visible_height()
                                        );
                                    }
                                    _ => {}
                                }
                            }
                        }
                        if crate::debug_flags::trace_burnin_ext_latch() {
                            use std::sync::atomic::{AtomicU32, Ordering};
                            static CNT: AtomicU32 = AtomicU32::new(0);
                            let n = CNT.fetch_add(1, Ordering::Relaxed);
                            if n < 2048 {
                                match offset {
                                    0x2137 | 0x213C | 0x213D | 0x213F => {
                                        println!(
                                            "[BURNIN-EXT][PPU-R] PC={:06X} ${:04X} -> {:02X} sl={} cyc={} vblank={} wio=0x{:02X}",
                                            self.last_cpu_pc,
                                            offset,
                                            v,
                                            self.ppu.scanline,
                                            self.ppu.get_cycle(),
                                            self.ppu.is_vblank() as u8,
                                            self.wio
                                        );
                                    }
                                    _ => {}
                                }
                            }
                        }
                        if crate::debug_flags::trace_burnin_obj() && offset == 0x213E {
                            use std::sync::atomic::{AtomicU32, Ordering};
                            static CNT: AtomicU32 = AtomicU32::new(0);
                            let n = CNT.fetch_add(1, Ordering::Relaxed);
                            if n < 256 {
                                println!(
                                    "[BURNIN-OBJ][STAT77] PC={:06X} -> {:02X} frame={} sl={} cyc={} vblank={}",
                                    self.last_cpu_pc,
                                    v,
                                    self.ppu.get_frame(),
                                    self.ppu.scanline,
                                    self.ppu.get_cycle(),
                                    self.ppu.is_vblank() as u8
                                );
                            }
                        }
                        v
                    }
                    // APU registers
                    0x2140..=0x217F => {
                        let apu_inline_cpu = self.take_apu_inline_cpu_cycles_for_current_access();
                        let val = self.apu
                                .lock()
                                .map(|mut apu| {
                                    if apu_inline_cpu != 0 {
                                        apu.add_cpu_cycles(apu_inline_cpu as u32);
                                    }
                                    apu.sync_for_port_access(); // Catch up SPC700 before reading port
                                    let p = (offset & 0x03) as u8;
                                    let mut v = apu.read_port(p);
                                    if offset == 0x2140
                                        && self.mapper_type
                                            == crate::cartridge::MapperType::SuperFx
                                        && self.ppu.get_frame() < 180
                                        && Self::is_starfox_apu_echo_wait_pc(self.last_cpu_pc)
                                        && std::env::var_os(
                                            "DISABLE_STARFOX_APU_ECHO_WAIT_ASSIST",
                                        )
                                        .is_none()
                                        && v != apu.port_latch[0]
                                    {
                                        apu.run_until_cpu_port_matches_latch(
                                            0,
                                            Self::apu_echo_wait_budget(),
                                        );
                                        v = apu.read_port(p);
                                    }
                                    if offset == 0x2140
                                        && self.mapper_type
                                            != crate::cartridge::MapperType::SuperFx
                                        && !Self::apu_echo_wait_assist_disabled()
                                        && v != apu.port_latch[0]
                                    {
                                        // Many SPC loaders acknowledge each transfer byte by
                                        // echoing APUIO0. If the CPU is already polling that
                                        // echo, give the APU a bounded catch-up window so large
                                        // sample uploads do not stretch across many video frames.
                                        apu.run_until_cpu_port_matches_latch(
                                            0,
                                            Self::apu_echo_wait_budget(),
                                        );
                                        v = apu.read_port(p);
                                    }
                                    // (read trace removed for clarity)
                                    // burn-in-test.sfc APU FAIL調査: CPU側が最終判定で $2141 を読む瞬間に
                                    // APU(S-SMP) の実行位置をログに出す（opt-in, 少量）。
                                    if crate::debug_flags::trace_burnin_apu_prog()
                                        && offset == 0x2141
                                        && self.last_cpu_pc == 0x00863F
                                    {
                                        use std::sync::atomic::{AtomicU32, Ordering};
                                        static CNT: AtomicU32 = AtomicU32::new(0);
                                        let n = CNT.fetch_add(1, Ordering::Relaxed);
                                        if n < 4 {
                                            if let Some(smp) = apu.inner.smp.as_ref() {
                                                let smp_pc = smp.reg_pc;
                                                let smp_a = smp.reg_a;
                                                let smp_x = smp.reg_x;
                                                let smp_y = smp.reg_y;
                                                let smp_sp = smp.reg_sp;
                                                let smp_psw = smp.get_psw();
                                                let ctx_start = smp_pc.wrapping_sub(0x10);
                                                let mut code = [0u8; 32];
                                                for (i, b) in code.iter_mut().enumerate() {
                                                    *b = apu
                                                        .inner
                                                        .read_u8(ctx_start.wrapping_add(i as u16) as u32);
                                                }
                                                let t0 = apu.inner.debug_timer_state(0);
                                                println!(
                                                    "[BURNIN-APU-PROG] cpu_pc=00:{:04X} apui1={:02X} sl={} cyc={} frame={} vblank={} vis_h={} apu_cycles={} smp_pc={:04X} A={:02X} X={:02X} Y={:02X} SP={:02X} PSW={:02X} t0={:?} code@{:04X}={:02X?}",
                                                    (self.last_cpu_pc & 0xFFFF) as u16,
                                                    v,
                                                    self.ppu.scanline,
                                                    self.ppu.get_cycle(),
                                                    self.ppu.get_frame(),
                                                    self.ppu.is_vblank() as u8,
                                                    self.ppu.get_visible_height(),
                                                    apu.total_smp_cycles,
                                                    smp_pc,
                                                    smp_a,
                                                    smp_x,
                                                    smp_y,
                                                    smp_sp,
                                                    smp_psw,
                                                    t0,
                                                    ctx_start,
                                                    code
                                                );
                                            } else {
                                                println!(
                                                    "[BURNIN-APU-PROG] cpu_pc=00:{:04X} apui1={:02X} smp=<none>",
                                                    (self.last_cpu_pc & 0xFFFF) as u16,
                                                    v
                                                );
                                            }
                                        }
                                    }
                                    if crate::debug_flags::trace_apu_port() {
                                        use std::sync::atomic::{AtomicU32, Ordering};
                                        static COUNT: AtomicU32 = AtomicU32::new(0);
                                        let n = COUNT.fetch_add(1, Ordering::Relaxed);
                                        if n < 256 {
                                            println!(
                                                "[APU] R ${:04X} (port{}) -> {:02X}",
                                                offset, p, v
                                            );
                                        }
                                    }
                                    if crate::debug_flags::trace_sfs_apu_wait()
                                        && offset == 0x2140
                                        && matches!(
                                            self.last_cpu_pc,
                                            0x008858 | 0x008884 | 0x0088BD
                                        )
                                    {
                                        use std::sync::OnceLock;
                                        static TRACE_PC: OnceLock<Option<u32>> = OnceLock::new();
                                        let watch_pc = TRACE_PC.get_or_init(|| {
                                            std::env::var("TRACE_SFS_APU_WAIT_PC")
                                                .ok()
                                                .and_then(|v| {
                                                    let t = v.trim();
                                                    let t = t.trim_start_matches("0x");
                                                    u32::from_str_radix(t, 16)
                                                        .ok()
                                                        .or_else(|| t.parse::<u32>().ok())
                                                })
                                        });
                                        if let Some(pc) = *watch_pc {
                                            if self.last_cpu_pc != pc {
                                                // Skip early noisy loops unless PC matches.
                                                return v;
                                            }
                                        }
                                        use std::sync::atomic::{AtomicU32, Ordering};
                                        static CNT: AtomicU32 = AtomicU32::new(0);
                                        let n = CNT.fetch_add(1, Ordering::Relaxed);
                                        if n < 64 {
                                            if let Some(smp) = apu.inner.smp.as_ref() {
                                                let smp_pc = smp.reg_pc;
                                                let smp_psw = smp.get_psw();
                                                println!(
                                                    "[SFS-APU-WAIT] cpu_pc=00:{:04X} apu_p0={:02X} cpu_to_apu=[{:02X} {:02X} {:02X} {:02X}] smp_pc={:04X} psw={:02X} stopped={} apu_cycles={}",
                                                    (self.last_cpu_pc & 0xFFFF) as u16,
                                                    v,
                                                    apu.port_latch[0],
                                                    apu.port_latch[1],
                                                    apu.port_latch[2],
                                                    apu.port_latch[3],
                                                    smp_pc,
                                                    smp_psw,
                                                    smp.is_stopped() as u8,
                                                    apu.total_smp_cycles
                                                );
                                                if crate::debug_flags::trace_sfs_apu_wait_dump()
                                                {
                                                    let mut code = [0u8; 16];
                                                    for (i, b) in code.iter_mut().enumerate() {
                                                        *b = apu
                                                            .inner
                                                            .read_u8(smp_pc.wrapping_add(i as u16) as u32);
                                                    }
                                                    println!(
                                                        "[SFS-APU-WAIT] smp_code@{:04X}={:02X?}",
                                                        smp_pc, code
                                                    );
                                                }
                                            } else {
                                                println!(
                                                    "[SFS-APU-WAIT] cpu_pc=00:{:04X} apu_p0={:02X} smp=<none>",
                                                    (self.last_cpu_pc & 0xFFFF) as u16,
                                                    v
                                                );
                                            }
                                        }
                                    }
                                    if crate::debug_flags::trace_sfs_apu_mismatch()
                                        && offset == 0x2140
                                        && matches!(self.last_cpu_pc, 0x008858 | 0x00885B)
                                    {
                                        let expected = self.wram.get(0x0006).copied().unwrap_or(0);
                                        if v != expected {
                                            use std::sync::atomic::{AtomicU32, Ordering};
                                            static CNT: AtomicU32 = AtomicU32::new(0);
                                            let n = CNT.fetch_add(1, Ordering::Relaxed);
                                            if n < 256 {
                                                let (smp_pc, psw) = apu
                                                    .inner
                                                    .smp
                                                    .as_ref()
                                                    .map(|s| (s.reg_pc, s.get_psw()))
                                                    .unwrap_or((0, 0));
                                                println!(
                                                    "[SFS-APU-MISMATCH] cpu_pc=00:{:04X} apu_p0={:02X} expected={:02X} wram04={:02X} wram02={:02X} cpu_to_apu=[{:02X} {:02X} {:02X} {:02X}] smp_pc={:04X} psw={:02X}",
                                                    (self.last_cpu_pc & 0xFFFF) as u16,
                                                    v,
                                                    expected,
                                                    self.wram.get(0x0004).copied().unwrap_or(0),
                                                    self.wram.get(0x0002).copied().unwrap_or(0),
                                                    apu.port_latch[0],
                                                    apu.port_latch[1],
                                                    apu.port_latch[2],
                                                    apu.port_latch[3],
                                                    smp_pc,
                                                    psw
                                                );
                                            }
                                        }
                                    }
                                    v
                                })
                                .unwrap_or(0);
                        if offset <= 0x2143 {
                            self.trace_starfox_boot_io("R", offset as u32, val);
                        }
                        // Test ROM support: SPC->CPU 2140 streamをコンソールへ転送
                        if (self.test_apu_print || crate::debug_flags::cpu_test_hle())
                            && offset == 0x2140
                        {
                            let ch = val as char;
                            if ch.is_ascii_graphic() || ch == ' ' || ch == '\n' || ch == '\r' {
                                self.test_apu_buf.push(ch);
                                if ch == '\n' || self.test_apu_buf.len() > 512 {
                                    let line = self.test_apu_buf.replace('\r', "");
                                    println!("[TESTROM] APU: {}", line.trim_end());
                                    let lower = line.to_ascii_lowercase();
                                    if lower.contains("passed") || lower.contains("pass") {
                                        println!("[TESTROM] PASS");
                                        crate::shutdown::request_quit();
                                    } else if lower.contains("fail") {
                                        println!("[TESTROM] FAIL");
                                        crate::shutdown::request_quit();
                                    }
                                    self.test_apu_buf.clear();
                                }
                            }
                        }
                        // Concise APU handshake trace (read side)
                        if crate::debug_flags::trace_apu_handshake() && offset <= 0x2143 {
                            let state = self
                                .apu
                                .lock()
                                .map(|apu| apu.handshake_state_str())
                                .unwrap_or("apu-lock");
                            use std::sync::atomic::{AtomicU32, Ordering};
                            static CNT: AtomicU32 = AtomicU32::new(0);
                            let n = CNT.fetch_add(1, Ordering::Relaxed);
                            let limit = crate::debug_flags::trace_apu_handshake_limit();
                            if n < limit {
                                println!(
                                    "[APU-HS][R] ${:04X} -> {:02X} state={} pc={:06X} frame={} sl={} cyc={}",
                                    offset,
                                    val,
                                    state,
                                    self.last_cpu_pc,
                                    self.ppu.get_frame(),
                                    self.ppu.scanline,
                                    self.ppu.get_cycle()
                                );
                            }
                        }
                        val
                    }
                    // WRAM access port
                    0x2180 => {
                        let addr = self.wram_address as usize;
                        if addr < self.wram.len() {
                            let value = self.wram[addr];
                            // WMADD ($2181-2183) is a 17-bit address; auto-increment carries across bit16.
                            self.wram_address = (self.wram_address + 1) & 0x1FFFF;
                            value
                        } else {
                            0xFF
                        }
                    }
                    0x2181..=0x2183 => self.mdr, // WRAM Address registers (write-only / open bus)
                    // Super FX registers/cache
                    0x3000..=0x34FF if self.is_superfx_active() => {
                        if let Some(ref mut gsu) = self.superfx {
                            match offset {
                                0x3000..=0x34FF => {
                                    let reg_offset = if (0x3300..=0x34FF).contains(&offset) {
                                        0x3000 + ((offset - 0x3300) & 0x00FF)
                                    } else {
                                        offset
                                    };
                                    if reg_offset == 0x3030 && gsu.running() {
                                        let poll_pc = self.last_cpu_exec_pc;
                                        let poll_bank = (poll_pc >> 16) as u8;
                                        let is_wram_poll = poll_bank == 0x7E || poll_bank == 0x7F;
                                        if self.superfx_status_poll_pc == poll_pc {
                                            self.superfx_status_poll_streak =
                                                self.superfx_status_poll_streak.saturating_add(1);
                                        } else {
                                            self.superfx_status_poll_pc = poll_pc;
                                            self.superfx_status_poll_streak = 1;
                                        }
                                        let streak = self.superfx_status_poll_streak;
                                        let disable_status_poll_assist_env =
                                            Self::disable_superfx_status_poll_assist_env();
                                        let enable_status_poll_assist =
                                            Self::enable_superfx_status_poll_assist_env();
                                        let disable_starfox_late_wait_assist =
                                            Self::disable_starfox_late_wait_assist_env();
                                        let disable_status_poll_catchup =
                                            Self::disable_superfx_status_poll_catchup_env();
                                        let disable_status_poll_run_until_stop =
                                            Self::disable_superfx_status_poll_run_until_stop_env();
                                        let late_parser_budget_override =
                                            Self::superfx_status_poll_late_parser_budget();
                                        let base_steps =
                                            crate::cartridge::superfx::SuperFx::status_poll_step_budget();
                                        let frame = self.ppu.get_frame();
                                        let early_bootstrap = frame < 120;
                                        let mid_bootstrap =
                                            is_wram_poll && (120..150).contains(&frame);
                                        let starfox_go_busy_wait = {
                                            let wram = &self.wram as *const Vec<u8>;
                                            unsafe {
                                                Self::matches_starfox_3030_go_busy_wait_in_wram(
                                                    &*wram, poll_pc,
                                                )
                                            }
                                        };
                                        // Star Fox later wait spins on the
                                        // 7E:4EFD-4F03 loop:
                                        //   4EFD: LDA $3030
                                        //   4F00: AND #$20
                                        //   4F02: BNE $4EFD
                                        // Only assist the exact LDA site for
                                        // the real WRAM-resident `$3030 & #$20`
                                        // loop. Star Fox enters this same wait
                                        // shape both early and late in boot, so
                                        // do not gate it on an arbitrary frame.
                                        let late_starfox_wait = starfox_go_busy_wait
                                            && Self::is_starfox_late_3030_busy_wait_pc(poll_pc);
                                        let starfox_cached_delay_loop = late_starfox_wait
                                            && gsu.debug_in_starfox_cached_delay_loop();
                                        let starfox_late_parser_loop =
                                            gsu.debug_in_starfox_late_parser_loop();
                                        // The 7E:4EFD loop is a real `$3030 & #$20`
                                        // busy-wait. Once we've matched that exact loop,
                                        // advancing the coprocessor until GO clears is
                                        // semantically equivalent to what the CPU is doing,
                                        // regardless of which frame the wait begins on.
                                        let starfox_exact_late_wait = late_starfox_wait;
                                        let starfox_live_producer_wait = starfox_exact_late_wait
                                            && gsu.debug_in_starfox_live_producer_loop();
                                        let starfox_live_producer_budget =
                                            Self::starfox_status_poll_producer_budget()
                                                .unwrap_or_else(|| {
                                                    base_steps.saturating_mul(16_384).max(262_144)
                                                });
                                        let starfox_early_wait_sfr_budget =
                                            base_steps.saturating_mul(128).max(65_536);
                                        // Keep generic status-poll assists opt-in, but allow
                                        // the exact Star Fox 7E:4EFD late wait helper by
                                        // default. That loop is a pure `$3030 & #$20` busy-wait.
                                        let disable_all_status_poll_assist =
                                            disable_status_poll_assist_env;
                                        let disable_generic_status_poll_assist =
                                            disable_status_poll_assist_env
                                                || !enable_status_poll_assist;
                                        let late_starfox_wait_full_assist = late_starfox_wait
                                            && !disable_starfox_late_wait_assist
                                            && streak == 1;
                                        let catch_up_steps = if let Some(override_steps) =
                                            late_parser_budget_override
                                        {
                                            if starfox_late_parser_loop
                                                && !disable_generic_status_poll_assist
                                                && !disable_status_poll_catchup
                                            {
                                                override_steps
                                            } else if disable_generic_status_poll_assist
                                                || disable_status_poll_catchup
                                            {
                                                0
                                            } else if early_bootstrap && is_wram_poll {
                                                if streak >= 3 && streak.is_multiple_of(3) {
                                                    base_steps.saturating_mul(16)
                                                } else {
                                                    0
                                                }
                                            } else if early_bootstrap {
                                                if streak >= 4 && streak.is_multiple_of(4) {
                                                    base_steps.saturating_mul(8)
                                                } else {
                                                    0
                                                }
                                            } else if starfox_go_busy_wait {
                                                0
                                            } else if mid_bootstrap {
                                                if streak >= 8 && (streak - 8).is_multiple_of(8) {
                                                    base_steps.saturating_mul(4)
                                                } else {
                                                    0
                                                }
                                            } else if is_wram_poll {
                                                if streak >= 8 {
                                                    base_steps
                                                } else {
                                                    0
                                                }
                                            } else if streak >= 8 && (streak - 8).is_multiple_of(16)
                                            {
                                                base_steps
                                            } else {
                                                0
                                            }
                                        } else if disable_generic_status_poll_assist
                                            || disable_status_poll_catchup
                                        {
                                            0
                                        } else if early_bootstrap && is_wram_poll {
                                            if streak >= 3 && streak.is_multiple_of(3) {
                                                base_steps.saturating_mul(16)
                                            } else {
                                                0
                                            }
                                        } else if early_bootstrap {
                                            if streak >= 4 && streak.is_multiple_of(4) {
                                                base_steps.saturating_mul(8)
                                            } else {
                                                0
                                            }
                                        } else if starfox_go_busy_wait {
                                            0
                                        } else if mid_bootstrap {
                                            if streak >= 8 && (streak - 8).is_multiple_of(8) {
                                                base_steps.saturating_mul(4)
                                            } else {
                                                0
                                            }
                                        } else if is_wram_poll {
                                            if streak >= 8 {
                                                base_steps
                                            } else {
                                                0
                                            }
                                        } else if streak >= 8 && (streak - 8).is_multiple_of(16) {
                                            base_steps
                                        } else {
                                            0
                                        };
                                        let frame = self.ppu.get_frame();
                                        let initial_sfr_low = gsu.observed_sfr_low();
                                        let starfox_blocking_late_wait_assist =
                                            Self::starfox_blocking_late_wait_assist_enabled();
                                        let exact_starfox_wait_stop_assist = starfox_exact_late_wait
                                            && starfox_blocking_late_wait_assist
                                            && !early_bootstrap
                                            && !disable_starfox_late_wait_assist;
                                        let starfox_exact_wait_frame_unseen =
                                            self.starfox_exact_wait_assist_frame != frame;
                                        let exact_starfox_wait_frame_assist =
                                            exact_starfox_wait_stop_assist
                                                && starfox_exact_wait_frame_unseen;
                                        let run_until_delay_exit =
                                            if disable_generic_status_poll_assist {
                                                None
                                            } else if starfox_late_parser_loop
                                                && late_parser_budget_override.is_some()
                                            {
                                                None
                                            } else if starfox_cached_delay_loop
                                                && late_starfox_wait_full_assist
                                            {
                                                Some(base_steps.saturating_mul(32_768))
                                            } else {
                                                None
                                            };
                                        let run_until_sfr_change = if disable_all_status_poll_assist
                                        {
                                            None
                                        } else if starfox_exact_late_wait
                                            && disable_starfox_late_wait_assist
                                        {
                                            None
                                        } else if starfox_exact_late_wait && early_bootstrap {
                                            if starfox_exact_wait_frame_unseen {
                                                self.starfox_exact_wait_assist_frame = frame;
                                                Some(starfox_early_wait_sfr_budget)
                                            } else {
                                                None
                                            }
                                        } else if starfox_live_producer_wait
                                            && starfox_blocking_late_wait_assist
                                        {
                                            if exact_starfox_wait_frame_assist {
                                                Some(starfox_live_producer_budget)
                                            } else {
                                                None
                                            }
                                        } else if starfox_exact_late_wait
                                            && starfox_blocking_late_wait_assist
                                        {
                                            Some(base_steps.saturating_mul(65_536))
                                        } else {
                                            None
                                        };
                                        let run_until_stop = if (disable_generic_status_poll_assist
                                            && !exact_starfox_wait_stop_assist)
                                            || disable_status_poll_run_until_stop
                                        {
                                            None
                                        } else if starfox_late_parser_loop
                                            && late_parser_budget_override.is_some()
                                        {
                                            None
                                        } else if starfox_exact_late_wait
                                            && disable_starfox_late_wait_assist
                                        {
                                            None
                                        } else if starfox_exact_late_wait
                                            && starfox_blocking_late_wait_assist
                                        {
                                            Some(base_steps.saturating_mul(65_536))
                                        } else if starfox_exact_late_wait {
                                            None
                                        } else if starfox_go_busy_wait {
                                            None
                                        } else if mid_bootstrap {
                                            if streak >= 64 && streak.is_multiple_of(64) {
                                                Some(base_steps.saturating_mul(64))
                                            } else {
                                                None
                                            }
                                        } else if !early_bootstrap && is_wram_poll {
                                            if streak >= 32 && streak.is_multiple_of(32) {
                                                Some(base_steps.saturating_mul(64))
                                            } else {
                                                None
                                            }
                                        } else if streak >= 64 && streak.is_multiple_of(64) {
                                            Some(base_steps.saturating_mul(512))
                                        } else {
                                            None
                                        };
                                        let scanline = self.ppu.scanline;
                                        let cycle = self.ppu.get_cycle();
                                        let cpu_pc = self.last_cpu_pc;
                                        let mapper_type = self.mapper_type;
                                        Self::trace_starfox_status_poll(
                                            frame,
                                            scanline,
                                            cycle,
                                            cpu_pc,
                                            mapper_type,
                                            poll_pc,
                                            streak,
                                            is_wram_poll,
                                            early_bootstrap,
                                            starfox_cached_delay_loop,
                                            catch_up_steps,
                                            run_until_delay_exit
                                                .or(run_until_sfr_change)
                                                .or(run_until_stop),
                                        );
                                        if catch_up_steps != 0 {
                                            let rom = &self.rom as *const Vec<u8>;
                                            unsafe {
                                                gsu.run_status_poll_catchup_steps(
                                                    &*rom,
                                                    catch_up_steps,
                                                );
                                            }
                                        }
                                        if let Some(max_steps) = run_until_delay_exit {
                                            let rom = &self.rom as *const Vec<u8>;
                                            unsafe {
                                                gsu.run_status_poll_until_starfox_cached_delay_loop_exit(
                                                    &*rom,
                                                    max_steps,
                                                );
                                            }
                                        }
                                        if let Some(max_steps) = run_until_sfr_change {
                                            let rom = &self.rom as *const Vec<u8>;
                                            unsafe {
                                                if starfox_live_producer_wait {
                                                    gsu.run_status_poll_until_go_clears_in_starfox_live_producer_loop(
                                                        &*rom,
                                                        max_steps,
                                                    );
                                                } else {
                                                    gsu.run_status_poll_until_sfr_low_mask_changes(
                                                        &*rom,
                                                        initial_sfr_low,
                                                        0x20,
                                                        max_steps,
                                                    );
                                                }
                                            }
                                        }
                                        if let Some(max_steps) = run_until_stop {
                                            let rom = &self.rom as *const Vec<u8>;
                                            unsafe {
                                                if late_starfox_wait {
                                                    gsu.run_status_poll_until_stop_with_starfox_late_wait_assist(
                                                        &*rom,
                                                        max_steps,
                                                    );
                                                } else {
                                                    gsu.run_status_poll_until_stop(
                                                        &*rom, max_steps,
                                                    );
                                                }
                                            }
                                        }
                                    } else {
                                        self.superfx_status_poll_pc = 0;
                                        self.superfx_status_poll_streak = 0;
                                    }
                                    let value = gsu.read_register(reg_offset, self.mdr);
                                    if matches!(reg_offset, 0x3030 | 0x3031) {
                                        self.trace_starfox_boot_io("R", reg_offset as u32, value);
                                    }
                                    value
                                }
                                0x3100..=0x32FF => gsu.cache_read(offset),
                                _ => self.mdr,
                            }
                        } else {
                            self.mdr
                        }
                    }
                    // Expansion / coprocessor area
                    0x2184..=0x21FF => self.read_expansion(addr),
                    0x2200..=0x3FFF => self.read_expansion(addr),
                    // Controller/IO registers
                    0x4000..=0x42FF => self.read_io_register(offset),
                    // DMA registers
                    0x4300..=0x43FF => self.dma_controller.read(offset),
                    // SPC7110 registers ($4800-$484F)
                    0x4800..=0x484F if self.spc7110.is_some() => {
                        let rom = &self.rom as *const Vec<u8>;
                        self.spc7110
                            .as_mut()
                            .unwrap()
                            .read_register(offset, unsafe { &*rom })
                    }
                    // S-DD1 registers ($4800-$4807)
                    0x4800..=0x4807 if self.sdd1.is_some() => {
                        self.sdd1.as_ref().unwrap().read_register(offset)
                    }
                    // Expansion / coprocessor registers
                    0x4400..=0x5FFF => self.read_expansion(addr),
                    // Cartridge expansion
                    0x6000..=0x7FFF => {
                        if self.is_superfx_active() {
                            if let Some(ref gsu) = self.superfx {
                                if gsu.cpu_has_ram_access() {
                                    return gsu
                                        .game_ram_read_linear(gsu.game_ram_window_addr(offset));
                                }
                                return self.mdr;
                            }
                        }
                        // DSP-1: banks $00-$1F/$80-$9F route $6000-$7FFF to DSP-1
                        // HiROM: boundary $7000 (DR at $6000-$6FFF, SR at $7000-$7FFF)
                        // LoROM: same mapping for SHVC-2A0N-01 PCB (Pilotwings)
                        if let Some(ref mut dsp) = self.dsp1 {
                            if bank <= 0x1F || (0x80..=0x9F).contains(&bank) {
                                if std::env::var_os("TRACE_DSP1_IO").is_some() {
                                    use std::sync::atomic::{AtomicU32, Ordering};
                                    static CNT: AtomicU32 = AtomicU32::new(0);
                                    let n = CNT.fetch_add(1, Ordering::Relaxed);
                                    if n < 200 {
                                        let reg = if offset < 0x7000 { "DR" } else { "SR" };
                                        eprintln!(
                                            "[DSP1-IO] READ {} {:02X}:{:04X} PC={:06X} f={}",
                                            reg,
                                            bank,
                                            offset,
                                            self.last_cpu_pc,
                                            self.ppu.get_frame()
                                        );
                                    }
                                }
                                return if offset < 0x7000 {
                                    dsp.read_dr()
                                } else {
                                    dsp.read_sr()
                                };
                            }
                        }
                        // OBC-1 register trace
                        if offset >= 0x7FF0 && std::env::var_os("TRACE_OBC1").is_some() {
                            use std::sync::atomic::{AtomicU32, Ordering};
                            static CNT: AtomicU32 = AtomicU32::new(0);
                            let n = CNT.fetch_add(1, Ordering::Relaxed);
                            if n < 500 {
                                eprintln!(
                                    "[OBC1-R] {:02X}:{:04X} PC={:06X} f={}",
                                    bank,
                                    offset,
                                    self.last_cpu_pc,
                                    self.ppu.get_frame()
                                );
                            }
                        }
                        if let Some(ref mapper) = self.mapper {
                            let v = mapper.read_sram_region(
                                &self.sram,
                                self.sram_size,
                                bank as u8,
                                offset,
                            );
                            trace_sram("R", bank, offset, 0, v);
                            v
                        } else {
                            // SA-1/DQ3/SPC7110: special handling
                            match self.mapper_type {
                                crate::cartridge::MapperType::DragonQuest3 => {
                                    if let Some(idx) = self.sa1_bwram_addr(offset) {
                                        self.sa1_bwram[idx]
                                    } else {
                                        0xFF
                                    }
                                }
                                crate::cartridge::MapperType::Spc7110 => {
                                    if self.sram_size > 0 {
                                        let idx = (offset - 0x6000) as usize % self.sram_size;
                                        let v = self.sram[idx];
                                        if std::env::var_os("TRACE_SPC7110").is_some() {
                                            use std::sync::atomic::{AtomicU32, Ordering};
                                            static CNT: AtomicU32 = AtomicU32::new(0);
                                            let n = CNT.fetch_add(1, Ordering::Relaxed);
                                            if n < 200 {
                                                println!("[SPC7110-SRAM] R {:02X}:{:04X} idx={:04X} -> {:02X} f={}", bank, offset, idx, v, self.ppu.get_frame());
                                            }
                                        }
                                        v
                                    } else {
                                        0x00 // ares returns 0x00 when SRAM disabled
                                    }
                                }
                                _ => 0xFF,
                            }
                        }
                    }
                    // ROM area
                    0x8000..=0xFFFF => {
                        // DSP-3 LoROM: banks $20-$3F/$A0-$BF map $8000-$BFFF=DR, $C000-$FFFF=SR.
                        if let Some(ref mut dsp) = self.dsp3 {
                            if (0x20..=0x3F).contains(&bank) || (0xA0..=0xBF).contains(&bank) {
                                return if offset < 0xC000 {
                                    dsp.read_dr()
                                } else {
                                    dsp.read_sr()
                                };
                            }
                        }
                        // DSP-1 Type A: banks $20-$3F/$A0-$BF map $8000-$BFFF=DR, $C000-$FFFF=SR
                        if let Some(ref mut dsp) = self.dsp1 {
                            if dsp.mapping == crate::cartridge::dsp1::Dsp1Mapping::TypeA
                                && ((0x20..=0x3F).contains(&bank) || (0xA0..=0xBF).contains(&bank))
                            {
                                return if offset < 0xC000 {
                                    dsp.read_dr()
                                } else {
                                    dsp.read_sr()
                                };
                            }
                        }
                        self.read_rom_lohi(bank, offset)
                    }
                }
            }
            // ROM banks 40-7D (HiROM/ExHiROM lower half)
            0x40..=0x7D => {
                if let Some(ref mapper) = self.mapper {
                    mapper.read_bank_40_7d(
                        &self.rom,
                        &self.sram,
                        self.rom_size,
                        self.sram_size,
                        bank as u8,
                        offset,
                    )
                } else {
                    // SA-1/DQ3/SPC7110: special handling
                    match self.mapper_type {
                        crate::cartridge::MapperType::DragonQuest3 => {
                            self.read_dq3_rom(bank, offset)
                        }
                        crate::cartridge::MapperType::Spc7110 => {
                            if bank == 0x50 {
                                // Bank $50: SPC7110 decompression data port
                                // Any read from $50:xxxx returns the next decompressed byte
                                // (equivalent to reading $4800)
                                let rom = &self.rom as *const Vec<u8>;
                                self.spc7110
                                    .as_mut()
                                    .unwrap()
                                    .read_register(0x4800, unsafe { &*rom })
                            } else {
                                // $40-$4F/$51-$7D: program ROM (HiROM style)
                                let rom_addr = (bank as usize) * 0x10000 + (offset as usize);
                                if self.rom_size > 0 {
                                    self.rom[rom_addr % self.rom_size]
                                } else {
                                    0xFF
                                }
                            }
                        }
                        crate::cartridge::MapperType::SuperFx => {
                            if let Some(ref gsu) = self.superfx {
                                match bank {
                                    0x40..=0x5F => {
                                        if !gsu.cpu_has_rom_access() {
                                            crate::cartridge::superfx::SuperFx::illegal_rom_read_value(offset)
                                        } else if let Some(rom_addr) =
                                            crate::cartridge::superfx::SuperFx::cpu_rom_addr(
                                                bank as u8, offset,
                                            )
                                        {
                                            if self.rom_size == 0 {
                                                0xFF
                                            } else {
                                                self.rom[rom_addr % self.rom_size]
                                            }
                                        } else {
                                            0xFF
                                        }
                                    }
                                    0x70..=0x71 => {
                                        if !gsu.cpu_has_ram_access() {
                                            if std::env::var_os("TRACE_RAM_BLOCK").is_some() {
                                                use std::sync::atomic::{AtomicU32, Ordering};
                                                static CNT: AtomicU32 = AtomicU32::new(0);
                                                let n = CNT.fetch_add(1, Ordering::Relaxed);
                                                if n < 32 {
                                                    let f = self.ppu.get_frame();
                                                    let sl = self.ppu.scanline;
                                                    eprintln!(
                                                        "[RAM-BLOCK] f={} sl={} bank={:02X} off={:04X} mdr={:02X}",
                                                        f, sl, bank, offset, self.mdr
                                                    );
                                                }
                                            }
                                            self.mdr
                                        } else {
                                            gsu.game_ram_read_linear(
                                                ((bank as usize - 0x70) << 16) | offset as usize,
                                            )
                                        }
                                    }
                                    0x7C..=0x7D => {
                                        if self.sram_size == 0 {
                                            0xFF
                                        } else {
                                            let idx = (((bank as usize - 0x7C) << 16)
                                                | offset as usize)
                                                % self.sram_size;
                                            self.sram[idx]
                                        }
                                    }
                                    _ => 0xFF,
                                }
                            } else {
                                0xFF
                            }
                        }
                        _ => 0xFF,
                    }
                }
            }
            // Extended WRAM banks
            0x7E..=0x7F => {
                // Optionally mirror 7E/7F to the same 64KB (useful for some test ROMs)
                let wram_addr = if self.wram_64k_mirror {
                    (offset as usize) & 0xFFFF
                } else {
                    ((bank - 0x7E) as usize) * 0x10000 + (offset as usize)
                };
                // Debug: trace key handshake variables in WRAM (NMI paths)
                if self.trace_nmi_wram {
                    use std::sync::atomic::{AtomicU32, Ordering};
                    static READ_COUNT: AtomicU32 = AtomicU32::new(0);
                    if let Some(label) = match wram_addr {
                        0x07DE => Some("00:07DE"),
                        0x07E0 => Some("00:07E0"),
                        0x07E4 => Some("00:07E4"),
                        0x07F6 => Some("00:07F6"),
                        0x0FDE => Some("7E:0FDE"),
                        0x0FE0 => Some("7E:0FE0"),
                        0x0FE4 => Some("7E:0FE4"),
                        0x0FF6 => Some("7E:0FF6"),
                        0x1FDE => Some("7F:0FDE"),
                        0x1FE0 => Some("7F:0FE0"),
                        0x1FE4 => Some("7F:0FE4"),
                        0x1FF6 => Some("7F:0FF6"),
                        _ => None,
                    } {
                        let idx = READ_COUNT.fetch_add(1, Ordering::Relaxed);
                        if idx < 64 {
                            let v = if wram_addr < self.wram.len() {
                                self.wram[wram_addr]
                            } else {
                                0xFF
                            };
                            println!(
                                "[WRAM TRACE READ {}] val=0x{:02X} bank={:02X} off={:04X}",
                                label, v, bank, offset
                            );
                        }
                    }
                }
                if wram_addr < self.wram.len() {
                    self.wram[wram_addr]
                } else {
                    0xFF
                }
            }
            // ROM mirror banks (HiROM/ExHiROM upper half)
            0xC0..=0xFF => {
                // S-DD1: override $C0-$FF with configurable page mapping (before standard mapper)
                if let Some(ref sdd) = self.sdd1 {
                    sdd.read_bank_c0_ff(bank as u8, offset, &self.rom, self.rom_size)
                } else if let Some(ref mapper) = self.mapper {
                    mapper.read_bank_c0_ff(
                        &self.rom,
                        &self.sram,
                        self.rom_size,
                        self.sram_size,
                        bank as u8,
                        offset,
                    )
                } else {
                    // SA-1/DQ3/SPC7110: special handling
                    match self.mapper_type {
                        crate::cartridge::MapperType::DragonQuest3 => {
                            self.read_dq3_rom(bank, offset)
                        }
                        crate::cartridge::MapperType::Spc7110 => {
                            if let Some(ref spc) = self.spc7110 {
                                spc.read_bank_c0_ff(bank as u8, offset, &self.rom, self.rom_size)
                            } else {
                                0xFF
                            }
                        }
                        crate::cartridge::MapperType::SuperFx => {
                            if self.superfx.is_some() {
                                if let Some(rom_addr) =
                                    crate::cartridge::superfx::SuperFx::cpu_rom_addr(
                                        bank as u8, offset,
                                    )
                                {
                                    if self.rom_size == 0 {
                                        0xFF
                                    } else {
                                        self.rom[rom_addr % self.rom_size]
                                    }
                                } else {
                                    0xFF
                                }
                            } else {
                                0xFF
                            }
                        }
                        _ => 0xFF,
                    }
                }
            }
            // Other banks - open bus
            _ => 0xFF,
        };

        self.mdr = value;
        value
    }
}
