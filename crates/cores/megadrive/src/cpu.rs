use crate::memory::MemoryMap;
use std::collections::BTreeMap;

const CCR_C: u16 = 0x0001;
const CCR_V: u16 = 0x0002;
const CCR_Z: u16 = 0x0004;
const CCR_N: u16 = 0x0008;
const CCR_X: u16 = 0x0010;
const SR_TRACE: u16 = 0x8000;
const SR_INT_MASK: u16 = 0x0700;
const SR_SUPERVISOR: u16 = 0x2000;
const SR_VALID_MASK_68000: u16 =
    SR_TRACE | SR_SUPERVISOR | SR_INT_MASK | CCR_X | CCR_N | CCR_Z | CCR_V | CCR_C;

#[derive(Debug, Clone, bincode::Encode, bincode::Decode)]
pub struct M68k {
    d_regs: [u32; 8],
    a_regs: [u32; 8],
    usp: u32,
    ssp: u32,
    sr: u16,
    pc: u32,
    cycles: u64,
    stopped: bool,
    hard_halted: bool,
    unknown_opcode_total: u64,
    unknown_opcode_histogram: BTreeMap<u16, u64>,
    unknown_opcode_pc_histogram: BTreeMap<u32, u64>,
    exception_histogram: BTreeMap<u32, u64>,
    pending_exception_cycles: Option<u32>,
    pending_group0_frames: u32,
    pending_trace_exception: bool,
    exception_raised_this_step: bool,
    current_opcode: u16,
}

impl Default for M68k {
    fn default() -> Self {
        Self {
            d_regs: [0; 8],
            a_regs: [0; 8],
            usp: 0,
            ssp: 0,
            sr: 0x2700,
            pc: 0,
            cycles: 0,
            stopped: false,
            hard_halted: false,
            unknown_opcode_total: 0,
            unknown_opcode_histogram: BTreeMap::new(),
            unknown_opcode_pc_histogram: BTreeMap::new(),
            exception_histogram: BTreeMap::new(),
            pending_exception_cycles: None,
            pending_group0_frames: 0,
            pending_trace_exception: false,
            exception_raised_this_step: false,
            current_opcode: 0,
        }
    }
}

impl M68k {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn reset(&mut self, memory: &mut MemoryMap) {
        // Initial SSP/PC are taken from vectors at 0x000000 and 0x000004.
        self.d_regs = [0; 8];
        self.a_regs = [0; 8];
        self.usp = 0;
        self.ssp = memory.read_u32(0x000000);
        self.sr = 0x2700;
        self.a_regs[7] = self.ssp;
        self.pc = memory.read_u32(0x000004);
        self.cycles = 0;
        self.stopped = false;
        self.hard_halted = false;
        self.unknown_opcode_total = 0;
        self.unknown_opcode_histogram.clear();
        self.unknown_opcode_pc_histogram.clear();
        self.exception_histogram.clear();
        self.pending_exception_cycles = None;
        self.pending_group0_frames = 0;
        self.pending_trace_exception = false;
        self.exception_raised_this_step = false;
        self.current_opcode = 0;
    }

    pub fn step(&mut self, memory: &mut MemoryMap) -> u32 {
        if self.hard_halted {
            let cycles = memory.take_dma_wait_cycles();
            self.cycles += cycles as u64;
            return cycles;
        }

        if self.pending_trace_exception {
            self.pending_trace_exception = false;
            self.exception_raised_this_step = false;
            self.raise_exception(9, memory, None);
            let cycles = 34u32.saturating_add(memory.take_dma_wait_cycles());
            self.cycles += cycles as u64;
            return cycles;
        }

        if let Some(level) = memory.pending_interrupt_level() {
            if self.service_interrupt(level, memory) {
                memory.acknowledge_interrupt(level);
                let cycles = 44u32.saturating_add(memory.take_dma_wait_cycles());
                self.cycles += cycles as u64;
                return cycles;
            }
        }

        if self.stopped {
            let cycles = 4u32.saturating_add(memory.take_dma_wait_cycles());
            self.cycles += cycles as u64;
            return cycles;
        }

        self.exception_raised_this_step = false;
        // MC68000 traps on odd-PC instruction fetches.
        if (self.pc & 1) != 0 {
            let cycles = self
                .exec_address_error(memory)
                .saturating_add(memory.take_dma_wait_cycles());
            self.cycles += cycles as u64;
            return cycles;
        }

        let opcode = self.fetch_u16(memory);
        self.current_opcode = opcode;
        macro_rules! opt_cycles {
            ($expr:expr) => {{
                match $expr {
                    Some(cycles) => cycles,
                    None => match self.take_pending_exception_cycles() {
                        Some(cycles) => cycles,
                        None => self.exec_unknown_as_illegal(opcode, memory),
                    },
                }
            }};
        }

        let cycles = match opcode {
            0x4E71 => 4, // NOP
            0x4E70 => self.exec_reset(memory),
            0x4E75 => self.exec_rts(memory),
            0x4E73 => self.exec_rte(memory),
            0x4E72 => self.exec_stop(memory),
            0x4E76 => self.exec_trapv(memory),
            0x4E77 => self.exec_rtr(memory),
            0x4AFC => self.exec_illegal(memory),
            _ if (opcode & 0xFFF0) == 0x4E60 => self.exec_move_usp(opcode, memory),
            _ if (opcode & 0xFFF8) == 0x4E50 => self.exec_link(opcode, memory),
            _ if (opcode & 0xFFF8) == 0x4E58 => self.exec_unlk(opcode, memory),
            _ if (opcode & 0xFFC0) == 0x4E80 => opt_cycles!(self.exec_jsr(opcode, memory)),
            _ if (opcode & 0xFFC0) == 0x4EC0 => opt_cycles!(self.exec_jmp(opcode, memory)),
            _ if (opcode & 0xFFC0) == 0x40C0 => opt_cycles!(self.exec_move_from_sr(opcode, memory)),
            _ if (opcode & 0xFFC0) == 0x46C0 => opt_cycles!(self.exec_move_to_sr(opcode, memory)),
            _ if (opcode & 0xFFF8) == 0x4848 => self.exec_bkpt_68000(memory),
            _ if (opcode & 0xFFC0) == 0x4840 && ((opcode >> 3) & 0x7) != 0b000 => {
                opt_cycles!(self.exec_pea(opcode, memory))
            }
            _ if (opcode & 0xFFF8) == 0x4840 => self.exec_swap(opcode),
            _ if (opcode & 0xFFF8) == 0x4880 => self.exec_ext_w(opcode),
            _ if (opcode & 0xFFF8) == 0x48C0 => self.exec_ext_l(opcode),
            _ if (opcode & 0xFFC0) == 0x4800 => opt_cycles!(self.exec_nbcd(opcode, memory)),
            _ if (opcode & 0xFB80) == 0x4880 && ((opcode >> 3) & 0x7) >= 0b010 => {
                opt_cycles!(self.exec_movem(opcode, memory))
            }
            _ if (opcode & 0xFFF0) == 0x4E40 => self.exec_trap(opcode, memory),
            _ if (opcode & 0xFF00) == 0x4000 => opt_cycles!(self.exec_negx(opcode, memory)),
            _ if (opcode & 0xFF00) == 0x6000 => self.exec_branch(opcode, memory, 0x0),
            _ if (opcode & 0xFF00) == 0x6100 => self.exec_bsr(opcode, memory),
            _ if (opcode & 0xFF00) == 0x6600 => self.exec_branch(opcode, memory, 0x6),
            _ if (opcode & 0xFF00) == 0x6700 => self.exec_branch(opcode, memory, 0x7),
            _ if (opcode & 0xF000) == 0x6000 => opt_cycles!(self.exec_bcc(opcode, memory)),
            _ if (opcode & 0xF000) == 0x5000 => opt_cycles!(self.exec_addq_subq(opcode, memory)),
            _ if (opcode & 0xF100) == 0x7000 => self.exec_moveq(opcode),
            _ if (opcode & 0xFFC0) == 0x44C0 => opt_cycles!(self.exec_move_to_ccr(opcode, memory)),
            _ if (opcode & 0xFF00) == 0x4200 => opt_cycles!(self.exec_clr(opcode, memory)),
            _ if (opcode & 0xFF00) == 0x4400 => opt_cycles!(self.exec_neg(opcode, memory)),
            _ if (opcode & 0xFF00) == 0x4600 => opt_cycles!(self.exec_not(opcode, memory)),
            _ if (opcode & 0xF138) == 0x0108 => opt_cycles!(self.exec_movep(opcode, memory)),
            _ if (opcode & 0xF100) == 0x0100 => opt_cycles!(self.exec_bit_dynamic(opcode, memory)),
            _ if (opcode & 0xFF00) == 0x0800 => {
                opt_cycles!(self.exec_bit_immediate(opcode, memory))
            }
            0x003C => self.exec_ori_to_ccr(memory),
            0x007C => self.exec_ori_to_sr(memory),
            _ if (opcode & 0xFF00) == 0x0000 => opt_cycles!(self.exec_ori(opcode, memory)),
            0x023C => self.exec_andi_to_ccr(memory),
            0x027C => self.exec_andi_to_sr(memory),
            _ if (opcode & 0xFF00) == 0x0400 => opt_cycles!(self.exec_subi(opcode, memory)),
            _ if (opcode & 0xFF00) == 0x0200 => opt_cycles!(self.exec_andi(opcode, memory)),
            _ if (opcode & 0xFF00) == 0x0600 => opt_cycles!(self.exec_addi(opcode, memory)),
            0x0A3C => self.exec_eori_to_ccr(memory),
            0x0A7C => self.exec_eori_to_sr(memory),
            _ if (opcode & 0xFF00) == 0x0A00 => opt_cycles!(self.exec_eori(opcode, memory)),
            _ if (opcode & 0xFF00) == 0x0C00 => opt_cycles!(self.exec_cmpi(opcode, memory)),
            _ if (opcode & 0xF138) == 0xB108 && ((opcode >> 6) & 0x3) != 0b11 => {
                opt_cycles!(self.exec_cmpm(opcode, memory))
            }
            _ if (opcode & 0xF1C0) == 0xB0C0 => opt_cycles!(self.exec_cmpa_w(opcode, memory)),
            _ if (opcode & 0xF1C0) == 0xB1C0 => opt_cycles!(self.exec_cmpa_l(opcode, memory)),
            _ if (opcode & 0xF000) == 0xB000 => opt_cycles!(self.exec_cmp_ea_to_dn(opcode, memory)),
            _ if (opcode & 0xFFC0) == 0x4AC0 => opt_cycles!(self.exec_tas(opcode, memory)),
            _ if (opcode & 0xFF00) == 0x4A00 => opt_cycles!(self.exec_tst(opcode, memory)),
            _ if (opcode & 0xF1C0) == 0x3040 => opt_cycles!(self.exec_movea_w(opcode, memory)),
            _ if (opcode & 0xF1C0) == 0x2040 => opt_cycles!(self.exec_movea_l(opcode, memory)),
            _ if (opcode & 0xF1C0) == 0x4180 => opt_cycles!(self.exec_chk_w(opcode, memory)),
            _ if (opcode & 0xF1C0) == 0x80C0 => opt_cycles!(self.exec_divu_w(opcode, memory)),
            _ if (opcode & 0xF1C0) == 0x81C0 => opt_cycles!(self.exec_divs_w(opcode, memory)),
            _ if (opcode & 0xF1F8) == 0x8108 => opt_cycles!(self.exec_sbcd(opcode, memory)),
            _ if (opcode & 0xF000) == 0x8000 => opt_cycles!(self.exec_or_ea_to_dn(opcode, memory)),
            _ if (opcode & 0xF1F8) == 0xC108 => opt_cycles!(self.exec_abcd(opcode, memory)),
            _ if (opcode & 0xF1F8) == 0xC140 => self.exec_exg_dd(opcode),
            _ if (opcode & 0xF1F8) == 0xC148 => self.exec_exg_aa(opcode),
            _ if (opcode & 0xF1F8) == 0xC188 => self.exec_exg_da(opcode),
            _ if (opcode & 0xF1C0) == 0xC0C0 => opt_cycles!(self.exec_mulu_w(opcode, memory)),
            _ if (opcode & 0xF1C0) == 0xC1C0 => opt_cycles!(self.exec_muls_w(opcode, memory)),
            _ if (opcode & 0xF000) == 0xC000 => opt_cycles!(self.exec_and_ea_to_dn(opcode, memory)),
            _ if (opcode & 0xF1C0) == 0xD0C0 => opt_cycles!(self.exec_adda_w(opcode, memory)),
            _ if (opcode & 0xF1C0) == 0xD1C0 => opt_cycles!(self.exec_adda_l(opcode, memory)),
            _ if (opcode & 0xF1C0) == 0x90C0 => opt_cycles!(self.exec_suba_w(opcode, memory)),
            _ if (opcode & 0xF1C0) == 0x91C0 => opt_cycles!(self.exec_suba_l(opcode, memory)),
            _ if (opcode & 0xF130) == 0x9100 => opt_cycles!(self.exec_subx(opcode, memory)),
            _ if (opcode & 0xF130) == 0xD100 => opt_cycles!(self.exec_addx(opcode, memory)),
            _ if (opcode & 0xF000) == 0x9000 => opt_cycles!(self.exec_sub_ea_to_dn(opcode, memory)),
            _ if (opcode & 0xF000) == 0xD000 => opt_cycles!(self.exec_add_ea_to_dn(opcode, memory)),
            _ if (opcode & 0xF000) == 0xE000 => opt_cycles!(self.exec_shift_rotate(opcode, memory)),
            _ if (opcode & 0xF1FF) == 0x203C => self.exec_move_l_imm_dn(opcode, memory),
            _ if (opcode & 0xFFF8) == 0x23C0 => self.exec_move_l_dn_abs_l(opcode, memory),
            _ if (opcode & 0xF1C0) == 0x41C0 => opt_cycles!(self.exec_lea(opcode, memory)),
            _ if (opcode & 0xF000) == 0x1000 => {
                opt_cycles!(self.exec_move_b_family(opcode, memory))
            }
            _ if (opcode & 0xF000) == 0x3000 => {
                opt_cycles!(self.exec_move_w_family(opcode, memory))
            }
            _ if opcode == 0x23FC => self.exec_move_l_imm_abs_l(memory),
            _ if (opcode & 0xF000) == 0x2000 => {
                opt_cycles!(self.exec_move_l_family(opcode, memory))
            }
            _ if (opcode & 0xF000) == 0xA000 => self.exec_line_a(memory),
            _ if (opcode & 0xF000) == 0xF000 => self.exec_line_f(memory),
            _ => self.exec_unknown_as_illegal(opcode, memory),
        };

        if !self.exception_raised_this_step
            && !self.stopped
            && !self.hard_halted
            && (self.sr & SR_TRACE) != 0
        {
            self.pending_trace_exception = true;
        }

        let total_cycles = cycles.saturating_add(memory.take_dma_wait_cycles());
        self.cycles += total_cycles as u64;
        total_cycles
    }

    pub fn pc(&self) -> u32 {
        self.pc
    }

    pub fn cycles(&self) -> u64 {
        self.cycles
    }

    pub fn a7(&self) -> u32 {
        self.a_regs[7]
    }

    pub fn d_reg(&self, index: usize) -> u32 {
        self.d_regs[index]
    }

    pub fn a_reg(&self, index: usize) -> u32 {
        self.a_regs[index]
    }

    pub fn sr_raw(&self) -> u16 {
        self.sr
    }

    pub fn unknown_opcode_total(&self) -> u64 {
        self.unknown_opcode_total
    }

    pub fn unknown_opcode_histogram(&self) -> Vec<(u16, u64)> {
        let mut entries: Vec<(u16, u64)> = self
            .unknown_opcode_histogram
            .iter()
            .map(|(opcode, count)| (*opcode, *count))
            .collect();
        entries.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        entries
    }

    pub fn unknown_opcode_pc_histogram(&self) -> Vec<(u32, u64)> {
        let mut entries: Vec<(u32, u64)> = self
            .unknown_opcode_pc_histogram
            .iter()
            .map(|(pc, count)| (*pc, *count))
            .collect();
        entries.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        entries
    }

    pub fn exception_histogram(&self) -> Vec<(u32, u64)> {
        let mut entries: Vec<(u32, u64)> = self
            .exception_histogram
            .iter()
            .map(|(vector, count)| (*vector, *count))
            .collect();
        entries.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        entries
    }

    #[cfg(test)]
    pub fn sr(&self) -> u16 {
        self.sr
    }

    fn exec_move_b_family(&mut self, opcode: u16, memory: &mut MemoryMap) -> Option<u32> {
        let dst_reg = ((opcode >> 9) & 0x7) as usize;
        let dst_mode = ((opcode >> 6) & 0x7) as u8;
        let src_mode = ((opcode >> 3) & 0x7) as u8;
        let src_reg = (opcode & 0x7) as usize;

        // Destination for MOVE.B cannot be An direct or immediate.
        if dst_mode == 0b001 || (dst_mode == 0b111 && dst_reg == 0b100) {
            return None;
        }

        let src_ea = self.word_ea_calculation_cycles(src_mode, src_reg)?;
        let src = self.read_ea_byte(src_mode, src_reg, memory)?;
        self.write_ea_byte(dst_mode, dst_reg, src, memory)?;
        self.update_move_flags_byte(src);

        let base = Self::move_dest_base_cycles(dst_mode, dst_reg, false);
        Some(base + src_ea)
    }

    fn exec_move_w_family(&mut self, opcode: u16, memory: &mut MemoryMap) -> Option<u32> {
        let dst_reg = ((opcode >> 9) & 0x7) as usize;
        let dst_mode = ((opcode >> 6) & 0x7) as u8;
        let src_mode = ((opcode >> 3) & 0x7) as u8;
        let src_reg = (opcode & 0x7) as usize;

        // Destination for MOVE.W cannot be An direct or immediate.
        if dst_mode == 0b001 || (dst_mode == 0b111 && dst_reg == 0b100) {
            return None;
        }

        let src_ea = self.word_ea_calculation_cycles(src_mode, src_reg)?;
        let src = self.read_ea_word(src_mode, src_reg, memory)?;
        self.write_ea_word(dst_mode, dst_reg, src, memory)?;
        self.update_move_flags_word(src);

        let base = Self::move_dest_base_cycles(dst_mode, dst_reg, false);
        Some(base + src_ea)
    }

    fn exec_move_l_imm_abs_l(&mut self, memory: &mut MemoryMap) -> u32 {
        let value = self.fetch_u32(memory);
        let dst = self.fetch_u32(memory);
        memory.write_u32(dst, value);
        self.update_move_flags_long(value);
        28 // MOVE.L #imm,xxx.L: dest_base(xxx.L,long)=20 + src_ea_long(#imm)=8
    }

    fn exec_move_l_family(&mut self, opcode: u16, memory: &mut MemoryMap) -> Option<u32> {
        let dst_reg = ((opcode >> 9) & 0x7) as usize;
        let dst_mode = ((opcode >> 6) & 0x7) as u8;
        let src_mode = ((opcode >> 3) & 0x7) as u8;
        let src_reg = (opcode & 0x7) as usize;

        // Destination for MOVE.L cannot be An direct or immediate.
        if dst_mode == 0b001 || (dst_mode == 0b111 && dst_reg == 0b100) {
            return None;
        }

        let src_ea = self.long_ea_calculation_cycles(src_mode, src_reg)?;
        let src = self.read_ea_long(src_mode, src_reg, memory)?;
        self.write_ea_long(dst_mode, dst_reg, src, memory)?;
        self.update_move_flags_long(src);

        let base = Self::move_dest_base_cycles(dst_mode, dst_reg, true);
        Some(base + src_ea)
    }

    fn exec_move_l_imm_dn(&mut self, opcode: u16, memory: &mut MemoryMap) -> u32 {
        let dst = ((opcode >> 9) & 0x7) as usize;
        let value = self.fetch_u32(memory);
        self.d_regs[dst] = value;
        self.update_move_flags_long(value);
        12
    }

    fn exec_move_l_dn_abs_l(&mut self, opcode: u16, memory: &mut MemoryMap) -> u32 {
        let src = (opcode & 0x7) as usize;
        let dst = self.fetch_u32(memory);
        let value = self.d_regs[src];
        memory.write_u32(dst, value);
        self.update_move_flags_long(value);
        20 // MOVE.L Dn,xxx.L: dest_base(xxx.L,long)=20 + src_ea(Dn)=0
    }

