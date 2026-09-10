use egui::{Modifiers, Pos2};
use sdl3::event::{Event, WindowEvent};
use sdl3::keyboard::{Keycode, Mod};
use sdl3::mouse::MouseButton;
use sdl3::video::Window;

pub(crate) struct EguiInput {
    ctx: egui::Context,
    raw_input: egui::RawInput,
    modifiers: Modifiers,
    pointer_pos: Pos2,
    start_time: std::time::Instant,
}

impl EguiInput {
    pub(crate) fn new(window: &Window) -> Self {
        Self {
            ctx: egui::Context::default(),
            raw_input: egui::RawInput {
                screen_rect: Some(window_screen_rect(window)),
                ..Default::default()
            },
            modifiers: Modifiers::default(),
            pointer_pos: Pos2::ZERO,
            start_time: std::time::Instant::now(),
        }
    }

    pub(crate) fn context(&self) -> egui::Context {
        self.ctx.clone()
    }

    pub(crate) fn handle_event(&mut self, event: &Event, video: &sdl3::VideoSubsystem) {
        match event {
            Event::Window {
                win_event: WindowEvent::Resized(_, _) | WindowEvent::PixelSizeChanged(_, _),
                ..
            } => {}
            Event::MouseMotion { x, y, .. } => {
                self.pointer_pos = Pos2::new(*x, *y) / self.ctx.pixels_per_point();
                self.raw_input
                    .events
                    .push(egui::Event::PointerMoved(self.pointer_pos));
            }
            Event::MouseButtonDown { mouse_btn, .. } => {
                if let Some(button) = pointer_button(*mouse_btn) {
                    self.raw_input.events.push(egui::Event::PointerButton {
                        pos: self.pointer_pos,
                        button,
                        pressed: true,
                        modifiers: self.modifiers,
                    });
                }
            }
            Event::MouseButtonUp { mouse_btn, .. } => {
                if let Some(button) = pointer_button(*mouse_btn) {
                    self.raw_input.events.push(egui::Event::PointerButton {
                        pos: self.pointer_pos,
                        button,
                        pressed: false,
                        modifiers: self.modifiers,
                    });
                }
            }
            Event::MouseWheel { x, y, .. } => {
                self.raw_input.events.push(egui::Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    delta: egui::vec2(*x, *y) * 32.0,
                    phase: egui::TouchPhase::Move,
                    modifiers: self.modifiers,
                });
            }
            Event::KeyDown {
                keycode,
                keymod,
                repeat,
                ..
            } => {
                self.update_modifiers(*keymod);
                if let Some(keycode) = keycode {
                    self.handle_shortcut(*keycode, video);
                    if let Some(key) = egui_key(*keycode) {
                        self.raw_input.events.push(egui::Event::Key {
                            key,
                            physical_key: Some(key),
                            pressed: true,
                            repeat: *repeat,
                            modifiers: self.modifiers,
                        });
                    }
                }
            }
            Event::KeyUp {
                keycode, keymod, ..
            } => {
                self.update_modifiers(*keymod);
                if let Some(keycode) = keycode.and_then(egui_key) {
                    self.raw_input.events.push(egui::Event::Key {
                        key: keycode,
                        physical_key: Some(keycode),
                        pressed: false,
                        repeat: false,
                        modifiers: self.modifiers,
                    });
                }
            }
            Event::TextInput { text, .. } => {
                if !text.is_empty() && !text.chars().any(char::is_control) {
                    self.raw_input.events.push(egui::Event::Text(text.clone()));
                }
            }
            _ => {}
        }
    }

    pub(crate) fn begin_frame(&mut self, window: &Window) -> egui::Context {
        let pixels_per_point = window.display_scale().max(1.0);
        self.ctx.set_pixels_per_point(pixels_per_point);
        self.raw_input.screen_rect = Some(window_screen_rect(window));
        self.raw_input.time = Some(self.start_time.elapsed().as_secs_f64());
        self.ctx.begin_pass(self.raw_input.take());
        self.ctx.clone()
    }

    pub(crate) fn end_frame(&mut self, video: &mut sdl3::VideoSubsystem) -> egui::FullOutput {
        let output = self.ctx.end_pass();
        for command in &output.platform_output.commands {
            if let egui::OutputCommand::CopyText(text) = command {
                let _ = video.clipboard().set_clipboard_text(text);
            }
        }
        output
    }

    pub(crate) fn tessellate(&self, full_output: &egui::FullOutput) -> Vec<egui::ClippedPrimitive> {
        self.ctx
            .tessellate(full_output.shapes.clone(), self.ctx.pixels_per_point())
    }

    fn update_modifiers(&mut self, keymod: Mod) {
        let alt = keymod.intersects(Mod::LALTMOD | Mod::RALTMOD);
        let ctrl = keymod.intersects(Mod::LCTRLMOD | Mod::RCTRLMOD);
        let shift = keymod.intersects(Mod::LSHIFTMOD | Mod::RSHIFTMOD);
        let mac_cmd = keymod.intersects(Mod::LGUIMOD | Mod::RGUIMOD);
        let command = if cfg!(target_os = "macos") {
            mac_cmd
        } else {
            ctrl
        };
        self.modifiers = Modifiers {
            alt,
            ctrl,
            shift,
            mac_cmd,
            command,
        };
        self.raw_input.modifiers = self.modifiers;
    }

    fn handle_shortcut(&mut self, keycode: Keycode, video: &sdl3::VideoSubsystem) {
        if !self.modifiers.command {
            return;
        }
        match egui_key(keycode) {
            Some(egui::Key::C) => self.raw_input.events.push(egui::Event::Copy),
            Some(egui::Key::X) => self.raw_input.events.push(egui::Event::Cut),
            Some(egui::Key::V) => {
                let clipboard = video.clipboard();
                if clipboard.has_clipboard_text() {
                    if let Ok(text) = clipboard.clipboard_text() {
                        self.raw_input.events.push(egui::Event::Paste(text));
                    }
                }
            }
            _ => {}
        }
    }
}

