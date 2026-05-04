use std::sync::{Arc, Mutex};

use crate::cartridge::sa1::Sa1;

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
    pub(crate) any_read_trace_active: bool,
    pub(crate) cpu_profile_read_ns: u64,
    pub(crate) cpu_profile_write_ns: u64,
    pub(crate) cpu_profile_bus_cycle_ns: u64,
    pub(crate) cpu_profile_tick_ns: u64,
    pub(crate) cpu_profile_read_count: u32,
    pub(crate) cpu_profile_write_count: u32,
    pub(crate) cpu_profile_bus_cycle_count: u32,
    pub(crate) cpu_profile_tick_count: u32,
    pub(crate) cpu_profile_read_bank_ns: [u64; 256],
    pub(crate) cpu_profile_read_bank_count: [u32; 256],
}
