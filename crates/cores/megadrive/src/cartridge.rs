use std::fmt::{Display, Formatter};

const HEADER_MIN_SIZE: usize = 0x200;
const BANK_SIZE: u32 = 0x80000; // 512 KB per bank slot

#[derive(Debug, Clone, bincode::Encode, bincode::Decode)]
pub struct Cartridge {
    rom: Vec<u8>,
    header: RomHeader,
    save_ram: Option<SaveRam>,
    /// Sega mapper bank registers (SSF2-style).
    /// bank_regs[0..7] map 512KB slots at 0x000000..0x3FFFFF.
    /// Default: identity mapping [0,1,2,3,4,5,6,7].
    bank_regs: [u8; 8],
    /// Set to true once any bank register is written (activates mapper).
    mapper_active: bool,
    /// Serial EEPROM state for games that use I2C EEPROM saves.
    eeprom: Option<Eeprom>,
}

#[derive(Debug, Clone, bincode::Encode, bincode::Decode)]
struct SaveRam {
    start: u32,
    end: u32,
    lane: SaveRamLane,
    data: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, bincode::Encode, bincode::Decode)]
enum SaveRamLane {
    Both,
    Even,
    Odd,
}

impl SaveRam {
    fn parse_from_header(rom: &[u8]) -> Option<Self> {
        if rom.len() < 0x1BC || &rom[0x1B0..0x1B2] != b"RA" {
            return None;
        }

        let start = read_u32_be(rom, 0x1B4) & 0x00FF_FFFF;
        let end = read_u32_be(rom, 0x1B8) & 0x00FF_FFFF;
        if end < start {
            return None;
        }

        let lane = if (start & 1) == 1 && (end & 1) == 1 {
            SaveRamLane::Odd
        } else if (start & 1) == 0 && (end & 1) == 0 {
            SaveRamLane::Even
        } else {
            SaveRamLane::Both
        };

        let len = match lane {
            SaveRamLane::Both => end.wrapping_sub(start).wrapping_add(1) as usize,
            SaveRamLane::Even | SaveRamLane::Odd => {
                end.wrapping_sub(start).wrapping_div(2).wrapping_add(1) as usize
            }
        };
        if len == 0 {
            return None;
        }

        Some(Self {
            start,
            end,
            lane,
            // Cartridge save RAM powers up to erased state.
            data: vec![0xFF; len],
        })
    }

    fn contains(&self, addr: u32) -> bool {
        addr >= self.start && addr <= self.end
    }

    fn offset_for_addr(&self, addr: u32) -> Option<usize> {
        if !self.contains(addr) {
            return None;
        }
        match self.lane {
            SaveRamLane::Both => Some((addr - self.start) as usize),
            SaveRamLane::Even => {
                if (addr & 1) == 0 {
                    Some(((addr - self.start) >> 1) as usize)
                } else {
                    None
                }
            }
            SaveRamLane::Odd => {
                if (addr & 1) == 1 {
                    Some(((addr - self.start) >> 1) as usize)
                } else {
                    None
                }
            }
        }
    }
}

/// I2C EEPROM types used by various Genesis/Mega Drive games.
#[derive(Debug, Clone, Copy, PartialEq, Eq, bincode::Encode, bincode::Decode)]
pub enum EepromType {
    /// 24C01: 128 bytes, 7-bit word address
    X24C01,
    /// 24C02: 256 bytes, 8-bit word address
    X24C02,
    /// 24C08: 1024 bytes, device address contains page bits
    X24C08,
    /// 24C16: 2048 bytes, device address contains page bits
    X24C16,
    /// 24C65: 8192 bytes, 16-bit word address
    X24C65,
}

impl EepromType {
    fn size_bytes(self) -> usize {
        match self {
            Self::X24C01 => 128,
            Self::X24C02 => 256,
            Self::X24C08 => 1024,
            Self::X24C16 => 2048,
            Self::X24C65 => 8192,
        }
    }

