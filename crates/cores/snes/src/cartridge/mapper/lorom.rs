use super::MapperType;
use super::MemoryMapper;

pub struct LoRomMapper;

impl MemoryMapper for LoRomMapper {
    fn mapper_type(&self) -> MapperType {
        MapperType::LoRom
    }

    fn map_rom(&self, bank: u8, offset: u16, rom_size: usize) -> usize {
        // LoROM: 32KB banks in upper half. Use 7-bit bank to reach >2MB.
        let rom_bank = (bank & 0x7F) as usize;
        let rom_addr = rom_bank * 0x8000 + ((offset.wrapping_sub(0x8000)) as usize);
        if rom_size == 0 {
            0
        } else {
            rom_addr % rom_size
        }
    }

    fn read_sram_region(&self, sram: &[u8], sram_size: usize, bank: u8, offset: u16) -> u8 {
        // SRAM window in system banks ($00-$3F/$80-$BF): $6000-$7FFF (8KB)
        if sram_size == 0 {
            0xFF
        } else {
            let bank_index = (bank & 0x3F) as usize;
            let window = bank_index * 0x2000 + ((offset - 0x6000) as usize);
            let idx = window % sram_size;
            sram[idx]
        }
    }

    fn write_sram_region(
        &self,
        sram: &mut [u8],
        sram_size: usize,
        bank: u8,
        offset: u16,
        value: u8,
    ) -> bool {
        // SRAM window in system banks ($00-$3F/$80-$BF): $6000-$7FFF (8KB)
        if sram_size > 0 {
            let bank_index = (bank & 0x3F) as usize;
            let window = bank_index * 0x2000 + ((offset - 0x6000) as usize);
            let idx = window % sram_size;
            sram[idx] = value;
            true
        } else {
            false
        }
    }

    fn read_bank_40_7d(
        &self,
        rom: &[u8],
        sram: &[u8],
        rom_size: usize,
        sram_size: usize,
        bank: u8,
        offset: u16,
    ) -> u8 {
        // LoROM:
        // - banks 0x70-0x7D: $0000-$7FFF maps to cartridge SRAM
        // - other banks: $8000-$FFFF maps to ROM
        if sram_size > 0 && offset < 0x8000 && (0x70..=0x7D).contains(&bank) {
            let window = ((bank - 0x70) as usize) * 0x8000 + (offset as usize);
            let idx = window % sram_size;
            sram[idx]
        } else {
            // ROM read via map_rom
            let rom_addr = self.map_rom(bank, offset, rom_size);
            if rom_size == 0 {
                0xFF
            } else {
                rom[rom_addr]
            }
        }
    }

    fn write_bank_40_7d(
        &self,
        sram: &mut [u8],
        sram_size: usize,
        bank: u8,
        offset: u16,
        value: u8,
    ) -> bool {
        // Banks 0x70-0x7D, offsets 0x0000-0x7FFF map to SRAM
        if sram_size > 0 && offset < 0x8000 && (0x70..=0x7D).contains(&bank) {
            let window = ((bank - 0x70) as usize) * 0x8000 + (offset as usize);
            let idx = window % sram_size;
            sram[idx] = value;
            true
        } else {
            false
        }
    }

    fn read_bank_c0_ff(
        &self,
        rom: &[u8],
        sram: &[u8],
        rom_size: usize,
        sram_size: usize,
        bank: u8,
        offset: u16,
    ) -> u8 {
        // LoROM: Mirror of 40-7F region
        let mirror_bank = bank.wrapping_sub(0x80); // C0->40 .. FF->7F
        if offset < 0x8000 {
            if sram_size == 0 || !(0x70..=0x7D).contains(&mirror_bank) {
                0xFF
            } else {
                let window = ((mirror_bank - 0x70) as usize) * 0x8000 + (offset as usize);
                let idx = window % sram_size;
                sram[idx]
            }
        } else {
            // ROM read using mirror bank
            let rom_addr = self.map_rom(mirror_bank, offset, rom_size);
            if rom_size == 0 {
                0xFF
            } else {
                rom[rom_addr]
            }
        }
    }

    fn write_bank_c0_ff(
        &self,
        sram: &mut [u8],
        sram_size: usize,
        bank: u8,
        offset: u16,
        value: u8,
    ) -> bool {
        // LoROM: banks $F0-$FD mirror SRAM banks $70-$7D in $0000-$7FFF
        if sram_size > 0 && offset < 0x8000 {
            let mirror_bank = bank.wrapping_sub(0x80);
            if (0x70..=0x7D).contains(&mirror_bank) {
                let window = ((mirror_bank - 0x70) as usize) * 0x8000 + (offset as usize);
                let idx = window % sram_size;
                sram[idx] = value;
                return true;
            }
        }
        false
    }

    fn is_rom_address(&self, bank: u8, offset: u16) -> bool {
        match bank {
            0x00..=0x3F | 0x80..=0xBF => offset >= 0x8000,
            0x40..=0x7D | 0xC0..=0xFF => offset >= 0x8000, // LoROM mirrors
            _ => false,
        }
    }
}
