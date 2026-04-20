use crate::cartridge::mapper::MemoryMapper;

use crate::bus::Bus;

impl Bus {
    pub(crate) fn sa1_bwram_addr(&self, offset: u16) -> Option<usize> {
        if self.sa1_bwram.is_empty() || offset < 0x6000 {
            return None;
        }
        let window_offset = (offset - 0x6000) as usize;
        let block = (self.sa1.registers.bwram_select_snes & 0x1F) as usize;
        let base = block << 13; // 8 KB blocks
        let idx = base.wrapping_add(window_offset) % self.sa1_bwram.len();
        Some(idx)
    }

    pub(crate) fn sa1_cpu_bwram_addr(&self, offset: u16) -> Option<usize> {
        if self.sa1_bwram.is_empty() || offset < 0x6000 {
            return None;
        }
        let window_offset = (offset - 0x6000) as usize;

        // Check bit 7 of bwram_select_sa1 for bitmap mode
        let select = self.sa1.registers.bwram_select_sa1;
        if (select & 0x80) != 0 {
            // Bitmap view handled separately (banks $60-$6F or projected bitmap window).
            return None;
        }
        // Normal mode: use bits 0-4 (5-bit block selector)
        let block = (select & 0x1F) as usize;
        let base = block << 13; // 8 KB blocks
        let idx = base.wrapping_add(window_offset) % self.sa1_bwram.len();
        Some(idx)
    }

    /// SA-1専用のROM物理アドレス計算 (MMCバンク考慮)
    ///
    /// SA-1は4つの1MBチャンク（C/D/E/F）をLoROM/HiROM窓にマップする。
    /// デフォルトでは C=0, D=1, E=2, F=3 (1MB単位)。
    pub(crate) fn sa1_phys_addr(&self, bank: u32, offset: u16) -> usize {
        // Current MMC mapping
        let reg = &self.sa1.registers;
        let chunk_index = match bank {
            0x00..=0x1F => reg.mmc_bank_c,
            0x20..=0x3F => reg.mmc_bank_d,
            0x80..=0x9F => reg.mmc_bank_e,
            0xA0..=0xBF => reg.mmc_bank_f,
            0xC0..=0xCF => reg.mmc_bank_c,
            0xD0..=0xDF => reg.mmc_bank_d,
            0xE0..=0xEF => reg.mmc_bank_e,
            0xF0..=0xFF => reg.mmc_bank_f,
            _ => 0,
        } as usize;
        let chunk_base = chunk_index * 0x100000; // 1MB units

        match bank {
            // LoROM style windows (32KB per bank, lower half mirrors upper)
            0x00..=0x1F | 0x20..=0x3F | 0x80..=0x9F | 0xA0..=0xBF => {
                let off = (offset | 0x8000) as usize;
                let bank_lo = (bank & 0x1F) as usize;
                chunk_base + bank_lo * 0x8000 + (off - 0x8000)
            }
            // HiROM mirrors for each chunk
            0xC0..=0xFF => chunk_base + offset as usize,
            _ => chunk_base,
        }
    }

    // Dragon Quest 3専用ROM読み取り処理
    pub(crate) fn read_dq3_rom(&mut self, bank: u32, offset: u16) -> u8 {
        let rom_addr = self.dq3_phys_addr(bank as u8, offset);

        let value = self.rom[rom_addr % self.rom_size];
        self.mdr = value;
        value
    }

    #[allow(dead_code)]
    pub(crate) fn is_dq3_enhancement_area(&self, bank: u32, _offset: u16) -> bool {
        matches!(bank, 0x03 | 0x24 | 0x30..=0x37)
    }

    pub(crate) fn handle_dq3_enhancement(&self, bank: u32, offset: u16) -> u8 {
        match bank {
            0x00..=0x3F => {
                let rom_addr = (bank as usize) * 0x10000 + (offset as usize);

                if rom_addr < self.rom_size {
                    self.rom[rom_addr]
                } else {
                    let mirror_addr = rom_addr % self.rom_size;
                    self.rom[mirror_addr]
                }
            }
            0x03 | 0x24 => {
                if offset < 0x8000 {
                    let rom_addr = match bank {
                        0x03 => 0x30000 + (offset as usize),
                        0x24 => 0x240000 + (offset as usize),
                        _ => (bank as usize) * 0x10000 + (offset as usize),
                    };
                    if rom_addr < self.rom_size {
                        self.rom[rom_addr]
                    } else {
                        let mirror_addr = rom_addr % self.rom_size;
                        self.rom[mirror_addr]
                    }
                } else {
                    let mapped_bank = match bank {
                        0x03 => 0x43,
                        0x24 => 0x64,
                        _ => bank,
                    };
                    let rom_addr = ((mapped_bank - 0x40) as usize) * 0x10000 + (offset as usize);
                    if rom_addr < self.rom_size {
                        self.rom[rom_addr]
                    } else {
                        let mirror_addr = rom_addr % self.rom_size;
                        self.rom[mirror_addr]
                    }
                }
            }
            0x30..=0x37 => {
                let rom_addr = ((bank - 0x30) as usize) * 0x10000 + (offset as usize);
                if rom_addr < self.rom_size {
                    self.rom[rom_addr]
                } else {
                    0xFF
                }
            }
            _ => 0xFF,
        }
    }