    fn word_address_bits(self) -> u8 {
        match self {
            Self::X24C01 => 7,
            Self::X24C02 => 8,
            Self::X24C08 => 8,
            Self::X24C16 => 8,
            Self::X24C65 => 16,
        }
    }

    fn uses_device_address_pages(self) -> bool {
        matches!(self, Self::X24C08 | Self::X24C16)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, bincode::Encode, bincode::Decode)]
enum EepromState {
    Standby,
    DeviceAddress,
    WordAddressHigh,
    WordAddressLow,
    ReadData,
    WriteData,
}

#[derive(Debug, Clone, bincode::Encode, bincode::Decode)]
struct EepromConfig {
    /// Which address(es) the EEPROM is mapped to for 68K access.
    map_addr: u32,
    /// Bit mask for SDA line on data bus (read & write).
    sda_in_bit: u8,
    sda_out_bit: u8,
    /// Bit mask for SCL line on data bus.
    scl_bit: u8,
}

#[derive(Debug, Clone, bincode::Encode, bincode::Decode)]
struct Eeprom {
    eeprom_type: EepromType,
    config: EepromConfig,
    data: Vec<u8>,
    state: EepromState,
    scl_prev: bool,
    sda_prev: bool,
    sda_out: bool,
    bit_counter: u8,
    shift_reg: u8,
    word_address: u16,
    device_address_page: u16,
    rw_bit: bool, // true = read
}

impl Eeprom {
    fn new(eeprom_type: EepromType, config: EepromConfig) -> Self {
        let size = eeprom_type.size_bytes();
        Self {
            eeprom_type,
            config,
            data: vec![0xFF; size],
            state: EepromState::Standby,
            scl_prev: true,
            sda_prev: true,
            sda_out: true,
            bit_counter: 0,
            shift_reg: 0,
            word_address: 0,
            device_address_page: 0,
            rw_bit: false,
        }
    }

    fn read_bit(&self) -> bool {
        self.sda_out
    }

