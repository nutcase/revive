use super::{
    parse_save_state_gsu_recent_exec_tail, parse_save_state_gsu_reg_eq,
    parse_save_state_superfx_ram_byte_eq, recent_exec_trace_ends_with,
    set_trace_superfx_exec_frame, SaveStateGsuRegEq, SaveStateSuperfxRamByteEq, StopSnapshot,
    SuperFx, SuperFxExecTrace, SuperFxRamWrite, SuperFxRegWrite, DEFAULT_SUPERFX_RATIO_FAST,
    DEFAULT_SUPERFX_RATIO_SLOW, SCMR_RAN_BIT, SCMR_RON_BIT, SFR_ALT1_BIT, SFR_ALT2_BIT, SFR_B_BIT,
    SFR_CY_BIT, SFR_GO_BIT, SFR_IRQ_BIT, SFR_OV_BIT, SFR_R_BIT, SFR_S_BIT, SFR_Z_BIT,
};
use std::sync::{Mutex, OnceLock};

fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

mod diagnostics;
mod instructions;
mod lifecycle;
mod memory;
mod registers;
mod runtime;
mod starfox_paths;
mod timing;
