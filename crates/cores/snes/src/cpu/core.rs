#![allow(dead_code)]
//! Shared 65C816 CPU core implementation.
//!
//! This module provides the complete 65C816 instruction set execution
//! that can be used by both S-CPU and SA-1 through bus abstraction.

mod addressing;
mod alu;
mod control_flow;
mod execution;
mod fetch;
mod flags;
mod interrupt;
mod memory;
mod stack;

pub use execution::execute_instruction_generic;
pub use fetch::{fetch_opcode, fetch_opcode_generic};
pub use interrupt::{service_irq, service_nmi};

use crate::{cpu::bus::CpuBus, cpu::StatusFlags};

#[derive(Debug, Clone)]
pub struct FetchResult {
    pub opcode: u8,
    pub memspeed_penalty: u8,
    pub pc_before: u16,
    pub full_addr: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeferredFetchState {
    pub opcode: u8,
    pub memspeed_penalty: u8,
    pub pc_before: u16,
    pub full_addr: u32,
}

#[derive(Debug, Clone)]
pub struct StepResult {
    pub cycles: u8,
    pub fetch: FetchResult,
}

#[derive(Debug, Clone)]
pub struct Core {
    pub state: CoreState,
    deferred_fetch: Option<FetchResult>,
}

#[derive(Debug, Clone)]
pub struct CoreState {
    pub a: u16,
    pub x: u16,
    pub y: u16,
    pub sp: u16,
    pub dp: u16,
    pub db: u8,
    pub pb: u8,
    pub pc: u16,
    pub p: StatusFlags,
    pub emulation_mode: bool,
    pub cycles: u64,
    pub waiting_for_irq: bool,
    pub stopped: bool,
    pub brk_is_nop: bool,
}

impl CoreState {
    pub fn new(default_flags: StatusFlags, emulation_mode: bool) -> Self {
        Self {
            a: 0,
            x: 0,
            y: 0,
            sp: 0x01FF,
            dp: 0,
            db: 0,
            pb: 0,
            pc: 0,
            p: default_flags,
            emulation_mode,
            cycles: 0,
            waiting_for_irq: false,
            stopped: false,
            brk_is_nop: false,
        }
    }
}

impl Core {
    pub fn new(default_flags: StatusFlags, emulation_mode: bool) -> Self {
        Self {
            state: CoreState::new(default_flags, emulation_mode),
            deferred_fetch: None,
        }
    }

    pub fn reset(&mut self, default_flags: StatusFlags, emulation_mode: bool) {
        self.state = CoreState::new(default_flags, emulation_mode);
        self.deferred_fetch = None;
    }

    #[inline]
    pub fn has_deferred_instruction(&self) -> bool {
        self.deferred_fetch.is_some()
    }

    #[inline]
    pub fn deferred_full_addr(&self) -> Option<u32> {
        self.deferred_fetch.as_ref().map(|f| f.full_addr)
    }

    #[inline]
    pub fn deferred_fetch_state(&self) -> Option<DeferredFetchState> {
        self.deferred_fetch
            .as_ref()
            .map(|fetch| DeferredFetchState {
                opcode: fetch.opcode,
                memspeed_penalty: fetch.memspeed_penalty,
                pc_before: fetch.pc_before,
                full_addr: fetch.full_addr,
            })
    }

    #[inline]
    pub fn set_deferred_fetch_state(&mut self, fetch: Option<DeferredFetchState>) {
        self.deferred_fetch = fetch.map(|fetch| FetchResult {
            opcode: fetch.opcode,
            memspeed_penalty: fetch.memspeed_penalty,
            pc_before: fetch.pc_before,
            full_addr: fetch.full_addr,
        });
    }

