#![allow(dead_code)]
//! Shared 65C816 CPU core implementation.
//!
//! This module provides the complete 65C816 instruction set execution
//! that can be used by both S-CPU and SA-1 through bus abstraction.

mod fetch;
mod interrupt;

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

// Build a 24bit address using the current data bank (DB) for absolute/absolute indexed operands.
#[inline]
fn abs_address(state: &CoreState, addr16: u32) -> u32 {
    ((state.db as u32) << 16) | (addr16 & 0xFFFF)
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

// Generic helper functions for instruction execution

#[inline(always)]
fn add_cycles(state: &mut CoreState, cycles: u8) {
    state.cycles = state.cycles.wrapping_add(cycles as u64);
}

#[inline(always)]
fn read_u8_generic<T: CpuBus>(state: &mut CoreState, bus: &mut T) -> u8 {
    let addr = full_address(state, state.pc);
    let value = bus.read_u8(addr);
    state.pc = state.pc.wrapping_add(1);
    add_cycles(state, 1);
    value
}

#[inline(always)]
fn write_u8_generic<T: CpuBus>(bus: &mut T, addr: u32, value: u8) {
    bus.write_u8(addr, value);
}

#[inline(always)]
fn set_flags_nz_8(state: &mut CoreState, value: u8) {
    state.p.set(StatusFlags::NEGATIVE, value & 0x80 != 0);
    state.p.set(StatusFlags::ZERO, value == 0);
}

#[inline(always)]
fn set_flags_nz_16(state: &mut CoreState, value: u16) {
    state.p.set(StatusFlags::NEGATIVE, value & 0x8000 != 0);
    state.p.set(StatusFlags::ZERO, value == 0);
}

#[inline(always)]
fn read_u16_generic<T: CpuBus>(state: &mut CoreState, bus: &mut T) -> u16 {
    let addr = full_address(state, state.pc);
    let value = bus.read_u16(addr);
    state.pc = state.pc.wrapping_add(2);
    add_cycles(state, 2);
    value
}

#[inline(always)]
fn read_u24_generic<T: CpuBus>(state: &mut CoreState, bus: &mut T) -> u32 {
    let addr = full_address(state, state.pc);
    let lo = bus.read_u8(addr) as u32;
    let mid = bus.read_u8(addr + 1) as u32;
    let hi = bus.read_u8(addr + 2) as u32;
    state.pc = state.pc.wrapping_add(3);
    add_cycles(state, 3);
    lo | (mid << 8) | (hi << 16)
}

#[inline(always)]
fn read_absolute_address_generic<T: CpuBus>(state: &mut CoreState, bus: &mut T) -> u32 {
    let addr = read_u16_generic(state, bus);
    ((state.db as u32) << 16) | (addr as u32)
}

#[inline(always)]
fn push_u8_generic<T: CpuBus>(state: &mut CoreState, bus: &mut T, value: u8) {
    let addr = if state.emulation_mode {
        0x0100 | (state.sp as u32)
    } else {
        state.sp as u32
    };
    if crate::debug_flags::trace_stack_guard() {
        println!(
            "STACK PUSH8 PB={:02X} PC={:04X} SP={:04X} -> [{:04X}]={:02X}",
            state.pb, state.pc, state.sp, addr, value
        );
    }
    bus.write_u8(addr, value);
    state.sp = if state.emulation_mode {
        0x0100 | ((state.sp.wrapping_sub(1)) & 0xFF)
    } else {
        state.sp.wrapping_sub(1)
    };
    add_cycles(state, 1);
}

#[inline(always)]
fn push_u16_generic<T: CpuBus>(state: &mut CoreState, bus: &mut T, value: u16) {
    push_u8_generic(state, bus, (value >> 8) as u8);
    push_u8_generic(state, bus, (value & 0xFF) as u8);
}

// W65C816S datasheet note (emulation mode): some opcodes that push/pull 2+ bytes use a 16-bit
// stack increment/decrement sequence and can access outside $0100-$01FF when SP is near the edge.
// Examples: PEA/PEI/PER/PHD/PLD, JSL/RTL, JSR (abs,X).
#[inline]
fn push_u16_emulation_edge<T: CpuBus>(state: &mut CoreState, bus: &mut T, value: u16) {
    bus.write_u8(state.sp as u32, (value >> 8) as u8);
    state.sp = state.sp.wrapping_sub(1);
    add_cycles(state, 1);
    bus.write_u8(state.sp as u32, (value & 0xFF) as u8);
    state.sp = state.sp.wrapping_sub(1);
    add_cycles(state, 1);
    // Re-assert emulation-mode stack high byte after the sequence.
    state.sp = 0x0100 | (state.sp & 0x00FF);
}

#[inline]
fn pop_u16_emulation_edge<T: CpuBus>(state: &mut CoreState, bus: &mut T) -> u16 {
    state.sp = state.sp.wrapping_add(1);
    let lo = bus.read_u8(state.sp as u32) as u16;
    add_cycles(state, 1);
    state.sp = state.sp.wrapping_add(1);
    let hi = bus.read_u8(state.sp as u32) as u16;
    add_cycles(state, 1);
    // Re-assert emulation-mode stack high byte after the sequence.
    state.sp = 0x0100 | (state.sp & 0x00FF);
    (hi << 8) | lo
}

#[inline(always)]
fn pop_u8_generic<T: CpuBus>(state: &mut CoreState, bus: &mut T) -> u8 {
    state.sp = if state.emulation_mode {
        0x0100 | ((state.sp.wrapping_add(1)) & 0xFF)
    } else {
        state.sp.wrapping_add(1)
    };
    let addr = if state.emulation_mode {
        0x0100 | (state.sp as u32)
    } else {
        state.sp as u32
    };
    add_cycles(state, 1);
    bus.read_u8(addr)
}

#[inline(always)]
fn pop_u16_generic<T: CpuBus>(state: &mut CoreState, bus: &mut T) -> u16 {
    let lo = pop_u8_generic(state, bus) as u16;
    let hi = pop_u8_generic(state, bus) as u16;
    (hi << 8) | lo
}

#[inline]
fn is_suspicious_exec_target(pb: u8, pc: u16) -> bool {
    !matches!(pb, 0x00 | 0x7E | 0x7F) && pc < 0x8000
}

fn trace_suspicious_control_flow(
    tag: &str,
    from_pb: u8,
    from_pc: u16,
    opcode: u8,
    to_pb: u8,
    to_pc: u16,
    sp_before: u16,
    extra: impl AsRef<str>,
) {
    if !crate::debug_flags::trace_cpu_suspicious_flow() || !is_suspicious_exec_target(to_pb, to_pc)
    {
        return;
    }
    use std::sync::atomic::{AtomicU32, Ordering};
    static COUNT: AtomicU32 = AtomicU32::new(0);
    if COUNT.fetch_add(1, Ordering::Relaxed) < 64 {
        println!(
            "[CPU-SUSP-{}] {:02X}:{:04X} op={:02X} -> {:02X}:{:04X} SP={:04X} {}",
            tag,
            from_pb,
            from_pc,
            opcode,
            to_pb,
            to_pc,
            sp_before,
            extra.as_ref()
        );
    }
}

#[inline(always)]
fn read_direct_address_generic<T: CpuBus>(state: &mut CoreState, bus: &mut T) -> (u32, u8) {
    let offset = read_u8_generic(state, bus) as u16;
    let penalty = if state.dp & 0x00FF != 0 { 1 } else { 0 };
    let addr = state.dp.wrapping_add(offset) as u32;
    (addr, penalty)
}

fn read_direct_x_address_generic<T: CpuBus>(state: &mut CoreState, bus: &mut T) -> (u32, u8) {
    let offset = read_u8_generic(state, bus) as u16;
    let penalty = if state.dp & 0x00FF != 0 { 1 } else { 0 };
    let addr = if state.emulation_mode && (state.dp & 0x00FF) == 0 {
        // 6502-style wrap within the direct page when D is page-aligned in emulation mode.
        let low = offset.wrapping_add(state.x & 0x00FF) & 0x00FF;
        ((state.dp & 0xFF00) | low) as u32
    } else {
        state.dp.wrapping_add(offset).wrapping_add(state.x) as u32
    };
    (addr, penalty)
}

fn read_direct_y_address_generic<T: CpuBus>(state: &mut CoreState, bus: &mut T) -> (u32, u8) {
    let offset = read_u8_generic(state, bus) as u16;
    let penalty = if state.dp & 0x00FF != 0 { 1 } else { 0 };
    let addr = if state.emulation_mode && (state.dp & 0x00FF) == 0 {
        // 6502-style wrap within the direct page when D is page-aligned in emulation mode.
        let low = offset.wrapping_add(state.y & 0x00FF) & 0x00FF;
        ((state.dp & 0xFF00) | low) as u32
    } else {
        state.dp.wrapping_add(offset).wrapping_add(state.y) as u32
    };
    (addr, penalty)
}

fn read_absolute_x_address_generic<T: CpuBus>(state: &mut CoreState, bus: &mut T) -> (u32, u8) {
    let base = read_u16_generic(state, bus);
    let low_sum = (base & 0x00FF) as u32 + (state.x & 0x00FF) as u32;
    let penalty = if low_sum >= 0x100 { 1 } else { 0 };
    // Absolute,X uses DBR for the bank. Indexing is applied to the full 24-bit address
    // (carry can propagate into the bank). This matters for WRAM $7E/$7F crossings.
    let base_full = ((state.db as u32) << 16) | (base as u32);
    let addr = base_full.wrapping_add(state.x as u32);
    (addr, penalty)
}

fn read_absolute_y_address_generic<T: CpuBus>(state: &mut CoreState, bus: &mut T) -> (u32, u8) {
    let base = read_u16_generic(state, bus);
    let low_sum = (base & 0x00FF) as u32 + (state.y & 0x00FF) as u32;
    let penalty = if low_sum >= 0x100 { 1 } else { 0 };
    // Absolute,Y uses DBR for the bank; indexing is applied to the full 24-bit address.
    let base_full = ((state.db as u32) << 16) | (base as u32);
    let addr = base_full.wrapping_add(state.y as u32);
    (addr, penalty)
}

fn read_absolute_long_address_generic<T: CpuBus>(state: &mut CoreState, bus: &mut T) -> u32 {
    read_u24_generic(state, bus)
}

fn read_absolute_long_x_address_generic<T: CpuBus>(state: &mut CoreState, bus: &mut T) -> u32 {
    let base = read_u24_generic(state, bus);
    base.wrapping_add(state.x as u32)
}

fn read_indirect_address_generic<T: CpuBus>(state: &mut CoreState, bus: &mut T) -> (u32, u8) {
    // (dp) - Direct Page Indirect
    // Operand is an 8-bit direct-page offset, not a 16-bit pointer operand.
    let base = read_u8_generic(state, bus) as u16;
    let penalty = if (state.dp & 0x00FF) != 0 { 1 } else { 0 };
    let ptr = state.dp.wrapping_add(base);
    let lo = bus.read_u8(ptr as u32) as u16;
    // Undocumented: in emulation mode, only a page-aligned D register exhibits 6502-style
    // wrapping for (dp) pointer reads.
    let hi_addr = if state.emulation_mode && (state.dp & 0x00FF) == 0 {
        (state.dp & 0xFF00) | ((base.wrapping_add(1)) & 0x00FF)
    } else {
        ptr.wrapping_add(1)
    };
    let hi = bus.read_u8(hi_addr as u32) as u16;
    let full = ((state.db as u32) << 16) | ((hi << 8) | lo) as u32;
    (full, penalty)
}

fn read_indirect_x_address_generic<T: CpuBus>(state: &mut CoreState, bus: &mut T) -> (u32, u8) {
    let base = read_u8_generic(state, bus) as u16;
    let penalty = if state.dp & 0x00FF != 0 { 1 } else { 0 };
    let addr = if state.emulation_mode && (state.dp & 0x00FF) == 0 {
        // 6502-style wrap within the direct page when D is page-aligned in emulation mode.
        let low = base.wrapping_add(state.x & 0x00FF) & 0x00FF;
        (state.dp & 0xFF00) | low
    } else {
        state.dp.wrapping_add(base).wrapping_add(state.x)
    };
    let lo = bus.read_u8(addr as u32) as u16;
    // In emulation mode, indirect pointer reads wrap within the direct page (6502-style).
    let hi_addr = if state.emulation_mode {
        (addr & 0xFF00) | (addr.wrapping_add(1) & 0x00FF)
    } else {
        addr.wrapping_add(1)
    };
    let hi = bus.read_u8(hi_addr as u32) as u16;
    let full = ((state.db as u32) << 16) | ((hi << 8) | lo) as u32;
    (full, penalty)
}

fn read_indirect_y_address_generic<T: CpuBus>(state: &mut CoreState, bus: &mut T) -> (u32, u8) {
    let base = read_u8_generic(state, bus) as u16;
    let mut penalty = 0u8;
    if state.dp & 0x00FF != 0 {
        penalty = penalty.saturating_add(1);
    }
    let addr = state.dp.wrapping_add(base);
    let lo = bus.read_u8(addr as u32) as u16;
    // Undocumented: in emulation mode, only a page-aligned D register exhibits 6502-style
    // wrapping for (dp),Y pointer reads.
    let hi_addr = if state.emulation_mode && (state.dp & 0x00FF) == 0 {
        (state.dp & 0xFF00) | ((base.wrapping_add(1)) & 0x00FF)
    } else {
        addr.wrapping_add(1)
    };
    let hi = bus.read_u8(hi_addr as u32) as u16;
    let base16 = (hi << 8) | lo;
    if ((base16 & 0x00FF) as u32) + (state.y & 0x00FF) as u32 >= 0x100 {
        penalty = penalty.saturating_add(1);
    }
    // (dp),Y uses DBR for the bank; indexing is applied to the full 24-bit address.
    let base_full = ((state.db as u32) << 16) | (base16 as u32);
    let full = base_full.wrapping_add(state.y as u32);
    (full, penalty)
}

fn read_indirect_long_address_generic<T: CpuBus>(state: &mut CoreState, bus: &mut T) -> (u32, u8) {
    let pointer = read_u8_generic(state, bus) as u16;
    let mut penalty = 0u8;
    if state.dp & 0x00FF != 0 {
        penalty = penalty.saturating_add(1);
    }
    let addr = state.dp.wrapping_add(pointer);
    let lo = bus.read_u8(addr as u32) as u32;
    let mid = bus.read_u8(addr.wrapping_add(1) as u32) as u32;
    let hi = bus.read_u8(addr.wrapping_add(2) as u32) as u32;
    ((hi << 16) | (mid << 8) | lo, penalty)
}

fn read_indirect_long_y_address_generic<T: CpuBus>(
    state: &mut CoreState,
    bus: &mut T,
) -> (u32, u8) {
    let pointer = read_u8_generic(state, bus) as u16;
    let mut penalty = 0u8;
    if state.dp & 0x00FF != 0 {
        penalty = penalty.saturating_add(1);
    }
    let addr = state.dp.wrapping_add(pointer);
    let lo = bus.read_u8(addr as u32) as u32;
    let mid = bus.read_u8(addr.wrapping_add(1) as u32) as u32;
    let hi = bus.read_u8(addr.wrapping_add(2) as u32) as u32;
    let full = (hi << 16) | (mid << 8) | lo;
    (full.wrapping_add(state.y as u32), penalty)
}

fn read_stack_relative_address_generic<T: CpuBus>(state: &mut CoreState, bus: &mut T) -> u32 {
    let offset = read_u8_generic(state, bus) as u16;
    state.sp.wrapping_add(offset) as u32
}

fn read_stack_relative_indirect_y_generic<T: CpuBus>(
    state: &mut CoreState,
    bus: &mut T,
) -> (u32, u8) {
    let offset = read_u8_generic(state, bus) as u16;
    let addr = state.sp.wrapping_add(offset);
    let lo = bus.read_u8(addr as u32) as u16;
    let hi = bus.read_u8(addr.wrapping_add(1) as u32) as u16;
    let base16 = (hi << 8) | lo;
    let mut penalty = 0u8;
    if ((base16 & 0x00FF) as u32) + (state.y & 0x00FF) as u32 >= 0x100 {
        penalty = penalty.saturating_add(1);
    }
    // (sr),Y uses DBR for the bank; indexing is applied to the full 24-bit address.
    let base_full = ((state.db as u32) << 16) | (base16 as u32);
    let full = base_full.wrapping_add(state.y as u32);
    (full, penalty)
}

#[inline]
fn memory_is_8bit(state: &CoreState) -> bool {
    state.emulation_mode || state.p.contains(StatusFlags::MEMORY_8BIT)
}

#[inline]
fn index_is_8bit(state: &CoreState) -> bool {
    state.emulation_mode || state.p.contains(StatusFlags::INDEX_8BIT)
}

fn write_a_generic<T: CpuBus>(state: &CoreState, bus: &mut T, addr: u32) {
    if memory_is_8bit(state) {
        bus.write_u8(addr, (state.a & 0xFF) as u8);
    } else {
        bus.write_u16(addr, state.a);
    }
}

fn write_x_generic<T: CpuBus>(state: &CoreState, bus: &mut T, addr: u32) {
    if index_is_8bit(state) {
        bus.write_u8(addr, (state.x & 0xFF) as u8);
    } else {
        bus.write_u16(addr, state.x);
    }
}

fn write_y_generic<T: CpuBus>(state: &CoreState, bus: &mut T, addr: u32) {
    if index_is_8bit(state) {
        bus.write_u8(addr, (state.y & 0xFF) as u8);
    } else {
        bus.write_u16(addr, state.y);
    }
}

fn set_flags_index(state: &mut CoreState, value: u16) {
    if index_is_8bit(state) {
        set_flags_nz_8(state, (value & 0xFF) as u8);
    } else {
        set_flags_nz_16(state, value);
    }
}

fn cmp_operand(state: &mut CoreState, operand: u16) {
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

fn read_operand_m<T: CpuBus>(_state: &CoreState, bus: &mut T, addr: u32, memory_8bit: bool) -> u16 {
    if memory_8bit {
        bus.read_u8(addr) as u16
    } else {
        bus.read_u16(addr)
    }
}

fn ora_operand(state: &mut CoreState, operand: u16) {
    if memory_is_8bit(state) {
        let result = ((state.a & 0xFF) | (operand & 0xFF)) as u8;
        state.a = (state.a & 0xFF00) | (result as u16);
        set_flags_nz_8(state, result);
    } else {
        state.a |= operand;
        set_flags_nz_16(state, state.a);
    }
}

fn and_operand(state: &mut CoreState, operand: u16) {
    if memory_is_8bit(state) {
        let result = ((state.a & 0xFF) & (operand & 0xFF)) as u8;
        state.a = (state.a & 0xFF00) | (result as u16);
        set_flags_nz_8(state, result);
    } else {
        state.a &= operand;
        set_flags_nz_16(state, state.a);
    }
}

fn eor_operand(state: &mut CoreState, operand: u16) {
    if memory_is_8bit(state) {
        let result = ((state.a & 0xFF) ^ (operand & 0xFF)) as u8;
        state.a = (state.a & 0xFF00) | (result as u16);
        set_flags_nz_8(state, result);
    } else {
        state.a ^= operand;
        set_flags_nz_16(state, state.a);
    }
}

fn modify_memory<T: CpuBus, F8, F16>(
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

fn asl8(state: &mut CoreState, value: u8) -> u8 {
    state.p.set(StatusFlags::CARRY, value & 0x80 != 0);
    let result = value << 1;
    set_flags_nz_8(state, result);
    result
}

fn asl16(state: &mut CoreState, value: u16) -> u16 {
    state.p.set(StatusFlags::CARRY, value & 0x8000 != 0);
    let result = value << 1;
    set_flags_nz_16(state, result);
    result
}

fn lsr8(state: &mut CoreState, value: u8) -> u8 {
    state.p.set(StatusFlags::CARRY, value & 0x01 != 0);
    let result = value >> 1;
    set_flags_nz_8(state, result);
    result
}

fn lsr16(state: &mut CoreState, value: u16) -> u16 {
    state.p.set(StatusFlags::CARRY, value & 0x0001 != 0);
    let result = value >> 1;
    set_flags_nz_16(state, result);
    result
}

fn rol8(state: &mut CoreState, value: u8) -> u8 {
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

fn rol16(state: &mut CoreState, value: u16) -> u16 {
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

fn ror8(state: &mut CoreState, value: u8) -> u8 {
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

fn ror16(state: &mut CoreState, value: u16) -> u16 {
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

fn bit_operand_immediate(state: &mut CoreState, operand: u16) {
    // BIT immediate affects Z only. N/V are not modified.
    bit_set_z(state, operand);
}

fn bit_operand_memory(state: &mut CoreState, operand: u16) {
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

fn apply_status_side_effects_after_pull(state: &mut CoreState, prev_p: StatusFlags) {
    // In emulation mode, M/X are forced to 1.
    if state.emulation_mode {
        state
            .p
            .insert(StatusFlags::MEMORY_8BIT | StatusFlags::INDEX_8BIT);
        return;
    }

    // If X flag changed 0->1 (16-bit -> 8-bit), high bytes of X/Y are cleared.
    let prev_x_16 = !prev_p.contains(StatusFlags::INDEX_8BIT);
    let new_x_16 = !state.p.contains(StatusFlags::INDEX_8BIT);
    if prev_x_16 && !new_x_16 {
        state.x &= 0x00FF;
        state.y &= 0x00FF;
    }
}

fn branch_if_generic<T: CpuBus>(state: &mut CoreState, bus: &mut T, condition: bool) -> u8 {
    let offset = read_u8_generic(state, bus) as i8;
    let pc_before = state.pc;
    if condition {
        let new_pc = state.pc.wrapping_add(offset as u16);
        state.pc = new_pc;
        let mut total_cycles = 3u8;
        if (pc_before & 0xFF00) != (new_pc & 0xFF00) {
            total_cycles = total_cycles.saturating_add(1);
        }
        // read_u8_generic already accounted for one cycle
        add_cycles(state, total_cycles.saturating_sub(1));
        if crate::debug_flags::debug_branch()
            && state.pb == 0x00
            && (0x8240..=0x82A0).contains(&pc_before)
        {
            println!(
                "[BRANCH] pc_before={:04X} pc_after={:04X} offset={:02X} P=0x{:02X} taken=true",
                pc_before,
                new_pc,
                offset as u8,
                state.p.bits()
            );
            if crate::debug_flags::exit_on_branch_neg() && state.p.contains(StatusFlags::NEGATIVE) {
                println!("[EXIT_ON_BRANCH_NEG] triggered");
                std::process::exit(0);
            }
        }
        total_cycles
    } else {
        // Not taken branch is 2 cycles total
        add_cycles(state, 1); // one more cycle beyond operand fetch
        if crate::debug_flags::debug_branch()
            && state.pb == 0x00
            && (0x8240..=0x82A0).contains(&pc_before)
        {
            println!(
                "[BRANCH] pc_before={:04X} pc_after={:04X} offset={:02X} P=0x{:02X} taken=false",
                pc_before,
                state.pc,
                offset as u8,
                state.p.bits()
            );
        }
        2
    }
}

fn brl_generic<T: CpuBus>(state: &mut CoreState, bus: &mut T) -> u8 {
    let offset = read_u16_generic(state, bus) as i16;
    let old_pc = state.pc;
    let new_pc = state.pc.wrapping_add(offset as u16);
    state.pc = new_pc;
    let mut total_cycles = 4u8;
    if (old_pc & 0xFF00) != (new_pc & 0xFF00) {
        total_cycles = total_cycles.saturating_add(1);
    }
    // read_u16_generic already accounted for 3 cycles (2 for read + 1 for add below)
    add_cycles(state, total_cycles.saturating_sub(2));
    total_cycles
}

fn per_generic<T: CpuBus>(state: &mut CoreState, bus: &mut T) -> u8 {
    let offset = read_u16_generic(state, bus) as i16;
    let value = state.pc.wrapping_add(offset as u16);
    if state.emulation_mode {
        push_u16_emulation_edge(state, bus, value);
    } else {
        push_u16_generic(state, bus, value);
    }
    let total_cycles: u8 = 6;
    // read_u16_generic accounted for 2 cycles, push_u16 added 2 cycles
    add_cycles(state, total_cycles.saturating_sub(4));
    total_cycles
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

fn adc_generic(state: &mut CoreState, operand: u16) {
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

fn sbc_generic(state: &mut CoreState, operand: u16) {
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

// Generic instruction implementations

fn jsr_generic<T: CpuBus>(state: &mut CoreState, bus: &mut T) -> u8 {
    let addr = read_absolute_address_generic(state, bus);
    if crate::debug_flags::trace_jsr_stack() {
        println!(
            "[JSR] PB={:02X} PC={:04X} SP={:04X} push_ret={:04X}",
            state.pb,
            state.pc.wrapping_sub(2),
            state.sp,
            state.pc
        );
    }
    if crate::debug_flags::trace_jsl() || crate::debug_flags::trace_pb_calls() {
        println!(
            "PB_CALL JSR from {:02X}:{:04X} PB={:02X} DB={:02X} SP={:04X} target={:04X}",
            state.pb,
            state.pc.wrapping_sub(2),
            state.pb,
            state.db,
            state.sp,
            addr
        );
    }
    push_u16_generic(state, bus, state.pc.wrapping_sub(1));
    state.pc = (addr & 0xFFFF) as u16;
    6
}

fn rts_generic<T: CpuBus>(state: &mut CoreState, bus: &mut T) -> u8 {
    if crate::debug_flags::trace_rts_detail() {
        let base = if state.emulation_mode {
            0x0100u32 | state.sp as u32
        } else {
            state.sp as u32
        };
        let lo = bus.read_u8(base.wrapping_add(1));
        let hi = bus.read_u8(base.wrapping_add(2));
        println!(
            "[RTS-PEEK] PB={:02X} PC={:04X} SP={:04X} peek={:04X} (bytes={:02X} {:02X}) emu={}",
            state.pb,
            state.pc.wrapping_sub(1),
            state.sp,
            ((hi as u16) << 8) | lo as u16,
            lo,
            hi,
            state.emulation_mode
        );
    }
    if crate::debug_flags::trace_rts_pop() {
        // Peek return address before popping
        let sp = state.sp;
        let base = if state.emulation_mode {
            0x0100u32 | sp as u32
        } else {
            sp as u32
        };
        let pcl = bus.read_u8(base.wrapping_add(1));
        let pch = bus.read_u8(base.wrapping_add(2));
        // Dump top 8 bytes of stack for corruption trace
        let mut bytes = [0u8; 8];
        for (i, byte) in bytes.iter_mut().enumerate() {
            *byte = bus.read_u8(base.wrapping_add(i as u32 + 1));
        }
        println!(
            "[RTS] PB={:02X} PC={:04X} SP={:04X} -> ret {:02X}:{:02X}{:02X} stack={:?}",
            state.pb,
            state.pc.wrapping_sub(1),
            state.sp,
            state.pb,
            pch,
            pcl,
            bytes
        );
    }
    // COMPAT: 特定の RTS を RTL 相当で扱う（Mario 初期化の誤帰還防止）
    // 有効化: COMPAT_RTS_AS_RTL_8D7F=1 または COMPAT_MARIO_RTS_AS_RTL=1
    let cur_op_addr = state.pc.wrapping_sub(1); // 実行中オペコードのアドレス
                                                // SMW/Super Mario Collection 専用: 8D7FのRTSをRTL扱い＋スタック誤帰還を補正
    let compat_rts_as_rtl_8d7f = crate::debug_flags::compat_rts_as_rtl_8d7f()
        || crate::debug_flags::compat_mario_rts_as_rtl();
    if compat_rts_as_rtl_8d7f && state.pb == 0x00 && cur_op_addr == 0x8D7F {
        // RTL: pop 16-bit PC then bank
        let mut addr = pop_u16_generic(state, bus);
        let mut bank = pop_u8_generic(state, bus);

        // Mario specific stack corruption guard:
        // もし誤って 00:805F が積まれていた場合は、本来戻るべき 8CBA へ強制修正する。
        let compat_fix = crate::debug_flags::compat_mario_rts_fix();
        if compat_fix && addr == 0x805F && bank == 0x00 {
            addr = 0x8CBA;
            bank = 0x00;
        }

        if crate::debug_flags::trace_rts_addr() {
            println!(
                "[RTS->RTL] PB={:02X} popped={:02X}:{:04X} -> next={:02X}:{:04X} SP={:04X}",
                state.pb,
                bank,
                addr,
                bank,
                addr.wrapping_add(1),
                state.sp
            );
        }
        state.pb = bank;
        state.pc = addr.wrapping_add(1);
        6
    } else {
        let addr = pop_u16_generic(state, bus);
        if crate::debug_flags::trace_rts_addr() {
            println!(
                "[RTS-POP] PB={:02X} popped={:04X} -> next={:04X} SP={:04X}",
                state.pb,
                addr,
                addr.wrapping_add(1),
                state.sp
            );
        }
        state.pc = addr.wrapping_add(1);
        6
    }
}

fn jsl_generic<T: CpuBus>(state: &mut CoreState, bus: &mut T) -> u8 {
    let addr_lo = bus.read_u8(full_address(state, state.pc)) as u32;
    let addr_hi = bus.read_u8(full_address(state, state.pc + 1)) as u32;
    let addr_bank = bus.read_u8(full_address(state, state.pc + 2)) as u32;
    let target = addr_lo | (addr_hi << 8) | (addr_bank << 16);
    state.pc = state.pc.wrapping_add(3);

    if crate::debug_flags::trace_pb_calls() || crate::debug_flags::trace_jsl() {
        let op0 = addr_lo as u8;
        let op1 = addr_hi as u8;
        let op2 = addr_bank as u8;
        println!(
            "PB_CALL JSL from {:02X}:{:04X} PB={:02X} DB={:02X} SP={:04X} target={:06X} op=[{:02X} {:02X} {:02X}]",
            state.pb,
            state.pc.wrapping_sub(3),
            state.pb,
            state.db,
            state.sp,
            target,
            op0,
            op1,
            op2
        );
    }

    let ret = state.pc.wrapping_sub(1);
    if state.emulation_mode {
        // Undocumented emulation edge: JSL pushes 3 bytes using a 16-bit stack decrement.
        // This can write outside $0100-$01FF when SP starts at $0100.
        bus.write_u8(state.sp as u32, state.pb);
        state.sp = state.sp.wrapping_sub(1);
        add_cycles(state, 1);
        bus.write_u8(state.sp as u32, (ret >> 8) as u8);
        state.sp = state.sp.wrapping_sub(1);
        add_cycles(state, 1);
        bus.write_u8(state.sp as u32, (ret & 0xFF) as u8);
        state.sp = state.sp.wrapping_sub(1);
        add_cycles(state, 1);
        // Re-assert emulation-mode stack high byte after the sequence.
        state.sp = 0x0100 | (state.sp & 0x00FF);
    } else {
        push_u8_generic(state, bus, state.pb);
        push_u16_generic(state, bus, ret);
    }

    state.pb = (target >> 16) as u8;
    state.pc = (target & 0xFFFF) as u16;
    8
}

fn rtl_generic<T: CpuBus>(state: &mut CoreState, bus: &mut T) -> u8 {
    let from_pb = state.pb;
    let from_pc = state.pc.wrapping_sub(1);
    let sp_before = state.sp;
    if crate::debug_flags::trace_pb_calls() || crate::debug_flags::trace_rtl() {
        // peek stack before pop
        let sp = state.sp;
        let sp_addr = if state.emulation_mode {
            0x0100u32 | (sp as u32)
        } else {
            sp as u32
        };
        let pcl = bus.read_u8(sp_addr.wrapping_add(1));
        let pch = bus.read_u8(sp_addr.wrapping_add(2));
        let pb = bus.read_u8(sp_addr.wrapping_add(3));
        println!(
            "PB_CALL RTL pull {:02X}:{:04X} SP={:04X} ret={:02X}:{:02X}{:02X}",
            state.pb, state.pc, state.sp, pb, pch, pcl
        );
    }

    let (addr, pb) = if state.emulation_mode {
        // Undocumented emulation edge: RTL pulls 3 bytes using a 16-bit stack increment.
        // This can read from $0200.. when SP starts at $01FF.
        state.sp = state.sp.wrapping_add(1);
        let lo = bus.read_u8(state.sp as u32) as u16;
        add_cycles(state, 1);
        state.sp = state.sp.wrapping_add(1);
        let hi = bus.read_u8(state.sp as u32) as u16;
        add_cycles(state, 1);
        state.sp = state.sp.wrapping_add(1);
        let pb = bus.read_u8(state.sp as u32);
        add_cycles(state, 1);
        // Re-assert emulation-mode stack high byte after the sequence.
        state.sp = 0x0100 | (state.sp & 0x00FF);
        ((hi << 8) | lo, pb)
    } else {
        (pop_u16_generic(state, bus), pop_u8_generic(state, bus))
    };
    state.pb = pb;
    state.pc = addr.wrapping_add(1);
    trace_suspicious_control_flow(
        "RTL",
        from_pb,
        from_pc,
        0x6B,
        state.pb,
        state.pc,
        sp_before,
        format!("popped={:02X}:{:04X}", pb, addr),
    );
    6
}

fn rep_generic<T: CpuBus>(state: &mut CoreState, bus: &mut T) -> u8 {
    let mask = bus.read_u8(full_address(state, state.pc));
    state.pc = state.pc.wrapping_add(1);
    let new_flags = StatusFlags::from_bits_truncate(state.p.bits() & !mask);
    state.p = new_flags;
    // Emulation mode forces M/X=1; REP cannot clear them effectively.
    if state.emulation_mode {
        state
            .p
            .insert(StatusFlags::MEMORY_8BIT | StatusFlags::INDEX_8BIT);
    }
    if crate::debug_flags::trace_mflag() {
        println!(
            "[MFLAG] PC={:02X}:{:04X} REP #{:02X} -> P={:02X} emu={} A={:04X} X={:04X} Y={:04X}",
            state.pb,
            state.pc.wrapping_sub(1),
            mask,
            state.p.bits(),
            state.emulation_mode,
            state.a,
            state.x,
            state.y
        );
    }
    add_cycles(state, 3);
    3
}

fn sep_generic<T: CpuBus>(state: &mut CoreState, bus: &mut T) -> u8 {
    let mask = read_u8_generic(state, bus);
    let prev_flags = state.p;
    let mut new_flags = StatusFlags::from_bits_truncate(prev_flags.bits() | mask);
    if state.emulation_mode {
        new_flags.insert(StatusFlags::MEMORY_8BIT | StatusFlags::INDEX_8BIT);
    }
    let prev_x_16 = !prev_flags.contains(StatusFlags::INDEX_8BIT) && !state.emulation_mode;
    let new_x_16 = !new_flags.contains(StatusFlags::INDEX_8BIT) && !state.emulation_mode;
    state.p = new_flags;
    if prev_x_16 && !new_x_16 {
        state.x &= 0x00FF;
        state.y &= 0x00FF;
    }
    // Accumulator upper byte (B) is preserved across M width changes.

    if crate::debug_flags::trace_mflag() {
        println!(
            "[MFLAG] PC={:02X}:{:04X} SEP #{:02X} -> P={:02X} emu={} A={:04X} X={:04X} Y={:04X}",
            state.pb,
            state.pc.wrapping_sub(1),
            mask,
            state.p.bits(),
            state.emulation_mode,
            state.a,
            state.x,
            state.y
        );
    }
    add_cycles(state, 2);
    3
}

// Main generic instruction execution function
pub fn execute_instruction_generic<T: CpuBus>(
    state: &mut CoreState,
    opcode: u8,
    bus: &mut T,
) -> u8 {
    // Debug: log first few iterations of the SMW APU upload loop to see what it waits for.
    if crate::debug_flags::trace_smw_apu_loop()
        && !crate::debug_flags::quiet()
        && state.pb == 0x00
        && (state.pc == 0x8BC5 || state.pc == 0x8BB2)
    {
        use std::sync::atomic::{AtomicU32, Ordering};
        static HITS: AtomicU32 = AtomicU32::new(0);
        let n = HITS.fetch_add(1, Ordering::Relaxed);
        if n < 32 {
            let p0 = bus.read_u8(0x2140);
            let p1 = bus.read_u8(0x2141);
            println!(
                "[SMW-APU-LOOP {:02}] p0={:02X} p1={:02X} X={:04X} Y={:04X}",
                n + 1,
                p0,
                p1,
                state.x,
                state.y
            );
        }
    }

    // PC watch hook (config: WATCH_PC=7DB6 or WATCH_PC=00:7DB6,7DC0,...)
    if let Some(list) = crate::debug_flags::watch_pc_list() {
        let full = ((state.pb as u32) << 16) | (state.pc as u32);
        if list.binary_search(&full).is_ok() || list.binary_search(&(state.pc as u32)).is_ok() {
            println!(
                "WATCH_PC hit at {:02X}:{:04X} A={:04X} X={:04X} Y={:04X} SP={:04X} D={:04X} DB={:02X} P={:02X}",
                state.pb,
                state.pc,
                state.a,
                state.x,
                state.y,
                state.sp,
                state.dp,
                state.db,
                state.p.bits()
            );
            // 直近のDPポインタ先頭16バイトをダンプして、間接参照の行方を追う
            let dbase = state.dp as u32;
            for i in 0..4u32 {
                let addr = dbase + i * 4;
                let b0 = bus.read_u8(addr);
                let b1 = bus.read_u8(addr + 1);
                let b2 = bus.read_u8(addr + 2);
                let b3 = bus.read_u8(addr + 3);
                println!(
                    "  DP+{:02X}: {:02X} {:02X} {:02X} {:02X}",
                    i * 4,
                    b0,
                    b1,
                    b2,
                    b3
                );
            }
        }
    }

    match opcode {
        // Interrupt instructions - Essential for proper CPU operation
        0x00 => {
            let from_pb = state.pb;
            let from_pc = state.pc.wrapping_sub(1);
            let sp_before = state.sp;
            if crate::debug_flags::trace_brk() {
                println!(
                    "[BRK] at {:02X}:{:04X} A={:04X} X={:04X} Y={:04X} SP={:04X} P={:02X}",
                    from_pb,
                    state.pc,
                    state.a,
                    state.x,
                    state.y,
                    sp_before,
                    state.p.bits()
                );
            }
            if state.brk_is_nop {
                // Treat BRK as NOP (debug)
                add_cycles(state, 2);
                2
            } else {
                // BRK - Software Interrupt
                // BRK pushes PC+2 and status register, then jumps to BRK vector
                let next_pc = state.pc.wrapping_add(1); // BRK has a dummy operand byte
                state.pc = next_pc;

                // Push program bank (only in native mode)
                if !state.emulation_mode {
                    push_u8_generic(state, bus, state.pb);
                }

                // Push return address (PC after BRK + 1)
                push_u16_generic(state, bus, next_pc);

                // Push status register:
                // - Native mode: push P as-is (bits 4/5 are X/M)
                // - Emulation mode: push with B=1 and bit5 forced 1
                let status_to_push = if state.emulation_mode {
                    state.p.bits() | 0x30
                } else {
                    state.p.bits()
                };
                push_u8_generic(state, bus, status_to_push);

                // Set interrupt disable flag and clear decimal mode (65C816 behavior)
                state.p.insert(StatusFlags::IRQ_DISABLE);
                state.p.remove(StatusFlags::DECIMAL);

                // Jump to BRK vector
                let vector_addr = if state.emulation_mode { 0xFFFE } else { 0xFFE6 };
                let vector = bus.read_u16(vector_addr);
                state.pc = vector;

                // Interrupt vectors are always in bank 00
                state.pb = 0;
                trace_suspicious_control_flow(
                    "BRK",
                    from_pb,
                    from_pc,
                    0x00,
                    state.pb,
                    state.pc,
                    sp_before,
                    format!("vector={:04X} next_pc={:04X}", vector, next_pc),
                );

                add_cycles(state, if state.emulation_mode { 7 } else { 8 });
                if state.emulation_mode {
                    7
                } else {
                    8
                }
            }
        }

        0x02 => {
            // COP - Co-Processor Enable (software interrupt)
            let _signature = read_u8_generic(state, bus);
            let return_pc = state.pc;
            // - Native mode: push P as-is (bits 4/5 are X/M)
            // - Emulation mode: push with B=1 and bit5 forced 1 (6502-style)
            let pushed_status = if state.emulation_mode {
                state.p.bits() | 0x30
            } else {
                state.p.bits()
            };

            if state.emulation_mode {
                push_u16_generic(state, bus, return_pc);
                push_u8_generic(state, bus, pushed_status);
                let accounted = 1 + 3; // operand fetch + pushes (3 cycles)
                add_cycles(state, 7 - accounted);
            } else {
                push_u8_generic(state, bus, state.pb);
                push_u16_generic(state, bus, return_pc);
                push_u8_generic(state, bus, pushed_status);
                let accounted = 1 + 4; // operand fetch + pushes (4 cycles)
                add_cycles(state, 7 - accounted);
            }

            state.p.insert(StatusFlags::IRQ_DISABLE);
            state.p.remove(StatusFlags::DECIMAL);
            state.pb = 0;
            let vector_addr = if state.emulation_mode { 0xFFF4 } else { 0xFFE4 };
            let vector = bus.read_u16(vector_addr as u32);
            state.pc = vector;
            7
        }

        // Additional instructions needed by SA-1 test cases
        0x20 => jsr_generic(state, bus), // JSR absolute
        0x22 => jsl_generic(state, bus), // JSL long
        0x60 => rts_generic(state, bus), // RTS
        0x62 => per_generic(state, bus), // PER push effective relative address
        0x6B => rtl_generic(state, bus), // RTL
        0xC2 => rep_generic(state, bus), // REP
        0xE2 => sep_generic(state, bus), // SEP

        // Simple instructions that don't need bus access
        0xEA => {
            // NOP
            add_cycles(state, 2);
            2
        }
        0x18 => {
            // CLC
            state.p.remove(StatusFlags::CARRY);
            add_cycles(state, 2);
            2
        }

        0x1A => {
            // INC A
            if memory_is_8bit(state) {
                let value = ((state.a & 0xFF).wrapping_add(1)) as u8;
                state.a = (state.a & 0xFF00) | (value as u16);
                set_flags_nz_8(state, value);
            } else {
                state.a = state.a.wrapping_add(1);
                set_flags_nz_16(state, state.a);
            }
            add_cycles(state, 2);
            2
        }

        0x38 => {
            // SEC
            state.p.insert(StatusFlags::CARRY);
            add_cycles(state, 2);
            2
        }

        0x44 => {
            // MVP (Block Move Positive)
            // Operand order in object code: dest bank then src bank
            let dest_bank = read_u8_generic(state, bus);
            let src_bank = read_u8_generic(state, bus);
            // DBR becomes destination bank.
            state.db = dest_bank;
            let x_addr = if index_is_8bit(state) {
                state.x & 0x00FF
            } else {
                state.x
            };
            let y_addr = if index_is_8bit(state) {
                state.y & 0x00FF
            } else {
                state.y
            };
            let src_addr = ((src_bank as u32) << 16) | (x_addr as u32);
            let dest_addr = ((dest_bank as u32) << 16) | (y_addr as u32);
            let value = bus.read_u8(src_addr);
            bus.write_u8(dest_addr, value);
            if index_is_8bit(state) {
                state.x = (state.x & 0xFF00) | ((state.x as u8).wrapping_sub(1) as u16);
                state.y = (state.y & 0xFF00) | ((state.y as u8).wrapping_sub(1) as u16);
            } else {
                state.x = state.x.wrapping_sub(1);
                state.y = state.y.wrapping_sub(1);
            }
            state.a = state.a.wrapping_sub(1);
            if state.a != 0xFFFF {
                state.pc = state.pc.wrapping_sub(3);
            }
            let base_cycles: u8 = 7;
            let already_accounted: u8 = 2; // two immediate bytes already consumed
            add_cycles(state, base_cycles.saturating_sub(already_accounted));
            base_cycles
        }

        0x54 => {
            // MVN (Block Move Negative)
            // Operand order in object code: dest bank then src bank
            let dest_bank = read_u8_generic(state, bus);
            let src_bank = read_u8_generic(state, bus);
            // DBR becomes destination bank.
            state.db = dest_bank;
            let x_addr = if index_is_8bit(state) {
                state.x & 0x00FF
            } else {
                state.x
            };
            let y_addr = if index_is_8bit(state) {
                state.y & 0x00FF
            } else {
                state.y
            };
            let src_addr = ((src_bank as u32) << 16) | (x_addr as u32);
            let dest_addr = ((dest_bank as u32) << 16) | (y_addr as u32);
            let value = bus.read_u8(src_addr);
            bus.write_u8(dest_addr, value);
            if index_is_8bit(state) {
                state.x = (state.x & 0xFF00) | ((state.x as u8).wrapping_add(1) as u16);
                state.y = (state.y & 0xFF00) | ((state.y as u8).wrapping_add(1) as u16);
            } else {
                state.x = state.x.wrapping_add(1);
                state.y = state.y.wrapping_add(1);
            }
            state.a = state.a.wrapping_sub(1);
            if state.a != 0xFFFF {
                state.pc = state.pc.wrapping_sub(3);
            }
            let base_cycles: u8 = 7;
            let already_accounted: u8 = 2;
            add_cycles(state, base_cycles.saturating_sub(already_accounted));
            base_cycles
        }
        0x78 => {
            // SEI
            state.p.insert(StatusFlags::IRQ_DISABLE);
            add_cycles(state, 2);
            2
        }
        0xFB => {
            // XCE
            // C と E を入れ替える（C→新E, 旧E→新C）
            let old_emulation = state.emulation_mode;
            let new_emulation = state.p.contains(StatusFlags::CARRY);
            state.p.set(StatusFlags::CARRY, old_emulation);
            state.emulation_mode = new_emulation;
            if state.emulation_mode {
                // E=1 に入るときは M/X=1 を強制し、X/Y 上位をクリア、SP の上位バイトを 0x01 にする
                state
                    .p
                    .insert(StatusFlags::MEMORY_8BIT | StatusFlags::INDEX_8BIT);
                state.x &= 0x00FF;
                state.y &= 0x00FF;
                state.sp = (state.sp & 0x00FF) | 0x0100;
            }
            add_cycles(state, 2);
            2
        }

        // Jump instructions
        0x4C => {
            // JMP absolute
            let addr = read_u16_generic(state, bus);
            state.pc = addr;
            3
        }
        0x5C => {
            // JML long
            let addr_lo = bus.read_u8(full_address(state, state.pc)) as u32;
            let addr_hi = bus.read_u8(full_address(state, state.pc.wrapping_add(1))) as u32;
            let addr_bank = bus.read_u8(full_address(state, state.pc.wrapping_add(2))) as u32;
            let target = addr_lo | (addr_hi << 8) | (addr_bank << 16);
            state.pb = (target >> 16) as u8;
            state.pc = (target & 0xFFFF) as u16;
            add_cycles(state, 4);
            4
        }
        0x6C => {
            // JMP (addr)
            let ptr = read_u16_generic(state, bus);
            // Indirect pointer fetch is from bank 00 (not PB).
            // Also preserves the 6502 page-wrap bug when ptr ends in 0xFF.
            let lo = bus.read_u8(ptr as u32) as u16;
            let hi_addr = (ptr & 0xFF00) | (ptr.wrapping_add(1) & 0x00FF);
            let hi = bus.read_u8(hi_addr as u32) as u16;
            let target = lo | (hi << 8);
            state.pc = target;
            add_cycles(state, 5 - 2);
            5
        }
        0x7C => {
            // JMP (addr,X)
            let base = read_u16_generic(state, bus);
            let ptr = base.wrapping_add(state.x);
            let target = bus.read_u16(full_address(state, ptr));
            state.pc = target;
            add_cycles(state, 6 - 2);
            6
        }
        0xDC => {
            // JMP [addr]
            let ptr = read_u16_generic(state, bus);
            // Indirect long pointer fetch is from bank 00.
            let base = ptr as u32;
            let lo = bus.read_u8(base) as u32;
            let mid = bus.read_u8((ptr.wrapping_add(1)) as u32) as u32;
            let hi = bus.read_u8((ptr.wrapping_add(2)) as u32) as u32;
            let target = (hi << 16) | (mid << 8) | lo;
            state.pb = ((target >> 16) & 0xFF) as u8;
            state.pc = (target & 0xFFFF) as u16;
            add_cycles(state, 6 - 2);
            6
        }

        // ORA logical OR operations
        0x04 => {
            // TSB direct page
            let (addr, penalty) = read_direct_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = memory_is_8bit(state);
            if memory_8bit {
                let value = bus.read_u8(addr);
                let a_low = (state.a & 0xFF) as u8;
                state.p.set(StatusFlags::ZERO, (value & a_low) == 0);
                bus.write_u8(addr, value | a_low);
            } else {
                let value = bus.read_u16(addr);
                state.p.set(StatusFlags::ZERO, (value & state.a) == 0);
                bus.write_u16(addr, value | state.a);
            }
            let base_cycles: u8 = if memory_8bit { 5 } else { 6 };
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x05 => {
            // ORA direct page
            let (addr, penalty) = read_direct_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = memory_is_8bit(state);
            let operand = read_operand_m(state, bus, addr, memory_8bit);
            ora_operand(state, operand);
            let base_cycles: u8 = 3;
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x15 => {
            // ORA direct page,X
            let (addr, penalty) = read_direct_x_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = memory_is_8bit(state);
            let operand = read_operand_m(state, bus, addr, memory_8bit);
            ora_operand(state, operand);
            let base_cycles: u8 = 4;
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x0D => {
            // ORA absolute
            let addr = read_absolute_address_generic(state, bus);
            let memory_8bit = memory_is_8bit(state);
            let operand = read_operand_m(state, bus, addr, memory_8bit);
            ora_operand(state, operand);
            let base_cycles: u8 = 4;
            let already_accounted: u8 = 2;
            add_cycles(state, base_cycles.saturating_sub(already_accounted));
            base_cycles
        }

        0x1D => {
            // ORA absolute,X
            let (addr, penalty) = read_absolute_x_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = memory_is_8bit(state);
            let operand = read_operand_m(state, bus, addr, memory_8bit);
            ora_operand(state, operand);
            let base_cycles: u8 = 4;
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 2 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x19 => {
            // ORA absolute,Y
            let (addr, penalty) = read_absolute_y_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = memory_is_8bit(state);
            let operand = read_operand_m(state, bus, addr, memory_8bit);
            ora_operand(state, operand);
            let base_cycles: u8 = if memory_8bit { 4 } else { 5 };
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 2 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x0F => {
            // ORA absolute long
            let addr = read_absolute_long_address_generic(state, bus);
            let memory_8bit = memory_is_8bit(state);
            let operand = read_operand_m(state, bus, addr, memory_8bit);
            ora_operand(state, operand);
            let base_cycles: u8 = 5;
            let already_accounted: u8 = 3;
            add_cycles(state, base_cycles.saturating_sub(already_accounted));
            base_cycles
        }

        0x1F => {
            // ORA absolute long,X
            let addr = read_absolute_long_x_address_generic(state, bus);
            let memory_8bit = memory_is_8bit(state);
            let operand = read_operand_m(state, bus, addr, memory_8bit);
            ora_operand(state, operand);
            let base_cycles: u8 = 5;
            let already_accounted: u8 = 3;
            add_cycles(state, base_cycles.saturating_sub(already_accounted));
            base_cycles
        }

        0x01 => {
            // ORA (dp,X)
            let (addr, penalty) = read_indirect_x_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = memory_is_8bit(state);
            let operand = read_operand_m(state, bus, addr, memory_8bit);
            ora_operand(state, operand);
            let base_cycles: u8 = 6;
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x11 => {
            // ORA (dp),Y
            let (addr, penalty) = read_indirect_y_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = memory_is_8bit(state);
            let operand = read_operand_m(state, bus, addr, memory_8bit);
            ora_operand(state, operand);
            let base_cycles: u8 = 5;
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x12 => {
            // ORA (dp)
            let (addr, penalty) = read_indirect_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = memory_is_8bit(state);
            let operand = read_operand_m(state, bus, addr, memory_8bit);
            ora_operand(state, operand);
            let base_cycles: u8 = 5;
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x13 => {
            // ORA (sr,S),Y
            let (addr, penalty) = read_stack_relative_indirect_y_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = memory_is_8bit(state);
            let operand = read_operand_m(state, bus, addr, memory_8bit);
            ora_operand(state, operand);
            let base_cycles: u8 = 7;
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x03 => {
            // ORA stack relative
            let addr = read_stack_relative_address_generic(state, bus);
            let memory_8bit = memory_is_8bit(state);
            let operand = read_operand_m(state, bus, addr, memory_8bit);
            ora_operand(state, operand);
            let base_cycles: u8 = 4;
            let already_accounted: u8 = 1;
            add_cycles(state, base_cycles.saturating_sub(already_accounted));
            base_cycles
        }

        0x07 => {
            // ORA [dp]
            let (addr, penalty) = read_indirect_long_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = memory_is_8bit(state);
            let operand = read_operand_m(state, bus, addr, memory_8bit);
            ora_operand(state, operand);
            let base_cycles: u8 = 6;
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x17 => {
            // ORA [dp],Y
            let (addr, penalty) = read_indirect_long_y_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = memory_is_8bit(state);
            let operand = read_operand_m(state, bus, addr, memory_8bit);
            ora_operand(state, operand);
            let base_cycles: u8 = 6;
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        // Load/Store instructions - extended coverage
        0x25 => {
            // AND direct page
            let (addr, penalty) = read_direct_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = memory_is_8bit(state);
            let operand = read_operand_m(state, bus, addr, memory_8bit);
            and_operand(state, operand);
            let base_cycles: u8 = if memory_8bit { 3 } else { 4 };
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x35 => {
            // AND direct page,X
            let (addr, penalty) = read_direct_x_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = memory_is_8bit(state);
            let operand = read_operand_m(state, bus, addr, memory_8bit);
            and_operand(state, operand);
            let base_cycles: u8 = if memory_8bit { 4 } else { 5 };
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x2D => {
            // AND absolute
            let addr = read_absolute_address_generic(state, bus);
            let memory_8bit = memory_is_8bit(state);
            let operand = read_operand_m(state, bus, addr, memory_8bit);
            and_operand(state, operand);
            let base_cycles: u8 = if memory_8bit { 4 } else { 5 };
            let already_accounted: u8 = 2;
            add_cycles(state, base_cycles.saturating_sub(already_accounted));
            base_cycles
        }

        0x3D => {
            // AND absolute,X
            let (addr, penalty) = read_absolute_x_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = memory_is_8bit(state);
            let operand = read_operand_m(state, bus, addr, memory_8bit);
            and_operand(state, operand);
            let base_cycles: u8 = if memory_8bit { 4 } else { 5 };
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 2 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x39 => {
            // AND absolute,Y
            let (addr, penalty) = read_absolute_y_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = memory_is_8bit(state);
            let operand = read_operand_m(state, bus, addr, memory_8bit);
            and_operand(state, operand);
            let base_cycles: u8 = if memory_8bit { 4 } else { 5 };
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 2 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x2F => {
            // AND absolute long
            let addr = read_absolute_long_address_generic(state, bus);
            let memory_8bit = memory_is_8bit(state);
            let operand = read_operand_m(state, bus, addr, memory_8bit);
            and_operand(state, operand);
            let base_cycles: u8 = if memory_8bit { 5 } else { 6 };
            let already_accounted: u8 = 3;
            add_cycles(state, base_cycles.saturating_sub(already_accounted));
            base_cycles
        }

        0x3F => {
            // AND absolute long,X
            let addr = read_absolute_long_x_address_generic(state, bus);
            let memory_8bit = memory_is_8bit(state);
            let operand = read_operand_m(state, bus, addr, memory_8bit);
            and_operand(state, operand);
            let base_cycles: u8 = if memory_8bit { 5 } else { 6 };
            let already_accounted: u8 = 3;
            add_cycles(state, base_cycles.saturating_sub(already_accounted));
            base_cycles
        }

        0x21 => {
            // AND (dp,X)
            let (addr, penalty) = read_indirect_x_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = memory_is_8bit(state);
            let operand = read_operand_m(state, bus, addr, memory_8bit);
            and_operand(state, operand);
            let base_cycles: u8 = if memory_8bit { 6 } else { 7 };
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x31 => {
            // AND (dp),Y
            let (addr, penalty) = read_indirect_y_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = memory_is_8bit(state);
            let operand = read_operand_m(state, bus, addr, memory_8bit);
            and_operand(state, operand);
            let base_cycles: u8 = if memory_8bit { 5 } else { 6 };
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x32 => {
            // AND (dp)
            let (addr, penalty) = read_indirect_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = memory_is_8bit(state);
            let operand = read_operand_m(state, bus, addr, memory_8bit);
            and_operand(state, operand);
            let base_cycles: u8 = if memory_8bit { 5 } else { 6 };
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x33 => {
            // AND (sr,S),Y
            let (addr, penalty) = read_stack_relative_indirect_y_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = memory_is_8bit(state);
            let operand = read_operand_m(state, bus, addr, memory_8bit);
            and_operand(state, operand);
            let base_cycles: u8 = if memory_8bit { 7 } else { 8 };
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x23 => {
            // AND stack relative
            let addr = read_stack_relative_address_generic(state, bus);
            let memory_8bit = memory_is_8bit(state);
            let operand = read_operand_m(state, bus, addr, memory_8bit);
            and_operand(state, operand);
            let base_cycles: u8 = if memory_8bit { 4 } else { 5 };
            let already_accounted: u8 = 1;
            add_cycles(state, base_cycles.saturating_sub(already_accounted));
            base_cycles
        }

        0x27 => {
            // AND [dp]
            let (addr, penalty) = read_indirect_long_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = memory_is_8bit(state);
            let operand = read_operand_m(state, bus, addr, memory_8bit);
            and_operand(state, operand);
            let base_cycles: u8 = if memory_8bit { 6 } else { 7 };
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x37 => {
            // AND [dp],Y
            let (addr, penalty) = read_indirect_long_y_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = memory_is_8bit(state);
            let operand = read_operand_m(state, bus, addr, memory_8bit);
            and_operand(state, operand);
            let base_cycles: u8 = if memory_8bit { 6 } else { 7 };
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        // EOR logical exclusive OR operations
        0x45 => {
            // EOR direct page
            let (addr, penalty) = read_direct_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = memory_is_8bit(state);
            let operand = read_operand_m(state, bus, addr, memory_8bit);
            eor_operand(state, operand);
            let base_cycles: u8 = if memory_8bit { 3 } else { 4 };
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x55 => {
            // EOR direct page,X
            let (addr, penalty) = read_direct_x_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = memory_is_8bit(state);
            let operand = read_operand_m(state, bus, addr, memory_8bit);
            eor_operand(state, operand);
            let base_cycles: u8 = if memory_8bit { 4 } else { 5 };
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x4D => {
            // EOR absolute
            let addr = read_absolute_address_generic(state, bus);
            let memory_8bit = memory_is_8bit(state);
            let operand = read_operand_m(state, bus, addr, memory_8bit);
            eor_operand(state, operand);
            let base_cycles: u8 = if memory_8bit { 4 } else { 5 };
            let already_accounted: u8 = 2;
            add_cycles(state, base_cycles.saturating_sub(already_accounted));
            base_cycles
        }

        0x5D => {
            // EOR absolute,X
            let (addr, penalty) = read_absolute_x_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = memory_is_8bit(state);
            let operand = read_operand_m(state, bus, addr, memory_8bit);
            eor_operand(state, operand);
            let base_cycles: u8 = if memory_8bit { 4 } else { 5 };
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 2 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x59 => {
            // EOR absolute,Y
            let (addr, penalty) = read_absolute_y_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = memory_is_8bit(state);
            let operand = read_operand_m(state, bus, addr, memory_8bit);
            eor_operand(state, operand);
            let base_cycles: u8 = if memory_8bit { 4 } else { 5 };
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 2 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x4F => {
            // EOR absolute long
            let addr = read_absolute_long_address_generic(state, bus);
            let memory_8bit = memory_is_8bit(state);
            let operand = read_operand_m(state, bus, addr, memory_8bit);
            eor_operand(state, operand);
            let base_cycles: u8 = if memory_8bit { 5 } else { 6 };
            let already_accounted: u8 = 3;
            add_cycles(state, base_cycles.saturating_sub(already_accounted));
            base_cycles
        }

        0x5F => {
            // EOR absolute long,X
            let addr = read_absolute_long_x_address_generic(state, bus);
            let memory_8bit = memory_is_8bit(state);
            let operand = read_operand_m(state, bus, addr, memory_8bit);
            eor_operand(state, operand);
            let base_cycles: u8 = if memory_8bit { 5 } else { 6 };
            let already_accounted: u8 = 3;
            add_cycles(state, base_cycles.saturating_sub(already_accounted));
            base_cycles
        }

        0x41 => {
            // EOR (dp,X)
            let (addr, penalty) = read_indirect_x_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = memory_is_8bit(state);
            let operand = read_operand_m(state, bus, addr, memory_8bit);
            eor_operand(state, operand);
            let base_cycles: u8 = if memory_8bit { 6 } else { 7 };
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x51 => {
            // EOR (dp),Y
            let (addr, penalty) = read_indirect_y_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = memory_is_8bit(state);
            let operand = read_operand_m(state, bus, addr, memory_8bit);
            eor_operand(state, operand);
            let base_cycles: u8 = if memory_8bit { 5 } else { 6 };
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x52 => {
            // EOR (dp)
            let (addr, penalty) = read_indirect_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = memory_is_8bit(state);
            let operand = read_operand_m(state, bus, addr, memory_8bit);
            eor_operand(state, operand);
            let base_cycles: u8 = if memory_8bit { 5 } else { 6 };
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x53 => {
            // EOR (sr,S),Y
            let (addr, penalty) = read_stack_relative_indirect_y_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = memory_is_8bit(state);
            let operand = read_operand_m(state, bus, addr, memory_8bit);
            eor_operand(state, operand);
            let base_cycles: u8 = if memory_8bit { 7 } else { 8 };
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x43 => {
            // EOR stack relative
            let addr = read_stack_relative_address_generic(state, bus);
            let memory_8bit = memory_is_8bit(state);
            let operand = read_operand_m(state, bus, addr, memory_8bit);
            eor_operand(state, operand);
            let base_cycles: u8 = if memory_8bit { 4 } else { 5 };
            let already_accounted: u8 = 1;
            add_cycles(state, base_cycles.saturating_sub(already_accounted));
            base_cycles
        }

        0x47 => {
            // EOR [dp]
            let (addr, penalty) = read_indirect_long_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = memory_is_8bit(state);
            let operand = read_operand_m(state, bus, addr, memory_8bit);
            eor_operand(state, operand);
            let base_cycles: u8 = if memory_8bit { 6 } else { 7 };
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x57 => {
            // EOR [dp],Y
            let (addr, penalty) = read_indirect_long_y_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = memory_is_8bit(state);
            let operand = read_operand_m(state, bus, addr, memory_8bit);
            eor_operand(state, operand);
            let base_cycles: u8 = if memory_8bit { 6 } else { 7 };
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x0A => {
            // ASL accumulator
            if memory_is_8bit(state) {
                let result = asl8(state, (state.a & 0xFF) as u8);
                state.a = (state.a & 0xFF00) | (result as u16);
            } else {
                state.a = asl16(state, state.a);
            }
            add_cycles(state, 2);
            2
        }

        0x2A => {
            // ROL accumulator
            if memory_is_8bit(state) {
                let result = rol8(state, (state.a & 0xFF) as u8);
                state.a = (state.a & 0xFF00) | (result as u16);
            } else {
                state.a = rol16(state, state.a);
            }
            add_cycles(state, 2);
            2
        }

        0x4A => {
            // LSR accumulator
            if memory_is_8bit(state) {
                let result = lsr8(state, (state.a & 0xFF) as u8);
                state.a = (state.a & 0xFF00) | (result as u16);
            } else {
                state.a = lsr16(state, state.a);
            }
            add_cycles(state, 2);
            2
        }

        0x6A => {
            // ROR accumulator
            if memory_is_8bit(state) {
                let result = ror8(state, (state.a & 0xFF) as u8);
                state.a = (state.a & 0xFF00) | (result as u16);
            } else {
                state.a = ror16(state, state.a);
            }
            add_cycles(state, 2);
            2
        }

        0x06 => {
            // ASL direct page
            let (addr, penalty) = read_direct_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = memory_is_8bit(state);
            modify_memory(state, bus, addr, memory_8bit, asl8, asl16);
            let base_cycles: u8 = 5;
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x16 => {
            // ASL direct page,X
            let (addr, penalty) = read_direct_x_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = memory_is_8bit(state);
            modify_memory(state, bus, addr, memory_8bit, asl8, asl16);
            let base_cycles: u8 = 6;
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x0E => {
            // ASL absolute
            let addr = read_absolute_address_generic(state, bus);
            let memory_8bit = memory_is_8bit(state);
            modify_memory(state, bus, addr, memory_8bit, asl8, asl16);
            let base_cycles: u8 = 6;
            let already_accounted: u8 = 3;
            add_cycles(state, base_cycles.saturating_sub(already_accounted));
            base_cycles
        }

        0x1E => {
            // ASL absolute,X
            let (addr, penalty) = read_absolute_x_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = memory_is_8bit(state);
            modify_memory(state, bus, addr, memory_8bit, asl8, asl16);
            let base_cycles: u8 = 7;
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 3 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x26 => {
            // ROL direct page
            let (addr, penalty) = read_direct_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = memory_is_8bit(state);
            modify_memory(state, bus, addr, memory_8bit, rol8, rol16);
            let base_cycles: u8 = 5;
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x36 => {
            // ROL direct page,X
            let (addr, penalty) = read_direct_x_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = memory_is_8bit(state);
            modify_memory(state, bus, addr, memory_8bit, rol8, rol16);
            let base_cycles: u8 = 6;
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x2E => {
            // ROL absolute
            let addr = read_absolute_address_generic(state, bus);
            let memory_8bit = memory_is_8bit(state);
            modify_memory(state, bus, addr, memory_8bit, rol8, rol16);
            let base_cycles: u8 = 6;
            let already_accounted: u8 = 3;
            add_cycles(state, base_cycles.saturating_sub(already_accounted));
            base_cycles
        }

        0x3E => {
            // ROL absolute,X
            let (addr, penalty) = read_absolute_x_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = memory_is_8bit(state);
            modify_memory(state, bus, addr, memory_8bit, rol8, rol16);
            let base_cycles: u8 = 7;
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 3 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x46 => {
            // LSR direct page
            let (addr, penalty) = read_direct_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = memory_is_8bit(state);
            modify_memory(state, bus, addr, memory_8bit, lsr8, lsr16);
            let base_cycles: u8 = 5;
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x56 => {
            // LSR direct page,X
            let (addr, penalty) = read_direct_x_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = memory_is_8bit(state);
            modify_memory(state, bus, addr, memory_8bit, lsr8, lsr16);
            let base_cycles: u8 = 6;
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x4E => {
            // LSR absolute
            let addr = read_absolute_address_generic(state, bus);
            let memory_8bit = memory_is_8bit(state);
            modify_memory(state, bus, addr, memory_8bit, lsr8, lsr16);
            let base_cycles: u8 = 6;
            let already_accounted: u8 = 3;
            add_cycles(state, base_cycles.saturating_sub(already_accounted));
            base_cycles
        }

        0x5E => {
            // LSR absolute,X
            let (addr, penalty) = read_absolute_x_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = memory_is_8bit(state);
            modify_memory(state, bus, addr, memory_8bit, lsr8, lsr16);
            let base_cycles: u8 = 7;
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 3 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x66 => {
            // ROR direct page
            let (addr, penalty) = read_direct_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = memory_is_8bit(state);
            modify_memory(state, bus, addr, memory_8bit, ror8, ror16);
            let base_cycles: u8 = 5;
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x76 => {
            // ROR direct page,X
            let (addr, penalty) = read_direct_x_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = memory_is_8bit(state);
            modify_memory(state, bus, addr, memory_8bit, ror8, ror16);
            let base_cycles: u8 = 6;
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x6E => {
            // ROR absolute
            let addr = read_absolute_address_generic(state, bus);
            let memory_8bit = memory_is_8bit(state);
            modify_memory(state, bus, addr, memory_8bit, ror8, ror16);
            let base_cycles: u8 = 6;
            let already_accounted: u8 = 3;
            add_cycles(state, base_cycles.saturating_sub(already_accounted));
            base_cycles
        }

        0x7E => {
            // ROR absolute,X
            let (addr, penalty) = read_absolute_x_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = memory_is_8bit(state);
            modify_memory(state, bus, addr, memory_8bit, ror8, ror16);
            let base_cycles: u8 = 7;
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 3 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x89 => {
            // BIT immediate
            let memory_8bit = memory_is_8bit(state);
            let operand = if memory_8bit {
                read_u8_generic(state, bus) as u16
            } else {
                read_u16_generic(state, bus)
            };
            bit_operand_immediate(state, operand);
            let total_cycles: u8 = if memory_8bit { 2 } else { 3 };
            let already_accounted: u8 = if memory_8bit { 1 } else { 2 };
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x24 => {
            // BIT direct page
            let (addr, penalty) = read_direct_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = memory_is_8bit(state);
            let operand = read_operand_m(state, bus, addr, memory_8bit);
            bit_operand_memory(state, operand);
            let base_cycles: u8 = 3;
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x34 => {
            // BIT direct page,X
            let (addr, penalty) = read_direct_x_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = memory_is_8bit(state);
            let operand = read_operand_m(state, bus, addr, memory_8bit);
            bit_operand_memory(state, operand);
            let base_cycles: u8 = 4;
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x2C => {
            // BIT absolute
            let addr = read_absolute_address_generic(state, bus);
            let memory_8bit = memory_is_8bit(state);
            let operand = read_operand_m(state, bus, addr, memory_8bit);
            bit_operand_memory(state, operand);
            if addr == 0x004210 && crate::debug_flags::debug_bit4210() {
                let log_all = crate::debug_flags::debug_bit4210_all();
                let interesting = operand != 0x0002 || log_all;
                if interesting {
                    println!(
                        "[BIT4210] pc_next={:04X} A=0x{:04X} operand=0x{:04X} M8={} P_after=0x{:02X} (N={} V={} Z={})",
                        state.pc,
                        state.a,
                        operand,
                        memory_8bit,
                        state.p.bits(),
                        state.p.contains(StatusFlags::NEGATIVE) as u8,
                        state.p.contains(StatusFlags::OVERFLOW) as u8,
                        state.p.contains(StatusFlags::ZERO) as u8,
                    );
                }
            }
            let base_cycles: u8 = 4;
            let already_accounted: u8 = 3;
            add_cycles(state, base_cycles.saturating_sub(already_accounted));
            base_cycles
        }

        0x3C => {
            // BIT absolute,X
            let (addr, penalty) = read_absolute_x_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = memory_is_8bit(state);
            let operand = read_operand_m(state, bus, addr, memory_8bit);
            bit_operand_memory(state, operand);
            let base_cycles: u8 = 4;
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 3 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0xA1 => {
            // LDA (dp,X)
            let (addr, penalty) = read_indirect_x_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = memory_is_8bit(state);
            let value = read_operand_m(state, bus, addr, memory_8bit);
            if memory_8bit {
                state.a = (state.a & 0xFF00) | (value & 0xFF);
                set_flags_nz_8(state, value as u8);
            } else {
                state.a = value;
                set_flags_nz_16(state, value);
            }
            let base_cycles: u8 = 6;
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0xA3 => {
            // LDA stack relative
            let addr = read_stack_relative_address_generic(state, bus);
            let memory_8bit = memory_is_8bit(state);
            let value = read_operand_m(state, bus, addr, memory_8bit);
            if memory_8bit {
                state.a = (state.a & 0xFF00) | (value & 0xFF);
                set_flags_nz_8(state, value as u8);
            } else {
                state.a = value;
                set_flags_nz_16(state, value);
            }
            let base_cycles: u8 = 4;
            let already_accounted: u8 = 1;
            add_cycles(state, base_cycles.saturating_sub(already_accounted));
            base_cycles
        }

        0xA4 => {
            // LDY direct page
            let (addr, penalty) = read_direct_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            if index_is_8bit(state) {
                let value = bus.read_u8(addr) as u16;
                state.y = (state.y & 0xFF00) | value;
                set_flags_nz_8(state, value as u8);
            } else {
                let value = bus.read_u16(addr);
                state.y = value;
                set_flags_nz_16(state, value);
            }
            let base_cycles: u8 = if index_is_8bit(state) { 3 } else { 4 };
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0xA5 => {
            // LDA direct page
            let (addr, penalty) = read_direct_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = state.emulation_mode || state.p.contains(StatusFlags::MEMORY_8BIT);
            if memory_8bit {
                let value = bus.read_u8(addr) as u16;
                state.a = (state.a & 0xFF00) | value;
                set_flags_nz_8(state, value as u8);
            } else {
                let value = bus.read_u16(addr);
                state.a = value;
                set_flags_nz_16(state, value);
            }
            let base_cycles: u8 = if memory_8bit { 3 } else { 4 };
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty; // operand fetch + dp penalty
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0xA6 => {
            // LDX direct page
            let (addr, penalty) = read_direct_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            if index_is_8bit(state) {
                let value = bus.read_u8(addr) as u16;
                state.x = (state.x & 0xFF00) | value;
                set_flags_nz_8(state, value as u8);
            } else {
                let value = bus.read_u16(addr);
                state.x = value;
                set_flags_nz_16(state, value);
            }
            let base_cycles: u8 = if index_is_8bit(state) { 3 } else { 4 };
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0xA7 => {
            // LDA [dp]
            let (addr, penalty) = read_indirect_long_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = memory_is_8bit(state);
            let value = read_operand_m(state, bus, addr, memory_8bit);
            if memory_8bit {
                state.a = (state.a & 0xFF00) | (value & 0xFF);
                set_flags_nz_8(state, value as u8);
            } else {
                state.a = value;
                set_flags_nz_16(state, value);
            }
            let base_cycles: u8 = 6;
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0xA9 => {
            // LDA immediate
            let memory_8bit = memory_is_8bit(state);
            let value = if memory_8bit {
                read_u8_generic(state, bus) as u16
            } else {
                read_u16_generic(state, bus)
            };
            if memory_8bit {
                state.a = (state.a & 0xFF00) | (value & 0x00FF);
                set_flags_nz_8(state, value as u8);
            } else {
                state.a = value;
                set_flags_nz_16(state, value);
            }
            add_cycles(state, if memory_8bit { 2 } else { 3 });
            if memory_8bit {
                2
            } else {
                3
            }
        }
        0xAC => {
            // LDY absolute
            let addr = read_absolute_address_generic(state, bus);
            if index_is_8bit(state) {
                let value = bus.read_u8(addr) as u16;
                state.y = (state.y & 0xFF00) | value;
                set_flags_nz_8(state, value as u8);
            } else {
                let value = bus.read_u16(addr);
                state.y = value;
                set_flags_nz_16(state, value);
            }
            add_cycles(state, 4);
            4
        }
        0xAE => {
            // LDX absolute
            let addr = read_absolute_address_generic(state, bus);
            if index_is_8bit(state) {
                let value = bus.read_u8(addr) as u16;
                state.x = (state.x & 0xFF00) | value;
                set_flags_nz_8(state, value as u8);
            } else {
                let value = bus.read_u16(addr);
                state.x = value;
                set_flags_nz_16(state, value);
            }
            add_cycles(state, 4);
            4
        }
        0xA2 => {
            // LDX immediate
            let index_8bit = index_is_8bit(state);
            let value = if index_8bit {
                read_u8_generic(state, bus) as u16
            } else {
                read_u16_generic(state, bus)
            };
            if index_8bit {
                state.x = (state.x & 0xFF00) | (value & 0x00FF);
                set_flags_nz_8(state, value as u8);
            } else {
                state.x = value;
                set_flags_nz_16(state, value);
            }
            add_cycles(state, if index_8bit { 2 } else { 3 });
            if index_8bit {
                2
            } else {
                3
            }
        }
        0xA8 => {
            // TAY (Transfer Accumulator to Y)
            if index_is_8bit(state) {
                state.y = (state.y & 0xFF00) | (state.a & 0xFF);
                set_flags_nz_8(state, (state.y & 0xFF) as u8);
            } else {
                state.y = state.a;
                set_flags_nz_16(state, state.y);
            }
            add_cycles(state, 2);
            2
        }
        0xAA => {
            // TAX (Transfer Accumulator to X)
            if index_is_8bit(state) {
                state.x = (state.x & 0xFF00) | (state.a & 0xFF);
                set_flags_nz_8(state, (state.x & 0xFF) as u8);
            } else {
                state.x = state.a;
                set_flags_nz_16(state, state.x);
            }
            add_cycles(state, 2);
            2
        }
        0xA0 => {
            // LDY immediate
            let index_8bit = index_is_8bit(state);
            let value = if index_8bit {
                read_u8_generic(state, bus) as u16
            } else {
                read_u16_generic(state, bus)
            };
            if index_8bit {
                state.y = (state.y & 0xFF00) | (value & 0x00FF);
                set_flags_nz_8(state, value as u8);
            } else {
                state.y = value;
                set_flags_nz_16(state, value);
            }
            add_cycles(state, if index_8bit { 2 } else { 3 });
            if index_8bit {
                2
            } else {
                3
            }
        }

        // Store instructions
        0x8D => {
            // STA absolute
            let addr = read_absolute_address_generic(state, bus);
            if state.p.contains(StatusFlags::MEMORY_8BIT) {
                write_u8_generic(bus, addr, state.a as u8);
                add_cycles(state, 4);
                4
            } else {
                bus.write_u16(addr, state.a);
                add_cycles(state, 5);
                5
            }
        }
        0x8E => {
            // STX absolute
            let addr = read_absolute_address_generic(state, bus);
            if state.p.contains(StatusFlags::INDEX_8BIT) {
                write_u8_generic(bus, addr, state.x as u8);
                add_cycles(state, 4);
                4
            } else {
                bus.write_u16(addr, state.x);
                add_cycles(state, 5);
                5
            }
        }
        0x8C => {
            // STY absolute
            let addr = read_absolute_address_generic(state, bus);
            if state.p.contains(StatusFlags::INDEX_8BIT) {
                write_u8_generic(bus, addr, state.y as u8);
                add_cycles(state, 4);
                4
            } else {
                bus.write_u16(addr, state.y);
                add_cycles(state, 5);
                5
            }
        }

        // Stack operations - Critical for SA-1 function calls
        0x0B => {
            // PHD - Push Direct Page register
            if state.emulation_mode {
                push_u16_emulation_edge(state, bus, state.dp);
            } else {
                push_u16_generic(state, bus, state.dp);
            }
            add_cycles(state, 4);
            4
        }

        0x48 => {
            // PHA
            if state.p.contains(StatusFlags::MEMORY_8BIT) {
                push_u8_generic(state, bus, state.a as u8);
                add_cycles(state, 3);
                3
            } else {
                push_u16_generic(state, bus, state.a);
                add_cycles(state, 4);
                4
            }
        }
        0x4B => {
            // PHK - Push Program Bank
            push_u8_generic(state, bus, state.pb);
            add_cycles(state, 3);
            3
        }
        0x5A => {
            // PHY - Push Y register
            if index_is_8bit(state) {
                push_u8_generic(state, bus, (state.y & 0xFF) as u8);
                add_cycles(state, 3);
                3
            } else {
                push_u16_generic(state, bus, state.y);
                add_cycles(state, 4);
                4
            }
        }
        0x68 => {
            // PLA
            if state.p.contains(StatusFlags::MEMORY_8BIT) {
                state.a = (state.a & 0xFF00) | (pop_u8_generic(state, bus) as u16);
                set_flags_nz_8(state, state.a as u8);
                add_cycles(state, 4);
                4
            } else {
                state.a = pop_u16_generic(state, bus);
                set_flags_nz_16(state, state.a);
                add_cycles(state, 5);
                5
            }
        }
        0x8B => {
            // PHB - Push Data Bank register
            push_u8_generic(state, bus, state.db);
            add_cycles(state, 3);
            3
        }
        0xAB => {
            // PLB - Pull Data Bank register
            if state.emulation_mode {
                // Undocumented emulation edge: PLB can pull using a 16-bit stack increment.
                // This can read from $0200.. when SP starts at $01FF.
                state.sp = state.sp.wrapping_add(1);
                state.db = bus.read_u8(state.sp as u32);
                add_cycles(state, 1);
                // Re-assert emulation-mode stack high byte after the sequence.
                state.sp = 0x0100 | (state.sp & 0x00FF);
            } else {
                state.db = pop_u8_generic(state, bus);
            }
            set_flags_nz_8(state, state.db);
            add_cycles(state, 4);
            4
        }
        0xDA => {
            // PHX - Push X register
            if index_is_8bit(state) {
                push_u8_generic(state, bus, (state.x & 0xFF) as u8);
                add_cycles(state, 3);
                3
            } else {
                push_u16_generic(state, bus, state.x);
                add_cycles(state, 4);
                4
            }
        }
        0xFA => {
            // PLX - Pull X register
            if index_is_8bit(state) {
                let value = pop_u8_generic(state, bus);
                state.x = (state.x & 0xFF00) | (value as u16);
                set_flags_nz_8(state, value);
                add_cycles(state, 4);
                4
            } else {
                state.x = pop_u16_generic(state, bus);
                set_flags_nz_16(state, state.x);
                add_cycles(state, 5);
                5
            }
        }
        0xF4 => {
            // PEA
            let value = read_u16_generic(state, bus);
            if state.emulation_mode {
                push_u16_emulation_edge(state, bus, value);
            } else {
                push_u16_generic(state, bus, value);
            }
            add_cycles(state, 5);
            5
        }
        0x1B => {
            // TCS - Transfer Accumulator to Stack Pointer
            let old_sp = state.sp;
            state.sp = if state.emulation_mode {
                0x0100 | (state.a & 0x00FF)
            } else {
                state.a
            };
            if crate::debug_flags::trace_sp_change() {
                println!(
                    "SP CHANGE TCS PB={:02X} PC={:04X} {:04X}->{:04X}",
                    state.pb, state.pc, old_sp, state.sp
                );
            }
            add_cycles(state, 2);
            2
        }

        // Arithmetic operations
        0x69 => {
            // ADC immediate (supports decimal mode)
            let memory_8bit = state.emulation_mode || state.p.contains(StatusFlags::MEMORY_8BIT);
            let operand = if memory_8bit {
                read_u8_generic(state, bus) as u16
            } else {
                read_u16_generic(state, bus)
            };
            adc_generic(state, operand);
            let total_cycles: u8 = if memory_8bit { 2 } else { 3 };
            let already_accounted: u8 = if memory_8bit { 1 } else { 2 };
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x65 => {
            // ADC direct page
            let (addr, penalty) = read_direct_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = state.emulation_mode || state.p.contains(StatusFlags::MEMORY_8BIT);
            let operand = if memory_8bit {
                bus.read_u8(addr) as u16
            } else {
                bus.read_u16(addr)
            };
            adc_generic(state, operand);
            let base_cycles: u8 = if memory_8bit { 3 } else { 4 };
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x75 => {
            // ADC direct page,X
            let (addr, penalty) = read_direct_x_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = state.emulation_mode || state.p.contains(StatusFlags::MEMORY_8BIT);
            let operand = if memory_8bit {
                bus.read_u8(addr) as u16
            } else {
                bus.read_u16(addr)
            };
            adc_generic(state, operand);
            let base_cycles: u8 = if memory_8bit { 4 } else { 5 };
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x6D => {
            // ADC absolute
            let addr = read_absolute_address_generic(state, bus);
            let memory_8bit = state.emulation_mode || state.p.contains(StatusFlags::MEMORY_8BIT);
            let operand = if memory_8bit {
                bus.read_u8(addr) as u16
            } else {
                bus.read_u16(addr)
            };
            adc_generic(state, operand);
            let base_cycles: u8 = if memory_8bit { 4 } else { 5 };
            let already_accounted: u8 = 2;
            add_cycles(state, base_cycles.saturating_sub(already_accounted));
            base_cycles
        }

        0x7D => {
            // ADC absolute,X
            let (addr, penalty) = read_absolute_x_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = state.emulation_mode || state.p.contains(StatusFlags::MEMORY_8BIT);
            let operand = if memory_8bit {
                bus.read_u8(addr) as u16
            } else {
                bus.read_u16(addr)
            };
            adc_generic(state, operand);
            let base_cycles: u8 = if memory_8bit { 4 } else { 5 };
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 2 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x79 => {
            // ADC absolute,Y
            let (addr, penalty) = read_absolute_y_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = state.emulation_mode || state.p.contains(StatusFlags::MEMORY_8BIT);
            let operand = if memory_8bit {
                bus.read_u8(addr) as u16
            } else {
                bus.read_u16(addr)
            };
            adc_generic(state, operand);
            let base_cycles: u8 = if memory_8bit { 4 } else { 5 };
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 2 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x6F => {
            // ADC absolute long
            let addr = read_absolute_long_address_generic(state, bus);
            let memory_8bit = state.emulation_mode || state.p.contains(StatusFlags::MEMORY_8BIT);
            let operand = if memory_8bit {
                bus.read_u8(addr) as u16
            } else {
                bus.read_u16(addr)
            };
            adc_generic(state, operand);
            let base_cycles: u8 = if memory_8bit { 5 } else { 6 };
            let already_accounted: u8 = 3;
            add_cycles(state, base_cycles.saturating_sub(already_accounted));
            base_cycles
        }

        0x7F => {
            // ADC absolute long,X
            let addr = read_absolute_long_x_address_generic(state, bus);
            let memory_8bit = state.emulation_mode || state.p.contains(StatusFlags::MEMORY_8BIT);
            let operand = if memory_8bit {
                bus.read_u8(addr) as u16
            } else {
                bus.read_u16(addr)
            };
            adc_generic(state, operand);
            let base_cycles: u8 = if memory_8bit { 5 } else { 6 };
            let already_accounted: u8 = 3;
            add_cycles(state, base_cycles.saturating_sub(already_accounted));
            base_cycles
        }

        0x61 => {
            // ADC (dp,X)
            let (addr, penalty) = read_indirect_x_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = state.emulation_mode || state.p.contains(StatusFlags::MEMORY_8BIT);
            let operand = if memory_8bit {
                bus.read_u8(addr) as u16
            } else {
                bus.read_u16(addr)
            };
            adc_generic(state, operand);
            let base_cycles: u8 = if memory_8bit { 6 } else { 7 };
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x71 => {
            // ADC (dp),Y
            let (addr, penalty) = read_indirect_y_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = state.emulation_mode || state.p.contains(StatusFlags::MEMORY_8BIT);
            let operand = if memory_8bit {
                bus.read_u8(addr) as u16
            } else {
                bus.read_u16(addr)
            };
            adc_generic(state, operand);
            let base_cycles: u8 = if memory_8bit { 5 } else { 6 };
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x72 => {
            // ADC (dp)
            let (addr, penalty) = read_indirect_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = state.emulation_mode || state.p.contains(StatusFlags::MEMORY_8BIT);
            let operand = if memory_8bit {
                bus.read_u8(addr) as u16
            } else {
                bus.read_u16(addr)
            };
            adc_generic(state, operand);
            let base_cycles: u8 = if memory_8bit { 5 } else { 6 };
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x67 => {
            // ADC [dp]
            let (addr, penalty) = read_indirect_long_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = state.emulation_mode || state.p.contains(StatusFlags::MEMORY_8BIT);
            let operand = if memory_8bit {
                bus.read_u8(addr) as u16
            } else {
                bus.read_u16(addr)
            };
            adc_generic(state, operand);
            let base_cycles: u8 = if memory_8bit { 6 } else { 7 };
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x77 => {
            // ADC [dp],Y
            let (addr, penalty) = read_indirect_long_y_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = state.emulation_mode || state.p.contains(StatusFlags::MEMORY_8BIT);
            let operand = if memory_8bit {
                bus.read_u8(addr) as u16
            } else {
                bus.read_u16(addr)
            };
            adc_generic(state, operand);
            let base_cycles: u8 = if memory_8bit { 6 } else { 7 };
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x63 => {
            // ADC stack relative
            let addr = read_stack_relative_address_generic(state, bus);
            let memory_8bit = state.emulation_mode || state.p.contains(StatusFlags::MEMORY_8BIT);
            let operand = if memory_8bit {
                bus.read_u8(addr) as u16
            } else {
                bus.read_u16(addr)
            };
            adc_generic(state, operand);
            let base_cycles: u8 = if memory_8bit { 4 } else { 5 };
            let already_accounted: u8 = 1;
            add_cycles(state, base_cycles.saturating_sub(already_accounted));
            base_cycles
        }

        0x73 => {
            // ADC (sr,S),Y
            let (addr, penalty) = read_stack_relative_indirect_y_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = state.emulation_mode || state.p.contains(StatusFlags::MEMORY_8BIT);
            let operand = if memory_8bit {
                bus.read_u8(addr) as u16
            } else {
                bus.read_u16(addr)
            };
            adc_generic(state, operand);
            let base_cycles: u8 = if memory_8bit { 7 } else { 8 };
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0xE9 => {
            // SBC immediate (supports decimal mode)
            let memory_8bit = state.emulation_mode || state.p.contains(StatusFlags::MEMORY_8BIT);
            let operand = if memory_8bit {
                read_u8_generic(state, bus) as u16
            } else {
                read_u16_generic(state, bus)
            };
            sbc_generic(state, operand);
            let total_cycles: u8 = if memory_8bit { 2 } else { 3 };
            let already_accounted: u8 = if memory_8bit { 1 } else { 2 };
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0xE5 => {
            // SBC direct page
            let (addr, penalty) = read_direct_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = state.emulation_mode || state.p.contains(StatusFlags::MEMORY_8BIT);
            let operand = if memory_8bit {
                bus.read_u8(addr) as u16
            } else {
                bus.read_u16(addr)
            };
            sbc_generic(state, operand);
            let base_cycles: u8 = if memory_8bit { 3 } else { 4 };
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0xF5 => {
            // SBC direct page,X
            let (addr, penalty) = read_direct_x_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = state.emulation_mode || state.p.contains(StatusFlags::MEMORY_8BIT);
            let operand = if memory_8bit {
                bus.read_u8(addr) as u16
            } else {
                bus.read_u16(addr)
            };
            sbc_generic(state, operand);
            let base_cycles: u8 = if memory_8bit { 4 } else { 5 };
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0xED => {
            // SBC absolute
            let addr = read_absolute_address_generic(state, bus);
            let memory_8bit = state.emulation_mode || state.p.contains(StatusFlags::MEMORY_8BIT);
            let operand = if memory_8bit {
                bus.read_u8(addr) as u16
            } else {
                bus.read_u16(addr)
            };
            sbc_generic(state, operand);
            let base_cycles: u8 = if memory_8bit { 4 } else { 5 };
            let already_accounted: u8 = 2;
            add_cycles(state, base_cycles.saturating_sub(already_accounted));
            base_cycles
        }

        0xFD => {
            // SBC absolute,X
            let (addr, penalty) = read_absolute_x_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = state.emulation_mode || state.p.contains(StatusFlags::MEMORY_8BIT);
            let operand = if memory_8bit {
                bus.read_u8(addr) as u16
            } else {
                bus.read_u16(addr)
            };
            sbc_generic(state, operand);
            let base_cycles: u8 = if memory_8bit { 4 } else { 5 };
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 2 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0xF9 => {
            // SBC absolute,Y
            let (addr, penalty) = read_absolute_y_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = state.emulation_mode || state.p.contains(StatusFlags::MEMORY_8BIT);
            let operand = if memory_8bit {
                bus.read_u8(addr) as u16
            } else {
                bus.read_u16(addr)
            };
            sbc_generic(state, operand);
            let base_cycles: u8 = if memory_8bit { 4 } else { 5 };
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 2 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0xEF => {
            // SBC absolute long
            let addr = read_absolute_long_address_generic(state, bus);
            let memory_8bit = state.emulation_mode || state.p.contains(StatusFlags::MEMORY_8BIT);
            let operand = if memory_8bit {
                bus.read_u8(addr) as u16
            } else {
                bus.read_u16(addr)
            };
            sbc_generic(state, operand);
            let base_cycles: u8 = if memory_8bit { 5 } else { 6 };
            let already_accounted: u8 = 3;
            add_cycles(state, base_cycles.saturating_sub(already_accounted));
            base_cycles
        }

        0xFF => {
            // SBC absolute long,X
            let addr = read_absolute_long_x_address_generic(state, bus);
            let memory_8bit = state.emulation_mode || state.p.contains(StatusFlags::MEMORY_8BIT);
            let operand = if memory_8bit {
                bus.read_u8(addr) as u16
            } else {
                bus.read_u16(addr)
            };
            sbc_generic(state, operand);
            let base_cycles: u8 = if memory_8bit { 5 } else { 6 };
            let already_accounted: u8 = 3;
            add_cycles(state, base_cycles.saturating_sub(already_accounted));
            base_cycles
        }

        0xE1 => {
            // SBC (dp,X)
            let (addr, penalty) = read_indirect_x_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = state.emulation_mode || state.p.contains(StatusFlags::MEMORY_8BIT);
            let operand = if memory_8bit {
                bus.read_u8(addr) as u16
            } else {
                bus.read_u16(addr)
            };
            sbc_generic(state, operand);
            let base_cycles: u8 = if memory_8bit { 6 } else { 7 };
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0xF1 => {
            // SBC (dp),Y
            let (addr, penalty) = read_indirect_y_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = state.emulation_mode || state.p.contains(StatusFlags::MEMORY_8BIT);
            let operand = if memory_8bit {
                bus.read_u8(addr) as u16
            } else {
                bus.read_u16(addr)
            };
            sbc_generic(state, operand);
            let base_cycles: u8 = if memory_8bit { 5 } else { 6 };
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0xE7 => {
            // SBC [dp]
            let (addr, penalty) = read_indirect_long_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = state.emulation_mode || state.p.contains(StatusFlags::MEMORY_8BIT);
            let operand = if memory_8bit {
                bus.read_u8(addr) as u16
            } else {
                bus.read_u16(addr)
            };
            sbc_generic(state, operand);
            let base_cycles: u8 = if memory_8bit { 6 } else { 7 };
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0xF7 => {
            // SBC [dp],Y
            let (addr, penalty) = read_indirect_long_y_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = state.emulation_mode || state.p.contains(StatusFlags::MEMORY_8BIT);
            let operand = if memory_8bit {
                bus.read_u8(addr) as u16
            } else {
                bus.read_u16(addr)
            };
            sbc_generic(state, operand);
            let base_cycles: u8 = if memory_8bit { 6 } else { 7 };
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0xE3 => {
            // SBC stack relative
            let addr = read_stack_relative_address_generic(state, bus);
            let memory_8bit = state.emulation_mode || state.p.contains(StatusFlags::MEMORY_8BIT);
            let operand = if memory_8bit {
                bus.read_u8(addr) as u16
            } else {
                bus.read_u16(addr)
            };
            sbc_generic(state, operand);
            let base_cycles: u8 = if memory_8bit { 4 } else { 5 };
            let already_accounted: u8 = 1;
            add_cycles(state, base_cycles.saturating_sub(already_accounted));
            base_cycles
        }

        0xF3 => {
            // SBC (sr,S),Y
            let (addr, penalty) = read_stack_relative_indirect_y_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = state.emulation_mode || state.p.contains(StatusFlags::MEMORY_8BIT);
            let operand = if memory_8bit {
                bus.read_u8(addr) as u16
            } else {
                bus.read_u16(addr)
            };
            sbc_generic(state, operand);
            let base_cycles: u8 = if memory_8bit { 7 } else { 8 };
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        // Logical operations
        0x29 => {
            // AND immediate
            let memory_8bit = memory_is_8bit(state);
            let operand = if memory_8bit {
                read_u8_generic(state, bus) as u16
            } else {
                read_u16_generic(state, bus)
            };
            and_operand(state, operand);
            let total_cycles: u8 = if memory_8bit { 2 } else { 3 };
            let already_accounted: u8 = if memory_8bit { 1 } else { 2 };
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }
        0x49 => {
            // EOR immediate
            let memory_8bit = memory_is_8bit(state);
            let operand = if memory_8bit {
                read_u8_generic(state, bus) as u16
            } else {
                read_u16_generic(state, bus)
            };
            eor_operand(state, operand);
            let total_cycles: u8 = if memory_8bit { 2 } else { 3 };
            let already_accounted: u8 = if memory_8bit { 1 } else { 2 };
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }
        0x09 => {
            // ORA immediate
            let memory_8bit = memory_is_8bit(state);
            let operand = if memory_8bit {
                read_u8_generic(state, bus) as u16
            } else {
                read_u16_generic(state, bus)
            };
            ora_operand(state, operand);
            let total_cycles: u8 = if memory_8bit { 2 } else { 3 };
            let already_accounted: u8 = if memory_8bit { 1 } else { 2 };
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        // Additional instruction coverage
        0xD8 => {
            // CLD - Clear Decimal Mode Flag
            state.p.remove(StatusFlags::DECIMAL);
            add_cycles(state, 2);
            2
        }
        0x7A => {
            // PLY - Pull Y from Stack
            if state.p.contains(StatusFlags::INDEX_8BIT) {
                // 8bitモード: 下位1バイトのみ読み、上位は必ずクリア
                state.y = pop_u8_generic(state, bus) as u16;
                set_flags_nz_8(state, state.y as u8);
                add_cycles(state, 4);
                4
            } else {
                state.y = pop_u16_generic(state, bus);
                set_flags_nz_16(state, state.y);
                add_cycles(state, 5);
                5
            }
        }

        0x7B => {
            // TDC - Transfer Direct Page register to Accumulator
            state.a = state.dp;
            // TDC transfers to the 16-bit accumulator (C) and sets N/Z based on the 16-bit value,
            // regardless of the M flag.
            set_flags_nz_16(state, state.a);
            add_cycles(state, 2);
            2
        }
        0xCE => {
            // DEC absolute - Decrement Absolute Memory
            let addr = read_absolute_address_generic(state, bus);
            if state.p.contains(StatusFlags::MEMORY_8BIT) {
                let value = bus.read_u8(addr).wrapping_sub(1);
                write_u8_generic(bus, addr, value);
                set_flags_nz_8(state, value);
                add_cycles(state, 6);
                6
            } else {
                let value = bus.read_u16(addr).wrapping_sub(1);
                bus.write_u16(addr, value);
                set_flags_nz_16(state, value);
                add_cycles(state, 7);
                7
            }
        }
        0xCB => {
            // WAI - Wait for Interrupt
            // Enter the "waiting for interrupt" state so the outer CPU
            // loop can stall until either IRQ or NMI arrives.
            // (Both S-CPU and SA-1 share this core.)
            if crate::debug_flags::trace_wai() {
                println!(
                    "[WAI] enter wait at {:02X}:{:04X} P={:02X}",
                    state.pb,
                    state.pc,
                    state.p.bits()
                );
            }
            state.waiting_for_irq = true;
            add_cycles(state, 3);
            3
        }

        0xDB => {
            // STP - Stop the processor until reset
            state.stopped = true;
            add_cycles(state, 3);
            3
        }
        0xCC => {
            // CPY absolute - Compare Y with Absolute Memory
            let addr = read_absolute_address_generic(state, bus);
            if state.p.contains(StatusFlags::INDEX_8BIT) {
                let value = bus.read_u8(addr);
                let result = (state.y as u8).wrapping_sub(value);
                state.p.set(StatusFlags::CARRY, (state.y as u8) >= value);
                state.p.set(StatusFlags::ZERO, result == 0);
                state.p.set(StatusFlags::NEGATIVE, (result & 0x80) != 0);
                add_cycles(state, 4);
                4
            } else {
                let value = bus.read_u16(addr);
                let result = state.y.wrapping_sub(value);
                state.p.set(StatusFlags::CARRY, state.y >= value);
                state.p.set(StatusFlags::ZERO, result == 0);
                state.p.set(StatusFlags::NEGATIVE, (result & 0x8000) != 0);
                add_cycles(state, 5);
                5
            }
        }
        0xC3 => {
            // CMP stack relative
            let addr = read_stack_relative_address_generic(state, bus);
            let operand = if memory_is_8bit(state) {
                bus.read_u8(addr) as u16
            } else {
                bus.read_u16(addr)
            };
            cmp_operand(state, operand);
            let base_cycles: u8 = if memory_is_8bit(state) { 4 } else { 5 };
            let already_accounted: u8 = 1;
            add_cycles(state, base_cycles.saturating_sub(already_accounted));
            base_cycles
        }

        // Second batch of instruction coverage
        0xB3 => {
            // LDA stack relative indirect indexed (sr,S),Y
            let (addr, penalty) = read_stack_relative_indirect_y_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = memory_is_8bit(state);
            let value = read_operand_m(state, bus, addr, memory_8bit);
            if memory_8bit {
                state.a = (state.a & 0xFF00) | (value & 0x00FF);
                set_flags_nz_8(state, value as u8);
            } else {
                state.a = value;
                set_flags_nz_16(state, value);
            }
            let base_cycles: u8 = if memory_8bit { 7 } else { 8 };
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }
        0xC4 => {
            // CPY direct page
            let addr = read_u8_generic(state, bus) as u32 + state.dp as u32;
            if state.p.contains(StatusFlags::INDEX_8BIT) {
                let value = bus.read_u8(addr);
                let result = (state.y as u8).wrapping_sub(value);
                state.p.set(StatusFlags::CARRY, (state.y as u8) >= value);
                state.p.set(StatusFlags::ZERO, result == 0);
                state.p.set(StatusFlags::NEGATIVE, (result & 0x80) != 0);
                add_cycles(state, 3);
                3
            } else {
                let value = bus.read_u16(addr);
                let result = state.y.wrapping_sub(value);
                state.p.set(StatusFlags::CARRY, state.y >= value);
                state.p.set(StatusFlags::ZERO, result == 0);
                state.p.set(StatusFlags::NEGATIVE, (result & 0x8000) != 0);
                add_cycles(state, 4);
                4
            }
        }
        0xDF => {
            // CMP long,X - Compare with Long Indexed X
            let addr_lo = bus.read_u8(full_address(state, state.pc)) as u32;
            let addr_hi = bus.read_u8(full_address(state, state.pc.wrapping_add(1))) as u32;
            let addr_bank = bus.read_u8(full_address(state, state.pc.wrapping_add(2))) as u32;
            state.pc = state.pc.wrapping_add(3);
            let addr = (addr_lo | (addr_hi << 8) | (addr_bank << 16)).wrapping_add(state.x as u32);

            if state.p.contains(StatusFlags::MEMORY_8BIT) {
                let value = bus.read_u8(addr);
                let result = (state.a as u8).wrapping_sub(value);
                state.p.set(StatusFlags::CARRY, (state.a as u8) >= value);
                state.p.set(StatusFlags::ZERO, result == 0);
                state.p.set(StatusFlags::NEGATIVE, (result & 0x80) != 0);
                add_cycles(state, 5);
                5
            } else {
                let value = bus.read_u16(addr);
                let result = state.a.wrapping_sub(value);
                state.p.set(StatusFlags::CARRY, state.a >= value);
                state.p.set(StatusFlags::ZERO, result == 0);
                state.p.set(StatusFlags::NEGATIVE, (result & 0x8000) != 0);
                add_cycles(state, 6);
                6
            }
        }
        0xF8 => {
            // SED - Set Decimal Mode Flag
            state.p.insert(StatusFlags::DECIMAL);
            add_cycles(state, 2);
            2
        }
        // Third batch of instruction coverage
        // Fourth batch: additional instruction coverage
        0xB6 => {
            // LDX direct page,Y
            let (addr, penalty) = read_direct_y_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            if index_is_8bit(state) {
                let value = bus.read_u8(addr);
                state.x = (state.x & 0xFF00) | (value as u16);
                set_flags_nz_8(state, value);
            } else {
                let value = bus.read_u16(addr);
                state.x = value;
                set_flags_nz_16(state, value);
            }
            let base_cycles: u8 = if index_is_8bit(state) { 4 } else { 5 };
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0xBE => {
            // LDX absolute,Y
            let (addr, penalty) = read_absolute_y_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            if index_is_8bit(state) {
                let value = bus.read_u8(addr);
                state.x = (state.x & 0xFF00) | (value as u16);
                set_flags_nz_8(state, value);
            } else {
                let value = bus.read_u16(addr);
                state.x = value;
                set_flags_nz_16(state, value);
            }
            let base_cycles: u8 = if index_is_8bit(state) { 4 } else { 5 };
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 2 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x58 => {
            // CLI - Clear Interrupt Disable
            state.p.remove(StatusFlags::IRQ_DISABLE);
            add_cycles(state, 2);
            2
        }

        0x08 => {
            // PHP - Push Processor Status
            let mut value = state.p.bits();
            if state.emulation_mode {
                // Emulation mode: push with B=1 and bit5 forced 1 (6502-compatible)
                value |= 0x30;
            }
            push_u8_generic(state, bus, value);
            add_cycles(state, 3);
            3
        }

        0xB4 => {
            // LDY direct page,X
            let (addr, penalty) = read_direct_x_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            if index_is_8bit(state) {
                let value = bus.read_u8(addr);
                state.y = (state.y & 0xFF00) | (value as u16);
                set_flags_nz_8(state, value);
            } else {
                let value = bus.read_u16(addr);
                state.y = value;
                set_flags_nz_16(state, value);
            }
            let base_cycles: u8 = if index_is_8bit(state) { 4 } else { 5 };
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x10 => branch_if_generic(state, bus, !state.p.contains(StatusFlags::NEGATIVE)),

        0xD5 => {
            // CMP direct page,X
            let (addr, penalty) = read_direct_x_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = memory_is_8bit(state);
            let operand = read_operand_m(state, bus, addr, memory_8bit);
            cmp_operand(state, operand);
            let base_cycles: u8 = if memory_8bit { 4 } else { 5 };
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0xD6 => {
            // DEC direct page,X
            let (addr, penalty) = read_direct_x_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            if memory_is_8bit(state) {
                let value = bus.read_u8(addr).wrapping_sub(1);
                bus.write_u8(addr, value);
                set_flags_nz_8(state, value);
            } else {
                let value = bus.read_u16(addr).wrapping_sub(1);
                bus.write_u16(addr, value);
                set_flags_nz_16(state, value);
            }
            let base_cycles: u8 = 6;
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0xB9 => {
            // LDA absolute,Y
            let (addr, penalty) = read_absolute_y_address_generic(state, bus);
            if state.p.contains(StatusFlags::MEMORY_8BIT) {
                let value = bus.read_u8(addr);
                state.a = (state.a & 0xFF00) | (value as u16);
                set_flags_nz_8(state, value);
                let base_cycles: u8 = 4;
                let total_cycles = base_cycles.saturating_add(penalty);
                let already_accounted: u8 = 2 + penalty;
                add_cycles(state, total_cycles.saturating_sub(already_accounted));
                total_cycles
            } else {
                let value = bus.read_u16(addr);
                state.a = value;
                set_flags_nz_16(state, value);
                let base_cycles: u8 = 5;
                let total_cycles = base_cycles.saturating_add(penalty);
                let already_accounted: u8 = 2 + penalty;
                add_cycles(state, total_cycles.saturating_sub(already_accounted));
                total_cycles
            }
        }

        0xBA => {
            // TSX - Transfer Stack Pointer to X
            if state.p.contains(StatusFlags::INDEX_8BIT) {
                let sp_low = state.sp as u8;
                state.x = (state.x & 0xFF00) | (sp_low as u16);
                set_flags_nz_8(state, sp_low);
            } else {
                state.x = state.sp;
                set_flags_nz_16(state, state.sp);
            }
            add_cycles(state, 2);
            2
        }

        0xBB => {
            // TYX - Transfer Y to X
            state.x = state.y;
            set_flags_index(state, state.x);
            add_cycles(state, 2);
            2
        }

        0xD4 => {
            // PEI - Push Effective Indirect Address
            let dp_offset = read_u8_generic(state, bus) as u32;
            let indirect_addr = (state.dp as u32 + dp_offset) & 0xFFFFFF;
            let effective_addr = bus.read_u16(indirect_addr);
            if state.emulation_mode {
                push_u16_emulation_edge(state, bus, effective_addr);
            } else {
                push_u16_generic(state, bus, effective_addr);
            }
            add_cycles(state, 6);
            6
        }

        // Fifth batch: more instruction coverage
        0x0C => {
            // TSB absolute - Test and Set Bits
            let addr = read_absolute_address_generic(state, bus);
            if state.p.contains(StatusFlags::MEMORY_8BIT) {
                let value = bus.read_u8(addr);
                let a_low = state.a as u8;
                state.p.set(StatusFlags::ZERO, (a_low & value) == 0);
                bus.write_u8(addr, value | a_low);
                add_cycles(state, 6);
                6
            } else {
                let value = bus.read_u16(addr);
                state.p.set(StatusFlags::ZERO, (state.a & value) == 0);
                bus.write_u16(addr, value | state.a);
                add_cycles(state, 8);
                8
            }
        }

        0xC1 => {
            // CMP indirect,X
            let (addr, penalty) = read_indirect_x_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let operand = if memory_is_8bit(state) {
                bus.read_u8(addr) as u16
            } else {
                bus.read_u16(addr)
            };
            cmp_operand(state, operand);
            let base_cycles: u8 = if memory_is_8bit(state) { 6 } else { 7 };
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0xC5 => {
            // CMP direct page
            let (addr, penalty) = read_direct_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            if memory_is_8bit(state) {
                let value = bus.read_u8(addr);
                let a_low = (state.a & 0xFF) as u8;
                let result = a_low.wrapping_sub(value);
                state.p.set(StatusFlags::CARRY, a_low >= value);
                state.p.set(StatusFlags::ZERO, result == 0);
                state.p.set(StatusFlags::NEGATIVE, (result & 0x80) != 0);
                let base_cycles: u8 = 3;
                let total_cycles = base_cycles.saturating_add(penalty);
                let already_accounted: u8 = 1 + penalty;
                add_cycles(state, total_cycles.saturating_sub(already_accounted));
                total_cycles
            } else {
                let value = bus.read_u16(addr);
                let result = state.a.wrapping_sub(value);
                state.p.set(StatusFlags::CARRY, state.a >= value);
                state.p.set(StatusFlags::ZERO, result == 0);
                state.p.set(StatusFlags::NEGATIVE, (result & 0x8000) != 0);
                let base_cycles: u8 = 4;
                let total_cycles = base_cycles.saturating_add(penalty);
                let already_accounted: u8 = 1 + penalty;
                add_cycles(state, total_cycles.saturating_sub(already_accounted));
                total_cycles
            }
        }

        0xC6 => {
            // DEC direct page
            let (addr, penalty) = read_direct_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            if memory_is_8bit(state) {
                let value = bus.read_u8(addr).wrapping_sub(1);
                bus.write_u8(addr, value);
                set_flags_nz_8(state, value);
            } else {
                let value = bus.read_u16(addr).wrapping_sub(1);
                bus.write_u16(addr, value);
                set_flags_nz_16(state, value);
            }
            let base_cycles: u8 = 5;
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0xC7 => {
            // CMP [dp]
            let (addr, penalty) = read_indirect_long_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            if memory_is_8bit(state) {
                let value = bus.read_u8(addr);
                let a_low = (state.a & 0xFF) as u8;
                let result = a_low.wrapping_sub(value);
                state.p.set(StatusFlags::CARRY, a_low >= value);
                state.p.set(StatusFlags::ZERO, result == 0);
                state.p.set(StatusFlags::NEGATIVE, (result & 0x80) != 0);
            } else {
                let value = bus.read_u16(addr);
                let result = state.a.wrapping_sub(value);
                state.p.set(StatusFlags::CARRY, state.a >= value);
                state.p.set(StatusFlags::ZERO, result == 0);
                state.p.set(StatusFlags::NEGATIVE, (result & 0x8000) != 0);
            }
            let base_cycles: u8 = 6;
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0xC8 => {
            // INY
            if index_is_8bit(state) {
                let value = ((state.y & 0xFF).wrapping_add(1)) as u8;
                state.y = (state.y & 0xFF00) | (value as u16);
                set_flags_nz_8(state, value);
            } else {
                state.y = state.y.wrapping_add(1);
                set_flags_nz_16(state, state.y);
            }
            add_cycles(state, 2);
            2
        }

        0xCA => {
            // DEX
            if index_is_8bit(state) {
                let value = ((state.x & 0xFF).wrapping_sub(1)) as u8;
                state.x = (state.x & 0xFF00) | (value as u16);
                set_flags_nz_8(state, value);
            } else {
                state.x = state.x.wrapping_sub(1);
                set_flags_nz_16(state, state.x);
            }
            add_cycles(state, 2);
            2
        }

        0xCD => {
            // CMP absolute
            let addr = read_absolute_address_generic(state, bus);
            let operand = if memory_is_8bit(state) {
                bus.read_u8(addr) as u16
            } else {
                bus.read_u16(addr)
            };
            cmp_operand(state, operand);
            let base_cycles: u8 = 4;
            let already_accounted: u8 = 3;
            add_cycles(state, base_cycles.saturating_sub(already_accounted));
            base_cycles
        }

        0xCF => {
            // CMP absolute long
            let addr = read_absolute_long_address_generic(state, bus);
            let operand = if memory_is_8bit(state) {
                bus.read_u8(addr) as u16
            } else {
                bus.read_u16(addr)
            };
            cmp_operand(state, operand);
            let base_cycles: u8 = 5;
            let already_accounted: u8 = 3;
            add_cycles(state, base_cycles.saturating_sub(already_accounted));
            base_cycles
        }

        0xD1 => {
            // CMP (dp),Y
            let (addr, penalty) = read_indirect_y_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let operand = if memory_is_8bit(state) {
                bus.read_u8(addr) as u16
            } else {
                bus.read_u16(addr)
            };
            cmp_operand(state, operand);
            let base_cycles: u8 = 5;
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0xD2 => {
            // CMP (dp)
            let (addr, penalty) = read_indirect_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let operand = if memory_is_8bit(state) {
                bus.read_u8(addr) as u16
            } else {
                bus.read_u16(addr)
            };
            cmp_operand(state, operand);
            let base_cycles: u8 = 5;
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0xD3 => {
            // CMP (sr,S),Y
            let (addr, penalty) = read_stack_relative_indirect_y_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let operand = if memory_is_8bit(state) {
                bus.read_u8(addr) as u16
            } else {
                bus.read_u16(addr)
            };
            cmp_operand(state, operand);
            let base_cycles: u8 = 7;
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0xD7 => {
            // CMP [dp],Y
            let (addr, penalty) = read_indirect_long_y_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let operand = if memory_is_8bit(state) {
                bus.read_u8(addr) as u16
            } else {
                bus.read_u16(addr)
            };
            cmp_operand(state, operand);
            let base_cycles: u8 = 6;
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0xD9 => {
            // CMP absolute,Y
            let (addr, penalty) = read_absolute_y_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let operand = if memory_is_8bit(state) {
                bus.read_u8(addr) as u16
            } else {
                bus.read_u16(addr)
            };
            cmp_operand(state, operand);
            let base_cycles: u8 = 4;
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 2 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0xDD => {
            // CMP absolute,X
            let (addr, penalty) = read_absolute_x_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let operand = if memory_is_8bit(state) {
                bus.read_u8(addr) as u16
            } else {
                bus.read_u16(addr)
            };
            cmp_operand(state, operand);
            let base_cycles: u8 = 4;
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 2 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0xDE => {
            // DEC absolute,X
            let (addr, penalty) = read_absolute_x_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            if memory_is_8bit(state) {
                let value = bus.read_u8(addr).wrapping_sub(1);
                bus.write_u8(addr, value);
                set_flags_nz_8(state, value);
            } else {
                let value = bus.read_u16(addr).wrapping_sub(1);
                bus.write_u16(addr, value);
                set_flags_nz_16(state, value);
            }
            let base_cycles: u8 = 7;
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 2 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0xE4 => {
            // CPX direct page
            let addr = (read_u8_generic(state, bus) as u32 + state.dp as u32) & 0xFFFFFF;
            if state.p.contains(StatusFlags::INDEX_8BIT) {
                let value = bus.read_u8(addr);
                let x_low = state.x as u8;
                let result = x_low.wrapping_sub(value);
                state.p.set(StatusFlags::CARRY, x_low >= value);
                state.p.set(StatusFlags::ZERO, result == 0);
                state.p.set(StatusFlags::NEGATIVE, (result & 0x80) != 0);
                add_cycles(state, 3);
                3
            } else {
                let value = bus.read_u16(addr);
                let result = state.x.wrapping_sub(value);
                state.p.set(StatusFlags::CARRY, state.x >= value);
                state.p.set(StatusFlags::ZERO, result == 0);
                state.p.set(StatusFlags::NEGATIVE, (result & 0x8000) != 0);
                add_cycles(state, 4);
                4
            }
        }

        0x2B => {
            // PLD - Pull Direct Page Register
            state.dp = if state.emulation_mode {
                pop_u16_emulation_edge(state, bus)
            } else {
                pop_u16_generic(state, bus)
            };
            set_flags_nz_16(state, state.dp);
            add_cycles(state, 5);
            5
        }

        0x40 => {
            // RTI - Return from Interrupt
            let from_pb = state.pb;
            let from_pc = state.pc.wrapping_sub(1);
            let sp_before = state.sp;
            let prev_p = state.p;
            if state.emulation_mode {
                let status = pop_u8_generic(state, bus);
                state.p = StatusFlags::from_bits_truncate(status);
                apply_status_side_effects_after_pull(state, prev_p);
                let lo = pop_u8_generic(state, bus) as u16;
                let hi = pop_u8_generic(state, bus) as u16;
                state.pc = (hi << 8) | lo;
                trace_suspicious_control_flow(
                    "RTI",
                    from_pb,
                    from_pc,
                    0x40,
                    state.pb,
                    state.pc,
                    sp_before,
                    format!("status={:02X} popped={:04X}", status, state.pc),
                );
                add_cycles(state, 6);
                6
            } else {
                let status = pop_u8_generic(state, bus);
                state.p = StatusFlags::from_bits_truncate(status);
                apply_status_side_effects_after_pull(state, prev_p);
                let lo = pop_u8_generic(state, bus) as u16;
                let hi = pop_u8_generic(state, bus) as u16;
                state.pc = (hi << 8) | lo;
                state.pb = pop_u8_generic(state, bus);
                trace_suspicious_control_flow(
                    "RTI",
                    from_pb,
                    from_pc,
                    0x40,
                    state.pb,
                    state.pc,
                    sp_before,
                    format!(
                        "status={:02X} popped={:02X}:{:04X}",
                        status, state.pb, state.pc
                    ),
                );
                add_cycles(state, 7);
                7
            }
        }

        0x30 => branch_if_generic(state, bus, state.p.contains(StatusFlags::NEGATIVE)),
        0x50 => branch_if_generic(state, bus, !state.p.contains(StatusFlags::OVERFLOW)),
        0x70 => branch_if_generic(state, bus, state.p.contains(StatusFlags::OVERFLOW)),
        0x80 => branch_if_generic(state, bus, true),
        0x82 => brl_generic(state, bus),
        0x90 => branch_if_generic(state, bus, !state.p.contains(StatusFlags::CARRY)),
        0xB0 => branch_if_generic(state, bus, state.p.contains(StatusFlags::CARRY)),
        0xD0 => branch_if_generic(state, bus, !state.p.contains(StatusFlags::ZERO)),
        0xF0 => branch_if_generic(state, bus, state.p.contains(StatusFlags::ZERO)),

        0xFE => {
            // INC absolute,X
            let (addr, penalty) = read_absolute_x_address_generic(state, bus);
            if state.p.contains(StatusFlags::MEMORY_8BIT) {
                let value = bus.read_u8(addr).wrapping_add(1);
                bus.write_u8(addr, value);
                set_flags_nz_8(state, value);
                let base_cycles: u8 = 7;
                let total_cycles = base_cycles.saturating_add(penalty);
                let already_accounted: u8 = 3 + penalty;
                add_cycles(state, total_cycles.saturating_sub(already_accounted));
                total_cycles
            } else {
                let value = bus.read_u16(addr).wrapping_add(1);
                bus.write_u16(addr, value);
                set_flags_nz_16(state, value);
                let base_cycles: u8 = 9;
                let total_cycles = base_cycles.saturating_add(penalty);
                let already_accounted: u8 = 3 + penalty;
                add_cycles(state, total_cycles.saturating_sub(already_accounted));
                total_cycles
            }
        }

        0x8F => {
            // STA long absolute
            let pc_before = state.pc;
            let addr_lo = read_u8_generic(state, bus) as u32;
            let addr_hi = read_u8_generic(state, bus) as u32;
            let addr_bank = read_u8_generic(state, bus) as u32;
            let full_addr = addr_lo | (addr_hi << 8) | (addr_bank << 16);

            if crate::debug_flags::trace_sta_long() {
                use std::sync::atomic::{AtomicU32, Ordering};
                static COUNT: AtomicU32 = AtomicU32::new(0);
                let n = COUNT.fetch_add(1, Ordering::Relaxed);
                if n < 64 {
                    println!(
                        "[STA_LONG] PB={:02X} PC={:04X} bytes={:02X} {:02X} {:02X} -> {:06X} A={:04X} M8={}",
                        state.pb,
                        pc_before,
                        addr_lo,
                        addr_hi,
                        addr_bank,
                        full_addr,
                        state.a,
                        state.p.contains(StatusFlags::MEMORY_8BIT)
                    );
                }
            }

            if state.p.contains(StatusFlags::MEMORY_8BIT) {
                bus.write_u8(full_addr, (state.a & 0xFF) as u8);
                add_cycles(state, 5);
                5
            } else {
                bus.write_u16(full_addr, state.a);
                add_cycles(state, 6);
                6
            }
        }

        0x42 => {
            // WDM (No operation on SA-1, but consume signature byte)
            read_u8_generic(state, bus); // Read and ignore signature byte
            add_cycles(state, 2);
            2
        }

        0x3A => {
            // DEC A (Decrement Accumulator)
            if state.p.contains(StatusFlags::MEMORY_8BIT) {
                state.a = ((state.a & 0xFF).wrapping_sub(1) & 0xFF) | (state.a & 0xFF00);
                set_flags_nz_8(state, (state.a & 0xFF) as u8);
            } else {
                state.a = state.a.wrapping_sub(1);
                set_flags_nz_16(state, state.a);
            }
            add_cycles(state, 2);
            2
        }

        0x3B => {
            // TSC - Transfer Stack Pointer to Accumulator
            state.a = state.sp;
            // TSC transfers to the 16-bit accumulator (C) and sets N/Z based on the 16-bit value,
            // regardless of the M flag.
            set_flags_nz_16(state, state.a);
            add_cycles(state, 2);
            2
        }

        0x9A => {
            // TXS (Transfer X to Stack Pointer)
            let old_sp = state.sp;
            if state.emulation_mode {
                state.sp = 0x0100 | (state.x & 0xFF);
            } else {
                state.sp = state.x;
            }
            if crate::debug_flags::trace_sp_change() {
                println!(
                    "SP CHANGE TXS PB={:02X} PC={:04X} {:04X}->{:04X}",
                    state.pb, state.pc, old_sp, state.sp
                );
            }
            add_cycles(state, 2);
            2
        }

        0x9B => {
            // TXY - Transfer X to Y
            state.y = state.x;
            set_flags_index(state, state.y);
            add_cycles(state, 2);
            2
        }

        // Missing opcodes used by some test/edge cases
        0x99 => {
            // STA absolute,Y
            let (addr, penalty) = read_absolute_y_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            write_a_generic(state, bus, addr);
            let base_cycles: u8 = 5;
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 2 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x9C => {
            // STZ absolute
            let addr = read_absolute_address_generic(state, bus);
            if memory_is_8bit(state) {
                bus.write_u8(addr, 0);
            } else {
                bus.write_u16(addr, 0);
            }
            let base_cycles: u8 = 4;
            let already_accounted: u8 = 3;
            add_cycles(state, base_cycles.saturating_sub(already_accounted));
            base_cycles
        }

        0x9D => {
            // STA absolute,X
            let (addr, penalty) = read_absolute_x_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            write_a_generic(state, bus, addr);
            let base_cycles: u8 = 5;
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 2 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x9E => {
            // STZ absolute,X
            let (addr, penalty) = read_absolute_x_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            if memory_is_8bit(state) {
                bus.write_u8(addr, 0);
            } else {
                bus.write_u16(addr, 0);
            }
            let base_cycles: u8 = 5;
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 2 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x9F => {
            // STA long absolute,X
            let addr_lo = read_u8_generic(state, bus) as u32;
            let addr_hi = read_u8_generic(state, bus) as u32;
            let addr_bank = read_u8_generic(state, bus) as u32;
            let full_addr =
                (addr_lo | (addr_hi << 8) | (addr_bank << 16)).wrapping_add(state.x as u32);

            if state.p.contains(StatusFlags::MEMORY_8BIT) {
                bus.write_u8(full_addr, (state.a & 0xFF) as u8);
            } else {
                bus.write_u16(full_addr, state.a);
            }
            add_cycles(state, 5);
            5
        }

        0xB1 => {
            // LDA (dp),Y
            let (addr, penalty) = read_indirect_y_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = memory_is_8bit(state);
            let value = read_operand_m(state, bus, addr, memory_8bit);
            if memory_8bit {
                state.a = (state.a & 0xFF00) | (value & 0xFF);
                set_flags_nz_8(state, value as u8);
            } else {
                state.a = value;
                set_flags_nz_16(state, value);
            }
            let base_cycles: u8 = 5;
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0xB2 => {
            // LDA (dp)
            let (addr, penalty) = read_indirect_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = memory_is_8bit(state);
            let value = read_operand_m(state, bus, addr, memory_8bit);
            if memory_8bit {
                state.a = (state.a & 0xFF00) | (value & 0xFF);
                set_flags_nz_8(state, value as u8);
            } else {
                state.a = value;
                set_flags_nz_16(state, value);
            }
            let base_cycles: u8 = 5;
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0xB7 => {
            // LDA [dp],Y
            let (addr, penalty) = read_indirect_long_y_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = memory_is_8bit(state);
            let value = read_operand_m(state, bus, addr, memory_8bit);
            if memory_8bit {
                state.a = (state.a & 0xFF00) | (value & 0xFF);
                set_flags_nz_8(state, value as u8);
            } else {
                state.a = value;
                set_flags_nz_16(state, value);
            }
            let base_cycles: u8 = 6;
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0xB5 => {
            // LDA direct page,X
            let (addr, penalty) = read_direct_x_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = memory_is_8bit(state);
            if memory_8bit {
                let value = bus.read_u8(addr) as u16;
                state.a = (state.a & 0xFF00) | value;
                set_flags_nz_8(state, value as u8);
            } else {
                let value = bus.read_u16(addr);
                state.a = value;
                set_flags_nz_16(state, value);
            }
            let base_cycles: u8 = if memory_8bit { 4 } else { 5 };
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x14 => {
            // TRB direct page
            let (addr, penalty) = read_direct_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = memory_is_8bit(state);
            if memory_8bit {
                let value = bus.read_u8(addr);
                let result = value & !(state.a as u8);
                bus.write_u8(addr, result);
                state
                    .p
                    .set(StatusFlags::ZERO, (value & (state.a as u8)) == 0);
            } else {
                let value = bus.read_u16(addr);
                let result = value & !state.a;
                bus.write_u16(addr, result);
                state.p.set(StatusFlags::ZERO, (value & state.a) == 0);
            }
            let base_cycles: u8 = if memory_8bit { 5 } else { 6 };
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x1C => {
            // TRB absolute
            let addr = read_absolute_address_generic(state, bus);
            if memory_is_8bit(state) {
                let value = bus.read_u8(addr);
                let a_low = (state.a & 0xFF) as u8;
                state.p.set(StatusFlags::ZERO, (value & a_low) == 0);
                bus.write_u8(addr, value & !a_low);
                let base_cycles: u8 = 6;
                let already_accounted: u8 = 3;
                add_cycles(state, base_cycles.saturating_sub(already_accounted));
                base_cycles
            } else {
                let value = bus.read_u16(addr);
                state.p.set(StatusFlags::ZERO, (value & state.a) == 0);
                bus.write_u16(addr, value & !state.a);
                let base_cycles: u8 = 8;
                let already_accounted: u8 = 3;
                add_cycles(state, base_cycles.saturating_sub(already_accounted));
                base_cycles
            }
        }

        0x88 => {
            // DEY
            if index_is_8bit(state) {
                let value = ((state.y & 0xFF).wrapping_sub(1)) as u8;
                state.y = (state.y & 0xFF00) | (value as u16);
                set_flags_nz_8(state, value);
            } else {
                state.y = state.y.wrapping_sub(1);
                set_flags_nz_16(state, state.y);
            }
            add_cycles(state, 2);
            2
        }

        0x8A => {
            // TXA (Transfer X to Accumulator)
            if state.p.contains(StatusFlags::MEMORY_8BIT) {
                state.a = (state.a & 0xFF00) | (state.x & 0xFF);
                set_flags_nz_8(state, (state.a & 0xFF) as u8);
            } else {
                state.a = state.x;
                set_flags_nz_16(state, state.a);
            }
            add_cycles(state, 2);
            2
        }

        0x98 => {
            // TYA (Transfer Y to Accumulator)
            if memory_is_8bit(state) {
                state.a = (state.a & 0xFF00) | (state.y & 0xFF);
                set_flags_nz_8(state, (state.a & 0xFF) as u8);
            } else {
                state.a = state.y;
                set_flags_nz_16(state, state.a);
            }
            add_cycles(state, 2);
            2
        }

        0xAD => {
            // LDA absolute
            let addr = read_absolute_address_generic(state, bus);
            if state.p.contains(StatusFlags::MEMORY_8BIT) {
                state.a = (state.a & 0xFF00) | (bus.read_u8(addr) as u16);
                set_flags_nz_8(state, (state.a & 0xFF) as u8);
            } else {
                state.a = bus.read_u16(addr);
                set_flags_nz_16(state, state.a);
            }
            add_cycles(state, 4);
            4
        }

        0xAF => {
            // LDA absolute long
            let addr = read_absolute_long_address_generic(state, bus);
            let memory_8bit = memory_is_8bit(state);
            let value = read_operand_m(state, bus, addr, memory_8bit);
            if memory_8bit {
                state.a = (state.a & 0xFF00) | (value & 0xFF);
                set_flags_nz_8(state, value as u8);
            } else {
                state.a = value;
                set_flags_nz_16(state, value);
            }
            let base_cycles: u8 = 5;
            let already_accounted: u8 = 3;
            add_cycles(state, base_cycles.saturating_sub(already_accounted));
            base_cycles
        }

        0xBD => {
            // LDA absolute,X with page-cross penalty
            let (addr, penalty) = read_absolute_x_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = state.emulation_mode || state.p.contains(StatusFlags::MEMORY_8BIT);
            if memory_8bit {
                let value = bus.read_u8(addr) as u16;
                state.a = (state.a & 0xFF00) | value;
                set_flags_nz_8(state, value as u8);
            } else {
                let value = bus.read_u16(addr);
                state.a = value;
                set_flags_nz_16(state, value);
            }
            let base_cycles: u8 = 4;
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 2 + penalty; // operand fetch (2) + penalty already applied
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0xBF => {
            // LDA absolute long,X
            let addr = read_absolute_long_x_address_generic(state, bus);
            let memory_8bit = memory_is_8bit(state);
            let value = read_operand_m(state, bus, addr, memory_8bit);
            if memory_8bit {
                state.a = (state.a & 0xFF00) | (value & 0xFF);
                set_flags_nz_8(state, value as u8);
            } else {
                state.a = value;
                set_flags_nz_16(state, value);
            }
            let base_cycles: u8 = 5;
            let already_accounted: u8 = 3;
            add_cycles(state, base_cycles.saturating_sub(already_accounted));
            base_cycles
        }

        0x28 => {
            // PLP (Pull Processor Status)
            let prev_p = state.p;
            let value = pop_u8_generic(state, bus);
            state.p = StatusFlags::from_bits_truncate(value);
            apply_status_side_effects_after_pull(state, prev_p);
            add_cycles(state, 4);
            4
        }

        0x5B => {
            // TCD (Transfer Accumulator to Direct Page)
            state.dp = state.a;
            set_flags_nz_16(state, state.dp);
            add_cycles(state, 2);
            2
        }

        0xBC => {
            // LDY absolute,X
            let (addr, penalty) = read_absolute_x_address_generic(state, bus);
            if state.p.contains(StatusFlags::INDEX_8BIT) {
                state.y = (state.y & 0xFF00) | (bus.read_u8(addr) as u16);
                set_flags_nz_8(state, (state.y & 0xFF) as u8);
                let base_cycles: u8 = 4;
                let total_cycles = base_cycles.saturating_add(penalty);
                let already_accounted: u8 = 2 + penalty;
                add_cycles(state, total_cycles.saturating_sub(already_accounted));
                total_cycles
            } else {
                state.y = bus.read_u16(addr);
                set_flags_nz_16(state, state.y);
                let base_cycles: u8 = 4; // LDX/LDY abs,X same timing both widths
                let total_cycles = base_cycles.saturating_add(penalty);
                let already_accounted: u8 = 2 + penalty;
                add_cycles(state, total_cycles.saturating_sub(already_accounted));
                total_cycles
            }
        }

        0x83 => {
            // STA stack relative,S
            let offset = read_u8_generic(state, bus) as u16;
            let addr = state.sp.wrapping_add(offset);
            if state.p.contains(StatusFlags::MEMORY_8BIT) {
                bus.write_u8(addr as u32, (state.a & 0xFF) as u8);
            } else {
                bus.write_u16(addr as u32, state.a);
            }
            add_cycles(state, 4);
            4
        }

        0x94 => {
            // STY zero page,X
            let (addr, penalty) = read_direct_x_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            write_y_generic(state, bus, addr);
            let base_cycles: u8 = 4;
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x96 => {
            // STX direct page,Y
            let (addr, penalty) = read_direct_y_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            write_x_generic(state, bus, addr);
            let base_cycles: u8 = if index_is_8bit(state) { 4 } else { 5 };
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        // Additional instructions for BW-RAM communication cases
        0x64 => {
            // STZ direct page
            let (addr, penalty) = read_direct_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            if memory_is_8bit(state) {
                bus.write_u8(addr, 0);
            } else {
                bus.write_u16(addr, 0);
            }
            let base_cycles: u8 = 3;
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x74 => {
            // STZ direct page,X
            let (addr, penalty) = read_direct_x_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            if memory_is_8bit(state) {
                bus.write_u8(addr, 0);
            } else {
                bus.write_u16(addr, 0);
            }
            let base_cycles: u8 = 4;
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x81 => {
            // STA (dp,X)
            let (addr, penalty) = read_indirect_x_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            write_a_generic(state, bus, addr);
            let base_cycles: u8 = 6;
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x87 => {
            // STA [dp]
            let (addr, penalty) = read_indirect_long_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            write_a_generic(state, bus, addr);
            let base_cycles: u8 = 6;
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x91 => {
            // STA (dp),Y
            let (addr, penalty) = read_indirect_y_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            write_a_generic(state, bus, addr);
            let base_cycles: u8 = 6;
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x92 => {
            // STA (dp)
            let (addr, penalty) = read_indirect_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            write_a_generic(state, bus, addr);
            let base_cycles: u8 = 5;
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x93 => {
            // STA (sr,S),Y
            let (addr, penalty) = read_stack_relative_indirect_y_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            write_a_generic(state, bus, addr);
            let base_cycles: u8 = 7;
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x95 => {
            // STA dp,X
            let (addr, penalty) = read_direct_x_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            write_a_generic(state, bus, addr);
            let base_cycles: u8 = 4;
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x97 => {
            // STA [dp],Y
            let (addr, penalty) = read_indirect_long_y_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            write_a_generic(state, bus, addr);
            let base_cycles: u8 = 6;
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x85 => {
            // STA direct page
            let (addr, penalty) = read_direct_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            write_a_generic(state, bus, addr);
            let base_cycles: u8 = if memory_is_8bit(state) { 3 } else { 4 };
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x86 => {
            // STX direct page
            let (addr, penalty) = read_direct_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            write_x_generic(state, bus, addr);
            let base_cycles: u8 = if index_is_8bit(state) { 3 } else { 4 };
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0x84 => {
            // STY direct page
            let (addr, penalty) = read_direct_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            write_y_generic(state, bus, addr);
            let base_cycles: u8 = if index_is_8bit(state) { 3 } else { 4 };
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0xC9 => {
            // CMP immediate
            if state.p.contains(StatusFlags::MEMORY_8BIT) {
                let value = read_u8_generic(state, bus);
                let result = (state.a as u8).wrapping_sub(value);
                state
                    .p
                    .set(StatusFlags::CARRY, (state.a & 0xFF) >= value as u16);
                state.p.set(StatusFlags::ZERO, result == 0);
                state.p.set(StatusFlags::NEGATIVE, (result & 0x80) != 0);
            } else {
                let value_lo = read_u8_generic(state, bus) as u16;
                let value_hi = read_u8_generic(state, bus) as u16;
                let value = value_lo | (value_hi << 8);
                let result = state.a.wrapping_sub(value);
                state.p.set(StatusFlags::CARRY, state.a >= value);
                state.p.set(StatusFlags::ZERO, result == 0);
                state.p.set(StatusFlags::NEGATIVE, (result & 0x8000) != 0);
            }
            add_cycles(state, 2);
            2
        }

        0xE0 => {
            // CPX immediate
            if state.p.contains(StatusFlags::INDEX_8BIT) {
                let value = read_u8_generic(state, bus);
                let result = (state.x as u8).wrapping_sub(value);
                state
                    .p
                    .set(StatusFlags::CARRY, (state.x & 0xFF) >= value as u16);
                state.p.set(StatusFlags::ZERO, result == 0);
                state.p.set(StatusFlags::NEGATIVE, (result & 0x80) != 0);
            } else {
                let value_lo = read_u8_generic(state, bus) as u16;
                let value_hi = read_u8_generic(state, bus) as u16;
                let value = value_lo | (value_hi << 8);
                let result = state.x.wrapping_sub(value);
                state.p.set(StatusFlags::CARRY, state.x >= value);
                state.p.set(StatusFlags::ZERO, result == 0);
                state.p.set(StatusFlags::NEGATIVE, (result & 0x8000) != 0);
            }
            add_cycles(state, 2);
            2
        }

        0xE6 => {
            // INC direct page
            let (addr, penalty) = read_direct_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            if memory_is_8bit(state) {
                let value = bus.read_u8(addr).wrapping_add(1);
                bus.write_u8(addr, value);
                set_flags_nz_8(state, value);
            } else {
                let value = bus.read_u16(addr).wrapping_add(1);
                bus.write_u16(addr, value);
                set_flags_nz_16(state, value);
            }
            let base_cycles: u8 = 5;
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0xE8 => {
            // INX
            if index_is_8bit(state) {
                let value = ((state.x & 0xFF).wrapping_add(1)) as u8;
                state.x = (state.x & 0xFF00) | (value as u16);
                set_flags_nz_8(state, value);
            } else {
                state.x = state.x.wrapping_add(1);
                set_flags_nz_16(state, state.x);
            }
            add_cycles(state, 2);
            2
        }

        0xEB => {
            // XBA - Exchange B and A
            let low = (state.a & 0xFF) as u8;
            let high = (state.a >> 8) as u8;
            state.a = ((low as u16) << 8) | (high as u16);
            let new_low = (state.a & 0xFF) as u8;
            state.p.set(StatusFlags::ZERO, new_low == 0);
            state.p.set(StatusFlags::NEGATIVE, (new_low & 0x80) != 0);
            add_cycles(state, 3);
            3
        }

        0xEC => {
            // CPX absolute
            let addr = read_absolute_address_generic(state, bus);
            if index_is_8bit(state) {
                let value = bus.read_u8(addr);
                let result = (state.x as u8).wrapping_sub(value);
                state
                    .p
                    .set(StatusFlags::CARRY, (state.x & 0xFF) >= value as u16);
                state.p.set(StatusFlags::ZERO, result == 0);
                state.p.set(StatusFlags::NEGATIVE, (result & 0x80) != 0);
            } else {
                let value = bus.read_u16(addr);
                let result = state.x.wrapping_sub(value);
                state.p.set(StatusFlags::CARRY, state.x >= value);
                state.p.set(StatusFlags::ZERO, result == 0);
                state.p.set(StatusFlags::NEGATIVE, (result & 0x8000) != 0);
            }
            add_cycles(state, 4);
            4
        }

        0xEE => {
            // INC absolute
            let addr = read_absolute_address_generic(state, bus);
            if memory_is_8bit(state) {
                let value = bus.read_u8(addr).wrapping_add(1);
                bus.write_u8(addr, value);
                set_flags_nz_8(state, value);
            } else {
                let value = bus.read_u16(addr).wrapping_add(1);
                bus.write_u16(addr, value);
                set_flags_nz_16(state, value);
            }
            add_cycles(state, 6);
            6
        }

        0xF2 => {
            // SBC (dp)
            let (addr, penalty) = read_indirect_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            let memory_8bit = memory_is_8bit(state);
            let operand = if memory_8bit {
                bus.read_u8(addr) as u16
            } else {
                bus.read_u16(addr)
            };
            sbc_generic(state, operand);
            let base_cycles: u8 = if memory_8bit { 5 } else { 6 };
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0xF6 => {
            // INC direct page,X
            let (addr, penalty) = read_direct_x_address_generic(state, bus);
            if penalty != 0 {
                add_cycles(state, penalty);
            }
            if memory_is_8bit(state) {
                let value = bus.read_u8(addr).wrapping_add(1);
                bus.write_u8(addr, value);
                set_flags_nz_8(state, value);
            } else {
                let value = bus.read_u16(addr).wrapping_add(1);
                bus.write_u16(addr, value);
                set_flags_nz_16(state, value);
            }
            let base_cycles: u8 = 6;
            let total_cycles = base_cycles.saturating_add(penalty);
            let already_accounted: u8 = 1 + penalty;
            add_cycles(state, total_cycles.saturating_sub(already_accounted));
            total_cycles
        }

        0xFC => {
            // JSR (addr,X)
            let base = read_u16_generic(state, bus);
            let addr = base.wrapping_add(state.x);
            // Indirect target fetch uses the current program bank (PB).
            let target = bus.read_u16(full_address(state, addr));
            let return_addr = state.pc.wrapping_sub(1);
            if state.emulation_mode {
                // Undocumented emulation edge: JSR (abs,X) uses a 16-bit stack decrement for the push.
                bus.write_u8(state.sp as u32, (return_addr >> 8) as u8);
                state.sp = state.sp.wrapping_sub(1);
                add_cycles(state, 1);
                bus.write_u8(state.sp as u32, (return_addr & 0xFF) as u8);
                state.sp = state.sp.wrapping_sub(1);
                add_cycles(state, 1);
                // Re-assert emulation-mode stack high byte after the sequence.
                state.sp = 0x0100 | (state.sp & 0x00FF);
            } else {
                push_u16_generic(state, bus, return_addr);
            }
            state.pc = target;
            let base_cycles: u8 = 8;
            let accounted: u8 = 2 + 2; // operand read + push
            add_cycles(state, base_cycles.saturating_sub(accounted));
            base_cycles
        }

        0xC0 => {
            // CPY immediate
            if state.p.contains(StatusFlags::INDEX_8BIT) {
                let value = read_u8_generic(state, bus);
                let result = (state.y as u8).wrapping_sub(value);
                state
                    .p
                    .set(StatusFlags::CARRY, (state.y & 0xFF) >= value as u16);
                state.p.set(StatusFlags::ZERO, result == 0);
                state.p.set(StatusFlags::NEGATIVE, (result & 0x80) != 0);
            } else {
                let value_lo = read_u8_generic(state, bus) as u16;
                let value_hi = read_u8_generic(state, bus) as u16;
                let value = value_lo | (value_hi << 8);
                let result = state.y.wrapping_sub(value);
                state.p.set(StatusFlags::CARRY, state.y >= value);
                state.p.set(StatusFlags::ZERO, result == 0);
                state.p.set(StatusFlags::NEGATIVE, (result & 0x8000) != 0);
            }
            add_cycles(state, 2);
            2
        }

        0xB8 => {
            // CLV (Clear Overflow)
            state.p.remove(StatusFlags::OVERFLOW);
            add_cycles(state, 2);
            2
        }
    }
}