    fn write(&mut self, scl: bool, sda: bool) {
        let scl_rise = scl && !self.scl_prev;
        let scl_high = scl;

        // START condition: SDA falls while SCL is high
        if scl_high && self.sda_prev && !sda {
            self.state = EepromState::DeviceAddress;
            self.bit_counter = 0;
            self.shift_reg = 0;
            self.scl_prev = scl;
            self.sda_prev = sda;
            return;
        }

        // STOP condition: SDA rises while SCL is high
        if scl_high && !self.sda_prev && sda {
            self.state = EepromState::Standby;
            self.sda_out = true;
            self.scl_prev = scl;
            self.sda_prev = sda;
            return;
        }

        if scl_rise {
            match self.state {
                EepromState::Standby => {}
                EepromState::DeviceAddress => {
                    if self.bit_counter < 8 {
                        self.shift_reg = (self.shift_reg << 1) | (sda as u8);
                        self.bit_counter += 1;
                        if self.bit_counter == 8 {
                            self.rw_bit = (self.shift_reg & 1) != 0;
                            if self.eeprom_type.uses_device_address_pages() {
                                // For 24C08/24C16: bits 3..1 of device address are page bits
                                self.device_address_page =
                                    ((self.shift_reg as u16 >> 1) & 0x07) << 8;
                            }
                            // Send ACK on next clock
                            self.sda_out = false;
                        }
                    } else {
                        // ACK bit sent, advance to next state
                        self.sda_out = true;
                        self.bit_counter = 0;
                        self.shift_reg = 0;
                        if self.rw_bit {
                            self.state = EepromState::ReadData;
                            self.load_read_byte();
                        } else if self.eeprom_type.word_address_bits() > 8 {
                            self.state = EepromState::WordAddressHigh;
                        } else {
                            self.state = EepromState::WordAddressLow;
                        }
                    }
                }
                EepromState::WordAddressHigh => {
                    if self.bit_counter < 8 {
                        self.shift_reg = (self.shift_reg << 1) | (sda as u8);
                        self.bit_counter += 1;
                        if self.bit_counter == 8 {
                            self.word_address =
                                (self.shift_reg as u16) << 8 | (self.word_address & 0xFF);
                            self.sda_out = false; // ACK
                        }
                    } else {
                        self.sda_out = true;
                        self.bit_counter = 0;
                        self.shift_reg = 0;
                        self.state = EepromState::WordAddressLow;
                    }
                }
                EepromState::WordAddressLow => {
                    if self.bit_counter < 8 {
                        self.shift_reg = (self.shift_reg << 1) | (sda as u8);
                        self.bit_counter += 1;
                        if self.bit_counter == 8 {
                            if self.eeprom_type.word_address_bits() > 8 {
                                self.word_address =
                                    (self.word_address & 0xFF00) | self.shift_reg as u16;
                            } else {
                                self.word_address = self.shift_reg as u16;
                            }
                            if self.eeprom_type.uses_device_address_pages() {
                                self.word_address |= self.device_address_page;
                            }
                            self.sda_out = false; // ACK
                        }
                    } else {
                        self.sda_out = true;
                        self.bit_counter = 0;
                        self.shift_reg = 0;
                        self.state = EepromState::WriteData;
                    }
                }
                EepromState::ReadData => {
                    if self.bit_counter < 8 {
                        // Output data bit (MSB first)
                        self.sda_out = (self.shift_reg >> (7 - self.bit_counter)) & 1 != 0;
                        self.bit_counter += 1;
                    } else {
                        // Master ACK/NACK
                        self.word_address = self.word_address.wrapping_add(1)
                            % self.eeprom_type.size_bytes() as u16;
                        self.bit_counter = 0;
                        self.load_read_byte();
                    }
                }
                EepromState::WriteData => {
                    if self.bit_counter < 8 {
                        self.shift_reg = (self.shift_reg << 1) | (sda as u8);
                        self.bit_counter += 1;
                        if self.bit_counter == 8 {
                            let addr = self.word_address as usize
                                % self.eeprom_type.size_bytes();
                            self.data[addr] = self.shift_reg;
                            self.word_address = self.word_address.wrapping_add(1)
                                % self.eeprom_type.size_bytes() as u16;
                            self.sda_out = false; // ACK
                        }
                    } else {
                        self.sda_out = true;
                        self.bit_counter = 0;
                        self.shift_reg = 0;
                    }
                }
            }
        }

        self.scl_prev = scl;
        self.sda_prev = sda;
    }

    fn load_read_byte(&mut self) {
        let addr = self.word_address as usize % self.eeprom_type.size_bytes();
        self.shift_reg = self.data[addr];
    }
}

/// Known game EEPROM configurations (product code → EEPROM type + mapping).
fn detect_eeprom(header: &RomHeader, _rom: &[u8]) -> Option<(EepromType, EepromConfig)> {
    let product = header.product_code.trim();
    // NBA Jam (T-081326, T-081276) — 24C02 at 0x200000
    // NBA Jam Tournament Edition (T-081586) — 24C02
    if product.contains("T-81326") || product.contains("T-81276")
        || product.contains("T-81586")
    {
        return Some((
            EepromType::X24C02,
            EepromConfig {
                map_addr: 0x200000,
                sda_in_bit: 0,
                sda_out_bit: 0,
                scl_bit: 1,
            },
        ));
    }

    // Mega Man: The Wily Wars (T-12056) — 24C01
    if product.contains("T-12056") {
        return Some((
            EepromType::X24C01,
            EepromConfig {
                map_addr: 0x200000,
                sda_in_bit: 0,
                sda_out_bit: 0,
                scl_bit: 1,
            },
        ));
    }

    // Wonderboy in Monster World (G-4060) — 24C01
    if product.contains("G-4060") {
        return Some((
            EepromType::X24C01,
            EepromConfig {
                map_addr: 0x200000,
                sda_in_bit: 0,
                sda_out_bit: 0,
                scl_bit: 1,
            },
        ));
    }

    // College Slam, Frank Thomas Big Hurt Baseball — 24C16
    if product.contains("T-81406") || product.contains("T-81576") {
        return Some((
            EepromType::X24C16,
            EepromConfig {
                map_addr: 0x200000,
                sda_in_bit: 0,
                sda_out_bit: 0,
                scl_bit: 1,
            },
        ));
    }

    // Rings of Power (T-50176) — 24C01
    if product.contains("T-50176") {
        return Some((
            EepromType::X24C01,
            EepromConfig {
                map_addr: 0x200000,
                sda_in_bit: 0,
                sda_out_bit: 0,
                scl_bit: 1,
            },
        ));
    }

    // EA games using EEPROM at 0x200000 (various)
    // Generic: ROMs > 2MB with "RA" header pointing to 0x200000-range and small size
    // suggest EEPROM rather than SRAM.

    None
}

