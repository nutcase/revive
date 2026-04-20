use super::super::super::Cartridge;

#[derive(Clone, Copy)]
enum Mmc3IrqClockHandler {
    Rambo1Scanline,
    Mmc3A,
    Standard,
    None,
}

impl Cartridge {
    fn clock_irq_mmc3a(&mut self) {
        if let Some(ref mut mmc3) = self.mappers.mmc3 {
            let counter_was_zero = mmc3.irq_counter == 0;
            if counter_was_zero || mmc3.irq_reload {
                mmc3.irq_counter = mmc3.irq_latch;
                mmc3.irq_reload = false;
            } else {
                mmc3.irq_counter = mmc3.irq_counter.wrapping_sub(1);
            }

            if mmc3.irq_counter == 0 && mmc3.irq_enabled && mmc3.irq_latch != 0 {
                mmc3.irq_pending.set(true);
            }
        }
    }

    pub fn clock_irq_counter(&mut self) {
        match self.mmc3_irq_clock_handler() {
            Mmc3IrqClockHandler::Rambo1Scanline => {
                if let Some(ref mut mmc3) = self.mappers.mmc3 {
                    if !mmc3.irq_cycle_mode {
                        mmc3.clock_irq_rambo1_mut();
                    }
                }
            }
            Mmc3IrqClockHandler::Mmc3A => self.clock_irq_mmc3a(),
            Mmc3IrqClockHandler::Standard => {
                if let Some(ref mut mmc3) = self.mappers.mmc3 {
                    mmc3.clock_irq_mut();
                }
            }
            Mmc3IrqClockHandler::None => {}
        }
        self.clock_mapper48_scanline_irq();
    }

    fn mmc3_irq_clock_handler(&self) -> Mmc3IrqClockHandler {
        if self.mappers.mmc3.is_none() {
            return Mmc3IrqClockHandler::None;
        }

        match self.mapper {
            64 => Mmc3IrqClockHandler::Rambo1Scanline,
            114 | 182 => Mmc3IrqClockHandler::Mmc3A,
            _ => Mmc3IrqClockHandler::Standard,
        }
    }

    fn clock_mapper48_scanline_irq(&mut self) {
        if self.uses_mapper48() {
            if let Some(ref mut taito) = self.mappers.taito_tc0190 {
                taito.clock_irq_mut();
            }
        }
    }

    pub fn clock_irq_counter_cycles(&mut self, cycles: u32) {
        self.clock_mapper_specific_irq_cycles(cycles);

        if let Some(ref mut fme7) = self.mappers.fme7 {
            for _ in 0..cycles {
                fme7.clock_irq_mut();
            }
        }
        if let Some(ref mut bandai) = self.mappers.bandai_fcg {
            for _ in 0..cycles {
                bandai.clock_irq_mut();
            }
        }
        if let Some(ref mut mapper40) = self.mappers.mapper40 {
            mapper40.clock_irq_mut(cycles);
        }
        if let Some(ref mut mapper42) = self.mappers.mapper42 {
            mapper42.clock_irq_mut(cycles);
        }
        if let Some(ref mut mapper43) = self.mappers.mapper43 {
            mapper43.clock_irq_mut(cycles);
        }
        if let Some(ref mut mapper50) = self.mappers.mapper50 {
            mapper50.clock_irq_mut(cycles);
        }
        if let Some(ref mut sunsoft3) = self.mappers.sunsoft3 {
            sunsoft3.clock_irq_mut(cycles);
        }
        if let Some(ref mut h3001) = self.mappers.irem_h3001 {
            h3001.clock_irq_mut(cycles);
        }
        if let Some(ref mut vrc3) = self.mappers.vrc3 {
            vrc3.clock_irq_mut(cycles);
        }
    }

    fn clock_mapper_specific_irq_cycles(&mut self, cycles: u32) {
        match self.mapper {
            64 => {
                if let Some(ref mut mmc3) = self.mappers.mmc3 {
                    for _ in 0..cycles {
                        mmc3.clock_irq_rambo1_cycle();
                    }
                }
            }
            18 => self.clock_irq_mapper18(cycles),
            19 => self.clock_irq_namco163(cycles),
            21 => self.clock_irq_mapper21(cycles),
            23 => self.clock_irq_mapper23(cycles),
            25 => self.clock_irq_mapper25(cycles),
            _ => {}
        }

        if self.uses_vrc6() {
            self.clock_irq_vrc6(cycles);
        }
        if self.uses_mapper48() {
            if let Some(ref mut taito) = self.mappers.taito_tc0190 {
                taito.clock_irq_delay_mut(cycles);
            }
        }
    }
}