    fn exec_moveq(&mut self, opcode: u16) -> u32 {
        let dst = ((opcode >> 9) & 0x7) as usize;
        let imm = (opcode & 0x00FF) as u8 as i8 as i32 as u32;
        self.d_regs[dst] = imm;
        self.update_move_flags_long(imm);
        4
    }

    fn exec_movea_w(&mut self, opcode: u16, memory: &mut MemoryMap) -> Option<u32> {
        let dst = ((opcode >> 9) & 0x7) as usize;
        let mode = ((opcode >> 3) & 0x7) as u8;
        let reg = (opcode & 0x7) as usize;
        let ea_cycles = self.word_ea_calculation_cycles(mode, reg)?;
        let value = self.read_ea_word(mode, reg, memory)? as i16 as i32 as u32;
        self.a_regs[dst] = value;
        Some(4 + ea_cycles) // MOVEA.W: 4 + ea
    }

    fn exec_movea_l(&mut self, opcode: u16, memory: &mut MemoryMap) -> Option<u32> {
        let dst = ((opcode >> 9) & 0x7) as usize;
        let mode = ((opcode >> 3) & 0x7) as u8;
        let reg = (opcode & 0x7) as usize;
        let ea_cycles = self.long_ea_calculation_cycles(mode, reg)?;
        let value = self.read_ea_long(mode, reg, memory)?;
        self.a_regs[dst] = value;
        Some(4 + ea_cycles) // MOVEA.L: 4 + ea(long)
    }

    fn exec_sub_ea_to_dn(&mut self, opcode: u16, memory: &mut MemoryMap) -> Option<u32> {
        self.exec_add_sub_ea_to_dn(opcode, memory, ArithOp::Sub)
    }

    fn exec_add_ea_to_dn(&mut self, opcode: u16, memory: &mut MemoryMap) -> Option<u32> {
        self.exec_add_sub_ea_to_dn(opcode, memory, ArithOp::Add)
    }

    fn exec_abcd(&mut self, opcode: u16, memory: &mut MemoryMap) -> Option<u32> {
        self.exec_bcd_arith(opcode, memory, true)
    }

    fn exec_sbcd(&mut self, opcode: u16, memory: &mut MemoryMap) -> Option<u32> {
        self.exec_bcd_arith(opcode, memory, false)
    }

    fn exec_exg_dd(&mut self, opcode: u16) -> u32 {
        let rx = ((opcode >> 9) & 0x7) as usize;
        let ry = (opcode & 0x7) as usize;
        self.d_regs.swap(rx, ry);
        6
    }

    fn exec_exg_aa(&mut self, opcode: u16) -> u32 {
        let rx = ((opcode >> 9) & 0x7) as usize;
        let ry = (opcode & 0x7) as usize;
        self.a_regs.swap(rx, ry);
        6
    }

    fn exec_exg_da(&mut self, opcode: u16) -> u32 {
        let dx = ((opcode >> 9) & 0x7) as usize;
        let ay = (opcode & 0x7) as usize;
        let d = self.d_regs[dx];
        self.d_regs[dx] = self.a_regs[ay];
        self.a_regs[ay] = d;
        6
    }

    fn exec_bcd_arith(&mut self, opcode: u16, memory: &mut MemoryMap, add: bool) -> Option<u32> {
        let dst = ((opcode >> 9) & 0x7) as usize;
        let src = (opcode & 0x7) as usize;
        let mem_mode = (opcode & 0x0008) != 0;
        let x_in = if self.flag_set(CCR_X) { 1i32 } else { 0i32 };

        let (src_byte, dst_byte, dst_addr) = if mem_mode {
            self.a_regs[src] = self.a_regs[src].wrapping_sub(self.byte_addr_step(src));
            let src_addr = self.a_regs[src];
            self.a_regs[dst] = self.a_regs[dst].wrapping_sub(self.byte_addr_step(dst));
            let dst_addr = self.a_regs[dst];
            (
                memory.read_u8(src_addr),
                memory.read_u8(dst_addr),
                Some(dst_addr),
            )
        } else {
            (self.d_regs[src] as u8, self.d_regs[dst] as u8, None)
        };

        let src_dec = ((src_byte >> 4) as i32) * 10 + (src_byte & 0x0F) as i32;
        let dst_dec = ((dst_byte >> 4) as i32) * 10 + (dst_byte & 0x0F) as i32;
        let (result_dec, carry_or_borrow) = if add {
            let sum = dst_dec + src_dec + x_in;
            (sum % 100, sum > 99)
        } else {
            let mut diff = dst_dec - src_dec - x_in;
            let borrow = diff < 0;
            if borrow {
                diff += 100;
            }
            (diff, borrow)
        };

        let result = (((result_dec / 10) as u8) << 4) | ((result_dec % 10) as u8);
        if let Some(addr) = dst_addr {
            memory.write_u8(addr, result);
        } else {
            self.d_regs[dst] = (self.d_regs[dst] & 0xFFFF_FF00) | result as u32;
        }

        self.set_flag(CCR_C, carry_or_borrow);
        self.set_flag(CCR_X, carry_or_borrow);
        self.set_flag(CCR_V, false);
        self.set_flag(CCR_N, (result & 0x80) != 0);
        if result != 0 {
            self.sr &= !CCR_Z;
        }

        Some(if mem_mode { 18 } else { 6 })
    }

    fn exec_or_ea_to_dn(&mut self, opcode: u16, memory: &mut MemoryMap) -> Option<u32> {
        self.exec_logic_ea_to_dn(opcode, memory, LogicOp::Or)
    }

    fn exec_and_ea_to_dn(&mut self, opcode: u16, memory: &mut MemoryMap) -> Option<u32> {
        self.exec_logic_ea_to_dn(opcode, memory, LogicOp::And)
    }

    fn exec_logic_ea_to_dn(
        &mut self,
        opcode: u16,
        memory: &mut MemoryMap,
        op: LogicOp,
    ) -> Option<u32> {
        let reg_x = ((opcode >> 9) & 0x7) as usize;
        let opmode = ((opcode >> 6) & 0x7) as u8;
        let mode = ((opcode >> 3) & 0x7) as u8;
        let reg = (opcode & 0x7) as usize;

        if (0b100..=0b110).contains(&opmode) {
            return self.exec_logic_dn_to_ea(reg_x, opmode, mode, reg, memory, op);
        }
        if opmode > 0b010 {
            return None;
        }
        // Logical source for <ea>,Dn cannot be An direct.
        if mode == 0b001 {
            return None;
        }

        match opmode {
            0b000 => {
                let ea_cycles = self.word_ea_calculation_cycles(mode, reg)?;
                let src = self.read_ea_byte(mode, reg, memory)?;
                let dst_val = self.d_regs[reg_x] as u8;
                let result = match op {
                    LogicOp::And => dst_val & src,
                    LogicOp::Or => dst_val | src,
                };
                self.d_regs[reg_x] = (self.d_regs[reg_x] & 0xFFFF_FF00) | result as u32;
                self.update_test_flags_byte(result);
                Some(4 + ea_cycles)
            }
            0b001 => {
                let ea_cycles = self.word_ea_calculation_cycles(mode, reg)?;
                let src = self.read_ea_word(mode, reg, memory)?;
                let dst_val = self.d_regs[reg_x] as u16;
                let result = match op {
                    LogicOp::And => dst_val & src,
                    LogicOp::Or => dst_val | src,
                };
                self.d_regs[reg_x] = (self.d_regs[reg_x] & 0xFFFF_0000) | result as u32;
                self.update_test_flags_word(result);
                Some(4 + ea_cycles)
            }
            0b010 => {
                let src = self.read_ea_long(mode, reg, memory)?;
                let dst_val = self.d_regs[reg_x];
                let result = match op {
                    LogicOp::And => dst_val & src,
                    LogicOp::Or => dst_val | src,
                };
                self.d_regs[reg_x] = result;
                self.update_test_flags_long(result);
                // Long to Dn: Dn/An/#imm → 8+word_ea; memory → 6+long_ea
                if mode == 0b000 || mode == 0b001 || (mode == 0b111 && reg == 0b100) {
                    let ea_cycles = self.word_ea_calculation_cycles(mode, reg)?;
                    Some(8 + ea_cycles)
                } else {
                    let ea_cycles = self.long_ea_calculation_cycles(mode, reg)?;
                    Some(6 + ea_cycles)
                }
            }
            _ => None,
        }
    }

    fn exec_logic_dn_to_ea(
        &mut self,
        src_dn: usize,
        opmode: u8,
        mode: u8,
        reg: usize,
        memory: &mut MemoryMap,
        op: LogicOp,
    ) -> Option<u32> {
        // Destination EA for AND/OR Dn,<ea> must be data alterable.
        if mode == 0b001 || (mode == 0b111 && reg >= 0b010) {
            return None;
        }

        match opmode {
            0b100 => {
                let ea_cycles = self.word_ea_calculation_cycles(mode, reg)?;
                let src = self.d_regs[src_dn] as u8;
                let result = if mode == 0b000 {
                    let dst = self.d_regs[reg] as u8;
                    let result = match op {
                        LogicOp::And => dst & src,
                        LogicOp::Or => dst | src,
                    };
                    self.d_regs[reg] = (self.d_regs[reg] & 0xFFFF_FF00) | result as u32;
                    result
                } else {
                    let addr = self.resolve_data_alterable_address(mode, reg, 1, memory)?;
                    let dst = memory.read_u8(addr);
                    let result = match op {
                        LogicOp::And => dst & src,
                        LogicOp::Or => dst | src,
                    };
                    memory.write_u8(addr, result);
                    result
                };
                self.update_test_flags_byte(result);
                // AND/OR Dn,<ea> byte: Dn→4, memory→8+ea
                Some(if mode == 0b000 { 4 } else { 8 + ea_cycles })
            }
            0b101 => {
                let ea_cycles = self.word_ea_calculation_cycles(mode, reg)?;
                let src = self.d_regs[src_dn] as u16;
                let result = if mode == 0b000 {
                    let dst = self.d_regs[reg] as u16;
                    let result = match op {
                        LogicOp::And => dst & src,
                        LogicOp::Or => dst | src,
                    };
                    self.d_regs[reg] = (self.d_regs[reg] & 0xFFFF_0000) | result as u32;
                    result
                } else {
                    let addr = self.resolve_data_alterable_address(mode, reg, 2, memory)?;
                    let dst = memory.read_u16(addr);
                    let result = match op {
                        LogicOp::And => dst & src,
                        LogicOp::Or => dst | src,
                    };
                    memory.write_u16(addr, result);
                    result
                };
                self.update_test_flags_word(result);
                // AND/OR Dn,<ea> word: Dn→4, memory→8+ea
                Some(if mode == 0b000 { 4 } else { 8 + ea_cycles })
            }
            0b110 => {
                let ea_cycles = self.word_ea_calculation_cycles(mode, reg)?;
                let src = self.d_regs[src_dn];
                let result = if mode == 0b000 {
                    let dst = self.d_regs[reg];
                    let result = match op {
                        LogicOp::And => dst & src,
                        LogicOp::Or => dst | src,
                    };
                    self.d_regs[reg] = result;
                    result
                } else {
                    let addr = self.resolve_data_alterable_address(mode, reg, 4, memory)?;
                    let dst = memory.read_u32(addr);
                    let result = match op {
                        LogicOp::And => dst & src,
                        LogicOp::Or => dst | src,
                    };
                    memory.write_u32(addr, result);
                    result
                };
                self.update_test_flags_long(result);
                // AND/OR Dn,<ea> long: Dn→8, memory→12+ea
                Some(if mode == 0b000 { 8 } else { 12 + ea_cycles })
            }
            _ => None,
        }
    }

    fn exec_add_sub_ea_to_dn(
        &mut self,
        opcode: u16,
        memory: &mut MemoryMap,
        op: ArithOp,
    ) -> Option<u32> {
        let dst = ((opcode >> 9) & 0x7) as usize;
        let opmode = ((opcode >> 6) & 0x7) as u8;
        let mode = ((opcode >> 3) & 0x7) as u8;
        let reg = (opcode & 0x7) as usize;

        match opmode {
            0b000 => {
                let ea_cycles = self.word_ea_calculation_cycles(mode, reg)?;
                let src = self.read_ea_byte(mode, reg, memory)?;
                let dst_val = self.d_regs[dst] as u8;
                match op {
                    ArithOp::Add => {
                        let (result, carry) = dst_val.overflowing_add(src);
                        let overflow = ((!(dst_val ^ src)) & (dst_val ^ result) & 0x80) != 0;
                        self.d_regs[dst] = (self.d_regs[dst] & 0xFFFF_FF00) | result as u32;
                        self.update_add_flags_byte_with_extend(result, carry, overflow);
                    }
                    ArithOp::Sub => {
                        let (result, _) = dst_val.overflowing_sub(src);
                        self.d_regs[dst] = (self.d_regs[dst] & 0xFFFF_FF00) | result as u32;
                        self.update_sub_flags_byte_with_extend(dst_val, src, result);
                    }
                }
                Some(4 + ea_cycles) // ADD/SUB byte <ea>,Dn: 4+ea
            }
            0b001 => {
                let ea_cycles = self.word_ea_calculation_cycles(mode, reg)?;
                let src = self.read_ea_word(mode, reg, memory)?;
                let dst_val = self.d_regs[dst] as u16;
                match op {
                    ArithOp::Add => {
                        let (result, carry) = dst_val.overflowing_add(src);
                        let overflow = ((!(dst_val ^ src)) & (dst_val ^ result) & 0x8000) != 0;
                        self.d_regs[dst] = (self.d_regs[dst] & 0xFFFF_0000) | result as u32;
                        self.update_add_flags_word_with_extend(result, carry, overflow);
                    }
                    ArithOp::Sub => {
                        let result = dst_val.wrapping_sub(src);
                        self.d_regs[dst] = (self.d_regs[dst] & 0xFFFF_0000) | result as u32;
                        self.update_sub_flags_word_with_extend(dst_val, src, result);
                    }
                }
                Some(4 + ea_cycles) // ADD/SUB word <ea>,Dn: 4+ea
            }
            0b010 => {
                let src = self.read_ea_long(mode, reg, memory)?;
                let dst_val = self.d_regs[dst];
                match op {
                    ArithOp::Add => {
                        let (result, carry) = dst_val.overflowing_add(src);
                        let overflow = ((!(dst_val ^ src)) & (dst_val ^ result) & 0x8000_0000) != 0;
                        self.d_regs[dst] = result;
                        self.update_add_flags_long_with_extend(result, carry, overflow);
                    }
                    ArithOp::Sub => {
                        let result = dst_val.wrapping_sub(src);
                        self.d_regs[dst] = result;
                        self.update_sub_flags_long_with_extend(dst_val, src, result);
                    }
                }
                // ADD/SUB long <ea>,Dn: Dn/An/#imm → 8+word_ea; memory → 6+long_ea
                if mode == 0b000 || mode == 0b001 || (mode == 0b111 && reg == 0b100) {
                    let ea_cycles = self.word_ea_calculation_cycles(mode, reg)?;
                    Some(8 + ea_cycles)
                } else {
                    let ea_cycles = self.long_ea_calculation_cycles(mode, reg)?;
                    Some(6 + ea_cycles)
                }
            }
            0b100 => {
                // Destination EA must be data alterable.
                if mode == 0b001 || (mode == 0b111 && reg >= 0b010) {
                    return None;
                }
                let ea_cycles = self.word_ea_calculation_cycles(mode, reg)?;
                let src = self.d_regs[dst] as u8;
                let dst_val = if mode == 0b000 {
                    self.d_regs[reg] as u8
                } else {
                    let addr = self.resolve_data_alterable_address(mode, reg, 1, memory)?;
                    let dst_val = memory.read_u8(addr);
                    let (result, carry, overflow) = match op {
                        ArithOp::Add => {
                            let (result, carry) = dst_val.overflowing_add(src);
                            let overflow = ((!(dst_val ^ src)) & (dst_val ^ result) & 0x80) != 0;
                            (result, carry, overflow)
                        }
                        ArithOp::Sub => {
                            let result = dst_val.wrapping_sub(src);
                            let carry = src > dst_val;
                            let overflow = ((dst_val ^ src) & (dst_val ^ result) & 0x80) != 0;
                            (result, carry, overflow)
                        }
                    };
                    memory.write_u8(addr, result);
                    match op {
                        ArithOp::Add => {
                            self.update_add_flags_byte_with_extend(result, carry, overflow)
                        }
                        ArithOp::Sub => {
                            self.update_sub_flags_byte_with_extend(dst_val, src, result)
                        }
                    }
                    return Some(8 + ea_cycles); // ADD/SUB Dn,<ea> byte mem: 8+ea
                };

                let (result, carry, overflow) = match op {
                    ArithOp::Add => {
                        let (result, carry) = dst_val.overflowing_add(src);
                        let overflow = ((!(dst_val ^ src)) & (dst_val ^ result) & 0x80) != 0;
                        (result, carry, overflow)
                    }
                    ArithOp::Sub => {
                        let result = dst_val.wrapping_sub(src);
                        let carry = src > dst_val;
                        let overflow = ((dst_val ^ src) & (dst_val ^ result) & 0x80) != 0;
                        (result, carry, overflow)
                    }
                };
                self.d_regs[reg] = (self.d_regs[reg] & 0xFFFF_FF00) | result as u32;
                match op {
                    ArithOp::Add => self.update_add_flags_byte_with_extend(result, carry, overflow),
                    ArithOp::Sub => self.update_sub_flags_byte_with_extend(dst_val, src, result),
                }
                Some(4) // ADD/SUB Dn,Dn byte: 4
            }
            0b101 => {
                // Destination EA must be data alterable.
                if mode == 0b001 || (mode == 0b111 && reg >= 0b010) {
                    return None;
                }
                let ea_cycles = self.word_ea_calculation_cycles(mode, reg)?;
                let src = self.d_regs[dst] as u16;
                let dst_val = if mode == 0b000 {
                    self.d_regs[reg] as u16
                } else {
                    let addr = self.resolve_data_alterable_address(mode, reg, 2, memory)?;
                    let dst_val = memory.read_u16(addr);
                    let (result, carry, overflow) = match op {
                        ArithOp::Add => {
                            let (result, carry) = dst_val.overflowing_add(src);
                            let overflow = ((!(dst_val ^ src)) & (dst_val ^ result) & 0x8000) != 0;
                            (result, carry, overflow)
                        }
                        ArithOp::Sub => {
                            let result = dst_val.wrapping_sub(src);
                            let carry = src > dst_val;
                            let overflow = ((dst_val ^ src) & (dst_val ^ result) & 0x8000) != 0;
                            (result, carry, overflow)
                        }
                    };
                    memory.write_u16(addr, result);
                    match op {
                        ArithOp::Add => {
                            self.update_add_flags_word_with_extend(result, carry, overflow)
                        }
                        ArithOp::Sub => {
                            self.update_sub_flags_word_with_extend(dst_val, src, result)
                        }
                    }
                    return Some(8 + ea_cycles); // ADD/SUB Dn,<ea> word mem: 8+ea
                };

                let (result, carry, overflow) = match op {
                    ArithOp::Add => {
                        let (result, carry) = dst_val.overflowing_add(src);
                        let overflow = ((!(dst_val ^ src)) & (dst_val ^ result) & 0x8000) != 0;
                        (result, carry, overflow)
                    }
                    ArithOp::Sub => {
                        let result = dst_val.wrapping_sub(src);
                        let carry = src > dst_val;
                        let overflow = ((dst_val ^ src) & (dst_val ^ result) & 0x8000) != 0;
                        (result, carry, overflow)
                    }
                };
                self.d_regs[reg] = (self.d_regs[reg] & 0xFFFF_0000) | result as u32;
                match op {
                    ArithOp::Add => self.update_add_flags_word_with_extend(result, carry, overflow),
                    ArithOp::Sub => self.update_sub_flags_word_with_extend(dst_val, src, result),
                }
                Some(4) // ADD/SUB Dn,Dn word: 4
            }
            0b110 => {
                // Destination EA must be data alterable.
                if mode == 0b001 || (mode == 0b111 && reg >= 0b010) {
                    return None;
                }
                let ea_cycles = self.word_ea_calculation_cycles(mode, reg)?;
                let src = self.d_regs[dst];
                let dst_val = if mode == 0b000 {
                    self.d_regs[reg]
                } else {
                    let addr = self.resolve_data_alterable_address(mode, reg, 4, memory)?;
                    let dst_val = memory.read_u32(addr);
                    let (result, carry, overflow) = match op {
                        ArithOp::Add => {
                            let (result, carry) = dst_val.overflowing_add(src);
                            let overflow =
                                ((!(dst_val ^ src)) & (dst_val ^ result) & 0x8000_0000) != 0;
                            (result, carry, overflow)
                        }
                        ArithOp::Sub => {
                            let result = dst_val.wrapping_sub(src);
                            let carry = src > dst_val;
                            let overflow =
                                ((dst_val ^ src) & (dst_val ^ result) & 0x8000_0000) != 0;
                            (result, carry, overflow)
                        }
                    };
                    memory.write_u32(addr, result);
                    match op {
                        ArithOp::Add => {
                            self.update_add_flags_long_with_extend(result, carry, overflow)
                        }
                        ArithOp::Sub => {
                            self.update_sub_flags_long_with_extend(dst_val, src, result)
                        }
                    }
                    return Some(12 + ea_cycles); // ADD/SUB Dn,<ea> long mem: 12+ea
                };

                let (result, carry, overflow) = match op {
                    ArithOp::Add => {
                        let (result, carry) = dst_val.overflowing_add(src);
                        let overflow = ((!(dst_val ^ src)) & (dst_val ^ result) & 0x8000_0000) != 0;
                        (result, carry, overflow)
                    }
                    ArithOp::Sub => {
                        let result = dst_val.wrapping_sub(src);
                        let carry = src > dst_val;
                        let overflow = ((dst_val ^ src) & (dst_val ^ result) & 0x8000_0000) != 0;
                        (result, carry, overflow)
                    }
                };
                self.d_regs[reg] = result;
                match op {
                    ArithOp::Add => self.update_add_flags_long_with_extend(result, carry, overflow),
                    ArithOp::Sub => self.update_sub_flags_long_with_extend(dst_val, src, result),
                }
                Some(8) // ADD/SUB Dn,Dn long: 8
            }
            _ => None,
        }
    }

