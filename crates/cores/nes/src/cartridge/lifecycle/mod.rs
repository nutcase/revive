mod reset;

use super::{Cartridge, Mirroring};

impl Cartridge {
    #[inline]
    pub(crate) fn is_nrom(&self) -> bool {
        self.mapper == 0
    }

    pub fn mirroring(&self) -> Mirroring {
        self.mirroring
    }

    pub(crate) fn uses_mmc2_latches(&self) -> bool {
        matches!(self.mapper, 9 | 10)
    }

    pub(crate) fn uses_mmc5(&self) -> bool {
        self.mapper == 5
    }

    pub(in crate::cartridge) fn uses_namco163(&self) -> bool {
        self.mapper == 19
    }

    pub(in crate::cartridge) fn uses_vrc6(&self) -> bool {
        matches!(self.mapper, 24 | 26)
    }

    pub(in crate::cartridge) fn uses_vrc7(&self) -> bool {
        self.mapper == 85
    }

    pub(in crate::cartridge) fn uses_vrc1(&self) -> bool {
        matches!(self.mapper, 75 | 151)
    }

    pub(in crate::cartridge) fn uses_taito_tc0190(&self) -> bool {
        matches!(self.mapper, 33 | 48)
    }

    pub(in crate::cartridge) fn uses_namco108(&self) -> bool {
        matches!(self.mapper, 76 | 88 | 95 | 112 | 154 | 206)
    }

    pub(in crate::cartridge) fn uses_taito_x1005(&self) -> bool {
        matches!(self.mapper, 80 | 207)
    }

    pub(in crate::cartridge) fn uses_taito_x1017(&self) -> bool {
        self.mapper == 82
    }

    pub(in crate::cartridge) fn uses_bandai_fcg(&self) -> bool {
        matches!(self.mapper, 16 | 153 | 159)
    }

    pub(in crate::cartridge) fn uses_fme7(&self) -> bool {
        self.mapper == 69
    }

    pub(in crate::cartridge) fn uses_mmc3_chr_banks(&self) -> bool {
        matches!(self.mapper, 4 | 118 | 123 | 189 | 208 | 250)
    }

    pub(in crate::cartridge) fn uses_mmc3_prg_ram(&self) -> bool {
        matches!(self.mapper, 4 | 74 | 118 | 119 | 192 | 194 | 245 | 250)
    }

    pub(in crate::cartridge) fn uses_mapper114_variant(&self) -> bool {
        matches!(self.mapper, 114 | 182)
    }

    pub(in crate::cartridge) fn uses_mapper115_variant(&self) -> bool {
        matches!(self.mapper, 115 | 248)
    }

    pub(in crate::cartridge) fn uses_linear_prg_ram(&self) -> bool {
        (self.mapper == 34 && self.mappers.simple.mapper34_nina001)
            || matches!(self.mapper, 227 | 240 | 241)
    }

    pub(in crate::cartridge) fn uses_mapper48(&self) -> bool {
        self.mapper == 48
    }

    pub(in crate::cartridge) fn uses_mapper234_read_latch(&self) -> bool {
        self.mapper == 234
    }
}
