use std::sync::{Arc, Mutex};

use crate::cartridge::sa1::Sa1;

use super::Bus;

impl Bus {
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
}
