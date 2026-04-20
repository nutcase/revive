use super::super::Cartridge;

pub(super) enum NametableReadHandler {
    Standard,
    Namco163,
    Mmc5,
    Mapper77,
    Mapper99,
}

pub(super) enum NametableResolveHandler {
    Standard,
    FourScreenAlias,
    Mmc5,
    Mapper137,
    Mapper118,
    Mapper207,
}

impl Cartridge {
    pub(super) fn nametable_read_handler(&self) -> NametableReadHandler {
        if self.uses_namco163() {
            NametableReadHandler::Namco163
        } else if self.uses_mmc5() {
            NametableReadHandler::Mmc5
        } else {
            match self.mapper {
                77 => NametableReadHandler::Mapper77,
                99 => NametableReadHandler::Mapper99,
                _ => NametableReadHandler::Standard,
            }
        }
    }

    pub(super) fn nametable_resolve_handler(&self) -> NametableResolveHandler {
        match self.mapper {
            77 | 99 => NametableResolveHandler::FourScreenAlias,
            137 => NametableResolveHandler::Mapper137,
            118 => NametableResolveHandler::Mapper118,
            207 => NametableResolveHandler::Mapper207,
            _ if self.uses_namco163() => NametableResolveHandler::FourScreenAlias,
            _ if self.uses_mmc5() => NametableResolveHandler::Mmc5,
            _ => NametableResolveHandler::Standard,
        }
    }
}
