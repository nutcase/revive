use super::super::Cartridge;

#[derive(Debug, Clone)]
pub struct Mapper246 {
    pub prg_banks: [u8; 4],
    pub chr_banks: [u8; 4],
}

impl Mapper246 {
    pub fn new() -> Self {
        let mut mapper = Self {
            prg_banks: [0; 4],
            chr_banks: [0; 4],
        };
        mapper.prg_banks[3] = 0xFF;
        mapper
    }
}

impl Cartridge {
    pub(in crate::cartridge) fn read_prg_mapper246(&self, addr: u16) -> u8 {
        let Some(mapper246) = self.mappers.mapper246.as_ref() else {
            return 0;
        };
        if self.prg_rom.is_empty() {
            return 0;
        }

        let slot = ((addr - 0x8000) as usize) / 0x2000;
        let mut bank = mapper246.prg_banks[slot];
        if slot == 3
            && matches!(
                addr,
                0xFFE4..=0xFFE7 | 0xFFEC..=0xFFEF | 0xFFF4..=0xFFF7 | 0xFFFC..=0xFFFF
            )
        {
            bank |= 0x10;
        }
        let bank_count = (self.prg_rom.len() / 0x2000).max(1);
        let offset = (bank as usize % bank_count) * 0x2000 + ((addr - 0x8000) as usize & 0x1FFF);
        self.prg_rom[offset % self.prg_rom.len()]
    }

    pub(in crate::cartridge) fn read_chr_mapper246(&self, addr: u16) -> u8 {
        let Some(mapper246) = self.mappers.mapper246.as_ref() else {
            return 0;
        };
        if self.chr_rom.is_empty() {
            return 0;
        }

        let slot = (addr as usize) / 0x0800;
        let bank = mapper246.chr_banks[slot];
        let bank_count = (self.chr_rom.len() / 0x0800).max(1);
        let offset = (bank as usize % bank_count) * 0x0800 + (addr as usize & 0x07FF);
        self.chr_rom[offset % self.chr_rom.len()]
    }

    pub(in crate::cartridge) fn read_prg_ram_mapper246(&self, addr: u16) -> u8 {
        if (0x6800..=0x6FFF).contains(&addr) {
            let offset = (addr - 0x6800) as usize;
            self.prg_ram.get(offset).copied().unwrap_or(0)
        } else {
            0
        }
    }

    pub(in crate::cartridge) fn write_prg_ram_mapper246(&mut self, addr: u16, data: u8) {
        if let Some(mapper246) = self.mappers.mapper246.as_mut() {
            match addr {
                0x6000..=0x6003 => {
                    mapper246.prg_banks[(addr - 0x6000) as usize] = data;
                }
                0x6004..=0x6007 => {
                    mapper246.chr_banks[(addr - 0x6004) as usize] = data;
                }
                0x6800..=0x6FFF => {
                    let offset = (addr - 0x6800) as usize;
                    if offset < self.prg_ram.len() {
                        self.prg_ram[offset] = data;
                    }
                }
                _ => {}
            }
        }
    }
}
