use crate::audio::Audio;
use crate::input::{Button, Input};
use crate::vdp::Vdp;

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
        self.audio.step(cycles);
        self.vdp.step(cycles)
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
            0xDC | 0xC0 => self.input.read_port1(),
            0xDD | 0xC1 => self.input.read_port2(),
            _ => 0xFF,
        }
    }

    pub fn write_port(&mut self, port: u8, value: u8) {
        match port {
            0x7F => self.audio.write_psg(value),
            0xBE => self.vdp.write_data_port(value),
            0xBF => self.vdp.write_control_port(value),
            _ => {}
        }
    }

    pub fn frame_buffer(&self) -> &[u8] {
        self.vdp.frame_buffer()
    }

    pub fn work_ram(&self) -> &[u8] {
        &self.work_ram
    }

    pub fn work_ram_mut(&mut self) -> &mut [u8] {
        &mut self.work_ram
    }

    pub fn vram(&self) -> &[u8] {
        self.vdp.vram()
    }

    pub fn vram_mut(&mut self) -> &mut [u8] {
        self.vdp.vram_mut()
    }

    pub fn set_button_pressed(&mut self, player: u8, button: Button, pressed: bool) {
        self.input.set_button_pressed(player, button, pressed);
    }

    pub fn pending_audio_samples(&self) -> usize {
        self.audio.pending_samples()
    }

    pub fn drain_audio_samples(&mut self, max_samples: usize) -> Vec<i16> {
        self.audio.drain_samples(max_samples)
    }

    pub fn set_audio_output_sample_rate_hz(&mut self, hz: u32) {
        self.audio.set_output_sample_rate_hz(hz);
    }

    pub fn audio_output_channels(&self) -> u8 {
        self.audio.output_channels()
    }

    pub fn vdp_interrupt_enabled(&self) -> bool {
        self.vdp.interrupt_enabled()
    }

    pub fn frame_count(&self) -> u64 {
        self.vdp.frame_count()
    }
}
