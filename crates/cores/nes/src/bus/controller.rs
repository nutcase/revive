use super::Bus;

const CONTROLLER_POST_READ_BITS: u16 = 0xFF00;

impl Bus {
    pub fn set_controller(&mut self, controller: u8) {
        self.controller = controller;
    }

    pub fn set_controller2(&mut self, controller: u8) {
        self.controller2 = controller;
    }

    pub(super) fn read_controller1(&mut self) -> u8 {
        self.read_controller_port(0)
    }

    pub(super) fn read_controller2(&mut self) -> u8 {
        self.read_controller_port(1)
    }

    fn read_controller_port(&mut self, port: usize) -> u8 {
        let controller = match port {
            0 => self.controller,
            1 => self.controller2,
            _ => 0,
        };

        if self.strobe {
            // While strobe is high, continuously reload and return bit 0 (A button).
            self.controller_state[port] = latched_controller_state(controller);
            return controller & 0x01;
        }

        let value = if self.controller_state[port] & 0x01 != 0 {
            0x01
        } else {
            0x00
        };
        self.controller_state[port] >>= 1;
        value
    }

    pub(super) fn write_controller_strobe(&mut self, data: u8) {
        let new_strobe = (data & 0x01) != 0;
        if self.strobe && !new_strobe {
            // Falling edge: latch both controller states.
            self.controller_state[0] = latched_controller_state(self.controller);
            self.controller_state[1] = latched_controller_state(self.controller2);
        }
        self.strobe = new_strobe;

        if let Some(ref mut cartridge) = self.cartridge {
            cartridge.write_prg_low(0x4016, data);
        }
    }
}

fn latched_controller_state(controller: u8) -> u16 {
    CONTROLLER_POST_READ_BITS | u16::from(controller)
}

#[cfg(test)]
mod tests {
    use super::Bus;
    use crate::cpu::CpuBus;

    #[test]
    fn controller_ports_latch_and_shift_independently() {
        let mut bus = Bus::new();
        bus.set_controller(0b1000_0001);
        bus.set_controller2(0b0100_0010);

        bus.write_controller_strobe(1);
        bus.write_controller_strobe(0);

        let p1: Vec<u8> = (0..8).map(|_| bus.read_controller1()).collect();
        let p2: Vec<u8> = (0..8).map(|_| bus.read_controller2()).collect();

        assert_eq!(p1, [1, 0, 0, 0, 0, 0, 0, 1]);
        assert_eq!(p2, [0, 1, 0, 0, 0, 0, 1, 0]);
        assert_eq!(bus.read_controller1(), 1);
        assert_eq!(bus.read_controller2(), 1);
    }

    #[test]
    fn cpu_bus_reads_second_controller_from_4017() {
        let mut bus = Bus::new();
        bus.set_controller2(0x01);

        bus.write(0x4016, 1);
        bus.write(0x4016, 0);

        assert_eq!(bus.read(0x4017) & 0x01, 0x01);
    }
}
