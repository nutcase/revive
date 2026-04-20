use crate::bus::{GbBus, INT_JOYPAD, INT_LCD_STAT, INT_SERIAL, INT_TIMER, INT_VBLANK};

const FLAG_Z: u8 = 0x80;
const FLAG_N: u8 = 0x40;
const FLAG_H: u8 = 0x20;
const FLAG_C: u8 = 0x10;

#[derive(Debug, Default)]
pub struct GbCpu {
    registers: Registers,
    halted: bool,
    ime: bool,
    ime_enable_delay: u8,
    halt_bug: bool,
}

#[derive(Debug, Clone, Copy)]
struct Registers {
    a: u8,
    f: u8,
    b: u8,
    c: u8,
    d: u8,
    e: u8,
    h: u8,
    l: u8,
    sp: u16,
    pc: u16,
}

impl Default for Registers {
    fn default() -> Self {
        Self::dmg_boot()
    }
}

impl Registers {
    fn dmg_boot() -> Self {
        Self {
            a: 0x01,
            f: 0xB0,
            b: 0x00,
            c: 0x13,
            d: 0x00,
            e: 0xD8,
            h: 0x01,
            l: 0x4D,
            sp: 0xFFFE,
            pc: 0x0100,
        }
    }

    fn cgb_boot() -> Self {
        Self {
            a: 0x11,
            f: 0x80,
            b: 0x00,
            c: 0x00,
            d: 0xFF,
            e: 0x56,
            h: 0x00,
            l: 0x0D,
            sp: 0xFFFE,
            pc: 0x0100,
        }
    }
}

impl GbCpu {
    #[allow(dead_code)]
    pub fn reset(&mut self) {
        self.reset_for_model(false);
    }

    pub fn reset_for_model(&mut self, cgb_mode: bool) {
        self.registers = if cgb_mode {
            Registers::cgb_boot()
        } else {
            Registers::dmg_boot()
        };
        self.halted = false;
        self.ime = false;
        self.ime_enable_delay = 0;
        self.halt_bug = false;
    }