impl Cartridge {
    pub fn from_bytes(rom: Vec<u8>) -> Result<Self, CartridgeError> {
        if rom.len() < HEADER_MIN_SIZE {
            return Err(CartridgeError::RomTooSmall {
                size: rom.len(),
                min_size: HEADER_MIN_SIZE,
            });
        }

        let header = RomHeader::parse(&rom);
        let eeprom = detect_eeprom(&header, &rom).map(|(t, c)| Eeprom::new(t, c));
        // If EEPROM detected, suppress SRAM (some headers declare both)
        let save_ram = if eeprom.is_some() {
            None
        } else {
            SaveRam::parse_from_header(&rom)
        };
        Ok(Self {
            rom,
            header,
            save_ram,
            bank_regs: [0, 1, 2, 3, 4, 5, 6, 7],
            mapper_active: false,
            eeprom,
        })
    }

    pub fn len(&self) -> usize {
        self.rom.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rom.is_empty()
    }

    pub fn header(&self) -> &RomHeader {
        &self.header
    }

    pub fn read_u8(&self, addr: u32) -> u8 {
        let len = self.rom.len();
        if len == 0 {
            return 0xFF;
        }
        let physical = if self.mapper_active {
            let slot = (addr >> 19) as usize & 7; // 512KB slots
            let bank = self.bank_regs[slot] as u32;
            (bank << 19) | (addr & (BANK_SIZE - 1))
        } else {
            addr
        };
        let index = (physical as usize) % len;
        self.rom[index]
    }

    pub fn has_save_ram(&self) -> bool {
        self.save_ram.is_some()
    }

    // --- Sega mapper (SSF2-style bank switching) ---

    /// Write to a Sega mapper bank register.
    /// `reg_index` is 0..7 corresponding to addresses 0xA130F0..0xA130FF (even bytes).
    pub fn write_bank_register(&mut self, reg_index: usize, value: u8) {
        // Slot 0 (first 512KB) is fixed — ignore writes to reg0 so that
        // the vector table and fixed code bank are never remapped.
        if reg_index >= 1 && reg_index < 8 {
            self.bank_regs[reg_index] = value;
            self.mapper_active = true;
        }
    }

    pub fn read_bank_register(&self, reg_index: usize) -> u8 {
        if reg_index < 8 {
            self.bank_regs[reg_index]
        } else {
            0xFF
        }
    }

    // --- EEPROM ---

    pub fn has_eeprom(&self) -> bool {
        self.eeprom.is_some()
    }

    /// Check if an address is in the EEPROM-mapped region (even or odd byte).
    pub fn eeprom_mapped(&self, addr: u32) -> bool {
        if let Some(eeprom) = &self.eeprom {
            (addr & 0xFFFFFE) == eeprom.config.map_addr
        } else {
            false
        }
    }

    /// Check if a write to this address should trigger I2C.
    /// Only the odd byte triggers — the even byte (high byte of a 16-bit
    /// write in big-endian) is ignored so that a word write does not
    /// double-toggle SCL/SDA.  I2C control bits (SCL/SDA) are in the low
    /// bits (D0-D7), which map to the odd byte in 68K big-endian words.
    pub fn eeprom_write_triggers(&self, addr: u32) -> bool {
        self.eeprom_mapped(addr) && (addr & 1) == 1
    }

