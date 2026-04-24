use super::debug::{
    cpu_test_auto_exit_enabled, trace_cpu_slow_read_ms, trace_starfox_slow_profile_enabled,
};
use super::{Bus, CpuTestResult};
use crate::cpu::bus::CpuBus;
use crate::debug_flags;
use std::time::Instant;

impl CpuBus for Bus {
    fn read_u8(&mut self, addr: u32) -> u8 {
        let bank = ((addr >> 16) & 0xFF) as usize;
        let slow_read_ms = trace_cpu_slow_read_ms();
        let profile_enabled = trace_starfox_slow_profile_enabled();
        let read_start = (profile_enabled || slow_read_ms.is_some()).then(Instant::now);
        let v = Bus::read_u8(self, addr);
        let elapsed_ns = read_start
            .map(|start| start.elapsed().as_nanos() as u64)
            .unwrap_or(0);
        {
            if let Some(threshold_ms) = slow_read_ms {
                if elapsed_ns >= threshold_ms.saturating_mul(1_000_000) {
                    eprintln!(
                        "[CPU-SLOW-READ] pc={:06X} addr={:02X}:{:04X} -> {:02X} ms={} frame={}",
                        self.last_cpu_pc,
                        bank as u8,
                        (addr & 0xFFFF) as u16,
                        v,
                        elapsed_ns / 1_000_000,
                        self.ppu.get_frame()
                    );
                }
            }
        }
        if profile_enabled {
            self.cpu_profile_read_ns = self.cpu_profile_read_ns.saturating_add(elapsed_ns);
            self.cpu_profile_read_count = self.cpu_profile_read_count.saturating_add(1);
            self.cpu_profile_read_bank_ns[bank] =
                self.cpu_profile_read_bank_ns[bank].saturating_add(elapsed_ns);
            self.cpu_profile_read_bank_count[bank] =
                self.cpu_profile_read_bank_count[bank].saturating_add(1);
        }
        if let Some(watch) = crate::debug_flags::watch_addr_read() {
            if watch == addr {
                let bank = (addr >> 16) & 0xFF;
                let offset = (addr & 0xFFFF) as u16;
                let sl = self.ppu.scanline;
                let cyc = self.ppu.get_cycle();
                println!(
                    "[watchR] {:02X}:{:04X} -> {:02X} PC={:06X} sl={} cyc={}",
                    bank, offset, v, self.last_cpu_pc, sl, cyc
                );
            }
        }
        self.last_cpu_bus_addr = addr;
        self.on_cpu_bus_cycle();
        v
    }

    fn write_u8(&mut self, addr: u32, value: u8) {
        let profile_enabled = trace_starfox_slow_profile_enabled();
        let write_start = profile_enabled.then(Instant::now);
        Bus::write_u8(self, addr, value);
        if let Some(start) = write_start {
            self.cpu_profile_write_ns = self
                .cpu_profile_write_ns
                .saturating_add(start.elapsed().as_nanos() as u64);
            self.cpu_profile_write_count = self.cpu_profile_write_count.saturating_add(1);
        }
        self.last_cpu_bus_addr = addr;
        self.on_cpu_bus_cycle();
    }

    fn begin_cpu_instruction(&mut self) {
        self.cpu_instr_active = true;
        self.cpu_instr_bus_cycles = 0;
        self.cpu_instr_apu_synced_bus_cycles = 0;
        self.cpu_instr_extra_master_cycles = 0;
    }

    fn end_cpu_instruction(&mut self, cycles: u8) {
        // 命令内で発生したバスアクセス分は read_u8/write_u8 側で tick 済み。
        // 残り（内部サイクル/ウェイト相当）だけ進める。
        let bus_cycles = self.cpu_instr_bus_cycles;
        let extra_master = self.cpu_instr_extra_master_cycles;
        self.last_cpu_instr_apu_synced_bus_cycles = self.cpu_instr_apu_synced_bus_cycles;
        self.cpu_instr_active = false;
        self.cpu_instr_bus_cycles = 0;
        self.cpu_instr_apu_synced_bus_cycles = 0;
        self.cpu_instr_extra_master_cycles = 0;
        let remaining = cycles.saturating_sub(bus_cycles);
        if remaining != 0 {
            self.tick_cpu_cycles(remaining);
        }
        // Slow/joypad access stretches CPU cycles in master clocks.
        // Store separately so the emulator can feed them to APU/PPU immediately
        // (rather than deferring to the next iteration via pending_stall).
        self.last_instr_extra_master = extra_master;
    }

    fn opcode_memory_penalty(&mut self, addr: u32) -> u8 {
        // General DMA (MDMAEN) begins after the *next opcode fetch* following the write to $420B.
        // We model that by consuming the queued mask here, right after the opcode byte has been
        // read by the core (see cpu_core::fetch_opcode_generic).
        if self.pending_mdma_mask != 0 {
            let mask = self.pending_mdma_mask;
            self.pending_mdma_mask = 0;
            let mut any = false;
            for i in 0..8 {
                if (mask & (1 << i)) == 0 {
                    continue;
                }
                if !self.dma_controller.channels[i].configured {
                    continue;
                }
                any = true;
                self.perform_dma_transfer(i);
            }
            if any {
                self.mdma_started_after_opcode_fetch = true;
            }
        }

        if debug_flags::mem_timing() && self.is_rom_address(addr) && !self.is_fastrom() {
            2
        } else {
            0
        }
    }

