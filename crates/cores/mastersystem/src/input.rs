#[derive(Debug, Clone, Copy, PartialEq, Eq, bincode::Encode, bincode::Decode)]
pub enum Button {
    Up,
    Down,
    Left,
    Right,
    Button1,
    Button2,
}

#[derive(Debug, Clone, Copy, Default, bincode::Encode, bincode::Decode)]
struct Controller {
    up: bool,
    down: bool,
    left: bool,
    right: bool,
    button1: bool,
    button2: bool,
}

impl Controller {
    fn set(&mut self, button: Button, pressed: bool) {
        match button {
            Button::Up => self.up = pressed,
            Button::Down => self.down = pressed,
            Button::Left => self.left = pressed,
            Button::Right => self.right = pressed,
            Button::Button1 => self.button1 = pressed,
            Button::Button2 => self.button2 = pressed,
        }
    }
}

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
        active_low(p1.up)
            | (active_low(p1.down) << 1)
            | (active_low(p1.left) << 2)
            | (active_low(p1.right) << 3)
            | (active_low(p1.button1) << 4)
            | (active_low(p1.button2) << 5)
            | (active_low(p2.up) << 6)
            | (active_low(p2.down) << 7)
    }

    pub fn read_port2(&self) -> u8 {
        let p2 = self.pads[1];
        active_low(p2.left)
            | (active_low(p2.right) << 1)
            | (active_low(p2.button1) << 2)
            | (active_low(p2.button2) << 3)
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

fn active_low(pressed: bool) -> u8 {
    if pressed { 0 } else { 1 }
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
