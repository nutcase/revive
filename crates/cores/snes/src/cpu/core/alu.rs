use super::{set_flags_nz_16, set_flags_nz_8, CoreState};
use crate::{cpu::bus::CpuBus, cpu::StatusFlags};

#[inline]
pub(super) fn memory_is_8bit(state: &CoreState) -> bool {
    state.emulation_mode || state.p.contains(StatusFlags::MEMORY_8BIT)
}

#[inline]
pub(super) fn index_is_8bit(state: &CoreState) -> bool {
    state.emulation_mode || state.p.contains(StatusFlags::INDEX_8BIT)
}

pub(super) fn write_a_generic<T: CpuBus>(state: &CoreState, bus: &mut T, addr: u32) {
    if memory_is_8bit(state) {
        bus.write_u8(addr, (state.a & 0xFF) as u8);
    } else {
        bus.write_u16(addr, state.a);
    }
}

pub(super) fn write_x_generic<T: CpuBus>(state: &CoreState, bus: &mut T, addr: u32) {
    if index_is_8bit(state) {
        bus.write_u8(addr, (state.x & 0xFF) as u8);
    } else {
        bus.write_u16(addr, state.x);
    }
}

pub(super) fn write_y_generic<T: CpuBus>(state: &CoreState, bus: &mut T, addr: u32) {
    if index_is_8bit(state) {
        bus.write_u8(addr, (state.y & 0xFF) as u8);
    } else {
        bus.write_u16(addr, state.y);
    }
}

pub(super) fn set_flags_index(state: &mut CoreState, value: u16) {
    if index_is_8bit(state) {
        set_flags_nz_8(state, (value & 0xFF) as u8);
    } else {
        set_flags_nz_16(state, value);
    }
}

pub(super) fn cmp_operand(state: &mut CoreState, operand: u16) {
    if memory_is_8bit(state) {
        let a = (state.a & 0xFF) as u8;
        let value = (operand & 0xFF) as u8;
        let result = a.wrapping_sub(value);
        state.p.set(StatusFlags::CARRY, a >= value);
        state.p.set(StatusFlags::ZERO, result == 0);
        state.p.set(StatusFlags::NEGATIVE, (result & 0x80) != 0);
    } else {
        let result = state.a.wrapping_sub(operand);
        state.p.set(StatusFlags::CARRY, state.a >= operand);
        state.p.set(StatusFlags::ZERO, result == 0);
        state.p.set(StatusFlags::NEGATIVE, (result & 0x8000) != 0);
    }
}

pub(super) fn read_operand_m<T: CpuBus>(
    _state: &CoreState,
    bus: &mut T,
    addr: u32,
    memory_8bit: bool,
) -> u16 {
    if memory_8bit {
        bus.read_u8(addr) as u16
    } else {
        bus.read_u16(addr)
    }
}

pub(super) fn ora_operand(state: &mut CoreState, operand: u16) {
    if memory_is_8bit(state) {
        let result = ((state.a & 0xFF) | (operand & 0xFF)) as u8;
        state.a = (state.a & 0xFF00) | (result as u16);
        set_flags_nz_8(state, result);
    } else {
        state.a |= operand;
        set_flags_nz_16(state, state.a);
    }
}

pub(super) fn and_operand(state: &mut CoreState, operand: u16) {
    if memory_is_8bit(state) {
        let result = ((state.a & 0xFF) & (operand & 0xFF)) as u8;
        state.a = (state.a & 0xFF00) | (result as u16);
        set_flags_nz_8(state, result);
    } else {
        state.a &= operand;
        set_flags_nz_16(state, state.a);
    }
}

pub(super) fn eor_operand(state: &mut CoreState, operand: u16) {
    if memory_is_8bit(state) {
        let result = ((state.a & 0xFF) ^ (operand & 0xFF)) as u8;
        state.a = (state.a & 0xFF00) | (result as u16);
        set_flags_nz_8(state, result);
    } else {
        state.a ^= operand;
        set_flags_nz_16(state, state.a);
    }
}

pub(super) fn modify_memory<T: CpuBus, F8, F16>(
    state: &mut CoreState,
    bus: &mut T,
    addr: u32,
    memory_8bit: bool,
    mut modify8: F8,
    mut modify16: F16,
) where
    F8: FnMut(&mut CoreState, u8) -> u8,
    F16: FnMut(&mut CoreState, u16) -> u16,
{
    if memory_8bit {
        let value = bus.read_u8(addr);
        let result = modify8(state, value);
        bus.write_u8(addr, result);
    } else {
        let value = bus.read_u16(addr);
        let result = modify16(state, value);
        bus.write_u16(addr, result);
    }
}

