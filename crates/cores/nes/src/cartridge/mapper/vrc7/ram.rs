use super::super::super::Cartridge;

impl Cartridge {
    pub(in crate::cartridge) fn read_prg_ram_vrc7(&self, addr: u16) -> u8 {
        if !self
            .mappers
            .vrc7
            .as_ref()
            .map(|vrc7| vrc7.wram_enabled)
            .unwrap_or(false)
        {
            return 0;
        }

        let offset = (addr as usize).saturating_sub(0x6000);
        self.prg_ram.get(offset).copied().unwrap_or(0)
    }

    pub(in crate::cartridge) fn write_prg_ram_vrc7(&mut self, addr: u16, data: u8) {
        if !self
            .mappers
            .vrc7
            .as_ref()
            .map(|vrc7| vrc7.wram_enabled)
            .unwrap_or(false)
        {
            return;
        }

        let offset = (addr as usize).saturating_sub(0x6000);
        if let Some(cell) = self.prg_ram.get_mut(offset) {
            *cell = data;
            self.has_valid_save_data = true;
        }
    }
}
