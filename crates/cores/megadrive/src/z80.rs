use crate::audio::AudioBus;
use crate::cartridge::Cartridge;
use crate::input::IoBus;
use crate::vdp::Vdp;
use std::collections::BTreeMap;
use std::sync::OnceLock;

const FLAG_S: u8 = 0x80;
const FLAG_Z: u8 = 0x40;
const FLAG_Y: u8 = 0x20;
const FLAG_PV: u8 = 0x04;
const FLAG_H: u8 = 0x10;
const FLAG_X: u8 = 0x08;
const FLAG_N: u8 = 0x02;
const FLAG_C: u8 = 0x01;
const M68K_CLOCK_HZ: u64 = 7_670_454;
const Z80_CLOCK_HZ: u64 = 3_579_545;
fn audio_io_wait_cycles() -> u16 {
    static WAIT_CYCLES: OnceLock<u16> = OnceLock::new();
    *WAIT_CYCLES.get_or_init(|| {
        std::env::var("MEGADRIVE_AUDIO_IO_WAIT_CYCLES")
            .ok()
            .and_then(|value| value.parse::<u16>().ok())
            .unwrap_or(2)
            .min(32)
    })
}
const IO_VERSION_ADDR: u32 = 0xA10000;
const IO_PORT1_DATA_ADDR: u32 = 0xA10002;
const IO_PORT2_DATA_ADDR: u32 = 0xA10004;
const IO_PORT1_CTRL_ADDR: u32 = 0xA10008;
const IO_PORT2_CTRL_ADDR: u32 = 0xA1000A;

struct Z80Bus<'a> {
    audio: &'a mut AudioBus,
    cartridge: &'a Cartridge,
    work_ram: &'a mut [u8; 0x10000],
    vdp: &'a mut Vdp,
    io: &'a mut IoBus,
}

#[derive(Debug, Clone, bincode::Encode, bincode::Decode)]
pub struct Z80 {
    bus_requested: bool,
    bus_granted: bool,
    bus_grant_delay_cycles: u32,
    reset_asserted: bool,
    cycles: u64,
    audio_clock_ratio_accumulator: u64,
    clock_ratio_accumulator: u64,
    execution_credit_cycles: i64,
    io_wait_cycles: u16,
    ram: [u8; 0x2000],
    a: u8,
    f: u8,
    a_alt: u8,
    f_alt: u8,
    b: u8,
    c: u8,
    d: u8,
    e: u8,
    h: u8,
    l: u8,
    b_alt: u8,
    c_alt: u8,
    d_alt: u8,
    e_alt: u8,
    h_alt: u8,
    l_alt: u8,
    ix: u16,
    iy: u16,
    pc: u16,
    sp: u16,
    bank_address: u32,
    vdp_data_write_latch: u16,
    vdp_control_write_latch: u16,
    i_reg: u8,
    r_reg: u8,
    interrupt_mode: u8,
    iff1: bool,
    iff2: bool,
    interrupt_inhibit_count: u8,
    interrupt_pending: bool,
    nmi_pending: bool,
    halted: bool,
    unknown_opcode_total: u64,
    unknown_opcode_histogram: BTreeMap<u8, u64>,
    unknown_opcode_pc_histogram: BTreeMap<u16, u64>,
    im0_interrupt_opcode: u8,
}

impl Default for Z80 {
    fn default() -> Self {
        Self {
            bus_requested: false,
            bus_granted: false,
            bus_grant_delay_cycles: 0,
            reset_asserted: true,
            cycles: 0,
            audio_clock_ratio_accumulator: 0,
            clock_ratio_accumulator: 0,
            execution_credit_cycles: 0,
            io_wait_cycles: 0,
            ram: [0; 0x2000],
            a: 0,
            f: 0,
            a_alt: 0,
            f_alt: 0,
            b: 0,
            c: 0,
            d: 0,
            e: 0,
            h: 0,
            l: 0,
            b_alt: 0,
            c_alt: 0,
            d_alt: 0,
            e_alt: 0,
            h_alt: 0,
            l_alt: 0,
            ix: 0,
            iy: 0,
            pc: 0,
            sp: 0x1FFF,
            bank_address: 0,
            vdp_data_write_latch: 0,
            vdp_control_write_latch: 0,
            i_reg: 0,
            r_reg: 0,
            interrupt_mode: 0,
            iff1: false,
            iff2: false,
            interrupt_inhibit_count: 0,
            interrupt_pending: false,
            nmi_pending: false,
            halted: false,
            unknown_opcode_total: 0,
            unknown_opcode_histogram: BTreeMap::new(),
            unknown_opcode_pc_histogram: BTreeMap::new(),
            im0_interrupt_opcode: 0xFF, // Open-bus default => RST 38h.
        }
    }
}

impl Z80 {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn read_busreq_byte(&self) -> u8 {
        // BUSREQ bit is active-low when read:
        // 0 => 68k bus request has been granted (Z80 halted)
        // 1 => bus still owned by Z80 / grant pending
        if self.bus_granted { 0x00 } else { 0x01 }
    }

    pub fn write_busreq_byte(&mut self, value: u8) {
        let requested = (value & 0x01) != 0;
        if requested {
            if !self.bus_requested {
                self.bus_requested = true;
                self.bus_grant_delay_cycles = 16;
            }
        } else {
            self.bus_requested = false;
            self.bus_granted = false;
            self.bus_grant_delay_cycles = 0;
        }
    }

    pub fn read_reset_byte(&self) -> u8 {
        if self.reset_asserted { 0x00 } else { 0x01 }
    }

    pub fn reset_asserted(&self) -> bool {
        self.reset_asserted
    }

    pub fn bus_requested(&self) -> bool {
        self.bus_requested
    }

    pub fn bus_granted(&self) -> bool {
        self.bus_granted
    }

    pub fn write_reset_byte(&mut self, value: u8) {
        let next_asserted = (value & 0x01) == 0;
        if self.reset_asserted && !next_asserted {
            self.a = 0;
            self.a_alt = 0;
            self.b = 0;
            self.c = 0;
            self.d = 0;
            self.e = 0;
            self.h = 0;
            self.l = 0;
            self.b_alt = 0;
            self.c_alt = 0;
            self.d_alt = 0;
            self.e_alt = 0;
            self.h_alt = 0;
            self.l_alt = 0;
            self.ix = 0;
            self.iy = 0;
            self.pc = 0;
            self.sp = 0x1FFF;
            self.bank_address = 0;
            self.vdp_data_write_latch = 0;
            self.vdp_control_write_latch = 0;
            self.i_reg = 0;
            self.r_reg = 0;
            self.interrupt_mode = 0;
            self.iff1 = false;
            self.iff2 = false;
            self.interrupt_inhibit_count = 0;
            self.interrupt_pending = false;
            self.nmi_pending = false;
            self.halted = false;
            self.f = 0;
            self.f_alt = 0;
            self.audio_clock_ratio_accumulator = 0;
            self.clock_ratio_accumulator = 0;
            self.execution_credit_cycles = 0;
            self.io_wait_cycles = 0;
            self.unknown_opcode_total = 0;
            self.unknown_opcode_histogram.clear();
            self.unknown_opcode_pc_histogram.clear();
            self.im0_interrupt_opcode = 0xFF;
        }
        self.reset_asserted = next_asserted;
    }

    pub fn m68k_can_access_ram(&self) -> bool {
        self.bus_granted
    }

    pub fn request_interrupt(&mut self) {
        self.interrupt_pending = true;
    }

    pub fn request_nmi(&mut self) {
        self.nmi_pending = true;
    }

    pub fn read_ram_u8(&self, addr: u16) -> u8 {
        self.ram[(addr as usize) & 0x1FFF]
    }

    pub fn write_ram_u8(&mut self, addr: u16, value: u8) {
        self.ram[(addr as usize) & 0x1FFF] = value;
    }

    pub fn step(
        &mut self,
        m68k_cycles: u32,
        audio: &mut AudioBus,
        cartridge: &Cartridge,
        work_ram: &mut [u8; 0x10000],
        vdp: &mut Vdp,
        io: &mut IoBus,
    ) {
        let mut bus = Z80Bus {
            audio,
            cartridge,
            work_ram,
            vdp,
            io,
        };
        self.audio_clock_ratio_accumulator += (m68k_cycles as u64) * Z80_CLOCK_HZ;
        let audio_granted_cycles = (self.audio_clock_ratio_accumulator / M68K_CLOCK_HZ) as u32;
        self.audio_clock_ratio_accumulator %= M68K_CLOCK_HZ;
        let was_granted = self.bus_granted;
        let mut runnable_m68k_cycles = m68k_cycles;
        if self.bus_requested && !self.bus_granted {
            if m68k_cycles >= self.bus_grant_delay_cycles {
                // Run only until BUSACK is asserted in this timeslice.
                runnable_m68k_cycles = self.bus_grant_delay_cycles;
                self.bus_granted = true;
                self.bus_grant_delay_cycles = 0;
            } else {
                self.bus_grant_delay_cycles -= m68k_cycles;
            }
        }

        if self.reset_asserted || was_granted || runnable_m68k_cycles == 0 {
            if audio_granted_cycles > 0 {
                bus.audio.step_z80_cycles(audio_granted_cycles);
            }
            return;
        }

        self.clock_ratio_accumulator += (runnable_m68k_cycles as u64) * Z80_CLOCK_HZ;
        let granted_cycles = (self.clock_ratio_accumulator / M68K_CLOCK_HZ) as usize;
        self.clock_ratio_accumulator %= M68K_CLOCK_HZ;
        if granted_cycles == 0 {
            if audio_granted_cycles > 0 {
                bus.audio.step_z80_cycles(audio_granted_cycles);
            }
            return;
        }
        self.execution_credit_cycles += granted_cycles as i64;

        let mut time_to_advance = granted_cycles as i64;
        let mut guard = 0usize;
        while self.execution_credit_cycles > 0 && guard < 2048 {
            guard += 1;
            if self.nmi_pending {
                self.nmi_pending = false;
                // Z80 NMI latches previous IFF1 into IFF2, then clears IFF1.
                self.iff2 = self.iff1;
                self.iff1 = false;
                self.halted = false;
                // Interrupt acknowledge includes an M1 cycle, which increments R.
                self.increment_refresh_counter();
                self.push_u16(self.pc, &mut bus);
                self.pc = 0x0066;
                self.execution_credit_cycles -= 11;
                let elapsed_now = 11i64.min(time_to_advance.max(0));
                if elapsed_now > 0 {
                    bus.audio.step_z80_cycles(elapsed_now as u32);
                    time_to_advance -= elapsed_now;
                }
                continue;
            }
            if self.interrupt_pending && self.iff1 {
                if self.interrupt_inhibit_count > 0 {
                    // EI enables IFF immediately, but maskable IRQ recognition is
                    // deferred until after the following instruction.
                } else {
                    self.interrupt_pending = false;
                    self.iff1 = false;
                    self.iff2 = false;
                    self.halted = false;
                    // Interrupt acknowledge includes an M1 cycle, which increments R.
                    self.increment_refresh_counter();
                    self.push_u16(self.pc, &mut bus);
                    let (vector, cycles) = if self.interrupt_mode == 2 {
                        let vector_addr = ((self.i_reg as u16) << 8) | 0x00FF;
                        let lo = self.read_byte(vector_addr, &mut bus);
                        let hi = self.read_byte(vector_addr.wrapping_add(1), &mut bus);
                        (u16::from_le_bytes([lo, hi]), 19u8)
                    } else if self.interrupt_mode == 1 {
                        (0x0038, 13u8)
                    } else {
                        // IM0 executes an externally supplied one-byte opcode.
                        // We model the common MD/open-bus case and accept any RST opcode.
                        let opcode = self.im0_interrupt_opcode;
                        if (opcode & 0xC7) == 0xC7 {
                            ((opcode as u16) & 0x0038, 13u8)
                        } else {
                            (0x0038, 13u8)
                        }
                    };
                    self.pc = vector;
                    self.execution_credit_cycles -= cycles as i64;
                    let elapsed_now = (cycles as i64).min(time_to_advance.max(0));
                    if elapsed_now > 0 {
                        bus.audio.step_z80_cycles(elapsed_now as u32);
                        time_to_advance -= elapsed_now;
                    }
                    continue;
                }
            }
            if self.halted {
                // HALT repeats internal M1 cycles without advancing PC until
                // an interrupt is accepted; each M1 increments the refresh register.
                let halt_cycles = self.execution_credit_cycles.max(0) as u32;
                let halt_m1_count = halt_cycles / 4;
                self.increment_refresh_counter_by(halt_m1_count);
                self.execution_credit_cycles = 0;
                let elapsed_now = (halt_cycles as i64).min(time_to_advance.max(0));
                if elapsed_now > 0 {
                    bus.audio.step_z80_cycles(elapsed_now as u32);
                    time_to_advance -= elapsed_now;
                }
                break;
            }
            self.io_wait_cycles = 0;
            let opcode_pc = self.pc;
            let opcode = self.fetch_opcode_u8(&mut bus);
            let elapsed = self.exec_opcode(opcode_pc, opcode, &mut bus) as usize
                + self.io_wait_cycles as usize;
            self.execution_credit_cycles -= elapsed as i64;
            let elapsed_now = (elapsed as i64).min(time_to_advance.max(0));
            if elapsed_now > 0 {
                bus.audio.step_z80_cycles(elapsed_now as u32);
                time_to_advance -= elapsed_now;
            }
            if self.interrupt_inhibit_count > 0 {
                self.interrupt_inhibit_count -= 1;
            }
        }
        if time_to_advance > 0 {
            bus.audio.step_z80_cycles(time_to_advance as u32);
        }
        if self.halted && self.execution_credit_cycles > 0 {
            self.execution_credit_cycles = 0;
        }

        let advanced_cycles = granted_cycles as u32;
        let remaining_audio_cycles = audio_granted_cycles.saturating_sub(advanced_cycles);
        if remaining_audio_cycles > 0 {
            bus.audio.step_z80_cycles(remaining_audio_cycles);
        }

        // Account wall-clock Z80 time even if halted or blocked by unsupported opcodes.
        self.cycles += granted_cycles as u64;
    }

    pub fn cycles(&self) -> u64 {
        self.cycles
    }

    pub fn pc(&self) -> u16 {
        self.pc
    }

    pub fn sp(&self) -> u16 {
        self.sp
    }

    pub fn a(&self) -> u8 {
        self.a
    }

    pub fn f(&self) -> u8 {
        self.f
    }

    pub fn bc_reg(&self) -> u16 {
        self.bc()
    }

    pub fn de_reg(&self) -> u16 {
        self.de()
    }

    pub fn hl_reg(&self) -> u16 {
        self.hl()
    }

    pub fn halted(&self) -> bool {
        self.halted
    }

    pub fn unknown_opcode_total(&self) -> u64 {
        self.unknown_opcode_total
    }

    pub fn unknown_opcode_histogram(&self) -> Vec<(u8, u64)> {
        let mut entries: Vec<(u8, u64)> = self
            .unknown_opcode_histogram
            .iter()
            .map(|(opcode, count)| (*opcode, *count))
            .collect();
        entries.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        entries
    }

    pub fn unknown_opcode_pc_histogram(&self) -> Vec<(u16, u64)> {
        let mut entries: Vec<(u16, u64)> = self
            .unknown_opcode_pc_histogram
            .iter()
            .map(|(pc, count)| (*pc, *count))
            .collect();
        entries.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        entries
    }

    pub fn set_im0_interrupt_opcode(&mut self, opcode: u8) {
        self.im0_interrupt_opcode = opcode;
    }

    #[allow(unreachable_patterns)]
    fn exec_opcode(&mut self, opcode_pc: u16, opcode: u8, bus: &mut Z80Bus<'_>) -> u8 {
        match opcode {
            0x00 => 4, // NOP
            0x76 => {
                self.halted = true;
                4
            }
            0xCB => {
                let op2 = self.fetch_opcode_u8(bus);
                self.exec_cb(op2, bus)
            }
            0xED => {
                let op2 = self.fetch_opcode_u8(bus);
                self.exec_ed(opcode_pc, op2, bus)
            }
            0xDD => self.exec_index_prefix(opcode_pc, true, bus),
            0xFD => self.exec_index_prefix(opcode_pc, false, bus),
            0x3E => {
                self.a = self.fetch_u8(bus);
                7
            }
            0x06 => {
                self.b = self.fetch_u8(bus);
                7
            }
            0x0E => {
                self.c = self.fetch_u8(bus);
                7
            }
            0x16 => {
                self.d = self.fetch_u8(bus);
                7
            }
            0x1E => {
                self.e = self.fetch_u8(bus);
                7
            }
            0x0A => {
                self.a = self.read_byte(self.bc(), bus);
                7
            }
            0x1A => {
                self.a = self.read_byte(self.de(), bus);
                7
            }
            0x12 => {
                self.write_byte(self.de(), self.a, bus);
                7
            }
            0x02 => {
                self.write_byte(self.bc(), self.a, bus);
                7
            }
            0x26 => {
                self.h = self.fetch_u8(bus);
                7
            }
            0x2E => {
                self.l = self.fetch_u8(bus);
                7
            }
            0x01 => {
                let value = self.fetch_u16(bus);
                self.set_bc(value);
                10
            }
            0x11 => {
                let value = self.fetch_u16(bus);
                self.set_de(value);
                10
            }
            0x03 => {
                self.set_bc(self.bc().wrapping_add(1));
                6
            }
            0x13 => {
                self.set_de(self.de().wrapping_add(1));
                6
            }
            0x0B => {
                self.set_bc(self.bc().wrapping_sub(1));
                6
            }
            0x1B => {
                self.set_de(self.de().wrapping_sub(1));
                6
            }
            0x21 => {
                let value = self.fetch_u16(bus);
                self.set_hl(value);
                10
            }
            0x31 => {
                self.sp = self.fetch_u16(bus);
                10
            }
            0x33 => {
                self.sp = self.sp.wrapping_add(1);
                6
            }
            0x3B => {
                self.sp = self.sp.wrapping_sub(1);
                6
            }
            0x32 => {
                let addr = self.fetch_u16(bus);
                self.write_byte(addr, self.a, bus);
                13
            }
            0x3A => {
                let addr = self.fetch_u16(bus);
                self.a = self.read_byte(addr, bus);
                13
            }
            0x22 => {
                let addr = self.fetch_u16(bus);
                let [lo, hi] = self.hl().to_le_bytes();
                self.write_byte(addr, lo, bus);
                self.write_byte(addr.wrapping_add(1), hi, bus);
                16
            }
            0x2A => {
                let addr = self.fetch_u16(bus);
                let lo = self.read_byte(addr, bus);
                let hi = self.read_byte(addr.wrapping_add(1), bus);
                self.set_hl(u16::from_le_bytes([lo, hi]));
                16
            }
            0x36 => {
                let value = self.fetch_u8(bus);
                let addr = self.hl();
                self.write_byte(addr, value, bus);
                10
            }
            0x77 => {
                let addr = self.hl();
                self.write_byte(addr, self.a, bus);
                7
            }
            0x7E => {
                let addr = self.hl();
                self.a = self.read_byte(addr, bus);
                7
            }
            0x23 => {
                self.set_hl(self.hl().wrapping_add(1));
                6
            }
            0x2B => {
                self.set_hl(self.hl().wrapping_sub(1));
                6
            }
            0x09 => {
                self.add_hl(self.bc());
                11
            }
            0x19 => {
                self.add_hl(self.de());
                11
            }
            0x29 => {
                self.add_hl(self.hl());
                11
            }
            0x39 => {
                self.add_hl(self.sp);
                11
            }
            0xAF => {
                self.xor_a(self.a);
                4
            }
            0x80..=0x87 => {
                let src = opcode & 0x07;
                let value = self.read_reg_code(src, bus);
                self.add_a(value);
                if src == 0b110 { 7 } else { 4 }
            }
            0x88..=0x8F => {
                let src = opcode & 0x07;
                let value = self.read_reg_code(src, bus);
                self.adc_a(value);
                if src == 0b110 { 7 } else { 4 }
            }
            0x98..=0x9F => {
                let src = opcode & 0x07;
                let value = self.read_reg_code(src, bus);
                self.sbc_a(value);
                if src == 0b110 { 7 } else { 4 }
            }
            0x90..=0x97 => {
                let src = opcode & 0x07;
                let value = self.read_reg_code(src, bus);
                self.sub_a(value);
                if src == 0b110 { 7 } else { 4 }
            }
            0xA0..=0xA7 => {
                let src = opcode & 0x07;
                let value = self.read_reg_code(src, bus);
                self.and_a(value);
                if src == 0b110 { 7 } else { 4 }
            }
            0xA8..=0xAF => {
                let src = opcode & 0x07;
                let value = self.read_reg_code(src, bus);
                self.xor_a(value);
                if src == 0b110 { 7 } else { 4 }
            }
            0xB0..=0xB7 => {
                let src = opcode & 0x07;
                let value = self.read_reg_code(src, bus);
                self.or_a(value);
                if src == 0b110 { 7 } else { 4 }
            }
            0xB8..=0xBF => {
                let src = opcode & 0x07;
                let value = self.read_reg_code(src, bus);
                self.set_compare_flags(value);
                if src == 0b110 { 7 } else { 4 }
            }
            0xD9 => {
                std::mem::swap(&mut self.b, &mut self.b_alt);
                std::mem::swap(&mut self.c, &mut self.c_alt);
                std::mem::swap(&mut self.d, &mut self.d_alt);
                std::mem::swap(&mut self.e, &mut self.e_alt);
                std::mem::swap(&mut self.h, &mut self.h_alt);
                std::mem::swap(&mut self.l, &mut self.l_alt);
                4
            }
            0x08 => {
                std::mem::swap(&mut self.a, &mut self.a_alt);
                std::mem::swap(&mut self.f, &mut self.f_alt);
                4
            }
            0x10 => {
                let disp = self.fetch_u8(bus) as i8;
                self.b = self.b.wrapping_sub(1);
                if self.b != 0 {
                    self.pc = self.pc.wrapping_add_signed(disp as i16);
                    13
                } else {
                    8
                }
            }
            0x1F => {
                let carry_in = if self.flag_c() { 1u8 } else { 0 };
                let carry_out = (self.a & 0x01) != 0;
                self.a = (self.a >> 1) | (carry_in << 7);
                let mut flags = (self.f & (FLAG_S | FLAG_Z | FLAG_PV)) | Self::xy_from_u8(self.a);
                if carry_out {
                    flags |= FLAG_C;
                }
                self.f = flags;
                4
            }
            0x17 => {
                let carry_in = if self.flag_c() { 1u8 } else { 0 };
                let carry_out = (self.a & 0x80) != 0;
                self.a = (self.a << 1) | carry_in;
                let mut flags = (self.f & (FLAG_S | FLAG_Z | FLAG_PV)) | Self::xy_from_u8(self.a);
                if carry_out {
                    flags |= FLAG_C;
                }
                self.f = flags;
                4
            }
            0x07 => {
                let carry_out = (self.a & 0x80) != 0;
                self.a = self.a.rotate_left(1);
                let mut flags = (self.f & (FLAG_S | FLAG_Z | FLAG_PV)) | Self::xy_from_u8(self.a);
                if carry_out {
                    flags |= FLAG_C;
                }
                self.f = flags;
                4
            }
            0x0F => {
                let carry_out = (self.a & 0x01) != 0;
                self.a = self.a.rotate_right(1);
                let mut flags = (self.f & (FLAG_S | FLAG_Z | FLAG_PV)) | Self::xy_from_u8(self.a);
                if carry_out {
                    flags |= FLAG_C;
                }
                self.f = flags;
                4
            }
            0xFE => {
                let value = self.fetch_u8(bus);
                self.set_compare_flags(value);
                7
            }
            0x2F => {
                // CPL sets N/H and preserves S/Z/C.
                self.a ^= 0xFF;
                self.f = (self.f & (FLAG_S | FLAG_Z | FLAG_PV | FLAG_C))
                    | Self::xy_from_u8(self.a)
                    | FLAG_N
                    | FLAG_H;
                4
            }
            0xC6 => {
                let value = self.fetch_u8(bus);
                self.add_a(value);
                7
            }
            0xCE => {
                let value = self.fetch_u8(bus);
                self.adc_a(value);
                7
            }
            0x18 => {
                let disp = self.fetch_u8(bus) as i8;
                self.pc = self.pc.wrapping_add_signed(disp as i16);
                12
            }
            0x20 => {
                let disp = self.fetch_u8(bus) as i8;
                if !self.flag_z() {
                    self.pc = self.pc.wrapping_add_signed(disp as i16);
                    12
                } else {
                    7
                }
            }
            0x38 => {
                let disp = self.fetch_u8(bus) as i8;
                if self.flag_c() {
                    self.pc = self.pc.wrapping_add_signed(disp as i16);
                    12
                } else {
                    7
                }
            }
            0x30 => {
                let disp = self.fetch_u8(bus) as i8;
                if !self.flag_c() {
                    self.pc = self.pc.wrapping_add_signed(disp as i16);
                    12
                } else {
                    7
                }
            }
            0x28 => {
                let disp = self.fetch_u8(bus) as i8;
                if self.flag_z() {
                    self.pc = self.pc.wrapping_add_signed(disp as i16);
                    12
                } else {
                    7
                }
            }
            0xC3 => {
                self.pc = self.fetch_u16(bus);
                10
            }
            0xC2 => {
                let addr = self.fetch_u16(bus);
                if !self.flag_z() {
                    self.pc = addr;
                }
                10
            }
            0xD2 => {
                let addr = self.fetch_u16(bus);
                if !self.flag_c() {
                    self.pc = addr;
                }
                10
            }
            0xE2 => {
                let addr = self.fetch_u16(bus);
                if !self.flag_pv() {
                    self.pc = addr;
                }
                10
            }
            0xCA => {
                let addr = self.fetch_u16(bus);
                if self.flag_z() {
                    self.pc = addr;
                }
                10
            }
            0xEA => {
                let addr = self.fetch_u16(bus);
                if self.flag_pv() {
                    self.pc = addr;
                }
                10
            }
            0xDA => {
                let addr = self.fetch_u16(bus);
                if self.flag_c() {
                    self.pc = addr;
                }
                10
            }
            0xFA => {
                let addr = self.fetch_u16(bus);
                if self.flag_s() {
                    self.pc = addr;
                }
                10
            }
            0xF2 => {
                let addr = self.fetch_u16(bus);
                if !self.flag_s() {
                    self.pc = addr;
                }
                10
            }
            0xCD => {
                let addr = self.fetch_u16(bus);
                self.push_u16(self.pc, bus);
                self.pc = addr;
                17
            }
            0xC4 => {
                let addr = self.fetch_u16(bus);
                if !self.flag_z() {
                    self.push_u16(self.pc, bus);
                    self.pc = addr;
                    17
                } else {
                    10
                }
            }
            0xD4 => {
                let addr = self.fetch_u16(bus);
                if !self.flag_c() {
                    self.push_u16(self.pc, bus);
                    self.pc = addr;
                    17
                } else {
                    10
                }
            }
            0xE4 => {
                let addr = self.fetch_u16(bus);
                if !self.flag_pv() {
                    self.push_u16(self.pc, bus);
                    self.pc = addr;
                    17
                } else {
                    10
                }
            }
            0xDC => {
                let addr = self.fetch_u16(bus);
                if self.flag_c() {
                    self.push_u16(self.pc, bus);
                    self.pc = addr;
                    17
                } else {
                    10
                }
            }
            0xEC => {
                let addr = self.fetch_u16(bus);
                if self.flag_pv() {
                    self.push_u16(self.pc, bus);
                    self.pc = addr;
                    17
                } else {
                    10
                }
            }
            0xCC => {
                let addr = self.fetch_u16(bus);
                if self.flag_z() {
                    self.push_u16(self.pc, bus);
                    self.pc = addr;
                    17
                } else {
                    10
                }
            }
            0xF4 => {
                let addr = self.fetch_u16(bus);
                if !self.flag_s() {
                    self.push_u16(self.pc, bus);
                    self.pc = addr;
                    17
                } else {
                    10
                }
            }
            0xFC => {
                let addr = self.fetch_u16(bus);
                if self.flag_s() {
                    self.push_u16(self.pc, bus);
                    self.pc = addr;
                    17
                } else {
                    10
                }
            }
            0xC0 => {
                if !self.flag_z() {
                    self.pc = self.pop_u16(bus);
                    11
                } else {
                    5
                }
            }
            0xC8 => {
                if self.flag_z() {
                    self.pc = self.pop_u16(bus);
                    11
                } else {
                    5
                }
            }
            0xD0 => {
                if !self.flag_c() {
                    self.pc = self.pop_u16(bus);
                    11
                } else {
                    5
                }
            }
            0xE0 => {
                if !self.flag_pv() {
                    self.pc = self.pop_u16(bus);
                    11
                } else {
                    5
                }
            }
            0xD8 => {
                if self.flag_c() {
                    self.pc = self.pop_u16(bus);
                    11
                } else {
                    5
                }
            }
            0xE8 => {
                if self.flag_pv() {
                    self.pc = self.pop_u16(bus);
                    11
                } else {
                    5
                }
            }
            0xF8 => {
                if self.flag_s() {
                    self.pc = self.pop_u16(bus);
                    11
                } else {
                    5
                }
            }
            0xF0 => {
                if !self.flag_s() {
                    self.pc = self.pop_u16(bus);
                    11
                } else {
                    5
                }
            }
            0xC9 => {
                self.pc = self.pop_u16(bus);
                10
            }
            0xC5 => {
                self.push_u16(self.bc(), bus);
                11
            }
            0xD5 => {
                self.push_u16(self.de(), bus);
                11
            }
            0xE3 => {
                let lo = self.read_byte(self.sp, bus);
                let hi = self.read_byte(self.sp.wrapping_add(1), bus);
                let stack_hl = u16::from_le_bytes([lo, hi]);
                let old_hl = self.hl();
                let [old_lo, old_hi] = old_hl.to_le_bytes();
                self.write_byte(self.sp, old_lo, bus);
                self.write_byte(self.sp.wrapping_add(1), old_hi, bus);
                self.set_hl(stack_hl);
                19
            }
            0xE5 => {
                self.push_u16(self.hl(), bus);
                11
            }
            0xF5 => {
                let af = u16::from_le_bytes([self.f, self.a]);
                self.push_u16(af, bus);
                11
            }
            0xC1 => {
                let value = self.pop_u16(bus);
                self.set_bc(value);
                10
            }
            0xD1 => {
                let value = self.pop_u16(bus);
                self.set_de(value);
                10
            }
            0xE1 => {
                let value = self.pop_u16(bus);
                self.set_hl(value);
                10
            }
            0xF1 => {
                let value = self.pop_u16(bus);
                let [f, a] = value.to_le_bytes();
                self.a = a;
                self.f =
                    f & (FLAG_S | FLAG_Z | FLAG_Y | FLAG_H | FLAG_X | FLAG_PV | FLAG_N | FLAG_C);
                10
            }
            0xE9 => {
                self.pc = self.hl();
                4
            }
            0xF9 => {
                // LD SP,HL
                self.sp = self.hl();
                6
            }
            0xEB => {
                let de = self.de();
                self.set_de(self.hl());
                self.set_hl(de);
                4
            }
            0x27 => {
                let a_before = self.a;
                let old_c = self.flag_c();
                let old_h = self.flag_h();
                let subtract = self.flag_n();
                let mut adjust = 0u8;
                let mut carry_out = old_c;

                if !subtract {
                    if old_h || (self.a & 0x0F) > 0x09 {
                        adjust |= 0x06;
                    }
                    if old_c || self.a > 0x99 {
                        adjust |= 0x60;
                        carry_out = true;
                    }
                    self.a = self.a.wrapping_add(adjust);
                } else {
                    if old_h {
                        adjust |= 0x06;
                    }
                    if old_c {
                        adjust |= 0x60;
                    }
                    self.a = self.a.wrapping_sub(adjust);
                }

                let mut flags = if subtract { FLAG_N } else { 0 };
                if self.a == 0 {
                    flags |= FLAG_Z;
                }
                if (self.a & 0x80) != 0 {
                    flags |= FLAG_S;
                }
                flags |= Self::xy_from_u8(self.a);
                if ((a_before ^ self.a ^ adjust) & 0x10) != 0 {
                    flags |= FLAG_H;
                }
                if Self::parity_even(self.a) {
                    flags |= FLAG_PV;
                }
                if carry_out {
                    flags |= FLAG_C;
                }
                self.f = flags;
                4
            }
            0x37 => {
                // SCF: set carry, clear N/H, preserve S/Z.
                self.f = (self.f & (FLAG_S | FLAG_Z | FLAG_PV)) | Self::xy_from_u8(self.a) | FLAG_C;
                4
            }
            0x3F => {
                // CCF: toggle carry, clear N, set H=old C, preserve S/Z.
                let old_c = self.f & FLAG_C;
                let next_c = if old_c == 0 { FLAG_C } else { 0 };
                self.f = (self.f & (FLAG_S | FLAG_Z | FLAG_PV))
                    | Self::xy_from_u8(self.a)
                    | next_c
                    | if old_c != 0 { FLAG_H } else { 0 };
                4
            }
            0xC7 | 0xCF | 0xD7 | 0xDF | 0xE7 | 0xEF | 0xF7 | 0xFF => {
                self.push_u16(self.pc, bus);
                self.pc = (opcode as u16) & 0x0038;
                11
            }
            0xE6 => {
                let value = self.fetch_u8(bus);
                self.and_a(value);
                7
            }
            0xF6 => {
                let value = self.fetch_u8(bus);
                self.or_a(value);
                7
            }
            0xEE => {
                let value = self.fetch_u8(bus);
                self.xor_a(value);
                7
            }
            0xD6 => {
                let value = self.fetch_u8(bus);
                self.sub_a(value);
                7
            }
            0xDE => {
                let value = self.fetch_u8(bus);
                self.sbc_a(value);
                7
            }
            0xD3 => {
                // OUT (n),A
                let port_low = self.fetch_u8(bus);
                let port = ((self.a as u16) << 8) | port_low as u16;
                self.write_port(port, self.a, bus);
                self.io_wait_cycles = self.io_wait_cycles.saturating_add(audio_io_wait_cycles());
                11
            }
            0xDB => {
                // IN A,(n)
                let port_low = self.fetch_u8(bus);
                let port = ((self.a as u16) << 8) | port_low as u16;
                self.a = self.read_port(port, bus);
                self.io_wait_cycles = self.io_wait_cycles.saturating_add(audio_io_wait_cycles());
                11
            }
            0xF3 => {
                self.iff1 = false;
                self.iff2 = false;
                self.interrupt_inhibit_count = 0;
                4
            }
            0xFB => {
                self.iff1 = true;
                self.iff2 = true;
                // Maskable IRQs are recognized only after the following instruction.
                self.interrupt_inhibit_count = 2;
                4
            }
            0x04 | 0x0C | 0x14 | 0x1C | 0x24 | 0x2C | 0x34 | 0x3C => {
                let reg = (opcode >> 3) & 0x7;
                let value = self.inc8(self.read_reg_code(reg, bus));
                self.write_reg_code(reg, value, bus);
                if reg == 0b110 { 11 } else { 4 }
            }
            0x05 | 0x0D | 0x15 | 0x1D | 0x25 | 0x2D | 0x35 | 0x3D => {
                let reg = (opcode >> 3) & 0x7;
                let value = self.dec8(self.read_reg_code(reg, bus));
                self.write_reg_code(reg, value, bus);
                if reg == 0b110 { 11 } else { 4 }
            }
            0x40..=0x7F => {
                // 0x76 (HALT) is handled above.
                let dst = (opcode >> 3) & 0x7;
                let src = opcode & 0x7;
                let value = self.read_reg_code(src, bus);
                self.write_reg_code(dst, value, bus);
                if dst == 0b110 || src == 0b110 { 7 } else { 4 }
            }
            _ => {
                self.record_unknown(opcode, opcode_pc);
                4
            }
        }
    }

