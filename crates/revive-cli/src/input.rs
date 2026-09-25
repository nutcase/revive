use revive_core::{CoreInstance, SystemKind, VirtualButton};
use sdl3::keyboard::{KeyboardState, Keycode, Scancode};

const INPUT_BUTTONS: [VirtualButton; 21] = [
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
    VirtualButton::L2,
    VirtualButton::R2,
    VirtualButton::CUp,
    VirtualButton::CDown,
    VirtualButton::CLeft,
    VirtualButton::CRight,
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
    event_pump: &sdl3::EventPump,
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
    core.set_stick(1, 0, 0);
    for button in INPUT_BUTTONS {
        core.set_button(1, button, false);
    }
}

pub(crate) fn keycode_button(system: SystemKind, key: Keycode) -> Option<VirtualButton> {
    bindings_for_system(system)
        .iter()
        .find(|binding| binding.keycodes.contains(&key))
        .map(|binding| binding.button)
}

fn button_pressed(system: SystemKind, keyboard: &KeyboardState<'_>, button: VirtualButton) -> bool {
    bindings_for_system(system)
        .iter()
        .find(|binding| binding.button == button)
        .is_some_and(|binding| {
            binding
                .scancodes
                .iter()
                .any(|scancode| keyboard.is_scancode_pressed(*scancode))
        })
}

struct ButtonBinding {
    button: VirtualButton,
    keycodes: &'static [Keycode],
    scancodes: &'static [Scancode],
}

fn bindings_for_system(system: SystemKind) -> &'static [ButtonBinding] {
    match system {
        SystemKind::Nintendo64 => &N64_BINDINGS,
        SystemKind::PlayStation => &PS1_BINDINGS,
        SystemKind::Nes => &NES_BINDINGS,
        SystemKind::Snes => &SNES_BINDINGS,
        SystemKind::Sg1000 | SystemKind::MasterSystem => &SEGA_8_BIT_BINDINGS,
        SystemKind::MegaDrive => &MEGA_DRIVE_BINDINGS,
        SystemKind::Pce => &PCE_BINDINGS,
        SystemKind::GameBoy | SystemKind::GameBoyColor => &GAME_BOY_BINDINGS,
        SystemKind::GameBoyAdvance => &GAME_BOY_ADVANCE_BINDINGS,
    }
}

static NES_BINDINGS: [ButtonBinding; 8] = [
    binding(VirtualButton::Up, &[Keycode::Up], &[Scancode::Up]),
    binding(VirtualButton::Down, &[Keycode::Down], &[Scancode::Down]),
    binding(VirtualButton::Left, &[Keycode::Left], &[Scancode::Left]),
    binding(VirtualButton::Right, &[Keycode::Right], &[Scancode::Right]),
    binding(
        VirtualButton::A,
        &[Keycode::Z, Keycode::J],
        &[Scancode::Z, Scancode::J],
    ),
    binding(
        VirtualButton::B,
        &[Keycode::X, Keycode::K],
        &[Scancode::X, Scancode::K],
    ),
    binding(
        VirtualButton::Start,
        &[Keycode::Return, Keycode::Space],
        &[Scancode::Return, Scancode::Space],
    ),
    binding(
        VirtualButton::Select,
        &[Keycode::Backspace, Keycode::RShift, Keycode::LShift],
        &[Scancode::Backspace, Scancode::RShift, Scancode::LShift],
    ),
];

static SNES_BINDINGS: [ButtonBinding; 12] = [
    binding(VirtualButton::Up, &[Keycode::Up], &[Scancode::Up]),
    binding(VirtualButton::Down, &[Keycode::Down], &[Scancode::Down]),
    binding(VirtualButton::Left, &[Keycode::Left], &[Scancode::Left]),
    binding(VirtualButton::Right, &[Keycode::Right], &[Scancode::Right]),
    binding(VirtualButton::A, &[Keycode::D], &[Scancode::D]),
    binding(VirtualButton::B, &[Keycode::S], &[Scancode::S]),
    binding(VirtualButton::X, &[Keycode::W], &[Scancode::W]),
    binding(VirtualButton::Y, &[Keycode::A], &[Scancode::A]),
    binding(VirtualButton::L, &[Keycode::E], &[Scancode::E]),
    binding(VirtualButton::R, &[Keycode::Q], &[Scancode::Q]),
    binding(
        VirtualButton::Start,
        &[Keycode::Return, Keycode::Space],
        &[Scancode::Return, Scancode::Space],
    ),
    binding(
        VirtualButton::Select,
        &[Keycode::Backspace, Keycode::RShift, Keycode::LShift],
        &[Scancode::Backspace, Scancode::RShift, Scancode::LShift],
    ),
];

