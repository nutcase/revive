use crate::z80::Z80_CLOCK_HZ;
use sega8_common::tms9918;

pub const FRAME_WIDTH: usize = tms9918::FRAME_WIDTH;
pub const FRAME_HEIGHT: usize = tms9918::FRAME_HEIGHT;

const VRAM_SIZE: usize = tms9918::VRAM_SIZE;
const CRAM_SIZE: usize = 0x20;
const REG_COUNT: usize = 16;
const CYCLES_PER_FRAME: u32 = (Z80_CLOCK_HZ / 60) as u32;
const NTSC_TOTAL_LINES: u32 = 262;
const STATUS_VBLANK: u8 = 0x80;
const STATUS_SPRITE_OVERFLOW: u8 = 0x40;
const STATUS_SPRITE_COLLISION: u8 = 0x20;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, bincode::Encode, bincode::Decode)]
enum AccessMode {
    VramRead,
    #[default]
    VramWrite,
    CramWrite,
}

#[derive(Debug, Clone, bincode::Encode, bincode::Decode)]
pub struct Vdp {
    frame_cycles: u32,
    frame_count: u64,
    vram: [u8; VRAM_SIZE],
    cram: [u8; CRAM_SIZE],
    registers: [u8; REG_COUNT],
    status: u8,
    control_latch: Option<u8>,
    access_addr: u16,
    access_mode: AccessMode,
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

    pub fn read_v_counter(&self) -> u8 {
        let line = (u64::from(self.frame_cycles) * u64::from(NTSC_TOTAL_LINES)
            / u64::from(CYCLES_PER_FRAME)) as u16;
        if line >= 0xDA {
            line.wrapping_sub(6) as u8
        } else {
            line as u8
        }
    }

    pub fn read_h_counter(&self) -> u8 {
        0
    }

    pub fn write_data_port(&mut self, value: u8) {
        self.control_latch = None;
        match self.access_mode {
            AccessMode::CramWrite => {
                self.cram[self.access_addr as usize & (CRAM_SIZE - 1)] = value & 0x3F;
            }
            AccessMode::VramRead | AccessMode::VramWrite => {
                self.vram[self.access_addr as usize % VRAM_SIZE] = value;
            }
        }
        self.access_addr = self.access_addr.wrapping_add(1) & 0x3FFF;
    }

