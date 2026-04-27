//! APU wrapper using the external `snes-apu` crate (SPC700 + DSP).
//! 現状: 精度よりも動作優先の簡易統合。セーブステートは内部状態を保存/復元。
mod config;
mod ports;

use super::spc::apu::{Apu as SpcApu, ApuState as SpcApuState};
use super::spc::smp::SmpState as SpcSmpState;
use super::spc::TimerState as SpcTimerState;
use config::{f64_to_fixed32, ApuConfig};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BootState {
    /// 初期シグネチャ (AA/BB) をCPUに見せる段階
    ReadySignature,
    /// CPUからのキック(0xCC)を待ち、転送中
    Uploading,
    /// APUプログラム稼働中。以降は実ポート値をそのまま返す
    Running,
}

pub struct Apu {
    pub(crate) inner: Box<SpcApu>,
    sample_rate: u32,
    // Reusable audio scratch buffers to avoid per-frame allocations.
    audio_left: Vec<i16>,
    audio_right: Vec<i16>,
    // Last sample emitted (for gentle underflow fill).
    last_audio_sample: (i16, i16),
    // Fractional SPC700 cycles accumulator (Q0.32 fixed-point, scaled from S-CPU cycles).
    cycle_accum_fixed: u64,
    cycle_scale_fixed: u64,
    // Pre-computed master-clock scale (cycle_scale_fixed / 6) for step_master_cycles().
    cycle_scale_master_fixed: u64,
    // Total SPC700 cycles executed (debug/diagnostics).
    pub(crate) total_smp_cycles: u64,
    // Debug: last observed values for $F1/$FA change tracing.
    trace_last_f1: u8,
    trace_last_fa: u8,
    // Debug: last observed APU->CPU port values (for change tracing).
    trace_last_out_ports: [u8; 4],
    // CPU<-APU方向のホストポート（CPUが読み取る値）
    apu_to_cpu_ports: [u8; 4],
    // CPU->APU方向の直近値（SMP側が$F4-$F7で読む値）
    pub(crate) port_latch: [u8; 4],
    // CPU->APU 書き込みの遅延適用キュー（APU側の同期遅延を近似）
    pending_port_writes: Vec<(u8, u8)>,
    boot_state: BootState,
    boot_hle_enabled: bool,
    fast_upload: bool,
    #[allow(dead_code)]
    fast_upload_bytes: u64,
    zero_write_seen: bool,
    last_port1: u8,
    upload_addr: u16,
    expected_index: u8,
    block_active: bool,
    pending_idx: Option<u8>,
    pending_cmd: Option<u8>,
    data_ready: bool,
    loader_hle_active: bool,
    loader_hle_enabled: bool,
    loader_hle_has_resume: bool,
    loader_hle_resume_pc: u16,
    loader_hle_resume_sp: u8,
    loader_ready_stall_reads: u16,
    upload_done_count: u64,
    upload_bytes: u64,
    last_upload_idx: u8,
    #[allow(dead_code)]
    end_zero_streak: u8,
    // Last value written to port0 during boot; echoed until next write.
    boot_port0_echo: u8,
    // Optional hard override for port0 echo (debug)
    force_port0: Option<u8>,
    // Whether skip-boot was requested (for consistent init)
    #[allow(dead_code)]
    skip_boot: bool,
    // Debug: echo last CPU-written port values even after boot (SMW workaround)
    smw_apu_echo: bool,
    // SMW用: HLEハンドシェイクを継続して走らせるか
    smw_apu_hle_handshake: bool,
    smw_hle_end_zero_streak: u8,
    // SMW用: ポートreadは常に直近CPU書き込み(latch)を返す強制モード
    smw_apu_port_echo_strict: bool,
    // Debug/HLE: skip actual SPC upload and jump to running state
    fake_upload: bool,
    // Pending S-CPU cycles accumulated by the emulator, flushed on APU port access or batch threshold.
    pending_cpu_cycles: u32,
    output_buffer_target_samples: i32,
}

