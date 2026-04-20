use super::Cartridge;

impl Cartridge {
    pub fn has_battery_save(&self) -> bool {
        self.has_battery && !self.prg_ram.is_empty()
    }

    pub fn get_sram_data(&self) -> Option<&[u8]> {
        if self.has_battery && !self.prg_ram.is_empty() && self.has_valid_save_data {
            Some(&self.prg_ram)
        } else {
            None
        }
    }

    pub fn set_sram_data(&mut self, data: Vec<u8>) {
        if self.has_battery && data.len() == self.prg_ram.len() {
            self.prg_ram = data;
            self.has_valid_save_data = true;
        }
    }

    /// Direct reference to PRG-RAM (returns None if empty).
    pub fn prg_ram_ref(&self) -> Option<&[u8]> {
        if self.prg_ram.is_empty() {
            None
        } else {
            Some(&self.prg_ram)
        }
    }

    /// Mutable reference to PRG-RAM (returns None if empty).
    pub fn prg_ram_mut(&mut self) -> Option<&mut [u8]> {
        if self.prg_ram.is_empty() {
            None
        } else {
            Some(&mut self.prg_ram)
        }
    }
}
