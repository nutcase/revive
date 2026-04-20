use crate::bus::{GbBus, INT_SERIAL};

const SERIAL_TRANSFER_CYCLES_NORMAL: u32 = 4096;
const SERIAL_TRANSFER_CYCLES_FAST_CGB: u32 = 128;

#[derive(Debug, Default)]
pub struct GbSerial {
    transfer_cycle_acc: u32,
}

impl GbSerial {
    pub fn reset(&mut self) {
        self.transfer_cycle_acc = 0;
    }

    pub fn step(&mut self, cycles: u32, bus: &mut GbBus) {
        let sc = bus.serial_sc();
        let transfer_active = (sc & 0x80) != 0;
        let internal_clock = (sc & 0x01) != 0;

        if !transfer_active {
            self.transfer_cycle_acc = 0;
            return;
        }

        if !internal_clock {
            // External-clock transfer: keep waiting.
            return;
        }

        let transfer_cycles = if bus.cgb_mode() && (sc & 0x02) != 0 {
            SERIAL_TRANSFER_CYCLES_FAST_CGB
        } else {
            SERIAL_TRANSFER_CYCLES_NORMAL
        };

        self.transfer_cycle_acc = self.transfer_cycle_acc.saturating_add(cycles);
        if self.transfer_cycle_acc < transfer_cycles {
            return;
        }

        self.transfer_cycle_acc %= transfer_cycles;

        // No link partner: line idles high and receives 0xFF.
        bus.set_serial_sb(0xFF);
        bus.set_serial_sc(sc & 0x7F);
        bus.request_interrupt(INT_SERIAL);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_rom() -> Vec<u8> {
        let mut rom = vec![0; 0x8000];
        rom[0x0147] = 0x00;
        rom[0x0148] = 0x00;
        rom[0x0149] = 0x00;
        rom
    }

    #[test]
    fn internal_clock_transfer_completes_and_raises_irq() {
        let mut bus = GbBus::default();
        bus.load_cartridge(&make_test_rom())
            .expect("cartridge should load");
        bus.write8(0xFFFF, INT_SERIAL);
        bus.write8(0xFF01, 0x42);
        bus.write8(0xFF02, 0x81);

        let mut serial = GbSerial::default();
        serial.step(SERIAL_TRANSFER_CYCLES_NORMAL, &mut bus);

        assert_eq!(bus.read8(0xFF02) & 0x80, 0);
        assert_eq!(bus.read8(0xFF01), 0xFF);
        assert_eq!(bus.pending_interrupts() & INT_SERIAL, INT_SERIAL);
    }

    #[test]
    fn cgb_fast_internal_clock_transfer_completes_quickly() {
        let mut bus = GbBus::default();
        bus.set_cgb_mode(true);
        bus.load_cartridge(&make_test_rom())
            .expect("cartridge should load");
        bus.reset();
        bus.write8(0xFFFF, INT_SERIAL);
        bus.write8(0xFF01, 0x99);
        bus.write8(0xFF02, 0x83);

        let mut serial = GbSerial::default();
        serial.step(SERIAL_TRANSFER_CYCLES_FAST_CGB - 1, &mut bus);
        assert_eq!(bus.read8(0xFF02) & 0x80, 0x80);

        serial.step(1, &mut bus);
        assert_eq!(bus.read8(0xFF02) & 0x80, 0x00);
        assert_eq!(bus.read8(0xFF01), 0xFF);
        assert_eq!(bus.pending_interrupts() & INT_SERIAL, INT_SERIAL);
    }
}
