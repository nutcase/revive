use revive_core::{CoreInstance, SystemKind, VirtualButton};
use sdl3::keyboard::{KeyboardState, Keycode, Scancode};

const INPUT_BUTTONS: [VirtualButton; 17] = [
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
    }
}

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
