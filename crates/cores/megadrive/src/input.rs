const TH_BIT: u8 = 0x40;
const SIX_BUTTON_TIMEOUT_CPU_CYCLES: u32 = 12_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, bincode::Encode, bincode::Decode)]
pub enum Button {
    Up,
    Down,
    Left,
    Right,
    A,
    B,
    C,
    Start,
    X,
    Y,
    Z,
    Mode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, bincode::Encode, bincode::Decode)]
pub enum ControllerType {
    ThreeButton,
    SixButton,
}

#[derive(Debug, Clone, Default, bincode::Encode, bincode::Decode)]
struct PadState {
    up: bool,
    down: bool,
    left: bool,
    right: bool,
    a: bool,
    b: bool,
    c: bool,
    start: bool,
    x: bool,
    y: bool,
    z: bool,
    mode: bool,
}

impl PadState {
    fn set_button(&mut self, button: Button, pressed: bool) {
        match button {
            Button::Up => self.up = pressed,
            Button::Down => self.down = pressed,
            Button::Left => self.left = pressed,
            Button::Right => self.right = pressed,
            Button::A => self.a = pressed,
            Button::B => self.b = pressed,
            Button::C => self.c = pressed,
            Button::Start => self.start = pressed,
            Button::X => self.x = pressed,
            Button::Y => self.y = pressed,
            Button::Z => self.z = pressed,
            Button::Mode => self.mode = pressed,
        }
    }
}

#[derive(Debug, Clone, Copy, bincode::Encode, bincode::Decode)]
struct PadProtocolState {
    controller_type: ControllerType,
    th_high: bool,
    rise_count: u8,
    idle_cycles: u32,
}

impl PadProtocolState {
    fn new(controller_type: ControllerType, th_high: bool) -> Self {
        Self {
            controller_type,
            th_high,
            rise_count: 0,
            idle_cycles: 0,
        }
    }

    fn set_controller_type(&mut self, controller_type: ControllerType, th_high: bool) {
        self.controller_type = controller_type;
        self.th_high = th_high;
        self.rise_count = 0;
        self.idle_cycles = 0;
    }

    fn observe_th_level(&mut self, th_high: bool) {
        if th_high == self.th_high {
            return;
        }

        // Six-button protocol advances on TH low->high transitions.
        if !self.th_high && th_high && matches!(self.controller_type, ControllerType::SixButton) {
            if self.idle_cycles >= SIX_BUTTON_TIMEOUT_CPU_CYCLES {
                self.rise_count = 0;
            }
            self.rise_count = (self.rise_count + 1) & 0x03;
            self.idle_cycles = 0;
        }

        self.th_high = th_high;
        if matches!(self.controller_type, ControllerType::ThreeButton) {
            self.rise_count = 0;
            self.idle_cycles = 0;
        }
    }

    fn step(&mut self, cpu_cycles: u32) {
        if !matches!(self.controller_type, ControllerType::SixButton) || self.rise_count == 0 {
            return;
        }
        self.idle_cycles = self.idle_cycles.saturating_add(cpu_cycles);
        if self.idle_cycles >= SIX_BUTTON_TIMEOUT_CPU_CYCLES {
            self.rise_count = 0;
            self.idle_cycles = 0;
        }
    }
}

#[derive(Debug, Clone, bincode::Encode, bincode::Decode)]
pub struct IoBus {
    version: u8,
    pad1: PadState,
    pad2: PadState,
    port1_data: u8,
    port1_ctrl: u8,
    port2_data: u8,
    port2_ctrl: u8,
    pad1_protocol: PadProtocolState,
    pad2_protocol: PadProtocolState,
}

impl Default for IoBus {
    fn default() -> Self {
        let port1_data = TH_BIT;
        let port1_ctrl = TH_BIT;
        let port2_data = TH_BIT;
        let port2_ctrl = TH_BIT;
        Self {
            // Default to JP/NTSC-compatible bits so JP-only ROMs can pass
            // early region checks during boot.
            version: 0x20,
            pad1: PadState::default(),
            pad2: PadState::default(),
            port1_data,
            port1_ctrl,
            port2_data,
            port2_ctrl,
            pad1_protocol: PadProtocolState::new(
                ControllerType::ThreeButton,
                effective_th(port1_data, port1_ctrl),
            ),
            pad2_protocol: PadProtocolState::new(
                ControllerType::ThreeButton,
                effective_th(port2_data, port2_ctrl),
            ),
        }
    }
}

impl IoBus {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_version(version: u8) -> Self {
        let mut io = Self::default();
        io.version = version;
        io
    }

    pub fn step(&mut self, cpu_cycles: u32) {
        self.pad1_protocol.step(cpu_cycles);
        self.pad2_protocol.step(cpu_cycles);
    }

