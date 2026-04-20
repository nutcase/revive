use super::Sunsoft4;
use crate::cartridge::Cartridge;

impl Cartridge {
    pub(in crate::cartridge) fn read_prg_sunsoft4(&self, addr: u16) -> u8 {
        let Some(sunsoft4) = self.mappers.sunsoft4.as_ref() else {
            return 0;
        };
        if self.prg_rom.is_empty() {
            return 0;
        }

        if addr < 0xC000 {
            let bank_count = (self.prg_rom.len() / 0x4000).max(1);
            let bank = (sunsoft4.prg_bank as usize) % bank_count;
            let offset = bank * 0x4000 + (addr.saturating_sub(0x8000) as usize);
            self.prg_rom[offset % self.prg_rom.len()]
        } else {
            let offset = self.prg_rom.len().saturating_sub(0x4000) + (addr - 0xC000) as usize;
            self.prg_rom[offset % self.prg_rom.len()]
        }
    }

    pub(in crate::cartridge) fn write_prg_sunsoft4(&mut self, addr: u16, data: u8) {
        let mut new_mirroring = None;
        let mut new_prg_bank = None;
        let new_chr_bank;
        if let Some(sunsoft4) = self.mappers.sunsoft4.as_mut() {
            match addr & 0xF000 {
                0x8000 => sunsoft4.chr_banks[0] = data,
                0x9000 => sunsoft4.chr_banks[1] = data,
                0xA000 => sunsoft4.chr_banks[2] = data,
                0xB000 => sunsoft4.chr_banks[3] = data,
                0xC000 => sunsoft4.nametable_banks[0] = 0x80 | (data & 0x7F),
                0xD000 => sunsoft4.nametable_banks[1] = 0x80 | (data & 0x7F),
                0xE000 => {
                    sunsoft4.control = data;
                    sunsoft4.nametable_chr_rom = data & 0x10 != 0;
                    new_mirroring = Some(Sunsoft4::decode_mirroring(data));
                }
                0xF000 => {
                    sunsoft4.prg_bank = data & 0x0F;
                    sunsoft4.prg_ram_enabled = data & 0x10 != 0;
                    new_prg_bank = Some(sunsoft4.prg_bank);
                }
                _ => {}
            }
            new_chr_bank = sunsoft4.chr_banks[0];
        } else {
            return;
        }

        if let Some(mirroring) = new_mirroring {
            self.mirroring = mirroring;
        }
        if let Some(prg_bank) = new_prg_bank {
            self.prg_bank = prg_bank;
        }
        self.chr_bank = new_chr_bank;
    }
}
