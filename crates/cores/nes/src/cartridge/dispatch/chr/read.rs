use crate::cartridge::Cartridge;

#[derive(Clone, Copy)]
enum ChrReadHandler {
    Mapper210,
    Mapper21,
    Mapper22,
    Mapper23,
    Vrc6,
    Mapper25,
    Mapper18,
    Namco163,
    Mmc5,
    Nrom,
    Mmc1,
    Uxrom,
    Mapper32,
    Mapper65,
    Mapper221,
    Mapper231,
    Mapper236,
    Vrc1,
    Cprom,
    Nina001,
    Split4k,
    TaitoTc0190,
    Cnrom,
    Mapper63,
    Mapper64,
    Mapper99,
    Mapper137,
    Mapper77,
    Sunsoft3,
    Mapper185,
    Mmc3,
    Mapper93,
    Mapper12,
    Mapper37,
    Mapper44,
    Mapper47,
    Mapper114,
    Mapper115,
    Mapper205,
    Mapper74,
    Mapper119,
    Mapper191,
    Mapper192,
    Mapper194,
    Mapper195,
    Mapper245,
    Mapper246,
    Sunsoft4,
    TaitoX1005,
    TaitoX1017,
    Namco108,
    Mmc2,
    Bandai,
    Fme7,
    RawChrRom,
}

impl Cartridge {
    #[inline]
    pub fn read_chr(&self, addr: u16) -> u8 {
        if self.is_nrom() {
            return self.read_chr_nrom(addr);
        }

        match self.chr_read_handler() {
            ChrReadHandler::Mapper210 => self.read_chr_mapper210(addr),
            ChrReadHandler::Mapper21 => self.read_chr_mapper21(addr),
            ChrReadHandler::Mapper22 => self.read_chr_mapper22(addr),
            ChrReadHandler::Mapper23 => self.read_chr_mapper23(addr),
            ChrReadHandler::Vrc6 => self.read_chr_vrc6(addr),
            ChrReadHandler::Mapper25 => self.read_chr_mapper25(addr),
            ChrReadHandler::Mapper18 => self.read_chr_mapper18(addr),
            ChrReadHandler::Namco163 => self.read_chr_namco163(addr),
            ChrReadHandler::Mmc5 => self.read_chr_mmc5(addr),
            ChrReadHandler::Nrom => self.read_chr_nrom(addr),
            ChrReadHandler::Mmc1 => self.read_chr_mmc1(addr),
            ChrReadHandler::Uxrom => self.read_chr_uxrom(addr),
            ChrReadHandler::Mapper32 => self.read_chr_mapper32(addr),
            ChrReadHandler::Mapper65 => self.read_chr_mapper65(addr),
            ChrReadHandler::Mapper221 => self.read_chr_mapper221(addr),
            ChrReadHandler::Mapper231 => self.read_chr_mapper231(addr),
            ChrReadHandler::Mapper236 => self.read_chr_mapper236(addr),
            ChrReadHandler::Vrc1 => self.read_chr_vrc1(addr),
            ChrReadHandler::Cprom => self.read_chr_cprom(addr),
            ChrReadHandler::Nina001 => self.read_chr_nina001(addr),
            ChrReadHandler::Split4k => self.read_chr_split_4k(addr),
            ChrReadHandler::TaitoTc0190 => self.read_chr_taito_tc0190(addr),
            ChrReadHandler::Cnrom => self.read_chr_cnrom(addr),
            ChrReadHandler::Mapper63 => self.read_chr_mapper63(addr),
            ChrReadHandler::Mapper64 => self.read_chr_mapper64(addr),
            ChrReadHandler::Mapper99 => self.read_chr_mapper99(addr),
            ChrReadHandler::Mapper137 => self.read_chr_mapper137(addr),
            ChrReadHandler::Mapper77 => self.read_chr_mapper77(addr),
            ChrReadHandler::Sunsoft3 => self.read_chr_sunsoft3(addr),
            ChrReadHandler::Mapper185 => self.read_chr_mapper185(addr),
            ChrReadHandler::Mmc3 => self.read_chr_mmc3(addr),
            ChrReadHandler::Mapper93 => self.read_chr_mapper93(addr),
            ChrReadHandler::Mapper12 => self.read_chr_mapper12(addr),
            ChrReadHandler::Mapper37 => self.read_chr_mapper37(addr),
            ChrReadHandler::Mapper44 => self.read_chr_mapper44(addr),
            ChrReadHandler::Mapper47 => self.read_chr_mapper47(addr),
            ChrReadHandler::Mapper114 => self.read_chr_mapper114(addr),
            ChrReadHandler::Mapper115 => self.read_chr_mapper115(addr),
            ChrReadHandler::Mapper205 => self.read_chr_mapper205(addr),
            ChrReadHandler::Mapper74 => self.read_chr_mapper74(addr),
            ChrReadHandler::Mapper119 => self.read_chr_mapper119(addr),
            ChrReadHandler::Mapper191 => self.read_chr_mapper191(addr),
            ChrReadHandler::Mapper192 => self.read_chr_mapper192(addr),
            ChrReadHandler::Mapper194 => self.read_chr_mapper194(addr),
            ChrReadHandler::Mapper195 => self.read_chr_mapper195(addr),
            ChrReadHandler::Mapper245 => self.read_chr_mapper245(addr),
            ChrReadHandler::Mapper246 => self.read_chr_mapper246(addr),
            ChrReadHandler::Sunsoft4 => self.read_chr_sunsoft4(addr),
            ChrReadHandler::TaitoX1005 => self.read_chr_taito_x1005(addr),
            ChrReadHandler::TaitoX1017 => self.read_chr_taito_x1017(addr),
            ChrReadHandler::Namco108 => self.read_chr_namco108(addr),
            ChrReadHandler::Mmc2 => self.read_chr_mmc2(addr),
            ChrReadHandler::Bandai => self.read_chr_bandai(addr),
            ChrReadHandler::Fme7 => self.read_chr_fme7(addr),
            ChrReadHandler::RawChrRom => {
                let chr_addr = (addr & 0x1FFF) as usize;
                if chr_addr < self.chr_rom.len() {
                    self.chr_rom[chr_addr]
                } else {
                    0
                }
            }
        }
    }