    fn exec_cb(&mut self, opcode: u8, bus: &mut Z80Bus<'_>) -> u8 {
        let x = opcode >> 6;
        let y = (opcode >> 3) & 0x7;
        let z = opcode & 0x7;
        let value = self.read_reg_code(z, bus);
        let (result, write_back, cycles) = self.apply_cb_to_value(x, y, value);
        if x == 1 && z == 0b110 {
            // BIT (HL): undocumented X/Y come from effective address high byte.
            self.f = (self.f & !(FLAG_X | FLAG_Y)) | Self::xy_from_u16_hi(self.hl());
        }
        if write_back {
            self.write_reg_code(z, result, bus);
        }
        if z == 0b110 {
            if x == 0 { 15 } else { 12 }
        } else {
            cycles
        }
    }

    fn exec_ed(&mut self, _opcode_pc: u16, opcode: u8, bus: &mut Z80Bus<'_>) -> u8 {
        match opcode {
            0x40 => {
                // IN B,(C)
                let value = self.read_port(self.bc(), bus);
                self.b = value;
                self.update_szp_preserve_c(value);
                self.io_wait_cycles = self.io_wait_cycles.saturating_add(audio_io_wait_cycles());
                12
            }
            0x41 => {
                // OUT (C),B
                self.write_port(self.bc(), self.b, bus);
                self.io_wait_cycles = self.io_wait_cycles.saturating_add(audio_io_wait_cycles());
                12
            }
            0x44 | 0x4C | 0x54 | 0x5C | 0x64 | 0x6C | 0x74 | 0x7C => {
                // NEG
                let value = self.a;
                let result = 0u8.wrapping_sub(value);
                self.a = result;
                let mut flags = FLAG_N | Self::xy_from_u8(result);
                if result == 0 {
                    flags |= FLAG_Z;
                }
                if (result & 0x80) != 0 {
                    flags |= FLAG_S;
                }
                if value == 0x80 {
                    flags |= FLAG_PV;
                }
                if (value & 0x0F) != 0 {
                    flags |= FLAG_H;
                }
                if value != 0 {
                    flags |= FLAG_C;
                }
                self.f = flags;
                8
            }
            0x48 => {
                // IN C,(C)
                let value = self.read_port(self.bc(), bus);
                self.c = value;
                self.update_szp_preserve_c(value);
                self.io_wait_cycles = self.io_wait_cycles.saturating_add(audio_io_wait_cycles());
                12
            }
            0x49 => {
                // OUT (C),C
                self.write_port(self.bc(), self.c, bus);
                self.io_wait_cycles = self.io_wait_cycles.saturating_add(audio_io_wait_cycles());
                12
            }
            0x42 => {
                self.sbc_hl(self.bc());
                15
            }
            0x4A => {
                self.adc_hl(self.bc());
                15
            }
            0x50 => {
                // IN D,(C)
                let value = self.read_port(self.bc(), bus);
                self.d = value;
                self.update_szp_preserve_c(value);
                self.io_wait_cycles = self.io_wait_cycles.saturating_add(audio_io_wait_cycles());
                12
            }
            0x51 => {
                // OUT (C),D
                self.write_port(self.bc(), self.d, bus);
                self.io_wait_cycles = self.io_wait_cycles.saturating_add(audio_io_wait_cycles());
                12
            }
            0x53 => {
                let addr = self.fetch_u16(bus);
                let [lo, hi] = self.de().to_le_bytes();
                self.write_byte(addr, lo, bus);
                self.write_byte(addr.wrapping_add(1), hi, bus);
                20
            }
            0x52 => {
                self.sbc_hl(self.de());
                15
            }
            0x5A => {
                self.adc_hl(self.de());
                15
            }
            0x58 => {
                // IN E,(C)
                let value = self.read_port(self.bc(), bus);
                self.e = value;
                self.update_szp_preserve_c(value);
                self.io_wait_cycles = self.io_wait_cycles.saturating_add(audio_io_wait_cycles());
                12
            }
            0x59 => {
                // OUT (C),E
                self.write_port(self.bc(), self.e, bus);
                self.io_wait_cycles = self.io_wait_cycles.saturating_add(audio_io_wait_cycles());
                12
            }
            0x62 => {
                let hl = self.hl();
                self.sbc_hl(hl);
                15
            }
            0x6A => {
                let hl = self.hl();
                self.adc_hl(hl);
                15
            }
            0x63 => {
                let addr = self.fetch_u16(bus);
                let [lo, hi] = self.hl().to_le_bytes();
                self.write_byte(addr, lo, bus);
                self.write_byte(addr.wrapping_add(1), hi, bus);
                20
            }
            0x60 => {
                // IN H,(C)
                let value = self.read_port(self.bc(), bus);
                self.h = value;
                self.update_szp_preserve_c(value);
                self.io_wait_cycles = self.io_wait_cycles.saturating_add(audio_io_wait_cycles());
                12
            }
            0x61 => {
                // OUT (C),H
                self.write_port(self.bc(), self.h, bus);
                self.io_wait_cycles = self.io_wait_cycles.saturating_add(audio_io_wait_cycles());
                12
            }
            0x68 => {
                // IN L,(C)
                let value = self.read_port(self.bc(), bus);
                self.l = value;
                self.update_szp_preserve_c(value);
                self.io_wait_cycles = self.io_wait_cycles.saturating_add(audio_io_wait_cycles());
                12
            }
            0x69 => {
                // OUT (C),L
                self.write_port(self.bc(), self.l, bus);
                self.io_wait_cycles = self.io_wait_cycles.saturating_add(audio_io_wait_cycles());
                12
            }
            0x67 => {
                // RRD
                let addr = self.hl();
                let mem = self.read_byte(addr, bus);
                let new_mem = ((self.a & 0x0F) << 4) | (mem >> 4);
                self.a = (self.a & 0xF0) | (mem & 0x0F);
                self.write_byte(addr, new_mem, bus);
                self.update_szp_preserve_c(self.a);
                18
            }
            0x6F => {
                // RLD
                let addr = self.hl();
                let mem = self.read_byte(addr, bus);
                let new_mem = (mem << 4) | (self.a & 0x0F);
                self.a = (self.a & 0xF0) | (mem >> 4);
                self.write_byte(addr, new_mem, bus);
                self.update_szp_preserve_c(self.a);
                18
            }
            0x6B => {
                let addr = self.fetch_u16(bus);
                let lo = self.read_byte(addr, bus);
                let hi = self.read_byte(addr.wrapping_add(1), bus);
                self.set_hl(u16::from_le_bytes([lo, hi]));
                20
            }
            0x72 => {
                self.sbc_hl(self.sp);
                15
            }
            0x7A => {
                self.adc_hl(self.sp);
                15
            }
            0x70 => {
                // IN (C) - updates flags only
                let value = self.read_port(self.bc(), bus);
                self.update_szp_preserve_c(value);
                self.io_wait_cycles = self.io_wait_cycles.saturating_add(audio_io_wait_cycles());
                12
            }
            0x71 => {
                // OUT (C),0 (undocumented NMOS behavior)
                self.write_port(self.bc(), 0x00, bus);
                self.io_wait_cycles = self.io_wait_cycles.saturating_add(audio_io_wait_cycles());
                12
            }
            0x78 => {
                // IN A,(C)
                let value = self.read_port(self.bc(), bus);
                self.a = value;
                self.update_szp_preserve_c(value);
                self.io_wait_cycles = self.io_wait_cycles.saturating_add(audio_io_wait_cycles());
                12
            }
            0x79 => {
                // OUT (C),A
                self.write_port(self.bc(), self.a, bus);
                self.io_wait_cycles = self.io_wait_cycles.saturating_add(audio_io_wait_cycles());
                12
            }
            0x43 => {
                let addr = self.fetch_u16(bus);
                let [lo, hi] = self.bc().to_le_bytes();
                self.write_byte(addr, lo, bus);
                self.write_byte(addr.wrapping_add(1), hi, bus);
                20
            }
            0x4B => {
                let addr = self.fetch_u16(bus);
                let lo = self.read_byte(addr, bus);
                let hi = self.read_byte(addr.wrapping_add(1), bus);
                self.set_bc(u16::from_le_bytes([lo, hi]));
                20
            }
            0x5B => {
                let addr = self.fetch_u16(bus);
                let lo = self.read_byte(addr, bus);
                let hi = self.read_byte(addr.wrapping_add(1), bus);
                self.set_de(u16::from_le_bytes([lo, hi]));
                20
            }
            0x73 => {
                let addr = self.fetch_u16(bus);
                let [lo, hi] = self.sp.to_le_bytes();
                self.write_byte(addr, lo, bus);
                self.write_byte(addr.wrapping_add(1), hi, bus);
                20
            }
            0x7B => {
                let addr = self.fetch_u16(bus);
                let lo = self.read_byte(addr, bus);
                let hi = self.read_byte(addr.wrapping_add(1), bus);
                self.sp = u16::from_le_bytes([lo, hi]);
                20
            }
            0x47 => {
                // LD I,A
                self.i_reg = self.a;
                9
            }
            0x4F => {
                // LD R,A
                self.r_reg = self.a;
                9
            }
            0x57 | 0x5F => {
                // LD A,I / LD A,R
                let carry = self.f & FLAG_C;
                self.a = if opcode == 0x57 {
                    self.i_reg
                } else {
                    self.r_reg
                };
                let mut flags = carry | Self::xy_from_u8(self.a);
                if self.a == 0 {
                    flags |= FLAG_Z;
                }
                if (self.a & 0x80) != 0 {
                    flags |= FLAG_S;
                }
                if self.iff2 {
                    flags |= FLAG_PV;
                }
                self.f = flags;
                9
            }
            0xA0 => {
                // LDI
                let value = self.read_byte(self.hl(), bus);
                self.write_byte(self.de(), value, bus);
                self.set_hl(self.hl().wrapping_add(1));
                self.set_de(self.de().wrapping_add(1));
                self.set_bc(self.bc().wrapping_sub(1));
                self.update_block_transfer_flags(value);
                16
            }
            0xA2 => {
                // INI
                let value = self.read_port(self.bc(), bus);
                self.write_byte(self.hl(), value, bus);
                self.b = self.b.wrapping_sub(1);
                self.set_hl(self.hl().wrapping_add(1));
                self.update_block_in_flags(value, 1);
                self.io_wait_cycles = self.io_wait_cycles.saturating_add(audio_io_wait_cycles());
                16
            }
            0xA3 => {
                // OUTI
                let value = self.read_byte(self.hl(), bus);
                self.write_port(self.bc(), value, bus);
                self.b = self.b.wrapping_sub(1);
                self.set_hl(self.hl().wrapping_add(1));
                self.update_block_out_flags(value, 1);
                self.io_wait_cycles = self.io_wait_cycles.saturating_add(audio_io_wait_cycles());
                16
            }
            0xA8 => {
                // LDD
                let value = self.read_byte(self.hl(), bus);
                self.write_byte(self.de(), value, bus);
                self.set_hl(self.hl().wrapping_sub(1));
                self.set_de(self.de().wrapping_sub(1));
                self.set_bc(self.bc().wrapping_sub(1));
                self.update_block_transfer_flags(value);
                16
            }
            0xAA => {
                // IND
                let value = self.read_port(self.bc(), bus);
                self.write_byte(self.hl(), value, bus);
                self.b = self.b.wrapping_sub(1);
                self.set_hl(self.hl().wrapping_sub(1));
                self.update_block_in_flags(value, -1);
                self.io_wait_cycles = self.io_wait_cycles.saturating_add(audio_io_wait_cycles());
                16
            }
            0xAB => {
                // OUTD
                let value = self.read_byte(self.hl(), bus);
                self.write_port(self.bc(), value, bus);
                self.b = self.b.wrapping_sub(1);
                self.set_hl(self.hl().wrapping_sub(1));
                self.update_block_out_flags(value, -1);
                self.io_wait_cycles = self.io_wait_cycles.saturating_add(audio_io_wait_cycles());
                16
            }
            0xA1 => {
                // CPI
                let value = self.read_byte(self.hl(), bus);
                self.set_hl(self.hl().wrapping_add(1));
                self.set_bc(self.bc().wrapping_sub(1));
                self.update_block_compare_flags(value);
                16
            }
            0xA9 => {
                // CPD
                let value = self.read_byte(self.hl(), bus);
                self.set_hl(self.hl().wrapping_sub(1));
                self.set_bc(self.bc().wrapping_sub(1));
                self.update_block_compare_flags(value);
                16
            }
            0x45 | 0x4D | 0x55 | 0x5D | 0x65 | 0x6D | 0x75 | 0x7D => {
                self.pc = self.pop_u16(bus);
                self.iff1 = self.iff2;
                14
            }
            0xB0 => {
                let value = self.read_byte(self.hl(), bus);
                self.write_byte(self.de(), value, bus);
                self.set_hl(self.hl().wrapping_add(1));
                self.set_de(self.de().wrapping_add(1));
                self.set_bc(self.bc().wrapping_sub(1));
                self.update_block_transfer_flags(value);
                if self.bc() != 0 {
                    self.pc = self.pc.wrapping_sub(2);
                    21
                } else {
                    16
                }
            }
            0xB2 => {
                // INIR
                let value = self.read_port(self.bc(), bus);
                self.write_byte(self.hl(), value, bus);
                self.b = self.b.wrapping_sub(1);
                self.set_hl(self.hl().wrapping_add(1));
                self.update_block_in_flags(value, 1);
                self.io_wait_cycles = self.io_wait_cycles.saturating_add(audio_io_wait_cycles());
                if self.b != 0 {
                    self.pc = self.pc.wrapping_sub(2);
                    21
                } else {
                    16
                }
            }
            0xB3 => {
                // OTIR
                let value = self.read_byte(self.hl(), bus);
                self.write_port(self.bc(), value, bus);
                self.b = self.b.wrapping_sub(1);
                self.set_hl(self.hl().wrapping_add(1));
                self.update_block_out_flags(value, 1);
                self.io_wait_cycles = self.io_wait_cycles.saturating_add(audio_io_wait_cycles());
                if self.b != 0 {
                    self.pc = self.pc.wrapping_sub(2);
                    21
                } else {
                    16
                }
            }
            0xB8 => {
                // LDDR
                let value = self.read_byte(self.hl(), bus);
                self.write_byte(self.de(), value, bus);
                self.set_hl(self.hl().wrapping_sub(1));
                self.set_de(self.de().wrapping_sub(1));
                self.set_bc(self.bc().wrapping_sub(1));
                self.update_block_transfer_flags(value);
                if self.bc() != 0 {
                    self.pc = self.pc.wrapping_sub(2);
                    21
                } else {
                    16
                }
            }
            0xBA => {
                // INDR
                let value = self.read_port(self.bc(), bus);
                self.write_byte(self.hl(), value, bus);
                self.b = self.b.wrapping_sub(1);
                self.set_hl(self.hl().wrapping_sub(1));
                self.update_block_in_flags(value, -1);
                self.io_wait_cycles = self.io_wait_cycles.saturating_add(audio_io_wait_cycles());
                if self.b != 0 {
                    self.pc = self.pc.wrapping_sub(2);
                    21
                } else {
                    16
                }
            }
            0xBB => {
                // OTDR
                let value = self.read_byte(self.hl(), bus);
                self.write_port(self.bc(), value, bus);
                self.b = self.b.wrapping_sub(1);
                self.set_hl(self.hl().wrapping_sub(1));
                self.update_block_out_flags(value, -1);
                self.io_wait_cycles = self.io_wait_cycles.saturating_add(audio_io_wait_cycles());
                if self.b != 0 {
                    self.pc = self.pc.wrapping_sub(2);
                    21
                } else {
                    16
                }
            }
            0xB1 => {
                // CPIR
                let value = self.read_byte(self.hl(), bus);
                self.set_hl(self.hl().wrapping_add(1));
                self.set_bc(self.bc().wrapping_sub(1));
                self.update_block_compare_flags(value);
                if self.bc() != 0 && !self.flag_z() {
                    self.pc = self.pc.wrapping_sub(2);
                    21
                } else {
                    16
                }
            }
            0xB9 => {
                // CPDR
                let value = self.read_byte(self.hl(), bus);
                self.set_hl(self.hl().wrapping_sub(1));
                self.set_bc(self.bc().wrapping_sub(1));
                self.update_block_compare_flags(value);
                if self.bc() != 0 && !self.flag_z() {
                    self.pc = self.pc.wrapping_sub(2);
                    21
                } else {
                    16
                }
            }
            0x46 | 0x4E | 0x66 | 0x6E => {
                // IM 0
                self.interrupt_mode = 0;
                8
            }
            0x56 | 0x76 => {
                // IM 1
                self.interrupt_mode = 1;
                8
            }
            0x5E | 0x7E => {
                // IM 2
                self.interrupt_mode = 2;
                8
            }
            _ => {
                // On Z80, undefined ED-prefixed opcodes behave like 2-byte NOPs.
                8
            }
        }
    }

