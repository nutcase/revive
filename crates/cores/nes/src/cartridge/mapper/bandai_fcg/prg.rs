use crate::cartridge::{Cartridge, Mirroring};

impl Cartridge {
    pub(in crate::cartridge) fn read_prg_bandai(&self, addr: u16) -> u8 {
        if let Some(ref bandai) = self.mappers.bandai_fcg {
            let num_16k_banks = self.prg_rom.len() / 0x4000;
            if num_16k_banks == 0 {
                return 0;
            }

            let (bank, offset) = match addr {
                0x8000..=0xBFFF => {
                    let bank = if self.mapper == 153 {
                        let outer = (bandai.outer_prg_bank as usize & 0x01) << 4;
                        (outer | bandai.prg_bank as usize) % num_16k_banks
                    } else {
                        (bandai.prg_bank as usize) % num_16k_banks
                    };
                    (bank, (addr - 0x8000) as usize)
                }
                0xC000..=0xFFFF => {
                    let bank = if self.mapper == 153 {
                        let outer = (bandai.outer_prg_bank as usize & 0x01) << 4;
                        (outer | 0x0F) % num_16k_banks
                    } else {
                        num_16k_banks - 1
                    };
                    (bank, (addr - 0xC000) as usize)
                }
                _ => return 0,
            };

            let rom_addr = bank * 0x4000 + offset;
            if rom_addr < self.prg_rom.len() {
                self.prg_rom[rom_addr]
            } else {
                0
            }
        } else {
            0
        }
    }

    pub(in crate::cartridge) fn write_prg_bandai(&mut self, addr: u16, data: u8) {
        let Cartridge {
            mapper,
            mappers,
            prg_ram,
            has_valid_save_data,
            mirroring,
            ..
        } = self;
        let bandai_fcg = &mut mappers.bandai_fcg;
        if let Some(ref mut bandai) = bandai_fcg {
            let reg = addr & 0x0F;
            match reg {
                0x00..=0x03 if *mapper == 153 => {
                    bandai.outer_prg_bank = data & 0x01;
                }
                0x00..=0x07 => {
                    if *mapper != 153 {
                        bandai.chr_banks[reg as usize] = data;
                    }
                }
                0x08 => {
                    bandai.prg_bank = data & 0x0F;
                }
                0x09 => {
                    *mirroring = match data & 0x03 {
                        0 => Mirroring::Vertical,
                        1 => Mirroring::Horizontal,
                        2 => Mirroring::OneScreenLower,
                        3 => Mirroring::OneScreenUpper,
                        _ => unreachable!(),
                    };
                }
                0x0A => {
                    bandai.irq_pending.set(false);
                    bandai.irq_enabled = (data & 0x01) != 0;
                    bandai.irq_counter = bandai.irq_latch;
                }
                0x0B => {
                    bandai.irq_latch = (bandai.irq_latch & 0xFF00) | (data as u16);
                }
                0x0C => {
                    bandai.irq_latch = (bandai.irq_latch & 0x00FF) | ((data as u16) << 8);
                }
                0x0D => {
                    if *mapper == 153 {
                        bandai.prg_ram_enabled = data & 0x40 != 0;
                    } else {
                        bandai.eeprom_clock_control(data, prg_ram, has_valid_save_data);
                    }
                }
                _ => {}
            }
        }
    }
}