    fn exec_addq_subq(&mut self, opcode: u16, memory: &mut MemoryMap) -> Option<u32> {
        let cond = ((opcode >> 8) & 0xF) as u8;
        let quick_raw = ((opcode >> 9) & 0x7) as u32;
        let quick = if quick_raw == 0 { 8 } else { quick_raw };
        let is_sub = ((opcode >> 8) & 0x1) != 0;
        let size = ((opcode >> 6) & 0x3) as u8;
        let mode = ((opcode >> 3) & 0x7) as u8;
        let reg = (opcode & 0x7) as usize;

        // ADDQ/SUBQ with size=0b11 are Scc/DBcc encodings.
        if size == 0b11 {
            if mode == 0b001 {
                return self.exec_dbcc(cond, reg, memory);
            }
            return self.exec_scc(cond, mode, reg, memory);
        }

        // Destination cannot be immediate or PC-relative.
        if mode == 0b111 && reg >= 0b010 {
            return None;
        }

        if mode == 0b000 {
            match size {
                0b00 => {
                    let src = quick as u8;
                    let dst = self.d_regs[reg] as u8;
                    if is_sub {
                        let result = dst.wrapping_sub(src);
                        self.d_regs[reg] = (self.d_regs[reg] & 0xFFFF_FF00) | result as u32;
                        self.update_sub_flags_byte_with_extend(dst, src, result);
                    } else {
                        let (result, carry) = dst.overflowing_add(src);
                        let overflow = ((!(dst ^ src)) & (dst ^ result) & 0x80) != 0;
                        self.d_regs[reg] = (self.d_regs[reg] & 0xFFFF_FF00) | result as u32;
                        self.update_add_flags_byte_with_extend(result, carry, overflow);
                    }
                    return Some(4);
                }
                0b01 => {
                    let src = quick as u16;
                    let dst = self.d_regs[reg] as u16;
                    if is_sub {
                        let result = dst.wrapping_sub(src);
                        self.d_regs[reg] = (self.d_regs[reg] & 0xFFFF_0000) | result as u32;
                        self.update_sub_flags_word_with_extend(dst, src, result);
                    } else {
                        let (result, carry) = dst.overflowing_add(src);
                        let overflow = ((!(dst ^ src)) & (dst ^ result) & 0x8000) != 0;
                        self.d_regs[reg] = (self.d_regs[reg] & 0xFFFF_0000) | result as u32;
                        self.update_add_flags_word_with_extend(result, carry, overflow);
                    }
                    return Some(4);
                }
                0b10 => {
                    let src = quick;
                    let dst = self.d_regs[reg];
                    if is_sub {
                        let result = dst.wrapping_sub(src);
                        self.d_regs[reg] = result;
                        self.update_sub_flags_long_with_extend(dst, src, result);
                    } else {
                        let (result, carry) = dst.overflowing_add(src);
                        let overflow = ((!(dst ^ src)) & (dst ^ result) & 0x8000_0000) != 0;
                        self.d_regs[reg] = result;
                        self.update_add_flags_long_with_extend(result, carry, overflow);
                    }
                    return Some(8);
                }
                _ => return None,
            }
        }

        // Address register direct is valid for word/long only and does not affect CCR.
        if mode == 0b001 {
            if size == 0b00 {
                return None;
            }
            if is_sub {
                self.a_regs[reg] = self.a_regs[reg].wrapping_sub(quick);
            } else {
                self.a_regs[reg] = self.a_regs[reg].wrapping_add(quick);
            }
            return Some(8);
        }

        let ea_cycles = self.word_ea_calculation_cycles(mode, reg)?;
        let cycles = match size {
            0b00 => {
                let addr = self.resolve_data_alterable_address(mode, reg, 1, memory)?;
                let dst = memory.read_u8(addr);
                let src = quick as u8;
                if is_sub {
                    let result = dst.wrapping_sub(src);
                    memory.write_u8(addr, result);
                    self.update_sub_flags_byte_with_extend(dst, src, result);
                } else {
                    let (result, carry) = dst.overflowing_add(src);
                    let overflow = ((!(dst ^ src)) & (dst ^ result) & 0x80) != 0;
                    memory.write_u8(addr, result);
                    self.update_add_flags_byte_with_extend(result, carry, overflow);
                }
                8 + ea_cycles // ADDQ/SUBQ byte mem: 8+ea
            }
            0b01 => {
                let addr = self.resolve_data_alterable_address(mode, reg, 2, memory)?;
                let dst = memory.read_u16(addr);
                let src = quick as u16;
                if is_sub {
                    let result = dst.wrapping_sub(src);
                    memory.write_u16(addr, result);
                    self.update_sub_flags_word_with_extend(dst, src, result);
                } else {
                    let (result, carry) = dst.overflowing_add(src);
                    let overflow = ((!(dst ^ src)) & (dst ^ result) & 0x8000) != 0;
                    memory.write_u16(addr, result);
                    self.update_add_flags_word_with_extend(result, carry, overflow);
                }
                8 + ea_cycles // ADDQ/SUBQ word mem: 8+ea
            }
            0b10 => {
                let addr = self.resolve_data_alterable_address(mode, reg, 4, memory)?;
                let dst = memory.read_u32(addr);
                let src = quick;
                if is_sub {
                    let result = dst.wrapping_sub(src);
                    memory.write_u32(addr, result);
                    self.update_sub_flags_long_with_extend(dst, src, result);
                } else {
                    let (result, carry) = dst.overflowing_add(src);
                    let overflow = ((!(dst ^ src)) & (dst ^ result) & 0x8000_0000) != 0;
                    memory.write_u32(addr, result);
                    self.update_add_flags_long_with_extend(result, carry, overflow);
                }
                12 + ea_cycles // ADDQ/SUBQ long mem: 12+ea
            }
            _ => return None,
        };
        Some(cycles)
    }

    fn exec_scc(&mut self, cond: u8, mode: u8, reg: usize, memory: &mut MemoryMap) -> Option<u32> {
        // Scc destination is data alterable (Dn + memory), but not An direct or immediate/PC-relative.
        if mode == 0b001 || (mode == 0b111 && reg >= 0b010) {
            return None;
        }

        let condition = self.condition_true(cond);
        let value = if condition { 0xFF } else { 0x00 };
        if mode == 0b000 {
            self.d_regs[reg] = (self.d_regs[reg] & 0xFFFF_FF00) | value as u32;
            Some(if condition { 6 } else { 4 }) // Scc Dn: true=6, false=4
        } else {
            let ea_cycles = self.word_ea_calculation_cycles(mode, reg)?;
            let addr = self.resolve_data_alterable_address(mode, reg, 1, memory)?;
            memory.write_u8(addr, value);
            Some(8 + ea_cycles) // Scc memory: 8+ea
        }
    }

    fn exec_dbcc(&mut self, cond: u8, reg: usize, memory: &mut MemoryMap) -> Option<u32> {
        let base_pc = self.pc;
        let disp = self.fetch_u16(memory) as i16 as i32;
        if self.condition_true(cond) {
            return Some(12);
        }

        let counter = self.d_regs[reg] as u16;
        let next = counter.wrapping_sub(1);
        self.d_regs[reg] = (self.d_regs[reg] & 0xFFFF_0000) | next as u32;

        if next != 0xFFFF {
            self.pc = base_pc.wrapping_add_signed(disp);
            Some(10)
        } else {
            Some(14)
        }
    }

    fn exec_adda_w(&mut self, opcode: u16, memory: &mut MemoryMap) -> Option<u32> {
        let dst = ((opcode >> 9) & 0x7) as usize;
        let mode = ((opcode >> 3) & 0x7) as u8;
        let reg = (opcode & 0x7) as usize;
        let ea_cycles = self.word_ea_calculation_cycles(mode, reg)?;
        let value = self.read_ea_word(mode, reg, memory)? as i16 as i32 as u32;
        self.a_regs[dst] = self.a_regs[dst].wrapping_add(value);
        Some(8 + ea_cycles) // ADDA.W: 8+ea
    }

    fn exec_mulu_w(&mut self, opcode: u16, memory: &mut MemoryMap) -> Option<u32> {
        let dst = ((opcode >> 9) & 0x7) as usize;
        let mode = ((opcode >> 3) & 0x7) as u8;
        let reg = (opcode & 0x7) as usize;

        // MULU source is data EA; An direct is not valid.
        if mode == 0b001 {
            return None;
        }

        let ea_cycles = self.word_ea_calculation_cycles(mode, reg)?;
        let src_word = self.read_ea_word(mode, reg, memory)?;
        let src = src_word as u32;
        let dst_word = (self.d_regs[dst] & 0xFFFF) as u32;
        let result = dst_word.wrapping_mul(src);
        self.d_regs[dst] = result;
        self.update_test_flags_long(result);

        // MC68000: MULU requires 38 + 2n clocks, n = number of ones in source.
        let n = src_word.count_ones();
        Some(38 + (2 * n) + ea_cycles)
    }

    fn exec_muls_w(&mut self, opcode: u16, memory: &mut MemoryMap) -> Option<u32> {
        let dst = ((opcode >> 9) & 0x7) as usize;
        let mode = ((opcode >> 3) & 0x7) as u8;
        let reg = (opcode & 0x7) as usize;

        // MULS source is data EA; An direct is not valid.
        if mode == 0b001 {
            return None;
        }

        let ea_cycles = self.word_ea_calculation_cycles(mode, reg)?;
        let src_word = self.read_ea_word(mode, reg, memory)?;
        let src = src_word as i16 as i32;
        let dst_word = (self.d_regs[dst] as u16) as i16 as i32;
        let result = dst_word.wrapping_mul(src) as u32;
        self.d_regs[dst] = result;
        self.update_test_flags_long(result);

        // MC68000: MULS requires 38 + 2n clocks where n is the number of
        // 01/10 patterns in (<ea> concatenated with zero as LSB).
        let pattern = (src_word as u32) << 1;
        let mut n = 0u32;
        for bit in 0..16 {
            let b0 = (pattern >> bit) & 1;
            let b1 = (pattern >> (bit + 1)) & 1;
            if b0 != b1 {
                n += 1;
            }
        }
        Some(38 + (2 * n) + ea_cycles)
    }

    fn exec_divu_w(&mut self, opcode: u16, memory: &mut MemoryMap) -> Option<u32> {
        let dst = ((opcode >> 9) & 0x7) as usize;
        let mode = ((opcode >> 3) & 0x7) as u8;
        let reg = (opcode & 0x7) as usize;

        // DIVU source is data EA; An direct is not valid.
        if mode == 0b001 {
            return None;
        }

        let ea_cycles = self.word_ea_calculation_cycles(mode, reg)?;
        let divzero_cycles = 38 + ea_cycles;
        let divisor = self.read_ea_word(mode, reg, memory)? as u32;
        if divisor == 0 {
            self.raise_exception(5, memory, None);
            return Some(divzero_cycles);
        }

        let dividend = self.d_regs[dst];
        let exec_cycles = Self::divu_word_exec_cycles(dividend, divisor as u16);
        let quotient = dividend / divisor;
        let remainder = dividend % divisor;
        if quotient > 0xFFFF {
            self.set_flag(CCR_V, true);
            self.set_flag(CCR_C, false);
            return Some(exec_cycles + ea_cycles);
        }

        self.d_regs[dst] = ((remainder & 0xFFFF) << 16) | (quotient & 0xFFFF);
        let q16 = quotient as u16;
        self.set_flag(CCR_N, (q16 & 0x8000) != 0);
        self.set_flag(CCR_Z, q16 == 0);
        self.set_flag(CCR_V, false);
        self.set_flag(CCR_C, false);
        Some(exec_cycles + ea_cycles)
    }

    fn exec_divs_w(&mut self, opcode: u16, memory: &mut MemoryMap) -> Option<u32> {
        let dst = ((opcode >> 9) & 0x7) as usize;
        let mode = ((opcode >> 3) & 0x7) as u8;
        let reg = (opcode & 0x7) as usize;

        // DIVS source is data EA; An direct is not valid.
        if mode == 0b001 {
            return None;
        }

        let ea_cycles = self.word_ea_calculation_cycles(mode, reg)?;
        let divzero_cycles = 38 + ea_cycles;
        let divisor = self.read_ea_word(mode, reg, memory)? as i16 as i32;
        if divisor == 0 {
            self.raise_exception(5, memory, None);
            return Some(divzero_cycles);
        }

        let dividend = self.d_regs[dst] as i32;
        let exec_cycles = Self::divs_word_exec_cycles(dividend, divisor as i16);
        let (quotient, remainder) =
            match (dividend.checked_div(divisor), dividend.checked_rem(divisor)) {
                (Some(q), Some(r)) => (q, r),
                _ => {
                    self.set_flag(CCR_V, true);
                    self.set_flag(CCR_C, false);
                    return Some(exec_cycles + ea_cycles);
                }
            };

        if !(-0x8000..=0x7FFF).contains(&quotient) {
            self.set_flag(CCR_V, true);
            self.set_flag(CCR_C, false);
            return Some(exec_cycles + ea_cycles);
        }

        let q16 = quotient as i16 as u16 as u32;
        let r16 = remainder as i16 as u16 as u32;
        self.d_regs[dst] = (r16 << 16) | q16;
        self.set_flag(CCR_N, (q16 as u16 & 0x8000) != 0);
        self.set_flag(CCR_Z, (q16 as u16) == 0);
        self.set_flag(CCR_V, false);
        self.set_flag(CCR_C, false);
        Some(exec_cycles + ea_cycles)
    }

    fn exec_adda_l(&mut self, opcode: u16, memory: &mut MemoryMap) -> Option<u32> {
        let dst = ((opcode >> 9) & 0x7) as usize;
        let mode = ((opcode >> 3) & 0x7) as u8;
        let reg = (opcode & 0x7) as usize;
        let value = self.read_ea_long(mode, reg, memory)?;
        self.a_regs[dst] = self.a_regs[dst].wrapping_add(value);
        // ADDA.L: Dn/An/#imm → 8+word_ea; memory → 6+long_ea
        if mode == 0b000 || mode == 0b001 || (mode == 0b111 && reg == 0b100) {
            let ea_cycles = self.word_ea_calculation_cycles(mode, reg)?;
            Some(8 + ea_cycles)
        } else {
            let ea_cycles = self.long_ea_calculation_cycles(mode, reg)?;
            Some(6 + ea_cycles)
        }
    }

    fn exec_suba_w(&mut self, opcode: u16, memory: &mut MemoryMap) -> Option<u32> {
        let dst = ((opcode >> 9) & 0x7) as usize;
        let mode = ((opcode >> 3) & 0x7) as u8;
        let reg = (opcode & 0x7) as usize;
        let ea_cycles = self.word_ea_calculation_cycles(mode, reg)?;
        let value = self.read_ea_word(mode, reg, memory)? as i16 as i32 as u32;
        self.a_regs[dst] = self.a_regs[dst].wrapping_sub(value);
        Some(8 + ea_cycles) // SUBA.W: 8+ea
    }

    fn exec_suba_l(&mut self, opcode: u16, memory: &mut MemoryMap) -> Option<u32> {
        let dst = ((opcode >> 9) & 0x7) as usize;
        let mode = ((opcode >> 3) & 0x7) as u8;
        let reg = (opcode & 0x7) as usize;
        let value = self.read_ea_long(mode, reg, memory)?;
        self.a_regs[dst] = self.a_regs[dst].wrapping_sub(value);
        // SUBA.L: Dn/An/#imm → 8+word_ea; memory → 6+long_ea
        if mode == 0b000 || mode == 0b001 || (mode == 0b111 && reg == 0b100) {
            let ea_cycles = self.word_ea_calculation_cycles(mode, reg)?;
            Some(8 + ea_cycles)
        } else {
            let ea_cycles = self.long_ea_calculation_cycles(mode, reg)?;
            Some(6 + ea_cycles)
        }
    }

    fn exec_addx(&mut self, opcode: u16, memory: &mut MemoryMap) -> Option<u32> {
        self.exec_addx_subx(opcode, memory, true)
    }

    fn exec_subx(&mut self, opcode: u16, memory: &mut MemoryMap) -> Option<u32> {
        self.exec_addx_subx(opcode, memory, false)
    }