    pub fn step<B: CpuBus>(&mut self, bus: &mut B) -> StepResult {
        // If an MDMA started after the previous opcode fetch, we deferred executing that
        // instruction until after the DMA stall time elapsed (hardware behavior).
        if let Some(fetch) = self.deferred_fetch.take() {
            let opcode = fetch.opcode;
            let mut cycles = execute_instruction_generic(&mut self.state, opcode, bus);
            // The opcode fetch cycle (and any memspeed penalty) was already accounted for in
            // the previous step, so subtract the opcode fetch here.
            cycles = cycles.saturating_sub(1);
            return StepResult { cycles, fetch };
        }

        let fetch = fetch_opcode_generic(&mut self.state, bus);
        let opcode = fetch.opcode;

        // If the bus started MDMA after this opcode fetch, return early with only the opcode
        // fetch time (1 cycle + optional wait state). The instruction will be executed on the
        // next CPU step after the DMA stall has been consumed by the main loop.
        if bus.take_dma_start_event() {
            self.deferred_fetch = Some(fetch.clone());
            if fetch.memspeed_penalty != 0 {
                self.state.cycles = self
                    .state
                    .cycles
                    .wrapping_add(fetch.memspeed_penalty as u64);
            }
            let cycles = 1u8.saturating_add(fetch.memspeed_penalty);
            return StepResult { cycles, fetch };
        }

        let mut cycles = execute_instruction_generic(&mut self.state, opcode, bus);
        if fetch.memspeed_penalty != 0 {
            self.state.cycles = self
                .state
                .cycles
                .wrapping_add(fetch.memspeed_penalty as u64);
        }
        cycles += fetch.memspeed_penalty;
        StepResult { cycles, fetch }
    }

    pub fn state(&self) -> &CoreState {
        &self.state
    }

    pub fn state_mut(&mut self) -> &mut CoreState {
        &mut self.state
    }
}

#[inline(always)]
pub fn full_address(state: &CoreState, offset: u16) -> u32 {
    ((state.pb as u32) << 16) | (offset as u32)
}

// --------------------------- tests ---------------------------
#[cfg(test)]
mod tests {
    use super::*;
    use crate::cpu::bus::CpuBus;

    #[derive(Clone)]
    struct TestBus {
        mem: Vec<u8>,
    }

    impl TestBus {
        fn new() -> Self {
            Self {
                mem: vec![0; 0x200000], // 2MB, plenty for tests
            }
        }
        fn load(&mut self, addr: u32, data: &[u8]) {
            let start = addr as usize;
            self.mem[start..start + data.len()].copy_from_slice(data);
        }
    }

    impl CpuBus for TestBus {
        fn read_u8(&mut self, addr: u32) -> u8 {
            *self.mem.get(addr as usize).unwrap_or(&0)
        }
        fn write_u8(&mut self, addr: u32, value: u8) {
            if let Some(slot) = self.mem.get_mut(addr as usize) {
                *slot = value;
            }
        }
        fn poll_irq(&mut self) -> bool {
            false
        }
        fn poll_nmi(&mut self) -> bool {
            false
        }
    }

    fn default_flags() -> StatusFlags {
        StatusFlags::IRQ_DISABLE | StatusFlags::MEMORY_8BIT | StatusFlags::INDEX_8BIT
    }

    fn make_core(pc: u16) -> Core {
        let mut c = Core::new(default_flags(), true);
        {
            let st = c.state_mut();
            st.pc = pc;
            st.pb = 0;
        }
        c
    }

    fn run_steps(core: &mut Core, bus: &mut TestBus, steps: usize) {
        for _ in 0..steps {
            core.step(bus);
        }
    }