    pub fn step(&mut self, bus: &mut GbBus) -> u32 {
        if let Some(cycles) = self.handle_interrupts(bus) {
            return cycles;
        }

        if self.halted {
            if bus.pending_interrupts() != 0 {
                self.halted = false;
            } else {
                return 4;
            }
        }

        let opcode = self.fetch8(bus);
        let cycles = match opcode {
            0x00 => 4,
            0x01 => {
                let value = self.fetch16(bus);
                self.set_bc(value);
                12
            }
            0x02 => {
                bus.write8(self.bc(), self.registers.a);
                8
            }
            0x03 => {
                let value = self.bc().wrapping_add(1);
                self.set_bc(value);
                8
            }
            0x04 => self.inc_r8(bus, 0),
            0x05 => self.dec_r8(bus, 0),
            0x06 => {
                self.registers.b = self.fetch8(bus);
                8
            }
            0x07 => {
                let carry = (self.registers.a & 0x80) != 0;
                self.registers.a = self.registers.a.rotate_left(1);
                self.set_flag(FLAG_Z, false);
                self.set_flag(FLAG_N, false);
                self.set_flag(FLAG_H, false);
                self.set_flag(FLAG_C, carry);
                4
            }
            0x08 => {
                let addr = self.fetch16(bus);
                bus.write16(addr, self.registers.sp);
                20
            }
            0x09 => {
                self.add_hl(self.bc());
                8
            }
            0x0A => {
                self.registers.a = bus.read8(self.bc());
                8
            }
            0x0B => {
                let value = self.bc().wrapping_sub(1);
                self.set_bc(value);
                8
            }
            0x0C => self.inc_r8(bus, 1),
            0x0D => self.dec_r8(bus, 1),
            0x0E => {
                self.registers.c = self.fetch8(bus);
                8
            }
            0x0F => {
                let carry = (self.registers.a & 0x01) != 0;
                self.registers.a = self.registers.a.rotate_right(1);
                self.set_flag(FLAG_Z, false);
                self.set_flag(FLAG_N, false);
                self.set_flag(FLAG_H, false);
                self.set_flag(FLAG_C, carry);
                4
            }
            0x10 => {
                self.fetch8(bus);
                bus.handle_stop();
                4
            }
            0x11 => {
                let value = self.fetch16(bus);
                self.set_de(value);
                12
            }
            0x12 => {
                bus.write8(self.de(), self.registers.a);
                8
            }
            0x13 => {
                let value = self.de().wrapping_add(1);
                self.set_de(value);
                8
            }
            0x14 => self.inc_r8(bus, 2),
            0x15 => self.dec_r8(bus, 2),
            0x16 => {
                self.registers.d = self.fetch8(bus);
                8
            }
            0x17 => {
                let carry_in = u8::from(self.flag(FLAG_C));
                let carry_out = (self.registers.a & 0x80) != 0;
                self.registers.a = (self.registers.a << 1) | carry_in;
                self.set_flag(FLAG_Z, false);
                self.set_flag(FLAG_N, false);
                self.set_flag(FLAG_H, false);
                self.set_flag(FLAG_C, carry_out);
                4
            }
            0x18 => {
                let offset = self.fetch8(bus) as i8;
                self.jr(offset);
                12
            }
            0x19 => {
                self.add_hl(self.de());
                8
            }
            0x1A => {
                self.registers.a = bus.read8(self.de());
                8
            }
            0x1B => {
                let value = self.de().wrapping_sub(1);
                self.set_de(value);
                8
            }
            0x1C => self.inc_r8(bus, 3),
            0x1D => self.dec_r8(bus, 3),
            0x1E => {
                self.registers.e = self.fetch8(bus);
                8
            }
            0x1F => {
                let carry_in = if self.flag(FLAG_C) { 0x80 } else { 0x00 };
                let carry_out = (self.registers.a & 0x01) != 0;
                self.registers.a = (self.registers.a >> 1) | carry_in;
                self.set_flag(FLAG_Z, false);
                self.set_flag(FLAG_N, false);
                self.set_flag(FLAG_H, false);
                self.set_flag(FLAG_C, carry_out);
                4
            }
            0x20 | 0x28 | 0x30 | 0x38 => {
                let offset = self.fetch8(bus) as i8;
                if self.condition(opcode) {
                    self.jr(offset);
                    12
                } else {
                    8
                }
            }
            0x21 => {
                let value = self.fetch16(bus);
                self.set_hl(value);
                12
            }
            0x22 => {
                let hl = self.hl();
                bus.write8(hl, self.registers.a);
                self.set_hl(hl.wrapping_add(1));
                8
            }
            0x23 => {
                self.set_hl(self.hl().wrapping_add(1));
                8
            }
            0x24 => self.inc_r8(bus, 4),
            0x25 => self.dec_r8(bus, 4),
            0x26 => {
                self.registers.h = self.fetch8(bus);
                8
            }
            0x27 => {
                self.daa();
                4
            }
            0x29 => {
                self.add_hl(self.hl());
                8
            }
            0x2A => {
                let hl = self.hl();
                self.registers.a = bus.read8(hl);
                self.set_hl(hl.wrapping_add(1));
                8
            }
            0x2B => {
                self.set_hl(self.hl().wrapping_sub(1));
                8
            }
            0x2C => self.inc_r8(bus, 5),
            0x2D => self.dec_r8(bus, 5),
            0x2E => {
                self.registers.l = self.fetch8(bus);
                8
            }
            0x2F => {
                self.registers.a = !self.registers.a;
                self.set_flag(FLAG_N, true);
                self.set_flag(FLAG_H, true);
                4
            }
            0x31 => {
                self.registers.sp = self.fetch16(bus);
                12
            }
            0x32 => {
                let hl = self.hl();
                bus.write8(hl, self.registers.a);
                self.set_hl(hl.wrapping_sub(1));
                8
            }
            0x33 => {
                self.registers.sp = self.registers.sp.wrapping_add(1);
                8
            }
            0x34 => self.inc_r8(bus, 6),
            0x35 => self.dec_r8(bus, 6),
            0x36 => {
                let value = self.fetch8(bus);
                bus.write8(self.hl(), value);
                12
            }
            0x37 => {
                self.set_flag(FLAG_N, false);
                self.set_flag(FLAG_H, false);
                self.set_flag(FLAG_C, true);
                4
            }
            0x39 => {
                self.add_hl(self.registers.sp);
                8
            }
            0x3A => {
                let hl = self.hl();
                self.registers.a = bus.read8(hl);
                self.set_hl(hl.wrapping_sub(1));
                8
            }
            0x3B => {
                self.registers.sp = self.registers.sp.wrapping_sub(1);
                8
            }
            0x3C => self.inc_r8(bus, 7),
            0x3D => self.dec_r8(bus, 7),
            0x3E => {
                self.registers.a = self.fetch8(bus);
                8
            }
            0x3F => {
                let carry = self.flag(FLAG_C);
                self.set_flag(FLAG_N, false);
                self.set_flag(FLAG_H, false);
                self.set_flag(FLAG_C, !carry);
                4
            }
            0x40..=0x7F => {
                if opcode == 0x76 {
                    if !self.ime && bus.pending_interrupts() != 0 {
                        self.halt_bug = true;
                    } else {
                        self.halted = true;
                    }
                    4
                } else {
                    let dst = (opcode >> 3) & 0x07;
                    let src = opcode & 0x07;
                    let value = self.read_r8(bus, src);
                    self.write_r8(bus, dst, value);
                    if dst == 6 || src == 6 { 8 } else { 4 }
                }
            }
            0x80..=0x87 => {
                let src = opcode & 0x07;
                let value = self.read_r8(bus, src);
                self.add_a(value);
                if src == 6 { 8 } else { 4 }
            }
            0x88..=0x8F => {
                let src = opcode & 0x07;
                let value = self.read_r8(bus, src);
                self.adc_a(value);
                if src == 6 { 8 } else { 4 }
            }
            0x90..=0x97 => {
                let src = opcode & 0x07;
                let value = self.read_r8(bus, src);
                self.sub_a(value);
                if src == 6 { 8 } else { 4 }
            }
            0x98..=0x9F => {
                let src = opcode & 0x07;
                let value = self.read_r8(bus, src);
                self.sbc_a(value);
                if src == 6 { 8 } else { 4 }
            }
            0xA0..=0xA7 => {
                let src = opcode & 0x07;
                let value = self.read_r8(bus, src);
                self.and_a(value);
                if src == 6 { 8 } else { 4 }
            }
            0xA8..=0xAF => {
                let src = opcode & 0x07;
                let value = self.read_r8(bus, src);
                self.xor_a(value);
                if src == 6 { 8 } else { 4 }
            }
            0xB0..=0xB7 => {
                let src = opcode & 0x07;
                let value = self.read_r8(bus, src);
                self.or_a(value);
                if src == 6 { 8 } else { 4 }
            }
            0xB8..=0xBF => {
                let src = opcode & 0x07;
                let value = self.read_r8(bus, src);
                self.cp_a(value);
                if src == 6 { 8 } else { 4 }
            }
            0xC0 | 0xC8 | 0xD0 | 0xD8 => {
                if self.condition(opcode) {
                    let addr = self.pop16(bus);
                    self.registers.pc = addr;
                    20
                } else {
                    8
                }
            }
            0xC1 => {
                let value = self.pop16(bus);
                self.set_bc(value);
                12
            }
            0xC2 | 0xCA | 0xD2 | 0xDA => {
                let addr = self.fetch16(bus);
                if self.condition(opcode) {
                    self.registers.pc = addr;
                    16
                } else {
                    12
                }
            }
            0xC3 => {
                self.registers.pc = self.fetch16(bus);
                16
            }
            0xC4 | 0xCC | 0xD4 | 0xDC => {
                let addr = self.fetch16(bus);
                if self.condition(opcode) {
                    self.push16(bus, self.registers.pc);
                    self.registers.pc = addr;
                    24
                } else {
                    12
                }
            }
            0xC5 => {
                self.push16(bus, self.bc());
                16
            }
            0xC6 => {
                let value = self.fetch8(bus);
                self.add_a(value);
                8
            }
            0xC7 | 0xCF | 0xD7 | 0xDF | 0xE7 | 0xEF | 0xF7 | 0xFF => {
                let vector = u16::from(opcode & 0x38);
                self.push16(bus, self.registers.pc);
                self.registers.pc = vector;
                16
            }
            0xC9 => {
                self.registers.pc = self.pop16(bus);
                16
            }
            0xCB => {
                let cb_opcode = self.fetch8(bus);
                self.execute_cb(bus, cb_opcode)
            }
            0xCE => {
                let value = self.fetch8(bus);
                self.adc_a(value);
                8
            }
            0xCD => {
                let addr = self.fetch16(bus);
                self.push16(bus, self.registers.pc);
                self.registers.pc = addr;
                24
            }
            0xD1 => {
                let value = self.pop16(bus);
                self.set_de(value);
                12
            }
            0xD5 => {
                self.push16(bus, self.de());
                16
            }
            0xD6 => {
                let value = self.fetch8(bus);
                self.sub_a(value);
                8
            }
            0xDE => {
                let value = self.fetch8(bus);
                self.sbc_a(value);
                8
            }
            0xD9 => {
                self.registers.pc = self.pop16(bus);
                self.ime = true;
                self.ime_enable_delay = 0;
                16
            }
            0xE0 => {
                let offset = self.fetch8(bus);
                let addr = 0xFF00u16 + offset as u16;
                bus.write8(addr, self.registers.a);
                12
            }
            0xE1 => {
                let value = self.pop16(bus);
                self.set_hl(value);
                12
            }
            0xE2 => {
                let addr = 0xFF00u16 + self.registers.c as u16;
                bus.write8(addr, self.registers.a);
                8
            }
            0xE5 => {
                self.push16(bus, self.hl());
                16
            }
            0xE6 => {
                let value = self.fetch8(bus);
                self.and_a(value);
                8
            }
            0xE8 => {
                let offset = self.fetch8(bus) as i8;
                let sp = self.registers.sp;
                let value = offset as i16 as u16;
                self.set_flag(FLAG_Z, false);
                self.set_flag(FLAG_N, false);
                self.set_flag(FLAG_H, ((sp & 0x000F) + (value & 0x000F)) > 0x000F);
                self.set_flag(FLAG_C, ((sp & 0x00FF) + (value & 0x00FF)) > 0x00FF);
                self.registers.sp = sp.wrapping_add_signed(offset as i16);
                16
            }
            0xE9 => {
                self.registers.pc = self.hl();
                4
            }
            0xEA => {
                let addr = self.fetch16(bus);
                bus.write8(addr, self.registers.a);
                16
            }
            0xEE => {
                let value = self.fetch8(bus);
                self.xor_a(value);
                8
            }
            0xF0 => {
                let offset = self.fetch8(bus);
                let addr = 0xFF00u16 + offset as u16;
                self.registers.a = bus.read8(addr);
                12
            }
            0xF1 => {
                let value = self.pop16(bus);
                self.set_af(value);
                12
            }
            0xF2 => {
                let addr = 0xFF00u16 + self.registers.c as u16;
                self.registers.a = bus.read8(addr);
                8
            }
            0xF3 => {
                self.ime = false;
                self.ime_enable_delay = 0;
                4
            }
            0xF5 => {
                self.push16(bus, self.af());
                16
            }
            0xF6 => {
                let value = self.fetch8(bus);
                self.or_a(value);
                8
            }
            0xF8 => {
                let offset = self.fetch8(bus) as i8;
                let sp = self.registers.sp;
                let value = offset as i16 as u16;
                self.set_flag(FLAG_Z, false);
                self.set_flag(FLAG_N, false);
                self.set_flag(FLAG_H, ((sp & 0x000F) + (value & 0x000F)) > 0x000F);
                self.set_flag(FLAG_C, ((sp & 0x00FF) + (value & 0x00FF)) > 0x00FF);
                self.set_hl(sp.wrapping_add_signed(offset as i16));
                12
            }
            0xF9 => {
                self.registers.sp = self.hl();
                8
            }
            0xFA => {
                let addr = self.fetch16(bus);
                self.registers.a = bus.read8(addr);
                16
            }
            0xFB => {
                self.ime_enable_delay = 2;
                4
            }
            0xFE => {
                let value = self.fetch8(bus);
                self.cp_a(value);
                8
            }
            _ => {
                let len = fallback_opcode_len(opcode);
                self.registers.pc = self.registers.pc.wrapping_add(len.saturating_sub(1) as u16);
                4
            }
        };

        self.tick_ime_delay();
        cycles
    }

