mod header;
mod instances;
mod memory;
mod spec;

use super::Cartridge;
use header::CartridgeHeader;
use instances::MapperInstances;
use memory::{allocate_chr_ram, allocate_prg_ram, load_chr_rom, load_prg_rom};
use spec::{MapperMemorySpec, MapperSpec};
use std::fs::File;
use std::io::{Read, Result};

impl Cartridge {
    pub fn load(path: &str) -> Result<Self> {
        let mut file = File::open(path)?;
        let mut data = Vec::new();
        file.read_to_end(&mut data)?;

        let header = CartridgeHeader::parse(&data)?;
        let mapper = header.mapper;
        let chr_rom_size = header.chr_rom_size;
        let has_battery = header.has_battery;
        let mapper34_nina001 = header.mapper34_nina001;
        let mapper93_chr_ram_enabled = header.mapper93_chr_ram_enabled;
        let mapper78_hv_mirroring = header.mapper78_hv_mirroring;
        let mapper236_chr_ram = header.mapper236_chr_ram;
        let mirroring = header.mirroring;
        let spec = MapperSpec::new(
            mapper,
            chr_rom_size,
            has_battery,
            mapper34_nina001,
            MapperMemorySpec {
                prg_ram_size: header.prg_ram_size,
                prg_nvram_size: header.prg_nvram_size,
                chr_ram_size: header.chr_ram_size,
                chr_nvram_size: header.chr_nvram_size,
            },
        );

        let prg_rom_start = 16 + header.trainer_size;
        let prg_rom = load_prg_rom(&data, header.prg_rom_size, prg_rom_start)?;
        let chr_rom_start = prg_rom_start + header.prg_rom_size;
        let chr_rom = load_chr_rom(&data, spec, chr_rom_start)?;

        let instances = MapperInstances::new(spec);

        let prg_ram = allocate_prg_ram(spec);
        let chr_ram = allocate_chr_ram(spec);
        let chr_bank_1 = spec.initial_chr_bank_1();

        let mut cart = Cartridge {
            prg_rom,
            chr_rom,
            chr_ram,
            prg_ram,
            has_valid_save_data: false,
            mapper,
            mirroring,
            has_battery,
            chr_bank: 0,
            chr_bank_1,
            prg_bank: spec.initial_prg_bank(),
            mappers: super::MapperRuntime {
                simple: super::SimpleMapperState::new(
                    mapper,
                    mapper34_nina001,
                    mapper93_chr_ram_enabled,
                    mapper78_hv_mirroring,
                ),
                multicart: super::MulticartMapperState::new(mapper, mapper236_chr_ram),
                mmc3_variant: super::Mmc3VariantState::new(mapper, chr_rom_size),
                mmc1: instances.mmc1,
                mmc2: instances.mmc2,
                mmc3: instances.mmc3,
                mmc5: instances.mmc5,
                namco163: instances.namco163,
                namco210: instances.namco210,
                jaleco_ss88006: instances.jaleco_ss88006,
                vrc2_vrc4: instances.vrc2_vrc4,
                mapper40: instances.mapper40,
                mapper42: instances.mapper42,
                mapper43: instances.mapper43,
                mapper50: instances.mapper50,
                fme7: instances.fme7,
                bandai_fcg: instances.bandai_fcg,
                irem_g101: instances.irem_g101,
                irem_h3001: instances.irem_h3001,
                vrc1: instances.vrc1,
                vrc3: instances.vrc3,
                vrc6: instances.vrc6,
                mapper15: instances.mapper15,
                sunsoft3: instances.sunsoft3,
                sunsoft4: instances.sunsoft4,
                taito_tc0190: instances.taito_tc0190,
                taito_x1005: instances.taito_x1005,
                taito_x1017: instances.taito_x1017,
                mapper246: instances.mapper246,
            },
        };
        if let Some(ref mut bandai) = cart.mappers.bandai_fcg {
            bandai.configure_mapper(mapper, has_battery);
        }
        Ok(cart)
    }
}

#[cfg(test)]
mod tests {
    use super::Cartridge;

    #[test]
    fn load_skips_ines_trainer_before_prg_rom() {
        let mut path = std::env::temp_dir();
        path.push(format!(
            "nes_emulator_trainer_test_{}_{}.nes",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));

        let mut rom = Vec::new();
        rom.extend_from_slice(b"NES\x1A");
        rom.push(1); // 16KB PRG
        rom.push(1); // 8KB CHR
        rom.push(0x04); // trainer present
        rom.extend_from_slice(&[0; 9]);
        rom.extend(std::iter::repeat_n(0xAA, 512));
        rom.extend(std::iter::repeat_n(0x11, 16 * 1024));
        rom.extend(std::iter::repeat_n(0x22, 8 * 1024));
        std::fs::write(&path, rom).unwrap();

        let cart = Cartridge::load(path.to_str().unwrap()).unwrap();

        assert_eq!(cart.prg_rom[0], 0x11);
        assert_eq!(cart.prg_rom[16 * 1024 - 1], 0x11);
        assert_eq!(cart.chr_rom[0], 0x22);
        assert_eq!(cart.chr_rom[8 * 1024 - 1], 0x22);

        let _ = std::fs::remove_file(path);
    }
}
