use revive_core::{CoreInstance, SystemKind, VirtualButton};
use sdl2::keyboard::{KeyboardState, Keycode, Scancode};

const INPUT_BUTTONS: [VirtualButton; 15] = [
    VirtualButton::Up,
    VirtualButton::Down,
    VirtualButton::Left,
    VirtualButton::Right,
    VirtualButton::A,
    VirtualButton::B,
    VirtualButton::X,
    VirtualButton::Y,
    VirtualButton::L,
    VirtualButton::R,
    VirtualButton::Start,
    VirtualButton::Select,
    VirtualButton::C,
    VirtualButton::Z,
    VirtualButton::Mode,
];

#[derive(Debug, Default)]
pub(crate) struct InputState {
    pressed: [bool; INPUT_BUTTONS.len()],
}

impl InputState {
    pub(crate) fn set(&mut self, button: VirtualButton, pressed: bool) {
        self.pressed[button_index(button)] = pressed;
    }

    fn is_pressed(&self, button: VirtualButton) -> bool {
        self.pressed[button_index(button)]
    }

    pub(crate) fn clear(&mut self) {
        self.pressed.fill(false);
    }
}
pub(crate) fn sync_keyboard_input(
    core: &mut CoreInstance,
    event_pump: &sdl2::EventPump,
    event_input: &InputState,
) {
    let system = core.system();
    let keyboard = event_pump.keyboard_state();
    for button in INPUT_BUTTONS {
        core.set_button(
            1,
            button,
            event_input.is_pressed(button) || button_pressed(system, &keyboard, button),
        );
    }
}

pub(crate) fn release_keyboard_input(core: &mut CoreInstance) {
    for button in INPUT_BUTTONS {
        core.set_button(1, button, false);
    }
}

pub(crate) fn keycode_button(system: SystemKind, key: Keycode) -> Option<VirtualButton> {
    match system {
        SystemKind::Nes => nes_keycode_button(key),
        SystemKind::Snes => snes_keycode_button(key),
        SystemKind::Sg1000 => sg1000_keycode_button(key),
        SystemKind::MasterSystem => mastersystem_keycode_button(key),
        SystemKind::MegaDrive => megadrive_keycode_button(key),
        SystemKind::Pce => pce_keycode_button(key),
        SystemKind::GameBoy | SystemKind::GameBoyColor => gameboy_keycode_button(key),
        SystemKind::GameBoyAdvance => gameboy_advance_keycode_button(key),
    }
}

fn nes_keycode_button(key: Keycode) -> Option<VirtualButton> {
    match key {
        Keycode::Up => Some(VirtualButton::Up),
        Keycode::Down => Some(VirtualButton::Down),
        Keycode::Left => Some(VirtualButton::Left),
        Keycode::Right => Some(VirtualButton::Right),
        Keycode::Z | Keycode::J => Some(VirtualButton::A),
        Keycode::X | Keycode::K => Some(VirtualButton::B),
        Keycode::Return | Keycode::Space => Some(VirtualButton::Start),
        Keycode::Backspace | Keycode::RShift | Keycode::LShift => Some(VirtualButton::Select),
        _ => None,
    }
}

fn snes_keycode_button(key: Keycode) -> Option<VirtualButton> {
    match key {
        Keycode::Up => Some(VirtualButton::Up),
        Keycode::Down => Some(VirtualButton::Down),
        Keycode::Left => Some(VirtualButton::Left),
        Keycode::Right => Some(VirtualButton::Right),
        Keycode::D => Some(VirtualButton::A),
        Keycode::S => Some(VirtualButton::B),
        Keycode::W => Some(VirtualButton::X),
        Keycode::A => Some(VirtualButton::Y),
        Keycode::E => Some(VirtualButton::L),
        Keycode::Q => Some(VirtualButton::R),
        Keycode::Return | Keycode::Space => Some(VirtualButton::Start),
        Keycode::Backspace | Keycode::RShift | Keycode::LShift => Some(VirtualButton::Select),
        _ => None,
    }
}

