use super::super::Cartridge;

impl Cartridge {
    pub(super) fn read_nametable_mapper77(
        &self,
        physical_nt: usize,
        offset: usize,
        internal: &[[u8; 1024]; 2],
    ) -> u8 {
        if physical_nt < 2 {
            let chr_addr = 0x1800 + physical_nt * 0x0400 + offset;
            return self.chr_ram.get(chr_addr).copied().unwrap_or(0);
        }
        internal[(physical_nt - 2) & 1][offset]
    }

    pub(super) fn write_nametable_mapper77(
        &mut self,
        physical_nt: usize,
        offset: usize,
        internal: &mut [[u8; 1024]; 2],
        data: u8,
    ) {
        if physical_nt < 2 {
            let chr_addr = 0x1800 + physical_nt * 0x0400 + offset;
            if let Some(slot) = self.chr_ram.get_mut(chr_addr) {
                *slot = data;
            }
        } else {
            internal[(physical_nt - 2) & 1][offset] = data;
        }
    }

    pub(super) fn read_nametable_mapper99(&self, physical_nt: usize, offset: usize) -> u8 {
        let chr_addr = ((physical_nt & 3) * 0x0400) + offset;
        self.chr_ram.get(chr_addr).copied().unwrap_or(0)
    }

    pub(super) fn write_nametable_mapper99(&mut self, physical_nt: usize, offset: usize, data: u8) {
        let chr_addr = ((physical_nt & 3) * 0x0400) + offset;
        if let Some(slot) = self.chr_ram.get_mut(chr_addr) {
            *slot = data;
        }
    }
}
