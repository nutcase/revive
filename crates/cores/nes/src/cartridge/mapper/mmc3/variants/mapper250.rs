use crate::cartridge::Cartridge;

impl Cartridge {
    pub(in crate::cartridge) fn write_prg_mapper250(&mut self, addr: u16, _data: u8) {
        if !(0x8000..=0xFFFF).contains(&addr) {
            return;
        }

        let synthetic_addr = (addr & 0xE000) | if (addr & 0x0400) != 0 { 1 } else { 0 };
        let synthetic_data = (addr & 0x00FF) as u8;
        self.write_prg_mmc3(synthetic_addr, synthetic_data);
    }
}