    fn exec_index_prefix(&mut self, _opcode_pc: u16, use_ix: bool, bus: &mut Z80Bus<'_>) -> u8 {
        let op2 = self.fetch_opcode_u8(bus);
        match op2 {
            0x09 => {
                self.add_index(self.bc(), use_ix);
                15
            }
            0x19 => {
                self.add_index(self.de(), use_ix);
                15
            }
            0x29 => {
                let idx = self.index_reg(use_ix);
                self.add_index(idx, use_ix);
                15
            }
            0x39 => {
                self.add_index(self.sp, use_ix);
                15
            }
            0x21 => {
                let value = self.fetch_u16(bus);
                self.set_index_reg(use_ix, value);
                14
            }
            0x22 => {
                let addr = self.fetch_u16(bus);
                let value = self.index_reg(use_ix);
                let [lo, hi] = value.to_le_bytes();
                self.write_byte(addr, lo, bus);
                self.write_byte(addr.wrapping_add(1), hi, bus);
                20
            }
            0x2A => {
                let addr = self.fetch_u16(bus);
                let lo = self.read_byte(addr, bus);
                let hi = self.read_byte(addr.wrapping_add(1), bus);
                self.set_index_reg(use_ix, u16::from_le_bytes([lo, hi]));
                20
            }
            0x23 => {
                self.set_index_reg(use_ix, self.index_reg(use_ix).wrapping_add(1));
                10
            }
            0x2B => {
                self.set_index_reg(use_ix, self.index_reg(use_ix).wrapping_sub(1));
                10
            }
            0x24 => {
                let value = self.inc8(self.index_reg_hi(use_ix));
                self.set_index_reg_hi(use_ix, value);
                8
            }
            0x25 => {
                let value = self.dec8(self.index_reg_hi(use_ix));
                self.set_index_reg_hi(use_ix, value);
                8
            }
            0x26 => {
                let value = self.fetch_u8(bus);
                self.set_index_reg_hi(use_ix, value);
                11
            }
            0x2C => {
                let value = self.inc8(self.index_reg_lo(use_ix));
                self.set_index_reg_lo(use_ix, value);
                8
            }
            0x2D => {
                let value = self.dec8(self.index_reg_lo(use_ix));
                self.set_index_reg_lo(use_ix, value);
                8
            }
            0x2E => {
                let value = self.fetch_u8(bus);
                self.set_index_reg_lo(use_ix, value);
                11
            }
            0x34 => {
                let disp = self.fetch_u8(bus) as i8;
                let addr = self.indexed_addr(use_ix, disp);
                let value = self.inc8(self.read_byte(addr, bus));
                self.write_byte(addr, value, bus);
                23
            }
            0x35 => {
                let disp = self.fetch_u8(bus) as i8;
                let addr = self.indexed_addr(use_ix, disp);
                let value = self.dec8(self.read_byte(addr, bus));
                self.write_byte(addr, value, bus);
                23
            }
            0x36 => {
                let disp = self.fetch_u8(bus) as i8;
                let value = self.fetch_u8(bus);
                let addr = self.indexed_addr(use_ix, disp);
                self.write_byte(addr, value, bus);
                19
            }
            0x40..=0x7F => {
                if op2 == 0x76 {
                    self.halted = true;
                    return 8;
                }
                let dst = (op2 >> 3) & 0x7;
                let src = op2 & 0x7;
                if dst == 0b110 || src == 0b110 {
                    let disp = self.fetch_u8(bus) as i8;
                    let addr = self.indexed_addr(use_ix, disp);
                    if src == 0b110 {
                        let value = self.read_byte(addr, bus);
                        // DD/FD + LD r,(HL): prefix applies to (HL)->(IX/IY+d),
                        // but register r remains the ordinary register set.
                        self.write_reg_code_no_mem(dst, value);
                    } else {
                        // DD/FD + LD (HL),r: prefix applies to destination memory
                        // address only; source r remains ordinary register set.
                        let value = self.read_reg_code_no_mem(src);
                        self.write_byte(addr, value, bus);
                    }
                    19
                } else {
                    let value = self.read_index_reg_code_no_mem(use_ix, src);
                    self.write_index_reg_code_no_mem(use_ix, dst, value);
                    8
                }
            }
            0x80..=0xBF => {
                let src = op2 & 0x7;
                let (value, cycles) = if src == 0b110 {
                    let disp = self.fetch_u8(bus) as i8;
                    let addr = self.indexed_addr(use_ix, disp);
                    (self.read_byte(addr, bus), 19)
                } else {
                    (self.read_index_reg_code_no_mem(use_ix, src), 8)
                };
                match op2 & 0xF8 {
                    0x80 => self.add_a(value),
                    0x88 => self.adc_a(value),
                    0x90 => self.sub_a(value),
                    0x98 => self.sbc_a(value),
                    0xA0 => {
                        self.and_a(value);
                    }
                    0xA8 => {
                        self.xor_a(value);
                    }
                    0xB0 => {
                        self.or_a(value);
                    }
                    0xB8 => self.set_compare_flags(value),
                    _ => unreachable!(),
                }
                cycles
            }
            0xE5 => {
                self.push_u16(self.index_reg(use_ix), bus);
                15
            }
            0xE1 => {
                let value = self.pop_u16(bus);
                self.set_index_reg(use_ix, value);
                14
            }
            0xF9 => {
                self.sp = self.index_reg(use_ix);
                10
            }
            0xE3 => {
                let lo = self.read_byte(self.sp, bus);
                let hi = self.read_byte(self.sp.wrapping_add(1), bus);
                let stack_value = u16::from_le_bytes([lo, hi]);
                let idx = self.index_reg(use_ix);
                let [idx_lo, idx_hi] = idx.to_le_bytes();
                self.write_byte(self.sp, idx_lo, bus);
                self.write_byte(self.sp.wrapping_add(1), idx_hi, bus);
                self.set_index_reg(use_ix, stack_value);
                23
            }
            0xE9 => {
                self.pc = self.index_reg(use_ix);
                8
            }
            0xCB => {
                let disp = self.fetch_u8(bus) as i8;
                let op3 = self.fetch_opcode_u8(bus);
                self.exec_index_cb(use_ix, disp, op3, bus)
            }
            _ => {
                // DD/FD prefixes only modify instructions that reference HL/H/L
                // or have dedicated IX/IY encodings. For other opcodes the prefix
                // is ignored and the following opcode executes normally.
                let op_pc = self.pc.wrapping_sub(1);
                4 + self.exec_opcode(op_pc, op2, bus)
            }
        }
    }

    fn exec_index_cb(&mut self, use_ix: bool, disp: i8, opcode: u8, bus: &mut Z80Bus<'_>) -> u8 {
        let x = opcode >> 6;
        let y = (opcode >> 3) & 0x7;
        let z = opcode & 0x7;
        let addr = self.indexed_addr(use_ix, disp);
        let value = self.read_byte(addr, bus);
        let (result, write_back, _cycles) = self.apply_cb_to_value(x, y, value);
        if x == 1 {
            // BIT (IX/IY+d): undocumented X/Y come from effective address high byte.
            self.f = (self.f & !(FLAG_X | FLAG_Y)) | Self::xy_from_u16_hi(addr);
        }
        if write_back {
            self.write_byte(addr, result, bus);
            if z != 0b110 {
                self.write_reg_code_no_mem(z, result);
            }
        }
        if x == 1 { 20 } else { 23 }
    }

    fn apply_cb_to_value(&mut self, x: u8, y: u8, value: u8) -> (u8, bool, u8) {
        match x {
            0 => {
                let (result, carry) = match y {
                    0 => (value.rotate_left(1), (value & 0x80) != 0), // RLC
                    1 => (value.rotate_right(1), (value & 0x01) != 0), // RRC
                    2 => {
                        let c = (self.f & FLAG_C) != 0;
                        let result = (value << 1) | (c as u8);
                        (result, (value & 0x80) != 0) // RL
                    }
                    3 => {
                        let c = (self.f & FLAG_C) != 0;
                        let result = (value >> 1) | ((c as u8) << 7);
                        (result, (value & 0x01) != 0) // RR
                    }
                    4 => (value << 1, (value & 0x80) != 0), // SLA
                    5 => ((value >> 1) | (value & 0x80), (value & 0x01) != 0), // SRA
                    6 => ((value << 1) | 1, (value & 0x80) != 0), // SLL (undoc)
                    7 => (value >> 1, (value & 0x01) != 0), // SRL
                    _ => (value, false),
                };
                let mut flags = 0;
                if result == 0 {
                    flags |= FLAG_Z;
                }
                if (result & 0x80) != 0 {
                    flags |= FLAG_S;
                }
                flags |= Self::xy_from_u8(result);
                if Self::parity_even(result) {
                    flags |= FLAG_PV;
                }
                if carry {
                    flags |= FLAG_C;
                }
                self.f = flags;
                (result, true, 8)
            }
            1 => {
                // BIT y,value
                let bit_set = (value & (1 << y)) != 0;
                let carry = self.f & FLAG_C;
                let mut flags = carry | FLAG_H | Self::xy_from_u8(value);
                if !bit_set {
                    flags |= FLAG_Z | FLAG_PV;
                }
                if y == 7 && bit_set {
                    flags |= FLAG_S;
                }
                self.f = flags;
                (value, false, 8)
            }
            2 => (value & !(1 << y), true, 8), // RES
            3 => (value | (1 << y), true, 8),  // SET
            _ => (value, false, 8),
        }
    }

    fn read_reg_code(&self, code: u8, bus: &mut Z80Bus<'_>) -> u8 {
        match code & 0x7 {
            0b000 => self.b,
            0b001 => self.c,
            0b010 => self.d,
            0b011 => self.e,
            0b100 => self.h,
            0b101 => self.l,
            0b110 => self.read_byte(self.hl(), bus),
            0b111 => self.a,
            _ => 0,
        }
    }

    fn read_reg_code_no_mem(&self, code: u8) -> u8 {
        match code & 0x7 {
            0b000 => self.b,
            0b001 => self.c,
            0b010 => self.d,
            0b011 => self.e,
            0b100 => self.h,
            0b101 => self.l,
            0b111 => self.a,
            _ => 0,
        }
    }

    fn write_reg_code(&mut self, code: u8, value: u8, bus: &mut Z80Bus<'_>) {
        match code & 0x7 {
            0b000 => self.b = value,
            0b001 => self.c = value,
            0b010 => self.d = value,
            0b011 => self.e = value,
            0b100 => self.h = value,
            0b101 => self.l = value,
            0b110 => {
                let addr = self.hl();
                self.write_byte(addr, value, bus);
            }
            0b111 => self.a = value,
            _ => {}
        }
    }

    fn write_reg_code_no_mem(&mut self, code: u8, value: u8) {
        match code & 0x7 {
            0b000 => self.b = value,
            0b001 => self.c = value,
            0b010 => self.d = value,
            0b011 => self.e = value,
            0b100 => self.h = value,
            0b101 => self.l = value,
            0b111 => self.a = value,
            _ => {}
        }
    }

    fn fetch_opcode_u8(&mut self, bus: &mut Z80Bus<'_>) -> u8 {
        let opcode = self.fetch_u8(bus);
        self.increment_refresh_counter();
        opcode
    }

    fn increment_refresh_counter(&mut self) {
        let next_low7 = self.r_reg.wrapping_add(1) & 0x7F;
        self.r_reg = (self.r_reg & 0x80) | next_low7;
    }

    fn increment_refresh_counter_by(&mut self, count: u32) {
        let count_mod_128 = (count & 0x7F) as u8;
        let next_low7 = (self.r_reg & 0x7F).wrapping_add(count_mod_128) & 0x7F;
        self.r_reg = (self.r_reg & 0x80) | next_low7;
    }

    fn fetch_u8(&mut self, bus: &mut Z80Bus<'_>) -> u8 {
        let value = self.read_byte(self.pc, bus);
        self.pc = self.pc.wrapping_add(1);
        value
    }

    fn fetch_u16(&mut self, bus: &mut Z80Bus<'_>) -> u16 {
        let lo = self.fetch_u8(bus);
        let hi = self.fetch_u8(bus);
        u16::from_le_bytes([lo, hi])
    }

    fn push_u16(&mut self, value: u16, bus: &mut Z80Bus<'_>) {
        let [lo, hi] = value.to_le_bytes();
        self.sp = self.sp.wrapping_sub(1);
        self.write_byte(self.sp, hi, bus);
        self.sp = self.sp.wrapping_sub(1);
        self.write_byte(self.sp, lo, bus);
    }

    fn pop_u16(&mut self, bus: &mut Z80Bus<'_>) -> u16 {
        let lo = self.read_byte(self.sp, bus);
        self.sp = self.sp.wrapping_add(1);
        let hi = self.read_byte(self.sp, bus);
        self.sp = self.sp.wrapping_add(1);
        u16::from_le_bytes([lo, hi])
    }

    fn read_byte(&self, addr: u16, bus: &mut Z80Bus<'_>) -> u8 {
        match addr {
            0x0000..=0x3FFF => self.ram[(addr as usize) & 0x1FFF],
            0x4000..=0x5FFF => bus.audio.read_ym2612((addr & 0x03) as u8),
            0x8000..=0xFFFF => self.read_68k_window(addr, bus),
            _ => 0xFF,
        }
    }

    fn write_byte(&mut self, addr: u16, value: u8, bus: &mut Z80Bus<'_>) {
        match addr {
            0x0000..=0x3FFF => {
                self.ram[(addr as usize) & 0x1FFF] = value;
            }
            0x4000..=0x5FFF => {
                bus.audio.write_ym2612_from_z80((addr & 0x03) as u8, value);
                self.io_wait_cycles = self.io_wait_cycles.saturating_add(audio_io_wait_cycles());
            }
            0x6000..=0x60FF => self.write_bank_register(value),
            0x7F11 => {
                bus.audio.write_psg_from_z80(value);
                self.io_wait_cycles = self.io_wait_cycles.saturating_add(audio_io_wait_cycles());
            }
            0x8000..=0xFFFF => self.write_68k_window(addr, value, bus),
            _ => {}
        }
    }

    fn read_port(&self, port: u16, _bus: &mut Z80Bus<'_>) -> u8 {
        match port as u8 {
            // YM2612 status/data ports (low-byte decode).
            0x40..=0x43 => _bus.audio.read_ym2612((port as u8) & 0x03),
            // External I/O ports are sparsely used on Mega Drive Z80 side.
            // Return open-bus style value for currently unmodeled inputs.
            _ => 0xFF,
        }
    }

    fn write_port(&mut self, port: u16, value: u8, bus: &mut Z80Bus<'_>) {
        match port as u8 {
            // YM2612 address/data ports (low-byte decode).
            0x40..=0x43 => {
                bus.audio.write_ym2612_from_z80((port as u8) & 0x03, value);
                self.io_wait_cycles = self.io_wait_cycles.saturating_add(audio_io_wait_cycles());
            }
            // PSG data port
            0x7F => {
                bus.audio.write_psg_from_z80(value);
                self.io_wait_cycles = self.io_wait_cycles.saturating_add(audio_io_wait_cycles());
            }
            _ => {}
        }
    }

    fn write_bank_register(&mut self, value: u8) {
        // Genesis Z80 bank register is a serial latch fed by bit0 writes.
        self.bank_address = (self.bank_address >> 1) | (((value as u32) & 1) << 23);
        self.bank_address &= 0x00FF_8000;
    }

    fn resolve_68k_window_addr(&self, z80_addr: u16) -> u32 {
        let offset = (z80_addr as u32).wrapping_sub(0x8000) & 0x7FFF;
        (self.bank_address & 0x00FF_8000) | offset
    }

    fn decode_68k_vdp_local_addr(addr: u32) -> Option<u32> {
        if (0xC00000..=0xDFFFFF).contains(&addr) {
            Some(0xC00000 | (addr & 0x1F))
        } else {
            None
        }
    }

    fn is_68k_psg_addr(addr: u32) -> bool {
        let Some(local) = Self::decode_68k_vdp_local_addr(addr) else {
            return false;
        };
        matches!(local, 0xC00011 | 0xC00013 | 0xC00015 | 0xC00017)
    }

    fn read_68k_window(&self, z80_addr: u16, bus: &mut Z80Bus<'_>) -> u8 {
        let addr = self.resolve_68k_window_addr(z80_addr);
        match addr {
            0x000000..=0x3FFFFF => bus.cartridge.read_u8(addr),
            0xA04000..=0xA04003 => bus.audio.read_ym2612((addr - 0xA04000) as u8),
            0xC00000..=0xDFFFFF => Self::read_vdp_port_byte(addr, bus),
            x if x == IO_VERSION_ADDR || x == IO_VERSION_ADDR + 1 => bus.io.read_version(),
            x if x == IO_PORT1_DATA_ADDR || x == IO_PORT1_DATA_ADDR + 1 => bus.io.read_port1_data(),
            x if x == IO_PORT2_DATA_ADDR || x == IO_PORT2_DATA_ADDR + 1 => bus.io.read_port2_data(),
            x if x == IO_PORT1_CTRL_ADDR || x == IO_PORT1_CTRL_ADDR + 1 => bus.io.read_port1_ctrl(),
            x if x == IO_PORT2_CTRL_ADDR || x == IO_PORT2_CTRL_ADDR + 1 => bus.io.read_port2_ctrl(),
            0xFF0000..=0xFFFFFF => bus.work_ram[(addr - 0xFF0000) as usize],
            _ => 0xFF,
        }
    }

    fn read_vdp_port_byte(addr: u32, bus: &mut Z80Bus<'_>) -> u8 {
        let Some(local) = Self::decode_68k_vdp_local_addr(addr) else {
            return 0xFF;
        };
        let aligned = local & !1;
        let word = match aligned {
            0xC00000 | 0xC00002 => bus.vdp.read_data_port(),
            0xC00004 | 0xC00006 => bus.vdp.read_control_port(),
            0xC00008 | 0xC0000A => bus.vdp.read_hv_counter(),
            _ => return 0xFF,
        };
        if (local & 1) == 0 {
            (word >> 8) as u8
        } else {
            word as u8
        }
    }

    fn write_68k_window(&mut self, z80_addr: u16, value: u8, bus: &mut Z80Bus<'_>) {
        let addr = self.resolve_68k_window_addr(z80_addr);
        match addr {
            0xA04000..=0xA04003 => {
                bus.audio
                    .write_ym2612_from_z80((addr - 0xA04000) as u8, value);
                self.io_wait_cycles = self.io_wait_cycles.saturating_add(audio_io_wait_cycles());
            }
            x if x == IO_PORT1_DATA_ADDR || x == IO_PORT1_DATA_ADDR + 1 => {
                bus.io.write_port1_data(value)
            }
            x if x == IO_PORT2_DATA_ADDR || x == IO_PORT2_DATA_ADDR + 1 => {
                bus.io.write_port2_data(value)
            }
            x if x == IO_PORT1_CTRL_ADDR || x == IO_PORT1_CTRL_ADDR + 1 => {
                bus.io.write_port1_ctrl(value)
            }
            x if x == IO_PORT2_CTRL_ADDR || x == IO_PORT2_CTRL_ADDR + 1 => {
                bus.io.write_port2_ctrl(value)
            }
            x if Self::is_68k_psg_addr(x) => {
                bus.audio.write_psg_from_z80(value);
                self.io_wait_cycles = self.io_wait_cycles.saturating_add(audio_io_wait_cycles());
            }
            0xC00000..=0xDFFFFF => self.write_vdp_port_byte(addr, value, bus),
            0xFF0000..=0xFFFFFF => {
                bus.work_ram[(addr - 0xFF0000) as usize] = value;
            }
            _ => {}
        }
    }

    fn write_vdp_port_byte(&mut self, addr: u32, value: u8, bus: &mut Z80Bus<'_>) {
        let Some(local) = Self::decode_68k_vdp_local_addr(addr) else {
            return;
        };
        let aligned = local & !1;
        let immediate_byte_commit =
            std::env::var_os("MEGADRIVE_DEBUG_VDP_BYTE_IMMEDIATE").is_some();
        let low_byte_write = (local & 1) != 0;
        let next = match aligned {
            0xC00000 | 0xC00002 => {
                let current = self.vdp_data_write_latch;
                let next = if (local & 1) == 0 {
                    ((value as u16) << 8) | (current & 0x00FF)
                } else {
                    (current & 0xFF00) | value as u16
                };
                self.vdp_data_write_latch = next;
                next
            }
            0xC00004 | 0xC00006 => {
                let current = self.vdp_control_write_latch;
                let next = if (local & 1) == 0 {
                    ((value as u16) << 8) | (current & 0x00FF)
                } else {
                    (current & 0xFF00) | value as u16
                };
                self.vdp_control_write_latch = next;
                next
            }
            _ => return,
        };
        match aligned {
            0xC00000 | 0xC00002 => {
                if immediate_byte_commit || low_byte_write {
                    bus.vdp.write_data_port(next);
                }
            }
            0xC00004 | 0xC00006 => {
                if immediate_byte_commit || low_byte_write {
                    bus.vdp.write_control_port(next);
                    self.process_pending_vdp_bus_dma(bus);
                }
            }
            _ => {}
        }
    }

    fn process_pending_vdp_bus_dma(&mut self, bus: &mut Z80Bus<'_>) {
        while let Some(request) = bus.vdp.take_bus_dma_request() {
            let mut next_source_addr = request.source_addr & 0x00FF_FFFE;
            for _ in 0..request.words {
                let hi = self.read_dma_source_u8(next_source_addr, bus);
                let lo = self.read_dma_source_u8(next_source_addr.wrapping_add(1), bus);
                bus.vdp.write_data_port(u16::from_be_bytes([hi, lo]));
                next_source_addr = next_source_addr.wrapping_add(2);
            }
            bus.vdp.complete_bus_dma(next_source_addr & 0x00FF_FFFE);

            let dma_wait_cycles = (request.words as u32).saturating_mul(2);
            self.io_wait_cycles = self
                .io_wait_cycles
                .saturating_add(dma_wait_cycles.min(u16::MAX as u32) as u16);
        }
    }

    fn read_dma_source_u8(&self, addr: u32, bus: &mut Z80Bus<'_>) -> u8 {
        let addr = addr & 0x00FF_FFFF;
        match addr {
            0x000000..=0x3FFFFF => bus.cartridge.read_u8(addr),
            0xA00000..=0xA01FFF => self.ram[(addr as usize - 0xA00000) & 0x1FFF],
            0xA04000..=0xA04003 => bus.audio.read_ym2612((addr - 0xA04000) as u8),
            0xC00000..=0xDFFFFF => Self::read_vdp_port_byte(addr, bus),
            x if x == IO_VERSION_ADDR || x == IO_VERSION_ADDR + 1 => bus.io.read_version(),
            x if x == IO_PORT1_DATA_ADDR || x == IO_PORT1_DATA_ADDR + 1 => bus.io.read_port1_data(),
            x if x == IO_PORT2_DATA_ADDR || x == IO_PORT2_DATA_ADDR + 1 => bus.io.read_port2_data(),
            x if x == IO_PORT1_CTRL_ADDR || x == IO_PORT1_CTRL_ADDR + 1 => bus.io.read_port1_ctrl(),
            x if x == IO_PORT2_CTRL_ADDR || x == IO_PORT2_CTRL_ADDR + 1 => bus.io.read_port2_ctrl(),
            0xFF0000..=0xFFFFFF => bus.work_ram[(addr - 0xFF0000) as usize],
            _ => 0xFF,
        }
    }

    fn hl(&self) -> u16 {
        ((self.h as u16) << 8) | self.l as u16
    }

    fn bc(&self) -> u16 {
        ((self.b as u16) << 8) | self.c as u16
    }

    fn de(&self) -> u16 {
        ((self.d as u16) << 8) | self.e as u16
    }

    fn index_reg(&self, use_ix: bool) -> u16 {
        if use_ix { self.ix } else { self.iy }
    }

    fn set_index_reg(&mut self, use_ix: bool, value: u16) {
        if use_ix {
            self.ix = value;
        } else {
            self.iy = value;
        }
    }

    fn index_reg_hi(&self, use_ix: bool) -> u8 {
        (self.index_reg(use_ix) >> 8) as u8
    }

    fn index_reg_lo(&self, use_ix: bool) -> u8 {
        self.index_reg(use_ix) as u8
    }

    fn set_index_reg_hi(&mut self, use_ix: bool, value: u8) {
        let next = ((value as u16) << 8) | (self.index_reg(use_ix) & 0x00FF);
        self.set_index_reg(use_ix, next);
    }

    fn set_index_reg_lo(&mut self, use_ix: bool, value: u8) {
        let next = (self.index_reg(use_ix) & 0xFF00) | value as u16;
        self.set_index_reg(use_ix, next);
    }

    fn read_index_reg_code_no_mem(&self, use_ix: bool, code: u8) -> u8 {
        match code & 0x7 {
            0b000 => self.b,
            0b001 => self.c,
            0b010 => self.d,
            0b011 => self.e,
            0b100 => self.index_reg_hi(use_ix),
            0b101 => self.index_reg_lo(use_ix),
            0b111 => self.a,
            _ => 0,
        }
    }

    fn write_index_reg_code_no_mem(&mut self, use_ix: bool, code: u8, value: u8) {
        match code & 0x7 {
            0b000 => self.b = value,
            0b001 => self.c = value,
            0b010 => self.d = value,
            0b011 => self.e = value,
            0b100 => self.set_index_reg_hi(use_ix, value),
            0b101 => self.set_index_reg_lo(use_ix, value),
            0b111 => self.a = value,
            _ => {}
        }
    }

    fn indexed_addr(&self, use_ix: bool, disp: i8) -> u16 {
        self.index_reg(use_ix).wrapping_add_signed(disp as i16)
    }

    fn set_hl(&mut self, value: u16) {
        self.h = (value >> 8) as u8;
        self.l = value as u8;
    }

    fn set_bc(&mut self, value: u16) {
        self.b = (value >> 8) as u8;
        self.c = value as u8;
    }

    fn set_de(&mut self, value: u16) {
        self.d = (value >> 8) as u8;
        self.e = value as u8;
    }

    fn flag_z(&self) -> bool {
        (self.f & FLAG_Z) != 0
    }

    fn flag_c(&self) -> bool {
        (self.f & FLAG_C) != 0
    }

    fn flag_pv(&self) -> bool {
        (self.f & FLAG_PV) != 0
    }

    fn flag_h(&self) -> bool {
        (self.f & FLAG_H) != 0
    }

    fn flag_n(&self) -> bool {
        (self.f & FLAG_N) != 0
    }

    fn flag_s(&self) -> bool {
        (self.f & FLAG_S) != 0
    }

    fn parity_even(value: u8) -> bool {
        (value.count_ones() & 1) == 0
    }

    fn xy_from_u8(value: u8) -> u8 {
        value & (FLAG_X | FLAG_Y)
    }

    fn xy_from_u16_hi(value: u16) -> u8 {
        ((value >> 8) as u8) & (FLAG_X | FLAG_Y)
    }

