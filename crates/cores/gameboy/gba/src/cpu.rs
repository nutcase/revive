use crate::bus::{GbaBus, IRQ_KEYPAD};
use crate::state::{StateReader, StateWriter};
#[cfg(feature = "runtime-debug-trace")]
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU32, Ordering};

fn write_option_u32(w: &mut StateWriter, v: Option<u32>) {
    w.write_bool(v.is_some());
    if let Some(val) = v {
        w.write_u32(val);
    }
}

fn read_option_u32(r: &mut StateReader) -> Result<Option<u32>, &'static str> {
    if r.read_bool()? {
        Ok(Some(r.read_u32()?))
    } else {
        Ok(None)
    }
}

const FLAG_N: u32 = 1 << 31;
const FLAG_Z: u32 = 1 << 30;
const FLAG_C: u32 = 1 << 29;
const FLAG_V: u32 = 1 << 28;
const FLAG_I: u32 = 1 << 7;
const FLAG_T: u32 = 1 << 5;

const MODE_MASK: u32 = 0x1F;
const MODE_USR: u32 = 0x10;
const MODE_IRQ: u32 = 0x12;
const MODE_SVC: u32 = 0x13;
const MODE_UND: u32 = 0x1B;
const MODE_SYS: u32 = 0x1F;
const NO_BIOS_IRQ_RETURN_TOKEN: u32 = 0xFFFF_FFF1;
const NO_BIOS_IRQ_RETURN_TOKEN_SUBS_PC_LR_4: u32 = NO_BIOS_IRQ_RETURN_TOKEN.wrapping_sub(4);
const NO_BIOS_INTR_CHECK_ADDR: u32 = 0x0300_7FF8;
const NO_BIOS_IRQ_HANDLER_ADDR: u32 = 0x0300_7FFC;
const SOUND_JUMP_LIST_BASE: u32 = 0x0203_FF80;
const SOUND_JUMP_LIST_BYTES: u32 = 0x120;
const SOUND_JUMP_LIST_START_SWI: u32 = 0x1A;
const SOUND_JUMP_LIST_END_SWI: u32 = 0x2A;
const SOUND_DRIVER_MAIN_SWI: u32 = 0x1C;
const REG_SOUNDCNT_H: u32 = 0x0400_0082;
const REG_SOUNDCNT_X: u32 = 0x0400_0084;
const REG_SOUNDBIAS: u32 = 0x0400_0088;
const FIFO_A_ADDR: u32 = 0x0400_00A0;
const FIFO_B_ADDR: u32 = 0x0400_00A4;
const SOUND_REG_START: u32 = 0x0400_0060;
const SOUND_REG_END: u32 = 0x0400_009F;
const IRQ_GAMEPAK: u16 = 1 << 13;
const STOP_WAKE_IRQ_MASK: u16 = IRQ_KEYPAD | IRQ_GAMEPAK;

const TRACE_LIMIT_DEFAULT: u32 = 64;

static TRACE_SWI_COUNT: AtomicU32 = AtomicU32::new(0);
static TRACE_UNKNOWN_COUNT: AtomicU32 = AtomicU32::new(0);
static TRACE_UNHANDLED_SWI_COUNT: AtomicU32 = AtomicU32::new(0);
static TRACE_BRANCH_COUNT: AtomicU32 = AtomicU32::new(0);
static TRACE_IRQ_COUNT: AtomicU32 = AtomicU32::new(0);
static TRACE_IRQ_CODE_COUNT: AtomicU32 = AtomicU32::new(0);
static TRACE_BAD_PC_COUNT: AtomicU32 = AtomicU32::new(0);

#[cfg(feature = "runtime-debug-trace")]
static TRACE_SWI_ENABLED: OnceLock<bool> = OnceLock::new();
#[cfg(feature = "runtime-debug-trace")]
static TRACE_UNKNOWN_ENABLED: OnceLock<bool> = OnceLock::new();
#[cfg(feature = "runtime-debug-trace")]
static TRACE_BRANCH_ENABLED: OnceLock<bool> = OnceLock::new();
#[cfg(feature = "runtime-debug-trace")]
static TRACE_IRQ_ENABLED: OnceLock<bool> = OnceLock::new();
#[cfg(feature = "runtime-debug-trace")]
static TRACE_IRQ_CODE_ENABLED: OnceLock<bool> = OnceLock::new();
#[cfg(feature = "runtime-debug-trace")]
static TRACE_BAD_PC_ENABLED: OnceLock<bool> = OnceLock::new();
#[cfg(feature = "runtime-debug-trace")]
static TRACE_IRQ_PTR_ENABLED: OnceLock<bool> = OnceLock::new();
#[cfg(feature = "runtime-debug-trace")]
static TRACE_SP_ENABLED: OnceLock<bool> = OnceLock::new();
#[cfg(feature = "runtime-debug-trace")]
static TRACE_PC_TARGET: OnceLock<Option<u32>> = OnceLock::new();
#[cfg(feature = "runtime-debug-trace")]
static TRACE_PC_RANGE_START: OnceLock<Option<u32>> = OnceLock::new();
#[cfg(feature = "runtime-debug-trace")]
static TRACE_PC_RANGE_END: OnceLock<Option<u32>> = OnceLock::new();
static TRACE_PC_COUNT: AtomicU32 = AtomicU32::new(0);
static TRACE_PC_MATCH_COUNT: AtomicU32 = AtomicU32::new(0);
#[cfg(feature = "runtime-debug-trace")]
static TRACE_PC_SKIP: OnceLock<u32> = OnceLock::new();
#[cfg(feature = "runtime-debug-trace")]
static TRACE_PC_SP_MIN: OnceLock<Option<u32>> = OnceLock::new();
static TRACE_SP_COUNT: AtomicU32 = AtomicU32::new(0);
static TRACE_IRQ_PTR_COUNT: AtomicU32 = AtomicU32::new(0);
#[cfg(feature = "runtime-debug-trace")]
static TRACE_SP_MIN: OnceLock<Option<u32>> = OnceLock::new();
#[cfg(feature = "runtime-debug-trace")]
static TRACE_LIMIT: OnceLock<u32> = OnceLock::new();
#[cfg(feature = "runtime-debug-trace")]
static TRACE_STEP_HOOKS: OnceLock<TraceStepHooks> = OnceLock::new();

#[derive(Debug, Clone, Copy)]
struct TraceStepHooks {
    pc: bool,
    sp: bool,
    bad_pc: bool,
    irq_ptr: bool,
}

#[cfg(not(feature = "runtime-debug-trace"))]
static TRACE_STEP_HOOKS_DISABLED: TraceStepHooks = TraceStepHooks {
    pc: false,
    sp: false,
    bad_pc: false,
    irq_ptr: false,
};

#[derive(Debug, Clone, Copy)]
struct NoBiosIrqState {
    saved_cpsr: u32,
    resume_pc: u32,
    saved_regs: [u32; 15], // r0-r14
    saved_irq_sp: u32,
    saved_thumb_bl_upper: Option<u32>,
}

#[derive(Debug, Clone, Copy, Default)]
struct MusicPlayerShadow {
    player_ptr: u32,
    song_ptr: u32,
    playing: bool,
    fadeout_frames: u16,
}

#[derive(Debug, Clone, Copy, Default)]
struct SoundBiosShadow {
    initialized: bool,
    mode: u32,
    vsync_enabled: bool,
    vsync_ticks: u32,
    player: MusicPlayerShadow,
}

#[derive(Debug, Default)]
pub struct Arm7Tdmi {
    regs: [u32; 16],
    cpsr: u32,
    spsr_irq: u32,
    spsr_svc: u32,
    spsr_und: u32,
    r13_usr: u32,
    r14_usr: u32,
    r13_irq: u32,
    r14_irq: u32,
    r13_svc: u32,
    r14_svc: u32,
    r13_und: u32,
    r14_und: u32,
    halted: bool,
    halt_irq_mask: u16,
    clear_irq_on_halt_wake: bool,
    thumb_bl_upper: Option<u32>,
    no_bios_irq_state: Option<NoBiosIrqState>,
    last_step_pc: Option<u32>,
    trace_irq_ptr_initialized: bool,
    trace_irq_callback_prev: u32,
    trace_irq_handler_prev: u32,
    sound_bios: SoundBiosShadow,
    thumb_bl_upper_irq: Option<u32>,
    thumb_bl_upper_svc: Option<u32>,
    thumb_bl_upper_und: Option<u32>,
}

impl Arm7Tdmi {
    pub fn reset(&mut self) {
        self.reset_for_boot(false);
    }

    pub fn reset_for_boot(&mut self, has_bios: bool) {
        self.regs = [0; 16];
        self.spsr_irq = 0;
        self.spsr_svc = 0;
        self.spsr_und = 0;
        self.r13_usr = 0;
        self.r14_usr = 0;
        self.r13_irq = 0;
        self.r14_irq = 0;
        self.r13_svc = 0;
        self.r14_svc = 0;
        self.r13_und = 0;
        self.r14_und = 0;

        if has_bios {
            // ARM state, SVC mode, IRQ/FIQ disabled, start at reset vector.
            self.regs[13] = 0;
            self.regs[15] = 0x0000_0000;
            self.cpsr = MODE_SVC | FLAG_I | 0x40;
            self.r13_svc = self.regs[13];
            self.r14_svc = self.regs[14];
        } else {
            // Typical post-BIOS stack setup.
            self.r13_usr = 0x0300_7F00;
            self.r13_irq = 0x0300_7FA0;
            self.r13_svc = 0x0300_7FE0;
            self.regs[13] = self.r13_usr;
            self.regs[15] = 0x0800_0000;
            // Typical post-BIOS-like state: system mode, IRQ enabled, FIQ disabled.
            self.cpsr = MODE_SYS | 0x40;
            self.r14_usr = self.regs[14];
        }
        self.halted = false;
        self.halt_irq_mask = 0;
        self.clear_irq_on_halt_wake = false;
        self.thumb_bl_upper = None;
        self.no_bios_irq_state = None;
        self.last_step_pc = None;
        self.trace_irq_ptr_initialized = false;
        self.trace_irq_callback_prev = 0;
        self.trace_irq_handler_prev = 0;
        self.sound_bios = SoundBiosShadow::default();
        self.sound_bios.vsync_enabled = true;
        self.thumb_bl_upper_irq = None;
        self.thumb_bl_upper_svc = None;
        self.thumb_bl_upper_und = None;
    }

    pub fn step(&mut self, bus: &mut GbaBus) -> u32 {
        let trace_hooks = trace_step_hooks();
        let mut woke_from_halt = false;

        if self.halted {
            let mask = if self.halt_irq_mask == 0 {
                u16::MAX
            } else {
                self.halt_irq_mask
            };
            let pending = bus.pending_interrupts() & mask;
            if pending == 0 {
                return 1;
            }
            if !bus.has_bios() {
                self.or_no_bios_intr_check(bus, pending);
            }
            if self.clear_irq_on_halt_wake {
                bus.clear_irq(pending);
            }
            self.halted = false;
            self.halt_irq_mask = 0;
            self.clear_irq_on_halt_wake = false;
            woke_from_halt = true;
        }

        let step_start_pc = if self.thumb_mode() {
            self.regs[15] & !1
        } else {
            self.regs[15] & !3
        };
        let step_was_thumb = self.thumb_mode();

        if let Some(cycles) = self.try_handle_no_bios_direct_call(bus) {
            if trace_hooks.irq_ptr {
                self.trace_irq_pointer_change(step_start_pc, step_was_thumb, bus);
            }
            self.last_step_pc = Some(step_start_pc);
            return cycles;
        }

        if self.try_enter_irq(bus, woke_from_halt) {
            if trace_hooks.irq_ptr {
                self.trace_irq_pointer_change(step_start_pc, step_was_thumb, bus);
            }
            self.last_step_pc = Some(step_start_pc);
            return 3;
        }

        if trace_hooks.pc {
            self.trace_pc_probe(bus);
        }

        let step_start_sp = self.regs[13];
        let cycles = if step_was_thumb {
            self.execute_thumb(bus)
        } else {
            self.execute_arm(bus)
        };
        if trace_hooks.sp {
            self.trace_sp_change(step_start_pc, step_was_thumb, step_start_sp, bus);
        }
        if trace_hooks.bad_pc {
            self.trace_bad_pc(step_start_pc, step_was_thumb, bus);
        }
        if trace_hooks.irq_ptr {
            self.trace_irq_pointer_change(step_start_pc, step_was_thumb, bus);
        }
        self.last_step_pc = Some(step_start_pc);
        cycles
    }

    fn try_handle_no_bios_direct_call(&mut self, bus: &mut GbaBus) -> Option<u32> {
        if bus.has_bios() {
            return None;
        }

        let mode = self.cpsr & MODE_MASK;
        if mode != MODE_SYS && mode != MODE_USR {
            return None;
        }

        let pc = self.regs[15] & !1;
        if !matches!(pc, 0x0000_0000..=0x0000_3FFF) {
            return None;
        }

        if let Some(prev_pc) = self.last_step_pc {
            let prev_aligned = prev_pc & !1;
            if matches!(prev_aligned, 0x0000_0000..=0x0000_3FFF) {
                // Ignore sequential execution that fell into BIOS space.
                // Only treat transitions from non-BIOS regions as synthetic BIOS calls.
                return None;
            }
        }

        let Some(swi_number) = direct_bios_call_to_swi_number(pc) else {
            return None;
        };

        let lr = self.regs[14];
        if trace_branch_enabled() {
            if let Some(slot) = take_trace_slot(&TRACE_BRANCH_COUNT) {
                eprintln!(
                    "[gba:trace:branch] slot={}/{} kind=no-bios-bioscall pc={:#010X} lr={:#010X} swi={:#04X}",
                    slot + 1,
                    trace_limit(),
                    pc,
                    lr,
                    swi_number,
                );
            }
        }

        self.handle_swi(swi_number, bus);
        if matches!(swi_number, 0x00 | 0x26) {
            // SoftReset/HardReset do not return to caller.
            Some(4)
        } else {
            self.branch_exchange(lr);
            Some(3)
        }
    }

    pub fn serialize_state(&self, w: &mut crate::state::StateWriter) {
        for &r in &self.regs {
            w.write_u32(r);
        }
        w.write_u32(self.cpsr);
        w.write_u32(self.spsr_irq);
        w.write_u32(self.spsr_svc);
        w.write_u32(self.spsr_und);
        w.write_u32(self.r13_usr);
        w.write_u32(self.r14_usr);
        w.write_u32(self.r13_irq);
        w.write_u32(self.r14_irq);
        w.write_u32(self.r13_svc);
        w.write_u32(self.r14_svc);
        w.write_u32(self.r13_und);
        w.write_u32(self.r14_und);
        w.write_bool(self.halted);
        w.write_u16(self.halt_irq_mask);
        w.write_bool(self.clear_irq_on_halt_wake);
        // thumb_bl_upper variants
        write_option_u32(w, self.thumb_bl_upper);
        write_option_u32(w, self.thumb_bl_upper_irq);
        write_option_u32(w, self.thumb_bl_upper_svc);
        write_option_u32(w, self.thumb_bl_upper_und);
        // no_bios_irq_state
        w.write_bool(self.no_bios_irq_state.is_some());
        if let Some(ref st) = self.no_bios_irq_state {
            w.write_u32(st.saved_cpsr);
            w.write_u32(st.resume_pc);
            for &r in &st.saved_regs {
                w.write_u32(r);
            }
            w.write_u32(st.saved_irq_sp);
            write_option_u32(w, st.saved_thumb_bl_upper);
        }
        // sound_bios
        w.write_bool(self.sound_bios.initialized);
        w.write_u32(self.sound_bios.mode);
        w.write_bool(self.sound_bios.vsync_enabled);
        w.write_u32(self.sound_bios.vsync_ticks);
        w.write_u32(self.sound_bios.player.player_ptr);
        w.write_u32(self.sound_bios.player.song_ptr);
        w.write_bool(self.sound_bios.player.playing);
        w.write_u16(self.sound_bios.player.fadeout_frames);
    }

