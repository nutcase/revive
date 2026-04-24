use revive_core::CoreInstance;
use sdl2::keyboard::{Keycode, Mod, Scancode};

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
        Some(Scancode::Num1 | Scancode::Kp1) => 1,
        Some(Scancode::Num2 | Scancode::Kp2) => 2,
        Some(Scancode::Num3 | Scancode::Kp3) => 3,
        Some(Scancode::Num4 | Scancode::Kp4) => 4,
        Some(Scancode::Num5 | Scancode::Kp5) => 5,
        Some(Scancode::Num6 | Scancode::Kp6) => 6,
        Some(Scancode::Num7 | Scancode::Kp7) => 7,
        Some(Scancode::Num8 | Scancode::Kp8) => 8,
        Some(Scancode::Num9 | Scancode::Kp9) => 9,
        _ => match key {
            Keycode::Num1 | Keycode::Kp1 => 1,
            Keycode::Num2 | Keycode::Kp2 => 2,
            Keycode::Num3 | Keycode::Kp3 => 3,
            Keycode::Num4 | Keycode::Kp4 => 4,
            Keycode::Num5 | Keycode::Kp5 => 5,
            Keycode::Num6 | Keycode::Kp6 => 6,
            Keycode::Num7 | Keycode::Kp7 => 7,
            Keycode::Num8 | Keycode::Kp8 => 8,
            Keycode::Num9 | Keycode::Kp9 => 9,
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
pub(crate) fn state_key_help() -> &'static str {
    "Cmd+1..9 load, Cmd+Shift+1..9 save"
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn state_key_help() -> &'static str {
    "Ctrl+1..9 load, Ctrl+Shift+1..9 save"
}
