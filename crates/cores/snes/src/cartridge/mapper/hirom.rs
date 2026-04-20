use super::MapperType;
use super::MemoryMapper;

pub struct HiRomMapper;

impl MemoryMapper for HiRomMapper {
    fn mapper_type(&self) -> MapperType {
        MapperType::HiRom
    }

    fn map_rom(&self, bank: u8, offset: u16, rom_size: usize) -> usize {
        // HiROM: 00-3F/80-BF high areas mirror by bank & 0x3F.
        let rom_bank = (bank & 0x3F) as usize;
        let rom_addr = rom_bank * 0x10000 + (offset as usize);
        if rom_addr < rom_size {
            rom_addr
        } else if rom_size > 0 {
            rom_addr % rom_size
        } else {
            0
        }
    }

    fn read_sram_region(&self, sram: &[u8], sram_size: usize, bank: u8, offset: u16) -> u8 {
        // SRAM in HiROM: only visible in banks $20-$3F/$A0-$BF at $6000-$7FFF
        if sram_size == 0 {
            0xFF
        } else {
            let bank_index = (bank & 0x3F) as usize;
            if bank_index < 0x20 {
                0xFF
            } else {
                let sram_addr = (bank_index - 0x20) * 0x2000 + ((offset - 0x6000) as usize);
                let idx = sram_addr % sram_size;
                sram[idx]
            }
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
        // SRAM in HiROM (banks $20-$3F/$A0-$BF only)
        if sram_size > 0 {
            let bank_index = (bank & 0x3F) as usize;
            if bank_index >= 0x20 {
                let sram_addr = (bank_index - 0x20) * 0x2000 + ((offset - 0x6000) as usize);
                let idx = sram_addr % sram_size;
                sram[idx] = value;
                return true;
            }
        }
        false
    }

    fn read_bank_40_7d(
        &self,
        rom: &[u8],
        _sram: &[u8],
        rom_size: usize,
        _sram_size: usize,
        bank: u8,
        offset: u16,
    ) -> u8 {
        // HiROM: Full 64KB banks mapped by bank & 0x3F
        let rom_bank = (bank & 0x3F) as usize;
        let phys = rom_bank * 0x10000 + (offset as usize);
        let phys = if rom_size > 0 { phys % rom_size } else { 0 };
        rom[phys]
    }

    fn write_bank_40_7d(
        &self,
        _sram: &mut [u8],
        _sram_size: usize,
        _bank: u8,
        _offset: u16,
        _value: u8,
    ) -> bool {
        // HiROM banks $40-$7D are ROM. Ignore writes.
        false
    }

    fn read_bank_c0_ff(
        &self,
        rom: &[u8],
        _sram: &[u8],
        rom_size: usize,
        _sram_size: usize,
        bank: u8,
        offset: u16,
    ) -> u8 {
        // HiROM: banks $C0-$FF map the same as $00-$3F (full 64KB ROM windows)
        let rom_bank = (bank & 0x3F) as usize;
        let phys = rom_bank * 0x10000 + (offset as usize);
        let phys = if rom_size > 0 { phys % rom_size } else { 0 };
        rom[phys]
    }

    fn write_bank_c0_ff(
        &self,
        sram: &mut [u8],
        _sram_size: usize,
        bank: u8,
        offset: u16,
        value: u8,
    ) -> bool {
        if (0x6000..0x8000).contains(&offset) {
            let sram_addr = ((bank - 0xC0) as usize) * 0x2000 + ((offset - 0x6000) as usize);
            if sram_addr < sram.len() {
                sram[sram_addr] = value;
                return true;
            }
        }
        false
    }

    fn is_rom_address(&self, bank: u8, offset: u16) -> bool {
        match bank {
            0x40..=0x7D | 0xC0..=0xFF => true,
            0x00..=0x3F | 0x80..=0xBF => offset >= 0x8000,
            _ => false,
        }
    }
}
