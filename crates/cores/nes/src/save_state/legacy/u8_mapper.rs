use super::super::types::SaveState;
use crate::apu::ApuState;
use crate::cartridge::*;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub(in crate::save_state) struct CartridgeStateU8Mapper {
    pub(in crate::save_state) mapper: u8,
    pub(in crate::save_state) mirroring: Mirroring,
    pub(in crate::save_state) prg_bank: u8,
    pub(in crate::save_state) chr_bank: u8,
    pub(in crate::save_state) prg_ram: Vec<u8>,
    pub(in crate::save_state) chr_ram: Vec<u8>,
    pub(in crate::save_state) has_valid_save_data: bool,
    pub(in crate::save_state) mmc1: Option<Mmc1State>,
    pub(in crate::save_state) mmc2: Option<Mmc2State>,
    #[serde(default)]
    pub(in crate::save_state) mmc3: Option<Mmc3State>,
    #[serde(default)]
    pub(in crate::save_state) mmc5: Option<Mmc5State>,
    #[serde(default)]
    pub(in crate::save_state) namco163: Option<Namco163State>,
    #[serde(default)]
    pub(in crate::save_state) fme7: Option<Fme7State>,
    #[serde(default)]
    pub(in crate::save_state) bandai_fcg: Option<BandaiFcgState>,
    #[serde(default)]
    pub(in crate::save_state) mapper34: Option<Mapper34State>,
    #[serde(default)]
    pub(in crate::save_state) mapper93: Option<Mapper93State>,
    #[serde(default)]
    pub(in crate::save_state) mapper184: Option<Mapper184State>,
    #[serde(default)]
    pub(in crate::save_state) vrc1: Option<Vrc1State>,
    #[serde(default)]
    pub(in crate::save_state) vrc2_vrc4: Option<Vrc2Vrc4State>,
    #[serde(default)]
    pub(in crate::save_state) mapper15: Option<Mapper15State>,
    #[serde(default)]
    pub(in crate::save_state) mapper72: Option<Mapper72State>,
    #[serde(default)]
    pub(in crate::save_state) mapper58: Option<Mapper58State>,
    #[serde(default)]
    pub(in crate::save_state) mapper59: Option<Mapper59State>,
    #[serde(default)]
    pub(in crate::save_state) mapper60: Option<Mapper60State>,
    #[serde(default)]
    pub(in crate::save_state) mapper225: Option<Mapper225State>,
    #[serde(default)]
    pub(in crate::save_state) mapper232: Option<Mapper232State>,
    #[serde(default)]
    pub(in crate::save_state) mapper234: Option<Mapper234State>,
    #[serde(default)]
    pub(in crate::save_state) mapper235: Option<Mapper235State>,
    #[serde(default)]
    pub(in crate::save_state) mapper202: Option<Mapper202State>,
    #[serde(default)]
    pub(in crate::save_state) mapper212: Option<Mapper212State>,
    #[serde(default)]
    pub(in crate::save_state) mapper226: Option<Mapper226State>,
    #[serde(default)]
    pub(in crate::save_state) mapper230: Option<Mapper230State>,
    #[serde(default)]
    pub(in crate::save_state) mapper228: Option<Mapper228State>,
    #[serde(default)]
    pub(in crate::save_state) mapper242: Option<Mapper242State>,
    #[serde(default)]
    pub(in crate::save_state) mapper243: Option<Mapper243State>,
    #[serde(default)]
    pub(in crate::save_state) mapper221: Option<Mapper221State>,
    #[serde(default)]
    pub(in crate::save_state) mapper191: Option<Mapper191State>,
    #[serde(default)]
    pub(in crate::save_state) mapper195: Option<Mapper195State>,
    #[serde(default)]
    pub(in crate::save_state) mapper208: Option<Mapper208State>,
    #[serde(default)]
    pub(in crate::save_state) mapper189: Option<Mapper189State>,
    #[serde(default)]
    pub(in crate::save_state) mapper236: Option<Mapper236State>,
    #[serde(default)]
    pub(in crate::save_state) mapper227: Option<Mapper227State>,
    #[serde(default)]
    pub(in crate::save_state) mapper246: Option<Mapper246State>,
    #[serde(default)]
    pub(in crate::save_state) sunsoft4: Option<Sunsoft4State>,
    #[serde(default)]
    pub(in crate::save_state) taito_tc0190: Option<TaitoTc0190State>,
    #[serde(default)]
    pub(in crate::save_state) taito_x1005: Option<TaitoX1005State>,
    #[serde(default)]
    pub(in crate::save_state) taito_x1017: Option<TaitoX1017State>,
    #[serde(default)]
    pub(in crate::save_state) mapper233: Option<Mapper233State>,
    #[serde(default)]
    pub(in crate::save_state) mapper41: Option<Mapper41State>,
    #[serde(default)]
    pub(in crate::save_state) mapper40: Option<Mapper40State>,
    #[serde(default)]
    pub(in crate::save_state) mapper42: Option<Mapper42State>,
    #[serde(default)]
    pub(in crate::save_state) mapper50: Option<Mapper50State>,
    #[serde(default)]
    pub(in crate::save_state) irem_g101: Option<IremG101State>,
    #[serde(default)]
    pub(in crate::save_state) vrc3: Option<Vrc3State>,
    #[serde(default)]
    pub(in crate::save_state) mapper43: Option<Mapper43State>,
    #[serde(default)]
    pub(in crate::save_state) irem_h3001: Option<IremH3001State>,
    #[serde(default)]
    pub(in crate::save_state) mapper103: Option<Mapper103State>,
    #[serde(default)]
    pub(in crate::save_state) mapper37: Option<Mapper37State>,
    #[serde(default)]
    pub(in crate::save_state) mapper44: Option<Mapper44State>,
    #[serde(default)]
    pub(in crate::save_state) mapper47: Option<Mapper47State>,
    #[serde(default)]
    pub(in crate::save_state) mapper12: Option<Mapper12State>,
    #[serde(default)]
    pub(in crate::save_state) mapper114: Option<Mapper114State>,
    #[serde(default)]
    pub(in crate::save_state) mapper123: Option<Mapper123State>,
    #[serde(default)]
    pub(in crate::save_state) mapper115: Option<Mapper115State>,
    #[serde(default)]
    pub(in crate::save_state) mapper205: Option<Mapper205State>,
    #[serde(default)]
    pub(in crate::save_state) mapper61: Option<Mapper61State>,
    #[serde(default)]
    pub(in crate::save_state) mapper185: Option<Mapper185State>,
    #[serde(default)]
    pub(in crate::save_state) sunsoft3: Option<Sunsoft3State>,
    #[serde(default)]
    pub(in crate::save_state) mapper63: Option<Mapper63State>,
    #[serde(default)]
    pub(in crate::save_state) mapper137: Option<Mapper137State>,
    #[serde(default)]
    pub(in crate::save_state) mapper142: Option<Mapper142State>,
    #[serde(default)]
    pub(in crate::save_state) mapper150: Option<Mapper150State>,
    #[serde(default)]
    pub(in crate::save_state) mapper18: Option<Mapper18State>,
    #[serde(default)]
    pub(in crate::save_state) mapper210: Option<Mapper210State>,
    #[serde(default)]
    pub(in crate::save_state) vrc6: Option<Vrc6State>,
    #[serde(default)]
    pub(in crate::save_state) vrc7: Option<Vrc7State>,
}