    fn exec_addx_subx(&mut self, opcode: u16, memory: &mut MemoryMap, add: bool) -> Option<u32> {
        let dst = ((opcode >> 9) & 0x7) as usize;
        let size = ((opcode >> 6) & 0x3) as u8;
        let mem_mode = (opcode & 0x0008) != 0;
        let src = (opcode & 0x7) as usize;
        let x_in = if self.flag_set(CCR_X) { 1u32 } else { 0u32 };
        let prev_z = self.flag_set(CCR_Z);

        match size {
            0b00 => {
                let (src_v, dst_v, dst_addr) = if mem_mode {
                    self.a_regs[src] = self.a_regs[src].wrapping_sub(self.byte_addr_step(src));
                    self.a_regs[dst] = self.a_regs[dst].wrapping_sub(self.byte_addr_step(dst));
                    (
                        memory.read_u8(self.a_regs[src]),
                        memory.read_u8(self.a_regs[dst]),
                        Some(self.a_regs[dst]),
                    )
                } else {
                    (self.d_regs[src] as u8, self.d_regs[dst] as u8, None)
                };

                let rhs = src_v as u16 + x_in as u16;
                let (result, carry, overflow) = if add {
                    let sum = dst_v as u16 + rhs;
                    let result = sum as u8;
                    let carry = sum > 0xFF;
                    let overflow = ((!(dst_v ^ src_v)) & (dst_v ^ result) & 0x80) != 0;
                    (result, carry, overflow)
                } else {
                    let result = (dst_v as u16).wrapping_sub(rhs) as u8;
                    let carry = rhs > dst_v as u16;
                    let overflow = ((dst_v ^ src_v) & (dst_v ^ result) & 0x80) != 0;
                    (result, carry, overflow)
                };

                if let Some(addr) = dst_addr {
                    memory.write_u8(addr, result);
                } else {
                    self.d_regs[dst] = (self.d_regs[dst] & 0xFFFF_FF00) | result as u32;
                }
                self.set_flag(CCR_N, (result & 0x80) != 0);
                self.set_flag(CCR_Z, prev_z && result == 0);
                self.set_flag(CCR_V, overflow);
                self.set_flag(CCR_C, carry);
                self.set_flag(CCR_X, carry);
                Some(if mem_mode { 18 } else { 4 })
            }
            0b01 => {
                let (src_v, dst_v, dst_addr) = if mem_mode {
                    self.a_regs[src] = self.a_regs[src].wrapping_sub(2);
                    self.a_regs[dst] = self.a_regs[dst].wrapping_sub(2);
                    (
                        memory.read_u16(self.a_regs[src]),
                        memory.read_u16(self.a_regs[dst]),
                        Some(self.a_regs[dst]),
                    )
                } else {
                    (self.d_regs[src] as u16, self.d_regs[dst] as u16, None)
                };

                let rhs = src_v as u32 + x_in;
                let (result, carry, overflow) = if add {
                    let sum = dst_v as u32 + rhs;
                    let result = sum as u16;
                    let carry = sum > 0xFFFF;
                    let overflow = ((!(dst_v ^ src_v)) & (dst_v ^ result) & 0x8000) != 0;
                    (result, carry, overflow)
                } else {
                    let result = (dst_v as u32).wrapping_sub(rhs) as u16;
                    let carry = rhs > dst_v as u32;
                    let overflow = ((dst_v ^ src_v) & (dst_v ^ result) & 0x8000) != 0;
                    (result, carry, overflow)
                };

                if let Some(addr) = dst_addr {
                    memory.write_u16(addr, result);
                } else {
                    self.d_regs[dst] = (self.d_regs[dst] & 0xFFFF_0000) | result as u32;
                }
                self.set_flag(CCR_N, (result & 0x8000) != 0);
                self.set_flag(CCR_Z, prev_z && result == 0);
                self.set_flag(CCR_V, overflow);
                self.set_flag(CCR_C, carry);
                self.set_flag(CCR_X, carry);
                Some(if mem_mode { 18 } else { 4 })
            }
            0b10 => {
                let (src_v, dst_v, dst_addr) = if mem_mode {
                    self.a_regs[src] = self.a_regs[src].wrapping_sub(4);
                    self.a_regs[dst] = self.a_regs[dst].wrapping_sub(4);
                    (
                        memory.read_u32(self.a_regs[src]),
                        memory.read_u32(self.a_regs[dst]),
                        Some(self.a_regs[dst]),
                    )
                } else {
                    (self.d_regs[src], self.d_regs[dst], None)
                };

                let rhs = src_v as u64 + x_in as u64;
                let (result, carry, overflow) = if add {
                    let sum = dst_v as u64 + rhs;
                    let result = sum as u32;
                    let carry = sum > 0xFFFF_FFFF;
                    let overflow = ((!(dst_v ^ src_v)) & (dst_v ^ result) & 0x8000_0000) != 0;
                    (result, carry, overflow)
                } else {
                    let result = (dst_v as u64).wrapping_sub(rhs) as u32;
                    let carry = rhs > dst_v as u64;
                    let overflow = ((dst_v ^ src_v) & (dst_v ^ result) & 0x8000_0000) != 0;
                    (result, carry, overflow)
                };

                if let Some(addr) = dst_addr {
                    memory.write_u32(addr, result);
                } else {
                    self.d_regs[dst] = result;
                }
                self.set_flag(CCR_N, (result & 0x8000_0000) != 0);
                self.set_flag(CCR_Z, prev_z && result == 0);
                self.set_flag(CCR_V, overflow);
                self.set_flag(CCR_C, carry);
                self.set_flag(CCR_X, carry);
                Some(if mem_mode { 30 } else { 8 })
            }
            _ => None,
        }
    }

    fn exec_lea(&mut self, opcode: u16, memory: &mut MemoryMap) -> Option<u32> {
        let dst = ((opcode >> 9) & 0x7) as usize;
        let mode = ((opcode >> 3) & 0x7) as u8;
        let reg = (opcode & 0x7) as usize;
        let addr = self.resolve_control_address(mode, reg, memory)?;
        self.a_regs[dst] = addr;
        // LEA timing per addressing mode
        let cycles = match mode {
            0b010 => 4,  // (An)
            0b101 => 8,  // d(An)
            0b110 => 12, // d(An,Xn)
            0b111 => match reg {
                0b000 => 8,  // xxx.W
                0b001 => 12, // xxx.L
                0b010 => 8,  // d(PC)
                0b011 => 12, // d(PC,Xn)
                _ => 8,
            },
            _ => 4,
        };
        Some(cycles)
    }

    fn exec_cmpi(&mut self, opcode: u16, memory: &mut MemoryMap) -> Option<u32> {
        let size = ((opcode >> 6) & 0x3) as u8;
        let mode = ((opcode >> 3) & 0x7) as u8;
        let reg = (opcode & 0x7) as usize;

        // Destination EA for CMPI is data alterable (Dn + alterable memory only).
        if mode == 0b001 || (mode == 0b111 && reg >= 0b010) {
            return None;
        }

        match size {
            0b00 => {
                let ea_cycles = self.word_ea_calculation_cycles(mode, reg)?;
                let imm = self.fetch_u16(memory) as u8;
                let value = self.read_ea_byte(mode, reg, memory)?;
                let result = value.wrapping_sub(imm);
                self.update_sub_flags_byte(value, imm, result);
                // CMPI byte: Dn=8, memory=8+ea
                Some(if mode == 0b000 { 8 } else { 8 + ea_cycles })
            }
            0b01 => {
                let ea_cycles = self.word_ea_calculation_cycles(mode, reg)?;
                let imm = self.fetch_u16(memory);
                let value = self.read_ea_word(mode, reg, memory)?;
                let result = value.wrapping_sub(imm);
                self.update_sub_flags_word(value, imm, result);
                // CMPI word: Dn=8, memory=8+ea
                Some(if mode == 0b000 { 8 } else { 8 + ea_cycles })
            }
            0b10 => {
                let ea_cycles = self.word_ea_calculation_cycles(mode, reg)?;
                let imm = self.fetch_u32(memory);
                let value = self.read_ea_long(mode, reg, memory)?;
                let result = value.wrapping_sub(imm);
                self.update_sub_flags_long(value, imm, result);
                // CMPI long: Dn=14, memory=12+ea
                Some(if mode == 0b000 { 14 } else { 12 + ea_cycles })
            }
            _ => None,
        }
    }

    fn exec_cmp_ea_to_dn(&mut self, opcode: u16, memory: &mut MemoryMap) -> Option<u32> {
        let reg_x = ((opcode >> 9) & 0x7) as usize;
        let opmode = ((opcode >> 6) & 0x7) as u8;
        let mode = ((opcode >> 3) & 0x7) as u8;
        let reg = (opcode & 0x7) as usize;

        if (0b100..=0b110).contains(&opmode) {
            return self.exec_eor_dn_to_ea(reg_x, opmode, mode, reg, memory);
        }

        match opmode {
            0b000 => {
                // Source for CMP.B <ea>,Dn cannot be An direct.
                if mode == 0b001 {
                    return None;
                }
                let ea_cycles = self.word_ea_calculation_cycles(mode, reg)?;
                let src = self.read_ea_byte(mode, reg, memory)?;
                let dst_val = self.d_regs[reg_x] as u8;
                let result = dst_val.wrapping_sub(src);
                self.update_sub_flags_byte(dst_val, src, result);
                Some(4 + ea_cycles) // CMP byte: 4+ea
            }
            0b001 => {
                let ea_cycles = self.word_ea_calculation_cycles(mode, reg)?;
                let src = self.read_ea_word(mode, reg, memory)?;
                let dst_val = self.d_regs[reg_x] as u16;
                let result = dst_val.wrapping_sub(src);
                self.update_sub_flags_word(dst_val, src, result);
                Some(4 + ea_cycles) // CMP word: 4+ea
            }
            0b010 => {
                let ea_cycles = self.word_ea_calculation_cycles(mode, reg)?;
                let src = self.read_ea_long(mode, reg, memory)?;
                let dst_val = self.d_regs[reg_x];
                let result = dst_val.wrapping_sub(src);
                self.update_sub_flags_long(dst_val, src, result);
                Some(6 + ea_cycles) // CMP long: 6+ea
            }
            _ => None,
        }
    }

    fn exec_cmpa_w(&mut self, opcode: u16, memory: &mut MemoryMap) -> Option<u32> {
        let dst = ((opcode >> 9) & 0x7) as usize;
        let mode = ((opcode >> 3) & 0x7) as u8;
        let reg = (opcode & 0x7) as usize;
        let ea_cycles = self.word_ea_calculation_cycles(mode, reg)?;
        let src = self.read_ea_word(mode, reg, memory)? as i16 as i32 as u32;
        let dst_val = self.a_regs[dst];
        let result = dst_val.wrapping_sub(src);
        self.update_sub_flags_long(dst_val, src, result);
        Some(6 + ea_cycles) // CMPA.W: 6+ea
    }

    fn exec_cmpa_l(&mut self, opcode: u16, memory: &mut MemoryMap) -> Option<u32> {
        let dst = ((opcode >> 9) & 0x7) as usize;
        let mode = ((opcode >> 3) & 0x7) as u8;
        let reg = (opcode & 0x7) as usize;
        let ea_cycles = self.word_ea_calculation_cycles(mode, reg)?;
        let src = self.read_ea_long(mode, reg, memory)?;
        let dst_val = self.a_regs[dst];
        let result = dst_val.wrapping_sub(src);
        self.update_sub_flags_long(dst_val, src, result);
        Some(6 + ea_cycles) // CMPA.L: 6+ea
    }

    fn exec_cmpm(&mut self, opcode: u16, memory: &mut MemoryMap) -> Option<u32> {
        let ax = ((opcode >> 9) & 0x7) as usize;
        let size = ((opcode >> 6) & 0x3) as u8;
        let ay = (opcode & 0x7) as usize;

        match size {
            0b00 => {
                let src_addr = self.a_regs[ay];
                let dst_addr = self.a_regs[ax];
                let src = memory.read_u8(src_addr);
                let dst = memory.read_u8(dst_addr);
                self.a_regs[ay] = self.a_regs[ay].wrapping_add(self.byte_addr_step(ay));
                self.a_regs[ax] = self.a_regs[ax].wrapping_add(self.byte_addr_step(ax));
                let result = dst.wrapping_sub(src);
                self.update_sub_flags_byte(dst, src, result);
                Some(12)
            }
            0b01 => {
                let src_addr = self.a_regs[ay];
                let dst_addr = self.a_regs[ax];
                let src = memory.read_u16(src_addr);
                let dst = memory.read_u16(dst_addr);
                self.a_regs[ay] = self.a_regs[ay].wrapping_add(2);
                self.a_regs[ax] = self.a_regs[ax].wrapping_add(2);
                let result = dst.wrapping_sub(src);
                self.update_sub_flags_word(dst, src, result);
                Some(12)
            }
            0b10 => {
                let src_addr = self.a_regs[ay];
                let dst_addr = self.a_regs[ax];
                let src = memory.read_u32(src_addr);
                let dst = memory.read_u32(dst_addr);
                self.a_regs[ay] = self.a_regs[ay].wrapping_add(4);
                self.a_regs[ax] = self.a_regs[ax].wrapping_add(4);
                let result = dst.wrapping_sub(src);
                self.update_sub_flags_long(dst, src, result);
                Some(20)
            }
            _ => None,
        }
    }

    fn exec_chk_w(&mut self, opcode: u16, memory: &mut MemoryMap) -> Option<u32> {
        let dn = ((opcode >> 9) & 0x7) as usize;
        let mode = ((opcode >> 3) & 0x7) as u8;
        let reg = (opcode & 0x7) as usize;
        if mode == 0b001 {
            return None;
        }
        let ea_cycles = self.word_ea_calculation_cycles(mode, reg)?;
        let upper = self.read_ea_word(mode, reg, memory)? as i16 as i32;
        let value = self.d_regs[dn] as i16 as i32;
        if value < 0 {
            self.set_flag(CCR_N, true);
            self.raise_exception(6, memory, None);
            return Some(40 + ea_cycles);
        }
        if value > upper {
            self.set_flag(CCR_N, false);
            self.raise_exception(6, memory, None);
            return Some(40 + ea_cycles);
        }
        Some(10 + ea_cycles) // CHK no trap: 10+ea
    }

    fn exec_eor_dn_to_ea(
        &mut self,
        src_dn: usize,
        opmode: u8,
        mode: u8,
        reg: usize,
        memory: &mut MemoryMap,
    ) -> Option<u32> {
        // Destination EA for EOR Dn,<ea> must be data alterable.
        if mode == 0b001 || (mode == 0b111 && reg >= 0b010) {
            return None;
        }

        let ea_cycles = self.word_ea_calculation_cycles(mode, reg)?;
        match opmode {
            0b100 => {
                let src = self.d_regs[src_dn] as u8;
                let result = if mode == 0b000 {
                    let dst = self.d_regs[reg] as u8;
                    let result = dst ^ src;
                    self.d_regs[reg] = (self.d_regs[reg] & 0xFFFF_FF00) | result as u32;
                    result
                } else {
                    let addr = self.resolve_data_alterable_address(mode, reg, 1, memory)?;
                    let dst = memory.read_u8(addr);
                    let result = dst ^ src;
                    memory.write_u8(addr, result);
                    result
                };
                self.update_test_flags_byte(result);
                // EOR Dn,<ea> byte: Dn=4, memory=8+ea
                Some(if mode == 0b000 { 4 } else { 8 + ea_cycles })
            }
            0b101 => {
                let src = self.d_regs[src_dn] as u16;
                let result = if mode == 0b000 {
                    let dst = self.d_regs[reg] as u16;
                    let result = dst ^ src;
                    self.d_regs[reg] = (self.d_regs[reg] & 0xFFFF_0000) | result as u32;
                    result
                } else {
                    let addr = self.resolve_data_alterable_address(mode, reg, 2, memory)?;
                    let dst = memory.read_u16(addr);
                    let result = dst ^ src;
                    memory.write_u16(addr, result);
                    result
                };
                self.update_test_flags_word(result);
                // EOR Dn,<ea> word: Dn=4, memory=8+ea
                Some(if mode == 0b000 { 4 } else { 8 + ea_cycles })
            }
            0b110 => {
                let src = self.d_regs[src_dn];
                let result = if mode == 0b000 {
                    let dst = self.d_regs[reg];
                    let result = dst ^ src;
                    self.d_regs[reg] = result;
                    result
                } else {
                    let addr = self.resolve_data_alterable_address(mode, reg, 4, memory)?;
                    let dst = memory.read_u32(addr);
                    let result = dst ^ src;
                    memory.write_u32(addr, result);
                    result
                };
                self.update_test_flags_long(result);
                // EOR Dn,<ea> long: Dn=8, memory=12+ea
                Some(if mode == 0b000 { 8 } else { 12 + ea_cycles })
            }
            _ => None,
        }
    }

    fn exec_tst(&mut self, opcode: u16, memory: &mut MemoryMap) -> Option<u32> {
        let size = ((opcode >> 6) & 0x3) as u8;
        let mode = ((opcode >> 3) & 0x7) as u8;
        let reg = (opcode & 0x7) as usize;

        // On 68000, TST supports Dn and memory data-alterable modes only.
        // PC-relative and immediate forms are not available.
        if mode == 0b001 || (mode == 0b111 && reg >= 0b010) {
            return None;
        }

        let ea_cycles = self.word_ea_calculation_cycles(mode, reg)?;
        match size {
            0b00 => {
                let value = self.read_ea_byte(mode, reg, memory)?;
                self.update_test_flags_byte(value);
                Some(4 + ea_cycles) // TST byte: 4+ea
            }
            0b01 => {
                let value = self.read_ea_word(mode, reg, memory)?;
                self.update_test_flags_word(value);
                Some(4 + ea_cycles) // TST word: 4+ea
            }
            0b10 => {
                let value = self.read_ea_long(mode, reg, memory)?;
                self.update_test_flags_long(value);
                Some(4 + ea_cycles) // TST long: 4+ea
            }
            _ => None,
        }
    }

    fn exec_ori(&mut self, opcode: u16, memory: &mut MemoryMap) -> Option<u32> {
        self.exec_imm_logical(opcode, memory, |dst, imm| dst | imm)
    }

    fn exec_andi(&mut self, opcode: u16, memory: &mut MemoryMap) -> Option<u32> {
        self.exec_imm_logical(opcode, memory, |dst, imm| dst & imm)
    }

    fn exec_eori(&mut self, opcode: u16, memory: &mut MemoryMap) -> Option<u32> {
        self.exec_imm_logical(opcode, memory, |dst, imm| dst ^ imm)
    }

    fn exec_addi(&mut self, opcode: u16, memory: &mut MemoryMap) -> Option<u32> {
        self.exec_imm_arith(opcode, memory, ArithOp::Add)
    }

    fn exec_subi(&mut self, opcode: u16, memory: &mut MemoryMap) -> Option<u32> {
        self.exec_imm_arith(opcode, memory, ArithOp::Sub)
    }

    fn exec_imm_logical<F>(&mut self, opcode: u16, memory: &mut MemoryMap, op: F) -> Option<u32>
    where
        F: Fn(u32, u32) -> u32,
    {
        let size = ((opcode >> 6) & 0x3) as u8;
        let mode = ((opcode >> 3) & 0x7) as u8;
        let reg = (opcode & 0x7) as usize;

        // Logical immediate destination is data alterable only.
        if mode == 0b001 || (mode == 0b111 && reg == 0b100) {
            return None;
        }

        let ea_cycles = self.word_ea_calculation_cycles(mode, reg)?;
        match size {
            0b00 => {
                let imm = self.fetch_u16(memory) as u8;
                let result = if mode == 0b000 {
                    let dst = self.d_regs[reg] as u8;
                    let result = op(dst as u32, imm as u32) as u8;
                    self.d_regs[reg] = (self.d_regs[reg] & 0xFFFF_FF00) | result as u32;
                    result
                } else {
                    let addr = self.resolve_data_alterable_address(mode, reg, 1, memory)?;
                    let dst = memory.read_u8(addr);
                    let result = op(dst as u32, imm as u32) as u8;
                    memory.write_u8(addr, result);
                    result
                };
                self.update_test_flags_byte(result);
                // ORI/ANDI/EORI byte: Dn=8, memory=12+ea
                Some(if mode == 0b000 { 8 } else { 12 + ea_cycles })
            }
            0b01 => {
                let imm = self.fetch_u16(memory);
                let result = if mode == 0b000 {
                    let dst = self.d_regs[reg] as u16;
                    let result = op(dst as u32, imm as u32) as u16;
                    self.d_regs[reg] = (self.d_regs[reg] & 0xFFFF_0000) | result as u32;
                    result
                } else {
                    let addr = self.resolve_data_alterable_address(mode, reg, 2, memory)?;
                    let dst = memory.read_u16(addr);
                    let result = op(dst as u32, imm as u32) as u16;
                    memory.write_u16(addr, result);
                    result
                };
                self.update_test_flags_word(result);
                // ORI/ANDI/EORI word: Dn=8, memory=12+ea
                Some(if mode == 0b000 { 8 } else { 12 + ea_cycles })
            }
            0b10 => {
                let imm = self.fetch_u32(memory);
                let result = if mode == 0b000 {
                    let dst = self.d_regs[reg];
                    let result = op(dst, imm);
                    self.d_regs[reg] = result;
                    result
                } else {
                    let addr = self.resolve_data_alterable_address(mode, reg, 4, memory)?;
                    let dst = memory.read_u32(addr);
                    let result = op(dst, imm);
                    memory.write_u32(addr, result);
                    result
                };
                self.update_test_flags_long(result);
                // ORI/ANDI/EORI long: Dn=16, memory=20+ea
                Some(if mode == 0b000 { 16 } else { 20 + ea_cycles })
            }
            _ => None,
        }
    }