static SEGA_8_BIT_BINDINGS: [ButtonBinding; 6] = [
    binding(VirtualButton::Up, &[Keycode::Up], &[Scancode::Up]),
    binding(VirtualButton::Down, &[Keycode::Down], &[Scancode::Down]),
    binding(VirtualButton::Left, &[Keycode::Left], &[Scancode::Left]),
    binding(VirtualButton::Right, &[Keycode::Right], &[Scancode::Right]),
    binding(
        VirtualButton::A,
        &[Keycode::Z, Keycode::J],
        &[Scancode::Z, Scancode::J],
    ),
    binding(
        VirtualButton::B,
        &[Keycode::X, Keycode::K],
        &[Scancode::X, Scancode::K],
    ),
];

static MEGA_DRIVE_BINDINGS: [ButtonBinding; 12] = [
    binding(VirtualButton::Up, &[Keycode::Up], &[Scancode::Up]),
    binding(VirtualButton::Down, &[Keycode::Down], &[Scancode::Down]),
    binding(VirtualButton::Left, &[Keycode::Left], &[Scancode::Left]),
    binding(VirtualButton::Right, &[Keycode::Right], &[Scancode::Right]),
    binding(VirtualButton::A, &[Keycode::A], &[Scancode::A]),
    binding(VirtualButton::B, &[Keycode::Z], &[Scancode::Z]),
    binding(VirtualButton::C, &[Keycode::X], &[Scancode::X]),
    binding(VirtualButton::X, &[Keycode::S], &[Scancode::S]),
    binding(VirtualButton::Y, &[Keycode::D], &[Scancode::D]),
    binding(VirtualButton::Z, &[Keycode::F], &[Scancode::F]),
    binding(VirtualButton::Mode, &[Keycode::Q], &[Scancode::Q]),
    binding(
        VirtualButton::Start,
        &[Keycode::Return, Keycode::Space],
        &[Scancode::Return, Scancode::Space],
    ),
];

static PCE_BINDINGS: [ButtonBinding; 8] = [
    binding(VirtualButton::Up, &[Keycode::Up], &[Scancode::Up]),
    binding(VirtualButton::Down, &[Keycode::Down], &[Scancode::Down]),
    binding(VirtualButton::Left, &[Keycode::Left], &[Scancode::Left]),
    binding(VirtualButton::Right, &[Keycode::Right], &[Scancode::Right]),
    binding(
        VirtualButton::A,
        &[Keycode::Z, Keycode::J],
        &[Scancode::Z, Scancode::J],
    ),
    binding(
        VirtualButton::B,
        &[Keycode::X, Keycode::K],
        &[Scancode::X, Scancode::K],
    ),
    binding(
        VirtualButton::Start,
        &[Keycode::Return, Keycode::Space],
        &[Scancode::Return, Scancode::Space],
    ),
    binding(
        VirtualButton::Select,
        &[Keycode::Backspace, Keycode::RShift, Keycode::LShift],
        &[Scancode::Backspace, Scancode::RShift, Scancode::LShift],
    ),
];

static GAME_BOY_BINDINGS: [ButtonBinding; 8] = [
    binding(VirtualButton::Up, &[Keycode::Up], &[Scancode::Up]),
    binding(VirtualButton::Down, &[Keycode::Down], &[Scancode::Down]),
    binding(VirtualButton::Left, &[Keycode::Left], &[Scancode::Left]),
    binding(VirtualButton::Right, &[Keycode::Right], &[Scancode::Right]),
    binding(
        VirtualButton::A,
        &[Keycode::X, Keycode::J],
        &[Scancode::X, Scancode::J],
    ),
    binding(
        VirtualButton::B,
        &[Keycode::Z, Keycode::K],
        &[Scancode::Z, Scancode::K],
    ),
    binding(
        VirtualButton::Start,
        &[Keycode::Return, Keycode::Space],
        &[Scancode::Return, Scancode::Space],
    ),
    binding(
        VirtualButton::Select,
        &[Keycode::Backspace, Keycode::RShift, Keycode::LShift],
        &[Scancode::Backspace, Scancode::RShift, Scancode::LShift],
    ),
];

