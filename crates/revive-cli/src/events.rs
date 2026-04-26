use crate::cheat_panel::{CheatPanel, MemorySnapshot};
use crate::hud::HudToast;
use crate::input::{button_label, keycode_button, InputState};
use crate::state::handle_state_key;
use egui_sdl2_gl::painter::Painter;
use egui_sdl2_gl::EguiStateHandler;
use revive_core::CoreInstance;
use sdl2::event::{Event, WindowEvent};
use sdl2::keyboard::{Keycode, Mod, Scancode};
use sdl2::video::Window;

pub(crate) enum EventLoopAction {
    Continue,
    Exit,
}

pub(crate) fn update_egui_time(egui_state: &mut EguiStateHandler) {
    egui_state.input.time = Some(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs_f64(),
    );
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn process_sdl_events(
    event_pump: &mut sdl2::EventPump,
    window: &Window,
    painter: &mut Painter,
    egui_state: &mut EguiStateHandler,
    egui_ctx: &egui::Context,
    core: &mut CoreInstance,
    cheat_panel: &mut CheatPanel,
    input_state: &mut InputState,
    hud_toast: &mut HudToast,
    input_debug: bool,
) -> EventLoopAction {
    for event in event_pump.poll_iter() {
        if cheat_panel.is_visible() {
            if let Some(filtered) = filter_event_for_ascii_text_input(&event) {
                egui_state.process_input(window, filtered, painter);
            }
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
) {
    if key == Keycode::Tab {
        toggle_cheat_panel(core, cheat_panel);
        return;
    }
    if handle_state_key(core, key, scancode, keymod, hud_toast) {
        return;
    }
    if cheat_panel.is_visible() && egui_ctx.wants_keyboard_input() {
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
    if cheat_panel.is_visible() && egui_ctx.wants_keyboard_input() {
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

fn toggle_cheat_panel(core: &CoreInstance, cheat_panel: &mut CheatPanel) {
    if cheat_panel.is_visible() {
        cheat_panel.hide();
    } else {
        let live_memory = MemorySnapshot::capture(core);
        cheat_panel.toggle(&live_memory);
    }
}

fn filter_event_for_ascii_text_input(event: &Event) -> Option<Event> {
    match event {
        Event::TextEditing { .. } => None,
        Event::TextInput {
            timestamp,
            window_id,
            text,
        } => {
            let ascii_text: String = text.chars().filter(|ch| ch.is_ascii()).collect();
            if ascii_text.is_empty() {
                None
            } else {
                Some(Event::TextInput {
                    timestamp: *timestamp,
                    window_id: *window_id,
                    text: ascii_text,
                })
            }
        }
        _ => Some(event.clone()),
    }
}

fn log_focus(state: &str, input_debug: bool) {
    if input_debug {
        eprintln!("input: focus {state}");
    }
}