pub(super) fn asl8(state: &mut CoreState, value: u8) -> u8 {
    state.p.set(StatusFlags::CARRY, value & 0x80 != 0);
    let result = value << 1;
    set_flags_nz_8(state, result);
    result
}

pub(super) fn asl16(state: &mut CoreState, value: u16) -> u16 {
    state.p.set(StatusFlags::CARRY, value & 0x8000 != 0);
    let result = value << 1;
    set_flags_nz_16(state, result);
    result
}

pub(super) fn lsr8(state: &mut CoreState, value: u8) -> u8 {
    state.p.set(StatusFlags::CARRY, value & 0x01 != 0);
    let result = value >> 1;
    set_flags_nz_8(state, result);
    result
}

pub(super) fn lsr16(state: &mut CoreState, value: u16) -> u16 {
    state.p.set(StatusFlags::CARRY, value & 0x0001 != 0);
    let result = value >> 1;
    set_flags_nz_16(state, result);
    result
}

pub(super) fn rol8(state: &mut CoreState, value: u8) -> u8 {
    let carry_in = if state.p.contains(StatusFlags::CARRY) {
        1
    } else {
        0
    };
    state.p.set(StatusFlags::CARRY, value & 0x80 != 0);
    let result = (value << 1) | carry_in;
    set_flags_nz_8(state, result);
    result
}

pub(super) fn rol16(state: &mut CoreState, value: u16) -> u16 {
    let carry_in = if state.p.contains(StatusFlags::CARRY) {
        1
    } else {
        0
    };
    state.p.set(StatusFlags::CARRY, value & 0x8000 != 0);
    let result = (value << 1) | carry_in;
    set_flags_nz_16(state, result);
    result
}

pub(super) fn ror8(state: &mut CoreState, value: u8) -> u8 {
    let carry_in = if state.p.contains(StatusFlags::CARRY) {
        0x80
    } else {
        0
    };
    state.p.set(StatusFlags::CARRY, value & 0x01 != 0);
    let result = (value >> 1) | carry_in;
    set_flags_nz_8(state, result);
    result
}

pub(super) fn ror16(state: &mut CoreState, value: u16) -> u16 {
    let carry_in = if state.p.contains(StatusFlags::CARRY) {
        0x8000
    } else {
        0
    };
    state.p.set(StatusFlags::CARRY, value & 0x0001 != 0);
    let result = (value >> 1) | carry_in;
    set_flags_nz_16(state, result);
    result
}

fn bit_set_z(state: &mut CoreState, operand: u16) {
    let memory_8bit = memory_is_8bit(state);
    let mask = if memory_8bit { 0x00FF } else { 0xFFFF };
    let masked_a = state.a & mask;
    let masked_op = operand & mask;
    state.p.set(StatusFlags::ZERO, (masked_a & masked_op) == 0);
}

pub(super) fn bit_operand_immediate(state: &mut CoreState, operand: u16) {
    // BIT immediate affects Z only. N/V are not modified.
    bit_set_z(state, operand);
}

pub(super) fn bit_operand_memory(state: &mut CoreState, operand: u16) {
    // BIT (memory) affects Z and also loads N/V from operand.
    // - 8-bit A (M=1 or E=1): N/V from bits 7/6
    // - 16-bit A (M=0 and E=0): N/V from bits 15/14
    let memory_8bit = memory_is_8bit(state);
    bit_set_z(state, operand);
    if memory_8bit {
        state.p.set(StatusFlags::NEGATIVE, (operand & 0x0080) != 0);
        state.p.set(StatusFlags::OVERFLOW, (operand & 0x0040) != 0);
    } else {
        state.p.set(StatusFlags::NEGATIVE, (operand & 0x8000) != 0);
        state.p.set(StatusFlags::OVERFLOW, (operand & 0x4000) != 0);
    }

    // cputest BIT $4210 ループ調査用: Pフラグをダンプして早期終了
    if crate::debug_flags::debug_bit4210() {
        println!(
            "[BIT4210] operand=0x{:04X} P=0x{:02X} N={} V={} Z={} C={}",
            operand,
            state.p.bits(),
            state.p.contains(StatusFlags::NEGATIVE),
            state.p.contains(StatusFlags::OVERFLOW),
            state.p.contains(StatusFlags::ZERO),
            state.p.contains(StatusFlags::CARRY)
        );
        std::process::exit(0);
    }
    // デバッグ: RDNMI が立った瞬間に終了して状態を観察したい場合（bit7）
    if crate::debug_flags::exit_on_bit82() && (operand & 0x0080) != 0 {
        println!(
            "[BIT82] operand=0x{:04X} P=0x{:02X} A=0x{:04X} PC={:04X} DB={:02X}",
            operand,
            state.p.bits(),
            state.a,
            state.pc,
            state.db
        );
        std::process::exit(0);
    }
}