unsafe impl Send for Apu {}
unsafe impl Sync for Apu {}

impl Apu {
    pub fn new() -> Self {
        let inner = SpcApu::new(); // comes with default IPL
        let config = ApuConfig::from_env();
        let mut apu = Self {
            inner,
            sample_rate: config.sample_rate,
            audio_left: Vec::new(),
            audio_right: Vec::new(),
            last_audio_sample: (0, 0),
            cycle_accum_fixed: 0,
            cycle_scale_fixed: config.cycle_scale_fixed,
            cycle_scale_master_fixed: config.cycle_scale_master_fixed,
            total_smp_cycles: 0,
            trace_last_f1: 0,
            trace_last_fa: 0,
            trace_last_out_ports: [0; 4],
            apu_to_cpu_ports: [0; 4],
            port_latch: [0; 4],
            pending_port_writes: Vec::new(),
            boot_state: config.initial_boot_state(), // AA/BBを必ず経由
            boot_hle_enabled: config.boot_hle_enabled,
            fast_upload: config.fast_upload,
            fast_upload_bytes: config.fast_upload_bytes,
            zero_write_seen: false,
            last_port1: 0,
            upload_addr: 0x0200,
            expected_index: 0,
            block_active: false,
            pending_idx: None,
            pending_cmd: None,
            data_ready: false,
            loader_hle_active: false,
            loader_hle_enabled: config.loader_hle_enabled,
            loader_hle_has_resume: false,
            loader_hle_resume_pc: 0,
            loader_hle_resume_sp: 0,
            loader_ready_stall_reads: 0,
            upload_done_count: 0,
            upload_bytes: 0,
            last_upload_idx: 0,
            end_zero_streak: 0,
            boot_port0_echo: 0xAA,
            force_port0: config.force_port0,
            skip_boot: config.skip_boot,
            smw_apu_echo: config.smw_apu_echo,
            smw_apu_hle_handshake: config.smw_apu_hle_handshake,
            smw_hle_end_zero_streak: 0,
            smw_apu_port_echo_strict: config.smw_apu_port_echo_strict,
            fake_upload: config.fake_upload,
            pending_cpu_cycles: 0,
            output_buffer_target_samples: config.output_buffer_target_samples,
        };
        apu.init_boot_ports();
        apu.refresh_trace_latches();
        if crate::debug_flags::trace_apu_bootstate() {
            println!(
                "[APU-BOOTSTATE] init: boot_hle_enabled={} loader_hle_enabled={} skip_boot={} fast_upload={} boot_state={:?}",
                config.boot_hle_enabled,
                config.loader_hle_enabled,
                config.skip_boot,
                config.fast_upload,
                apu.boot_state
            );
        }
        if config.skip_boot {
            apu.start_skip_boot_ack();
        }
        apu
    }

    #[allow(dead_code)]
    pub fn reset(&mut self) {
        let config = ApuConfig::from_reset_env(
            self.sample_rate,
            self.boot_hle_enabled,
            self.fake_upload,
            self.force_port0,
            self.smw_apu_echo,
            self.smw_apu_port_echo_strict,
        );
        self.inner.reset();
        self.audio_left.clear();
        self.audio_right.clear();
        self.last_audio_sample = (0, 0);
        self.sample_rate = config.sample_rate;
        self.fast_upload = config.fast_upload;
        self.fast_upload_bytes = config.fast_upload_bytes;
        self.skip_boot = config.skip_boot;
        self.loader_hle_enabled = config.loader_hle_enabled;
        self.smw_apu_echo = config.smw_apu_echo;
        self.smw_apu_hle_handshake = config.smw_apu_hle_handshake;
        self.smw_apu_port_echo_strict = config.smw_apu_port_echo_strict;
        self.smw_hle_end_zero_streak = 0;
        self.boot_state = config.initial_boot_state();
        self.zero_write_seen = false;
        self.last_port1 = 0;
        self.cycle_accum_fixed = 0;
        self.cycle_scale_fixed = config.cycle_scale_fixed;
        self.cycle_scale_master_fixed = config.cycle_scale_master_fixed;
        self.output_buffer_target_samples = config.output_buffer_target_samples;
        self.total_smp_cycles = 0;
        self.refresh_trace_latches();
        self.port_latch = [0; 4];
        self.pending_port_writes.clear();
        self.pending_cpu_cycles = 0;
        self.upload_addr = 0x0200;
        self.expected_index = 0;
        self.block_active = false;
        self.pending_idx = None;
        self.pending_cmd = None;
        self.data_ready = false;
        self.loader_hle_active = false;
        self.loader_hle_has_resume = false;
        self.loader_hle_resume_pc = 0;
        self.loader_hle_resume_sp = 0;
        self.loader_ready_stall_reads = 0;
        self.upload_done_count = 0;
        self.upload_bytes = 0;
        self.last_upload_idx = 0;
        self.end_zero_streak = 0;
        self.boot_port0_echo = 0xAA;
        self.init_boot_ports();
        if self.skip_boot {
            self.start_skip_boot_ack();
        }
    }