    pub fn set_controller_type(&mut self, player: u8, controller_type: ControllerType) {
        match player {
            1 => self.set_port1_controller_type(controller_type),
            2 => self.set_port2_controller_type(controller_type),
            _ => {}
        }
    }

    pub fn set_port1_controller_type(&mut self, controller_type: ControllerType) {
        let th = effective_th(self.port1_data, self.port1_ctrl);
        self.pad1_protocol.set_controller_type(controller_type, th);
    }

    pub fn set_port2_controller_type(&mut self, controller_type: ControllerType) {
        let th = effective_th(self.port2_data, self.port2_ctrl);
        self.pad2_protocol.set_controller_type(controller_type, th);
    }

    pub fn set_button_pressed(&mut self, button: Button, pressed: bool) {
        self.pad1.set_button(button, pressed);
    }

    pub fn set_button2_pressed(&mut self, button: Button, pressed: bool) {
        self.pad2.set_button(button, pressed);
    }

    pub fn read_version(&self) -> u8 {
        self.version
    }

    pub fn read_port1_data(&self) -> u8 {
        read_pad_data(
            &self.pad1,
            self.port1_data,
            self.port1_ctrl,
            self.pad1_protocol,
        )
    }

    pub fn read_port2_data(&self) -> u8 {
        read_pad_data(
            &self.pad2,
            self.port2_data,
            self.port2_ctrl,
            self.pad2_protocol,
        )
    }

    pub fn write_port1_data(&mut self, value: u8) {
        self.port1_data = value & 0x7F;
        let th = effective_th(self.port1_data, self.port1_ctrl);
        self.pad1_protocol.observe_th_level(th);
    }

    pub fn write_port2_data(&mut self, value: u8) {
        self.port2_data = value & 0x7F;
        let th = effective_th(self.port2_data, self.port2_ctrl);
        self.pad2_protocol.observe_th_level(th);
    }

    pub fn read_port1_ctrl(&self) -> u8 {
        self.port1_ctrl
    }

    pub fn read_port2_ctrl(&self) -> u8 {
        self.port2_ctrl
    }

    pub fn write_port1_ctrl(&mut self, value: u8) {
        self.port1_ctrl = value & 0x7F;
        let th = effective_th(self.port1_data, self.port1_ctrl);
        self.pad1_protocol.observe_th_level(th);
    }

    pub fn write_port2_ctrl(&mut self, value: u8) {
        self.port2_ctrl = value & 0x7F;
        let th = effective_th(self.port2_data, self.port2_ctrl);
        self.pad2_protocol.observe_th_level(th);
    }
}

fn read_pad_data(pad: &PadState, port_data: u8, port_ctrl: u8, protocol: PadProtocolState) -> u8 {
    // Start from output latch state. Inputs are then overlaid for bits configured
    // as input in the control register.
    let mut value = port_data & 0x7F;

    let th_high = effective_th(port_data, port_ctrl);
    let mut pad_input = if th_high {
        if matches!(protocol.controller_type, ControllerType::SixButton) && protocol.rise_count == 3
        {
            six_button_extended_high_bits(pad)
        } else {
            three_button_high_bits(pad)
        }
    } else if matches!(protocol.controller_type, ControllerType::SixButton) {
        match protocol.rise_count {
            2 => six_button_detect_low_bits(pad),
            3 => six_button_extended_low_bits(pad),
            _ => three_button_low_bits(pad),
        }
    } else {
        three_button_low_bits(pad)
    };

    // If TH is configured as input, keep it high (pulled up).
    if (port_ctrl & TH_BIT) == 0 {
        pad_input |= TH_BIT;
    }

    // Bits set as input in control register are sourced from the controller.
    let input_mask = !port_ctrl & 0x7F;
    value = (value & !input_mask) | (pad_input & input_mask);
    value
}

fn effective_th(port_data: u8, port_ctrl: u8) -> bool {
    if (port_ctrl & TH_BIT) != 0 {
        (port_data & TH_BIT) != 0
    } else {
        true
    }
}

fn three_button_high_bits(pad: &PadState) -> u8 {
    (active_low_bit(pad.up) << 0)
        | (active_low_bit(pad.down) << 1)
        | (active_low_bit(pad.left) << 2)
        | (active_low_bit(pad.right) << 3)
        | (active_low_bit(pad.b) << 4)
        | (active_low_bit(pad.c) << 5)
        | TH_BIT
}

fn three_button_low_bits(pad: &PadState) -> u8 {
    (active_low_bit(pad.up) << 0)
        | (active_low_bit(pad.down) << 1)
        | (active_low_bit(pad.a) << 4)
        | (active_low_bit(pad.start) << 5)
}

fn six_button_detect_low_bits(pad: &PadState) -> u8 {
    (active_low_bit(pad.a) << 4) | (active_low_bit(pad.start) << 5)
}

