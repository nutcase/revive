use bitflags::bitflags;

mod instruction_add_sub;
mod instruction_addressing;
mod instruction_compare;
mod instruction_control;
mod instruction_inc_dec;
mod instruction_load_store;
mod instruction_logic;
mod instruction_memory;
mod instruction_shift;
mod opcode;
mod opcode_unofficial;
mod opcode_unofficial_immediate;
mod opcode_unofficial_inc_dec;
mod opcode_unofficial_load;
mod opcode_unofficial_nop;
mod opcode_unofficial_rotate;
mod opcode_unofficial_shift;
mod opcode_unofficial_store;
mod runtime;
#[cfg(test)]
mod tests;

bitflags! {
    #[derive(Debug, Clone, Copy)]
    pub struct StatusFlags: u8 {
        const CARRY = 0b00000001;
        const ZERO = 0b00000010;
        const INTERRUPT_DISABLE = 0b00000100;
        const DECIMAL = 0b00001000;
        const BREAK = 0b00010000;
        const UNUSED = 0b00100000;
        const OVERFLOW = 0b01000000;
        const NEGATIVE = 0b10000000;
    }
}

pub struct Cpu {
    pub a: u8,   // Accumulator
    pub x: u8,   // X register
    pub y: u8,   // Y register
    pub sp: u8,  // Stack pointer
    pub pc: u16, // Program counter
    pub status: StatusFlags,
    cycles: u64,
    halted: bool,
    rts_count: u32,   // Counter for consecutive RTS calls at same PC
    last_rts_pc: u16, // Last PC where RTS was executed
}

pub trait CpuBus {
    fn on_reset(&mut self) {}
    fn read(&mut self, addr: u16) -> u8;
    fn write(&mut self, addr: u16, data: u8);
}
