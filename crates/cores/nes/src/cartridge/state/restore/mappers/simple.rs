use crate::cartridge::state::types::CartridgeState;
use crate::cartridge::{Cartridge, Mirroring};

impl Cartridge {
    pub(super) fn restore_simple_mapper_states(&mut self, state: &CartridgeState) {
        if let (Some(ref mut mapper15), Some(saved)) =
            (self.mappers.mapper15.as_mut(), state.mapper15.as_ref())
        {
            mapper15.mode = saved.mode;
            mapper15.data = saved.data;
        }
        if let Some(saved) = state.mapper72.as_ref() {
            self.chr_bank_1 = saved.last_command;
        }
        if let Some(saved) = state.mapper58.as_ref() {
            self.mappers.multicart.mapper58_nrom128 = saved.nrom128;
        }
        if let Some(saved) = state.mapper59.as_ref() {
            self.mappers.multicart.mapper59_latch = saved.latch;
            self.mappers.multicart.mapper59_locked = saved.locked;
            self.sync_mapper59_latch();
        }
        if let Some(saved) = state.mapper60.as_ref() {
            self.mappers.multicart.mapper60_game_select = saved.game_select;
            self.sync_mapper60_game();
        }
        if let Some(saved) = state.mapper61.as_ref() {
            self.mappers.multicart.mapper61_latch = saved.latch;
            self.sync_mapper61_latch();
        }
        if let Some(saved) = state.mapper63.as_ref() {
            self.mappers.multicart.mapper63_latch = saved.latch;
            self.prg_bank =
                (((saved.latch as usize) >> 2) % (self.prg_rom.len() / 0x4000).max(1)) as u8;
            self.mirroring = if saved.latch & 0x0001 != 0 {
                Mirroring::Horizontal
            } else {
                Mirroring::Vertical
            };
        }
        if let Some(saved) = state.mapper137.as_ref() {
            self.mappers.simple.mapper137_index = saved.index;
            self.mappers.simple.mapper137_registers = saved.registers;
            self.update_mapper137_state();
        }
        if let Some(saved) = state.mapper142.as_ref() {
            self.mappers.simple.mapper142_bank_select = saved.bank_select;
            self.mappers.simple.mapper142_prg_banks = saved.prg_banks;
            self.prg_bank = saved.prg_banks[0];
        }
        if let Some(saved) = state.mapper150.as_ref() {
            self.mappers.simple.mapper150_index = saved.index;
            self.mappers.simple.mapper150_registers = saved.registers;
            self.update_mapper150_state();
        }
    }
}