    #[test]
    fn adc_dp_indirect_x_wraps_pointer_read_in_emulation_mode() {
        // cputest-full Test 0024 expects (dp,X) to wrap the pointer read within the direct page
        // when crossing the low-byte boundary (6502-style) in emulation mode.
        let mut bus = TestBus::new();
        // ADC ($EF,X)
        bus.load(0x8000, &[0x61, 0xEF, 0xEA]);

        // D=$0100, X=$0010 => pointer fetch at $01FF, high byte wraps to $0100.
        bus.load(0x0001FF, &[0x34]); // low
        bus.load(0x000100, &[0x12]); // high (wrapped)

        // DBR=$01, effective addr $01:1234 holds operand 0xED
        bus.load(0x011234, &[0xED]);

        let mut core = make_core(0x8000);
        {
            let st = core.state_mut();
            st.emulation_mode = true;
            st.p = StatusFlags::IRQ_DISABLE | StatusFlags::CARRY; // emulation => M/X=1
            st.a = 0x1112;
            st.x = 0x0010;
            st.dp = 0x0100;
            st.db = 0x01;
        }

        run_steps(&mut core, &mut bus, 1);
        let st = core.state();
        assert_eq!(st.a, 0x1100);
        assert!(st.p.contains(StatusFlags::CARRY));
        assert!(st.p.contains(StatusFlags::ZERO));
    }

    #[test]
    fn adc_dp_indirect_x_wraps_index_sum_in_emulation_mode_when_dp_aligned() {
        // cputest-full Test 0025 expects (dp,X) to wrap the direct-page index sum when D is
        // page-aligned in emulation mode: (base + X) uses 8-bit wrapping.
        let mut bus = TestBus::new();
        // ADC ($F0,X)
        bus.load(0x8000, &[0x61, 0xF0, 0xEA]);

        // D=$0100, X=$0010 => (0xF0 + 0x10)=0x00 (8-bit wrap), so pointer at $0100.
        bus.load(0x000100, &[0x34, 0x12]);

        // DBR=$01, effective addr $01:1234 holds operand 0xED
        bus.load(0x011234, &[0xED]);

        let mut core = make_core(0x8000);
        {
            let st = core.state_mut();
            st.emulation_mode = true;
            st.p = StatusFlags::IRQ_DISABLE | StatusFlags::CARRY; // emulation => M/X=1
            st.a = 0x1112;
            st.x = 0x0010;
            st.dp = 0x0100;
            st.db = 0x01;
        }

        run_steps(&mut core, &mut bus, 1);
        let st = core.state();
        assert_eq!(st.a, 0x1100);
        assert!(st.p.contains(StatusFlags::CARRY));
        assert!(st.p.contains(StatusFlags::ZERO));
    }

    #[test]
    fn trb_absolute_16bit_can_cross_bank_boundary() {
        // TRB absolute in 16-bit mode should operate on a 16-bit operand and use a 24-bit
        // increment for the upper byte read/write, allowing bank carry (e.g., 0x01FFFF -> 0x020000).
        let mut bus = TestBus::new();
        // TRB $FFFF
        bus.load(0x8000, &[0x1C, 0xFF, 0xFF, 0xEA]);

        // Operand at DBR:FFFF spans two bytes: 0x01FFFF (lo) and 0x020000 (hi).
        bus.load(0x01FFFF, &[0x34]);
        bus.load(0x020000, &[0x92]);

        let mut core = make_core(0x8000);
        {
            let st = core.state_mut();
            st.emulation_mode = false;
            st.p = StatusFlags::IRQ_DISABLE; // 16-bit A/X/Y
            st.a = 0x1630;
            st.db = 0x01;
        }

        core.step(&mut bus);

        assert_eq!(bus.read_u8(0x01FFFF), 0x04);
        assert_eq!(bus.read_u8(0x020000), 0x80);
        assert!(!core.state().p.contains(StatusFlags::ZERO));
    }