    fn refresh_trace_latches(&mut self) {
        self.trace_last_f1 = self.inner.read_u8(0x00F1);
        self.trace_last_fa = self.inner.read_u8(0x00FA);
        for p in 0..4 {
            self.trace_last_out_ports[p] = self.inner.cpu_read_port(p as u8);
        }
    }

    /// CPUサイクルに合わせてSPC700を回す。
    /// 仮想周波数: S-CPU 3.58MHz / SPC700 1.024MHz ⇒ およそ 1 : 3.5 で遅らせる。
    pub fn step(&mut self, cpu_cycles: u8) {
        self.cycle_accum_fixed += (cpu_cycles as u64) * self.cycle_scale_fixed;
        let run = (self.cycle_accum_fixed >> 32) as i32;
        self.cycle_accum_fixed &= 0xFFFF_FFFF;

        // CPU->APU ポート書き込みは SPC 実行前に反映する。
        self.flush_pending_port_writes();
        if run > 0 && self.loader_hle_active {
            self.advance_time_without_smp(run);
        } else if run > 0 {
            // SPC700 のバッチ実行中にポート書き込み ($F4-$F7) が発生すると
            // run() が中断する。中間ポート値を S-CPU 側に反映してから再開し、
            // IPL 転送プロトコルのエコーが消失するレースを防ぐ。
            self.inner.port_written = false;
            let executed = self.run_spc_interleaved(run);
            self.total_smp_cycles = self.total_smp_cycles.saturating_add(executed.max(0) as u64);
            // ポート書き込みで中断した場合、SMP 内部の残サイクルは捨てる。
            // ただし elapsed time そのものは失わせず、APU 側の固定小数点
            // accumulator に戻して次回以降に実行する。SMP cycle_count に残すと
            // 次の同期でCPU時間以上に走り、ACK/command handshake を追い越してしまう。
            if self.inner.port_written {
                self.defer_unexecuted_cycles(run, executed);
                if let Some(smp) = self.inner.smp.as_mut() {
                    smp.cycle_count = 0;
                }
            }
            if let Some(dsp) = self.inner.dsp.as_mut() {
                dsp.flush();
            }
        }

        // ハンドシェイク終了後は実ポート値を表側にも反映。
        // ただし port_written で中断した場合は run_spc_interleaved() が
        // セットした中間値を保持する（sync() が早期終了して S-CPU に見せるため）。
        if self.boot_state == BootState::Running && !self.inner.port_written {
            for p in 0..4 {
                self.apu_to_cpu_ports[p] = self.inner.cpu_read_port(p as u8);
            }
        }

        self.maybe_trace_apu_control();
        self.maybe_trace_out_ports();
        self.maybe_trace_smp_pc();
    }

