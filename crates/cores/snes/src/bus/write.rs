use super::{
    debug::{trace_cpu_sfx_ram_callers_enabled, trace_sram},
    Bus,
};
use crate::cartridge::mapper::MemoryMapper;
use std::sync::OnceLock;

impl Bus {
    pub fn write_u8(&mut self, addr: u32, value: u8) {
        let bank = (addr >> 16) & 0xFF;
        let offset = (addr & 0xFFFF) as u16;

        // Debug: watch a specific address write (S-CPU side)
        if let Some(watch) = crate::debug_flags::watch_addr_write() {
            if watch == addr {
                let sl = self.ppu.scanline;
                let cyc = self.ppu.get_cycle();
                println!(
                    "[watchW] {:02X}:{:04X} <= {:02X} PC={:06X} sl={} cyc={} frame={}",
                    bank, offset, value, self.last_cpu_pc, sl, cyc, self.ppu.frame
                );
            }
        }
        // Debug: watch/force WRAM writes (banks 7E/7F)
        if bank == 0x7E || bank == 0x7F {
            if let Some(watch) = crate::debug_flags::watch_wram_write() {
                if watch == addr {
                    println!(
                        "[WRAM-WATCH] PC={:06X} {:02X}:{:04X} <= {:02X}",
                        self.last_cpu_pc, bank, offset, value
                    );
                }
            }
            if let Some((watch, forced)) = crate::debug_flags::watch_wram_write_force() {
                if watch == addr {
                    println!(
                        "[WRAM-FORCE] PC={:06X} {:02X}:{:04X} {:02X} -> {:02X}",
                        self.last_cpu_pc, bank, offset, value, forced
                    );
                    // 監視アドレス以外でも、強制書き込みモードでは値を差し替える
                    self.wram[offset as usize] = forced;
                    return;
                }
            }
        }

        if ((0x0100..=0x01FF).contains(&offset) || offset == 0xFFFF)
            && crate::debug_flags::trace_stack_write()
        {
            println!(
                "[STACK-WRITE] PC={:06X} wrote {:02X} to {:02X}:{:04X}",
                self.last_cpu_pc, value, bank, offset
            );
        }

        // SA-1 BW-RAM mapping for S-CPU in banks $40-$4F and $60-$6F
        if self.is_sa1_active() && ((0x40..=0x4F).contains(&bank) || (0x60..=0x6F).contains(&bank))
        {
            if !self.sa1_bwram.is_empty() {
                let base = if (0x60..=0x6F).contains(&bank) {
                    (bank - 0x60) as usize
                } else {
                    (bank - 0x40) as usize
                };
                let idx = (base << 16) | offset as usize;
                let actual = idx % self.sa1_bwram.len();
                if self.bwram_write_allowed_scpu(actual) {
                    self.sa1_bwram[actual] = value;
                }
            }
            return;
        }

        match bank {
            // System area banks (mirror in 80-BF)
            0x00..=0x3F | 0x80..=0xBF => {
                match offset {
                    // Stack area (0x0100-0x01FF)
                    0x0100..=0x01FF => {
                        // Debug stack corruption - trace suspicious writes
                        if crate::debug_flags::debug_stack_trace() {
                            static mut STACK_TRACE_COUNT: u32 = 0;
                            unsafe {
                                STACK_TRACE_COUNT += 1;
                                if STACK_TRACE_COUNT <= 50 || value == 0xFF {
                                    println!(
                                        "🔍 STACK WRITE #{}: addr=0x{:04X} value=0x{:02X} (suspect={})",
                                        STACK_TRACE_COUNT,
                                        offset,
                                        value,
                                        if value == 0xFF { "YES" } else { "no" }
                                    );
                                }
                            }
                        }
                        self.wram[offset as usize] = value;
                    }
                    // Mirror WRAM in first 8KB (excluding stack area already handled above)
                    0x0000..=0x00FF | 0x0200..=0x1FFF => {
                        if let Some(watch) = crate::debug_flags::watch_wram_write() {
                            let full = (bank << 16) | offset as u32;
                            // Match either exact addr or WRAM mirror (bank 00-3F maps to 7E)
                            let watch_off = watch & 0xFFFF;
                            if full == watch
                                || ((0x7E0000..0x7F0000).contains(&watch)
                                    && offset == watch_off as u16)
                            {
                                println!(
                                    "[WRAM-WATCH] PC={:06X} {:02X}:{:04X} <= {:02X}",
                                    self.last_cpu_pc, bank, offset, value
                                );
                            }
                        }
                        if crate::debug_flags::trace_burnin_zp16()
                            && matches!(offset, 0x0016 | 0x0017 | 0x001F)
                        {
                            println!(
                                "[BURNIN-ZP] PC={:06X} ${:04X} <- {:02X} frame={} sl={} cyc={}",
                                self.last_cpu_pc,
                                offset,
                                value,
                                self.ppu.get_frame(),
                                self.ppu.scanline,
                                self.ppu.get_cycle()
                            );
                        }
                        if offset < 0x0010 && crate::debug_flags::trace_zp() {
                            use std::sync::atomic::{AtomicU32, Ordering};
                            static COUNT: AtomicU32 = AtomicU32::new(0);
                            let n = COUNT.fetch_add(1, Ordering::Relaxed);
                            if n < 64 {
                                println!(
                                    "[ZP-W] PC={:06X} addr=0x{:04X} <= {:02X}",
                                    self.last_cpu_pc, offset, value
                                );
                            }
                        }
                        self.wram[offset as usize] = value;
                    }
                    // $2000-$20FF is unmapped on real hardware (writes ignored)
                    0x2000..=0x20FF => {}
                    0x6000..=0x7FFF if self.is_sa1_active() => {
                        if let Some(idx) = self.sa1_bwram_addr(offset) {
                            if self.bwram_write_allowed_scpu(idx) {
                                self.sa1_bwram[idx] = value;
                            }
                            if crate::debug_flags::trace_bwram_sys() {
                                use std::sync::atomic::{AtomicU32, Ordering};
                                static COUNT: AtomicU32 = AtomicU32::new(0);
                                let n = COUNT.fetch_add(1, Ordering::Relaxed);
                                if n < 32 {
                                    println!(
                                        "BWRAM SYS W bank={:02X} off={:04X} idx=0x{:05X} val={:02X}",
                                        bank, offset, idx, value
                                    );
                                }
                            }
                        }
                    }
                    // PPU registers (no overrides)
                    0x2100..=0x213F => {
                        if crate::debug_flags::trace_burnin_v224() {
                            let pc16 = (self.last_cpu_pc & 0xFFFF) as u16;
                            if (0x97D0..=0x98FF).contains(&pc16) && offset == 0x2133 {
                                println!(
                                    "[BURNIN-V224][PPU-W] PC={:06X} ${:04X} <- {:02X} frame={} sl={} cyc={} vblank={} vis_h={}",
                                    self.last_cpu_pc,
                                    offset,
                                    value,
                                    self.ppu.get_frame(),
                                    self.ppu.scanline,
                                    self.ppu.get_cycle(),
                                    self.ppu.is_vblank() as u8,
                                    self.ppu.get_visible_height()
                                );
                            }
                        }
                        let ppu_reg = offset & 0xFF;
                        // burn-in-test.sfc diagnostics: include S-CPU PC for VRAM data port writes
                        // that touch the DMA MEMORY test region (VMADD 0x5000..0x57FF).
                        if matches!(ppu_reg, 0x18 | 0x19) {
                            let trace_dmamem = crate::debug_flags::trace_burnin_dma_memory();
                            let trace_status = crate::debug_flags::trace_burnin_status();
                            let trace_apu_status = crate::debug_flags::trace_burnin_apu_status();
                            if trace_dmamem || trace_status || trace_apu_status {
                                use std::sync::atomic::{AtomicU32, Ordering};
                                let (vmadd, _inc, vmain) = self.ppu.dbg_vram_regs();

                                // burn-in-test.sfc diagnostics: include S-CPU PC for VRAM data port writes
                                // that touch the DMA MEMORY test region (VMADD 0x5000..0x57FF).
                                // Only count/log writes that actually land in the interesting range;
                                // otherwise early VRAM traffic (font/tiles) exhausts the counter.
                                if trace_dmamem && (0x5000..0x5800).contains(&vmadd) {
                                    static CNT: AtomicU32 = AtomicU32::new(0);
                                    let n = CNT.fetch_add(1, Ordering::Relaxed);
                                    if n < 256 {
                                        println!(
	                                            "[BURNIN-VRAM-PC] PC={:06X} ${:04X} <- {:02X} VMADD={:04X} VMAIN={:02X}",
	                                            self.last_cpu_pc,
	                                            offset,
	                                            value,
	                                            vmadd,
	                                            vmain
	                                        );
                                    }
                                }

                                // Focused logging for PASS/FAIL column updates (opt-in).
                                if trace_status && (0x50F0..0x5200).contains(&vmadd) {
                                    let ch = value as char;
                                    let printable = ch.is_ascii_graphic() || ch == ' ';
                                    println!(
	                                        "[BURNIN-STATUS] PC={:06X} ${:04X} <- {:02X}{} VMADD={:04X} VMAIN={:02X}",
	                                        self.last_cpu_pc,
	                                        offset,
	                                        value,
	                                        if printable {
	                                            format!(" ('{}')", ch)
	                                        } else {
	                                            String::new()
	                                        },
	                                        vmadd,
	                                        vmain
	                                    );
                                }

                                // Focused logging for the APU status row (menu 5 results).
                                // The PASS/FAIL column for the bottom rows lives around VMADD ~= $52D0.
                                if trace_apu_status && (0x52C0..=0x52FF).contains(&vmadd) {
                                    println!(
	                                        "[BURNIN-APU-STATUS] PC={:06X} ${:04X} <- {:02X} VMADD={:04X} VMAIN={:02X}",
	                                        self.last_cpu_pc, offset, value, vmadd, vmain
	                                    );
                                }
                            }
                        }
                        self.ppu.write(ppu_reg, value);
                        self.trace_ppu_reg_write(ppu_reg as u8, value);
                        if matches!(ppu_reg, 0x00 | 0x05 | 0x07..=0x0C | 0x15..=0x19 | 0x2C) {
                            self.trace_starfox_boot_io("W", 0x2100 + ppu_reg as u32, value);
                        }
                        if matches!(ppu_reg, 0x00 | 0x15 | 0x16 | 0x17)
                            && crate::debug_flags::trace_burnin_dma_memory()
                        {
                            let pc16 = (self.last_cpu_pc & 0xFFFF) as u16;
                            if (0xAE80..=0xAEEF).contains(&pc16) {
                                use std::sync::atomic::{AtomicU32, Ordering};
                                static CNT: AtomicU32 = AtomicU32::new(0);
                                let n = CNT.fetch_add(1, Ordering::Relaxed);
                                if n < 128 {
                                    let (vmadd, inc, vmain) = self.ppu.dbg_vram_regs();
                                    println!(
                                        "[BURNIN-DMAMEM][PPU-W] PC={:06X} ${:04X} <- {:02X} VMADD={:04X} VMAIN={:02X} inc={}",
                                        self.last_cpu_pc, offset, value, vmadd, vmain, inc
                                    );
                                }
                            }
                        }
                    }
                    0x2200..=0x23FF if self.is_sa1_active() => {
                        if crate::debug_flags::trace_sa1_reg() {
                            println!(
                                "SA1 REG W (S-CPU) {:02X}:{:04X} = {:02X}",
                                bank, offset, value
                            );
                        }
                        self.sa1.write_register_scpu(offset - 0x2200, value);
                    }
                    // APU registers
                    0x2140..=0x217F => {
                        let apu_inline_cpu = self.take_apu_inline_cpu_cycles_for_current_access();
                        if offset <= 0x2143 {
                            self.trace_starfox_boot_io("W", offset as u32, value);
                        }
                        // burn-in-test.sfc APU test: trace the CPU command sequence (opt-in, low volume).
                        if crate::debug_flags::trace_burnin_apu_cpu()
                            && offset <= 0x2143
                            && (0x008600..=0x008700).contains(&self.last_cpu_pc)
                        {
                            let apu_cycles =
                                self.apu.lock().map(|apu| apu.total_smp_cycles).unwrap_or(0);
                            println!(
                                "[BURNIN-APU-CPU] PC={:06X} ${:04X} <- {:02X} frame={} sl={} cyc={} apu_cycles={}",
                                self.last_cpu_pc,
                                offset,
                                value,
                                self.ppu.get_frame(),
                                self.ppu.scanline,
                                self.ppu.get_cycle(),
                                apu_cycles
                            );
                        }
                        // burn-in-test.sfc: broader APU port write trace with frame correlation (opt-in).
                        if crate::debug_flags::trace_burnin_apu_writes()
                            && offset <= 0x2143
                            && (150..=420).contains(&self.ppu.get_frame())
                        {
                            use std::sync::atomic::{AtomicU32, Ordering};
                            static CNT: AtomicU32 = AtomicU32::new(0);
                            let n = CNT.fetch_add(1, Ordering::Relaxed);
                            if n < 2048 {
                                println!(
                                    "[BURNIN-APU-W] PC={:06X} ${:04X} <- {:02X} frame={} sl={} cyc={}",
                                    self.last_cpu_pc,
                                    offset,
                                    value,
                                    self.ppu.get_frame(),
                                    self.ppu.scanline,
                                    self.ppu.get_cycle()
                                );
                            }
                        }
                        if crate::debug_flags::trace_apu_port_all()
                            || (offset == 0x2140 && crate::debug_flags::trace_apu_port0())
                        {
                            use std::sync::atomic::{AtomicU32, Ordering};
                            static CNT: AtomicU32 = AtomicU32::new(0);
                            let n = CNT.fetch_add(1, Ordering::Relaxed);
                            if n < 512 {
                                println!("[APU-W] ${:04X} <- {:02X}", offset, value);
                            }
                        }
                        // Concise handshake trace (write side)
                        if crate::debug_flags::trace_apu_handshake() && offset <= 0x2143 {
                            use std::sync::atomic::{AtomicU32, Ordering};
                            static CNT: AtomicU32 = AtomicU32::new(0);
                            let n = CNT.fetch_add(1, Ordering::Relaxed);
                            let limit = crate::debug_flags::trace_apu_handshake_limit();
                            if n < limit {
                                if let Ok(apu) = self.apu.lock() {
                                    println!(
                                        "[APU-HS][W] ${:04X} <- {:02X} state={} pc={:06X} frame={} sl={} cyc={}",
                                        offset,
                                        value,
                                        apu.handshake_state_str(),
                                        self.last_cpu_pc,
                                        self.ppu.get_frame(),
                                        self.ppu.scanline,
                                        self.ppu.get_cycle()
                                    );
                                }
                            }
                        }
                        if let Ok(mut apu) = self.apu.lock() {
                            if apu_inline_cpu != 0 {
                                apu.add_cpu_cycles(apu_inline_cpu as u32);
                            }
                            apu.sync_for_port_write();
                            let p = (offset & 0x03) as u8;
                            if crate::debug_flags::trace_apu_port() {
                                use std::sync::atomic::{AtomicU32, Ordering};
                                static COUNT_W: AtomicU32 = AtomicU32::new(0);
                                let n = COUNT_W.fetch_add(1, Ordering::Relaxed);
                                if n < 256 {
                                    println!("[APU] W ${:04X} port{} <- {:02X}", offset, p, value);
                                }
                            }
                            // Trace IPL transfer: log ALL port1 writes with CPU PC
                            if crate::debug_flags::trace_ipl_xfer() {
                                use std::sync::atomic::{AtomicU32, Ordering};
                                if p == 1 {
                                    static P1_CNT: AtomicU32 = AtomicU32::new(0);
                                    let n = P1_CNT.fetch_add(1, Ordering::Relaxed);
                                    if n < 16384 {
                                        eprintln!(
                                            "[IPL-P1] #{:05} val={:02X} cpu_pc={:06X}",
                                            n, value, self.last_cpu_pc
                                        );
                                    }
                                }
                            }
                            // Trace CPU->APU port writes for ToP voice streaming
                            // Skip the IPL upload phase (pc=00F149 incremental transfer)
                            if crate::debug_flags::trace_top_spc_cmd() {
                                use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
                                static CPU_W_CNT: AtomicU32 = AtomicU32::new(0);
                                static LAST_P: [std::sync::atomic::AtomicU8; 4] = [
                                    std::sync::atomic::AtomicU8::new(0),
                                    std::sync::atomic::AtomicU8::new(0),
                                    std::sync::atomic::AtomicU8::new(0),
                                    std::sync::atomic::AtomicU8::new(0),
                                ];
                                static POST_BOOT: AtomicBool = AtomicBool::new(false);
                                let prev = LAST_P[p as usize].swap(value, Ordering::Relaxed);
                                // Detect post-boot: when CPU writes from a non-IPL PC
                                if self.last_cpu_pc != 0x00F149
                                    && self.last_cpu_pc != 0x00F177
                                    && value != 0xCC
                                {
                                    POST_BOOT.store(true, Ordering::Relaxed);
                                }
                                if POST_BOOT.load(Ordering::Relaxed) {
                                    let n = CPU_W_CNT.fetch_add(1, Ordering::Relaxed);
                                    if p == 0 && prev != value && n < 50000 {
                                        eprintln!(
                                            "[CPU-P0] #{} pc={:06X} {:02X}->{:02X} p1={:02X} p2={:02X} p3={:02X}",
                                            n, self.last_cpu_pc, prev, value,
                                            apu.port_latch[1], apu.port_latch[2], apu.port_latch[3]
                                        );
                                    }
                                }
                            }
                            apu.write_port(p, value);
                            if offset == 0x2141
                                && self.mapper_type == crate::cartridge::MapperType::SuperFx
                                && self.ppu.get_frame() < 180
                                && Self::is_starfox_apu_upload_write_high_pc(self.last_cpu_pc)
                                && std::env::var_os("DISABLE_STARFOX_APU_ECHO_WAIT_ASSIST")
                                    .is_none()
                            {
                                // The 16-bit STA $2140/$2141 pair is complete here, so it is
                                // safe to flush the deferred CPU-time debt without exposing the
                                // half-written port state that `sync_for_port_write()` avoids.
                                apu.sync();
                                if apu.read_port(0) != apu.port_latch[0] {
                                    apu.run_until_cpu_port_matches_latch(
                                        0,
                                        Self::apu_echo_wait_budget(),
                                    );
                                }
                            }
                        }
                        // Optional: treat writes to $2140 as ASCII stream for test ROMs
                        if self.test_apu_print && offset == 0x2140 {
                            let ch = value as char;
                            if ch.is_ascii_graphic() || ch == ' ' || ch == '\n' || ch == '\r' {
                                self.test_apu_buf.push(ch);
                                if ch == '\n' || self.test_apu_buf.len() > 512 {
                                    let line = self.test_apu_buf.replace('\r', "");
                                    println!("[TESTROM] APU: {}", line.trim_end());
                                    let lower = line.to_ascii_lowercase();
                                    if lower.contains("passed") {
                                        println!("[TESTROM] PASS");
                                        crate::shutdown::request_quit();
                                    } else if lower.contains("fail") || lower.contains("failed") {
                                        println!("[TESTROM] FAIL");
                                        crate::shutdown::request_quit();
                                    }
                                    self.test_apu_buf.clear();
                                }
                            }
                        }
                    }
                    // WRAM access port
                    0x2180 => {
                        let addr = self.wram_address as usize;
                        if addr < self.wram.len() {
                            let abs = 0x7E0000u32 + addr as u32;
                            self.trace_wram_abs_write("port=$2180", abs, value);
                            if (0x0100..=0x01FF).contains(&(addr as u32))
                                && crate::debug_flags::trace_wram_stack_dma()
                            {
                                println!(
                                    "[WRAM-STACK] PC={:06X} addr=0x{:05X} val=0x{:02X}",
                                    self.last_cpu_pc, addr, value
                                );
                            }
                            self.wram[addr] = value;
                            // WMADD ($2181-2183) is a 17-bit address; auto-increment carries across bit16.
                            self.wram_address = (self.wram_address + 1) & 0x1FFFF;
                            if crate::debug_flags::trace_wram_addr() {
                                static TRACE_WRAM_CNT: OnceLock<std::sync::atomic::AtomicU32> =
                                    OnceLock::new();
                                let n = TRACE_WRAM_CNT
                                    .get_or_init(|| std::sync::atomic::AtomicU32::new(0))
                                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                                if n < 32 {
                                    println!(
                                        "[WRAM PORT] W addr=0x{:05X} val=0x{:02X}",
                                        addr, value
                                    );
                                }
                            }
                        }
                    }
                    // WRAM Address registers
                    0x2181 => {
                        self.wram_address = (self.wram_address & 0xFFFF00) | (value as u32);
                        if crate::debug_flags::trace_wram_addr() {
                            println!(
                                "[WRAM ADR] write 2181 = {:02X} -> addr=0x{:05X}",
                                value, self.wram_address
                            );
                        }
                    }
                    0x2182 => {
                        self.wram_address = (self.wram_address & 0xFF00FF) | ((value as u32) << 8);
                        if crate::debug_flags::trace_wram_addr() {
                            println!(
                                "[WRAM ADR] write 2182 = {:02X} -> addr=0x{:05X}",
                                value, self.wram_address
                            );
                        }
                    }
                    0x2183 => {
                        self.wram_address =
                            (self.wram_address & 0x00FFFF) | (((value & 0x01) as u32) << 16);
                        if crate::debug_flags::trace_wram_addr() {
                            println!(
                                "[WRAM ADR] write 2183 = {:02X} -> addr=0x{:05X}",
                                value, self.wram_address
                            );
                        }
                    }
                    // Expansion / coprocessor area
                    0x2184..=0x21FF => self.write_expansion(addr, value),
                    // SA-1 I-RAM window for S-CPU
                    0x3000..=0x37FF if self.is_sa1_active() => {
                        let idx = (offset - 0x3000) as usize;
                        if idx < self.sa1_iram.len() && self.iram_write_allowed_scpu(offset) {
                            self.sa1_iram[idx] = value;
                        }
                    }
                    0x3000..=0x34FF if self.is_superfx_active() => {
                        if let Some(ref mut gsu) = self.superfx {
                            match offset {
                                0x3000..=0x34FF => {
                                    let reg_offset = if (0x3300..=0x34FF).contains(&offset) {
                                        0x3000 + ((offset - 0x3300) & 0x00FF)
                                    } else {
                                        offset
                                    };
                                    let rom = &self.rom as *const Vec<u8>;
                                    gsu.write_register_with_rom(reg_offset, value, unsafe {
                                        &*rom
                                    });
                                    if (0x3100..=0x32FF).contains(&reg_offset) {
                                        self.trace_superfx_cache_upload(reg_offset, value);
                                    }
                                    let trace_all_superfx_regs =
                                        std::env::var_os("TRACE_STARFOX_BOOT_SUPERFX_ALL")
                                            .is_some();
                                    if (trace_all_superfx_regs
                                        && (0x3000..=0x303A).contains(&reg_offset))
                                        || matches!(reg_offset, 0x3030 | 0x3031 | 0x303A)
                                    {
                                        self.trace_starfox_boot_io("W", reg_offset as u32, value);
                                    }
                                }
                                0x3100..=0x32FF => gsu.cache_write(offset, value),
                                _ => {}
                            }
                        }
                    }
                    0x2200..=0x3FFF => self.write_expansion(addr, value),
                    // Controller/IO registers
                    0x4000..=0x42FF => self.write_io_register(offset, value),
                    // DMA registers
                    0x4300..=0x43FF => {
                        if crate::debug_flags::trace_dma_reg_pc() {
                            let pc = self.last_cpu_pc;
                            println!(
                                "[DMA-REG-PC] PC={:06X} W ${:04X} val={:02X}",
                                pc, offset, value
                            );
                        }
                        if crate::debug_flags::trace_dma_addr() {
                            println!(
                                "[DMA-REG-W] bank={:02X} addr={:04X} value=0x{:02X}",
                                bank, offset, value
                            );
                        }
                        // S-DD1: snoop DMA register writes to track per-channel addr/size
                        if let Some(ref mut sdd) = self.sdd1 {
                            sdd.snoop_dma_write(offset, value);
                        }
                        self.dma_controller.write(offset, value);
                        self.dma_reg_writes = self.dma_reg_writes.saturating_add(1);
                    }
                    // SPC7110 registers ($4800-$484F)
                    0x4800..=0x484F if self.spc7110.is_some() => {
                        let rom = &self.rom as *const Vec<u8>;
                        self.spc7110
                            .as_mut()
                            .unwrap()
                            .write_register(offset, value, unsafe { &*rom });
                    }
                    // S-DD1 registers ($4800-$4807)
                    0x4800..=0x4807 if self.sdd1.is_some() => {
                        self.sdd1.as_mut().unwrap().write_register(offset, value);
                    }
                    // Expansion / coprocessor registers
                    0x4400..=0x5FFF => self.write_expansion(addr, value),
                    // Expansion area/unused
                    0x6000..=0x7FFF => {
                        if self.is_superfx_active() {
                            if let Some(ref mut gsu) = self.superfx {
                                if gsu.cpu_has_ram_access() {
                                    let ram_addr = gsu.game_ram_window_addr(offset);
                                    let gram = gsu.game_ram_slice();
                                    let idx = if gram.is_empty() {
                                        0
                                    } else {
                                        ram_addr % gram.len()
                                    };
                                    let old = gram.get(idx).copied().unwrap_or(0xFF);
                                    gsu.game_ram_write_linear(ram_addr, value);
                                    if crate::cartridge::superfx::debug_trace_superfx_ram_addr_matches_for_frame(
                                        idx,
                                        self.ppu.get_frame(),
                                    ) {
                                        let callers = if trace_cpu_sfx_ram_callers_enabled() {
                                            self.recent_cpu_exec_pcs
                                                .iter()
                                                .map(|pc| format!("{:06X}", pc))
                                                .collect::<Vec<_>>()
                                                .join(">")
                                        } else {
                                            String::new()
                                        };
                                        eprintln!(
                                            "[CPU-SFX-RAM-W] {:02X}:{:04X} -> {:05X} {:02X}->{:02X} PC={:06X} A={:04X} X={:04X} Y={:04X} DB={:02X} PB={:02X} P={:02X} f={}{}",
                                            bank,
                                            offset,
                                            idx,
                                            old,
                                            value,
                                            self.last_cpu_pc,
                                            self.last_cpu_a,
                                            self.last_cpu_x,
                                            self.last_cpu_y,
                                            self.last_cpu_db,
                                            self.last_cpu_pb,
                                            self.last_cpu_p,
                                            self.ppu.get_frame(),
                                            if callers.is_empty() {
                                                String::new()
                                            } else {
                                                format!(" callers={}", callers)
                                            }
                                        );
                                    }
                                }
                            }
                            return;
                        }
                        // DSP-1 LoROM: banks $00-$1F/$80-$9F route $6000-$6FFF writes to DR
                        if let Some(ref mut dsp) = self.dsp1 {
                            if bank <= 0x1F || (0x80..=0x9F).contains(&bank) {
                                if offset < 0x7000 {
                                    dsp.write_dr(value);
                                }
                                // Writes to $7000-$7FFF (SR) are ignored
                                return;
                            }
                        }
                        // OBC-1 register trace
                        if offset >= 0x7FF0 && std::env::var_os("TRACE_OBC1").is_some() {
                            use std::sync::atomic::{AtomicU32, Ordering};
                            static CNT: AtomicU32 = AtomicU32::new(0);
                            let n = CNT.fetch_add(1, Ordering::Relaxed);
                            if n < 500 {
                                eprintln!(
                                    "[OBC1-W] {:02X}:{:04X} <- {:02X} PC={:06X} f={}",
                                    bank,
                                    offset,
                                    value,
                                    self.last_cpu_pc,
                                    self.ppu.get_frame()
                                );
                            }
                        }
                        if let Some(ref mapper) = self.mapper {
                            if mapper.write_sram_region(
                                &mut self.sram,
                                self.sram_size,
                                bank as u8,
                                offset,
                                value,
                            ) {
                                self.sram_dirty = true;
                                trace_sram("W", bank, offset, 0, value);
                            }
                        } else {
                            // SA-1/DQ3/SPC7110: special handling
                            if self.mapper_type == crate::cartridge::MapperType::DragonQuest3 {
                                if let Some(idx) = self.sa1_bwram_addr(offset) {
                                    self.sa1_bwram[idx] = value;
                                    self.sram_dirty = true;
                                }
                            } else if self.mapper_type == crate::cartridge::MapperType::Spc7110 {
                                let write_ok = self
                                    .spc7110
                                    .as_ref()
                                    .is_some_and(|s| s.sram_write_enabled());
                                if std::env::var_os("TRACE_SPC7110").is_some() {
                                    println!("[SPC7110-SRAM] W {:02X}:{:04X} <- {:02X} (write_en={}) PC={:06X}", bank, offset, value, write_ok, self.last_cpu_pc);
                                }
                                if write_ok && self.sram_size > 0 {
                                    let idx = (offset - 0x6000) as usize % self.sram_size;
                                    self.sram[idx] = value;
                                    self.sram_dirty = true;
                                }
                            }
                        }
                    }
                    // ROM area - writes ignored (except DSP-1 Type A)
                    0x8000..=0xFFFF => {
                        if let Some(ref mut dsp) = self.dsp3 {
                            if ((0x20..=0x3F).contains(&bank) || (0xA0..=0xBF).contains(&bank))
                                && offset < 0xC000
                            {
                                dsp.write_dr(value);
                                return;
                            }
                        }
                        if let Some(ref mut dsp) = self.dsp1 {
                            if dsp.mapping == crate::cartridge::dsp1::Dsp1Mapping::TypeA
                                && ((0x20..=0x3F).contains(&bank) || (0xA0..=0xBF).contains(&bank))
                                && offset < 0xC000
                            {
                                dsp.write_dr(value);
                            }
                        }
                    }
                }
            }
            // ROM banks 40-7D - writes to SRAM only
            0x40..=0x7D => {
                if let Some(ref mapper) = self.mapper {
                    if mapper.write_bank_40_7d(
                        &mut self.sram,
                        self.sram_size,
                        bank as u8,
                        offset,
                        value,
                    ) {
                        self.sram_dirty = true;
                    }
                } else {
                    // SA-1/DQ3: special handling
                    if self.mapper_type == crate::cartridge::MapperType::DragonQuest3
                        && (0x6000..0x8000).contains(&offset)
                        && self.sram_size > 0
                    {
                        let sram_addr =
                            ((bank - 0x40) as usize) * 0x2000 + ((offset - 0x6000) as usize);
                        let idx = sram_addr % self.sram_size;
                        self.sram[idx] = value;
                        self.sram_dirty = true;
                        trace_sram("W", bank, offset, idx, value);
                    } else if self.mapper_type == crate::cartridge::MapperType::SuperFx {
                        if let Some(ref mut gsu) = self.superfx {
                            match bank {
                                0x70..=0x71 => {
                                    if gsu.cpu_has_ram_access() {
                                        let ram_addr =
                                            ((bank as usize - 0x70) << 16) | offset as usize;
                                        let gram = gsu.game_ram_slice();
                                        let idx = if gram.is_empty() {
                                            0
                                        } else {
                                            ram_addr % gram.len()
                                        };
                                        let old = gram.get(idx).copied().unwrap_or(0xFF);
                                        gsu.game_ram_write_linear(ram_addr, value);
                                        if crate::cartridge::superfx::debug_trace_superfx_ram_addr_matches_for_frame(
                                            idx,
                                            self.ppu.get_frame(),
                                        ) {
                                            let callers = if trace_cpu_sfx_ram_callers_enabled() {
                                                self.recent_cpu_exec_pcs
                                                    .iter()
                                                    .map(|pc| format!("{:06X}", pc))
                                                    .collect::<Vec<_>>()
                                                    .join(">")
                                            } else {
                                                String::new()
                                            };
                                            eprintln!(
                                                "[CPU-SFX-RAM-W] {:02X}:{:04X} -> {:05X} {:02X}->{:02X} PC={:06X} A={:04X} X={:04X} Y={:04X} DB={:02X} PB={:02X} P={:02X} f={}{}",
                                                bank,
                                                offset,
                                                idx,
                                                old,
                                                value,
                                                self.last_cpu_pc,
                                                self.last_cpu_a,
                                                self.last_cpu_x,
                                                self.last_cpu_y,
                                                self.last_cpu_db,
                                                self.last_cpu_pb,
                                                self.last_cpu_p,
                                                self.ppu.get_frame(),
                                                if callers.is_empty() {
                                                    String::new()
                                                } else {
                                                    format!(" callers={}", callers)
                                                }
                                            );
                                        }
                                    }
                                }
                                0x7C..=0x7D
                                    if gsu.backup_ram_write_enabled() && self.sram_size > 0 =>
                                {
                                    let idx = (((bank as usize - 0x7C) << 16) | offset as usize)
                                        % self.sram_size;
                                    self.sram[idx] = value;
                                    self.sram_dirty = true;
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
            // Extended WRAM banks
            0x7E..=0x7F => {
                let wram_addr = if self.wram_64k_mirror {
                    (offset as usize) & 0xFFFF
                } else {
                    ((bank - 0x7E) as usize) * 0x10000 + (offset as usize)
                };
                let abs = 0x7E0000u32 + (wram_addr as u32);
                self.trace_wram_abs_write("direct", abs, value);
                // Watch suspected handshake flag 7F:7DC0 (opt-in)
                if wram_addr == 0x1FDC0
                    && crate::debug_flags::trace_handshake()
                    && !crate::debug_flags::quiet()
                {
                    println!(
                        "[WRAM 7F:7DC0 WRITE] val=0x{:02X} bank={:02X} off={:04X}",
                        value, bank, offset
                    );
                }
                if self.trace_nmi_wram {
                    use std::sync::atomic::{AtomicU32, Ordering};
                    static WRITE_COUNT: AtomicU32 = AtomicU32::new(0);
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
                        let idx = WRITE_COUNT.fetch_add(1, Ordering::Relaxed);
                        if idx < 64 {
                            println!(
                                "[WRAM TRACE WRITE {}] val=0x{:02X} bank={:02X} off={:04X}",
                                label, value, bank, offset
                            );
                        }
                    }
                }
                if wram_addr < self.wram.len() {
                    self.wram[wram_addr] = value;
                }
            }
            // ROM mirror banks - writes ignored (except SRAM areas)
            0xC0..=0xFF => {
                if let Some(ref mapper) = self.mapper {
                    if mapper.write_bank_c0_ff(
                        &mut self.sram,
                        self.sram_size,
                        bank as u8,
                        offset,
                        value,
                    ) {
                        self.sram_dirty = true;
                    }
                } else {
                    // SA-1/DQ3: special handling
                    if self.mapper_type == crate::cartridge::MapperType::DragonQuest3
                        && (0x6000..0x8000).contains(&offset)
                    {
                        let sram_addr =
                            ((bank - 0xC0) as usize) * 0x2000 + ((offset - 0x6000) as usize);
                        if sram_addr < self.sram.len() {
                            self.sram[sram_addr] = value;
                        }
                    } else if self.mapper_type == crate::cartridge::MapperType::SuperFx {
                        // CPU-side ROM banks are read-only for Super FX cartridges.
                    }
                }
            }
            // Other banks - ignore writes
            _ => {}
        }
    }
}