fn megadrive_keycode_button(key: Keycode) -> Option<VirtualButton> {
    match key {
        Keycode::Up => Some(VirtualButton::Up),
        Keycode::Down => Some(VirtualButton::Down),
        Keycode::Left => Some(VirtualButton::Left),
        Keycode::Right => Some(VirtualButton::Right),
        Keycode::A => Some(VirtualButton::A),
        Keycode::Z => Some(VirtualButton::B),
        Keycode::X => Some(VirtualButton::C),
        Keycode::S => Some(VirtualButton::X),
        Keycode::D => Some(VirtualButton::Y),
        Keycode::F => Some(VirtualButton::Z),
        Keycode::Q => Some(VirtualButton::Mode),
        Keycode::Return | Keycode::Space => Some(VirtualButton::Start),
        _ => None,
    }
}

fn sg1000_keycode_button(key: Keycode) -> Option<VirtualButton> {
    match key {
        Keycode::Up => Some(VirtualButton::Up),
        Keycode::Down => Some(VirtualButton::Down),
        Keycode::Left => Some(VirtualButton::Left),
        Keycode::Right => Some(VirtualButton::Right),
        Keycode::Z | Keycode::J => Some(VirtualButton::A),
        Keycode::X | Keycode::K => Some(VirtualButton::B),
        _ => None,
    }
}

fn mastersystem_keycode_button(key: Keycode) -> Option<VirtualButton> {
    sg1000_keycode_button(key)
}

fn pce_keycode_button(key: Keycode) -> Option<VirtualButton> {
    match key {
        Keycode::Up => Some(VirtualButton::Up),
        Keycode::Down => Some(VirtualButton::Down),
        Keycode::Left => Some(VirtualButton::Left),
        Keycode::Right => Some(VirtualButton::Right),
        Keycode::Z | Keycode::J => Some(VirtualButton::A),
        Keycode::X | Keycode::K => Some(VirtualButton::B),
        Keycode::Return | Keycode::Space => Some(VirtualButton::Start),
        Keycode::Backspace | Keycode::RShift | Keycode::LShift => Some(VirtualButton::Select),
        _ => None,
    }
}

fn gameboy_keycode_button(key: Keycode) -> Option<VirtualButton> {
    match key {
        Keycode::Up => Some(VirtualButton::Up),
        Keycode::Down => Some(VirtualButton::Down),
        Keycode::Left => Some(VirtualButton::Left),
        Keycode::Right => Some(VirtualButton::Right),
        Keycode::X | Keycode::J => Some(VirtualButton::A),
        Keycode::Z | Keycode::K => Some(VirtualButton::B),
        Keycode::Return | Keycode::Space => Some(VirtualButton::Start),
        Keycode::Backspace | Keycode::RShift | Keycode::LShift => Some(VirtualButton::Select),
        _ => None,
    }
}

fn gameboy_advance_keycode_button(key: Keycode) -> Option<VirtualButton> {
    match key {
        Keycode::A => Some(VirtualButton::L),
        Keycode::S => Some(VirtualButton::R),
        _ => gameboy_keycode_button(key),
    }
}

fn button_pressed(system: SystemKind, keyboard: &KeyboardState<'_>, button: VirtualButton) -> bool {
    match system {
        SystemKind::Nes => nes_button_pressed(keyboard, button),
        SystemKind::Snes => snes_button_pressed(keyboard, button),
        SystemKind::Sg1000 => sg1000_button_pressed(keyboard, button),
        SystemKind::MasterSystem => mastersystem_button_pressed(keyboard, button),
        SystemKind::MegaDrive => megadrive_button_pressed(keyboard, button),
        SystemKind::Pce => pce_button_pressed(keyboard, button),
        SystemKind::GameBoy | SystemKind::GameBoyColor => gameboy_button_pressed(keyboard, button),
        SystemKind::GameBoyAdvance => gameboy_advance_button_pressed(keyboard, button),
    }
}

