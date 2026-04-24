pub use sega8_common::input::Button;

use sega8_common::input::{Controller, active_low};

#[derive(Debug, Clone, bincode::Encode, bincode::Decode)]
pub struct Input {
    pads: [Controller; 2],
}

impl Input {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_button_pressed(&mut self, player: u8, button: Button, pressed: bool) {
        let Some(index) = player.checked_sub(1).map(usize::from) else {
            return;
        };
        if let Some(pad) = self.pads.get_mut(index) {
            pad.set(button, pressed);
        }
    }

    pub fn read_port1(&self) -> u8 {
        let p1 = self.pads[0];
        let p2 = self.pads[1];
        p1.active_low_bits() | (active_low(p2.up()) << 6) | (active_low(p2.down()) << 7)
    }

    pub fn read_port2(&self) -> u8 {
        let p2 = self.pads[1];
        active_low(p2.left())
            | (active_low(p2.right()) << 1)
            | (active_low(p2.button1()) << 2)
            | (active_low(p2.button2()) << 3)
            | 0xF0
    }
}

impl Default for Input {
    fn default() -> Self {
        Self {
            pads: [Controller::default(), Controller::default()],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn port1_contains_player_one_and_player_two_vertical_bits() {
        let mut input = Input::new();
        input.set_button_pressed(1, Button::Button1, true);
        input.set_button_pressed(2, Button::Down, true);

        let value = input.read_port1();

        assert_eq!(value & 0x10, 0x00);
        assert_eq!(value & 0x80, 0x00);
        assert_eq!(value & 0x20, 0x20);
    }

    #[test]
    fn port2_contains_player_two_remaining_bits() {
        let mut input = Input::new();
        input.set_button_pressed(2, Button::Left, true);
        input.set_button_pressed(2, Button::Button2, true);

        let value = input.read_port2();

        assert_eq!(value & 0x01, 0x00);
        assert_eq!(value & 0x08, 0x00);
        assert_eq!(value & 0xF0, 0xF0);
    }
}
