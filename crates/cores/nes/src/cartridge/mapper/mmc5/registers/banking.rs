use super::super::super::super::Cartridge;

impl Cartridge {
    pub(super) fn write_mmc5_banking_register(&mut self, addr: u16, data: u8) {
        let Some(mmc5) = self.mappers.mmc5.as_mut() else {
            return;
        };

        match addr {
            0x5100 => mmc5.prg_mode = data & 0x03,
            0x5101 => mmc5.chr_mode = data & 0x03,
            0x5102 => mmc5.prg_ram_protect_1 = data & 0x03,
            0x5103 => mmc5.prg_ram_protect_2 = data & 0x03,
            0x5104 => mmc5.exram_mode = data & 0x03,
            0x5105 => {
                for index in 0..4 {
                    mmc5.nametable_map[index] = (data >> (index * 2)) & 0x03;
                }
            }
            0x5106 => mmc5.fill_tile = data,
            0x5107 => mmc5.fill_attr = data & 0x03,
            0x5113 => mmc5.prg_ram_bank = data & 0x0F,
            0x5114..=0x5117 => mmc5.prg_banks[(addr - 0x5114) as usize] = data,
            0x5120..=0x5127 => {
                mmc5.sprite_chr_banks[(addr - 0x5120) as usize] = data;
                mmc5.ppu_data_uses_bg_banks = false;
            }
            0x5128..=0x512B => {
                mmc5.bg_chr_banks[(addr - 0x5128) as usize] = data;
                mmc5.ppu_data_uses_bg_banks = true;
            }
            0x5130 => mmc5.chr_upper = data & 0x03,
            _ => {}
        }
    }

    pub(super) fn write_mmc5_prg_window(&mut self, addr: u16, data: u8) {
        let Some((prg_mode, prg_banks)) = self
            .mappers
            .mmc5
            .as_ref()
            .map(|mmc5| (mmc5.prg_mode & 0x03, mmc5.prg_banks))
        else {
            return;
        };

        match prg_mode {
            0 => self.write_mmc5_prg_target(prg_banks[3], 4, (addr - 0x8000) as usize, data, true),
            1 => {
                if addr < 0xC000 {
                    self.write_mmc5_prg_target(
                        prg_banks[1],
                        2,
                        (addr - 0x8000) as usize,
                        data,
                        false,
                    );
                }
            }
            2 => {
                if addr < 0xC000 {
                    self.write_mmc5_prg_target(
                        prg_banks[1],
                        2,
                        (addr - 0x8000) as usize,
                        data,
                        false,
                    );
                } else if addr < 0xE000 {
                    self.write_mmc5_prg_target(
                        prg_banks[2],
                        1,
                        (addr - 0xC000) as usize,
                        data,
                        false,
                    );
                }
            }
            _ => match addr {
                0x8000..=0x9FFF => self.write_mmc5_prg_target(
                    prg_banks[0],
                    1,
                    (addr - 0x8000) as usize,
                    data,
                    false,
                ),
                0xA000..=0xBFFF => self.write_mmc5_prg_target(
                    prg_banks[1],
                    1,
                    (addr - 0xA000) as usize,
                    data,
                    false,
                ),
                0xC000..=0xDFFF => self.write_mmc5_prg_target(
                    prg_banks[2],
                    1,
                    (addr - 0xC000) as usize,
                    data,
                    false,
                ),
                _ => {}
            },
        }
    }
}
