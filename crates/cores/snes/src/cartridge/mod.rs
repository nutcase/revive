#![cfg_attr(not(feature = "dev"), allow(dead_code))]

pub mod dsp1;
pub mod dsp3;
pub mod mapper;
pub mod sa1;
pub mod sdd1;
pub mod spc7110;
pub mod superfx;
pub use mapper::MapperType;

use std::fs::File;
use std::io::Read;
use std::path::Path;

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct CartridgeHeader {
    pub title: String,
    pub mapper_type: MapperType,
    pub rom_size: usize,
    pub ram_size: usize,
    pub country: u8,
    pub developer: u8,
    pub version: u8,
    pub checksum: u16,
    pub checksum_complement: u16,
}

#[allow(dead_code)]
pub struct Cartridge {
    pub rom: Vec<u8>,
    pub header: CartridgeHeader,
    pub has_header: bool,
}

impl Cartridge {
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Self, String> {
        let mut file = File::open(path).map_err(|e| format!("Failed to open ROM file: {}", e))?;
        let mut data = Vec::new();
        file.read_to_end(&mut data)
            .map_err(|e| format!("Failed to read ROM file: {}", e))?;

        Self::load_from_bytes(data)
    }

    pub fn load_from_bytes(mut data: Vec<u8>) -> Result<Self, String> {
        if data.is_empty() {
            return Err("ROM file is empty".to_string());
        }

        let has_header = data.len() % 1024 == 512;

        if has_header {
            data.drain(0..512);
        }

        let header = Self::parse_header(&data)?;

        Ok(Cartridge {
            rom: data,
            header,
            has_header,
        })
    }

    fn parse_header(rom: &[u8]) -> Result<CartridgeHeader, String> {
        // Try both HiROM and LoROM locations and pick the one with best score
        let (header_offset, detected_mapper) = Self::detect_mapper_and_location(rom)?;

        if rom.len() <= header_offset + 0x2F {
            return Err("ROM too small to contain header".to_string());
        }

        // Extract and validate title (21 characters max)
        // Title starts at header base + 0x10 (LoROM: 0x7FC0, HiROM: 0xFFC0)
        let title_bytes = &rom[header_offset + 0x10..header_offset + 0x10 + 21];
        let title = Self::extract_title(title_bytes);

        // Validate title contains printable ASCII
        if !Self::is_valid_title(&title) && !crate::debug_flags::quiet() {
            println!(
                "Warning: ROM title contains non-printable characters: {:?}",
                title
            );
        }

        // Header bytes:
        // - Map mode ($FFD5/$7FD5) is at base + 0x15
        // - ROM type / chipset ($FFD6/$7FD6) is at base + 0x16
        //
        // Our `header_offset` is 0x7FB0/0xFFB0 (0x10 bytes before base), so:
        // - map_mode: +0x25
        // - rom_type: +0x26
        let map_mode = rom[header_offset + 0x25];
        let rom_type = rom[header_offset + 0x26];
        let mapper_type = if title.trim_start().starts_with("DRAGONQUEST3") {
            MapperType::DragonQuest3
        } else if rom_type == 0x05 && map_mode == 0x30 && rom[header_offset + 0x2A] == 0xB2 {
            MapperType::Dsp3
        } else {
            Self::determine_mapper_type(rom_type, detected_mapper)
        };

        // Enhanced mapper validation
        Self::validate_mapper_compatibility(rom_type, &mapper_type)?;

        // Parse and validate ROM size
        let rom_size_code = rom[header_offset + 0x27];
        let rom_size = Self::decode_rom_size(rom_size_code)?;

        // Validate ROM size against actual file size
        Self::validate_rom_size(rom.len(), rom_size)?;

        // Parse and validate RAM size
        let ram_size_code = rom[header_offset + 0x28];
        let ram_size = Self::decode_ram_size(ram_size_code);

        let country = rom[header_offset + 0x29];
        let developer = rom[header_offset + 0x2A];
        let version = rom[header_offset + 0x2B];

        // Parse checksums
        let checksum_complement =
            ((rom[header_offset + 0x2D] as u16) << 8) | (rom[header_offset + 0x2C] as u16);
        let checksum =
            ((rom[header_offset + 0x2F] as u16) << 8) | (rom[header_offset + 0x2E] as u16);

        // Validate checksums
        Self::validate_checksums(checksum, checksum_complement)?;

        // Calculate and verify actual ROM checksum
        let calculated_checksum = Self::calculate_rom_checksum(rom);
        if calculated_checksum != checksum && !crate::debug_flags::quiet() {
            println!(
                "Warning: Stored checksum (0x{:04X}) doesn't match calculated checksum (0x{:04X})",
                checksum, calculated_checksum
            );
        }

        Ok(CartridgeHeader {
            title,
            mapper_type,
            rom_size,
            ram_size,
            country,
            developer,
            version,
            checksum,
            checksum_complement,
        })
    }

