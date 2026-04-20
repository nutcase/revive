mod banks;
mod battery;
mod dispatch;
mod lifecycle;
mod load;
mod mapper;
mod mapper_state;
mod nametable;
mod state;

use mapper::{
    BandaiFcg, Fme7, IremG101, IremH3001, JalecoSs88006, Mapper15, Mapper246, Mapper40, Mapper42,
    Mapper43, Mapper50, Mmc1, Mmc2, Mmc3, Mmc5, Namco163, Namco210, Sunsoft3, Sunsoft4,
    TaitoTc0190, TaitoX1005, TaitoX1017, Vrc1, Vrc2Vrc4, Vrc3, Vrc6,
};
pub(in crate::cartridge) use mapper_state::{
    Mmc3VariantState, MulticartMapperState, SimpleMapperState,
};
use serde::{Deserialize, Serialize};
pub use state::*;

pub struct Cartridge {
    prg_rom: Vec<u8>,
    chr_rom: Vec<u8>,
    chr_ram: Vec<u8>, // CHR-RAM for MMC1 and other mappers
    prg_ram: Vec<u8>, // Battery-backed SRAM for save data
    has_valid_save_data: bool,
    mapper: u16,
    mirroring: Mirroring,
    has_battery: bool,
    chr_bank: u8,
    chr_bank_1: u8,
    prg_bank: u8,
    mappers: MapperRuntime,
}

struct MapperRuntime {
    simple: SimpleMapperState,
    multicart: MulticartMapperState,
    mmc3_variant: Mmc3VariantState,
    mmc1: Option<Mmc1>,
    mmc2: Option<Mmc2>,
    mmc3: Option<Mmc3>,
    mmc5: Option<Mmc5>,
    namco163: Option<Namco163>,
    namco210: Option<Namco210>,
    jaleco_ss88006: Option<JalecoSs88006>,
    vrc2_vrc4: Option<Vrc2Vrc4>,
    mapper40: Option<Mapper40>,
    mapper42: Option<Mapper42>,
    mapper43: Option<Mapper43>,
    mapper50: Option<Mapper50>,
    fme7: Option<Fme7>,
    bandai_fcg: Option<BandaiFcg>,
    irem_g101: Option<IremG101>,
    irem_h3001: Option<IremH3001>,
    vrc1: Option<Vrc1>,
    vrc3: Option<Vrc3>,
    vrc6: Option<Vrc6>,
    mapper15: Option<Mapper15>,
    sunsoft3: Option<Sunsoft3>,
    sunsoft4: Option<Sunsoft4>,
    taito_tc0190: Option<TaitoTc0190>,
    taito_x1005: Option<TaitoX1005>,
    taito_x1017: Option<TaitoX1017>,
    mapper246: Option<Mapper246>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Mirroring {
    Horizontal,
    HorizontalSwapped,
    ThreeScreenLower,
    Vertical,
    FourScreen,
    OneScreenLower,
    OneScreenUpper,
}

#[cfg(test)]
mod tests;