    #[test]
    fn adc_dp_indirect_consumes_one_operand_byte() {
        // 0x72 ADC (dp) uses an 8-bit direct page operand (not 16-bit).
        // Regression: we previously used read_u16_generic and skipped the next opcode byte.
        let mut bus = TestBus::new();
        // Program: LDA #$01 ; ADC ($34)
        bus.load(0x8000, &[0xA9, 0x01, 0x72, 0x34]);
        // DP pointer at $0034 -> $9000
        bus.load(0x0034, &[0x00, 0x90]);
        bus.load(0x9000, &[0x05]);

        let mut core = make_core(0x8000);
        {
            let st = core.state_mut();
            st.emulation_mode = false;
            st.db = 0x00;
            st.dp = 0x0000;
        }

        run_steps(&mut core, &mut bus, 2);
        let st = core.state();
        assert_eq!(st.a & 0x00FF, 0x06);
        assert_eq!(st.pc, 0x8004);
    }

    #[test]
    fn jmp_abs_x_reads_pointer_from_program_bank() {
        // 0x7C JMP (abs,X) reads the 16-bit pointer from the current program bank (PB).
        let mut bus = TestBus::new();
        // Place the instruction in bank 01 at 01:8000.
        // JMP ($2000,X)
        bus.load(0x018000, &[0x7C, 0x00, 0x20]);
        // X=4 => pointer read from 01:2004.
        // Pointer value 0x1234 stored in program bank 01 at 01:2004.
        bus.load(0x012004, &[0x34, 0x12]);
        // Bank 00 has a different value to catch incorrect addressing.
        bus.load(0x00002004, &[0xFF, 0xFF]);

        let mut core = make_core(0x8000);
        {
            let st = core.state_mut();
            st.emulation_mode = false;
            st.pb = 0x01;
            st.x = 0x0004;
        }

        run_steps(&mut core, &mut bus, 1);
        let st = core.state();
        assert_eq!(st.pb, 0x01);
        assert_eq!(st.pc, 0x1234);
    }

    #[test]
    fn jmp_abs_reads_pointer_from_bank00() {
        // 0x6C JMP (abs) reads the 16-bit pointer from bank 00 (not PB/DB).
        let mut bus = TestBus::new();
        // Place the instruction in bank 01 at 01:8000.
        // JMP ($FFA2)
        bus.load(0x018000, &[0x6C, 0xA2, 0xFF]);
        // Pointer at 00:FFA2 -> $1234
        bus.load(0x00FFA2, &[0x34, 0x12]);
        // Put a different value in the program bank to ensure we don't read PB.
        bus.load(0x01FFA2, &[0xFF, 0xFF]);

        let mut core = make_core(0x8000);
        {
            let st = core.state_mut();
            st.emulation_mode = false;
            st.pb = 0x01;
        }
        run_steps(&mut core, &mut bus, 1);
        let st = core.state();
        assert_eq!(st.pb, 0x01);
        assert_eq!(st.pc, 0x1234);
    }

    #[test]
    fn jsr_abs_x_reads_pointer_from_program_bank() {
        // 0xFC JSR (abs,X) reads the 16-bit target from the current program bank (PB).
        let mut bus = TestBus::new();
        // Place the instruction in bank 01 at 01:8000.
        // JSR ($2000,X)
        bus.load(0x018000, &[0xFC, 0x00, 0x20]);
        // X=4 => pointer read from 01:2004 => $1234.
        bus.load(0x012004, &[0x34, 0x12]);
        // Bank 00 has a different value to catch incorrect addressing.
        bus.load(0x00002004, &[0xFF, 0xFF]);

        let mut core = make_core(0x8000);
        {
            let st = core.state_mut();
            st.emulation_mode = false;
            st.pb = 0x01;
            st.x = 0x0004;
        }
        run_steps(&mut core, &mut bus, 1);
        let st = core.state();
        assert_eq!(st.pb, 0x01);
        assert_eq!(st.pc, 0x1234);
    }

    #[test]
    fn adc_decimal_is_disabled_by_default() {
        // decimal flag should not alter addition because DECIMAL is off
        let mut bus = TestBus::new();
        bus.load(
            0x8000,
            &[
                0xA9, 0x15, // LDA #$15
                0x69, 0x27, // ADC #$27 -> 0x3C, no carry
            ],
        );
        let mut core = make_core(0x8000);
        run_steps(&mut core, &mut bus, 2);
        let st = core.state();
        assert_eq!(st.a & 0x00FF, 0x3C);
        assert!(!st.p.contains(StatusFlags::CARRY));
    }

