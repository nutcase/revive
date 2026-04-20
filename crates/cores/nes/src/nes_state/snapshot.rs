use std::time::{SystemTime, UNIX_EPOCH};

use crate::{save_state, Nes};

impl Nes {
    pub(in crate::nes_state) fn build_save_state(
        &self,
        rom_stem: &str,
    ) -> crate::Result<save_state::SaveState> {
        let apu_state = self.bus.get_apu_state();
        let ppu_registers = self.bus.ppu_register_state();
        let timing = self.bus.timing_state();

        Ok(save_state::SaveState {
            cpu_a: self.cpu.a,
            cpu_x: self.cpu.x,
            cpu_y: self.cpu.y,
            cpu_pc: self.cpu.pc,
            cpu_sp: self.cpu.sp,
            cpu_status: self.cpu.status.bits(),
            cpu_cycles: self.cpu.total_cycles(),
            cpu_halted: self.cpu.is_halted(),
            ppu_control: ppu_registers.control,
            ppu_mask: ppu_registers.mask,
            ppu_status: ppu_registers.status,
            ppu_oam_addr: ppu_registers.oam_addr,
            ppu_scroll_x: 0,
            ppu_scroll_y: 0,
            ppu_addr: ppu_registers.v,
            ppu_data_buffer: ppu_registers.read_buffer,
            ppu_w: ppu_registers.w,
            ppu_t: ppu_registers.t,
            ppu_v: ppu_registers.v,
            ppu_x: ppu_registers.x,
            ppu_scanline: ppu_registers.scanline,
            ppu_cycle: ppu_registers.cycle,
            ppu_frame: ppu_registers.frame,
            ppu_palette: self.bus.get_ppu_palette(),
            ppu_nametable: self.bus.get_ppu_nametables_flat(),
            ppu_oam: self.bus.get_ppu_oam_flat(),
            ram: self.bus.get_ram_flat(),
            cartridge_prg_bank: self.bus.get_cartridge_prg_bank(),
            cartridge_chr_bank: self.bus.get_cartridge_chr_bank(),
            cartridge_state: self.bus.get_cartridge_state(),
            apu_frame_counter: apu_state.frame_counter as u8,
            apu_frame_interrupt: apu_state.frame_irq,
            apu_state: Some(apu_state),
            rom_filename: rom_stem.to_string(),
            timestamp: SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
            bus_dma_cycles: timing.dma_cycles,
            bus_dma_in_progress: timing.dma_in_progress,
            bus_dmc_stall_cycles: timing.dmc_stall_cycles,
            ppu_frame_complete: timing.ppu_frame_complete,
        })
    }
}
