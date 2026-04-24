use std::collections::BTreeMap;

const FLAG_S: u8 = 0x80;
const FLAG_Z: u8 = 0x40;
const FLAG_Y: u8 = 0x20;
const FLAG_PV: u8 = 0x04;
const FLAG_H: u8 = 0x10;
const FLAG_X: u8 = 0x08;
const FLAG_N: u8 = 0x02;
const FLAG_C: u8 = 0x01;
pub const Z80_CLOCK_HZ: u64 = 3_579_545;

fn audio_io_wait_cycles() -> u16 {
    0
}

pub trait BusIo {
    fn read_memory(&mut self, addr: u16) -> u8;
    fn write_memory(&mut self, addr: u16, value: u8);
    fn read_port(&mut self, port: u8) -> u8;
    fn write_port(&mut self, port: u8, value: u8);
}

struct Z80Bus<'a> {
    bus: &'a mut dyn BusIo,
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

#[allow(dead_code)]
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

    pub fn step(&mut self, cycles: u32, bus: &mut dyn BusIo) {
        if self.reset_asserted || cycles == 0 {
            return;
        }

        let mut bus = Z80Bus { bus };
        self.execution_credit_cycles += cycles as i64;

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
                break;
            }
            self.io_wait_cycles = 0;
            let opcode_pc = self.pc;
            let opcode = self.fetch_opcode_u8(&mut bus);
            let elapsed = self.exec_opcode(opcode_pc, opcode, &mut bus) as usize
                + self.io_wait_cycles as usize;
            self.execution_credit_cycles -= elapsed as i64;
            if self.interrupt_inhibit_count > 0 {
                self.interrupt_inhibit_count -= 1;
            }
        }
        if self.halted && self.execution_credit_cycles > 0 {
            self.execution_credit_cycles = 0;
        }

        // Account wall-clock Z80 time even if halted or blocked by unsupported opcodes.
        self.cycles += cycles as u64;
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
        bus.bus.read_memory(addr)
    }

    fn write_byte(&mut self, addr: u16, value: u8, bus: &mut Z80Bus<'_>) {
        bus.bus.write_memory(addr, value);
    }

    fn read_port(&self, port: u16, bus: &mut Z80Bus<'_>) -> u8 {
        bus.bus.read_port(port as u8)
    }

    fn write_port(&mut self, port: u16, value: u8, bus: &mut Z80Bus<'_>) {
        bus.bus.write_port(port as u8, value);
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