    /// Master clock に合わせてSPC700を回す（S-CPU が停止している期間の進行用）。
    ///
    /// MDMAなどでS-CPUが止まっていても、実機ではAPUは独立して動作し続けるため、
    /// エミュレータ側でも「経過時間」ぶんだけSPC700/DSPを進める必要がある。
    #[allow(dead_code)]
    pub fn step_master_cycles(&mut self, master_cycles: u64) {
        if master_cycles == 0 {
            return;
        }

        // APU_CYCLE_SCALE is defined in terms of "S-CPU cycles" (as used by `step()`).
        // Convert master cycles -> S-CPU cycles using our fixed divider (master/6).
        self.cycle_accum_fixed += master_cycles * self.cycle_scale_master_fixed;
        let run = (self.cycle_accum_fixed >> 32) as i32;
        self.cycle_accum_fixed &= 0xFFFF_FFFF;

        // CPU停止中でもポート更新は先に反映しておく
        self.flush_pending_port_writes();

        if run > 0 && self.loader_hle_active {
            self.advance_time_without_smp(run);
        } else if run > 0 {
            self.inner.port_written = false;
            let executed = self.run_spc_interleaved(run);
            self.total_smp_cycles = self.total_smp_cycles.saturating_add(executed.max(0) as u64);
            if self.inner.port_written {
                self.defer_unexecuted_cycles(run, executed);
                if let Some(smp) = self.inner.smp.as_mut() {
                    smp.cycle_count = 0;
                }
            }
            if let Some(dsp) = self.inner.dsp.as_mut() {
                dsp.flush();
            }
        }

        if self.boot_state == BootState::Running {
            for p in 0..4 {
                self.apu_to_cpu_ports[p] = self.inner.cpu_read_port(p as u8);
            }
        }

        self.maybe_trace_apu_control();
        self.maybe_trace_out_ports();
        self.maybe_trace_smp_pc();
    }

    fn advance_time_without_smp(&mut self, cycles: i32) {
        self.total_smp_cycles = self.total_smp_cycles.saturating_add(cycles as u64);
        self.advance_dsp_time_without_smp(cycles);
    }

    fn advance_dsp_time_without_smp(&mut self, cycles: i32) {
        self.inner.cpu_cycles_callback(cycles);
        if let Some(dsp) = self.inner.dsp.as_mut() {
            dsp.flush();
        }
    }

    fn defer_unexecuted_cycles(&mut self, requested: i32, executed: i32) {
        let unexecuted = requested.saturating_sub(executed.max(0));
        if unexecuted <= 0 {
            return;
        }
        self.cycle_accum_fixed = self
            .cycle_accum_fixed
            .saturating_add((unexecuted as u64) << 32);
    }

    /// SPC700 を実行し、ポート $F4-$F7 への書き込みが発生したら中断する。
    /// 中間値を S-CPU 側の `apu_to_cpu_ports` に反映し、`port_written` フラグを
    /// 維持したまま返す（呼び出し元の sync() が残りサイクルを保持して早期終了できる）。
    fn run_spc_interleaved(&mut self, initial_cycles: i32) -> i32 {
        let executed = if let Some(smp) = self.inner.smp.as_mut() {
            smp.run(initial_cycles)
        } else {
            0
        };
        // ポート書き込みで中断した場合、中間値を反映して返す。
        // port_written フラグはクリアしない — sync() 側で残りdebtを保持するため。
        if self.inner.port_written && self.boot_state == BootState::Running {
            for p in 0..4 {
                self.apu_to_cpu_ports[p] = self.inner.cpu_read_port(p as u8);
            }
        }
        executed
    }

    fn maybe_trace_apu_control(&mut self) {
        if !crate::debug_flags::trace_burnin_apu_f1() {
            return;
        }
        let f1 = self.inner.read_u8(0x00F1);
        let fa = self.inner.read_u8(0x00FA);
        if f1 == self.trace_last_f1 && fa == self.trace_last_fa {
            return;
        }
        let smp_pc = self.inner.smp.as_ref().map(|s| s.reg_pc).unwrap_or(0);
        println!(
            "[APU-F1] apu_cycles={} smp_pc={:04X} $F1 {:02X}->{:02X} $FA {:02X}->{:02X}",
            self.total_smp_cycles, smp_pc, self.trace_last_f1, f1, self.trace_last_fa, fa
        );
        self.trace_last_f1 = f1;
        self.trace_last_fa = fa;
    }

