use super::*;

impl Ppu {
    // rendering_enabled is now a cached field updated on $2001 write

    #[inline]
    pub(in crate::ppu) fn resolve_nametable(
        &self,
        logical_nt: usize,
        cartridge: Option<&crate::cartridge::Cartridge>,
    ) -> usize {
        if let Some(cart) = cartridge {
            if let Some(table) = cart.resolve_nametable(logical_nt) {
                return table;
            }
            match cart.mirroring() {
                crate::cartridge::Mirroring::Vertical => match logical_nt & 3 {
                    0 | 2 => 0,
                    1 | 3 => 1,
                    _ => 0,
                },
                crate::cartridge::Mirroring::Horizontal => match logical_nt & 3 {
                    0 | 1 => 0,
                    2 | 3 => 1,
                    _ => 0,
                },
                crate::cartridge::Mirroring::HorizontalSwapped => match logical_nt & 3 {
                    0 | 1 => 1,
                    2 | 3 => 0,
                    _ => 0,
                },
                crate::cartridge::Mirroring::ThreeScreenLower => match logical_nt & 3 {
                    0..=2 => 0,
                    3 => 1,
                    _ => 0,
                },
                crate::cartridge::Mirroring::FourScreen => logical_nt & 1,
                crate::cartridge::Mirroring::OneScreenLower => 0,
                crate::cartridge::Mirroring::OneScreenUpper => 1,
            }
        } else {
            logical_nt & 1
        }
    }

    #[inline]
    pub(in crate::ppu) fn read_nametable_byte(
        &self,
        physical_nt: usize,
        offset: usize,
        cartridge: Option<&crate::cartridge::Cartridge>,
    ) -> u8 {
        if offset >= 1024 {
            return 0;
        }

        if let Some(cart) = cartridge {
            cart.read_nametable_byte(physical_nt, offset, &self.nametable)
        } else {
            self.nametable[physical_nt & 1][offset]
        }
    }
}
