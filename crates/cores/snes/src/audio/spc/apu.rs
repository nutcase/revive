use super::consts::{Spc, IPL_ROM_LEN, RAM_LEN, REG_LEN};
use super::dsp::dsp::Dsp;
use super::smp::{Smp, SmpState};
use super::timer::{Timer, TimerState};

// Standard SPC700 IPL ROM (64 bytes, bsnes/higan reference)
static DEFAULT_IPL_ROM: [u8; IPL_ROM_LEN] = [
    0xcd, 0xef, 0xbd, 0xe8, 0x00, 0xc6, 0x1d, 0xd0, // FFC0
    0xfc, 0x8f, 0xaa, 0xf4, 0x8f, 0xbb, 0xf5, 0x78, // FFC8
    0xcc, 0xf4, 0xd0, 0xfb, 0x2f, 0x19, 0xeb, 0xf4, // FFD0
    0xd0, 0xfc, 0x7e, 0xf4, 0xd0, 0x0b, 0xe4, 0xf5, // FFD8
    0xcb, 0xf4, 0xd7, 0x00, 0xfc, 0xd0, 0xf3, 0xab, // FFE0
    0x01, 0x10, 0xef, 0x7e, 0xf4, 0x10, 0xeb, 0xba, // FFE8
    0xf6, 0xda, 0x00, 0xba, 0xf4, 0xc4, 0xf4, 0xdd, // FFF0
    0x5d, 0xd0, 0xdb, 0x1f, 0x00, 0x00, 0xc0, 0xff,
]; // FFF8

#[derive(Clone)]
pub struct ApuState {
    pub ram: [u8; RAM_LEN],
    pub ipl_rom: [u8; IPL_ROM_LEN],
    pub smp: SmpState,
    pub dsp_regs: [u8; REG_LEN],
    pub timers: [TimerState; 3],
    pub is_ipl_rom_enabled: bool,
    pub dsp_reg_address: u8,
    pub cpu_to_apu_ports: [u8; 4],
    pub apu_to_cpu_ports: [u8; 4],
}

pub struct Apu {
    ram: Box<[u8; RAM_LEN]>,
    ipl_rom: Box<[u8; IPL_ROM_LEN]>,

    pub smp: Option<Box<Smp>>,
    pub dsp: Option<Box<Dsp>>,

    timers: [Timer; 3],

    is_ipl_rom_enabled: bool,
    dsp_reg_address: u8,

    // CPU<->APU I/O ports ($2140-$2143 <-> $F4-$F7)
    //
    // 実機では S-CPU 側と S-SMP 側で「読み出し対象」が異なる（2組のラッチ）。
    // - S-SMP が $F4-$F7 を読むと、S-CPU が APUIO に最後に書いた値を読む（CPU->APU）。
    // - S-SMP が $F4-$F7 に書くと、S-CPU が APUIO を読んだときに返る値が更新される（APU->CPU）。
    //
    // 参考: S-SMP CONTROL($F1) の Clear data ports ビットは「Data-from-CPU read registers」をクリアする。
    cpu_to_apu_ports: [u8; 4],
    apu_to_cpu_ports: [u8; 4],

    /// SPC700 が $F4-$F7 に書き込んだことを示すフラグ。
    /// バッチ実行中にポート書き込みが発生したら run() を中断し、
    /// 中間値を S-CPU 側に反映するために使用する。
    pub port_written: bool,
}

impl Apu {
    #[inline]
    fn test_timers_enabled(test: u8) -> bool {
        // TEST($F0): timer clocks run only when "enable timers" is set and halt is clear.
        (test & 0x08) != 0 && (test & 0x01) == 0
    }

    #[inline]
    fn test_ram_write_enabled(_test: u8) -> bool {
        // TEST($F0) can inhibit Audio-RAM writes on hardware, but commercial
        // sound drivers can accidentally write $00 to $F0 while using indexed
        // direct-page queues. If we hard-gate all RAM writes here, stack pushes
        // stop working and the SPC returns through stale addresses.
        true
    }

    #[inline]
    fn test_ram_read_enabled(_test: u8) -> bool {
        // See test_ram_write_enabled(). Keep normal Audio-RAM readable even if
        // a game writes diagnostic TEST values.
        true
    }

    pub fn new() -> Box<Apu> {
        let mut ret = Box::new(Apu {
            ram: Box::new([0; RAM_LEN]),
            ipl_rom: Box::new([0; IPL_ROM_LEN]),

            smp: None,
            dsp: None,

            timers: [Timer::new(256), Timer::new(256), Timer::new(32)],

            is_ipl_rom_enabled: true,
            dsp_reg_address: 0,

            cpu_to_apu_ports: [0; 4],
            apu_to_cpu_ports: [0; 4],
            port_written: false,
        });
        let ret_ptr = &mut *ret as *mut _;
        ret.smp = Some(Box::new(Smp::new(ret_ptr)));
        ret.dsp = Some(Dsp::new(ret_ptr));
        ret.reset();
        ret
    }

