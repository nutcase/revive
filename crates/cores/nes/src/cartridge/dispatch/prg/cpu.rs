use super::super::super::Cartridge;

impl Cartridge {
    #[inline]
    pub fn read_prg_cpu(&mut self, addr: u16) -> u8 {
        if self.is_nrom() {
            return self.read_prg_nrom(addr - 0x8000);
        }

        let value = self.read_prg(addr);
        if self.uses_mmc5() {
            self.mmc5_cpu_read_side_effects(addr, value);
        }
        if self.uses_mapper234_read_latch() {
            self.apply_mapper234_value(addr, value);
        }
        value
    }
}
