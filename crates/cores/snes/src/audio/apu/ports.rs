use super::{Apu, BootState};

impl Apu {
    pub(super) fn start_skip_boot_ack(&mut self) {
        // skip_bootでも AA/BB を見せてから CC エコーを開始する。
        self.apu_to_cpu_ports = [0xAA, 0xBB, 0x00, 0x00];
        self.boot_port0_echo = 0xAA;
        self.finish_upload_and_start_with_ack(0xCC);
        self.boot_port0_echo = 0xCC;
        self.apu_to_cpu_ports[0] = 0xCC;
        self.apu_to_cpu_ports[1] = 0xBB;
    }

    pub(super) fn init_boot_ports(&mut self) {
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
        if self.loader_hle_active {
            self.advance_time_without_smp(max_smp_cycles as i32);
            return self.cpu_visible_port_value(port) == target;
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

    fn maybe_enter_loader_hle_from_ready_signature(&mut self, p: usize, visible: u8) -> u8 {
        // Some SPC programs re-enter an IPL-style loader after they have already
        // accepted a block byte. If the CPU is still polling for that byte's
        // echo, bridge the rest of the APUIO upload into ARAM directly.
        if p != 0
            || self.boot_state != BootState::Running
            || self.loader_hle_active
            || !self.loader_hle_enabled
            || self.apu_to_cpu_ports[0] != 0xAA
            || self.apu_to_cpu_ports[1] != 0xBB
            || self.port_latch[0] == 0xCC
            || !self.is_spc_at_ipl_style_loader_ready()
        {
            self.loader_ready_stall_reads = 0;
            return visible;
        }

        self.loader_ready_stall_reads = self.loader_ready_stall_reads.saturating_add(1);
        if self.loader_ready_stall_reads < 1 {
            return visible;
        }

        self.capture_loader_hle_resume();
        if !self.loader_hle_has_resume {
            return visible;
        }

        self.loader_hle_active = true;
        self.boot_state = BootState::ReadySignature;
        self.boot_port0_echo = 0xAA;
        self.apu_to_cpu_ports[0] = 0xAA;
        self.apu_to_cpu_ports[1] = 0xBB;
        if crate::debug_flags::trace_apu_bootstate() {
            println!(
                "[APU-BOOTSTATE] loader HLE ready resume={} pc=${:04X} sp={:02X} latch=[{:02X} {:02X} {:02X} {:02X}]",
                self.loader_hle_has_resume as u8,
                self.loader_hle_resume_pc,
                self.loader_hle_resume_sp,
                self.port_latch[0],
                self.port_latch[1],
                self.port_latch[2],
                self.port_latch[3]
            );
        }
        self.expected_index = 0;
        self.pending_idx = None;
        self.pending_cmd = None;
        self.data_ready = false;
        visible
    }

    fn is_spc_at_ipl_style_loader_ready(&self) -> bool {
        let Some(pc) = self.inner.smp.as_ref().map(|smp| smp.reg_pc) else {
            return false;
        };

        // Detect a RAM copy of the standard IPL loader's AA/BB ready loop.
        // The S-SMP may be in the compare or branch instruction when the S-CPU
        // polls port0, so scan a small window ending at the current PC.
        const READY_LOOP: [u8; 11] = [
            0x8F, 0xAA, 0xF4, // MOV $F4,#$AA
            0x8F, 0xBB, 0xF5, // MOV $F5,#$BB
            0x78, 0xCC, 0xF4, // CMP $F4,#$CC
            0xD0, 0xFB, // BNE back to CMP
        ];

        for back in 0..=16u16 {
            let start = pc.wrapping_sub(back);
            if start >= 0xFFC0 {
                continue;
            }
            let matches = READY_LOOP.iter().enumerate().all(|(offset, &byte)| {
                self.inner.peek_u8(start.wrapping_add(offset as u16)) == byte
            });
            if matches {
                return true;
            }
        }
        false
    }

    fn capture_loader_hle_resume(&mut self) {
        let Some(sp) = self.inner.smp.as_ref().map(|smp| smp.reg_sp) else {
            self.loader_hle_has_resume = false;
            self.loader_hle_resume_pc = 0;
            self.loader_hle_resume_sp = 0;
            return;
        };

        let lo_addr = 0x0100 | u16::from(sp.wrapping_add(1));
        let hi_addr = 0x0100 | u16::from(sp.wrapping_add(2));
        let lo = self.inner.peek_u8(lo_addr);
        let hi = self.inner.peek_u8(hi_addr);
        let pc = u16::from(lo) | (u16::from(hi) << 8);
        self.loader_hle_has_resume = pc != 0 && pc < 0xFFC0;
        self.loader_hle_resume_pc = pc;
        self.loader_hle_resume_sp = sp.wrapping_add(2);
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
                let v =
                    self.maybe_enter_loader_hle_from_ready_signature(p, self.apu_to_cpu_ports[p]);
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

        if !(self.boot_hle_enabled || self.loader_hle_active) {
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
        let was_loader_hle = self.loader_hle_active;
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
        self.loader_hle_active = was_loader_hle;
        if !was_loader_hle {
            self.loader_hle_has_resume = false;
            self.loader_hle_resume_pc = 0;
            self.loader_hle_resume_sp = 0;
        }
        self.loader_ready_stall_reads = 0;
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
        let addr = self.upload_addr;
        self.inner.write_u8(addr as u32, data);
        self.upload_addr = self.upload_addr.wrapping_add(1);
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
        self.loader_hle_active = false;
        self.loader_hle_has_resume = false;
        self.loader_hle_resume_pc = 0;
        self.loader_hle_resume_sp = 0;
        self.loader_ready_stall_reads = 0;
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
        let was_loader_hle = self.loader_hle_active;
        self.boot_state = BootState::Running;
        self.loader_hle_active = false;
        self.loader_ready_stall_reads = 0;
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

        if was_loader_hle {
            self.pending_port_writes.clear();
            for p in 0..4 {
                self.inner.cpu_write_port(p as u8, self.port_latch[p]);
            }
            if self.loader_hle_has_resume {
                if let Some(smp) = self.inner.smp.as_mut() {
                    smp.reg_pc = self.loader_hle_resume_pc;
                    smp.reg_sp = self.loader_hle_resume_sp;
                    smp.is_stopped = false;
                    smp.is_sleeping = false;
                    smp.cycle_count = 0;
                }
            }
            self.loader_hle_has_resume = false;
            self.loader_hle_resume_pc = 0;
            self.loader_hle_resume_sp = 0;
            self.inner.write_u8(0x00F4, ack);
            for i in 0..4 {
                self.apu_to_cpu_ports[i] = self.inner.cpu_read_port(i as u8);
            }
            return;
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

    pub(super) fn flush_pending_port_writes(&mut self) {
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