    pub fn reset(&mut self) {
        for i in 0..RAM_LEN {
            self.ram[i] = 0;
        }

        for i in 0..IPL_ROM_LEN {
            self.ipl_rom[i] = DEFAULT_IPL_ROM[i];
        }

        self.smp.as_mut().unwrap().reset();
        self.dsp.as_mut().unwrap().reset();
        for timer in self.timers.iter_mut() {
            timer.reset();
        }

        self.is_ipl_rom_enabled = true;
        self.dsp_reg_address = 0;
        self.cpu_to_apu_ports = [0; 4];
        self.apu_to_cpu_ports = [0; 4];
        // Power-on/reset defaults (S-SMP):
        // - TEST($F0) = $0A
        // - CONTROL($F1) behaves as if IPL is enabled and ports are cleared.
        // Note: $F0/$F1 are write-only and read back as $00; we still store the
        // written values internally for side effects.
        self.set_test_reg(0x0A);
        self.ram[0x00f0] = 0x0A;
        self.set_control_reg(0xB0);
        self.ram[0x00f1] = 0xB0;
    }

    /// TEST($F0) wait states: add 0/1/4/9 cycles on RAM vs I/O/ROM access.
    pub(crate) fn wait_cycles(&self, address: u16) -> i32 {
        let test = self.ram[0x00f0];
        let internal = (test >> 6) & 0x03;
        let external = (test >> 4) & 0x03;
        let wait_table = [0i32, 1, 4, 9];
        let is_io = (address & 0x00f0) == 0x00f0;
        let is_rom = address >= 0xffc0 && self.is_ipl_rom_enabled;
        let wait = if is_io || is_rom { external } else { internal };
        wait_table[wait as usize]
    }

    pub(crate) fn internal_wait_penalty(&self) -> i32 {
        let test = self.ram[0x00f0];
        let internal = (test >> 6) & 0x03;
        let wait_table = [0i32, 1, 4, 9];
        wait_table[internal as usize]
    }

    /// S-CPU からの APUIO 書き込み（$2140-$2143）。S-SMP からは $F4-$F7 の読み出しで観測される。
    pub fn cpu_write_port(&mut self, port: u8, value: u8) {
        let p = (port & 0x03) as usize;
        self.cpu_to_apu_ports[p] = value;
    }

    /// S-CPU からの APUIO 読み出し（$2140-$2143）。S-SMP が $F4-$F7 に書いた値が返る。
    pub fn cpu_read_port(&self, port: u8) -> u8 {
        let p = (port & 0x03) as usize;
        self.apu_to_cpu_ports[p]
    }

    pub fn render(&mut self, left_buffer: &mut [i16], right_buffer: &mut [i16], num_samples: i32) {
        let smp = self.smp.as_mut().unwrap();
        let dsp = self.dsp.as_mut().unwrap();
        while dsp.output_buffer.get_sample_count() < num_samples {
            smp.run(num_samples * 64);
            dsp.flush();
        }

        dsp.output_buffer
            .read(left_buffer, right_buffer, num_samples);
    }

    pub fn cpu_cycles_callback(&mut self, num_cycles: i32) {
        self.dsp.as_mut().unwrap().cycles_callback(num_cycles);
        // TEST($F0) can gate timer clocks.
        let test = self.ram[0x00f0];
        if Self::test_timers_enabled(test) {
            for timer in self.timers.iter_mut() {
                timer.cpu_cycles_callback(num_cycles);
            }
        }
    }

    /// Return DSP echo buffer info: (start_address, length, echo_write_enabled).
    pub fn echo_info(&self) -> (u16, i32, bool) {
        if let Some(dsp) = &self.dsp {
            (
                dsp.get_echo_start_address(),
                dsp.calculate_echo_length(),
                dsp.echo_write_enabled,
            )
        } else {
            (0, 0, false)
        }
    }

    /// Check if any timer has fired since the last check. Clears the flags.
    /// Used to wake SPC700 from SLEEP instruction.
    pub fn any_timer_fired(&mut self) -> bool {
        let mut result = false;
        for timer in self.timers.iter_mut() {
            if timer.fired {
                result = true;
                timer.fired = false;
            }
        }
        result
    }

    pub fn debug_timer_state(&self, idx: usize) -> Option<(i32, bool, u8, u8, u8, i32)> {
        self.timers.get(idx).map(|t| t.debug_state())
    }

