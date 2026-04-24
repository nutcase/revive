use std::path::Path;

use emulator_gb::GbModel;

use super::gameboy::{GameBoyAdapter, GameBoyAdvanceAdapter};
use super::megadrive::MegaDriveAdapter;
use super::nes::NesAdapter;
use super::pce::PceAdapter;
use super::sega8::{MasterSystemAdapter, Sg1000Adapter};
use super::snes::SnesAdapter;
use crate::system::{
    detect_system, AudioSpec, FrameView, MemoryRegion, Result, SystemKind, VirtualButton,
};

pub enum CoreInstance {
    Nes(NesAdapter),
    Snes(Box<SnesAdapter>),
    Sg1000(Sg1000Adapter),
    MasterSystem(MasterSystemAdapter),
    MegaDrive(MegaDriveAdapter),
    Pce(Box<PceAdapter>),
    GameBoy(GameBoyAdapter),
    GameBoyAdvance(Box<GameBoyAdvanceAdapter>),
}

impl CoreInstance {
    pub fn load_rom(path: &Path, system: Option<SystemKind>) -> Result<Self> {
        let system = match system {
            Some(system) => system,
            None => detect_system(path)?,
        };

        match system {
            SystemKind::Nes => NesAdapter::load(path).map(Self::Nes),
            SystemKind::Snes => {
                SnesAdapter::load(path).map(|adapter| Self::Snes(Box::new(adapter)))
            }
            SystemKind::Sg1000 => Sg1000Adapter::load(path).map(Self::Sg1000),
            SystemKind::MasterSystem => MasterSystemAdapter::load(path).map(Self::MasterSystem),
            SystemKind::MegaDrive => MegaDriveAdapter::load(path).map(Self::MegaDrive),
            SystemKind::Pce => PceAdapter::load(path).map(|adapter| Self::Pce(Box::new(adapter))),
            SystemKind::GameBoy => {
                GameBoyAdapter::load(path, GbModel::Dmg, SystemKind::GameBoy).map(Self::GameBoy)
            }
            SystemKind::GameBoyColor => {
                GameBoyAdapter::load(path, GbModel::Cgb, SystemKind::GameBoyColor)
                    .map(Self::GameBoy)
            }
            SystemKind::GameBoyAdvance => GameBoyAdvanceAdapter::load(path)
                .map(|adapter| Self::GameBoyAdvance(Box::new(adapter))),
        }
    }

    pub fn system(&self) -> SystemKind {
        match self {
            Self::Nes(_) => SystemKind::Nes,
            Self::Snes(_) => SystemKind::Snes,
            Self::Sg1000(_) => SystemKind::Sg1000,
            Self::MasterSystem(_) => SystemKind::MasterSystem,
            Self::MegaDrive(_) => SystemKind::MegaDrive,
            Self::Pce(_) => SystemKind::Pce,
            Self::GameBoy(adapter) => adapter.system(),
            Self::GameBoyAdvance(_) => SystemKind::GameBoyAdvance,
        }
    }

    pub fn title(&self) -> &str {
        match self {
            Self::Nes(adapter) => adapter.title(),
            Self::Snes(adapter) => adapter.title(),
            Self::Sg1000(adapter) => adapter.title(),
            Self::MasterSystem(adapter) => adapter.title(),
            Self::MegaDrive(adapter) => adapter.title(),
            Self::Pce(adapter) => adapter.title(),
            Self::GameBoy(adapter) => adapter.title(),
            Self::GameBoyAdvance(adapter) => adapter.title(),
        }
    }

    pub fn step_frame(&mut self) -> Result<()> {
        match self {
            Self::Nes(adapter) => adapter.step_frame(),
            Self::Snes(adapter) => adapter.step_frame(),
            Self::Sg1000(adapter) => adapter.step_frame(),
            Self::MasterSystem(adapter) => adapter.step_frame(),
            Self::MegaDrive(adapter) => adapter.step_frame(),
            Self::Pce(adapter) => adapter.step_frame(),
            Self::GameBoy(adapter) => adapter.step_frame(),
            Self::GameBoyAdvance(adapter) => adapter.step_frame(),
        }
    }