    #[test]
    fn adc_immediate_8bit_basic() {
        // A=0x10, ADC #0x05 => 0x15, flags: none set except IRQ_DISABLE and size flags
        let mut bus = TestBus::new();
        bus.load(
            0x8000,
            &[
                0xA9, 0x10, // LDA #$10 (8-bit)
                0x69, 0x05, // ADC #$05
            ],
        );
        let mut core = make_core(0x8000);
        run_steps(&mut core, &mut bus, 2);
        let st = core.state();
        assert_eq!(st.a & 0x00FF, 0x15);
        assert!(!st.p.contains(StatusFlags::CARRY));
        assert!(!st.p.contains(StatusFlags::ZERO));
        assert!(!st.p.contains(StatusFlags::NEGATIVE));
    }

    #[test]
    fn adc_immediate_8bit_overflow() {
        // A=0x7F, ADC #0x01 => 0x80, V and N set, C clear
        let mut bus = TestBus::new();
        bus.load(
            0x8000,
            &[
                0xA9, 0x7F, // LDA #$7F
                0x69, 0x01, // ADC #$01
            ],
        );
        let mut core = make_core(0x8000);
        run_steps(&mut core, &mut bus, 2);
        let st = core.state();
        assert_eq!(st.a & 0x00FF, 0x80);
        assert!(st.p.contains(StatusFlags::OVERFLOW));
        assert!(st.p.contains(StatusFlags::NEGATIVE));
        assert!(!st.p.contains(StatusFlags::CARRY));
    }

    #[test]
    fn adc_immediate_16bit() {
        // REP #$20 -> 16-bit A; LDA #$1234; ADC #$0001 => 0x1235
        let mut bus = TestBus::new();
        bus.load(
            0x8000,
            &[
                0xC2, 0x20, // REP #$20 (clear M)
                0xA9, 0x34, 0x12, // LDA #$1234
                0x69, 0x01, 0x00, // ADC #$0001
            ],
        );
        let mut core = make_core(0x8000);
        {
            // 16-bit A の検証なので native mode に切り替える
            let st = core.state_mut();
            st.emulation_mode = false;
        }
        run_steps(&mut core, &mut bus, 3);
        let st = core.state();
        assert_eq!(st.a, 0x1235);
        // Carry/Overflow 挙動は実装依存なので値のみ検証
    }

    #[test]
    fn adc_immediate_8bit_carry_ignores_b() {
        // In 8-bit accumulator mode, ADC carries out of bit7 (low byte) only.
        // The upper accumulator byte (B) must not affect carry/overflow.
        //
        // Scenario from cputest-full (Test 0032):
        // REP #$20 ; LDA #$1167 ; SEP #$20 ; ADC #$20
        // => A=$1187, C=0, V=1, N=1
        let mut bus = TestBus::new();
        bus.load(
            0x8000,
            &[
                0xC2, 0x20, // REP #$20 (clear M => 16-bit A)
                0xA9, 0x67, 0x11, // LDA #$1167
                0xE2, 0x20, // SEP #$20 (set M => 8-bit A)
                0x69, 0x20, // ADC #$20
            ],
        );
        let mut core = make_core(0x8000);
        {
            let st = core.state_mut();
            st.emulation_mode = false;
        }
        run_steps(&mut core, &mut bus, 4);
        let st = core.state();
        assert_eq!(st.a, 0x1187);
        assert!(st.p.contains(StatusFlags::MEMORY_8BIT));
        assert!(st.p.contains(StatusFlags::OVERFLOW));
        assert!(st.p.contains(StatusFlags::NEGATIVE));
        assert!(!st.p.contains(StatusFlags::CARRY));
        assert!(!st.p.contains(StatusFlags::ZERO));
    }