#[inline]
fn bcd_adc8(a: u8, b: u8, carry_in: u8) -> (u8, bool) {
    // W65C816 decimal adjust (works for invalid BCD digits as well):
    // - low adjust: based on low-nibble sum > 9
    // - high adjust: based on the *binary* sum > 0x99
    let sum = a as u16 + b as u16 + carry_in as u16;
    let low = (a & 0x0F) as u16 + (b & 0x0F) as u16 + carry_in as u16;
    let mut adjust = 0u16;
    if low > 0x09 {
        adjust += 0x06;
    }
    if sum > 0x99 {
        adjust += 0x60;
    }
    let result = sum.wrapping_add(adjust);
    ((result & 0xFF) as u8, sum > 0x99)
}

#[inline]
fn bcd_sbc8(a: u8, b: u8, borrow_in: u8) -> (u8, bool) {
    // W65C816 decimal adjust (works for invalid BCD digits as well):
    // - low adjust: based on low-nibble borrow
    // - high adjust: based on the *binary* borrow (result < 0)
    let diff = (a as i16) - (b as i16) - (borrow_in as i16);
    let low = (a & 0x0F) as i16 - (b & 0x0F) as i16 - (borrow_in as i16);
    let mut adjust = 0i16;
    if low < 0 {
        adjust -= 0x06;
    }
    if diff < 0 {
        adjust -= 0x60;
    }
    let result = (diff + adjust) as u8;
    let carry_out = diff >= 0;
    (result, carry_out)
}

pub(super) fn adc_generic(state: &mut CoreState, operand: u16) {
    let carry_in = if state.p.contains(StatusFlags::CARRY) {
        1
    } else {
        0
    };
    let memory_8bit = state.emulation_mode || state.p.contains(StatusFlags::MEMORY_8BIT);
    let original_a = state.a;

    if state.p.contains(StatusFlags::DECIMAL) {
        if memory_8bit {
            let a8 = (original_a & 0x00FF) as u8;
            let b8 = (operand & 0x00FF) as u8;
            // Overflow in decimal mode (W65C816): computed like binary-mode, but using the
            // intermediate result after the low-nibble adjust and *before* the high adjust.
            let sum = a8 as u16 + b8 as u16 + carry_in as u16;
            let low = (a8 & 0x0F) as u16 + (b8 & 0x0F) as u16 + carry_in as u16;
            let res_v = (sum.wrapping_add(if low > 0x09 { 0x06 } else { 0x00 }) & 0xFF) as u8;
            let (res, carry_out) = bcd_adc8(a8, b8, carry_in as u8);
            state.p.set(StatusFlags::CARRY, carry_out);
            let overflow = ((!(a8 ^ b8)) & (a8 ^ res_v) & 0x80) != 0;
            state.p.set(StatusFlags::OVERFLOW, overflow);
            state.a = (original_a & 0xFF00) | (res as u16);
        } else {
            let a = original_a;
            let b = operand;
            let a_lo = (a & 0x00FF) as u8;
            let b_lo = (b & 0x00FF) as u8;
            let a_hi = (a >> 8) as u8;
            let b_hi = (b >> 8) as u8;

            let (lo, carry1) = bcd_adc8(a_lo, b_lo, carry_in as u8);
            let (hi, carry2) = bcd_adc8(a_hi, b_hi, carry1 as u8);
            state.p.set(StatusFlags::CARRY, carry2);
            let result16 = ((hi as u16) << 8) | (lo as u16);

            // See 8-bit case above: V uses the intermediate result after low-nibble adjust only.
            let sum_lo = a_lo as u16 + b_lo as u16 + carry_in as u16;
            let low_lo = (a_lo & 0x0F) as u16 + (b_lo & 0x0F) as u16 + carry_in as u16;
            let lo_v = (sum_lo.wrapping_add(if low_lo > 0x09 { 0x06 } else { 0x00 }) & 0xFF) as u8;
            let carry1_v = sum_lo > 0x99;
            let sum_hi = a_hi as u16 + b_hi as u16 + carry1_v as u16;
            let low_hi = (a_hi & 0x0F) as u16 + (b_hi & 0x0F) as u16 + carry1_v as u16;
            let hi_v = (sum_hi.wrapping_add(if low_hi > 0x09 { 0x06 } else { 0x00 }) & 0xFF) as u8;
            let result_v = ((hi_v as u16) << 8) | (lo_v as u16);

            let overflow = (((!(a ^ b)) & (a ^ result_v)) & 0x8000) != 0;
            state.p.set(StatusFlags::OVERFLOW, overflow);
            state.a = result16;
        }
    } else if memory_8bit {
        let a8 = (original_a & 0x00FF) as u8;
        let b8 = (operand & 0x00FF) as u8;
        let sum = (a8 as u16) + (b8 as u16) + (carry_in as u16);
        let res8 = (sum & 0x00FF) as u8;
        state.p.set(StatusFlags::CARRY, sum > 0x00FF);
        let overflow = ((!(a8 ^ b8)) & (a8 ^ res8) & 0x80) != 0;
        state.p.set(StatusFlags::OVERFLOW, overflow);
        state.a = (original_a & 0xFF00) | (res8 as u16);
    } else {
        let result = (original_a as u32) + (operand as u32) + (carry_in as u32);
        state.p.set(StatusFlags::CARRY, result > 0xFFFF);
        let overflow =
            ((original_a ^ operand) & 0x8000) == 0 && ((original_a ^ result as u16) & 0x8000) != 0;
        state.p.set(StatusFlags::OVERFLOW, overflow);
        state.a = (result & 0xFFFF) as u16;
    }

    if memory_8bit {
        set_flags_nz_8(state, (state.a & 0x00FF) as u8);
    } else {
        set_flags_nz_16(state, state.a);
    }
}