    fn take_dma_start_event(&mut self) -> bool {
        let v = self.mdma_started_after_opcode_fetch;
        self.mdma_started_after_opcode_fetch = false;
        v
    }

    fn poll_nmi(&mut self) -> bool {
        self.ppu.nmi_pending()
    }

    fn poll_irq(&mut self) -> bool {
        self.irq_is_pending()
    }

    fn is_superfx_irq_asserted(&self) -> bool {
        self.is_superfx_active()
            && self
                .superfx
                .as_ref()
                .is_some_and(|gsu| gsu.scpu_irq_asserted())
    }

    fn is_timer_irq_pending(&self) -> bool {
        self.irq_pending
    }

    fn acknowledge_nmi(&mut self) {
        // Clear the latched NMI flag so we don't immediately retrigger.
        self.ppu.clear_nmi();

        if let Some((start, end)) = crate::debug_flags::trace_nmi_state_range() {
            let frame = self.ppu.get_frame();
            if frame >= start && frame <= end {
                let w30 = self.wram[0x30];
                let w8c = self.wram[0x8C];
                let inidisp = self.ppu.screen_display;
                let bgmode = self.ppu.bg_mode;
                let tm = self.ppu.main_screen_designation;
                let _ts = self.ppu.sub_screen_designation;
                let hdma = self.dma_controller.hdma_enable;
                let pc = self.last_cpu_pc;
                let sub_major = self.wram[0x0180];
                let sub_minor = self.wram[0x0181];
                let w1d9 = self.wram[0x01D9];
                let w1da = self.wram[0x01DA];
                let w1e1 = self.wram[0x01E1];
                eprintln!(
                    "[NMI-STATE] frame={} PC={:06X} $30={:02X} $8C={:02X} INIDISP={:02X} BG={} TM={:02X} HDMA={:02X} sub={:02X}/{:02X} D9={:02X} DA={:02X} E1={:02X}",
                    frame, pc, w30, w8c, inidisp, bgmode, tm, hdma, sub_major, sub_minor, w1d9, w1da, w1e1
                );
            }
        }
    }

    fn set_last_cpu_pc(&mut self, pc24: u32) {
        self.last_cpu_pc = pc24;

        // burn-in-test.sfc EXT LATCH: trace tight PC flow with PPU timing (opt-in).
        // Useful to understand whether the latch pulse is occurring at the expected H/V position.
        if crate::debug_flags::trace_burnin_ext_flow() {
            let bank = (pc24 >> 16) as u8;
            let pc = (pc24 & 0xFFFF) as u16;
            if bank == 0x00 && (0x94C0..=0x9610).contains(&pc) {
                println!(
                    "[BURNIN-EXT][FLOW] PC={:06X} sl={} cyc={} frame={} vblank={} hblank={} wio=0x{:02X}",
                    pc24,
                    self.ppu.scanline,
                    self.ppu.get_cycle(),
                    self.ppu.get_frame(),
                    self.ppu.is_vblank() as u8,
                    self.ppu.is_hblank() as u8,
                    self.wio
                );
            }
        }

        // cputest-full.sfc: detect PASS/FAIL/Invalid by watching known PC points where it prints
        // the result string. This is used by headless runners to exit with an appropriate code.
        //
        // Guarded by:
        // - cpu_test_mode (title-based) AND
        // - HEADLESS=1 AND
        // - CPU_TEST_MODE env var set (explicit opt-in for auto-exit)
        if !self.cpu_test_mode || self.cpu_test_result.is_some() || !crate::debug_flags::headless()
        {
            return;
        }
        if !cpu_test_auto_exit_enabled() {
            return;
        }

        let test_idx = ((self.wram.get(0x0011).copied().unwrap_or(0) as u16) << 8)
            | (self.wram.get(0x0010).copied().unwrap_or(0) as u16);

        // These addresses are stable for the bundled roms/tests/cputest-full.sfc.
        // 00:8199 -> prints "Success"
        // 00:8148/00:81B8 -> prints "Failed"
        // 00:8150 -> prints "Invalid test order"
        self.cpu_test_result = match pc24 {
            0x008199 => Some(CpuTestResult::Pass { test_idx }),
            0x008148 | 0x0081B8 => Some(CpuTestResult::Fail { test_idx }),
            0x008150 => Some(CpuTestResult::InvalidOrder { test_idx }),
            _ => None,
        };
    }

    fn set_last_cpu_exec_pc(&mut self, pc24: u32) {
        self.last_cpu_exec_pc = pc24;
        self.push_recent_cpu_exec_pc(pc24);
    }

    fn set_last_cpu_state(&mut self, a: u16, x: u16, y: u16, db: u8, pb: u8, p: u8) {
        self.last_cpu_a = a;
        self.last_cpu_x = x;
        self.last_cpu_y = y;
        self.last_cpu_db = db;
        self.last_cpu_pb = pb;
        self.last_cpu_p = p;
    }
}