    pub fn read_u8(&mut self, address: u32) -> u8 {
        let address = address & 0xffff;
        if address >= 0xf0 && address < 0x0100 {
            match address {
                // Write-only registers read back as $00.
                0xf0 | 0xf1 => 0x00,

                0xf2 => self.dsp_reg_address,
                0xf3 => self
                    .dsp
                    .as_mut()
                    .unwrap()
                    .get_register(self.dsp_reg_address),

                // Write-only timer targets read back as $00.
                0xfa..=0xfc => 0x00,

                0xfd => {
                    let v = self.timers[0].read_counter();
                    if crate::debug_flags::trace_burnin_smp_timer() {
                        use std::sync::atomic::{AtomicU32, Ordering};
                        static CNT: AtomicU32 = AtomicU32::new(0);
                        let n = CNT.fetch_add(1, Ordering::Relaxed);
                        if n < 64 {
                            let pc = self.smp.as_ref().map(|s| s.reg_pc).unwrap_or(0);
                            let ctrl = self.ram[0x00f1];
                            let target = self.ram[0x00fa];
                            let (ticks, running, t_target, low, high, res) =
                                self.timers[0].debug_state();
                            println!(
                                "[SMP-T0] pc={:04X} $FD -> {:02X} (ctrl={:02X} target={:02X} timer:run={} res={} ticks={} low={:02X} high={:02X} tgt={:02X})",
                                pc, v, ctrl, target, running, res, ticks, low, high, t_target
                            );
                        }
                    }
                    v
                }
                0xfe => self.timers[1].read_counter(),
                0xff => self.timers[2].read_counter(),

                // CPU->APU ports: reads return values written by the S-CPU.
                0xf4..=0xf7 => {
                    let idx = (address - 0xf4) as usize;
                    let v = self.cpu_to_apu_ports[idx];

                    if crate::debug_flags::trace_sfs_apu_f4_read() {
                        use std::sync::atomic::{AtomicU32, Ordering};
                        static CNT: AtomicU32 = AtomicU32::new(0);
                        let (pc, a, x, y, psw) = self
                            .smp
                            .as_ref()
                            .map(|s| (s.reg_pc, s.reg_a, s.reg_x, s.reg_y, s.get_psw()))
                            .unwrap_or((0, 0, 0, 0, 0));
                        // Skip IPL-only noise so we can see post-upload port reads.
                        if pc < 0xFFC0 {
                            let n = CNT.fetch_add(1, Ordering::Relaxed);
                            if n < 512 {
                                println!(
                                    "[SFS][SMP][F{:X}R] pc={:04X} A={:02X} X={:02X} Y={:02X} psw={:02X} -> {:02X} in=[{:02X} {:02X} {:02X} {:02X}]",
                                    4 + idx,
                                    pc,
                                    a,
                                    x,
                                    y,
                                    psw,
                                    v,
                                    self.cpu_to_apu_ports[0],
                                    self.cpu_to_apu_ports[1],
                                    self.cpu_to_apu_ports[2],
                                    self.cpu_to_apu_ports[3]
                                );
                            }
                        }
                    }
                    if crate::debug_flags::trace_burnin_apu_f4f7_reads() {
                        use std::sync::atomic::{AtomicU32, Ordering};
                        static CNT: AtomicU32 = AtomicU32::new(0);
                        static LAST: std::sync::OnceLock<std::sync::Mutex<[u8; 4]>> =
                            std::sync::OnceLock::new();
                        let mut last = LAST
                            .get_or_init(|| std::sync::Mutex::new([0; 4]))
                            .lock()
                            .unwrap();
                        if last[idx] != v {
                            let n = CNT.fetch_add(1, Ordering::Relaxed);
                            if n < 512 {
                                let pc = self.smp.as_ref().map(|s| s.reg_pc).unwrap_or(0);
                                println!(
                                    "[SMP][F{:X}R] pc={:04X} {:02X}->{:02X}",
                                    4 + idx,
                                    pc,
                                    last[idx],
                                    v
                                );
                            }
                            last[idx] = v;
                        }
                    }
                    // Trace SPC port reads for ToP voice streaming investigation
                    if crate::debug_flags::trace_top_spc_cmd() {
                        let pc = self.smp.as_ref().map(|s| s.reg_pc).unwrap_or(0);
                        if pc < 0xFFC0 {
                            use std::sync::atomic::{AtomicU32, Ordering};
                            static CMD_CNT: AtomicU32 = AtomicU32::new(0);
                            static LAST_P0: std::sync::atomic::AtomicU8 =
                                std::sync::atomic::AtomicU8::new(0);
                            // One-time dump of SPC code at key addresses
                            static DUMP_DONE: std::sync::OnceLock<()> = std::sync::OnceLock::new();
                            if pc == 0x19A1 && DUMP_DONE.set(()).is_ok() {
                                // Dump code at main loop ($1990-$19C0), transfer ($1F80-$1FA0),
                                // cmd handler ($2110-$2130), and response ($1A00-$1A80)
                                for &(label, start, len) in &[
                                    ("MAINLOOP", 0x1980u16, 96u16),
                                    ("XFER1F00", 0x1F00u16, 192u16),
                                    ("CMD2117", 0x2100, 80),
                                    ("RESP1A", 0x19F0, 144),
                                    ("INIT0840", 0x0840, 32),
                                    ("JMPTBL20B7", 0x20B7u16, 64u16),
                                    ("MAINDISP19CF", 0x19CFu16, 48u16),
                                    ("VOICECMD1C00", 0x1C00u16, 224u16),
                                    ("VOICEDAT1D00", 0x1D00u16, 224u16),
                                    ("PATTENG0F00", 0x0F00u16, 128u16),
                                    ("PATTENG0980", 0x0980u16, 128u16),
                                    ("DSPCFG2500", 0x2500u16, 160u16),
                                    ("CMD11_1B80", 0x1B80u16, 80u16),
                                    ("CMD12_1BD0", 0x1BD0u16, 64u16),
                                    ("CMD12_1BEF", 0x1BEFu16, 64u16),
                                    ("SUB09EC", 0x09E0u16, 64u16),
                                    ("SUB0C5F", 0x0C50u16, 96u16),
                                    ("VOICE2480", 0x2480u16, 128u16),
                                    ("SUB2115", 0x2100u16, 96u16),
                                    ("SUB09C0", 0x09B0u16, 48u16),
                                    ("VOICEPROC25E0", 0x25E0u16, 160u16),
                                    ("SUB230D", 0x2300u16, 80u16),
                                    ("OVERFLOW20F0", 0x20F0u16, 32u16),
                                    ("SUB0CA0", 0x0CA0u16, 64u16),
                                    ("SUB14C0", 0x14C0u16, 64u16),
                                    ("SUB1368", 0x1360u16, 128u16),
                                    ("SUB22B7", 0x2280u16, 128u16),
                                    ("SUB2260", 0x2260u16, 48u16),
                                    ("CMD10_1B50", 0x1B50u16, 56u16),
                                ] {
                                    let mut buf = vec![0u8; len as usize];
                                    for i in 0..len {
                                        buf[i as usize] =
                                            self.ram[(start.wrapping_add(i)) as usize];
                                    }
                                    eprintln!("[SPC-DUMP] {}@{:04X}: {:02X?}", label, start, buf);
                                }
                                // Dump DSP source directory (DIR * 256, 64 entries * 4 bytes each)
                                let dsp = self.dsp.as_mut().unwrap();
                                let dir_base = (dsp.get_register(0x5D) as u16) << 8;
                                eprintln!("[SPC-DUMP] DIR_BASE={:04X}", dir_base);
                                for src in 0..64u16 {
                                    let addr = dir_base + src * 4;
                                    let start_lo = self.ram[addr as usize];
                                    let start_hi = self.ram[(addr + 1) as usize];
                                    let loop_lo = self.ram[(addr + 2) as usize];
                                    let loop_hi = self.ram[(addr + 3) as usize];
                                    let start_addr = (start_hi as u16) << 8 | start_lo as u16;
                                    let loop_addr = (loop_hi as u16) << 8 | loop_lo as u16;
                                    if start_addr != 0 || loop_addr != 0 {
                                        // Also dump first 9 bytes (1 BRR block) at the sample start
                                        let mut brr = [0u8; 9];
                                        for i in 0..9 {
                                            brr[i] = self.ram
                                                [(start_addr.wrapping_add(i as u16)) as usize];
                                        }
                                        eprintln!(
                                            "[SPC-DIR] src={:02} start={:04X} loop={:04X} brr[0]={:02X}({}) hdr={}{}",
                                            src, start_addr, loop_addr, brr[0],
                                            match (brr[0] >> 2) & 3 { 0 => "flt0", 1 => "flt1", 2 => "flt2", 3 => "flt3", _ => "?" },
                                            if brr[0] & 1 != 0 { "E" } else { "-" },
                                            if brr[0] & 2 != 0 { "L" } else { "-" },
                                        );
                                    }
                                }
                            }
                            if idx == 0 {
                                let prev = LAST_P0.swap(v, Ordering::Relaxed);
                                if prev != v {
                                    let n = CMD_CNT.fetch_add(1, Ordering::Relaxed);
                                    if n < 50000 {
                                        let a = self.smp.as_ref().map(|s| s.reg_a).unwrap_or(0);
                                        let dp83 = self.ram[0x83];
                                        let xor_val = v ^ dp83;
                                        let cmd_idx = xor_val & 0x7F;
                                        let is_new_cmd = (xor_val & 0x80) != 0;
                                        eprintln!(
                                            "[SPC-CMD] pc={:04X} A={:02X} F4={:02X} dp83={:02X} xor={:02X} {}cmd={} out=[{:02X} {:02X} {:02X} {:02X}]",
                                            pc, a, v, dp83, xor_val,
                                            if is_new_cmd { "NEW " } else { "--- " },
                                            cmd_idx,
                                            self.apu_to_cpu_ports[0],
                                            self.apu_to_cpu_ports[1],
                                            self.apu_to_cpu_ports[2],
                                            self.apu_to_cpu_ports[3]
                                        );
                                        // Dump dp state for voice-related commands
                                        if is_new_cmd
                                            && (cmd_idx == 11
                                                || cmd_idx == 12
                                                || cmd_idx == 13
                                                || cmd_idx == 15
                                                || cmd_idx == 16
                                                || cmd_idx == 18
                                                || cmd_idx == 29
                                                || cmd_idx == 43
                                                || cmd_idx == 4
                                                || cmd_idx == 10
                                                || cmd_idx == 30)
                                        {
                                            let ports = (
                                                self.cpu_to_apu_ports[1],
                                                self.cpu_to_apu_ports[2],
                                                self.cpu_to_apu_ports[3],
                                            );
                                            eprintln!(
                                                "[SPC-VDUMP] cmd={} ports=({:02X},{:02X},{:02X}) dp[$CC]={:02X} dp[$CD]={:02X} dp[$B4..BF]=[{:02X} {:02X} {:02X} {:02X} {:02X} {:02X} {:02X} {:02X} {:02X} {:02X} {:02X} {:02X}] dp[$C0..CF]=[{:02X} {:02X} {:02X} {:02X} {:02X} {:02X} {:02X} {:02X} {:02X} {:02X} {:02X} {:02X} {:02X} {:02X} {:02X} {:02X}]",
                                                cmd_idx, ports.0, ports.1, ports.2,
                                                self.ram[0xCC], self.ram[0xCD],
                                                self.ram[0xB4], self.ram[0xB5], self.ram[0xB6], self.ram[0xB7],
                                                self.ram[0xB8], self.ram[0xB9], self.ram[0xBA], self.ram[0xBB],
                                                self.ram[0xBC], self.ram[0xBD], self.ram[0xBE], self.ram[0xBF],
                                                self.ram[0xC0], self.ram[0xC1], self.ram[0xC2], self.ram[0xC3],
                                                self.ram[0xC4], self.ram[0xC5], self.ram[0xC6], self.ram[0xC7],
                                                self.ram[0xC8], self.ram[0xC9], self.ram[0xCA], self.ram[0xCB],
                                                self.ram[0xCC], self.ram[0xCD], self.ram[0xCE], self.ram[0xCF],
                                            );
                                            // Also dump dp[$A0], dp[$80], and first bytes of voice slots 14,15
                                            eprintln!(
                                                "[SPC-VDUMP2] dp[$80]={:02X} dp[$A0]={:02X} dp[$A1]={:02X} dp[$C3]={:02X} dp[$C6]={:02X} slot14[0..8]=[{:02X} {:02X} {:02X} {:02X} {:02X} {:02X} {:02X} {:02X}] slot15[0..8]=[{:02X} {:02X} {:02X} {:02X} {:02X} {:02X} {:02X} {:02X}]",
                                                self.ram[0x80], self.ram[0xA0], self.ram[0xA1],
                                                self.ram[0xC3], self.ram[0xC6],
                                                self.ram[0x03C0], self.ram[0x03C1], self.ram[0x03C2], self.ram[0x03C3],
                                                self.ram[0x03C4], self.ram[0x03C5], self.ram[0x03C6], self.ram[0x03C7],
                                                self.ram[0x03E0], self.ram[0x03E1], self.ram[0x03E2], self.ram[0x03E3],
                                                self.ram[0x03E4], self.ram[0x03E5], self.ram[0x03E6], self.ram[0x03E7],
                                            );
                                        }
                                    }
                                }
                            }
                        }
                    }
                    v
                }

                _ => self.ram[address as usize],
            }
        } else if address >= 0xffc0 && self.is_ipl_rom_enabled {
            self.ipl_rom[(address - 0xffc0) as usize]
        } else {
            let test = self.ram[0x00f0];
            if !Self::test_ram_read_enabled(test) {
                0x00
            } else {
                self.ram[address as usize]
            }
        }
    }