pub(super) fn sbc_generic(state: &mut CoreState, operand: u16) {
    let carry_clear = if state.p.contains(StatusFlags::CARRY) {
        0
    } else {
        1
    };
    let memory_8bit = state.emulation_mode || state.p.contains(StatusFlags::MEMORY_8BIT);
    let original_a = state.a;

    if state.p.contains(StatusFlags::DECIMAL) {
        if memory_8bit {
            let a8 = (original_a & 0xFF) as u8;
            let b8 = (operand & 0xFF) as u8;
            let binary = (a8 as i16) - (b8 as i16) - (carry_clear as i16);
            let (res, borrow) = bcd_sbc8(a8, b8, carry_clear as u8);
            state.p.set(StatusFlags::CARRY, borrow);
            // In decimal mode, V follows the binary subtraction overflow rule based on the
            // binary (pre-BCD-adjust) result.
            let result8 = binary as u8;
            let overflow = ((a8 ^ b8) & (a8 ^ result8) & 0x80) != 0;
            state.p.set(StatusFlags::OVERFLOW, overflow);
            state.a = (original_a & 0xFF00) | (res as u16);
        } else {
            let a = original_a;
            let b = operand;
            let binary = (a as i32) - (b as i32) - carry_clear;
            let (lo, borrow_lo) =
                bcd_sbc8((a & 0x00FF) as u8, (b & 0x00FF) as u8, carry_clear as u8);
            let (hi, borrow_hi) = bcd_sbc8((a >> 8) as u8, (b >> 8) as u8, (!borrow_lo) as u8);
            state.p.set(StatusFlags::CARRY, borrow_hi);
            let result16 = binary as u16;
            let overflow = ((a ^ b) & (a ^ result16) & 0x8000) != 0;
            state.p.set(StatusFlags::OVERFLOW, overflow);
            state.a = ((hi as u16) << 8) | (lo as u16);
        }
    } else if memory_8bit {
        let a8 = (original_a & 0x00FF) as u8;
        let b8 = (operand & 0x00FF) as u8;
        let diff = (a8 as i16) - (b8 as i16) - (carry_clear as i16);
        let res8 = diff as u8;
        state.p.set(StatusFlags::CARRY, diff >= 0);
        let overflow = ((a8 ^ b8) & (a8 ^ res8) & 0x80) != 0;
        state.p.set(StatusFlags::OVERFLOW, overflow);
        state.a = (original_a & 0xFF00) | (res8 as u16);
    } else {
        let result = (original_a as i32) - (operand as i32) - carry_clear;
        state.p.set(StatusFlags::CARRY, result >= 0);
        let overflow =
            ((original_a ^ operand) & 0x8000) != 0 && ((original_a ^ result as u16) & 0x8000) != 0;
        state.p.set(StatusFlags::OVERFLOW, overflow);
        state.a = result as u16;
    }

    if memory_8bit {
        set_flags_nz_8(state, (state.a & 0xFF) as u8);
    } else {
        set_flags_nz_16(state, state.a);
    }
}
