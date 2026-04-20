use crate::bus::{BusFlatState, BusTimingState};
use crate::cpu::StatusFlags;
use crate::{ppu, save_state::SaveState, Nes};

impl Nes {
    pub(in crate::nes_state) fn restore_save_state(
        &mut self,
        save_state: &SaveState,
    ) -> crate::Result<()> {
        self.restore_cpu_save_state(save_state);
        self.restore_bus_save_state(save_state)?;
        self.restore_cartridge_and_apu_save_state(save_state);
        self.restore_bus_timing_save_state(save_state);
        Ok(())
    }

    fn restore_cpu_save_state(&mut self, save_state: &SaveState) {
        self.cpu.a = save_state.cpu_a;
        self.cpu.x = save_state.cpu_x;
        self.cpu.y = save_state.cpu_y;
        self.cpu.pc = save_state.cpu_pc;
        self.cpu.sp = save_state.cpu_sp;
        self.cpu.status = StatusFlags::from_bits_truncate(save_state.cpu_status);
        self.cpu.set_halted(save_state.cpu_halted);
        self.cpu.set_total_cycles(save_state.cpu_cycles);
    }

    fn restore_bus_save_state(&mut self, save_state: &SaveState) -> crate::Result<()> {
        self.bus.restore_state_flat(BusFlatState {
            palette: &save_state.ppu_palette,
            nametables: &save_state.ppu_nametable,
            oam: &save_state.ppu_oam,
            ram: &save_state.ram,
            prg_bank: save_state.cartridge_prg_bank,
            chr_bank: save_state.cartridge_chr_bank,
            ppu_registers: Some(ppu_register_state_from_save(save_state)),
        })
    }

    fn restore_cartridge_and_apu_save_state(&mut self, save_state: &SaveState) {
        if let Some(ref state) = save_state.cartridge_state {
            self.bus.restore_cartridge_state(state);
        }
        if let Some(ref state) = save_state.apu_state {
            self.bus.restore_apu_state(state);
        } else {
            self.bus.restore_legacy_apu_state(
                save_state.apu_frame_counter,
                save_state.apu_frame_interrupt,
            );
        }
    }

    fn restore_bus_timing_save_state(&mut self, save_state: &SaveState) {
        self.bus.restore_timing_state(BusTimingState {
            dma_cycles: save_state.bus_dma_cycles,
            dma_in_progress: save_state.bus_dma_in_progress,
            dmc_stall_cycles: save_state.bus_dmc_stall_cycles,
            ppu_frame_complete: save_state.ppu_frame_complete,
        });
    }
}

fn ppu_register_state_from_save(save_state: &SaveState) -> ppu::PpuRegisterState {
    ppu::PpuRegisterState {
        control: save_state.ppu_control,
        mask: save_state.ppu_mask,
        status: save_state.ppu_status,
        oam_addr: save_state.ppu_oam_addr,
        v: save_state.ppu_v,
        t: save_state.ppu_t,
        x: save_state.ppu_x,
        w: save_state.ppu_w,
        scanline: save_state.ppu_scanline,
        cycle: save_state.ppu_cycle,
        frame: save_state.ppu_frame,
        read_buffer: save_state.ppu_data_buffer,
    }
}