    #[test]
    fn sbc_immediate_8bit_borrow_ignores_b() {
        // In 8-bit accumulator mode, SBC borrow/carry is computed from the low byte only.
        // Upper accumulator byte (B) must be preserved and must not affect carry.
        let mut bus = TestBus::new();
        bus.load(
            0x8000,
            &[
                0xC2, 0x20, // REP #$20 (clear M => 16-bit A)
                0xA9, 0x67, 0x11, // LDA #$1167
                0xE2, 0x20, // SEP #$20 (set M => 8-bit A)
                0x38, // SEC (no borrow)
                0xE9, 0x20, // SBC #$20 => low: 0x67-0x20=0x47
            ],
        );
        let mut core = make_core(0x8000);
        {
            let st = core.state_mut();
            st.emulation_mode = false;
        }
        run_steps(&mut core, &mut bus, 5);
        let st = core.state();
        assert_eq!(st.a, 0x1147);
        assert!(st.p.contains(StatusFlags::MEMORY_8BIT));
        assert!(st.p.contains(StatusFlags::CARRY));
        assert!(!st.p.contains(StatusFlags::OVERFLOW));
        assert!(!st.p.contains(StatusFlags::NEGATIVE));
        assert!(!st.p.contains(StatusFlags::ZERO));
    }

    #[test]
    fn mvn_operand_order_is_dest_then_src_and_sets_dbr() {
        // MVN takes two immediate operands in object code: destination bank, then source bank.
        // It copies A+1 bytes from src: X.. to dest: Y.., increments X/Y each step,
        // decrements A and repeats until A becomes 0xFFFF, and sets DBR=dest bank.
        let mut bus = TestBus::new();
        // MVN #$00,#$01 (src=00, dest=01) => bytes are 0x54, dest=01, src=00
        bus.load(0x8000, &[0x54, 0x01, 0x00, 0xEA]); // NOP after
                                                     // Source bytes at 00:1000..1003
        bus.load(0x001000, &[0xDE, 0xAD, 0xBE, 0xEF]);
        // Destination area at 01:2000..2003 (init to 0)
        bus.load(0x012000, &[0x00, 0x00, 0x00, 0x00]);

        let mut core = make_core(0x8000);
        {
            let st = core.state_mut();
            st.emulation_mode = false;
            st.p = StatusFlags::IRQ_DISABLE; // 16-bit A/X/Y
            st.a = 0x0003; // copy 4 bytes total
            st.x = 0x1000;
            st.y = 0x2000;
            st.db = 0x7E; // should become dest bank (0x01)
        }

        run_steps(&mut core, &mut bus, 4);
        let st = core.state();
        assert_eq!(st.a, 0xFFFF);
        assert_eq!(st.x, 0x1004);
        assert_eq!(st.y, 0x2004);
        assert_eq!(st.db, 0x01);
        assert_eq!(st.pc, 0x8003);

        assert_eq!(bus.read_u8(0x012000), 0xDE);
        assert_eq!(bus.read_u8(0x012001), 0xAD);
        assert_eq!(bus.read_u8(0x012002), 0xBE);
        assert_eq!(bus.read_u8(0x012003), 0xEF);
    }

