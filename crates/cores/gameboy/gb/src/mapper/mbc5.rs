const RAM_BANK_SIZE: usize = 8 * 1024;

#[derive(Debug, Clone, Copy)]
pub struct Mbc5State {
    rom_bank_low8: u8,
    rom_bank_high1: u8,
    ram_bank: u8,
    ram_enabled: bool,
}

impl Default for Mbc5State {
    fn default() -> Self {
        Self {
            rom_bank_low8: 1,
            rom_bank_high1: 0,
            ram_bank: 0,
            ram_enabled: false,
        }
    }
}

impl Mbc5State {
    pub fn write_rom_control(&mut self, addr: u16, value: u8) {
        match addr {
            0x0000..=0x1FFF => {
                self.ram_enabled = (value & 0x0F) == 0x0A;
            }
            0x2000..=0x2FFF => {
                self.rom_bank_low8 = value;
            }
            0x3000..=0x3FFF => {
                self.rom_bank_high1 = value & 0x01;
            }
            0x4000..=0x5FFF => {
                self.ram_bank = value & 0x0F;
            }
            _ => {}
        }
    }

    pub fn current_rom_bank(&self, bank_count: usize) -> usize {
        let bank = (usize::from(self.rom_bank_high1 & 0x01) << 8) | usize::from(self.rom_bank_low8);
        bank % bank_count.max(1)
    }

    pub fn read_ram(&self, ram: &[u8], addr: u16) -> u8 {
        if !self.ram_enabled || ram.is_empty() {
            return 0xFF;
        }
        let bank = usize::from(self.ram_bank & 0x0F);
        let offset = (usize::from(addr).wrapping_sub(0xA000)) % RAM_BANK_SIZE;
        let index = bank.saturating_mul(RAM_BANK_SIZE).saturating_add(offset) % ram.len();
        ram[index]
    }

    pub fn write_ram(&self, ram: &mut [u8], addr: u16, value: u8) {
        if !self.ram_enabled || ram.is_empty() {
            return;
        }
        let bank = usize::from(self.ram_bank & 0x0F);
        let offset = (usize::from(addr).wrapping_sub(0xA000)) % RAM_BANK_SIZE;
        let index = bank.saturating_mul(RAM_BANK_SIZE).saturating_add(offset) % ram.len();
        ram[index] = value;
    }
}