    fn exec_imm_arith(&mut self, opcode: u16, memory: &mut MemoryMap, op: ArithOp) -> Option<u32> {
        let size = ((opcode >> 6) & 0x3) as u8;
        let mode = ((opcode >> 3) & 0x7) as u8;
        let reg = (opcode & 0x7) as usize;

        if mode == 0b001 || (mode == 0b111 && reg == 0b100) {
            return None;
        }

        let ea_cycles = self.word_ea_calculation_cycles(mode, reg)?;
        match size {
            0b00 => {
                let imm = self.fetch_u16(memory) as u8;
                let (dst, store) = if mode == 0b000 {
                    let dst = self.d_regs[reg] as u8;
                    (dst, ImmStore::DnByte(reg))
                } else {
                    let addr = self.resolve_data_alterable_address(mode, reg, 1, memory)?;
                    (memory.read_u8(addr), ImmStore::MemByte(addr))
                };
                let (result, carry, overflow) = match op {
                    ArithOp::Add => {
                        let (result, carry) = dst.overflowing_add(imm);
                        let overflow = ((!(dst ^ imm)) & (dst ^ result) & 0x80) != 0;
                        (result, carry, overflow)
                    }
                    ArithOp::Sub => {
                        let (result, borrow) = dst.overflowing_sub(imm);
                        let overflow = ((dst ^ imm) & (dst ^ result) & 0x80) != 0;
                        (result, borrow, overflow)
                    }
                };
                match store {
                    ImmStore::DnByte(r) => {
                        self.d_regs[r] = (self.d_regs[r] & 0xFFFF_FF00) | result as u32;
                    }
                    ImmStore::MemByte(addr) => memory.write_u8(addr, result),
                    _ => unreachable!(),
                }
                match op {
                    ArithOp::Add => self.update_add_flags_byte_with_extend(result, carry, overflow),
                    ArithOp::Sub => self.update_sub_flags_byte_with_extend(dst, imm, result),
                }
                // ADDI/SUBI byte: Dn=8, memory=12+ea
                Some(if mode == 0b000 { 8 } else { 12 + ea_cycles })
            }
            0b01 => {
                let imm = self.fetch_u16(memory);
                let (dst, store) = if mode == 0b000 {
                    let dst = self.d_regs[reg] as u16;
                    (dst, ImmStore::DnWord(reg))
                } else {
                    let addr = self.resolve_data_alterable_address(mode, reg, 2, memory)?;
                    (memory.read_u16(addr), ImmStore::MemWord(addr))
                };
                let (result, carry, overflow) = match op {
                    ArithOp::Add => {
                        let (result, carry) = dst.overflowing_add(imm);
                        let overflow = ((!(dst ^ imm)) & (dst ^ result) & 0x8000) != 0;
                        (result, carry, overflow)
                    }
                    ArithOp::Sub => {
                        let result = dst.wrapping_sub(imm);
                        let carry = imm > dst;
                        let overflow = ((dst ^ imm) & (dst ^ result) & 0x8000) != 0;
                        (result, carry, overflow)
                    }
                };
                match store {
                    ImmStore::DnWord(r) => {
                        self.d_regs[r] = (self.d_regs[r] & 0xFFFF_0000) | result as u32;
                    }
                    ImmStore::MemWord(addr) => memory.write_u16(addr, result),
                    _ => unreachable!(),
                }
                match op {
                    ArithOp::Add => self.update_add_flags_word_with_extend(result, carry, overflow),
                    ArithOp::Sub => self.update_sub_flags_word_with_extend(dst, imm, result),
                }
                // ADDI/SUBI word: Dn=8, memory=12+ea
                Some(if mode == 0b000 { 8 } else { 12 + ea_cycles })
            }
            0b10 => {
                let imm = self.fetch_u32(memory);
                let (dst, store) = if mode == 0b000 {
                    let dst = self.d_regs[reg];
                    (dst, ImmStore::DnLong(reg))
                } else {
                    let addr = self.resolve_data_alterable_address(mode, reg, 4, memory)?;
                    (memory.read_u32(addr), ImmStore::MemLong(addr))
                };
                let (result, carry, overflow) = match op {
                    ArithOp::Add => {
                        let (result, carry) = dst.overflowing_add(imm);
                        let overflow = ((!(dst ^ imm)) & (dst ^ result) & 0x8000_0000) != 0;
                        (result, carry, overflow)
                    }
                    ArithOp::Sub => {
                        let result = dst.wrapping_sub(imm);
                        let carry = imm > dst;
                        let overflow = ((dst ^ imm) & (dst ^ result) & 0x8000_0000) != 0;
                        (result, carry, overflow)
                    }
                };
                match store {
                    ImmStore::DnLong(r) => self.d_regs[r] = result,
                    ImmStore::MemLong(addr) => memory.write_u32(addr, result),
                    _ => unreachable!(),
                }
                match op {
                    ArithOp::Add => self.update_add_flags_long_with_extend(result, carry, overflow),
                    ArithOp::Sub => self.update_sub_flags_long_with_extend(dst, imm, result),
                }
                // ADDI/SUBI long: Dn=16, memory=20+ea
                Some(if mode == 0b000 { 16 } else { 20 + ea_cycles })
            }
            _ => None,
        }
    }

    fn exec_clr(&mut self, opcode: u16, memory: &mut MemoryMap) -> Option<u32> {
        let size = ((opcode >> 6) & 0x3) as u8;
        let mode = ((opcode >> 3) & 0x7) as u8;
        let reg = (opcode & 0x7) as usize;

        // CLR supports data alterable destinations (Dn + memory), but not An direct or immediate.
        if mode == 0b001 || (mode == 0b111 && reg == 0b100) {
            return None;
        }

        let ea_cycles = self.word_ea_calculation_cycles(mode, reg)?;
        match size {
            0b00 => {
                self.write_ea_byte(mode, reg, 0, memory)?;
                self.update_test_flags_byte(0);
                Some(if mode == 0b000 { 4 } else { 8 + ea_cycles })
            }
            0b01 => {
                self.write_ea_word(mode, reg, 0, memory)?;
                self.update_test_flags_word(0);
                Some(if mode == 0b000 { 4 } else { 8 + ea_cycles })
            }
            0b10 => {
                self.write_ea_long(mode, reg, 0, memory)?;
                self.update_test_flags_long(0);
                Some(if mode == 0b000 { 6 } else { 12 + ea_cycles })
            }
            _ => None,
        }
    }

    fn exec_jsr(&mut self, opcode: u16, memory: &mut MemoryMap) -> Option<u32> {
        let mode = ((opcode >> 3) & 0x7) as u8;
        let reg = (opcode & 0x7) as usize;
        let target = self.resolve_control_address(mode, reg, memory)?;
        let return_addr = self.pc;
        self.push_u32(memory, return_addr);
        self.pc = target;
        // JSR timing per addressing mode
        let cycles = match mode {
            0b010 => 16,  // (An)
            0b101 => 18,  // d(An)
            0b110 => 22,  // d(An,Xn)
            0b111 => match reg {
                0b000 => 18,  // xxx.W
                0b001 => 20,  // xxx.L
                0b010 => 18,  // d(PC)
                0b011 => 22,  // d(PC,Xn)
                _ => 16,
            },
            _ => 16,
        };
        Some(cycles)
    }

    fn exec_jmp(&mut self, opcode: u16, memory: &mut MemoryMap) -> Option<u32> {
        let mode = ((opcode >> 3) & 0x7) as u8;
        let reg = (opcode & 0x7) as usize;
        let target = self.resolve_control_address(mode, reg, memory)?;
        self.pc = target;
        // JMP timing per addressing mode
        let cycles = match mode {
            0b010 => 8,   // (An)
            0b101 => 10,  // d(An)
            0b110 => 14,  // d(An,Xn)
            0b111 => match reg {
                0b000 => 10,  // xxx.W
                0b001 => 12,  // xxx.L
                0b010 => 10,  // d(PC)
                0b011 => 14,  // d(PC,Xn)
                _ => 10,
            },
            _ => 8,
        };
        Some(cycles)
    }

    fn exec_link(&mut self, opcode: u16, memory: &mut MemoryMap) -> u32 {
        let reg = (opcode & 0x7) as usize;
        let displacement = self.fetch_u16(memory) as i16 as i32;
        self.push_u32(memory, self.a_regs[reg]);
        self.a_regs[reg] = self.a_regs[7];
        self.a_regs[7] = self.a_regs[7].wrapping_add_signed(displacement);
        16
    }

    fn exec_unlk(&mut self, opcode: u16, memory: &mut MemoryMap) -> u32 {
        let reg = (opcode & 0x7) as usize;
        self.a_regs[7] = self.a_regs[reg];
        self.a_regs[reg] = self.pop_u32(memory);
        12
    }

    fn exec_move_usp(&mut self, opcode: u16, memory: &mut MemoryMap) -> u32 {
        if (self.sr & SR_SUPERVISOR) == 0 {
            self.raise_exception(8, memory, None);
            return 34;
        }

        let reg = (opcode & 0x7) as usize;
        if (opcode & 0x0008) == 0 {
            self.usp = self.a_regs[reg];
        } else {
            self.a_regs[reg] = self.usp;
        }
        4
    }

    fn exec_move_from_sr(&mut self, opcode: u16, memory: &mut MemoryMap) -> Option<u32> {
        let mode = ((opcode >> 3) & 0x7) as u8;
        let reg = (opcode & 0x7) as usize;
        // Destination must be data alterable; An direct and immediate are invalid.
        if mode == 0b001 || (mode == 0b111 && reg == 0b100) {
            return None;
        }
        self.write_ea_word(mode, reg, self.sr, memory)?;
        // MC68000: MOVE from SR — Dn=6, mem=8+ea
        if mode == 0b000 {
            Some(6)
        } else {
            let ea_cycles = self.word_ea_calculation_cycles(mode, reg).unwrap_or(0);
            Some(8 + ea_cycles)
        }
    }

    fn exec_move_to_sr(&mut self, opcode: u16, memory: &mut MemoryMap) -> Option<u32> {
        if (self.sr & SR_SUPERVISOR) == 0 {
            self.raise_exception(8, memory, None);
            return Some(34);
        }

        let mode = ((opcode >> 3) & 0x7) as u8;
        let reg = (opcode & 0x7) as usize;
        // Source must be data addressing mode; An direct is invalid.
        if mode == 0b001 {
            return None;
        }
        let ea_cycles = self.word_ea_calculation_cycles(mode, reg)?;
        let value = self.read_ea_word(mode, reg, memory)?;
        self.write_sr(value);
        // MC68000: MOVE to SR = 12 + ea
        Some(12 + ea_cycles)
    }

    fn exec_ori_to_ccr(&mut self, memory: &mut MemoryMap) -> u32 {
        let imm = self.fetch_u16(memory);
        self.sr = (self.sr & !0x001F) | ((self.sr | imm) & 0x001F);
        20
    }

    fn exec_ori_to_sr(&mut self, memory: &mut MemoryMap) -> u32 {
        if (self.sr & SR_SUPERVISOR) == 0 {
            self.raise_exception(8, memory, None);
            return 34;
        }
        let imm = self.fetch_u16(memory);
        self.write_sr(self.sr | imm);
        20
    }

    fn exec_andi_to_ccr(&mut self, memory: &mut MemoryMap) -> u32 {
        let imm = self.fetch_u16(memory) & 0x001F;
        self.sr = (self.sr & !0x001F) | ((self.sr & imm) & 0x001F);
        20
    }

    fn exec_andi_to_sr(&mut self, memory: &mut MemoryMap) -> u32 {
        if (self.sr & SR_SUPERVISOR) == 0 {
            self.raise_exception(8, memory, None);
            return 34;
        }
        let imm = self.fetch_u16(memory);
        self.write_sr(self.sr & imm);
        20
    }

    fn exec_eori_to_ccr(&mut self, memory: &mut MemoryMap) -> u32 {
        let imm = self.fetch_u16(memory) & 0x001F;
        self.sr = (self.sr & !0x001F) | ((self.sr ^ imm) & 0x001F);
        20
    }

    fn exec_eori_to_sr(&mut self, memory: &mut MemoryMap) -> u32 {
        if (self.sr & SR_SUPERVISOR) == 0 {
            self.raise_exception(8, memory, None);
            return 34;
        }
        let imm = self.fetch_u16(memory);
        self.write_sr(self.sr ^ imm);
        20
    }

    fn exec_move_to_ccr(&mut self, opcode: u16, memory: &mut MemoryMap) -> Option<u32> {
        let mode = ((opcode >> 3) & 0x7) as u8;
        let reg = (opcode & 0x7) as usize;
        // Source must be data addressing mode; An direct is invalid.
        if mode == 0b001 {
            return None;
        }
        let ea_cycles = self.word_ea_calculation_cycles(mode, reg)?;
        let value = self.read_ea_word(mode, reg, memory)?;
        self.sr = (self.sr & !0x001F) | (value & 0x001F);
        // MC68000: MOVE to CCR = 12 + ea
        Some(12 + ea_cycles)
    }

    fn exec_neg(&mut self, opcode: u16, memory: &mut MemoryMap) -> Option<u32> {
        let size = ((opcode >> 6) & 0x3) as u8;
        let mode = ((opcode >> 3) & 0x7) as u8;
        let reg = (opcode & 0x7) as usize;

        // NEG supports data alterable destinations (Dn + memory), but not An direct or immediate.
        if mode == 0b001 || (mode == 0b111 && reg == 0b100) {
            return None;
        }

        let ea_cycles = self.word_ea_calculation_cycles(mode, reg)?;
        match size {
            0b00 => {
                let (dst, addr) = if mode == 0b000 {
                    (self.d_regs[reg] as u8, None)
                } else {
                    let addr = self.resolve_data_alterable_address(mode, reg, 1, memory)?;
                    (memory.read_u8(addr), Some(addr))
                };
                let result = (0u8).wrapping_sub(dst);
                if mode == 0b000 {
                    self.d_regs[reg] = (self.d_regs[reg] & 0xFFFF_FF00) | result as u32;
                } else {
                    memory.write_u8(addr.expect("memory mode must resolve address"), result);
                }
                self.update_sub_flags_byte_with_extend(0, dst, result);
                self.set_flag(CCR_X, dst != 0);
                Some(if mode == 0b000 { 4 } else { 8 + ea_cycles })
            }
            0b01 => {
                let (dst, addr) = if mode == 0b000 {
                    (self.d_regs[reg] as u16, None)
                } else {
                    let addr = self.resolve_data_alterable_address(mode, reg, 2, memory)?;
                    (memory.read_u16(addr), Some(addr))
                };
                let result = (0u16).wrapping_sub(dst);
                if mode == 0b000 {
                    self.d_regs[reg] = (self.d_regs[reg] & 0xFFFF_0000) | result as u32;
                } else {
                    memory.write_u16(addr.expect("memory mode must resolve address"), result);
                }
                self.update_sub_flags_word_with_extend(0, dst, result);
                self.set_flag(CCR_X, dst != 0);
                Some(if mode == 0b000 { 4 } else { 8 + ea_cycles })
            }
            0b10 => {
                let (dst, addr) = if mode == 0b000 {
                    (self.d_regs[reg], None)
                } else {
                    let addr = self.resolve_data_alterable_address(mode, reg, 4, memory)?;
                    (memory.read_u32(addr), Some(addr))
                };
                let result = (0u32).wrapping_sub(dst);
                if mode == 0b000 {
                    self.d_regs[reg] = result;
                } else {
                    memory.write_u32(addr.expect("memory mode must resolve address"), result);
                }
                self.update_sub_flags_long_with_extend(0, dst, result);
                self.set_flag(CCR_X, dst != 0);
                Some(if mode == 0b000 { 6 } else { 12 + ea_cycles })
            }
            _ => None,
        }
    }

    fn exec_not(&mut self, opcode: u16, memory: &mut MemoryMap) -> Option<u32> {
        let size = ((opcode >> 6) & 0x3) as u8;
        let mode = ((opcode >> 3) & 0x7) as u8;
        let reg = (opcode & 0x7) as usize;

        // NOT supports data alterable destinations (Dn + memory), but not An direct or immediate.
        if mode == 0b001 || (mode == 0b111 && reg == 0b100) {
            return None;
        }

        let ea_cycles = self.word_ea_calculation_cycles(mode, reg)?;
        match size {
            0b00 => {
                let (dst, addr) = if mode == 0b000 {
                    (self.d_regs[reg] as u8, None)
                } else {
                    let addr = self.resolve_data_alterable_address(mode, reg, 1, memory)?;
                    (memory.read_u8(addr), Some(addr))
                };
                let result = !dst;
                if mode == 0b000 {
                    self.d_regs[reg] = (self.d_regs[reg] & 0xFFFF_FF00) | result as u32;
                } else {
                    memory.write_u8(addr.expect("memory mode must resolve address"), result);
                }
                self.update_test_flags_byte(result);
                Some(if mode == 0b000 { 4 } else { 8 + ea_cycles })
            }
            0b01 => {
                let (dst, addr) = if mode == 0b000 {
                    (self.d_regs[reg] as u16, None)
                } else {
                    let addr = self.resolve_data_alterable_address(mode, reg, 2, memory)?;
                    (memory.read_u16(addr), Some(addr))
                };
                let result = !dst;
                if mode == 0b000 {
                    self.d_regs[reg] = (self.d_regs[reg] & 0xFFFF_0000) | result as u32;
                } else {
                    memory.write_u16(addr.expect("memory mode must resolve address"), result);
                }
                self.update_test_flags_word(result);
                Some(if mode == 0b000 { 4 } else { 8 + ea_cycles })
            }
            0b10 => {
                let (dst, addr) = if mode == 0b000 {
                    (self.d_regs[reg], None)
                } else {
                    let addr = self.resolve_data_alterable_address(mode, reg, 4, memory)?;
                    (memory.read_u32(addr), Some(addr))
                };
                let result = !dst;
                if mode == 0b000 {
                    self.d_regs[reg] = result;
                } else {
                    memory.write_u32(addr.expect("memory mode must resolve address"), result);
                }
                self.update_test_flags_long(result);
                Some(if mode == 0b000 { 6 } else { 12 + ea_cycles })
            }
            _ => None,
        }
    }

    fn exec_negx(&mut self, opcode: u16, memory: &mut MemoryMap) -> Option<u32> {
        let size = ((opcode >> 6) & 0x3) as u8;
        let mode = ((opcode >> 3) & 0x7) as u8;
        let reg = (opcode & 0x7) as usize;

        // NEGX supports data alterable destinations only.
        if mode == 0b001 || (mode == 0b111 && reg == 0b100) {
            return None;
        }

        let ea_cycles = self.word_ea_calculation_cycles(mode, reg)?;
        let x_in = if self.flag_set(CCR_X) { 1u32 } else { 0u32 };
        let prev_z = self.flag_set(CCR_Z);
        match size {
            0b00 => {
                let (dst, addr) = if mode == 0b000 {
                    (self.d_regs[reg] as u8, None)
                } else {
                    let addr = self.resolve_data_alterable_address(mode, reg, 1, memory)?;
                    (memory.read_u8(addr), Some(addr))
                };
                let src = (dst as u16) + x_in as u16;
                let result = (0u16).wrapping_sub(src) as u8;
                let borrow = src != 0;
                let overflow = ((0u8 ^ src as u8) & (0u8 ^ result) & 0x80) != 0;
                if mode == 0b000 {
                    self.d_regs[reg] = (self.d_regs[reg] & 0xFFFF_FF00) | result as u32;
                } else {
                    memory.write_u8(addr.expect("memory mode must resolve address"), result);
                }
                self.set_flag(CCR_N, (result & 0x80) != 0);
                self.set_flag(CCR_Z, prev_z && result == 0);
                self.set_flag(CCR_V, overflow);
                self.set_flag(CCR_C, borrow);
                self.set_flag(CCR_X, borrow);
                Some(if mode == 0b000 { 4 } else { 8 + ea_cycles })
            }
            0b01 => {
                let (dst, addr) = if mode == 0b000 {
                    (self.d_regs[reg] as u16, None)
                } else {
                    let addr = self.resolve_data_alterable_address(mode, reg, 2, memory)?;
                    (memory.read_u16(addr), Some(addr))
                };
                let src = (dst as u32) + x_in;
                let result = (0u32).wrapping_sub(src) as u16;
                let borrow = src != 0;
                let overflow = ((0u16 ^ src as u16) & (0u16 ^ result) & 0x8000) != 0;
                if mode == 0b000 {
                    self.d_regs[reg] = (self.d_regs[reg] & 0xFFFF_0000) | result as u32;
                } else {
                    memory.write_u16(addr.expect("memory mode must resolve address"), result);
                }
                self.set_flag(CCR_N, (result & 0x8000) != 0);
                self.set_flag(CCR_Z, prev_z && result == 0);
                self.set_flag(CCR_V, overflow);
                self.set_flag(CCR_C, borrow);
                self.set_flag(CCR_X, borrow);
                Some(if mode == 0b000 { 4 } else { 8 + ea_cycles })
            }
            0b10 => {
                let (dst, addr) = if mode == 0b000 {
                    (self.d_regs[reg], None)
                } else {
                    let addr = self.resolve_data_alterable_address(mode, reg, 4, memory)?;
                    (memory.read_u32(addr), Some(addr))
                };
                let src = (dst as u64) + x_in as u64;
                let result = (0u64).wrapping_sub(src) as u32;
                let borrow = src != 0;
                let overflow = ((0u32 ^ src as u32) & (0u32 ^ result) & 0x8000_0000) != 0;
                if mode == 0b000 {
                    self.d_regs[reg] = result;
                } else {
                    memory.write_u32(addr.expect("memory mode must resolve address"), result);
                }
                self.set_flag(CCR_N, (result & 0x8000_0000) != 0);
                self.set_flag(CCR_Z, prev_z && result == 0);
                self.set_flag(CCR_V, overflow);
                self.set_flag(CCR_C, borrow);
                self.set_flag(CCR_X, borrow);
                Some(if mode == 0b000 { 6 } else { 12 + ea_cycles })
            }
            _ => None,
        }
    }