impl From<CartridgeStateU8Mapper> for CartridgeState {
    fn from(state: CartridgeStateU8Mapper) -> Self {
        Self {
            mapper: u16::from(state.mapper),
            mirroring: state.mirroring,
            prg_bank: state.prg_bank,
            chr_bank: state.chr_bank,
            prg_ram: state.prg_ram,
            chr_ram: state.chr_ram,
            has_valid_save_data: state.has_valid_save_data,
            mmc1: state.mmc1,
            mmc2: state.mmc2,
            mmc3: state.mmc3,
            mmc5: state.mmc5,
            namco163: state.namco163,
            fme7: state.fme7,
            bandai_fcg: state.bandai_fcg,
            mapper34: state.mapper34,
            mapper93: state.mapper93,
            mapper184: state.mapper184,
            vrc1: state.vrc1,
            vrc2_vrc4: state.vrc2_vrc4,
            mapper15: state.mapper15,
            mapper72: state.mapper72,
            mapper58: state.mapper58,
            mapper59: state.mapper59,
            mapper60: state.mapper60,
            mapper225: state.mapper225,
            mapper232: state.mapper232,
            mapper234: state.mapper234,
            mapper235: state.mapper235,
            mapper202: state.mapper202,
            mapper212: state.mapper212,
            mapper226: state.mapper226,
            mapper230: state.mapper230,
            mapper228: state.mapper228,
            mapper242: state.mapper242,
            mapper243: state.mapper243,
            mapper221: state.mapper221,
            mapper191: state.mapper191,
            mapper195: state.mapper195,
            mapper208: state.mapper208,
            mapper189: state.mapper189,
            mapper236: state.mapper236,
            mapper227: state.mapper227,
            mapper246: state.mapper246,
            sunsoft4: state.sunsoft4,
            taito_tc0190: state.taito_tc0190,
            taito_x1005: state.taito_x1005,
            taito_x1017: state.taito_x1017,
            mapper233: state.mapper233,
            mapper41: state.mapper41,
            mapper40: state.mapper40,
            mapper42: state.mapper42,
            mapper50: state.mapper50,
            irem_g101: state.irem_g101,
            vrc3: state.vrc3,
            mapper43: state.mapper43,
            irem_h3001: state.irem_h3001,
            mapper103: state.mapper103,
            mapper37: state.mapper37,
            mapper44: state.mapper44,
            mapper47: state.mapper47,
            mapper12: state.mapper12,
            mapper114: state.mapper114,
            mapper123: state.mapper123,
            mapper115: state.mapper115,
            mapper205: state.mapper205,
            mapper61: state.mapper61,
            mapper185: state.mapper185,
            sunsoft3: state.sunsoft3,
            mapper63: state.mapper63,
            mapper137: state.mapper137,
            mapper142: state.mapper142,
            mapper150: state.mapper150,
            mapper18: state.mapper18,
            mapper210: state.mapper210,
            vrc6: state.vrc6,
            vrc7: state.vrc7,
        }
    }
}