    pub fn deserialize_state(
        &mut self,
        r: &mut crate::state::StateReader,
    ) -> Result<(), &'static str> {
        for reg in &mut self.regs {
            *reg = r.read_u32()?;
        }
        self.cpsr = r.read_u32()?;
        self.spsr_irq = r.read_u32()?;
        self.spsr_svc = r.read_u32()?;
        self.spsr_und = r.read_u32()?;
        self.r13_usr = r.read_u32()?;
        self.r14_usr = r.read_u32()?;
        self.r13_irq = r.read_u32()?;
        self.r14_irq = r.read_u32()?;
        self.r13_svc = r.read_u32()?;
        self.r14_svc = r.read_u32()?;
        self.r13_und = r.read_u32()?;
        self.r14_und = r.read_u32()?;
        self.halted = r.read_bool()?;
        self.halt_irq_mask = r.read_u16()?;
        self.clear_irq_on_halt_wake = r.read_bool()?;
        self.thumb_bl_upper = read_option_u32(r)?;
        self.thumb_bl_upper_irq = read_option_u32(r)?;
        self.thumb_bl_upper_svc = read_option_u32(r)?;
        self.thumb_bl_upper_und = read_option_u32(r)?;
        // no_bios_irq_state
        if r.read_bool()? {
            let saved_cpsr = r.read_u32()?;
            let resume_pc = r.read_u32()?;
            let mut saved_regs = [0u32; 15];
            for reg in &mut saved_regs {
                *reg = r.read_u32()?;
            }
            let saved_irq_sp = r.read_u32()?;
            let saved_thumb_bl_upper = read_option_u32(r)?;
            self.no_bios_irq_state = Some(NoBiosIrqState {
                saved_cpsr,
                resume_pc,
                saved_regs,
                saved_irq_sp,
                saved_thumb_bl_upper,
            });
        } else {
            self.no_bios_irq_state = None;
        }
        // sound_bios
        self.sound_bios.initialized = r.read_bool()?;
        self.sound_bios.mode = r.read_u32()?;
        self.sound_bios.vsync_enabled = r.read_bool()?;
        self.sound_bios.vsync_ticks = r.read_u32()?;
        self.sound_bios.player.player_ptr = r.read_u32()?;
        self.sound_bios.player.song_ptr = r.read_u32()?;
        self.sound_bios.player.playing = r.read_bool()?;
        self.sound_bios.player.fadeout_frames = r.read_u16()?;
        // Reset debug-only fields.
        self.last_step_pc = None;
        self.trace_irq_ptr_initialized = false;
        self.trace_irq_callback_prev = 0;
        self.trace_irq_handler_prev = 0;
        Ok(())
    }

    pub fn program_counter(&self) -> u32 {
        self.regs[15]
    }

    pub fn cpsr(&self) -> u32 {
        self.cpsr
    }

    pub fn reg(&self, index: usize) -> u32 {
        self.regs[index & 0x0F]
    }

    fn try_enter_irq(&mut self, bus: &mut GbaBus, allow_vector_fallback: bool) -> bool {
        if !bus.interrupts_master_enabled() || self.irq_masked() {
            return false;
        }

        let pending = bus.pending_interrupts();
        if pending == 0 {
            return false;
        }

        if bus.has_bios() {
            self.trace_irq_entry(pending, 0x0000_0018, true);
            let return_addr = self.regs[15].wrapping_add(4);
            self.enter_exception(MODE_IRQ, return_addr, 0x0000_0018);
            return true;
        }

        self.or_no_bios_intr_check(bus, pending);
        let handler = bus.read32(NO_BIOS_IRQ_HANDLER_ADDR);
        if handler == 0 {
            if !allow_vector_fallback {
                // In no-BIOS mode, jumping to 0x00000018 has no valid BIOS IRQ vector
                // and can run into unmapped memory before games install their handler.
                // Keep executing until the game sets 0x0300_7FFC.
                return false;
            }
            self.trace_irq_entry(pending, 0x0000_0018, false);
            let return_addr = self.regs[15].wrapping_add(4);
            self.enter_exception(MODE_IRQ, return_addr, 0x0000_0018);
            return true;
        }

        let mut saved_regs = [0u32; 15];
        saved_regs.copy_from_slice(&self.regs[..15]);
        self.no_bios_irq_state = Some(NoBiosIrqState {
            saved_cpsr: self.cpsr,
            resume_pc: self.regs[15],
            saved_regs,
            saved_irq_sp: self.r13_irq,
            saved_thumb_bl_upper: self.thumb_bl_upper,
        });
        // BIOS IRQ trampoline enters user handler with IO base in r0.
        self.regs[0] = 0x0400_0000;
        self.trace_irq_entry(pending, handler, false);
        let return_addr = self.regs[15].wrapping_add(4);
        self.enter_exception(MODE_IRQ, return_addr, handler);
        self.seed_no_bios_irq_register_frame(bus);
        // Keep callback execution in IRQ mode to match BIOS IRQ trampoline
        // behavior (r0-r3,r12,lr are stacked on IRQ SP before dispatch).
        self.regs[14] = NO_BIOS_IRQ_RETURN_TOKEN;
        true
    }

    fn seed_no_bios_irq_register_frame(&mut self, bus: &mut GbaBus) {
        if (self.cpsr & MODE_MASK) != MODE_IRQ {
            return;
        }
        let Some(state) = self.no_bios_irq_state else {
            return;
        };

        // BIOS vector @0x18 does:
        //   stmfd sp!,{r0-r3,r12,lr}
        //   mov   r0,#0x04000000
        //   ...
        // Mirror that stack frame so IRQ callbacks that inspect IRQ SP state
        // can run without BIOS.
        let irq_sp = self.regs[13];
        let frame_sp = irq_sp.wrapping_sub(24);
        let stacked = [
            state.saved_regs[0],
            state.saved_regs[1],
            state.saved_regs[2],
            state.saved_regs[3],
            state.saved_regs[12],
            self.regs[14], // IRQ LR set by enter_exception above.
        ];
        for (index, value) in stacked.into_iter().enumerate() {
            bus.write32(frame_sp.wrapping_add((index as u32) * 4), value);
        }
        self.regs[13] = frame_sp;
        self.r13_irq = frame_sp;
    }

    fn no_bios_intr_check(&self, bus: &GbaBus) -> u16 {
        bus.read32(NO_BIOS_INTR_CHECK_ADDR) as u16
    }

    fn write_no_bios_intr_check(&self, bus: &mut GbaBus, value: u16) {
        bus.write32(NO_BIOS_INTR_CHECK_ADDR, u32::from(value));
    }

    fn or_no_bios_intr_check(&self, bus: &mut GbaBus, bits: u16) {
        if bits == 0 {
            return;
        }
        let next = self.no_bios_intr_check(bus) | bits;
        self.write_no_bios_intr_check(bus, next);
    }

    fn execute_arm(&mut self, bus: &mut GbaBus) -> u32 {
        let (pc, instr) = self.fetch_arm(bus);
        self.trace_irq_instruction(pc, instr, false, bus);
        let cond = (instr >> 28) & 0x0F;
        if !self.condition_passed(cond as u8) {
            return 1;
        }

        if (instr & 0x0FFF_FFF0) == 0x012F_FF10 {
            let rm = (instr & 0x0F) as usize;
            let target = self.read_arm_reg(rm, pc);
            self.branch_exchange(target);
            return 3;
        }

        if (instr & 0x0FFF_FFF0) == 0x012F_FF30 {
            let rm = (instr & 0x0F) as usize;
            let target = self.read_arm_reg(rm, pc);
            self.regs[14] = pc.wrapping_add(4);
            self.branch_exchange(target);
            return 3;
        }

        if (instr & 0x0F00_0000) == 0x0F00_0000 {
            let swi_number = instr & 0xFF;
            if bus.has_bios() {
                self.enter_exception(MODE_SVC, pc.wrapping_add(4), 0x0000_0008);
            } else {
                self.handle_swi(swi_number, bus);
            }
            return 4;
        }

        if (instr & 0x0E00_0000) == 0x0A00_0000 {
            return self.execute_arm_branch(instr, pc);
        }

        if (instr & 0x0E00_0000) == 0x0800_0000 {
            return self.execute_arm_block_data_transfer(instr, pc, bus);
        }

        if (instr & 0x0C00_0000) == 0x0400_0000 {
            return self.execute_arm_single_data_transfer(instr, pc, bus);
        }

        if (instr & 0x0FC0_00F0) == 0x0000_0090 {
            return self.execute_arm_multiply(instr, pc);
        }

        if (instr & 0x0F80_00F0) == 0x0080_0090 {
            return self.execute_arm_multiply_long(instr, pc);
        }

        if (instr & 0x0FB0_0FF0) == 0x0100_0090 {
            return self.execute_arm_swap(instr, pc, bus);
        }

        if (instr & 0x0E00_0090) == 0x0000_0090 && (instr & 0x0000_0060) != 0 {
            return self.execute_arm_halfword_transfer(instr, pc, bus);
        }

        if (instr & 0x0FBF_0FFF) == 0x010F_0000 {
            let rd = ((instr >> 12) & 0x0F) as usize;
            let use_spsr = (instr & (1 << 22)) != 0;
            let value = if use_spsr {
                self.current_spsr()
            } else {
                self.cpsr
            };
            self.write_reg(rd, value);
            return 1;
        }

        if (instr & 0x0FB0_FFF0) == 0x0120_F000 {
            let rm = (instr & 0x0F) as usize;
            let fields = ((instr >> 16) & 0x0F) as u8;
            let operand = self.read_arm_reg(rm, pc);
            let use_spsr = (instr & (1 << 22)) != 0;
            self.write_psr_fields(fields, operand, use_spsr);
            return 1;
        }

        if (instr & 0x0FB0_F000) == 0x0320_F000 {
            let fields = ((instr >> 16) & 0x0F) as u8;
            let imm = (instr & 0xFF) as u32;
            let rotate = ((instr >> 8) & 0x0F) * 2;
            let operand = imm.rotate_right(rotate);
            let use_spsr = (instr & (1 << 22)) != 0;
            self.write_psr_fields(fields, operand, use_spsr);
            return 1;
        }

        if (instr & 0x0C00_0000) == 0x0000_0000 {
            return self.execute_arm_data_processing(instr, pc, bus);
        }

        self.trace_unknown_arm(instr, pc, bus.has_bios());
        if !bus.has_bios() && (self.cpsr & MODE_MASK) == MODE_IRQ {
            // No-BIOS mode can observe THUMB IRQ handlers directly; retry decode in THUMB.
            self.set_thumb_mode(true);
            self.regs[15] = pc & !1;
            self.thumb_bl_upper = None;
            return 1;
        }
        if self.enter_undefined_exception(bus, pc.wrapping_add(4)) {
            return 4;
        }

        1
    }

    fn execute_arm_branch(&mut self, instr: u32, pc: u32) -> u32 {
        let offset = sign_extend((instr & 0x00FF_FFFF) << 2, 26) as u32;
        if (instr & (1 << 24)) != 0 {
            self.regs[14] = pc.wrapping_add(4);
        }
        self.regs[15] = pc.wrapping_add(8).wrapping_add(offset) & !3;
        self.thumb_bl_upper = None;
        3
    }

    fn execute_arm_multiply(&mut self, instr: u32, pc: u32) -> u32 {
        let accumulate = (instr & (1 << 21)) != 0;
        let set_flags = (instr & (1 << 20)) != 0;
        let rd = ((instr >> 16) & 0x0F) as usize;
        let rn = ((instr >> 12) & 0x0F) as usize;
        let rs = ((instr >> 8) & 0x0F) as usize;
        let rm = (instr & 0x0F) as usize;

        let mut result = self
            .read_arm_reg(rm, pc)
            .wrapping_mul(self.read_arm_reg(rs, pc));
        if accumulate {
            result = result.wrapping_add(self.read_arm_reg(rn, pc));
        }

        self.write_reg(rd, result);
        if set_flags {
            self.set_nz(result);
        }

        2
    }

    fn execute_arm_multiply_long(&mut self, instr: u32, pc: u32) -> u32 {
        let signed = (instr & (1 << 22)) != 0;
        let accumulate = (instr & (1 << 21)) != 0;
        let set_flags = (instr & (1 << 20)) != 0;
        let rd_hi = ((instr >> 16) & 0x0F) as usize;
        let rd_lo = ((instr >> 12) & 0x0F) as usize;
        let rs = ((instr >> 8) & 0x0F) as usize;
        let rm = (instr & 0x0F) as usize;

        let result = if signed {
            let lhs = self.read_arm_reg(rm, pc) as i32 as i64;
            let rhs = self.read_arm_reg(rs, pc) as i32 as i64;
            let mut value = lhs.wrapping_mul(rhs);
            if accumulate {
                let acc = ((self.read_arm_reg(rd_hi, pc) as u64) << 32)
                    | self.read_arm_reg(rd_lo, pc) as u64;
                value = value.wrapping_add(acc as i64);
            }
            value as u64
        } else {
            let lhs = self.read_arm_reg(rm, pc) as u64;
            let rhs = self.read_arm_reg(rs, pc) as u64;
            let mut value = lhs.wrapping_mul(rhs);
            if accumulate {
                let acc = ((self.read_arm_reg(rd_hi, pc) as u64) << 32)
                    | self.read_arm_reg(rd_lo, pc) as u64;
                value = value.wrapping_add(acc);
            }
            value
        };

        self.write_reg(rd_lo, result as u32);
        self.write_reg(rd_hi, (result >> 32) as u32);

        if set_flags {
            self.set_flag(FLAG_N, (result & 0x8000_0000_0000_0000) != 0);
            self.set_flag(FLAG_Z, result == 0);
        }

        3
    }

    fn execute_arm_swap(&mut self, instr: u32, pc: u32, bus: &mut GbaBus) -> u32 {
        let byte = (instr & (1 << 22)) != 0;
        let rn = ((instr >> 16) & 0x0F) as usize;
        let rd = ((instr >> 12) & 0x0F) as usize;
        let rm = (instr & 0x0F) as usize;

        let addr = self.read_arm_reg(rn, pc);
        let source = self.read_arm_reg(rm, pc);
        let old = if byte {
            let v = bus.read8(addr) as u32;
            bus.write8(addr, source as u8);
            v
        } else {
            let v = read_word_rotate(bus, addr);
            bus.write32(addr, source);
            v
        };

        self.write_reg(rd, old);
        3
    }

    fn execute_arm_halfword_transfer(&mut self, instr: u32, pc: u32, bus: &mut GbaBus) -> u32 {
        let p = (instr & (1 << 24)) != 0;
        let u = (instr & (1 << 23)) != 0;
        let i = (instr & (1 << 22)) != 0;
        let w = (instr & (1 << 21)) != 0;
        let l = (instr & (1 << 20)) != 0;
        let rn = ((instr >> 16) & 0x0F) as usize;
        let rd = ((instr >> 12) & 0x0F) as usize;
        let op = ((instr >> 5) & 0x03) as u8;

        let offset = if i {
            (((instr >> 8) & 0x0F) << 4) | (instr & 0x0F)
        } else {
            let rm = (instr & 0x0F) as usize;
            self.read_arm_reg(rm, pc)
        };

        let base = self.read_arm_reg(rn, pc);
        let index_addr = if u {
            base.wrapping_add(offset)
        } else {
            base.wrapping_sub(offset)
        };
        let addr = if p { index_addr } else { base };
        let writeback_value = if p {
            index_addr
        } else if u {
            base.wrapping_add(offset)
        } else {
            base.wrapping_sub(offset)
        };

        if l {
            let value = match op {
                0x01 => read_halfword_rotate(bus, addr),
                0x02 => bus.read8(addr) as i8 as i32 as u32,
                0x03 => read_signed_halfword(bus, addr),
                _ => 0,
            };
            self.write_reg(rd, value);
        } else if op == 0x01 {
            write_halfword_aligned(bus, addr, self.store_value(rd, pc) as u16);
        }

        if (!p) || w {
            self.write_reg(rn, writeback_value);
        }

        3
    }

    fn execute_arm_data_processing(&mut self, instr: u32, pc: u32, bus: &GbaBus) -> u32 {
        let opcode = ((instr >> 21) & 0x0F) as u8;
        let set_flags = (instr & (1 << 20)) != 0;
        let rn = ((instr >> 16) & 0x0F) as usize;
        let rd = ((instr >> 12) & 0x0F) as usize;
        let op1 = self.read_arm_reg(rn, pc);
        let (op2, shifter_carry) = self.decode_arm_operand2(instr, pc, bus);

        let (result, write_result, carry, overflow) = match opcode {
            0x0 => (op1 & op2, true, shifter_carry, None),
            0x1 => (op1 ^ op2, true, shifter_carry, None),
            0x2 => {
                let (res, c, v) = add_with_carry(op1, !op2, true);
                (res, true, Some(c), Some(v))
            }
            0x3 => {
                let (res, c, v) = add_with_carry(op2, !op1, true);
                (res, true, Some(c), Some(v))
            }
            0x4 => {
                let (res, c, v) = add_with_carry(op1, op2, false);
                (res, true, Some(c), Some(v))
            }
            0x5 => {
                let (res, c, v) = add_with_carry(op1, op2, self.flag_c());
                (res, true, Some(c), Some(v))
            }
            0x6 => {
                let (res, c, v) = add_with_carry(op1, !op2, self.flag_c());
                (res, true, Some(c), Some(v))
            }
            0x7 => {
                let (res, c, v) = add_with_carry(op2, !op1, self.flag_c());
                (res, true, Some(c), Some(v))
            }
            0x8 => (op1 & op2, false, shifter_carry, None),
            0x9 => (op1 ^ op2, false, shifter_carry, None),
            0xA => {
                let (res, c, v) = add_with_carry(op1, !op2, true);
                (res, false, Some(c), Some(v))
            }
            0xB => {
                let (res, c, v) = add_with_carry(op1, op2, false);
                (res, false, Some(c), Some(v))
            }
            0xC => (op1 | op2, true, shifter_carry, None),
            0xD => (op2, true, shifter_carry, None),
            0xE => (op1 & !op2, true, shifter_carry, None),
            0xF => (!op2, true, shifter_carry, None),
            _ => (0, false, None, None),
        };

        let writes_pc_with_s = write_result && rd == 15 && set_flags;

        if write_result {
            if writes_pc_with_s {
                // Exception-return style writes (for example, MOVS/SUBS pc,lr,#imm)
                // must restore CPSR first-class, including Thumb state, without
                // prematurely forcing ARM word alignment.
                if !self.try_no_bios_irq_exception_return(result) {
                    self.regs[15] = result;
                    self.restore_cpsr_from_spsr();
                }
            } else {
                self.write_reg(rd, result);
            }
        }

        if !writes_pc_with_s && (set_flags || matches!(opcode, 0x8..=0xB)) {
            self.set_nz(result);
            if let Some(c) = carry {
                self.set_flag(FLAG_C, c);
            }
            if let Some(v) = overflow {
                self.set_flag(FLAG_V, v);
            }
        }

        1
    }

    fn execute_arm_single_data_transfer(&mut self, instr: u32, pc: u32, bus: &mut GbaBus) -> u32 {
        let i = (instr & (1 << 25)) != 0;
        let p = (instr & (1 << 24)) != 0;
        let u = (instr & (1 << 23)) != 0;
        let b = (instr & (1 << 22)) != 0;
        let w = (instr & (1 << 21)) != 0;
        let l = (instr & (1 << 20)) != 0;

        let rn = ((instr >> 16) & 0x0F) as usize;
        let rd = ((instr >> 12) & 0x0F) as usize;

        let offset = if i {
            let rm = (instr & 0x0F) as usize;
            let shift_type = ((instr >> 5) & 0x03) as u8;
            let shift_imm = ((instr >> 7) & 0x1F) as u8;
            let rm_val = self.read_arm_reg(rm, pc);
            shift_imm_only(rm_val, shift_type, shift_imm, self.flag_c()).0
        } else {
            instr & 0x0FFF
        };

        let base = self.read_arm_reg(rn, pc);
        let index_addr = if u {
            base.wrapping_add(offset)
        } else {
            base.wrapping_sub(offset)
        };

        let addr = if p { index_addr } else { base };
        let writeback_value = if p {
            index_addr
        } else if u {
            base.wrapping_add(offset)
        } else {
            base.wrapping_sub(offset)
        };

        if l {
            let value = if b {
                bus.read8(addr) as u32
            } else {
                read_word_rotate(bus, addr)
            };
            self.write_reg(rd, value);
        } else if b {
            let value = self.store_value(rd, pc) as u8;
            bus.write8(addr, value);
        } else {
            let value = self.store_value(rd, pc);
            bus.write32(addr, value);
        }

        if (!p) || w {
            self.write_reg(rn, writeback_value);
        }

        3
    }

    fn execute_arm_block_data_transfer(&mut self, instr: u32, pc: u32, bus: &mut GbaBus) -> u32 {
        let p = (instr & (1 << 24)) != 0;
        let u = (instr & (1 << 23)) != 0;
        let s = (instr & (1 << 22)) != 0;
        let w = (instr & (1 << 21)) != 0;
        let l = (instr & (1 << 20)) != 0;
        let rn = ((instr >> 16) & 0x0F) as usize;
        let list = (instr & 0xFFFF) as u16;

        let reg_count = list.count_ones() as u32;
        if reg_count == 0 {
            return 1;
        }

        let base = self.read_arm_reg(rn, pc);
        let transfer_bytes = reg_count * 4;
        let mut addr = if u {
            if p { base.wrapping_add(4) } else { base }
        } else if p {
            base.wrapping_sub(transfer_bytes)
        } else {
            base.wrapping_sub(transfer_bytes).wrapping_add(4)
        };
        let transfer_user_bank = s && !(l && (list & (1 << 15)) != 0);

        for reg in 0..16usize {
            if (list & (1u16 << reg)) == 0 {
                continue;
            }
            if l {
                let value = bus.read32(addr);
                if transfer_user_bank {
                    self.write_user_reg(reg, value);
                } else if s && reg == 15 {
                    // LDM ...^ with PC restores CPSR from SPSR after load.
                    // Keep full loaded address bits until that restore occurs.
                    self.regs[15] = value;
                } else {
                    self.write_reg(reg, value);
                }
            } else {
                let value = if transfer_user_bank {
                    self.read_user_reg(reg, pc)
                } else {
                    self.store_value(reg, pc)
                };
                bus.write32(addr, value);
            }
            addr = addr.wrapping_add(4);
        }

        if l && s && (list & (1 << 15)) != 0 {
            if !self.try_no_bios_irq_exception_return(self.regs[15]) {
                self.restore_cpsr_from_spsr();
            }
        }

        if w {
            let new_base = if u {
                base.wrapping_add(transfer_bytes)
            } else {
                base.wrapping_sub(transfer_bytes)
            };
            self.write_reg(rn, new_base);
        }

        reg_count.saturating_add(1)
    }

    fn execute_thumb(&mut self, bus: &mut GbaBus) -> u32 {
        let (pc, instr) = self.fetch_thumb(bus);
        self.trace_irq_instruction(pc, u32::from(instr), true, bus);

        if (instr & 0xF800) == 0xF000 {
            let hi = sign_extend((instr & 0x07FF) as u32, 11) << 12;
            self.thumb_bl_upper = Some(pc.wrapping_add(4).wrapping_add(hi as u32));
            return 2;
        }

        if (instr & 0xF800) == 0xF800 {
            let base = self.thumb_bl_upper.take().unwrap_or(pc.wrapping_add(4));
            let low = ((instr & 0x07FF) as u32) << 1;
            self.regs[14] = pc.wrapping_add(2) | 1;
            self.regs[15] = base.wrapping_add(low) & !1;
            self.set_thumb_mode(true);
            return 3;
        }

        self.thumb_bl_upper = None;

        if (instr & 0xE000) == 0x0000 {
            if (instr & 0x1800) != 0x1800 {
                return self.thumb_move_shifted_register(instr, pc);
            }
            return self.thumb_add_sub(instr, pc);
        }

        if (instr & 0xE000) == 0x2000 {
            return self.thumb_immediate_ops(instr);
        }

        if (instr & 0xFC00) == 0x4000 {
            return self.thumb_alu(instr, pc);
        }

        if (instr & 0xFC00) == 0x4400 {
            return self.thumb_hi_reg_ops(instr, pc);
        }

        if (instr & 0xF800) == 0x4800 {
            let rd = ((instr >> 8) & 0x07) as usize;
            let imm = ((instr & 0xFF) as u32) << 2;
            let addr = (pc.wrapping_add(4) & !3).wrapping_add(imm);
            self.write_reg(rd, read_word_rotate(bus, addr));
            return 3;
        }

        if (instr & 0xF000) == 0x5000 {
            return self.thumb_load_store_reg_offset(instr, pc, bus);
        }

        if (instr & 0xE000) == 0x6000 {
            return self.thumb_load_store_imm(instr, pc, bus);
        }

        if (instr & 0xF000) == 0x8000 {
            return self.thumb_load_store_halfword_imm(instr, pc, bus);
        }

        if (instr & 0xF000) == 0x9000 {
            let load = (instr & (1 << 11)) != 0;
            let rd = ((instr >> 8) & 0x07) as usize;
            let imm = ((instr & 0xFF) as u32) << 2;
            let addr = self.regs[13].wrapping_add(imm);
            if load {
                self.write_reg(rd, read_word_rotate(bus, addr));
            } else {
                bus.write32(addr, self.store_value(rd, pc));
            }
            return 2;
        }

        if (instr & 0xF000) == 0xA000 {
            let rd = ((instr >> 8) & 0x07) as usize;
            let imm = ((instr & 0xFF) as u32) << 2;
            if (instr & (1 << 11)) == 0 {
                self.write_reg(rd, (pc.wrapping_add(4) & !3).wrapping_add(imm));
            } else {
                self.write_reg(rd, self.regs[13].wrapping_add(imm));
            }
            return 1;
        }

        if (instr & 0xFE00) == 0xB400 || (instr & 0xFE00) == 0xBC00 {
            return self.thumb_push_pop(instr, pc, bus);
        }

        if (instr & 0xFF00) == 0xB000 {
            let subtract = (instr & (1 << 7)) != 0;
            let imm = ((instr & 0x7F) as u32) << 2;
            if subtract {
                self.regs[13] = self.regs[13].wrapping_sub(imm);
            } else {
                self.regs[13] = self.regs[13].wrapping_add(imm);
            }
            return 1;
        }

        if (instr & 0xF000) == 0xC000 {
            return self.thumb_multiple_load_store(instr, pc, bus);
        }

        if (instr & 0xF000) == 0xD000 {
            let cond = ((instr >> 8) & 0x0F) as u8;
            if cond == 0x0F {
                if bus.has_bios() {
                    self.enter_exception(MODE_SVC, pc.wrapping_add(2), 0x0000_0008);
                } else {
                    self.handle_swi((instr & 0xFF) as u32, bus);
                }
                return 4;
            }
            if cond == 0x0E {
                self.trace_unknown_thumb(instr, pc, bus.has_bios());
                if self.enter_undefined_exception(bus, pc.wrapping_add(2)) {
                    return 4;
                }
                return 1;
            }
            if self.thumb_condition_passed(cond) {
                let offset = sign_extend(((instr & 0xFF) as u32) << 1, 9) as u32;
                self.regs[15] = pc.wrapping_add(4).wrapping_add(offset);
            }
            return 2;
        }

        if (instr & 0xF800) == 0xE000 {
            let offset = sign_extend(((instr & 0x07FF) as u32) << 1, 12) as u32;
            self.regs[15] = pc.wrapping_add(4).wrapping_add(offset);
            return 2;
        }

        self.trace_unknown_thumb(instr, pc, bus.has_bios());
        if self.enter_undefined_exception(bus, pc.wrapping_add(2)) {
            return 4;
        }

        1
    }

    fn thumb_move_shifted_register(&mut self, instr: u16, _pc: u32) -> u32 {
        let op = ((instr >> 11) & 0x03) as u8;
        let offset = ((instr >> 6) & 0x1F) as u8;
        let rs = ((instr >> 3) & 0x07) as usize;
        let rd = (instr & 0x07) as usize;

        let (result, carry) = shift_imm_only(self.regs[rs], op, offset, self.flag_c());
        self.write_reg(rd, result);
        self.set_nz(result);
        self.set_flag(FLAG_C, carry);
        1
    }

    fn thumb_add_sub(&mut self, instr: u16, _pc: u32) -> u32 {
        let immediate = (instr & (1 << 10)) != 0;
        let subtract = (instr & (1 << 9)) != 0;
        let rn = ((instr >> 3) & 0x07) as usize;
        let rd = (instr & 0x07) as usize;
        let operand = if immediate {
            ((instr >> 6) & 0x07) as u32
        } else {
            self.regs[((instr >> 6) & 0x07) as usize]
        };

        let (result, carry, overflow) = if subtract {
            add_with_carry(self.regs[rn], !operand, true)
        } else {
            add_with_carry(self.regs[rn], operand, false)
        };
        self.write_reg(rd, result);
        self.set_nz(result);
        self.set_flag(FLAG_C, carry);
        self.set_flag(FLAG_V, overflow);
        1
    }

    fn thumb_immediate_ops(&mut self, instr: u16) -> u32 {
        let op = ((instr >> 11) & 0x03) as u8;
        let rd = ((instr >> 8) & 0x07) as usize;
        let imm = (instr & 0xFF) as u32;

        match op {
            0 => {
                self.write_reg(rd, imm);
                self.set_nz(imm);
            }
            1 => {
                let (res, c, v) = add_with_carry(self.regs[rd], !imm, true);
                self.set_nz(res);
                self.set_flag(FLAG_C, c);
                self.set_flag(FLAG_V, v);
            }
            2 => {
                let (res, c, v) = add_with_carry(self.regs[rd], imm, false);
                self.write_reg(rd, res);
                self.set_nz(res);
                self.set_flag(FLAG_C, c);
                self.set_flag(FLAG_V, v);
            }
            _ => {
                let (res, c, v) = add_with_carry(self.regs[rd], !imm, true);
                self.write_reg(rd, res);
                self.set_nz(res);
                self.set_flag(FLAG_C, c);
                self.set_flag(FLAG_V, v);
            }
        }

        1
    }

    fn thumb_alu(&mut self, instr: u16, _pc: u32) -> u32 {
        let op = ((instr >> 6) & 0x0F) as u8;
        let rs = ((instr >> 3) & 0x07) as usize;
        let rd = (instr & 0x07) as usize;
        let lhs = self.regs[rd];
        let rhs = self.regs[rs];

        match op {
            0x0 => {
                let r = lhs & rhs;
                self.write_reg(rd, r);
                self.set_nz(r);
            }
            0x1 => {
                let r = lhs ^ rhs;
                self.write_reg(rd, r);
                self.set_nz(r);
            }
            0x2 => {
                let (r, c) = shift_reg(lhs, 0, rhs as u8, self.flag_c());
                self.write_reg(rd, r);
                self.set_nz(r);
                self.set_flag(FLAG_C, c);
            }
            0x3 => {
                let (r, c) = shift_reg(lhs, 1, rhs as u8, self.flag_c());
                self.write_reg(rd, r);
                self.set_nz(r);
                self.set_flag(FLAG_C, c);
            }
            0x4 => {
                let (r, c) = shift_reg(lhs, 2, rhs as u8, self.flag_c());
                self.write_reg(rd, r);
                self.set_nz(r);
                self.set_flag(FLAG_C, c);
            }
            0x5 => {
                let (r, c, v) = add_with_carry(lhs, rhs, self.flag_c());
                self.write_reg(rd, r);
                self.set_nz(r);
                self.set_flag(FLAG_C, c);
                self.set_flag(FLAG_V, v);
            }
            0x6 => {
                let (r, c, v) = add_with_carry(lhs, !rhs, self.flag_c());
                self.write_reg(rd, r);
                self.set_nz(r);
                self.set_flag(FLAG_C, c);
                self.set_flag(FLAG_V, v);
            }
            0x7 => {
                let (r, c) = shift_reg(lhs, 3, rhs as u8, self.flag_c());
                self.write_reg(rd, r);
                self.set_nz(r);
                self.set_flag(FLAG_C, c);
            }
            0x8 => {
                self.set_nz(lhs & rhs);
            }
            0x9 => {
                let (r, c, v) = add_with_carry(0, !rhs, true);
                self.write_reg(rd, r);
                self.set_nz(r);
                self.set_flag(FLAG_C, c);
                self.set_flag(FLAG_V, v);
            }
            0xA => {
                let (r, c, v) = add_with_carry(lhs, !rhs, true);
                self.set_nz(r);
                self.set_flag(FLAG_C, c);
                self.set_flag(FLAG_V, v);
            }
            0xB => {
                let (r, c, v) = add_with_carry(lhs, rhs, false);
                self.set_nz(r);
                self.set_flag(FLAG_C, c);
                self.set_flag(FLAG_V, v);
            }
            0xC => {
                let r = lhs | rhs;
                self.write_reg(rd, r);
                self.set_nz(r);
            }
            0xD => {
                let r = lhs.wrapping_mul(rhs);
                self.write_reg(rd, r);
                self.set_nz(r);
            }
            0xE => {
                let r = lhs & !rhs;
                self.write_reg(rd, r);
                self.set_nz(r);
            }
            _ => {
                let r = !rhs;
                self.write_reg(rd, r);
                self.set_nz(r);
            }
        }

        1
    }

    fn thumb_hi_reg_ops(&mut self, instr: u16, pc: u32) -> u32 {
        let op = ((instr >> 8) & 0x03) as u8;
        let h1 = ((instr >> 7) & 0x01) as usize;
        let h2 = ((instr >> 6) & 0x01) as usize;
        let rs = (h2 << 3) | ((instr as usize >> 3) & 0x07);
        let rd = (h1 << 3) | ((instr as usize) & 0x07);

        let source = if rs == 15 {
            pc.wrapping_add(4)
        } else {
            self.regs[rs]
        };

        match op {
            0 => {
                let result = self.regs[rd].wrapping_add(source);
                self.write_reg(rd, result);
            }
            1 => {
                let (r, c, v) = add_with_carry(self.regs[rd], !source, true);
                self.set_nz(r);
                self.set_flag(FLAG_C, c);
                self.set_flag(FLAG_V, v);
            }
            2 => {
                self.write_reg(rd, source);
            }
            _ => {
                self.branch_exchange(source);
            }
        }

        1
    }

    fn thumb_load_store_reg_offset(&mut self, instr: u16, _pc: u32, bus: &mut GbaBus) -> u32 {
        let ro = ((instr >> 6) & 0x07) as usize;
        let rb = ((instr >> 3) & 0x07) as usize;
        let rd = (instr & 0x07) as usize;
        let addr = self.regs[rb].wrapping_add(self.regs[ro]);

        if (instr & (1 << 9)) == 0 {
            let load = (instr & (1 << 11)) != 0;
            let byte = (instr & (1 << 10)) != 0;
            match (load, byte) {
                (false, false) => bus.write32(addr, self.regs[rd]),
                (false, true) => bus.write8(addr, self.regs[rd] as u8),
                (true, false) => self.write_reg(rd, read_word_rotate(bus, addr)),
                (true, true) => self.write_reg(rd, bus.read8(addr) as u32),
            }
        } else {
            let signed = (instr & (1 << 10)) != 0;
            let halfword = (instr & (1 << 11)) != 0;
            match (signed, halfword) {
                (false, false) => write_halfword_aligned(bus, addr, self.regs[rd] as u16),
                (true, false) => {
                    let value = bus.read8(addr) as i8 as i32 as u32;
                    self.write_reg(rd, value);
                }
                (false, true) => self.write_reg(rd, read_halfword_rotate(bus, addr)),
                (true, true) => {
                    let value = read_signed_halfword(bus, addr);
                    self.write_reg(rd, value);
                }
            }
        }

        2
    }

    fn thumb_load_store_imm(&mut self, instr: u16, _pc: u32, bus: &mut GbaBus) -> u32 {
        let op = (instr >> 11) & 0x03;
        let offset = ((instr >> 6) & 0x1F) as u32;
        let rb = ((instr >> 3) & 0x07) as usize;
        let rd = (instr & 0x07) as usize;

        match op {
            0 => {
                let addr = self.regs[rb].wrapping_add(offset << 2);
                bus.write32(addr, self.regs[rd]);
            }
            1 => {
                let addr = self.regs[rb].wrapping_add(offset << 2);
                self.write_reg(rd, read_word_rotate(bus, addr));
            }
            2 => {
                let addr = self.regs[rb].wrapping_add(offset);
                bus.write8(addr, self.regs[rd] as u8);
            }
            _ => {
                let addr = self.regs[rb].wrapping_add(offset);
                self.write_reg(rd, bus.read8(addr) as u32);
            }
        }

        2
    }

    fn thumb_load_store_halfword_imm(&mut self, instr: u16, _pc: u32, bus: &mut GbaBus) -> u32 {
        let load = (instr & (1 << 11)) != 0;
        let offset = ((instr >> 6) & 0x1F) as u32;
        let rb = ((instr >> 3) & 0x07) as usize;
        let rd = (instr & 0x07) as usize;
        let addr = self.regs[rb].wrapping_add(offset << 1);

        if load {
            self.write_reg(rd, read_halfword_rotate(bus, addr));
        } else {
            write_halfword_aligned(bus, addr, self.regs[rd] as u16);
        }

        2
    }

    fn thumb_push_pop(&mut self, instr: u16, _pc: u32, bus: &mut GbaBus) -> u32 {
        let pop = (instr & (1 << 11)) != 0;
        let include_r = (instr & (1 << 8)) != 0;
        let list = (instr & 0xFF) as u8;
        let reg_count = list.count_ones() as u32;

        if pop {
            for reg in 0..8usize {
                if (list & (1u8 << reg)) == 0 {
                    continue;
                }
                let value = bus.read32(self.regs[13]);
                self.regs[13] = self.regs[13].wrapping_add(4);
                self.write_reg(reg, value);
            }
            if include_r {
                let value = bus.read32(self.regs[13]);
                self.regs[13] = self.regs[13].wrapping_add(4);
                if !self.try_no_bios_irq_return(value) {
                    self.set_thumb_mode((value & 1) != 0);
                    self.regs[15] = value & !1;
                }
            }
        } else {
            if include_r {
                self.regs[13] = self.regs[13].wrapping_sub(4);
                bus.write32(self.regs[13], self.regs[14]);
            }
            for reg in (0..8usize).rev() {
                if (list & (1u8 << reg)) == 0 {
                    continue;
                }
                self.regs[13] = self.regs[13].wrapping_sub(4);
                bus.write32(self.regs[13], self.regs[reg]);
            }
        }

        reg_count.saturating_add(1)
    }

    fn thumb_multiple_load_store(&mut self, instr: u16, _pc: u32, bus: &mut GbaBus) -> u32 {
        let load = (instr & (1 << 11)) != 0;
        let rb = ((instr >> 8) & 0x07) as usize;
        let list = (instr & 0xFF) as u8;
        let reg_count = list.count_ones() as u32;
        if reg_count == 0 {
            return 1;
        }

        let mut addr = self.regs[rb];
        for reg in 0..8usize {
            if (list & (1u8 << reg)) == 0 {
                continue;
            }
            if load {
                self.write_reg(reg, bus.read32(addr));
            } else {
                bus.write32(addr, self.regs[reg]);
            }
            addr = addr.wrapping_add(4);
        }
        self.regs[rb] = addr;

        reg_count.saturating_add(1)
    }

    fn handle_swi(&mut self, number: u32, bus: &mut GbaBus) {
        self.trace_swi_call(number);
        match number & 0xFF {
            0x00 => self.reset(),
            0x01 => {
                let flags = self.regs[0] as u8;
                if (flags & 0x01) != 0 {
                    bus.clear_ewram();
                }
                if (flags & 0x02) != 0 {
                    bus.clear_iwram();
                }
                if (flags & 0x04) != 0 {
                    bus.clear_pram();
                }
                if (flags & 0x08) != 0 {
                    bus.clear_vram();
                }
                if (flags & 0x10) != 0 {
                    bus.clear_oam();
                }
            }
            0x02 => {
                self.halted = true;
                self.halt_irq_mask = u16::MAX;
                self.clear_irq_on_halt_wake = false;
            }
            0x03 => {
                self.halted = true;
                self.halt_irq_mask = STOP_WAKE_IRQ_MASK;
                self.clear_irq_on_halt_wake = false;
            }
            0x04 => self.swi_intr_wait(bus, false),
            0x05 => self.swi_intr_wait(bus, true),
            0x06 => {
                let numerator = self.regs[0] as i32;
                let denominator = self.regs[1] as i32;
                self.swi_div(numerator, denominator);
            }
            0x07 => {
                let numerator = self.regs[1] as i32;
                let denominator = self.regs[0] as i32;
                self.swi_div(numerator, denominator);
            }
            0x08 => {
                self.regs[0] = integer_sqrt(self.regs[0]);
            }
            0x09 => self.swi_arctan(),
            0x0A => self.swi_arctan2(),
            0x0D => {
                // BIOS checksum constant returned by official GBA BIOS.
                self.regs[0] = 0xBAAE_187F;
            }
            0x0E => self.swi_bg_affine_set(bus),
            0x0F => self.swi_obj_affine_set(bus),
            0x10 => self.swi_bit_unpack(bus),
            0x11 => self.swi_lz77_uncomp(bus, false),
            0x12 => self.swi_lz77_uncomp(bus, true),
            0x13 => self.swi_huff_uncomp(bus),
            0x14 => self.swi_rl_uncomp(bus, false),
            0x15 => self.swi_rl_uncomp(bus, true),
            0x16 => self.swi_diff8_unfilter(bus, false),
            0x17 => self.swi_diff8_unfilter(bus, true),
            0x18 => self.swi_diff16_unfilter(bus),
            0x19 => {
                self.swi_sound_bias(bus);
            }
            0x1A => self.swi_sound_driver_init(bus),
            0x1B => self.swi_sound_driver_mode(bus),
            0x1C => self.swi_sound_driver_main(bus),
            0x1D => self.swi_sound_driver_vsync(bus),
            0x1E => {
                self.swi_sound_channel_clear(bus);
            }
            0x1F => self.swi_midi_key2freq(bus),
            0x20 => self.swi_music_player_open(),
            0x21 => self.swi_music_player_start(),
            0x22 => self.swi_music_player_stop(),
            0x23 => self.swi_music_player_continue(),
            0x24 => self.swi_music_player_fade_out(),
            0x25 => {
                // MultiBoot success.
                self.regs[0] = 0;
            }
            0x26 => {
                self.swi_hard_reset(bus);
            }
            0x27 => self.swi_custom_halt(),
            0x28 => self.sound_bios.vsync_enabled = false,
            0x29 => self.sound_bios.vsync_enabled = true,
            0x2A => {
                self.swi_sound_get_jump_list(bus);
            }
            0x0B => self.swi_cpuset(bus),
            0x0C => self.swi_cpufastset(bus),
            _ => self.trace_unhandled_swi(number),
        }
    }

    fn swi_sound_bias(&mut self, bus: &mut GbaBus) {
        bus.write16(REG_SOUNDBIAS, self.regs[0] as u16);
    }

    fn swi_sound_driver_init(&mut self, bus: &mut GbaBus) {
        self.sound_bios.initialized = true;
        self.sound_bios.vsync_enabled = true;
        self.sound_bios.vsync_ticks = 0;

        let soundcnt_x = bus.read16(REG_SOUNDCNT_X) | 0x0080;
        bus.write16(REG_SOUNDCNT_X, soundcnt_x);
        if bus.read16(REG_SOUNDBIAS) == 0 {
            bus.write16(REG_SOUNDBIAS, 0x0200);
        }

        let mut soundcnt_h = bus.read16(REG_SOUNDCNT_H);
        soundcnt_h |= (1 << 8) | (1 << 9) | (1 << 12) | (1 << 13);
        bus.write16(REG_SOUNDCNT_H, soundcnt_h);

        self.swi_sound_channel_clear(bus);
    }

    fn swi_sound_driver_mode(&mut self, bus: &mut GbaBus) {
        self.sound_bios.mode = self.regs[0];
        if !self.sound_bios.initialized {
            self.swi_sound_driver_init(bus);
            return;
        }

        let soundcnt_x = bus.read16(REG_SOUNDCNT_X) | 0x0080;
        bus.write16(REG_SOUNDCNT_X, soundcnt_x);
    }

    fn swi_sound_driver_main(&mut self, bus: &mut GbaBus) {
        if !self.sound_bios.initialized {
            self.swi_sound_driver_init(bus);
        }
        self.sound_bios.vsync_ticks = self.sound_bios.vsync_ticks.wrapping_add(1);
        if self.sound_bios.player.playing && self.sound_bios.player.fadeout_frames > 0 {
            self.sound_bios.player.fadeout_frames -= 1;
            if self.sound_bios.player.fadeout_frames == 0 {
                self.sound_bios.player.playing = false;
            }
        }
    }

    fn swi_sound_driver_vsync(&mut self, bus: &mut GbaBus) {
        if self.sound_bios.vsync_enabled {
            self.swi_sound_driver_main(bus);
        }
    }

    fn swi_sound_channel_clear(&mut self, bus: &mut GbaBus) {
        let mut addr = SOUND_REG_START;
        while addr <= SOUND_REG_END {
            if addr != REG_SOUNDCNT_X && addr != REG_SOUNDBIAS {
                bus.write16(addr, 0);
            }
            addr = addr.wrapping_add(2);
        }
        bus.write32(FIFO_A_ADDR, 0);
        bus.write32(FIFO_B_ADDR, 0);
    }

    fn swi_music_player_open(&mut self) {
        self.sound_bios.player.player_ptr = self.regs[0];
        self.sound_bios.player.song_ptr = self.regs[1];
        self.sound_bios.player.playing = false;
        self.sound_bios.player.fadeout_frames = 0;
    }

    fn swi_music_player_start(&mut self) {
        if self.regs[0] != 0 {
            self.sound_bios.player.player_ptr = self.regs[0];
        }
        if self.regs[1] != 0 {
            self.sound_bios.player.song_ptr = self.regs[1];
        }
        self.sound_bios.player.playing = true;
        self.sound_bios.player.fadeout_frames = 0;
    }

    fn swi_music_player_stop(&mut self) {
        self.sound_bios.player.playing = false;
        self.sound_bios.player.fadeout_frames = 0;
    }

    fn swi_music_player_continue(&mut self) {
        if self.sound_bios.player.song_ptr != 0 {
            self.sound_bios.player.playing = true;
        }
    }

    fn swi_music_player_fade_out(&mut self) {
        let frames = if self.regs[1] != 0 {
            self.regs[1]
        } else {
            self.regs[0]
        };
        self.sound_bios.player.fadeout_frames = frames as u16;
        if self.sound_bios.player.fadeout_frames == 0 {
            self.sound_bios.player.playing = false;
        }
    }

    fn swi_sound_get_jump_list(&mut self, bus: &mut GbaBus) {
        let mut table_addr = self.regs[0] & !3;
        if table_addr == 0 {
            table_addr = SOUND_JUMP_LIST_BASE;
        }

        let known_count = (SOUND_JUMP_LIST_END_SWI - SOUND_JUMP_LIST_START_SWI + 1) as usize;
        let word_count = (SOUND_JUMP_LIST_BYTES / 4) as usize;
        let fallback_entry = 0x0000_0008 + ((SOUND_DRIVER_MAIN_SWI - 1) << 2);
        for index in 0..word_count {
            let entry = if index < known_count {
                let swi = SOUND_JUMP_LIST_START_SWI + index as u32;
                0x0000_0008 + ((swi - 1) << 2)
            } else {
                fallback_entry
            };
            bus.write32(table_addr.wrapping_add((index as u32) * 4), entry);
        }
        self.regs[0] = table_addr;
    }

    fn swi_intr_wait(&mut self, bus: &mut GbaBus, vblank_only: bool) {
        let (clear_old, mask) = if vblank_only {
            // BIOS SWI 0x05 is equivalent to IntrWait(1, 1), regardless of caller registers.
            self.regs[0] = 1;
            self.regs[1] = 1;
            (true, 1u16)
        } else {
            ((self.regs[0] & 1) != 0, self.regs[1] as u16)
        };

        let effective_mask = if mask == 0 { u16::MAX } else { mask };
        if !bus.has_bios() {
            let mut intr_check = self.no_bios_intr_check(bus);
            if clear_old {
                intr_check &= !effective_mask;
                self.write_no_bios_intr_check(bus, intr_check);
            }

            let pending = intr_check & effective_mask;
            if pending != 0 {
                self.write_no_bios_intr_check(bus, intr_check & !pending);
                self.clear_irq_on_halt_wake = false;
                return;
            }
        } else {
            if clear_old {
                bus.clear_irq(effective_mask);
            }

            let pending = bus.pending_interrupts() & effective_mask;
            if pending != 0 {
                bus.clear_irq(pending);
                self.clear_irq_on_halt_wake = false;
                return;
            }
        }

        self.halted = true;
        self.halt_irq_mask = effective_mask;
        // IntrWait/VBlankIntrWait should allow installed no-BIOS IRQ handlers to run.
        // If no handler exists in no-BIOS mode, keep the previous behavior and consume IF
        // on wake to avoid falling through to an unmapped IRQ vector.
        self.clear_irq_on_halt_wake = if bus.has_bios() {
            false
        } else {
            bus.read32(NO_BIOS_IRQ_HANDLER_ADDR) == 0
        };
    }

    fn swi_div(&mut self, numerator: i32, denominator: i32) {
        if denominator == 0 {
            self.regs[0] = 0;
            self.regs[1] = numerator as u32;
            self.regs[3] = 0;
            return;
        }

        let quotient = numerator.wrapping_div(denominator);
        let remainder = numerator.wrapping_rem(denominator);
        self.regs[0] = quotient as u32;
        self.regs[1] = remainder as u32;
        self.regs[3] = quotient.unsigned_abs();
    }

    fn swi_arctan(&mut self) {
        let tan = self.regs[0] as i32 as f64 / 16384.0;
        let angle = tan.atan();
        // 0x8000 represents PI.
        let scaled = (angle * (32768.0 / std::f64::consts::PI)).round() as i32;
        self.regs[0] = scaled as i16 as i32 as u32;
    }

    fn swi_arctan2(&mut self) {
        let y = self.regs[0] as i32 as f64;
        let x = self.regs[1] as i32 as f64;
        let mut angle = y.atan2(x);
        if angle < 0.0 {
            angle += std::f64::consts::TAU;
        }
        // 0x0000..=0xFFFF maps to 0..2*PI.
        let scaled = (angle * (65536.0 / std::f64::consts::TAU)).round() as u32;
        self.regs[0] = scaled & 0xFFFF;
    }

    fn swi_midi_key2freq(&mut self, bus: &GbaBus) {
        let wave_data = self.regs[0];
        if wave_data == 0 {
            self.regs[0] = 0;
            return;
        }

        // WaveData + 4 contains the base frequency used by BIOS SWI.
        let base_freq = bus.read32(wave_data.wrapping_add(4)) as f64;
        if base_freq <= 0.0 {
            self.regs[0] = 0;
            return;
        }

        let key = self.regs[1] as i32 as f64;
        let fine = self.regs[2] as i32 as f64;
        let semitone = (180.0 - key - (fine / 256.0)) / 12.0;
        let freq = base_freq * 2.0_f64.powf(semitone);
        self.regs[0] = if freq.is_finite() && freq > 0.0 {
            freq.min(u32::MAX as f64) as u32
        } else {
            0
        };
    }

    fn swi_custom_halt(&mut self) {
        let mask = self.regs[0] as u16;
        self.halted = true;
        self.halt_irq_mask = if mask == 0 { u16::MAX } else { mask };
        self.clear_irq_on_halt_wake = false;
    }

    fn swi_hard_reset(&mut self, bus: &mut GbaBus) {
        bus.reset();
        self.reset_for_boot(bus.has_bios());
    }

    fn swi_cpuset(&mut self, bus: &mut GbaBus) {
        let control = self.regs[2];
        let count = control & 0x001F_FFFF;
        if count == 0 {
            return;
        }

        let fixed = (control & (1 << 24)) != 0;
        let word = (control & (1 << 26)) != 0;

        if word {
            let src = self.regs[0] & !3;
            let dst = self.regs[1] & !3;
            let source_value = bus.read32(src);
            for i in 0..count {
                let value = if fixed {
                    source_value
                } else {
                    bus.read32(src.wrapping_add(i * 4))
                };
                bus.write32(dst.wrapping_add(i * 4), value);
            }
        } else {
            let src = self.regs[0] & !1;
            let dst = self.regs[1] & !1;
            let source_value = bus.read16(src);
            for i in 0..count {
                let value = if fixed {
                    source_value
                } else {
                    bus.read16(src.wrapping_add(i * 2))
                };
                bus.write16(dst.wrapping_add(i * 2), value);
            }
        }
    }

    fn swi_cpufastset(&mut self, bus: &mut GbaBus) {
        let src = self.regs[0] & !3;
        let dst = self.regs[1] & !3;
        let control = self.regs[2];
        let count_words = control & 0x001F_FFFF;
        if count_words == 0 {
            return;
        }

        // CpuFastSet transfers in 8-word blocks.
        // Length values not divisible by 8 are truncated, matching BIOS behavior.
        let total_words = count_words & !7;
        if total_words == 0 {
            return;
        }
        let fixed = (control & (1 << 24)) != 0;
        let source_value = bus.read32(src);

        for i in 0..total_words {
            let value = if fixed {
                source_value
            } else {
                bus.read32(src.wrapping_add(i * 4))
            };
            bus.write32(dst.wrapping_add(i * 4), value);
        }
    }

    fn swi_bit_unpack(&mut self, bus: &mut GbaBus) {
        let src = self.regs[0];
        let dst = self.regs[1];
        let info = self.regs[2];

        let src_len = bus.read16(info) as u32;
        let src_width = bus.read8(info.wrapping_add(2)) as u32;
        let dst_width = bus.read8(info.wrapping_add(3)) as u32;
        let offset_and_flags = bus.read32(info.wrapping_add(4));
        if src_len == 0
            || src_width == 0
            || dst_width == 0
            || src_width > 8
            || dst_width > 32
            || (8 % src_width) != 0
        {
            return;
        }

        let offset = offset_and_flags & 0x7FFF_FFFF;
        let offset_for_zero = (offset_and_flags & 0x8000_0000) != 0;
        let src_mask = if src_width == 32 {
            u32::MAX
        } else {
            (1u32 << src_width) - 1
        };
        let dst_mask = if dst_width == 32 {
            u32::MAX
        } else {
            (1u32 << dst_width) - 1
        };
        let total_units = (src_len * 8) / src_width;

        let mut src_ptr = src;
        let mut dst_ptr = dst;
        let mut src_cache = 0u32;
        let mut src_bits = 0u32;
        let mut dst_cache = 0u64;
        let mut dst_bits = 0u32;

        for _ in 0..total_units {
            while src_bits < src_width {
                src_cache |= (bus.read8(src_ptr) as u32) << src_bits;
                src_ptr = src_ptr.wrapping_add(1);
                src_bits += 8;
            }

            let mut value = src_cache & src_mask;
            src_cache >>= src_width;
            src_bits -= src_width;

            if value != 0 || offset_for_zero {
                value = value.wrapping_add(offset);
            }
            value &= dst_mask;

            dst_cache |= (value as u64) << dst_bits;
            dst_bits += dst_width;

            while dst_bits >= 8 {
                bus.write8(dst_ptr, dst_cache as u8);
                dst_ptr = dst_ptr.wrapping_add(1);
                dst_cache >>= 8;
                dst_bits -= 8;
            }
        }

        if dst_bits > 0 {
            bus.write8(dst_ptr, dst_cache as u8);
        }
    }

    fn write_decompressed_bytes(bus: &mut GbaBus, dst: u32, bytes: &[u8], to_vram: bool) {
        if !to_vram {
            for (i, byte) in bytes.iter().enumerate() {
                bus.write8(dst.wrapping_add(i as u32), *byte);
            }
            return;
        }

        let mut offset = 0u32;
        let mut index = 0usize;
        while index < bytes.len() {
            let lo = bytes[index] as u16;
            let hi = if index + 1 < bytes.len() {
                (bytes[index + 1] as u16) << 8
            } else {
                0
            };
            bus.write16(dst.wrapping_add(offset), lo | hi);
            index += 2;
            offset = offset.wrapping_add(2);
        }
    }

    fn swi_lz77_uncomp(&mut self, bus: &mut GbaBus, to_vram: bool) {
        let src = self.regs[0];
        let dst = self.regs[1];
        let header = bus.read32(src);
        if (header & 0xFF) != 0x10 {
            return;
        }

        let output_size = header >> 8;
        let mut src_ptr = src.wrapping_add(4);
        let mut out = Vec::with_capacity(output_size as usize);

        while (out.len() as u32) < output_size {
            let flags = bus.read8(src_ptr);
            src_ptr = src_ptr.wrapping_add(1);

            for bit in 0..8 {
                if (out.len() as u32) >= output_size {
                    break;
                }

                if (flags & (0x80 >> bit)) == 0 {
                    let value = bus.read8(src_ptr);
                    src_ptr = src_ptr.wrapping_add(1);
                    out.push(value);
                } else {
                    let b1 = bus.read8(src_ptr) as u16;
                    let b2 = bus.read8(src_ptr.wrapping_add(1)) as u16;
                    src_ptr = src_ptr.wrapping_add(2);

                    let length = ((b1 >> 4) as u32) + 3;
                    let disp = ((((b1 & 0x000F) << 8) | b2) as u32) + 1;
                    for _ in 0..length {
                        if (out.len() as u32) >= output_size {
                            break;
                        }
                        let source_index = out.len().saturating_sub(disp as usize);
                        let value = out[source_index];
                        out.push(value);
                    }
                }
            }
        }

        Self::write_decompressed_bytes(bus, dst, &out, to_vram);
    }

    fn swi_huff_uncomp(&mut self, bus: &mut GbaBus) {
        let mut src = self.regs[0] & !3;
        let mut dst = self.regs[1];
        let header = bus.read32(src);
        // HuffUnComp header:
        // bits 0-3   = symbol bit width (4 or 8)
        // bits 4-7   = type (0x2)
        // bits 8-31  = uncompressed size in bytes
        if (header & 0xF0) != 0x20 {
            return;
        }

        let mut remaining = header >> 8;
        let symbol_bits = (header & 0x0F) as u8;
        if symbol_bits == 0 || symbol_bits == 1 || symbol_bits > 16 || (32 % symbol_bits) != 0 {
            return;
        }

        // Tree byte count in bytes (not including header), as used by BIOS.
        let tree_bytes = (u32::from(bus.read8(src.wrapping_add(4))) << 1) + 1;
        let tree_base = src.wrapping_add(5);
        src = src.wrapping_add(5 + tree_bytes);

        let mut node_ptr = tree_base;
        let mut node = bus.read8(node_ptr);
        let symbol_mask = if symbol_bits == 32 {
            u32::MAX
        } else {
            (1u32 << symbol_bits) - 1
        };
        let mut block = 0u32;
        let mut bits_seen = 0u32;

        while remaining > 0 {
            let mut bitstream = bus.read32(src);
            src = src.wrapping_add(4);
            for _ in 0..32 {
                if remaining == 0 {
                    break;
                }

                let next = (node_ptr & !1)
                    .wrapping_add(u32::from(node & 0x3F) * 2)
                    .wrapping_add(2);
                let symbol = if (bitstream & 0x8000_0000) != 0 {
                    // Right child: terminal flag is bit6.
                    if (node & 0x40) != 0 {
                        bus.read8(next.wrapping_add(1))
                    } else {
                        node_ptr = next.wrapping_add(1);
                        node = bus.read8(node_ptr);
                        bitstream <<= 1;
                        continue;
                    }
                } else if (node & 0x80) != 0 {
                    // Left child: terminal flag is bit7.
                    bus.read8(next)
                } else {
                    node_ptr = next;
                    node = bus.read8(node_ptr);
                    bitstream <<= 1;
                    continue;
                };

                block |= (u32::from(symbol) & symbol_mask) << bits_seen;
                bits_seen += u32::from(symbol_bits);
                node_ptr = tree_base;
                node = bus.read8(node_ptr);

                if bits_seen == 32 {
                    bits_seen = 0;
                    bus.write32(dst, block);
                    dst = dst.wrapping_add(4);
                    remaining = remaining.saturating_sub(4);
                    block = 0;
                }

                bitstream <<= 1;
            }
        }
        if trace_swi_enabled() {
            eprintln!(
                "[gba:trace:swi13] src={:#010X} dst={:#010X} hdr={:#010X} out0={:#010X} out1={:#010X}",
                self.regs[0],
                self.regs[1],
                header,
                bus.read32(self.regs[1] & !3),
                bus.read32((self.regs[1] & !3).wrapping_add(4))
            );
        }
        self.regs[0] = src;
        self.regs[1] = dst;
    }

    fn swi_diff8_unfilter(&mut self, bus: &mut GbaBus, to_vram: bool) {
        let src = self.regs[0];
        let dst = self.regs[1];
        let header = bus.read32(src);
        if (header & 0xFF) != 0x81 {
            return;
        }

        let output_size = header >> 8;
        if output_size == 0 {
            return;
        }

        let mut src_ptr = src.wrapping_add(4);
        let mut out = Vec::with_capacity(output_size as usize);
        let mut accumulator = bus.read8(src_ptr);
        src_ptr = src_ptr.wrapping_add(1);
        out.push(accumulator);

        for _ in 1..output_size {
            let delta = bus.read8(src_ptr);
            src_ptr = src_ptr.wrapping_add(1);
            accumulator = accumulator.wrapping_add(delta);
            out.push(accumulator);
        }

        Self::write_decompressed_bytes(bus, dst, &out, to_vram);
    }

    fn swi_diff16_unfilter(&mut self, bus: &mut GbaBus) {
        let src = self.regs[0];
        let dst = self.regs[1];
        let header = bus.read32(src);
        if (header & 0xFF) != 0x82 {
            if trace_swi_enabled() {
                eprintln!(
                    "[gba:trace:swi18] skipped src={:#010X} dst={:#010X} hdr={:#010X}",
                    src, dst, header
                );
            }
            return;
        }

        let output_size = header >> 8;
        if output_size < 2 {
            return;
        }

        let mut src_ptr = src.wrapping_add(4);
        let mut accumulator = bus.read16(src_ptr);
        src_ptr = src_ptr.wrapping_add(2);
        bus.write16(dst, accumulator);

        let halfwords = output_size / 2;
        for i in 1..halfwords {
            let delta = bus.read16(src_ptr);
            src_ptr = src_ptr.wrapping_add(2);
            accumulator = accumulator.wrapping_add(delta);
            bus.write16(dst.wrapping_add(i * 2), accumulator);
        }
        if trace_swi_enabled() {
            eprintln!(
                "[gba:trace:swi18] src={:#010X} dst={:#010X} hdr={:#010X} out0={:#010X} out1={:#010X}",
                src,
                dst,
                header,
                bus.read32(dst & !3),
                bus.read32((dst & !3).wrapping_add(4))
            );
        }
    }

    fn swi_bg_affine_set(&mut self, bus: &mut GbaBus) {
        let mut src = self.regs[0];
        let mut dst = self.regs[1];
        let count = self.regs[2];

        for _ in 0..count {
            let ox = bus.read32(src) as i32;
            let oy = bus.read32(src.wrapping_add(4)) as i32;
            let cx = bus.read16(src.wrapping_add(8)) as i16 as i32;
            let cy = bus.read16(src.wrapping_add(10)) as i16 as i32;
            let sx = bus.read16(src.wrapping_add(12)) as i16 as i32;
            let sy = bus.read16(src.wrapping_add(14)) as i16 as i32;
            let theta = bus.read16(src.wrapping_add(16));

            let (pa, pb, pc, pd) = affine_matrix_from_scale_angle(sx, sy, theta);
            bus.write16(dst, pa as u16);
            bus.write16(dst.wrapping_add(2), pb as u16);
            bus.write16(dst.wrapping_add(4), pc as u16);
            bus.write16(dst.wrapping_add(6), pd as u16);

            let dx = ox as i64 - (pa as i64 * cx as i64) - (pb as i64 * cy as i64);
            let dy = oy as i64 - (pc as i64 * cx as i64) - (pd as i64 * cy as i64);
            bus.write32(dst.wrapping_add(8), dx as i32 as u32);
            bus.write32(dst.wrapping_add(12), dy as i32 as u32);

            src = src.wrapping_add(20);
            dst = dst.wrapping_add(16);
        }
    }

    fn swi_obj_affine_set(&mut self, bus: &mut GbaBus) {
        let mut src = self.regs[0];
        let mut dst = self.regs[1];
        let count = self.regs[2];
        let offset = self.regs[3];
        if offset == 0 {
            return;
        }

        for _ in 0..count {
            let sx = bus.read16(src) as i16 as i32;
            let sy = bus.read16(src.wrapping_add(2)) as i16 as i32;
            let theta = bus.read16(src.wrapping_add(4));
            let (pa, pb, pc, pd) = affine_matrix_from_scale_angle(sx, sy, theta);

            bus.write16(dst, pa as u16);
            bus.write16(dst.wrapping_add(offset), pb as u16);
            bus.write16(dst.wrapping_add(offset.wrapping_mul(2)), pc as u16);
            bus.write16(dst.wrapping_add(offset.wrapping_mul(3)), pd as u16);

            src = src.wrapping_add(8);
            dst = dst.wrapping_add(offset.wrapping_mul(4));
        }
    }

    fn swi_rl_uncomp(&mut self, bus: &mut GbaBus, to_vram: bool) {
        let src = self.regs[0];
        let dst = self.regs[1];
        let header = bus.read32(src);
        if (header & 0xFF) != 0x30 {
            return;
        }

        let output_size = header >> 8;
        let mut src_ptr = src.wrapping_add(4);
        let mut out = Vec::with_capacity(output_size as usize);

        while (out.len() as u32) < output_size {
            let block = bus.read8(src_ptr);
            src_ptr = src_ptr.wrapping_add(1);

            if (block & 0x80) != 0 {
                let run_length = ((block & 0x7F) as u32) + 3;
                let value = bus.read8(src_ptr);
                src_ptr = src_ptr.wrapping_add(1);
                for _ in 0..run_length {
                    if (out.len() as u32) >= output_size {
                        break;
                    }
                    out.push(value);
                }
            } else {
                let raw_length = ((block & 0x7F) as u32) + 1;
                for _ in 0..raw_length {
                    if (out.len() as u32) >= output_size {
                        break;
                    }
                    let value = bus.read8(src_ptr);
                    src_ptr = src_ptr.wrapping_add(1);
                    out.push(value);
                }
            }
        }

        Self::write_decompressed_bytes(bus, dst, &out, to_vram);
    }

    fn decode_arm_operand2(&self, instr: u32, pc: u32, bus: &GbaBus) -> (u32, Option<bool>) {
        if (instr & (1 << 25)) != 0 {
            let imm = instr & 0xFF;
            let rotate = ((instr >> 8) & 0x0F) * 2;
            if rotate == 0 {
                (imm, Some(self.flag_c()))
            } else {
                let result = imm.rotate_right(rotate);
                (result, Some((result & 0x8000_0000) != 0))
            }
        } else {
            let rm = (instr & 0x0F) as usize;
            let rm_value = self.read_arm_reg(rm, pc);
            if (instr & (1 << 4)) == 0 {
                let shift_type = ((instr >> 5) & 0x03) as u8;
                let shift_imm = ((instr >> 7) & 0x1F) as u8;
                let (value, carry) = shift_imm_only(rm_value, shift_type, shift_imm, self.flag_c());
                (value, Some(carry))
            } else {
                let rs = ((instr >> 8) & 0x0F) as usize;
                let shift = (self.read_arm_reg(rs, pc) & 0xFF) as u8;
                let shift_type = ((instr >> 5) & 0x03) as u8;
                let (value, carry) = shift_reg(rm_value, shift_type, shift, self.flag_c());
                let _ = bus;
                (value, Some(carry))
            }
        }
    }

    fn fetch_arm(&mut self, bus: &mut GbaBus) -> (u32, u32) {
        let pc = self.regs[15] & !3;
        bus.note_exec_fetch(pc);
        let instr = bus.fetch32_instr(pc);
        self.regs[15] = pc.wrapping_add(4);
        (pc, instr)
    }

    fn fetch_thumb(&mut self, bus: &mut GbaBus) -> (u32, u16) {
        let pc = self.regs[15] & !1;
        bus.note_exec_fetch(pc);
        let instr = bus.fetch16_instr(pc);
        self.regs[15] = pc.wrapping_add(2);
        (pc, instr)
    }

    fn read_arm_reg(&self, index: usize, pc: u32) -> u32 {
        if index == 15 {
            pc.wrapping_add(8)
        } else {
            self.regs[index]
        }
    }

    fn read_user_reg(&self, index: usize, current_pc: u32) -> u32 {
        match index {
            13 => self.r13_usr,
            14 => self.r14_usr,
            15 => current_pc.wrapping_add(12),
            _ => self.regs[index],
        }
    }

    fn write_reg(&mut self, index: usize, value: u32) {
        if index == 15 {
            if self.try_no_bios_irq_return(value) {
                return;
            }
            if self.thumb_mode() {
                self.regs[15] = value & !1;
            } else {
                self.regs[15] = value & !3;
            }
        } else {
            self.regs[index] = value;
        }
    }

    fn write_user_reg(&mut self, index: usize, value: u32) {
        match index {
            13 => {
                self.r13_usr = value;
                if matches!(self.cpsr & MODE_MASK, MODE_USR | MODE_SYS) {
                    self.regs[13] = value;
                }
            }
            14 => {
                self.r14_usr = value;
                if matches!(self.cpsr & MODE_MASK, MODE_USR | MODE_SYS) {
                    self.regs[14] = value;
                }
            }
            15 => self.write_reg(index, value),
            _ => self.regs[index] = value,
        }
    }

    fn store_value(&self, reg: usize, current_pc: u32) -> u32 {
        if reg == 15 {
            current_pc.wrapping_add(12)
        } else {
            self.regs[reg]
        }
    }

    fn branch_exchange(&mut self, target: u32) {
        if self.try_no_bios_irq_return(target) {
            return;
        }

        if self.no_bios_irq_state.is_some() && !is_valid_exec_addr(target & !1) {
            // No-BIOS IRQ callbacks sometimes unwind through hand-rolled stubs that can
            // transiently surface non-executable branch targets when our BIOS wrapper
            // emulation diverges. Prefer a safe IRQ return over running into IO/open bus.
            if self.try_no_bios_irq_return(NO_BIOS_IRQ_RETURN_TOKEN) {
                return;
            }
        }

        if trace_branch_enabled() && target < 0x0200_0000 {
            if let Some(slot) = take_trace_slot(&TRACE_BRANCH_COUNT) {
                eprintln!(
                    "[gba:trace:branch] slot={}/{} pc={:#010X} mode={:#04X} target={:#010X} sp={:#010X} lr={:#010X} r0={:#010X}",
                    slot + 1,
                    trace_limit(),
                    self.regs[15],
                    self.cpsr & MODE_MASK,
                    target,
                    self.regs[13],
                    self.regs[14],
                    self.regs[0]
                );
            }
        }
        let thumb = (target & 1) != 0;
        self.set_thumb_mode(thumb);
        self.regs[15] = if thumb { target & !1 } else { target & !3 };
        self.thumb_bl_upper = None;
    }

    fn try_no_bios_irq_exception_return(&mut self, target: u32) -> bool {
        if self.no_bios_irq_state.is_none() {
            return false;
        }
        if !matches!(
            target | 1,
            NO_BIOS_IRQ_RETURN_TOKEN | NO_BIOS_IRQ_RETURN_TOKEN_SUBS_PC_LR_4
        ) {
            return false;
        }
        self.try_no_bios_irq_return(target)
    }

    fn enter_exception(&mut self, mode: u32, return_addr: u32, target: u32) {
        let old_cpsr = self.cpsr;
        self.set_spsr_for_mode(mode, old_cpsr);
        let pending_thumb_bl_upper = self.thumb_bl_upper.take();
        self.set_exception_thumb_bl_upper(mode, pending_thumb_bl_upper);

        let target_thumb = (target & 1) != 0;
        let mut next_cpsr = (old_cpsr & !MODE_MASK) | mode;
        next_cpsr |= FLAG_I;
        if target_thumb {
            next_cpsr |= FLAG_T;
        } else {
            next_cpsr &= !FLAG_T;
        }
        self.set_cpsr(next_cpsr);

        self.regs[14] = return_addr;
        self.regs[15] = if target_thumb {
            target & !1
        } else {
            target & !3
        };
    }

    fn try_no_bios_irq_return(&mut self, target: u32) -> bool {
        if self.no_bios_irq_state.is_none() {
            return false;
        }
        // Accept both direct callback returns (BX/MOV PC,LR) and ARM-style
        // exception returns such as SUBS PC,LR,#4 used by some no-BIOS stubs.
        if !matches!(
            target | 1,
            NO_BIOS_IRQ_RETURN_TOKEN | NO_BIOS_IRQ_RETURN_TOKEN_SUBS_PC_LR_4
        ) {
            return false;
        }

        if let Some(state) = self.no_bios_irq_state.take() {
            self.trace_irq_return();
            self.set_cpsr(state.saved_cpsr);
            self.regs[..15].copy_from_slice(&state.saved_regs);
            // Keep the synthetic no-BIOS IRQ stack frame transient; BIOS callback
            // wrappers can inspect it, but IRQ SP itself must not drift each IRQ.
            self.r13_irq = state.saved_irq_sp;
            self.regs[15] = if self.thumb_mode() {
                state.resume_pc & !1
            } else {
                state.resume_pc & !3
            };
            self.thumb_bl_upper = state.saved_thumb_bl_upper;
            true
        } else {
            false
        }
    }

    fn enter_undefined_exception(&mut self, bus: &GbaBus, return_addr: u32) -> bool {
        if !bus.has_bios() {
            return false;
        }
        self.enter_exception(MODE_UND, return_addr, 0x0000_0004);
        true
    }

    fn trace_unknown_arm(&self, instr: u32, pc: u32, has_bios: bool) {
        if !trace_unknown_enabled() {
            return;
        }
        if let Some(slot) = take_trace_slot(&TRACE_UNKNOWN_COUNT) {
            eprintln!(
                "[gba:trace:unknown] slot={}/{} kind=arm pc={:#010X} instr={:#010X} mode={:#04X} bios={}",
                slot + 1,
                trace_limit(),
                pc,
                instr,
                self.cpsr & MODE_MASK,
                if has_bios { 1 } else { 0 }
            );
        }
    }

    fn trace_unknown_thumb(&self, instr: u16, pc: u32, has_bios: bool) {
        if !trace_unknown_enabled() {
            return;
        }
        if let Some(slot) = take_trace_slot(&TRACE_UNKNOWN_COUNT) {
            eprintln!(
                "[gba:trace:unknown] slot={}/{} kind=thumb pc={:#010X} instr={:#06X} mode={:#04X} bios={}",
                slot + 1,
                trace_limit(),
                pc,
                instr,
                self.cpsr & MODE_MASK,
                if has_bios { 1 } else { 0 }
            );
        }
    }

    fn trace_swi_call(&self, number: u32) {
        if !trace_swi_enabled() {
            return;
        }
        if let Some(slot) = take_trace_slot(&TRACE_SWI_COUNT) {
            eprintln!(
                "[gba:trace:swi] slot={}/{} pc={:#010X} num={:#04X} r0={:#010X} r1={:#010X} r2={:#010X} r3={:#010X}",
                slot + 1,
                trace_limit(),
                self.regs[15],
                number & 0xFF,
                self.regs[0],
                self.regs[1],
                self.regs[2],
                self.regs[3]
            );
        }
    }

    fn trace_unhandled_swi(&self, number: u32) {
        if !(trace_swi_enabled() || trace_unknown_enabled()) {
            return;
        }
        if let Some(slot) = take_trace_slot(&TRACE_UNHANDLED_SWI_COUNT) {
            eprintln!(
                "[gba:trace:swi-unhandled] slot={}/{} pc={:#010X} num={:#04X}",
                slot + 1,
                trace_limit(),
                self.regs[15],
                number & 0xFF
            );
        }
    }

    fn trace_irq_entry(&self, pending: u16, target: u32, bios: bool) {
        if !trace_irq_enabled() {
            return;
        }
        if let Some(slot) = take_trace_slot(&TRACE_IRQ_COUNT) {
            eprintln!(
                "[gba:trace:irq] slot={}/{} kind=enter pc={:#010X} mode={:#04X} pending={:#06X} target={:#010X} bios={} sp={:#010X} lr={:#010X}",
                slot + 1,
                trace_limit(),
                self.regs[15],
                self.cpsr & MODE_MASK,
                pending,
                target,
                if bios { 1 } else { 0 },
                self.regs[13],
                self.regs[14]
            );
        }
    }

    fn trace_irq_return(&self) {
        if !trace_irq_enabled() {
            return;
        }
        if let Some(slot) = take_trace_slot(&TRACE_IRQ_COUNT) {
            eprintln!(
                "[gba:trace:irq] slot={}/{} kind=return pc={:#010X} mode={:#04X} sp={:#010X} lr={:#010X}",
                slot + 1,
                trace_limit(),
                self.regs[15],
                self.cpsr & MODE_MASK,
                self.regs[13],
                self.regs[14]
            );
        }
    }

    fn trace_irq_instruction(&self, pc: u32, instr: u32, thumb: bool, bus: &GbaBus) {
        if !trace_irq_code_enabled() {
            return;
        }
        let in_irq_flow = (self.cpsr & MODE_MASK) == MODE_IRQ || self.no_bios_irq_state.is_some();
        if !in_irq_flow {
            return;
        }
        if let Some(slot) = take_trace_slot(&TRACE_IRQ_CODE_COUNT) {
            if thumb {
                eprintln!(
                    "[gba:trace:irq-code] slot={}/{} mode=thumb pc={:#010X} instr={:#06X} r0={:#010X} r1={:#010X} r2={:#010X} r3={:#010X} r4={:#010X} r5={:#010X} r6={:#010X} r7={:#010X} r12={:#010X} sp={:#010X} lr={:#010X} pending={:#06X}",
                    slot + 1,
                    trace_limit(),
                    pc,
                    instr & 0xFFFF,
                    self.regs[0],
                    self.regs[1],
                    self.regs[2],
                    self.regs[3],
                    self.regs[4],
                    self.regs[5],
                    self.regs[6],
                    self.regs[7],
                    self.regs[12],
                    self.regs[13],
                    self.regs[14],
                    bus.pending_interrupts()
                );
            } else {
                eprintln!(
                    "[gba:trace:irq-code] slot={}/{} mode=arm pc={:#010X} instr={:#010X} r0={:#010X} r1={:#010X} r2={:#010X} r3={:#010X} r4={:#010X} r5={:#010X} r6={:#010X} r7={:#010X} r12={:#010X} sp={:#010X} lr={:#010X} pending={:#06X}",
                    slot + 1,
                    trace_limit(),
                    pc,
                    instr,
                    self.regs[0],
                    self.regs[1],
                    self.regs[2],
                    self.regs[3],
                    self.regs[4],
                    self.regs[5],
                    self.regs[6],
                    self.regs[7],
                    self.regs[12],
                    self.regs[13],
                    self.regs[14],
                    bus.pending_interrupts()
                );
            }
        }
    }

    fn trace_bad_pc(&self, from_pc: u32, was_thumb: bool, bus: &GbaBus) {
        if !trace_bad_pc_enabled() {
            return;
        }
        let pc = self.regs[15];
        if is_valid_exec_addr(pc) {
            return;
        }
        if let Some(slot) = take_trace_slot(&TRACE_BAD_PC_COUNT) {
            let fetched = if was_thumb {
                u32::from(bus.read16(from_pc & !1))
            } else {
                bus.read32(from_pc & !3)
            };
            let sp = self.regs[13];
            let stack_m1 = bus.read32(sp.wrapping_sub(4));
            let stack_m2 = bus.read32(sp.wrapping_sub(8));
            let stack_m3 = bus.read32(sp.wrapping_sub(12));
            let stack_m4 = bus.read32(sp.wrapping_sub(16));
            eprintln!(
                "[gba:trace:bad-pc] slot={}/{} from_pc={:#010X} from_mode={} fetched={:#010X} to_pc={:#010X} cpsr={:#010X} sp={:#010X} lr={:#010X} r0={:#010X} r1={:#010X} r2={:#010X} r3={:#010X} r4={:#010X} r5={:#010X} r6={:#010X} r7={:#010X} [sp-4]={:#010X} [sp-8]={:#010X} [sp-12]={:#010X} [sp-16]={:#010X}",
                slot + 1,
                trace_limit(),
                from_pc,
                if was_thumb { "thumb" } else { "arm" },
                fetched,
                pc,
                self.cpsr,
                sp,
                self.regs[14],
                self.regs[0],
                self.regs[1],
                self.regs[2],
                self.regs[3],
                self.regs[4],
                self.regs[5],
                self.regs[6],
                self.regs[7],
                stack_m1,
                stack_m2,
                stack_m3,
                stack_m4,
            );
        }
    }

    fn trace_sp_change(&self, from_pc: u32, was_thumb: bool, old_sp: u32, bus: &GbaBus) {
        if !trace_sp_enabled() {
            return;
        }
        let new_sp = self.regs[13];
        if new_sp == old_sp {
            return;
        }
        if let Some(min_sp) = trace_sp_min() {
            if old_sp < min_sp && new_sp < min_sp {
                return;
            }
        }

        if let Some(slot) = take_trace_slot(&TRACE_SP_COUNT) {
            let fetched = if was_thumb {
                u32::from(bus.read16(from_pc & !1))
            } else {
                bus.read32(from_pc & !3)
            };
            let delta = (new_sp as i64) - (old_sp as i64);
            eprintln!(
                "[gba:trace:sp] slot={}/{} pc={:#010X} mode={} instr={:#010X} sp_old={:#010X} sp_new={:#010X} delta={:+#x} lr={:#010X} r0={:#010X} r1={:#010X} r2={:#010X} r3={:#010X}",
                slot + 1,
                trace_limit(),
                from_pc,
                if was_thumb { "thumb" } else { "arm" },
                fetched,
                old_sp,
                new_sp,
                delta,
                self.regs[14],
                self.regs[0],
                self.regs[1],
                self.regs[2],
                self.regs[3],
            );
        }
    }

    fn trace_irq_pointer_change(&mut self, from_pc: u32, was_thumb: bool, bus: &GbaBus) {
        if !trace_irq_ptr_enabled() || bus.has_bios() {
            return;
        }

        let callback = bus.read32(0x0300_7FF8);
        let handler = bus.read32(0x0300_7FFC);

        if !self.trace_irq_ptr_initialized {
            self.trace_irq_ptr_initialized = true;
            self.trace_irq_callback_prev = callback;
            self.trace_irq_handler_prev = handler;
            return;
        }

        if callback == self.trace_irq_callback_prev && handler == self.trace_irq_handler_prev {
            return;
        }

        if let Some(slot) = take_trace_slot(&TRACE_IRQ_PTR_COUNT) {
            let fetched = if was_thumb {
                u32::from(bus.read16(from_pc & !1))
            } else {
                bus.read32(from_pc & !3)
            };
            eprintln!(
                "[gba:trace:irq-ptr] slot={}/{} pc={:#010X} mode={} instr={:#010X} callback={:#010X}->{:#010X} handler={:#010X}->{:#010X} sp={:#010X} lr={:#010X}",
                slot + 1,
                trace_limit(),
                from_pc,
                if was_thumb { "thumb" } else { "arm" },
                fetched,
                self.trace_irq_callback_prev,
                callback,
                self.trace_irq_handler_prev,
                handler,
                self.regs[13],
                self.regs[14],
            );
        }

        self.trace_irq_callback_prev = callback;
        self.trace_irq_handler_prev = handler;
    }

    fn trace_pc_probe(&self, bus: &GbaBus) {
        let pc = if self.thumb_mode() {
            self.regs[15] & !1
        } else {
            self.regs[15] & !3
        };
        let matched = if let Some(target) = trace_pc_target() {
            pc == target
        } else if let Some((range_start, range_end)) = trace_pc_range() {
            pc >= range_start && pc <= range_end
        } else {
            false
        };
        if !matched {
            return;
        }
        let seen = TRACE_PC_MATCH_COUNT.fetch_add(1, Ordering::Relaxed) + 1;
        if seen <= trace_pc_skip() {
            return;
        }
        if let Some(sp_min) = trace_pc_sp_min() {
            if self.regs[13] < sp_min {
                return;
            }
        }
        if let Some(slot) = take_trace_slot(&TRACE_PC_COUNT) {
            let instr = if self.thumb_mode() {
                u32::from(bus.read16(pc))
            } else {
                bus.read32(pc)
            };
            eprintln!(
                "[gba:trace:pc] slot={}/{} seen={} pc={:#010X} mode={} instr={:#010X} cpsr={:#010X} sp={:#010X} lr={:#010X} r0={:#010X} r1={:#010X} r2={:#010X} r3={:#010X} r4={:#010X} r5={:#010X} r6={:#010X} r7={:#010X}",
                slot + 1,
                trace_limit(),
                seen,
                pc,
                if self.thumb_mode() { "thumb" } else { "arm" },
                instr,
                self.cpsr,
                self.regs[13],
                self.regs[14],
                self.regs[0],
                self.regs[1],
                self.regs[2],
                self.regs[3],
                self.regs[4],
                self.regs[5],
                self.regs[6],
                self.regs[7],
            );
        }
    }

    fn write_psr_fields(&mut self, fields: u8, value: u32, use_spsr: bool) {
        let mut mask = 0u32;
        if (fields & 0x1) != 0 {
            mask |= 0x0000_00FF;
        }
        if (fields & 0x2) != 0 {
            mask |= 0x0000_FF00;
        }
        if (fields & 0x4) != 0 {
            mask |= 0x00FF_0000;
        }
        if (fields & 0x8) != 0 {
            mask |= 0xFF00_0000;
        }

        if use_spsr {
            if let Some(spsr) = self.current_spsr_mut() {
                *spsr = (*spsr & !mask) | (value & mask);
            }
            return;
        }

        let next = (self.cpsr & !mask) | (value & mask);
        self.set_cpsr(next);
    }

    fn set_cpsr(&mut self, next: u32) {
        let old_mode = self.cpsr & MODE_MASK;
        let new_mode = next & MODE_MASK;
        if old_mode != new_mode {
            self.save_banked_sp_lr(old_mode);
            self.load_banked_sp_lr(new_mode);
        }

        self.cpsr = next;
        if !self.thumb_mode() {
            self.regs[15] &= !3;
        } else {
            self.regs[15] &= !1;
        }
    }

    fn save_banked_sp_lr(&mut self, mode: u32) {
        match mode {
            MODE_IRQ => {
                self.r13_irq = self.regs[13];
                self.r14_irq = self.regs[14];
            }
            MODE_SVC => {
                self.r13_svc = self.regs[13];
                self.r14_svc = self.regs[14];
            }
            MODE_UND => {
                self.r13_und = self.regs[13];
                self.r14_und = self.regs[14];
            }
            _ => {
                self.r13_usr = self.regs[13];
                self.r14_usr = self.regs[14];
            }
        }
    }

    fn load_banked_sp_lr(&mut self, mode: u32) {
        match mode {
            MODE_IRQ => {
                self.regs[13] = self.r13_irq;
                self.regs[14] = self.r14_irq;
            }
            MODE_SVC => {
                self.regs[13] = self.r13_svc;
                self.regs[14] = self.r14_svc;
            }
            MODE_UND => {
                self.regs[13] = self.r13_und;
                self.regs[14] = self.r14_und;
            }
            _ => {
                self.regs[13] = self.r13_usr;
                self.regs[14] = self.r14_usr;
            }
        }
    }

    fn set_spsr_for_mode(&mut self, mode: u32, value: u32) {
        match mode {
            MODE_IRQ => self.spsr_irq = value,
            MODE_SVC => self.spsr_svc = value,
            MODE_UND => self.spsr_und = value,
            _ => {}
        }
    }

    fn set_exception_thumb_bl_upper(&mut self, mode: u32, value: Option<u32>) {
        match mode {
            MODE_IRQ => self.thumb_bl_upper_irq = value,
            MODE_SVC => self.thumb_bl_upper_svc = value,
            MODE_UND => self.thumb_bl_upper_und = value,
            _ => {}
        }
    }

    fn take_exception_thumb_bl_upper(&mut self, mode: u32) -> Option<u32> {
        match mode {
            MODE_IRQ => self.thumb_bl_upper_irq.take(),
            MODE_SVC => self.thumb_bl_upper_svc.take(),
            MODE_UND => self.thumb_bl_upper_und.take(),
            _ => None,
        }
    }

    fn current_spsr(&self) -> u32 {
        match self.cpsr & MODE_MASK {
            MODE_IRQ => self.spsr_irq,
            MODE_SVC => self.spsr_svc,
            MODE_UND => self.spsr_und,
            _ => self.cpsr,
        }
    }

    fn current_spsr_mut(&mut self) -> Option<&mut u32> {
        match self.cpsr & MODE_MASK {
            MODE_IRQ => Some(&mut self.spsr_irq),
            MODE_SVC => Some(&mut self.spsr_svc),
            MODE_UND => Some(&mut self.spsr_und),
            _ => None,
        }
    }

    fn restore_cpsr_from_spsr(&mut self) {
        let mode = self.cpsr & MODE_MASK;
        let spsr = self.current_spsr();
        self.set_cpsr(spsr);
        self.thumb_bl_upper = self.take_exception_thumb_bl_upper(mode);
    }

    fn condition_passed(&self, cond: u8) -> bool {
        let n = self.flag_n();
        let z = self.flag_z();
        let c = self.flag_c();
        let v = self.flag_v();

        match cond {
            0x0 => z,
            0x1 => !z,
            0x2 => c,
            0x3 => !c,
            0x4 => n,
            0x5 => !n,
            0x6 => v,
            0x7 => !v,
            0x8 => c && !z,
            0x9 => !c || z,
            0xA => n == v,
            0xB => n != v,
            0xC => !z && (n == v),
            0xD => z || (n != v),
            0xE => true,
            _ => false,
        }
    }

    fn thumb_condition_passed(&self, cond: u8) -> bool {
        self.condition_passed(cond)
    }

    fn thumb_mode(&self) -> bool {
        (self.cpsr & FLAG_T) != 0
    }

    fn set_thumb_mode(&mut self, thumb: bool) {
        if thumb {
            self.cpsr |= FLAG_T;
        } else {
            self.cpsr &= !FLAG_T;
        }
    }

    fn flag_n(&self) -> bool {
        (self.cpsr & FLAG_N) != 0
    }

    fn flag_z(&self) -> bool {
        (self.cpsr & FLAG_Z) != 0
    }

    fn flag_c(&self) -> bool {
        (self.cpsr & FLAG_C) != 0
    }

    fn flag_v(&self) -> bool {
        (self.cpsr & FLAG_V) != 0
    }

    fn irq_masked(&self) -> bool {
        (self.cpsr & FLAG_I) != 0
    }

    fn set_flag(&mut self, flag: u32, value: bool) {
        if value {
            self.cpsr |= flag;
        } else {
            self.cpsr &= !flag;
        }
    }

    fn set_nz(&mut self, value: u32) {
        self.set_flag(FLAG_N, (value & 0x8000_0000) != 0);
        self.set_flag(FLAG_Z, value == 0);
    }
}