    /// Debug-only peek: read RAM/IPL without side effects (no timers/ports).
    pub(crate) fn peek_u8(&self, address: u16) -> u8 {
        let address = address as usize & 0xFFFF;
        if address >= 0xFFC0 && self.is_ipl_rom_enabled {
            self.ipl_rom[address - 0xFFC0]
        } else {
            self.ram[address]
        }
    }

    pub fn write_u8(&mut self, address: u32, value: u8) {
        let address = address & 0xffff;
        if address >= 0x00f0 && address < 0x0100 {
            match address {
                0xf0 => {
                    // TEST ($F0) writes only take effect when the P flag is clear.
                    let psw = self.smp.as_ref().map(|s| s.get_psw()).unwrap_or(0);
                    if (psw & 0x20) == 0 {
                        if crate::debug_flags::trace_burnin_apu_f0_writes() {
                            use std::sync::atomic::{AtomicU32, Ordering};
                            static CNT: AtomicU32 = AtomicU32::new(0);
                            let n = CNT.fetch_add(1, Ordering::Relaxed);
                            if n < 256 {
                                let pc = self.smp.as_ref().map(|s| s.reg_pc).unwrap_or(0);
                                let prev = self.ram[address as usize];
                                println!(
                                    "[SMP][F0W] pc={:04X} psw={:02X} {:02X}->{:02X}",
                                    pc, psw, prev, value
                                );
                            }
                        }
                        self.set_test_reg(value);
                        self.ram[address as usize] = value;
                    }
                }
                0xf1 => {
                    if crate::debug_flags::trace_burnin_apu_f1_writes() {
                        use std::sync::atomic::{AtomicU32, Ordering};
                        static CNT: AtomicU32 = AtomicU32::new(0);
                        let n = CNT.fetch_add(1, Ordering::Relaxed);
                        if n < 1024 {
                            let pc = self.smp.as_ref().map(|s| s.reg_pc).unwrap_or(0);
                            let prev = self.ram[address as usize];
                            println!("[SMP][F1W] pc={:04X} {:02X}->{:02X}", pc, prev, value);
                        }
                    }
                    self.set_control_reg(value);
                    self.ram[address as usize] = value;
                    if crate::debug_flags::trace_burnin_smp_timer() {
                        use std::sync::atomic::{AtomicU32, Ordering};
                        static CNT: AtomicU32 = AtomicU32::new(0);
                        let n = CNT.fetch_add(1, Ordering::Relaxed);
                        if n < 64 {
                            let pc = self.smp.as_ref().map(|s| s.reg_pc).unwrap_or(0);
                            println!("[SMP-CTRL] pc={:04X} $F1 <- {:02X}", pc, value);
                        }
                    }
                }
                0xf2 => {
                    self.dsp_reg_address = value;
                    self.ram[address as usize] = value;
                }
                0xf3 => {
                    if crate::debug_flags::trace_top_spc_cmd() {
                        let reg = self.dsp_reg_address;
                        // Trace KON, KOFF, VxSRCN, VxVOLL/R, VxPITCHL/H, DIR, FLG
                        let is_interesting = reg == 0x4C || reg == 0x5C || reg == 0x6C || reg == 0x7C
                            || reg == 0x5D  // DIR
                            || (reg & 0x0F) == 0x04  // VxSRCN
                            || (reg & 0x0F) == 0x00  // VxVOLL
                            || (reg & 0x0F) == 0x01  // VxVOLR
                            || (reg & 0x0F) == 0x02  // VxPITCHL
                            || (reg & 0x0F) == 0x03  // VxPITCHH
                            || (reg & 0x0F) == 0x05  // VxADSR0
                            || (reg & 0x0F) == 0x06  // VxADSR1
                            || (reg & 0x0F) == 0x07; // VxGAIN
                        if is_interesting {
                            use std::sync::atomic::{AtomicU32, Ordering};
                            static DSP_W_CNT: AtomicU32 = AtomicU32::new(0);
                            let n = DSP_W_CNT.fetch_add(1, Ordering::Relaxed);
                            let pc = self.smp.as_ref().map(|s| s.reg_pc).unwrap_or(0);
                            let noisy_reset_write = pc == 0x040F
                                && matches!(reg, 0x4C | 0x5C | 0x6C)
                                && matches!((reg, value), (0x4C | 0x5C, 0x00) | (0x6C, 0x20));
                            if n < 5000 && !noisy_reset_write {
                                let name = match reg {
                                    0x4C => "KON",
                                    0x5C => "KOFF",
                                    0x6C => "FLG",
                                    0x7C => "ENDX",
                                    0x5D => "DIR",
                                    _ if (reg & 0x0F) == 0x04 => "SRCN",
                                    _ if (reg & 0x0F) == 0x00 => "VOLL",
                                    _ if (reg & 0x0F) == 0x01 => "VOLR",
                                    _ if (reg & 0x0F) == 0x02 => "PITCHL",
                                    _ if (reg & 0x0F) == 0x03 => "PITCHH",
                                    _ if (reg & 0x0F) == 0x05 => "ADSR0",
                                    _ if (reg & 0x0F) == 0x06 => "ADSR1",
                                    _ if (reg & 0x0F) == 0x07 => "GAIN",
                                    _ => "?",
                                };
                                let voice = (reg >> 4) & 0x07;
                                eprintln!(
                                    "[DSP-W] pc={:04X} reg=${:02X}({} v{}) val={:02X}",
                                    pc, reg, name, voice, value
                                );
                            }
                        }
                    }
                    self.dsp
                        .as_mut()
                        .unwrap()
                        .set_register(self.dsp_reg_address, value);
                }

                // APU->CPU ports: writes update values readable by the S-CPU.
                0xf4..=0xf7 => {
                    let idx = (address - 0xf4) as usize;
                    let prev = self.apu_to_cpu_ports[idx];
                    self.apu_to_cpu_ports[idx] = value;
                    self.port_written = true;
                    if crate::debug_flags::trace_burnin_apu_f4_writes()
                        && address == 0x00f4
                        && prev != value
                    {
                        let spc_pc = self.smp.as_ref().map(|s| s.reg_pc).unwrap_or(0);
                        if spc_pc < 0xFFC0 {
                            use std::sync::atomic::{AtomicU32, Ordering};
                            static GAME_CNT: AtomicU32 = AtomicU32::new(0);
                            let n = GAME_CNT.fetch_add(1, Ordering::Relaxed);
                            if n < 64 {
                                let psw = self.smp.as_ref().map(|s| s.get_psw()).unwrap_or(0);
                                let sp = self.smp.as_ref().map(|s| s.reg_sp).unwrap_or(0);
                                let a = self.smp.as_ref().map(|s| s.reg_a).unwrap_or(0);
                                eprintln!(
                                    "[SMP][F4W-GAME] pc={:04X} A={:02X} SP={:02X} psw={:02X} {:02X}->{:02X}",
                                    spc_pc, a, sp, psw, prev, value
                                );
                            }
                        }
                    }
                    // Trace SPC $F4-$F7 writes for ToP voice streaming investigation
                    if crate::debug_flags::trace_top_spc_cmd() && prev != value {
                        let spc_pc = self.smp.as_ref().map(|s| s.reg_pc).unwrap_or(0);
                        if spc_pc < 0xFFC0 {
                            use std::sync::atomic::{AtomicU32, Ordering};
                            static RESP_CNT: AtomicU32 = AtomicU32::new(0);
                            let n = RESP_CNT.fetch_add(1, Ordering::Relaxed);
                            if n < 2000 {
                                let a = self.smp.as_ref().map(|s| s.reg_a).unwrap_or(0);
                                eprintln!(
                                    "[SPC-RESP] pc={:04X} A={:02X} F{:X}write {:02X}->{:02X} in=[{:02X} {:02X} {:02X} {:02X}]",
                                    spc_pc, a, 4 + idx, prev, value,
                                    self.cpu_to_apu_ports[0],
                                    self.cpu_to_apu_ports[1],
                                    self.cpu_to_apu_ports[2],
                                    self.cpu_to_apu_ports[3]
                                );
                            }
                        }
                    }
                    if crate::debug_flags::trace_burnin_apu_f5_writes()
                        && address == 0x00f5
                        && prev != value
                    {
                        use std::sync::atomic::{AtomicU32, Ordering};
                        static CNT: AtomicU32 = AtomicU32::new(0);
                        let n = CNT.fetch_add(1, Ordering::Relaxed);
                        if n < 256 {
                            let pc = self.smp.as_ref().map(|s| s.reg_pc).unwrap_or(0);
                            let psw = self.smp.as_ref().map(|s| s.get_psw()).unwrap_or(0);
                            let ctx_start = pc.wrapping_sub(0x10);
                            let mut code = [0u8; 32];
                            for (i, b) in code.iter_mut().enumerate() {
                                *b = self.read_u8(ctx_start.wrapping_add(i as u16) as u32);
                            }
                            println!(
                                "[SMP][F5W] pc={:04X} psw={:02X} {:02X}->{:02X} in=[{:02X} {:02X} {:02X} {:02X}] code@{:04X}={:02X?}",
                                pc,
                                psw,
                                prev,
                                value,
                                self.cpu_to_apu_ports[0],
                                self.cpu_to_apu_ports[1],
                                self.cpu_to_apu_ports[2],
                                self.cpu_to_apu_ports[3],
                                ctx_start,
                                code
                            );
                            // Optional: dump more of the APU routine around $09E7 once.
                            if crate::debug_flags::trace_burnin_apu_dump_09e7() && pc == 0x09E6 {
                                static ONCE: std::sync::OnceLock<()> = std::sync::OnceLock::new();
                                if ONCE.set(()).is_ok() {
                                    let start = 0x09E7u16;
                                    let mut blob = [0u8; 128];
                                    for (i, b) in blob.iter_mut().enumerate() {
                                        *b = self.read_u8(start.wrapping_add(i as u16) as u32);
                                    }
                                    println!("[SMP][DUMP09E7] @09E7={:02X?}", blob);
                                }
                            }
                        }
                    }
                }

                // $F8-$F9 are general-purpose RAM locations (not CPU ports).
                0xf8..=0xf9 => {
                    self.ram[address as usize] = value;
                }

                0xfa => {
                    self.timers[0].set_target(value);
                    self.ram[address as usize] = value;
                    if crate::debug_flags::trace_burnin_smp_timer() {
                        use std::sync::atomic::{AtomicU32, Ordering};
                        static CNT: AtomicU32 = AtomicU32::new(0);
                        let n = CNT.fetch_add(1, Ordering::Relaxed);
                        if n < 64 {
                            let pc = self.smp.as_ref().map(|s| s.reg_pc).unwrap_or(0);
                            println!("[SMP-T0TGT] pc={:04X} $FA <- {:02X}", pc, value);
                        }
                    }
                }
                0xfb => {
                    self.timers[1].set_target(value);
                    self.ram[address as usize] = value;
                }
                0xfc => {
                    self.timers[2].set_target(value);
                    self.ram[address as usize] = value;
                }

                _ => (), // Do nothing
            }
        } else {
            let test = self.ram[0x00f0];
            if !Self::test_ram_write_enabled(test) {
                return;
            }
            if crate::debug_flags::trace_sfs_apu_var81() && (address == 0x0081 || address == 0x0181)
            {
                use std::sync::atomic::{AtomicU32, Ordering};
                static CNT: AtomicU32 = AtomicU32::new(0);
                let n = CNT.fetch_add(1, Ordering::Relaxed);
                if n < 256 {
                    let pc = self.smp.as_ref().map(|s| s.reg_pc).unwrap_or(0);
                    let psw = self.smp.as_ref().map(|s| s.get_psw()).unwrap_or(0);
                    let prev = self.ram[address as usize];
                    println!(
                        "[SFS][SMP][VAR81] pc={:04X} psw={:02X} ${:04X} {:02X}->{:02X}",
                        pc, psw, address, prev, value
                    );
                }
            }
            if crate::debug_flags::trace_sfs_apu_var14() && (address == 0x0014 || address == 0x0114)
            {
                use std::sync::atomic::{AtomicU32, Ordering};
                static CNT: AtomicU32 = AtomicU32::new(0);
                let n = CNT.fetch_add(1, Ordering::Relaxed);
                if n < 256 {
                    let pc = self.smp.as_ref().map(|s| s.reg_pc).unwrap_or(0);
                    let psw = self.smp.as_ref().map(|s| s.get_psw()).unwrap_or(0);
                    let prev = self.ram[address as usize];
                    println!(
                        "[SFS][SMP][VAR14] pc={:04X} psw={:02X} ${:04X} {:02X}->{:02X}",
                        pc, psw, address, prev, value
                    );
                }
            }
            if crate::debug_flags::trace_burnin_apu_var2a()
                && (address == 0x002A || address == 0x012A)
            {
                use std::sync::atomic::{AtomicU32, Ordering};
                static CNT: AtomicU32 = AtomicU32::new(0);
                let n = CNT.fetch_add(1, Ordering::Relaxed);
                if n < 256 {
                    let pc = self.smp.as_ref().map(|s| s.reg_pc).unwrap_or(0);
                    let psw = self.smp.as_ref().map(|s| s.get_psw()).unwrap_or(0);
                    let prev = self.ram[address as usize];
                    println!(
                        "[SMP][VAR2A] pc={:04X} psw={:02X} ${:04X} {:02X}->{:02X}",
                        pc, psw, address, prev, value
                    );
                }
            }
            // Hatsushiba sound driver fix (Tales of Phantasia, Star Ocean):
            // The dispatch at $1659 uses CMP #$FD / BEQ ($F0) which only catches $FD,
            // leaving $FE/$FF to fall through to the jump table and crash when the
            // echo buffer overwrites handler code. Fix: patch BEQ→BCS at load time.
            if address == 0x165B
                && value == 0xF0
                && self.ram[0x1659] == 0x68
                && self.ram[0x165A] == 0xFD
            {
                self.ram[address as usize] = 0xB0; // BEQ → BCS
                return;
            }
            self.ram[address as usize] = value;
        }
    }