    pub fn write_control_port(&mut self, value: u8) {
        if let Some(first) = self.control_latch.take() {
            match value & 0xC0 {
                0x80 => {
                    let reg = (value & 0x0F) as usize;
                    if reg < REG_COUNT {
                        self.registers[reg] = first;
                    }
                }
                0xC0 => {
                    self.access_addr = (((value & 0x3F) as u16) << 8) | first as u16;
                    self.access_mode = AccessMode::CramWrite;
                }
                0x40 => {
                    self.access_addr = (((value & 0x3F) as u16) << 8) | first as u16;
                    self.access_mode = AccessMode::VramWrite;
                }
                _ => {
                    self.access_addr = (((value & 0x3F) as u16) << 8) | first as u16;
                    self.access_mode = AccessMode::VramRead;
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
        if self.mode4_enabled() {
            self.render_mode4();
            return;
        }

        self.status |= tms9918::render_frame(&mut self.frame_buffer, &self.vram, &self.registers);
    }

    fn mode4_enabled(&self) -> bool {
        (self.registers[0] & 0x04) != 0
    }

    fn display_enabled(&self) -> bool {
        (self.registers[1] & 0x40) != 0
    }

    fn sms_backdrop_index(&self) -> usize {
        (self.registers[7] & 0x0F) as usize
    }

    fn fill_frame(&mut self, color: (u8, u8, u8)) {
        for pixel in self.frame_buffer.chunks_exact_mut(3) {
            pixel[0] = color.0;
            pixel[1] = color.1;
            pixel[2] = color.2;
        }
    }

    fn render_mode4(&mut self) {
        let backdrop_index = self.sms_backdrop_index();
        let backdrop = self.sms_color(backdrop_index);
        self.fill_frame(backdrop);

        if !self.display_enabled() {
            return;
        }

        let nt_base = ((self.registers[2] as usize >> 1) & 0x07) * 0x800;
        let sat_base = ((self.registers[5] as usize >> 1) & 0x3F) * 0x100;
        let sprite_tile_offset = if (self.registers[6] & 0x04) != 0 {
            256
        } else {
            0
        };
        let sprites_8x16 = (self.registers[1] & 0x02) != 0;
        let sprite_height = if sprites_8x16 { 16 } else { 8 };
        let shift_sprites_left = (self.registers[0] & 0x08) != 0;
        let mask_left_column = (self.registers[0] & 0x20) != 0;
        let lock_top_hscroll = (self.registers[0] & 0x40) != 0;
        let lock_right_vscroll = (self.registers[0] & 0x80) != 0;
        let hscroll = self.registers[8] as usize;
        let vscroll = self.registers[9] as usize;

        #[derive(Debug)]
        struct SpriteEntry {
            y: i16,
            x: i16,
            tile: usize,
        }

        let mut sprites = Vec::with_capacity(64);
        for i in 0..64 {
            let y_byte = self.vram[(sat_base + i) % VRAM_SIZE];
            if y_byte == 0xD0 {
                break;
            }
            let x_addr = sat_base + 0x80 + i * 2;
            let mut x = self.vram[x_addr % VRAM_SIZE] as i16;
            if shift_sprites_left {
                x -= 8;
            }
            sprites.push(SpriteEntry {
                y: y_byte.wrapping_add(1) as i16,
                x,
                tile: self.vram[(x_addr + 1) % VRAM_SIZE] as usize,
            });
        }

        for y in 0..FRAME_HEIGHT {
            let mut line_sprites = Vec::with_capacity(8);
            for sprite in &sprites {
                let y_i16 = y as i16;
                if y_i16 >= sprite.y && y_i16 < sprite.y + sprite_height as i16 {
                    if line_sprites.len() == 8 {
                        self.status |= STATUS_SPRITE_OVERFLOW;
                        break;
                    }
                    line_sprites.push(sprite);
                }
            }

            let mut sprite_colors = [0u8; FRAME_WIDTH];
            let mut sprite_filled = [false; FRAME_WIDTH];
            for sprite in line_sprites {
                let py = (y as i16 - sprite.y) as usize;
                let mut tile = sprite.tile + sprite_tile_offset;
                let tile_row = if sprites_8x16 {
                    tile &= !1;
                    if py >= 8 {
                        tile += 1;
                        py - 8
                    } else {
                        py
                    }
                } else {
                    py
                };

                for px in 0..8 {
                    let x = sprite.x + px as i16;
                    if !(0..FRAME_WIDTH as i16).contains(&x) {
                        continue;
                    }
                    let color = self.sms_tile_pixel(tile, tile_row, px);
                    if color == 0 {
                        continue;
                    }
                    let x = x as usize;
                    if sprite_filled[x] {
                        self.status |= STATUS_SPRITE_COLLISION;
                        continue;
                    }
                    sprite_filled[x] = true;
                    sprite_colors[x] = color + 16;
                }
            }

            let effective_hscroll = if lock_top_hscroll && y < 16 {
                0
            } else {
                hscroll
            };

            for x in 0..FRAME_WIDTH {
                let effective_vscroll = if lock_right_vscroll && x >= 192 {
                    0
                } else {
                    vscroll
                };
                let scrolled_x = (x + FRAME_WIDTH - effective_hscroll) % FRAME_WIDTH;
                let scrolled_y = (y + effective_vscroll) % (28 * 8);
                let tile_col = scrolled_x / 8;
                let tile_row = scrolled_y / 8;
                let pixel_x = scrolled_x & 7;
                let pixel_y = scrolled_y & 7;

                let nt_addr = nt_base + (tile_row * 32 + tile_col) * 2;
                let nt_word = u16::from_le_bytes([
                    self.vram[nt_addr % VRAM_SIZE],
                    self.vram[(nt_addr + 1) % VRAM_SIZE],
                ]);
                let bg_tile = (nt_word & 0x01FF) as usize;
                let bg_hflip = (nt_word & 0x0200) != 0;
                let bg_vflip = (nt_word & 0x0400) != 0;
                let bg_palette = if (nt_word & 0x0800) != 0 { 16 } else { 0 };
                let bg_priority = (nt_word & 0x1000) != 0;
                let sample_x = if bg_hflip { 7 - pixel_x } else { pixel_x };
                let sample_y = if bg_vflip { 7 - pixel_y } else { pixel_y };
                let bg_color = self.sms_tile_pixel(bg_tile, sample_y, sample_x);
                let bg_opaque = bg_color != 0;
                let sprite_color = sprite_colors[x];
                let sprite_opaque = sprite_color != 0;

                let color_index = if bg_priority && bg_opaque {
                    bg_palette + bg_color as usize
                } else if sprite_opaque {
                    sprite_color as usize
                } else if bg_opaque {
                    bg_palette + bg_color as usize
                } else {
                    backdrop_index
                };

                let color = if mask_left_column && x < 8 {
                    backdrop
                } else {
                    self.sms_color(color_index)
                };
                self.set_pixel(x, y, color);
            }
        }
    }

    fn sms_tile_pixel(&self, tile: usize, row: usize, col: usize) -> u8 {
        let addr = tile * 32 + (row & 7) * 4;
        if addr + 3 >= VRAM_SIZE {
            return 0;
        }
        let bit = 7 - (col & 7);
        let b0 = (self.vram[addr] >> bit) & 1;
        let b1 = (self.vram[addr + 1] >> bit) & 1;
        let b2 = (self.vram[addr + 2] >> bit) & 1;
        let b3 = (self.vram[addr + 3] >> bit) & 1;
        b0 | (b1 << 1) | (b2 << 2) | (b3 << 3)
    }

    fn sms_color(&self, index: usize) -> (u8, u8, u8) {
        sms_cram_to_rgb(self.cram[index % CRAM_SIZE])
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

    fn set_pixel(&mut self, x: usize, y: usize, color: (u8, u8, u8)) {
        if x >= FRAME_WIDTH || y >= FRAME_HEIGHT {
            return;
        }
        let offset = (y * FRAME_WIDTH + x) * 3;
        self.frame_buffer[offset] = color.0;
        self.frame_buffer[offset + 1] = color.1;
        self.frame_buffer[offset + 2] = color.2;
    }
}

impl Default for Vdp {
    fn default() -> Self {
        let mut vdp = Self {
            frame_cycles: 0,
            frame_count: 0,
            vram: [0; VRAM_SIZE],
            cram: [0; CRAM_SIZE],
            registers: [0; REG_COUNT],
            status: 0,
            control_latch: None,
            access_addr: 0,
            access_mode: AccessMode::VramWrite,
            read_buffer: 0,
            frame_buffer: vec![0; FRAME_WIDTH * FRAME_HEIGHT * 3],
        };
        vdp.render_frame();
        vdp
    }
}

fn sms_cram_to_rgb(value: u8) -> (u8, u8, u8) {
    let r = (value & 0x03) * 85;
    let g = ((value >> 2) & 0x03) * 85;
    let b = ((value >> 4) & 0x03) * 85;
    (r, g, b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mode4_renders_background_tile_with_sms_cram_color() {
        let mut vdp = Vdp::new();
        vdp.registers[0] = 0x04;
        vdp.registers[1] = 0x40;
        vdp.registers[2] = 0x0E;
        vdp.cram[1] = 0x03;
        vdp.vram[0] = 0x80;
        vdp.vram[0x3800] = 0;
        vdp.vram[0x3801] = 0;

        vdp.render_frame();

        assert_eq!(&vdp.frame_buffer()[0..3], &[255, 0, 0]);
    }

    #[test]
    fn control_port_selects_cram_write_mode() {
        let mut vdp = Vdp::new();

        vdp.write_control_port(0);
        vdp.write_control_port(0xC0);
        vdp.write_data_port(0x3F);

        assert_eq!(vdp.cram[0], 0x3F);
    }

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

        for tile_offset in [0, 1, 2, 3] {
            vdp.vram[pattern_base + (pattern + tile_offset) * 8] = 0x80;
        }

        assert!(vdp.sprite_pattern_bit(pattern_base, pattern, 16, 0, 0));
        assert!(vdp.sprite_pattern_bit(pattern_base, pattern, 16, 0, 8));
        assert!(vdp.sprite_pattern_bit(pattern_base, pattern, 16, 8, 0));
        assert!(vdp.sprite_pattern_bit(pattern_base, pattern, 16, 8, 8));
    }
}