    fn chr_read_handler(&self) -> ChrReadHandler {
        match self.mapper {
            210 => ChrReadHandler::Mapper210,
            21 => ChrReadHandler::Mapper21,
            22 => ChrReadHandler::Mapper22,
            23 => ChrReadHandler::Mapper23,
            24 | 26 => ChrReadHandler::Vrc6,
            25 => ChrReadHandler::Mapper25,
            18 => ChrReadHandler::Mapper18,
            19 => ChrReadHandler::Namco163,
            5 => ChrReadHandler::Mmc5,
            34 if self.mappers.simple.mapper34_nina001 => ChrReadHandler::Nina001,
            0 | 34 | 43 | 97 | 103 | 142 | 226 | 241 | 242 => ChrReadHandler::Nrom,
            1 => ChrReadHandler::Mmc1,
            2 | 7 | 15 | 40 | 42 | 50 | 71 | 73 | 94 | 180 | 227 | 230 | 232 | 235 => {
                ChrReadHandler::Uxrom
            }
            32 => ChrReadHandler::Mapper32,
            65 => ChrReadHandler::Mapper65,
            221 => ChrReadHandler::Mapper221,
            231 => ChrReadHandler::Mapper231,
            236 => ChrReadHandler::Mapper236,
            75 | 151 => ChrReadHandler::Vrc1,
            13 => ChrReadHandler::Cprom,
            184 => ChrReadHandler::Split4k,
            33 | 48 => ChrReadHandler::TaitoTc0190,
            3 | 11 | 38 | 41 | 46 | 57 | 58 | 59 | 60 | 61 | 66 | 70 | 72 | 78 | 79 | 81 | 86
            | 87 | 89 | 92 | 101 | 107 | 113 | 133 | 140 | 144 | 145 | 146 | 147 | 148 | 150
            | 152 | 200 | 201 | 202 | 203 | 212 | 213 | 225 | 228 | 229 | 233 | 234 | 240 | 243
            | 255 => ChrReadHandler::Cnrom,
            63 => ChrReadHandler::Mapper63,
            64 => ChrReadHandler::Mapper64,
            99 => ChrReadHandler::Mapper99,
            137 => ChrReadHandler::Mapper137,
            77 => ChrReadHandler::Mapper77,
            67 => ChrReadHandler::Sunsoft3,
            185 => ChrReadHandler::Mapper185,
            4 | 118 | 123 | 189 | 208 | 250 => ChrReadHandler::Mmc3,
            93 => ChrReadHandler::Mapper93,
            12 => ChrReadHandler::Mapper12,
            37 => ChrReadHandler::Mapper37,
            44 => ChrReadHandler::Mapper44,
            47 => ChrReadHandler::Mapper47,
            114 | 182 => ChrReadHandler::Mapper114,
            115 | 248 => ChrReadHandler::Mapper115,
            205 => ChrReadHandler::Mapper205,
            74 => ChrReadHandler::Mapper74,
            119 => ChrReadHandler::Mapper119,
            191 => ChrReadHandler::Mapper191,
            192 => ChrReadHandler::Mapper192,
            194 => ChrReadHandler::Mapper194,
            195 => ChrReadHandler::Mapper195,
            245 => ChrReadHandler::Mapper245,
            246 => ChrReadHandler::Mapper246,
            68 => ChrReadHandler::Sunsoft4,
            80 | 207 => ChrReadHandler::TaitoX1005,
            82 => ChrReadHandler::TaitoX1017,
            76 | 88 | 95 | 112 | 154 | 206 => ChrReadHandler::Namco108,
            9 | 10 => ChrReadHandler::Mmc2,
            16 | 153 | 159 => ChrReadHandler::Bandai,
            69 => ChrReadHandler::Fme7,
            _ => ChrReadHandler::RawChrRom,
        }
    }

    pub fn read_chr_sprite(&self, addr: u16, _sprite_y: u8) -> u8 {
        if self.uses_mmc5() {
            self.read_chr_sprite_mmc5(addr, _sprite_y)
        } else {
            self.read_chr(addr)
        }
    }
}
