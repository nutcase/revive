use super::super::super::super::Cartridge;

impl Cartridge {
    pub(super) fn write_mmc5_control_register(&mut self, addr: u16, data: u8) {
        let Some(mmc5) = self.mappers.mmc5.as_mut() else {
            return;
        };

        match addr {
            0x5200 => mmc5.split_control = data,
            0x5201 => mmc5.split_scroll = data,
            0x5202 => mmc5.split_bank = data,
            0x5203 => mmc5.irq_scanline_compare = data,
            0x5204 => {
                mmc5.irq_enabled = data & 0x80 != 0;
                if !mmc5.irq_enabled {
                    mmc5.irq_pending.set(false);
                }
            }
            0x5205 => mmc5.multiplier_a = data,
            0x5206 => mmc5.multiplier_b = data,
            _ => {}
        }
    }

    pub(super) fn read_mmc5_control_register(&self, addr: u16) -> u8 {
        let Some(mmc5) = self.mappers.mmc5.as_ref() else {
            return 0;
        };

        match addr {
            0x5204 => {
                let mut status = 0;
                if mmc5.in_frame.get() {
                    status |= 0x40;
                }
                if mmc5.irq_pending.get() {
                    status |= 0x80;
                    mmc5.irq_pending.set(false);
                }
                status
            }
            0x5205 => {
                let product = (mmc5.multiplier_a as u16) * (mmc5.multiplier_b as u16);
                product as u8
            }
            0x5206 => {
                let product = (mmc5.multiplier_a as u16) * (mmc5.multiplier_b as u16);
                (product >> 8) as u8
            }
            _ => 0,
        }
    }
}
