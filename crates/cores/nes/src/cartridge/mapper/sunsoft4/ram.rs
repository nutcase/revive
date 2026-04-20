use crate::cartridge::Cartridge;

impl Cartridge {
    pub(in crate::cartridge) fn read_prg_ram_sunsoft4(&self, addr: u16) -> u8 {
        let Some(prg_ram_enabled) = self
            .mappers
            .sunsoft4
            .as_ref()
            .map(|sunsoft4| sunsoft4.prg_ram_enabled)
        else {
            return 0;
        };
        if !prg_ram_enabled {
            return 0;
        }

        let offset = (addr.saturating_sub(0x6000) as usize) % self.prg_ram.len().max(1);
        self.prg_ram.get(offset).copied().unwrap_or(0)
    }

    pub(in crate::cartridge) fn write_prg_ram_sunsoft4(&mut self, addr: u16, data: u8) {
        let Some(prg_ram_enabled) = self
            .mappers
            .sunsoft4
            .as_ref()
            .map(|sunsoft4| sunsoft4.prg_ram_enabled)
        else {
            return;
        };
        if !prg_ram_enabled || self.prg_ram.is_empty() {
            return;
        }

        let offset = (addr.saturating_sub(0x6000) as usize) % self.prg_ram.len();
        self.prg_ram[offset] = data;
        self.has_valid_save_data = true;
    }
}