    pub fn set_state(&mut self, spc: &Spc) {
        self.reset();

        for i in 0..RAM_LEN {
            self.ram[i] = spc.ram[i];
        }
        for i in 0..IPL_ROM_LEN {
            self.ipl_rom[i] = spc.ipl_rom[i];
        }

        {
            let smp = self.smp.as_mut().unwrap();
            smp.reg_pc = spc.pc;
            smp.reg_a = spc.a;
            smp.reg_x = spc.x;
            smp.reg_y = spc.y;
            smp.set_psw(spc.psw);
            smp.reg_sp = spc.sp;
        }

        self.dsp.as_mut().unwrap().set_state(spc);

        for i in 0..3 {
            self.timers[i].set_target(self.ram[0xfa + i]);
        }
        let test_reg = self.ram[0x00f0];
        self.set_test_reg(test_reg);
        let control_reg = self.ram[0xf1];
        self.set_control_reg(control_reg);

        self.dsp_reg_address = self.ram[0xf2];
    }

    pub fn get_state(&mut self) -> ApuState {
        let smp_state = self.smp.as_ref().unwrap().get_state();
        let mut dsp_regs = [0u8; REG_LEN];
        let dsp = self.dsp.as_mut().unwrap();
        for i in 0..REG_LEN {
            dsp_regs[i] = dsp.get_register(i as u8);
        }
        ApuState {
            ram: self.ram.as_ref().clone(),
            ipl_rom: self.ipl_rom.as_ref().clone(),
            smp: smp_state,
            dsp_regs,
            timers: [
                self.timers[0].get_state(),
                self.timers[1].get_state(),
                self.timers[2].get_state(),
            ],
            is_ipl_rom_enabled: self.is_ipl_rom_enabled,
            dsp_reg_address: self.dsp_reg_address,
            cpu_to_apu_ports: self.cpu_to_apu_ports,
            apu_to_cpu_ports: self.apu_to_cpu_ports,
        }
    }

