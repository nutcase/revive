mod chr;
mod latches;
mod multicart;

use super::super::Cartridge;

impl Cartridge {
    /// NROM PRG read - 16KB/32KB mirroring (shared by Mapper 0/3/87)
    pub(in crate::cartridge) fn read_prg_nrom(&self, rom_addr: u16) -> u8 {
        let len = self.prg_rom.len();
        if len == 16384 {
            // 16KB PRG: Mirror at 0xC000
            self.prg_rom[(rom_addr & 0x3FFF) as usize]
        } else {
            // 32KB PRG: Direct mapping
            self.prg_rom[(rom_addr & 0x7FFF) as usize]
        }
    }
}