    fn exec_nbcd(&mut self, opcode: u16, memory: &mut MemoryMap) -> Option<u32> {
        let mode = ((opcode >> 3) & 0x7) as u8;
        let reg = (opcode & 0x7) as usize;
        if mode == 0b001 || (mode == 0b111 && reg == 0b100) {
            return None;
        }

        let x_in = if self.flag_set(CCR_X) { 1i32 } else { 0i32 };
        let prev_z = self.flag_set(CCR_Z);
        let (dst, addr) = if mode == 0b000 {
            (self.d_regs[reg] as u8, None)
        } else {
            let addr = self.resolve_data_alterable_address(mode, reg, 1, memory)?;
            (memory.read_u8(addr), Some(addr))
        };
        let dst_dec = ((dst >> 4) as i32) * 10 + (dst & 0x0F) as i32;
        let mut diff = -dst_dec - x_in;
        let borrow = diff < 0;
        if borrow {
            diff += 100;
        }
        let result = (((diff / 10) as u8) << 4) | ((diff % 10) as u8);
        if mode == 0b000 {
            self.d_regs[reg] = (self.d_regs[reg] & 0xFFFF_FF00) | result as u32;
        } else {
            memory.write_u8(addr.expect("memory mode must resolve address"), result);
        }

        self.set_flag(CCR_N, (result & 0x80) != 0);
        self.set_flag(CCR_Z, prev_z && result == 0);
        self.set_flag(CCR_V, false);
        self.set_flag(CCR_C, borrow);
        self.set_flag(CCR_X, borrow);
        if mode == 0b000 {
            Some(6)
        } else {
            let ea_cycles = self.word_ea_calculation_cycles(mode, reg).unwrap_or(0);
            Some(8 + ea_cycles)
        }
    }

    fn exec_tas(&mut self, opcode: u16, memory: &mut MemoryMap) -> Option<u32> {
        let mode = ((opcode >> 3) & 0x7) as u8;
        let reg = (opcode & 0x7) as usize;
        if mode == 0b001 || (mode == 0b111 && reg == 0b100) {
            return None;
        }

        let value = if mode == 0b000 {
            self.d_regs[reg] as u8
        } else {
            let addr = self.resolve_data_alterable_address(mode, reg, 1, memory)?;
            let value = memory.read_u8(addr);
            // Real Genesis TAS has a broken write cycle on the external bus:
            // the read-modify-write sequence completes the read but the write
            // never asserts /LDS+/UDS properly, so memory is NOT updated.
            // Only register-direct mode (handled above) writes back.
            value
        };
        if mode == 0b000 {
            let result = value | 0x80;
            self.d_regs[reg] = (self.d_regs[reg] & 0xFFFF_FF00) | result as u32;
        }

        self.set_flag(CCR_N, (value & 0x80) != 0);
        self.set_flag(CCR_Z, value == 0);
        self.set_flag(CCR_V, false);
        self.set_flag(CCR_C, false);
        Some(if mode == 0b000 { 4 } else { 10 })
    }

    fn exec_swap(&mut self, opcode: u16) -> u32 {
        let reg = (opcode & 0x7) as usize;
        let result = self.d_regs[reg].rotate_left(16);
        self.d_regs[reg] = result;
        self.update_test_flags_long(result);
        4
    }

    fn exec_pea(&mut self, opcode: u16, memory: &mut MemoryMap) -> Option<u32> {
        let mode = ((opcode >> 3) & 0x7) as u8;
        let reg = (opcode & 0x7) as usize;
        let addr = match mode {
            0b010 => self.a_regs[reg],
            0b101 => {
                let disp = self.fetch_u16(memory) as i16 as i32;
                self.a_regs[reg].wrapping_add_signed(disp)
            }
            0b111 => match reg {
                0b000 => self.fetch_u16(memory) as i16 as i32 as u32,
                0b001 => self.fetch_u32(memory),
                0b010 => {
                    let base_pc = self.pc;
                    let disp = self.fetch_u16(memory) as i16 as i32;
                    base_pc.wrapping_add_signed(disp)
                }
                _ => return None,
            },
            _ => return None,
        };
        self.push_u32(memory, addr);
        // PEA timing: (An)=12, d(An)=16, xxx.W=16, xxx.L=20, d(PC)=16
        let cycles = match mode {
            0b010 => 12,
            0b101 => 16,
            0b111 => match reg {
                0b000 => 16,  // xxx.W
                0b001 => 20,  // xxx.L
                0b010 => 16,  // d(PC)
                _ => 16,
            },
            _ => 16,
        };
        Some(cycles)
    }

    fn exec_ext_w(&mut self, opcode: u16) -> u32 {
        let reg = (opcode & 0x7) as usize;
        let extended = (self.d_regs[reg] as u8 as i8 as i16) as u16;
        self.d_regs[reg] = (self.d_regs[reg] & 0xFFFF_0000) | extended as u32;
        self.update_test_flags_word(extended);
        4
    }

    fn exec_ext_l(&mut self, opcode: u16) -> u32 {
        let reg = (opcode & 0x7) as usize;
        let extended = (self.d_regs[reg] as u16 as i16 as i32) as u32;
        self.d_regs[reg] = extended;
        self.update_test_flags_long(extended);
        4
    }

    fn exec_movem(&mut self, opcode: u16, memory: &mut MemoryMap) -> Option<u32> {
        let mem_to_regs = (opcode & 0x0400) != 0;
        let size_long = (opcode & 0x0040) != 0;
        let mode = ((opcode >> 3) & 0x7) as u8;
        let reg = (opcode & 0x7) as usize;
        let mask = self.fetch_u16(memory);
        let count = mask.count_ones();
        if count == 0 {
            return Some(8);
        }

        if mem_to_regs {
            let (mut addr, postinc_reg) = self.movem_resolve_mem_source(mode, reg, memory)?;
            let step = if size_long { 4 } else { 2 };

            for bit in 0..16 {
                if (mask & (1u16 << bit)) == 0 {
                    continue;
                }
                let value = if size_long {
                    memory.read_u32(addr)
                } else {
                    memory.read_u16(addr) as i16 as i32 as u32
                };
                self.movem_set_register(bit as usize, value);
                addr = addr.wrapping_add(step);
            }

            if let Some(an) = postinc_reg {
                self.a_regs[an] = addr;
            }
        } else {
            let (mut addr, predec_reg) = self.movem_resolve_mem_dest(mode, reg, memory)?;
            let step = if size_long { 4 } else { 2 };

            if let Some(an) = predec_reg {
                for bit in 0..16 {
                    if (mask & (1u16 << bit)) == 0 {
                        continue;
                    }
                    let reg_index = 15 - bit as usize;
                    let value = self.movem_get_register(reg_index);
                    addr = addr.wrapping_sub(step);
                    if size_long {
                        memory.write_u32(addr, value);
                    } else {
                        memory.write_u16(addr, value as u16);
                    }
                }
                self.a_regs[an] = addr;
            } else {
                for bit in 0..16 {
                    if (mask & (1u16 << bit)) == 0 {
                        continue;
                    }
                    let value = self.movem_get_register(bit as usize);
                    if size_long {
                        memory.write_u32(addr, value);
                    } else {
                        memory.write_u16(addr, value as u16);
                    }
                    addr = addr.wrapping_add(step);
                }
            }
        }

        // MOVEM timing: per-register cost is 4 (word) or 8 (long)
        let per_reg = if size_long { 8 } else { 4 };
        if mem_to_regs {
            // mem to reg: 12 + n*per_reg (+ ea for non-postinc modes)
            let base = 12 + count * per_reg;
            let ea_extra = match mode {
                0b011 => 0, // (An)+ has no extra EA cost
                0b010 => 0, // (An) — no extension
                0b101 => 8, // d(An)
                0b110 => 10, // d(An,Xn)
                0b111 => match reg {
                    0b000 => 8,  // xxx.W
                    0b001 => 12, // xxx.L
                    0b010 => 8,  // d(PC)
                    0b011 => 10, // d(PC,Xn)
                    _ => 0,
                },
                _ => 0,
            };
            Some(base + ea_extra)
        } else {
            // reg to mem: 8 + n*per_reg (+ ea for non-predec modes)
            let base = 8 + count * per_reg;
            let ea_extra = match mode {
                0b100 => 0, // -(An) has no extra EA cost
                0b010 => 0, // (An) — no extension
                0b101 => 8, // d(An)
                0b110 => 10, // d(An,Xn)
                0b111 => match reg {
                    0b000 => 8,  // xxx.W
                    0b001 => 12, // xxx.L
                    _ => 0,
                },
                _ => 0,
            };
            Some(base + ea_extra)
        }
    }

    fn exec_shift_rotate(&mut self, opcode: u16, memory: &mut MemoryMap) -> Option<u32> {
        let size = ((opcode >> 6) & 0x3) as u8;
        if size == 0b11 {
            let op = ((opcode >> 9) & 0x7) as u8;
            let mode = ((opcode >> 3) & 0x7) as u8;
            let reg = (opcode & 0x7) as usize;

            // Memory form uses data-alterable memory EA only.
            if mode == 0b000 || mode == 0b001 || (mode == 0b111 && reg >= 0b010) {
                return None;
            }

            let ea_cycles = self.word_ea_calculation_cycles(mode, reg)?;
            let addr = self.resolve_data_alterable_address(mode, reg, 2, memory)?;
            let value = memory.read_u16(addr);
            let (result, carry_out, overflow) = match op {
                // ASR.W <ea>
                0b000 => {
                    let carry = (value & 0x0001) != 0;
                    let result = ((value as i16) >> 1) as u16;
                    self.set_flag(CCR_X, carry);
                    (result, carry, false)
                }
                // ASL.W <ea>
                0b001 => {
                    let carry = (value & 0x8000) != 0;
                    let result = value.wrapping_shl(1);
                    let overflow = ((value ^ result) & 0x8000) != 0;
                    self.set_flag(CCR_X, carry);
                    (result, carry, overflow)
                }
                // LSR.W <ea>
                0b010 => {
                    let carry = (value & 0x0001) != 0;
                    let result = value >> 1;
                    self.set_flag(CCR_X, carry);
                    (result, carry, false)
                }
                // LSL.W <ea>
                0b011 => {
                    let carry = (value & 0x8000) != 0;
                    let result = value.wrapping_shl(1);
                    self.set_flag(CCR_X, carry);
                    (result, carry, false)
                }
                // ROXR.W <ea>
                0b100 => {
                    let x_in = self.flag_set(CCR_X);
                    let carry = (value & 0x0001) != 0;
                    let result = (value >> 1) | ((x_in as u16) << 15);
                    self.set_flag(CCR_X, carry);
                    (result, carry, false)
                }
                // ROXL.W <ea>
                0b101 => {
                    let x_in = self.flag_set(CCR_X);
                    let carry = (value & 0x8000) != 0;
                    let result = value.wrapping_shl(1) | (x_in as u16);
                    self.set_flag(CCR_X, carry);
                    (result, carry, false)
                }
                // ROR.W <ea>
                0b110 => {
                    let carry = (value & 0x0001) != 0;
                    let result = (value >> 1) | ((carry as u16) << 15);
                    (result, carry, false)
                }
                // ROL.W <ea>
                0b111 => {
                    let carry = (value & 0x8000) != 0;
                    let result = value.wrapping_shl(1) | (carry as u16);
                    (result, carry, false)
                }
                _ => return None,
            };

            memory.write_u16(addr, result);
            self.set_flag(CCR_N, (result & 0x8000) != 0);
            self.set_flag(CCR_Z, result == 0);
            self.set_flag(CCR_V, overflow);
            self.set_flag(CCR_C, carry_out);
            return Some(8 + ea_cycles); // Memory shift/rotate: 8+ea
        }

        let dst = (opcode & 0x7) as usize;
        let op = ((opcode >> 3) & 0x3) as u8;
        let left = (opcode & 0x0100) != 0;
        let count_from_reg = (opcode & 0x0020) != 0;
        let count_field = ((opcode >> 9) & 0x7) as usize;
        let shift_count = if count_from_reg {
            (self.d_regs[count_field] & 0x3F) as u32
        } else {
            let imm = count_field as u32;
            if imm == 0 { 8 } else { imm }
        };
        let mut count = shift_count;

        let (width, mask, sign_bit) = match size {
            0b00 => (8u32, 0x0000_00FFu32, 0x0000_0080u32),
            0b01 => (16u32, 0x0000_FFFFu32, 0x0000_8000u32),
            0b10 => (32u32, 0xFFFF_FFFFu32, 0x8000_0000u32),
            _ => return None,
        };
        let mut value = self.d_regs[dst] & mask;
        let mut carry_out = false;
        let x_before = self.flag_set(CCR_X);
        let mut as_left_overflow = false;

        if count > 0 {
            while count > 0 {
                match op {
                    // ASx
                    0b00 => {
                        if left {
                            let old = value;
                            carry_out = (value & sign_bit) != 0;
                            value = (value << 1) & mask;
                            as_left_overflow |= ((old ^ value) & sign_bit) != 0;
                        } else {
                            carry_out = (value & 0x1) != 0;
                            let fill = value & sign_bit;
                            value >>= 1;
                            if fill != 0 {
                                value |= sign_bit;
                            }
                        }
                    }
                    // LSx
                    0b01 => {
                        if left {
                            carry_out = (value & sign_bit) != 0;
                            value = (value << 1) & mask;
                        } else {
                            carry_out = (value & 0x1) != 0;
                            value >>= 1;
                        }
                    }
                    // ROXx
                    0b10 => {
                        let x_in = self.flag_set(CCR_X);
                        if left {
                            carry_out = (value & sign_bit) != 0;
                            value = ((value << 1) & mask) | (x_in as u32);
                        } else {
                            carry_out = (value & 0x1) != 0;
                            value = (value >> 1) | ((x_in as u32) << (width - 1));
                            value &= mask;
                        }
                        self.set_flag(CCR_X, carry_out);
                    }
                    // ROx
                    0b11 => {
                        if left {
                            carry_out = (value & sign_bit) != 0;
                            value = ((value << 1) & mask) | (carry_out as u32);
                        } else {
                            carry_out = (value & 0x1) != 0;
                            value = (value >> 1) | ((carry_out as u32) << (width - 1));
                            value &= mask;
                        }
                    }
                    _ => return None,
                }
                count -= 1;
            }
        }

        self.set_shift_rotate_result(dst, size, value);
        self.set_flag(CCR_N, (value & sign_bit) != 0);
        self.set_flag(CCR_Z, value == 0);
        self.set_flag(
            CCR_V,
            if op == 0b00 && left {
                as_left_overflow
            } else {
                false
            },
        );

        if shift_count == 0 {
            // 68000 register shift/rotate semantics for count=0:
            // ASx/LSx/ROx: C cleared, X unaffected.
            // ROXx: C reflects previous X, X unaffected.
            let c = if op == 0b10 { x_before } else { false };
            self.set_flag(CCR_C, c);
        } else {
            self.set_flag(CCR_C, carry_out);
            if op != 0b11 {
                self.set_flag(CCR_X, carry_out);
            }
        }
        // Byte/Word: 6+2n, Long: 8+2n
        let base = if size == 0b10 { 8 } else { 6 };
        Some(base + shift_count * 2)
    }

    fn exec_movep(&mut self, opcode: u16, memory: &mut MemoryMap) -> Option<u32> {
        let dn = ((opcode >> 9) & 0x7) as usize;
        let opmode = ((opcode >> 6) & 0x3) as u8;
        let an = (opcode & 0x7) as usize;
        let displacement = self.fetch_u16(memory) as i16 as i32;
        let addr = self.a_regs[an].wrapping_add_signed(displacement);

        match opmode {
            // MOVEP.W (d16,An),Dn
            0b00 => {
                let hi = memory.read_u8(addr);
                let lo = memory.read_u8(addr.wrapping_add(2));
                let value = u16::from_be_bytes([hi, lo]) as u32;
                self.d_regs[dn] = (self.d_regs[dn] & 0xFFFF_0000) | value;
                Some(16)
            }
            // MOVEP.L (d16,An),Dn
            0b01 => {
                let b0 = memory.read_u8(addr);
                let b1 = memory.read_u8(addr.wrapping_add(2));
                let b2 = memory.read_u8(addr.wrapping_add(4));
                let b3 = memory.read_u8(addr.wrapping_add(6));
                self.d_regs[dn] = u32::from_be_bytes([b0, b1, b2, b3]);
                Some(24)
            }
            // MOVEP.W Dn,(d16,An)
            0b10 => {
                let bytes = (self.d_regs[dn] as u16).to_be_bytes();
                memory.write_u8(addr, bytes[0]);
                memory.write_u8(addr.wrapping_add(2), bytes[1]);
                Some(16)
            }
            // MOVEP.L Dn,(d16,An)
            0b11 => {
                let bytes = self.d_regs[dn].to_be_bytes();
                memory.write_u8(addr, bytes[0]);
                memory.write_u8(addr.wrapping_add(2), bytes[1]);
                memory.write_u8(addr.wrapping_add(4), bytes[2]);
                memory.write_u8(addr.wrapping_add(6), bytes[3]);
                Some(24)
            }
            _ => None,
        }
    }

    fn exec_bit_dynamic(&mut self, opcode: u16, memory: &mut MemoryMap) -> Option<u32> {
        let bit_reg = ((opcode >> 9) & 0x7) as usize;
        let op = ((opcode >> 6) & 0x3) as u8;
        let mode = ((opcode >> 3) & 0x7) as u8;
        let reg = (opcode & 0x7) as usize;
        let bit_num = self.d_regs[bit_reg] as u8;
        self.exec_bit_op(op, mode, reg, bit_num, memory, true)
    }

    fn exec_bit_immediate(&mut self, opcode: u16, memory: &mut MemoryMap) -> Option<u32> {
        let op = ((opcode >> 6) & 0x3) as u8;
        let mode = ((opcode >> 3) & 0x7) as u8;
        let reg = (opcode & 0x7) as usize;
        let bit_num = self.fetch_u16(memory) as u8;
        self.exec_bit_op(op, mode, reg, bit_num, memory, false)
    }

    fn exec_bit_op(
        &mut self,
        op: u8,
        mode: u8,
        reg: usize,
        bit_num: u8,
        memory: &mut MemoryMap,
        dynamic: bool,
    ) -> Option<u32> {
        if mode == 0b000 {
            let bit = (bit_num & 0x1F) as u32;
            let mask = 1u32 << bit;
            let old_set = (self.d_regs[reg] & mask) != 0;
            self.set_flag(CCR_Z, !old_set);
            match op {
                0b00 => {}
                0b01 => self.d_regs[reg] ^= mask,
                0b10 => self.d_regs[reg] &= !mask,
                0b11 => self.d_regs[reg] |= mask,
                _ => return None,
            }
            // MC68000: BTST Dn=6/10, BCHG/BCLR/BSET Dn=8/12
            let base = if op == 0b00 {
                // BTST
                if dynamic { 6 } else { 10 }
            } else {
                // BCHG/BCLR/BSET
                if dynamic { 8 } else { 12 }
            };
            return Some(base);
        }

        // Memory destinations are byte-sized and must be data alterable.
        if mode == 0b001 || (mode == 0b111 && reg == 0b100) {
            return None;
        }
        let addr = self.resolve_data_alterable_address(mode, reg, 1, memory)?;
        let mut value = memory.read_u8(addr);
        let bit = bit_num & 0x07;
        let mask = 1u8 << bit;
        let old_set = (value & mask) != 0;
        self.set_flag(CCR_Z, !old_set);
        match op {
            0b00 => {}
            0b01 => value ^= mask,
            0b10 => value &= !mask,
            0b11 => value |= mask,
            _ => return None,
        }
        if op != 0b00 {
            memory.write_u8(addr, value);
        }
        let ea_cycles = self.word_ea_calculation_cycles(mode, reg).unwrap_or(0);
        // MC68000: BTST mem=4+ea/8+ea, BCHG/BCLR/BSET mem=8+ea/12+ea
        let base = if op == 0b00 {
            if dynamic { 4 } else { 8 }
        } else {
            if dynamic { 8 } else { 12 }
        };
        Some(base + ea_cycles)
    }