    fn handle_interrupts(&mut self, bus: &mut GbBus) -> Option<u32> {
        let pending = bus.pending_interrupts();
        if pending == 0 {
            return None;
        }

        if self.halted {
            self.halted = false;
        }

        if !self.ime {
            return None;
        }

        const PRIORITY: &[(u8, u16)] = &[
            (INT_VBLANK, 0x0040),
            (INT_LCD_STAT, 0x0048),
            (INT_TIMER, 0x0050),
            (INT_SERIAL, 0x0058),
            (INT_JOYPAD, 0x0060),
        ];

        for (mask, vector) in PRIORITY {
            if (pending & *mask) != 0 {
                self.ime = false;
                self.ime_enable_delay = 0;
                bus.acknowledge_interrupt(*mask);
                self.push16(bus, self.registers.pc);
                self.registers.pc = *vector;
                return Some(20);
            }
        }

        None
    }

    fn tick_ime_delay(&mut self) {
        if self.ime_enable_delay == 0 {
            return;
        }

        self.ime_enable_delay -= 1;
        if self.ime_enable_delay == 0 {
            self.ime = true;
        }
    }

    fn fetch8(&mut self, bus: &GbBus) -> u8 {
        let pc = self.registers.pc;
        let value = bus.read8(pc);

        if self.halt_bug {
            self.halt_bug = false;
        } else {
            self.registers.pc = self.registers.pc.wrapping_add(1);
        }

        value
    }

