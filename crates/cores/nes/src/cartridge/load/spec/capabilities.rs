use super::MapperSpec;

const MMC3_FAMILY_MAPPERS: &[u16] = &[
    4, 12, 37, 44, 47, 64, 74, 76, 88, 95, 112, 114, 115, 118, 119, 123, 154, 182, 189, 191, 192,
    194, 195, 205, 206, 208, 245, 248, 250,
];

impl MapperSpec {
    pub(in crate::cartridge::load) fn uses_mmc1(self) -> bool {
        self.mapper == 1
    }

    pub(in crate::cartridge::load) fn uses_mmc2(self) -> bool {
        matches!(self.mapper, 9 | 10)
    }

    pub(in crate::cartridge::load) fn uses_mmc3(self) -> bool {
        MMC3_FAMILY_MAPPERS.contains(&self.mapper)
    }

    pub(in crate::cartridge::load) fn uses_mmc5(self) -> bool {
        self.mapper == 5
    }

    pub(in crate::cartridge::load) fn uses_namco163(self) -> bool {
        self.mapper == 19
    }

    pub(in crate::cartridge::load) fn uses_namco210(self) -> bool {
        self.mapper == 210
    }

    pub(in crate::cartridge::load) fn namco210_hardwired_mirroring(self) -> bool {
        !self.has_battery
    }

    pub(in crate::cartridge::load) fn uses_jaleco_ss88006(self) -> bool {
        self.mapper == 18
    }

    pub(in crate::cartridge::load) fn uses_vrc2_vrc4(self) -> bool {
        matches!(self.mapper, 21 | 22 | 23 | 25)
    }

    pub(in crate::cartridge::load) fn vrc2_vrc4_starts_in_vrc4_mode(self) -> bool {
        self.mapper == 21
    }

    pub(in crate::cartridge::load) fn uses_fme7(self) -> bool {
        self.mapper == 69
    }

    pub(in crate::cartridge::load) fn uses_mapper40(self) -> bool {
        self.mapper == 40
    }

    pub(in crate::cartridge::load) fn uses_mapper42(self) -> bool {
        self.mapper == 42
    }

    pub(in crate::cartridge::load) fn uses_mapper43(self) -> bool {
        self.mapper == 43
    }

    pub(in crate::cartridge::load) fn uses_mapper50(self) -> bool {
        self.mapper == 50
    }

    pub(in crate::cartridge::load) fn uses_bandai_fcg(self) -> bool {
        matches!(self.mapper, 16 | 153 | 159)
    }

    pub(in crate::cartridge::load) fn uses_irem_g101(self) -> bool {
        self.mapper == 32
    }

    pub(in crate::cartridge::load) fn uses_irem_h3001(self) -> bool {
        self.mapper == 65
    }

    pub(in crate::cartridge::load) fn uses_vrc1(self) -> bool {
        matches!(self.mapper, 75 | 151)
    }

    pub(in crate::cartridge::load) fn uses_vrc3(self) -> bool {
        matches!(self.mapper, 73 | 142)
    }

    pub(in crate::cartridge::load) fn uses_vrc6(self) -> bool {
        matches!(self.mapper, 24 | 26)
    }

    pub(in crate::cartridge::load) fn uses_mapper15(self) -> bool {
        self.mapper == 15
    }

    pub(in crate::cartridge::load) fn uses_taito_tc0190(self) -> bool {
        matches!(self.mapper, 33 | 48)
    }

    pub(in crate::cartridge::load) fn uses_taito_x1005(self) -> bool {
        matches!(self.mapper, 80 | 207)
    }

    pub(in crate::cartridge::load) fn uses_taito_x1017(self) -> bool {
        self.mapper == 82
    }

    pub(in crate::cartridge::load) fn uses_mapper246(self) -> bool {
        self.mapper == 246
    }

    pub(in crate::cartridge::load) fn uses_sunsoft4(self) -> bool {
        self.mapper == 68
    }

    pub(in crate::cartridge::load) fn uses_sunsoft3(self) -> bool {
        self.mapper == 67
    }
}
