mod exhirom;
mod hirom;
mod lorom;

pub use exhirom::ExHiRomMapper;
pub use hirom::HiRomMapper;
pub use lorom::LoRomMapper;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MapperType {
    LoRom,
    HiRom,
    ExHiRom,
    SuperFx,
    Sa1,
    DragonQuest3,
    Spc7110,
    Sdd1,
    Dsp1,
    Dsp1HiRom,
    Dsp3,
}

/// Trait for standard memory mappers (LoROM/HiROM/ExHiROM).
/// SA-1/DQ3 are handled as special cases in Bus due to deep coprocessor coupling.
pub trait MemoryMapper {
    fn mapper_type(&self) -> MapperType;
    fn map_rom(&self, bank: u8, offset: u16, rom_size: usize) -> usize;
    fn read_sram_region(&self, sram: &[u8], sram_size: usize, bank: u8, offset: u16) -> u8;
    fn write_sram_region(
        &self,
        sram: &mut [u8],
        sram_size: usize,
        bank: u8,
        offset: u16,
        value: u8,
    ) -> bool;
    fn read_bank_40_7d(
        &self,
        rom: &[u8],
        sram: &[u8],
        rom_size: usize,
        sram_size: usize,
        bank: u8,
        offset: u16,
    ) -> u8;
    fn write_bank_40_7d(
        &self,
        sram: &mut [u8],
        sram_size: usize,
        bank: u8,
        offset: u16,
        value: u8,
    ) -> bool;
    fn read_bank_c0_ff(
        &self,
        rom: &[u8],
        sram: &[u8],
        rom_size: usize,
        sram_size: usize,
        bank: u8,
        offset: u16,
    ) -> u8;
    fn write_bank_c0_ff(
        &self,
        sram: &mut [u8],
        sram_size: usize,
        bank: u8,
        offset: u16,
        value: u8,
    ) -> bool;
    fn is_rom_address(&self, bank: u8, offset: u16) -> bool;
}

/// Enum wrapper for static dispatch on hot paths (read_u8/write_u8).
#[allow(clippy::enum_variant_names)]
pub enum MapperImpl {
    LoRom(LoRomMapper),
    HiRom(HiRomMapper),
    ExHiRom(ExHiRomMapper),
}

macro_rules! dispatch_mapper {
    ($self:expr, $method:ident $(, $arg:expr)*) => {
        match $self {
            MapperImpl::LoRom(m) => m.$method($($arg),*),
            MapperImpl::HiRom(m) => m.$method($($arg),*),
            MapperImpl::ExHiRom(m) => m.$method($($arg),*),
        }
    };
}

impl MapperImpl {
    /// Create a MapperImpl from a MapperType. Returns None for SA-1/DQ3/SuperFx
    /// which are handled as special cases.
    pub fn from_type(mapper_type: MapperType) -> Option<Self> {
        match mapper_type {
            MapperType::LoRom => Some(MapperImpl::LoRom(LoRomMapper)),
            MapperType::HiRom => Some(MapperImpl::HiRom(HiRomMapper)),
            MapperType::ExHiRom => Some(MapperImpl::ExHiRom(ExHiRomMapper)),
            MapperType::Sa1
            | MapperType::DragonQuest3
            | MapperType::SuperFx
            | MapperType::Spc7110 => None,
            // S-DD1 uses standard LoROM for most banks; only $C0-$FF is overridden.
            MapperType::Sdd1 => Some(MapperImpl::LoRom(LoRomMapper)),
            // DSP-1 uses standard LoROM; $6000-$7FFF in low banks routed to DSP-1 in bus.
            MapperType::Dsp1 => Some(MapperImpl::LoRom(LoRomMapper)),
            // DSP-1 HiROM; $6000-$7FFF in banks $00-$1F/$80-$9F routed to DSP-1 in bus.
            MapperType::Dsp1HiRom => Some(MapperImpl::HiRom(HiRomMapper)),
            MapperType::Dsp3 => Some(MapperImpl::LoRom(LoRomMapper)),
        }
    }
}

