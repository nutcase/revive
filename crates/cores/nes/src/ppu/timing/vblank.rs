use super::super::{mapper_hooks, Ppu, PpuControl, PpuStatus};

impl Ppu {
    pub(super) fn step_post_render_scanline(
        &mut self,
        cartridge: Option<&crate::cartridge::Cartridge>,
    ) {
        // Post-render scanline - no sprite evaluation needed here anymore
        // Sprite evaluation is now done at the start of each visible scanline
        if self.cycle == 0 {
            mapper_hooks::end_mmc5_frame(cartridge);
        }
    }

    pub(super) fn step_vblank_start(&mut self) -> bool {
        if self.cycle != 1 {
            return false;
        }

        self.vblank_flag_set_this_frame = true;
        self.status.insert(PpuStatus::VBLANK);

        let should_nmi = self.control.contains(PpuControl::NMI_ENABLE) && !self.nmi_suppressed;

        self.nmi_suppressed = false;
        should_nmi
    }
}