#[derive(Serialize, Deserialize)]
pub(in crate::save_state) struct SaveStateRawU8Mapper {
    pub(in crate::save_state) cpu_a: u8,
    pub(in crate::save_state) cpu_x: u8,
    pub(in crate::save_state) cpu_y: u8,
    pub(in crate::save_state) cpu_pc: u16,
    pub(in crate::save_state) cpu_sp: u8,
    pub(in crate::save_state) cpu_status: u8,
    pub(in crate::save_state) cpu_cycles: u64,
    pub(in crate::save_state) ppu_control: u8,
    pub(in crate::save_state) ppu_mask: u8,
    pub(in crate::save_state) ppu_status: u8,
    pub(in crate::save_state) ppu_oam_addr: u8,
    pub(in crate::save_state) ppu_scroll_x: u8,
    pub(in crate::save_state) ppu_scroll_y: u8,
    pub(in crate::save_state) ppu_addr: u16,
    pub(in crate::save_state) ppu_data_buffer: u8,
    pub(in crate::save_state) ppu_w: bool,
    pub(in crate::save_state) ppu_t: u16,
    pub(in crate::save_state) ppu_v: u16,
    pub(in crate::save_state) ppu_x: u8,
    pub(in crate::save_state) ppu_scanline: i16,
    pub(in crate::save_state) ppu_cycle: u16,
    pub(in crate::save_state) ppu_frame: u64,
    pub(in crate::save_state) ppu_palette: [u8; 32],
    pub(in crate::save_state) ppu_nametable: Vec<u8>,
    pub(in crate::save_state) ppu_oam: Vec<u8>,
    pub(in crate::save_state) ram: Vec<u8>,
    pub(in crate::save_state) cartridge_prg_bank: u8,
    pub(in crate::save_state) cartridge_chr_bank: u8,
    #[serde(default)]
    pub(in crate::save_state) cartridge_state: Option<CartridgeStateU8Mapper>,
    pub(in crate::save_state) apu_frame_counter: u8,
    pub(in crate::save_state) apu_frame_interrupt: bool,
    #[serde(default)]
    pub(in crate::save_state) apu_state: Option<ApuState>,
    pub(in crate::save_state) rom_filename: String,
    pub(in crate::save_state) timestamp: u64,
    #[serde(default)]
    pub(in crate::save_state) cpu_halted: bool,
    #[serde(default)]
    pub(in crate::save_state) bus_dma_cycles: u32,
    #[serde(default)]
    pub(in crate::save_state) bus_dma_in_progress: bool,
    #[serde(default)]
    pub(in crate::save_state) bus_dmc_stall_cycles: u32,
    #[serde(default)]
    pub(in crate::save_state) ppu_frame_complete: bool,
}

impl From<SaveStateRawU8Mapper> for SaveState {
    fn from(state: SaveStateRawU8Mapper) -> Self {
        Self {
            cpu_a: state.cpu_a,
            cpu_x: state.cpu_x,
            cpu_y: state.cpu_y,
            cpu_pc: state.cpu_pc,
            cpu_sp: state.cpu_sp,
            cpu_status: state.cpu_status,
            cpu_cycles: state.cpu_cycles,
            ppu_control: state.ppu_control,
            ppu_mask: state.ppu_mask,
            ppu_status: state.ppu_status,
            ppu_oam_addr: state.ppu_oam_addr,
            ppu_scroll_x: state.ppu_scroll_x,
            ppu_scroll_y: state.ppu_scroll_y,
            ppu_addr: state.ppu_addr,
            ppu_data_buffer: state.ppu_data_buffer,
            ppu_w: state.ppu_w,
            ppu_t: state.ppu_t,
            ppu_v: state.ppu_v,
            ppu_x: state.ppu_x,
            ppu_scanline: state.ppu_scanline,
            ppu_cycle: state.ppu_cycle,
            ppu_frame: state.ppu_frame,
            ppu_palette: state.ppu_palette,
            ppu_nametable: state.ppu_nametable,
            ppu_oam: state.ppu_oam,
            ram: state.ram,
            cartridge_prg_bank: state.cartridge_prg_bank,
            cartridge_chr_bank: state.cartridge_chr_bank,
            cartridge_state: state.cartridge_state.map(Into::into),
            apu_frame_counter: state.apu_frame_counter,
            apu_frame_interrupt: state.apu_frame_interrupt,
            apu_state: state.apu_state,
            rom_filename: state.rom_filename,
            timestamp: state.timestamp,
            cpu_halted: state.cpu_halted,
            bus_dma_cycles: state.bus_dma_cycles,
            bus_dma_in_progress: state.bus_dma_in_progress,
            bus_dmc_stall_cycles: state.bus_dmc_stall_cycles,
            ppu_frame_complete: state.ppu_frame_complete,
        }
    }
}
