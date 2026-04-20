use crate::cartridge::{Cartridge, Mirroring};

impl Cartridge {
    /// MMC2/MMC4 PRG read
    /// MMC2 (mapper 9): 8KB switchable ($8000-$9FFF) + 24KB fixed ($A000-$FFFF)
    /// MMC4 (mapper 10): 16KB switchable ($8000-$BFFF) + 16KB fixed ($C000-$FFFF)
    pub(in crate::cartridge) fn read_prg_mmc2(&self, addr: u16, rom_addr: u16) -> u8 {
        if let Some(ref mmc2) = self.mappers.mmc2 {
            if self.mapper == 9 {
                // MMC2: 8KB switchable + 24KB fixed (last 3 x 8KB banks)
                if addr < 0xA000 {
                    let offset = (mmc2.prg_bank as usize) * 0x2000 + (rom_addr as usize);
                    if offset < self.prg_rom.len() {
                        self.prg_rom[offset]
                    } else {
                        0
                    }
                } else {
                    let fixed_start = self.prg_rom.len() - 0x6000;
                    let offset = fixed_start + ((addr - 0xA000) as usize);
                    if offset < self.prg_rom.len() {
                        self.prg_rom[offset]
                    } else {
                        0
                    }
                }
            } else {
                // MMC4 (mapper 10): 16KB switchable + 16KB fixed
                if addr < 0xC000 {
                    let offset = (mmc2.prg_bank as usize) * 0x4000 + (rom_addr as usize);
                    if offset < self.prg_rom.len() {
                        self.prg_rom[offset]
                    } else {
                        0
                    }
                } else {
                    let last_bank_offset = self.prg_rom.len() - 0x4000;
                    let offset = last_bank_offset + ((addr - 0xC000) as usize);
                    if offset < self.prg_rom.len() {
                        self.prg_rom[offset]
                    } else {
                        0
                    }
                }
            }
        } else {
            0
        }
    }

    /// MMC2/MMC4 PRG write - register decode at $A000-$FFFF
    pub(in crate::cartridge) fn write_prg_mmc2(&mut self, addr: u16, data: u8) {
        if let Some(ref mut mmc2) = self.mappers.mmc2 {
            match addr {
                0xA000..=0xAFFF => {
                    mmc2.prg_bank = data & 0x0F;
                }
                0xB000..=0xBFFF => {
                    mmc2.chr_bank_0_fd = data & 0x1F;
                }
                0xC000..=0xCFFF => {
                    mmc2.chr_bank_0_fe = data & 0x1F;
                }
                0xD000..=0xDFFF => {
                    mmc2.chr_bank_1_fd = data & 0x1F;
                }
                0xE000..=0xEFFF => {
                    mmc2.chr_bank_1_fe = data & 0x1F;
                }
                0xF000..=0xFFFF => {
                    self.mirroring = if data & 0x01 != 0 {
                        Mirroring::Horizontal
                    } else {
                        Mirroring::Vertical
                    };
                }
                _ => {}
            }
        }
    }
}