#[cfg(feature = "runtime-debug-trace")]
#[cold]
#[inline(never)]
fn env_flag(name: &str) -> bool {
    let value = match std::env::var(name) {
        Ok(value) => value,
        Err(_) => return false,
    };

    let lowered = value.trim().to_ascii_lowercase();
    !(lowered.is_empty()
        || lowered == "0"
        || lowered == "false"
        || lowered == "off"
        || lowered == "no")
}

#[inline(always)]
fn trace_swi_enabled() -> bool {
    #[cfg(not(feature = "runtime-debug-trace"))]
    {
        false
    }
    #[cfg(feature = "runtime-debug-trace")]
    {
        *TRACE_SWI_ENABLED.get_or_init(|| env_flag("GBA_TRACE_SWI"))
    }
}

#[inline(always)]
fn trace_unknown_enabled() -> bool {
    #[cfg(not(feature = "runtime-debug-trace"))]
    {
        false
    }
    #[cfg(feature = "runtime-debug-trace")]
    {
        *TRACE_UNKNOWN_ENABLED.get_or_init(|| env_flag("GBA_TRACE_UNKNOWN"))
    }
}

#[inline(always)]
fn trace_branch_enabled() -> bool {
    #[cfg(not(feature = "runtime-debug-trace"))]
    {
        false
    }
    #[cfg(feature = "runtime-debug-trace")]
    {
        *TRACE_BRANCH_ENABLED.get_or_init(|| env_flag("GBA_TRACE_BRANCH"))
    }
}