fn nes_button_pressed(keyboard: &KeyboardState<'_>, button: VirtualButton) -> bool {
    match button {
        VirtualButton::Up => scancode_down(keyboard, &[Scancode::Up]),
        VirtualButton::Down => scancode_down(keyboard, &[Scancode::Down]),
        VirtualButton::Left => scancode_down(keyboard, &[Scancode::Left]),
        VirtualButton::Right => scancode_down(keyboard, &[Scancode::Right]),
        VirtualButton::A => scancode_down(keyboard, &[Scancode::Z, Scancode::J]),
        VirtualButton::B => scancode_down(keyboard, &[Scancode::X, Scancode::K]),
        VirtualButton::Start => scancode_down(keyboard, &[Scancode::Return, Scancode::Space]),
        VirtualButton::Select => scancode_down(
            keyboard,
            &[Scancode::Backspace, Scancode::LShift, Scancode::RShift],
        ),
        _ => false,
    }
}

fn snes_button_pressed(keyboard: &KeyboardState<'_>, button: VirtualButton) -> bool {
    match button {
        VirtualButton::Up => scancode_down(keyboard, &[Scancode::Up]),
        VirtualButton::Down => scancode_down(keyboard, &[Scancode::Down]),
        VirtualButton::Left => scancode_down(keyboard, &[Scancode::Left]),
        VirtualButton::Right => scancode_down(keyboard, &[Scancode::Right]),
        VirtualButton::A => scancode_down(keyboard, &[Scancode::D]),
        VirtualButton::B => scancode_down(keyboard, &[Scancode::S]),
        VirtualButton::X => scancode_down(keyboard, &[Scancode::W]),
        VirtualButton::Y => scancode_down(keyboard, &[Scancode::A]),
        VirtualButton::L => scancode_down(keyboard, &[Scancode::E]),
        VirtualButton::R => scancode_down(keyboard, &[Scancode::Q]),
        VirtualButton::Start => scancode_down(keyboard, &[Scancode::Return, Scancode::Space]),
        VirtualButton::Select => scancode_down(
            keyboard,
            &[Scancode::Backspace, Scancode::LShift, Scancode::RShift],
        ),
        _ => false,
    }
}

fn megadrive_button_pressed(keyboard: &KeyboardState<'_>, button: VirtualButton) -> bool {
    match button {
        VirtualButton::Up => scancode_down(keyboard, &[Scancode::Up]),
        VirtualButton::Down => scancode_down(keyboard, &[Scancode::Down]),
        VirtualButton::Left => scancode_down(keyboard, &[Scancode::Left]),
        VirtualButton::Right => scancode_down(keyboard, &[Scancode::Right]),
        VirtualButton::A => scancode_down(keyboard, &[Scancode::A]),
        VirtualButton::B => scancode_down(keyboard, &[Scancode::Z]),
        VirtualButton::C => scancode_down(keyboard, &[Scancode::X]),
        VirtualButton::X => scancode_down(keyboard, &[Scancode::S]),
        VirtualButton::Y => scancode_down(keyboard, &[Scancode::D]),
        VirtualButton::Z => scancode_down(keyboard, &[Scancode::F]),
        VirtualButton::Mode => scancode_down(keyboard, &[Scancode::Q]),
        VirtualButton::Start => scancode_down(keyboard, &[Scancode::Return, Scancode::Space]),
        _ => false,
    }
}

fn sg1000_button_pressed(keyboard: &KeyboardState<'_>, button: VirtualButton) -> bool {
    match button {
        VirtualButton::Up => scancode_down(keyboard, &[Scancode::Up]),
        VirtualButton::Down => scancode_down(keyboard, &[Scancode::Down]),
        VirtualButton::Left => scancode_down(keyboard, &[Scancode::Left]),
        VirtualButton::Right => scancode_down(keyboard, &[Scancode::Right]),
        VirtualButton::A => scancode_down(keyboard, &[Scancode::Z, Scancode::J]),
        VirtualButton::B => scancode_down(keyboard, &[Scancode::X, Scancode::K]),
        _ => false,
    }
}