    /// Read EEPROM SDA bit (returns full byte with SDA bit set/clear).
    pub fn read_eeprom(&self, _addr: u32) -> u8 {
        if let Some(eeprom) = &self.eeprom {
            if eeprom.read_bit() {
                1 << eeprom.config.sda_out_bit
            } else {
                0
            }
        } else {
            0xFF
        }
    }

    /// Write EEPROM SCL/SDA from data bus byte.
    pub fn write_eeprom(&mut self, _addr: u32, value: u8) {
        if let Some(eeprom) = &mut self.eeprom {
            let scl = (value >> eeprom.config.scl_bit) & 1 != 0;
            let sda = (value >> eeprom.config.sda_in_bit) & 1 != 0;
            eeprom.write(scl, sda);
        }
    }

    pub fn read_save_ram_u8(&self, addr: u32) -> Option<u8> {
        let save_ram = self.save_ram.as_ref()?;
        if !save_ram.contains(addr) {
            return None;
        }
        Some(
            save_ram
                .offset_for_addr(addr)
                .and_then(|idx| save_ram.data.get(idx).copied())
                .unwrap_or(0xFF),
        )
    }

    pub fn write_save_ram_u8(&mut self, addr: u32, value: u8) -> bool {
        let Some(save_ram) = self.save_ram.as_mut() else {
            return false;
        };
        if !save_ram.contains(addr) {
            return false;
        }
        if let Some(idx) = save_ram.offset_for_addr(addr)
            && let Some(slot) = save_ram.data.get_mut(idx)
        {
            *slot = value;
        }
        true
    }
}

#[derive(Debug, Clone, bincode::Encode, bincode::Decode)]
pub struct RomHeader {
    pub console_name: String,
    pub domestic_title: String,
    pub overseas_title: String,
    pub product_code: String,
    pub checksum: u16,
    pub io_support: String,
    pub rom_start: u32,
    pub rom_end: u32,
    pub ram_start: u32,
    pub ram_end: u32,
    pub region: String,
}

impl RomHeader {
    fn parse(rom: &[u8]) -> Self {
        Self {
            console_name: read_ascii(rom, 0x100, 0x110),
            domestic_title: read_ascii(rom, 0x120, 0x150),
            overseas_title: read_ascii(rom, 0x150, 0x180),
            product_code: read_ascii(rom, 0x180, 0x18E),
            checksum: read_u16_be(rom, 0x18E),
            io_support: read_ascii(rom, 0x190, 0x1A0),
            rom_start: read_u32_be(rom, 0x1A0),
            rom_end: read_u32_be(rom, 0x1A4),
            ram_start: read_u32_be(rom, 0x1A8),
            ram_end: read_u32_be(rom, 0x1AC),
            region: read_ascii(rom, 0x1F0, 0x200),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, bincode::Encode, bincode::Decode)]
pub enum CartridgeError {
    RomTooSmall { size: usize, min_size: usize },
}

impl Display for CartridgeError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::RomTooSmall { size, min_size } => {
                write!(
                    f,
                    "ROM image is too small: {size} bytes (minimum required: {min_size})"
                )
            }
        }
    }
}

impl std::error::Error for CartridgeError {}

fn read_ascii(rom: &[u8], start: usize, end: usize) -> String {
    let end = end.min(rom.len());
    let start = start.min(end);
    let bytes = &rom[start..end];
    let mut text = String::with_capacity(bytes.len());

    for &b in bytes {
        let c = if b.is_ascii_graphic() || b == b' ' {
            b as char
        } else {
            ' '
        };
        text.push(c);
    }

    text.trim().to_string()
}

fn read_u16_be(rom: &[u8], offset: usize) -> u16 {
    if offset + 1 >= rom.len() {
        return 0;
    }
    u16::from_be_bytes([rom[offset], rom[offset + 1]])
}

