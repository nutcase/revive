#![allow(static_mut_refs)]
#![allow(unreachable_patterns)]

mod access;
mod cpu_bus;
mod debug;
mod hdma;
mod io;
mod mdma;
mod read;
mod sa1;
mod superfx_assist;
mod timing;
mod trace;
mod write;

use std::sync::{Arc, Mutex};
use std::time::Instant;

// Logging controls
use crate::cartridge::mapper::MemoryMapper;
use crate::cartridge::sa1::Sa1;
use crate::savestate::BusSaveState;

use debug::trace_starfox_slow_profile_enabled;

const CPU_EXEC_TRACE_RING_LEN: usize = 16;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cpu::bus::CpuBus;

    fn configure_dma_dest(bus: &mut Bus, channel: usize, dest: u8) {
        let ch = &mut bus.dma_controller.channels[channel];
        ch.dest_address = dest;
        ch.configured = true;
    }

    #[test]
    fn strict_mdma_allows_cgram_during_active_hblank() {
        let mut bus = Bus::new(vec![]);
        configure_dma_dest(&mut bus, 0, 0x22);
        bus.ppu.screen_display = 0x00;
        bus.ppu.v_blank = false;
        bus.ppu.h_blank = true;
        bus.ppu.scanline = 10;
        bus.ppu.cycle = 300;

        let (now_mask, defer_mask) = bus.partition_mdma_mask_for_current_window(0x01, true);
        assert_eq!(now_mask, 0x01);
        assert_eq!(defer_mask, 0x00);
    }

    #[test]
    fn strict_mdma_defers_oam_outside_vblank_even_in_hblank() {
        let mut bus = Bus::new(vec![]);
        configure_dma_dest(&mut bus, 0, 0x04);
        bus.ppu.screen_display = 0x00;
        bus.ppu.v_blank = false;
        bus.ppu.h_blank = true;
        bus.ppu.scanline = 10;
        bus.ppu.cycle = 300;

        let (now_mask, defer_mask) = bus.partition_mdma_mask_for_current_window(0x01, true);
        assert_eq!(now_mask, 0x00);
        assert_eq!(defer_mask, 0x01);
    }

    #[test]
    fn strict_mdma_allows_oam_during_forced_blank() {
        let mut bus = Bus::new(vec![]);
        configure_dma_dest(&mut bus, 0, 0x04);
        bus.ppu.screen_display = 0x80;
        bus.ppu.v_blank = false;
        bus.ppu.h_blank = false;
        bus.ppu.scanline = 42;
        bus.ppu.cycle = 12;

        let (now_mask, defer_mask) = bus.partition_mdma_mask_for_current_window(0x01, true);
        assert_eq!(now_mask, 0x01);
        assert_eq!(defer_mask, 0x00);
    }

    #[test]
    fn cpu_exec_trace_ring_keeps_latest_entries() {
        let mut bus = Bus::new(vec![]);
        for i in 0..(CPU_EXEC_TRACE_RING_LEN as u32 + 3) {
            bus.set_last_cpu_exec_pc(0x008000 + i);
        }

        assert_eq!(
            bus.debug_recent_cpu_exec_pcs().len(),
            CPU_EXEC_TRACE_RING_LEN
        );
        assert_eq!(bus.debug_recent_cpu_exec_pcs()[0], 0x008003);
        assert_eq!(
            *bus.debug_recent_cpu_exec_pcs().last().unwrap(),
            0x008000 + CPU_EXEC_TRACE_RING_LEN as u32 + 2
        );
    }

    #[test]
    fn bus_superfx_r15_high_write_does_not_mutate_starfox_working_regs_immediately() {
        let rom = vec![0u8; 0x20_0000];
        let mut bus = Bus::new_with_mapper(rom, crate::cartridge::MapperType::SuperFx, 0x2000);
        let gsu = bus.superfx.as_mut().unwrap();
        gsu.debug_set_pbr(0x01);
        gsu.debug_set_rombr(0x14);
        gsu.debug_set_scmr(0x39);
        gsu.debug_set_reg(9, 0x2800);
        gsu.debug_set_reg(13, 0xB3DE);
        gsu.debug_set_reg(14, 0x6242);
        gsu.debug_set_reg(15, 0xB3E6);

        bus.write_u8(0x00_301E, 0x01);
        let gsu = bus.superfx.as_ref().unwrap();
        assert_eq!(gsu.debug_reg(15), 0xB301);
        assert_eq!(gsu.debug_reg(9), 0x2800);
        assert_eq!(gsu.debug_reg(13), 0xB3DE);
        assert_eq!(gsu.debug_reg(14), 0x6242);
        assert!(!gsu.running());

        bus.write_u8(0x00_301F, 0xB3);
        let gsu = bus.superfx.as_ref().unwrap();
        assert!(gsu.running());
        assert_eq!(gsu.debug_reg(15), 0xB301);
        assert_eq!(gsu.debug_reg(9), 0x2800);
        assert_eq!(gsu.debug_reg(13), 0xB3DE);
        assert_eq!(gsu.debug_reg(14), 0x6242);
    }

    #[test]
    fn hdmaen_rising_edge_enables_configured_channel_without_reinitialising_table() {
        let mut bus = Bus::new(vec![]);
        bus.ppu.scanline = 42;
        bus.ppu.cycle = 120;
        let ch = &mut bus.dma_controller.channels[1];
        ch.configured = true;
        ch.control = 0x40;
        ch.hdma_enabled = false;
        ch.hdma_terminated = false;
        ch.hdma_indirect = false;
        ch.hdma_table_addr = 0x12_3456;
        ch.hdma_line_counter = 0x23;
        ch.hdma_do_transfer = false;
        bus.dma_controller.hdma_enable = 0x00;

        bus.write_u8(0x420C, 0x02);

        let ch = &bus.dma_controller.channels[1];
        assert_eq!(bus.dma_controller.hdma_enable, 0x02);
        assert!(ch.hdma_enabled);
        assert!(!ch.hdma_terminated);
        assert!(ch.hdma_indirect);
        assert_eq!(ch.hdma_table_addr, 0x12_3456);
        assert_eq!(ch.hdma_line_counter, 0x23);
        assert!(!ch.hdma_do_transfer);
    }

    #[test]
    fn hdmaen_rising_edge_before_first_hblank_initialises_frame_channel() {
        let mut bus = Bus::new(vec![]);
        bus.ppu.scanline = 0;
        bus.ppu.cycle = 225;
        bus.ppu.h_blank = false;

        let ch = &mut bus.dma_controller.channels[1];
        ch.configured = true;
        ch.control = 0x40;
        ch.src_address = 0x12_3456;
        ch.hdma_enabled = false;
        ch.hdma_terminated = false;
        ch.hdma_indirect = false;
        ch.hdma_indirect_addr = 0x7E_9999;
        ch.hdma_table_addr = 0x7E_2222;
        ch.hdma_line_counter = 0x23;
        ch.hdma_repeat_flag = true;
        ch.hdma_do_transfer = true;
        ch.a2a = 0x2222;
        ch.nltr = 0xA3;
        bus.dma_controller.hdma_enable = 0x00;

        bus.write_u8(0x420C, 0x02);

        let ch = &bus.dma_controller.channels[1];
        assert_eq!(bus.dma_controller.hdma_enable, 0x02);
        assert!(ch.hdma_enabled);
        assert!(!ch.hdma_terminated);
        assert!(ch.hdma_indirect);
        assert_eq!(ch.hdma_indirect_addr, 0);
        assert_eq!(ch.hdma_table_addr, 0x12_3456);
        assert_eq!(ch.hdma_line_counter, 0);
        assert!(!ch.hdma_repeat_flag);
        assert!(!ch.hdma_do_transfer);
        assert_eq!(ch.a2a, 0x3456);
        assert_eq!(ch.nltr, 0x80);
    }

    #[test]
    fn hdmaen_rising_edge_does_not_restart_channel_terminated_this_frame() {
        let mut bus = Bus::new(vec![]);
        bus.ppu.scanline = 42;
        bus.ppu.cycle = 120;
        let ch = &mut bus.dma_controller.channels[1];
        ch.configured = true;
        ch.hdma_enabled = false;
        ch.hdma_terminated = true;
        bus.dma_controller.hdma_enable = 0x00;

        bus.write_u8(0x420C, 0x02);

        let ch = &bus.dma_controller.channels[1];
        assert!(!ch.hdma_enabled);
        assert!(ch.hdma_terminated);
    }

    #[test]
    fn hdma_table_line_80_is_nonrepeat_128_lines() {
        let mut bus = Bus::new(vec![]);
        bus.wram[0] = 0x80;
        let ch = &mut bus.dma_controller.channels[0];
        ch.configured = true;
        ch.control = 0x00;
        ch.hdma_table_addr = 0x7E_0000;

        assert!(bus.load_hdma_entry(0));

        let ch = &bus.dma_controller.channels[0];
        assert_eq!(ch.hdma_line_counter, 128);
        assert!(!ch.hdma_repeat_flag);
        assert!(ch.hdma_do_transfer);
        assert_eq!(ch.hdma_table_addr, 0x7E_0001);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CpuTestResult {
    Pass { test_idx: u16 },
    Fail { test_idx: u16 },
    InvalidOrder { test_idx: u16 },
}

pub struct Bus {
    pub(crate) wram: Vec<u8>,
    pub(crate) wram_64k_mirror: bool,
    pub(crate) trace_nmi_wram: bool,
    pub(crate) sram: Vec<u8>,
    pub(crate) rom: Vec<u8>,
    pub(crate) ppu: crate::ppu::Ppu,
    pub(crate) apu: Arc<Mutex<crate::audio::apu::Apu>>,
    pub(crate) dma_controller: crate::dma::DmaController,
    pub(crate) input_system: crate::input::InputSystem,
    pub(crate) mapper_type: crate::cartridge::MapperType,
    pub(crate) mapper: Option<crate::cartridge::mapper::MapperImpl>,
    pub(crate) rom_size: usize,
    pub(crate) sram_size: usize,
    // Mark when battery-backed RAM was modified
    pub(crate) sram_dirty: bool,
    // Memory mapping registers
    pub(crate) nmitimen: u8,      // $4200 - Interrupt Enable
    pub(crate) wram_address: u32, // $2181-2183 - WRAM Address
    pub(crate) mdr: u8,           // Memory Data Register (open bus)
    // Hardware math registers (CPU I/O $4202-$4206; results at $4214-$4217)
    pub(crate) mul_a: u8,
    pub(crate) mul_b: u8,
    pub(crate) mul_result: u16,
    pub(crate) div_a: u16,
    pub(crate) div_b: u8,
    pub(crate) div_quot: u16,
    pub(crate) div_rem: u16,
    // Hardware math in-flight timing (coarse per S-CPU cycle slice)
    pub(crate) mul_busy: bool,
    pub(crate) mul_just_started: bool,
    pub(crate) mul_cycles_left: u8,
    pub(crate) mul_work_a: u16,
    pub(crate) mul_work_b: u8,
    pub(crate) mul_partial: u16,
    pub(crate) div_busy: bool,
    pub(crate) div_just_started: bool,
    pub(crate) div_cycles_left: u8,
    pub(crate) div_work_dividend: u16,
    pub(crate) div_work_divisor: u8,
    pub(crate) div_work_quot: u16,
    pub(crate) div_work_rem: u16,
    pub(crate) div_work_bit: i8,
    // CPU命令内のバスアクセス数（サイクル近似）を数えるためのフック。
    // - CpuBusトレイト経由の read_u8/write_u8 を 1回=1サイクル相当として扱い、
    //   $4202-$4206 等の時間依存I/Oをより正確に進める。
    pub(crate) cpu_instr_active: bool,
    pub(crate) cpu_instr_bus_cycles: u8,
    // 命令途中の APU ポートアクセスで、どこまでの bus cycle を APU 側へ
    // 先行反映したか。命令末尾の通常バッチ更新で二重加算しないために使う。
    pub(crate) cpu_instr_apu_synced_bus_cycles: u8,
    pub(crate) last_cpu_instr_apu_synced_bus_cycles: u8,
    // CPUアクセスのウェイト状態（Fast/Slow/JOYSER）を master cycles で積む。
    // ベースは 6 master cycles/CPU cycle としているため、差分（+2/+6）だけをここに蓄積する。
    pub(crate) cpu_instr_extra_master_cycles: u64,
    // Slow-memory extra master cycles from the last completed CPU instruction.
    // Separate from pending_stall so the emulator can feed them to APU immediately.
    pub(crate) last_instr_extra_master: u64,
    // DMA転送中フラグ。DMA中のread_u8/write_u8をCPUバスサイクルとしてカウントしない。
    pub(crate) dma_in_progress: bool,
    // IRQ/Timer
    pub(crate) irq_h_enabled: bool,             // $4200 bit4
    pub(crate) irq_v_enabled: bool,             // $4200 bit5
    pub(crate) irq_pending: bool,               // TIMEUP ($4211)
    pub(crate) irq_v_matched_line: Option<u16>, // remember V-match scanline when both H&V are enabled
    pub(crate) h_timer: u16,                    // $4207/$4208 (not fully used yet)
    pub(crate) v_timer: u16,                    // $4209/$420A
    pub(crate) h_timer_set: bool,
    pub(crate) v_timer_set: bool,

    // Auto-joypad (NMITIMEN bit0) + JOYBUSY/JOY registers
    pub(crate) joy_busy_counter: u8, // >0 while auto-joy is in progress
    pub(crate) joy_data: [u8; 8], // $4218..$421F (JOY1L,JOY1H,JOY2L,JOY2H,JOY3L,JOY3H,JOY4L,JOY4H)
    pub(crate) joy_busy_scanlines: u8, // configurable duration of JOYBUSY after VBlank start
    pub(crate) cpu_test_mode: bool,
    pub(crate) cpu_test_result: Option<CpuTestResult>,

    // Run-wide counters for headless init summary
    pub(crate) nmitimen_writes_count: u32,
    pub(crate) mdmaen_nonzero_count: u32,
    pub(crate) hdmaen_nonzero_count: u32,

    // DMA config observation (how many writes to $43x0-$43x6 etc.)
    pub(crate) dma_reg_writes: u32,
    // DMA destination histogram (B-bus low 7 bits)
    pub(crate) dma_dest_hist: [u32; 256],
    // Pending graphics DMA mask (strict timing: defer VRAM/CGRAM/OAM MDMA to VBlank)
    pub(crate) pending_gdma_mask: u8,
    // Pending general DMA mask (MDMAEN): starts after the *next opcode fetch*.
    pub(crate) pending_mdma_mask: u8,
    // One-shot: set when an opcode fetch triggered MDMA start.
    // Used by the CPU core to defer executing that instruction until after the DMA stall.
    pub(crate) mdma_started_after_opcode_fetch: bool,
    pub(crate) last_cpu_pc: u32, // debug: last S-CPU operand/fetch address that touched the bus
    pub(crate) last_cpu_exec_pc: u32, // debug: last S-CPU instruction PC
    pub(crate) last_cpu_a: u16,  // debug: last S-CPU A at instruction start
    pub(crate) last_cpu_x: u16,  // debug: last S-CPU X at instruction start
    pub(crate) last_cpu_y: u16,  // debug: last S-CPU Y at instruction start
    pub(crate) last_cpu_db: u8,  // debug: last S-CPU DB at instruction start
    pub(crate) last_cpu_pb: u8,  // debug: last S-CPU PB at instruction start
    pub(crate) last_cpu_p: u8,   // debug: last S-CPU P at instruction start
    pub(crate) last_cpu_bus_addr: u32, // debug: last S-CPU bus address (for timing heuristics)
    pub(crate) recent_cpu_exec_pcs: Vec<u32>, // debug: recent S-CPU instruction PCs
    pub(crate) superfx_status_poll_pc: u32,
    pub(crate) superfx_status_poll_streak: u16,
    pub(crate) starfox_exact_wait_assist_frame: u64,
    // HDMA aggregate stats (visible for headless summaries)
    pub(crate) hdma_lines_executed: u32,
    pub(crate) hdma_bytes_vram: u32,
    pub(crate) hdma_bytes_cgram: u32,
    pub(crate) hdma_bytes_oam: u32,
    pub(crate) hdma_bytes_window: u32,
    pub(crate) rdnmi_consumed: bool,
    pub(crate) rdnmi_high_byte_for_test: u8,

    // Extra master cycles consumed by DMA stalls (CPU is halted while PPU/APU continue).
    pub(crate) pending_stall_master_cycles: u64,

    // SMW用デバッグHLE: WRAM DMAからSPCコードを抜き取り即ロードする
    pub(crate) smw_apu_hle: bool,
    pub(crate) smw_apu_hle_done: bool,
    pub(crate) smw_apu_hle_buf: Vec<u8>,
    pub(crate) smw_apu_hle_echo_idx: u32,

    // Programmable I/O and memory speed
    pub(crate) wio: u8,       // $4201 write; read back via $4213
    pub(crate) fastrom: bool, // $420D bit0
    // Test ROM integration: capture APU $2140 prints
    pub(crate) test_apu_print: bool,
    pub(crate) test_apu_buf: String,
    pub(crate) superfx: Option<crate::cartridge::superfx::SuperFx>,
    pub(crate) spc7110: Option<crate::cartridge::spc7110::Spc7110>,
    pub(crate) sdd1: Option<crate::cartridge::sdd1::Sdd1>,
    pub(crate) dsp1: Option<crate::cartridge::dsp1::Dsp1>,
    pub(crate) dsp3: Option<crate::cartridge::dsp3::Dsp3>,
    pub(crate) sa1: Sa1,
    pub(crate) sa1_bwram: Vec<u8>,
    #[allow(dead_code)]
    pub(crate) sa1_iram: [u8; 0x800],
    pub(crate) sa1_cycle_deficit: i64,
    pub(crate) sa1_cycles_accum_frame: u64,
    // SA-1 initialization support: delay NMI during boot
    pub(crate) sa1_nmi_delay_active: bool,
    // Cached at init: true if any read_u8 debug trace flags are active.
    // Avoids per-read OnceLock lookups on the hot path.
    any_read_trace_active: bool,
    cpu_profile_read_ns: u64,
    cpu_profile_write_ns: u64,
    cpu_profile_bus_cycle_ns: u64,
    cpu_profile_tick_ns: u64,
    cpu_profile_read_count: u32,
    cpu_profile_write_count: u32,
    cpu_profile_bus_cycle_count: u32,
    cpu_profile_tick_count: u32,
    cpu_profile_read_bank_ns: [u64; 256],
    cpu_profile_read_bank_count: [u32; 256],
}

impl Bus {
    #[inline]
    pub fn wram(&self) -> &[u8] {
        &self.wram
    }
    #[allow(dead_code)]
    pub fn new(rom: Vec<u8>) -> Self {
        let rom_size = rom.len();
        let wram_fill: u8 = std::env::var("WRAM_FILL")
            .ok()
            .and_then(|s| u8::from_str_radix(s.trim_start_matches("0x"), 16).ok())
            .unwrap_or(0x55);
        let mut bus = Self {
            wram: vec![wram_fill; 0x20000],
            wram_64k_mirror: std::env::var_os("WRAM_64K_MIRROR").is_some(),
            trace_nmi_wram: std::env::var_os("TRACE_NMI_WRAM").is_some(),
            sram: vec![0xFF; 0x8000],
            rom,
            ppu: crate::ppu::Ppu::new(),
            apu: Arc::new(Mutex::new(crate::audio::apu::Apu::new())),
            dma_controller: crate::dma::DmaController::new(),
            input_system: crate::input::InputSystem::new(),
            mapper_type: crate::cartridge::MapperType::LoRom, // Default to LoROM
            mapper: crate::cartridge::mapper::MapperImpl::from_type(
                crate::cartridge::MapperType::LoRom,
            ),
            rom_size,
            sram_size: 0x8000,
            sram_dirty: false,
            nmitimen: 0,
            wram_address: 0,
            mdr: 0,
            mul_a: 0,
            mul_b: 0,
            mul_result: 0,
            div_a: 0,
            div_b: 0,
            div_quot: 0,
            div_rem: 0,
            mul_busy: false,
            mul_just_started: false,
            mul_cycles_left: 0,
            mul_work_a: 0,
            mul_work_b: 0,
            mul_partial: 0,
            div_busy: false,
            div_just_started: false,
            div_cycles_left: 0,
            div_work_dividend: 0,
            div_work_divisor: 0,
            div_work_quot: 0,
            div_work_rem: 0,
            div_work_bit: 0,
            cpu_instr_active: false,
            cpu_instr_bus_cycles: 0,
            cpu_instr_apu_synced_bus_cycles: 0,
            last_cpu_instr_apu_synced_bus_cycles: 0,
            cpu_instr_extra_master_cycles: 0,
            dma_in_progress: false,
            irq_h_enabled: false,
            irq_v_enabled: false,
            irq_pending: false,
            irq_v_matched_line: None,
            h_timer: 0,
            v_timer: 0,
            h_timer_set: false,
            v_timer_set: false,

            joy_busy_counter: 0,
            // $4218-$421F (JOY1..4): power-on should read as "no buttons pressed".
            // Bits are treated as 1=pressed, so default is 0x00.
            joy_data: [0x00; 8],
            // JOYBUSY はオートジョイパッド読み取り中だけ立つ。
            // 実機では約 3 本分のスキャンライン相当 (4224 master cycles) 継続する。
            // CPU テスト ROM では VBlank 突入から数ライン後に $4212 を覗くため、
            // cpu_test_mode のときだけ 8 ライン相当に拡張して読み損ねを防ぐ。
            joy_busy_scanlines: std::env::var("JOYBUSY_SCANLINES")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(3),
            cpu_test_mode: false,
            cpu_test_result: None,

            nmitimen_writes_count: 0,
            mdmaen_nonzero_count: 0,
            hdmaen_nonzero_count: 0,

            // WRIO ($4201) behaves as if initialized to all-1s at power-on.
            wio: 0xFF,
            fastrom: false,
            dma_reg_writes: 0,
            dma_dest_hist: [0; 256],
            pending_gdma_mask: 0,
            pending_mdma_mask: 0,
            mdma_started_after_opcode_fetch: false,
            last_cpu_pc: 0,
            last_cpu_exec_pc: 0,
            last_cpu_a: 0,
            last_cpu_x: 0,
            last_cpu_y: 0,
            last_cpu_db: 0,
            last_cpu_pb: 0,
            last_cpu_p: 0,
            last_cpu_bus_addr: 0,
            recent_cpu_exec_pcs: Vec::new(),
            superfx_status_poll_pc: 0,
            superfx_status_poll_streak: 0,
            starfox_exact_wait_assist_frame: u64::MAX,
            hdma_lines_executed: 0,
            hdma_bytes_vram: 0,
            hdma_bytes_cgram: 0,
            hdma_bytes_oam: 0,
            hdma_bytes_window: 0,
            rdnmi_consumed: false,
            rdnmi_high_byte_for_test: 0,
            pending_stall_master_cycles: 0,
            last_instr_extra_master: 0,
            // SMW専用のWRAM→APU自動ロード（HLE）はデフォルト無効。
            smw_apu_hle: std::env::var("SMW_APU_HLE")
                .map(|v| v != "0" && v.to_lowercase() != "false")
                .unwrap_or(false),
            smw_apu_hle_done: false,
            smw_apu_hle_buf: Vec::new(),
            smw_apu_hle_echo_idx: 0,
            test_apu_print: std::env::var("TESTROM_APU_PRINT")
                .map(|v| v == "1" || v.to_lowercase() == "true")
                .unwrap_or(false),
            test_apu_buf: String::new(),
            superfx: None,
            spc7110: None,
            sdd1: None,
            dsp1: None,
            dsp3: None,
            sa1: Sa1::new(),
            sa1_bwram: vec![0xFF; 0x40000],
            sa1_iram: [0; 0x800],
            sa1_cycle_deficit: 0,
            sa1_cycles_accum_frame: 0,
            sa1_nmi_delay_active: false,
            any_read_trace_active: false,
            cpu_profile_read_ns: 0,
            cpu_profile_write_ns: 0,
            cpu_profile_bus_cycle_ns: 0,
            cpu_profile_tick_ns: 0,
            cpu_profile_read_count: 0,
            cpu_profile_write_count: 0,
            cpu_profile_bus_cycle_count: 0,
            cpu_profile_tick_count: 0,
            cpu_profile_read_bank_ns: [0; 256],
            cpu_profile_read_bank_count: [0; 256],
        };
        bus.any_read_trace_active = crate::debug_flags::trace_vectors()
            || crate::debug_flags::trace_4212()
            || crate::debug_flags::trace_sfr()
            || crate::debug_flags::trace_sfr_values();

        // Mirror WRIO bit7 to PPU latch enable.
        bus.ppu.set_wio_latch_enable(true);

        bus
    }

    pub fn new_with_mapper(
        rom: Vec<u8>,
        mapper: crate::cartridge::MapperType,
        sram_size: usize,
    ) -> Self {
        let rom_size = rom.len();
        let wram_fill: u8 = std::env::var("WRAM_FILL")
            .ok()
            .and_then(|s| u8::from_str_radix(s.trim_start_matches("0x"), 16).ok())
            .unwrap_or(0x55);
        let mut bus = Self {
            wram: vec![wram_fill; 0x20000],
            wram_64k_mirror: std::env::var_os("WRAM_64K_MIRROR").is_some(),
            trace_nmi_wram: std::env::var_os("TRACE_NMI_WRAM").is_some(),
            sram: vec![0xFF; sram_size.max(0x2000)], // Minimum 8KB SRAM
            rom,
            ppu: crate::ppu::Ppu::new(),
            apu: Arc::new(Mutex::new(crate::audio::apu::Apu::new())),
            dma_controller: crate::dma::DmaController::new(),
            input_system: crate::input::InputSystem::new(),
            mapper_type: mapper,
            mapper: crate::cartridge::mapper::MapperImpl::from_type(mapper),
            rom_size,
            sram_size,
            sram_dirty: false,
            nmitimen: 0,
            wram_address: 0,
            mdr: 0,
            mul_a: 0,
            mul_b: 0,
            mul_result: 0,
            div_a: 0,
            div_b: 0,
            div_quot: 0,
            div_rem: 0,
            mul_busy: false,
            mul_just_started: false,
            mul_cycles_left: 0,
            mul_work_a: 0,
            mul_work_b: 0,
            mul_partial: 0,
            div_busy: false,
            div_just_started: false,
            div_cycles_left: 0,
            div_work_dividend: 0,
            div_work_divisor: 0,
            div_work_quot: 0,
            div_work_rem: 0,
            div_work_bit: 0,
            cpu_instr_active: false,
            cpu_instr_bus_cycles: 0,
            cpu_instr_apu_synced_bus_cycles: 0,
            last_cpu_instr_apu_synced_bus_cycles: 0,
            cpu_instr_extra_master_cycles: 0,
            dma_in_progress: false,
            irq_h_enabled: false,
            irq_v_enabled: false,
            irq_pending: false,
            irq_v_matched_line: None,
            h_timer: 0,
            v_timer: 0,
            h_timer_set: false,
            v_timer_set: false,

            joy_busy_counter: 0,
            // $4218-$421F (JOY1..4): power-on should read as "no buttons pressed".
            // Bits are treated as 1=pressed, so default is 0x00.
            joy_data: [0x00; 8],
            joy_busy_scanlines: std::env::var("JOYBUSY_SCANLINES")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(3),
            cpu_test_mode: false,
            cpu_test_result: None,

            nmitimen_writes_count: 0,
            mdmaen_nonzero_count: 0,
            hdmaen_nonzero_count: 0,

            // WRIO ($4201) behaves as if initialized to all-1s at power-on.
            wio: 0xFF,
            fastrom: false,
            dma_reg_writes: 0,
            dma_dest_hist: [0; 256],
            pending_gdma_mask: 0,
            pending_mdma_mask: 0,
            mdma_started_after_opcode_fetch: false,
            last_cpu_pc: 0,
            last_cpu_exec_pc: 0,
            last_cpu_a: 0,
            last_cpu_x: 0,
            last_cpu_y: 0,
            last_cpu_db: 0,
            last_cpu_pb: 0,
            last_cpu_p: 0,
            last_cpu_bus_addr: 0,
            recent_cpu_exec_pcs: Vec::new(),
            superfx_status_poll_pc: 0,
            superfx_status_poll_streak: 0,
            starfox_exact_wait_assist_frame: u64::MAX,
            hdma_lines_executed: 0,
            hdma_bytes_vram: 0,
            hdma_bytes_cgram: 0,
            hdma_bytes_oam: 0,
            hdma_bytes_window: 0,
            rdnmi_consumed: false,
            rdnmi_high_byte_for_test: 0,
            pending_stall_master_cycles: 0,
            last_instr_extra_master: 0,
            smw_apu_hle: std::env::var("SMW_APU_HLE")
                .map(|v| v != "0" && v.to_lowercase() != "false")
                .unwrap_or(false),
            smw_apu_hle_done: false,
            smw_apu_hle_buf: Vec::new(),
            smw_apu_hle_echo_idx: 0,
            test_apu_print: std::env::var("TESTROM_APU_PRINT")
                .map(|v| v == "1" || v.to_lowercase() == "true")
                .unwrap_or(false),
            test_apu_buf: String::new(),
            superfx: if mapper == crate::cartridge::MapperType::SuperFx {
                Some(crate::cartridge::superfx::SuperFx::new(rom_size))
            } else {
                None
            },
            spc7110: if mapper == crate::cartridge::MapperType::Spc7110 {
                Some(crate::cartridge::spc7110::Spc7110::new(rom_size))
            } else {
                None
            },
            sdd1: if mapper == crate::cartridge::MapperType::Sdd1 {
                Some(crate::cartridge::sdd1::Sdd1::new())
            } else {
                None
            },
            dsp1: match mapper {
                crate::cartridge::MapperType::Dsp1 => {
                    Some(crate::cartridge::dsp1::Dsp1::new(rom_size))
                }
                crate::cartridge::MapperType::Dsp1HiRom => {
                    Some(crate::cartridge::dsp1::Dsp1::new_hirom())
                }
                _ => None,
            },
            dsp3: if mapper == crate::cartridge::MapperType::Dsp3 {
                Some(crate::cartridge::dsp3::Dsp3::new())
            } else {
                None
            },
            sa1: Sa1::new(),
            sa1_bwram: vec![0xFF; sram_size.max(0x2000)], // fill with 0xFF for SA-1
            sa1_iram: [0; 0x800],
            sa1_cycle_deficit: 0,
            sa1_cycles_accum_frame: 0,
            sa1_nmi_delay_active: false,
            any_read_trace_active: false,
            cpu_profile_read_ns: 0,
            cpu_profile_write_ns: 0,
            cpu_profile_bus_cycle_ns: 0,
            cpu_profile_tick_ns: 0,
            cpu_profile_read_count: 0,
            cpu_profile_write_count: 0,
            cpu_profile_bus_cycle_count: 0,
            cpu_profile_tick_count: 0,
            cpu_profile_read_bank_ns: [0; 256],
            cpu_profile_read_bank_count: [0; 256],
        };
        bus.any_read_trace_active = crate::debug_flags::trace_vectors()
            || crate::debug_flags::trace_4212()
            || crate::debug_flags::trace_sfr()
            || crate::debug_flags::trace_sfr_values();

        // Mirror WRIO bit7 to PPU latch enable.
        bus.ppu.set_wio_latch_enable(true);

        bus
    }

    #[inline]
    pub fn is_sa1_active(&self) -> bool {
        matches!(
            self.mapper_type,
            crate::cartridge::MapperType::Sa1 | crate::cartridge::MapperType::DragonQuest3
        )
    }

    #[inline]
    pub fn is_superfx_active(&self) -> bool {
        self.mapper_type == crate::cartridge::MapperType::SuperFx
    }

    #[inline]
    fn read_sa1_register_scpu(&mut self, reg: u16) -> u8 {
        match reg {
            0x102 => (self.ppu.get_cycle() & 0x00FF) as u8,
            0x103 => ((self.ppu.get_cycle() >> 8) & 0x01) as u8,
            0x104 => (self.ppu.get_scanline() & 0x00FF) as u8,
            0x105 => ((self.ppu.get_scanline() >> 8) & 0x01) as u8,
            0x10E => 0x23,
            _ => self.sa1.read_register_scpu(reg, self.mdr),
        }
    }

    #[inline]
    fn sa1_varlen_rom_byte(&self, addr: u32) -> u8 {
        let phys = self.sa1_phys_addr(addr >> 16, addr as u16);
        self.rom.get(phys % self.rom_size).copied().unwrap_or(0xFF)
    }

    fn read_sa1_varlen_word(&self) -> u16 {
        let start_addr = self.sa1.registers.varlen_addr & !1;
        let start_bit = self.sa1.registers.varlen_current_bits as usize;
        let mut word = 0u16;
        for bit in 0..16usize {
            let absolute_bit = start_bit + bit;
            let byte_addr = start_addr.wrapping_add((absolute_bit / 8) as u32);
            let byte = self.sa1_varlen_rom_byte(byte_addr);
            let bit_index = 7 - (absolute_bit % 8);
            let value = (byte >> bit_index) & 1;
            word |= (value as u16) << bit;
        }
        word
    }

    fn read_sa1_varlen_port(&mut self, high_byte: bool) -> u8 {
        if !self.sa1.registers.varlen_latched {
            self.sa1.registers.varlen_latched_word = self.read_sa1_varlen_word();
            self.sa1.registers.varlen_latched = true;
        }

        let result = if high_byte {
            (self.sa1.registers.varlen_latched_word >> 8) as u8
        } else {
            (self.sa1.registers.varlen_latched_word & 0xFF) as u8
        };

        if high_byte {
            if (self.sa1.registers.varlen_control & 0x80) != 0 {
                self.sa1.registers.varlen_current_bits =
                    self.sa1.registers.varlen_current_bits.wrapping_add(
                        crate::cartridge::sa1::Sa1::decode_varlen_bits(
                            self.sa1.registers.varlen_control,
                        ),
                    );
            }
            self.sa1.registers.varlen_latched = false;
        }

        result
    }

    /// Force disable all IRQs (for SA-1 initialization delay)
    #[allow(dead_code)]
    pub(crate) fn force_disable_irq(&mut self) {
        self.irq_h_enabled = false;
        self.irq_v_enabled = false;
        self.irq_pending = false;
    }

    #[allow(dead_code)]
    pub fn sa1(&self) -> &Sa1 {
        &self.sa1
    }

    #[allow(dead_code)]
    pub fn sa1_mut(&mut self) -> &mut Sa1 {
        &mut self.sa1
    }

    #[inline]
    pub fn sa1_dma_pending(&self) -> bool {
        self.is_sa1_active() && self.sa1.dma_busy()
    }

    #[inline]
    pub fn reset_sa1_cycle_accum(&mut self) {
        self.sa1_cycles_accum_frame = 0;
    }

    #[inline]
    pub fn take_sa1_cycle_accum(&mut self) -> u64 {
        let v = self.sa1_cycles_accum_frame;
        self.sa1_cycles_accum_frame = 0;
        v
    }

    #[inline]
    pub fn run_sa1_cycles_direct(&mut self, sa1_cycles: u32) {
        if !self.is_sa1_active() || sa1_cycles == 0 {
            return;
        }
        self.sa1_cycle_deficit = self.sa1_cycle_deficit.saturating_add(sa1_cycles as i64);
        // Reuse the scheduler with zero CPU cycles; it will consume the deficit.
        self.run_sa1_scheduler(0);
    }

    fn init_sa1_vectors_from_rom(&mut self) {
        if !self.is_sa1_active() {
            return;
        }
        let debug = std::env::var_os("TRACE_SA1_BOOT").is_some();
        let fetch_vec = |addr: u16, this: &mut Self| -> u16 {
            let phys = this.sa1_phys_addr(0x00, addr);
            let lo = this.rom.get(phys % this.rom_size).copied().unwrap_or(0x00);
            let hi = this
                .rom
                .get((phys + 1) % this.rom_size)
                .copied()
                .unwrap_or(0x00);
            (hi as u16) << 8 | lo as u16
        };
        let reset_vec = fetch_vec(0xFFFC, self);
        let nmi_vec = fetch_vec(0xFFEA, self);
        let irq_vec = fetch_vec(0xFFEE, self);
        self.sa1.registers.reset_vector = reset_vec;
        self.sa1.registers.nmi_vector = nmi_vec;
        self.sa1.registers.irq_vector = irq_vec;
        // Default SA-1: use ROM header vectors, program bank chunk = 0 (C block).
        self.sa1.boot_pb = 0x00;

        // If a real SA-1 IPL dump is present, load it into IRAM and boot from 0x0000.
        let mut ipl_loaded = false;
        if self.is_sa1_active() {
            let candidates = [
                std::path::Path::new("sa1.rom"),
                std::path::Path::new("roms/sa1.rom"),
                std::path::Path::new("roms/ipl.rom"),
            ];
            for path in candidates.iter() {
                if let Ok(data) = std::fs::read(path) {
                    // Real SA-1 IPL is exactly 0x800 bytes. Reject other sizes to avoid
                    // accidentally treating a full game ROM or placeholder as the IPL.
                    if data.len() == 0x800 {
                        self.sa1_iram
                            .iter_mut()
                            .zip(data.iter())
                            .for_each(|(dst, src)| *dst = *src);
                        self.sa1.registers.reset_vector = 0x0000;
                        self.sa1.registers.control = 0x20;
                        ipl_loaded = true;
                        if debug {
                            println!(
                                "[SA1] Loaded external IPL from {:?} ({} bytes)",
                                path,
                                data.len()
                            );
                        }
                        break;
                    } else if debug {
                        println!(
                            "[SA1] Ignoring IPL candidate {:?} ({} bytes, expected 2048)",
                            path,
                            data.len()
                        );
                    }
                }
            }
        }

        // HLE fallback IPL: when no external IPL is present, seed IRAM from the ROM's
        // first SA-1 window, then overlay a tiny stub that jumps to the ROM reset vector.
        // This preserves the ROM-provided IRAM tables/bootstrap data that some titles
        // expect after IPL, while still avoiding a hard dependency on an external IPL dump.
        if self.is_sa1_active() && !ipl_loaded {
            self.sa1_iram.fill(0xFF);
            self.copy_sa1_iram_from_rom(self.sa1.boot_pb, 0x0000, self.sa1_iram.len());
            let stub_offset = self.find_sa1_ipl_stub_offset();
            self.write_sa1_ipl_stub(stub_offset, reset_vec, self.sa1.boot_pb);
            self.sa1.registers.reset_vector = stub_offset;
            self.sa1.registers.control = 0x20;
            // Signal “DMA complete / BW-RAM ready” and raise SA-1→S-CPU IRQ once,
            // which matches the observable post-IPL state many games rely on.
            self.sa1.registers.sie |= Sa1::IRQ_LINE_BIT;
            self.sa1.registers.interrupt_enable = self.sa1.registers.sie;
            self.sa1.registers.interrupt_pending |= Sa1::IRQ_DMA_FLAG | Sa1::IRQ_LINE_BIT;
            if debug {
                println!(
                    "[SA1] HLE IPL injected (IRAM seeded from ROM, stub jump to {:04X})",
                    reset_vec
                );
            }
        }

        // Immediately position SA-1 core at reset vector (avoid pending_reset wiping flags)
        self.sa1.cpu.set_emulation_mode(false);
        self.sa1
            .cpu
            .set_p(crate::cpu::StatusFlags::from_bits_truncate(0x34));
        self.sa1.cpu.set_pb(self.sa1.boot_pb);
        self.sa1.cpu.set_pc(self.sa1.registers.reset_vector);
        self.sa1.boot_vector_applied = true;
        self.sa1.pending_reset = false;
        self.sa1.ipl_ran = true;
        if debug {
            println!(
                "[SA1] init vectors from ROM: reset={:04X} nmi={:04X} irq={:04X}",
                reset_vec, nmi_vec, irq_vec
            );
        }
    }

    /// Run the SA-1 core for a slice of time proportional to the S-CPU cycles just executed.
    /// We use a coarse 3:1 frequency ratio (SA-1 ~10.74MHz vs S-CPU 3.58MHz).
    pub fn run_sa1_scheduler(&mut self, cpu_cycles: u8) {
        if !self.is_sa1_active() {
            return;
        }

        // Optional: dump SA-1 IRAM/BWRAM head once for debugging
        let trace_sa1_mem = {
            static FLAG: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
            *FLAG.get_or_init(|| std::env::var_os("TRACE_SA1_MEM").is_some())
        };
        if trace_sa1_mem {
            use std::sync::atomic::{AtomicBool, Ordering};
            use std::sync::OnceLock;
            static DUMPED: OnceLock<AtomicBool> = OnceLock::new();
            let flag = DUMPED.get_or_init(|| AtomicBool::new(false));
            if !flag.swap(true, Ordering::SeqCst) {
                let iram_head: Vec<u8> = self.sa1_iram.iter().take(64).copied().collect();
                let bwram_head: Vec<u8> = self.sa1_bwram.iter().take(64).copied().collect();
                // Also dump area around 00:7DE0
                let bw_idx = 0x07DE0usize;
                let bw_slice: Vec<u8> = self
                    .sa1_bwram
                    .iter()
                    .skip(bw_idx)
                    .take(32)
                    .copied()
                    .collect();
                println!(
                    "[SA1-MEM] IRAM[0..64]={:02X?}\n[SA1-MEM] BWRAM[0..64]={:02X?}\n[SA1-MEM] BWRAM[0x07DE0..]={:02X?}",
                    iram_head, bwram_head, bw_slice
                );
            }
        }

        // Ensure vectors are seeded from ROM header at first use
        if !self.sa1.boot_vector_applied && self.sa1.registers.reset_vector == 0 {
            self.init_sa1_vectors_from_rom();
        }

        struct Sa1SchedConfig {
            ratio_num: i64,
            ratio_den: i64,
            max_steps: usize,
            batch_max: i64,
        }
        static CFG: std::sync::OnceLock<Sa1SchedConfig> = std::sync::OnceLock::new();
        let cfg = CFG.get_or_init(|| {
            let ratio_num = std::env::var("SA1_RATIO_NUM")
                .ok()
                .and_then(|v| v.parse::<i64>().ok())
                .filter(|v| *v > 0)
                .unwrap_or(3);
            let ratio_den = std::env::var("SA1_RATIO_DEN")
                .ok()
                .and_then(|v| v.parse::<i64>().ok())
                .filter(|v| *v > 0)
                .unwrap_or(1);
            let max_steps = std::env::var("SA1_MAX_STEPS")
                .ok()
                .and_then(|v| v.parse::<usize>().ok())
                .filter(|v| *v > 0)
                .unwrap_or(512);
            let batch_max = std::env::var("SA1_BATCH_MAX")
                .ok()
                .and_then(|v| v.parse::<i64>().ok())
                .filter(|v| *v > 0)
                .unwrap_or(192);
            Sa1SchedConfig {
                ratio_num,
                ratio_den,
                max_steps,
                batch_max,
            }
        });
        let sa1_ratio_num = cfg.ratio_num;
        let sa1_ratio_den = cfg.ratio_den;
        // Allow the SA-1 to catch up under heavy workloads (graphics unpack, DMA prep, etc.).
        // This helps avoid visible artifacting when the SA-1 falls behind the S-CPU.
        let sa1_max_steps = cfg.max_steps;
        let sa1_batch_max = cfg.batch_max;

        self.sa1_cycle_deficit += (cpu_cycles as i64) * sa1_ratio_num;

        if self.sa1.control_reset() {
            self.sa1.apply_pending_reset();
            self.sa1_cycle_deficit = 0;
            return;
        }

        if self.sa1.control_wait() {
            let pending = self.sa1_cycle_deficit.max(0) as u32;
            if pending > 0 {
                self.sa1.tick_timers(pending);
                self.sa1_cycles_accum_frame =
                    self.sa1_cycles_accum_frame.saturating_add(pending as u64);
            }
            self.sa1_cycle_deficit = 0;
            return;
        }

        // If DMA/CC-DMA has priority, stall SA-1 CPU execution and only advance timers.
        if self.sa1.dma_has_priority() && self.sa1.dma_busy() {
            let pending = self.sa1_cycle_deficit.max(0) as u32;
            if pending > 0 {
                self.sa1.tick_timers(pending);
                self.sa1_cycles_accum_frame =
                    self.sa1_cycles_accum_frame.saturating_add(pending as u64);
            }
            self.sa1_cycle_deficit = 0;
            return;
        }

        // If SA-1 is sleeping with no pending IRQ/NMI, just advance timers and skip execution.
        if (self.sa1.cpu.core.state.waiting_for_irq || self.sa1.cpu.core.state.stopped)
            && !self.sa1.has_pending_wakeup()
        {
            let pending = self.sa1_cycle_deficit.max(0) as u32;
            if pending > 0 {
                self.sa1.tick_timers(pending);
                self.sa1_cycles_accum_frame =
                    self.sa1_cycles_accum_frame.saturating_add(pending as u64);
            }
            self.sa1_cycle_deficit = 0;
            return;
        }

        let debug_sa1_sched = {
            static FLAG: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
            *FLAG.get_or_init(|| std::env::var_os("DEBUG_SA1_SCHEDULER").is_some())
        };
        let trace_sa1_boot = {
            static FLAG: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
            *FLAG.get_or_init(|| std::env::var_os("TRACE_SA1_BOOT").is_some())
        };
        let trace_sa1_step = {
            static FLAG: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
            *FLAG.get_or_init(|| std::env::var_os("TRACE_SA1_STEP").is_some())
        };

        // Log SA-1 reset vector on first run
        static FIRST_RUN: std::sync::Once = std::sync::Once::new();
        FIRST_RUN.call_once(|| {
            if debug_sa1_sched || trace_sa1_boot {
                println!(
                    "SA-1 first run: reset_vector=0x{:04X} PC=${:02X}:{:04X} boot_applied={}",
                    self.sa1.registers.reset_vector,
                    self.sa1.cpu.pb(),
                    self.sa1.cpu.pc(),
                    self.sa1.boot_vector_applied
                );
            }
        });

        let mut steps = 0usize;
        let mut total_sa1_cycles = 0u32;
        let mut wake_trace_left = crate::debug_flags::trace_sa1_wake_steps().unwrap_or(0);
        while self.sa1_cycle_deficit >= sa1_ratio_den && steps < sa1_max_steps {
            let mut budget = self.sa1_cycle_deficit.min(sa1_batch_max) as u16;
            if budget == 0 {
                budget = 1;
            }
            let sa1_cycles = unsafe {
                let bus_ptr = self as *mut Bus;
                let sa1_ptr = &mut self.sa1 as *mut Sa1;
                (*sa1_ptr).step_batch(&mut *bus_ptr, budget)
            } as i64;

            if sa1_cycles <= 0 {
                if debug_sa1_sched && steps == 0 {
                    println!(
                        "SA-1 scheduler: step returned 0 cycles at PC=${:02X}:{:04X}",
                        self.sa1.cpu.pb(),
                        self.sa1.cpu.pc()
                    );
                }
                break;
            }

            total_sa1_cycles = total_sa1_cycles.saturating_add(sa1_cycles as u32);

            // Optional wake trace: print first N instructions after forced IRQ poke
            if wake_trace_left > 0 {
                println!(
                    "[SA1-wake] PB={:02X} PC={:04X} cycles={} ctrl=0x{:02X} scnt=0x{:02X}",
                    self.sa1.cpu.pb(),
                    self.sa1.cpu.pc(),
                    sa1_cycles,
                    self.sa1.registers.control,
                    self.sa1.registers.scnt
                );
                wake_trace_left -= 1;
            }

            // Check if SA-1 is in WAI or STP state - if so, break early to avoid spinning
            if self.sa1.cpu.core.state.waiting_for_irq || self.sa1.cpu.core.state.stopped {
                if debug_sa1_sched {
                    println!(
                        "SA-1 scheduler: breaking at step {} (WAI={} STP={} PC=${:02X}:{:04X})",
                        steps,
                        self.sa1.cpu.core.state.waiting_for_irq,
                        self.sa1.cpu.core.state.stopped,
                        self.sa1.cpu.pb(),
                        self.sa1.cpu.pc()
                    );
                }
                break;
            }

            if trace_sa1_step && steps < 64 {
                println!(
                    "SA1 STEP {} PB={:02X} PC={:04X} cycles={} ctrl=0x{:02X} scnt=0x{:02X} WAI={} STP={}",
                    steps + 1,
                    self.sa1.cpu.pb(),
                    self.sa1.cpu.pc(),
                    sa1_cycles,
                    self.sa1.registers.control,
                    self.sa1.registers.scnt,
                    self.sa1.cpu.core.state.waiting_for_irq,
                    self.sa1.cpu.core.state.stopped,
                );
            }

            self.sa1_cycle_deficit -= sa1_cycles * sa1_ratio_den;
            steps += 1;
        }

        // Tick SA-1 timers with accumulated cycles
        if total_sa1_cycles > 0 {
            self.sa1.tick_timers(total_sa1_cycles);
            self.sa1_cycles_accum_frame = self
                .sa1_cycles_accum_frame
                .saturating_add(total_sa1_cycles as u64);
        }

        // Log statistics every 1000 steps
        if debug_sa1_sched {
            static mut STEP_COUNT: usize = 0;
            unsafe {
                STEP_COUNT += steps;
                if STEP_COUNT >= 1000 {
                    println!(
                        "SA-1 scheduler: {} total steps executed, PC=${:02X}:{:04X}",
                        STEP_COUNT,
                        self.sa1.cpu.pb(),
                        self.sa1.cpu.pc()
                    );
                    STEP_COUNT = 0;
                }
            }
        }
    }

    /// Process pending SA-1 DMA/CC-DMA transfers and notify S-CPU via IRQ
    #[inline]
    #[allow(dead_code)]
    pub fn sa1_bwram_slice(&self) -> &[u8] {
        &self.sa1_bwram
    }

    #[allow(dead_code)]
    pub fn sa1_bwram_slice_mut(&mut self) -> &mut [u8] {
        &mut self.sa1_bwram
    }

    #[inline]
    #[allow(dead_code)]
    pub fn sa1_iram_slice(&self) -> &[u8] {
        &self.sa1_iram
    }

    pub fn superfx_game_ram_slice(&self) -> Option<&[u8]> {
        self.superfx.as_ref().map(|gsu| gsu.game_ram_slice())
    }

    pub fn superfx_screen_buffer_snapshot(&self) -> Option<(Vec<u8>, u16, u8, u8)> {
        self.superfx
            .as_ref()
            .and_then(|gsu| gsu.screen_buffer_snapshot())
    }

    pub fn superfx_screen_buffer_display_snapshot(&self) -> Option<(Vec<u8>, u16, u8, u8)> {
        self.superfx
            .as_ref()
            .and_then(|gsu| gsu.screen_buffer_display_snapshot())
    }

    pub fn superfx_tile_buffer_snapshot(&self) -> Option<(Vec<u8>, u16, u8, u8)> {
        self.superfx
            .as_ref()
            .and_then(|gsu| gsu.tile_buffer_snapshot())
    }

    pub fn superfx_screen_buffer_live(&self) -> Option<(Vec<u8>, u16, u8, u8)> {
        self.superfx
            .as_ref()
            .and_then(|gsu| gsu.screen_buffer_live())
    }

    #[inline]
    #[allow(dead_code)]
    pub fn sa1_iram_slice_mut(&mut self) -> &mut [u8] {
        &mut self.sa1_iram
    }

    #[inline]
    fn bwram_protected_len(&self) -> usize {
        if self.sa1_bwram.is_empty() {
            return 0;
        }
        let area = (self.sa1.registers.bwram_protect & 0x0F) as u32;
        let size = 1024u32 << (area + 1);
        size.min(self.sa1_bwram.len() as u32) as usize
    }

    #[inline]
    fn bwram_is_protected(&self, idx: usize) -> bool {
        let protected = self.bwram_protected_len();
        protected > 0 && idx < protected
    }

    #[inline]
    fn bwram_write_allowed_scpu(&self, idx: usize) -> bool {
        // SNES BW-RAM write enable: 1 = allow writes.
        if (self.sa1.registers.sbwe & 0x80) == 0 {
            return false;
        }
        !self.bwram_is_protected(idx)
    }

    #[inline]
    pub(crate) fn bwram_write_allowed_sa1(&self, idx: usize) -> bool {
        // SA-1 BW-RAM write enable: 1 = allow writes.
        if (self.sa1.registers.cbwe & 0x80) == 0 {
            return false;
        }
        !self.bwram_is_protected(idx)
    }

    #[inline]
    fn iram_write_allowed_scpu(&self, offset: u16) -> bool {
        let bit = ((offset.wrapping_sub(0x3000)) >> 8) & 0x07;
        // SA-1 I-RAM write mask: 1=write enable for the 256B block.
        (self.sa1.registers.iram_wp_snes & (1 << bit)) != 0
    }

    #[inline]
    pub(crate) fn iram_write_allowed_sa1(&self, offset: u16) -> bool {
        let bit = ((offset & 0x7FF) >> 8) & 0x07;
        // SA-1 I-RAM write mask: 1=write enable for the 256B block.
        (self.sa1.registers.iram_wp_sa1 & (1 << bit)) != 0
    }

    #[inline]
    fn sa1_bwram_bitmap_is_2bpp(&self) -> bool {
        (self.sa1.registers.bwram_bitmap_format & 0x80) != 0
    }

    #[inline]
    fn sa1_bwram_bitmap_read(&self, bitmap_addr: usize) -> u8 {
        if self.sa1_bwram.is_empty() {
            return 0;
        }
        if self.sa1_bwram_bitmap_is_2bpp() {
            let byte_index = bitmap_addr >> 2;
            let shift = (bitmap_addr & 0x03) * 2;
            let idx = byte_index % self.sa1_bwram.len();
            (self.sa1_bwram[idx] >> shift) & 0x03
        } else {
            let byte_index = bitmap_addr >> 1;
            let shift = (bitmap_addr & 0x01) * 4;
            let idx = byte_index % self.sa1_bwram.len();
            (self.sa1_bwram[idx] >> shift) & 0x0F
        }
    }

    #[inline]
    fn sa1_bwram_bitmap_write(&mut self, bitmap_addr: usize, value: u8) {
        if self.sa1_bwram.is_empty() {
            return;
        }
        if self.sa1_bwram_bitmap_is_2bpp() {
            let byte_index = bitmap_addr >> 2;
            let shift = (bitmap_addr & 0x03) * 2;
            let idx = byte_index % self.sa1_bwram.len();
            if !self.bwram_write_allowed_sa1(idx) {
                return;
            }
            let mask = 0x03 << shift;
            let new_val = (self.sa1_bwram[idx] & !mask) | ((value & 0x03) << shift);
            self.sa1_bwram[idx] = new_val;
        } else {
            let byte_index = bitmap_addr >> 1;
            let shift = (bitmap_addr & 0x01) * 4;
            let idx = byte_index % self.sa1_bwram.len();
            if !self.bwram_write_allowed_sa1(idx) {
                return;
            }
            let mask = 0x0F << shift;
            let new_val = (self.sa1_bwram[idx] & !mask) | ((value & 0x0F) << shift);
            self.sa1_bwram[idx] = new_val;
        }
    }

    #[inline]
    fn sa1_bwram_bitmap_addr_from_window(&self, offset: u16) -> Option<usize> {
        if offset < 0x6000 {
            return None;
        }
        let select = self.sa1.registers.bwram_select_sa1;
        if (select & 0x80) == 0 {
            return None;
        }
        let block = (select & 0x7F) as usize;
        let base = block << 13; // 8 KB blocks in bitmap address space
        Some(base + (offset - 0x6000) as usize)
    }

    /// Copy a slice from SA-1 ROM into SA-1 IRAM (used to emulate the missing SA-1 IPL).
    #[allow(dead_code)]
    fn copy_sa1_iram_from_rom(&mut self, bank: u8, offset: u16, len: usize) {
        let dst = &mut self.sa1_iram;
        let mut remaining = len.min(dst.len());
        let mut off = offset as usize;
        let mut written = 0usize;
        while remaining > 0 {
            let phys = {
                let b = bank as u32;
                let o = (off & 0xFFFF) as u16;
                // Compute without borrowing dst

                if (0x00..=0x1F).contains(&b)
                    || (0x20..=0x3F).contains(&b)
                    || (0x80..=0x9F).contains(&b)
                    || (0xA0..=0xBF).contains(&b)
                {
                    let chunk = match b {
                        0x00..=0x1F => self.sa1.registers.mmc_bank_c,
                        0x20..=0x3F => self.sa1.registers.mmc_bank_d,
                        0x80..=0x9F => self.sa1.registers.mmc_bank_e,
                        _ => self.sa1.registers.mmc_bank_f,
                    } as usize;
                    let off = (o | 0x8000) as usize;
                    let bank_lo = (b & 0x1F) as usize;
                    chunk * 0x100000 + bank_lo * 0x8000 + (off - 0x8000)
                } else {
                    let chunk = match b {
                        0xC0..=0xCF => self.sa1.registers.mmc_bank_c,
                        0xD0..=0xDF => self.sa1.registers.mmc_bank_d,
                        0xE0..=0xEF => self.sa1.registers.mmc_bank_e,
                        _ => self.sa1.registers.mmc_bank_f,
                    } as usize;
                    chunk * 0x100000 + o as usize
                }
            };
            let byte = self.rom.get(phys % self.rom_size).copied().unwrap_or(0x00);
            dst[written] = byte;
            written += 1;
            off = off.wrapping_add(1);
            remaining -= 1;
        }
        if std::env::var_os("TRACE_SA1_BOOT").is_some() {
            println!(
                "[SA1] IRAM filled from ROM bank {:02X} offset 0x{:04X} len=0x{:04X}",
                bank, offset, len
            );
        }
    }

    /// Find the least invasive location for the HLE SA-1 IPL stub.
    /// Prefer an unused 0xFF-filled gap, otherwise fall back to the IRAM tail.
    fn find_sa1_ipl_stub_offset(&self) -> u16 {
        const STUB_LEN: usize = 4;
        if self.sa1_iram.len() <= STUB_LEN {
            return 0;
        }

        for start in (0..=self.sa1_iram.len() - STUB_LEN).rev() {
            if self.sa1_iram[start..start + STUB_LEN]
                .iter()
                .all(|&byte| byte == 0xFF)
            {
                return start as u16;
            }
        }

        (self.sa1_iram.len() - STUB_LEN) as u16
    }

    /// Minimal SA-1 IPL stub: place a JML at the chosen IRAM offset.
    #[allow(dead_code)]
    fn write_sa1_ipl_stub(&mut self, stub_offset: u16, target_addr: u16, target_bank: u8) {
        let stub_offset = stub_offset as usize;
        // JML absolute long: opcode 0x5C
        self.sa1_iram[stub_offset] = 0x5C;
        self.sa1_iram[stub_offset + 1] = (target_addr & 0xFF) as u8;
        self.sa1_iram[stub_offset + 2] = (target_addr >> 8) as u8;
        self.sa1_iram[stub_offset + 3] = target_bank;
        // After jump, unused
        if std::env::var_os("TRACE_SA1_BOOT").is_some() {
            println!(
                "[SA1] IPL stub @ ${:04X} -> JML ${:02X}:{:04X}",
                stub_offset, target_bank, target_addr
            );
        }
    }

    /// SA-1 CPU側のROM/BWRAMリード
    pub fn sa1_read_u8(&mut self, addr: u32) -> u8 {
        let bank = (addr >> 16) & 0xFF;
        let offset = (addr & 0xFFFF) as u16;
        match bank {
            0x00..=0x3F | 0x80..=0xBF => {
                // SA-1 I-RAM (2KB) mapped at 00:0000-07FF for SA-1 CPU
                if (0x0000..=0x07FF).contains(&offset) {
                    return self.sa1_iram[(offset as usize) % self.sa1_iram.len()];
                }
                // Mirror at 00:3000-37FF
                if (0x3000..=0x37FF).contains(&offset) {
                    let idx = (offset - 0x3000) as usize;
                    return self.sa1_iram[idx % self.sa1_iram.len()];
                }
                if (0x6000..=0x7FFF).contains(&offset) {
                    if let Some(bitmap_addr) = self.sa1_bwram_bitmap_addr_from_window(offset) {
                        return self.sa1_bwram_bitmap_read(bitmap_addr);
                    }
                    if let Some(idx) = self.sa1_cpu_bwram_addr(offset) {
                        return self.sa1_bwram[idx];
                    }
                }
                // SA-1 CPU can access its control registers in this window
                if (0x2200..=0x23FF).contains(&offset) {
                    if crate::debug_flags::trace_sa1_reg() {
                        use std::sync::atomic::{AtomicU32, Ordering};
                        static COUNT: AtomicU32 = AtomicU32::new(0);
                        let n = COUNT.fetch_add(1, Ordering::Relaxed);
                        if n < 64 {
                            println!("SA1 REG R (SA1) {:02X}:{:04X} = (deferred)", bank, offset);
                        }
                    }
                    return match offset - 0x2200 {
                        0x10C => self.read_sa1_varlen_port(false),
                        0x10D => self.read_sa1_varlen_port(true),
                        reg => self.sa1.read_register(reg),
                    };
                }
                let phys = self.sa1_phys_addr(bank, offset);
                self.rom.get(phys % self.rom_size).copied().unwrap_or(0xFF)
            }
            0x60..=0x6F => {
                let bitmap_addr = ((bank - 0x60) as usize) << 16 | (offset as usize);
                self.sa1_bwram_bitmap_read(bitmap_addr)
            }
            0x40..=0x5F => {
                // Direct BWRAM access for SA-1
                let idx = ((bank & 0x1F) as usize) << 16 | (offset as usize);
                self.sa1_bwram
                    .get(idx % self.sa1_bwram.len())
                    .copied()
                    .unwrap_or(0)
            }
            0xC0..=0xFF => {
                let phys = self.sa1_phys_addr(bank, offset);
                self.rom.get(phys % self.rom_size).copied().unwrap_or(0xFF)
            }
            _ => 0xFF,
        }
    }

    pub fn sa1_write_u8(&mut self, addr: u32, value: u8) {
        let bank = (addr >> 16) & 0xFF;
        let offset = (addr & 0xFFFF) as u16;
        match bank {
            0x00..=0x3F | 0x80..=0xBF => {
                // SA-1 I-RAM (2KB) mapped at 00:0000-07FF for SA-1 CPU
                if (0x0000..=0x07FF).contains(&offset) {
                    let idx = (offset as usize) % self.sa1_iram.len();
                    if self.iram_write_allowed_sa1(offset) {
                        self.sa1_iram[idx] = value;
                    }
                    return;
                }
                // Mirror at 00:3000-37FF
                if (0x3000..=0x37FF).contains(&offset) {
                    let idx = ((offset - 0x3000) as usize) % self.sa1_iram.len();
                    if self.iram_write_allowed_sa1(offset) {
                        self.sa1_iram[idx] = value;
                    }
                    return;
                }
                if (0x6000..=0x7FFF).contains(&offset) {
                    // Use SA-1 CPU's own BWRAM mapping register
                    if let Some(bitmap_addr) = self.sa1_bwram_bitmap_addr_from_window(offset) {
                        self.sa1_bwram_bitmap_write(bitmap_addr, value);
                    } else if let Some(idx) = self.sa1_cpu_bwram_addr(offset) {
                        if self.bwram_write_allowed_sa1(idx) {
                            self.sa1_bwram[idx] = value;
                        }
                    }
                }
                // SA-1 CPU access to its registers
                if (0x2200..=0x23FF).contains(&offset) {
                    if crate::debug_flags::trace_sa1_reg() {
                        use std::sync::atomic::{AtomicU32, Ordering};
                        static COUNT: AtomicU32 = AtomicU32::new(0);
                        let n = COUNT.fetch_add(1, Ordering::Relaxed);
                        if n < 64 {
                            println!(
                                "SA1 REG W (SA1) {:02X}:{:04X} = {:02X}",
                                bank, offset, value
                            );
                        }
                    }
                    self.sa1.write_register_sa1(offset - 0x2200, value);
                }
            }
            0x60..=0x6F => {
                let bitmap_addr = ((bank - 0x60) as usize) << 16 | (offset as usize);
                self.sa1_bwram_bitmap_write(bitmap_addr, value);
            }
            0x40..=0x5F => {
                // Direct BWRAM access for SA-1
                let idx = ((bank & 0x1F) as usize) << 16 | (offset as usize);
                if !self.sa1_bwram.is_empty() {
                    let actual = idx % self.sa1_bwram.len();
                    if self.bwram_write_allowed_sa1(actual) {
                        self.sa1_bwram[actual] = value;
                    }
                }
            }
            _ => {}
        }
    }

    pub fn read_u16(&mut self, addr: u32) -> u16 {
        // CPUテストROMで $4210 を16bit読みした場合、上位バイトにも bit7 を複製して
        // BIT (16bit) でも VBlank フラグを検出できるようにする。
        if self.cpu_test_mode && addr == 0x004210 {
            let lo = self.read_u8(addr) as u16;
            let hi = if (lo & 0x80) != 0 { 0x80 } else { 0x00 };
            return (hi << 8) | lo;
        }

        let lo = self.read_u8(addr) as u16;
        let hi = self.read_u8(addr.wrapping_add(1)) as u16;
        (hi << 8) | lo
    }

    #[allow(dead_code)]
    pub fn write_u16(&mut self, addr: u32, value: u16) {
        if crate::debug_flags::trace_apu_u16() {
            let off = (addr & 0xFFFF) as u16;
            if (0x2140..=0x2143).contains(&off) {
                println!(
                    "[APU-U16] PC={:06X} ${:04X} <- {:04X}",
                    self.last_cpu_pc, off, value
                );
            }
        }
        self.write_u8(addr, (value & 0xFF) as u8);
        self.write_u8(addr.wrapping_add(1), (value >> 8) as u8);
    }

    // --- Save-state helpers (WRAM/SRAM and simple IO) ---
    pub fn snapshot_memory(&self) -> (Vec<u8>, Vec<u8>) {
        (self.wram.clone(), self.sram.clone())
    }

    pub fn restore_memory(&mut self, wram: &[u8], sram: &[u8]) {
        if self.wram.len() == wram.len() {
            self.wram.copy_from_slice(wram);
        }
        if self.sram.len() == sram.len() {
            self.sram.copy_from_slice(sram);
            self.sram_dirty = false;
        }
    }

    // --- SRAM access/persistence helpers ---
    pub fn sram(&self) -> &[u8] {
        &self.sram
    }
    pub fn sram_mut(&mut self) -> &mut [u8] {
        &mut self.sram
    }
    pub fn sram_size(&self) -> usize {
        self.sram_size
    }
    pub fn is_sram_dirty(&self) -> bool {
        self.sram_dirty
    }
    pub fn clear_sram_dirty(&mut self) {
        self.sram_dirty = false;
    }

    pub fn get_input_system(&self) -> &crate::input::InputSystem {
        &self.input_system
    }

    #[allow(dead_code)]
    fn read_expansion(&mut self, _addr: u32) -> u8 {
        // Unmapped expansion/coprocessor windows read as open bus unless a mapper hooks them.
        self.mdr
    }

    #[allow(dead_code)]
    fn write_expansion(&mut self, _addr: u32, _value: u8) {
        // Unmapped expansion/coprocessor windows ignore writes.
    }

    pub fn get_ppu(&self) -> &crate::ppu::Ppu {
        &self.ppu
    }

    pub fn get_ppu_mut(&mut self) -> &mut crate::ppu::Ppu {
        &mut self.ppu
    }

    /// 現在のNMITIMEN値（$4200）を取得（デバッグ/フォールバック用）
    #[inline]
    #[allow(dead_code)]
    pub fn nmitimen(&self) -> u8 {
        self.nmitimen
    }

    pub fn to_save_state(&self) -> BusSaveState {
        let sa1_state = if self.is_sa1_active() {
            let cpu_state = self.sa1.cpu.get_state();
            Some(crate::savestate::Sa1SaveState {
                cpu_state: crate::savestate::CpuSaveState {
                    a: cpu_state.a,
                    x: cpu_state.x,
                    y: cpu_state.y,
                    sp: cpu_state.sp,
                    dp: cpu_state.dp,
                    db: cpu_state.db,
                    pb: cpu_state.pb,
                    pc: cpu_state.pc,
                    p: cpu_state.p,
                    emulation_mode: cpu_state.emulation_mode,
                    cycles: cpu_state.cycles,
                    waiting_for_irq: cpu_state.waiting_for_irq,
                    stopped: cpu_state.stopped,
                    deferred_fetch: cpu_state.deferred_fetch.map(|fetch| {
                        crate::savestate::CpuDeferredFetchSaveState {
                            opcode: fetch.opcode,
                            memspeed_penalty: fetch.memspeed_penalty,
                            pc_before: fetch.pc_before,
                            full_addr: fetch.full_addr,
                        }
                    }),
                },
                registers: self.sa1.registers.clone(),
                boot_vector_applied: self.sa1.boot_vector_applied,
                boot_pb: self.sa1.boot_pb,
                pending_reset: self.sa1.pending_reset,
                hold_reset: self.sa1.hold_reset,
                ipl_ran: self.sa1.ipl_ran,
                h_timer_accum: self.sa1.h_timer_accum,
                v_timer_accum: self.sa1.v_timer_accum,
                math_cycles_left: self.sa1.math_cycles_left,
                math_pending_result: self.sa1.math_pending_result,
                math_pending_overflow: self.sa1.math_pending_overflow,
                bwram: self.sa1_bwram.clone(),
                iram: self.sa1_iram.to_vec(),
                cycle_deficit: self.sa1_cycle_deficit,
                cycles_accum_frame: self.sa1_cycles_accum_frame,
                nmi_delay_active: self.sa1_nmi_delay_active,
            })
        } else {
            None
        };
        let spc7110_state = self.spc7110.as_ref().map(|spc| spc.save_data());
        let superfx_state = self.superfx.as_ref().map(|gsu| gsu.save_data());
        BusSaveState {
            nmitimen: self.nmitimen,
            wram_address: self.wram_address,
            mdr: self.mdr,
            mul_a: self.mul_a,
            mul_b: self.mul_b,
            mul_result: self.mul_result,
            div_a: self.div_a,
            div_b: self.div_b,
            div_quot: self.div_quot,
            div_rem: self.div_rem,
            mul_busy: self.mul_busy,
            mul_just_started: self.mul_just_started,
            mul_cycles_left: self.mul_cycles_left,
            mul_work_a: self.mul_work_a,
            mul_work_b: self.mul_work_b,
            mul_partial: self.mul_partial,
            div_busy: self.div_busy,
            div_just_started: self.div_just_started,
            div_cycles_left: self.div_cycles_left,
            div_work_dividend: self.div_work_dividend,
            div_work_divisor: self.div_work_divisor,
            div_work_quot: self.div_work_quot,
            div_work_rem: self.div_work_rem,
            div_work_bit: self.div_work_bit,
            cpu_instr_active: self.cpu_instr_active,
            cpu_instr_bus_cycles: self.cpu_instr_bus_cycles,
            cpu_instr_extra_master_cycles: self.cpu_instr_extra_master_cycles,
            irq_h_enabled: self.irq_h_enabled,
            irq_v_enabled: self.irq_v_enabled,
            irq_pending: self.irq_pending,
            irq_v_matched_line: self.irq_v_matched_line,
            h_timer: self.h_timer,
            v_timer: self.v_timer,
            h_timer_set: self.h_timer_set,
            v_timer_set: self.v_timer_set,
            joy_busy_counter: self.joy_busy_counter,
            joy_data: self.joy_data,
            joy_busy_scanlines: self.joy_busy_scanlines,
            pending_gdma_mask: self.pending_gdma_mask,
            pending_mdma_mask: self.pending_mdma_mask,
            mdma_started_after_opcode_fetch: self.mdma_started_after_opcode_fetch,
            rdnmi_consumed: self.rdnmi_consumed,
            rdnmi_high_byte_for_test: self.rdnmi_high_byte_for_test,
            pending_stall_master_cycles: self.pending_stall_master_cycles,
            smw_apu_hle: self.smw_apu_hle,
            smw_apu_hle_done: self.smw_apu_hle_done,
            smw_apu_hle_buf: self.smw_apu_hle_buf.clone(),
            smw_apu_hle_echo_idx: self.smw_apu_hle_echo_idx,
            wio: self.wio,
            fastrom: self.fastrom,
            dma_state: self.dma_controller.to_save_state(),
            spc7110_state,
            superfx_state,
            sa1_state,
        }
    }

    pub fn load_from_save_state(&mut self, st: &BusSaveState) {
        self.nmitimen = st.nmitimen;
        self.wram_address = st.wram_address;
        self.mdr = st.mdr;
        self.mul_a = st.mul_a;
        self.mul_b = st.mul_b;
        self.mul_result = st.mul_result;
        self.div_a = st.div_a;
        self.div_b = st.div_b;
        self.div_quot = st.div_quot;
        self.div_rem = st.div_rem;
        self.mul_busy = st.mul_busy;
        self.mul_just_started = st.mul_just_started;
        self.mul_cycles_left = st.mul_cycles_left;
        self.mul_work_a = st.mul_work_a;
        self.mul_work_b = st.mul_work_b;
        self.mul_partial = st.mul_partial;
        self.div_busy = st.div_busy;
        self.div_just_started = st.div_just_started;
        self.div_cycles_left = st.div_cycles_left;
        self.div_work_dividend = st.div_work_dividend;
        self.div_work_divisor = st.div_work_divisor;
        self.div_work_quot = st.div_work_quot;
        self.div_work_rem = st.div_work_rem;
        self.div_work_bit = st.div_work_bit;
        self.cpu_instr_active = st.cpu_instr_active;
        self.cpu_instr_bus_cycles = st.cpu_instr_bus_cycles;
        self.cpu_instr_extra_master_cycles = st.cpu_instr_extra_master_cycles;
        self.irq_h_enabled = st.irq_h_enabled;
        self.irq_v_enabled = st.irq_v_enabled;
        self.irq_pending = st.irq_pending;
        self.irq_v_matched_line = st.irq_v_matched_line;
        self.h_timer = st.h_timer;
        self.v_timer = st.v_timer;
        self.h_timer_set = st.h_timer_set;
        self.v_timer_set = st.v_timer_set;
        self.joy_busy_counter = st.joy_busy_counter;
        self.joy_data = st.joy_data;
        // Normalize auto-joy busy duration on load.
        // Old save states may carry legacy values (e.g. 8) that make input feel sluggish.
        self.joy_busy_scanlines = std::env::var("JOYBUSY_SCANLINES")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(3);
        self.joy_busy_counter = self.joy_busy_counter.min(self.joy_busy_scanlines);
        self.pending_gdma_mask = st.pending_gdma_mask;
        self.pending_mdma_mask = st.pending_mdma_mask;
        self.mdma_started_after_opcode_fetch = st.mdma_started_after_opcode_fetch;
        self.superfx_status_poll_pc = 0;
        self.superfx_status_poll_streak = 0;
        self.starfox_exact_wait_assist_frame = u64::MAX;
        self.rdnmi_consumed = st.rdnmi_consumed;
        self.rdnmi_high_byte_for_test = st.rdnmi_high_byte_for_test;
        self.pending_stall_master_cycles = st.pending_stall_master_cycles;
        self.smw_apu_hle = st.smw_apu_hle;
        self.smw_apu_hle_done = st.smw_apu_hle_done;
        self.smw_apu_hle_buf = st.smw_apu_hle_buf.clone();
        self.smw_apu_hle_echo_idx = st.smw_apu_hle_echo_idx;
        self.wio = st.wio;
        self.fastrom = st.fastrom;
        self.dma_controller.load_from_save_state(&st.dma_state);
        if let (Some(spc), Some(state)) = (self.spc7110.as_mut(), st.spc7110_state.as_ref()) {
            spc.load_data(state);
        }
        if let (Some(gsu), Some(state)) = (self.superfx.as_mut(), st.superfx_state.as_ref()) {
            gsu.load_data(state);
        }
        if self.is_sa1_active() {
            if let Some(sa1_state) = &st.sa1_state {
                self.sa1.cpu.set_state(crate::cpu::CpuState {
                    a: sa1_state.cpu_state.a,
                    x: sa1_state.cpu_state.x,
                    y: sa1_state.cpu_state.y,
                    sp: sa1_state.cpu_state.sp,
                    dp: sa1_state.cpu_state.dp,
                    db: sa1_state.cpu_state.db,
                    pb: sa1_state.cpu_state.pb,
                    pc: sa1_state.cpu_state.pc,
                    p: sa1_state.cpu_state.p,
                    emulation_mode: sa1_state.cpu_state.emulation_mode,
                    cycles: sa1_state.cpu_state.cycles,
                    waiting_for_irq: sa1_state.cpu_state.waiting_for_irq,
                    stopped: sa1_state.cpu_state.stopped,
                    deferred_fetch: sa1_state.cpu_state.deferred_fetch.map(|fetch| {
                        crate::cpu::core::DeferredFetchState {
                            opcode: fetch.opcode,
                            memspeed_penalty: fetch.memspeed_penalty,
                            pc_before: fetch.pc_before,
                            full_addr: fetch.full_addr,
                        }
                    }),
                });
                self.sa1.registers = sa1_state.registers.clone();
                self.sa1.boot_vector_applied = sa1_state.boot_vector_applied;
                self.sa1.boot_pb = sa1_state.boot_pb;
                self.sa1.pending_reset = sa1_state.pending_reset;
                self.sa1.hold_reset = sa1_state.hold_reset;
                self.sa1.ipl_ran = sa1_state.ipl_ran;
                self.sa1.h_timer_accum = sa1_state.h_timer_accum;
                self.sa1.v_timer_accum = sa1_state.v_timer_accum;
                self.sa1.math_cycles_left = sa1_state.math_cycles_left;
                self.sa1.math_pending_result = sa1_state.math_pending_result;
                self.sa1.math_pending_overflow = sa1_state.math_pending_overflow;
                self.sa1_bwram = sa1_state.bwram.clone();
                self.sa1_iram.fill(0);
                let copy_len = self.sa1_iram.len().min(sa1_state.iram.len());
                self.sa1_iram[..copy_len].copy_from_slice(&sa1_state.iram[..copy_len]);
                self.sa1_cycle_deficit = sa1_state.cycle_deficit;
                self.sa1_cycles_accum_frame = sa1_state.cycles_accum_frame;
                self.sa1_nmi_delay_active = sa1_state.nmi_delay_active;
            } else {
                self.sa1.math_cycles_left = 0;
                self.sa1.math_pending_result = 0;
                self.sa1.math_pending_overflow = false;
                self.sa1_cycle_deficit = 0;
                self.sa1_cycles_accum_frame = 0;
                self.sa1_nmi_delay_active = false;
            }
        } else {
            self.sa1.math_cycles_left = 0;
            self.sa1.math_pending_result = 0;
            self.sa1.math_pending_overflow = false;
            self.sa1_cycles_accum_frame = 0;
        }
    }

    // Debug accessor for JOYBUSY counter (auto-joypad in progress)
    pub fn joy_busy_counter(&self) -> u8 {
        self.joy_busy_counter
    }

    /// CPUテストROM向けのPASS/FAIL検出を有効化する（入力は注入しない）
    pub fn enable_cpu_test_mode(&mut self) {
        self.cpu_test_mode = true;
        self.cpu_test_result = None;
    }

    #[inline]
    pub fn is_cpu_test_mode(&self) -> bool {
        self.cpu_test_mode
    }

    pub fn take_cpu_test_result(&mut self) -> Option<CpuTestResult> {
        self.cpu_test_result.take()
    }

    // --- ROM mapping helpers (approximate) ---
    pub fn is_rom_address(&self, addr: u32) -> bool {
        let bank = ((addr >> 16) & 0xFF) as u8;
        let off = (addr & 0xFFFF) as u16;
        if let Some(ref mapper) = self.mapper {
            mapper.is_rom_address(bank, off)
        } else {
            // SA-1/DQ3: use LoROM-style check
            match bank {
                0x00..=0x3F | 0x80..=0xBF => off >= 0x8000,
                0x40..=0x7D | 0xC0..=0xFF => off >= 0x8000,
                _ => false,
            }
        }
    }

    #[inline]
    pub fn is_fastrom(&self) -> bool {
        self.fastrom
    }

    pub fn get_apu_shared(&self) -> Arc<Mutex<crate::audio::apu::Apu>> {
        self.apu.clone()
    }

    #[inline]
    pub fn with_apu_mut<F>(&mut self, f: F)
    where
        F: FnOnce(&mut crate::audio::apu::Apu),
    {
        if let Some(apu_mutex) = Arc::get_mut(&mut self.apu) {
            let apu = apu_mutex.get_mut().unwrap_or_else(|e| e.into_inner());
            f(apu);
            return;
        }

        let mut apu = self.apu.lock().unwrap_or_else(|e| e.into_inner());
        f(&mut apu);
    }

    #[inline]
    #[allow(dead_code)]
    pub fn try_with_apu_mut<F>(&mut self, f: F) -> bool
    where
        F: FnOnce(&mut crate::audio::apu::Apu),
    {
        if let Some(apu_mutex) = Arc::get_mut(&mut self.apu) {
            let apu = apu_mutex.get_mut().unwrap_or_else(|e| e.into_inner());
            f(apu);
            return true;
        }

        if let Ok(mut apu) = self.apu.try_lock() {
            f(&mut apu);
            true
        } else {
            false
        }
    }

    pub fn set_mapper_type(&mut self, mapper: crate::cartridge::MapperType) {
        self.mapper_type = mapper;
        self.mapper = crate::cartridge::mapper::MapperImpl::from_type(mapper);
    }

    pub fn get_mapper_type(&self) -> crate::cartridge::MapperType {
        self.mapper_type
    }

    pub fn get_input_system_mut(&mut self) -> &mut crate::input::InputSystem {
        &mut self.input_system
    }

    // Headless init counters (for concise summary)
    pub fn get_init_counters(&self) -> (u32, u32, u32, u32) {
        (
            self.nmitimen_writes_count,
            self.mdmaen_nonzero_count,
            self.hdmaen_nonzero_count,
            self.dma_reg_writes,
        )
    }

    // Short DMA config summary for INIT logs
    pub fn get_dma_config_summary(&self) -> String {
        let mut parts = Vec::new();
        for (i, ch) in self.dma_controller.channels.iter().enumerate() {
            let mut flags = String::new();
            if ch.cfg_ctrl {
                flags.push('C');
            }
            if ch.cfg_dest {
                flags.push('D');
            }
            if ch.cfg_src {
                flags.push('S');
            }
            if ch.cfg_size {
                flags.push('Z');
            }
            if !flags.is_empty() {
                parts.push(format!("ch{}:{}", i, flags));
            }
        }
        if parts.is_empty() {
            "DMAcfg:none".to_string()
        } else {
            format!("DMAcfg:{}", parts.join(","))
        }
    }

    pub fn irq_is_pending(&mut self) -> bool {
        if self.irq_pending {
            return true;
        }
        // SA-1 -> S-CPU IRQ (via CIE mask)
        if self.is_sa1_active() {
            // S-CPU can only see SA-1 IRQ when SIE permits it.
            if self.sa1.scpu_irq_asserted() {
                return true;
            }
        }
        if self.is_superfx_active()
            && self
                .superfx
                .as_ref()
                .is_some_and(|gsu| gsu.scpu_irq_asserted())
        {
            return true;
        }
        false
    }

    #[allow(dead_code)]
    pub fn clear_irq_pending(&mut self) {
        self.irq_pending = false;
    }

    /// Tick CPU-cycle based peripherals (currently: hardware math).
    /// Call once per executed S-CPU instruction slice with the number of cycles consumed.
    pub fn tick_cpu_cycles(&mut self, cpu_cycles: u8) {
        if cpu_cycles == 0 {
            return;
        }
        let profile_enabled = trace_starfox_slow_profile_enabled();
        let profile_start = profile_enabled.then(Instant::now);

        if self.is_superfx_active() {
            let rom = &self.rom as *const Vec<u8>;
            if let Some(gsu) = self.superfx.as_mut() {
                unsafe {
                    gsu.run_for_cpu_cycles(&*rom, cpu_cycles);
                }
            }
        }

        // Fast path: no in-flight math units after advancing coprocessors.
        if !self.mul_busy && !self.div_busy {
            return;
        }

        for _ in 0..cpu_cycles {
            if self.mul_busy {
                // Defer by 1 CPU cycle so we don't advance within the same cycle as the
                // start write (WRMPYB). This matches common documentation and is enough
                // to satisfy in-flight test ROMs.
                if self.mul_just_started {
                    self.mul_just_started = false;
                    continue;
                }
                if self.mul_cycles_left == 0 {
                    self.mul_busy = false;
                    continue;
                }
                if (self.mul_work_b & 1) != 0 {
                    self.mul_partial = self.mul_partial.wrapping_add(self.mul_work_a);
                }
                self.mul_work_b >>= 1;
                self.mul_work_a = self.mul_work_a.wrapping_shl(1);
                self.mul_cycles_left = self.mul_cycles_left.saturating_sub(1);
                self.mul_result = self.mul_partial;
                if self.mul_cycles_left == 0 {
                    self.mul_busy = false;
                }
                continue;
            }

            if self.div_busy {
                // Defer by 1 CPU cycle so we don't advance within the same cycle as the
                // start write (WRDIVB).
                if self.div_just_started {
                    self.div_just_started = false;
                    continue;
                }
                if self.div_cycles_left == 0 {
                    self.div_busy = false;
                    continue;
                }
                let divisor = self.div_work_divisor as u16;
                if divisor == 0 {
                    // Shouldn't happen (handled on start), but keep behavior safe.
                    self.div_quot = 0xFFFF;
                    self.div_rem = self.div_work_dividend;
                    self.mul_result = self.div_rem;
                    self.div_busy = false;
                    continue;
                }

                let bit = self.div_work_bit;
                if bit < 0 {
                    // Completed.
                    self.div_quot = self.div_work_quot;
                    self.div_rem = self.div_work_rem;
                    self.mul_result = self.div_rem;
                    self.div_busy = false;
                    continue;
                }

                let next = (self.div_work_dividend >> (bit as u16)) & 1;
                self.div_work_rem = (self.div_work_rem << 1) | next;
                if self.div_work_rem >= divisor {
                    self.div_work_rem = self.div_work_rem.wrapping_sub(divisor);
                    self.div_work_quot |= 1u16 << (bit as u16);
                }
                self.div_work_bit = self.div_work_bit.saturating_sub(1);
                self.div_cycles_left = self.div_cycles_left.saturating_sub(1);

                // Expose intermediate state through result registers.
                self.div_quot = self.div_work_quot;
                self.div_rem = self.div_work_rem;
                self.mul_result = self.div_work_rem;

                if self.div_cycles_left == 0 {
                    self.div_busy = false;
                }
            }
        }
        if let Some(start) = profile_start {
            self.cpu_profile_tick_ns = self
                .cpu_profile_tick_ns
                .saturating_add(start.elapsed().as_nanos() as u64);
            self.cpu_profile_tick_count = self.cpu_profile_tick_count.saturating_add(1);
        }
    }

    /// Tick only the SuperFX scheduler for the given elapsed S-CPU cycles.
    ///
    /// This is used when master time advances without executing S-CPU instructions
    /// (slow-memory extra clocks, DMA stalls, frame-boundary catchup). Do not route
    /// through `tick_cpu_cycles`, because that would also advance unrelated S-CPU
    /// hardware units such as the internal multiply/divide state machines.
    pub fn tick_superfx_cpu_cycles(&mut self, cpu_cycles: u8) {
        if cpu_cycles == 0 || !self.is_superfx_active() {
            return;
        }

        let rom = &self.rom as *const Vec<u8>;
        if let Some(gsu) = self.superfx.as_mut() {
            unsafe {
                gsu.run_for_cpu_cycles(&*rom, cpu_cycles);
            }
        }
    }

    #[cfg(debug_assertions)]
    #[allow(dead_code)]
    pub fn ppu_vram_snapshot(&self) -> Vec<u8> {
        self.ppu.get_vram().to_vec()
    }
}