    fn maybe_trace_out_ports(&mut self) {
        if !crate::debug_flags::trace_burnin_apu_port1() {
            return;
        }
        let cur = self.inner.cpu_read_port(1);
        if cur == self.trace_last_out_ports[1] {
            return;
        }
        use std::sync::atomic::{AtomicU32, Ordering};
        static CNT: AtomicU32 = AtomicU32::new(0);
        let n = CNT.fetch_add(1, Ordering::Relaxed);
        if n < 256 {
            let smp_pc = self.inner.smp.as_ref().map(|s| s.reg_pc).unwrap_or(0);
            let mut code = [0u8; 8];
            for (i, b) in code.iter_mut().enumerate() {
                *b = self.inner.read_u8(smp_pc.wrapping_add(i as u16) as u32);
            }
            println!(
                "[APU-PORT1] apu_cycles={} smp_pc={:04X} {:02X}->{:02X} code={:02X?}",
                self.total_smp_cycles, smp_pc, self.trace_last_out_ports[1], cur, code
            );
        }
        self.trace_last_out_ports[1] = cur;
    }

    fn maybe_trace_smp_pc(&mut self) {
        if !crate::debug_flags::trace_apu_smp_pc() {
            return;
        }
        use std::sync::atomic::{AtomicU32, Ordering};
        static CNT: AtomicU32 = AtomicU32::new(0);
        let n = CNT.fetch_add(1, Ordering::Relaxed);
        if n < 256 {
            if let Some(smp) = self.inner.smp.as_ref() {
                println!(
                    "[APU-SMP] apu_cycles={} pc={:04X} stopped={} ports=[{:02X} {:02X} {:02X} {:02X}]",
                    self.total_smp_cycles,
                    smp.reg_pc,
                    smp.is_stopped() as u8,
                    self.inner.cpu_read_port(0),
                    self.inner.cpu_read_port(1),
                    self.inner.cpu_read_port(2),
                    self.inner.cpu_read_port(3)
                );
            }
        }
    }

    /// Accumulate S-CPU cycles for deferred APU stepping.
    /// These are flushed on port access (sync) or when the emulator explicitly calls sync().
    #[inline]
    pub fn add_cpu_cycles(&mut self, cycles: u32) {
        self.pending_cpu_cycles = self.pending_cpu_cycles.saturating_add(cycles);
    }

    /// Flush pending S-CPU cycles: run the SPC700 for the accumulated time.
    /// Called before APU port reads/writes to ensure synchronization.
    pub fn sync(&mut self) {
        let debt = self.pending_cpu_cycles;
        if debt == 0 {
            // Even without cycle debt, make CPU->APU port writes visible.
            // Otherwise command/ack handshakes can stall when the CPU polls
            // immediately after writing APUIO.
            if !self.pending_port_writes.is_empty() {
                self.flush_pending_port_writes();
                if self.boot_state == BootState::Running {
                    for p in 0..4 {
                        self.apu_to_cpu_ports[p] = self.inner.cpu_read_port(p as u8);
                    }
                }
            }
            return;
        }
        self.pending_cpu_cycles = 0;
        let mut remaining = debt;
        while remaining > 0 {
            let chunk = remaining.min(u8::MAX as u32) as u8;
            self.step(chunk);
            remaining -= chunk as u32;
            // SPC700 がポート $F4-$F7 に書き込んだ場合、残りのデットを保持して
            // 中断する。S-CPU 側が中間値を読めるようにし、IPL 転送プロトコルの
            // エコー消失レースを防ぐ。
            if self.inner.port_written {
                self.pending_cpu_cycles = remaining;
                break;
            }
        }
    }

    /// Return current pending CPU cycles (for save state / diagnostics).
    #[inline]
    pub fn pending_cpu_cycles(&self) -> u32 {
        self.pending_cpu_cycles
    }