impl MemoryMapper for MapperImpl {
    fn mapper_type(&self) -> MapperType {
        dispatch_mapper!(self, mapper_type)
    }

    fn map_rom(&self, bank: u8, offset: u16, rom_size: usize) -> usize {
        dispatch_mapper!(self, map_rom, bank, offset, rom_size)
    }

    fn read_sram_region(&self, sram: &[u8], sram_size: usize, bank: u8, offset: u16) -> u8 {
        dispatch_mapper!(self, read_sram_region, sram, sram_size, bank, offset)
    }

    fn write_sram_region(
        &self,
        sram: &mut [u8],
        sram_size: usize,
        bank: u8,
        offset: u16,
        value: u8,
    ) -> bool {
        dispatch_mapper!(
            self,
            write_sram_region,
            sram,
            sram_size,
            bank,
            offset,
            value
        )
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
        dispatch_mapper!(
            self,
            read_bank_40_7d,
            rom,
            sram,
            rom_size,
            sram_size,
            bank,
            offset
        )
    }

    fn write_bank_40_7d(
        &self,
        sram: &mut [u8],
        sram_size: usize,
        bank: u8,
        offset: u16,
        value: u8,
    ) -> bool {
        dispatch_mapper!(self, write_bank_40_7d, sram, sram_size, bank, offset, value)
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
        dispatch_mapper!(
            self,
            read_bank_c0_ff,
            rom,
            sram,
            rom_size,
            sram_size,
            bank,
            offset
        )
    }

    fn write_bank_c0_ff(
        &self,
        sram: &mut [u8],
        sram_size: usize,
        bank: u8,
        offset: u16,
        value: u8,
    ) -> bool {
        dispatch_mapper!(self, write_bank_c0_ff, sram, sram_size, bank, offset, value)
    }

    fn is_rom_address(&self, bank: u8, offset: u16) -> bool {
        dispatch_mapper!(self, is_rom_address, bank, offset)
    }
}

/// LoROM address mapping: 32KB banks in upper half.
pub fn map_lorom(addr: u32) -> usize {
    let bank = (addr >> 16) & 0xFF;
    let offset = addr & 0xFFFF;

    match bank {
        0x00..=0x7D | 0x80..=0xFF => {
            if offset >= 0x8000 {
                ((bank & 0x7F) as usize) * 0x8000 + (offset as usize) - 0x8000
            } else {
                0
            }
        }
        _ => 0,
    }
}

/// HiROM address mapping: 64KB linear banks.
pub fn map_hirom(addr: u32) -> usize {
    let bank = (addr >> 16) & 0xFF;
    let offset = addr & 0xFFFF;

    match bank {
        0x00..=0x3F => {
            if offset >= 0x8000 {
                (bank as usize) * 0x10000 + (offset as usize)
            } else {
                0
            }
        }
        0x40..=0x7D => (bank as usize) * 0x10000 + (offset as usize),
        0x80..=0xBF => {
            if offset >= 0x8000 {
                ((bank - 0x80) as usize) * 0x10000 + (offset as usize)
            } else {
                0
            }
        }
        0xC0..=0xFF => ((bank - 0xC0) as usize) * 0x10000 + (offset as usize),
        _ => 0,
    }
}

/// ExHiROM address mapping: extended 64KB linear banks with 4MB offset.
pub fn map_exhirom(addr: u32) -> usize {
    let bank = (addr >> 16) & 0xFF;
    let offset = addr & 0xFFFF;

    match bank {
        0x00..=0x3F => {
            if offset >= 0x8000 {
                0x400000 + (bank as usize) * 0x10000 + (offset as usize)
            } else {
                0
            }
        }
        0x40..=0x7D => (bank as usize) * 0x10000 + (offset as usize),
        0x80..=0xBF => {
            if offset >= 0x8000 {
                0x400000 + ((bank - 0x80) as usize) * 0x10000 + (offset as usize)
            } else {
                0
            }
        }
        0xC0..=0xFF => ((bank - 0xC0) as usize) * 0x10000 + (offset as usize),
        _ => 0,
    }
}