#[inline(always)]
fn trace_irq_enabled() -> bool {
    #[cfg(not(feature = "runtime-debug-trace"))]
    {
        false
    }
    #[cfg(feature = "runtime-debug-trace")]
    {
        *TRACE_IRQ_ENABLED.get_or_init(|| env_flag("GBA_TRACE_IRQ"))
    }
}

#[inline(always)]
fn trace_irq_code_enabled() -> bool {
    #[cfg(not(feature = "runtime-debug-trace"))]
    {
        false
    }
    #[cfg(feature = "runtime-debug-trace")]
    {
        *TRACE_IRQ_CODE_ENABLED.get_or_init(|| env_flag("GBA_TRACE_IRQ_CODE"))
    }
}

#[inline(always)]
fn trace_bad_pc_enabled() -> bool {
    #[cfg(not(feature = "runtime-debug-trace"))]
    {
        false
    }
    #[cfg(feature = "runtime-debug-trace")]
    {
        *TRACE_BAD_PC_ENABLED.get_or_init(|| env_flag("GBA_TRACE_BAD_PC"))
    }
}

#[inline(always)]
fn trace_irq_ptr_enabled() -> bool {
    #[cfg(not(feature = "runtime-debug-trace"))]
    {
        false
    }
    #[cfg(feature = "runtime-debug-trace")]
    {
        *TRACE_IRQ_PTR_ENABLED.get_or_init(|| env_flag("GBA_TRACE_IRQ_PTR"))
    }
}

