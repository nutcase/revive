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
pub struct Controller {
    up: bool,
    down: bool,
    left: bool,
    right: bool,
    button1: bool,
    button2: bool,
}

impl Controller {
    pub fn set(&mut self, button: Button, pressed: bool) {
        match button {
            Button::Up => self.up = pressed,
            Button::Down => self.down = pressed,
            Button::Left => self.left = pressed,
            Button::Right => self.right = pressed,
            Button::Button1 => self.button1 = pressed,
            Button::Button2 => self.button2 = pressed,
        }
    }

    pub fn active_low_bits(self) -> u8 {
        active_low(self.up)
            | (active_low(self.down) << 1)
            | (active_low(self.left) << 2)
            | (active_low(self.right) << 3)
            | (active_low(self.button1) << 4)
            | (active_low(self.button2) << 5)
    }

    pub fn up(self) -> bool {
        self.up
    }

    pub fn down(self) -> bool {
        self.down
    }

    pub fn left(self) -> bool {
        self.left
    }

    pub fn right(self) -> bool {
        self.right
    }

    pub fn button1(self) -> bool {
        self.button1
    }

    pub fn button2(self) -> bool {
        self.button2
    }
}

pub fn active_low(pressed: bool) -> u8 {
    if pressed { 0 } else { 1 }
}
