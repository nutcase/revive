use super::spec::{ChrRomLoad, MapperSpec};
use std::io::{Error, ErrorKind, Result};

pub(super) fn load_prg_rom(
    data: &[u8],
    prg_rom_size: usize,
    prg_rom_start: usize,
) -> Result<Vec<u8>> {
    copy_rom_range(data, prg_rom_start, prg_rom_size)
}

pub(super) fn load_chr_rom(data: &[u8], spec: MapperSpec, chr_rom_start: usize) -> Result<Vec<u8>> {
    match spec.chr_rom_load() {
        ChrRomLoad::Cprom => load_cprom_chr_rom(data, spec.chr_rom_size(), chr_rom_start),
        ChrRomLoad::Namco163ChrRamBacked => Ok(vec![0; 0x2000]),
        ChrRomLoad::Empty => Ok(vec![]),
        ChrRomLoad::Mapper77 => {
            if spec.chr_rom_size() > 0 {
                copy_rom_range(data, chr_rom_start, spec.chr_rom_size())
            } else {
                Ok(vec![0; 0x0800])
            }
        }
        ChrRomLoad::Standard => {
            if spec.chr_rom_size() > 0 {
                copy_rom_range(data, chr_rom_start, spec.chr_rom_size())
            } else {
                Ok(vec![0; 8192])
            }
        }
    }
}

pub(super) fn allocate_prg_ram(spec: MapperSpec) -> Vec<u8> {
    if let Some((size, fill)) = spec.prg_ram_init() {
        vec![fill; size]
    } else {
        Vec::new()
    }
}

pub(super) fn allocate_chr_ram(spec: MapperSpec) -> Vec<u8> {
    let size = spec.chr_ram_size();
    if size > 0 {
        vec![0x00; size]
    } else {
        vec![]
    }
}

fn load_cprom_chr_rom(data: &[u8], chr_rom_size: usize, chr_rom_start: usize) -> Result<Vec<u8>> {
    if chr_rom_size > 0 {
        let mut chr = copy_rom_range(data, chr_rom_start, chr_rom_size)?;
        if chr.len() < 0x4000 {
            chr.resize(0x4000, 0);
        }
        Ok(chr)
    } else {
        Ok(vec![0; 0x4000])
    }
}

fn copy_rom_range(data: &[u8], start: usize, len: usize) -> Result<Vec<u8>> {
    let end = start.checked_add(len).ok_or_else(truncated_rom)?;
    data.get(start..end)
        .map(|slice| slice.to_vec())
        .ok_or_else(truncated_rom)
}

fn truncated_rom() -> Error {
    Error::new(ErrorKind::UnexpectedEof, "NES ROM is truncated")
}