    fn fetch16(&mut self, bus: &GbBus) -> u16 {
        let low = self.fetch8(bus) as u16;
        let high = self.fetch8(bus) as u16;
        low | (high << 8)
    }

    fn execute_cb(&mut self, bus: &mut GbBus, opcode: u8) -> u32 {
        let reg_index = opcode & 0x07;
        let uses_hl = reg_index == 6;
        let base_cycles = if uses_hl { 16 } else { 8 };

        match opcode {
            0x00..=0x07 => {
                let value = self.read_r8(bus, reg_index);
                let carry = (value & 0x80) != 0;
                let result = value.rotate_left(1);
                self.write_r8(bus, reg_index, result);
                self.set_flag(FLAG_Z, result == 0);
                self.set_flag(FLAG_N, false);
                self.set_flag(FLAG_H, false);
                self.set_flag(FLAG_C, carry);
                base_cycles
            }
            0x08..=0x0F => {
                let value = self.read_r8(bus, reg_index);
                let carry = (value & 0x01) != 0;
                let result = value.rotate_right(1);
                self.write_r8(bus, reg_index, result);
                self.set_flag(FLAG_Z, result == 0);
                self.set_flag(FLAG_N, false);
                self.set_flag(FLAG_H, false);
                self.set_flag(FLAG_C, carry);
                base_cycles
            }
            0x10..=0x17 => {
                let value = self.read_r8(bus, reg_index);
                let carry_in = u8::from(self.flag(FLAG_C));
                let carry = (value & 0x80) != 0;
                let result = (value << 1) | carry_in;
                self.write_r8(bus, reg_index, result);
                self.set_flag(FLAG_Z, result == 0);
                self.set_flag(FLAG_N, false);
                self.set_flag(FLAG_H, false);
                self.set_flag(FLAG_C, carry);
                base_cycles
            }
            0x18..=0x1F => {
                let value = self.read_r8(bus, reg_index);
                let carry_in = if self.flag(FLAG_C) { 0x80 } else { 0x00 };
                let carry = (value & 0x01) != 0;
                let result = (value >> 1) | carry_in;
                self.write_r8(bus, reg_index, result);
                self.set_flag(FLAG_Z, result == 0);
                self.set_flag(FLAG_N, false);
                self.set_flag(FLAG_H, false);
                self.set_flag(FLAG_C, carry);
                base_cycles
            }
            0x20..=0x27 => {
                let value = self.read_r8(bus, reg_index);
                let carry = (value & 0x80) != 0;
                let result = value << 1;
                self.write_r8(bus, reg_index, result);
                self.set_flag(FLAG_Z, result == 0);
                self.set_flag(FLAG_N, false);
                self.set_flag(FLAG_H, false);
                self.set_flag(FLAG_C, carry);
                base_cycles
            }
            0x28..=0x2F => {
                let value = self.read_r8(bus, reg_index);
                let carry = (value & 0x01) != 0;
                let result = (value >> 1) | (value & 0x80);
                self.write_r8(bus, reg_index, result);
                self.set_flag(FLAG_Z, result == 0);
                self.set_flag(FLAG_N, false);
                self.set_flag(FLAG_H, false);
                self.set_flag(FLAG_C, carry);
                base_cycles
            }
            0x30..=0x37 => {
                let value = self.read_r8(bus, reg_index);
                let result = value.rotate_left(4);
                self.write_r8(bus, reg_index, result);
                self.set_flag(FLAG_Z, result == 0);
                self.set_flag(FLAG_N, false);
                self.set_flag(FLAG_H, false);
                self.set_flag(FLAG_C, false);
                base_cycles
            }
            0x38..=0x3F => {
                let value = self.read_r8(bus, reg_index);
                let carry = (value & 0x01) != 0;
                let result = value >> 1;
                self.write_r8(bus, reg_index, result);
                self.set_flag(FLAG_Z, result == 0);
                self.set_flag(FLAG_N, false);
                self.set_flag(FLAG_H, false);
                self.set_flag(FLAG_C, carry);
                base_cycles
            }
            0x40..=0x7F => {
                let bit = (opcode >> 3) & 0x07;
                let value = self.read_r8(bus, reg_index);
                self.set_flag(FLAG_Z, (value & (1 << bit)) == 0);
                self.set_flag(FLAG_N, false);
                self.set_flag(FLAG_H, true);
                if uses_hl { 12 } else { 8 }
            }
            0x80..=0xBF => {
                let bit = (opcode >> 3) & 0x07;
                let value = self.read_r8(bus, reg_index) & !(1 << bit);
                self.write_r8(bus, reg_index, value);
                base_cycles
            }
            0xC0..=0xFF => {
                let bit = (opcode >> 3) & 0x07;
                let value = self.read_r8(bus, reg_index) | (1 << bit);
                self.write_r8(bus, reg_index, value);
                base_cycles
            }
        }
    }

