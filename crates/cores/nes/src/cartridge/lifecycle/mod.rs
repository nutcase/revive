mod reset;

use super::{Cartridge, Mirroring};

impl Cartridge {
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

    pub(in crate::cartridge) fn uses_mapper48(&self) -> bool {
        self.mapper == 48
    }

    pub(in crate::cartridge) fn uses_mapper234_read_latch(&self) -> bool {
        self.mapper == 234
    }
}
