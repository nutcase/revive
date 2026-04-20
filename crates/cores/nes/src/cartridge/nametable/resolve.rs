use super::super::Cartridge;

impl Cartridge {
    pub(super) fn resolve_nametable_mapper137(&self, logical_nt: usize) -> Option<usize> {
        if (self.mappers.simple.mapper137_registers[7] >> 1) & 0x03 == 0 {
            Some(match logical_nt & 3 {
                0 => 0,
                _ => 1,
            })
        } else {
            None
        }
    }

    pub(super) fn resolve_nametable_mapper118(&self, logical_nt: usize) -> Option<usize> {
        let mmc3 = self.mappers.mmc3.as_ref()?;
        let chr_a12_invert = (mmc3.bank_select >> 7) & 1;
        let physical_nt = if chr_a12_invert == 0 {
            match logical_nt & 3 {
                0 | 1 => (mmc3.bank_registers[0] >> 7) as usize,
                2 | 3 => (mmc3.bank_registers[1] >> 7) as usize,
                _ => 0,
            }
        } else {
            match logical_nt & 3 {
                0 => (mmc3.bank_registers[2] >> 7) as usize,
                1 => (mmc3.bank_registers[3] >> 7) as usize,
                2 => (mmc3.bank_registers[4] >> 7) as usize,
                3 => (mmc3.bank_registers[5] >> 7) as usize,
                _ => 0,
            }
        };
        Some(physical_nt & 1)
    }

    pub(super) fn resolve_nametable_mapper207(&self, logical_nt: usize) -> Option<usize> {
        let taito = self.mappers.taito_x1005.as_ref()?;
        let physical_nt = match logical_nt & 3 {
            0 | 1 => (taito.chr_banks[0] >> 7) as usize,
            2 | 3 => (taito.chr_banks[1] >> 7) as usize,
            _ => 0,
        };
        Some(physical_nt & 1)
    }
}