fn mastersystem_button_pressed(keyboard: &KeyboardState<'_>, button: VirtualButton) -> bool {
    sg1000_button_pressed(keyboard, button)
}

fn pce_button_pressed(keyboard: &KeyboardState<'_>, button: VirtualButton) -> bool {
    match button {
        VirtualButton::Up => scancode_down(keyboard, &[Scancode::Up]),
        VirtualButton::Down => scancode_down(keyboard, &[Scancode::Down]),
        VirtualButton::Left => scancode_down(keyboard, &[Scancode::Left]),
        VirtualButton::Right => scancode_down(keyboard, &[Scancode::Right]),
        VirtualButton::A => scancode_down(keyboard, &[Scancode::Z, Scancode::J]),
        VirtualButton::B => scancode_down(keyboard, &[Scancode::X, Scancode::K]),
        VirtualButton::Start => scancode_down(keyboard, &[Scancode::Return, Scancode::Space]),
        VirtualButton::Select => scancode_down(
            keyboard,
            &[Scancode::Backspace, Scancode::LShift, Scancode::RShift],
        ),
        _ => false,
    }
}

fn gameboy_button_pressed(keyboard: &KeyboardState<'_>, button: VirtualButton) -> bool {
    match button {
        VirtualButton::Up => scancode_down(keyboard, &[Scancode::Up]),
        VirtualButton::Down => scancode_down(keyboard, &[Scancode::Down]),
        VirtualButton::Left => scancode_down(keyboard, &[Scancode::Left]),
        VirtualButton::Right => scancode_down(keyboard, &[Scancode::Right]),
        VirtualButton::A => scancode_down(keyboard, &[Scancode::X, Scancode::J]),
        VirtualButton::B => scancode_down(keyboard, &[Scancode::Z, Scancode::K]),
        VirtualButton::Start => scancode_down(keyboard, &[Scancode::Return, Scancode::Space]),
        VirtualButton::Select => scancode_down(
            keyboard,
            &[Scancode::Backspace, Scancode::LShift, Scancode::RShift],
        ),
        _ => false,
    }
}

fn gameboy_advance_button_pressed(keyboard: &KeyboardState<'_>, button: VirtualButton) -> bool {
    match button {
        VirtualButton::L => scancode_down(keyboard, &[Scancode::A]),
        VirtualButton::R => scancode_down(keyboard, &[Scancode::S]),
        _ => gameboy_button_pressed(keyboard, button),
    }
}

fn scancode_down(keyboard: &KeyboardState<'_>, scancodes: &[Scancode]) -> bool {
    scancodes
        .iter()
        .any(|scancode| keyboard.is_scancode_pressed(*scancode))
}

fn button_index(button: VirtualButton) -> usize {
    match button {
        VirtualButton::Up => 0,
        VirtualButton::Down => 1,
        VirtualButton::Left => 2,
        VirtualButton::Right => 3,
        VirtualButton::A => 4,
        VirtualButton::B => 5,
        VirtualButton::X => 6,
        VirtualButton::Y => 7,
        VirtualButton::L => 8,
        VirtualButton::R => 9,
        VirtualButton::Start => 10,
        VirtualButton::Select => 11,
        VirtualButton::C => 12,
        VirtualButton::Z => 13,
        VirtualButton::Mode => 14,
    }
}

pub(crate) fn button_label(button: VirtualButton) -> &'static str {
    match button {
        VirtualButton::Up => "Up",
        VirtualButton::Down => "Down",
        VirtualButton::Left => "Left",
        VirtualButton::Right => "Right",
        VirtualButton::A => "A",
        VirtualButton::B => "B",
        VirtualButton::X => "X",
        VirtualButton::Y => "Y",
        VirtualButton::L => "L",
        VirtualButton::R => "R",
        VirtualButton::Start => "Start",
        VirtualButton::Select => "Select",
        VirtualButton::C => "C",
        VirtualButton::Z => "Z",
        VirtualButton::Mode => "Mode",
    }
}
