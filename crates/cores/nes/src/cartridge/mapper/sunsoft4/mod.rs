mod chr;
mod nametable;
mod prg;
mod ram;

use super::super::Mirroring;

#[derive(Debug, Clone, Copy)]
pub struct Sunsoft4 {
    pub chr_banks: [u8; 4],
    pub nametable_banks: [u8; 2],
    pub control: u8,
    pub prg_bank: u8,
    pub prg_ram_enabled: bool,
    pub nametable_chr_rom: bool,
}

impl Sunsoft4 {
    pub fn new() -> Self {
        Self {
            chr_banks: [0; 4],
            nametable_banks: [0x80; 2],
            control: 0,
            prg_bank: 0,
            prg_ram_enabled: false,
            nametable_chr_rom: false,
        }
    }

    pub fn decode_mirroring(control: u8) -> Mirroring {
        match control & 0x03 {
            0 => Mirroring::Vertical,
            1 => Mirroring::Horizontal,
            2 => Mirroring::OneScreenLower,
            _ => Mirroring::OneScreenUpper,
        }
    }
}
