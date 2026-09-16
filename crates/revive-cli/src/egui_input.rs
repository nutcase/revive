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
    metrics: UiMetrics,
    start_time: std::time::Instant,
}

impl EguiInput {
    pub(crate) fn new(window: &Window) -> Self {
        Self {
            ctx: egui::Context::default(),
            raw_input: egui::RawInput {
                screen_rect: Some(UiMetrics::for_window(window).screen_rect()),
                ..Default::default()
            },
            modifiers: Modifiers::default(),
            pointer_pos: Pos2::ZERO,
            metrics: UiMetrics::for_window(window),
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
                self.pointer_pos = self.metrics.pointer_pos(*x, *y);
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

    pub(crate) fn update_window_metrics(&mut self, window: &Window) {
        self.metrics = UiMetrics::for_window(window);
        self.metrics.pixels_per_point *= self.ctx.zoom_factor();
    }

    pub(crate) fn panel_width_window_units(&self, width_points: f32) -> u32 {
        self.metrics
            .window_width(width_points * self.ctx.pixels_per_point())
    }

    pub(crate) fn begin_frame(&mut self, window: &Window) -> egui::Context {
        self.update_window_metrics(window);
        self.raw_input.screen_rect = Some(self.metrics.screen_rect());
        self.raw_input
            .viewports
            .entry(egui::ViewportId::ROOT)
            .or_default()
            .native_pixels_per_point = Some(window.display_scale().max(1.0));
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
            .tessellate(full_output.shapes.clone(), full_output.pixels_per_point)
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

// SDL mouse positions and window sizes use window units. On macOS these
// differ from drawable pixels; on Windows display scaling can instead change
// pixels-per-point without changing the window-to-pixel ratio.
#[derive(Clone, Copy)]
struct UiMetrics {
    pixel_size: egui::Vec2,
    pixels_per_window_unit: egui::Vec2,
    pixels_per_point: f32,
}

impl UiMetrics {
    fn new(window_size: (u32, u32), pixel_size: (u32, u32), pixels_per_point: f32) -> Self {
        let pixel_size = egui::vec2(pixel_size.0 as f32, pixel_size.1 as f32);
        Self {
            pixel_size,
            pixels_per_window_unit: pixel_size
                / egui::vec2(window_size.0.max(1) as f32, window_size.1.max(1) as f32),
            pixels_per_point,
        }
    }

    fn for_window(window: &Window) -> Self {
        Self::new(
            window.size(),
            window.size_in_pixels(),
            window.display_scale().max(1.0),
        )
    }

    fn screen_rect(self) -> egui::Rect {
        egui::Rect::from_min_size(Pos2::ZERO, self.pixel_size / self.pixels_per_point)
    }

    fn pointer_pos(self, x: f32, y: f32) -> Pos2 {
        (egui::vec2(x, y) * self.pixels_per_window_unit / self.pixels_per_point).to_pos2()
    }

    fn window_width(self, pixels: f32) -> u32 {
        (pixels / self.pixels_per_window_unit.x.max(f32::EPSILON)).round() as u32
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn side_panel_and_pointer_share_drawable_coordinates_at_all_scales() {
        // macOS Retina has a 2x drawable; Windows can scale egui while SDL
        // mouse coordinates are already physical pixels. Cover both models.
        for (window_size, pixel_size, scale) in [
            ((1380, 720), (1380, 720), 1.0),
            ((1380, 720), (2760, 1440), 2.0),
            ((2070, 1080), (2070, 1080), 1.5),
        ] {
            let metrics = UiMetrics::new(window_size, pixel_size, scale);
            let ctx = egui::Context::default();
            let mut button_rect = egui::Rect::NOTHING;
            let mut clicked = false;
            for pressed in [None, Some(true), Some(false)] {
                let mut input = egui::RawInput {
                    screen_rect: Some(metrics.screen_rect()),
                    ..Default::default()
                };
                input
                    .viewports
                    .get_mut(&egui::ViewportId::ROOT)
                    .unwrap()
                    .native_pixels_per_point = Some(scale);
                if let Some(pressed) = pressed {
                    // Simulate a mouse at the button's displayed pixel center,
                    // as reported in SDL window coordinates.
                    let window_pos =
                        button_rect.center().to_vec2() * scale / metrics.pixels_per_window_unit;
                    let pos = metrics.pointer_pos(window_pos.x, window_pos.y);
                    input.events.push(egui::Event::PointerMoved(pos));
                    input.events.push(egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::default(),
                    });
                }
                ctx.begin_pass(input);
                #[allow(deprecated)]
                let panel = egui::SidePanel::right("memory")
                    .exact_width(420.0)
                    .show(&ctx, |ui| {
                        let button = ui.button("Refresh");
                        button_rect = button.rect;
                        clicked |= button.clicked();
                    });
                let panel_rect = panel.response.rect;
                let output = ctx.end_pass();
                let ppp = output.pixels_per_point;
                assert_eq!(ppp, scale);
                assert_eq!(panel_rect.right() * ppp, pixel_size.0 as f32);
                assert_eq!(panel_rect.bottom() * ppp, pixel_size.1 as f32);
                let reserved_pixels = (panel_rect.width() * ppp).ceil() as u32;
                let game_right = pixel_size.0 - reserved_pixels;
                assert!(game_right as f32 <= panel_rect.left() * ppp);
                assert!((panel_rect.left() * ppp - game_right as f32) < 1.0);
                assert_eq!(
                    metrics.window_width(panel_rect.width() * ppp),
                    if scale == 1.5 { 630 } else { 420 }
                );
            }
            assert!(
                clicked,
                "pointer must hit the visible button at scale {scale}"
            );
        }
    }
}
