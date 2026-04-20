use super::{Ppu, PpuMask, PpuStatus, PALETTE_COLORS};

const EMPHASIS_DIM_NUMERATOR: u16 = 3;
const EMPHASIS_DIM_DENOMINATOR: u16 = 4;

impl Ppu {
    #[inline]
    pub(super) fn render_pixel(&mut self, cartridge: Option<&crate::cartridge::Cartridge>) {
        let x = self.cycle - 1;
        let y = self.scanline;

        if x >= 256 || !(0..240).contains(&y) {
            return;
        }

        let (bg_color, bg_pixel) = self.render_background_pixel(x, y, cartridge);

        let mut sprite_result = None;
        let mut sprite_0_hit = false;

        if self.scanline_sprite_enable {
            if !self.scanline_sprite_left && x < 8 {
                // Skip sprite rendering in left 8 pixels
            } else {
                sprite_result = self.render_sprites(x as u8, y as u8, cartridge, &mut sprite_0_hit);

                if sprite_0_hit && bg_pixel != 0 {
                    self.status.insert(PpuStatus::SPRITE_0_HIT);
                }
            }
        }

        let final_color = if let Some((sprite_color, priority_behind_bg)) = sprite_result {
            if priority_behind_bg && bg_pixel != 0 {
                bg_color
            } else {
                sprite_color
            }
        } else {
            bg_color
        };

        let pixel_index = ((y as usize * 256) + x as usize) * 3;

        let mut masked_color = final_color & 0x3F;
        if self.scanline_grayscale {
            masked_color &= 0x30;
        }
        let color = Self::apply_color_emphasis(
            PALETTE_COLORS[masked_color as usize],
            self.scanline_color_emphasis,
        );
        // Safety: x is 0..255 and y is 0..239 (guarded above), buffer is 256*240*3
        let dest = &mut self.buffer[pixel_index..pixel_index + 3];
        dest[0] = color.0;
        dest[1] = color.1;
        dest[2] = color.2;
    }

    #[inline]
    fn apply_color_emphasis(color: (u8, u8, u8), emphasis: u8) -> (u8, u8, u8) {
        let (mut red, mut green, mut blue) = color;

        if emphasis & PpuMask::EMPHASIZE_RED.bits() != 0 {
            green = Self::dim_emphasis_channel(green);
            blue = Self::dim_emphasis_channel(blue);
        }
        if emphasis & PpuMask::EMPHASIZE_GREEN.bits() != 0 {
            red = Self::dim_emphasis_channel(red);
            blue = Self::dim_emphasis_channel(blue);
        }
        if emphasis & PpuMask::EMPHASIZE_BLUE.bits() != 0 {
            red = Self::dim_emphasis_channel(red);
            green = Self::dim_emphasis_channel(green);
        }

        (red, green, blue)
    }

    #[inline]
    fn dim_emphasis_channel(channel: u8) -> u8 {
        ((channel as u16 * EMPHASIS_DIM_NUMERATOR) / EMPHASIS_DIM_DENOMINATOR) as u8
    }
}