    #[test]
    fn cmp_indirect_x_uses_dbr_for_effective_address() {
        // cputest-full Test 00C8 expects CMP ($10,X) to read the operand from DBR:ptr,
        // not from bank 00. Also exercises DP wrapping with D=$FFFF.
        let mut bus = TestBus::new();
        // CMP ($10,X)
        bus.load(0x8000, &[0xC1, 0x10]);
        // D=$FFFF, X=$FF91 => pointer read from $FFA0 (00:FFA0)
        // ptr=$1212
        bus.load(0x00FFA0, &[0x12, 0x12]);
        // DBR=$01 => operand at 01:1212 is $ABCD (little-endian)
        bus.load(0x011212, &[0xCD, 0xAB]);

        let mut core = make_core(0x8000);
        {
            let st = core.state_mut();
            st.emulation_mode = false;
            st.p = StatusFlags::IRQ_DISABLE; // 16-bit A/X/Y
            st.a = 0xABCD;
            st.x = 0xFF91;
            st.dp = 0xFFFF;
            st.db = 0x01;
        }

        run_steps(&mut core, &mut bus, 1);
        let st = core.state();
        assert_eq!(st.a, 0xABCD);
        assert_eq!(st.pc, 0x8002);
        assert!(st.p.contains(StatusFlags::CARRY));
        assert!(st.p.contains(StatusFlags::ZERO));
        assert!(!st.p.contains(StatusFlags::NEGATIVE));
    }

    #[test]
    fn cmp_stack_relative_reads_from_sp_plus_offset() {
        // cputest-full Test 00CA: CMP $12,S with SP=$01EF should read from $0201.
        let mut bus = TestBus::new();
        // CMP $12,S
        bus.load(0x8000, &[0xC3, 0x12]);
        // SP=$01EF => $01EF + 0x12 = $0201
        bus.load(0x000201, &[0xCD, 0xAB]);

        let mut core = make_core(0x8000);
        {
            let st = core.state_mut();
            st.emulation_mode = false;
            st.p = StatusFlags::IRQ_DISABLE; // 16-bit A
            st.a = 0xABCD;
            st.sp = 0x01EF;
        }

        run_steps(&mut core, &mut bus, 1);
        let st = core.state();
        assert_eq!(st.a, 0xABCD);
        assert_eq!(st.pc, 0x8002);
        assert!(st.p.contains(StatusFlags::CARRY));
        assert!(st.p.contains(StatusFlags::ZERO));
        assert!(!st.p.contains(StatusFlags::NEGATIVE));
    }

    #[test]
    fn lda_stack_relative_indirect_y_uses_dbr_and_bank_carry() {
        // cputest-full Test 01C8 expects (sr,S),Y to use DBR for the bank and allow carry into bank.
        // Use a small DBR to keep addresses within TestBus memory.
        //
        // DBR=$01, ptr=$FEDC, Y=$1100 => effective $02:0FDC (bank carry).
        let mut bus = TestBus::new();
        // LDA ($10,S),Y
        bus.load(0x8000, &[0xB3, 0x10]);
        // SP=$01EF => base=$01FF, ptr=$FEDC
        bus.load(0x0001FF, &[0xDC, 0xFE]);
        // value at 02:0FDC is $8000
        bus.load(0x020FDC, &[0x00, 0x80]);

        let mut core = make_core(0x8000);
        {
            let st = core.state_mut();
            st.emulation_mode = false;
            st.p = StatusFlags::IRQ_DISABLE; // 16-bit A
            st.a = 0x1234;
            st.sp = 0x01EF;
            st.y = 0x1100;
            st.db = 0x01;
        }

        run_steps(&mut core, &mut bus, 1);
        let st = core.state();
        assert_eq!(st.a, 0x8000);
        assert_eq!(st.pc, 0x8002);
        assert!(st.p.contains(StatusFlags::NEGATIVE));
        assert!(!st.p.contains(StatusFlags::ZERO));
    }

    #[test]
    fn pha_pla_preserves_a() {
        // A=0x42; PHA; LDA #$00; PLA -> A should be 0x42, SP should round-trip
        let mut bus = TestBus::new();
        bus.load(
            0x8000,
            &[
                0xA9, 0x42, // LDA #$42
                0x48, // PHA
                0xA9, 0x00, // LDA #$00
                0x68, // PLA
            ],
        );
        let mut core = make_core(0x8000);
        let sp_start = core.state.sp;
        run_steps(&mut core, &mut bus, 4);
        let st = core.state();
        assert_eq!(st.a & 0x00FF, 0x42);
        assert_eq!(st.sp, sp_start);
    }
}