    fn movem_resolve_mem_source(
        &mut self,
        mode: u8,
        reg: usize,
        memory: &mut MemoryMap,
    ) -> Option<(u32, Option<usize>)> {
        match mode {
            0b010 => Some((self.a_regs[reg], None)),
            0b011 => Some((self.a_regs[reg], Some(reg))),
            0b101 => {
                let disp = self.fetch_u16(memory) as i16 as i32;
                Some((self.a_regs[reg].wrapping_add_signed(disp), None))
            }
            0b110 => Some((self.resolve_indexed_address(self.a_regs[reg], memory), None)),
            0b111 => match reg {
                0b000 => Some((self.fetch_u16(memory) as i16 as i32 as u32, None)),
                0b001 => Some((self.fetch_u32(memory), None)),
                0b010 => {
                    let base_pc = self.pc;
                    let disp = self.fetch_u16(memory) as i16 as i32;
                    Some((base_pc.wrapping_add_signed(disp), None))
                }
                0b011 => Some((self.resolve_pc_indexed_address(memory), None)),
                _ => None,
            },
            _ => None,
        }
    }

    fn movem_resolve_mem_dest(
        &mut self,
        mode: u8,
        reg: usize,
        memory: &mut MemoryMap,
    ) -> Option<(u32, Option<usize>)> {
        match mode {
            0b010 => Some((self.a_regs[reg], None)),
            0b100 => Some((self.a_regs[reg], Some(reg))),
            0b101 => {
                let disp = self.fetch_u16(memory) as i16 as i32;
                Some((self.a_regs[reg].wrapping_add_signed(disp), None))
            }
            0b110 => Some((self.resolve_indexed_address(self.a_regs[reg], memory), None)),
            0b111 => match reg {
                0b000 => Some((self.fetch_u16(memory) as i16 as i32 as u32, None)),
                0b001 => Some((self.fetch_u32(memory), None)),
                _ => None,
            },
            _ => None,
        }
    }

    fn movem_get_register(&self, index: usize) -> u32 {
        if index < 8 {
            self.d_regs[index]
        } else {
            self.a_regs[index - 8]
        }
    }

    fn set_shift_rotate_result(&mut self, reg: usize, size: u8, value: u32) {
        match size {
            0b00 => {
                self.d_regs[reg] = (self.d_regs[reg] & 0xFFFF_FF00) | (value & 0xFF);
            }
            0b01 => {
                self.d_regs[reg] = (self.d_regs[reg] & 0xFFFF_0000) | (value & 0xFFFF);
            }
            0b10 => {
                self.d_regs[reg] = value;
            }
            _ => {}
        }
    }

    fn movem_set_register(&mut self, index: usize, value: u32) {
        if index < 8 {
            self.d_regs[index] = value;
        } else {
            self.a_regs[index - 8] = value;
        }
    }

    fn exec_rts(&mut self, memory: &mut MemoryMap) -> u32 {
        self.pc = self.pop_u32(memory);
        16
    }

    fn exec_rte(&mut self, memory: &mut MemoryMap) -> u32 {
        if (self.sr & SR_SUPERVISOR) == 0 {
            self.raise_exception(8, memory, None);
            return 34;
        }

        if self.pending_group0_frames > 0 {
            let _access_info = self.pop_u16(memory);
            let _fault_addr = self.pop_u32(memory);
            let _instruction_word = self.pop_u16(memory);
            let restored_sr = self.pop_u16(memory);
            self.pc = self.pop_u32(memory);
            self.write_sr(restored_sr);
            self.pending_group0_frames = self.pending_group0_frames.saturating_sub(1);
            return 20;
        }

        let restored_sr = self.pop_u16(memory);
        self.pc = self.pop_u32(memory);
        self.write_sr(restored_sr);
        20
    }

    fn exec_rtr(&mut self, memory: &mut MemoryMap) -> u32 {
        let ccr = self.pop_u16(memory) & 0x001F;
        self.sr = (self.sr & !0x001F) | ccr;
        self.pc = self.pop_u32(memory);
        20
    }

    fn exec_trapv(&mut self, memory: &mut MemoryMap) -> u32 {
        if self.flag_set(CCR_V) {
            self.raise_exception(7, memory, None);
            34
        } else {
            4
        }
    }

    fn exec_stop(&mut self, memory: &mut MemoryMap) -> u32 {
        if (self.sr & SR_SUPERVISOR) == 0 {
            self.raise_exception(8, memory, None);
            return 34;
        }
        let value = self.fetch_u16(memory);
        self.write_sr(value);
        self.stopped = true;
        4
    }

    fn exec_branch(&mut self, opcode: u16, memory: &mut MemoryMap, cond: u8) -> u32 {
        let displacement = (opcode & 0x00FF) as u8;
        let should_branch = self.condition_true(cond);
        if displacement == 0 {
            let base_pc = self.pc;
            let disp16 = self.fetch_u16(memory) as i16 as i32;
            if should_branch {
                self.pc = base_pc.wrapping_add_signed(disp16);
                10
            } else {
                12
            }
        } else {
            if should_branch {
                let disp8 = displacement as i8 as i32;
                self.pc = self.pc.wrapping_add_signed(disp8);
                10
            } else {
                8
            }
        }
    }

    fn exec_bcc(&mut self, opcode: u16, memory: &mut MemoryMap) -> Option<u32> {
        let cond = ((opcode >> 8) & 0xF) as u8;
        if cond == 0x0 || cond == 0x1 {
            return None;
        }
        Some(self.exec_branch(opcode, memory, cond))
    }

    fn exec_bsr(&mut self, opcode: u16, memory: &mut MemoryMap) -> u32 {
        let displacement = (opcode & 0x00FF) as u8;
        if displacement == 0 {
            let base_pc = self.pc;
            let disp16 = self.fetch_u16(memory) as i16 as i32;
            let return_addr = self.pc;
            self.push_u32(memory, return_addr);
            self.pc = base_pc.wrapping_add_signed(disp16);
        } else {
            let return_addr = self.pc;
            self.push_u32(memory, return_addr);
            let disp8 = displacement as i8 as i32;
            self.pc = self.pc.wrapping_add_signed(disp8);
        }
        18
    }

    fn exec_trap(&mut self, opcode: u16, memory: &mut MemoryMap) -> u32 {
        let vector = 32 + (opcode as u32 & 0x0F);
        self.raise_exception(vector, memory, None);
        34
    }

    fn exec_illegal(&mut self, memory: &mut MemoryMap) -> u32 {
        self.raise_exception(4, memory, None);
        34
    }

    fn exec_unknown_as_illegal(&mut self, opcode: u16, memory: &mut MemoryMap) -> u32 {
        self.record_unknown_opcode(opcode, self.pc.wrapping_sub(2));
        self.exec_illegal(memory)
    }

    fn exec_bkpt_68000(&mut self, memory: &mut MemoryMap) -> u32 {
        // BKPT is not implemented on MC68000; treat it as ILLEGAL.
        self.exec_illegal(memory)
    }

    fn exec_line_a(&mut self, memory: &mut MemoryMap) -> u32 {
        self.raise_exception(10, memory, None);
        34
    }

    fn exec_line_f(&mut self, memory: &mut MemoryMap) -> u32 {
        self.raise_exception(11, memory, None);
        34
    }

    fn exec_address_error(&mut self, memory: &mut MemoryMap) -> u32 {
        self.raise_address_error(
            memory,
            self.pc,
            AddressErrorAccess::InstructionRead,
            self.current_opcode,
        );
        self.pending_exception_cycles.take().unwrap_or(50)
    }

    fn exec_reset(&mut self, memory: &mut MemoryMap) -> u32 {
        if (self.sr & SR_SUPERVISOR) == 0 {
            self.raise_exception(8, memory, None);
            34
        } else {
            memory.pulse_external_reset();
            132
        }
    }

    fn service_interrupt(&mut self, level: u8, memory: &mut MemoryMap) -> bool {
        if !(1..=7).contains(&level) {
            return false;
        }
        let current_mask = ((self.sr & SR_INT_MASK) >> 8) as u8;
        if level <= current_mask {
            return false;
        }

        self.raise_exception(24 + level as u32, memory, Some(level));
        self.stopped = false;
        true
    }

    fn raise_exception(
        &mut self,
        vector: u32,
        memory: &mut MemoryMap,
        interrupt_level: Option<u8>,
    ) {
        self.exception_raised_this_step = true;
        *self.exception_histogram.entry(vector).or_insert(0) += 1;
        self.stopped = false;
        let old_sr = self.sr;

        // Exceptions always stack on the supervisor stack.
        if (self.sr & SR_SUPERVISOR) == 0 {
            self.usp = self.a_regs[7];
            self.a_regs[7] = self.ssp;
        }

        self.push_u32(memory, self.pc);
        self.push_u16(memory, old_sr);
        self.ssp = self.a_regs[7];

        // Exceptions force supervisor mode and clear trace on 68000.
        self.sr = (old_sr | SR_SUPERVISOR) & !SR_TRACE;
        if let Some(level) = interrupt_level {
            self.sr = (self.sr & !SR_INT_MASK) | ((level as u16) << 8);
        }

        let vector_addr = vector * 4;
        self.pc = memory.read_u32(vector_addr);
    }

    fn condition_true(&self, cond: u8) -> bool {
        let n = self.flag_set(CCR_N);
        let z = self.flag_set(CCR_Z);
        let v = self.flag_set(CCR_V);
        let c = self.flag_set(CCR_C);
        match cond & 0xF {
            0x0 => true,
            0x1 => false,
            0x2 => !c && !z,
            0x3 => c || z,
            0x4 => !c,
            0x5 => c,
            0x6 => !z,
            0x7 => z,
            0x8 => !v,
            0x9 => v,
            0xA => !n,
            0xB => n,
            0xC => n == v,
            0xD => n != v,
            0xE => !z && (n == v),
            0xF => z || (n != v),
            _ => unreachable!(),
        }
    }

    fn resolve_data_alterable_address(
        &mut self,
        mode: u8,
        reg: usize,
        size_bytes: u32,
        memory: &mut MemoryMap,
    ) -> Option<u32> {
        let addr_step = if size_bytes == 1 {
            self.byte_addr_step(reg)
        } else {
            size_bytes
        };
        match mode {
            0b010 => self.ensure_aligned_for_size(self.a_regs[reg], size_bytes, memory),
            0b011 => {
                let addr = self.a_regs[reg];
                self.a_regs[reg] = self.a_regs[reg].wrapping_add(addr_step);
                self.ensure_aligned_for_size(addr, size_bytes, memory)
            }
            0b100 => {
                self.a_regs[reg] = self.a_regs[reg].wrapping_sub(addr_step);
                self.ensure_aligned_for_size(self.a_regs[reg], size_bytes, memory)
            }
            0b101 => {
                let disp = self.fetch_u16(memory) as i16 as i32;
                let addr = self.a_regs[reg].wrapping_add_signed(disp);
                self.ensure_aligned_for_size(addr, size_bytes, memory)
            }
            0b110 => {
                let addr = self.resolve_indexed_address(self.a_regs[reg], memory);
                self.ensure_aligned_for_size(addr, size_bytes, memory)
            }
            0b111 => match reg {
                0b000 => {
                    let addr = self.fetch_u16(memory) as i16 as i32 as u32;
                    self.ensure_aligned_for_size(addr, size_bytes, memory)
                }
                0b001 => {
                    let addr = self.fetch_u32(memory);
                    self.ensure_aligned_for_size(addr, size_bytes, memory)
                }
                _ => None,
            },
            _ => None,
        }
    }

    fn read_ea_byte(&mut self, mode: u8, reg: usize, memory: &mut MemoryMap) -> Option<u8> {
        match mode {
            0b000 => Some(self.d_regs[reg] as u8),
            0b001 => None,
            0b010 => Some(memory.read_u8(self.a_regs[reg])),
            0b011 => {
                let addr = self.a_regs[reg];
                let value = memory.read_u8(addr);
                self.a_regs[reg] = self.a_regs[reg].wrapping_add(self.byte_addr_step(reg));
                Some(value)
            }
            0b100 => {
                self.a_regs[reg] = self.a_regs[reg].wrapping_sub(self.byte_addr_step(reg));
                Some(memory.read_u8(self.a_regs[reg]))
            }
            0b101 => {
                let disp = self.fetch_u16(memory) as i16 as i32;
                let addr = self.a_regs[reg].wrapping_add_signed(disp);
                Some(memory.read_u8(addr))
            }
            0b110 => {
                let addr = self.resolve_indexed_address(self.a_regs[reg], memory);
                Some(memory.read_u8(addr))
            }
            0b111 => match reg {
                0b000 => {
                    let addr = self.fetch_u16(memory) as i16 as i32 as u32;
                    Some(memory.read_u8(addr))
                }
                0b001 => {
                    let addr = self.fetch_u32(memory);
                    Some(memory.read_u8(addr))
                }
                0b010 => {
                    let base_pc = self.pc;
                    let disp = self.fetch_u16(memory) as i16 as i32;
                    Some(memory.read_u8(base_pc.wrapping_add_signed(disp)))
                }
                0b011 => {
                    let addr = self.resolve_pc_indexed_address(memory);
                    Some(memory.read_u8(addr))
                }
                0b100 => Some(self.fetch_u16(memory) as u8),
                _ => None,
            },
            _ => None,
        }
    }

    fn write_ea_byte(
        &mut self,
        mode: u8,
        reg: usize,
        value: u8,
        memory: &mut MemoryMap,
    ) -> Option<()> {
        match mode {
            0b000 => {
                self.d_regs[reg] = (self.d_regs[reg] & 0xFFFF_FF00) | value as u32;
                Some(())
            }
            0b001 => None,
            0b010 => {
                memory.write_u8(self.a_regs[reg], value);
                Some(())
            }
            0b011 => {
                let addr = self.a_regs[reg];
                memory.write_u8(addr, value);
                self.a_regs[reg] = self.a_regs[reg].wrapping_add(self.byte_addr_step(reg));
                Some(())
            }
            0b100 => {
                self.a_regs[reg] = self.a_regs[reg].wrapping_sub(self.byte_addr_step(reg));
                memory.write_u8(self.a_regs[reg], value);
                Some(())
            }
            0b101 => {
                let disp = self.fetch_u16(memory) as i16 as i32;
                let addr = self.a_regs[reg].wrapping_add_signed(disp);
                memory.write_u8(addr, value);
                Some(())
            }
            0b110 => {
                let addr = self.resolve_indexed_address(self.a_regs[reg], memory);
                memory.write_u8(addr, value);
                Some(())
            }
            0b111 => match reg {
                0b000 => {
                    let addr = self.fetch_u16(memory) as i16 as i32 as u32;
                    memory.write_u8(addr, value);
                    Some(())
                }
                0b001 => {
                    let addr = self.fetch_u32(memory);
                    memory.write_u8(addr, value);
                    Some(())
                }
                _ => None,
            },
            _ => None,
        }
    }

    fn read_ea_word(&mut self, mode: u8, reg: usize, memory: &mut MemoryMap) -> Option<u16> {
        match mode {
            0b000 => Some(self.d_regs[reg] as u16),
            0b001 => Some(self.a_regs[reg] as u16),
            0b010 => {
                let addr = self.ensure_aligned_for_size(self.a_regs[reg], 2, memory)?;
                Some(memory.read_u16(addr))
            }
            0b011 => {
                let addr = self.a_regs[reg];
                self.ensure_aligned_for_size(addr, 2, memory)?;
                let value = memory.read_u16(addr);
                self.a_regs[reg] = self.a_regs[reg].wrapping_add(2);
                Some(value)
            }
            0b100 => {
                self.a_regs[reg] = self.a_regs[reg].wrapping_sub(2);
                let addr = self.ensure_aligned_for_size(self.a_regs[reg], 2, memory)?;
                Some(memory.read_u16(addr))
            }
            0b101 => {
                let disp = self.fetch_u16(memory) as i16 as i32;
                let addr = self.a_regs[reg].wrapping_add_signed(disp);
                self.ensure_aligned_for_size(addr, 2, memory)?;
                Some(memory.read_u16(addr))
            }
            0b110 => {
                let addr = self.resolve_indexed_address(self.a_regs[reg], memory);
                self.ensure_aligned_for_size(addr, 2, memory)?;
                Some(memory.read_u16(addr))
            }
            0b111 => match reg {
                0b000 => {
                    let addr = self.fetch_u16(memory) as i16 as i32 as u32;
                    self.ensure_aligned_for_size(addr, 2, memory)?;
                    Some(memory.read_u16(addr))
                }
                0b001 => {
                    let addr = self.fetch_u32(memory);
                    self.ensure_aligned_for_size(addr, 2, memory)?;
                    Some(memory.read_u16(addr))
                }
                0b010 => {
                    let base_pc = self.pc;
                    let disp = self.fetch_u16(memory) as i16 as i32;
                    let addr = base_pc.wrapping_add_signed(disp);
                    self.ensure_aligned_for_size(addr, 2, memory)?;
                    Some(memory.read_u16(addr))
                }
                0b011 => {
                    let addr = self.resolve_pc_indexed_address(memory);
                    self.ensure_aligned_for_size(addr, 2, memory)?;
                    Some(memory.read_u16(addr))
                }
                0b100 => Some(self.fetch_u16(memory)),
                _ => None,
            },
            _ => None,
        }
    }

    fn read_ea_long(&mut self, mode: u8, reg: usize, memory: &mut MemoryMap) -> Option<u32> {
        match mode {
            0b000 => Some(self.d_regs[reg]),
            0b001 => Some(self.a_regs[reg]),
            0b010 => {
                let addr = self.ensure_aligned_for_size(self.a_regs[reg], 4, memory)?;
                Some(memory.read_u32(addr))
            }
            0b011 => {
                let addr = self.a_regs[reg];
                self.ensure_aligned_for_size(addr, 4, memory)?;
                let value = memory.read_u32(addr);
                self.a_regs[reg] = self.a_regs[reg].wrapping_add(4);
                Some(value)
            }
            0b100 => {
                self.a_regs[reg] = self.a_regs[reg].wrapping_sub(4);
                let addr = self.ensure_aligned_for_size(self.a_regs[reg], 4, memory)?;
                Some(memory.read_u32(addr))
            }
            0b101 => {
                let disp = self.fetch_u16(memory) as i16 as i32;
                let addr = self.a_regs[reg].wrapping_add_signed(disp);
                self.ensure_aligned_for_size(addr, 4, memory)?;
                Some(memory.read_u32(addr))
            }
            0b110 => {
                let addr = self.resolve_indexed_address(self.a_regs[reg], memory);
                self.ensure_aligned_for_size(addr, 4, memory)?;
                Some(memory.read_u32(addr))
            }
            0b111 => match reg {
                0b000 => {
                    let addr = self.fetch_u16(memory) as i16 as i32 as u32;
                    self.ensure_aligned_for_size(addr, 4, memory)?;
                    Some(memory.read_u32(addr))
                }
                0b001 => {
                    let addr = self.fetch_u32(memory);
                    self.ensure_aligned_for_size(addr, 4, memory)?;
                    Some(memory.read_u32(addr))
                }
                0b010 => {
                    let base_pc = self.pc;
                    let disp = self.fetch_u16(memory) as i16 as i32;
                    let addr = base_pc.wrapping_add_signed(disp);
                    self.ensure_aligned_for_size(addr, 4, memory)?;
                    Some(memory.read_u32(addr))
                }
                0b011 => {
                    let addr = self.resolve_pc_indexed_address(memory);
                    self.ensure_aligned_for_size(addr, 4, memory)?;
                    Some(memory.read_u32(addr))
                }
                0b100 => Some(self.fetch_u32(memory)),
                _ => None,
            },
            _ => None,
        }
    }

    /// MC68000 MOVE destination base cycles (before adding source EA time).
    /// For MOVE.L, memory destinations add 4 cycles vs MOVE.B/W; Dn stays at 4.
    fn move_dest_base_cycles(dst_mode: u8, dst_reg: usize, is_long: bool) -> u32 {
        let mem_extra = if is_long { 4 } else { 0 };
        match dst_mode {
            0b000 => 4,                       // Dn (same for all sizes)
            0b010 => 8 + mem_extra,            // (An)
            0b011 => 8 + mem_extra,            // (An)+
            0b100 => 8 + mem_extra,            // -(An)
            0b101 => 12 + mem_extra,           // d(An)
            0b110 => 14 + mem_extra,           // d(An,Xn)
            0b111 => match dst_reg {
                0b000 => 12 + mem_extra,       // xxx.W
                0b001 => 16 + mem_extra,       // xxx.L
                _ => 8 + mem_extra,            // fallback
            },
            _ => 8 + mem_extra,                // fallback
        }
    }