static GAME_BOY_ADVANCE_BINDINGS: [ButtonBinding; 10] = [
    binding(VirtualButton::Up, &[Keycode::Up], &[Scancode::Up]),
    binding(VirtualButton::Down, &[Keycode::Down], &[Scancode::Down]),
    binding(VirtualButton::Left, &[Keycode::Left], &[Scancode::Left]),
    binding(VirtualButton::Right, &[Keycode::Right], &[Scancode::Right]),
    binding(
        VirtualButton::A,
        &[Keycode::X, Keycode::J],
        &[Scancode::X, Scancode::J],
    ),
    binding(
        VirtualButton::B,
        &[Keycode::Z, Keycode::K],
        &[Scancode::Z, Scancode::K],
    ),
    binding(VirtualButton::L, &[Keycode::A], &[Scancode::A]),
    binding(VirtualButton::R, &[Keycode::S], &[Scancode::S]),
    binding(
        VirtualButton::Start,
        &[Keycode::Return, Keycode::Space],
        &[Scancode::Return, Scancode::Space],
    ),
    binding(
        VirtualButton::Select,
        &[Keycode::Backspace, Keycode::RShift, Keycode::LShift],
        &[Scancode::Backspace, Scancode::RShift, Scancode::LShift],
    ),
];

const fn binding(
    button: VirtualButton,
    keycodes: &'static [Keycode],
    scancodes: &'static [Scancode],
) -> ButtonBinding {
    ButtonBinding {
        button,
        keycodes,
        scancodes,
    }
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
        VirtualButton::L2 => 15,
        VirtualButton::R2 => 16,
        VirtualButton::CUp => 17,
        VirtualButton::CDown => 18,
        VirtualButton::CLeft => 19,
        VirtualButton::CRight => 20,
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
        VirtualButton::L2 => "L2",
        VirtualButton::R2 => "R2",
        VirtualButton::CUp => "C Up",
        VirtualButton::CDown => "C Down",
        VirtualButton::CLeft => "C Left",
        VirtualButton::CRight => "C Right",
    }
}

// Face buttons follow physical positions: Z=Cross, X=Circle, A=Square, S=Triangle.
static PS1_BINDINGS: [ButtonBinding; 14] = [
    binding(VirtualButton::Up, &[Keycode::Up], &[Scancode::Up]),
    binding(VirtualButton::Down, &[Keycode::Down], &[Scancode::Down]),
    binding(VirtualButton::Left, &[Keycode::Left], &[Scancode::Left]),
    binding(VirtualButton::Right, &[Keycode::Right], &[Scancode::Right]),
    binding(VirtualButton::B, &[Keycode::Z], &[Scancode::Z]),
    binding(VirtualButton::A, &[Keycode::X], &[Scancode::X]),
    binding(VirtualButton::Y, &[Keycode::A], &[Scancode::A]),
    binding(VirtualButton::X, &[Keycode::S], &[Scancode::S]),
    binding(VirtualButton::L, &[Keycode::Q], &[Scancode::Q]),
    binding(VirtualButton::R, &[Keycode::W], &[Scancode::W]),
    binding(VirtualButton::L2, &[Keycode::E], &[Scancode::E]),
    binding(VirtualButton::R2, &[Keycode::R], &[Scancode::R]),
    binding(
        VirtualButton::Start,
        &[Keycode::Return, Keycode::Space],
        &[Scancode::Return, Scancode::Space],
    ),
    binding(
        VirtualButton::Select,
        &[Keycode::Backspace, Keycode::RShift, Keycode::LShift],
        &[Scancode::Backspace, Scancode::RShift, Scancode::LShift],
    ),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keycode_bindings_cover_shared_system_controls() {
        assert_eq!(
            keycode_button(SystemKind::MegaDrive, Keycode::Return),
            Some(VirtualButton::Start)
        );
        assert_eq!(
            keycode_button(SystemKind::MasterSystem, Keycode::Z),
            Some(VirtualButton::A)
        );
        assert_eq!(
            keycode_button(SystemKind::GameBoyAdvance, Keycode::A),
            Some(VirtualButton::L)
        );
    }

    #[test]
    fn playstation_keys_include_face_buttons_and_both_shoulders() {
        for (key, button) in [
            (Keycode::Z, VirtualButton::B),
            (Keycode::X, VirtualButton::A),
            (Keycode::A, VirtualButton::Y),
            (Keycode::S, VirtualButton::X),
            (Keycode::Q, VirtualButton::L),
            (Keycode::W, VirtualButton::R),
            (Keycode::E, VirtualButton::L2),
            (Keycode::R, VirtualButton::R2),
        ] {
            assert_eq!(keycode_button(SystemKind::PlayStation, key), Some(button));
        }
        let mut state = InputState::default();
        state.set(VirtualButton::L2, true);
        state.set(VirtualButton::R2, true);
        state.clear();
        assert!(!state.is_pressed(VirtualButton::L2) && !state.is_pressed(VirtualButton::R2));
    }

    #[test]
    fn every_keycode_binding_has_scancode_coverage() {
        for system in [
            SystemKind::PlayStation,
            SystemKind::Nes,
            SystemKind::Snes,
            SystemKind::Sg1000,
            SystemKind::MasterSystem,
            SystemKind::MegaDrive,
            SystemKind::Pce,
            SystemKind::GameBoy,
            SystemKind::GameBoyColor,
            SystemKind::GameBoyAdvance,
        ] {
            for binding in bindings_for_system(system) {
                assert!(
                    !binding.keycodes.is_empty() && !binding.scancodes.is_empty(),
                    "{system:?} {:?} should map both event and keyboard-state input",
                    binding.button
                );
            }
        }
    }
}

