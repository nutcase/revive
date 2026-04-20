//! APU wrapper using the external `snes-apu` crate (SPC700 + DSP).
//! 現状: 精度よりも動作優先の簡易統合。セーブステートは内部状態を保存/復元。
use super::spc::apu::{Apu as SpcApu, ApuState as SpcApuState};
use super::spc::smp::SmpState as SpcSmpState;
use super::spc::TimerState as SpcTimerState;

// Clock ratio used to convert S-CPU cycles (3.579545MHz NTSC) to `snes-apu` internal cycles.
//
// `snes-apu` uses a 2.048MHz internal tick rate (32kHz * 64 cycles/sample), which corresponds to
// the SNES APU oscillator (24.576MHz / 12).
//
// ratio = 2_048_000 / 3_579_545.333... ≈ 0.5721397019
const DEFAULT_APU_CYCLE_SCALE: f64 = 0.572_139_701_913_725_3;
const DEFAULT_APU_SAMPLE_RATE: u32 = 32000;
const DEFAULT_FAST_UPLOAD_BYTES: u64 = 0x10000;
const DEFAULT_APU_OUTPUT_TARGET_SAMPLES: i32 = 2048;

/// Convert an f64 fractional scale (0..1 range typical) to Q0.32 fixed-point.
#[inline]
fn f64_to_fixed32(v: f64) -> u64 {
    (v * (1u64 << 32) as f64) as u64
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BootState {
    /// 初期シグネチャ (AA/BB) をCPUに見せる段階
    ReadySignature,
    /// CPUからのキック(0xCC)を待ち、転送中
    Uploading,
    /// APUプログラム稼働中。以降は実ポート値をそのまま返す
    Running,
}

#[derive(Debug, Clone, Copy)]
struct ApuConfig {
    sample_rate: u32,
    boot_hle_enabled: bool,
    fast_upload: bool,
    fast_upload_bytes: u64,
    skip_boot: bool,
    fake_upload: bool,
    force_port0: Option<u8>,
    smw_apu_echo: bool,
    smw_apu_hle_handshake: bool,
    smw_apu_port_echo_strict: bool,
    cycle_scale_fixed: u64,
    cycle_scale_master_fixed: u64,
    output_buffer_target_samples: i32,
}

impl ApuConfig {
    fn from_env() -> Self {
        let (cycle_scale_fixed, cycle_scale_master_fixed) = Self::read_cycle_scale_fixed();
        Self {
            sample_rate: DEFAULT_APU_SAMPLE_RATE,
            // デフォルト: 実IPL（正確性優先）。必要なら APU_BOOT_HLE=1 でHLE有効化。
            boot_hle_enabled: Self::read_strict_bool_env("APU_BOOT_HLE", false),
            // 正確さ優先: デフォルトではフルサイズ転送を行う。
            // 速さが欲しい場合のみ APU_FAST_UPLOAD=1 を明示する。
            fast_upload: Self::read_loose_bool_env("APU_FAST_UPLOAD", false),
            fast_upload_bytes: Self::read_u64_env("APU_FAST_BYTES", DEFAULT_FAST_UPLOAD_BYTES),
            skip_boot: Self::read_loose_bool_env("APU_SKIP_BOOT", false),
            fake_upload: Self::read_loose_bool_env("APU_FAKE_UPLOAD", false),
            force_port0: Self::read_u8_env("APU_FORCE_PORT0"),
            smw_apu_echo: Self::read_loose_bool_env("SMW_APU_ECHO", false),
            // SMW専用。既定では無効（他ROMへの副作用回避）
            smw_apu_hle_handshake: Self::read_loose_bool_env("SMW_APU_HLE_HANDSHAKE", false),
            smw_apu_port_echo_strict: Self::read_loose_bool_env("SMW_APU_PORT_ECHO_STRICT", false),
            cycle_scale_fixed,
            cycle_scale_master_fixed,
            output_buffer_target_samples: Self::read_i32_env(
                "APU_OUTPUT_TARGET_SAMPLES",
                DEFAULT_APU_OUTPUT_TARGET_SAMPLES,
            ),
        }
    }

    fn from_reset_env(
        sample_rate: u32,
        boot_hle_enabled: bool,
        fake_upload: bool,
        force_port0: Option<u8>,
        smw_apu_echo_default: bool,
        smw_apu_port_echo_strict_default: bool,
    ) -> Self {
        let base = Self::from_env();
        Self {
            sample_rate,
            boot_hle_enabled,
            fake_upload,
            force_port0,
            smw_apu_echo: Self::read_loose_bool_env("SMW_APU_ECHO", smw_apu_echo_default),
            smw_apu_hle_handshake: Self::read_loose_bool_env("SMW_APU_HLE_HANDSHAKE", false),
            smw_apu_port_echo_strict: Self::read_loose_bool_env(
                "SMW_APU_PORT_ECHO_STRICT",
                smw_apu_port_echo_strict_default,
            ),
            ..base
        }
    }

    fn initial_boot_state(self) -> BootState {
        if self.skip_boot || !self.boot_hle_enabled {
            BootState::Running
        } else {
            BootState::ReadySignature
        }
    }

    fn read_strict_bool_env(name: &str, default: bool) -> bool {
        std::env::var(name)
            .map(|v| v == "1" || v.to_lowercase() == "true")
            .unwrap_or(default)
    }

    fn read_loose_bool_env(name: &str, default: bool) -> bool {
        std::env::var(name)
            .map(|v| v != "0" && v.to_lowercase() != "false")
            .unwrap_or(default)
    }

    fn read_u64_env(name: &str, default: u64) -> u64 {
        std::env::var(name)
            .ok()
            .and_then(|v| Self::parse_hex_or_decimal_u64(&v))
            .unwrap_or(default)
    }

    fn read_u8_env(name: &str) -> Option<u8> {
        std::env::var(name)
            .ok()
            .and_then(|v| Self::parse_hex_or_decimal_u8(&v))
    }

    fn read_i32_env(name: &str, default: i32) -> i32 {
        std::env::var(name)
            .ok()
            .and_then(|v| v.parse::<i32>().ok())
            .filter(|&v| v > 0)
            .unwrap_or(default)
    }

    fn parse_hex_or_decimal_u64(value: &str) -> Option<u64> {
        u64::from_str_radix(value.trim_start_matches("0x"), 16)
            .ok()
            .or_else(|| value.parse().ok())
    }

    fn parse_hex_or_decimal_u8(value: &str) -> Option<u8> {
        u8::from_str_radix(value.trim_start_matches("0x"), 16)
            .ok()
            .or_else(|| value.parse().ok())
    }

    fn read_cycle_scale_f64() -> f64 {
        std::env::var("APU_CYCLE_SCALE")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(DEFAULT_APU_CYCLE_SCALE)
    }

    fn read_cycle_scale_fixed() -> (u64, u64) {
        let scale = Self::read_cycle_scale_f64();
        let fixed = f64_to_fixed32(scale);
        let master = f64_to_fixed32(scale / 6.0);
        (fixed, master)
    }
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
                "[APU-BOOTSTATE] init: boot_hle_enabled={} skip_boot={} fast_upload={} boot_state={:?}",
                config.boot_hle_enabled, config.skip_boot, config.fast_upload, apu.boot_state
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

    fn start_skip_boot_ack(&mut self) {
        // skip_bootでも AA/BB を見せてから CC エコーを開始する。
        self.apu_to_cpu_ports = [0xAA, 0xBB, 0x00, 0x00];
        self.boot_port0_echo = 0xAA;
        self.finish_upload_and_start_with_ack(0xCC);
        self.boot_port0_echo = 0xCC;
        self.apu_to_cpu_ports[0] = 0xCC;
        self.apu_to_cpu_ports[1] = 0xBB;
    }

    fn init_boot_ports(&mut self) {
        // Even when we skip the real IPL, seed ports with AA/BB so S-CPU handshake loops pass.
        if self.boot_state == BootState::ReadySignature || self.fast_upload {
            self.apu_to_cpu_ports = [0xAA, 0xBB, 0x00, 0x00];
            // CPU側から読む値（APUIO）は APU->CPU ラッチ。実機ではIPLが書くが、HLE時は先に用意する。
            self.inner.write_u8(0x00F4, 0xAA);
            self.inner.write_u8(0x00F5, 0xBB);
            self.inner.write_u8(0x00F6, 0x00);
            self.inner.write_u8(0x00F7, 0x00);
            // CPU->APU ラッチ（SMPが読む側）は既定で 0。
            self.port_latch = [0; 4];
            self.boot_port0_echo = 0xAA;
        } else {
            self.apu_to_cpu_ports = [0; 4];
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
        if run > 0 {
            self.total_smp_cycles = self.total_smp_cycles.saturating_add(run as u64);
            // SPC700 のバッチ実行中にポート書き込み ($F4-$F7) が発生すると
            // run() が中断する。中間ポート値を S-CPU 側に反映してから再開し、
            // IPL 転送プロトコルのエコーが消失するレースを防ぐ。
            self.inner.port_written = false;
            self.run_spc_interleaved(run);
            // ポート書き込みで中断した場合、SMP の残サイクルをドロップする。
            // 次の step() で cycle_count に残りが蓄積し、IPL echo + game driver reset
            // が一括処理されるレースを防ぐ。
            if self.inner.port_written {
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

        if run > 0 {
            self.total_smp_cycles = self.total_smp_cycles.saturating_add(run as u64);
            self.inner.port_written = false;
            self.run_spc_interleaved(run);
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
    /// In Running state, avoid flushing tiny debts on every write to reduce
    /// overhead while still keeping command latency bounded.
    #[inline]
    pub fn sync_for_port_write(&mut self) {
        // ポート書き込み時は pending writes のフラッシュのみ行う。
        // sync() は実行しない — S-CPU がマルチポート書き込み中（port1+port0 など）に
        // SPC が中間状態で走ると、IPL転送プロトコルのブロック境界チェックで
        // port1 のデータバイト 0x00 を execute コマンドと誤判定するレースが発生する。
        self.flush_pending_port_writes();
    }

    #[inline]
    fn cpu_visible_port_value(&self, port: u8) -> u8 {
        let p = (port & 0x03) as usize;
        match self.boot_state {
            BootState::Running => self.apu_to_cpu_ports[p],
            _ => {
                if let Some(force) = self.force_port0 {
                    if p == 0 {
                        force
                    } else {
                        self.apu_to_cpu_ports[p]
                    }
                } else if p == 0 {
                    self.boot_port0_echo
                } else {
                    self.apu_to_cpu_ports[p]
                }
            }
        }
    }

    #[inline]
    fn refresh_cpu_visible_ports(&mut self) {
        if self.boot_state == BootState::Running {
            for p in 0..4 {
                self.apu_to_cpu_ports[p] = self.inner.cpu_read_port(p as u8);
            }
        }
    }

    pub fn run_until_cpu_port_matches_latch(&mut self, port: u8, max_smp_cycles: usize) -> bool {
        if max_smp_cycles == 0 {
            return false;
        }

        self.flush_pending_port_writes();

        let p = (port & 0x03) as usize;
        let target = self.port_latch[p];
        if self.cpu_visible_port_value(port) == target {
            return false;
        }

        let mut remaining = max_smp_cycles as i32;
        while remaining > 0 {
            let chunk = remaining.min(64);
            self.total_smp_cycles = self.total_smp_cycles.saturating_add(chunk as u64);
            self.inner.port_written = false;
            let executed = self.run_spc_interleaved(chunk);
            if let Some(dsp) = self.inner.dsp.as_mut() {
                dsp.flush();
            }
            self.refresh_cpu_visible_ports();
            if self.cpu_visible_port_value(port) == target {
                return true;
            }
            remaining -= executed.max(1);
        }

        false
    }

    pub fn clear_audio_output_buffer(&mut self) {
        self.audio_left.clear();
        self.audio_right.clear();
        self.last_audio_sample = (0, 0);
        if let Some(dsp) = self.inner.dsp.as_mut() {
            dsp.output_buffer.reset();
        }
    }

    /// CPU側ポート読み出し ($2140-$2143)
    pub fn read_port(&mut self, port: u8) -> u8 {
        let p = (port & 0x03) as usize;

        // 強制値（デバッグ/HLE）指定時は即返す
        if let Some(forced) = if p == 0 {
            crate::debug_flags::apu_force_port0()
        } else if p == 1 {
            crate::debug_flags::apu_force_port1()
        } else {
            None
        } {
            return forced;
        }

        match self.boot_state {
            BootState::Running => {
                // sync() のインターリーブ実行で更新された apu_to_cpu_ports を返す。
                // これにより SPC700 のバッチ実行中に書き込まれた中間値が
                // S-CPU から見えるようになる。
                let v = self.apu_to_cpu_ports[p];
                if crate::debug_flags::trace_apu_port_once()
                    || crate::debug_flags::trace_apu_port_all()
                    || (p == 0 && crate::debug_flags::trace_apu_port0())
                {
                    use std::sync::atomic::{AtomicU32, Ordering};
                    static CNT: AtomicU32 = AtomicU32::new(0);
                    let n = CNT.fetch_add(1, Ordering::Relaxed);
                    if n < 32 {
                        println!("[APU-R] port{} -> {:02X} (boot=Running)", p, v);
                    }
                }
                // Diagnostic: log SPC700 state when CPU reads port0
                if p == 0 && crate::debug_flags::trace_top_apu_diag() {
                    use std::sync::atomic::{AtomicU32, AtomicU8, Ordering};
                    static CHG_CNT: AtomicU32 = AtomicU32::new(0);
                    static STALL_CNT: AtomicU32 = AtomicU32::new(0);
                    static STALL_READS: AtomicU32 = AtomicU32::new(0);
                    static LAST: AtomicU8 = AtomicU8::new(0xFF);
                    let prev = LAST.swap(v, Ordering::Relaxed);
                    if prev != v {
                        STALL_READS.store(0, Ordering::Relaxed);
                        let n = CHG_CNT.fetch_add(1, Ordering::Relaxed);
                        if (500..564).contains(&n) {
                            let (smp_pc, stopped) = self
                                .inner
                                .smp
                                .as_ref()
                                .map(|s| (s.reg_pc, s.is_stopped()))
                                .unwrap_or((0, false));
                            println!(
                                "[TOP-APU-DIAG] port0={:02X} (was {:02X}) smp_pc={:04X} stopped={} apu_cycles={} out=[{:02X} {:02X} {:02X} {:02X}] in=[{:02X} {:02X} {:02X} {:02X}]",
                                v, prev, smp_pc, stopped as u8, self.total_smp_cycles,
                                self.inner.cpu_read_port(0), self.inner.cpu_read_port(1),
                                self.inner.cpu_read_port(2), self.inner.cpu_read_port(3),
                                self.port_latch[0], self.port_latch[1],
                                self.port_latch[2], self.port_latch[3],
                            );
                        }
                    } else {
                        // Value hasn't changed — track stall
                        let reads = STALL_READS.fetch_add(1, Ordering::Relaxed);
                        if reads == 1000
                            || reads == 5000
                            || reads == 10000
                            || reads == 50000
                            || reads == 100000
                            || reads == 1000000
                        {
                            let n = STALL_CNT.fetch_add(1, Ordering::Relaxed);
                            if n < 32 {
                                let (smp_pc, stopped, smp_a, smp_x, smp_y, smp_psw, smp_sp) = self
                                    .inner
                                    .smp
                                    .as_ref()
                                    .map(|s| {
                                        (
                                            s.reg_pc,
                                            s.is_stopped(),
                                            s.reg_a,
                                            s.reg_x,
                                            s.reg_y,
                                            s.get_psw(),
                                            s.reg_sp,
                                        )
                                    })
                                    .unwrap_or((0, false, 0, 0, 0, 0, 0));
                                println!(
                                    "[TOP-APU-STALL] port0={:02X} stall_reads={} smp_pc={:04X} stopped={} A={:02X} X={:02X} Y={:02X} PSW={:02X} SP={:02X} apu_cycles={} out=[{:02X} {:02X} {:02X} {:02X}] in=[{:02X} {:02X} {:02X} {:02X}]",
                                    v, reads, smp_pc, stopped as u8, smp_a, smp_x, smp_y, smp_psw, smp_sp,
                                    self.total_smp_cycles,
                                    self.inner.cpu_read_port(0), self.inner.cpu_read_port(1),
                                    self.inner.cpu_read_port(2), self.inner.cpu_read_port(3),
                                    self.port_latch[0], self.port_latch[1],
                                    self.port_latch[2], self.port_latch[3],
                                );
                                // Dump SPC RAM around PC, port area, and stack when stalled
                                if reads >= 5000 {
                                    // Code around SPC PC
                                    let mut code = [0u8; 32];
                                    for (i, b) in code.iter_mut().enumerate() {
                                        *b = self
                                            .inner
                                            .read_u8(smp_pc.wrapping_add(i as u16) as u32);
                                    }
                                    println!("[TOP-APU-STALL] code@{:04X}={:02X?}", smp_pc, code);
                                    // Port area ($F0-$FF)
                                    let mut ports = [0u8; 16];
                                    for (i, b) in ports.iter_mut().enumerate() {
                                        *b = self.inner.read_u8(0xF0u32 + i as u32);
                                    }
                                    println!("[TOP-APU-STALL] ram@00F0={:02X?}", ports);
                                    // Stack area (SP and above)
                                    let sp_base = 0x0100u16 | (smp_sp as u16);
                                    let mut stack = [0u8; 16];
                                    for (i, b) in stack.iter_mut().enumerate() {
                                        let addr = sp_base.wrapping_add(1).wrapping_add(i as u16);
                                        *b = self.inner.read_u8(addr as u32);
                                    }
                                    println!(
                                        "[TOP-APU-STALL] stack@{:04X}={:02X?}",
                                        sp_base.wrapping_add(1),
                                        stack
                                    );
                                }
                            }
                        }
                    }
                }
                v
            }
            // ブート中: port0は「最後にCPUが書いた値」を保持して返す。port1-3も表キャッシュを返す。
            _ => {
                let v = if let Some(force) = self.force_port0 {
                    if p == 0 {
                        force
                    } else {
                        self.apu_to_cpu_ports[p]
                    }
                } else if p == 0 {
                    self.boot_port0_echo
                } else {
                    self.apu_to_cpu_ports[p]
                };
                if crate::debug_flags::trace_apu_port_once()
                    || crate::debug_flags::trace_apu_port_all()
                    || (p == 0 && crate::debug_flags::trace_apu_port0())
                {
                    use std::sync::atomic::{AtomicU32, Ordering};
                    static CNT: AtomicU32 = AtomicU32::new(0);
                    let n = CNT.fetch_add(1, Ordering::Relaxed);
                    if n < 32 {
                        println!(
                            "[APU-R] port{} -> {:02X} (boot={:?})",
                            p, v, self.boot_state
                        );
                    }
                }
                v
            }
        }
    }

    /// CPU側ポート書き込み ($2140-$2143)
    pub fn write_port(&mut self, port: u8, value: u8) {
        let p = (port & 0x03) as usize;
        self.queue_cpu_port_write(p, value);

        // 簡易HLE: 転送を省略して即稼働させる
        if self.boot_hle_enabled && self.fake_upload && self.boot_state != BootState::Running {
            self.finish_upload_and_start_with_ack(0);
        }

        self.maybe_trace_port_write(p, value);

        if !self.boot_hle_enabled {
            return;
        }

        match self.boot_state {
            BootState::ReadySignature => self.handle_ready_signature_write(p, value),
            BootState::Uploading => self.handle_uploading_write(p, value),
            BootState::Running => self.handle_running_write(p, value),
        }
    }

    fn queue_cpu_port_write(&mut self, p: usize, value: u8) {
        self.port_latch[p] = value;
        // CPU->APU ポート書き込みは常に遅延キュー経由で反映する。
        // step() 内で SMP 実行後に flush されるため、SPC は「1ステップ前」の
        // ポート値で動作する。これにより実機同様の伝搬遅延をシミュレートし、
        // 転送プロトコルの race condition を回避する。
        self.pending_port_writes.push((p as u8, value));
    }

    fn maybe_trace_port_write(&self, p: usize, value: u8) {
        if crate::debug_flags::trace_apu_port_all()
            || (p == 0 && crate::debug_flags::trace_apu_port0())
        {
            use std::sync::atomic::{AtomicU32, Ordering};
            static CNT: AtomicU32 = AtomicU32::new(0);
            let n = CNT.fetch_add(1, Ordering::Relaxed);
            if n < 512 {
                println!(
                    "[APU-W] port{} <- {:02X} state={:?} echo0={:02X} to_cpu=[{:02X} {:02X} {:02X} {:02X}]",
                    p,
                    value,
                    self.boot_state,
                    self.boot_port0_echo,
                    self.apu_to_cpu_ports[0],
                    self.apu_to_cpu_ports[1],
                    self.apu_to_cpu_ports[2],
                    self.apu_to_cpu_ports[3]
                );
            }
        }
    }

    fn handle_ready_signature_write(&mut self, p: usize, value: u8) {
        // CPUが0xCCを書いたらIPL転送開始。CC以外の値で署名を潰さない
        // （SMC等の初期化で $2140 を0クリアするため）。
        if p == 0 {
            if value != 0xCC {
                // 署名は維持したまま無視
                return;
            }
            self.enter_uploading_state();
        }

        match p {
            1 => self.last_port1 = value,
            2 => self.set_upload_addr_low(value),
            3 => self.set_upload_addr_high(value),
            _ => {}
        }
    }

    fn enter_uploading_state(&mut self) {
        // HLEでもアップロード状態に入り、CPUのインデックスエコーを行う。
        self.boot_state = BootState::Uploading;
        if crate::debug_flags::trace_apu_bootstate() {
            println!("[APU-BOOTSTATE] -> Uploading (kick=0xCC)");
        }
        self.apu_to_cpu_ports[0] = 0xCC;
        self.boot_port0_echo = 0xCC;
        self.expected_index = 0;
        self.block_active = false;
        self.zero_write_seen = false;
        self.pending_idx = None;
        self.pending_cmd = None;
        self.data_ready = false;
        self.upload_bytes = 0;
        self.last_upload_idx = 0;
        // fast_upload は Uploading 中の閾値判定で早期完了する。
    }

    fn handle_uploading_write(&mut self, p: usize, value: u8) {
        // 転送先アドレス（毎ブロックごとに書き替えられる）。
        match p {
            2 => {
                self.set_upload_addr_low(value);
                return;
            }
            3 => {
                self.set_upload_addr_high(value);
                return;
            }
            _ => {}
        }

        // port0/port1 の書き込み順は ROM により異なる（8bit書き込み / 16bit書き込み）。
        // 実機IPLは port0 の変化をトリガに port1 を読み取るため、ここでは
        // 「port0(idx) と port1(data) の両方が揃ったタイミング」で1バイトを確定する。
        match p {
            0 => self.handle_upload_index_write(value),
            1 => self.handle_upload_data_write(value),
            _ => {}
        }
    }

    fn handle_upload_index_write(&mut self, idx: u8) {
        // SPC700 IPL protocol:
        // - Data byte: APUIO0 must equal expected_index (starts at 0 for each block)
        // - Command: APUIO0 != expected_index; APUIO1==0 means "start program at APUIO2/3",
        //   otherwise it means "set new base address (APUIO2/3) and continue upload".
        if idx == self.expected_index {
            self.pending_idx = Some(idx);
            if self.data_ready {
                // port1 が先に来たケース: ここで確定
                self.write_upload_byte(idx, self.last_port1);
            }
        } else {
            // Command / state sync (port1が揃ってから確定)
            self.pending_cmd = Some(idx);
            if self.data_ready {
                self.handle_upload_command(idx, self.last_port1);
            }
        }
    }

    fn handle_upload_data_write(&mut self, value: u8) {
        self.last_port1 = value;
        self.data_ready = true;
        if let Some(idx) = self.pending_idx {
            // port0 が先に来たケース: ここで確定
            if idx == self.expected_index {
                self.write_upload_byte(idx, value);
                return;
            }
        }
        if let Some(cmd) = self.pending_cmd {
            self.handle_upload_command(cmd, value);
        }
    }

    fn write_upload_byte(&mut self, idx: u8, data: u8) {
        self.data_ready = false;
        self.pending_idx = None;
        let addr = self.upload_addr.wrapping_add(idx as u16);
        self.inner.write_u8(addr as u32, data);
        self.upload_bytes = self.upload_bytes.saturating_add(1);
        self.last_upload_idx = idx;
        self.expected_index = self.expected_index.wrapping_add(1);
        // ACKはデータ書き込み後に返す
        self.apu_to_cpu_ports[0] = idx;
        self.boot_port0_echo = idx;
    }

    fn handle_upload_command(&mut self, cmd: u8, port1: u8) {
        self.pending_cmd = None;
        self.pending_idx = None;
        self.data_ready = false;
        self.expected_index = 0;
        // コマンドはACKをエコー
        self.apu_to_cpu_ports[0] = cmd;
        self.boot_port0_echo = cmd;
        if port1 == 0 {
            // Start program; ACK must echo the command value the CPU wrote.
            self.finish_upload_and_start_with_ack(cmd);
        }
    }

    fn set_upload_addr_low(&mut self, value: u8) {
        self.upload_addr = (self.upload_addr & 0xFF00) | value as u16;
    }

    fn set_upload_addr_high(&mut self, value: u8) {
        self.upload_addr = (self.upload_addr & 0x00FF) | ((value as u16) << 8);
    }

    fn handle_running_write(&mut self, p: usize, value: u8) {
        // 稼働後はCPU->APU書き込みをそのまま渡すのみ。キャッシュ更新はHLE/SMW用途のみ。
        if self.smw_apu_echo || self.smw_apu_hle_handshake || self.smw_apu_port_echo_strict {
            self.apu_to_cpu_ports[p] = value;
        }

        if self.smw_apu_hle_handshake && p == 0 {
            self.handle_smw_hle_running_port0(value);
        }
    }

    fn handle_smw_hle_running_port0(&mut self, value: u8) {
        // SMW HLE 継続モード: 0,0 が2回続いたら即 start (upload_done) とみなす。
        if value == 0 {
            self.smw_hle_end_zero_streak = self.smw_hle_end_zero_streak.saturating_add(1);
        } else {
            self.smw_hle_end_zero_streak = 0;
        }
        if self.smw_hle_end_zero_streak >= 2 {
            if crate::debug_flags::trace_apu_bootstate() {
                println!("[APU-BOOTSTATE] SMW force start (running echo)");
            }
            if crate::debug_flags::trace_apu_boot() {
                println!("[APU-HLE] Forced start after port0=0 twice (running-phase echo)");
            }
            self.finish_upload_and_start_with_ack(0);
            self.smw_hle_end_zero_streak = 0;
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

    /// デバッグ/HLE用途: 任意のバイナリをARAmにロードして即実行開始する。
    /// ポートの初期値は 0x00 に揃え、boot_state を Running に移行する。
    pub fn load_and_start(&mut self, data: &[u8], base: u16, start_pc: u16) {
        // 書き込み先は base から。I/Oレジスタ(0xF0-0xFF)は避ける。
        let mut offset = base as usize;
        for &b in data.iter() {
            if offset >= 0x10000 {
                break;
            }
            if (0x00F0..0x0100).contains(&offset) {
                // スキップして次のページへ
                offset = (offset & 0xFF00) + 0x0100;
            }
            if offset >= 0x10000 {
                break;
            }
            self.inner.write_u8(offset as u32, b);
            offset += 1;
        }
        if let Some(smp) = self.inner.smp.as_mut() {
            // Ensure the SPC core is not left in STOP/SLEEP from the IPL path.
            smp.reset();
            smp.reg_pc = start_pc;
        }
        // IPL を無効化
        self.inner.write_u8(0x00F1, 0x00);
        // ポート初期値はAA/BB署名を維持してCPU側ハンドシェイクを満たす
        let init_ports = [0xAA, 0xBB, 0x00, 0x00];
        for (p, &v) in init_ports.iter().enumerate() {
            self.inner.write_u8(0x00F4 + p as u32, v);
            self.apu_to_cpu_ports[p] = v;
        }
        self.port_latch = [0; 4];
        self.boot_port0_echo = 0xAA;
        self.boot_state = BootState::Running;
        if crate::debug_flags::trace_apu_bootstate() {
            println!(
                "[APU-BOOTSTATE] load_and_start -> Running (base=${:04X} start_pc=${:04X} len={})",
                base,
                start_pc,
                data.len()
            );
        }
    }

    /// 転送完了後にSPCプログラムを実行状態へ進める。
    #[allow(dead_code)]
    fn finish_upload_and_start(&mut self) {
        // 実機IPL同様、完了時のACKは 0 を返す
        self.finish_upload_and_start_with_ack(0);
    }

    fn finish_upload_and_start_with_ack(&mut self, ack: u8) {
        self.boot_state = BootState::Running;
        if crate::debug_flags::trace_apu_bootstate() {
            println!(
                "[APU-BOOTSTATE] finish_upload_and_start ack={:02X} addr=${:04X}",
                ack, self.upload_addr
            );
        }
        self.block_active = false;
        self.data_ready = false;
        self.upload_done_count += 1;
        if crate::debug_flags::trace_apu_port()
            || crate::debug_flags::trace_apu_boot()
            || crate::debug_flags::trace_apu_port_all()
        {
            println!(
                "[APU-BOOT] upload complete count={} start_pc=${:04X} addr_base=${:04X}",
                self.upload_done_count, self.upload_addr, self.upload_addr
            );
        }
        // IPL ROM を無効化
        self.inner.write_u8(0x00F1, 0x00);
        // ジャンプ先をセット（IPLがジャンプする直前の初期レジスタ状態に寄せる）
        if let Some(smp) = self.inner.smp.as_mut() {
            // Clear STOP/SLEEP and reset core timing before jumping to uploaded code.
            smp.reset();
            let pc = if self.upload_addr == 0 {
                0x0200
            } else {
                self.upload_addr
            };
            // IPL直後の基本状態（Smp::reset 相当）。
            // これを揃えないと、HLEで中途半端なIPL実行状態のままジャンプしてSPC側が暴走しやすい。
            smp.reg_a = 0;
            smp.reg_x = 0;
            smp.reg_y = 0;
            smp.reg_sp = 0xEF;
            smp.set_psw(0x02);
            smp.reg_pc = pc;
        }
        // 実行開始をCPUへ知らせるためポート0にACK値を置く（既定=0）
        self.inner.write_u8(0x00F4, ack);
        self.apu_to_cpu_ports[0] = ack;
        // 初期ACKはそのままにして、以後は実値を返す
        for i in 0..4 {
            self.apu_to_cpu_ports[i] = self.inner.cpu_read_port(i as u8);
        }
    }

    fn flush_pending_port_writes(&mut self) {
        if self.pending_port_writes.is_empty() {
            return;
        }
        // Detect when multiple writes to the same port are flushed in one batch
        if crate::debug_flags::trace_top_spc_cmd() && self.pending_port_writes.len() > 1 {
            let mut port_seen = [false; 4];
            let mut dup_port = false;
            for &(p, _) in &self.pending_port_writes {
                if port_seen[p as usize] {
                    dup_port = true;
                    break;
                }
                port_seen[p as usize] = true;
            }
            if dup_port {
                use std::sync::atomic::{AtomicU32, Ordering};
                static DUP_CNT: AtomicU32 = AtomicU32::new(0);
                let n = DUP_CNT.fetch_add(1, Ordering::Relaxed);
                if n < 200 {
                    eprintln!(
                        "[APU-DUP] batch={} writes: {:?}",
                        self.pending_port_writes.len(),
                        &self.pending_port_writes
                    );
                }
            }
        }
        for (p, value) in self.pending_port_writes.drain(..) {
            self.inner.cpu_write_port(p, value);
        }
    }
}
