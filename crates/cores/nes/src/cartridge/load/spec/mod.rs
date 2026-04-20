mod capabilities;
mod chr;
mod memory;

pub(super) use chr::ChrRomLoad;

#[derive(Debug, Clone, Copy, Default)]
pub(super) struct MapperMemorySpec {
    pub(super) prg_ram_size: usize,
    pub(super) prg_nvram_size: usize,
    pub(super) chr_ram_size: usize,
    pub(super) chr_nvram_size: usize,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct MapperSpec {
    mapper: u16,
    chr_rom_size: usize,
    has_battery: bool,
    mapper34_nina001: bool,
    memory: MapperMemorySpec,
}

impl MapperSpec {
    pub(super) fn new(
        mapper: u16,
        chr_rom_size: usize,
        has_battery: bool,
        mapper34_nina001: bool,
        memory: MapperMemorySpec,
    ) -> Self {
        Self {
            mapper,
            chr_rom_size,
            has_battery,
            mapper34_nina001,
            memory,
        }
    }

    pub(super) fn chr_rom_size(self) -> usize {
        self.chr_rom_size
    }

    pub(super) fn explicit_prg_ram_size(self) -> usize {
        self.memory.prg_ram_size + self.memory.prg_nvram_size
    }

    pub(super) fn explicit_chr_ram_size(self) -> usize {
        self.memory.chr_ram_size + self.memory.chr_nvram_size
    }
}
