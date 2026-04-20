mod mappers;
mod mmc;

use super::super::Cartridge;
use super::types::*;

impl Cartridge {
    pub fn snapshot_state(&self) -> CartridgeState {
        let mmc1 = self.snapshot_mmc1_state();
        let mmc2 = self.snapshot_mmc2_state();
        let mmc3 = self.snapshot_mmc3_state();
        let mmc5 = self.snapshot_mmc5_state();

        self.build_cartridge_state(mmc1, mmc2, mmc3, mmc5)
    }
}