fn six_button_extended_low_bits(pad: &PadState) -> u8 {
    (1 << 0)
        | (1 << 1)
        | (1 << 2)
        | (1 << 3)
        | (active_low_bit(pad.a) << 4)
        | (active_low_bit(pad.start) << 5)
}

fn six_button_extended_high_bits(pad: &PadState) -> u8 {
    (active_low_bit(pad.z) << 0)
        | (active_low_bit(pad.y) << 1)
        | (active_low_bit(pad.x) << 2)
        | (active_low_bit(pad.mode) << 3)
        | (active_low_bit(pad.b) << 4)
        | (active_low_bit(pad.c) << 5)
        | TH_BIT
}

fn active_low_bit(pressed: bool) -> u8 {
    if pressed { 0 } else { 1 }
}

#[cfg(test)]
mod tests {
    use super::{Button, ControllerType, IoBus, SIX_BUTTON_TIMEOUT_CPU_CYCLES};

    fn set_th_and_read_port1(io: &mut IoBus, high: bool) -> u8 {
        io.write_port1_data(if high { 0x40 } else { 0x00 });
        io.read_port1_data()
    }

    #[test]
    fn reads_three_button_pad_with_th_high() {
        let mut io = IoBus::new();
        io.set_button_pressed(Button::Right, true);
        io.set_button_pressed(Button::B, true);

        assert_eq!(io.read_port1_data(), 0x67);
    }

    #[test]
    fn reads_start_and_a_with_th_low() {
        let mut io = IoBus::new();
        io.write_port1_data(0x00); // TH low
        io.set_button_pressed(Button::A, true);
        io.set_button_pressed(Button::Start, true);

        assert_eq!(io.read_port1_data(), 0x03);
    }

    #[test]
    fn six_button_cycle_exposes_signature_and_extended_buttons() {
        let mut io = IoBus::new();
        io.set_port1_controller_type(ControllerType::SixButton);

        // Advance into six-button detect phase (6th read: TH low, rising edge count=2).
        set_th_and_read_port1(&mut io, true);
        set_th_and_read_port1(&mut io, false);
        set_th_and_read_port1(&mut io, true);
        set_th_and_read_port1(&mut io, false);
        set_th_and_read_port1(&mut io, true);
        let detect = set_th_and_read_port1(&mut io, false);
        assert_eq!(detect & 0x0F, 0x00);

        io.set_button_pressed(Button::X, true);
        io.set_button_pressed(Button::Mode, true);
        let extended = set_th_and_read_port1(&mut io, true);
        assert_eq!(extended & 0x0F, 0x03);
    }

    #[test]
    fn six_button_cycle_times_out_back_to_three_button_view() {
        let mut io = IoBus::new();
        io.set_port1_controller_type(ControllerType::SixButton);
        io.set_button_pressed(Button::X, true);

        set_th_and_read_port1(&mut io, true);
        set_th_and_read_port1(&mut io, false);
        set_th_and_read_port1(&mut io, true);
        set_th_and_read_port1(&mut io, false);
        set_th_and_read_port1(&mut io, true);
        set_th_and_read_port1(&mut io, false);
        let extended = set_th_and_read_port1(&mut io, true);
        assert_eq!(extended & 0x04, 0x00);

        io.step(SIX_BUTTON_TIMEOUT_CPU_CYCLES + 1);
        let reset_view = io.read_port1_data();
        assert_eq!(reset_view & 0x04, 0x04);
    }

    #[test]
    fn reads_second_pad_independently() {
        let mut io = IoBus::new();
        io.set_button_pressed(Button::Right, true);
        io.set_button2_pressed(Button::Left, true);
        io.set_button2_pressed(Button::C, true);

        assert_eq!(io.read_port1_data(), 0x77);
        assert_eq!(io.read_port2_data(), 0x5B);
    }

    #[test]
    fn second_pad_th_low_exposes_a_and_start() {
        let mut io = IoBus::new();
        io.write_port2_data(0x00);
        io.set_button2_pressed(Button::A, true);
        io.set_button2_pressed(Button::Start, true);

        assert_eq!(io.read_port2_data(), 0x03);
    }

    #[test]
    fn control_register_keeps_output_bits_from_data_latch() {
        let mut io = IoBus::new();
        io.write_port1_ctrl(0x70);
        io.write_port1_data(0x10);
        io.set_button_pressed(Button::B, true);

        // Bit4 is configured as output, so the pressed-B input must not override it.
        assert_eq!(io.read_port1_data() & 0x10, 0x10);
    }

    #[test]
    fn version_register_is_exposed() {
        let io = IoBus::new();
        assert_eq!(io.read_version(), 0x20);
    }

    #[test]
    fn supports_custom_version_register() {
        let io = IoBus::with_version(0xA0);
        assert_eq!(io.read_version(), 0xA0);
    }
}
