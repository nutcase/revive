use revive_core::{CoreInstance, SystemKind};
use sdl3::keyboard::{Keycode, Mod, Scancode};

use crate::hud::HudToast;

pub(crate) fn handle_state_key(
    core: &mut CoreInstance,
    key: Keycode,
    scancode: Option<Scancode>,
    keymod: Mod,
    hud_toast: &mut HudToast,
) -> bool {
    let Some((slot, save)) = state_key_binding(key, scancode, keymod) else {
        return false;
    };
    if !core.system().supports_save_state() {
        let system = core.system().label();
        eprintln!("save states are not available for {system}");
        hud_toast.show(format!("{system} states unavailable"));
        return true;
    }
    if save {
        match core.save_state_to_slot(slot) {
            Ok(()) => {
                println!("Saved state slot {slot}");
                hud_toast.show(format!("Saved slot {slot}"));
            }
            Err(err) => {
                eprintln!("failed to save state slot {slot}: {err}");
                hud_toast.show(format!("Save slot {slot} failed"));
            }
        }
    } else {
        match core.load_state_from_slot(slot) {
            Ok(()) => {
                println!("Loaded state slot {slot}");
                hud_toast.show(format!("Loaded slot {slot}"));
            }
            Err(err) if err.starts_with("no saved state file found") => {
                eprintln!("state slot {slot} is empty: {err}");
                hud_toast.show(format!("Slot {slot} empty"));
            }
            Err(err) => {
                eprintln!("failed to load state slot {slot}: {err}");
                hud_toast.show(format!("Load slot {slot} failed"));
            }
        }
    }
    true
}

fn state_key_binding(key: Keycode, scancode: Option<Scancode>, keymod: Mod) -> Option<(u8, bool)> {
    if !state_command_modifier(keymod) {
        return None;
    }

    let slot = match scancode {
        Some(Scancode::_1 | Scancode::Kp1) => 1,
        Some(Scancode::_2 | Scancode::Kp2) => 2,
        Some(Scancode::_3 | Scancode::Kp3) => 3,
        Some(Scancode::_4 | Scancode::Kp4) => 4,
        Some(Scancode::_5 | Scancode::Kp5) => 5,
        Some(Scancode::_6 | Scancode::Kp6) => 6,
        Some(Scancode::_7 | Scancode::Kp7) => 7,
        Some(Scancode::_8 | Scancode::Kp8) => 8,
        Some(Scancode::_9 | Scancode::Kp9) => 9,
        _ => match key {
            Keycode::_1 | Keycode::Kp1 => 1,
            Keycode::_2 | Keycode::Kp2 => 2,
            Keycode::_3 | Keycode::Kp3 => 3,
            Keycode::_4 | Keycode::Kp4 => 4,
            Keycode::_5 | Keycode::Kp5 => 5,
            Keycode::_6 | Keycode::Kp6 => 6,
            Keycode::_7 | Keycode::Kp7 => 7,
            Keycode::_8 | Keycode::Kp8 => 8,
            Keycode::_9 | Keycode::Kp9 => 9,
            _ => return None,
        },
    };
    Some((slot, state_save_modifier(keymod)))
}

fn state_command_modifier(keymod: Mod) -> bool {
    state_primary_modifier(keymod)
}

#[cfg(target_os = "macos")]
fn state_primary_modifier(keymod: Mod) -> bool {
    keymod.intersects(Mod::LGUIMOD | Mod::RGUIMOD)
}

#[cfg(not(target_os = "macos"))]
fn state_primary_modifier(keymod: Mod) -> bool {
    keymod.intersects(Mod::LCTRLMOD | Mod::RCTRLMOD)
}

fn state_save_modifier(keymod: Mod) -> bool {
    keymod.intersects(Mod::LSHIFTMOD | Mod::RSHIFTMOD)
}

#[cfg(target_os = "macos")]
pub(crate) fn state_key_help(system: SystemKind) -> &'static str {
    if system.supports_save_state() {
        "Cmd+1..9 load, Cmd+Shift+1..9 save"
    } else {
        "not available for this system"
    }
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn state_key_help(system: SystemKind) -> &'static str {
    if system.supports_save_state() {
        "Ctrl+1..9 load, Ctrl+Shift+1..9 save"
    } else {
        "not available for this system"
    }
}
