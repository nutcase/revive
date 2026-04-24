mod chips;
mod expansion;
mod irq;
mod multicart;
mod simple;
mod variants;

use super::super::super::Cartridge;
use super::super::types::*;

impl Cartridge {
    pub(super) fn build_cartridge_state(
        &self,
        mmc1: Option<Mmc1State>,
        mmc2: Option<Mmc2State>,
        mmc3: Option<Mmc3State>,
        mmc5: Option<Mmc5State>,
    ) -> CartridgeState {
        let chips_states = self.snapshot_chip_states();
        let simple_states = self.snapshot_simple_mapper_states();
        let multicart_states = self.snapshot_multicart_mapper_states();
        let irq_states = self.snapshot_irq_mapper_states();
        let variants_states = self.snapshot_variant_mapper_states();
        let expansion_states = self.snapshot_expansion_mapper_states();

        CartridgeState {
            mapper: self.mapper,
            mirroring: self.mirroring,
            prg_bank: self.get_prg_bank(),
            chr_bank: self.get_chr_bank(),
            prg_ram: self.prg_ram.clone(),
            chr_ram: self.chr_ram.clone(),
            has_valid_save_data: self.has_valid_save_data,
            mmc1,
            mmc2,
            mmc3,
            mmc5,
            namco163: chips_states.namco163,
            fme7: chips_states.fme7,
            bandai_fcg: chips_states.bandai_fcg,
            mapper34: simple_states.mapper34,
            mapper93: simple_states.mapper93,
            mapper184: simple_states.mapper184,
            vrc1: simple_states.vrc1,
            vrc2_vrc4: simple_states.vrc2_vrc4,
            mapper15: simple_states.mapper15,
            mapper72: simple_states.mapper72,
            mapper58: multicart_states.mapper58,
            mapper59: multicart_states.mapper59,
            mapper60: multicart_states.mapper60,
            mapper225: multicart_states.mapper225,
            mapper232: multicart_states.mapper232,
            mapper234: variants_states.mapper234,
            mapper235: variants_states.mapper235,
            mapper202: variants_states.mapper202,
            mapper212: variants_states.mapper212,
            mapper226: variants_states.mapper226,
            mapper230: variants_states.mapper230,
            mapper228: variants_states.mapper228,
            mapper242: variants_states.mapper242,
            mapper243: variants_states.mapper243,
            mapper221: variants_states.mapper221,
            mapper191: variants_states.mapper191,
            mapper195: variants_states.mapper195,
            mapper208: variants_states.mapper208,
            mapper189: variants_states.mapper189,
            mapper236: variants_states.mapper236,
            mapper227: variants_states.mapper227,
            mapper246: expansion_states.mapper246,
            sunsoft4: expansion_states.sunsoft4,
            taito_tc0190: expansion_states.taito_tc0190,
            taito_x1005: expansion_states.taito_x1005,
            taito_x1017: expansion_states.taito_x1017,
            mapper233: variants_states.mapper233,
            mapper41: multicart_states.mapper41,
            mapper40: irq_states.mapper40,
            mapper42: irq_states.mapper42,
            mapper50: irq_states.mapper50,
            irem_g101: irq_states.irem_g101,
            vrc3: irq_states.vrc3,
            mapper43: irq_states.mapper43,
            irem_h3001: irq_states.irem_h3001,
            mapper103: variants_states.mapper103,
            mapper37: variants_states.mapper37,
            mapper44: variants_states.mapper44,
            mapper47: variants_states.mapper47,
            mapper12: variants_states.mapper12,
            mapper114: variants_states.mapper114,
            mapper123: variants_states.mapper123,
            mapper115: variants_states.mapper115,
            mapper205: variants_states.mapper205,
            mapper61: multicart_states.mapper61,
            mapper185: variants_states.mapper185,
            sunsoft3: expansion_states.sunsoft3,
            mapper63: multicart_states.mapper63,
            mapper137: multicart_states.mapper137,
            mapper142: multicart_states.mapper142,
            mapper150: multicart_states.mapper150,
            mapper18: chips_states.mapper18,
            mapper210: chips_states.mapper210,
            vrc6: expansion_states.vrc6,
            vrc7: expansion_states.vrc7,
        }
    }
}
