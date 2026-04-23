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

    fn active_low_bits(self) -> u8 {
        active_low(self.up)
            | (active_low(self.down) << 1)
            | (active_low(self.left) << 2)
            | (active_low(self.right) << 3)
            | (active_low(self.button1) << 4)
            | (active_low(self.button2) << 5)
            | 0xC0
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
        self.pads[0].active_low_bits()
    }

    pub fn read_port2(&self) -> u8 {
        self.pads[1].active_low_bits()
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
