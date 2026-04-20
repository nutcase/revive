use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mapper34State {
    pub nina001: bool,
    pub chr_bank_1: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mapper93State {
    pub chr_ram_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mapper184State {
    pub chr_bank_1: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mapper15State {
    pub mode: u8,
    pub data: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mapper72State {
    pub last_command: u8,
}