    fn detect_mapper_and_location(rom: &[u8]) -> Result<(usize, MapperType), String> {
        if rom.len() < 0x10000 {
            return Err("ROM too small for SNES format".to_string());
        }

        let lo_hdr = 0x7FB0;
        let hi_hdr = 0xFFB0;

        let lorom_score = Self::score_header(rom, lo_hdr);
        let hirom_score = Self::score_header(rom, hi_hdr);
        let exhirom_score = if rom.len() >= 0x40FFB0 + 0x30 {
            Self::score_header(rom, 0x40FFB0)
        } else {
            0
        };

        // Determine best match
        if exhirom_score >= hirom_score && exhirom_score >= lorom_score && exhirom_score > 6 {
            Ok((0x40FFB0, MapperType::ExHiRom))
        } else if hirom_score > lorom_score && hirom_score > 4 {
            Ok((0xFFB0, MapperType::HiRom))
        } else if lorom_score > 4 {
            Ok((0x7FB0, MapperType::LoRom))
        } else {
            // Fallback to LoROM if scores are low
            if !crate::debug_flags::quiet() {
                println!("Warning: Low header scores, defaulting to LoROM");
            }
            Ok((0x7FB0, MapperType::LoRom))
        }
    }

    fn extract_title(title_bytes: &[u8]) -> String {
        // Convert to string, handling both ASCII and Shift-JIS
        let mut title = String::new();
        for &byte in title_bytes {
            if byte == 0x00 {
                break; // Null terminator
            } else if (0x20..=0x7E).contains(&byte) {
                title.push(byte as char); // ASCII printable
            } else if byte >= 0x80 {
                title.push('?'); // Non-ASCII, replace with placeholder
            }
        }
        title.trim().to_string()
    }

    fn is_valid_title(title: &str) -> bool {
        !title.is_empty() && title.chars().all(|c| c.is_ascii_graphic() || c == ' ')
    }

    fn determine_mapper_type(rom_type: u8, detected: MapperType) -> MapperType {
        // Prefer the detected mapping (LoROM/HiROM/ExHiROM) and only override when
        // the ROM type clearly indicates an enhancement chip that changes the bus.
        match rom_type {
            // DSP-1: ROM type $03/$05 with LoROM or HiROM
            0x03 | 0x05 if detected == MapperType::LoRom => MapperType::Dsp1,
            0x03 | 0x05 if detected == MapperType::HiRom => MapperType::Dsp1HiRom,
            0x13 | 0x14 | 0x15 | 0x1A => MapperType::SuperFx, // Super FX variants
            0x34 => MapperType::Sa1,                          // SA-1
            0x43 | 0x45 => MapperType::Sdd1, // S-DD1 (+ variant header used by Star Ocean)
            0xF5 | 0xF9 => MapperType::Spc7110, // SPC7110 (+ RTC variant header)
            _ => detected,
        }
    }

    fn validate_mapper_compatibility(rom_type: u8, mapper_type: &MapperType) -> Result<(), String> {
        let has_enhancement_chip = rom_type & 0xF0 != 0x00;

        match mapper_type {
            MapperType::SuperFx => {
                if !matches!(rom_type, 0x13 | 0x14 | 0x15 | 0x1A) {
                    return Err(
                        "SuperFX mapper requires ROM type 0x13, 0x14, 0x15, or 0x1A".to_string()
                    );
                }
            }
            MapperType::Sa1 => {
                if rom_type != 0x34 {
                    return Err("SA-1 mapper requires ROM type 0x34".to_string());
                }
            }
            MapperType::Spc7110 => {
                if rom_type != 0xF5 && rom_type != 0xF9 {
                    return Err("SPC7110 mapper requires ROM type 0xF5 or 0xF9".to_string());
                }
            }
            MapperType::Sdd1 => {
                if rom_type != 0x43 && rom_type != 0x45 {
                    return Err("S-DD1 mapper requires ROM type 0x43 or 0x45".to_string());
                }
            }
            MapperType::Dsp1 | MapperType::Dsp1HiRom | MapperType::Dsp3 => {
                if rom_type != 0x03 && rom_type != 0x05 {
                    return Err(format!(
                        "DSP mapper requires ROM type 0x03 or 0x05, got 0x{:02X}",
                        rom_type
                    ));
                }
            }
            _ => {
                if has_enhancement_chip && !crate::debug_flags::quiet() {
                    println!("Warning: ROM has enhancement chip (type: 0x{:02X}) but using standard mapper", rom_type);
                }
            }
        }

        Ok(())
    }

