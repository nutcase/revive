use super::super::{Cartridge, Mirroring};

#[derive(Debug, Clone)]
pub(in crate::cartridge) struct Mapper15 {
    pub(in crate::cartridge) mode: u8,
    pub(in crate::cartridge) data: u8,
}

impl Mapper15 {
    pub(in crate::cartridge) fn new() -> Self {
        Self { mode: 0, data: 0 }
    }
}

impl Cartridge {
    pub(in crate::cartridge) fn read_prg_mapper15(&self, addr: u16) -> u8 {
        let Some(ref mapper15) = self.mappers.mapper15 else {
            return 0;
        };

        let bank_count_8k = (self.prg_rom.len() / 0x2000).max(1);
        let slot = ((addr - 0x8000) / 0x2000) as usize;
        let bank_8k = match mapper15.mode {
            0 => {
                let bank_32k = ((mapper15.data & 0x3F) >> 1) as usize;
                bank_32k * 4 + slot
            }
            1 => {
                if slot < 2 {
                    let bank_16k = (mapper15.data & 0x3F) as usize;
                    bank_16k * 2 + slot
                } else {
                    let bank_16k = ((mapper15.data & 0x38) | 0x07) as usize;
                    bank_16k * 2 + (slot - 2)
                }
            }
            2 => {
                (((mapper15.data & 0x3F) as usize) << 1) | (((mapper15.data >> 7) & 0x01) as usize)
            }
            3 => {
                let bank_16k = (mapper15.data & 0x3F) as usize;
                bank_16k * 2 + (slot & 1)
            }
            _ => 0,
        } % bank_count_8k;

        let rom_addr = bank_8k * 0x2000 + (addr as usize & 0x1FFF);
        self.prg_rom.get(rom_addr).copied().unwrap_or(0)
    }

    pub(in crate::cartridge) fn write_prg_mapper15(&mut self, addr: u16, data: u8) {
        if let Some(ref mut mapper15) = self.mappers.mapper15 {
            mapper15.mode = (addr & 0x0003) as u8;
            mapper15.data = data;
            self.mirroring = if data & 0x40 != 0 {
                Mirroring::Horizontal
            } else {
                Mirroring::Vertical
            };
        }
    }

    pub(in crate::cartridge) fn read_prg_ram_mapper15(&self, addr: u16) -> u8 {
        let offset = (addr - 0x6000) as usize;
        self.prg_ram.get(offset).copied().unwrap_or(0)
    }

    pub(in crate::cartridge) fn write_prg_ram_mapper15(&mut self, addr: u16, data: u8) {
        let offset = (addr - 0x6000) as usize;
        if let Some(byte) = self.prg_ram.get_mut(offset) {
            *byte = data;
        }
    }
}
