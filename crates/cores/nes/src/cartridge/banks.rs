use super::Cartridge;

impl Cartridge {
    pub fn get_prg_bank(&self) -> u8 {
        if let Some(ref mmc1) = self.mappers.mmc1 {
            mmc1.prg_bank
        } else if let Some(ref g101) = self.mappers.irem_g101 {
            g101.prg_banks[0]
        } else if let Some(ref h3001) = self.mappers.irem_h3001 {
            h3001.prg_banks[0]
        } else if let Some(ref mapper210) = self.mappers.namco210 {
            mapper210.prg_banks[0]
        } else if let Some(ref vrc2_vrc4) = self.mappers.vrc2_vrc4 {
            vrc2_vrc4.prg_banks[0]
        } else if let Some(ref vrc6) = self.mappers.vrc6 {
            vrc6.prg_bank_16k
        } else if let Some(ref mapper18) = self.mappers.jaleco_ss88006 {
            mapper18.prg_banks[0]
        } else if let Some(ref vrc1) = self.mappers.vrc1 {
            vrc1.prg_banks[0]
        } else if let Some(ref sunsoft3) = self.mappers.sunsoft3 {
            sunsoft3.prg_bank
        } else if let Some(ref sunsoft4) = self.mappers.sunsoft4 {
            sunsoft4.prg_bank
        } else if let Some(ref taito_tc0190) = self.mappers.taito_tc0190 {
            taito_tc0190.prg_banks[0]
        } else if let Some(ref taito_x1005) = self.mappers.taito_x1005 {
            taito_x1005.prg_banks[0]
        } else if let Some(ref taito_x1017) = self.mappers.taito_x1017 {
            taito_x1017.prg_banks[0]
        } else {
            self.prg_bank
        }
    }

    pub fn get_chr_bank(&self) -> u8 {
        if let Some(ref mmc1) = self.mappers.mmc1 {
            mmc1.chr_bank_0
        } else if let Some(ref g101) = self.mappers.irem_g101 {
            g101.chr_banks[0]
        } else if let Some(ref h3001) = self.mappers.irem_h3001 {
            h3001.chr_banks[0]
        } else if let Some(ref mapper210) = self.mappers.namco210 {
            mapper210.chr_banks[0]
        } else if let Some(ref vrc2_vrc4) = self.mappers.vrc2_vrc4 {
            vrc2_vrc4.chr_banks[0] as u8
        } else if let Some(ref vrc6) = self.mappers.vrc6 {
            vrc6.chr_banks[0]
        } else if let Some(ref mapper18) = self.mappers.jaleco_ss88006 {
            mapper18.chr_banks[0]
        } else if let Some(ref vrc1) = self.mappers.vrc1 {
            vrc1.chr_bank_0
        } else if let Some(ref sunsoft3) = self.mappers.sunsoft3 {
            sunsoft3.chr_banks[0]
        } else if let Some(ref sunsoft4) = self.mappers.sunsoft4 {
            sunsoft4.chr_banks[0]
        } else if let Some(ref taito_tc0190) = self.mappers.taito_tc0190 {
            taito_tc0190.chr_banks[0]
        } else if let Some(ref taito_x1005) = self.mappers.taito_x1005 {
            taito_x1005.chr_banks[0]
        } else if let Some(ref taito_x1017) = self.mappers.taito_x1017 {
            taito_x1017.chr_banks[0]
        } else {
            self.chr_bank
        }
    }

    pub fn set_prg_bank(&mut self, bank: u8) {
        self.prg_bank = bank;
        if let Some(ref mut mmc1) = self.mappers.mmc1 {
            mmc1.prg_bank = bank & 0x0F;
        }
        if let Some(ref mut g101) = self.mappers.irem_g101 {
            g101.prg_banks[0] = bank;
        }
        if let Some(ref mut h3001) = self.mappers.irem_h3001 {
            h3001.prg_banks[0] = bank;
        }
        if let Some(ref mut mapper210) = self.mappers.namco210 {
            mapper210.prg_banks[0] = bank;
        }
        if let Some(ref mut vrc2_vrc4) = self.mappers.vrc2_vrc4 {
            vrc2_vrc4.prg_banks[0] = bank & 0x1F;
        }
        if let Some(ref mut vrc6) = self.mappers.vrc6 {
            vrc6.prg_bank_16k = bank & 0x0F;
        }
        if let Some(ref mut mapper18) = self.mappers.jaleco_ss88006 {
            mapper18.prg_banks[0] = bank;
        }
        if let Some(ref mut vrc1) = self.mappers.vrc1 {
            vrc1.prg_banks[0] = bank & 0x0F;
        }
        if let Some(ref mut sunsoft3) = self.mappers.sunsoft3 {
            sunsoft3.prg_bank = bank & 0x0F;
        }
        if let Some(ref mut sunsoft4) = self.mappers.sunsoft4 {
            sunsoft4.prg_bank = bank & 0x0F;
        }
        if let Some(ref mut taito_tc0190) = self.mappers.taito_tc0190 {
            taito_tc0190.prg_banks[0] = bank;
        }
        if let Some(ref mut taito_x1005) = self.mappers.taito_x1005 {
            taito_x1005.prg_banks[0] = bank;
        }
        if let Some(ref mut taito_x1017) = self.mappers.taito_x1017 {
            taito_x1017.prg_banks[0] = bank;
        }
    }

    pub fn set_chr_bank(&mut self, bank: u8) {
        self.chr_bank = bank;
        if let Some(ref mut mmc1) = self.mappers.mmc1 {
            mmc1.chr_bank_0 = bank;
            mmc1.chr_bank_1 = bank;
        }
        if let Some(ref mut g101) = self.mappers.irem_g101 {
            g101.chr_banks[0] = bank;
        }
        if let Some(ref mut h3001) = self.mappers.irem_h3001 {
            h3001.chr_banks[0] = bank;
        }
        if let Some(ref mut mapper210) = self.mappers.namco210 {
            mapper210.chr_banks[0] = bank;
        }
        if let Some(ref mut vrc2_vrc4) = self.mappers.vrc2_vrc4 {
            vrc2_vrc4.chr_banks[0] = bank as u16;
        }
        if let Some(ref mut vrc6) = self.mappers.vrc6 {
            vrc6.chr_banks[0] = bank;
        }
        if let Some(ref mut mapper18) = self.mappers.jaleco_ss88006 {
            mapper18.chr_banks[0] = bank;
        }
        if let Some(ref mut vrc1) = self.mappers.vrc1 {
            vrc1.chr_bank_0 = bank & 0x1F;
        }
        if let Some(ref mut sunsoft3) = self.mappers.sunsoft3 {
            sunsoft3.chr_banks[0] = bank;
        }
        if let Some(ref mut sunsoft4) = self.mappers.sunsoft4 {
            sunsoft4.chr_banks[0] = bank;
        }
        if let Some(ref mut taito_tc0190) = self.mappers.taito_tc0190 {
            taito_tc0190.chr_banks[0] = bank;
        }
        if let Some(ref mut taito_x1005) = self.mappers.taito_x1005 {
            taito_x1005.chr_banks[0] = bank;
        }
        if let Some(ref mut taito_x1017) = self.mappers.taito_x1017 {
            taito_x1017.chr_banks[0] = bank;
        }
    }
}