    /// ポート書き込みが保留中か。
    /// 書き込みハンドラ内では SPC を走らせず、命令境界の通常同期でまとめて反映する。
    #[inline]
    pub fn has_pending_port_writes(&self) -> bool {
        !self.pending_port_writes.is_empty()
    }

    /// Sync APU before CPU port access.
    #[inline]
    pub fn sync_for_port_access(&mut self) {
        self.sync();
    }

    /// Sync APU before CPU port write.
    ///
    /// Keep adjacent multi-port writes atomic from the S-SMP side.
    #[inline]
    pub fn sync_for_port_write(&mut self) {
        // ポート書き込み時は pending writes のフラッシュのみ行う。
        // sync() は実行しない — S-CPU がマルチポート書き込み中（port1+port0 など）に
        // SPC が中間状態で走ると、IPL転送プロトコルのブロック境界チェックで
        // port1 のデータバイト 0x00 を execute コマンドと誤判定するレースが発生する。
        self.flush_pending_port_writes();
    }

    pub fn clear_audio_output_buffer(&mut self) {
        self.audio_left.clear();
        self.audio_right.clear();
        self.last_audio_sample = (0, 0);
        if let Some(dsp) = self.inner.dsp.as_mut() {
            dsp.output_buffer.reset();
        }
    }

    /// オーディオサンプル生成（ステレオ）
    pub fn generate_audio_samples(&mut self, samples: &mut [(i16, i16)]) {
        let need = samples.len() as i32;
        if need <= 0 {
            return;
        }

        // `step()` 側でSPC700/DSPを進めて output_buffer に溜める。
        // ここでは output_buffer から読むだけにして、二重にSMPを回さない。
        let Some(dsp) = self.inner.dsp.as_mut() else {
            for s in samples.iter_mut() {
                *s = (0, 0);
            }
            self.last_audio_sample = (0, 0);
            return;
        };

        dsp.flush();

        let mut avail = dsp.output_buffer.get_sample_count().max(0);
        let target = self.output_buffer_target_samples.max(need);
        if avail > target {
            let drop = avail - target;
            dsp.output_buffer.discard_oldest(drop);
            avail -= drop;
        }
        let to_read = need.min(avail);
        let to_read_usize = to_read as usize;

        if to_read > 0 {
            let read_len = to_read_usize;
            if self.audio_left.len() < read_len {
                self.audio_left.resize(read_len, 0);
                self.audio_right.resize(read_len, 0);
            }
            let left = &mut self.audio_left[..read_len];
            let right = &mut self.audio_right[..read_len];
            dsp.output_buffer.read(left, right, to_read);
            for i in 0..read_len {
                samples[i] = (left[i], right[i]);
            }
            self.last_audio_sample = samples[read_len - 1];
        }

        // 足りない分は無音で埋める（リングバッファのアンダーラン対策）
        let fill = self.last_audio_sample;
        for s in samples.iter_mut().skip(to_read_usize) {
            *s = fill;
        }
    }

    pub fn get_sample_rate(&self) -> u32 {
        self.sample_rate
    }