static N64_BINDINGS: [ButtonBinding; 14] = [
    binding(VirtualButton::Up, &[Keycode::W], &[Scancode::W]),
    binding(VirtualButton::Down, &[Keycode::S], &[Scancode::S]),
    binding(VirtualButton::Left, &[Keycode::A], &[Scancode::A]),
    binding(VirtualButton::Right, &[Keycode::D], &[Scancode::D]),
    binding(VirtualButton::A, &[Keycode::Z], &[Scancode::Z]),
    binding(VirtualButton::B, &[Keycode::X], &[Scancode::X]),
    binding(
        VirtualButton::Z,
        &[Keycode::LShift, Keycode::RShift],
        &[Scancode::LShift, Scancode::RShift],
    ),
    binding(VirtualButton::L, &[Keycode::Q], &[Scancode::Q]),
    binding(VirtualButton::R, &[Keycode::E], &[Scancode::E]),
    binding(
        VirtualButton::Start,
        &[Keycode::Return],
        &[Scancode::Return],
    ),
    binding(VirtualButton::CUp, &[Keycode::I], &[Scancode::I]),
    binding(VirtualButton::CDown, &[Keycode::K], &[Scancode::K]),
    binding(VirtualButton::CLeft, &[Keycode::J], &[Scancode::J]),
    binding(VirtualButton::CRight, &[Keycode::L], &[Scancode::L]),
];

