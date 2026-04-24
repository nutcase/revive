pub const FRAME_WIDTH: usize = 256;
pub const FRAME_HEIGHT: usize = 192;
pub const VRAM_SIZE: usize = 0x4000;
pub const STATUS_SPRITE_OVERFLOW: u8 = 0x40;
pub const STATUS_SPRITE_COLLISION: u8 = 0x20;

pub fn render_frame(frame_buffer: &mut [u8], vram: &[u8; VRAM_SIZE], registers: &[u8]) -> u8 {
    let backdrop = backdrop_color(registers);
    fill_frame(frame_buffer, backdrop);

    if !display_enabled(registers) {
        return 0;
    }

    let mode1 = (registers[1] & 0x10) != 0;
    let mode2 = (registers[1] & 0x08) != 0;
    let mode3 = (registers[0] & 0x02) != 0;
    match (mode1, mode2, mode3) {
        (true, false, false) => render_text_mode(frame_buffer, vram, registers),
        (false, true, false) => render_multicolor_mode(frame_buffer, vram, registers),
        (false, false, true) => render_graphics_ii_mode(frame_buffer, vram, registers),
        _ => render_graphics_i_mode(frame_buffer, vram, registers),
    }

    render_sprites(frame_buffer, vram, registers)
}

pub fn tms_color_to_rgb(index: u8, backdrop: (u8, u8, u8)) -> (u8, u8, u8) {
    if index == 0 {
        return backdrop;
    }
    tms_palette_color(index)
}

