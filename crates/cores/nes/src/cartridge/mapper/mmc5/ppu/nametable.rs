use super::super::super::super::Cartridge;

impl Cartridge {
    fn mmc5_fill_attribute(&self) -> u8 {
        let Some(mmc5) = self.mappers.mmc5.as_ref() else {
            return 0;
        };
        let attr = mmc5.fill_attr & 0x03;
        attr | (attr << 2) | (attr << 4) | (attr << 6)
    }

    fn mmc5_exram_palette_attr(&self) -> u8 {
        let Some(mmc5) = self.mappers.mmc5.as_ref() else {
            return 0;
        };
        let palette = mmc5.cached_ext_palette.get() & 0x03;
        let tile_x = mmc5.cached_tile_x.get() as usize;
        let tile_y = mmc5.cached_tile_y.get() as usize;
        let block_x = (tile_x & 3) >> 1;
        let block_y = (tile_y & 3) >> 1;
        let shift = (block_y * 2 + block_x) * 2;
        palette << shift
    }

    pub(in crate::cartridge) fn read_nametable_mmc5(
        &self,
        logical_nt: usize,
        offset: usize,
        internal: &[[u8; 1024]; 2],
    ) -> u8 {
        let Some(mmc5) = self.mappers.mmc5.as_ref() else {
            return 0;
        };

        let source = mmc5.nametable_map[logical_nt & 3] & 0x03;
        if offset < 960 {
            let tile_x = (offset & 31) as u8;
            let tile_y = (offset / 32) as u8;
            mmc5.cached_tile_x.set(tile_x);
            mmc5.cached_tile_y.set(tile_y);

            let tile = match source {
                0 => internal[0][offset],
                1 => internal[1][offset],
                2 if mmc5.exram_mode <= 0x01 => mmc5.exram[offset],
                3 => mmc5.fill_tile,
                _ => 0,
            };

            if mmc5.substitutions_enabled() && mmc5.exram_mode == 0x01 {
                let exattr = mmc5.exram[offset];
                mmc5.cached_ext_bank.set(exattr & 0x3F);
                mmc5.cached_ext_palette.set((exattr >> 6) & 0x03);
            } else {
                mmc5.cached_ext_bank.set(0);
                mmc5.cached_ext_palette.set(0);
            }

            tile
        } else {
            if mmc5.substitutions_enabled() && mmc5.exram_mode == 0x01 {
                return self.mmc5_exram_palette_attr();
            }
            match source {
                0 => internal[0][offset],
                1 => internal[1][offset],
                2 if mmc5.exram_mode <= 0x01 => mmc5.exram[offset],
                3 => self.mmc5_fill_attribute(),
                _ => 0,
            }
        }
    }

    pub(in crate::cartridge) fn write_nametable_mmc5(
        &mut self,
        logical_nt: usize,
        offset: usize,
        internal: &mut [[u8; 1024]; 2],
        data: u8,
    ) {
        let Some(mmc5) = self.mappers.mmc5.as_mut() else {
            return;
        };

        match mmc5.nametable_map[logical_nt & 3] & 0x03 {
            0 => internal[0][offset] = data,
            1 => internal[1][offset] = data,
            2 if mmc5.exram_mode != 0x03 => {
                if let Some(slot) = mmc5.exram.get_mut(offset) {
                    *slot = data;
                }
            }
            _ => {}
        }
    }

    pub(in crate::cartridge) fn resolve_nametable_mmc5(&self, logical_nt: usize) -> usize {
        logical_nt & 3
    }
}
