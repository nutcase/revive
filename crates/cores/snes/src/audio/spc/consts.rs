/// Constants and types formerly provided by the external `spc` crate.
///
/// Only the items actually referenced by the APU/DSP code are reproduced here.

pub const RAM_LEN: usize = 0x10000;
pub const REG_LEN: usize = 128;
pub const IPL_ROM_LEN: usize = 64;

/// Minimal SPC state container used by `Apu::set_state()` and `Dsp::set_state()`.
pub struct Spc {
    pub pc: u16,
    pub a: u8,
    pub x: u8,
    pub y: u8,
    pub psw: u8,
    pub sp: u8,
    pub ram: [u8; RAM_LEN],
    pub regs: [u8; REG_LEN],
    pub ipl_rom: [u8; IPL_ROM_LEN],
}
