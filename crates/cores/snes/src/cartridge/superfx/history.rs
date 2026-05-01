use super::*;

impl SuperFx {
    pub(super) fn is_trivial_reg_write_for_diagnostic(reg: u8, opcode: u8) -> bool {
        match reg & 0x0F {
            4 => opcode == 0xE4,
            12 => opcode == 0x3C,
            14 => opcode == 0xEE,
            _ => false,
        }
    }

    pub(super) fn push_nontrivial_reg_write_history(
        history: &mut Vec<SuperFxRegWrite>,
        write: SuperFxRegWrite,
    ) {
        if let Some(last) = history.last_mut() {
            if last.opcode == write.opcode
                && last.pbr == write.pbr
                && last.pc == write.pc
                && last.reg == write.reg
                && last.src_reg == write.src_reg
                && last.dst_reg == write.dst_reg
            {
                last.new_value = write.new_value;
                last.sfr = write.sfr;
                last.repeats = write.repeats;
                return;
            }
        }
        if history.len() >= trace_superfx_reg_history_cap() {
            history.remove(0);
        }
        history.push(write);
    }

    pub(super) fn push_reg_write_history(
        history: &mut Vec<SuperFxRegWrite>,
        write: SuperFxRegWrite,
    ) {
        if let Some(last) = history.last_mut() {
            if last.opcode == write.opcode
                && last.pbr == write.pbr
                && last.pc == write.pc
                && last.reg == write.reg
                && last.src_reg == write.src_reg
                && last.dst_reg == write.dst_reg
            {
                last.new_value = write.new_value;
                last.sfr = write.sfr;
                last.repeats = write.repeats;
                return;
            }
        }
        if history.len() >= trace_superfx_reg_history_cap() {
            history.remove(0);
        }
        history.push(write);
    }

    pub(super) fn record_low_ram_write(&mut self, addr: u16, old_value: u8, new_value: u8) {
        if addr >= 0x200 || !trace_superfx_low_ram_writes_enabled() {
            return;
        }
        let write = SuperFxRamWrite {
            opcode: self.current_exec_opcode,
            pbr: self.current_exec_pbr,
            pc: self.current_exec_pc,
            addr,
            old_value,
            new_value,
            src_reg: self.src_reg,
            dst_reg: self.dst_reg,
            sfr: self.sfr,
            r10: self.regs[10],
            r12: self.regs[12],
            r14: self.regs[14],
            r15: self.regs[15],
            repeats: 1,
        };
        if let Some(last) = self.recent_low_ram_writes.last_mut() {
            if last.opcode == write.opcode
                && last.pbr == write.pbr
                && last.pc == write.pc
                && last.addr == write.addr
                && last.old_value == write.old_value
                && last.new_value == write.new_value
                && last.src_reg == write.src_reg
                && last.dst_reg == write.dst_reg
                && last.sfr == write.sfr
                && last.r10 == write.r10
                && last.r12 == write.r12
                && last.r14 == write.r14
                && last.r15 == write.r15
            {
                last.repeats = last.repeats.saturating_add(1);
                self.last_low_ram_writes[addr as usize] = Some(last.clone());
                return;
            }
        }
        if self.recent_low_ram_writes.len() >= 64 {
            self.recent_low_ram_writes.remove(0);
        }
        self.recent_low_ram_writes.push(write.clone());
        self.last_low_ram_writes[addr as usize] = Some(write);
    }

    pub(super) fn push_recent_exec_trace(&mut self, exec_pbr: u8, pc: u16, opcode: u8) {
        if !trace_superfx_reg_flow_enabled() {
            return;
        }
        if let Some((bank, start, end)) = *trace_superfx_reg_flow_exclude_range() {
            if exec_pbr == bank && pc >= start && pc <= end {
                return;
            }
        }
        if self.recent_exec_trace.len() >= 64 {
            self.recent_exec_trace.remove(0);
        }
        self.recent_exec_trace.push(SuperFxExecTrace {
            opcode,
            pbr: exec_pbr,
            pc,
            src_reg: self.src_reg,
            dst_reg: self.dst_reg,
            sfr: self.sfr,
            r0: self.regs[0],
            r1: self.regs[1],
            r2: self.regs[2],
            r3: self.regs[3],
            r4: self.regs[4],
            r5: self.regs[5],
            r6: self.regs[6],
            r11: self.regs[11],
            r12: self.regs[12],
            r13: self.regs[13],
            r14: self.regs[14],
            r15: self.regs[15],
        });
    }
}
