use crate::bus::{GbBus, INT_TIMER};

#[derive(Debug, Default)]
pub struct GbTimer {
    div_counter: u16,
    last_tac: u8,
    tima_reload_delay: u8,
}

impl GbTimer {
    pub fn reset(&mut self) {
        self.div_counter = 0;
        self.last_tac = 0;
        self.tima_reload_delay = 0;
    }

    pub fn step(&mut self, cycles: u32, bus: &mut GbBus) {
        self.handle_tac_write_side_effect(bus);
        if bus.take_div_reset_request() {
            self.handle_div_reset(bus);
        }
        for _ in 0..cycles {
            self.step_cycle(bus);
        }
        self.last_tac = bus.timer_tac();
    }

    fn step_cycle(&mut self, bus: &mut GbBus) {
        self.step_tima_reload(bus);

        let tac = bus.timer_tac();
        let old_input = timer_input_high(tac, self.div_counter);

        self.div_counter = self.div_counter.wrapping_add(1);
        bus.set_timer_div((self.div_counter >> 8) as u8);

        let new_input = timer_input_high(tac, self.div_counter);
        if old_input && !new_input {
            self.increment_tima(bus);
        }
    }

    fn handle_div_reset(&mut self, bus: &mut GbBus) {
        let tac = bus.timer_tac();
        let old_input = timer_input_high(tac, self.div_counter);

        self.div_counter = 0;
        bus.set_timer_div(0);

        let new_input = timer_input_high(tac, self.div_counter);
        if old_input && !new_input {
            self.increment_tima(bus);
        }
    }

    fn handle_tac_write_side_effect(&mut self, bus: &mut GbBus) {
        let tac = bus.timer_tac();
        if tac == self.last_tac {
            return;
        }

        let old_input = timer_input_high(self.last_tac, self.div_counter);
        let new_input = timer_input_high(tac, self.div_counter);
        if old_input && !new_input {
            self.increment_tima(bus);
        }
        self.last_tac = tac;
    }

    fn increment_tima(&mut self, bus: &mut GbBus) {
        let tima = bus.timer_tima();
        if tima == 0xFF {
            bus.set_timer_tima(0x00);
            if self.tima_reload_delay == 0 {
                // Hardware reload/IRQ occurs 4 T-cycles after overflow.
                self.tima_reload_delay = 4;
            }
        } else {
            bus.set_timer_tima(tima.wrapping_add(1));
        }
    }

    fn step_tima_reload(&mut self, bus: &mut GbBus) {
        if self.tima_reload_delay == 0 {
            return;
        }
        self.tima_reload_delay -= 1;
        if self.tima_reload_delay == 0 {
            bus.set_timer_tima(bus.timer_tma());
            bus.request_interrupt(INT_TIMER);
        }
    }
}

fn timer_input_high(tac: u8, div_counter: u16) -> bool {
    if (tac & 0x04) == 0 {
        return false;
    }
    let bit = match tac & 0x03 {
        0x00 => 9, // 4096 Hz
        0x01 => 3, // 262144 Hz
        0x02 => 5, // 65536 Hz
        _ => 7,    // 16384 Hz
    };
    ((div_counter >> bit) & 1) != 0
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
    fn div_increments_every_256_cycles() {
        let mut bus = GbBus::default();
        bus.load_cartridge(&make_test_rom())
            .expect("cartridge should load");
        let mut timer = GbTimer::default();

        timer.step(255, &mut bus);
        assert_eq!(bus.read8(0xFF04), 0);

        timer.step(1, &mut bus);
        assert_eq!(bus.read8(0xFF04), 1);
    }

    #[test]
    fn tima_overflow_reloads_tma_and_requests_interrupt() {
        let mut bus = GbBus::default();
        bus.load_cartridge(&make_test_rom())
            .expect("cartridge should load");
        let mut timer = GbTimer::default();

        bus.write8(0xFFFF, INT_TIMER);
        bus.write8(0xFF06, 0xAB);
        bus.write8(0xFF05, 0xFF);
        bus.write8(0xFF07, 0x05);

        timer.step(16, &mut bus);
        assert_eq!(bus.read8(0xFF05), 0x00);
        assert_eq!(bus.pending_interrupts() & INT_TIMER, 0);

        timer.step(4, &mut bus);

        assert_eq!(bus.read8(0xFF05), 0xAB);
        assert_eq!(bus.pending_interrupts() & INT_TIMER, INT_TIMER);
    }

    #[test]
    fn div_write_resets_internal_div_counter() {
        let mut bus = GbBus::default();
        bus.load_cartridge(&make_test_rom())
            .expect("cartridge should load");
        let mut timer = GbTimer::default();

        timer.step(255, &mut bus);
        bus.write8(0xFF04, 0x99);
        timer.step(1, &mut bus);

        assert_eq!(bus.read8(0xFF04), 0);
    }

    #[test]
    fn div_reset_can_increment_tima_on_falling_edge() {
        let mut bus = GbBus::default();
        bus.load_cartridge(&make_test_rom())
            .expect("cartridge should load");
        let mut timer = GbTimer::default();

        bus.write8(0xFF07, 0x05);
        bus.write8(0xFF05, 0x10);
        timer.step(8, &mut bus);
        bus.write8(0xFF04, 0x00);
        timer.step(0, &mut bus);

        assert_eq!(bus.read8(0xFF05), 0x11);
    }

    #[test]
    fn tac_write_can_increment_tima_on_falling_edge() {
        let mut bus = GbBus::default();
        bus.load_cartridge(&make_test_rom())
            .expect("cartridge should load");
        let mut timer = GbTimer::default();

        bus.write8(0xFF07, 0x05);
        bus.write8(0xFF05, 0x22);
        timer.step(8, &mut bus);
        bus.write8(0xFF07, 0x04);
        timer.step(0, &mut bus);

        assert_eq!(bus.read8(0xFF05), 0x23);
    }
}