pub(crate) struct N64Input {
    subsystem: sdl3::GamepadSubsystem,
    pad: Option<sdl3::gamepad::Gamepad>,
    next_scan: std::time::Instant,
}
impl N64Input {
    pub(crate) fn new(sdl: &sdl3::Sdl) -> Result<Self, sdl3::Error> {
        Ok(Self {
            subsystem: sdl.gamepad()?,
            pad: None,
            next_scan: std::time::Instant::now(),
        })
    }
    pub(crate) fn sync(
        &mut self,
        core: &mut CoreInstance,
        pump: &sdl3::EventPump,
        input: &InputState,
    ) {
        use sdl3::gamepad::{Axis, Button};
        if self.pad.as_ref().is_some_and(|p| !p.connected()) {
            self.pad = None;
        }
        if self.pad.is_none() && std::time::Instant::now() >= self.next_scan {
            self.next_scan = std::time::Instant::now() + std::time::Duration::from_secs(1);
            if let Ok(ids) = self.subsystem.gamepads() {
                self.pad = ids.into_iter().find_map(|id| self.subsystem.open(id).ok());
            }
        }
        let keyboard = pump.keyboard_state();
        let mut stick = keyboard_stick(
            keyboard.is_scancode_pressed(Scancode::Left),
            keyboard.is_scancode_pressed(Scancode::Right),
            keyboard.is_scancode_pressed(Scancode::Up),
            keyboard.is_scancode_pressed(Scancode::Down),
        );
        if let Some(pad) = &self.pad {
            let x = pad.axis(Axis::LeftX);
            let y = pad.axis(Axis::LeftY);
            let analog = deadzone(x, y);
            if analog != (0, 0) {
                stick = analog;
            }
        }
        core.set_stick(1, stick.0, stick.1);
        for button in INPUT_BUTTONS {
            let gamepad = self.pad.as_ref().is_some_and(|pad| match button {
                VirtualButton::A => pad.button(Button::South),
                VirtualButton::B => pad.button(Button::West),
                VirtualButton::Start => pad.button(Button::Start),
                VirtualButton::L => pad.button(Button::LeftShoulder),
                VirtualButton::R => pad.button(Button::RightShoulder),
                VirtualButton::Z => pad.axis(Axis::TriggerLeft) > 8192,
                VirtualButton::Up => pad.button(Button::DPadUp),
                VirtualButton::Down => pad.button(Button::DPadDown),
                VirtualButton::Left => pad.button(Button::DPadLeft),
                VirtualButton::Right => pad.button(Button::DPadRight),
                VirtualButton::CUp => pad.axis(Axis::RightY) < -16384,
                VirtualButton::CDown => pad.axis(Axis::RightY) > 16384,
                VirtualButton::CLeft => pad.axis(Axis::RightX) < -16384,
                VirtualButton::CRight => pad.axis(Axis::RightX) > 16384,
                _ => false,
            });
            core.set_button(
                1,
                button,
                gamepad
                    || input.is_pressed(button)
                    || button_pressed(SystemKind::Nintendo64, &keyboard, button),
            );
        }
    }
}
fn keyboard_stick(left: bool, right: bool, up: bool, down: bool) -> (i16, i16) {
    let x = i16::from(right) - i16::from(left);
    let y = i16::from(down) - i16::from(up);
    let scale = if x != 0 && y != 0 { 23170 } else { 32767 };
    (x * scale, y * scale)
}
fn deadzone(x: i16, y: i16) -> (i16, i16) {
    let radius = (f64::from(x).powi(2) + f64::from(y).powi(2)).sqrt();
    if radius <= 6000.0 {
        return (0, 0);
    }
    let magnitude = ((radius - 6000.0) / (32767.0 - 6000.0)).min(1.0) * 32767.0;
    (
        (f64::from(x) / radius * magnitude) as i16,
        (f64::from(y) / radius * magnitude) as i16,
    )
}

#[cfg(test)]
mod n64_tests {
    use super::*;
    #[test]
    fn keyboard_stick_cancels_opposites_and_limits_diagonals() {
        assert_eq!(keyboard_stick(true, true, true, true), (0, 0));
        assert_eq!(keyboard_stick(false, false, true, false), (0, -32767));
        assert_eq!(keyboard_stick(false, true, true, false), (23170, -23170));
    }
    #[test]
    fn analog_deadzone_preserves_direction_and_saturates() {
        assert_eq!(deadzone(3000, -3000), (0, 0));
        assert_eq!(deadzone(0, -32768), (0, -32767));
        let (x, y) = deadzone(32767, 32767);
        assert!((x as f64).hypot(y as f64) <= 32767.0);
    }
    #[test]
    fn c_buttons_are_distinct_from_dpad_and_face_buttons() {
        for (key, button) in [
            (Keycode::I, VirtualButton::CUp),
            (Keycode::K, VirtualButton::CDown),
            (Keycode::J, VirtualButton::CLeft),
            (Keycode::L, VirtualButton::CRight),
            (Keycode::Z, VirtualButton::A),
            (Keycode::X, VirtualButton::B),
        ] {
            assert_eq!(keycode_button(SystemKind::Nintendo64, key), Some(button));
        }
        assert_eq!(keycode_button(SystemKind::Nintendo64, Keycode::Up), None);
    }
}