    pub(crate) fn read_rom_lohi(&self, bank: u32, offset: u16) -> u8 {
        if let Some(ref mapper) = self.mapper {
            let rom_addr = mapper.map_rom(bank as u8, offset, self.rom_size);
            if self.rom_size == 0 {
                0xFF
            } else {
                self.rom[rom_addr % self.rom_size]
            }
        } else {
            // SA-1/DQ3: special handling
            match self.mapper_type {
                crate::cartridge::MapperType::Sa1 => {
                    let rom_addr = self.sa1_phys_addr(bank, offset);
                    if self.rom_size == 0 {
                        0xFF
                    } else {
                        self.rom[rom_addr % self.rom_size]
                    }
                }
                crate::cartridge::MapperType::DragonQuest3 => {
                    if offset < 0x8000 {
                        return self.handle_dq3_enhancement(bank, offset);
                    }
                    let rom_addr = self.dq3_phys_addr(bank as u8, offset);
                    if rom_addr >= self.rom_size {
                        let mirror_addr = rom_addr % self.rom_size;
                        return self.rom[mirror_addr];
                    }
                    let value = self.rom[rom_addr];

                    if bank == 0x08 && offset <= 0x0010 {
                        static mut BANK08_DEBUG_COUNT: u32 = 0;
                        unsafe {
                            BANK08_DEBUG_COUNT += 1;
                            if BANK08_DEBUG_COUNT <= 20 {
                                println!(
                                    "BANK08 READ: {:02X}:{:04X} -> rom_addr=0x{:06X} -> value=0x{:02X}",
                                    bank, offset, rom_addr, value
                                );
                            }
                        }
                    }

                    if (0xFF98..=0xFFA0).contains(&offset) && crate::debug_flags::debug_reset_area()
                    {
                        println!(
                            "RESET AREA read: bank=0x{:02X}, offset=0x{:04X}, value=0x{:02X}",
                            bank, offset, value
                        );
                    }
                    value
                }
                crate::cartridge::MapperType::Spc7110 => {
                    // HiROM-style: bank & 0x3F * 0x10000 + offset (program ROM, first 1MB)
                    let rom_bank = (bank & 0x3F) as usize;
                    let rom_addr = rom_bank * 0x10000 + (offset as usize);
                    if self.rom_size == 0 {
                        0xFF
                    } else {
                        self.rom[rom_addr % self.rom_size]
                    }
                }
                crate::cartridge::MapperType::SuperFx => {
                    if let Some(ref gsu) = self.superfx {
                        let cpu_visible_low_lorom =
                            matches!(bank as u8, 0x00..=0x3F | 0x80..=0xBF) && offset >= 0x8000;
                        if !cpu_visible_low_lorom && !gsu.cpu_has_rom_access() {
                            crate::cartridge::superfx::SuperFx::illegal_rom_read_value(offset)
                        } else if let Some(rom_addr) =
                            crate::cartridge::superfx::SuperFx::cpu_rom_addr(bank as u8, offset)
                        {
                            if self.rom_size == 0 {
                                0xFF
                            } else {
                                self.rom[rom_addr % self.rom_size]
                            }
                        } else {
                            0xFF
                        }
                    } else {
                        0xFF
                    }
                }
                _ => 0xFF,
            }
        }
    }

    /// Dragon Quest III / SA-1 用の物理ROMアドレス計算（Fast HiROM, 4MB）
    pub(crate) fn dq3_phys_addr(&self, bank: u8, offset: u16) -> usize {
        let bank_idx = bank as usize;
        let addr = bank_idx * 0x10000 + offset as usize;
        addr % self.rom_size
    }
}