#[inline(always)]
fn trace_sp_enabled() -> bool {
    #[cfg(not(feature = "runtime-debug-trace"))]
    {
        false
    }
    #[cfg(feature = "runtime-debug-trace")]
    {
        *TRACE_SP_ENABLED.get_or_init(|| env_flag("GBA_TRACE_SP"))
    }
}

#[inline(always)]
fn trace_pc_target() -> Option<u32> {
    #[cfg(not(feature = "runtime-debug-trace"))]
    {
        None
    }
    #[cfg(feature = "runtime-debug-trace")]
    {
        *TRACE_PC_TARGET.get_or_init(|| {
            std::env::var("GBA_TRACE_PC")
                .ok()
                .and_then(|value| parse_u32_auto_radix(&value))
        })
    }
}

#[inline(always)]
fn trace_pc_range() -> Option<(u32, u32)> {
    #[cfg(not(feature = "runtime-debug-trace"))]
    {
        None
    }
    #[cfg(feature = "runtime-debug-trace")]
    {
        let start = *TRACE_PC_RANGE_START.get_or_init(|| {
            std::env::var("GBA_TRACE_PC_START")
                .ok()
                .and_then(|value| parse_u32_auto_radix(&value))
        });
        let end = *TRACE_PC_RANGE_END.get_or_init(|| {
            std::env::var("GBA_TRACE_PC_END")
                .ok()
                .and_then(|value| parse_u32_auto_radix(&value))
        });
        match (start, end) {
            (Some(start), Some(end)) if start <= end => Some((start, end)),
            _ => None,
        }
    }
}

#[inline(always)]
fn trace_pc_skip() -> u32 {
    #[cfg(not(feature = "runtime-debug-trace"))]
    {
        0
    }
    #[cfg(feature = "runtime-debug-trace")]
    {
        *TRACE_PC_SKIP.get_or_init(|| {
            std::env::var("GBA_TRACE_PC_SKIP")
                .ok()
                .and_then(|value| parse_u32_auto_radix(&value))
                .unwrap_or(0)
        })
    }
}

#[inline(always)]
fn trace_limit() -> u32 {
    #[cfg(not(feature = "runtime-debug-trace"))]
    {
        TRACE_LIMIT_DEFAULT
    }
    #[cfg(feature = "runtime-debug-trace")]
    {
        *TRACE_LIMIT.get_or_init(|| {
            std::env::var("GBA_TRACE_LIMIT")
                .ok()
                .and_then(|value| value.parse::<u32>().ok())
                .filter(|value| *value > 0)
                .unwrap_or(TRACE_LIMIT_DEFAULT)
        })
    }
}

#[inline(always)]
fn trace_pc_sp_min() -> Option<u32> {
    #[cfg(not(feature = "runtime-debug-trace"))]
    {
        None
    }
    #[cfg(feature = "runtime-debug-trace")]
    {
        *TRACE_PC_SP_MIN.get_or_init(|| {
            std::env::var("GBA_TRACE_PC_SP_MIN")
                .ok()
                .and_then(|value| parse_u32_auto_radix(&value))
        })
    }
}

#[inline(always)]
fn trace_sp_min() -> Option<u32> {
    #[cfg(not(feature = "runtime-debug-trace"))]
    {
        None
    }
    #[cfg(feature = "runtime-debug-trace")]
    {
        *TRACE_SP_MIN.get_or_init(|| {
            std::env::var("GBA_TRACE_SP_MIN")
                .ok()
                .and_then(|value| parse_u32_auto_radix(&value))
        })
    }
}

#[inline(always)]
fn trace_step_hooks() -> &'static TraceStepHooks {
    #[cfg(not(feature = "runtime-debug-trace"))]
    {
        &TRACE_STEP_HOOKS_DISABLED
    }
    #[cfg(feature = "runtime-debug-trace")]
    {
        TRACE_STEP_HOOKS.get_or_init(|| TraceStepHooks {
            pc: trace_pc_target().is_some() || trace_pc_range().is_some(),
            sp: trace_sp_enabled(),
            bad_pc: trace_bad_pc_enabled(),
            irq_ptr: trace_irq_ptr_enabled(),
        })
    }
}

#[cfg(feature = "runtime-debug-trace")]
#[cold]
#[inline(never)]
fn parse_u32_auto_radix(value: &str) -> Option<u32> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }

    if let Some(hex) = trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"))
    {
        return u32::from_str_radix(hex, 16).ok();
    }

    if let Some(bin) = trimmed
        .strip_prefix("0b")
        .or_else(|| trimmed.strip_prefix("0B"))
    {
        return u32::from_str_radix(bin, 2).ok();
    }

    trimmed.parse::<u32>().ok()
}

#[inline(always)]
fn take_trace_slot(counter: &AtomicU32) -> Option<u32> {
    #[cfg(not(feature = "runtime-debug-trace"))]
    {
        let _ = counter;
        None
    }
    #[cfg(feature = "runtime-debug-trace")]
    {
        let slot = counter.fetch_add(1, Ordering::Relaxed);
        if slot < trace_limit() {
            Some(slot)
        } else {
            None
        }
    }
}

fn is_valid_exec_addr(addr: u32) -> bool {
    matches!(
        addr,
        0x0000_0000..=0x0000_3FFF
            | 0x0200_0000..=0x02FF_FFFF
            | 0x0300_0000..=0x03FF_FFFF
            | 0x0800_0000..=0x0DFF_FFFF
    )
}

fn direct_bios_call_to_swi_number(addr: u32) -> Option<u32> {
    let aligned = addr & !1;
    if !(0x0000_0008..=0x0000_00AC).contains(&aligned) || (aligned & 0x3) != 0 {
        return None;
    }

    Some(((aligned - 0x0000_0008) >> 2) + 1)
}

fn add_with_carry(x: u32, y: u32, carry_in: bool) -> (u32, bool, bool) {
    let carry = u64::from(carry_in as u8);
    let unsigned_sum = x as u64 + y as u64 + carry;
    let result = unsigned_sum as u32;
    let carry_out = unsigned_sum > 0xFFFF_FFFF;

    let signed_sum = (x as i32 as i64) + (y as i32 as i64) + (carry as i64);
    let overflow = signed_sum < i32::MIN as i64 || signed_sum > i32::MAX as i64;

    (result, carry_out, overflow)
}

fn shift_imm_only(value: u32, shift_type: u8, amount: u8, old_carry: bool) -> (u32, bool) {
    match shift_type {
        0 => lsl(value, amount, old_carry),
        1 => {
            let amount = if amount == 0 { 32 } else { amount };
            lsr(value, amount, old_carry)
        }
        2 => {
            let amount = if amount == 0 { 32 } else { amount };
            asr(value, amount, old_carry)
        }
        _ => {
            if amount == 0 {
                let carry = value & 1 != 0;
                let result = ((old_carry as u32) << 31) | (value >> 1);
                (result, carry)
            } else {
                ror(value, amount, old_carry)
            }
        }
    }
}

fn shift_reg(value: u32, shift_type: u8, amount: u8, old_carry: bool) -> (u32, bool) {
    if amount == 0 {
        return (value, old_carry);
    }

    match shift_type {
        0 => lsl(value, amount, old_carry),
        1 => lsr(value, amount, old_carry),
        2 => asr(value, amount, old_carry),
        _ => ror(value, amount, old_carry),
    }
}

fn lsl(value: u32, amount: u8, old_carry: bool) -> (u32, bool) {
    match amount {
        0 => (value, old_carry),
        1..=31 => (value << amount, (value & (1 << (32 - amount))) != 0),
        32 => (0, (value & 1) != 0),
        _ => (0, false),
    }
}

fn lsr(value: u32, amount: u8, old_carry: bool) -> (u32, bool) {
    match amount {
        0 => (value, old_carry),
        1..=31 => (value >> amount, (value & (1 << (amount - 1))) != 0),
        32 => (0, (value & 0x8000_0000) != 0),
        _ => (0, false),
    }
}

fn asr(value: u32, amount: u8, old_carry: bool) -> (u32, bool) {
    match amount {
        0 => (value, old_carry),
        1..=31 => (
            ((value as i32) >> amount) as u32,
            (value & (1 << (amount - 1))) != 0,
        ),
        _ => {
            let carry = (value & 0x8000_0000) != 0;
            if carry { (u32::MAX, true) } else { (0, false) }
        }
    }
}

fn ror(value: u32, amount: u8, old_carry: bool) -> (u32, bool) {
    if amount == 0 {
        return (value, old_carry);
    }

    let rot = (amount as u32) & 31;
    if rot == 0 {
        (value, (value & 0x8000_0000) != 0)
    } else {
        let result = value.rotate_right(rot);
        (result, (result & 0x8000_0000) != 0)
    }
}