    pub fn set_state_from(&mut self, state: &ApuState) {
        self.reset();

        self.ram.as_mut().copy_from_slice(&state.ram);
        self.ipl_rom.as_mut().copy_from_slice(&state.ipl_rom);

        self.smp.as_mut().unwrap().set_state(&state.smp);
        self.dsp
            .as_mut()
            .unwrap()
            .set_state_from_regs(&state.dsp_regs);

        self.timers[0].set_state(&state.timers[0]);
        self.timers[1].set_state(&state.timers[1]);
        self.timers[2].set_state(&state.timers[2]);

        self.is_ipl_rom_enabled = state.is_ipl_rom_enabled;
        self.dsp_reg_address = state.dsp_reg_address;
        self.cpu_to_apu_ports = state.cpu_to_apu_ports;
        self.apu_to_cpu_ports = state.apu_to_cpu_ports;
    }

    /// Debug: dump SPC700 state (PC, registers, sleep/stop, ports)
    pub fn debug_spc_state(&self) -> String {
        let smp = self.smp.as_ref().unwrap();
        let pc = smp.reg_pc;
        let a = smp.reg_a;
        let x = smp.reg_x;
        let y = smp.reg_y;
        let sp = smp.reg_sp;
        let psw = smp.get_psw();
        let stopped = smp.is_stopped;
        let sleeping = smp.is_sleeping;
        let cpu_to_apu = self.cpu_to_apu_ports;
        let apu_to_cpu = self.apu_to_cpu_ports;
        let ipl_enabled = self.is_ipl_rom_enabled;
        // Read a few bytes around PC from RAM
        let mut code_bytes = String::new();
        for i in 0..8u16 {
            let addr = pc.wrapping_add(i);
            let byte = if addr >= 0xFFC0 && ipl_enabled {
                self.ipl_rom[(addr - 0xFFC0) as usize]
            } else {
                self.ram[addr as usize]
            };
            code_bytes.push_str(&format!("{:02X} ", byte));
        }
        format!(
            "PC={:04X} A={:02X} X={:02X} Y={:02X} SP={:02X} PSW={:02X} stopped={} sleeping={} ipl={} c2a=[{:02X},{:02X},{:02X},{:02X}] a2c=[{:02X},{:02X},{:02X},{:02X}] code=[{}]",
            pc, a, x, y, sp, psw, stopped, sleeping, ipl_enabled,
            cpu_to_apu[0], cpu_to_apu[1], cpu_to_apu[2], cpu_to_apu[3],
            apu_to_cpu[0], apu_to_cpu[1], apu_to_cpu[2], apu_to_cpu[3],
            code_bytes.trim()
        )
    }

