use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mapper246State {
    pub prg_banks: [u8; 4],
    pub chr_banks: [u8; 4],
}
