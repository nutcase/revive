use super::super::super::super::Cartridge;

impl Cartridge {
    pub(in crate::cartridge) fn read_prg_fme7(&self, addr: u16) -> u8 {
        if let Some(ref fme7) = self.mappers.fme7 {
            let num_8k_banks = self.prg_rom.len() / 0x2000;
            if num_8k_banks == 0 {
                return 0;
            }
            let bank_mask = num_8k_banks - 1;

            let (bank, offset) = match addr {
                0x8000..=0x9FFF => {
                    let bank = (fme7.prg_banks[0] as usize) & bank_mask;
                    (bank, (addr - 0x8000) as usize)
                }
                0xA000..=0xBFFF => {
                    let bank = (fme7.prg_banks[1] as usize) & bank_mask;
                    (bank, (addr - 0xA000) as usize)
                }
                0xC000..=0xDFFF => {
                    let bank = (fme7.prg_banks[2] as usize) & bank_mask;
                    (bank, (addr - 0xC000) as usize)
                }
                0xE000..=0xFFFF => {
                    let bank = (num_8k_banks - 1) & bank_mask;
                    (bank, (addr - 0xE000) as usize)
                }
                _ => return 0,
            };

            let rom_addr = bank * 0x2000 + offset;
            self.prg_rom.get(rom_addr).copied().unwrap_or(0)
        } else {
            0
        }
    }
}
