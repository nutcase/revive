use std::cell::Cell;

pub(in crate::cartridge) struct SimpleMapperState {
    pub(in crate::cartridge) mapper34_nina001: bool,
    pub(in crate::cartridge) mapper93_chr_ram_enabled: bool,
    pub(in crate::cartridge) mapper78_hv_mirroring: bool,
    pub(in crate::cartridge) mapper41_inner_bank: u8,
    pub(in crate::cartridge) mapper103_prg_ram_disabled: bool,
    pub(in crate::cartridge) mapper137_index: u8,
    pub(in crate::cartridge) mapper137_registers: [u8; 8],
    pub(in crate::cartridge) mapper142_bank_select: u8,
    pub(in crate::cartridge) mapper142_prg_banks: [u8; 4],
    pub(in crate::cartridge) mapper150_index: u8,
    pub(in crate::cartridge) mapper150_registers: [u8; 8],
    pub(in crate::cartridge) mapper185_disabled_reads: Cell<u8>,
}

impl SimpleMapperState {
    pub(in crate::cartridge) fn new(
        mapper: u16,
        mapper34_nina001: bool,
        mapper93_chr_ram_enabled: bool,
        mapper78_hv_mirroring: bool,
    ) -> Self {
        Self {
            mapper34_nina001,
            mapper93_chr_ram_enabled,
            mapper78_hv_mirroring,
            mapper41_inner_bank: 0,
            mapper103_prg_ram_disabled: false,
            mapper137_index: 0,
            mapper137_registers: [0; 8],
            mapper142_bank_select: 0,
            mapper142_prg_banks: [0; 4],
            mapper150_index: 0,
            mapper150_registers: [0; 8],
            mapper185_disabled_reads: Cell::new(if mapper == 185 { 2 } else { 0 }),
        }
    }
}

pub(in crate::cartridge) struct MulticartMapperState {
    pub(in crate::cartridge) mapper58_nrom128: bool,
    pub(in crate::cartridge) mapper59_latch: u16,
    pub(in crate::cartridge) mapper59_locked: bool,
    pub(in crate::cartridge) mapper60_game_select: u8,
    pub(in crate::cartridge) mapper61_latch: u16,
    pub(in crate::cartridge) mapper63_latch: u16,
    pub(in crate::cartridge) mapper225_nrom128: bool,
    pub(in crate::cartridge) mapper232_outer_bank: u8,
    pub(in crate::cartridge) mapper233_nrom128: bool,
    pub(in crate::cartridge) mapper234_reg0: u8,
    pub(in crate::cartridge) mapper234_reg1: u8,
    pub(in crate::cartridge) mapper235_nrom128: bool,
    pub(in crate::cartridge) mapper202_32k_mode: bool,
    pub(in crate::cartridge) mapper212_32k_mode: bool,
    pub(in crate::cartridge) mapper226_nrom128: bool,
    pub(in crate::cartridge) mapper230_contra_mode: bool,
    pub(in crate::cartridge) mapper230_nrom128: bool,
    pub(in crate::cartridge) mapper228_chip_select: u8,
    pub(in crate::cartridge) mapper228_nrom128: bool,
    pub(in crate::cartridge) mapper242_latch: u16,
    pub(in crate::cartridge) mapper243_index: u8,
    pub(in crate::cartridge) mapper243_registers: [u8; 8],
    pub(in crate::cartridge) mapper221_mode: u8,
    pub(in crate::cartridge) mapper221_outer_bank: u8,
    pub(in crate::cartridge) mapper221_chr_write_protect: bool,
    pub(in crate::cartridge) mapper227_latch: u16,
    pub(in crate::cartridge) mapper236_mode: u8,
    pub(in crate::cartridge) mapper236_outer_bank: u8,
    pub(in crate::cartridge) mapper236_chr_ram: bool,
}

impl MulticartMapperState {
    pub(in crate::cartridge) fn new(mapper: u16, mapper236_chr_ram: bool) -> Self {
        Self {
            mapper58_nrom128: false,
            mapper59_latch: 0,
            mapper59_locked: false,
            mapper60_game_select: 0,
            mapper61_latch: 0,
            mapper63_latch: 0,
            mapper225_nrom128: false,
            mapper232_outer_bank: 0,
            mapper233_nrom128: false,
            mapper234_reg0: 0,
            mapper234_reg1: 0,
            mapper235_nrom128: false,
            mapper202_32k_mode: false,
            mapper212_32k_mode: false,
            mapper226_nrom128: false,
            mapper230_contra_mode: mapper == 230,
            mapper230_nrom128: false,
            mapper228_chip_select: 0,
            mapper228_nrom128: false,
            mapper242_latch: 0,
            mapper243_index: 0,
            mapper243_registers: [0; 8],
            mapper221_mode: 0,
            mapper221_outer_bank: 0,
            mapper221_chr_write_protect: false,
            mapper227_latch: 0,
            mapper236_mode: 0,
            mapper236_outer_bank: 0,
            mapper236_chr_ram,
        }
    }
}

pub(in crate::cartridge) struct Mmc3VariantState {
    pub(in crate::cartridge) mapper37_outer_bank: u8,
    pub(in crate::cartridge) mapper44_outer_bank: u8,
    pub(in crate::cartridge) mapper47_outer_bank: u8,
    pub(in crate::cartridge) mapper12_chr_outer: u8,
    pub(in crate::cartridge) mapper114_override: u8,
    pub(in crate::cartridge) mapper114_chr_outer_bank: u8,
    pub(in crate::cartridge) mapper115_override: u8,
    pub(in crate::cartridge) mapper115_chr_outer_bank: u8,
    pub(in crate::cartridge) mapper123_override: u8,
    pub(in crate::cartridge) mapper205_block: u8,
    pub(in crate::cartridge) mapper191_outer_bank: u8,
    pub(in crate::cartridge) mapper195_mode: u8,
    pub(in crate::cartridge) mapper208_protection_index: u8,
    pub(in crate::cartridge) mapper208_protection_regs: [u8; 4],
    pub(in crate::cartridge) mapper189_prg_bank: u8,
}

impl Mmc3VariantState {
    pub(in crate::cartridge) fn new(mapper: u16, chr_rom_size: usize) -> Self {
        Self {
            mapper37_outer_bank: 0,
            mapper44_outer_bank: 0,
            mapper47_outer_bank: 0,
            mapper12_chr_outer: 0,
            mapper114_override: 0,
            mapper114_chr_outer_bank: 0,
            mapper115_override: 0,
            mapper115_chr_outer_bank: 0,
            mapper123_override: 0,
            mapper205_block: 0,
            mapper191_outer_bank: if mapper == 191 && chr_rom_size <= 0x20000 {
                3
            } else {
                0
            },
            mapper195_mode: 0x80,
            mapper208_protection_index: 0,
            mapper208_protection_regs: [0; 4],
            mapper189_prg_bank: 0,
        }
    }
}
