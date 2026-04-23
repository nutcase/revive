use super::MapperType;
use super::MemoryMapper;

pub struct ExHiRomMapper;

const EXHIROM_SEGMENT_SIZE: usize = 0x400000;

#[inline]
fn map_first_segment(logical_addr: usize, rom_size: usize) -> usize {
    let segment_len = rom_size.min(EXHIROM_SEGMENT_SIZE);
    if segment_len == 0 {
        0
    } else {
        logical_addr % segment_len
    }
}

#[inline]
fn map_extended_segment(logical_addr: usize, rom_size: usize) -> usize {
    if rom_size <= EXHIROM_SEGMENT_SIZE {
        return 0;
    }

    let segment_len = rom_size - EXHIROM_SEGMENT_SIZE;
    EXHIROM_SEGMENT_SIZE + (logical_addr % segment_len)
}

impl MemoryMapper for ExHiRomMapper {
    fn mapper_type(&self) -> MapperType {
        MapperType::ExHiRom
    }

    fn map_rom(&self, bank: u8, offset: u16, rom_size: usize) -> usize {
        let offset = offset as usize;
        match bank {
            0x00..=0x3F => map_extended_segment((bank as usize) * 0x10000 + offset, rom_size),
            0x80..=0xBF => map_first_segment(((bank - 0x80) as usize) * 0x10000 + offset, rom_size),
            _ => 0,
        }
    }

    fn read_sram_region(&self, sram: &[u8], sram_size: usize, bank: u8, offset: u16) -> u8 {
        if sram_size == 0 {
            return 0xFF;
        }

        if !(0x80..=0xBF).contains(&bank) {
            return 0xFF;
        }

        let bank_index = (bank - 0x80) as usize;
        let sram_addr = bank_index * 0x2000 + ((offset - 0x6000) as usize);
        let idx = sram_addr % sram_size;
        sram[idx]
    }

    fn write_sram_region(
        &self,
        sram: &mut [u8],
        sram_size: usize,
        bank: u8,
        offset: u16,
        value: u8,
    ) -> bool {
        if sram_size == 0 || !(0x80..=0xBF).contains(&bank) {
            return false;
        }

        let bank_index = (bank - 0x80) as usize;
        let sram_addr = bank_index * 0x2000 + ((offset - 0x6000) as usize);
        let idx = sram_addr % sram_size;
        sram[idx] = value;
        true
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
        let rom_addr = map_extended_segment(
            ((bank - 0x40) as usize) * 0x10000 + offset as usize,
            rom_size,
        );
        if rom_size > 0 {
            rom[rom_addr % rom_size]
        } else {
            0xFF
        }
    }

    fn write_bank_40_7d(
        &self,
        _sram: &mut [u8],
        _sram_size: usize,
        _bank: u8,
        _offset: u16,
        _value: u8,
    ) -> bool {
        // ExHiROM banks $40-$7D are ROM. Ignore writes.
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
        let rom_addr = map_first_segment(
            ((bank - 0xC0) as usize) * 0x10000 + offset as usize,
            rom_size,
        );
        if rom_size > 0 {
            rom[rom_addr % rom_size]
        } else {
            0xFF
        }
    }

    fn write_bank_c0_ff(
        &self,
        _sram: &mut [u8],
        _sram_size: usize,
        _bank: u8,
        _offset: u16,
        _value: u8,
    ) -> bool {
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