    fn decode_rom_size(size_code: u8) -> Result<usize, String> {
        match size_code {
            0x08 => Ok(256 * 1024),  // 256KB
            0x09 => Ok(512 * 1024),  // 512KB
            0x0A => Ok(1024 * 1024), // 1MB
            0x0B => Ok(2048 * 1024), // 2MB
            0x0C => Ok(4096 * 1024), // 4MB
            0x0D => Ok(8192 * 1024), // 8MB
            _ => {
                if size_code <= 0x0F {
                    // Try standard formula: 1KB << size_code
                    Ok(1024 << size_code)
                } else {
                    Err(format!("Invalid ROM size code: 0x{:02X}", size_code))
                }
            }
        }
    }

    fn decode_ram_size(size_code: u8) -> usize {
        // RAM size is encoded as "1 << N kilobytes" (with 0 meaning no RAM).
        // Examples:
        //   N=1 => 2KB
        //   N=3 => 8KB
        //   N=5 => 32KB
        // Some ROMs use 0xFF as "unknown/none"; treat as no RAM.
        if size_code == 0x00 || size_code == 0xFF {
            return 0;
        }
        let shift = size_code as usize;
        if shift >= usize::BITS as usize {
            return 0;
        }
        1024usize << shift
    }

    fn validate_rom_size(actual_size: usize, header_size: usize) -> Result<(), String> {
        // Allow some tolerance for header variations
        let tolerance = 0x200; // 512 bytes tolerance

        if (actual_size + tolerance < header_size || actual_size > header_size + tolerance)
            && !crate::debug_flags::quiet()
        {
            println!(
                "Warning: ROM file size ({} bytes) doesn't match header size ({} bytes)",
                actual_size, header_size
            );
        }
        Ok(())
    }

    fn validate_checksums(checksum: u16, checksum_complement: u16) -> Result<(), String> {
        // Debugフラグでチェックサムを無視できるようにする
        if std::env::var_os("ALLOW_BAD_CHECKSUM").is_some() {
            return Ok(());
        }

        // 多くの ROM（特にテストROM/改造ROM）ではヘッダの checksum/complement が壊れていることがある。
        // その場合でも実行自体は可能なので、デフォルトは警告に留める。
        //
        // 厳密に弾きたい場合だけ STRICT_CHECKSUM=1 を指定する。
        if checksum ^ checksum_complement != 0xFFFF {
            let strict = std::env::var("STRICT_CHECKSUM")
                .map(|v| v != "0" && v.to_lowercase() != "false")
                .unwrap_or(false);
            if strict {
                return Err(format!(
                    "Invalid checksums: 0x{:04X} ^ 0x{:04X} != 0xFFFF",
                    checksum, checksum_complement
                ));
            }
            if !crate::debug_flags::quiet() {
                println!(
                    "Warning: Invalid checksums: 0x{:04X} ^ 0x{:04X} != 0xFFFF (set STRICT_CHECKSUM=1 to reject)",
                    checksum, checksum_complement
                );
            }
        }
        Ok(())
    }

    fn calculate_rom_checksum(rom: &[u8]) -> u16 {
        let mut sum = 0u32;

        // Calculate checksum of entire ROM
        for &byte in rom.iter() {
            sum = sum.wrapping_add(byte as u32);
        }

        (sum & 0xFFFF) as u16
    }

