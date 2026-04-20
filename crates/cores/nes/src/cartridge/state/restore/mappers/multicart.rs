use crate::cartridge::state::types::CartridgeState;
use crate::cartridge::Cartridge;

impl Cartridge {
    pub(super) fn restore_multicart_mapper_states(&mut self, state: &CartridgeState) {
        if let Some(saved) = state.mapper225.as_ref() {
            self.mappers.multicart.mapper225_nrom128 = saved.nrom128;
        }
        if let Some(saved) = state.mapper232.as_ref() {
            self.mappers.multicart.mapper232_outer_bank = saved.outer_bank;
        }
        if let Some(saved) = state.mapper41.as_ref() {
            self.mappers.simple.mapper41_inner_bank = saved.inner_bank;
            self.sync_mapper41_chr_bank();
        }
        if let Some(saved) = state.mapper233.as_ref() {
            self.mappers.multicart.mapper233_nrom128 = saved.nrom128;
        }
        if let Some(saved) = state.mapper234.as_ref() {
            self.mappers.multicart.mapper234_reg0 = saved.reg0;
            self.mappers.multicart.mapper234_reg1 = saved.reg1;
            self.sync_mapper234_state();
        }
        if let Some(saved) = state.mapper235.as_ref() {
            self.mappers.multicart.mapper235_nrom128 = saved.nrom128;
        }
        if let Some(saved) = state.mapper202.as_ref() {
            self.mappers.multicart.mapper202_32k_mode = saved.mode_32k;
        }
        if let Some(saved) = state.mapper37.as_ref() {
            self.mappers.mmc3_variant.mapper37_outer_bank = saved.outer_bank;
        }
        if let Some(saved) = state.mapper44.as_ref() {
            self.mappers.mmc3_variant.mapper44_outer_bank = saved.outer_bank;
        }
        if let Some(saved) = state.mapper103.as_ref() {
            self.mappers.simple.mapper103_prg_ram_disabled = saved.prg_ram_disabled;
        }
        if let Some(saved) = state.mapper12.as_ref() {
            self.mappers.mmc3_variant.mapper12_chr_outer = saved.chr_outer;
        }
        if let Some(saved) = state.mapper114.as_ref() {
            self.mappers.mmc3_variant.mapper114_override = saved.nrom_override;
            self.mappers.mmc3_variant.mapper114_chr_outer_bank = saved.chr_outer_bank;
        }
        if let Some(saved) = state.mapper212.as_ref() {
            self.mappers.multicart.mapper212_32k_mode = saved.mode_32k;
        }
        if let Some(saved) = state.mapper47.as_ref() {
            self.mappers.mmc3_variant.mapper47_outer_bank = saved.outer_bank;
        }
        if let Some(saved) = state.mapper123.as_ref() {
            self.mappers.mmc3_variant.mapper123_override = saved.nrom_override;
        }
        if let Some(saved) = state.mapper115.as_ref() {
            self.mappers.mmc3_variant.mapper115_override = saved.nrom_override;
            self.mappers.mmc3_variant.mapper115_chr_outer_bank = saved.chr_outer_bank;
        }
        if let Some(saved) = state.mapper205.as_ref() {
            self.mappers.mmc3_variant.mapper205_block = saved.block;
        }
        if let Some(saved) = state.mapper226.as_ref() {
            self.mappers.multicart.mapper226_nrom128 = saved.nrom128;
        }
        if let Some(saved) = state.mapper230.as_ref() {
            self.mappers.multicart.mapper230_contra_mode = saved.contra_mode;
            self.mappers.multicart.mapper230_nrom128 = saved.nrom128;
        }
        if let Some(saved) = state.mapper228.as_ref() {
            self.mappers.multicart.mapper228_chip_select = saved.chip_select;
            self.mappers.multicart.mapper228_nrom128 = saved.nrom128;
        }
        if let Some(saved) = state.mapper242.as_ref() {
            self.mappers.multicart.mapper242_latch = saved.latch;
        }
        if let Some(saved) = state.mapper243.as_ref() {
            self.mappers.multicart.mapper243_index = saved.index;
            self.mappers.multicart.mapper243_registers = saved.registers;
        }
        if let Some(saved) = state.mapper221.as_ref() {
            self.mappers.multicart.mapper221_mode = saved.mode;
            self.mappers.multicart.mapper221_outer_bank = saved.outer_bank;
            self.mappers.multicart.mapper221_chr_write_protect = saved.chr_write_protect;
        }
        if let Some(saved) = state.mapper191.as_ref() {
            self.mappers.mmc3_variant.mapper191_outer_bank = saved.outer_bank;
        }
        if let Some(saved) = state.mapper195.as_ref() {
            self.mappers.mmc3_variant.mapper195_mode = saved.mode;
        }
        if let Some(saved) = state.mapper208.as_ref() {
            self.mappers.mmc3_variant.mapper208_protection_index = saved.protection_index;
            self.mappers.mmc3_variant.mapper208_protection_regs = saved.protection_regs;
        }
        if let Some(saved) = state.mapper189.as_ref() {
            self.mappers.mmc3_variant.mapper189_prg_bank = saved.prg_bank;
            self.prg_bank = saved.prg_bank;
        }
        if let Some(saved) = state.mapper185.as_ref() {
            self.mappers
                .simple
                .mapper185_disabled_reads
                .set(saved.disabled_reads);
        }
        if let Some(saved) = state.mapper236.as_ref() {
            self.mappers.multicart.mapper236_mode = saved.mode;
            self.mappers.multicart.mapper236_outer_bank = saved.outer_bank;
        }
        if let Some(saved) = state.mapper227.as_ref() {
            self.mappers.multicart.mapper227_latch = saved.latch;
        }
    }
}
