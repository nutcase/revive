#![allow(static_mut_refs)]

use super::Ppu;

impl Ppu {
    pub(crate) fn update_cgram_rgb_cache(&mut self, index: u8) {
        let addr = (index as usize) * 2;
        if addr + 1 >= self.cgram.len() {
            self.cgram_rgb_cache[index as usize] = 0xFF000000;
            return;
        }

        let lo = self.cgram[addr];
        let hi = self.cgram[addr + 1] & 0x7F;
        let color = ((hi as u16) << 8) | (lo as u16);

        let r5 = (color & 0x001F) as u32;
        let g5 = ((color >> 5) & 0x001F) as u32;
        let b5 = ((color >> 10) & 0x001F) as u32;
        let r = (r5 << 3) | (r5 >> 2);
        let g = (g5 << 3) | (g5 >> 2);
        let b = (b5 << 3) | (b5 >> 2);
        self.cgram_rgb_cache[index as usize] = 0xFF000000 | (r << 16) | (g << 8) | b;
    }

    pub(crate) fn rebuild_cgram_rgb_cache(&mut self) {
        for i in 0..=255u8 {
            self.update_cgram_rgb_cache(i);
        }
    }

    pub(crate) fn cgram_to_rgb(&self, index: u8) -> u32 {
        // Fast path: cached RGB for the palette index.
        let cached = self.cgram_rgb_cache[index as usize];

        // Optional debug: log raw CGRAM values for early reads.
        if crate::debug_flags::debug_cgram_read() && !crate::debug_flags::quiet() {
            let addr = (index as usize) * 2;
            if addr + 1 < self.cgram.len() {
                let lo = self.cgram[addr];
                let hi = self.cgram[addr + 1] & 0x7F;
                let color = ((hi as u16) << 8) | (lo as u16);
                static mut CGRAM_ACCESS_COUNT: [u32; 256] = [0; 256];
                unsafe {
                    CGRAM_ACCESS_COUNT[index as usize] += 1;
                    if CGRAM_ACCESS_COUNT[index as usize] <= 3
                        && (index <= 16 || index == 0 || color != 0)
                    {
                        let r = color & 0x1F;
                        let g = (color >> 5) & 0x1F;
                        let b = (color >> 10) & 0x1F;
                        println!(
                            "🎨 CGRAM[{}]: color=0x{:04X} RGB555=({},{},{}) RGB888=0x{:02X}{:02X}{:02X}",
                            index,
                            color,
                            r,
                            g,
                            b,
                            ((r << 3) | (r >> 2)) as u8,
                            ((g << 3) | (g >> 2)) as u8,
                            ((b << 3) | (b >> 2)) as u8
                        );
                    }
                }
            }
        }

        cached
    }

    // 透明ピクセルのチョック
    pub(crate) fn is_transparent_pixel(&self, color: u32) -> bool {
        color == 0
    }

    // BGパレットとスプライトパレットの区別
    pub(crate) fn get_bg_palette_index(&self, palette: u8, color_index: u8, bpp: u8) -> u8 {
        match bpp {
            2 => palette * 4 + color_index,  // 2bpp: 4色/パレット
            4 => palette * 16 + color_index, // 4bpp: 16色/パレット
            8 => color_index,                // 8bpp: 直接インデックス
            _ => 0,
        }
    }

    pub(crate) fn get_sprite_palette_index(&self, palette: u8, color_index: u8, bpp: u8) -> u8 {
        match bpp {
            2 => 128 + palette * 4 + color_index, // スプライトは128番以降
            4 => 128 + palette * 16 + color_index,
            8 => 128 + color_index,
            _ => 128,
        }
    }

    #[allow(dead_code)]
    pub fn write_cgram_color(&mut self, color_index: u8, rgb15: u16) {
        let offset = (color_index as usize) * 2;
        if offset + 1 < self.cgram.len() {
            self.cgram[offset] = (rgb15 & 0xFF) as u8;
            self.cgram[offset + 1] = ((rgb15 >> 8) & 0xFF) as u8;
            self.update_cgram_rgb_cache(color_index);
        }
    }
}
