use crate::audio::Audio;
use crate::input::{Button, Input};
use crate::vdp::Vdp;

const WORK_RAM_SIZE: usize = 0x2000;
const CART_RAM_SIZE: usize = 0x8000;
const ROM_BANK_SIZE: usize = 0x4000;

#[derive(Debug, Clone, bincode::Encode, bincode::Decode)]
pub struct Bus {
    rom: Vec<u8>,
    work_ram: [u8; WORK_RAM_SIZE],
    cart_ram: [u8; CART_RAM_SIZE],
    mapper_control: u8,
    mapper_pages: [u8; 3],
    vdp: Vdp,
    audio: Audio,
    input: Input,
}

impl Bus {
    pub fn new(rom: Vec<u8>) -> Self {
        Self {
            rom,
            work_ram: [0; WORK_RAM_SIZE],
            cart_ram: [0; CART_RAM_SIZE],
            mapper_control: 0,
            mapper_pages: [0, 1, 2],
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
            0x0000..=0x03FF => self.read_rom_addr(addr as usize),
            0x0400..=0x3FFF => self.read_rom_bank(self.mapper_pages[0], addr as usize),
            0x4000..=0x7FFF => self.read_rom_bank(self.mapper_pages[1], (addr as usize) - 0x4000),
            0x8000..=0xBFFF if self.cartridge_ram_enabled() => {
                let bank = self.cartridge_ram_bank();
                self.cart_ram[bank * ROM_BANK_SIZE + (addr as usize - 0x8000)]
            }
            0x8000..=0xBFFF => self.read_rom_bank(self.mapper_pages[2], (addr as usize) - 0x8000),
            0xC000..=0xFFFF => self.work_ram[((addr as usize) - 0xC000) & (WORK_RAM_SIZE - 1)],
        }
    }

    pub fn write_memory(&mut self, addr: u16, value: u8) {
        match addr {
            0x8000..=0xBFFF if self.cartridge_ram_enabled() => {
                let bank = self.cartridge_ram_bank();
                self.cart_ram[bank * ROM_BANK_SIZE + (addr as usize - 0x8000)] = value;
            }
            0xC000..=0xFFFF => {
                self.work_ram[((addr as usize) - 0xC000) & (WORK_RAM_SIZE - 1)] = value;
                match addr {
                    0xFFFC => self.mapper_control = value,
                    0xFFFD => self.mapper_pages[0] = value,
                    0xFFFE => self.mapper_pages[1] = value,
                    0xFFFF => self.mapper_pages[2] = value,
                    _ => {}
                }
            }
            _ => {}
        }
    }

    pub fn read_port(&mut self, port: u8) -> u8 {
        match port {
            0x7E => self.vdp.read_v_counter(),
            0x7F => self.vdp.read_h_counter(),
            0x80..=0xBF if (port & 0x01) == 0 => self.vdp.read_data_port(),
            0x80..=0xBF => self.vdp.read_status_port(),
            0xDC | 0xC0 => self.input.read_port1(),
            0xDD | 0xC1 => self.input.read_port2(),
            _ => 0xFF,
        }
    }

    pub fn write_port(&mut self, port: u8, value: u8) {
        match port {
            0x7E | 0x7F => self.audio.write_psg(value),
            0x80..=0xBF if (port & 0x01) == 0 => self.vdp.write_data_port(value),
            0x80..=0xBF => self.vdp.write_control_port(value),
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

    pub fn cart_ram(&self) -> &[u8] {
        &self.cart_ram
    }

    pub fn cart_ram_mut(&mut self) -> &mut [u8] {
        &mut self.cart_ram
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

    fn cartridge_ram_enabled(&self) -> bool {
        (self.mapper_control & 0x08) != 0
    }

    fn cartridge_ram_bank(&self) -> usize {
        if (self.mapper_control & 0x04) != 0 {
            1
        } else {
            0
        }
    }

    fn read_rom_addr(&self, addr: usize) -> u8 {
        if self.rom.is_empty() {
            return 0xFF;
        }
        self.rom[addr % self.rom.len()]
    }

    fn read_rom_bank(&self, bank: u8, offset: usize) -> u8 {
        if self.rom.is_empty() {
            return 0xFF;
        }
        let bank_count = self.rom.len().div_ceil(ROM_BANK_SIZE).max(1);
        let bank = (bank as usize) % bank_count;
        let addr = bank * ROM_BANK_SIZE + (offset & (ROM_BANK_SIZE - 1));
        self.rom[addr % self.rom.len()]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mapper_switches_slot_two() {
        let mut rom = vec![0; ROM_BANK_SIZE * 4];
        for bank in 0..4 {
            rom[bank * ROM_BANK_SIZE] = bank as u8;
        }
        let mut bus = Bus::new(rom);

        assert_eq!(bus.read_memory(0x8000), 2);
        bus.write_memory(0xFFFF, 3);
        assert_eq!(bus.read_memory(0x8000), 3);
    }

    #[test]
    fn work_ram_is_e000_mirror() {
        let mut bus = Bus::new(vec![0]);

        bus.write_memory(0xC000, 0x5A);

        assert_eq!(bus.read_memory(0xE000), 0x5A);
    }

    #[test]
    fn cartridge_ram_can_replace_slot_two() {
        let mut bus = Bus::new(vec![0; ROM_BANK_SIZE * 3]);

        bus.write_memory(0xFFFC, 0x08);
        bus.write_memory(0x8000, 0x9C);

        assert_eq!(bus.read_memory(0x8000), 0x9C);
        assert_eq!(bus.cart_ram()[0], 0x9C);
    }
}
