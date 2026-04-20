use std::cell::Cell;

use super::super::super::Cartridge;

fn irq_cell_pending<T>(mapper: Option<&T>, irq_pending: impl FnOnce(&T) -> &Cell<bool>) -> bool {
    mapper
        .map(|mapper| irq_pending(mapper).get())
        .unwrap_or(false)
}

#[cfg(test)]
fn clear_irq_cell<T>(mapper: Option<&T>, irq_pending: impl FnOnce(&T) -> &Cell<bool>) {
    if let Some(mapper) = mapper {
        irq_pending(mapper).set(false);
    }
}

impl Cartridge {
    pub fn irq_pending(&self) -> bool {
        if let Some(ref mmc5) = self.mappers.mmc5 {
            if mmc5.combined_irq_pending() {
                return true;
            }
        }

        if irq_cell_pending(self.mappers.jaleco_ss88006.as_ref(), |m| &m.irq_pending)
            || irq_cell_pending(self.mappers.namco163.as_ref(), |m| &m.irq_pending)
            || irq_cell_pending(self.mappers.mmc3.as_ref(), |m| &m.irq_pending)
            || irq_cell_pending(self.mappers.fme7.as_ref(), |m| &m.irq_pending)
            || irq_cell_pending(self.mappers.bandai_fcg.as_ref(), |m| &m.irq_pending)
            || irq_cell_pending(self.mappers.mapper40.as_ref(), |m| &m.irq_pending)
            || irq_cell_pending(self.mappers.mapper42.as_ref(), |m| &m.irq_pending)
            || irq_cell_pending(self.mappers.mapper43.as_ref(), |m| &m.irq_pending)
            || irq_cell_pending(self.mappers.mapper50.as_ref(), |m| &m.irq_pending)
            || irq_cell_pending(self.mappers.sunsoft3.as_ref(), |m| &m.irq_pending)
            || irq_cell_pending(self.mappers.irem_h3001.as_ref(), |m| &m.irq_pending)
            || irq_cell_pending(self.mappers.vrc3.as_ref(), |m| &m.irq_pending)
            || irq_cell_pending(self.mappers.vrc2_vrc4.as_ref(), |m| &m.irq_pending)
            || irq_cell_pending(self.mappers.vrc6.as_ref(), |m| &m.irq_pending)
        {
            return true;
        }

        if self.uses_mapper48()
            && irq_cell_pending(self.mappers.taito_tc0190.as_ref(), |m| &m.irq_pending)
        {
            return true;
        }
        false
    }

    #[cfg(test)]
    pub fn acknowledge_irq(&self) {
        clear_irq_cell(self.mappers.jaleco_ss88006.as_ref(), |m| &m.irq_pending);
        clear_irq_cell(self.mappers.namco163.as_ref(), |m| &m.irq_pending);
        clear_irq_cell(self.mappers.mmc3.as_ref(), |m| &m.irq_pending);
        clear_irq_cell(self.mappers.fme7.as_ref(), |m| &m.irq_pending);
        clear_irq_cell(self.mappers.bandai_fcg.as_ref(), |m| &m.irq_pending);
        clear_irq_cell(self.mappers.mapper40.as_ref(), |m| &m.irq_pending);
        clear_irq_cell(self.mappers.mapper42.as_ref(), |m| &m.irq_pending);
        clear_irq_cell(self.mappers.mapper43.as_ref(), |m| &m.irq_pending);
        clear_irq_cell(self.mappers.mapper50.as_ref(), |m| &m.irq_pending);
        clear_irq_cell(self.mappers.sunsoft3.as_ref(), |m| &m.irq_pending);
        clear_irq_cell(self.mappers.irem_h3001.as_ref(), |m| &m.irq_pending);
        clear_irq_cell(self.mappers.vrc3.as_ref(), |m| &m.irq_pending);
        clear_irq_cell(self.mappers.vrc2_vrc4.as_ref(), |m| &m.irq_pending);
        clear_irq_cell(self.mappers.vrc6.as_ref(), |m| &m.irq_pending);

        if self.uses_mapper48() {
            clear_irq_cell(self.mappers.taito_tc0190.as_ref(), |m| &m.irq_pending);
        }
    }
}
