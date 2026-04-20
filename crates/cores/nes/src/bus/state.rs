use super::Bus;
use crate::apu::ApuState;
use crate::cartridge::CartridgeState;
use crate::ppu::PpuRegisterState;

pub struct BusFlatState<'a> {
    pub palette: &'a [u8],
    pub nametables: &'a [u8],
    pub oam: &'a [u8],
    pub ram: &'a [u8],
    pub prg_bank: u8,
    pub chr_bank: u8,
    pub ppu_registers: Option<PpuRegisterState>,
}

impl Bus {
    pub fn get_sram_data(&self) -> Option<Vec<u8>> {
        if let Some(ref cartridge) = self.cartridge {
            cartridge.get_sram_data().map(|data| data.to_vec())
        } else {
            None
        }
    }

    pub fn ppu_register_state(&self) -> PpuRegisterState {
        self.ppu.register_state()
    }

    pub fn get_ppu_palette(&self) -> [u8; 32] {
        self.ppu.get_palette()
    }

    pub fn get_ppu_nametables_flat(&self) -> Vec<u8> {
        let nt = self.ppu.get_nametable();
        let mut data = Vec::with_capacity(2048);
        data.extend_from_slice(&nt[0]);
        data.extend_from_slice(&nt[1]);
        data
    }

    pub fn get_ppu_oam_flat(&self) -> Vec<u8> {
        self.ppu.get_oam().to_vec()
    }

    pub fn get_ram_flat(&self) -> Vec<u8> {
        self.memory.get_ram().to_vec()
    }

    pub fn get_cartridge_prg_bank(&self) -> u8 {
        if let Some(ref cartridge) = self.cartridge {
            cartridge.get_prg_bank()
        } else {
            0
        }
    }

    pub fn get_cartridge_chr_bank(&self) -> u8 {
        if let Some(ref cartridge) = self.cartridge {
            cartridge.get_chr_bank()
        } else {
            0
        }
    }

    pub fn get_cartridge_state(&self) -> Option<CartridgeState> {
        self.cartridge.as_ref().map(|c| c.snapshot_state())
    }

    pub fn get_apu_state(&self) -> ApuState {
        self.apu.snapshot_state()
    }

    pub fn restore_cartridge_state(&mut self, state: &CartridgeState) {
        if let Some(ref mut cartridge) = self.cartridge {
            cartridge.restore_state(state);
        }
    }

    pub fn restore_apu_state(&mut self, state: &ApuState) {
        self.apu.restore_state(state);
        self.dmc_stall_cycles = 0;
    }

    pub fn restore_legacy_apu_state(&mut self, frame_counter: u8, frame_irq: bool) {
        self.apu.restore_legacy_state(frame_counter, frame_irq);
        self.dmc_stall_cycles = 0;
    }

    pub fn restore_state_flat(&mut self, state: BusFlatState<'_>) -> crate::Result<()> {
        let ram = state.ram;
        let palette = state.palette;
        let nametables = state.nametables;
        let oam = state.oam;

        // Restore RAM
        if ram.len() >= 0x800 {
            let mut ram_array = [0u8; 0x800];
            ram_array.copy_from_slice(&ram[..0x800]);
            self.memory.set_ram(ram_array);
        }

        // Restore PPU palette
        if palette.len() >= 32 {
            let mut pal = [0u8; 32];
            pal.copy_from_slice(&palette[..32]);
            self.ppu.set_palette(pal);
        }

        // Restore PPU nametables
        if nametables.len() >= 2048 {
            let mut nt = [[0u8; 1024]; 2];
            nt[0].copy_from_slice(&nametables[..1024]);
            nt[1].copy_from_slice(&nametables[1024..2048]);
            self.ppu.set_nametable(nt);
        }

        // Restore PPU OAM
        if oam.len() >= 256 {
            let mut oam_array = [0u8; 256];
            oam_array.copy_from_slice(&oam[..256]);
            self.ppu.set_oam(oam_array);
        }

        // Restore PPU registers
        if let Some(registers) = state.ppu_registers {
            self.ppu.restore_registers(registers);
        }

        // Restore cartridge bank state
        if let Some(ref mut cartridge) = self.cartridge {
            cartridge.set_prg_bank(state.prg_bank);
            cartridge.set_chr_bank(state.chr_bank);
        }

        Ok(())
    }

    /// Direct reference to CPU RAM (2KB).
    pub fn ram_ref(&self) -> &[u8] {
        &self.memory.ram
    }

    /// Mutable reference to CPU RAM (2KB).
    pub fn ram_mut(&mut self) -> &mut [u8] {
        &mut self.memory.ram
    }

    /// Direct reference to PRG-RAM / SRAM (mapper-dependent).
    pub fn prg_ram_ref(&self) -> Option<&[u8]> {
        self.cartridge.as_ref().and_then(|c| c.prg_ram_ref())
    }

    /// Mutable reference to PRG-RAM / SRAM.
    pub fn prg_ram_mut(&mut self) -> Option<&mut [u8]> {
        self.cartridge.as_mut().and_then(|c| c.prg_ram_mut())
    }
}
