use super::*;

impl SuperFx {
    pub(crate) fn debug_run_steps(&mut self, rom: &[u8], step_budget: usize) {
        self.run_steps(rom, step_budget);
    }

    pub(crate) fn debug_set_reg(&mut self, index: usize, value: u16) {
        self.write_reg(index, value);
    }

    pub(crate) fn debug_set_pbr(&mut self, value: u8) {
        self.pbr = value & 0x7F;
    }

    pub(crate) fn debug_set_rombr(&mut self, value: u8) {
        self.rombr = value & (self.rom_bank_mask as u8);
    }

    pub(crate) fn debug_set_scmr(&mut self, value: u8) {
        self.scmr = value & 0x3F;
    }

    pub(crate) fn debug_set_sfr(&mut self, value: u16) {
        self.sfr = value;
    }

    pub(crate) fn debug_set_src_reg(&mut self, value: u8) {
        self.src_reg = value & 0x0F;
    }

    pub(crate) fn debug_set_dst_reg(&mut self, value: u8) {
        self.dst_reg = value & 0x0F;
    }

    pub(crate) fn debug_set_with_reg(&mut self, value: u8) {
        self.with_reg = value & 0x0F;
    }

    pub(crate) fn debug_clear_pipe(&mut self) {
        self.pipe = default_superfx_pipe();
        self.pipe_valid = false;
        self.pipe_pc = 0;
        self.pipe_pbr = self.pbr;
        self.r14_modified = false;
        self.r15_modified = false;
    }

    pub(crate) fn debug_invoke_cpu_start(&mut self, rom: &[u8]) {
        self.invoke_cpu_start(rom);
    }

    pub(crate) fn debug_prepare_cpu_start(&mut self, rom: &[u8]) {
        let _ = self.prepare_start_execution(rom);
    }
}
