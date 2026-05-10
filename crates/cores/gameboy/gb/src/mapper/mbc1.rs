const RAM_BANK_SIZE: usize = 8 * 1024;

#[derive(Debug, Clone, Copy)]
pub struct Mbc1State {
    rom_bank_low5: u8,
    bank_high2: u8,
    ram_enabled: bool,
    mode: Mbc1Mode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mbc1Mode {
    RomBanking,
    RamBanking,
}

impl Default for Mbc1State {
    fn default() -> Self {
        Self {
            rom_bank_low5: 1,
            bank_high2: 0,
            ram_enabled: false,
            mode: Mbc1Mode::RomBanking,
        }
    }
}

impl Mbc1State {
    pub(crate) fn serialize_state(&self, w: &mut crate::state::StateWriter) {
        w.write_u8(self.rom_bank_low5);
        w.write_u8(self.bank_high2);
        w.write_bool(self.ram_enabled);
        w.write_u8(match self.mode {
            Mbc1Mode::RomBanking => 0,
            Mbc1Mode::RamBanking => 1,
        });
    }

    pub(crate) fn deserialize_state(
        &mut self,
        r: &mut crate::state::StateReader<'_>,
    ) -> Result<(), &'static str> {
        self.rom_bank_low5 = r.read_u8()?;
        self.bank_high2 = r.read_u8()? & 0x03;
        self.ram_enabled = r.read_bool()?;
        self.mode = match r.read_u8()? {
            0 => Mbc1Mode::RomBanking,
            1 => Mbc1Mode::RamBanking,
            _ => return Err("invalid MBC1 mode"),
        };
        Ok(())
    }

    pub fn write_rom_control(&mut self, addr: u16, value: u8) {
        match addr {
            0x0000..=0x1FFF => {
                self.ram_enabled = (value & 0x0F) == 0x0A;
            }
            0x2000..=0x3FFF => {
                let mut bank = value & 0x1F;
                if bank == 0 {
                    bank = 1;
                }
                self.rom_bank_low5 = bank;
            }
            0x4000..=0x5FFF => {
                self.bank_high2 = value & 0x03;
            }
            0x6000..=0x7FFF => {
                self.mode = if (value & 0x01) == 0 {
                    Mbc1Mode::RomBanking
                } else {
                    Mbc1Mode::RamBanking
                };
            }
            _ => {}
        }
    }

    pub fn rom_bank_zero(&self, bank_count: usize) -> usize {
        if self.mode == Mbc1Mode::RamBanking {
            ((self.bank_high2 as usize) << 5) % bank_count.max(1)
        } else {
            0
        }
    }

    pub fn current_rom_bank(&self, bank_count: usize) -> usize {
        let mut bank = ((self.bank_high2 as usize) << 5) | (self.rom_bank_low5 as usize & 0x1F);
        if bank.is_multiple_of(0x20) {
            bank += 1;
        }
        bank % bank_count.max(1)
    }

    pub fn read_ram(&self, ram: &[u8], addr: u16) -> u8 {
        if !self.ram_enabled || ram.is_empty() {
            return 0xFF;
        }

        let bank = if self.mode == Mbc1Mode::RamBanking {
            self.bank_high2 as usize
        } else {
            0
        };
        let offset = (usize::from(addr).wrapping_sub(0xA000)) % RAM_BANK_SIZE;
        let index = bank.saturating_mul(RAM_BANK_SIZE).saturating_add(offset) % ram.len();
        ram[index]
    }

    pub fn write_ram(&self, ram: &mut [u8], addr: u16, value: u8) {
        if !self.ram_enabled || ram.is_empty() {
            return;
        }

        let bank = if self.mode == Mbc1Mode::RamBanking {
            self.bank_high2 as usize
        } else {
            0
        };
        let offset = (usize::from(addr).wrapping_sub(0xA000)) % RAM_BANK_SIZE;
        let index = bank.saturating_mul(RAM_BANK_SIZE).saturating_add(offset) % ram.len();
        ram[index] = value;
    }
}
