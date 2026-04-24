use crate::z80::Z80_CLOCK_HZ;

pub const FRAME_WIDTH: usize = 256;
pub const FRAME_HEIGHT: usize = 192;

const VRAM_SIZE: usize = 0x4000;
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

        let backdrop = self.backdrop_color();
        self.fill_frame(backdrop);

        if !self.display_enabled() {
            return;
        }

        let mode1 = (self.registers[1] & 0x10) != 0;
        let mode2 = (self.registers[1] & 0x08) != 0;
        let mode3 = (self.registers[0] & 0x02) != 0;
        match (mode1, mode2, mode3) {
            (true, false, false) => self.render_text_mode(),
            (false, true, false) => self.render_multicolor_mode(),
            (false, false, true) => self.render_graphics_ii_mode(),
            _ => self.render_graphics_i_mode(),
        }

        self.render_tms_sprites();
    }

    fn mode4_enabled(&self) -> bool {
        (self.registers[0] & 0x04) != 0
    }

    fn display_enabled(&self) -> bool {
        (self.registers[1] & 0x40) != 0
    }

    fn backdrop_color(&self) -> (u8, u8, u8) {
        tms_palette_color(self.registers[7] & 0x0F)
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

    fn render_graphics_i_mode(&mut self) {
        let backdrop = self.backdrop_color();
        let name_base = ((self.registers[2] as usize) & 0x0F) << 10;
        let color_base = (self.registers[3] as usize) << 6;
        let pattern_base = ((self.registers[4] as usize) & 0x07) << 11;

        for y in 0..FRAME_HEIGHT {
            let tile_y = y / 8;
            let row = y & 7;
            for x in 0..FRAME_WIDTH {
                let tile_x = x / 8;
                let col = x & 7;
                let tile = self.vram[(name_base + tile_y * 32 + tile_x) % VRAM_SIZE] as usize;
                let pattern = self.vram[(pattern_base + tile * 8 + row) % VRAM_SIZE];
                let color = self.vram[(color_base + tile / 8) % VRAM_SIZE];
                let fg = color >> 4;
                let bg = color & 0x0F;
                let bit = (pattern >> (7 - col)) & 1;
                let color_index = if bit != 0 { fg } else { bg };
                self.set_pixel(x, y, tms_color_to_rgb(color_index, backdrop));
            }
        }
    }

    fn render_graphics_ii_mode(&mut self) {
        let backdrop = self.backdrop_color();
        let name_base = ((self.registers[2] as usize) & 0x0F) << 10;
        let color_base = ((self.registers[3] as usize) & 0x80) << 6;
        let pattern_base = ((self.registers[4] as usize) & 0x04) << 11;

        for y in 0..FRAME_HEIGHT {
            let page = y / 64;
            let tile_y = y / 8;
            let row = y & 7;
            for x in 0..FRAME_WIDTH {
                let tile_x = x / 8;
                let col = x & 7;
                let name = self.vram[(name_base + tile_y * 32 + tile_x) % VRAM_SIZE] as usize;
                let tile = page * 256 + name;
                let pattern = self.vram[(pattern_base + tile * 8 + row) % VRAM_SIZE];
                let color = self.vram[(color_base + tile * 8 + row) % VRAM_SIZE];
                let fg = color >> 4;
                let bg = color & 0x0F;
                let bit = (pattern >> (7 - col)) & 1;
                let color_index = if bit != 0 { fg } else { bg };
                self.set_pixel(x, y, tms_color_to_rgb(color_index, backdrop));
            }
        }
    }

    fn render_text_mode(&mut self) {
        let backdrop = self.backdrop_color();
        let name_base = ((self.registers[2] as usize) & 0x0F) << 10;
        let pattern_base = ((self.registers[4] as usize) & 0x07) << 11;
        let fg = (self.registers[7] >> 4) & 0x0F;
        let bg = self.registers[7] & 0x0F;
        let x_offset = 8;

        for y in 0..FRAME_HEIGHT {
            let tile_y = y / 8;
            let row = y & 7;
            for cell_x in 0..40 {
                let tile = self.vram[(name_base + tile_y * 40 + cell_x) % VRAM_SIZE] as usize;
                let pattern = self.vram[(pattern_base + tile * 8 + row) % VRAM_SIZE];
                for col in 0..6 {
                    let bit = (pattern >> (7 - col)) & 1;
                    let color_index = if bit != 0 { fg } else { bg };
                    self.set_pixel(
                        x_offset + cell_x * 6 + col,
                        y,
                        tms_color_to_rgb(color_index, backdrop),
                    );
                }
            }
        }
    }

    fn render_multicolor_mode(&mut self) {
        let backdrop = self.backdrop_color();
        let name_base = ((self.registers[2] as usize) & 0x0F) << 10;
        let pattern_base = ((self.registers[4] as usize) & 0x07) << 11;

        for y in 0..FRAME_HEIGHT {
            let tile_y = y / 8;
            let block_row = (y & 7) / 4;
            for x in 0..FRAME_WIDTH {
                let tile_x = x / 8;
                let block_col = (x & 7) / 4;
                let tile = self.vram[(name_base + tile_y * 32 + tile_x) % VRAM_SIZE] as usize;
                let color_byte =
                    self.vram[(pattern_base + tile * 8 + block_row * 2 + block_col) % VRAM_SIZE];
                let color_index = if block_col == 0 {
                    color_byte >> 4
                } else {
                    color_byte & 0x0F
                };
                self.set_pixel(x, y, tms_color_to_rgb(color_index, backdrop));
            }
        }
    }

    fn render_tms_sprites(&mut self) {
        let backdrop = self.backdrop_color();
        let sat_base = ((self.registers[5] as usize) & 0x7F) << 7;
        let pattern_base = ((self.registers[6] as usize) & 0x07) << 11;
        let sprites_16x16 = (self.registers[1] & 0x02) != 0;
        let magnify = (self.registers[1] & 0x01) != 0;
        let base_size = if sprites_16x16 { 16 } else { 8 };
        let drawn_size = if magnify { base_size * 2 } else { base_size };
        let mut occupied = vec![false; FRAME_WIDTH * FRAME_HEIGHT];

        for i in 0..32 {
            let addr = sat_base + i * 4;
            let y_byte = self.vram[addr % VRAM_SIZE];
            if y_byte == 0xD0 {
                break;
            }
            let sprite_y = y_byte.wrapping_add(1) as i16;
            let mut sprite_x = self.vram[(addr + 1) % VRAM_SIZE] as i16;
            let mut pattern = self.vram[(addr + 2) % VRAM_SIZE] as usize;
            let color = self.vram[(addr + 3) % VRAM_SIZE];
            if color & 0x80 != 0 {
                sprite_x -= 32;
            }
            let color_index = color & 0x0F;
            if color_index == 0 {
                continue;
            }
            if sprites_16x16 {
                pattern &= !0x03;
            }

            for sy in 0..drawn_size {
                let src_y = if magnify { sy / 2 } else { sy };
                let dy = sprite_y + sy as i16;
                if !(0..FRAME_HEIGHT as i16).contains(&dy) {
                    continue;
                }
                for sx in 0..drawn_size {
                    let src_x = if magnify { sx / 2 } else { sx };
                    let dx = sprite_x + sx as i16;
                    if !(0..FRAME_WIDTH as i16).contains(&dx) {
                        continue;
                    }
                    if !self.sprite_pattern_bit(pattern_base, pattern, base_size, src_x, src_y) {
                        continue;
                    }
                    let index = dy as usize * FRAME_WIDTH + dx as usize;
                    if occupied[index] {
                        self.status |= STATUS_SPRITE_COLLISION;
                        continue;
                    }
                    self.set_pixel(
                        dx as usize,
                        dy as usize,
                        tms_color_to_rgb(color_index, backdrop),
                    );
                    occupied[index] = true;
                }
            }
        }
    }

    fn sprite_pattern_bit(
        &self,
        pattern_base: usize,
        pattern: usize,
        sprite_size: usize,
        x: usize,
        y: usize,
    ) -> bool {
        let tile_col = x / 8;
        let tile_row = y / 8;
        let tile = if sprite_size == 16 {
            pattern + tile_col * 2 + tile_row
        } else {
            pattern
        };
        let row = y & 7;
        let col = x & 7;
        let byte = self.vram[(pattern_base + tile * 8 + row) % VRAM_SIZE];
        ((byte >> (7 - col)) & 1) != 0
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

fn tms_color_to_rgb(index: u8, backdrop: (u8, u8, u8)) -> (u8, u8, u8) {
    if index == 0 {
        return backdrop;
    }
    tms_palette_color(index)
}

fn tms_palette_color(index: u8) -> (u8, u8, u8) {
    const PALETTE: [(u8, u8, u8); 16] = [
        (0, 0, 0),
        (0, 0, 0),
        (33, 200, 66),
        (94, 220, 120),
        (84, 85, 237),
        (125, 118, 252),
        (212, 82, 77),
        (66, 235, 245),
        (252, 85, 84),
        (255, 121, 120),
        (212, 193, 84),
        (230, 206, 128),
        (33, 176, 59),
        (201, 91, 186),
        (204, 204, 204),
        (255, 255, 255),
    ];
    PALETTE[index as usize & 0x0F]
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

        let backdrop = tms_palette_color(0x0C);
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