fn window_screen_rect(window: &Window) -> egui::Rect {
    let (width, height) = window.size();
    let scale = window.display_scale().max(1.0);
    egui::Rect::from_min_size(
        egui::Pos2::ZERO,
        egui::vec2(width as f32, height as f32) / scale,
    )
}

fn pointer_button(button: MouseButton) -> Option<egui::PointerButton> {
    match button {
        MouseButton::Left => Some(egui::PointerButton::Primary),
        MouseButton::Middle => Some(egui::PointerButton::Middle),
        MouseButton::Right => Some(egui::PointerButton::Secondary),
        _ => None,
    }
}

fn egui_key(keycode: Keycode) -> Option<egui::Key> {
    Some(match keycode {
        Keycode::Left => egui::Key::ArrowLeft,
        Keycode::Up => egui::Key::ArrowUp,
        Keycode::Right => egui::Key::ArrowRight,
        Keycode::Down => egui::Key::ArrowDown,
        Keycode::Escape => egui::Key::Escape,
        Keycode::Tab => egui::Key::Tab,
        Keycode::Backspace => egui::Key::Backspace,
        Keycode::Space => egui::Key::Space,
        Keycode::Return => egui::Key::Enter,
        Keycode::Insert => egui::Key::Insert,
        Keycode::Home => egui::Key::Home,
        Keycode::Delete => egui::Key::Delete,
        Keycode::End => egui::Key::End,
        Keycode::PageDown => egui::Key::PageDown,
        Keycode::PageUp => egui::Key::PageUp,
        Keycode::Kp0 | Keycode::_0 => egui::Key::Num0,
        Keycode::Kp1 | Keycode::_1 => egui::Key::Num1,
        Keycode::Kp2 | Keycode::_2 => egui::Key::Num2,
        Keycode::Kp3 | Keycode::_3 => egui::Key::Num3,
        Keycode::Kp4 | Keycode::_4 => egui::Key::Num4,
        Keycode::Kp5 | Keycode::_5 => egui::Key::Num5,
        Keycode::Kp6 | Keycode::_6 => egui::Key::Num6,
        Keycode::Kp7 | Keycode::_7 => egui::Key::Num7,
        Keycode::Kp8 | Keycode::_8 => egui::Key::Num8,
        Keycode::Kp9 | Keycode::_9 => egui::Key::Num9,
        Keycode::A => egui::Key::A,
        Keycode::B => egui::Key::B,
        Keycode::C => egui::Key::C,
        Keycode::D => egui::Key::D,
        Keycode::E => egui::Key::E,
        Keycode::F => egui::Key::F,
        Keycode::G => egui::Key::G,
        Keycode::H => egui::Key::H,
        Keycode::I => egui::Key::I,
        Keycode::J => egui::Key::J,
        Keycode::K => egui::Key::K,
        Keycode::L => egui::Key::L,
        Keycode::M => egui::Key::M,
        Keycode::N => egui::Key::N,
        Keycode::O => egui::Key::O,
        Keycode::P => egui::Key::P,
        Keycode::Q => egui::Key::Q,
        Keycode::R => egui::Key::R,
        Keycode::S => egui::Key::S,
        Keycode::T => egui::Key::T,
        Keycode::U => egui::Key::U,
        Keycode::V => egui::Key::V,
        Keycode::W => egui::Key::W,
        Keycode::X => egui::Key::X,
        Keycode::Y => egui::Key::Y,
        Keycode::Z => egui::Key::Z,
        _ => return None,
    })
}