    fn score_header(rom: &[u8], offset: usize) -> u32 {
        if offset + 0x2F >= rom.len() {
            return 0;
        }

        let mut score: u32 = 0;

        let checksum = ((rom[offset + 0x2F] as u16) << 8) | (rom[offset + 0x2E] as u16);
        let checksum_complement = ((rom[offset + 0x2D] as u16) << 8) | (rom[offset + 0x2C] as u16);

        if checksum ^ checksum_complement == 0xFFFF {
            score += 8;
        }

        // ROM type ($FFD6/$7FD6) is at base + 0x16; our offset is 0x10 bytes earlier.
        let rom_type = rom[offset + 0x26];
        if rom_type <= 0x37 {
            score += 2;
        }

        let rom_size = rom[offset + 0x27];
        if (0x08..=0x0D).contains(&rom_size) {
            score += 2;

            // Bonus if ROM size roughly matches file size
            let expected_size = 1024usize << rom_size;
            if rom.len() >= expected_size / 2 && rom.len() <= expected_size * 2 {
                score += 2;
            }
        }

        let ram_size = rom[offset + 0x28];
        if ram_size <= 0x08 || ram_size == 0xFF {
            score += 1;
        }

        let country = rom[offset + 0x29];
        if country <= 0x0D || country == 0xFF {
            score += 1;
        }

        // Check title for valid ASCII characters (title starts at base + 0x10)
        let title_valid = rom[offset + 0x10..offset + 0x10 + 21]
            .iter()
            .all(|&b| (0x20..=0x7E).contains(&b) || b == 0x00);
        if title_valid {
            score += 2;
        }

        // Penalize obviously invalid values
        if rom[offset + 0x26] == 0xFF || rom[offset + 0x2A] == 0xFF {
            score = score.saturating_sub(3);
        }

        score
    }

    pub fn read(&self, addr: u32, mapper: MapperType) -> u8 {
        let rom_addr = match mapper {
            MapperType::LoRom => mapper::map_lorom(addr),
            MapperType::HiRom => mapper::map_hirom(addr),
            MapperType::ExHiRom => mapper::map_exhirom(addr),
            _ => addr as usize,
        };

        if rom_addr < self.rom.len() {
            self.rom[rom_addr]
        } else {
            0xFF
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn determine_mapper_type_accepts_sdd1_variant_header() {
        assert_eq!(
            Cartridge::determine_mapper_type(0x45, MapperType::LoRom),
            MapperType::Sdd1
        );
    }

    #[test]
    fn determine_mapper_type_accepts_spc7110_rtc_variant_header() {
        assert_eq!(
            Cartridge::determine_mapper_type(0xF9, MapperType::HiRom),
            MapperType::Spc7110
        );
    }

    #[test]
    fn load_from_bytes_detects_sd_gundam_gx_dsp3_header() {
        let mut rom = vec![0u8; 0x10000];
        let header = 0x7FB0;
        let title = b"SD GUNDAMGX";
        rom[header + 0x10..header + 0x10 + title.len()].copy_from_slice(title);
        rom[header + 0x25] = 0x30;
        rom[header + 0x26] = 0x05;
        rom[header + 0x27] = 0x06;
        rom[header + 0x28] = 0x00;
        rom[header + 0x29] = 0x00;
        rom[header + 0x2A] = 0xB2;
        let checksum_base = rom
            .iter()
            .enumerate()
            .filter(|(idx, _)| !(header + 0x2C..=header + 0x2F).contains(idx))
            .fold(0u32, |sum, (_, byte)| sum.wrapping_add(*byte as u32));
        let checksum = checksum_base.wrapping_add(0x1FE) as u16;
        let checksum_complement = !checksum;
        rom[header + 0x2C] = checksum_complement as u8;
        rom[header + 0x2D] = (checksum_complement >> 8) as u8;
        rom[header + 0x2E] = checksum as u8;
        rom[header + 0x2F] = (checksum >> 8) as u8;

        let cartridge = Cartridge::load_from_bytes(rom).unwrap();

        assert_eq!(cartridge.header.mapper_type, MapperType::Dsp3);
    }

    #[test]
    fn validate_mapper_compatibility_accepts_variant_headers() {
        assert_eq!(
            Cartridge::determine_mapper_type(0x13, MapperType::LoRom),
            MapperType::SuperFx
        );
        assert!(Cartridge::validate_mapper_compatibility(0x45, &MapperType::Sdd1).is_ok());
        assert!(Cartridge::validate_mapper_compatibility(0xF9, &MapperType::Spc7110).is_ok());
        assert!(Cartridge::validate_mapper_compatibility(0x13, &MapperType::SuperFx).is_ok());
    }
}
