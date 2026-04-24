use crate::cartridge::Cartridge;
use crate::cpu::{CCR_C, CCR_N, CCR_V, CCR_X, CCR_Z, M68k, SR_INT_MASK, SR_SUPERVISOR};
use crate::memory::MemoryMap;

#[path = "cpu_tests/arithmetic_logic.rs"]
mod arithmetic_logic;
#[path = "cpu_tests/branch_control.rs"]
mod branch_control;
#[path = "cpu_tests/misc_ops.rs"]
mod misc_ops;
#[path = "cpu_tests/move_ops.rs"]
mod move_ops;
#[path = "cpu_tests/system_exception.rs"]
mod system_exception;