    /// MC68000 EA calculation time for long-sized source in MOVE.L.
    /// Register direct is 0; all memory/immediate modes add 4 to the word EA time.
    fn long_ea_calculation_cycles(&self, mode: u8, reg: usize) -> Option<u32> {
        let base = self.word_ea_calculation_cycles(mode, reg)?;
        if mode == 0b000 || mode == 0b001 {
            Some(base) // register direct: 0 for both word and long
        } else {
            Some(base + 4)
        }
    }

    fn word_ea_calculation_cycles(&self, mode: u8, reg: usize) -> Option<u32> {
        // MC68000 Table 8-1: effective address calculation times (byte/word column).
        let cycles = match mode {
            0b000 => 0,  // Dn
            0b001 => 0,  // An
            0b010 => 4,  // (An)
            0b011 => 4,  // (An)+
            0b100 => 6,  // -(An)
            0b101 => 8,  // (d16,An)
            0b110 => 10, // (d8,An,Xn)
            0b111 => match reg {
                0b000 => 8,  // (xxx).W
                0b001 => 12, // (xxx).L
                0b010 => 8,  // (d16,PC)
                0b011 => 10, // (d8,PC,Xn)
                0b100 => 4,  // #<data>
                _ => return None,
            },
            _ => return None,
        };
        Some(cycles)
    }

    fn divu_word_exec_cycles(dividend: u32, divisor: u16) -> u32 {
        let divisor_u32 = divisor as u32;

        // Overflow is detected before the restoring division loop.
        if (dividend >> 16) >= divisor_u32 {
            return 10;
        }

        // MC68000 unsigned restoring divide timing model.
        let mut mcycles = 38u32;
        let hdivisor = divisor_u32 << 16;
        let mut rem = dividend;
        for _ in 0..15 {
            let old_rem = rem;
            rem <<= 1;
            if (old_rem & 0x8000_0000) != 0 {
                rem = rem.wrapping_sub(hdivisor);
            } else {
                mcycles += 2;
                if rem >= hdivisor {
                    rem = rem.wrapping_sub(hdivisor);
                    mcycles -= 1;
                }
            }
        }
        mcycles * 2
    }

    fn divs_word_exec_cycles(dividend: i32, divisor: i16) -> u32 {
        // MC68000 signed divide timing model.
        let mut mcycles = 6u32;
        if dividend < 0 {
            mcycles += 1;
        }

        let dividend_abs = dividend.unsigned_abs();
        let divisor_abs = (divisor as i32).unsigned_abs();

        // Detect absolute overflow early.
        if (dividend_abs >> 16) >= divisor_abs {
            return (mcycles + 2) * 2;
        }

        let mut abs_quotient = (dividend_abs / divisor_abs) as u16;
        mcycles += 55;

        if divisor >= 0 {
            if dividend >= 0 {
                mcycles -= 1;
            } else {
                mcycles += 1;
            }
        }

        for _ in 0..15 {
            if (abs_quotient & 0x8000) == 0 {
                mcycles += 1;
            }
            abs_quotient <<= 1;
        }

        mcycles * 2
    }

    fn write_ea_word(
        &mut self,
        mode: u8,
        reg: usize,
        value: u16,
        memory: &mut MemoryMap,
    ) -> Option<()> {
        match mode {
            0b000 => {
                self.d_regs[reg] = (self.d_regs[reg] & 0xFFFF_0000) | value as u32;
                Some(())
            }
            0b010 => {
                let addr = self.ensure_aligned_for_size_write(self.a_regs[reg], 2, memory)?;
                memory.write_u16(addr, value);
                Some(())
            }
            0b011 => {
                let addr = self.a_regs[reg];
                self.ensure_aligned_for_size_write(addr, 2, memory)?;
                memory.write_u16(addr, value);
                self.a_regs[reg] = self.a_regs[reg].wrapping_add(2);
                Some(())
            }
            0b100 => {
                self.a_regs[reg] = self.a_regs[reg].wrapping_sub(2);
                let addr = self.ensure_aligned_for_size_write(self.a_regs[reg], 2, memory)?;
                memory.write_u16(addr, value);
                Some(())
            }
            0b101 => {
                let disp = self.fetch_u16(memory) as i16 as i32;
                let addr = self.a_regs[reg].wrapping_add_signed(disp);
                self.ensure_aligned_for_size_write(addr, 2, memory)?;
                memory.write_u16(addr, value);
                Some(())
            }
            0b110 => {
                let addr = self.resolve_indexed_address(self.a_regs[reg], memory);
                self.ensure_aligned_for_size_write(addr, 2, memory)?;
                memory.write_u16(addr, value);
                Some(())
            }
            0b111 => match reg {
                0b000 => {
                    let addr = self.fetch_u16(memory) as i16 as i32 as u32;
                    self.ensure_aligned_for_size_write(addr, 2, memory)?;
                    memory.write_u16(addr, value);
                    Some(())
                }
                0b001 => {
                    let addr = self.fetch_u32(memory);
                    self.ensure_aligned_for_size_write(addr, 2, memory)?;
                    memory.write_u16(addr, value);
                    Some(())
                }
                _ => None,
            },
            _ => None,
        }
    }

    fn write_ea_long(
        &mut self,
        mode: u8,
        reg: usize,
        value: u32,
        memory: &mut MemoryMap,
    ) -> Option<()> {
        match mode {
            0b000 => {
                self.d_regs[reg] = value;
                Some(())
            }
            0b010 => {
                let addr = self.ensure_aligned_for_size_write(self.a_regs[reg], 4, memory)?;
                memory.write_u32(addr, value);
                Some(())
            }
            0b011 => {
                let addr = self.a_regs[reg];
                self.ensure_aligned_for_size_write(addr, 4, memory)?;
                memory.write_u32(addr, value);
                self.a_regs[reg] = self.a_regs[reg].wrapping_add(4);
                Some(())
            }
            0b100 => {
                self.a_regs[reg] = self.a_regs[reg].wrapping_sub(4);
                let addr = self.ensure_aligned_for_size_write(self.a_regs[reg], 4, memory)?;
                memory.write_u32(addr, value);
                Some(())
            }
            0b101 => {
                let disp = self.fetch_u16(memory) as i16 as i32;
                let addr = self.a_regs[reg].wrapping_add_signed(disp);
                self.ensure_aligned_for_size_write(addr, 4, memory)?;
                memory.write_u32(addr, value);
                Some(())
            }
            0b110 => {
                let addr = self.resolve_indexed_address(self.a_regs[reg], memory);
                self.ensure_aligned_for_size_write(addr, 4, memory)?;
                memory.write_u32(addr, value);
                Some(())
            }
            0b111 => match reg {
                0b000 => {
                    let addr = self.fetch_u16(memory) as i16 as i32 as u32;
                    self.ensure_aligned_for_size_write(addr, 4, memory)?;
                    memory.write_u32(addr, value);
                    Some(())
                }
                0b001 => {
                    let addr = self.fetch_u32(memory);
                    self.ensure_aligned_for_size_write(addr, 4, memory)?;
                    memory.write_u32(addr, value);
                    Some(())
                }
                _ => None,
            },
            _ => None,
        }
    }

    fn fetch_u16(&mut self, memory: &mut MemoryMap) -> u16 {
        if (self.pc & 1) != 0 {
            self.raise_address_error(
                memory,
                self.pc,
                AddressErrorAccess::InstructionRead,
                self.current_opcode,
            );
            return 0;
        }
        let value = memory.read_u16(self.pc);
        self.pc = self.pc.wrapping_add(2);
        value
    }

    fn fetch_u32(&mut self, memory: &mut MemoryMap) -> u32 {
        if (self.pc & 1) != 0 {
            self.raise_address_error(
                memory,
                self.pc,
                AddressErrorAccess::InstructionRead,
                self.current_opcode,
            );
            return 0;
        }
        let value = memory.read_u32(self.pc);
        self.pc = self.pc.wrapping_add(4);
        value
    }

    fn ensure_aligned_for_size(
        &mut self,
        addr: u32,
        size_bytes: u32,
        memory: &mut MemoryMap,
    ) -> Option<u32> {
        if size_bytes >= 2 && (addr & 1) != 0 {
            self.raise_address_error(
                memory,
                addr,
                AddressErrorAccess::DataRead,
                self.current_opcode,
            );
            return None;
        }
        Some(addr)
    }

    fn ensure_aligned_for_size_write(
        &mut self,
        addr: u32,
        size_bytes: u32,
        memory: &mut MemoryMap,
    ) -> Option<u32> {
        if size_bytes >= 2 && (addr & 1) != 0 {
            self.raise_address_error(
                memory,
                addr,
                AddressErrorAccess::DataWrite,
                self.current_opcode,
            );
            return None;
        }
        Some(addr)
    }

    fn resolve_control_address(
        &mut self,
        mode: u8,
        reg: usize,
        memory: &mut MemoryMap,
    ) -> Option<u32> {
        match mode {
            0b010 => Some(self.a_regs[reg]),
            0b101 => {
                let disp = self.fetch_u16(memory) as i16 as i32;
                Some(self.a_regs[reg].wrapping_add_signed(disp))
            }
            0b110 => Some(self.resolve_indexed_address(self.a_regs[reg], memory)),
            0b111 => match reg {
                0b000 => Some(self.fetch_u16(memory) as i16 as i32 as u32),
                0b001 => Some(self.fetch_u32(memory)),
                0b010 => {
                    let base_pc = self.pc;
                    let disp = self.fetch_u16(memory) as i16 as i32;
                    Some(base_pc.wrapping_add_signed(disp))
                }
                0b011 => Some(self.resolve_pc_indexed_address(memory)),
                _ => None,
            },
            _ => None,
        }
    }

    fn resolve_indexed_address(&mut self, base: u32, memory: &mut MemoryMap) -> u32 {
        let ext = self.fetch_u16(memory);
        self.resolve_indexed_address_with_ext(base, ext)
    }

    fn resolve_pc_indexed_address(&mut self, memory: &mut MemoryMap) -> u32 {
        let base_pc = self.pc;
        let ext = self.fetch_u16(memory);
        self.resolve_indexed_address_with_ext(base_pc, ext)
    }

    fn resolve_indexed_address_with_ext(&self, base: u32, ext: u16) -> u32 {
        let displacement = (ext & 0x00FF) as u8 as i8 as i32;
        let index_reg = ((ext >> 12) & 0x7) as usize;
        let index_is_addr = (ext & 0x8000) != 0;
        let index_is_long = (ext & 0x0800) != 0;

        let index_value = if index_is_addr {
            self.a_regs[index_reg]
        } else {
            self.d_regs[index_reg]
        };
        let index_offset = if index_is_long {
            index_value as i32
        } else {
            index_value as u16 as i16 as i32
        };

        base.wrapping_add_signed(displacement)
            .wrapping_add_signed(index_offset)
    }

    fn byte_addr_step(&self, reg: usize) -> u32 {
        if reg == 7 { 2 } else { 1 }
    }

    fn push_u32(&mut self, memory: &mut MemoryMap, value: u32) {
        self.a_regs[7] = self.a_regs[7].wrapping_sub(4);
        memory.write_u32(self.a_regs[7], value);
    }

    fn push_u16(&mut self, memory: &mut MemoryMap, value: u16) {
        self.a_regs[7] = self.a_regs[7].wrapping_sub(2);
        memory.write_u16(self.a_regs[7], value);
    }

    fn pop_u32(&mut self, memory: &mut MemoryMap) -> u32 {
        let value = memory.read_u32(self.a_regs[7]);
        self.a_regs[7] = self.a_regs[7].wrapping_add(4);
        value
    }

    fn pop_u16(&mut self, memory: &mut MemoryMap) -> u16 {
        let value = memory.read_u16(self.a_regs[7]);
        self.a_regs[7] = self.a_regs[7].wrapping_add(2);
        value
    }

    fn raise_address_error(
        &mut self,
        memory: &mut MemoryMap,
        fault_addr: u32,
        access: AddressErrorAccess,
        instruction_word: u16,
    ) {
        self.exception_raised_this_step = true;
        if self.pending_group0_frames > 0 {
            // Double bus/address fault while already processing a group-0
            // exception halts the 68000 until external reset.
            self.hard_halted = true;
            self.stopped = false;
            self.pending_exception_cycles = Some(0);
            self.pending_trace_exception = false;
            return;
        }

        *self.exception_histogram.entry(3).or_insert(0) += 1;
        self.stopped = false;
        let old_sr = self.sr;

        // Address/bus errors use the 68000 group 0 frame.
        if (self.sr & SR_SUPERVISOR) == 0 {
            self.usp = self.a_regs[7];
            self.a_regs[7] = self.ssp;
        }

        let stacked_pc = self.pc;
        self.push_u32(memory, stacked_pc);
        self.push_u16(memory, old_sr);
        self.push_u16(memory, instruction_word);
        self.push_u32(memory, fault_addr);
        self.push_u16(memory, self.address_error_access_info(access));
        self.ssp = self.a_regs[7];
        self.pending_group0_frames = self.pending_group0_frames.saturating_add(1);

        // Address/bus error entry also clears trace.
        self.sr = (old_sr | SR_SUPERVISOR) & !SR_TRACE;
        let vector_addr = 3 * 4;
        self.pc = memory.read_u32(vector_addr);
        self.pending_exception_cycles = Some(50);
    }

    fn address_error_access_info(&self, access: AddressErrorAccess) -> u16 {
        let supervisor = (self.sr & SR_SUPERVISOR) != 0;
        let instruction = matches!(access, AddressErrorAccess::InstructionRead);
        let read = !matches!(access, AddressErrorAccess::DataWrite);
        let fc = match (supervisor, instruction) {
            (false, false) => 0b001, // user data
            (false, true) => 0b010,  // user program
            (true, false) => 0b101,  // supervisor data
            (true, true) => 0b110,   // supervisor program
        };
        (if read { 0x0010 } else { 0 }) | (if instruction { 0 } else { 0x0008 }) | fc
    }

    fn take_pending_exception_cycles(&mut self) -> Option<u32> {
        self.pending_exception_cycles.take()
    }

    fn update_move_flags_word(&mut self, value: u16) {
        self.set_flag(CCR_N, (value & 0x8000) != 0);
        self.set_flag(CCR_Z, value == 0);
        self.set_flag(CCR_V, false);
        self.set_flag(CCR_C, false);
    }

    fn update_move_flags_byte(&mut self, value: u8) {
        self.set_flag(CCR_N, (value & 0x80) != 0);
        self.set_flag(CCR_Z, value == 0);
        self.set_flag(CCR_V, false);
        self.set_flag(CCR_C, false);
    }

    fn update_move_flags_long(&mut self, value: u32) {
        self.set_flag(CCR_N, (value & 0x8000_0000) != 0);
        self.set_flag(CCR_Z, value == 0);
        self.set_flag(CCR_V, false);
        self.set_flag(CCR_C, false);
    }

    fn update_test_flags_word(&mut self, value: u16) {
        self.set_flag(CCR_N, (value & 0x8000) != 0);
        self.set_flag(CCR_Z, value == 0);
        self.set_flag(CCR_V, false);
        self.set_flag(CCR_C, false);
    }

    fn update_test_flags_byte(&mut self, value: u8) {
        self.set_flag(CCR_N, (value & 0x80) != 0);
        self.set_flag(CCR_Z, value == 0);
        self.set_flag(CCR_V, false);
        self.set_flag(CCR_C, false);
    }

    fn update_add_flags_byte(&mut self, result: u8, carry: bool, overflow: bool) {
        self.set_flag(CCR_N, (result & 0x80) != 0);
        self.set_flag(CCR_Z, result == 0);
        self.set_flag(CCR_V, overflow);
        self.set_flag(CCR_C, carry);
    }

    fn update_add_flags_word(&mut self, result: u16, carry: bool, overflow: bool) {
        self.set_flag(CCR_N, (result & 0x8000) != 0);
        self.set_flag(CCR_Z, result == 0);
        self.set_flag(CCR_V, overflow);
        self.set_flag(CCR_C, carry);
    }

    fn update_add_flags_long(&mut self, result: u32, carry: bool, overflow: bool) {
        self.set_flag(CCR_N, (result & 0x8000_0000) != 0);
        self.set_flag(CCR_Z, result == 0);
        self.set_flag(CCR_V, overflow);
        self.set_flag(CCR_C, carry);
    }

    fn update_add_flags_byte_with_extend(&mut self, result: u8, carry: bool, overflow: bool) {
        self.update_add_flags_byte(result, carry, overflow);
        self.set_flag(CCR_X, carry);
    }

    fn update_add_flags_word_with_extend(&mut self, result: u16, carry: bool, overflow: bool) {
        self.update_add_flags_word(result, carry, overflow);
        self.set_flag(CCR_X, carry);
    }

    fn update_add_flags_long_with_extend(&mut self, result: u32, carry: bool, overflow: bool) {
        self.update_add_flags_long(result, carry, overflow);
        self.set_flag(CCR_X, carry);
    }

    fn update_sub_flags_byte(&mut self, dst: u8, src: u8, result: u8) {
        self.set_flag(CCR_N, (result & 0x80) != 0);
        self.set_flag(CCR_Z, result == 0);
        self.set_flag(CCR_V, ((dst ^ src) & (dst ^ result) & 0x80) != 0);
        self.set_flag(CCR_C, src > dst);
    }

    fn update_test_flags_long(&mut self, value: u32) {
        self.set_flag(CCR_N, (value & 0x8000_0000) != 0);
        self.set_flag(CCR_Z, value == 0);
        self.set_flag(CCR_V, false);
        self.set_flag(CCR_C, false);
    }

    fn update_sub_flags_word(&mut self, dst: u16, src: u16, result: u16) {
        self.set_flag(CCR_N, (result & 0x8000) != 0);
        self.set_flag(CCR_Z, result == 0);
        self.set_flag(CCR_V, ((dst ^ src) & (dst ^ result) & 0x8000) != 0);
        self.set_flag(CCR_C, src > dst);
    }

    fn update_sub_flags_long(&mut self, dst: u32, src: u32, result: u32) {
        self.set_flag(CCR_N, (result & 0x8000_0000) != 0);
        self.set_flag(CCR_Z, result == 0);
        self.set_flag(CCR_V, ((dst ^ src) & (dst ^ result) & 0x8000_0000) != 0);
        self.set_flag(CCR_C, src > dst);
    }

    fn update_sub_flags_byte_with_extend(&mut self, dst: u8, src: u8, result: u8) {
        self.update_sub_flags_byte(dst, src, result);
        self.set_flag(CCR_X, src > dst);
    }

    fn update_sub_flags_word_with_extend(&mut self, dst: u16, src: u16, result: u16) {
        self.update_sub_flags_word(dst, src, result);
        self.set_flag(CCR_X, src > dst);
    }

    fn update_sub_flags_long_with_extend(&mut self, dst: u32, src: u32, result: u32) {
        self.update_sub_flags_long(dst, src, result);
        self.set_flag(CCR_X, src > dst);
    }

    fn flag_set(&self, flag: u16) -> bool {
        (self.sr & flag) != 0
    }

    fn set_flag(&mut self, flag: u16, enabled: bool) {
        if enabled {
            self.sr |= flag;
        } else {
            self.sr &= !flag;
        }
    }

    fn write_sr(&mut self, value: u16) {
        let value = value & SR_VALID_MASK_68000;
        let old_supervisor = (self.sr & SR_SUPERVISOR) != 0;
        let new_supervisor = (value & SR_SUPERVISOR) != 0;
        if old_supervisor != new_supervisor {
            if old_supervisor {
                self.ssp = self.a_regs[7];
                self.a_regs[7] = self.usp;
            } else {
                self.usp = self.a_regs[7];
                self.a_regs[7] = self.ssp;
            }
        }
        self.sr = value;
    }

    fn record_unknown_opcode(&mut self, opcode: u16, pc: u32) {
        self.unknown_opcode_total += 1;
        *self.unknown_opcode_histogram.entry(opcode).or_insert(0) += 1;
        *self.unknown_opcode_pc_histogram.entry(pc).or_insert(0) += 1;
    }
}

#[derive(Debug, Clone, Copy)]
enum ArithOp {
    Add,
    Sub,
}

#[derive(Debug, Clone, Copy)]
enum LogicOp {
    And,
    Or,
}

#[derive(Debug, Clone, Copy)]
enum AddressErrorAccess {
    InstructionRead,
    DataRead,
    DataWrite,
}

#[derive(Debug, Clone, Copy)]
enum ImmStore {
    DnByte(usize),
    DnWord(usize),
    DnLong(usize),
    MemByte(u32),
    MemWord(u32),
    MemLong(u32),
}

#[cfg(test)]
#[path = "tests/cpu_tests.rs"]
mod tests;
