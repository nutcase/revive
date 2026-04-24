mod bus;

use self::bus::{MdBusState, Z80Bus, audio_io_wait_cycles};
use crate::audio::AudioBus;
use crate::cartridge::Cartridge;
use crate::input::IoBus;
use crate::vdp::Vdp;
use std::collections::BTreeMap;

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
    md_bus_state: MdBusState,
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
            md_bus_state: MdBusState::default(),
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
            self.md_bus_state = MdBusState::default();
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
}

// Shared Z80 instruction executor; Mega Drive keeps bus and timing hooks local.
sega8_common::impl_z80_ops!();

#[cfg(test)]
mod tests;
