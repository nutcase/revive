use super::super::super::super::Cartridge;

impl Cartridge {
    pub(in crate::cartridge) fn vrc6_normalize_addr(&self, addr: u16) -> u16 {
        let low = addr & 0x0003;
        let low = if self.mapper == 26 {
            ((low & 0x0001) << 1) | ((low & 0x0002) >> 1)
        } else {
            low
        };
        (addr & 0xF000) | low
    }

    pub(in crate::cartridge) fn vrc6_chr_data(&self) -> &[u8] {
        if !self.chr_ram.is_empty() {
            &self.chr_ram
        } else {
            &self.chr_rom
        }
    }
}
