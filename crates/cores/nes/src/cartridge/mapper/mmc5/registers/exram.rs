use super::super::super::super::Cartridge;

impl Cartridge {
    pub(super) fn write_mmc5_exram(&mut self, addr: u16, data: u8) {
        let Some(mmc5) = self.mappers.mmc5.as_mut() else {
            return;
        };

        if mmc5.exram_mode == 0x03 {
            return;
        }

        let exram_addr = (addr - 0x5C00) as usize;
        if let Some(slot) = mmc5.exram.get_mut(exram_addr) {
            *slot = data;
        }
    }

    pub(super) fn read_mmc5_exram(&self, addr: u16) -> u8 {
        let Some(mmc5) = self.mappers.mmc5.as_ref() else {
            return 0;
        };

        match mmc5.exram_mode {
            0x02 | 0x03 => mmc5.exram[(addr - 0x5C00) as usize],
            _ => 0,
        }
    }
}