    fn update_szp_preserve_c(&mut self, value: u8) {
        let carry = self.f & FLAG_C;
        let mut next = carry | Self::xy_from_u8(value);
        if value == 0 {
            next |= FLAG_Z;
        }
        if (value & 0x80) != 0 {
            next |= FLAG_S;
        }
        if Self::parity_even(value) {
            next |= FLAG_PV;
        }
        self.f = next;
    }

    fn inc8(&mut self, value: u8) -> u8 {
        let result = value.wrapping_add(1);
        let carry = self.f & FLAG_C;
        let mut flags = carry | Self::xy_from_u8(result);
        if result == 0 {
            flags |= FLAG_Z;
        }
        if (result & 0x80) != 0 {
            flags |= FLAG_S;
        }
        if (value & 0x0F) == 0x0F {
            flags |= FLAG_H;
        }
        if value == 0x7F {
            flags |= FLAG_PV;
        }
        self.f = flags;
        result
    }

    fn dec8(&mut self, value: u8) -> u8 {
        let result = value.wrapping_sub(1);
        let carry = self.f & FLAG_C;
        let mut flags = carry | FLAG_N | Self::xy_from_u8(result);
        if result == 0 {
            flags |= FLAG_Z;
        }
        if (result & 0x80) != 0 {
            flags |= FLAG_S;
        }
        if (value & 0x0F) == 0x00 {
            flags |= FLAG_H;
        }
        if value == 0x80 {
            flags |= FLAG_PV;
        }
        self.f = flags;
        result
    }

    fn and_a(&mut self, value: u8) {
        self.a &= value;
        let mut flags = FLAG_H | Self::xy_from_u8(self.a);
        if self.a == 0 {
            flags |= FLAG_Z;
        }
        if (self.a & 0x80) != 0 {
            flags |= FLAG_S;
        }
        if Self::parity_even(self.a) {
            flags |= FLAG_PV;
        }
        self.f = flags;
    }

    fn xor_a(&mut self, value: u8) {
        self.a ^= value;
        let mut flags = Self::xy_from_u8(self.a);
        if self.a == 0 {
            flags |= FLAG_Z;
        }
        if (self.a & 0x80) != 0 {
            flags |= FLAG_S;
        }
        if Self::parity_even(self.a) {
            flags |= FLAG_PV;
        }
        self.f = flags;
    }

    fn or_a(&mut self, value: u8) {
        self.a |= value;
        let mut flags = Self::xy_from_u8(self.a);
        if self.a == 0 {
            flags |= FLAG_Z;
        }
        if (self.a & 0x80) != 0 {
            flags |= FLAG_S;
        }
        if Self::parity_even(self.a) {
            flags |= FLAG_PV;
        }
        self.f = flags;
    }

    fn add_a(&mut self, value: u8) {
        let lhs = self.a;
        let (result, carry) = self.a.overflowing_add(value);
        self.a = result;
        let mut flags = Self::xy_from_u8(result);
        if result == 0 {
            flags |= FLAG_Z;
        }
        if (result & 0x80) != 0 {
            flags |= FLAG_S;
        }
        if ((lhs & 0x0F) + (value & 0x0F)) > 0x0F {
            flags |= FLAG_H;
        }
        if ((lhs ^ result) & (value ^ result) & 0x80) != 0 {
            flags |= FLAG_PV;
        }
        if carry {
            flags |= FLAG_C;
        }
        self.f = flags;
    }

    fn adc_a(&mut self, value: u8) {
        let carry_in = if self.flag_c() { 1u8 } else { 0 };
        let lhs = self.a;
        let sum = self.a as u16 + value as u16 + carry_in as u16;
        let result = sum as u8;
        self.a = result;
        let mut flags = Self::xy_from_u8(result);
        if result == 0 {
            flags |= FLAG_Z;
        }
        if (result & 0x80) != 0 {
            flags |= FLAG_S;
        }
        if ((lhs & 0x0F) as u16 + (value & 0x0F) as u16 + carry_in as u16) > 0x0F {
            flags |= FLAG_H;
        }
        if ((lhs ^ result) & (value ^ result) & 0x80) != 0 {
            flags |= FLAG_PV;
        }
        if sum > 0xFF {
            flags |= FLAG_C;
        }
        self.f = flags;
    }

    fn set_compare_flags(&mut self, value: u8) {
        let lhs = self.a;
        let result = self.a.wrapping_sub(value);
        let mut flags = FLAG_N | Self::xy_from_u8(result);
        if result == 0 {
            flags |= FLAG_Z;
        }
        if (result & 0x80) != 0 {
            flags |= FLAG_S;
        }
        if (lhs & 0x0F) < (value & 0x0F) {
            flags |= FLAG_H;
        }
        if ((lhs ^ value) & (lhs ^ result) & 0x80) != 0 {
            flags |= FLAG_PV;
        }
        if value > self.a {
            flags |= FLAG_C;
        }
        self.f = flags;
    }

    fn update_block_transfer_flags(&mut self, value: u8) {
        // LDI/LDD/LDIR/LDDR preserve S/Z/C, clear H/N, and set PV if BC != 0.
        let mut flags = self.f & (FLAG_S | FLAG_Z | FLAG_C);
        if self.bc() != 0 {
            flags |= FLAG_PV;
        }
        flags |= Self::xy_from_u8(self.a.wrapping_add(value));
        self.f = flags;
    }

    fn update_block_compare_flags(&mut self, value: u8) {
        // CPI/CPD/CPIR/CPDR preserve C, set N, and use BC!=0 for PV.
        let carry = self.f & FLAG_C;
        let result = self.a.wrapping_sub(value);
        let half_borrow = (self.a & 0x0F) < (value & 0x0F);
        let xy_src = result.wrapping_sub(if half_borrow { 1 } else { 0 });
        let mut flags = carry | FLAG_N | Self::xy_from_u8(xy_src);
        if result == 0 {
            flags |= FLAG_Z;
        }
        if (result & 0x80) != 0 {
            flags |= FLAG_S;
        }
        if half_borrow {
            flags |= FLAG_H;
        }
        if self.bc() != 0 {
            flags |= FLAG_PV;
        }
        self.f = flags;
    }

    fn update_block_in_flags(&mut self, value: u8, addr_delta: i8) {
        // INI/IND/INIR/INDR: derive H/C/PV from (C +/- 1) + data using 8-bit wrap.
        let c_adjusted = self.c.wrapping_add_signed(addr_delta);
        let io_sum = c_adjusted as u16 + value as u16;
        self.update_block_io_common_flags(value, io_sum as u8, io_sum > 0xFF);
    }

    fn update_block_out_flags(&mut self, value: u8, addr_delta: i8) {
        // OUTI/OUTD/OTIR/OTDR: derive H/C/PV from (C +/- 1) + data using 8-bit wrap.
        let c_adjusted = self.c.wrapping_add_signed(addr_delta);
        let io_sum = c_adjusted as u16 + value as u16;
        self.update_block_io_common_flags(value, io_sum as u8, io_sum > 0xFF);
    }

    fn update_block_io_common_flags(&mut self, value: u8, io_sum: u8, carry_hc: bool) {
        let mut flags = Self::xy_from_u8(self.b);
        if self.b == 0 {
            flags |= FLAG_Z;
        }
        if (self.b & 0x80) != 0 {
            flags |= FLAG_S;
        }
        if (value & 0x80) != 0 {
            flags |= FLAG_N;
        }
        if carry_hc {
            flags |= FLAG_H | FLAG_C;
        }
        let mix = (io_sum & 0x07) ^ self.b;
        if Self::parity_even(mix) {
            flags |= FLAG_PV;
        }
        self.f = flags;
    }

    fn sub_a(&mut self, value: u8) {
        let lhs = self.a;
        let (result, borrow) = self.a.overflowing_sub(value);
        self.a = result;
        let mut flags = FLAG_N | Self::xy_from_u8(result);
        if result == 0 {
            flags |= FLAG_Z;
        }
        if (result & 0x80) != 0 {
            flags |= FLAG_S;
        }
        if (lhs & 0x0F) < (value & 0x0F) {
            flags |= FLAG_H;
        }
        if ((lhs ^ value) & (lhs ^ result) & 0x80) != 0 {
            flags |= FLAG_PV;
        }
        if borrow {
            flags |= FLAG_C;
        }
        self.f = flags;
    }

    fn sbc_a(&mut self, value: u8) {
        let carry_in = if self.flag_c() { 1u8 } else { 0 };
        let lhs8 = self.a;
        let lhs = self.a as u16;
        let rhs = value as u16 + carry_in as u16;
        let result16 = lhs.wrapping_sub(rhs);
        let result = result16 as u8;
        self.a = result;
        let mut flags = FLAG_N | Self::xy_from_u8(result);
        if result == 0 {
            flags |= FLAG_Z;
        }
        if (result & 0x80) != 0 {
            flags |= FLAG_S;
        }
        let lhs_low = (lhs8 & 0x0F) as u16;
        let rhs_low = (value & 0x0F) as u16 + carry_in as u16;
        if rhs_low > lhs_low {
            flags |= FLAG_H;
        }
        if ((lhs8 ^ value) & (lhs8 ^ result) & 0x80) != 0 {
            flags |= FLAG_PV;
        }
        if rhs > lhs {
            flags |= FLAG_C;
        }
        self.f = flags;
    }

    fn adc_hl(&mut self, value: u16) {
        let hl = self.hl();
        let carry_in = if self.flag_c() { 1u32 } else { 0 };
        let sum = hl as u32 + value as u32 + carry_in;
        let result = sum as u16;
        self.set_hl(result);
        let mut flags = Self::xy_from_u16_hi(result);
        if result == 0 {
            flags |= FLAG_Z;
        }
        if (result & 0x8000) != 0 {
            flags |= FLAG_S;
        }
        if ((hl ^ result) & (value ^ result) & 0x8000) != 0 {
            flags |= FLAG_PV;
        }
        if ((hl & 0x0FFF) + (value & 0x0FFF) + carry_in as u16) > 0x0FFF {
            flags |= FLAG_H;
        }
        if sum > 0xFFFF {
            flags |= FLAG_C;
        }
        self.f = flags;
    }

    fn sbc_hl(&mut self, value: u16) {
        let hl = self.hl();
        let carry_in = if self.flag_c() { 1u32 } else { 0 };
        let rhs = value as u32 + carry_in;
        let lhs = hl as u32;
        let result = hl.wrapping_sub(value).wrapping_sub(carry_in as u16);
        self.set_hl(result);
        let mut flags = FLAG_N | Self::xy_from_u16_hi(result);
        if result == 0 {
            flags |= FLAG_Z;
        }
        if (result & 0x8000) != 0 {
            flags |= FLAG_S;
        }
        let rhs16 = value.wrapping_add(carry_in as u16);
        if ((hl ^ rhs16) & (hl ^ result) & 0x8000) != 0 {
            flags |= FLAG_PV;
        }
        if ((value & 0x0FFF) + carry_in as u16) > (hl & 0x0FFF) {
            flags |= FLAG_H;
        }
        if rhs > lhs {
            flags |= FLAG_C;
        }
        self.f = flags;
    }

    fn record_unknown(&mut self, opcode: u8, pc: u16) {
        self.unknown_opcode_total = self.unknown_opcode_total.saturating_add(1);
        *self.unknown_opcode_histogram.entry(opcode).or_insert(0) += 1;
        *self.unknown_opcode_pc_histogram.entry(pc).or_insert(0) += 1;
    }

    fn add_hl(&mut self, value: u16) {
        let hl = self.hl();
        let (result, carry) = hl.overflowing_add(value);
        self.set_hl(result);
        let mut flags = (self.f & (FLAG_S | FLAG_Z | FLAG_PV)) | Self::xy_from_u16_hi(result);
        if ((hl & 0x0FFF) + (value & 0x0FFF)) > 0x0FFF {
            flags |= FLAG_H;
        }
        if carry {
            flags |= FLAG_C;
        }
        self.f = flags;
    }

    fn add_index(&mut self, value: u16, use_ix: bool) {
        let idx = self.index_reg(use_ix);
        let (result, carry) = idx.overflowing_add(value);
        self.set_index_reg(use_ix, result);
        let mut flags = (self.f & (FLAG_S | FLAG_Z | FLAG_PV)) | Self::xy_from_u16_hi(result);
        if ((idx & 0x0FFF) + (value & 0x0FFF)) > 0x0FFF {
            flags |= FLAG_H;
        }
        if carry {
            flags |= FLAG_C;
        }
        self.f = flags;
    }
}

#[cfg(test)]
mod tests {
    use super::Z80;
    use crate::audio::AudioBus;
    use crate::cartridge::Cartridge;
    use crate::input::IoBus;
    use crate::vdp::Vdp;

    fn dummy_cart() -> Cartridge {
        Cartridge::from_bytes(vec![0; 0x200]).expect("valid cart")
    }