fn read_u32_be(rom: &[u8], offset: usize) -> u32 {
    if offset + 3 >= rom.len() {
        return 0;
    }
    u32::from_be_bytes([
        rom[offset],
        rom[offset + 1],
        rom[offset + 2],
        rom[offset + 3],
    ])
}

#[cfg(test)]
mod tests {
    use super::{Cartridge, CartridgeError};

    #[test]
    fn parses_header_fields() {
        let mut rom = vec![0u8; 0x400];
        rom[0x100..0x110].copy_from_slice(b"SEGA MEGA DRIVE ");
        rom[0x120..0x126].copy_from_slice(b"SONIC ");
        rom[0x180..0x188].copy_from_slice(b"GM 00001");
        rom[0x18E..0x190].copy_from_slice(&0x4E71u16.to_be_bytes());
        rom[0x1F0..0x1F3].copy_from_slice(b"JUE");

        let cart = Cartridge::from_bytes(rom).expect("valid rom");
        let header = cart.header();

        assert_eq!(header.console_name, "SEGA MEGA DRIVE");
        assert_eq!(header.domestic_title, "SONIC");
        assert_eq!(header.product_code, "GM 00001");
        assert_eq!(header.checksum, 0x4E71);
        assert_eq!(header.region, "JUE");
    }

    #[test]
    fn rejects_too_small_rom() {
        let rom = vec![0u8; 0x100];
        let err = Cartridge::from_bytes(rom).expect_err("must fail");
        assert_eq!(
            err,
            CartridgeError::RomTooSmall {
                size: 0x100,
                min_size: 0x200
            }
        );
    }

    #[test]
    fn parses_backup_ram_header_and_maps_odd_lane() {
        let mut rom = vec![0u8; 0x400];
        rom[0x1B0..0x1B2].copy_from_slice(b"RA");
        rom[0x1B4..0x1B8].copy_from_slice(&0x0020_0001u32.to_be_bytes());
        rom[0x1B8..0x1BC].copy_from_slice(&0x0020_0007u32.to_be_bytes());

        let mut cart = Cartridge::from_bytes(rom).expect("valid rom");
        assert!(cart.has_save_ram());
        assert_eq!(cart.read_save_ram_u8(0x0020_0001), Some(0xFF));
        assert_eq!(cart.read_save_ram_u8(0x0020_0000), None);
        assert_eq!(cart.read_save_ram_u8(0x0020_0002), Some(0xFF));

        assert!(cart.write_save_ram_u8(0x0020_0001, 0x12));
        assert_eq!(cart.read_save_ram_u8(0x0020_0001), Some(0x12));
        // Even lane is not writable in odd-lane SRAM range.
        assert!(cart.write_save_ram_u8(0x0020_0002, 0x34));
        assert_eq!(cart.read_save_ram_u8(0x0020_0001), Some(0x12));
        assert_eq!(cart.read_save_ram_u8(0x0020_0002), Some(0xFF));
    }

    #[test]
    fn sega_mapper_bank_switching() {
        // Create a 2MB ROM with distinct bytes in each 512KB bank
        let mut rom = vec![0u8; 0x200000];
        rom[0x000000] = 0xAA; // bank 0
        rom[0x080000] = 0xBB; // bank 1
        rom[0x100000] = 0xCC; // bank 2
        rom[0x180000] = 0xDD; // bank 3

        let mut cart = Cartridge::from_bytes(rom).expect("valid rom");

        // Default identity mapping
        assert_eq!(cart.read_u8(0x000000), 0xAA);
        assert_eq!(cart.read_u8(0x080000), 0xBB);
        assert_eq!(cart.read_u8(0x100000), 0xCC);
        assert_eq!(cart.read_u8(0x180000), 0xDD);

        // Swap bank 1 slot to point to bank 3
        cart.write_bank_register(1, 3);
        assert_eq!(cart.read_u8(0x080000), 0xDD);

        // Bank 0 slot unchanged
        assert_eq!(cart.read_u8(0x000000), 0xAA);

        // Swap back
        cart.write_bank_register(1, 1);
        assert_eq!(cart.read_u8(0x080000), 0xBB);
    }

