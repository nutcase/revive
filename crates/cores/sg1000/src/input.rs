pub use sega8_common::input::Button;

use sega8_common::input::Controller;

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
        self.pads[0].active_low_bits() | 0xC0
    }

    pub fn read_port2(&self) -> u8 {
        self.pads[1].active_low_bits() | 0xC0
    }
}

impl Default for Input {
    fn default() -> Self {
        Self {
            pads: [Controller::default(), Controller::default()],
        }
    }
}
