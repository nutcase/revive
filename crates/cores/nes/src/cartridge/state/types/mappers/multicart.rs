use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mapper58State {
    pub nrom128: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mapper59State {
    pub latch: u16,
    pub locked: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mapper60State {
    pub game_select: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mapper61State {
    pub latch: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mapper63State {
    pub latch: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mapper137State {
    pub index: u8,
    pub registers: [u8; 8],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mapper142State {
    pub bank_select: u8,
    pub prg_banks: [u8; 4],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mapper150State {
    pub index: u8,
    pub registers: [u8; 8],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mapper225State {
    pub nrom128: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mapper232State {
    pub outer_bank: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mapper41State {
    pub inner_bank: u8,
}
