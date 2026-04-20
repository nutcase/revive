use super::super::super::super::Cartridge;

impl Cartridge {
    pub(crate) fn notify_ppuctrl_mmc5(&mut self, data: u8) {
        if let Some(mmc5) = self.mappers.mmc5.as_ref() {
            mmc5.ppu_ctrl.set(data);
        }
    }

    pub(crate) fn notify_ppumask_mmc5(&mut self, data: u8) {
        if let Some(mmc5) = self.mappers.mmc5.as_ref() {
            mmc5.ppu_mask.set(data);
            if data & 0x18 == 0 {
                mmc5.in_frame.set(false);
                mmc5.scanline_counter.set(0);
            }
        }
    }

    pub(crate) fn mmc5_scanline_tick(&self) {
        let Some(mmc5) = self.mappers.mmc5.as_ref() else {
            return;
        };
        if !mmc5.substitutions_enabled() {
            return;
        }
        let next_scanline = if mmc5.in_frame.get() {
            mmc5.scanline_counter.get().wrapping_add(1)
        } else {
            mmc5.in_frame.set(true);
            0
        };
        mmc5.scanline_counter.set(next_scanline);
        if mmc5.irq_enabled
            && mmc5.irq_scanline_compare != 0
            && next_scanline == mmc5.irq_scanline_compare
        {
            mmc5.irq_pending.set(true);
        }
    }

    pub(crate) fn mmc5_end_frame(&self) {
        if let Some(mmc5) = self.mappers.mmc5.as_ref() {
            mmc5.in_frame.set(false);
            mmc5.scanline_counter.set(0);
        }
    }
}
