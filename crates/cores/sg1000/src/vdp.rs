use crate::z80::Z80_CLOCK_HZ;
use sega8_common::tms9918;

pub const FRAME_WIDTH: usize = tms9918::FRAME_WIDTH;
pub const FRAME_HEIGHT: usize = tms9918::FRAME_HEIGHT;

const VRAM_SIZE: usize = tms9918::VRAM_SIZE;
const REG_COUNT: usize = 8;
const CYCLES_PER_FRAME: u32 = (Z80_CLOCK_HZ / 60) as u32;
const STATUS_VBLANK: u8 = 0x80;
const STATUS_SPRITE_OVERFLOW: u8 = 0x40;
const STATUS_SPRITE_COLLISION: u8 = 0x20;

#[derive(Debug, Clone, bincode::Encode, bincode::Decode)]
pub struct Vdp {
    frame_cycles: u32,
    frame_count: u64,
    vram: [u8; VRAM_SIZE],
    registers: [u8; REG_COUNT],
    status: u8,
    control_latch: Option<u8>,
    access_addr: u16,
    read_buffer: u8,
    frame_buffer: Vec<u8>,
}

impl Vdp {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn step(&mut self, cycles: u32) -> bool {
        self.frame_cycles = self.frame_cycles.saturating_add(cycles);
        if self.frame_cycles < CYCLES_PER_FRAME {
            return false;
        }
        self.frame_cycles -= CYCLES_PER_FRAME;
        self.frame_count = self.frame_count.saturating_add(1);
        self.status |= STATUS_VBLANK;
        self.render_frame();
        true
    }

    pub fn frame_count(&self) -> u64 {
        self.frame_count
    }

    pub fn frame_buffer(&self) -> &[u8] {
        &self.frame_buffer
    }

    pub fn vram(&self) -> &[u8] {
        &self.vram
    }

    pub fn vram_mut(&mut self) -> &mut [u8] {
        &mut self.vram
    }

    pub fn interrupt_enabled(&self) -> bool {
        (self.registers[1] & 0x20) != 0
    }

    pub fn read_data_port(&mut self) -> u8 {
        self.control_latch = None;
        let value = self.read_buffer;
        self.read_buffer = self.vram[self.access_addr as usize % VRAM_SIZE];
        self.access_addr = self.access_addr.wrapping_add(1) & 0x3FFF;
        value
    }

    pub fn read_status_port(&mut self) -> u8 {
        self.control_latch = None;
        let status = self.status;
        self.status &= !(STATUS_VBLANK | STATUS_SPRITE_OVERFLOW | STATUS_SPRITE_COLLISION);
        status
    }

    pub fn write_data_port(&mut self, value: u8) {
        self.control_latch = None;
        self.vram[self.access_addr as usize % VRAM_SIZE] = value;
        self.access_addr = self.access_addr.wrapping_add(1) & 0x3FFF;
    }

    pub fn write_control_port(&mut self, value: u8) {
        if let Some(first) = self.control_latch.take() {
            if (value & 0x80) != 0 {
                let reg = (value & 0x07) as usize;
                self.registers[reg] = first;
            } else {
                self.access_addr = (((value & 0x3F) as u16) << 8) | first as u16;
                if (value & 0x40) == 0 {
                    self.read_buffer = self.vram[self.access_addr as usize % VRAM_SIZE];
                    self.access_addr = self.access_addr.wrapping_add(1) & 0x3FFF;
                }
            }
        } else {
            self.control_latch = Some(value);
        }
    }

    fn render_frame(&mut self) {
        self.status &= !(STATUS_SPRITE_OVERFLOW | STATUS_SPRITE_COLLISION);
        self.status |= tms9918::render_frame(&mut self.frame_buffer, &self.vram, &self.registers);
    }

    #[cfg(test)]
    fn sprite_pattern_bit(
        &self,
        pattern_base: usize,
        pattern: usize,
        sprite_size: usize,
        x: usize,
        y: usize,
    ) -> bool {
        tms9918::sprite_pattern_bit(&self.vram, pattern_base, pattern, sprite_size, x, y)
    }
}

impl Default for Vdp {
    fn default() -> Self {
        let mut vdp = Self {
            frame_cycles: 0,
            frame_count: 0,
            vram: [0; VRAM_SIZE],
            registers: [0; REG_COUNT],
            status: 0,
            control_latch: None,
            access_addr: 0,
            read_buffer: 0,
            frame_buffer: vec![0; FRAME_WIDTH * FRAME_HEIGHT * 3],
        };
        vdp.render_frame();
        vdp
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn color_zero_uses_backdrop_color_in_graphics_ii() {
        let mut vdp = Vdp::new();
        vdp.registers[0] = 0x02;
        vdp.registers[1] = 0x40;
        vdp.registers[2] = 0x0E;
        vdp.registers[3] = 0xFF;
        vdp.registers[4] = 0x03;
        vdp.registers[7] = 0x0C;
        vdp.vram[0x3800] = 0;
        vdp.vram[0x0000] = 0x00;
        vdp.vram[0x2000] = 0xF0;

        vdp.render_frame();

        let backdrop = tms9918::tms_palette_color(0x0C);
        assert_eq!(
            &vdp.frame_buffer()[0..3],
            &[backdrop.0, backdrop.1, backdrop.2]
        );
    }

    #[test]
    fn sixteen_by_sixteen_sprites_use_tms_quadrant_order() {
        let mut vdp = Vdp::new();
        vdp.registers[1] = 0x42;
        let pattern_base = 0x0000;
        let pattern = 4;

        let quadrants = [
            0, // top-left
            1, // bottom-left
            2, // top-right
            3, // bottom-right
        ];
        for tile_offset in quadrants {
            vdp.vram[pattern_base + (pattern + tile_offset) * 8] = 0x80;
        }

        assert!(vdp.sprite_pattern_bit(pattern_base, pattern, 16, 0, 0));
        assert!(vdp.sprite_pattern_bit(pattern_base, pattern, 16, 0, 8));
        assert!(vdp.sprite_pattern_bit(pattern_base, pattern, 16, 8, 0));
        assert!(vdp.sprite_pattern_bit(pattern_base, pattern, 16, 8, 8));
    }
}
