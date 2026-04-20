use super::super::super::Cartridge;

impl Cartridge {
    pub(in crate::cartridge) fn read_prg_ram_mapper21(&self, addr: u16) -> u8 {
        self.read_prg_ram_mapper23(addr)
    }

    pub(in crate::cartridge) fn read_prg_ram_mapper22(&self, addr: u16) -> u8 {
        self.read_prg_ram_mapper23(addr)
    }

    pub(in crate::cartridge) fn read_prg_ram_mapper23(&self, addr: u16) -> u8 {
        if let Some(vrc) = self.mappers.vrc2_vrc4.as_ref() {
            if self.mapper == 25 && self.has_battery && !self.prg_ram.is_empty() {
                let offset = (addr as usize - 0x6000) % self.prg_ram.len();
                return self.prg_ram[offset];
            }
            if vrc.wram_enabled && !self.prg_ram.is_empty() {
                let offset = (addr as usize - 0x6000) % self.prg_ram.len();
                return self.prg_ram[offset];
            }

            let open_bus = (addr >> 8) as u8;
            if (0x6000..=0x6FFF).contains(&addr) {
                open_bus | (vrc.latch & 0x01)
            } else {
                open_bus
            }
        } else {
            0
        }
    }

    pub(in crate::cartridge) fn read_prg_ram_mapper25(&self, addr: u16) -> u8 {
        self.read_prg_ram_mapper23(addr)
    }

    pub(in crate::cartridge) fn write_prg_ram_mapper21(&mut self, addr: u16, data: u8) {
        self.write_prg_ram_mapper23(addr, data);
    }

    pub(in crate::cartridge) fn write_prg_ram_mapper22(&mut self, addr: u16, data: u8) {
        self.write_prg_ram_mapper23(addr, data);
    }

    pub(in crate::cartridge) fn write_prg_ram_mapper23(&mut self, addr: u16, data: u8) {
        if let Some(vrc) = self.mappers.vrc2_vrc4.as_mut() {
            if self.mapper == 25 && self.has_battery && !self.prg_ram.is_empty() {
                let offset = (addr as usize - 0x6000) % self.prg_ram.len();
                self.prg_ram[offset] = data;
                return;
            }
            if vrc.wram_enabled && !self.prg_ram.is_empty() {
                let offset = (addr as usize - 0x6000) % self.prg_ram.len();
                self.prg_ram[offset] = data;
            } else if (0x6000..=0x6FFF).contains(&addr) {
                vrc.latch = data & 0x01;
            }
        }
    }

    pub(in crate::cartridge) fn write_prg_ram_mapper25(&mut self, addr: u16, data: u8) {
        self.write_prg_ram_mapper23(addr, data);
    }
}