    // --- セーブステート ---
    #[allow(clippy::wrong_self_convention, clippy::field_reassign_with_default)]
    pub fn to_save_state(&mut self) -> crate::savestate::ApuSaveState {
        let core = self.inner.get_state();
        let mut st = crate::savestate::ApuSaveState::default();
        st.ram = core.ram.to_vec();
        st.ipl_rom = core.ipl_rom.to_vec();
        st.dsp_registers = core.dsp_regs.to_vec();
        st.ports = self.apu_to_cpu_ports;
        st.cpu_to_apu_ports = core.cpu_to_apu_ports;
        st.apu_to_cpu_ports = self.apu_to_cpu_ports;
        st.port_latch = self.port_latch;
        st.cycle_counter = self.total_smp_cycles;
        st.smp_pc = core.smp.pc;
        st.smp_a = core.smp.a;
        st.smp_x = core.smp.x;
        st.smp_y = core.smp.y;
        st.smp_psw = core.smp.psw;
        st.smp_sp = core.smp.sp;
        st.smp_stopped = core.smp.is_stopped;
        st.smp_cycle_count = core.smp.cycle_count;
        st.dsp_reg_address = core.dsp_reg_address;
        st.is_ipl_rom_enabled = core.is_ipl_rom_enabled;
        st.boot_state = Self::boot_state_to_u8(self.boot_state);
        st.boot_port0_echo = self.boot_port0_echo;
        st.cycle_accum = (self.cycle_accum_fixed as f64) / (1u64 << 32) as f64;
        st.pending_cpu_cycles = self.pending_cpu_cycles;
        st.pending_port_writes = self
            .pending_port_writes
            .iter()
            .map(|(p, value)| [*p, *value])
            .collect();
        st.zero_write_seen = self.zero_write_seen;
        st.last_port1 = self.last_port1;
        st.upload_addr = self.upload_addr;
        st.expected_index = self.expected_index;
        st.block_active = self.block_active;
        st.pending_idx = self.pending_idx;
        st.pending_cmd = self.pending_cmd;
        st.data_ready = self.data_ready;
        st.loader_hle_active = self.loader_hle_active;
        st.loader_hle_has_resume = self.loader_hle_has_resume;
        st.loader_hle_resume_pc = self.loader_hle_resume_pc;
        st.loader_hle_resume_sp = self.loader_hle_resume_sp;
        st.loader_ready_stall_reads = self.loader_ready_stall_reads;
        st.upload_done_count = self.upload_done_count;
        st.upload_bytes = self.upload_bytes;
        st.last_upload_idx = self.last_upload_idx;
        st.end_zero_streak = self.end_zero_streak;
        st.smw_hle_end_zero_streak = self.smw_hle_end_zero_streak;
        st.master_volume_left = core.dsp_regs[0x0c];
        st.master_volume_right = core.dsp_regs[0x1c];
        st.echo_volume_left = core.dsp_regs[0x2c];
        st.echo_volume_right = core.dsp_regs[0x3c];
        st.timers = core
            .timers
            .iter()
            .map(|t| crate::savestate::TimerSaveState {
                enabled: t.is_running,
                target: t.target,
                counter: t.counter_high,
                divider: t.ticks as u16,
                divider_target: t.resolution as u16,
            })
            .collect();
        st
    }

