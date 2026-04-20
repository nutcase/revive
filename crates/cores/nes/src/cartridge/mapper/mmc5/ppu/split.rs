use super::super::super::super::Cartridge;

impl Cartridge {
    fn mmc5_split_palette(&self, attr_byte: u8, tile_x: usize, tile_y: usize) -> u8 {
        let block_x = (tile_x & 3) >> 1;
        let block_y = (tile_y & 3) >> 1;
        let shift = (block_y * 2 + block_x) * 2;
        (attr_byte >> shift) & 0x03
    }

    pub(crate) fn mmc5_split_bg_fetch(
        &self,
        screen_x: u8,
        screen_y: u8,
        fine_x: u8,
    ) -> Option<(u8, u8, u8)> {
        let mmc5 = self.mappers.mmc5.as_ref()?;
        if !mmc5.split_enabled() {
            return None;
        }

        let tile_fetch = (screen_x as usize + fine_x as usize) >> 3;
        let threshold = mmc5.split_threshold_tiles();
        let in_split = if mmc5.split_on_right() {
            tile_fetch >= threshold
        } else {
            tile_fetch < threshold
        };
        if !in_split {
            return None;
        }

        let tile_x = (screen_x as usize) >> 3;
        let split_y = screen_y as usize + mmc5.split_scroll as usize;
        let tile_y = (split_y >> 3) % 30;
        let fine_y = split_y & 0x07;
        let tile_offset = tile_y * 32 + tile_x;
        let tile_id = mmc5.exram.get(tile_offset).copied().unwrap_or(0);
        let attr_offset = 960 + ((tile_y >> 2) << 3) + (tile_x >> 2);
        let attr_byte = mmc5.exram.get(attr_offset).copied().unwrap_or(0);
        let palette = self.mmc5_split_palette(attr_byte, tile_x, tile_y);
        let chr_addr = (mmc5.split_bank as usize * 0x1000) + (tile_id as usize * 16) + fine_y;
        let low = self.read_mmc5_chr_1k(chr_addr >> 10, chr_addr & 0x03FF);
        let high = self.read_mmc5_chr_1k((chr_addr + 8) >> 10, (chr_addr + 8) & 0x03FF);
        Some((low, high, palette))
    }
}
