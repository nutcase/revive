use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mapper233State {
    pub nrom128: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mapper234State {
    pub reg0: u8,
    pub reg1: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mapper235State {
    pub nrom128: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mapper202State {
    pub mode_32k: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mapper37State {
    pub outer_bank: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mapper44State {
    pub outer_bank: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mapper103State {
    pub prg_ram_disabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mapper12State {
    pub chr_outer: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mapper114State {
    pub nrom_override: u8,
    pub chr_outer_bank: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mapper115State {
    pub nrom_override: u8,
    pub chr_outer_bank: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mapper212State {
    pub mode_32k: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mapper47State {
    pub outer_bank: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mapper123State {
    pub nrom_override: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mapper205State {
    pub block: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mapper226State {
    pub nrom128: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mapper230State {
    pub contra_mode: bool,
    pub nrom128: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mapper228State {
    pub chip_select: u8,
    pub nrom128: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mapper242State {
    pub latch: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mapper243State {
    pub index: u8,
    pub registers: [u8; 8],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mapper221State {
    pub mode: u8,
    pub outer_bank: u8,
    pub chr_write_protect: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mapper191State {
    pub outer_bank: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mapper195State {
    pub mode: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mapper208State {
    pub protection_index: u8,
    pub protection_regs: [u8; 4],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mapper189State {
    pub prg_bank: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mapper185State {
    pub disabled_reads: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mapper236State {
    pub mode: u8,
    pub outer_bank: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mapper227State {
    pub latch: u16,
}