fn sign_extend(value: u32, bits: u8) -> i32 {
    let shift = 32 - bits;
    ((value << shift) as i32) >> shift
}

fn read_word_rotate(bus: &GbaBus, addr: u32) -> u32 {
    let aligned = addr & !3;
    let value = bus.read32(aligned);
    let rotate = (addr & 3) * 8;
    value.rotate_right(rotate)
}

fn read_halfword_rotate(bus: &GbaBus, addr: u32) -> u32 {
    let aligned = addr & !1;
    let value = bus.read16(aligned) as u32;
    if (addr & 1) != 0 {
        value.rotate_right(8)
    } else {
        value
    }
}

fn read_signed_halfword(bus: &GbaBus, addr: u32) -> u32 {
    if (addr & 1) != 0 {
        bus.read8(addr) as i8 as i32 as u32
    } else {
        bus.read16(addr) as i16 as i32 as u32
    }
}

fn write_halfword_aligned(bus: &mut GbaBus, addr: u32, value: u16) {
    bus.write16(addr & !1, value);
}

fn integer_sqrt(value: u32) -> u32 {
    let mut rem = value;
    let mut root = 0u32;
    let mut bit = 1u32 << 30;

    while bit > rem {
        bit >>= 2;
    }

    while bit != 0 {
        if rem >= root + bit {
            rem -= root + bit;
            root = (root >> 1) + bit;
        } else {
            root >>= 1;
        }
        bit >>= 2;
    }

    root
}

