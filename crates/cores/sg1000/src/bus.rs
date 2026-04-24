use crate::audio::Audio;
use crate::input::{Button, Input};
use crate::vdp::Vdp;
use sega8_common::bus::{InputDevice, Sega8BusDevices, VdpDevice};

const WORK_RAM_SIZE: usize = 0x400;

#[derive(Debug, Clone, bincode::Encode, bincode::Decode)]
pub struct Bus {
    rom: Vec<u8>,
    work_ram: [u8; WORK_RAM_SIZE],
    vdp: Vdp,
    audio: Audio,
    input: Input,
}

impl Bus {
    pub fn new(rom: Vec<u8>) -> Self {
        Self {
            rom,
            work_ram: [0; WORK_RAM_SIZE],
            vdp: Vdp::new(),
            audio: Audio::new(),
            input: Input::new(),
        }
    }

    pub fn step(&mut self, cycles: u32) -> bool {
        <Self as Sega8BusDevices>::step_devices(self, cycles)
    }

    pub fn read_memory(&self, addr: u16) -> u8 {
        match addr {
            0x0000..=0xBFFF => {
                if self.rom.is_empty() {
                    0xFF
                } else {
                    self.rom[addr as usize % self.rom.len()]
                }
            }
            0xC000..=0xFFFF => self.work_ram[((addr as usize) - 0xC000) & (WORK_RAM_SIZE - 1)],
        }
    }

    pub fn write_memory(&mut self, addr: u16, value: u8) {
        if (0xC000..=0xFFFF).contains(&addr) {
            self.work_ram[((addr as usize) - 0xC000) & (WORK_RAM_SIZE - 1)] = value;
        }
    }

    pub fn read_port(&mut self, port: u8) -> u8 {
        match port {
            0xBE => self.vdp.read_data_port(),
            0xBF => self.vdp.read_status_port(),
            0xDC | 0xC0 => <Self as Sega8BusDevices>::read_input_port1(self),
            0xDD | 0xC1 => <Self as Sega8BusDevices>::read_input_port2(self),
            _ => 0xFF,
        }
    }

    pub fn write_port(&mut self, port: u8, value: u8) {
        match port {
            0x7F => <Self as Sega8BusDevices>::write_psg(self, value),
            0xBE => self.vdp.write_data_port(value),
            0xBF => self.vdp.write_control_port(value),
            _ => {}
        }
    }

    pub fn frame_buffer(&self) -> &[u8] {
        <Self as Sega8BusDevices>::device_frame_buffer(self)
    }

    pub fn work_ram(&self) -> &[u8] {
        &self.work_ram
    }

    pub fn work_ram_mut(&mut self) -> &mut [u8] {
        &mut self.work_ram
    }

    pub fn vram(&self) -> &[u8] {
        <Self as Sega8BusDevices>::device_vram(self)
    }

    pub fn vram_mut(&mut self) -> &mut [u8] {
        <Self as Sega8BusDevices>::device_vram_mut(self)
    }

    pub fn set_button_pressed(&mut self, player: u8, button: Button, pressed: bool) {
        <Self as Sega8BusDevices>::set_device_button_pressed(self, player, button, pressed);
    }

    pub fn pending_audio_samples(&self) -> usize {
        <Self as Sega8BusDevices>::pending_audio_samples(self)
    }

    pub fn drain_audio_samples(&mut self, max_samples: usize) -> Vec<i16> {
        <Self as Sega8BusDevices>::drain_audio_samples(self, max_samples)
    }

    pub fn set_audio_output_sample_rate_hz(&mut self, hz: u32) {
        <Self as Sega8BusDevices>::set_audio_output_sample_rate_hz(self, hz);
    }

    pub fn audio_output_channels(&self) -> u8 {
        <Self as Sega8BusDevices>::audio_output_channels(self)
    }

    pub fn vdp_interrupt_enabled(&self) -> bool {
        <Self as Sega8BusDevices>::device_vdp_interrupt_enabled(self)
    }

    pub fn frame_count(&self) -> u64 {
        <Self as Sega8BusDevices>::device_frame_count(self)
    }
}

impl Sega8BusDevices for Bus {
    type Button = Button;
    type Input = Input;
    type Vdp = Vdp;

    fn audio(&self) -> &Audio {
        &self.audio
    }

    fn audio_mut(&mut self) -> &mut Audio {
        &mut self.audio
    }

    fn input(&self) -> &Self::Input {
        &self.input
    }

    fn input_mut(&mut self) -> &mut Self::Input {
        &mut self.input
    }

    fn vdp(&self) -> &Self::Vdp {
        &self.vdp
    }

    fn vdp_mut(&mut self) -> &mut Self::Vdp {
        &mut self.vdp
    }
}

impl InputDevice<Button> for Input {
    fn set_button_pressed(&mut self, player: u8, button: Button, pressed: bool) {
        Input::set_button_pressed(self, player, button, pressed);
    }

    fn read_port1(&self) -> u8 {
        Input::read_port1(self)
    }

    fn read_port2(&self) -> u8 {
        Input::read_port2(self)
    }
}

impl VdpDevice for Vdp {
    fn step(&mut self, cycles: u32) -> bool {
        Vdp::step(self, cycles)
    }

    fn frame_buffer(&self) -> &[u8] {
        Vdp::frame_buffer(self)
    }

    fn vram(&self) -> &[u8] {
        Vdp::vram(self)
    }

    fn vram_mut(&mut self) -> &mut [u8] {
        Vdp::vram_mut(self)
    }

    fn interrupt_enabled(&self) -> bool {
        Vdp::interrupt_enabled(self)
    }

    fn frame_count(&self) -> u64 {
        Vdp::frame_count(self)
    }
}

impl sega8_common::z80::BusIo for Bus {
    fn read_memory(&mut self, addr: u16) -> u8 {
        Bus::read_memory(self, addr)
    }

    fn write_memory(&mut self, addr: u16, value: u8) {
        Bus::write_memory(self, addr, value);
    }

    fn read_port(&mut self, port: u8) -> u8 {
        Bus::read_port(self, port)
    }

    fn write_port(&mut self, port: u8, value: u8) {
        Bus::write_port(self, port, value);
    }
}

impl sega8_common::emulator::FrameBus for Bus {
    fn step_components(&mut self, cycles: u32) -> bool {
        Bus::step(self, cycles)
    }

    fn vdp_interrupt_enabled(&self) -> bool {
        Bus::vdp_interrupt_enabled(self)
    }

    fn frame_count(&self) -> u64 {
        Bus::frame_count(self)
    }
}