    fn read_r8(&self, bus: &GbBus, index: u8) -> u8 {
        match index {
            0 => self.registers.b,
            1 => self.registers.c,
            2 => self.registers.d,
            3 => self.registers.e,
            4 => self.registers.h,
            5 => self.registers.l,
            6 => bus.read8(self.hl()),
            7 => self.registers.a,
            _ => 0xFF,
        }
    }

    fn write_r8(&mut self, bus: &mut GbBus, index: u8, value: u8) {
        match index {
            0 => self.registers.b = value,
            1 => self.registers.c = value,
            2 => self.registers.d = value,
            3 => self.registers.e = value,
            4 => self.registers.h = value,
            5 => self.registers.l = value,
            6 => bus.write8(self.hl(), value),
            7 => self.registers.a = value,
            _ => {}
        }
    }

    fn inc_r8(&mut self, bus: &mut GbBus, index: u8) -> u32 {
        let value = self.read_r8(bus, index);
        let result = value.wrapping_add(1);
        self.write_r8(bus, index, result);

        self.set_flag(FLAG_Z, result == 0);
        self.set_flag(FLAG_N, false);
        self.set_flag(FLAG_H, (value & 0x0F) == 0x0F);

        if index == 6 { 12 } else { 4 }
    }

    fn dec_r8(&mut self, bus: &mut GbBus, index: u8) -> u32 {
        let value = self.read_r8(bus, index);
        let result = value.wrapping_sub(1);
        self.write_r8(bus, index, result);

        self.set_flag(FLAG_Z, result == 0);
        self.set_flag(FLAG_N, true);
        self.set_flag(FLAG_H, (value & 0x0F) == 0);

        if index == 6 { 12 } else { 4 }
    }

    fn add_a(&mut self, value: u8) {
        let a = self.registers.a;
        let (result, carry) = a.overflowing_add(value);
        let half = ((a & 0x0F) + (value & 0x0F)) > 0x0F;

        self.registers.a = result;
        self.set_flag(FLAG_Z, result == 0);
        self.set_flag(FLAG_N, false);
        self.set_flag(FLAG_H, half);
        self.set_flag(FLAG_C, carry);
    }

    fn adc_a(&mut self, value: u8) {
        let a = self.registers.a;
        let carry_in = u8::from(self.flag(FLAG_C));
        let (tmp, carry1) = a.overflowing_add(value);
        let (result, carry2) = tmp.overflowing_add(carry_in);
        let half = ((a & 0x0F) + (value & 0x0F) + carry_in) > 0x0F;

        self.registers.a = result;
        self.set_flag(FLAG_Z, result == 0);
        self.set_flag(FLAG_N, false);
        self.set_flag(FLAG_H, half);
        self.set_flag(FLAG_C, carry1 || carry2);
    }

    fn sub_a(&mut self, value: u8) {
        let a = self.registers.a;
        let (result, borrow) = a.overflowing_sub(value);
        let half = (a & 0x0F) < (value & 0x0F);

        self.registers.a = result;
        self.set_flag(FLAG_Z, result == 0);
        self.set_flag(FLAG_N, true);
        self.set_flag(FLAG_H, half);
        self.set_flag(FLAG_C, borrow);
    }

