pub const MBC2_RAM_SIZE: usize = 512;

#[derive(Debug, Clone, Copy)]
pub struct Mbc2State {
    rom_bank: u8,
    ram_enabled: bool,
}

impl Default for Mbc2State {
    fn default() -> Self {
        Self {
            rom_bank: 1,
            ram_enabled: false,
        }
    }
}

impl Mbc2State {
    pub fn write_rom_control(&mut self, addr: u16, value: u8) {
        match addr {
            0x0000..=0x1FFF => {
                // MBC2 uses A8 to distinguish RAM enable from ROM-bank writes.
                if (addr & 0x0100) == 0 {
                    self.ram_enabled = (value & 0x0F) == 0x0A;
                }
            }
            0x2000..=0x3FFF => {
                if (addr & 0x0100) != 0 {
                    let mut bank = value & 0x0F;
                    if bank == 0 {
                        bank = 1;
                    }
                    self.rom_bank = bank;
                }
            }
            _ => {}
        }
    }

    pub fn current_rom_bank(&self, bank_count: usize) -> usize {
        usize::from(self.rom_bank) % bank_count.max(1)
    }

    pub fn read_ram(&self, ram: &[u8], addr: u16) -> u8 {
        if !self.ram_enabled || ram.is_empty() {
            return 0xFF;
        }
        let index = Self::ram_index(addr) % ram.len();
        0xF0 | (ram[index] & 0x0F)
    }

    pub fn write_ram(&self, ram: &mut [u8], addr: u16, value: u8) {
        if !self.ram_enabled || ram.is_empty() {
            return;
        }
        let index = Self::ram_index(addr) % ram.len();
        ram[index] = value & 0x0F;
    }

    #[inline]
    fn ram_index(addr: u16) -> usize {
        (usize::from(addr).wrapping_sub(0xA000)) & 0x01FF
    }
}
