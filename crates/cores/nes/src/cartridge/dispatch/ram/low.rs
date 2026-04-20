use super::*;

#[derive(Clone, Copy)]
enum PrgLowReadHandler {
    Namco163,
    Mmc5,
    Mapper43,
    Mapper137,
    Mapper150,
    Mapper208,
    Mapper225,
    Mapper243,
    None,
}

#[derive(Clone, Copy)]
enum PrgLowWriteHandler {
    Mapper99,
    Mmc5,
    Namco163,
    None,
}

impl Cartridge {
    pub fn read_prg_low(&self, addr: u16) -> u8 {
        match self.prg_low_read_handler() {
            PrgLowReadHandler::Namco163 => self.read_prg_low_namco163(addr),
            PrgLowReadHandler::Mmc5 => self.read_prg_low_mmc5(addr),
            PrgLowReadHandler::Mapper43 => self.read_prg_low_mapper43(addr),
            PrgLowReadHandler::Mapper137 => self.read_prg_low_mapper137(addr),
            PrgLowReadHandler::Mapper150 => self.read_prg_low_mapper150(addr),
            PrgLowReadHandler::Mapper208 => self.read_prg_low_mapper208(addr),
            PrgLowReadHandler::Mapper225 => self.read_prg_low_mapper225(addr),
            PrgLowReadHandler::Mapper243 => self.read_prg_low_mapper243(addr),
            PrgLowReadHandler::None => 0,
        }
    }

    pub fn write_prg_low(&mut self, addr: u16, data: u8) {
        match self.prg_low_write_handler() {
            PrgLowWriteHandler::Mapper99 => self.write_prg_low_mapper99(addr, data),
            PrgLowWriteHandler::Mmc5 => self.write_prg_mmc5(addr, data),
            PrgLowWriteHandler::Namco163 => self.write_prg_low_namco163(addr, data),
            PrgLowWriteHandler::None => {}
        }
    }

    fn prg_low_read_handler(&self) -> PrgLowReadHandler {
        match self.mapper {
            19 => PrgLowReadHandler::Namco163,
            5 => PrgLowReadHandler::Mmc5,
            43 => PrgLowReadHandler::Mapper43,
            137 => PrgLowReadHandler::Mapper137,
            150 => PrgLowReadHandler::Mapper150,
            208 => PrgLowReadHandler::Mapper208,
            225 => PrgLowReadHandler::Mapper225,
            243 => PrgLowReadHandler::Mapper243,
            _ => PrgLowReadHandler::None,
        }
    }

    fn prg_low_write_handler(&self) -> PrgLowWriteHandler {
        match self.mapper {
            99 => PrgLowWriteHandler::Mapper99,
            5 => PrgLowWriteHandler::Mmc5,
            19 => PrgLowWriteHandler::Namco163,
            _ => PrgLowWriteHandler::None,
        }
    }
}
