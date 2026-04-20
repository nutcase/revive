use super::super::super::super::Cartridge;

impl Cartridge {
    pub(in crate::cartridge) fn read_prg_ram_vrc6(&self, addr: u16) -> u8 {
        let Some(vrc6) = self.mappers.vrc6.as_ref() else {
            return 0;
        };
        if vrc6.banking_control & 0x80 == 0 {
            return 0;
        }
        let offset = (addr as usize).saturating_sub(0x6000);
        self.prg_ram.get(offset).copied().unwrap_or(0)
    }

    pub(in crate::cartridge) fn write_prg_ram_vrc6(&mut self, addr: u16, data: u8) {
        let wram_enabled = self
            .mappers
            .vrc6
            .as_ref()
            .map(|vrc6| vrc6.banking_control & 0x80 != 0)
            .unwrap_or(false);
        if !wram_enabled {
            return;
        }
        let offset = (addr as usize).saturating_sub(0x6000);
        if let Some(cell) = self.prg_ram.get_mut(offset) {
            *cell = data;
        }
    }
}