    pub fn frame(&mut self) -> FrameView<'_> {
        match self {
            Self::Nes(adapter) => adapter.frame(),
            Self::Snes(adapter) => adapter.frame(),
            Self::Sg1000(adapter) => adapter.frame(),
            Self::MasterSystem(adapter) => adapter.frame(),
            Self::MegaDrive(adapter) => adapter.frame(),
            Self::Pce(adapter) => adapter.frame(),
            Self::GameBoy(adapter) => adapter.frame(),
            Self::GameBoyAdvance(adapter) => adapter.frame(),
        }
    }

    pub fn audio_spec(&self) -> AudioSpec {
        match self {
            Self::Nes(adapter) => adapter.audio_spec(),
            Self::Snes(adapter) => adapter.audio_spec(),
            Self::Sg1000(adapter) => adapter.audio_spec(),
            Self::MasterSystem(adapter) => adapter.audio_spec(),
            Self::MegaDrive(adapter) => adapter.audio_spec(),
            Self::Pce(adapter) => adapter.audio_spec(),
            Self::GameBoy(adapter) => adapter.audio_spec(),
            Self::GameBoyAdvance(adapter) => adapter.audio_spec(),
        }
    }

    pub fn configure_audio_output(&mut self, sample_rate_hz: u32) {
        match self {
            Self::Nes(adapter) => adapter.configure_audio_output(sample_rate_hz),
            Self::Snes(adapter) => adapter.configure_audio_output(sample_rate_hz),
            Self::Sg1000(adapter) => adapter.configure_audio_output(sample_rate_hz),
            Self::MasterSystem(adapter) => adapter.configure_audio_output(sample_rate_hz),
            Self::MegaDrive(adapter) => adapter.configure_audio_output(sample_rate_hz),
            Self::Pce(adapter) => adapter.configure_audio_output(sample_rate_hz),
            Self::GameBoy(adapter) => adapter.configure_audio_output(sample_rate_hz),
            Self::GameBoyAdvance(adapter) => adapter.configure_audio_output(sample_rate_hz),
        }
    }

    pub fn drain_audio_i16(&mut self, out: &mut Vec<i16>) {
        match self {
            Self::Nes(adapter) => adapter.drain_audio_i16(out),
            Self::Snes(adapter) => adapter.drain_audio_i16(out),
            Self::Sg1000(adapter) => adapter.drain_audio_i16(out),
            Self::MasterSystem(adapter) => adapter.drain_audio_i16(out),
            Self::MegaDrive(adapter) => adapter.drain_audio_i16(out),
            Self::Pce(adapter) => adapter.drain_audio_i16(out),
            Self::GameBoy(adapter) => adapter.drain_audio_i16(out),
            Self::GameBoyAdvance(adapter) => adapter.drain_audio_i16(out),
        }
    }

    pub fn set_button(&mut self, player: u8, button: VirtualButton, pressed: bool) {
        match self {
            Self::Nes(adapter) => adapter.set_button(player, button, pressed),
            Self::Snes(adapter) => adapter.set_button(player, button, pressed),
            Self::Sg1000(adapter) => adapter.set_button(player, button, pressed),
            Self::MasterSystem(adapter) => adapter.set_button(player, button, pressed),
            Self::MegaDrive(adapter) => adapter.set_button(player, button, pressed),
            Self::Pce(adapter) => adapter.set_button(player, button, pressed),
            Self::GameBoy(adapter) => adapter.set_button(player, button, pressed),
            Self::GameBoyAdvance(adapter) => adapter.set_button(player, button, pressed),
        }
    }

    pub fn memory_regions(&self) -> Vec<MemoryRegion> {
        match self {
            Self::Nes(adapter) => adapter.memory_regions(),
            Self::Snes(adapter) => adapter.memory_regions(),
            Self::Sg1000(adapter) => adapter.memory_regions(),
            Self::MasterSystem(adapter) => adapter.memory_regions(),
            Self::MegaDrive(adapter) => adapter.memory_regions(),
            Self::Pce(adapter) => adapter.memory_regions(),
            Self::GameBoy(adapter) => adapter.memory_regions(),
            Self::GameBoyAdvance(adapter) => adapter.memory_regions(),
        }
    }

    pub fn read_memory(&self, region_id: &str) -> Option<&[u8]> {
        match self {
            Self::Nes(adapter) => adapter.read_memory(region_id),
            Self::Snes(adapter) => adapter.read_memory(region_id),
            Self::Sg1000(adapter) => adapter.read_memory(region_id),
            Self::MasterSystem(adapter) => adapter.read_memory(region_id),
            Self::MegaDrive(adapter) => adapter.read_memory(region_id),
            Self::Pce(adapter) => adapter.read_memory(region_id),
            Self::GameBoy(adapter) => adapter.read_memory(region_id),
            Self::GameBoyAdvance(adapter) => adapter.read_memory(region_id),
        }
    }

    pub fn write_memory_byte(&mut self, region_id: &str, offset: usize, value: u8) -> bool {
        match self {
            Self::Nes(adapter) => adapter.write_memory_byte(region_id, offset, value),
            Self::Snes(adapter) => adapter.write_memory_byte(region_id, offset, value),
            Self::Sg1000(adapter) => adapter.write_memory_byte(region_id, offset, value),
            Self::MasterSystem(adapter) => adapter.write_memory_byte(region_id, offset, value),
            Self::MegaDrive(adapter) => adapter.write_memory_byte(region_id, offset, value),
            Self::Pce(adapter) => adapter.write_memory_byte(region_id, offset, value),
            Self::GameBoy(adapter) => adapter.write_memory_byte(region_id, offset, value),
            Self::GameBoyAdvance(adapter) => adapter.write_memory_byte(region_id, offset, value),
        }
    }

    pub fn save_state_to_slot(&mut self, slot: u8) -> Result<()> {
        match self {
            Self::Nes(adapter) => adapter.save_state_to_slot(slot),
            Self::Snes(adapter) => adapter.save_state_to_slot(slot),
            Self::Sg1000(adapter) => adapter.save_state_to_slot(slot),
            Self::MasterSystem(adapter) => adapter.save_state_to_slot(slot),
            Self::MegaDrive(adapter) => adapter.save_state_to_slot(slot),
            Self::Pce(adapter) => adapter.save_state_to_slot(slot),
            Self::GameBoy(adapter) => adapter.save_state_to_slot(slot),
            Self::GameBoyAdvance(adapter) => adapter.save_state_to_slot(slot),
        }
    }

    pub fn load_state_from_slot(&mut self, slot: u8) -> Result<()> {
        match self {
            Self::Nes(adapter) => adapter.load_state_from_slot(slot),
            Self::Snes(adapter) => adapter.load_state_from_slot(slot),
            Self::Sg1000(adapter) => adapter.load_state_from_slot(slot),
            Self::MasterSystem(adapter) => adapter.load_state_from_slot(slot),
            Self::MegaDrive(adapter) => adapter.load_state_from_slot(slot),
            Self::Pce(adapter) => adapter.load_state_from_slot(slot),
            Self::GameBoy(adapter) => adapter.load_state_from_slot(slot),
            Self::GameBoyAdvance(adapter) => adapter.load_state_from_slot(slot),
        }
    }

    pub fn flush_persistent_save(&mut self) -> Result<()> {
        match self {
            Self::Nes(adapter) => adapter.flush_persistent_save(),
            Self::Snes(adapter) => adapter.flush_persistent_save(),
            Self::Sg1000(adapter) => adapter.flush_persistent_save(),
            Self::MasterSystem(adapter) => adapter.flush_persistent_save(),
            Self::MegaDrive(adapter) => adapter.flush_persistent_save(),
            Self::Pce(adapter) => adapter.flush_persistent_save(),
            Self::GameBoy(adapter) => adapter.flush_persistent_save(),
            Self::GameBoyAdvance(adapter) => adapter.flush_persistent_save(),
        }
    }
}
