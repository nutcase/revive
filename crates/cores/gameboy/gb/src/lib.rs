mod apu;
mod bus;
mod cartridge;
mod cpu;
mod mapper;
mod ppu;
mod serial;
pub mod state;
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

    pub fn backup_data_mut(&mut self) -> Option<&mut [u8]> {
        self.bus.cartridge_ram_data_mut()
    }

    pub fn has_backup(&self) -> bool {
        self.bus.cartridge_ram_data().is_some()
    }

    pub fn work_ram(&self) -> &[u8] {
        self.bus.work_ram()
    }

    pub fn work_ram_mut(&mut self) -> &mut [u8] {
        self.bus.work_ram_mut()
    }

    pub fn video_ram(&self) -> &[u8] {
        self.bus.video_ram()
    }

    pub fn video_ram_mut(&mut self) -> &mut [u8] {
        self.bus.video_ram_mut()
    }

    pub fn oam(&self) -> &[u8] {
        self.bus.oam()
    }

    pub fn oam_mut(&mut self) -> &mut [u8] {
        self.bus.oam_mut()
    }

    pub fn high_ram(&self) -> &[u8] {
        self.bus.high_ram()
    }

    pub fn high_ram_mut(&mut self) -> &mut [u8] {
        self.bus.high_ram_mut()
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

    fn serialize_state_payload(&self) -> Vec<u8> {
        let mut w = state::StateWriter::new();
        self.bus.serialize_state(&mut w);
        self.cpu.serialize_state(&mut w);
        self.ppu.serialize_state(&mut w);
        self.timer.serialize_state(&mut w);
        self.serial.serialize_state(&mut w);
        w.write_u64(self.frame_number);
        w.write_u32(self.cgb_ppu_cycle_carry);
        w.write_bool(self.rom_loaded);
        w.into_vec()
    }

    pub fn save_state(&self) -> Vec<u8> {
        let payload = self.serialize_state_payload();
        let payload_len = payload.len() as u32;
        let mut out = Vec::with_capacity(20 + payload.len());
        out.extend_from_slice(b"GBST");
        out.extend_from_slice(&1u32.to_le_bytes());
        out.extend_from_slice(&gb_model_tag(self.model.unwrap_or(GbModel::Dmg)).to_le_bytes());
        out.extend_from_slice(&self.rom_crc32().to_le_bytes());
        out.extend_from_slice(&payload_len.to_le_bytes());
        out.extend_from_slice(&payload);
        out
    }

    pub fn load_state(&mut self, data: &[u8]) -> Result<(), &'static str> {
        if data.len() < 20 {
            return Err("state data too short");
        }
        if &data[0..4] != b"GBST" {
            return Err("invalid state magic");
        }
        let version = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
        if version != 1 {
            return Err("unsupported state version");
        }
        let model = u32::from_le_bytes([data[8], data[9], data[10], data[11]]);
        if model != gb_model_tag(self.model.unwrap_or(GbModel::Dmg)) {
            return Err("model mismatch");
        }
        let rom_crc = u32::from_le_bytes([data[12], data[13], data[14], data[15]]);
        if rom_crc != self.rom_crc32() {
            return Err("ROM CRC mismatch");
        }
        let payload_len = u32::from_le_bytes([data[16], data[17], data[18], data[19]]) as usize;
        if data.len() < 20 + payload_len {
            return Err("state data truncated");
        }

        let mut r = state::StateReader::new(&data[20..20 + payload_len]);
        self.bus.deserialize_state(&mut r)?;
        self.cpu.deserialize_state(&mut r)?;
        self.ppu.deserialize_state(&mut r)?;
        self.timer.deserialize_state(&mut r)?;
        self.serial.deserialize_state(&mut r)?;
        self.frame_number = r.read_u64()?;
        self.cgb_ppu_cycle_carry = r.read_u32()?;
        self.rom_loaded = r.read_bool()?;
        if r.remaining() != 0 {
            return Err("state payload has trailing data");
        }
        Ok(())
    }

    fn rom_crc32(&self) -> u32 {
        state::crc32(self.bus.rom_bytes())
    }

    pub fn take_audio_samples_i16_into(&mut self, out: &mut Vec<i16>) {
        self.bus.take_audio_samples_i16_into(out);
    }
}

fn gb_model_tag(model: GbModel) -> u32 {
    match model {
        GbModel::Dmg => 0,
        GbModel::Cgb => 1,
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

    #[test]
    fn save_state_roundtrip_restores_cpu_and_memory() {
        let mut emulator = GbEmulator::new(GbModel::Dmg);
        let rom = RomImage::from_bytes(vec![0x00; 0x8000]).expect("dummy ROM should be valid");
        emulator.load_rom(rom).expect("ROM load should succeed");
        emulator.bus.write8(0xC123, 0x42);
        emulator.bus.write8(0xFF40, 0x91);
        emulator.step_frame().expect("frame should step");
        let expected_frame = emulator.frame_number;

        let state_data = emulator.save_state();
        assert!(state_data.len() > 20);
        assert_eq!(&state_data[0..4], b"GBST");

        emulator.bus.write8(0xC123, 0x99);
        emulator.load_state(&state_data).expect("state should load");

        assert_eq!(emulator.debug_read8(0xC123), 0x42);
        assert_eq!(emulator.debug_read8(0xFF40), 0x91);
        assert_eq!(emulator.frame_number, expected_frame);
    }

    #[test]
    fn save_state_rejects_wrong_model() {
        let mut dmg = GbEmulator::new(GbModel::Dmg);
        dmg.load_rom(RomImage::from_bytes(vec![0x00; 0x8000]).unwrap())
            .unwrap();
        let state_data = dmg.save_state();

        let mut cgb = GbEmulator::new(GbModel::Cgb);
        let mut cgb_rom = vec![0x00; 0x8000];
        cgb_rom[0x0143] = 0x80;
        cgb.load_rom(RomImage::from_bytes(cgb_rom).unwrap())
            .unwrap();

        assert_eq!(cgb.load_state(&state_data), Err("model mismatch"));
    }

    #[test]
    fn save_state_rejects_wrong_crc() {
        let mut emulator = GbEmulator::new(GbModel::Dmg);
        emulator
            .load_rom(RomImage::from_bytes(vec![0x00; 0x8000]).unwrap())
            .unwrap();
        let mut state_data = emulator.save_state();
        state_data[12] ^= 0xFF;
        assert_eq!(emulator.load_state(&state_data), Err("ROM CRC mismatch"));
    }
}
