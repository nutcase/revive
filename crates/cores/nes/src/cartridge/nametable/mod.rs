mod chr_ram;
mod resolve;
mod routing;

use super::Cartridge;
use routing::{NametableReadHandler, NametableResolveHandler};

impl Cartridge {
    pub fn read_nametable_byte(
        &self,
        physical_nt: usize,
        offset: usize,
        internal: &[[u8; 1024]; 2],
    ) -> u8 {
        if offset >= 1024 {
            return 0;
        }

        match self.nametable_read_handler() {
            NametableReadHandler::Namco163 => {
                self.read_nametable_namco163(physical_nt, offset, internal)
            }
            NametableReadHandler::Mmc5 => self.read_nametable_mmc5(physical_nt, offset, internal),
            NametableReadHandler::Mapper77 => {
                self.read_nametable_mapper77(physical_nt, offset, internal)
            }
            NametableReadHandler::Mapper99 => self.read_nametable_mapper99(physical_nt, offset),
            NametableReadHandler::Standard => {
                self.read_standard_nametable(physical_nt, offset, internal)
            }
        }
    }

    pub fn write_nametable_byte(
        &mut self,
        physical_nt: usize,
        offset: usize,
        internal: &mut [[u8; 1024]; 2],
        data: u8,
    ) {
        if offset >= 1024 {
            return;
        }

        match self.nametable_read_handler() {
            NametableReadHandler::Namco163 => {
                self.write_nametable_namco163(physical_nt, offset, internal, data)
            }
            NametableReadHandler::Mmc5 => {
                self.write_nametable_mmc5(physical_nt, offset, internal, data)
            }
            NametableReadHandler::Mapper77 => {
                self.write_nametable_mapper77(physical_nt, offset, internal, data)
            }
            NametableReadHandler::Mapper99 => {
                self.write_nametable_mapper99(physical_nt, offset, data)
            }
            NametableReadHandler::Standard => {
                self.write_standard_nametable(physical_nt, offset, internal, data)
            }
        }
    }

    pub fn resolve_nametable(&self, logical_nt: usize) -> Option<usize> {
        match self.nametable_resolve_handler() {
            NametableResolveHandler::FourScreenAlias => Some(logical_nt & 3),
            NametableResolveHandler::Mmc5 => Some(self.resolve_nametable_mmc5(logical_nt)),
            NametableResolveHandler::Mapper137 => self.resolve_nametable_mapper137(logical_nt),
            NametableResolveHandler::Mapper118 => self.resolve_nametable_mapper118(logical_nt),
            NametableResolveHandler::Mapper207 => self.resolve_nametable_mapper207(logical_nt),
            NametableResolveHandler::Standard => None,
        }
    }

    #[cfg(test)]
    pub fn nametable_writes_to_internal_vram(&self) -> bool {
        self.mappers
            .sunsoft4
            .as_ref()
            .map(|sunsoft4| !sunsoft4.nametable_chr_rom)
            .unwrap_or(true)
    }

    fn read_standard_nametable(
        &self,
        physical_nt: usize,
        offset: usize,
        internal: &[[u8; 1024]; 2],
    ) -> u8 {
        if let Some(sunsoft4) = self.mappers.sunsoft4.as_ref() {
            if sunsoft4.nametable_chr_rom {
                return self.read_sunsoft4_nametable_chr(physical_nt & 1, offset);
            }
        }

        internal[physical_nt & 1][offset]
    }

    fn write_standard_nametable(
        &mut self,
        physical_nt: usize,
        offset: usize,
        internal: &mut [[u8; 1024]; 2],
        data: u8,
    ) {
        if let Some(sunsoft4) = self.mappers.sunsoft4.as_ref() {
            if sunsoft4.nametable_chr_rom {
                return;
            }
        }

        internal[physical_nt & 1][offset] = data;
    }
}
