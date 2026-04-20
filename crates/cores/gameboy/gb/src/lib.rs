mod apu;
mod bus;
mod cartridge;
mod cpu;
mod mapper;
mod ppu;
mod serial;
mod timer;

use bus::GbBus;
use cartridge::GbCartridgeHeader;
use cpu::GbCpu;
use emulator_core::{ConsoleKind, EmuError, EmuResult, EmulatorCore, FrameResult, RomImage};
use ppu::GB_FRAME_CYCLES;
use ppu::GbPpu;
use serial::GbSerial;
use timer::GbTimer;

pub use bus::{
    GB_KEY_A, GB_KEY_B, GB_KEY_DOWN, GB_KEY_LEFT, GB_KEY_RIGHT, GB_KEY_SELECT, GB_KEY_START,
    GB_KEY_UP,
};
pub use ppu::{GB_LCD_HEIGHT, GB_LCD_WIDTH};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GbModel {
    Dmg,
    Cgb,
}

#[derive(Debug, Default)]
pub struct GbEmulator {
    model: Option<GbModel>,
    bus: GbBus,
    cpu: GbCpu,
    ppu: GbPpu,
    timer: GbTimer,
    serial: GbSerial,
    cartridge_header: Option<GbCartridgeHeader>,
    frame_number: u64,
    cgb_ppu_cycle_carry: u32,
    rom_loaded: bool,
}

impl GbEmulator {
    pub fn new(model: GbModel) -> Self {
        Self {
            model: Some(model),
            ..Self::default()
        }
    }

    pub fn set_keyinput_pressed_mask(&mut self, pressed_mask: u8) {
        self.bus.set_keyinput_pressed_mask(pressed_mask);
    }

    pub fn frame_rgba8888(&self) -> &[u8] {
        self.ppu.frame_rgba8888()
    }

    pub fn load_backup_data(&mut self, data: &[u8]) {
        self.bus.load_cartridge_ram(data);
    }

    pub fn backup_data(&self) -> Option<&[u8]> {
        self.bus.cartridge_ram_data()
    }

    pub fn has_backup(&self) -> bool {
        self.bus.cartridge_ram_data().is_some()
    }

    pub fn debug_read8(&self, addr: u16) -> u8 {
        self.bus.read8(addr)
    }

    pub fn debug_read16(&self, addr: u16) -> u16 {
        self.bus.read16(addr)
    }

    pub fn debug_pc(&self) -> u16 {
        self.cpu.debug_pc()
    }

    pub fn debug_sp(&self) -> u16 {
        self.cpu.debug_sp()
    }

    pub fn debug_af(&self) -> u16 {
        self.cpu.debug_af()
    }

    pub fn debug_bc(&self) -> u16 {
        self.cpu.debug_bc()
    }

    pub fn debug_de(&self) -> u16 {
        self.cpu.debug_de()
    }

    pub fn debug_hl(&self) -> u16 {
        self.cpu.debug_hl()
    }

    pub fn debug_ime(&self) -> bool {
        self.cpu.debug_ime()
    }

    pub fn debug_halted(&self) -> bool {
        self.cpu.debug_halted()
    }

    pub fn debug_ppu_read_vram_bank(&self, bank: u8, addr: u16) -> u8 {
        self.bus.ppu_read_vram_bank(bank, addr)
    }

    pub fn debug_cgb_bg_palette_byte(&self, index: u8) -> u8 {
        self.bus.cgb_bg_palette_byte(index)
    }

    pub fn debug_cgb_obj_palette_byte(&self, index: u8) -> u8 {
        self.bus.cgb_obj_palette_byte(index)
    }

    pub fn debug_vram_write_count(&self) -> u64 {
        self.bus.debug_vram_write_count()
    }

    pub fn debug_hdma_bytes_copied(&self) -> u64 {
        self.bus.debug_hdma_bytes_copied()
    }

    pub fn debug_audio_sample_rate_hz(&self) -> u32 {
        self.bus.audio_sample_rate_hz()
    }

    pub fn take_audio_samples_i16_into(&mut self, out: &mut Vec<i16>) {
        self.bus.take_audio_samples_i16_into(out);
    }
}

impl EmulatorCore for GbEmulator {
    fn console_kind(&self) -> ConsoleKind {
        match self.model.unwrap_or(GbModel::Dmg) {
            GbModel::Dmg => ConsoleKind::Gb,
            GbModel::Cgb => ConsoleKind::Gbc,
        }
    }

    fn load_rom(&mut self, rom: RomImage) -> EmuResult<()> {
        let header = self.bus.load_cartridge(rom.bytes())?;
        self.cartridge_header = Some(header);
        self.rom_loaded = true;
        self.reset();
        Ok(())
    }

    fn reset(&mut self) {
        let cgb_mode = matches!(self.model.unwrap_or(GbModel::Dmg), GbModel::Cgb);
        self.bus.set_cgb_mode(cgb_mode);
        self.bus.reset();
        self.cpu.reset_for_model(cgb_mode);
        self.ppu.reset(&mut self.bus);
        self.timer.reset();
        self.serial.reset();
        self.frame_number = 0;
        self.cgb_ppu_cycle_carry = 0;
    }

    fn step_frame(&mut self) -> EmuResult<FrameResult> {
        if !self.rom_loaded {
            return Err(EmuError::InvalidState("ROM is not loaded"));
        }

        let mut ppu_cycles_this_frame = 0;
        while ppu_cycles_this_frame < GB_FRAME_CYCLES {
            let step_cycles = self.cpu.step(&mut self.bus);
            self.timer.step(step_cycles, &mut self.bus);
            self.serial.step(step_cycles, &mut self.bus);
            let ppu_cycles = if self.bus.cgb_double_speed() {
                let total = self.cgb_ppu_cycle_carry + step_cycles;
                self.cgb_ppu_cycle_carry = total & 1;
                total >> 1
            } else {
                step_cycles
            };
            self.bus.mix_audio_for_cycles(ppu_cycles);
            let frame_ready = self.ppu.step(ppu_cycles, &mut self.bus);
            ppu_cycles_this_frame += ppu_cycles;
            if frame_ready {
                break;
            }
        }

        self.frame_number += 1;
        Ok(FrameResult {
            cycles: ppu_cycles_this_frame,
            frame_number: self.frame_number,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gbc_reports_color_console_kind() {
        let emulator = GbEmulator::new(GbModel::Cgb);
        assert_eq!(emulator.console_kind(), ConsoleKind::Gbc);
    }

    #[test]
    fn gb_steps_frame_with_dummy_rom() {
        let mut emulator = GbEmulator::new(GbModel::Dmg);
        let dummy_rom =
            RomImage::from_bytes(vec![0x00; 0x8000]).expect("dummy ROM should be valid");
        emulator
            .load_rom(dummy_rom)
            .expect("ROM load should succeed");

        let frame = emulator.step_frame().expect("frame should step");
        assert!(frame.cycles > 0);
        assert_eq!(frame.frame_number, 1);
    }
}
