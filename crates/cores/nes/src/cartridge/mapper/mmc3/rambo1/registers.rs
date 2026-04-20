use super::super::Mmc3;

impl Mmc3 {
    pub(super) fn rambo1_register(&self, reg: usize) -> u8 {
        if reg < 8 {
            self.bank_registers[reg]
        } else {
            self.extra_bank_registers[reg - 8]
        }
    }

    pub(super) fn set_rambo1_register(&mut self, reg: usize, data: u8) {
        if reg < 8 {
            self.bank_registers[reg] = data;
        } else {
            self.extra_bank_registers[reg - 8] = data;
        }
    }
}