    #[test]
    fn bus_request_register_controls_halt_state() {
        let mut z80 = Z80::new();
        let mut audio = AudioBus::new();
        let cart = dummy_cart();
        let mut work_ram = [0u8; 0x10000];
        let mut vdp = Vdp::new();
        let mut io = IoBus::new();
        assert_eq!(z80.read_busreq_byte(), 0x01);

        z80.write_busreq_byte(0x01);
        assert_eq!(z80.read_busreq_byte(), 0x01);
        z80.step(16, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
        assert_eq!(z80.read_busreq_byte(), 0x00);
        z80.write_busreq_byte(0x00);
        assert_eq!(z80.read_busreq_byte(), 0x01);
    }

    #[test]
    fn reset_register_controls_run_state_and_cycles() {
        let mut z80 = Z80::new();
        let mut audio = AudioBus::new();
        let cart = dummy_cart();
        let mut work_ram = [0u8; 0x10000];
        let mut vdp = Vdp::new();
        let mut io = IoBus::new();
        z80.step(100, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
        assert_eq!(z80.cycles(), 0);

        z80.write_reset_byte(0x01); // release reset
        z80.step(100, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
        assert_eq!(z80.cycles(), 46);

        z80.write_busreq_byte(0x01); // bus requested -> grant pending, still running
        z80.step(8, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
        assert_eq!(z80.cycles(), 50);

        z80.step(8, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io); // grant reached at the end of this slice.
        assert_eq!(z80.cycles(), 54);

        z80.step(100, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io); // bus granted -> halt
        assert_eq!(z80.cycles(), 54);
    }

    #[test]
    fn bus_grant_mid_slice_only_runs_until_grant_edge() {
        let mut z80 = Z80::new();
        let mut audio = AudioBus::new();
        let cart = dummy_cart();
        let mut work_ram = [0u8; 0x10000];
        let mut vdp = Vdp::new();
        let mut io = IoBus::new();
        z80.write_reset_byte(0x01);
        z80.write_busreq_byte(0x01);

        // BUSACK delay is 16 M68k cycles; a larger slice must not run beyond it.
        z80.step(64, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);

        let expected = ((16u64 * super::Z80_CLOCK_HZ) / super::M68K_CLOCK_HZ) as u64;
        assert_eq!(z80.cycles(), expected);
        assert!(z80.bus_granted());
    }

    #[test]
    fn m68k_ram_access_becomes_available_after_bus_grant_delay() {
        let mut z80 = Z80::new();
        let mut audio = AudioBus::new();
        let cart = dummy_cart();
        let mut work_ram = [0u8; 0x10000];
        let mut vdp = Vdp::new();
        let mut io = IoBus::new();
        z80.write_busreq_byte(0x01);
        assert!(!z80.m68k_can_access_ram());

        z80.step(8, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
        assert!(!z80.m68k_can_access_ram());

        z80.step(100, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
        assert!(z80.m68k_can_access_ram());
    }

    #[test]
    fn bus_granted_or_reset_still_advances_audio_busy_timer() {
        let mut z80 = Z80::new();
        let mut audio = AudioBus::new();
        let cart = dummy_cart();
        let mut work_ram = [0u8; 0x10000];
        let mut vdp = Vdp::new();
        let mut io = IoBus::new();

        audio.write_ym2612(0, 0x22);
        audio.write_ym2612(1, 0x0F);
        assert_ne!(audio.read_ym2612(0) & 0x80, 0);

        // While reset is asserted, Z80 CPU is halted, but YM time should still pass.
        let mut cleared = false;
        for _ in 0..8 {
            z80.step(64, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
            if (audio.read_ym2612(0) & 0x80) == 0 {
                cleared = true;
                break;
            }
        }
        assert!(cleared);

        // Re-arm busy and verify BUSREQ-granted state also advances YM time.
        audio.write_ym2612(0, 0x22);
        audio.write_ym2612(1, 0x10);
        assert_ne!(audio.read_ym2612(0) & 0x80, 0);
        z80.write_reset_byte(0x01);
        z80.write_busreq_byte(0x01);
        z80.step(64, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
        z80.step(64, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
        assert!(z80.bus_granted());
        let mut cleared = false;
        for _ in 0..8 {
            z80.step(64, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
            if (audio.read_ym2612(0) & 0x80) == 0 {
                cleared = true;
                break;
            }
        }
        assert!(cleared);
    }

    #[test]
    fn z80_ram_is_8kb_and_mirrored() {
        let mut z80 = Z80::new();
        z80.write_ram_u8(0x0001, 0x12);
        z80.write_ram_u8(0x2001, 0x34); // mirror of 0x0001

        assert_eq!(z80.read_ram_u8(0x0001), 0x34);
        assert_eq!(z80.read_ram_u8(0x2001), 0x34);
    }

    #[test]
    fn executes_program_that_writes_ym2612_register() {
        let mut z80 = Z80::new();
        let mut audio = AudioBus::new();
        let cart = dummy_cart();
        let mut work_ram = [0u8; 0x10000];
        let mut vdp = Vdp::new();
        let mut io = IoBus::new();
        z80.write_reset_byte(0x01);

        // ld a,0x22 ; ld (0x4000),a ; ld a,0x0F ; ld (0x4001),a ; halt
        let program = [
            0x3E, 0x22, 0x32, 0x00, 0x40, 0x3E, 0x0F, 0x32, 0x01, 0x40, 0x76,
        ];
        for (i, byte) in program.iter().enumerate() {
            z80.write_ram_u8(i as u16, *byte);
        }

        z80.step(400, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
        assert_eq!(audio.ym2612().register(0, 0x22), 0x0F);
    }

    #[test]
    fn executes_program_that_writes_psg() {
        let mut z80 = Z80::new();
        let mut audio = AudioBus::new();
        let cart = dummy_cart();
        let mut work_ram = [0u8; 0x10000];
        let mut vdp = Vdp::new();
        let mut io = IoBus::new();
        z80.write_reset_byte(0x01);

        // ld a,0x9F ; ld (0x7F11),a ; halt
        let program = [0x3E, 0x9F, 0x32, 0x11, 0x7F, 0x76];
        for (i, byte) in program.iter().enumerate() {
            z80.write_ram_u8(i as u16, *byte);
        }

        z80.step(200, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
        assert_eq!(audio.psg().last_data(), 0x9F);
    }

    #[test]
    fn cpl_and_rla_are_implemented() {
        let mut z80 = Z80::new();
        let mut audio = AudioBus::new();
        let cart = dummy_cart();
        let mut work_ram = [0u8; 0x10000];
        let mut vdp = Vdp::new();
        let mut io = IoBus::new();
        z80.write_reset_byte(0x01);

        // LD A,0x80 ; RLA ; CPL ; HALT
        let program = [0x3E, 0x80, 0x17, 0x2F, 0x76];
        for (i, byte) in program.iter().enumerate() {
            z80.write_ram_u8(i as u16, *byte);
        }

        z80.step(128, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
        assert_eq!(z80.unknown_opcode_total(), 0);
        assert_eq!(z80.a, 0xFF);
        assert_ne!(z80.f & super::FLAG_C, 0);
        assert_ne!(z80.f & super::FLAG_N, 0);
        assert_ne!(z80.f & super::FLAG_H, 0);
    }

    #[test]
    fn scf_and_ccf_update_halfcarry_and_subtract_flags() {
        let mut z80 = Z80::new();
        let mut audio = AudioBus::new();
        let cart = dummy_cart();
        let mut work_ram = [0u8; 0x10000];
        let mut vdp = Vdp::new();
        let mut io = IoBus::new();
        z80.write_reset_byte(0x01);

        // XOR A ; SCF ; CCF ; HALT
        let program = [0xAF, 0x37, 0x3F, 0x76];
        for (i, byte) in program.iter().enumerate() {
            z80.write_ram_u8(i as u16, *byte);
        }
        z80.step(128, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);

        assert_eq!(z80.unknown_opcode_total(), 0);
        assert_eq!(z80.f & super::FLAG_C, 0);
        // CCF should move old carry into H and clear N.
        assert_ne!(z80.f & super::FLAG_H, 0);
        assert_eq!(z80.f & super::FLAG_N, 0);
    }

    #[test]
    fn scf_and_ccf_take_xy_from_a() {
        let mut z80 = Z80::new();
        let mut audio = AudioBus::new();
        let cart = dummy_cart();
        let mut work_ram = [0u8; 0x10000];
        let mut vdp = Vdp::new();
        let mut io = IoBus::new();
        z80.write_reset_byte(0x01);

        // LD A,0x28 ; SCF ; CCF ; HALT
        let program = [0x3E, 0x28, 0x37, 0x3F, 0x76];
        for (i, byte) in program.iter().enumerate() {
            z80.write_ram_u8(i as u16, *byte);
        }
        z80.step(128, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);

        assert_eq!(z80.unknown_opcode_total(), 0);
        assert_eq!(z80.f & super::FLAG_C, 0);
        assert_ne!(z80.f & super::FLAG_H, 0);
        assert_eq!(z80.f & super::FLAG_N, 0);
        assert_eq!(
            z80.f & (super::FLAG_X | super::FLAG_Y),
            super::FLAG_X | super::FLAG_Y
        );
    }

    #[test]
    fn index_prefixed_sub_and_sbc_memory_are_implemented() {
        let mut z80 = Z80::new();
        let mut audio = AudioBus::new();
        let cart = dummy_cart();
        let mut work_ram = [0u8; 0x10000];
        let mut vdp = Vdp::new();
        let mut io = IoBus::new();
        z80.write_reset_byte(0x01);
        z80.ix = 0x0100;
        z80.a = 5;

        z80.write_ram_u8(0x0101, 1);
        z80.write_ram_u8(0x0102, 2);

        // SUB A,(IX+1) ; SBC A,(IX+2) ; HALT
        let program = [0xDD, 0x96, 0x01, 0xDD, 0x9E, 0x02, 0x76];
        for (i, byte) in program.iter().enumerate() {
            z80.write_ram_u8(i as u16, *byte);
        }

        z80.step(256, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
        assert_eq!(z80.unknown_opcode_total(), 0);
        assert_eq!(z80.a, 2);
    }

    #[test]
    fn ed_neg_sets_n_h_and_c_flags() {
        let mut z80 = Z80::new();
        let mut audio = AudioBus::new();
        let cart = dummy_cart();
        let mut work_ram = [0u8; 0x10000];
        let mut vdp = Vdp::new();
        let mut io = IoBus::new();
        z80.write_reset_byte(0x01);

        // LD A,0x01 ; ED 44 (NEG) ; HALT
        let program = [0x3E, 0x01, 0xED, 0x44, 0x76];
        for (i, byte) in program.iter().enumerate() {
            z80.write_ram_u8(i as u16, *byte);
        }
        z80.step(128, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
        assert_eq!(z80.unknown_opcode_total(), 0);
        assert_eq!(z80.a, 0xFF);
        assert_ne!(z80.f & super::FLAG_N, 0);
        assert_ne!(z80.f & super::FLAG_H, 0);
        assert_ne!(z80.f & super::FLAG_C, 0);
        assert_eq!(
            z80.f & (super::FLAG_X | super::FLAG_Y),
            super::FLAG_X | super::FLAG_Y
        );

        z80.write_reset_byte(0x00);
        z80.write_reset_byte(0x01);
        // LD A,0x80 ; NEG ; HALT -> overflow sets PV
        let program = [0x3E, 0x80, 0xED, 0x44, 0x76];
        for (i, byte) in program.iter().enumerate() {
            z80.write_ram_u8(i as u16, *byte);
        }
        z80.step(128, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
        assert_eq!(z80.unknown_opcode_total(), 0);
        assert_eq!(z80.a, 0x80);
        assert_ne!(z80.f & super::FLAG_PV, 0);
        assert_eq!(z80.f & (super::FLAG_X | super::FLAG_Y), 0);
    }

    #[test]
    fn ed_in_b_sets_parity_and_preserves_carry() {
        let mut z80 = Z80::new();
        let mut audio = AudioBus::new();
        let cart = dummy_cart();
        let mut work_ram = [0u8; 0x10000];
        let mut vdp = Vdp::new();
        let mut io = IoBus::new();
        z80.write_reset_byte(0x01);

        // SCF ; LD BC,0x007F ; IN B,(C) ; HALT
        let program = [0x37, 0x01, 0x7F, 0x00, 0xED, 0x40, 0x76];
        for (i, byte) in program.iter().enumerate() {
            z80.write_ram_u8(i as u16, *byte);
        }
        z80.step(128, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
        assert_eq!(z80.unknown_opcode_total(), 0);
        assert_eq!(z80.b, 0xFF);
        assert_ne!(z80.f & super::FLAG_C, 0);
        assert_ne!(z80.f & super::FLAG_S, 0);
        assert_ne!(z80.f & super::FLAG_PV, 0);
        assert_eq!(z80.f & super::FLAG_Z, 0);
        assert_eq!(z80.f & super::FLAG_N, 0);
        assert_eq!(z80.f & super::FLAG_H, 0);
    }

    #[test]
    fn index_prefixed_cp_memory_sets_compare_flags() {
        let mut z80 = Z80::new();
        let mut audio = AudioBus::new();
        let cart = dummy_cart();
        let mut work_ram = [0u8; 0x10000];
        let mut vdp = Vdp::new();
        let mut io = IoBus::new();
        z80.write_reset_byte(0x01);
        z80.ix = 0x0100;
        z80.a = 0x10;
        z80.write_ram_u8(0x0101, 0x01);

        // CP (IX+1) ; HALT
        let program = [0xDD, 0xBE, 0x01, 0x76];
        for (i, byte) in program.iter().enumerate() {
            z80.write_ram_u8(i as u16, *byte);
        }
        z80.step(128, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
        assert_eq!(z80.unknown_opcode_total(), 0);
        // 0x10 - 0x01 => N set, H set, C clear, Z clear.
        assert_ne!(z80.f & super::FLAG_N, 0);
        assert_ne!(z80.f & super::FLAG_H, 0);
        assert_eq!(z80.f & super::FLAG_C, 0);
        assert_eq!(z80.f & super::FLAG_Z, 0);
    }

    #[test]
    fn bit_ix_d_uses_effective_address_high_for_xy_flags() {
        let mut z80 = Z80::new();
        let mut audio = AudioBus::new();
        let cart = dummy_cart();
        let mut work_ram = [0u8; 0x10000];
        let mut vdp = Vdp::new();
        let mut io = IoBus::new();
        z80.write_reset_byte(0x01);
        z80.ix = 0x2810;
        z80.write_ram_u8(0x0815, 0x00);

        // SCF ; BIT 0,(IX+5) ; HALT
        let program = [0x37, 0xDD, 0xCB, 0x05, 0x46, 0x76];
        for (i, byte) in program.iter().enumerate() {
            z80.write_ram_u8(i as u16, *byte);
        }
        z80.step(256, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
        assert_eq!(z80.unknown_opcode_total(), 0);
        assert_ne!(z80.f & super::FLAG_C, 0);
        assert_ne!(z80.f & super::FLAG_H, 0);
        assert_ne!(z80.f & super::FLAG_Z, 0);
        assert_ne!(z80.f & super::FLAG_PV, 0);
        assert_eq!(
            z80.f & (super::FLAG_X | super::FLAG_Y),
            super::FLAG_X | super::FLAG_Y
        );
    }

    #[test]
    fn index_high_low_register_ops_and_alu_are_implemented() {
        let mut z80 = Z80::new();
        let mut audio = AudioBus::new();
        let cart = dummy_cart();
        let mut work_ram = [0u8; 0x10000];
        let mut vdp = Vdp::new();
        let mut io = IoBus::new();
        z80.write_reset_byte(0x01);

        // LD IX,1234 ; LD IXH,20 ; LD IXL,05 ; LD A,IXH ; ADD A,IXL ; AND IXH ; OR IXL ; HALT
        let program = [
            0xDD, 0x21, 0x34, 0x12, 0xDD, 0x26, 0x20, 0xDD, 0x2E, 0x05, 0xDD, 0x7C, 0xDD, 0x85,
            0xDD, 0xA4, 0xDD, 0xB5, 0x76,
        ];
        for (i, byte) in program.iter().enumerate() {
            z80.write_ram_u8(i as u16, *byte);
        }

        z80.step(512, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
        assert_eq!(z80.unknown_opcode_total(), 0);
        assert_eq!(z80.ix, 0x2005);
        assert_eq!(z80.a, 0x25);
        assert_eq!(z80.pc, program.len() as u16);
    }

    #[test]
    fn index_displacement_h_l_forms_keep_plain_h_l_registers() {
        let mut z80 = Z80::new();
        let mut audio = AudioBus::new();
        let cart = dummy_cart();
        let mut work_ram = [0u8; 0x10000];
        let mut vdp = Vdp::new();
        let mut io = IoBus::new();
        z80.write_reset_byte(0x01);

        z80.write_ram_u8(0x0124, 0x11);
        z80.write_ram_u8(0x0125, 0x22);
        z80.write_ram_u8(0x0133, 0xAA);
        z80.write_ram_u8(0x0134, 0xBB);

        // LD H,99 ; LD L,88 ; LD IX,0120
        // LD (IX+2),H ; LD (IX+3),L ; LD H,(IX+4) ; LD L,(IX+5)
        // LD H,33 ; LD L,44 ; LD IY,0130
        // LD (IY+1),H ; LD (IY+2),L ; LD H,(IY+3) ; LD L,(IY+4) ; HALT
        let program = [
            0x26, 0x99, 0x2E, 0x88, 0xDD, 0x21, 0x20, 0x01, 0xDD, 0x74, 0x02, 0xDD, 0x75, 0x03,
            0xDD, 0x66, 0x04, 0xDD, 0x6E, 0x05, 0x26, 0x33, 0x2E, 0x44, 0xFD, 0x21, 0x30, 0x01,
            0xFD, 0x74, 0x01, 0xFD, 0x75, 0x02, 0xFD, 0x66, 0x03, 0xFD, 0x6E, 0x04, 0x76,
        ];
        for (i, byte) in program.iter().enumerate() {
            z80.write_ram_u8(i as u16, *byte);
        }

        z80.step(2048, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
        assert_eq!(z80.unknown_opcode_total(), 0);

        // DD/FD + LD (IX/IY+d),H/L and LD H/L,(IX/IY+d) keep plain H/L semantics.
        assert_eq!(z80.read_ram_u8(0x0122), 0x99);
        assert_eq!(z80.read_ram_u8(0x0123), 0x88);
        assert_eq!(z80.read_ram_u8(0x0131), 0x33);
        assert_eq!(z80.read_ram_u8(0x0132), 0x44);

        // Final H/L come from memory loads.
        assert_eq!(z80.h, 0xAA);
        assert_eq!(z80.l, 0xBB);
        // IX/IY themselves are unchanged by these H/L memory forms.
        assert_eq!(z80.ix, 0x0120);
        assert_eq!(z80.iy, 0x0130);
    }

    #[test]
    fn index_cb_bit_uses_20_cycles_while_res_set_use_23() {
        let mut z80 = Z80::new();
        let mut audio = AudioBus::new();
        let cart = dummy_cart();
        let mut work_ram = [0u8; 0x10000];
        let mut vdp = Vdp::new();
        let mut io = IoBus::new();
        z80.write_reset_byte(0x01);
        z80.ix = 0x0100;
        z80.write_ram_u8(0x0101, 0x01);

        let mut bus = super::Z80Bus {
            audio: &mut audio,
            cartridge: &cart,
            work_ram: &mut work_ram,
            vdp: &mut vdp,
            io: &mut io,
        };

        // BIT 0,(IX+1)
        let c_bit = z80.exec_index_cb(true, 1, 0x46, &mut bus);
        assert_eq!(c_bit, 20);
        assert_eq!(z80.read_ram_u8(0x0101), 0x01);

        // RES 0,(IX+1)
        let c_res = z80.exec_index_cb(true, 1, 0x86, &mut bus);
        assert_eq!(c_res, 23);
        assert_eq!(z80.read_ram_u8(0x0101), 0x00);

        // SET 0,(IX+1)
        let c_set = z80.exec_index_cb(true, 1, 0xC6, &mut bus);
        assert_eq!(c_set, 23);
        assert_eq!(z80.read_ram_u8(0x0101), 0x01);
    }

    #[test]
    fn add_ix_iy_rr_update_halfcarry_and_carry_flags() {
        let mut z80 = Z80::new();
        let mut audio = AudioBus::new();
        let cart = dummy_cart();
        let mut work_ram = [0u8; 0x10000];
        let mut vdp = Vdp::new();
        let mut io = IoBus::new();
        z80.write_reset_byte(0x01);
        z80.ix = 0xFFFF;
        z80.set_bc(0x0001);
        z80.f = super::FLAG_S | super::FLAG_Z | super::FLAG_PV | super::FLAG_N;

        // ADD IX,BC ; HALT
        let ix_program = [0xDD, 0x09, 0x76];
        for (i, byte) in ix_program.iter().enumerate() {
            z80.write_ram_u8(i as u16, *byte);
        }

        z80.step(256, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
        assert_eq!(z80.unknown_opcode_total(), 0);
        assert_eq!(z80.ix, 0x0000);
        assert_ne!(z80.f & super::FLAG_H, 0);
        assert_ne!(z80.f & super::FLAG_C, 0);
        assert_eq!(z80.f & super::FLAG_N, 0);
        assert_ne!(z80.f & super::FLAG_S, 0);
        assert_ne!(z80.f & super::FLAG_Z, 0);
        assert_ne!(z80.f & super::FLAG_PV, 0);

        z80.write_reset_byte(0x00);
        z80.write_reset_byte(0x01);
        z80.iy = 0x0FFF;
        z80.sp = 0x0001;

        // ADD IY,SP ; HALT
        let iy_program = [0xFD, 0x39, 0x76];
        for (i, byte) in iy_program.iter().enumerate() {
            z80.write_ram_u8(i as u16, *byte);
        }

        z80.step(256, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
        assert_eq!(z80.unknown_opcode_total(), 0);
        assert_eq!(z80.iy, 0x1000);
        assert_ne!(z80.f & super::FLAG_H, 0);
        assert_eq!(z80.f & super::FLAG_C, 0);
    }

    #[test]
    fn index_prefixed_halt_is_supported() {
        let mut z80 = Z80::new();
        let mut audio = AudioBus::new();
        let cart = dummy_cart();
        let mut work_ram = [0u8; 0x10000];
        let mut vdp = Vdp::new();
        let mut io = IoBus::new();
        z80.write_reset_byte(0x01);

        z80.write_ram_u8(0x0000, 0xDD);
        z80.write_ram_u8(0x0001, 0x76);

        z80.step(64, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
        assert_eq!(z80.unknown_opcode_total(), 0);
        assert!(z80.halted);
        assert_eq!(z80.pc, 0x0002);
    }

    #[test]
    fn ed_ld_bc_mem_and_ld_mem_bc_are_implemented() {
        let mut z80 = Z80::new();
        let mut audio = AudioBus::new();
        let cart = dummy_cart();
        let mut work_ram = [0u8; 0x10000];
        let mut vdp = Vdp::new();
        let mut io = IoBus::new();
        z80.write_reset_byte(0x01);
        z80.set_bc(0x1234);

        // LD (0x0100),BC ; LD BC,(0x0100) ; HALT
        let program = [0xED, 0x43, 0x00, 0x01, 0xED, 0x4B, 0x00, 0x01, 0x76];
        for (i, byte) in program.iter().enumerate() {
            z80.write_ram_u8(i as u16, *byte);
        }

        z80.step(256, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
        assert_eq!(z80.unknown_opcode_total(), 0);
        assert_eq!(z80.read_ram_u8(0x0100), 0x34);
        assert_eq!(z80.read_ram_u8(0x0101), 0x12);
        assert_eq!(z80.bc(), 0x1234);
    }

    #[test]
    fn adc_immediate_and_adc_indexed_are_implemented() {
        let mut z80 = Z80::new();
        let mut audio = AudioBus::new();
        let cart = dummy_cart();
        let mut work_ram = [0u8; 0x10000];
        let mut vdp = Vdp::new();
        let mut io = IoBus::new();
        z80.write_reset_byte(0x01);
        z80.ix = 0x0100;
        z80.a = 1;
        z80.write_ram_u8(0x0105, 2);

        // SBC A,0x00 ; ADC A,(IX+5) ; HALT
        let program = [0xDE, 0x00, 0xDD, 0x8E, 0x05, 0x76];
        for (i, byte) in program.iter().enumerate() {
            z80.write_ram_u8(i as u16, *byte);
        }

        z80.step(256, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
        assert_eq!(z80.unknown_opcode_total(), 0);
        assert_eq!(z80.a, 3);
    }

    #[test]
    fn ed_adc_sbc_hl_rr_are_implemented() {
        let mut z80 = Z80::new();
        let mut audio = AudioBus::new();
        let cart = dummy_cart();
        let mut work_ram = [0u8; 0x10000];
        let mut vdp = Vdp::new();
        let mut io = IoBus::new();
        z80.write_reset_byte(0x01);
        z80.set_hl(0x1000);
        z80.set_bc(0x0001);
        z80.set_de(0x0002);
        z80.sp = 0x0003;

        // ADC HL,BC ; ADC HL,DE ; ADC HL,SP ; SBC HL,BC ; HALT
        let program = [0xED, 0x4A, 0xED, 0x5A, 0xED, 0x7A, 0xED, 0x42, 0x76];
        for (i, byte) in program.iter().enumerate() {
            z80.write_ram_u8(i as u16, *byte);
        }

        z80.step(512, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
        assert_eq!(z80.unknown_opcode_total(), 0);
        assert_eq!(z80.hl(), 0x1005);
    }

    #[test]
    fn ed_ldd_and_lddr_are_implemented() {
        let mut z80 = Z80::new();
        let mut audio = AudioBus::new();
        let cart = dummy_cart();
        let mut work_ram = [0u8; 0x10000];
        let mut vdp = Vdp::new();
        let mut io = IoBus::new();
        z80.write_reset_byte(0x01);
        z80.set_hl(0x0102);
        z80.set_de(0x0202);
        z80.set_bc(0x0003);
        z80.write_ram_u8(0x0100, 0x11);
        z80.write_ram_u8(0x0101, 0x22);
        z80.write_ram_u8(0x0102, 0x33);

        // LDD ; LDDR ; HALT
        let program = [0xED, 0xA8, 0xED, 0xB8, 0x76];
        for (i, byte) in program.iter().enumerate() {
            z80.write_ram_u8(i as u16, *byte);
        }

        z80.step(512, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
        assert_eq!(z80.unknown_opcode_total(), 0);
        assert_eq!(z80.read_ram_u8(0x0202), 0x33);
        assert_eq!(z80.read_ram_u8(0x0201), 0x22);
        assert_eq!(z80.read_ram_u8(0x0200), 0x11);
        assert_eq!(z80.bc(), 0x0000);
        assert_eq!(z80.hl(), 0x00FF);
        assert_eq!(z80.de(), 0x01FF);
    }

    #[test]
    fn ed_ldi_updates_block_transfer_flags() {
        let mut z80 = Z80::new();
        let mut audio = AudioBus::new();
        let cart = dummy_cart();
        let mut work_ram = [0u8; 0x10000];
        let mut vdp = Vdp::new();
        let mut io = IoBus::new();
        z80.write_reset_byte(0x01);
        z80.a = 0x20;
        z80.f = super::FLAG_S | super::FLAG_Z | super::FLAG_C | super::FLAG_H | super::FLAG_N;
        z80.set_hl(0x0100);
        z80.set_de(0x0200);
        z80.set_bc(0x0002);
        z80.write_ram_u8(0x0100, 0x11);

        // LDI ; HALT
        let program = [0xED, 0xA0, 0x76];
        for (i, byte) in program.iter().enumerate() {
            z80.write_ram_u8(i as u16, *byte);
        }

        z80.step(256, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
        assert_eq!(z80.unknown_opcode_total(), 0);
        assert_eq!(z80.read_ram_u8(0x0200), 0x11);
        assert_eq!(z80.bc(), 0x0001);
        assert_ne!(z80.f & super::FLAG_S, 0);
        assert_ne!(z80.f & super::FLAG_Z, 0);
        assert_ne!(z80.f & super::FLAG_C, 0);
        assert_ne!(z80.f & super::FLAG_PV, 0);
        assert_eq!(z80.f & (super::FLAG_H | super::FLAG_N), 0);
    }

    #[test]
    fn ed_cpi_uses_bc_for_pv_and_preserves_carry() {
        let mut z80 = Z80::new();
        let mut audio = AudioBus::new();
        let cart = dummy_cart();
        let mut work_ram = [0u8; 0x10000];
        let mut vdp = Vdp::new();
        let mut io = IoBus::new();
        z80.write_reset_byte(0x01);
        z80.a = 0x10;
        z80.f = super::FLAG_C;
        z80.set_hl(0x0100);
        z80.set_bc(0x0002);
        z80.write_ram_u8(0x0100, 0x01);

        // CPI ; HALT
        let program = [0xED, 0xA1, 0x76];
        for (i, byte) in program.iter().enumerate() {
            z80.write_ram_u8(i as u16, *byte);
        }

        z80.step(256, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
        assert_eq!(z80.unknown_opcode_total(), 0);
        assert_eq!(z80.hl(), 0x0101);
        assert_eq!(z80.bc(), 0x0001);
        assert_ne!(z80.f & super::FLAG_C, 0);
        assert_ne!(z80.f & super::FLAG_N, 0);
        assert_ne!(z80.f & super::FLAG_H, 0);
        assert_ne!(z80.f & super::FLAG_PV, 0);
        assert_eq!(z80.f & (super::FLAG_Z | super::FLAG_S), 0);
    }

    #[test]
    fn ed_cpi_uses_a_minus_mem_minus_h_for_xy_flags() {
        let mut z80 = Z80::new();
        let mut audio = AudioBus::new();
        let cart = dummy_cart();
        let mut work_ram = [0u8; 0x10000];
        let mut vdp = Vdp::new();
        let mut io = IoBus::new();
        z80.write_reset_byte(0x01);
        z80.a = 0x30;
        z80.set_hl(0x0100);
        z80.set_bc(0x0002);
        z80.write_ram_u8(0x0100, 0x08);

        // CPI ; HALT
        let program = [0xED, 0xA1, 0x76];
        for (i, byte) in program.iter().enumerate() {
            z80.write_ram_u8(i as u16, *byte);
        }

        z80.step(256, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
        assert_eq!(z80.unknown_opcode_total(), 0);
        assert_eq!(z80.hl(), 0x0101);
        assert_eq!(z80.bc(), 0x0001);
        assert_ne!(z80.f & super::FLAG_N, 0);
        assert_ne!(z80.f & super::FLAG_H, 0);
        assert_ne!(z80.f & super::FLAG_PV, 0);
        // result=0x28, H=1 => undocumented XY come from 0x27.
        assert_eq!(z80.f & (super::FLAG_X | super::FLAG_Y), super::FLAG_Y);
    }

    #[test]
    fn ed_cpir_repeats_until_match() {
        let mut z80 = Z80::new();
        let mut audio = AudioBus::new();
        let cart = dummy_cart();
        let mut work_ram = [0u8; 0x10000];
        let mut vdp = Vdp::new();
        let mut io = IoBus::new();
        z80.write_reset_byte(0x01);
        z80.a = 0x22;
        z80.set_hl(0x0100);
        z80.set_bc(0x0003);
        z80.write_ram_u8(0x0100, 0x10);
        z80.write_ram_u8(0x0101, 0x22);
        z80.write_ram_u8(0x0102, 0x33);

        // CPIR ; HALT
        let program = [0xED, 0xB1, 0x76];
        for (i, byte) in program.iter().enumerate() {
            z80.write_ram_u8(i as u16, *byte);
        }

        z80.step(512, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
        assert_eq!(z80.unknown_opcode_total(), 0);
        assert_eq!(z80.hl(), 0x0102);
        assert_eq!(z80.bc(), 0x0001);
        assert_ne!(z80.f & super::FLAG_Z, 0);
    }

    #[test]
    fn ed_cpir_clears_pv_when_bc_reaches_zero_without_match() {
        let mut z80 = Z80::new();
        let mut audio = AudioBus::new();
        let cart = dummy_cart();
        let mut work_ram = [0u8; 0x10000];
        let mut vdp = Vdp::new();
        let mut io = IoBus::new();
        z80.write_reset_byte(0x01);
        z80.a = 0x7E;
        z80.f = super::FLAG_C;
        z80.set_hl(0x0100);
        z80.set_bc(0x0002);
        z80.write_ram_u8(0x0100, 0x10);
        z80.write_ram_u8(0x0101, 0x20);

        // CPIR ; HALT
        let program = [0xED, 0xB1, 0x76];
        for (i, byte) in program.iter().enumerate() {
            z80.write_ram_u8(i as u16, *byte);
        }

        z80.step(1024, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
        assert_eq!(z80.unknown_opcode_total(), 0);
        assert_eq!(z80.hl(), 0x0102);
        assert_eq!(z80.bc(), 0x0000);
        assert_ne!(z80.f & super::FLAG_C, 0);
        assert_ne!(z80.f & super::FLAG_N, 0);
        assert_eq!(z80.f & super::FLAG_PV, 0);
        assert_eq!(z80.f & super::FLAG_Z, 0);
    }

    #[test]
    fn ed_cpdr_repeats_until_match() {
        let mut z80 = Z80::new();
        let mut audio = AudioBus::new();
        let cart = dummy_cart();
        let mut work_ram = [0u8; 0x10000];
        let mut vdp = Vdp::new();
        let mut io = IoBus::new();
        z80.write_reset_byte(0x01);
        z80.a = 0x22;
        z80.set_hl(0x0102);
        z80.set_bc(0x0003);
        z80.write_ram_u8(0x0100, 0x10);
        z80.write_ram_u8(0x0101, 0x22);
        z80.write_ram_u8(0x0102, 0x33);

        // CPDR ; HALT
        let program = [0xED, 0xB9, 0x76];
        for (i, byte) in program.iter().enumerate() {
            z80.write_ram_u8(i as u16, *byte);
        }

        z80.step(512, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
        assert_eq!(z80.unknown_opcode_total(), 0);
        assert_eq!(z80.hl(), 0x0100);
        assert_eq!(z80.bc(), 0x0001);
        assert_ne!(z80.f & super::FLAG_Z, 0);
    }

    #[test]
    fn out_immediate_writes_psg_port() {
        let mut z80 = Z80::new();
        let mut audio = AudioBus::new();
        let cart = dummy_cart();
        let mut work_ram = [0u8; 0x10000];
        let mut vdp = Vdp::new();
        let mut io = IoBus::new();
        z80.write_reset_byte(0x01);

        // LD A,0x9A ; OUT (0x7F),A ; HALT
        let program = [0x3E, 0x9A, 0xD3, 0x7F, 0x76];
        for (i, byte) in program.iter().enumerate() {
            z80.write_ram_u8(i as u16, *byte);
        }

        z80.step(256, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
        assert_eq!(z80.unknown_opcode_total(), 0);
        assert_eq!(audio.psg().last_data(), 0x9A);
    }

    #[test]
    fn out_immediate_writes_ym2612_via_port_io() {
        let mut z80 = Z80::new();
        let mut audio = AudioBus::new();
        let cart = dummy_cart();
        let mut work_ram = [0u8; 0x10000];
        let mut vdp = Vdp::new();
        let mut io = IoBus::new();
        z80.write_reset_byte(0x01);

        // LD A,0x22 ; OUT (0x40),A ; LD A,0x0F ; OUT (0x41),A ; HALT
        let program = [0x3E, 0x22, 0xD3, 0x40, 0x3E, 0x0F, 0xD3, 0x41, 0x76];
        for (i, byte) in program.iter().enumerate() {
            z80.write_ram_u8(i as u16, *byte);
        }

        z80.step(320, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
        assert_eq!(z80.unknown_opcode_total(), 0);
        assert_eq!(audio.ym2612().register(0, 0x22), 0x0F);
    }

    #[test]
    fn ed_out_c_a_writes_ym2612_via_port_io() {
        let mut z80 = Z80::new();
        let mut audio = AudioBus::new();
        let cart = dummy_cart();
        let mut work_ram = [0u8; 0x10000];
        let mut vdp = Vdp::new();
        let mut io = IoBus::new();
        z80.write_reset_byte(0x01);

        // LD BC,0x0040 ; LD A,0x22 ; OUT (C),A ; INC C ; LD A,0x0F ; OUT (C),A ; HALT
        let program = [
            0x01, 0x40, 0x00, 0x3E, 0x22, 0xED, 0x79, 0x0C, 0x3E, 0x0F, 0xED, 0x79, 0x76,
        ];
        for (i, byte) in program.iter().enumerate() {
            z80.write_ram_u8(i as u16, *byte);
        }

        z80.step(512, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
        assert_eq!(z80.unknown_opcode_total(), 0);
        assert_eq!(audio.ym2612().register(0, 0x22), 0x0F);
    }

    #[test]
    fn in_immediate_reads_ym2612_status_via_port_io() {
        let mut z80 = Z80::new();
        let mut audio = AudioBus::new();
        let cart = dummy_cart();
        let mut work_ram = [0u8; 0x10000];
        let mut vdp = Vdp::new();
        let mut io = IoBus::new();
        z80.write_reset_byte(0x01);

        // Preload YM timer-A status bit so IN can verify YM status routing robustly.
        audio.write_ym2612(0, 0x24);
        audio.write_ym2612(1, 0xFF);
        audio.write_ym2612(0, 0x25);
        audio.write_ym2612(1, 0x03);
        audio.write_ym2612(0, 0x27);
        audio.write_ym2612(1, 0x05);
        audio.step_z80_cycles(80);
        assert_ne!(audio.read_ym2612(0) & 0x01, 0);

        // IN A,(0x40) ; HALT
        let program = [0xDB, 0x40, 0x76];
        for (i, byte) in program.iter().enumerate() {
            z80.write_ram_u8(i as u16, *byte);
        }

        z80.step(256, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
        assert_eq!(z80.unknown_opcode_total(), 0);
        assert_ne!(z80.a & 0x01, 0);
    }

    #[test]
    fn ed_in_c_a_reads_ym2612_status_via_port_io() {
        let mut z80 = Z80::new();
        let mut audio = AudioBus::new();
        let cart = dummy_cart();
        let mut work_ram = [0u8; 0x10000];
        let mut vdp = Vdp::new();
        let mut io = IoBus::new();
        z80.write_reset_byte(0x01);

        // Preload YM timer-A status bit so IN can verify YM status routing robustly.
        audio.write_ym2612(0, 0x24);
        audio.write_ym2612(1, 0xFF);
        audio.write_ym2612(0, 0x25);
        audio.write_ym2612(1, 0x03);
        audio.write_ym2612(0, 0x27);
        audio.write_ym2612(1, 0x05);
        audio.step_z80_cycles(80);
        assert_ne!(audio.read_ym2612(0) & 0x01, 0);

        // LD BC,0x0040 ; IN A,(C) ; HALT
        let program = [0x01, 0x40, 0x00, 0xED, 0x78, 0x76];
        for (i, byte) in program.iter().enumerate() {
            z80.write_ram_u8(i as u16, *byte);
        }

        z80.step(256, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
        assert_eq!(z80.unknown_opcode_total(), 0);
        assert_ne!(z80.a & 0x01, 0);
    }

    #[test]
    fn ed_otir_repeats_and_writes_psg_port() {
        let mut z80 = Z80::new();
        let mut audio = AudioBus::new();
        let cart = dummy_cart();
        let mut work_ram = [0u8; 0x10000];
        let mut vdp = Vdp::new();
        let mut io = IoBus::new();
        z80.write_reset_byte(0x01);
        z80.set_hl(0x0100);
        z80.set_bc(0x027F); // B=2, C=0x7F
        z80.write_ram_u8(0x0100, 0x9A);
        z80.write_ram_u8(0x0101, 0x9B);

        // OTIR ; HALT
        let program = [0xED, 0xB3, 0x76];
        for (i, byte) in program.iter().enumerate() {
            z80.write_ram_u8(i as u16, *byte);
        }

        z80.step(512, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
        assert_eq!(z80.unknown_opcode_total(), 0);
        assert_eq!(audio.psg().last_data(), 0x9B);
        assert_eq!(z80.b, 0);
        assert_eq!(z80.hl(), 0x0102);
    }

    #[test]
    fn ed_inir_reads_port_into_memory_until_b_zero() {
        let mut z80 = Z80::new();
        let mut audio = AudioBus::new();
        let cart = dummy_cart();
        let mut work_ram = [0u8; 0x10000];
        let mut vdp = Vdp::new();
        let mut io = IoBus::new();
        z80.write_reset_byte(0x01);
        z80.set_hl(0x0100);
        z80.set_bc(0x0200); // B=2

        // INIR ; HALT
        let program = [0xED, 0xB2, 0x76];
        for (i, byte) in program.iter().enumerate() {
            z80.write_ram_u8(i as u16, *byte);
        }

        z80.step(512, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
        assert_eq!(z80.unknown_opcode_total(), 0);
        assert_eq!(z80.read_ram_u8(0x0100), 0xFF);
        assert_eq!(z80.read_ram_u8(0x0101), 0xFF);
        assert_eq!(z80.b, 0);
        assert_eq!(z80.hl(), 0x0102);
    }

    #[test]
    fn ed_ini_updates_block_io_flags() {
        let mut z80 = Z80::new();
        let mut audio = AudioBus::new();
        let cart = dummy_cart();
        let mut work_ram = [0u8; 0x10000];
        let mut vdp = Vdp::new();
        let mut io = IoBus::new();
        z80.write_reset_byte(0x01);
        z80.set_hl(0x0100);
        z80.set_bc(0x0100); // B=1, C=0

        // INI ; HALT
        let program = [0xED, 0xA2, 0x76];
        for (i, byte) in program.iter().enumerate() {
            z80.write_ram_u8(i as u16, *byte);
        }

        z80.step(256, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
        assert_eq!(z80.unknown_opcode_total(), 0);
        assert_eq!(z80.read_ram_u8(0x0100), 0xFF);
        assert_eq!(z80.b, 0);
        assert_eq!(z80.hl(), 0x0101);
        assert_ne!(z80.f & super::FLAG_Z, 0);
        assert_ne!(z80.f & super::FLAG_N, 0);
        assert_ne!(z80.f & super::FLAG_H, 0);
        assert_ne!(z80.f & super::FLAG_C, 0);
        assert_ne!(z80.f & super::FLAG_PV, 0);
        assert_eq!(z80.f & super::FLAG_S, 0);
    }

    #[test]
    fn ed_outd_updates_block_io_flags() {
        let mut z80 = Z80::new();
        let mut audio = AudioBus::new();
        let cart = dummy_cart();
        let mut work_ram = [0u8; 0x10000];
        let mut vdp = Vdp::new();
        let mut io = IoBus::new();
        z80.write_reset_byte(0x01);
        z80.set_hl(0x0081);
        z80.set_bc(0x017F); // B=1, C=0x7F (PSG port)
        z80.write_ram_u8(0x0081, 0x80);

        // OUTD ; HALT
        let program = [0xED, 0xAB, 0x76];
        for (i, byte) in program.iter().enumerate() {
            z80.write_ram_u8(i as u16, *byte);
        }

        z80.step(256, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
        assert_eq!(z80.unknown_opcode_total(), 0);
        assert_eq!(audio.psg().last_data(), 0x80);
        assert_eq!(z80.b, 0);
        assert_eq!(z80.hl(), 0x0080);
        assert_ne!(z80.f & super::FLAG_Z, 0);
        assert_ne!(z80.f & super::FLAG_N, 0);
        assert_eq!(z80.f & super::FLAG_H, 0);
        assert_eq!(z80.f & super::FLAG_C, 0);
        assert_ne!(z80.f & super::FLAG_PV, 0);
        assert_eq!(z80.f & super::FLAG_S, 0);
    }

    #[test]
    fn ed_outi_uses_c_plus_one_for_hc_calculation() {
        let mut z80 = Z80::new();
        let mut audio = AudioBus::new();
        let cart = dummy_cart();
        let mut work_ram = [0u8; 0x10000];
        let mut vdp = Vdp::new();
        let mut io = IoBus::new();
        z80.write_reset_byte(0x01);
        z80.set_hl(0x1200);
        z80.set_bc(0x01FE); // B=1, C=0xFE
        z80.write_ram_u8(0x1200, 0x02);

        // OUTI ; HALT
        let program = [0xED, 0xA3, 0x76];
        for (i, byte) in program.iter().enumerate() {
            z80.write_ram_u8(i as u16, *byte);
        }

        z80.step(256, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
        assert_eq!(z80.unknown_opcode_total(), 0);
        assert_eq!(z80.b, 0);
        assert_eq!(z80.hl(), 0x1201);
        // (C+1)=0xFF; 0xFF + 0x02 overflows -> H and C set.
        assert_ne!(z80.f & super::FLAG_H, 0);
        assert_ne!(z80.f & super::FLAG_C, 0);
    }

    #[test]
    fn ed_outd_uses_c_minus_one_for_hc_calculation() {
        let mut z80 = Z80::new();
        let mut audio = AudioBus::new();
        let cart = dummy_cart();
        let mut work_ram = [0u8; 0x10000];
        let mut vdp = Vdp::new();
        let mut io = IoBus::new();
        z80.write_reset_byte(0x01);
        z80.set_hl(0x1201);
        z80.set_bc(0x0100); // B=1, C=0x00
        z80.write_ram_u8(0x1201, 0x80);

        // OUTD ; HALT
        let program = [0xED, 0xAB, 0x76];
        for (i, byte) in program.iter().enumerate() {
            z80.write_ram_u8(i as u16, *byte);
        }

        z80.step(256, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
        assert_eq!(z80.unknown_opcode_total(), 0);
        assert_eq!(z80.b, 0);
        assert_eq!(z80.hl(), 0x1200);
        // (C-1)=0xFF; 0xFF + 0x80 overflows -> H and C set.
        assert_ne!(z80.f & super::FLAG_H, 0);
        assert_ne!(z80.f & super::FLAG_C, 0);
    }

    #[test]
    fn ed_ini_wraps_c_plus_one_for_hc_calculation() {
        let mut z80 = Z80::new();
        let mut audio = AudioBus::new();
        let cart = dummy_cart();
        let mut work_ram = [0u8; 0x10000];
        let mut vdp = Vdp::new();
        let mut io = IoBus::new();
        z80.write_reset_byte(0x01);
        z80.set_hl(0x0100);
        z80.set_bc(0x01FF); // B=1, C=0xFF (port returns 0xFF default)

        // INI ; HALT
        let program = [0xED, 0xA2, 0x76];
        for (i, byte) in program.iter().enumerate() {
            z80.write_ram_u8(i as u16, *byte);
        }

        z80.step(256, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
        assert_eq!(z80.unknown_opcode_total(), 0);
        assert_eq!(z80.read_ram_u8(0x0100), 0xFF);
        assert_eq!(z80.b, 0);
        // (C+1) wraps to 0x00; 0x00 + 0xFF does not set carry/half-carry in this rule.
        assert_eq!(z80.f & (super::FLAG_H | super::FLAG_C), 0);
    }

    #[test]
    fn ed_ind_wraps_c_minus_one_for_hc_calculation() {
        let mut z80 = Z80::new();
        let mut audio = AudioBus::new();
        let cart = dummy_cart();
        let mut work_ram = [0u8; 0x10000];
        let mut vdp = Vdp::new();
        let mut io = IoBus::new();
        z80.write_reset_byte(0x01);
        z80.set_hl(0x0100);
        z80.set_bc(0x0100); // B=1, C=0x00 (port returns 0xFF default)

        // IND ; HALT
        let program = [0xED, 0xAA, 0x76];
        for (i, byte) in program.iter().enumerate() {
            z80.write_ram_u8(i as u16, *byte);
        }

        z80.step(256, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
        assert_eq!(z80.unknown_opcode_total(), 0);
        assert_eq!(z80.read_ram_u8(0x0100), 0xFF);
        assert_eq!(z80.b, 0);
        // (C-1) wraps to 0xFF; 0xFF + 0xFF sets both H and C in block I/O rules.
        assert_ne!(z80.f & super::FLAG_H, 0);
        assert_ne!(z80.f & super::FLAG_C, 0);
    }

    #[test]
    fn ed_rld_and_rrd_transform_nibbles_between_a_and_hl() {
        let mut z80 = Z80::new();
        let mut audio = AudioBus::new();
        let cart = dummy_cart();
        let mut work_ram = [0u8; 0x10000];
        let mut vdp = Vdp::new();
        let mut io = IoBus::new();
        z80.write_reset_byte(0x01);
        z80.set_hl(0x0100);
        z80.a = 0xAB;
        z80.write_ram_u8(0x0100, 0xCD);

        // RLD ; RRD ; HALT
        let program = [0xED, 0x6F, 0xED, 0x67, 0x76];
        for (i, byte) in program.iter().enumerate() {
            z80.write_ram_u8(i as u16, *byte);
        }

        z80.step(256, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
        assert_eq!(z80.unknown_opcode_total(), 0);
        assert_eq!(z80.a, 0xAB);
        assert_eq!(z80.read_ram_u8(0x0100), 0xCD);
    }

    #[test]
    fn ed_ld_i_r_and_ld_a_i_r_are_implemented() {
        let mut z80 = Z80::new();
        let mut audio = AudioBus::new();
        let cart = dummy_cart();
        let mut work_ram = [0u8; 0x10000];
        let mut vdp = Vdp::new();
        let mut io = IoBus::new();
        z80.write_reset_byte(0x01);
        z80.f = super::FLAG_C;

        // LD A,0xA5 ; LD I,A ; XOR A ; LD A,I ; LD A,0x80 ; LD R,A ; LD A,R ; HALT
        let program = [
            0x3E, 0xA5, 0xED, 0x47, 0xAF, 0xED, 0x57, 0x3E, 0x80, 0xED, 0x4F, 0xED, 0x5F, 0x76,
        ];
        for (i, byte) in program.iter().enumerate() {
            z80.write_ram_u8(i as u16, *byte);
        }

        z80.step(512, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
        assert_eq!(z80.unknown_opcode_total(), 0);
        assert_eq!(z80.i_reg, 0xA5);
        assert_eq!(z80.a, 0x82);
    }

    #[test]
    fn ed_ld_a_i_and_ld_a_r_update_xy_and_control_bits() {
        let mut z80 = Z80::new();
        let mut audio = AudioBus::new();
        let cart = dummy_cart();
        let mut work_ram = [0u8; 0x10000];
        let mut vdp = Vdp::new();
        let mut io = IoBus::new();
        z80.write_reset_byte(0x01);

        // LD A,I ; HALT
        z80.i_reg = 0x28;
        z80.f = super::FLAG_C | super::FLAG_H | super::FLAG_N;
        z80.iff2 = false;
        z80.write_ram_u8(0x0000, 0xED);
        z80.write_ram_u8(0x0001, 0x57);
        z80.write_ram_u8(0x0002, 0x76);

        z80.step(128, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
        assert_eq!(z80.unknown_opcode_total(), 0);
        assert_eq!(z80.a, 0x28);
        assert_ne!(z80.f & super::FLAG_C, 0);
        assert_eq!(z80.f & (super::FLAG_H | super::FLAG_N), 0);
        assert_eq!(z80.f & super::FLAG_PV, 0);
        assert_eq!(
            z80.f & (super::FLAG_X | super::FLAG_Y),
            super::FLAG_X | super::FLAG_Y
        );

        // LD A,R ; HALT
        z80.write_reset_byte(0x00);
        z80.write_reset_byte(0x01);
        z80.r_reg = 0x26;
        z80.f = super::FLAG_C;
        z80.iff2 = true;
        z80.write_ram_u8(0x0000, 0xED);
        z80.write_ram_u8(0x0001, 0x5F);
        z80.write_ram_u8(0x0002, 0x76);
        let expected = z80.r_reg.wrapping_add(2);

        z80.step(128, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
        assert_eq!(z80.unknown_opcode_total(), 0);
        assert_eq!(z80.a, expected);
        assert_ne!(z80.f & super::FLAG_C, 0);
        assert_eq!(z80.f & (super::FLAG_H | super::FLAG_N), 0);
        assert_ne!(z80.f & super::FLAG_PV, 0);
        assert_eq!(
            z80.f & (super::FLAG_X | super::FLAG_Y),
            expected & (super::FLAG_X | super::FLAG_Y)
        );
    }

    #[test]
    fn refresh_counter_advances_on_opcode_fetches() {
        let mut z80 = Z80::new();
        let mut audio = AudioBus::new();
        let cart = dummy_cart();
        let mut work_ram = [0u8; 0x10000];
        let mut vdp = Vdp::new();
        let mut io = IoBus::new();
        z80.write_reset_byte(0x01);

        // LD A,0x12 ; NOP ; HALT
        z80.write_ram_u8(0x0000, 0x3E);
        z80.write_ram_u8(0x0001, 0x12);
        z80.write_ram_u8(0x0002, 0x00);
        z80.write_ram_u8(0x0003, 0x76);

        // 33 M68k cycles -> 15 Z80 cycles (exactly enough for this sequence).
        z80.step(33, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
        assert_eq!(z80.r_reg & 0x7F, 3);
    }

    #[test]
    fn refresh_counter_preserves_high_bit_during_opcode_fetch() {
        let mut z80 = Z80::new();
        let mut audio = AudioBus::new();
        let cart = dummy_cart();
        let mut work_ram = [0u8; 0x10000];
        let mut vdp = Vdp::new();
        let mut io = IoBus::new();
        z80.write_reset_byte(0x01);
        z80.r_reg = 0x80;

        // NOP ; NOP ; HALT
        z80.write_ram_u8(0x0000, 0x00);
        z80.write_ram_u8(0x0001, 0x00);
        z80.write_ram_u8(0x0002, 0x76);

        // 27 M68k cycles -> 12 Z80 cycles (exactly enough for this sequence).
        z80.step(27, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
        assert_eq!(z80.r_reg, 0x83);
    }

    #[test]
    fn halt_repeats_m1_and_advances_refresh_counter_without_advancing_pc() {
        let mut z80 = Z80::new();
        let mut audio = AudioBus::new();
        let cart = dummy_cart();
        let mut work_ram = [0u8; 0x10000];
        let mut vdp = Vdp::new();
        let mut io = IoBus::new();
        z80.write_reset_byte(0x01);

        // HALT
        z80.write_ram_u8(0x0000, 0x76);
        z80.step(128, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);

        assert_eq!(z80.unknown_opcode_total(), 0);
        assert!(z80.halted);
        assert_eq!(z80.pc, 0x0001);
        // 128 M68k cycles grant 59 Z80 cycles:
        // first HALT fetch increments R once, then HALT M1 repeats for remaining cycles.
        assert_eq!(z80.r_reg & 0x7F, 14);
    }

    #[test]
    fn halt_refresh_counter_wraps_low_7_bits_and_preserves_high_bit() {
        let mut z80 = Z80::new();
        let mut audio = AudioBus::new();
        let cart = dummy_cart();
        let mut work_ram = [0u8; 0x10000];
        let mut vdp = Vdp::new();
        let mut io = IoBus::new();
        z80.write_reset_byte(0x01);
        z80.r_reg = 0xFE;

        // HALT
        z80.write_ram_u8(0x0000, 0x76);
        z80.step(64, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);

        assert_eq!(z80.unknown_opcode_total(), 0);
        assert!(z80.halted);
        assert_eq!(z80.pc, 0x0001);
        // Low 7 bits wrap, bit7 is preserved.
        assert_eq!(z80.r_reg, 0x85);
    }

    #[test]
    fn ed_undefined_opcode_behaves_as_nop_and_does_not_increment_unknown() {
        let mut z80 = Z80::new();
        let mut audio = AudioBus::new();
        let cart = dummy_cart();
        let mut work_ram = [0u8; 0x10000];
        let mut vdp = Vdp::new();
        let mut io = IoBus::new();
        z80.write_reset_byte(0x01);
        z80.write_ram_u8(0x0000, 0xED);
        z80.write_ram_u8(0x0001, 0xFF);
        z80.write_ram_u8(0x0002, 0x76);

        z80.step(64, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
        assert_eq!(z80.unknown_opcode_total(), 0);
        assert!(z80.halted);
        assert_eq!(z80.pc, 0x0003);
    }

    #[test]
    fn all_ed_prefixed_opcodes_do_not_increment_unknown_counter() {
        for op in 0u16..=0xFF {
            let mut z80 = Z80::new();
            let mut audio = AudioBus::new();
            let cart = dummy_cart();
            let mut work_ram = [0u8; 0x10000];
            let mut vdp = Vdp::new();
            let mut io = IoBus::new();
            z80.write_reset_byte(0x01);

            // ED op ; HALT
            z80.write_ram_u8(0x0000, 0xED);
            z80.write_ram_u8(0x0001, op as u8);
            z80.write_ram_u8(0x0002, 0x76);

            z80.step(96, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
            assert_eq!(
                z80.unknown_opcode_total(),
                0,
                "ED sub-opcode {:02X} should not be unknown",
                op
            );
        }
    }

    #[test]
    fn all_base_opcodes_do_not_increment_unknown_counter() {
        for op in 0u16..=0xFF {
            let mut z80 = Z80::new();
            let mut audio = AudioBus::new();
            let cart = dummy_cart();
            let mut work_ram = [0u8; 0x10000];
            let mut vdp = Vdp::new();
            let mut io = IoBus::new();
            z80.write_reset_byte(0x01);

            // opcode ; filler for immediate/displacement consumers ; HALT
            z80.write_ram_u8(0x0000, op as u8);
            z80.write_ram_u8(0x0001, 0x00);
            z80.write_ram_u8(0x0002, 0x00);
            z80.write_ram_u8(0x0003, 0x00);
            z80.write_ram_u8(0x0004, 0x76);

            z80.step(160, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
            assert_eq!(
                z80.unknown_opcode_total(),
                0,
                "base opcode {:02X} should not be unknown",
                op
            );
        }
    }

    #[test]
    fn all_dd_fd_prefixed_second_bytes_do_not_increment_unknown_counter() {
        for &prefix in &[0xDDu8, 0xFDu8] {
            for op in 0u16..=0xFF {
                let mut z80 = Z80::new();
                let mut audio = AudioBus::new();
                let cart = dummy_cart();
                let mut work_ram = [0u8; 0x10000];
                let mut vdp = Vdp::new();
                let mut io = IoBus::new();
                z80.write_reset_byte(0x01);

                // prefix op d op3 ; HALT
                // The trailing bytes satisfy forms that need displacement/immediate
                // (including prefix CB d op3) while remaining harmless otherwise.
                z80.write_ram_u8(0x0000, prefix);
                z80.write_ram_u8(0x0001, op as u8);
                z80.write_ram_u8(0x0002, 0x00);
                z80.write_ram_u8(0x0003, 0x00);
                z80.write_ram_u8(0x0004, 0x76);

                z80.step(160, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
                assert_eq!(
                    z80.unknown_opcode_total(),
                    0,
                    "{:02X} sub-opcode {:02X} should not be unknown",
                    prefix,
                    op
                );
            }
        }
    }

    #[test]
    fn inc_sp_opcode_is_implemented() {
        let mut z80 = Z80::new();
        let mut audio = AudioBus::new();
        let cart = dummy_cart();
        let mut work_ram = [0u8; 0x10000];
        let mut vdp = Vdp::new();
        let mut io = IoBus::new();
        z80.write_reset_byte(0x01);
        z80.sp = 0x1234;

        // INC SP ; HALT
        z80.write_ram_u8(0x0000, 0x33);
        z80.write_ram_u8(0x0001, 0x76);

        z80.step(64, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
        assert_eq!(z80.unknown_opcode_total(), 0);
        assert_eq!(z80.sp, 0x1235);
    }

    #[test]
    fn dd_prefix_before_inc_sp_is_ignored_and_executes_inc_sp() {
        let mut z80 = Z80::new();
        let mut audio = AudioBus::new();
        let cart = dummy_cart();
        let mut work_ram = [0u8; 0x10000];
        let mut vdp = Vdp::new();
        let mut io = IoBus::new();
        z80.write_reset_byte(0x01);
        z80.sp = 0x00FF;

        // DD ; INC SP ; HALT
        z80.write_ram_u8(0x0000, 0xDD);
        z80.write_ram_u8(0x0001, 0x33);
        z80.write_ram_u8(0x0002, 0x76);

        z80.step(96, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
        assert_eq!(z80.unknown_opcode_total(), 0);
        assert_eq!(z80.sp, 0x0100);
        assert_eq!(z80.pc, 0x0003);
    }

    #[test]
    fn dd_prefix_is_ignored_for_non_indexed_opcode() {
        let mut z80 = Z80::new();
        let mut audio = AudioBus::new();
        let cart = dummy_cart();
        let mut work_ram = [0u8; 0x10000];
        let mut vdp = Vdp::new();
        let mut io = IoBus::new();
        z80.write_reset_byte(0x01);

        // DD ; NOP ; HALT
        z80.write_ram_u8(0x0000, 0xDD);
        z80.write_ram_u8(0x0001, 0x00);
        z80.write_ram_u8(0x0002, 0x76);

        // 27 M68k cycles -> 12 Z80 cycles (exactly enough for DD NOP HALT).
        z80.step(27, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
        assert_eq!(z80.unknown_opcode_total(), 0);
        assert_eq!(z80.r_reg & 0x7F, 3);
    }

    #[test]
    fn dd_prefix_before_ed_executes_ed_opcode_normally() {
        let mut z80 = Z80::new();
        let mut audio = AudioBus::new();
        let cart = dummy_cart();
        let mut work_ram = [0u8; 0x10000];
        let mut vdp = Vdp::new();
        let mut io = IoBus::new();
        z80.write_reset_byte(0x01);
        z80.a = 0x5A;

        // DD ; ED 47 (LD I,A) ; HALT
        z80.write_ram_u8(0x0000, 0xDD);
        z80.write_ram_u8(0x0001, 0xED);
        z80.write_ram_u8(0x0002, 0x47);
        z80.write_ram_u8(0x0003, 0x76);

        z80.step(192, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
        assert_eq!(z80.unknown_opcode_total(), 0);
        assert_eq!(z80.i_reg, 0x5A);
    }

    #[test]
    fn repeated_dd_prefix_uses_last_prefix_and_does_not_mark_unknown() {
        let mut z80 = Z80::new();
        let mut audio = AudioBus::new();
        let cart = dummy_cart();
        let mut work_ram = [0u8; 0x10000];
        let mut vdp = Vdp::new();
        let mut io = IoBus::new();
        z80.write_reset_byte(0x01);

        // DD ; DD ; LD IX,0x1234 ; HALT
        let program = [0xDD, 0xDD, 0x21, 0x34, 0x12, 0x76];
        for (i, byte) in program.iter().enumerate() {
            z80.write_ram_u8(i as u16, *byte);
        }

        z80.step(256, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
        assert_eq!(z80.unknown_opcode_total(), 0);
        assert_eq!(z80.ix, 0x1234);
    }

    #[test]
    fn ed_ld_mem_sp_and_ld_sp_mem_are_implemented() {
        let mut z80 = Z80::new();
        let mut audio = AudioBus::new();
        let cart = dummy_cart();
        let mut work_ram = [0u8; 0x10000];
        let mut vdp = Vdp::new();
        let mut io = IoBus::new();
        z80.write_reset_byte(0x01);
        z80.sp = 0xBEEF;

        // LD (0x1234),SP ; LD SP,(0x1234) ; HALT
        let program = [0xED, 0x73, 0x34, 0x12, 0xED, 0x7B, 0x34, 0x12, 0x76];
        for (i, byte) in program.iter().enumerate() {
            z80.write_ram_u8(i as u16, *byte);
        }

        z80.step(256, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
        assert_eq!(z80.unknown_opcode_total(), 0);
        assert_eq!(z80.read_ram_u8(0x1234), 0xEF);
        assert_eq!(z80.read_ram_u8(0x1235), 0xBE);
        assert_eq!(z80.sp, 0xBEEF);
    }

    #[test]
    fn ed_ld_mem_hl_and_ld_hl_mem_are_implemented() {
        let mut z80 = Z80::new();
        let mut audio = AudioBus::new();
        let cart = dummy_cart();
        let mut work_ram = [0u8; 0x10000];
        let mut vdp = Vdp::new();
        let mut io = IoBus::new();
        z80.write_reset_byte(0x01);
        z80.set_hl(0xCAFE);

        // LD (0x1234),HL ; LD HL,(0x1234) ; HALT
        let program = [0xED, 0x63, 0x34, 0x12, 0xED, 0x6B, 0x34, 0x12, 0x76];
        for (i, byte) in program.iter().enumerate() {
            z80.write_ram_u8(i as u16, *byte);
        }

        z80.step(256, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
        assert_eq!(z80.unknown_opcode_total(), 0);
        assert_eq!(z80.read_ram_u8(0x1234), 0xFE);
        assert_eq!(z80.read_ram_u8(0x1235), 0xCA);
        assert_eq!(z80.hl(), 0xCAFE);
    }

    #[test]
    fn ed_retn_alias_restores_iff_and_returns() {
        let mut z80 = Z80::new();
        let mut audio = AudioBus::new();
        let cart = dummy_cart();
        let mut work_ram = [0u8; 0x10000];
        let mut vdp = Vdp::new();
        let mut io = IoBus::new();
        z80.write_reset_byte(0x01);
        z80.iff2 = true;
        z80.sp = 0x0100;
        z80.write_ram_u8(0x0100, 0x34);
        z80.write_ram_u8(0x0101, 0x12);

        // RETN alias: ED 55 ; HALT (at return target)
        z80.write_ram_u8(0x0000, 0xED);
        z80.write_ram_u8(0x0001, 0x55);
        z80.write_ram_u8(0x1234, 0x76);

        z80.step(256, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
        assert_eq!(z80.unknown_opcode_total(), 0);
        assert_eq!(z80.pc, 0x1235);
        assert!(z80.iff1);
    }

    #[test]
    fn ed_neg_opcode_is_implemented() {
        let mut z80 = Z80::new();
        let mut audio = AudioBus::new();
        let cart = dummy_cart();
        let mut work_ram = [0u8; 0x10000];
        let mut vdp = Vdp::new();
        let mut io = IoBus::new();
        z80.write_reset_byte(0x01);
        z80.a = 0x01;
        // NEG ; HALT
        z80.write_ram_u8(0x0000, 0xED);
        z80.write_ram_u8(0x0001, 0x44);
        z80.write_ram_u8(0x0002, 0x76);

        z80.step(64, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
        assert_eq!(z80.unknown_opcode_total(), 0);
        assert_eq!(z80.a, 0xFF);
        assert_eq!(z80.f & super::FLAG_Z, 0);
        assert_ne!(z80.f & super::FLAG_S, 0);
        assert_ne!(z80.f & super::FLAG_C, 0);
    }

    #[test]
    fn or_a_updates_flags_and_is_not_unknown() {
        let mut z80 = Z80::new();
        let mut audio = AudioBus::new();
        let cart = dummy_cart();
        let mut work_ram = [0u8; 0x10000];
        let mut vdp = Vdp::new();
        let mut io = IoBus::new();
        z80.write_reset_byte(0x01);
        // xor a ; or a ; halt
        z80.write_ram_u8(0x0000, 0xAF);
        z80.write_ram_u8(0x0001, 0xB7);
        z80.write_ram_u8(0x0002, 0x76);

        z80.step(64, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
        assert_eq!(z80.unknown_opcode_total(), 0);
        assert_eq!(z80.a, 0);
        assert_ne!(z80.f & super::FLAG_Z, 0);
        assert_eq!(z80.f & super::FLAG_C, 0);
    }

    #[test]
    fn djnz_and_ret_nz_execute_control_flow() {
        let mut z80 = Z80::new();
        let mut audio = AudioBus::new();
        let cart = dummy_cart();
        let mut work_ram = [0u8; 0x10000];
        let mut vdp = Vdp::new();
        let mut io = IoBus::new();
        z80.write_reset_byte(0x01);

        // ld b,3
        z80.write_ram_u8(0x0000, 0x06);
        z80.write_ram_u8(0x0001, 0x03);
        // ld a,0
        z80.write_ram_u8(0x0002, 0x3E);
        z80.write_ram_u8(0x0003, 0x00);
        // add a,1
        z80.write_ram_u8(0x0004, 0xC6);
        z80.write_ram_u8(0x0005, 0x01);
        // djnz -4 (to add a,1)
        z80.write_ram_u8(0x0006, 0x10);
        z80.write_ram_u8(0x0007, 0xFC);
        // call 0x0010
        z80.write_ram_u8(0x0008, 0xCD);
        z80.write_ram_u8(0x0009, 0x10);
        z80.write_ram_u8(0x000A, 0x00);
        // halt
        z80.write_ram_u8(0x000B, 0x76);
        // subroutine @0x0010: or a ; ret nz
        z80.write_ram_u8(0x0010, 0xB7);
        z80.write_ram_u8(0x0011, 0xC0);
        // halt (should not reach)
        z80.write_ram_u8(0x0012, 0x76);

        z80.step(512, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
        assert_eq!(z80.unknown_opcode_total(), 0);
        assert_eq!(z80.a, 3);
        assert_eq!(z80.pc, 0x000C);
    }

    #[test]
    fn conditional_call_c_and_ret_c_execute_control_flow() {
        let mut z80 = Z80::new();
        let mut audio = AudioBus::new();
        let cart = dummy_cart();
        let mut work_ram = [0u8; 0x10000];
        let mut vdp = Vdp::new();
        let mut io = IoBus::new();
        z80.write_reset_byte(0x01);

        // ld a,0 ; sub 1 ; call c,0x0010 ; halt
        z80.write_ram_u8(0x0000, 0x3E);
        z80.write_ram_u8(0x0001, 0x00);
        z80.write_ram_u8(0x0002, 0xD6);
        z80.write_ram_u8(0x0003, 0x01);
        z80.write_ram_u8(0x0004, 0xDC);
        z80.write_ram_u8(0x0005, 0x10);
        z80.write_ram_u8(0x0006, 0x00);
        z80.write_ram_u8(0x0007, 0x76);

        // subroutine @0x0010: ld b,0x42 ; ret c
        z80.write_ram_u8(0x0010, 0x06);
        z80.write_ram_u8(0x0011, 0x42);
        z80.write_ram_u8(0x0012, 0xD8);
        z80.write_ram_u8(0x0013, 0x76);

        z80.step(512, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
        assert_eq!(z80.unknown_opcode_total(), 0);
        assert_eq!(z80.b, 0x42);
        assert_eq!(z80.pc, 0x0008);
    }

    #[test]
    fn conditional_call_nc_and_call_p_are_implemented() {
        let mut z80 = Z80::new();
        let mut audio = AudioBus::new();
        let cart = dummy_cart();
        let mut work_ram = [0u8; 0x10000];
        let mut vdp = Vdp::new();
        let mut io = IoBus::new();
        z80.write_reset_byte(0x01);

        // xor a ; call nc,0x0010 ; call p,0x0020 ; halt
        z80.write_ram_u8(0x0000, 0xAF);
        z80.write_ram_u8(0x0001, 0xD4);
        z80.write_ram_u8(0x0002, 0x10);
        z80.write_ram_u8(0x0003, 0x00);
        z80.write_ram_u8(0x0004, 0xF4);
        z80.write_ram_u8(0x0005, 0x20);
        z80.write_ram_u8(0x0006, 0x00);
        z80.write_ram_u8(0x0007, 0x76);

        // @0x0010: ld b,0x11 ; ret
        z80.write_ram_u8(0x0010, 0x06);
        z80.write_ram_u8(0x0011, 0x11);
        z80.write_ram_u8(0x0012, 0xC9);
        // @0x0020: ld c,0x22 ; ret
        z80.write_ram_u8(0x0020, 0x0E);
        z80.write_ram_u8(0x0021, 0x22);
        z80.write_ram_u8(0x0022, 0xC9);

        z80.step(512, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
        assert_eq!(z80.unknown_opcode_total(), 0);
        assert_eq!(z80.b, 0x11);
        assert_eq!(z80.c, 0x22);
        assert_eq!(z80.pc, 0x0008);
    }

    #[test]
    fn parity_condition_jp_call_and_ret_are_implemented() {
        let mut z80 = Z80::new();
        let mut audio = AudioBus::new();
        let cart = dummy_cart();
        let mut work_ram = [0u8; 0x10000];
        let mut vdp = Vdp::new();
        let mut io = IoBus::new();
        z80.write_reset_byte(0x01);

        // Stack seed for POP AF (A=0x00, F=0x04 => PV=1).
        z80.sp = 0x0100;
        z80.write_ram_u8(0x0100, super::FLAG_PV);
        z80.write_ram_u8(0x0101, 0x00);

        // pop af ; call po,0x0030 ; call pe,0x0040 ; jp po,0x0010 ; jp pe,0x0020 ; halt
        let program = [
            0xF1, 0xE4, 0x30, 0x00, 0xEC, 0x40, 0x00, 0xE2, 0x10, 0x00, 0xEA, 0x20, 0x00, 0x76,
        ];
        for (i, byte) in program.iter().enumerate() {
            z80.write_ram_u8(i as u16, *byte);
        }

        // Should be skipped (JP PO / CALL PO not taken with PV=1).
        z80.write_ram_u8(0x0010, 0x76);
        z80.write_ram_u8(0x0030, 0x76);

        // JP PE target: execute payload and halt.
        z80.write_ram_u8(0x0020, 0x0E); // LD C,0x22
        z80.write_ram_u8(0x0021, 0x22);
        z80.write_ram_u8(0x0022, 0x76);

        // Subroutine @0x0040: RET PO (not taken) ; LD B,0x44 ; RET PE (taken)
        z80.write_ram_u8(0x0040, 0xE0);
        z80.write_ram_u8(0x0041, 0x06);
        z80.write_ram_u8(0x0042, 0x44);
        z80.write_ram_u8(0x0043, 0xE8);
        z80.write_ram_u8(0x0044, 0x76);

        z80.step(512, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
        assert_eq!(z80.unknown_opcode_total(), 0);
        assert_eq!(z80.b, 0x44);
        assert_eq!(z80.c, 0x22);
        assert_eq!(z80.pc, 0x0023);
    }

    #[test]
    fn add_and_sub_set_overflow_parity_flag() {
        let mut z80 = Z80::new();
        let mut audio = AudioBus::new();
        let cart = dummy_cart();
        let mut work_ram = [0u8; 0x10000];
        let mut vdp = Vdp::new();
        let mut io = IoBus::new();
        z80.write_reset_byte(0x01);

        // LD A,0x7F ; ADD A,0x01 ; HALT
        let add_prog = [0x3E, 0x7F, 0xC6, 0x01, 0x76];
        for (i, byte) in add_prog.iter().enumerate() {
            z80.write_ram_u8(i as u16, *byte);
        }
        z80.step(128, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
        assert_eq!(z80.a, 0x80);
        assert_ne!(z80.f & super::FLAG_PV, 0);
        assert_ne!(z80.f & super::FLAG_S, 0);
        assert_eq!(z80.f & super::FLAG_N, 0);

        z80.write_reset_byte(0x00);
        z80.write_reset_byte(0x01);
        // LD A,0x80 ; SUB 0x01 ; HALT
        let sub_prog = [0x3E, 0x80, 0xD6, 0x01, 0x76];
        for (i, byte) in sub_prog.iter().enumerate() {
            z80.write_ram_u8(i as u16, *byte);
        }
        z80.step(128, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
        assert_eq!(z80.a, 0x7F);
        assert_ne!(z80.f & super::FLAG_PV, 0);
        assert_ne!(z80.f & super::FLAG_N, 0);
        assert_eq!(z80.f & super::FLAG_S, 0);
    }

    #[test]
    fn adc_and_sbc_with_carry_in_have_correct_overflow_flag() {
        let mut z80 = Z80::new();
        let mut audio = AudioBus::new();
        let cart = dummy_cart();
        let mut work_ram = [0u8; 0x10000];
        let mut vdp = Vdp::new();
        let mut io = IoBus::new();
        z80.write_reset_byte(0x01);

        // SCF ; LD A,0x00 ; ADC A,0x7F ; HALT
        // 0 + 127 + 1 = -128 (signed overflow set)
        let adc_prog = [0x37, 0x3E, 0x00, 0xCE, 0x7F, 0x76];
        for (i, byte) in adc_prog.iter().enumerate() {
            z80.write_ram_u8(i as u16, *byte);
        }
        z80.step(256, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
        assert_eq!(z80.unknown_opcode_total(), 0);
        assert_eq!(z80.a, 0x80);
        assert_ne!(z80.f & super::FLAG_PV, 0);
        assert_ne!(z80.f & super::FLAG_S, 0);

        z80.write_reset_byte(0x00);
        z80.write_reset_byte(0x01);

        // SCF ; LD A,0x00 ; SBC A,0x7F ; HALT
        // 0 - 127 - 1 = -128 (signed overflow clear)
        let sbc_prog = [0x37, 0x3E, 0x00, 0xDE, 0x7F, 0x76];
        for (i, byte) in sbc_prog.iter().enumerate() {
            z80.write_ram_u8(i as u16, *byte);
        }
        z80.step(256, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
        assert_eq!(z80.unknown_opcode_total(), 0);
        assert_eq!(z80.a, 0x80);
        assert_eq!(z80.f & super::FLAG_PV, 0);
        assert_ne!(z80.f & super::FLAG_S, 0);
    }

    #[test]
    fn and_sets_halfcarry_and_parity_flags() {
        let mut z80 = Z80::new();
        let mut audio = AudioBus::new();
        let cart = dummy_cart();
        let mut work_ram = [0u8; 0x10000];
        let mut vdp = Vdp::new();
        let mut io = IoBus::new();
        z80.write_reset_byte(0x01);

        // LD A,0xF0 ; AND 0x0F ; HALT
        let prog = [0x3E, 0xF0, 0xE6, 0x0F, 0x76];
        for (i, byte) in prog.iter().enumerate() {
            z80.write_ram_u8(i as u16, *byte);
        }
        z80.step(128, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
        assert_eq!(z80.a, 0x00);
        assert_ne!(z80.f & super::FLAG_Z, 0);
        assert_ne!(z80.f & super::FLAG_H, 0);
        assert_ne!(z80.f & super::FLAG_PV, 0);
        assert_eq!(z80.f & (super::FLAG_N | super::FLAG_C), 0);
    }

    #[test]
    fn inc_dec_and_bit_update_pv_related_flags() {
        let mut z80 = Z80::new();
        let mut audio = AudioBus::new();
        let cart = dummy_cart();
        let mut work_ram = [0u8; 0x10000];
        let mut vdp = Vdp::new();
        let mut io = IoBus::new();
        z80.write_reset_byte(0x01);

        // SCF ; LD B,0x7F ; INC B ; HALT
        let inc_prog = [0x37, 0x06, 0x7F, 0x04, 0x76];
        for (i, byte) in inc_prog.iter().enumerate() {
            z80.write_ram_u8(i as u16, *byte);
        }
        z80.step(128, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
        assert_eq!(z80.b, 0x80);
        assert_ne!(z80.f & super::FLAG_C, 0);
        assert_ne!(z80.f & super::FLAG_H, 0);
        assert_ne!(z80.f & super::FLAG_PV, 0);
        assert_eq!(z80.f & super::FLAG_N, 0);

        z80.write_reset_byte(0x00);
        z80.write_reset_byte(0x01);
        // SCF ; LD C,0x80 ; DEC C ; HALT
        let dec_prog = [0x37, 0x0E, 0x80, 0x0D, 0x76];
        for (i, byte) in dec_prog.iter().enumerate() {
            z80.write_ram_u8(i as u16, *byte);
        }
        z80.step(128, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
        assert_eq!(z80.c, 0x7F);
        assert_ne!(z80.f & super::FLAG_C, 0);
        assert_ne!(z80.f & super::FLAG_H, 0);
        assert_ne!(z80.f & super::FLAG_PV, 0);
        assert_ne!(z80.f & super::FLAG_N, 0);

        z80.write_reset_byte(0x00);
        z80.write_reset_byte(0x01);
        // SCF ; LD B,0x00 ; BIT 0,B ; HALT
        let bit_prog = [0x37, 0x06, 0x00, 0xCB, 0x40, 0x76];
        for (i, byte) in bit_prog.iter().enumerate() {
            z80.write_ram_u8(i as u16, *byte);
        }
        z80.step(128, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
        assert_ne!(z80.f & super::FLAG_C, 0);
        assert_ne!(z80.f & super::FLAG_H, 0);
        assert_ne!(z80.f & super::FLAG_Z, 0);
        assert_ne!(z80.f & super::FLAG_PV, 0);
        assert_eq!(z80.f & super::FLAG_N, 0);
    }

    #[test]
    fn bit_hl_uses_h_for_xy_flags() {
        let mut z80 = Z80::new();
        let mut audio = AudioBus::new();
        let cart = dummy_cart();
        let mut work_ram = [0u8; 0x10000];
        let mut vdp = Vdp::new();
        let mut io = IoBus::new();
        z80.write_reset_byte(0x01);
        z80.set_hl(0x2810);
        z80.write_ram_u8(0x0810, 0x00);

        // SCF ; BIT 0,(HL) ; HALT
        let prog = [0x37, 0xCB, 0x46, 0x76];
        for (i, byte) in prog.iter().enumerate() {
            z80.write_ram_u8(i as u16, *byte);
        }
        z80.step(128, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
        assert_ne!(z80.f & super::FLAG_C, 0);
        assert_ne!(z80.f & super::FLAG_H, 0);
        assert_ne!(z80.f & super::FLAG_Z, 0);
        assert_ne!(z80.f & super::FLAG_PV, 0);
        assert_eq!(
            z80.f & (super::FLAG_X | super::FLAG_Y),
            super::FLAG_X | super::FLAG_Y
        );
    }

    #[test]
    fn undocumented_xy_flags_follow_alu_and_bit_results() {
        let mut z80 = Z80::new();
        let mut audio = AudioBus::new();
        let cart = dummy_cart();
        let mut work_ram = [0u8; 0x10000];
        let mut vdp = Vdp::new();
        let mut io = IoBus::new();
        z80.write_reset_byte(0x01);

        // LD A,0 ; OR 0x28 ; HALT
        let or_prog = [0x3E, 0x00, 0xF6, 0x28, 0x76];
        for (i, byte) in or_prog.iter().enumerate() {
            z80.write_ram_u8(i as u16, *byte);
        }
        z80.step(128, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
        assert_eq!(z80.a, 0x28);
        assert_eq!(
            z80.f & (super::FLAG_X | super::FLAG_Y),
            super::FLAG_X | super::FLAG_Y
        );

        z80.write_reset_byte(0x00);
        z80.write_reset_byte(0x01);

        // SCF ; LD B,0x28 ; BIT 0,B ; HALT
        let bit_prog = [0x37, 0x06, 0x28, 0xCB, 0x40, 0x76];
        for (i, byte) in bit_prog.iter().enumerate() {
            z80.write_ram_u8(i as u16, *byte);
        }
        z80.step(128, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
        assert_ne!(z80.f & super::FLAG_C, 0);
        assert_ne!(z80.f & super::FLAG_H, 0);
        assert_ne!(z80.f & super::FLAG_Z, 0);
        assert_ne!(z80.f & super::FLAG_PV, 0);
        assert_eq!(
            z80.f & (super::FLAG_X | super::FLAG_Y),
            super::FLAG_X | super::FLAG_Y
        );
    }

    #[test]
    fn add_hl_sets_xy_from_result_high_byte() {
        let mut z80 = Z80::new();
        let mut audio = AudioBus::new();
        let cart = dummy_cart();
        let mut work_ram = [0u8; 0x10000];
        let mut vdp = Vdp::new();
        let mut io = IoBus::new();
        z80.write_reset_byte(0x01);

        // LD HL,0x1000 ; LD BC,0x1800 ; ADD HL,BC ; HALT  => HL=0x2800 (high=0x28)
        let prog = [0x21, 0x00, 0x10, 0x01, 0x00, 0x18, 0x09, 0x76];
        for (i, byte) in prog.iter().enumerate() {
            z80.write_ram_u8(i as u16, *byte);
        }
        z80.step(256, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
        assert_eq!(z80.hl(), 0x2800);
        assert_eq!(
            z80.f & (super::FLAG_X | super::FLAG_Y),
            super::FLAG_X | super::FLAG_Y
        );
    }

    #[test]
    fn add_hl_preserves_szpv_and_sets_halfcarry() {
        let mut z80 = Z80::new();
        let mut audio = AudioBus::new();
        let cart = dummy_cart();
        let mut work_ram = [0u8; 0x10000];
        let mut vdp = Vdp::new();
        let mut io = IoBus::new();
        z80.write_reset_byte(0x01);

        // XOR A ; LD HL,0x0FFF ; LD BC,0x0001 ; ADD HL,BC ; HALT
        let prog = [0xAF, 0x21, 0xFF, 0x0F, 0x01, 0x01, 0x00, 0x09, 0x76];
        for (i, byte) in prog.iter().enumerate() {
            z80.write_ram_u8(i as u16, *byte);
        }
        z80.step(256, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);

        assert_eq!(z80.hl(), 0x1000);
        assert_ne!(z80.f & super::FLAG_Z, 0);
        assert_ne!(z80.f & super::FLAG_PV, 0);
        assert_ne!(z80.f & super::FLAG_H, 0);
        assert_eq!(z80.f & super::FLAG_N, 0);
        assert_eq!(z80.f & super::FLAG_C, 0);
    }

    #[test]
    fn adc_hl_and_sbc_hl_set_extended_flags() {
        let mut z80 = Z80::new();
        let mut audio = AudioBus::new();
        let cart = dummy_cart();
        let mut work_ram = [0u8; 0x10000];
        let mut vdp = Vdp::new();
        let mut io = IoBus::new();
        z80.write_reset_byte(0x01);

        // SCF ; LD HL,0x7FFF ; LD BC,0 ; ADC HL,BC ; HALT
        let adc_prog = [0x37, 0x21, 0xFF, 0x7F, 0x01, 0x00, 0x00, 0xED, 0x4A, 0x76];
        for (i, byte) in adc_prog.iter().enumerate() {
            z80.write_ram_u8(i as u16, *byte);
        }
        z80.step(256, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
        assert_eq!(z80.hl(), 0x8000);
        assert_ne!(z80.f & super::FLAG_S, 0);
        assert_ne!(z80.f & super::FLAG_PV, 0);
        assert_ne!(z80.f & super::FLAG_H, 0);
        assert_eq!(z80.f & super::FLAG_N, 0);
        assert_eq!(z80.f & super::FLAG_C, 0);

        z80.write_reset_byte(0x00);
        z80.write_reset_byte(0x01);
        // SCF ; LD HL,0x8000 ; LD BC,0 ; SBC HL,BC ; HALT
        let sbc_prog = [0x37, 0x21, 0x00, 0x80, 0x01, 0x00, 0x00, 0xED, 0x42, 0x76];
        for (i, byte) in sbc_prog.iter().enumerate() {
            z80.write_ram_u8(i as u16, *byte);
        }
        z80.step(256, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
        assert_eq!(z80.hl(), 0x7FFF);
        assert_eq!(z80.f & super::FLAG_S, 0);
        assert_ne!(z80.f & super::FLAG_PV, 0);
        assert_ne!(z80.f & super::FLAG_H, 0);
        assert_ne!(z80.f & super::FLAG_N, 0);
        assert_eq!(z80.f & super::FLAG_C, 0);
    }

    #[test]
    fn rotate_a_instructions_preserve_pv() {
        let mut z80 = Z80::new();
        let mut audio = AudioBus::new();
        let cart = dummy_cart();
        let mut work_ram = [0u8; 0x10000];
        let mut vdp = Vdp::new();
        let mut io = IoBus::new();
        z80.write_reset_byte(0x01);

        // XOR A (PV=1, Z=1) ; LD A,0x80 ; RLCA ; HALT
        let prog = [0xAF, 0x3E, 0x80, 0x07, 0x76];
        for (i, byte) in prog.iter().enumerate() {
            z80.write_ram_u8(i as u16, *byte);
        }
        z80.step(128, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
        assert_eq!(z80.a, 0x01);
        assert_ne!(z80.f & super::FLAG_C, 0);
        assert_ne!(z80.f & super::FLAG_PV, 0);
        assert_ne!(z80.f & super::FLAG_Z, 0);
    }

    #[test]
    fn misc_flag_and_sp_ops_are_implemented() {
        let mut z80 = Z80::new();
        let mut audio = AudioBus::new();
        let cart = dummy_cart();
        let mut work_ram = [0u8; 0x10000];
        let mut vdp = Vdp::new();
        let mut io = IoBus::new();
        z80.write_reset_byte(0x01);

        // LD HL,0x1234 ; LD SP,HL ; LD A,0x09 ; ADD A,0x01 ; DAA ; SCF ; CCF ; HALT
        let program = [
            0x21, 0x34, 0x12, 0xF9, 0x3E, 0x09, 0xC6, 0x01, 0x27, 0x37, 0x3F, 0x76,
        ];
        for (i, byte) in program.iter().enumerate() {
            z80.write_ram_u8(i as u16, *byte);
        }

        z80.step(256, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
        assert_eq!(z80.unknown_opcode_total(), 0);
        assert_eq!(z80.sp, 0x1234);
        assert_eq!(z80.a, 0x10);
        assert_eq!(z80.f & super::FLAG_C, 0);
    }

    #[test]
    fn daa_handles_subtraction_path() {
        let mut z80 = Z80::new();
        let mut audio = AudioBus::new();
        let cart = dummy_cart();
        let mut work_ram = [0u8; 0x10000];
        let mut vdp = Vdp::new();
        let mut io = IoBus::new();
        z80.write_reset_byte(0x01);

        // LD A,0x10 ; SUB 0x01 ; DAA ; HALT  => 0x09
        let program = [0x3E, 0x10, 0xD6, 0x01, 0x27, 0x76];
        for (i, byte) in program.iter().enumerate() {
            z80.write_ram_u8(i as u16, *byte);
        }
        z80.step(256, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
        assert_eq!(z80.unknown_opcode_total(), 0);
        assert_eq!(z80.a, 0x09);
        assert_eq!(z80.f & super::FLAG_C, 0);
        assert_ne!(z80.f & super::FLAG_PV, 0);

        // LD A,0x00 ; SUB 0x01 ; DAA ; HALT => 0x99 with carry.
        z80.write_reset_byte(0x00);
        z80.write_reset_byte(0x01);
        let program = [0x3E, 0x00, 0xD6, 0x01, 0x27, 0x76];
        for (i, byte) in program.iter().enumerate() {
            z80.write_ram_u8(i as u16, *byte);
        }
        z80.step(256, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
        assert_eq!(z80.unknown_opcode_total(), 0);
        assert_eq!(z80.a, 0x99);
        assert_ne!(z80.f & super::FLAG_C, 0);
        assert_ne!(z80.f & super::FLAG_PV, 0);
    }

    #[test]
    fn daa_handles_addition_carry_and_xy_cases() {
        let mut z80 = Z80::new();
        let mut audio = AudioBus::new();
        let cart = dummy_cart();
        let mut work_ram = [0u8; 0x10000];
        let mut vdp = Vdp::new();
        let mut io = IoBus::new();
        z80.write_reset_byte(0x01);

        // 0x15 + 0x27 = 0x42 (BCD, no carry)
        let program = [0x3E, 0x15, 0xC6, 0x27, 0x27, 0x76];
        for (i, byte) in program.iter().enumerate() {
            z80.write_ram_u8(i as u16, *byte);
        }
        z80.step(256, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
        assert_eq!(z80.unknown_opcode_total(), 0);
        assert_eq!(z80.a, 0x42);
        assert_eq!(z80.f & super::FLAG_C, 0);
        assert_eq!(z80.f & super::FLAG_N, 0);
        assert_eq!(z80.f & (super::FLAG_X | super::FLAG_Y), 0x00);

        // 0x99 + 0x01 = 0x00 with decimal carry.
        z80.write_reset_byte(0x00);
        z80.write_reset_byte(0x01);
        let program = [0x3E, 0x99, 0xC6, 0x01, 0x27, 0x76];
        for (i, byte) in program.iter().enumerate() {
            z80.write_ram_u8(i as u16, *byte);
        }
        z80.step(256, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
        assert_eq!(z80.unknown_opcode_total(), 0);
        assert_eq!(z80.a, 0x00);
        assert_ne!(z80.f & super::FLAG_C, 0);
        assert_ne!(z80.f & super::FLAG_Z, 0);
        assert_ne!(z80.f & super::FLAG_PV, 0);
        assert_eq!(z80.f & (super::FLAG_X | super::FLAG_Y), 0x00);

        // 0x08 + 0x08 = 0x16: verifies H-driven adjust path.
        z80.write_reset_byte(0x00);
        z80.write_reset_byte(0x01);
        let program = [0x3E, 0x08, 0xC6, 0x08, 0x27, 0x76];
        for (i, byte) in program.iter().enumerate() {
            z80.write_ram_u8(i as u16, *byte);
        }
        z80.step(256, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
        assert_eq!(z80.unknown_opcode_total(), 0);
        assert_eq!(z80.a, 0x16);
        assert_eq!(z80.f & super::FLAG_C, 0);
        assert_eq!(z80.f & super::FLAG_N, 0);
    }

    #[test]
    fn index_ex_sp_and_jp_index_are_implemented() {
        let mut z80 = Z80::new();
        let mut audio = AudioBus::new();
        let cart = dummy_cart();
        let mut work_ram = [0u8; 0x10000];
        let mut vdp = Vdp::new();
        let mut io = IoBus::new();
        z80.write_reset_byte(0x01);
        z80.ix = 0x0200;
        z80.sp = 0x0100;
        z80.write_ram_u8(0x0100, 0x34);
        z80.write_ram_u8(0x0101, 0x12);

        // EX (SP),IX ; JP (IX)
        z80.write_ram_u8(0x0000, 0xDD);
        z80.write_ram_u8(0x0001, 0xE3);
        z80.write_ram_u8(0x0002, 0xDD);
        z80.write_ram_u8(0x0003, 0xE9);
        z80.write_ram_u8(0x1234, 0x76); // HALT at jump target

        z80.step(256, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
        assert_eq!(z80.unknown_opcode_total(), 0);
        assert_eq!(z80.ix, 0x1234);
        assert_eq!(z80.read_ram_u8(0x0100), 0x00);
        assert_eq!(z80.read_ram_u8(0x0101), 0x02);
        assert_eq!(z80.pc, 0x1235);
    }

    #[test]
    fn pop_af_restores_accumulator_and_flags() {
        let mut z80 = Z80::new();
        let mut audio = AudioBus::new();
        let cart = dummy_cart();
        let mut work_ram = [0u8; 0x10000];
        let mut vdp = Vdp::new();
        let mut io = IoBus::new();
        z80.write_reset_byte(0x01);

        // Seed stack with AF value 0xAA45 and execute POP AF.
        z80.sp = 0x0100;
        z80.write_ram_u8(0x0100, 0x45);
        z80.write_ram_u8(0x0101, 0xAA);
        z80.write_ram_u8(0x0000, 0xF1);
        z80.write_ram_u8(0x0001, 0x76);

        z80.step(64, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
        assert_eq!(z80.unknown_opcode_total(), 0);
        assert_eq!(z80.a, 0xAA);
        assert_eq!(
            z80.f & (super::FLAG_S | super::FLAG_Z | super::FLAG_PV | super::FLAG_C),
            0x45
        );
        assert_eq!(z80.sp, 0x0102);
    }

    #[test]
    fn push_pop_bc_and_conditional_call_nz() {
        let mut z80 = Z80::new();
        let mut audio = AudioBus::new();
        let cart = dummy_cart();
        let mut work_ram = [0u8; 0x10000];
        let mut vdp = Vdp::new();
        let mut io = IoBus::new();
        z80.write_reset_byte(0x01);

        // ld bc,0x1234 ; push bc ; ld bc,0 ; pop bc ; call nz,0x0010 ; halt
        // 0x0010: and 0x0F ; sub 0x01 ; ret
        let program = [
            0x01, 0x34, 0x12, 0xC5, 0x01, 0x00, 0x00, 0xC1, 0xC4, 0x10, 0x00, 0x76, 0x00, 0x00,
            0x00, 0x00, 0xE6, 0x0F, 0xD6, 0x01, 0xC9,
        ];
        for (i, byte) in program.iter().enumerate() {
            z80.write_ram_u8(i as u16, *byte);
        }
        z80.a = 0x3C;
        z80.f = 0; // NZ true

        z80.step(512, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
        assert_eq!(z80.unknown_opcode_total(), 0);
        assert_eq!(z80.bc(), 0x1234);
        assert_eq!(z80.a, 0x0B);
    }

    #[test]
    fn bank_window_reads_from_68k_rom_space() {
        let mut z80 = Z80::new();
        let mut audio = AudioBus::new();
        let mut rom = vec![0u8; 0x200];
        rom[0x0000] = 0xAB;
        let cart = Cartridge::from_bytes(rom).expect("valid cart");
        let mut work_ram = [0u8; 0x10000];
        let mut vdp = Vdp::new();
        let mut io = IoBus::new();
        z80.write_reset_byte(0x01);

        // ld a,(0x8000) ; halt
        z80.write_ram_u8(0x0000, 0x3A);
        z80.write_ram_u8(0x0001, 0x00);
        z80.write_ram_u8(0x0002, 0x80);
        z80.write_ram_u8(0x0003, 0x76);

        z80.step(128, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
        assert_eq!(z80.a, 0xAB);
    }

    #[test]
    fn bank_window_writes_to_68k_work_ram_space() {
        let mut z80 = Z80::new();
        let mut audio = AudioBus::new();
        let cart = dummy_cart();
        let mut work_ram = [0u8; 0x10000];
        let mut vdp = Vdp::new();
        let mut io = IoBus::new();
        z80.write_reset_byte(0x01);
        z80.bank_address = 0x00FF_0000;

        // ld a,0x5A ; ld (0x8000),a ; halt
        z80.write_ram_u8(0x0000, 0x3E);
        z80.write_ram_u8(0x0001, 0x5A);
        z80.write_ram_u8(0x0002, 0x32);
        z80.write_ram_u8(0x0003, 0x00);
        z80.write_ram_u8(0x0004, 0x80);
        z80.write_ram_u8(0x0005, 0x76);

        z80.step(160, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
        assert_eq!(work_ram[0], 0x5A);
    }

    #[test]
    fn bank_window_reads_io_version_register() {
        let mut z80 = Z80::new();
        let mut audio = AudioBus::new();
        let cart = dummy_cart();
        let mut work_ram = [0u8; 0x10000];
        let mut vdp = Vdp::new();
        let mut io = IoBus::new();
        z80.write_reset_byte(0x01);
        z80.bank_address = 0x00A1_0000;

        // ld a,(0x8000) ; halt
        z80.write_ram_u8(0x0000, 0x3A);
        z80.write_ram_u8(0x0001, 0x00);
        z80.write_ram_u8(0x0002, 0x80);
        z80.write_ram_u8(0x0003, 0x76);

        z80.step(128, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
        assert_eq!(z80.a, 0x20);
    }

    #[test]
    fn bank_window_reads_vdp_hv_counter_bytes() {
        let mut z80 = Z80::new();
        let mut audio = AudioBus::new();
        let cart = dummy_cart();
        let mut work_ram = [0u8; 0x10000];
        let mut vdp = Vdp::new();
        let mut io = IoBus::new();
        z80.write_reset_byte(0x01);
        z80.bank_address = 0x00C0_0000;
        let expected = vdp.read_hv_counter();

        // ld a,(0x8008) ; ld b,a ; ld a,(0x8009) ; halt
        let program = [0x3A, 0x08, 0x80, 0x47, 0x3A, 0x09, 0x80, 0x76];
        for (i, byte) in program.iter().enumerate() {
            z80.write_ram_u8(i as u16, *byte);
        }

        z80.step(224, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
        assert_eq!(z80.b, (expected >> 8) as u8);
        assert_eq!(z80.a, expected as u8);
    }

    #[test]
    fn bank_window_reads_vdp_control_status_bytes() {
        let mut z80 = Z80::new();
        let mut audio = AudioBus::new();
        let cart = dummy_cart();
        let mut work_ram = [0u8; 0x10000];
        let mut vdp = Vdp::new();
        let mut io = IoBus::new();
        z80.write_reset_byte(0x01);
        z80.bank_address = 0x00C0_0000;
        let expected = vdp.read_control_port();

        // ld a,(0x8004) ; ld b,a ; ld a,(0x8005) ; halt
        let program = [0x3A, 0x04, 0x80, 0x47, 0x3A, 0x05, 0x80, 0x76];
        for (i, byte) in program.iter().enumerate() {
            z80.write_ram_u8(i as u16, *byte);
        }

        z80.step(224, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
        assert_eq!(z80.b, (expected >> 8) as u8);
        assert_eq!(z80.a, expected as u8);
    }

    #[test]
    fn bank_window_control_write_executes_pending_vdp_bus_dma() {
        let mut z80 = Z80::new();
        let mut audio = AudioBus::new();
        let cart = dummy_cart();
        let mut work_ram = [0u8; 0x10000];
        let mut vdp = Vdp::new();
        let mut io = IoBus::new();
        z80.write_reset_byte(0x01);
        z80.bank_address = 0x00C0_0000;
        work_ram[0] = 0x12;
        work_ram[1] = 0x34;

        let mut bus = super::Z80Bus {
            audio: &mut audio,
            cartridge: &cart,
            work_ram: &mut work_ram,
            vdp: &mut vdp,
            io: &mut io,
        };
        let mut write_control_word = |word: u16| {
            z80.write_68k_window(0x8004, (word >> 8) as u8, &mut bus);
            z80.write_68k_window(0x8005, word as u8, &mut bus);
        };

        // Enable DMA and setup one-word 68k-bus DMA from 0xFF0000 to VRAM 0x0000.
        write_control_word(0x8150);
        write_control_word(0x8F02);
        write_control_word(0x9301);
        write_control_word(0x9400);
        write_control_word(0x9500);
        write_control_word(0x9680);
        write_control_word(0x977F);

        // Set VRAM write command with DMA request bit.
        write_control_word(0x4000);
        write_control_word(0x0080);

        assert_eq!(bus.vdp.read_vram_u8(0x0000), 0x12);
        assert_eq!(bus.vdp.read_vram_u8(0x0001), 0x34);
    }

    #[test]
    fn bank_window_vdp_data_byte_pair_commits_single_word() {
        let mut z80 = Z80::new();
        let mut audio = AudioBus::new();
        let cart = dummy_cart();
        let mut work_ram = [0u8; 0x10000];
        let mut vdp = Vdp::new();
        let mut io = IoBus::new();
        z80.write_reset_byte(0x01);
        z80.bank_address = 0x00C0_0000;

        let mut bus = super::Z80Bus {
            audio: &mut audio,
            cartridge: &cart,
            work_ram: &mut work_ram,
            vdp: &mut vdp,
            io: &mut io,
        };
        let mut write_control_word = |word: u16| {
            z80.write_68k_window(0x8004, (word >> 8) as u8, &mut bus);
            z80.write_68k_window(0x8005, word as u8, &mut bus);
        };

        // VRAM write at address 0.
        write_control_word(0x4000);
        write_control_word(0x0000);

        // Write one 16-bit data word through byte path.
        z80.write_68k_window(0x8000, 0x12, &mut bus);
        z80.write_68k_window(0x8001, 0x34, &mut bus);

        assert_eq!(bus.vdp.read_vram_u8(0x0000), 0x12);
        assert_eq!(bus.vdp.read_vram_u8(0x0001), 0x34);
        assert_eq!(bus.vdp.read_vram_u8(0x0002), 0x00);
        assert_eq!(bus.vdp.read_vram_u8(0x0003), 0x00);
    }

    #[test]
    fn bank_window_can_write_psg_through_68k_bus_address() {
        let mut z80 = Z80::new();
        let mut audio = AudioBus::new();
        let cart = dummy_cart();
        let mut work_ram = [0u8; 0x10000];
        let mut vdp = Vdp::new();
        let mut io = IoBus::new();
        z80.write_reset_byte(0x01);
        z80.bank_address = 0x00C0_0000;

        // ld a,0x9A ; ld (0x8011),a ; halt
        z80.write_ram_u8(0x0000, 0x3E);
        z80.write_ram_u8(0x0001, 0x9A);
        z80.write_ram_u8(0x0002, 0x32);
        z80.write_ram_u8(0x0003, 0x11);
        z80.write_ram_u8(0x0004, 0x80);
        z80.write_ram_u8(0x0005, 0x76);

        z80.step(160, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
        assert_eq!(audio.psg().last_data(), 0x9A);
    }

    #[test]
    fn bank_window_can_write_psg_through_68k_bus_mirror_addresses() {
        let mut z80 = Z80::new();
        let mut audio = AudioBus::new();
        let cart = dummy_cart();
        let mut work_ram = [0u8; 0x10000];
        let mut vdp = Vdp::new();
        let mut io = IoBus::new();
        z80.write_reset_byte(0x01);
        z80.bank_address = 0x00C0_0000;

        // ld a,0x9B ; ld (0x8013),a ; halt
        z80.write_ram_u8(0x0000, 0x3E);
        z80.write_ram_u8(0x0001, 0x9B);
        z80.write_ram_u8(0x0002, 0x32);
        z80.write_ram_u8(0x0003, 0x13);
        z80.write_ram_u8(0x0004, 0x80);
        z80.write_ram_u8(0x0005, 0x76);

        z80.step(160, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
        assert_eq!(audio.psg().last_data(), 0x9B);

        // Same via Dxxxxx mirror region.
        z80 = Z80::new();
        audio = AudioBus::new();
        z80.write_reset_byte(0x01);
        z80.bank_address = 0x00D0_0000;
        z80.write_ram_u8(0x0000, 0x3E);
        z80.write_ram_u8(0x0001, 0x9C);
        z80.write_ram_u8(0x0002, 0x32);
        z80.write_ram_u8(0x0003, 0x11);
        z80.write_ram_u8(0x0004, 0x80);
        z80.write_ram_u8(0x0005, 0x76);

        z80.step(160, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
        assert_eq!(audio.psg().last_data(), 0x9C);
    }

    #[test]
    fn bank_register_uses_serial_bit_latch() {
        let mut z80 = Z80::new();
        for _ in 0..8 {
            z80.write_bank_register(1);
        }
        assert_eq!(z80.bank_address, 0x00FF_0000);
    }

    #[test]
    fn maskable_interrupt_acknowledge_increments_refresh_counter() {
        let mut z80 = Z80::new();
        let mut audio = AudioBus::new();
        let cart = dummy_cart();
        let mut work_ram = [0u8; 0x10000];
        let mut vdp = Vdp::new();
        let mut io = IoBus::new();
        z80.write_reset_byte(0x01);
        z80.r_reg = 0xFF;
        z80.iff1 = true;
        z80.request_interrupt();

        // 28 M68k cycles grant exactly 13 Z80 cycles (IRQ acknowledge only).
        z80.step(28, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);

        assert_eq!(z80.unknown_opcode_total(), 0);
        assert_eq!(z80.pc, 0x0038);
        // R increments on acknowledge M1, low 7 bits wrap and bit7 stays unchanged.
        assert_eq!(z80.r_reg, 0x80);
    }

    #[test]
    fn nmi_acknowledge_increments_refresh_counter() {
        let mut z80 = Z80::new();
        let mut audio = AudioBus::new();
        let cart = dummy_cart();
        let mut work_ram = [0u8; 0x10000];
        let mut vdp = Vdp::new();
        let mut io = IoBus::new();
        z80.write_reset_byte(0x01);
        z80.r_reg = 0x7F;
        z80.iff1 = true;
        z80.request_nmi();

        // 24 M68k cycles grant exactly 11 Z80 cycles (NMI acknowledge only).
        z80.step(24, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);

        assert_eq!(z80.unknown_opcode_total(), 0);
        assert_eq!(z80.pc, 0x0066);
        // R increments on acknowledge M1 and wraps modulo 128.
        assert_eq!(z80.r_reg, 0x00);
    }

    #[test]
    fn interrupt_requests_vector_to_0038_and_reti_returns() {
        let mut z80 = Z80::new();
        let mut audio = AudioBus::new();
        let cart = dummy_cart();
        let mut work_ram = [0u8; 0x10000];
        let mut vdp = Vdp::new();
        let mut io = IoBus::new();
        z80.write_reset_byte(0x01);

        // ei ; halt
        z80.write_ram_u8(0x0000, 0xFB);
        z80.write_ram_u8(0x0001, 0x76);
        z80.write_ram_u8(0x0002, 0x76);
        // IRQ vector @0x0038: ld a,0x42 ; reti
        z80.write_ram_u8(0x0038, 0x3E);
        z80.write_ram_u8(0x0039, 0x42);
        z80.write_ram_u8(0x003A, 0xED);
        z80.write_ram_u8(0x003B, 0x4D);

        z80.step(64, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
        assert_eq!(z80.pc, 0x0002);

        z80.request_interrupt();
        z80.step(256, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);

        assert_eq!(z80.unknown_opcode_total(), 0);
        assert_eq!(z80.a, 0x42);
        assert_eq!(z80.pc, 0x0003);
    }

    #[test]
    fn ei_defers_maskable_irq_until_after_next_instruction() {
        let mut z80 = Z80::new();
        let mut audio = AudioBus::new();
        let cart = dummy_cart();
        let mut work_ram = [0u8; 0x10000];
        let mut vdp = Vdp::new();
        let mut io = IoBus::new();
        z80.write_reset_byte(0x01);

        // ei ; ld a,0x11 ; halt ; halt
        z80.write_ram_u8(0x0000, 0xFB);
        z80.write_ram_u8(0x0001, 0x3E);
        z80.write_ram_u8(0x0002, 0x11);
        z80.write_ram_u8(0x0003, 0x76);
        z80.write_ram_u8(0x0004, 0x76);

        // IRQ vector @0x0038: ld a,0x22 ; reti
        z80.write_ram_u8(0x0038, 0x3E);
        z80.write_ram_u8(0x0039, 0x22);
        z80.write_ram_u8(0x003A, 0xED);
        z80.write_ram_u8(0x003B, 0x4D);

        z80.request_interrupt();
        z80.step(512, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);

        // If IRQ were taken immediately after EI, LD A,0x11 would run after RETI.
        assert_eq!(z80.unknown_opcode_total(), 0);
        assert_eq!(z80.a, 0x22);
        assert_eq!(z80.pc, 0x0004);
    }

    #[test]
    fn nmi_vectors_to_0066_even_when_maskable_irqs_are_disabled() {
        let mut z80 = Z80::new();
        let mut audio = AudioBus::new();
        let cart = dummy_cart();
        let mut work_ram = [0u8; 0x10000];
        let mut vdp = Vdp::new();
        let mut io = IoBus::new();
        z80.write_reset_byte(0x01);

        // di ; halt ; halt
        z80.write_ram_u8(0x0000, 0xF3);
        z80.write_ram_u8(0x0001, 0x76);
        z80.write_ram_u8(0x0002, 0x76);

        // NMI vector @0x0066: ld a,0x5A ; retn
        z80.write_ram_u8(0x0066, 0x3E);
        z80.write_ram_u8(0x0067, 0x5A);
        z80.write_ram_u8(0x0068, 0xED);
        z80.write_ram_u8(0x0069, 0x45);

        z80.step(128, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
        assert!(z80.halted);
        assert_eq!(z80.pc, 0x0002);

        z80.request_nmi();
        z80.step(512, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);

        assert_eq!(z80.unknown_opcode_total(), 0);
        assert_eq!(z80.a, 0x5A);
        assert_eq!(z80.pc, 0x0003);
        assert!(z80.halted);
    }

    #[test]
    fn nmi_latches_previous_iff1_into_iff2_and_retn_restores_it() {
        let mut z80 = Z80::new();
        let mut audio = AudioBus::new();
        let cart = dummy_cart();
        let mut work_ram = [0u8; 0x10000];
        let mut vdp = Vdp::new();
        let mut io = IoBus::new();
        z80.write_reset_byte(0x01);

        // halt ; halt
        z80.write_ram_u8(0x0000, 0x76);
        z80.write_ram_u8(0x0001, 0x76);

        // NMI handler @0x0066: retn
        z80.write_ram_u8(0x0066, 0xED);
        z80.write_ram_u8(0x0067, 0x45);

        // Set up a state where only IFF1 is enabled so NMI must copy it to IFF2.
        z80.iff1 = true;
        z80.iff2 = false;

        z80.step(64, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
        assert!(z80.halted);
        assert_eq!(z80.pc, 0x0001);

        z80.request_nmi();
        z80.step(256, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);

        assert_eq!(z80.unknown_opcode_total(), 0);
        assert!(z80.iff1);
        assert!(z80.iff2);
        assert_eq!(z80.pc, 0x0002);
        assert!(z80.halted);
    }

    #[test]
    fn im_opcodes_update_interrupt_mode_state() {
        let mut z80 = Z80::new();
        let mut audio = AudioBus::new();
        let cart = dummy_cart();
        let mut work_ram = [0u8; 0x10000];
        let mut vdp = Vdp::new();
        let mut io = IoBus::new();
        z80.write_reset_byte(0x01);

        // im 1 ; im 2 ; im 0 ; halt
        z80.write_ram_u8(0x0000, 0xED);
        z80.write_ram_u8(0x0001, 0x56);
        z80.write_ram_u8(0x0002, 0xED);
        z80.write_ram_u8(0x0003, 0x5E);
        z80.write_ram_u8(0x0004, 0xED);
        z80.write_ram_u8(0x0005, 0x46);
        z80.write_ram_u8(0x0006, 0x76);

        z80.step(256, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
        assert_eq!(z80.interrupt_mode, 0);
        assert_eq!(z80.unknown_opcode_total(), 0);
    }

    #[test]
    fn interrupt_mode_0_uses_configured_rst_vector_opcode() {
        let mut z80 = Z80::new();
        let mut audio = AudioBus::new();
        let cart = dummy_cart();
        let mut work_ram = [0u8; 0x10000];
        let mut vdp = Vdp::new();
        let mut io = IoBus::new();
        z80.write_reset_byte(0x01);

        // im 0 ; ei ; halt ; halt
        z80.write_ram_u8(0x0000, 0xED);
        z80.write_ram_u8(0x0001, 0x46);
        z80.write_ram_u8(0x0002, 0xFB);
        z80.write_ram_u8(0x0003, 0x76);
        z80.write_ram_u8(0x0004, 0x76);

        // Handler @0x0028: ld a,0x66 ; reti
        z80.write_ram_u8(0x0028, 0x3E);
        z80.write_ram_u8(0x0029, 0x66);
        z80.write_ram_u8(0x002A, 0xED);
        z80.write_ram_u8(0x002B, 0x4D);

        z80.set_im0_interrupt_opcode(0xEF); // RST 28h
        z80.step(256, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
        assert_eq!(z80.interrupt_mode, 0);
        assert_eq!(z80.pc, 0x0004);

        z80.request_interrupt();
        z80.step(512, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);

        assert_eq!(z80.unknown_opcode_total(), 0);
        assert_eq!(z80.a, 0x66);
        assert_eq!(z80.pc, 0x0005);
    }

    #[test]
    fn interrupt_mode_0_falls_back_to_rst38_for_non_rst_opcode() {
        let mut z80 = Z80::new();
        let mut audio = AudioBus::new();
        let cart = dummy_cart();
        let mut work_ram = [0u8; 0x10000];
        let mut vdp = Vdp::new();
        let mut io = IoBus::new();
        z80.write_reset_byte(0x01);

        // im 0 ; ei ; halt ; halt
        z80.write_ram_u8(0x0000, 0xED);
        z80.write_ram_u8(0x0001, 0x46);
        z80.write_ram_u8(0x0002, 0xFB);
        z80.write_ram_u8(0x0003, 0x76);
        z80.write_ram_u8(0x0004, 0x76);

        // Fallback handler @0x0038: ld a,0x44 ; reti
        z80.write_ram_u8(0x0038, 0x3E);
        z80.write_ram_u8(0x0039, 0x44);
        z80.write_ram_u8(0x003A, 0xED);
        z80.write_ram_u8(0x003B, 0x4D);

        z80.set_im0_interrupt_opcode(0x00); // Non-RST opcode
        z80.step(256, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
        assert_eq!(z80.interrupt_mode, 0);
        assert_eq!(z80.pc, 0x0004);

        z80.request_interrupt();
        z80.step(512, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);

        assert_eq!(z80.unknown_opcode_total(), 0);
        assert_eq!(z80.a, 0x44);
        assert_eq!(z80.pc, 0x0005);
    }

    #[test]
    fn interrupt_mode_2_uses_i_register_vector_table() {
        let mut z80 = Z80::new();
        let mut audio = AudioBus::new();
        let cart = dummy_cart();
        let mut work_ram = [0u8; 0x10000];
        let mut vdp = Vdp::new();
        let mut io = IoBus::new();
        z80.write_reset_byte(0x01);

        // ld a,0x12 ; ld i,a ; im 2 ; ei ; halt ; halt
        z80.write_ram_u8(0x0000, 0x3E);
        z80.write_ram_u8(0x0001, 0x12);
        z80.write_ram_u8(0x0002, 0xED);
        z80.write_ram_u8(0x0003, 0x47);
        z80.write_ram_u8(0x0004, 0xED);
        z80.write_ram_u8(0x0005, 0x5E);
        z80.write_ram_u8(0x0006, 0xFB);
        z80.write_ram_u8(0x0007, 0x76);
        z80.write_ram_u8(0x0008, 0x76);

        // IM2 vector table at I:0x12FF -> 0x3456.
        z80.write_ram_u8(0x12FF, 0x56);
        z80.write_ram_u8(0x1300, 0x34);
        // Handler @0x3456: ld a,0x77 ; reti
        z80.write_ram_u8(0x3456, 0x3E);
        z80.write_ram_u8(0x3457, 0x77);
        z80.write_ram_u8(0x3458, 0xED);
        z80.write_ram_u8(0x3459, 0x4D);

        z80.step(256, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
        assert_eq!(z80.interrupt_mode, 2);
        assert_eq!(z80.pc, 0x0008);

        z80.request_interrupt();
        z80.step(512, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
        assert_eq!(z80.unknown_opcode_total(), 0);
        assert_eq!(z80.a, 0x77);
        assert_eq!(z80.pc, 0x0009);
    }

    #[test]
    fn inc_de_opcode_updates_register_pair() {
        let mut z80 = Z80::new();
        let mut audio = AudioBus::new();
        let cart = dummy_cart();
        let mut work_ram = [0u8; 0x10000];
        let mut vdp = Vdp::new();
        let mut io = IoBus::new();
        z80.write_reset_byte(0x01);

        // ld de,0x00FF ; inc de ; halt
        z80.write_ram_u8(0x0000, 0x11);
        z80.write_ram_u8(0x0001, 0xFF);
        z80.write_ram_u8(0x0002, 0x00);
        z80.write_ram_u8(0x0003, 0x13);
        z80.write_ram_u8(0x0004, 0x76);

        z80.step(128, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
        assert_eq!(z80.de(), 0x0100);
    }

    #[test]
    fn ldi_copies_byte_and_updates_pairs() {
        let mut z80 = Z80::new();
        let mut audio = AudioBus::new();
        let cart = dummy_cart();
        let mut work_ram = [0u8; 0x10000];
        let mut vdp = Vdp::new();
        let mut io = IoBus::new();
        z80.write_reset_byte(0x01);

        z80.set_hl(0x0100);
        z80.set_de(0x0200);
        z80.set_bc(0x0001);
        z80.write_ram_u8(0x0100, 0x5A);
        z80.write_ram_u8(0x0000, 0xED);
        z80.write_ram_u8(0x0001, 0xA0);
        z80.write_ram_u8(0x0002, 0x76);

        z80.step(128, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
        assert_eq!(z80.read_ram_u8(0x0200), 0x5A);
        assert_eq!(z80.hl(), 0x0101);
        assert_eq!(z80.de(), 0x0201);
        assert_eq!(z80.bc(), 0x0000);
    }
}
