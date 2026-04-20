use super::super::super::super::Cartridge;

impl Cartridge {
    pub(in crate::cartridge) fn read_prg_vrc6(&self, addr: u16) -> u8 {
        let Some(vrc6) = self.mappers.vrc6.as_ref() else {
            return 0;
        };
        let bank_count_16k = (self.prg_rom.len() / 0x4000).max(1);
        let bank_count_8k = (self.prg_rom.len() / 0x2000).max(1);

        let prg_addr = match addr {
            0x8000..=0xBFFF => {
                let bank = vrc6.prg_bank_16k as usize % bank_count_16k;
                bank * 0x4000 + (addr as usize - 0x8000)
            }
            0xC000..=0xDFFF => {
                let bank = vrc6.prg_bank_8k as usize % bank_count_8k;
                bank * 0x2000 + (addr as usize - 0xC000)
            }
            0xE000..=0xFFFF => {
                let bank = bank_count_8k - 1;
                bank * 0x2000 + (addr as usize - 0xE000)
            }
            _ => return 0,
        };
        self.prg_rom.get(prg_addr).copied().unwrap_or(0)
    }
}
