use crate::cartridge::Mirroring;
use serde::{Deserialize, Serialize};

use super::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CartridgeState {
    pub mapper: u16,
    pub mirroring: Mirroring,
    pub prg_bank: u8,
    pub chr_bank: u8,
    pub prg_ram: Vec<u8>,
    pub chr_ram: Vec<u8>,
    pub has_valid_save_data: bool,
    pub mmc1: Option<Mmc1State>,
    pub mmc2: Option<Mmc2State>,
    #[serde(default)]
    pub mmc3: Option<Mmc3State>,
    #[serde(default)]
    pub mmc5: Option<Mmc5State>,
    #[serde(default)]
    pub namco163: Option<Namco163State>,
    #[serde(default)]
    pub fme7: Option<Fme7State>,
    #[serde(default)]
    pub bandai_fcg: Option<BandaiFcgState>,
    #[serde(default)]
    pub mapper34: Option<Mapper34State>,
    #[serde(default)]
    pub mapper93: Option<Mapper93State>,
    #[serde(default)]
    pub mapper184: Option<Mapper184State>,
    #[serde(default)]
    pub vrc1: Option<Vrc1State>,
    #[serde(default)]
    pub vrc2_vrc4: Option<Vrc2Vrc4State>,
    #[serde(default)]
    pub mapper15: Option<Mapper15State>,
    #[serde(default)]
    pub mapper72: Option<Mapper72State>,
    #[serde(default)]
    pub mapper58: Option<Mapper58State>,
    #[serde(default)]
    pub mapper59: Option<Mapper59State>,
    #[serde(default)]
    pub mapper60: Option<Mapper60State>,
    #[serde(default)]
    pub mapper225: Option<Mapper225State>,
    #[serde(default)]
    pub mapper232: Option<Mapper232State>,
    #[serde(default)]
    pub mapper234: Option<Mapper234State>,
    #[serde(default)]
    pub mapper235: Option<Mapper235State>,
    #[serde(default)]
    pub mapper202: Option<Mapper202State>,
    #[serde(default)]
    pub mapper212: Option<Mapper212State>,
    #[serde(default)]
    pub mapper226: Option<Mapper226State>,
    #[serde(default)]
    pub mapper230: Option<Mapper230State>,
    #[serde(default)]
    pub mapper228: Option<Mapper228State>,
    #[serde(default)]
    pub mapper242: Option<Mapper242State>,
    #[serde(default)]
    pub mapper243: Option<Mapper243State>,
    #[serde(default)]
    pub mapper221: Option<Mapper221State>,
    #[serde(default)]
    pub mapper191: Option<Mapper191State>,
    #[serde(default)]
    pub mapper195: Option<Mapper195State>,
    #[serde(default)]
    pub mapper208: Option<Mapper208State>,
    #[serde(default)]
    pub mapper189: Option<Mapper189State>,
    #[serde(default)]
    pub mapper236: Option<Mapper236State>,
    #[serde(default)]
    pub mapper227: Option<Mapper227State>,
    #[serde(default)]
    pub mapper246: Option<Mapper246State>,
    #[serde(default)]
    pub sunsoft4: Option<Sunsoft4State>,
    #[serde(default)]
    pub taito_tc0190: Option<TaitoTc0190State>,
    #[serde(default)]
    pub taito_x1005: Option<TaitoX1005State>,
    #[serde(default)]
    pub taito_x1017: Option<TaitoX1017State>,
    #[serde(default)]
    pub mapper233: Option<Mapper233State>,
    #[serde(default)]
    pub mapper41: Option<Mapper41State>,
    #[serde(default)]
    pub mapper40: Option<Mapper40State>,
    #[serde(default)]
    pub mapper42: Option<Mapper42State>,
    #[serde(default)]
    pub mapper50: Option<Mapper50State>,
    #[serde(default)]
    pub irem_g101: Option<IremG101State>,
    #[serde(default)]
    pub vrc3: Option<Vrc3State>,
    #[serde(default)]
    pub mapper43: Option<Mapper43State>,
    #[serde(default)]
    pub irem_h3001: Option<IremH3001State>,
    #[serde(default)]
    pub mapper103: Option<Mapper103State>,
    #[serde(default)]
    pub mapper37: Option<Mapper37State>,
    #[serde(default)]
    pub mapper44: Option<Mapper44State>,
    #[serde(default)]
    pub mapper47: Option<Mapper47State>,
    #[serde(default)]
    pub mapper12: Option<Mapper12State>,
    #[serde(default)]
    pub mapper114: Option<Mapper114State>,
    #[serde(default)]
    pub mapper123: Option<Mapper123State>,
    #[serde(default)]
    pub mapper115: Option<Mapper115State>,
    #[serde(default)]
    pub mapper205: Option<Mapper205State>,
    #[serde(default)]
    pub mapper61: Option<Mapper61State>,
    #[serde(default)]
    pub mapper185: Option<Mapper185State>,
    #[serde(default)]
    pub sunsoft3: Option<Sunsoft3State>,
    #[serde(default)]
    pub mapper63: Option<Mapper63State>,
    #[serde(default)]
    pub mapper137: Option<Mapper137State>,
    #[serde(default)]
    pub mapper142: Option<Mapper142State>,
    #[serde(default)]
    pub mapper150: Option<Mapper150State>,
    #[serde(default)]
    pub mapper18: Option<Mapper18State>,
    #[serde(default)]
    pub mapper210: Option<Mapper210State>,
    #[serde(default)]
    pub vrc6: Option<Vrc6State>,
}