fn affine_matrix_from_scale_angle(sx: i32, sy: i32, theta: u16) -> (i16, i16, i16, i16) {
    let angle = (theta as f64) * (std::f64::consts::TAU / 65536.0);
    let sin = angle.sin();
    let cos = angle.cos();

    let pa = (cos * sx as f64).round() as i32;
    let pb = (-sin * sx as f64).round() as i32;
    let pc = (sin * sy as f64).round() as i32;
    let pd = (cos * sy as f64).round() as i32;

    (
        pa.clamp(i16::MIN as i32, i16::MAX as i32) as i16,
        pb.clamp(i16::MIN as i32, i16::MAX as i32) as i16,
        pc.clamp(i16::MIN as i32, i16::MAX as i32) as i16,
        pd.clamp(i16::MIN as i32, i16::MAX as i32) as i16,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn thumb_ls_reg(op: u16, ro: u16, rb: u16, rd: u16) -> u16 {
        0x5000 | ((op & 0x7) << 9) | ((ro & 0x7) << 6) | ((rb & 0x7) << 3) | (rd & 0x7)
    }

    fn setup_cpu_with_program(words: &[u32]) -> (Arm7Tdmi, GbaBus) {
        let mut rom = vec![0; 0x200];
        for (i, word) in words.iter().enumerate() {
            let offset = i * 4;
            rom[offset] = (*word & 0xFF) as u8;
            rom[offset + 1] = ((*word >> 8) & 0xFF) as u8;
            rom[offset + 2] = ((*word >> 16) & 0xFF) as u8;
            rom[offset + 3] = ((*word >> 24) & 0xFF) as u8;
        }

        let mut bus = GbaBus::default();
        bus.load_rom(&rom);
        bus.reset();

        let mut cpu = Arm7Tdmi::default();
        cpu.reset();
        (cpu, bus)
    }

    #[test]
    fn no_bios_direct_call_invokes_hle_and_returns_to_lr() {
        let (mut cpu, mut bus) = setup_cpu_with_program(&[0xE1A0_0000]); // NOP filler
        bus.write8(0x0300_0010, 0xAA);

        // BIOS direct call: RegisterRamReset (entry 0x00000008, SWI 0x01)
        cpu.regs[15] = 0x0000_0008;
        cpu.regs[14] = 0x0800_0011; // return in THUMB
        cpu.regs[0] = 0x02; // clear IWRAM

        let cycles = cpu.step(&mut bus);
        assert_eq!(cycles, 3);
        assert_eq!(bus.read8(0x0300_0010), 0x00);
        assert_eq!(cpu.regs[15], 0x0800_0010);
        assert!(cpu.thumb_mode());
    }

    #[test]
    fn no_bios_unknown_low_address_is_not_auto_returned() {
        let (mut cpu, mut bus) = setup_cpu_with_program(&[0xE1A0_0000]); // NOP filler

        cpu.regs[15] = 0x0000_0004; // not a known BIOS direct-call entry
        cpu.regs[14] = 0x0800_1235;

        let cycles = cpu.step(&mut bus);
        assert_eq!(cycles, 1);
        // Execute memory at 0x00000004 (0x00000000), not a synthetic BX LR return.
        assert_eq!(cpu.regs[15], 0x0000_0008);
    }

    #[test]
    fn no_bios_zero_address_is_not_treated_as_soft_reset_entry() {
        let (mut cpu, mut bus) = setup_cpu_with_program(&[0xE1A0_0000]); // NOP filler
        cpu.regs[15] = 0x0000_0000;
        cpu.regs[14] = 0x0800_1235;

        let cycles = cpu.step(&mut bus);
        assert_eq!(cycles, 1);
        // Execute BIOS area contents (0x00000000), do not force SWI 0x00.
        assert_eq!(cpu.regs[15], 0x0000_0004);
    }

    #[test]
    fn no_bios_direct_call_handles_sound_get_jump_list_entry() {
        let (mut cpu, mut bus) = setup_cpu_with_program(&[0xE1A0_0000]); // NOP filler

        // BIOS direct call: SoundGetJumpList (entry 0x000000AC, SWI 0x2A)
        cpu.regs[15] = 0x0000_00AC;
        cpu.regs[14] = 0x0800_0011; // return in THUMB

        let cycles = cpu.step(&mut bus);
        assert_eq!(cycles, 3);
        assert_eq!(cpu.regs[15], 0x0800_0010);
        assert!(cpu.thumb_mode());
        assert_eq!(cpu.regs[0], SOUND_JUMP_LIST_BASE);
    }

    #[test]
    fn swi_sound_get_jump_list_populates_synthetic_table() {
        let (mut cpu, mut bus) = setup_cpu_with_program(&[0xE1A0_0000]);

        cpu.handle_swi(0x2A, &mut bus);
        assert_eq!(cpu.regs[0], SOUND_JUMP_LIST_BASE);

        // SWI 0x1A -> direct-call entry 0x0000006C
        assert_eq!(bus.read32(SOUND_JUMP_LIST_BASE), 0x0000_006C);
        // SWI 0x2A -> direct-call entry 0x000000AC
        let last = (SOUND_JUMP_LIST_END_SWI - SOUND_JUMP_LIST_START_SWI) * 4;
        assert_eq!(bus.read32(SOUND_JUMP_LIST_BASE + last), 0x0000_00AC);
        // Remaining entries are safe fallbacks to SoundDriverMain.
        assert_eq!(
            bus.read32(SOUND_JUMP_LIST_BASE + SOUND_JUMP_LIST_BYTES - 4),
            0x0000_0074
        );
    }

    #[test]
    fn swi_sound_get_jump_list_uses_callers_destination_when_provided() {
        let (mut cpu, mut bus) = setup_cpu_with_program(&[0xE1A0_0000]);
        cpu.regs[0] = 0x0200_1002; // unaligned on purpose

        cpu.handle_swi(0x2A, &mut bus);

        assert_eq!(cpu.regs[0], 0x0200_1000);
        assert_eq!(bus.read32(0x0200_1000), 0x0000_006C);
    }

    #[test]
    fn swi_sound_driver_init_enables_master_sound_and_clears_fifos() {
        let (mut cpu, mut bus) = setup_cpu_with_program(&[0xE1A0_0000]);
        bus.write16(REG_SOUNDCNT_X, 0);
        bus.write16(REG_SOUNDBIAS, 0);
        bus.write32(FIFO_A_ADDR, 0xDEAD_BEEF);
        bus.write32(FIFO_B_ADDR, 0xCAFE_BABE);

        cpu.handle_swi(0x1A, &mut bus);

        assert_ne!(bus.read16(REG_SOUNDCNT_X) & 0x0080, 0);
        assert_eq!(bus.read16(REG_SOUNDBIAS), 0x0200);
        assert_eq!(bus.read32(FIFO_A_ADDR), 0);
        assert_eq!(bus.read32(FIFO_B_ADDR), 0);
    }

    #[test]
    fn swi_sound_bias_writes_soundbias_register() {
        let (mut cpu, mut bus) = setup_cpu_with_program(&[0xE1A0_0000]);
        cpu.regs[0] = 0x1234;

        cpu.handle_swi(0x19, &mut bus);

        assert_eq!(bus.read16(REG_SOUNDBIAS), 0x1234);
    }

    #[test]
    fn swi_sound_driver_mode_sets_shadow_mode_and_keeps_master_enable() {
        let (mut cpu, mut bus) = setup_cpu_with_program(&[0xE1A0_0000]);
        bus.write16(REG_SOUNDCNT_X, 0);
        cpu.regs[0] = 0xA5A5_0001;

        cpu.handle_swi(0x1B, &mut bus);

        assert!(cpu.sound_bios.initialized);
        assert_eq!(cpu.sound_bios.mode, 0xA5A5_0001);
        assert_ne!(bus.read16(REG_SOUNDCNT_X) & 0x0080, 0);
    }

    #[test]
    fn swi_sound_driver_vsync_obeys_off_and_on_switches() {
        let (mut cpu, mut bus) = setup_cpu_with_program(&[0xE1A0_0000]);
        cpu.handle_swi(0x1A, &mut bus);
        assert_eq!(cpu.sound_bios.vsync_ticks, 0);

        cpu.handle_swi(0x28, &mut bus); // VSync off
        cpu.handle_swi(0x1D, &mut bus); // VSync callback
        assert_eq!(cpu.sound_bios.vsync_ticks, 0);

        cpu.handle_swi(0x29, &mut bus); // VSync on
        cpu.handle_swi(0x1D, &mut bus);
        assert_eq!(cpu.sound_bios.vsync_ticks, 1);
    }

    #[test]
    fn swi_music_player_shadow_state_tracks_open_start_fade_stop() {
        let (mut cpu, mut bus) = setup_cpu_with_program(&[0xE1A0_0000]);

        cpu.regs[0] = 0x0300_1000;
        cpu.regs[1] = 0x0801_0000;
        cpu.handle_swi(0x20, &mut bus); // open
        assert_eq!(cpu.sound_bios.player.player_ptr, 0x0300_1000);
        assert_eq!(cpu.sound_bios.player.song_ptr, 0x0801_0000);
        assert!(!cpu.sound_bios.player.playing);

        cpu.handle_swi(0x21, &mut bus); // start
        assert!(cpu.sound_bios.player.playing);
        assert_eq!(cpu.sound_bios.player.fadeout_frames, 0);

        cpu.regs[1] = 3;
        cpu.handle_swi(0x24, &mut bus); // fade out
        assert_eq!(cpu.sound_bios.player.fadeout_frames, 3);

        cpu.handle_swi(0x1C, &mut bus);
        cpu.handle_swi(0x1C, &mut bus);
        cpu.handle_swi(0x1C, &mut bus);
        assert_eq!(cpu.sound_bios.player.fadeout_frames, 0);
        assert!(!cpu.sound_bios.player.playing);

        cpu.handle_swi(0x23, &mut bus); // continue
        assert!(cpu.sound_bios.player.playing);
        cpu.handle_swi(0x22, &mut bus); // stop
        assert!(!cpu.sound_bios.player.playing);
    }

    #[test]
    fn arm_add_and_store_to_iwram() {
        let (mut cpu, mut bus) = setup_cpu_with_program(&[
            0xE3A00012, // MOV r0,#0x12
            0xE2801008, // ADD r1,r0,#8
            0xE5821000, // STR r1,[r2]
        ]);
        cpu.regs[2] = 0x0300_0000;

        for _ in 0..3 {
            cpu.step(&mut bus);
        }
        assert_eq!(bus.read32(0x0300_0000), 0x1A);
    }

    #[test]
    fn arm_branch_and_link_sets_lr() {
        let (mut cpu, mut bus) = setup_cpu_with_program(&[
            0xEB000001, // BL +1 instruction
            0xE3A00001, // MOV r0,#1 (skipped)
            0xE3A00077, // MOV r0,#0x77
        ]);

        cpu.step(&mut bus);
        assert_eq!(cpu.regs[14], 0x0800_0004);
        assert_eq!(cpu.regs[15], 0x0800_000C);
    }

    #[test]
    fn arm_bx_switches_to_thumb() {
        let (mut cpu, mut bus) = setup_cpu_with_program(&[
            0xE12FFF11, // BX r1
        ]);
        cpu.regs[1] = 0x0800_0011;
        cpu.step(&mut bus);
        assert!(cpu.thumb_mode());
        assert_eq!(cpu.regs[15], 0x0800_0010);
    }

    #[test]
    fn thumb_mov_add_cmp_flow() {
        let mut rom = vec![0; 0x200];
        // thumb code at 0x20: MOV r0,#3; ADD r0,#5; CMP r0,#8
        let code: [u16; 3] = [0x2003, 0x3005, 0x2808];
        for (i, half) in code.iter().enumerate() {
            let off = 0x20 + i * 2;
            rom[off] = (*half & 0xFF) as u8;
            rom[off + 1] = ((*half >> 8) & 0xFF) as u8;
        }

        let mut bus = GbaBus::default();
        bus.load_rom(&rom);
        bus.reset();

        let mut cpu = Arm7Tdmi::default();
        cpu.reset();
        cpu.set_thumb_mode(true);
        cpu.regs[15] = 0x0800_0020;

        cpu.step(&mut bus);
        cpu.step(&mut bus);
        cpu.step(&mut bus);

        assert_eq!(cpu.regs[0], 8);
        assert!(cpu.flag_z());
    }

    #[test]
    fn add_with_carry_subtract_zero_does_not_set_overflow() {
        let (result, carry, overflow) = add_with_carry(0x0000_0014, !0, true);
        assert_eq!(result, 0x0000_0014);
        assert!(carry);
        assert!(!overflow);
    }

    #[test]
    fn thumb_blt_is_not_taken_for_positive_cmp_result() {
        let mut rom = vec![0; 0x200];
        // THUMB at 0x20:
        //   MOV  r0, #0
        //   MOV  r6, #0x14
        //   CMP  r6, r0
        //   BLT  +2 (should not branch)
        //   MOV  r1, #1
        //   MOV  r2, #2
        let code: [u16; 6] = [0x2000, 0x2614, 0x4286, 0xDB01, 0x2101, 0x2202];
        for (i, half) in code.iter().enumerate() {
            let off = 0x20 + i * 2;
            rom[off] = (*half & 0xFF) as u8;
            rom[off + 1] = ((*half >> 8) & 0xFF) as u8;
        }

        let mut bus = GbaBus::default();
        bus.load_rom(&rom);
        bus.reset();

        let mut cpu = Arm7Tdmi::default();
        cpu.reset();
        cpu.set_thumb_mode(true);
        cpu.regs[15] = 0x0800_0020;

        for _ in 0..6 {
            cpu.step(&mut bus);
        }

        assert_eq!(cpu.regs[1], 1);
        assert_eq!(cpu.regs[2], 2);
    }

    #[test]
    fn thumb_register_offset_word_and_byte_ops_decode_correctly() {
        let mut rom = vec![0; 0x200];
        let code: [u16; 4] = [
            thumb_ls_reg(0, 1, 0, 2), // STR  r2,[r0,r1]
            thumb_ls_reg(4, 1, 0, 3), // LDR  r3,[r0,r1]
            thumb_ls_reg(2, 1, 0, 4), // STRB r4,[r0,r1]
            thumb_ls_reg(6, 1, 0, 5), // LDRB r5,[r0,r1]
        ];
        for (i, half) in code.iter().enumerate() {
            let off = 0x20 + i * 2;
            rom[off] = (*half & 0xFF) as u8;
            rom[off + 1] = ((*half >> 8) & 0xFF) as u8;
        }

        let mut bus = GbaBus::default();
        bus.load_rom(&rom);
        bus.reset();

        let mut cpu = Arm7Tdmi::default();
        cpu.reset();
        cpu.set_thumb_mode(true);
        cpu.regs[15] = 0x0800_0020;
        cpu.regs[0] = 0x0300_0100;
        cpu.regs[1] = 4;
        cpu.regs[2] = 0x1122_3344;
        cpu.regs[4] = 0xAA;

        for _ in 0..4 {
            cpu.step(&mut bus);
        }

        assert_eq!(bus.read32(0x0300_0104), 0x1122_33AA);
        assert_eq!(cpu.regs[3], 0x1122_3344);
        assert_eq!(cpu.regs[5], 0xAA);
    }

    #[test]
    fn thumb_register_offset_halfword_and_signed_ops_decode_correctly() {
        let mut rom = vec![0; 0x200];
        let code: [u16; 4] = [
            thumb_ls_reg(1, 1, 0, 2), // STRH  r2,[r0,r1]
            thumb_ls_reg(3, 1, 0, 3), // LDRSB r3,[r0,r1]
            thumb_ls_reg(5, 1, 0, 4), // LDRH  r4,[r0,r1]
            thumb_ls_reg(7, 1, 0, 5), // LDRSH r5,[r0,r1]
        ];
        for (i, half) in code.iter().enumerate() {
            let off = 0x20 + i * 2;
            rom[off] = (*half & 0xFF) as u8;
            rom[off + 1] = ((*half >> 8) & 0xFF) as u8;
        }

        let mut bus = GbaBus::default();
        bus.load_rom(&rom);
        bus.reset();

        let mut cpu = Arm7Tdmi::default();
        cpu.reset();
        cpu.set_thumb_mode(true);
        cpu.regs[15] = 0x0800_0020;
        cpu.regs[0] = 0x0300_0200;
        cpu.regs[1] = 1; // odd address
        cpu.regs[2] = 0x0000_80F1;

        for _ in 0..4 {
            cpu.step(&mut bus);
        }

        assert_eq!(bus.read16(0x0300_0200), 0x80F1);
        assert_eq!(cpu.regs[3], 0xFFFF_FF80); // LDRSB from odd byte
        assert_eq!(cpu.regs[4], 0xF100_0080); // odd LDRH rotate
        assert_eq!(cpu.regs[5], 0xFFFF_FF80); // odd LDRSH behaves like LDRSB
    }

    #[test]
    fn swi_div_sets_expected_registers() {
        let (mut cpu, mut bus) = setup_cpu_with_program(&[0xEF000006]); // SWI 0x06
        cpu.regs[0] = 20;
        cpu.regs[1] = 3;
        cpu.step(&mut bus);

        assert_eq!(cpu.regs[0], 6);
        assert_eq!(cpu.regs[1], 2);
        assert_eq!(cpu.regs[3], 6);
    }

    #[test]
    fn arm_swi_with_bios_enters_svc_vector() {
        let (mut cpu, mut bus) = setup_cpu_with_program(&[0xEF000006]); // SWI 0x06
        bus.load_bios(&[0; 16 * 1024]);

        let old_cpsr = cpu.cpsr;
        let cycles = cpu.step(&mut bus);

        assert_eq!(cycles, 4);
        assert_eq!(cpu.regs[15], 0x0000_0008);
        assert_eq!(cpu.regs[14], 0x0800_0004);
        assert_eq!(cpu.cpsr & MODE_MASK, MODE_SVC);
        assert!(cpu.irq_masked());
        assert!(!cpu.thumb_mode());
        assert_eq!(cpu.spsr_svc, old_cpsr);
    }

    #[test]
    fn thumb_swi_with_bios_enters_svc_vector() {
        let mut rom = vec![0; 0x200];
        let off = 0x20usize;
        let swi: u16 = 0xDF06;
        rom[off] = (swi & 0x00FF) as u8;
        rom[off + 1] = (swi >> 8) as u8;

        let mut bus = GbaBus::default();
        bus.load_rom(&rom);
        bus.load_bios(&[0; 16 * 1024]);
        bus.reset();

        let mut cpu = Arm7Tdmi::default();
        cpu.reset();
        cpu.set_thumb_mode(true);
        cpu.regs[15] = 0x0800_0020;

        let old_cpsr = cpu.cpsr;
        let cycles = cpu.step(&mut bus);

        assert_eq!(cycles, 4);
        assert_eq!(cpu.regs[15], 0x0000_0008);
        assert_eq!(cpu.regs[14], 0x0800_0022);
        assert_eq!(cpu.cpsr & MODE_MASK, MODE_SVC);
        assert!(cpu.irq_masked());
        assert!(!cpu.thumb_mode());
        assert_eq!(cpu.spsr_svc, old_cpsr);
    }

    #[test]
    fn arm_unknown_opcode_with_bios_enters_undefined_vector() {
        let (mut cpu, mut bus) = setup_cpu_with_program(&[0xEE00_0010]); // coprocessor opcode
        bus.load_bios(&[0; 16 * 1024]);

        let old_cpsr = cpu.cpsr;
        let cycles = cpu.step(&mut bus);

        assert_eq!(cycles, 4);
        assert_eq!(cpu.regs[15], 0x0000_0004);
        assert_eq!(cpu.regs[14], 0x0800_0004);
        assert_eq!(cpu.cpsr & MODE_MASK, MODE_UND);
        assert!(cpu.irq_masked());
        assert!(!cpu.thumb_mode());
        assert_eq!(cpu.spsr_und, old_cpsr);
    }

    #[test]
    fn thumb_cond_e_is_undefined_with_bios() {
        let mut rom = vec![0; 0x200];
        let off = 0x20usize;
        let undef: u16 = 0xDE00; // reserved in THUMB (cond=0xE)
        rom[off] = (undef & 0x00FF) as u8;
        rom[off + 1] = (undef >> 8) as u8;

        let mut bus = GbaBus::default();
        bus.load_rom(&rom);
        bus.load_bios(&[0; 16 * 1024]);
        bus.reset();

        let mut cpu = Arm7Tdmi::default();
        cpu.reset();
        cpu.set_thumb_mode(true);
        cpu.regs[15] = 0x0800_0020;

        let old_cpsr = cpu.cpsr;
        let cycles = cpu.step(&mut bus);

        assert_eq!(cycles, 4);
        assert_eq!(cpu.regs[15], 0x0000_0004);
        assert_eq!(cpu.regs[14], 0x0800_0022);
        assert_eq!(cpu.cpsr & MODE_MASK, MODE_UND);
        assert!(cpu.irq_masked());
        assert!(!cpu.thumb_mode());
        assert_eq!(cpu.spsr_und, old_cpsr);
    }

    #[test]
    fn swi_cpuset_copies_words() {
        let (mut cpu, mut bus) = setup_cpu_with_program(&[0xEF00000B]); // SWI 0x0B
        bus.write32(0x0300_0000, 0x1122_3344);
        bus.write32(0x0300_0004, 0x5566_7788);
        cpu.regs[0] = 0x0300_0000;
        cpu.regs[1] = 0x0200_0000;
        cpu.regs[2] = (1 << 26) | 2; // word copy, count=2

        cpu.step(&mut bus);
        assert_eq!(bus.read32(0x0200_0000), 0x1122_3344);
        assert_eq!(bus.read32(0x0200_0004), 0x5566_7788);
    }

    #[test]
    fn swi_cpufastset_truncates_length_to_8_word_blocks() {
        let (mut cpu, mut bus) = setup_cpu_with_program(&[0xEF00000C]); // SWI 0x0C
        let src: u32 = 0x0300_0000;
        let dst: u32 = 0x0200_0000;
        for i in 0..16u32 {
            bus.write32(src.wrapping_add(i * 4), 0xA000_0000 | i);
        }

        cpu.regs[0] = src;
        cpu.regs[1] = dst;
        cpu.regs[2] = 9; // 9 words -> truncated to 8 words

        cpu.step(&mut bus);

        for i in 0..8u32 {
            assert_eq!(bus.read32(dst.wrapping_add(i * 4)), 0xA000_0000 | i);
        }
        assert_eq!(bus.read32(dst.wrapping_add(8 * 4)), 0);
    }

    #[test]
    fn swi_cpufastset_fixed_fill_repeats_first_word() {
        let (mut cpu, mut bus) = setup_cpu_with_program(&[0xEF00000C]); // SWI 0x0C
        let src: u32 = 0x0300_0100;
        let dst: u32 = 0x0200_0100;
        bus.write32(src, 0xDEAD_BEEF);
        for i in 0..8u32 {
            bus.write32(dst.wrapping_add(i * 4), 0);
        }

        cpu.regs[0] = src;
        cpu.regs[1] = dst;
        cpu.regs[2] = (1 << 24) | 15; // fixed fill, 15 words -> truncated to 8 words

        cpu.step(&mut bus);

        for i in 0..8u32 {
            assert_eq!(bus.read32(dst.wrapping_add(i * 4)), 0xDEAD_BEEF);
        }
        assert_eq!(bus.read32(dst.wrapping_add(8 * 4)), 0);
    }

    #[test]
    fn arm_halfword_and_signed_loads_work() {
        let (mut cpu, mut bus) = setup_cpu_with_program(&[
            0xE1C210B0, // STRH r1,[r2]
            0xE1D230B0, // LDRH r3,[r2]
            0xE1D240D0, // LDRSB r4,[r2]
            0xE1D250F0, // LDRSH r5,[r2]
        ]);
        cpu.regs[1] = 0x0000_FF80;
        cpu.regs[2] = 0x0300_0010;

        for _ in 0..4 {
            cpu.step(&mut bus);
        }

        assert_eq!(cpu.regs[3], 0x0000_FF80);
        assert_eq!(cpu.regs[4], 0xFFFF_FF80);
        assert_eq!(cpu.regs[5], 0xFFFF_FF80);
    }

    #[test]
    fn arm_halfword_loads_handle_odd_addresses_like_arm7() {
        let (mut cpu, mut bus) = setup_cpu_with_program(&[
            0xE1D230B0, // LDRH r3,[r2]
            0xE1D250F0, // LDRSH r5,[r2]
            0xE1C210B0, // STRH r1,[r2]
        ]);

        bus.write16(0x0300_0000, 0x1122);
        bus.write8(0x0300_0005, 0x80);
        cpu.regs[1] = 0x0000_A1B2;
        cpu.regs[2] = 0x0300_0001; // odd address for LDRH

        cpu.step(&mut bus); // LDRH from 0x0300_0001
        cpu.regs[2] = 0x0300_0005; // odd address for LDRSH/STRH
        cpu.step(&mut bus); // LDRSH
        cpu.step(&mut bus); // STRH

        assert_eq!(cpu.regs[3], 0x2200_0011); // odd LDRH rotates by 8 within 32-bit word
        assert_eq!(cpu.regs[5], 0xFFFF_FF80); // odd LDRSH behaves like LDRSB
        assert_eq!(bus.read16(0x0300_0004), 0xA1B2); // STRH aligns to halfword
    }

    #[test]
    fn arm_swp_exchanges_value() {
        let (mut cpu, mut bus) = setup_cpu_with_program(&[
            0xE1021093, // SWP r1,r3,[r2]
        ]);
        cpu.regs[2] = 0x0300_0020;
        cpu.regs[3] = 0xAABB_CCDD;
        bus.write32(0x0300_0020, 0x1234_5678);

        cpu.step(&mut bus);

        assert_eq!(cpu.regs[1], 0x1234_5678);
        assert_eq!(bus.read32(0x0300_0020), 0xAABB_CCDD);
    }

    #[test]
    fn arm_umull_writes_64bit_result() {
        let (mut cpu, mut bus) = setup_cpu_with_program(&[
            0xE0810392, // UMULL r0,r1,r2,r3
        ]);
        cpu.regs[2] = 0xFFFF_0000;
        cpu.regs[3] = 2;

        cpu.step(&mut bus);
        assert_eq!(cpu.regs[0], 0xFFFE_0000);
        assert_eq!(cpu.regs[1], 0x0000_0001);
    }

    #[test]
    fn arm_smull_handles_signed_operands() {
        let (mut cpu, mut bus) = setup_cpu_with_program(&[
            0xE0C10392, // SMULL r0,r1,r2,r3
        ]);
        cpu.regs[2] = 0xFFFF_FFFE; // -2
        cpu.regs[3] = 3;

        cpu.step(&mut bus);
        assert_eq!(cpu.regs[0], 0xFFFF_FFFA);
        assert_eq!(cpu.regs[1], 0xFFFF_FFFF);
    }

    #[test]
    fn swi_sqrt_returns_integer_sqrt() {
        let (mut cpu, mut bus) = setup_cpu_with_program(&[0xEF000008]); // SWI 0x08
        cpu.regs[0] = 81;
        cpu.step(&mut bus);
        assert_eq!(cpu.regs[0], 9);
    }

    #[test]
    fn swi_arctan_and_arctan2_return_expected_angles() {
        let (mut cpu, mut bus) = setup_cpu_with_program(&[
            0xEF000009, // SWI 0x09 ArcTan
            0xEF00000A, // SWI 0x0A ArcTan2
        ]);

        cpu.regs[0] = 16_384; // tan(45deg) in 2.14 fixed-point
        cpu.step(&mut bus);
        assert_eq!(cpu.regs[0] & 0xFFFF, 0x2000);

        cpu.regs[0] = 1; // y
        cpu.regs[1] = 0; // x
        cpu.step(&mut bus);
        assert_eq!(cpu.regs[0] & 0xFFFF, 0x4000);
    }

    #[test]
    fn swi_midi_key2freq_scales_with_pitch() {
        let (mut cpu, mut bus) = setup_cpu_with_program(&[
            0xEF00001F, // SWI 0x1F MidiKey2Freq
            0xEF00001F, // SWI 0x1F MidiKey2Freq
        ]);
        let wave: u32 = 0x0300_0A00;
        bus.write32(wave + 4, 44_000); // base frequency

        cpu.regs[0] = wave;
        cpu.regs[1] = 180;
        cpu.regs[2] = 0;
        cpu.step(&mut bus);
        let base = cpu.regs[0];

        cpu.regs[0] = wave;
        cpu.regs[1] = 192; // +12 semitones => lower timer value
        cpu.regs[2] = 0;
        cpu.step(&mut bus);
        let up_octave = cpu.regs[0];

        assert!(base > 0);
        assert!(up_octave > 0);
        assert!(up_octave < base);
    }

    #[test]
    fn swi_get_bios_checksum_returns_constant() {
        let (mut cpu, mut bus) = setup_cpu_with_program(&[0xEF00000D]); // SWI 0x0D
        cpu.step(&mut bus);
        assert_eq!(cpu.regs[0], 0xBAAE_187F);
    }

    #[test]
    fn intr_wait_halts_until_irq_pending() {
        let (mut cpu, mut bus) = setup_cpu_with_program(&[
            0xEF000004, // SWI 0x04 IntrWait
            0xE3A00042, // MOV r0,#0x42
        ]);
        // r0=0 don't clear IF, r1=VBlank
        cpu.regs[1] = 1;
        // Enable VBlank interrupt line and IME.
        bus.write16(0x0400_0200, 1);
        bus.write16(0x0400_0208, 1);

        cpu.step(&mut bus);
        assert!(cpu.halted);
        // CPU should remain halted until IF bit appears.
        cpu.step(&mut bus);
        assert!(cpu.halted);

        bus.request_irq(1);
        cpu.step(&mut bus);
        assert!(!cpu.halted);
        assert_eq!(bus.pending_interrupts(), 0);
        cpu.step(&mut bus);
        assert_eq!(cpu.regs[0], 0x42);
    }

    #[test]
    fn vblank_intr_wait_ignores_caller_mask_and_waits_for_vblank() {
        let (mut cpu, mut bus) = setup_cpu_with_program(&[
            0xEF000005, // SWI 0x05 VBlankIntrWait
            0xE3A00042, // MOV r0,#0x42
        ]);
        // Deliberately pass a non-vblank mask; SWI 0x05 should ignore it.
        cpu.regs[0] = 0;
        cpu.regs[1] = 1 << 1; // HBlank only (must be ignored)

        // Enable only VBlank interrupt line and IME.
        bus.write16(0x0400_0200, 1);
        bus.write16(0x0400_0208, 1);

        cpu.step(&mut bus);
        assert!(cpu.halted);

        // HBlank only must not wake SWI 0x05.
        bus.request_irq(1 << 1);
        cpu.step(&mut bus);
        assert!(cpu.halted);

        // VBlank wakes it.
        bus.request_irq(1);
        cpu.step(&mut bus);
        assert!(!cpu.halted);
        assert_eq!(bus.pending_interrupts() & 1, 0);
        cpu.step(&mut bus);
        assert_eq!(cpu.regs[0], 0x42);
    }

    #[test]
    fn halt_wake_keeps_if_and_enters_irq_when_ime_is_enabled() {
        let (mut cpu, mut bus) = setup_cpu_with_program(&[
            0xEF000002, // SWI 0x02 Halt
            0xE1A00000, // NOP
        ]);
        bus.write32(0x0300_7FFC, 0x0800_0101); // THUMB IRQ handler pointer
        bus.write16(0x0400_0200, 1); // IE: VBlank
        bus.write16(0x0400_0208, 1); // IME: on

        cpu.step(&mut bus);
        assert!(cpu.halted);

        bus.request_irq(1); // IF: VBlank
        let cycles = cpu.step(&mut bus);

        assert_eq!(cycles, 3);
        assert!(!cpu.halted);
        assert_eq!(cpu.regs[15], 0x0800_0100);
        assert!(cpu.thumb_mode());
        assert_eq!(bus.pending_interrupts(), 1);
    }

    #[test]
    fn custom_halt_uses_irq_mask_argument() {
        let (mut cpu, mut bus) = setup_cpu_with_program(&[
            0xEF000027, // SWI 0x27 CustomHalt
            0xE1A00000, // NOP
        ]);
        cpu.regs[0] = 1; // wake only on VBlank
        bus.write16(0x0400_0200, 1); // IE: VBlank
        bus.write16(0x0400_0208, 1); // IME: on

        cpu.step(&mut bus);
        assert!(cpu.halted);

        bus.request_irq(1 << 1); // HBlank (masked out)
        cpu.step(&mut bus);
        assert!(cpu.halted);

        bus.request_irq(1); // VBlank
        let cycles = cpu.step(&mut bus);
        assert_eq!(cycles, 3);
        assert!(!cpu.halted);
    }

    #[test]
    fn stop_waits_for_keypad_or_gamepak_irq() {
        let (mut cpu, mut bus) = setup_cpu_with_program(&[
            0xEF000003, // SWI 0x03 Stop
            0xE3A00042, // MOV r0,#0x42
        ]);
        bus.write16(0x0400_0200, 1 | IRQ_KEYPAD); // IE: VBlank + Keypad
        bus.write16(0x0400_0208, 0); // IME: off, so wake doesn't vector to IRQ handler

        cpu.step(&mut bus);
        assert!(cpu.halted);
        assert_eq!(cpu.halt_irq_mask, STOP_WAKE_IRQ_MASK);

        bus.request_irq(1); // VBlank should not wake STOP
        cpu.step(&mut bus);
        assert!(cpu.halted);

        bus.request_irq(IRQ_KEYPAD); // Keypad wakes STOP
        cpu.step(&mut bus);
        assert!(!cpu.halted);
        cpu.step(&mut bus);
        assert_eq!(cpu.regs[0], 0x42);
    }

    #[test]
    fn hard_reset_swi_resets_bus_and_cpu_state() {
        let (mut cpu, mut bus) = setup_cpu_with_program(&[
            0xEF000026, // SWI 0x26 HardReset
            0xE3A00042, // MOV r0,#0x42
        ]);
        bus.write8(0x0200_0010, 0xAA);
        bus.write16(0x0400_0200, 0xFFFF); // IE
        bus.write16(REG_SOUNDBIAS, 0x1234);
        cpu.regs[0] = 0xDEAD_BEEF;

        cpu.step(&mut bus);

        assert_eq!(cpu.regs[15], 0x0800_0000);
        assert_eq!(cpu.regs[13], 0x0300_7F00);
        assert_eq!(cpu.regs[0], 0);
        assert_eq!(bus.read8(0x0200_0010), 0);
        assert_eq!(bus.read16(0x0400_0200), 0);
        assert_eq!(bus.read16(0x0400_0130), 0x03FF);
        assert_eq!(bus.read8(0x0400_0300), 1); // POSTFLG in no-BIOS mode
        assert_eq!(bus.read16(REG_SOUNDBIAS), 0x0200);
    }

    #[test]
    fn multiboot_swi_returns_success() {
        let (mut cpu, mut bus) = setup_cpu_with_program(&[0xEF000025]); // SWI 0x25
        cpu.regs[0] = u32::MAX;
        cpu.step(&mut bus);
        assert_eq!(cpu.regs[0], 0);
    }

    #[test]
    fn swi_lz77_uncomp_decodes_literals() {
        let (mut cpu, mut bus) = setup_cpu_with_program(&[0xEF000011]); // SWI 0x11
        let src: u32 = 0x0300_0000;
        let dst: u32 = 0x0200_0000;
        let payload = [
            0x10u8, 0x04, 0x00, 0x00, // header: type=0x10, size=4
            0x00, // 8 literal flags
            b'A', b'B', b'C', b'D',
        ];
        for (i, byte) in payload.iter().enumerate() {
            bus.write8(src.wrapping_add(i as u32), *byte);
        }

        cpu.regs[0] = src;
        cpu.regs[1] = dst;
        cpu.step(&mut bus);

        assert_eq!(bus.read8(dst), b'A');
        assert_eq!(bus.read8(dst + 1), b'B');
        assert_eq!(bus.read8(dst + 2), b'C');
        assert_eq!(bus.read8(dst + 3), b'D');
    }

    #[test]
    fn swi_lz77_uncomp_vram_packs_odd_byte_tail_into_halfword() {
        let (mut cpu, mut bus) = setup_cpu_with_program(&[0xEF000012]); // SWI 0x12
        let src: u32 = 0x0300_0700;
        let dst: u32 = 0x0600_0000;
        let payload = [
            0x10u8, 0x03, 0x00, 0x00, // header: type=0x10, size=3
            0x00, // 8 literal flags
            b'X', b'Y', b'Z',
        ];
        for (i, byte) in payload.iter().enumerate() {
            bus.write8(src.wrapping_add(i as u32), *byte);
        }

        cpu.regs[0] = src;
        cpu.regs[1] = dst;
        cpu.step(&mut bus);

        assert_eq!(bus.read16(dst), 0x5958);
        assert_eq!(bus.read16(dst + 2), 0x005A);
    }

    #[test]
    fn swi_huff_uncomp_accepts_0x2n_header_and_decodes_symbols() {
        let (mut cpu, mut bus) = setup_cpu_with_program(&[0xEF000013]); // SWI 0x13
        let src: u32 = 0x0300_0900;
        let dst: u32 = 0x0200_0900;
        let payload = [
            0x28u8, 0x02, 0x00, 0x00, // header: type=0x2, symbol bits=8, size=2
            0x01, // tree length metadata
            0xC0, // root: both children are terminals
            0x11, 0x22, // left symbol, right symbol
            0x00, 0x00, 0x00, 0x40, // bitstream: 0 then 1
        ];
        for (i, byte) in payload.iter().enumerate() {
            bus.write8(src.wrapping_add(i as u32), *byte);
        }

        cpu.regs[0] = src;
        cpu.regs[1] = dst;
        cpu.step(&mut bus);

        assert_eq!(bus.read8(dst), 0x11);
        assert_eq!(bus.read8(dst + 1), 0x22);
    }

    #[test]
    fn swi_bit_unpack_expands_2bpp_to_4bpp() {
        let (mut cpu, mut bus) = setup_cpu_with_program(&[0xEF000010]); // SWI 0x10
        let src: u32 = 0x0300_0400;
        let info: u32 = 0x0300_0410;
        let dst: u32 = 0x0200_0400;

        bus.write8(src, 0xE4); // 11 10 01 00 (LSB-first units: 0,1,2,3)
        bus.write16(info, 1); // source length in bytes
        bus.write8(info + 2, 2); // source unit width
        bus.write8(info + 3, 4); // destination unit width
        bus.write32(info + 4, 0); // no offset

        cpu.regs[0] = src;
        cpu.regs[1] = dst;
        cpu.regs[2] = info;
        cpu.step(&mut bus);

        assert_eq!(bus.read8(dst), 0x10);
        assert_eq!(bus.read8(dst + 1), 0x32);
    }

    #[test]
    fn swi_diff8_unfilter_reconstructs_wrapped_sequence() {
        let (mut cpu, mut bus) = setup_cpu_with_program(&[0xEF000016]); // SWI 0x16
        let src: u32 = 0x0300_0500;
        let dst: u32 = 0x0200_0500;
        let payload = [
            0x81u8, 0x05, 0x00, 0x00, // header: type=0x81, size=5
            3, 1, 2, 255, 4,
        ];
        for (i, byte) in payload.iter().enumerate() {
            bus.write8(src.wrapping_add(i as u32), *byte);
        }

        cpu.regs[0] = src;
        cpu.regs[1] = dst;
        cpu.step(&mut bus);

        assert_eq!(bus.read8(dst), 3);
        assert_eq!(bus.read8(dst + 1), 4);
        assert_eq!(bus.read8(dst + 2), 6);
        assert_eq!(bus.read8(dst + 3), 5);
        assert_eq!(bus.read8(dst + 4), 9);
    }

    #[test]
    fn swi_diff8_unfilter_vram_writes_halfword_stream() {
        let (mut cpu, mut bus) = setup_cpu_with_program(&[0xEF000017]); // SWI 0x17
        let src: u32 = 0x0300_0800;
        let dst: u32 = 0x0600_0100;
        let payload = [
            0x81u8, 0x03, 0x00, 0x00, // header: type=0x81, size=3
            1, 1, 1, // reconstructed: 1,2,3
        ];
        for (i, byte) in payload.iter().enumerate() {
            bus.write8(src.wrapping_add(i as u32), *byte);
        }

        cpu.regs[0] = src;
        cpu.regs[1] = dst;
        cpu.step(&mut bus);

        assert_eq!(bus.read16(dst), 0x0201);
        assert_eq!(bus.read16(dst + 2), 0x0003);
    }

    #[test]
    fn swi_diff16_unfilter_reconstructs_sequence() {
        let (mut cpu, mut bus) = setup_cpu_with_program(&[0xEF000018]); // SWI 0x18
        let src: u32 = 0x0300_0600;
        let dst: u32 = 0x0200_0600;

        bus.write32(src, 0x0000_0682); // type=0x82, size=6 bytes
        bus.write16(src + 4, 1000);
        bus.write16(src + 6, 24);
        bus.write16(src + 8, 0xFFFE);

        cpu.regs[0] = src;
        cpu.regs[1] = dst;
        cpu.step(&mut bus);

        assert_eq!(bus.read16(dst), 1000);
        assert_eq!(bus.read16(dst + 2), 1024);
        assert_eq!(bus.read16(dst + 4), 1022);
    }

    #[test]
    fn swi_rl_uncomp_decodes_runs_and_literals() {
        let (mut cpu, mut bus) = setup_cpu_with_program(&[0xEF000014]); // SWI 0x14
        let src: u32 = 0x0300_0100;
        let dst: u32 = 0x0200_0100;
        let payload = [
            0x30u8, 0x05, 0x00, 0x00, // header: type=0x30, size=5
            0x80, 0x7F, // run-length 3 of 0x7F
            0x01, 0x11, 0x22, // raw-length 2: 0x11,0x22
        ];
        for (i, byte) in payload.iter().enumerate() {
            bus.write8(src.wrapping_add(i as u32), *byte);
        }

        cpu.regs[0] = src;
        cpu.regs[1] = dst;
        cpu.step(&mut bus);

        assert_eq!(bus.read8(dst), 0x7F);
        assert_eq!(bus.read8(dst + 1), 0x7F);
        assert_eq!(bus.read8(dst + 2), 0x7F);
        assert_eq!(bus.read8(dst + 3), 0x11);
        assert_eq!(bus.read8(dst + 4), 0x22);
    }

    #[test]
    fn swi_rl_uncomp_vram_packs_odd_byte_tail_into_halfword() {
        let (mut cpu, mut bus) = setup_cpu_with_program(&[0xEF000015]); // SWI 0x15
        let src: u32 = 0x0300_0900;
        let dst: u32 = 0x0600_0200;
        let payload = [
            0x30u8, 0x03, 0x00, 0x00, // header: type=0x30, size=3
            0x02, b'A', b'B', b'C', // raw-length 3
        ];
        for (i, byte) in payload.iter().enumerate() {
            bus.write8(src.wrapping_add(i as u32), *byte);
        }

        cpu.regs[0] = src;
        cpu.regs[1] = dst;
        cpu.step(&mut bus);

        assert_eq!(bus.read16(dst), 0x4241);
        assert_eq!(bus.read16(dst + 2), 0x0043);
    }

    #[test]
    fn swi_bg_affine_set_identity_matrix() {
        let (mut cpu, mut bus) = setup_cpu_with_program(&[0xEF00000E]); // SWI 0x0E
        let src: u32 = 0x0300_0200;
        let dst: u32 = 0x0200_0200;

        bus.write32(src, 0); // ox
        bus.write32(src + 4, 0); // oy
        bus.write16(src + 8, 0); // cx
        bus.write16(src + 10, 0); // cy
        bus.write16(src + 12, 256); // sx (1.0 in 8.8)
        bus.write16(src + 14, 256); // sy (1.0 in 8.8)
        bus.write16(src + 16, 0); // angle

        cpu.regs[0] = src;
        cpu.regs[1] = dst;
        cpu.regs[2] = 1;
        cpu.step(&mut bus);

        assert_eq!(bus.read16(dst) as i16, 256);
        assert_eq!(bus.read16(dst + 2) as i16, 0);
        assert_eq!(bus.read16(dst + 4) as i16, 0);
        assert_eq!(bus.read16(dst + 6) as i16, 256);
        assert_eq!(bus.read32(dst + 8), 0);
        assert_eq!(bus.read32(dst + 12), 0);
    }

    #[test]
    fn swi_obj_affine_set_identity_matrix() {
        let (mut cpu, mut bus) = setup_cpu_with_program(&[0xEF00000F]); // SWI 0x0F
        let src: u32 = 0x0300_0300;
        let dst: u32 = 0x0200_0300;

        bus.write16(src, 256); // sx
        bus.write16(src + 2, 256); // sy
        bus.write16(src + 4, 0); // angle

        cpu.regs[0] = src;
        cpu.regs[1] = dst;
        cpu.regs[2] = 1;
        cpu.regs[3] = 2; // tightly packed halfword matrix
        cpu.step(&mut bus);

        assert_eq!(bus.read16(dst) as i16, 256);
        assert_eq!(bus.read16(dst + 2) as i16, 0);
        assert_eq!(bus.read16(dst + 4) as i16, 0);
        assert_eq!(bus.read16(dst + 6) as i16, 256);
    }

    #[test]
    fn irq_exception_enters_vector_when_bios_is_present() {
        let (mut cpu, mut bus) = setup_cpu_with_program(&[0xE1A00000]); // NOP
        bus.load_bios(&[0; 16 * 1024]);
        bus.write16(0x0400_0200, 1); // IE: VBlank
        bus.write16(0x0400_0208, 1); // IME: on
        bus.request_irq(1); // IF: VBlank

        let cycles = cpu.step(&mut bus);

        assert_eq!(cycles, 3);
        assert_eq!(cpu.regs[15], 0x0000_0018);
        assert_eq!(cpu.regs[14], 0x0800_0004);
        assert!(cpu.irq_masked());
        assert!(!cpu.thumb_mode());
    }

    #[test]
    fn irq_without_bios_uses_irq_handler_pointer() {
        let (mut cpu, mut bus) = setup_cpu_with_program(&[0xE1A00000]); // NOP
        bus.write32(0x0300_7FFC, 0x0800_0101); // handler in THUMB
        bus.write16(0x0400_0200, 1); // IE: VBlank
        bus.write16(0x0400_0208, 1); // IME: on
        bus.request_irq(1); // IF: VBlank

        let cycles = cpu.step(&mut bus);

        assert_eq!(cycles, 3);
        assert_eq!(cpu.regs[15], 0x0800_0100);
        assert_eq!(cpu.regs[14], NO_BIOS_IRQ_RETURN_TOKEN);
        assert!(cpu.no_bios_irq_state.is_some());
        assert!(cpu.irq_masked());
        assert_eq!(cpu.cpsr & MODE_MASK, MODE_IRQ);
        assert!(cpu.thumb_mode());
    }

    #[test]
    fn irq_from_thumb_without_bios_bx_lr_restores_pre_irq_context() {
        let mut rom = vec![0; 0x300];
        // Main code (THUMB) at 0x20: NOP
        rom[0x20] = 0xC0;
        rom[0x21] = 0x46;
        // IRQ handler (THUMB) at 0x100: BX LR
        rom[0x100] = 0x70;
        rom[0x101] = 0x47;

        let mut bus = GbaBus::default();
        bus.load_rom(&rom);
        bus.reset();
        bus.write32(0x0300_7FFC, 0x0800_0101); // THUMB handler
        bus.write16(0x0400_0200, 1); // IE: VBlank
        bus.write16(0x0400_0208, 1); // IME: on
        bus.request_irq(1); // IF: VBlank

        let mut cpu = Arm7Tdmi::default();
        cpu.reset_for_boot(false);
        cpu.set_thumb_mode(true);
        cpu.regs[15] = 0x0800_0020;
        let pre_irq_lr = 0x0800_1235;
        cpu.regs[14] = pre_irq_lr;

        let enter_cycles = cpu.step(&mut bus);
        assert_eq!(enter_cycles, 3);
        assert_eq!(cpu.regs[15], 0x0800_0100);
        assert!(cpu.thumb_mode());
        assert_eq!(cpu.regs[14], NO_BIOS_IRQ_RETURN_TOKEN);
        assert_eq!(cpu.cpsr & MODE_MASK, MODE_IRQ);

        let return_cycles = cpu.step(&mut bus); // BX LR
        assert_eq!(return_cycles, 1);
        assert_eq!(cpu.regs[15], 0x0800_0020);
        assert_eq!(cpu.regs[14], pre_irq_lr);
        assert!(cpu.thumb_mode());
        assert_eq!(cpu.cpsr & MODE_MASK, MODE_SYS);
        assert!(cpu.no_bios_irq_state.is_none());
    }

    #[test]
    fn irq_between_thumb_bl_halves_preserves_pending_bl_state() {
        let mut rom = vec![0; 0x300];
        // Main code (THUMB) at 0x20:
        //   BL +0   (two-half encoding)
        //   MOV r0,#1
        //   NOP
        let code: [u16; 4] = [0xF000, 0xF800, 0x2001, 0x46C0];
        for (i, half) in code.iter().enumerate() {
            let off = 0x20 + i * 2;
            rom[off] = (*half & 0xFF) as u8;
            rom[off + 1] = ((*half >> 8) & 0xFF) as u8;
        }
        // IRQ handler (THUMB) at 0x100: BX LR
        rom[0x100] = 0x70;
        rom[0x101] = 0x47;

        let mut bus = GbaBus::default();
        bus.load_rom(&rom);
        bus.reset();
        bus.write32(0x0300_7FFC, 0x0800_0101); // THUMB handler
        bus.write16(0x0400_0200, 1); // IE: VBlank
        bus.write16(0x0400_0208, 1); // IME: on

        let mut cpu = Arm7Tdmi::default();
        cpu.reset_for_boot(false);
        cpu.set_thumb_mode(true);
        cpu.regs[15] = 0x0800_0020;

        // Execute BL upper half.
        let first_half_cycles = cpu.step(&mut bus);
        assert_eq!(first_half_cycles, 2);
        assert_eq!(cpu.regs[15], 0x0800_0022);
        assert_eq!(cpu.thumb_bl_upper, Some(0x0800_0024));

        // Interrupt fires between BL halves.
        bus.request_irq(1);
        let enter_cycles = cpu.step(&mut bus);
        assert_eq!(enter_cycles, 3);
        assert_eq!(cpu.regs[15], 0x0800_0100);

        let return_cycles = cpu.step(&mut bus); // BX LR
        assert_eq!(return_cycles, 1);
        assert_eq!(cpu.regs[15], 0x0800_0022);
        assert_eq!(cpu.thumb_bl_upper, Some(0x0800_0024));

        // Clear pending interrupt to avoid re-entry and execute BL lower half.
        bus.clear_irq(1);
        let second_half_cycles = cpu.step(&mut bus);
        assert_eq!(second_half_cycles, 3);
        assert_eq!(cpu.regs[15], 0x0800_0024);
        assert_eq!(cpu.regs[14], 0x0800_0025);
    }

    #[test]
    fn bios_irq_between_thumb_bl_halves_preserves_pending_bl_state() {
        let mut rom = vec![0; 0x80];
        // Main code (THUMB) at 0x20:
        //   BL +0   (two-half encoding)
        //   MOV r0,#1
        //   NOP
        let code: [u16; 4] = [0xF000, 0xF800, 0x2001, 0x46C0];
        for (i, half) in code.iter().enumerate() {
            let off = 0x20 + i * 2;
            rom[off] = (*half & 0xFF) as u8;
            rom[off + 1] = ((*half >> 8) & 0xFF) as u8;
        }

        let mut bios = vec![0; 16 * 1024];
        // IRQ vector @0x18 -> branch to 0x100.
        let irq_vector_branch = 0xEA00_0038u32;
        bios[0x18] = (irq_vector_branch & 0xFF) as u8;
        bios[0x19] = ((irq_vector_branch >> 8) & 0xFF) as u8;
        bios[0x1A] = ((irq_vector_branch >> 16) & 0xFF) as u8;
        bios[0x1B] = ((irq_vector_branch >> 24) & 0xFF) as u8;
        // IRQ handler @0x100: SUBS PC, LR, #4 (exception return).
        let irq_return = 0xE25E_F004u32;
        bios[0x100] = (irq_return & 0xFF) as u8;
        bios[0x101] = ((irq_return >> 8) & 0xFF) as u8;
        bios[0x102] = ((irq_return >> 16) & 0xFF) as u8;
        bios[0x103] = ((irq_return >> 24) & 0xFF) as u8;

        let mut bus = GbaBus::default();
        bus.load_rom(&rom);
        bus.load_bios(&bios);
        bus.reset();
        bus.write16(0x0400_0200, 1); // IE: VBlank
        bus.write16(0x0400_0208, 1); // IME: on

        let mut cpu = Arm7Tdmi::default();
        cpu.reset_for_boot(true);
        cpu.cpsr = MODE_SYS | FLAG_T;
        cpu.regs[15] = 0x0800_0020;

        // Execute BL upper half.
        let first_half_cycles = cpu.step(&mut bus);
        assert_eq!(first_half_cycles, 2);
        assert_eq!(cpu.regs[15], 0x0800_0022);
        assert_eq!(cpu.thumb_bl_upper, Some(0x0800_0024));

        // Interrupt fires between BL halves.
        bus.request_irq(1);
        let enter_cycles = cpu.step(&mut bus);
        assert_eq!(enter_cycles, 3);
        assert_eq!(cpu.regs[15], 0x0000_0018);

        // Service vector and clear pending IRQ before returning.
        let vector_cycles = cpu.step(&mut bus);
        assert_eq!(vector_cycles, 3);
        assert_eq!(cpu.regs[15], 0x0000_0100);
        bus.clear_irq(1);

        let return_cycles = cpu.step(&mut bus);
        assert_eq!(return_cycles, 1);
        assert_eq!(cpu.regs[15], 0x0800_0022);
        assert_eq!(cpu.thumb_bl_upper, Some(0x0800_0024));
        assert!(cpu.thumb_mode());
        assert_eq!(cpu.cpsr & MODE_MASK, MODE_SYS);

        // Execute BL lower half.
        let second_half_cycles = cpu.step(&mut bus);
        assert_eq!(second_half_cycles, 3);
        assert_eq!(cpu.regs[15], 0x0800_0024);
        assert_eq!(cpu.regs[14], 0x0800_0025);
    }

    #[test]
    fn irq_from_thumb_without_bios_mov_pc_lr_restores_pre_irq_context() {
        let mut rom = vec![0; 0x300];
        // Main code (THUMB) at 0x20: NOP
        rom[0x20] = 0xC0;
        rom[0x21] = 0x46;
        // IRQ handler (THUMB) at 0x100: MOV PC, LR
        rom[0x100] = 0xF7;
        rom[0x101] = 0x46;

        let mut bus = GbaBus::default();
        bus.load_rom(&rom);
        bus.reset();
        bus.write32(0x0300_7FFC, 0x0800_0101); // THUMB handler
        bus.write16(0x0400_0200, 1); // IE: VBlank
        bus.write16(0x0400_0208, 1); // IME: on
        bus.request_irq(1); // IF: VBlank

        let mut cpu = Arm7Tdmi::default();
        cpu.reset_for_boot(false);
        cpu.set_thumb_mode(true);
        cpu.regs[15] = 0x0800_0020;

        let enter_cycles = cpu.step(&mut bus);
        assert_eq!(enter_cycles, 3);
        assert_eq!(cpu.regs[15], 0x0800_0100);
        assert!(cpu.thumb_mode());
        assert_eq!(cpu.cpsr & MODE_MASK, MODE_IRQ);

        let return_cycles = cpu.step(&mut bus); // MOV PC, LR
        assert_eq!(return_cycles, 1);
        assert_eq!(cpu.regs[15], 0x0800_0020);
        assert!(cpu.thumb_mode());
        assert_eq!(cpu.cpsr & MODE_MASK, MODE_SYS);
        assert!(cpu.no_bios_irq_state.is_none());
    }

    #[test]
    fn irq_from_thumb_without_bios_pop_pc_restores_pre_irq_context() {
        let mut rom = vec![0; 0x300];
        // Main code (THUMB) at 0x20: NOP
        rom[0x20] = 0xC0;
        rom[0x21] = 0x46;
        // IRQ handler (THUMB) at 0x100:
        //   PUSH {LR}
        //   POP  {PC}
        rom[0x100] = 0x00;
        rom[0x101] = 0xB5;
        rom[0x102] = 0x00;
        rom[0x103] = 0xBD;

        let mut bus = GbaBus::default();
        bus.load_rom(&rom);
        bus.reset();
        bus.write32(0x0300_7FFC, 0x0800_0101); // THUMB handler
        bus.write16(0x0400_0200, 1); // IE: VBlank
        bus.write16(0x0400_0208, 1); // IME: on
        bus.request_irq(1); // IF: VBlank

        let mut cpu = Arm7Tdmi::default();
        cpu.reset_for_boot(false);
        cpu.set_thumb_mode(true);
        cpu.regs[15] = 0x0800_0020;

        let enter_cycles = cpu.step(&mut bus);
        assert_eq!(enter_cycles, 3);
        assert_eq!(cpu.regs[15], 0x0800_0100);
        assert!(cpu.thumb_mode());
        assert_eq!(cpu.cpsr & MODE_MASK, MODE_IRQ);

        let push_cycles = cpu.step(&mut bus); // PUSH {LR}
        assert_eq!(push_cycles, 1);
        assert_eq!(cpu.regs[15], 0x0800_0102);
        assert_eq!(cpu.cpsr & MODE_MASK, MODE_IRQ);

        let pop_cycles = cpu.step(&mut bus); // POP {PC}
        assert_eq!(pop_cycles, 1);
        assert_eq!(cpu.regs[15], 0x0800_0020);
        assert!(cpu.thumb_mode());
        assert_eq!(cpu.cpsr & MODE_MASK, MODE_SYS);
        assert!(cpu.no_bios_irq_state.is_none());
    }

    #[test]
    fn irq_from_arm_without_bios_subs_pc_lr_restores_pre_irq_context() {
        let mut rom = vec![0; 0x300];
        // Main code (ARM) at 0x0: NOP
        rom[0x00] = 0x00;
        rom[0x01] = 0x00;
        rom[0x02] = 0xA0;
        rom[0x03] = 0xE1;
        // IRQ handler (ARM) at 0x100: SUBS PC, LR, #4
        rom[0x100] = 0x04;
        rom[0x101] = 0xF0;
        rom[0x102] = 0x5E;
        rom[0x103] = 0xE2;

        let mut bus = GbaBus::default();
        bus.load_rom(&rom);
        bus.reset();
        bus.write32(0x0300_7FFC, 0x0800_0100); // ARM handler
        bus.write16(0x0400_0200, 1); // IE: VBlank
        bus.write16(0x0400_0208, 1); // IME: on
        bus.request_irq(1); // IF: VBlank

        let mut cpu = Arm7Tdmi::default();
        cpu.reset_for_boot(false);
        cpu.regs[15] = 0x0800_0000;
        let pre_irq_lr = 0x0800_1234;
        cpu.regs[14] = pre_irq_lr;

        let enter_cycles = cpu.step(&mut bus);
        assert_eq!(enter_cycles, 3);
        assert_eq!(cpu.regs[15], 0x0800_0100);
        assert!(!cpu.thumb_mode());
        assert_eq!(cpu.regs[14], NO_BIOS_IRQ_RETURN_TOKEN);
        assert_eq!(cpu.cpsr & MODE_MASK, MODE_IRQ);

        let return_cycles = cpu.step(&mut bus); // SUBS PC, LR, #4
        assert_eq!(return_cycles, 1);
        assert_eq!(cpu.regs[15], 0x0800_0000);
        assert_eq!(cpu.regs[14], pre_irq_lr);
        assert!(!cpu.thumb_mode());
        assert_eq!(cpu.cpsr & MODE_MASK, MODE_SYS);
        assert!(cpu.no_bios_irq_state.is_none());
    }

    #[test]
    fn mode_switch_preserves_banked_sp_lr() {
        let mut cpu = Arm7Tdmi::default();
        cpu.reset_for_boot(false);
        cpu.regs[13] = 0x1111_1111;
        cpu.regs[14] = 0x2222_2222;

        cpu.set_cpsr((cpu.cpsr & !MODE_MASK) | MODE_IRQ);
        assert_eq!(cpu.regs[13], 0x0300_7FA0);
        assert_eq!(cpu.regs[14], 0);

        cpu.regs[13] = 0x3333_3333;
        cpu.regs[14] = 0x4444_4444;

        cpu.set_cpsr((cpu.cpsr & !MODE_MASK) | MODE_SYS);
        assert_eq!(cpu.regs[13], 0x1111_1111);
        assert_eq!(cpu.regs[14], 0x2222_2222);

        cpu.set_cpsr((cpu.cpsr & !MODE_MASK) | MODE_IRQ);
        assert_eq!(cpu.regs[13], 0x3333_3333);
        assert_eq!(cpu.regs[14], 0x4444_4444);
    }

    #[test]
    fn arm_stm_with_s_bit_uses_user_sp_in_exception_mode() {
        let (mut cpu, mut bus) = setup_cpu_with_program(&[0xE1A0_0000]); // NOP filler
        cpu.set_cpsr((cpu.cpsr & !MODE_MASK) | MODE_IRQ);
        cpu.r13_usr = 0x1111_2222;
        cpu.regs[13] = 0xAAAA_BBBB; // IRQ SP
        cpu.regs[0] = 0x0200_0000;

        // STMIA r0, {r13}^
        let instr = 0xE800_0000 | (1 << 23) | (1 << 22) | (1 << 13);
        cpu.execute_arm_block_data_transfer(instr, 0x0800_0000, &mut bus);

        assert_eq!(bus.read32(0x0200_0000), 0x1111_2222);
        assert_eq!(cpu.regs[13], 0xAAAA_BBBB);
    }

    #[test]
    fn arm_ldm_with_s_bit_writes_user_sp_in_exception_mode() {
        let (mut cpu, mut bus) = setup_cpu_with_program(&[0xE1A0_0000]); // NOP filler
        cpu.set_cpsr((cpu.cpsr & !MODE_MASK) | MODE_IRQ);
        cpu.r13_usr = 0x2222_3333;
        cpu.regs[13] = 0xCCCC_DDDD; // IRQ SP
        cpu.regs[0] = 0x0200_0000;
        bus.write32(0x0200_0000, 0x4444_5555);

        // LDMIA r0, {r13}^
        let instr = 0xE800_0000 | (1 << 23) | (1 << 22) | (1 << 20) | (1 << 13);
        cpu.execute_arm_block_data_transfer(instr, 0x0800_0000, &mut bus);

        assert_eq!(cpu.r13_usr, 0x4444_5555);
        assert_eq!(cpu.regs[13], 0xCCCC_DDDD);
    }

    #[test]
    fn arm_ldm_with_s_and_pc_restores_cpsr_from_spsr() {
        let (mut cpu, mut bus) = setup_cpu_with_program(&[0xE1A0_0000]); // NOP filler
        cpu.set_cpsr((cpu.cpsr & !(MODE_MASK | FLAG_T)) | MODE_IRQ);
        cpu.spsr_irq = MODE_SYS | FLAG_T;
        cpu.regs[0] = 0x0200_0000;
        bus.write32(0x0200_0000, 0x1234_5678); // r1
        bus.write32(0x0200_0004, 0x0800_0101); // pc

        // LDMIA r0!, {r1,pc}^
        let instr =
            0xE800_0000 | (1 << 23) | (1 << 22) | (1 << 21) | (1 << 20) | (1 << 1) | (1 << 15);
        cpu.execute_arm_block_data_transfer(instr, 0x0800_0000, &mut bus);

        assert_eq!(cpu.regs[1], 0x1234_5678);
        assert_eq!(cpu.regs[0], 0x0200_0008);
        assert_eq!(cpu.cpsr & MODE_MASK, MODE_SYS);
        assert!(cpu.thumb_mode());
        assert_eq!(cpu.regs[15], 0x0800_0100);
    }

    #[test]
    fn arm_ldm_with_s_and_pc_preserves_thumb_halfword_alignment() {
        let (mut cpu, mut bus) = setup_cpu_with_program(&[0xE1A0_0000]); // NOP filler
        cpu.set_cpsr((cpu.cpsr & !(MODE_MASK | FLAG_T)) | MODE_IRQ);
        cpu.spsr_irq = MODE_SYS | FLAG_T;
        cpu.regs[0] = 0x0200_0000;
        bus.write32(0x0200_0000, 0x1234_5678); // r1
        bus.write32(0x0200_0004, 0x0800_0102); // pc (Thumb halfword-aligned)

        // LDMIA r0!, {r1,pc}^
        let instr =
            0xE800_0000 | (1 << 23) | (1 << 22) | (1 << 21) | (1 << 20) | (1 << 1) | (1 << 15);
        cpu.execute_arm_block_data_transfer(instr, 0x0800_0000, &mut bus);

        assert_eq!(cpu.regs[1], 0x1234_5678);
        assert_eq!(cpu.regs[0], 0x0200_0008);
        assert_eq!(cpu.cpsr & MODE_MASK, MODE_SYS);
        assert!(cpu.thumb_mode());
        assert_eq!(cpu.regs[15], 0x0800_0102);
    }

    #[test]
    fn arm_mrs_spsr_reads_saved_program_status_register() {
        let (mut cpu, mut bus) = setup_cpu_with_program(&[0xE14F_0000]); // MRS r0, SPSR
        cpu.set_cpsr((cpu.cpsr & !(MODE_MASK | FLAG_T | FLAG_I)) | MODE_IRQ);
        cpu.spsr_irq = MODE_SYS | FLAG_T | FLAG_N | FLAG_C;
        cpu.regs[15] = 0x0800_0000;

        let cycles = cpu.step(&mut bus);

        assert_eq!(cycles, 1);
        assert_eq!(cpu.regs[0], MODE_SYS | FLAG_T | FLAG_N | FLAG_C);
    }

    #[test]
    fn arm_subs_pc_lr_restores_thumb_and_preserves_halfword_alignment() {
        let (mut cpu, mut bus) = setup_cpu_with_program(&[0xE25E_F004]); // SUBS PC,LR,#4
        cpu.set_cpsr((cpu.cpsr & !(MODE_MASK | FLAG_T)) | MODE_IRQ);
        cpu.spsr_irq = MODE_SYS | FLAG_T;
        cpu.regs[14] = 0x0800_0106;
        cpu.regs[15] = 0x0800_0000;

        let cycles = cpu.step(&mut bus);

        assert_eq!(cycles, 1);
        assert_eq!(cpu.cpsr & MODE_MASK, MODE_SYS);
        assert!(cpu.thumb_mode());
        assert_eq!(cpu.regs[15], 0x0800_0102);
    }

    #[test]
    fn reset_without_bios_sets_irq_and_svc_stack_pointers() {
        let mut cpu = Arm7Tdmi::default();
        cpu.reset_for_boot(false);

        assert_eq!(cpu.regs[13], 0x0300_7F00);
        cpu.set_cpsr((cpu.cpsr & !MODE_MASK) | MODE_IRQ);
        assert_eq!(cpu.regs[13], 0x0300_7FA0);
        cpu.set_cpsr((cpu.cpsr & !MODE_MASK) | MODE_SVC);
        assert_eq!(cpu.regs[13], 0x0300_7FE0);
    }

    #[test]
    fn unknown_arm_in_irq_mode_without_bios_retries_as_thumb() {
        let (mut cpu, mut bus) = setup_cpu_with_program(&[0xEE00_2001]); // ARM cp-opcode + THUMB MOV r0,#1
        cpu.set_cpsr((cpu.cpsr & !MODE_MASK) | MODE_IRQ);
        cpu.set_thumb_mode(false);
        cpu.regs[15] = 0x0800_0000;

        let first = cpu.step(&mut bus);
        assert_eq!(first, 1);
        assert!(cpu.thumb_mode());
        assert_eq!(cpu.regs[15], 0x0800_0000);

        let second = cpu.step(&mut bus);
        assert_eq!(second, 1);
        assert_eq!(cpu.regs[0], 1);
    }

    #[test]
    fn subs_pc_lr_restores_cpsr_from_spsr_irq() {
        let (mut cpu, mut bus) = setup_cpu_with_program(&[0xE1A00000]); // filler
        let mut bios = vec![0; 16 * 1024];
        let vector_instr = 0xE25E_F004u32; // SUBS PC,LR,#4
        bios[0x18] = (vector_instr & 0xFF) as u8;
        bios[0x19] = ((vector_instr >> 8) & 0xFF) as u8;
        bios[0x1A] = ((vector_instr >> 16) & 0xFF) as u8;
        bios[0x1B] = ((vector_instr >> 24) & 0xFF) as u8;
        bus.load_bios(&bios);
        bus.reset();

        cpu.reset_for_boot(true);
        cpu.set_cpsr((cpu.cpsr & !(MODE_MASK | FLAG_T | FLAG_I)) | MODE_SYS | FLAG_T);
        cpu.regs[15] = 0x0800_0020;

        bus.write16(0x0400_0200, 1); // IE: VBlank
        bus.write16(0x0400_0208, 1); // IME: on
        bus.request_irq(1); // IF: VBlank

        cpu.step(&mut bus); // enter IRQ
        assert_eq!(cpu.cpsr & MODE_MASK, MODE_IRQ);
        assert!(!cpu.thumb_mode());

        cpu.step(&mut bus); // execute SUBS PC,LR,#4
        assert_eq!(cpu.cpsr & MODE_MASK, MODE_SYS);
        assert!(cpu.thumb_mode());
        assert_eq!(cpu.regs[15], 0x0800_0020);
    }
}
