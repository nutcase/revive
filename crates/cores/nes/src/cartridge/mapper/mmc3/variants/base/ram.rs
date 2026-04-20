use super::super::super::super::super::Cartridge;

impl Cartridge {
    pub(in crate::cartridge) fn mmc3_prg_ram_writable(&self) -> bool {
        self.mappers
            .mmc3
            .as_ref()
            .map(|mmc3| mmc3.prg_ram_enabled && !mmc3.prg_ram_write_protect)
            .unwrap_or(false)
    }

    pub(in crate::cartridge) fn read_prg_ram_mmc3(&self, addr: u16) -> u8 {
        if let Some(ref mmc3) = self.mappers.mmc3 {
            if !mmc3.prg_ram_enabled {
                return 0;
            }
        }
        if !self.prg_ram.is_empty() {
            let ram_addr = (addr - 0x6000) as usize;
            if ram_addr < self.prg_ram.len() {
                self.prg_ram[ram_addr]
            } else {
                0
            }
        } else {
            0
        }
    }

    pub(in crate::cartridge) fn read_prg_ram_mapper115(&self, addr: u16) -> u8 {
        match addr {
            0x6002 => 0,
            _ => self.read_prg_ram_mmc3(addr),
        }
    }

    pub(in crate::cartridge) fn write_prg_ram_mmc3(&mut self, addr: u16, data: u8) {
        if let Some(ref mmc3) = self.mappers.mmc3 {
            if !mmc3.prg_ram_enabled || mmc3.prg_ram_write_protect {
                return;
            }
        }
        if !self.prg_ram.is_empty() {
            let ram_addr = (addr - 0x6000) as usize;
            if ram_addr < self.prg_ram.len() {
                self.prg_ram[ram_addr] = data;
            }
        }
    }
}