    fn sbc_a(&mut self, value: u8) {
        let a = self.registers.a;
        let carry_in = u8::from(self.flag(FLAG_C));
        let (tmp, borrow1) = a.overflowing_sub(value);
        let (result, borrow2) = tmp.overflowing_sub(carry_in);
        let half = (a & 0x0F) < ((value & 0x0F).wrapping_add(carry_in));

        self.registers.a = result;
        self.set_flag(FLAG_Z, result == 0);
        self.set_flag(FLAG_N, true);
        self.set_flag(FLAG_H, half);
        self.set_flag(FLAG_C, borrow1 || borrow2);
    }

    fn daa(&mut self) {
        let mut adjust = 0u8;
        let subtract = self.flag(FLAG_N);
        let carry = self.flag(FLAG_C);
        let mut next_carry = carry;

        if !subtract {
            if self.flag(FLAG_H) || (self.registers.a & 0x0F) > 0x09 {
                adjust |= 0x06;
            }
            if carry || self.registers.a > 0x99 {
                adjust |= 0x60;
                next_carry = true;
            }
            self.registers.a = self.registers.a.wrapping_add(adjust);
        } else {
            if self.flag(FLAG_H) {
                adjust |= 0x06;
            }
            if carry {
                adjust |= 0x60;
            }
            self.registers.a = self.registers.a.wrapping_sub(adjust);
        }

        self.set_flag(FLAG_Z, self.registers.a == 0);
        self.set_flag(FLAG_H, false);
        self.set_flag(FLAG_C, next_carry);
    }

    fn and_a(&mut self, value: u8) {
        self.registers.a &= value;
        self.set_flag(FLAG_Z, self.registers.a == 0);
        self.set_flag(FLAG_N, false);
        self.set_flag(FLAG_H, true);
        self.set_flag(FLAG_C, false);
    }

    fn xor_a(&mut self, value: u8) {
        self.registers.a ^= value;
        self.set_flag(FLAG_Z, self.registers.a == 0);
        self.set_flag(FLAG_N, false);
        self.set_flag(FLAG_H, false);
        self.set_flag(FLAG_C, false);
    }

    fn or_a(&mut self, value: u8) {
        self.registers.a |= value;
        self.set_flag(FLAG_Z, self.registers.a == 0);
        self.set_flag(FLAG_N, false);
        self.set_flag(FLAG_H, false);
        self.set_flag(FLAG_C, false);
    }

    fn cp_a(&mut self, value: u8) {
        let a = self.registers.a;
        let (result, borrow) = a.overflowing_sub(value);
        let half = (a & 0x0F) < (value & 0x0F);

        self.set_flag(FLAG_Z, result == 0);
        self.set_flag(FLAG_N, true);
        self.set_flag(FLAG_H, half);
        self.set_flag(FLAG_C, borrow);
    }

    fn add_hl(&mut self, value: u16) {
        let hl = self.hl();
        let (result, carry) = hl.overflowing_add(value);
        let half = ((hl & 0x0FFF) + (value & 0x0FFF)) > 0x0FFF;
        self.set_hl(result);

        self.set_flag(FLAG_N, false);
        self.set_flag(FLAG_H, half);
        self.set_flag(FLAG_C, carry);
    }

    fn jr(&mut self, offset: i8) {
        self.registers.pc = self.registers.pc.wrapping_add_signed(offset as i16);
    }

    fn condition(&self, opcode: u8) -> bool {
        match (opcode >> 3) & 0x03 {
            0 => !self.flag(FLAG_Z),
            1 => self.flag(FLAG_Z),
            2 => !self.flag(FLAG_C),
            3 => self.flag(FLAG_C),
            _ => false,
        }
    }

    fn push16(&mut self, bus: &mut GbBus, value: u16) {
        self.registers.sp = self.registers.sp.wrapping_sub(1);
        bus.write8(self.registers.sp, (value >> 8) as u8);
        self.registers.sp = self.registers.sp.wrapping_sub(1);
        bus.write8(self.registers.sp, (value & 0x00FF) as u8);
    }

    fn pop16(&mut self, bus: &GbBus) -> u16 {
        let value = bus.read16(self.registers.sp);
        self.registers.sp = self.registers.sp.wrapping_add(2);
        value
    }

    fn flag(&self, flag: u8) -> bool {
        (self.registers.f & flag) != 0
    }

    fn set_flag(&mut self, flag: u8, enabled: bool) {
        if enabled {
            self.registers.f |= flag;
        } else {
            self.registers.f &= !flag;
        }
        self.registers.f &= 0xF0;
    }

    fn af(&self) -> u16 {
        ((self.registers.a as u16) << 8) | self.registers.f as u16
    }

    fn bc(&self) -> u16 {
        ((self.registers.b as u16) << 8) | self.registers.c as u16
    }

    fn de(&self) -> u16 {
        ((self.registers.d as u16) << 8) | self.registers.e as u16
    }

    fn hl(&self) -> u16 {
        ((self.registers.h as u16) << 8) | self.registers.l as u16
    }

    fn set_af(&mut self, value: u16) {
        self.registers.a = (value >> 8) as u8;
        self.registers.f = (value as u8) & 0xF0;
    }

    fn set_bc(&mut self, value: u16) {
        self.registers.b = (value >> 8) as u8;
        self.registers.c = value as u8;
    }

    fn set_de(&mut self, value: u16) {
        self.registers.d = (value >> 8) as u8;
        self.registers.e = value as u8;
    }

