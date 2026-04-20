use crate::mapper::mbc1::Mbc1State;
use crate::mapper::mbc2::{MBC2_RAM_SIZE, Mbc2State};
use crate::mapper::mbc3::Mbc3State;
use crate::mapper::mbc5::Mbc5State;
use emulator_core::{EmuError, EmuResult};

const ROM_BANK_SIZE: usize = 16 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GbMapper {
    RomOnly,
    Mbc2,
    Mbc1,
    Mbc3,
    Mbc5,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CgbSupport {
    None,
    Enhanced,
    Only,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GbCartridgeHeader {
    pub title: String,
    pub mapper: GbMapper,
    pub cgb_support: CgbSupport,
    pub rom_banks: usize,
    pub ram_size: usize,
    pub has_rtc: bool,
}

#[derive(Debug)]
pub struct GbCartridge {
    rom: Vec<u8>,
    ram: Vec<u8>,
    header: GbCartridgeHeader,
    state: MapperState,
}

#[derive(Debug)]
enum MapperState {
    RomOnly,
    Mbc2(Mbc2State),
    Mbc1(Mbc1State),
    Mbc3(Mbc3State),
    Mbc5(Mbc5State),
}

impl GbCartridge {
    pub fn from_rom(rom: Vec<u8>) -> EmuResult<Self> {
        if rom.len() < 0x150 {
            return Err(EmuError::InvalidRom("ROM is too small"));
        }

        let header = parse_header(&rom)?;
        let state = match header.mapper {
            GbMapper::RomOnly => MapperState::RomOnly,
            GbMapper::Mbc2 => MapperState::Mbc2(Mbc2State::default()),
            GbMapper::Mbc1 => MapperState::Mbc1(Mbc1State::default()),
            GbMapper::Mbc3 => MapperState::Mbc3(Mbc3State::new(header.has_rtc)),
            GbMapper::Mbc5 => MapperState::Mbc5(Mbc5State::default()),
        };

        Ok(Self {
            rom,
            ram: vec![0; header.ram_size],
            header,
            state,
        })
    }

    pub fn header(&self) -> &GbCartridgeHeader {
        &self.header
    }

    pub fn reset(&mut self) {
        self.state = match self.header.mapper {
            GbMapper::RomOnly => MapperState::RomOnly,
            GbMapper::Mbc2 => MapperState::Mbc2(Mbc2State::default()),
            GbMapper::Mbc1 => MapperState::Mbc1(Mbc1State::default()),
            GbMapper::Mbc3 => MapperState::Mbc3(Mbc3State::new(self.header.has_rtc)),
            GbMapper::Mbc5 => MapperState::Mbc5(Mbc5State::default()),
        };
    }

    pub fn read_rom(&self, addr: u16) -> u8 {
        let addr_usize = addr as usize;
        match addr {
            0x0000..=0x3FFF => {
                let bank = match &self.state {
                    MapperState::RomOnly
                    | MapperState::Mbc2(_)
                    | MapperState::Mbc3(_)
                    | MapperState::Mbc5(_) => 0,
                    MapperState::Mbc1(state) => state.rom_bank_zero(self.header.rom_banks),
                };
                self.read_rom_bank(bank, addr_usize)
            }
            0x4000..=0x7FFF => {
                let bank = self.current_switchable_rom_bank();
                self.read_rom_bank(bank, addr_usize - 0x4000)
            }
            _ => 0xFF,
        }
    }

    pub fn read_ram(&self, addr: u16) -> u8 {
        let offset = (addr as usize).wrapping_sub(0xA000);
        match &self.state {
            MapperState::RomOnly => self.read_ram_index(offset),
            MapperState::Mbc2(state) => state.read_ram(&self.ram, addr),
            MapperState::Mbc1(state) => state.read_ram(&self.ram, addr),
            MapperState::Mbc3(state) => state.read_ram(&self.ram, addr),
            MapperState::Mbc5(state) => state.read_ram(&self.ram, addr),
        }
    }

    pub fn write_ram(&mut self, addr: u16, value: u8) {
        match &mut self.state {
            MapperState::RomOnly => {
                if !self.ram.is_empty() {
                    let index = (usize::from(addr).wrapping_sub(0xA000)) % self.ram.len();
                    self.ram[index] = value;
                }
            }
            MapperState::Mbc2(state) => state.write_ram(&mut self.ram, addr, value),
            MapperState::Mbc1(state) => state.write_ram(&mut self.ram, addr, value),
            MapperState::Mbc3(state) => state.write_ram(&mut self.ram, addr, value),
            MapperState::Mbc5(state) => state.write_ram(&mut self.ram, addr, value),
        }
    }

    pub fn write_rom_control(&mut self, addr: u16, value: u8) {
        match &mut self.state {
            MapperState::RomOnly => {}
            MapperState::Mbc2(state) => state.write_rom_control(addr, value),
            MapperState::Mbc1(state) => state.write_rom_control(addr, value),
            MapperState::Mbc3(state) => state.write_rom_control(addr, value),
            MapperState::Mbc5(state) => state.write_rom_control(addr, value),
        }
    }

    pub fn ram_data(&self) -> Option<&[u8]> {
        if self.ram.is_empty() {
            None
        } else {
            Some(&self.ram)
        }
    }

    pub fn load_ram_data(&mut self, data: &[u8]) {
        if self.ram.is_empty() {
            return;
        }
        let len = self.ram.len().min(data.len());
        self.ram[..len].copy_from_slice(&data[..len]);
        if len < self.ram.len() {
            self.ram[len..].fill(0);
        }
    }

    fn current_switchable_rom_bank(&self) -> usize {
        let banks = self.header.rom_banks.max(1);
        match &self.state {
            MapperState::RomOnly => {
                if banks > 1 {
                    1
                } else {
                    0
                }
            }
            MapperState::Mbc1(state) => state.current_rom_bank(banks),
            MapperState::Mbc3(state) => state.current_rom_bank(banks),
            MapperState::Mbc5(state) => state.current_rom_bank(banks),
            MapperState::Mbc2(state) => state.current_rom_bank(banks),
        }
    }

    fn read_rom_bank(&self, bank: usize, offset: usize) -> u8 {
        let banks = self.header.rom_banks.max(1);
        let bank = bank % banks;
        let base = bank * ROM_BANK_SIZE;
        self.rom.get(base + offset).copied().unwrap_or(0xFF)
    }

    fn read_ram_index(&self, index: usize) -> u8 {
        if self.ram.is_empty() {
            return 0xFF;
        }

        let normalized = index % self.ram.len();
        self.ram[normalized]
    }
}

fn parse_header(rom: &[u8]) -> EmuResult<GbCartridgeHeader> {
    let title_bytes = &rom[0x0134..=0x0143];
    let title_len = title_bytes
        .iter()
        .position(|&b| b == 0)
        .unwrap_or(title_bytes.len());
    let title = String::from_utf8_lossy(&title_bytes[..title_len]).to_string();

    let cgb_support = match rom[0x0143] {
        0x80 => CgbSupport::Enhanced,
        0xC0 => CgbSupport::Only,
        _ => CgbSupport::None,
    };

    let mapper_code = rom[0x0147];
    let mapper = match mapper_code {
        0x00 => GbMapper::RomOnly,
        0x05 | 0x06 => GbMapper::Mbc2,
        0x01..=0x03 => GbMapper::Mbc1,
        0x0F..=0x13 => GbMapper::Mbc3,
        0x19..=0x1E => GbMapper::Mbc5,
        _ => return Err(EmuError::InvalidRom("unsupported cartridge type")),
    };

    let rom_banks = match rom[0x0148] {
        0x00 => 2,
        0x01 => 4,
        0x02 => 8,
        0x03 => 16,
        0x04 => 32,
        0x05 => 64,
        0x06 => 128,
        0x07 => 256,
        0x08 => 512,
        0x52 => 72,
        0x53 => 80,
        0x54 => 96,
        _ => return Err(EmuError::InvalidRom("unsupported ROM size code")),
    };

    let ram_size = if mapper == GbMapper::Mbc2 {
        MBC2_RAM_SIZE
    } else {
        match rom[0x0149] {
            0x00 => 0,
            0x01 => 2 * 1024,
            0x02 => 8 * 1024,
            0x03 => 32 * 1024,
            0x04 => 128 * 1024,
            0x05 => 64 * 1024,
            _ => return Err(EmuError::InvalidRom("unsupported RAM size code")),
        }
    };

    Ok(GbCartridgeHeader {
        title,
        mapper,
        cgb_support,
        rom_banks,
        ram_size,
        has_rtc: matches!(mapper_code, 0x0F | 0x10),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rom_size_code(rom_banks: usize) -> u8 {
        match rom_banks {
            2 => 0x00,
            4 => 0x01,
            8 => 0x02,
            16 => 0x03,
            32 => 0x04,
            64 => 0x05,
            128 => 0x06,
            256 => 0x07,
            512 => 0x08,
            _ => panic!("unsupported test ROM bank count"),
        }
    }

    fn make_test_rom(rom_banks: usize, mapper_type: u8, ram_size_code: u8) -> Vec<u8> {
        let mut rom = vec![0x00; ROM_BANK_SIZE * rom_banks];
        for bank in 0..rom_banks {
            rom[bank * ROM_BANK_SIZE] = bank as u8;
        }

        let title = b"TESTROM";
        rom[0x0134..0x0134 + title.len()].copy_from_slice(title);
        rom[0x0143] = 0x80;
        rom[0x0147] = mapper_type;
        rom[0x0148] = rom_size_code(rom_banks);
        rom[0x0149] = ram_size_code;
        rom
    }

    #[test]
    fn parse_rom_only_header() {
        let rom = make_test_rom(2, 0x00, 0x00);
        let cart = GbCartridge::from_rom(rom).expect("ROM should parse");

        assert_eq!(cart.header().title, "TESTROM");
        assert_eq!(cart.header().mapper, GbMapper::RomOnly);
        assert_eq!(cart.header().cgb_support, CgbSupport::Enhanced);
        assert_eq!(cart.header().rom_banks, 2);
    }

    #[test]
    fn parse_mbc2_header_uses_internal_ram_size() {
        let rom = make_test_rom(8, 0x06, 0x00);
        let cart = GbCartridge::from_rom(rom).expect("ROM should parse");

        assert_eq!(cart.header().mapper, GbMapper::Mbc2);
        assert_eq!(cart.header().ram_size, MBC2_RAM_SIZE);
    }

    #[test]
    fn mbc2_switches_rom_bank_and_uses_nibble_ram() {
        let rom = make_test_rom(8, 0x06, 0x00);
        let mut cart = GbCartridge::from_rom(rom).expect("ROM should parse");

        assert_eq!(cart.read_rom(0x4000), 1);

        // A8=0 on MBC2 control area: must not change ROM bank.
        cart.write_rom_control(0x2000, 0x04);
        assert_eq!(cart.read_rom(0x4000), 1);

        // A8=1 selects lower 4-bit ROM bank (bank 0 maps to 1).
        cart.write_rom_control(0x2100, 0x04);
        assert_eq!(cart.read_rom(0x4000), 4);
        cart.write_rom_control(0x2100, 0x00);
        assert_eq!(cart.read_rom(0x4000), 1);

        // RAM disabled by default.
        assert_eq!(cart.read_ram(0xA000), 0xFF);

        // A8=0 enables RAM.
        cart.write_rom_control(0x0000, 0x0A);
        cart.write_ram(0xA000, 0xAB);
        assert_eq!(cart.read_ram(0xA000), 0xFB);

        // MBC2 RAM mirrors over 0x200-byte space.
        cart.write_ram(0xA200, 0x05);
        assert_eq!(cart.read_ram(0xA000), 0xF5);

        // Disable RAM again.
        cart.write_rom_control(0x0000, 0x00);
        assert_eq!(cart.read_ram(0xA000), 0xFF);
    }

    #[test]
    fn mbc1_switches_rom_bank() {
        let rom = make_test_rom(8, 0x01, 0x03);
        let mut cart = GbCartridge::from_rom(rom).expect("ROM should parse");

        assert_eq!(cart.read_rom(0x4000), 1);
        cart.write_rom_control(0x2000, 0x03);
        assert_eq!(cart.read_rom(0x4000), 3);
    }

    #[test]
    fn mbc3_switches_rom_and_ram_bank() {
        let rom = make_test_rom(16, 0x13, 0x03);
        let mut cart = GbCartridge::from_rom(rom).expect("ROM should parse");

        cart.write_rom_control(0x2000, 0x07);
        assert_eq!(cart.read_rom(0x4000), 7);

        cart.write_rom_control(0x0000, 0x0A);
        cart.write_rom_control(0x4000, 0x02);
        cart.write_ram(0xA000, 0x5A);
        assert_eq!(cart.read_ram(0xA000), 0x5A);
    }

    #[test]
    fn mbc5_switches_rom_and_ram_bank() {
        let rom = make_test_rom(32, 0x1B, 0x03);
        let mut cart = GbCartridge::from_rom(rom).expect("ROM should parse");

        cart.write_rom_control(0x2000, 0x09);
        cart.write_rom_control(0x3000, 0x00);
        assert_eq!(cart.read_rom(0x4000), 9);

        cart.write_rom_control(0x0000, 0x0A);
        cart.write_rom_control(0x4000, 0x01);
        cart.write_ram(0xA000, 0x77);
        assert_eq!(cart.read_ram(0xA000), 0x77);
    }
}