    #[test]
    fn eeprom_detected_by_product_code() {
        let mut rom = vec![0u8; 0x400];
        // Set product code to NBA Jam
        rom[0x180..0x18E].copy_from_slice(b"T-81326 -00\x00\x00\x00");

        let cart = Cartridge::from_bytes(rom).expect("valid rom");
        assert!(cart.has_eeprom());
        assert!(!cart.has_save_ram()); // EEPROM suppresses SRAM
    }

    #[test]
    fn eeprom_write_and_read_byte() {
        let mut rom = vec![0u8; 0x400];
        rom[0x180..0x18E].copy_from_slice(b"T-81326 -00\x00\x00\x00");

        let mut cart = Cartridge::from_bytes(rom).expect("valid rom");
        assert!(cart.eeprom_mapped(0x200000));

        // I2C sequence: START, device addr 0xA0 (write), word addr 0x00, data 0x42, STOP
        let sda_bit = 0;
        let scl_bit = 1;
        let scl = 1 << scl_bit;
        let sda = 1 << sda_bit;

        // Helper: send a byte over I2C (MSB first)
        let mut send_byte = |cart: &mut Cartridge, byte: u8| {
            for i in (0..8).rev() {
                let sda_val = if (byte >> i) & 1 != 0 { sda } else { 0 };
                cart.write_eeprom(0x200000, sda_val);        // SCL low, set SDA
                cart.write_eeprom(0x200000, sda_val | scl);  // SCL high (rising edge)
                cart.write_eeprom(0x200000, sda_val);        // SCL low
            }
            // ACK clock
            cart.write_eeprom(0x200000, 0);                  // SCL low, release SDA
            cart.write_eeprom(0x200000, scl);                // SCL high
            cart.write_eeprom(0x200000, 0);                  // SCL low
        };

        // START: SDA falls while SCL high
        cart.write_eeprom(0x200000, scl | sda); // SCL=1, SDA=1
        cart.write_eeprom(0x200000, scl);       // SCL=1, SDA=0 (START)

        // Device address: 0xA0 (write) = 1010_0000
        send_byte(&mut cart, 0xA0);
        // Word address: 0x00
        send_byte(&mut cart, 0x00);
        // Data: 0x42
        send_byte(&mut cart, 0x42);

        // STOP: SDA rises while SCL high
        cart.write_eeprom(0x200000, 0);         // SCL=0, SDA=0
        cart.write_eeprom(0x200000, scl);       // SCL=1, SDA=0
        cart.write_eeprom(0x200000, scl | sda); // SCL=1, SDA=1 (STOP)

        // Now read back: START, device addr 0xA0 (write), word addr 0x00, re-START,
        // device addr 0xA1 (read), read 8 bits
        cart.write_eeprom(0x200000, scl | sda);
        cart.write_eeprom(0x200000, scl);       // START

        send_byte(&mut cart, 0xA0); // device write
        send_byte(&mut cart, 0x00); // word addr 0

        // Re-START
        cart.write_eeprom(0x200000, sda);       // SCL low, SDA high
        cart.write_eeprom(0x200000, scl | sda); // SCL high, SDA high
        cart.write_eeprom(0x200000, scl);       // SCL high, SDA low (START)

        send_byte(&mut cart, 0xA1); // device read

        // Read 8 bits
        let mut read_byte = 0u8;
        for _ in 0..8 {
            cart.write_eeprom(0x200000, 0);     // SCL low
            cart.write_eeprom(0x200000, scl);   // SCL high
            let bit = cart.read_eeprom(0x200000) & sda;
            read_byte = (read_byte << 1) | if bit != 0 { 1 } else { 0 };
        }

        assert_eq!(read_byte, 0x42, "EEPROM should read back 0x42");
    }
}
