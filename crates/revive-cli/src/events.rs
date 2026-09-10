use crate::cheat_panel::CheatPanel;
use crate::egui_input::EguiInput;
use crate::hud::HudToast;
use crate::input::{button_label, keycode_button, InputState};
use crate::state::handle_state_key;
use revive_core::CoreInstance;
use sdl3::event::{Event, WindowEvent};
use sdl3::keyboard::{Keycode, Mod, Scancode};

pub(crate) enum EventLoopAction {
    Continue,
    Exit,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn process_sdl_events(
    event_pump: &mut sdl3::EventPump,
    video: &sdl3::VideoSubsystem,
    egui_input: &mut EguiInput,
    egui_ctx: &egui::Context,
    core: &mut CoreInstance,
    cheat_panel: &mut CheatPanel,
    input_state: &mut InputState,
    hud_toast: &mut HudToast,
    input_debug: bool,
    core_changed: &mut bool,
) -> EventLoopAction {
    for event in event_pump.poll_iter() {
        if cheat_panel.is_visible() {
            egui_input.handle_event(&event, video);
        }

        if matches!(
            handle_event(
                &event,
                egui_ctx,
                core,
                cheat_panel,
                input_state,
                hud_toast,
                input_debug,
                core_changed,
            ),
            EventLoopAction::Exit
        ) {
            return EventLoopAction::Exit;
        }
    }

    EventLoopAction::Continue
}

fn handle_event(
    event: &Event,
    egui_ctx: &egui::Context,
    core: &mut CoreInstance,
    cheat_panel: &mut CheatPanel,
    input_state: &mut InputState,
    hud_toast: &mut HudToast,
    input_debug: bool,
    core_changed: &mut bool,
) -> EventLoopAction {
    match event {
        Event::Quit { .. } => EventLoopAction::Exit,
        Event::Window {
            win_event: WindowEvent::FocusGained,
            ..
        } => {
            log_focus("gained", input_debug);
            EventLoopAction::Continue
        }
        Event::Window {
            win_event: WindowEvent::FocusLost,
            ..
        } => {
            log_focus("lost", input_debug);
            input_state.clear();
            EventLoopAction::Continue
        }
        Event::KeyDown {
            keycode: Some(Keycode::Escape),
            repeat: false,
            ..
        } if cheat_panel.is_visible() => {
            cheat_panel.hide();
            EventLoopAction::Continue
        }
        Event::KeyDown {
            keycode: Some(Keycode::Escape),
            ..
        } => EventLoopAction::Exit,
        Event::KeyDown {
            keycode: Some(key),
            scancode,
            keymod,
            repeat: false,
            ..
        } => {
            handle_key_down(
                core,
                cheat_panel,
                input_state,
                hud_toast,
                egui_ctx,
                *key,
                *scancode,
                *keymod,
                input_debug,
                core_changed,
            );
            EventLoopAction::Continue
        }
        Event::KeyUp {
            keycode: Some(key),
            repeat: false,
            ..
        } => {
            handle_key_up(core, cheat_panel, input_state, egui_ctx, *key, input_debug);
            EventLoopAction::Continue
        }
        _ => EventLoopAction::Continue,
    }
}

#[allow(clippy::too_many_arguments)]
fn handle_key_down(
    core: &mut CoreInstance,
    cheat_panel: &mut CheatPanel,
    input_state: &mut InputState,
    hud_toast: &mut HudToast,
    egui_ctx: &egui::Context,
    key: Keycode,
    scancode: Option<Scancode>,
    keymod: Mod,
    input_debug: bool,
    core_changed: &mut bool,
) {
    if key == Keycode::Tab {
        cheat_panel.toggle();
        return;
    }
    if handle_state_key(core, key, scancode, keymod, hud_toast) {
        *core_changed = true;
        return;
    }
    if cheat_panel.is_visible() && egui_ctx.egui_wants_keyboard_input() {
        return;
    }
    if let Some(button) = keycode_button(core.system(), key) {
        if input_debug {
            eprintln!("input: key down {key:?} -> {}", button_label(button));
        }
        input_state.set(button, true);
    } else if input_debug {
        eprintln!("input: key down {key:?}");
    }
}

fn handle_key_up(
    core: &CoreInstance,
    cheat_panel: &CheatPanel,
    input_state: &mut InputState,
    egui_ctx: &egui::Context,
    key: Keycode,
    input_debug: bool,
) {
    if cheat_panel.is_visible() && egui_ctx.egui_wants_keyboard_input() {
        return;
    }
    if let Some(button) = keycode_button(core.system(), key) {
        if input_debug {
            eprintln!("input: key up {key:?} -> {}", button_label(button));
        }
        input_state.set(button, false);
    } else if input_debug {
        eprintln!("input: key up {key:?}");
    }
}

fn log_focus(state: &str, input_debug: bool) {
    if input_debug {
        eprintln!("input: focus {state}");
    }
}
