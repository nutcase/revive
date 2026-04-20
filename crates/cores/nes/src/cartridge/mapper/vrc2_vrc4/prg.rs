use super::super::super::Cartridge;

impl Cartridge {
    pub(in crate::cartridge) fn read_prg_mapper21(&self, addr: u16) -> u8 {
        self.read_prg_mapper23(addr)
    }

    pub(in crate::cartridge) fn read_prg_mapper22(&self, addr: u16) -> u8 {
        self.read_prg_mapper23(addr)
    }

    pub(in crate::cartridge) fn read_prg_mapper23(&self, addr: u16) -> u8 {
        if let Some(vrc) = self.mappers.vrc2_vrc4.as_ref() {
            if self.prg_rom.is_empty() {
                return 0;
            }

            let bank_count = (self.prg_rom.len() / 0x2000).max(1);
            let second_last = bank_count.saturating_sub(2);
            let last = bank_count.saturating_sub(1);
            let bank = match addr {
                0x8000..=0x9FFF => {
                    if vrc.prg_swap_mode {
                        second_last
                    } else {
                        vrc.prg_banks[0] as usize
                    }
                }
                0xA000..=0xBFFF => vrc.prg_banks[1] as usize,
                0xC000..=0xDFFF => {
                    if vrc.prg_swap_mode {
                        vrc.prg_banks[0] as usize
                    } else {
                        second_last
                    }
                }
                0xE000..=0xFFFF => last,
                _ => return 0,
            } % bank_count;

            let rom_addr = bank * 0x2000 + (addr as usize & 0x1FFF);
            self.prg_rom[rom_addr % self.prg_rom.len()]
        } else {
            0
        }
    }

    pub(in crate::cartridge) fn read_prg_mapper25(&self, addr: u16) -> u8 {
        self.read_prg_mapper23(addr)
    }

    pub(in crate::cartridge) fn write_prg_mapper21(&mut self, addr: u16, data: u8) {
        self.write_prg_mapper23(addr, data);
    }

    pub(in crate::cartridge) fn write_prg_mapper22(&mut self, addr: u16, data: u8) {
        self.write_prg_mapper23(addr, data);
    }

    pub(in crate::cartridge) fn write_prg_mapper23(&mut self, addr: u16, data: u8) {
        let reg = match self.vrc2_vrc4_decode_index(addr) {
            Some(reg) => reg,
            None => return,
        };

        let prg_bank_count = (self.prg_rom.len() / 0x2000).max(1);
        let chr_bank_count = (self.vrc2_vrc4_chr_data().len() / 0x0400).max(1);
        let use_alt_vrc4_decode = self.vrc2_vrc4_uses_alt_vrc4_decode(addr);

        if let Some(vrc) = self.mappers.vrc2_vrc4.as_mut() {
            if use_alt_vrc4_decode {
                vrc.vrc4_mode = true;
            }

            match addr & 0xF000 {
                0x8000 => {
                    vrc.prg_banks[0] = ((data & 0x1F) as usize % prg_bank_count) as u8;
                    self.prg_bank = vrc.prg_banks[0];
                }
                0x9000 => {
                    if self.mapper == 22 {
                        self.mirroring = Self::vrc2_vrc4_decode_mirroring(vrc.vrc4_mode, data);
                    } else {
                        match reg {
                            0 => {
                                self.mirroring =
                                    Self::vrc2_vrc4_decode_mirroring(vrc.vrc4_mode, data);
                            }
                            2 => {
                                vrc.vrc4_mode = true;
                                vrc.wram_enabled = data & 0x01 != 0;
                                vrc.prg_swap_mode = data & 0x02 != 0;
                            }
                            3 => {
                                vrc.vrc4_mode = true;
                            }
                            _ => {}
                        }
                    }
                }
                0xA000 => {
                    vrc.prg_banks[1] = ((data & 0x1F) as usize % prg_bank_count) as u8;
                }
                0xB000..=0xE000 => {
                    if let Some((bank_index, high)) = Self::vrc2_vrc4_decode_chr_index(addr, reg) {
                        let bank = &mut vrc.chr_banks[bank_index];
                        if high {
                            let high_mask = if vrc.vrc4_mode { 0x1F } else { 0x0F };
                            *bank = (*bank & 0x000F) | (((data & high_mask) as u16) << 4);
                        } else {
                            *bank = (*bank & !0x000F) | u16::from(data & 0x0F);
                        }
                        *bank %= chr_bank_count as u16;
                        if bank_index == 0 {
                            self.chr_bank = Self::vrc2_vrc4_effective_chr_bank(
                                self.mapper,
                                *bank,
                                chr_bank_count,
                            ) as u8;
                        }
                    }
                }
                0xF000 => {
                    if self.mapper == 22 {
                        return;
                    }
                    vrc.vrc4_mode = true;
                    match reg {
                        0 => {
                            vrc.irq_latch = (vrc.irq_latch & 0xF0) | (data & 0x0F);
                        }
                        1 => {
                            vrc.irq_latch = (vrc.irq_latch & 0x0F) | ((data & 0x0F) << 4);
                        }
                        2 => {
                            vrc.irq_enable_after_ack = data & 0x01 != 0;
                            vrc.irq_enabled = data & 0x02 != 0;
                            vrc.irq_cycle_mode = data & 0x04 != 0;
                            vrc.irq_pending.set(false);
                            vrc.irq_prescaler = 341;
                            if vrc.irq_enabled {
                                vrc.irq_counter = vrc.irq_latch;
                            }
                        }
                        3 => {
                            vrc.irq_pending.set(false);
                            vrc.irq_enabled = vrc.irq_enable_after_ack;
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        }
    }

    pub(in crate::cartridge) fn write_prg_mapper25(&mut self, addr: u16, data: u8) {
        self.write_prg_mapper23(addr, data);
    }
}
