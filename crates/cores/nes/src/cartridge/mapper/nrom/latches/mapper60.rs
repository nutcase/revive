use crate::cartridge::Cartridge;

impl Cartridge {
    pub(in crate::cartridge) fn sync_mapper60_game(&mut self) {
        let prg_bank_count = (self.prg_rom.len() / 0x4000).max(1);
        let chr_bank_count = (self.chr_rom.len() / 0x2000).max(1);
        let selected_prg = (self.mappers.multicart.mapper60_game_select as usize) % prg_bank_count;
        let selected_chr = (self.mappers.multicart.mapper60_game_select as usize) % chr_bank_count;

        self.prg_bank = selected_prg as u8;
        self.chr_bank = selected_chr as u8;
    }

    pub(in crate::cartridge) fn advance_mapper60_game(&mut self) {
        let prg_game_count = (self.prg_rom.len() / 0x4000).max(1);
        let chr_game_count = (self.chr_rom.len() / 0x2000).max(1);
        let game_count = prg_game_count.min(chr_game_count).clamp(1, 4);

        self.mappers.multicart.mapper60_game_select =
            ((self.mappers.multicart.mapper60_game_select as usize + 1) % game_count) as u8;
        self.sync_mapper60_game();
    }

    /// Mapper 60: reset-cycled 4-in-1 board selecting one mirrored 16KB PRG
    /// bank and one 8KB CHR bank per game.
    pub(in crate::cartridge) fn read_prg_mapper60(&self, addr: u16) -> u8 {
        if self.prg_rom.is_empty() || addr < 0x8000 {
            return 0;
        }

        let bank_count = (self.prg_rom.len() / 0x4000).max(1);
        let bank = (self.prg_bank as usize) % bank_count;
        let rom_addr = bank * 0x4000 + ((addr - 0x8000) as usize & 0x3FFF);
        self.prg_rom[rom_addr % self.prg_rom.len()]
    }
}