    pub fn load_from_save_state(&mut self, st: &crate::savestate::ApuSaveState) {
        let ram = Self::vec_to_array::<0x10000>(&st.ram);
        let dsp_regs = Self::vec_to_array::<0x80>(&st.dsp_registers);
        let ipl_rom = if st.ipl_rom.is_empty() {
            self.inner.get_state().ipl_rom
        } else {
            Self::vec_to_array::<0x40>(&st.ipl_rom)
        };

        let timers = [
            Self::timer_state_from_save(st.timers.first(), 256),
            Self::timer_state_from_save(st.timers.get(1), 256),
            Self::timer_state_from_save(st.timers.get(2), 32),
        ];

        let smp_state = SpcSmpState {
            pc: st.smp_pc,
            a: st.smp_a,
            x: st.smp_x,
            y: st.smp_y,
            psw: st.smp_psw,
            sp: st.smp_sp,
            is_stopped: st.smp_stopped,
            cycle_count: st.smp_cycle_count,
        };

        let mut apu_to_cpu_ports = st.apu_to_cpu_ports;
        if apu_to_cpu_ports == [0; 4] {
            apu_to_cpu_ports = st.ports;
        }

        let apu_state = SpcApuState {
            ram,
            ipl_rom,
            smp: smp_state,
            dsp_regs,
            timers,
            is_ipl_rom_enabled: st.is_ipl_rom_enabled,
            dsp_reg_address: st.dsp_reg_address,
            cpu_to_apu_ports: st.cpu_to_apu_ports,
            apu_to_cpu_ports,
        };

        self.inner.set_state_from(&apu_state);
        self.apu_to_cpu_ports = apu_state.apu_to_cpu_ports;
        self.port_latch = if st.port_latch == [0; 4] {
            apu_state.cpu_to_apu_ports
        } else {
            st.port_latch
        };
        self.pending_port_writes = st
            .pending_port_writes
            .iter()
            .filter_map(|pair| {
                let p = pair[0];
                if p < 4 {
                    Some((p, pair[1]))
                } else {
                    None
                }
            })
            .collect();
        self.pending_cpu_cycles = st.pending_cpu_cycles;
        self.total_smp_cycles = st.cycle_counter;
        self.cycle_accum_fixed = f64_to_fixed32(st.cycle_accum);
        let (cycle_scale_fixed, cycle_scale_master_fixed) = ApuConfig::read_cycle_scale_fixed();
        self.cycle_scale_fixed = cycle_scale_fixed;
        self.cycle_scale_master_fixed = cycle_scale_master_fixed;
        self.boot_state = Self::boot_state_from_u8(st.boot_state);
        self.zero_write_seen = st.zero_write_seen;
        self.last_port1 = st.last_port1;
        self.upload_addr = if st.upload_addr == 0 && self.boot_state != BootState::Running {
            0x0200
        } else {
            st.upload_addr
        };
        self.expected_index = st.expected_index;
        self.block_active = st.block_active;
        self.pending_idx = st.pending_idx;
        self.pending_cmd = st.pending_cmd;
        self.data_ready = st.data_ready;
        self.loader_hle_active = st.loader_hle_active;
        self.loader_hle_has_resume = st.loader_hle_has_resume;
        self.loader_hle_resume_pc = st.loader_hle_resume_pc;
        self.loader_hle_resume_sp = st.loader_hle_resume_sp;
        self.loader_ready_stall_reads = st.loader_ready_stall_reads;
        self.upload_done_count = st.upload_done_count;
        self.upload_bytes = st.upload_bytes;
        self.last_upload_idx = st.last_upload_idx;
        self.end_zero_streak = st.end_zero_streak;
        self.smw_hle_end_zero_streak = st.smw_hle_end_zero_streak;
        self.boot_port0_echo = if st.boot_port0_echo == 0 {
            self.apu_to_cpu_ports[0]
        } else {
            st.boot_port0_echo
        };
        self.refresh_trace_latches();
    }

    fn vec_to_array<const N: usize>(data: &[u8]) -> [u8; N] {
        let mut out = [0u8; N];
        if !data.is_empty() {
            let len = data.len().min(N);
            out[..len].copy_from_slice(&data[..len]);
        }
        out
    }

    fn timer_state_from_save(
        st: Option<&crate::savestate::TimerSaveState>,
        resolution: i32,
    ) -> SpcTimerState {
        match st {
            Some(t) => SpcTimerState {
                resolution: if t.divider_target == 0 {
                    resolution
                } else {
                    t.divider_target as i32
                },
                is_running: t.enabled,
                ticks: t.divider as i32,
                target: t.target,
                counter_low: 0,
                counter_high: t.counter,
            },
            None => SpcTimerState {
                resolution,
                ..SpcTimerState::default()
            },
        }
    }

    fn boot_state_to_u8(state: BootState) -> u8 {
        match state {
            BootState::ReadySignature => 1,
            BootState::Uploading => 2,
            BootState::Running => 3,
        }
    }

    fn boot_state_from_u8(value: u8) -> BootState {
        match value {
            1 => BootState::ReadySignature,
            2 => BootState::Uploading,
            _ => BootState::Running,
        }
    }

    // 旧ハンドシェイクAPI互換ダミー
    #[allow(dead_code)]
    pub fn set_handshake_enabled(&mut self, _enabled: bool) {}
    pub fn handshake_state_str(&self) -> &'static str {
        match self.boot_state {
            BootState::ReadySignature => "ipl-signature",
            BootState::Uploading => "ipl-upload",
            BootState::Running => "spc700",
        }
    }
}