    pub fn clear_echo_buffer(&mut self) {
        let dsp = self.dsp.as_mut().unwrap();
        let length = dsp.calculate_echo_length();
        let mut end_addr = dsp.get_echo_start_address() as i32 + length;
        if end_addr > RAM_LEN as i32 {
            end_addr = RAM_LEN as i32;
        }
        for i in dsp.get_echo_start_address() as i32..end_addr {
            self.ram[i as usize] = 0xff;
        }
    }

    fn set_test_reg(&mut self, value: u8) {
        let prev = self.ram[0x00f0];
        let prev_enabled = Self::test_timers_enabled(prev);
        let next_enabled = Self::test_timers_enabled(value);
        // When TEST gates timer clocks off, clear pending wake edges so SLEEP does not
        // observe stale timer events from before the gate transition.
        if prev_enabled && !next_enabled {
            for timer in self.timers.iter_mut() {
                timer.fired = false;
            }
        }
    }

    fn set_control_reg(&mut self, value: u8) {
        self.is_ipl_rom_enabled = (value & 0x80) != 0;
        if (value & 0x20) != 0 {
            // Clear data-from-CPU read registers (ports 2/3).
            self.cpu_to_apu_ports[2] = 0x00;
            self.cpu_to_apu_ports[3] = 0x00;
        }
        if (value & 0x10) != 0 {
            // Clear data-from-CPU read registers (ports 0/1).
            self.cpu_to_apu_ports[0] = 0x00;
            self.cpu_to_apu_ports[1] = 0x00;
        }
        self.timers[0].set_start_stop_bit((value & 0x01) != 0);
        self.timers[1].set_start_stop_bit((value & 0x02) != 0);
        self.timers[2].set_start_stop_bit((value & 0x04) != 0);
    }
}