    fn set_hl(&mut self, value: u16) {
        self.registers.h = (value >> 8) as u8;
        self.registers.l = value as u8;
    }

    pub fn debug_pc(&self) -> u16 {
        self.registers.pc
    }

    pub fn debug_sp(&self) -> u16 {
        self.registers.sp
    }

    pub fn debug_af(&self) -> u16 {
        self.af()
    }

    pub fn debug_bc(&self) -> u16 {
        self.bc()
    }

    pub fn debug_de(&self) -> u16 {
        self.de()
    }

    pub fn debug_hl(&self) -> u16 {
        self.hl()
    }

    pub fn debug_ime(&self) -> bool {
        self.ime
    }

    pub fn debug_halted(&self) -> bool {
        self.halted
    }
}

fn fallback_opcode_len(opcode: u8) -> u8 {
    match opcode {
        0x01 | 0x08 | 0x11 | 0x21 | 0x31 | 0xC2 | 0xC3 | 0xC4 | 0xCA | 0xCC | 0xCD | 0xD2
        | 0xD4 | 0xDA | 0xDC | 0xEA | 0xFA => 3,
        0x06 | 0x0E | 0x16 | 0x18 | 0x1E | 0x20 | 0x26 | 0x28 | 0x2E | 0x30 | 0x36 | 0x38
        | 0x3E | 0xC6 | 0xCB | 0xD6 | 0xE0 | 0xE6 | 0xEE | 0xF0 | 0xF6 | 0xFE => 2,
        _ => 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_rom(program: &[u8]) -> Vec<u8> {
        let mut rom = vec![0x00; 0x8000];
        rom[0x0147] = 0x00;
        rom[0x0148] = 0x00;
        rom[0x0149] = 0x00;
        rom[0x0100..0x0100 + program.len()].copy_from_slice(program);
        rom
    }

    fn run_until_halt(cpu: &mut GbCpu, bus: &mut GbBus, max_steps: usize) {
        for _ in 0..max_steps {
            cpu.step(bus);
            if cpu.halted {
                return;
            }
        }
        panic!("CPU did not halt within step limit");
    }

    #[test]
    fn cgb_reset_uses_cgb_boot_registers() {
        let mut cpu = GbCpu::default();
        cpu.reset_for_model(true);

        assert_eq!(cpu.registers.a, 0x11);
        assert_eq!(cpu.registers.f, 0x80);
        assert_eq!(cpu.registers.d, 0xFF);
        assert_eq!(cpu.registers.e, 0x56);
        assert_eq!(cpu.registers.sp, 0xFFFE);
        assert_eq!(cpu.registers.pc, 0x0100);
    }

    #[test]
    fn executes_add_and_store_program() {
        let mut bus = GbBus::default();
        let rom = make_test_rom(&[
            0x3E, 0x12, // LD A,0x12
            0x06, 0x08, // LD B,0x08
            0x80, // ADD A,B
            0xEA, 0x00, 0xC0, // LD (0xC000),A
            0x76, // HALT
        ]);
        bus.load_cartridge(&rom).expect("ROM should load");

        let mut cpu = GbCpu::default();
        cpu.reset();
        run_until_halt(&mut cpu, &mut bus, 16);

        assert_eq!(bus.read8(0xC000), 0x1A);
        assert_eq!(cpu.registers.a, 0x1A);
        assert!(!cpu.flag(FLAG_Z));
    }

    #[test]
    fn call_and_ret_store_result() {
        let mut bus = GbBus::default();
        let rom = make_test_rom(&[
            0xCD, 0x08, 0x01, // CALL 0x0108
            0xEA, 0x00, 0xC0, // LD (0xC000),A
            0x76, // HALT
            0x00, // padding
            0x3E, 0x42, // LD A,0x42
            0xC9, // RET
        ]);
        bus.load_cartridge(&rom).expect("ROM should load");

        let mut cpu = GbCpu::default();
        cpu.reset();
        run_until_halt(&mut cpu, &mut bus, 32);

        assert_eq!(bus.read8(0xC000), 0x42);
        assert_eq!(cpu.registers.sp, 0xFFFE);
    }

    #[test]
    fn jr_z_takes_branch_when_zero_flag_set() {
        let mut bus = GbBus::default();
        let rom = make_test_rom(&[
            0xAF, // XOR A
            0x28, 0x02, // JR Z,+2
            0x3E, 0x11, // LD A,0x11 (skip)
            0x3E, 0x22, // LD A,0x22
            0xEA, 0x00, 0xC0, // LD (0xC000),A
            0x76, // HALT
        ]);
        bus.load_cartridge(&rom).expect("ROM should load");

        let mut cpu = GbCpu::default();
        cpu.reset();
        run_until_halt(&mut cpu, &mut bus, 32);

        assert_eq!(bus.read8(0xC000), 0x22);
    }

    #[test]
    fn timer_interrupt_pushes_pc_and_jumps_to_vector() {
        let mut bus = GbBus::default();
        let rom = make_test_rom(&[0x00, 0x76]);
        bus.load_cartridge(&rom).expect("ROM should load");

        let mut cpu = GbCpu::default();
        cpu.reset();
        cpu.ime = true;

        bus.write8(0xFFFF, INT_TIMER);
        bus.request_interrupt(INT_TIMER);

        let cycles = cpu.step(&mut bus);
        assert_eq!(cycles, 20);
        assert_eq!(cpu.registers.pc, 0x0050);
        assert_eq!(cpu.registers.sp, 0xFFFC);
        assert_eq!(bus.read16(0xFFFC), 0x0100);
        assert_eq!(bus.pending_interrupts() & INT_TIMER, 0);
    }

    #[test]
    fn reti_restores_pc_and_reenables_ime() {
        let mut bus = GbBus::default();
        let mut rom = make_test_rom(&[0x00, 0x76]);
        rom[0x0050] = 0xD9; // RETI
        bus.load_cartridge(&rom).expect("ROM should load");

        let mut cpu = GbCpu::default();
        cpu.reset();
        cpu.ime = true;

        bus.write8(0xFFFF, INT_TIMER);
        bus.request_interrupt(INT_TIMER);
        cpu.step(&mut bus);

        let cycles = cpu.step(&mut bus);
        assert_eq!(cycles, 16);
        assert_eq!(cpu.registers.pc, 0x0100);
        assert_eq!(cpu.registers.sp, 0xFFFE);
        assert!(cpu.ime);
    }

    #[test]
    fn halt_resumes_when_interrupt_pending_with_ime_off() {
        let mut bus = GbBus::default();
        let rom = make_test_rom(&[
            0x76, // HALT
            0x00, // NOP
            0x76, // HALT
        ]);
        bus.load_cartridge(&rom).expect("ROM should load");

        let mut cpu = GbCpu::default();
        cpu.reset();
        cpu.ime = false;

        cpu.step(&mut bus);
        assert!(cpu.halted);

        bus.write8(0xFFFF, INT_TIMER);
        bus.request_interrupt(INT_TIMER);

        let cycles = cpu.step(&mut bus);
        assert_eq!(cycles, 4);
        assert!(!cpu.halted);
        assert_eq!(cpu.registers.pc, 0x0102);
    }

    #[test]
    fn ei_enables_ime_after_one_instruction_delay() {
        let mut bus = GbBus::default();
        let rom = make_test_rom(&[
            0xFB, // EI
            0x00, // NOP
            0x76, // HALT
        ]);
        bus.load_cartridge(&rom).expect("ROM should load");

        let mut cpu = GbCpu::default();
        cpu.reset();
        assert!(!cpu.ime);

        cpu.step(&mut bus);
        assert!(!cpu.ime);
        assert_eq!(cpu.registers.pc, 0x0101);

        cpu.step(&mut bus);
        assert!(cpu.ime);
        assert_eq!(cpu.registers.pc, 0x0102);
    }

    #[test]
    fn di_cancels_pending_ei_enable() {
        let mut bus = GbBus::default();
        let rom = make_test_rom(&[
            0xFB, // EI
            0xF3, // DI
            0x00, // NOP
            0x76, // HALT
        ]);
        bus.load_cartridge(&rom).expect("ROM should load");

        let mut cpu = GbCpu::default();
        cpu.reset();

        cpu.step(&mut bus);
        assert!(!cpu.ime);

        cpu.step(&mut bus);
        assert!(!cpu.ime);

        cpu.step(&mut bus);
        assert!(!cpu.ime);
    }

    #[test]
    fn halt_bug_repeats_next_opcode_fetch() {
        let mut bus = GbBus::default();
        let rom = make_test_rom(&[
            0x76, // HALT (with IME=0 + pending interrupt => HALT bug)
            0x3C, // INC A
            0xEA, 0x00, 0xC0, // LD (0xC000), A
            0x76, // HALT
        ]);
        bus.load_cartridge(&rom).expect("ROM should load");

        let mut cpu = GbCpu::default();
        cpu.reset();
        cpu.ime = false;

        bus.write8(0xFFFF, INT_TIMER);
        bus.request_interrupt(INT_TIMER);

        cpu.step(&mut bus);
        assert!(!cpu.halted);
        assert_eq!(cpu.registers.pc, 0x0101);

        cpu.step(&mut bus);
        assert_eq!(cpu.registers.a, 0x02);
        assert_eq!(cpu.registers.pc, 0x0101);

        cpu.step(&mut bus);
        assert_eq!(cpu.registers.a, 0x03);
        assert_eq!(cpu.registers.pc, 0x0102);
    }

    #[test]
    fn interrupt_is_taken_after_instruction_following_ei() {
        let mut bus = GbBus::default();
        let rom = make_test_rom(&[
            0xFB, // EI
            0x00, // NOP
            0x00, // NOP (interrupt should be taken before this)
            0x76, // HALT
        ]);
        bus.load_cartridge(&rom).expect("ROM should load");

        let mut cpu = GbCpu::default();
        cpu.reset();
        bus.write8(0xFFFF, INT_VBLANK);
        bus.request_interrupt(INT_VBLANK);

        let cycles1 = cpu.step(&mut bus);
        assert_eq!(cycles1, 4);
        assert_eq!(cpu.registers.pc, 0x0101);
        assert!(!cpu.ime);

        let cycles2 = cpu.step(&mut bus);
        assert_eq!(cycles2, 4);
        assert_eq!(cpu.registers.pc, 0x0102);
        assert!(cpu.ime);

        let cycles3 = cpu.step(&mut bus);
        assert_eq!(cycles3, 20);
        assert_eq!(cpu.registers.pc, 0x0040);
        assert!(!cpu.ime);
    }
}