pub fn tms_palette_color(index: u8) -> (u8, u8, u8) {
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

pub fn sprite_pattern_bit(
    vram: &[u8; VRAM_SIZE],
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
    let byte = vram[(pattern_base + tile * 8 + row) % VRAM_SIZE];
    ((byte >> (7 - col)) & 1) != 0
}

fn display_enabled(registers: &[u8]) -> bool {
    (registers[1] & 0x40) != 0
}

fn backdrop_color(registers: &[u8]) -> (u8, u8, u8) {
    tms_palette_color(registers[7] & 0x0F)
}

fn fill_frame(frame_buffer: &mut [u8], color: (u8, u8, u8)) {
    for pixel in frame_buffer.chunks_exact_mut(3) {
        pixel[0] = color.0;
        pixel[1] = color.1;
        pixel[2] = color.2;
    }
}

fn render_graphics_i_mode(frame_buffer: &mut [u8], vram: &[u8; VRAM_SIZE], registers: &[u8]) {
    let backdrop = backdrop_color(registers);
    let name_base = ((registers[2] as usize) & 0x0F) << 10;
    let color_base = (registers[3] as usize) << 6;
    let pattern_base = ((registers[4] as usize) & 0x07) << 11;

    for y in 0..FRAME_HEIGHT {
        let tile_y = y / 8;
        let row = y & 7;
        for x in 0..FRAME_WIDTH {
            let tile_x = x / 8;
            let col = x & 7;
            let tile = vram[(name_base + tile_y * 32 + tile_x) % VRAM_SIZE] as usize;
            let pattern = vram[(pattern_base + tile * 8 + row) % VRAM_SIZE];
            let color = vram[(color_base + tile / 8) % VRAM_SIZE];
            let fg = color >> 4;
            let bg = color & 0x0F;
            let bit = (pattern >> (7 - col)) & 1;
            let color_index = if bit != 0 { fg } else { bg };
            set_pixel(frame_buffer, x, y, tms_color_to_rgb(color_index, backdrop));
        }
    }
}

fn render_graphics_ii_mode(frame_buffer: &mut [u8], vram: &[u8; VRAM_SIZE], registers: &[u8]) {
    let backdrop = backdrop_color(registers);
    let name_base = ((registers[2] as usize) & 0x0F) << 10;
    let color_base = ((registers[3] as usize) & 0x80) << 6;
    let pattern_base = ((registers[4] as usize) & 0x04) << 11;

    for y in 0..FRAME_HEIGHT {
        let page = y / 64;
        let tile_y = y / 8;
        let row = y & 7;
        for x in 0..FRAME_WIDTH {
            let tile_x = x / 8;
            let col = x & 7;
            let name = vram[(name_base + tile_y * 32 + tile_x) % VRAM_SIZE] as usize;
            let tile = page * 256 + name;
            let pattern = vram[(pattern_base + tile * 8 + row) % VRAM_SIZE];
            let color = vram[(color_base + tile * 8 + row) % VRAM_SIZE];
            let fg = color >> 4;
            let bg = color & 0x0F;
            let bit = (pattern >> (7 - col)) & 1;
            let color_index = if bit != 0 { fg } else { bg };
            set_pixel(frame_buffer, x, y, tms_color_to_rgb(color_index, backdrop));
        }
    }
}

fn render_text_mode(frame_buffer: &mut [u8], vram: &[u8; VRAM_SIZE], registers: &[u8]) {
    let backdrop = backdrop_color(registers);
    let name_base = ((registers[2] as usize) & 0x0F) << 10;
    let pattern_base = ((registers[4] as usize) & 0x07) << 11;
    let fg = (registers[7] >> 4) & 0x0F;
    let bg = registers[7] & 0x0F;
    let x_offset = 8;

    for y in 0..FRAME_HEIGHT {
        let tile_y = y / 8;
        let row = y & 7;
        for cell_x in 0..40 {
            let tile = vram[(name_base + tile_y * 40 + cell_x) % VRAM_SIZE] as usize;
            let pattern = vram[(pattern_base + tile * 8 + row) % VRAM_SIZE];
            for col in 0..6 {
                let bit = (pattern >> (7 - col)) & 1;
                let color_index = if bit != 0 { fg } else { bg };
                set_pixel(
                    frame_buffer,
                    x_offset + cell_x * 6 + col,
                    y,
                    tms_color_to_rgb(color_index, backdrop),
                );
            }
        }
    }
}

fn render_multicolor_mode(frame_buffer: &mut [u8], vram: &[u8; VRAM_SIZE], registers: &[u8]) {
    let backdrop = backdrop_color(registers);
    let name_base = ((registers[2] as usize) & 0x0F) << 10;
    let pattern_base = ((registers[4] as usize) & 0x07) << 11;

    for y in 0..FRAME_HEIGHT {
        let tile_y = y / 8;
        let block_row = (y & 7) / 4;
        for x in 0..FRAME_WIDTH {
            let tile_x = x / 8;
            let block_col = (x & 7) / 4;
            let tile = vram[(name_base + tile_y * 32 + tile_x) % VRAM_SIZE] as usize;
            let color_byte =
                vram[(pattern_base + tile * 8 + block_row * 2 + block_col) % VRAM_SIZE];
            let color_index = if block_col == 0 {
                color_byte >> 4
            } else {
                color_byte & 0x0F
            };
            set_pixel(frame_buffer, x, y, tms_color_to_rgb(color_index, backdrop));
        }
    }
}

fn render_sprites(frame_buffer: &mut [u8], vram: &[u8; VRAM_SIZE], registers: &[u8]) -> u8 {
    let backdrop = backdrop_color(registers);
    let sat_base = ((registers[5] as usize) & 0x7F) << 7;
    let pattern_base = ((registers[6] as usize) & 0x07) << 11;
    let sprites_16x16 = (registers[1] & 0x02) != 0;
    let magnify = (registers[1] & 0x01) != 0;
    let base_size = if sprites_16x16 { 16 } else { 8 };
    let drawn_size = if magnify { base_size * 2 } else { base_size };
    let mut occupied = vec![false; FRAME_WIDTH * FRAME_HEIGHT];
    let mut status = 0;

    for i in 0..32 {
        let addr = sat_base + i * 4;
        let y_byte = vram[addr % VRAM_SIZE];
        if y_byte == 0xD0 {
            break;
        }
        let sprite_y = y_byte.wrapping_add(1) as i16;
        let mut sprite_x = vram[(addr + 1) % VRAM_SIZE] as i16;
        let mut pattern = vram[(addr + 2) % VRAM_SIZE] as usize;
        let color = vram[(addr + 3) % VRAM_SIZE];
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
                if !sprite_pattern_bit(vram, pattern_base, pattern, base_size, src_x, src_y) {
                    continue;
                }
                let index = dy as usize * FRAME_WIDTH + dx as usize;
                if occupied[index] {
                    status |= STATUS_SPRITE_COLLISION;
                    continue;
                }
                set_pixel(
                    frame_buffer,
                    dx as usize,
                    dy as usize,
                    tms_color_to_rgb(color_index, backdrop),
                );
                occupied[index] = true;
            }
        }
    }

    status
}

fn set_pixel(frame_buffer: &mut [u8], x: usize, y: usize, color: (u8, u8, u8)) {
    if x >= FRAME_WIDTH || y >= FRAME_HEIGHT {
        return;
    }
    let offset = (y * FRAME_WIDTH + x) * 3;
    frame_buffer[offset] = color.0;
    frame_buffer[offset + 1] = color.1;
    frame_buffer[offset + 2] = color.2;
}
