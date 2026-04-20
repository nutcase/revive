mod mappers;
mod mmc;

use super::super::Cartridge;
use super::types::*;

impl Cartridge {
    pub fn restore_state(&mut self, state: &CartridgeState) {
        if state.mapper != self.mapper {
            return;
        }

        self.mirroring = state.mirroring;
        self.set_prg_bank(state.prg_bank);
        self.set_chr_bank(state.chr_bank);
        if let Some(saved) = state.mapper34.as_ref() {
            self.mappers.simple.mapper34_nina001 = saved.nina001;
            self.chr_bank_1 = saved.chr_bank_1;
        }
        if let Some(saved) = state.mapper93.as_ref() {
            self.mappers.simple.mapper93_chr_ram_enabled = saved.chr_ram_enabled;
        }
        if let Some(saved) = state.mapper184.as_ref() {
            self.chr_bank_1 = saved.chr_bank_1;
        }
        if let Some(saved) = state.vrc1.as_ref() {
            self.chr_bank_1 = saved.chr_bank_1;
        }
        if let Some(saved) = state.vrc2_vrc4.as_ref() {
            self.prg_bank = saved.prg_banks[0];
            self.chr_bank = saved.chr_banks[0] as u8;
        }
        self.has_valid_save_data = state.has_valid_save_data;

        let prg_len = self.prg_ram.len().min(state.prg_ram.len());
        if prg_len > 0 {
            self.prg_ram[..prg_len].copy_from_slice(&state.prg_ram[..prg_len]);
        }

        let chr_len = self.chr_ram.len().min(state.chr_ram.len());
        if chr_len > 0 {
            self.chr_ram[..chr_len].copy_from_slice(&state.chr_ram[..chr_len]);
        }

        self.restore_mmc_states(state);

        self.restore_mapper_states(state);
    }
}
