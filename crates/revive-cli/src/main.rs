use std::error::Error;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

mod cheat_panel;
mod gl_game;

use cheat_panel::{CheatPanel, MemorySnapshot};
use egui_sdl2_gl::gl;
use egui_sdl2_gl::{DpiScaling, ShaderVersion};
use gl_game::GlGameRenderer;
use revive_cheat::CheatManager;
use revive_core::{CoreInstance, SystemKind, VirtualButton};
use sdl2::audio::{AudioQueue, AudioSpecDesired};
use sdl2::event::{Event, WindowEvent};
use sdl2::keyboard::{KeyboardState, Keycode, Mod, Scancode};
use sdl2::video::{GLProfile, SwapInterval, Window};

const DEFAULT_SCALE: u32 = 3;
const PANEL_WIDTH_DEFAULT: f32 = 420.0;
const PANEL_WIDTH_MIN: f32 = 300.0;
const HUD_TOAST_DURATION: Duration = Duration::from_millis(1400);
const HUD_TOAST_FONT_SIZE: f32 = 20.0;
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
struct InputState {
    pressed: [bool; INPUT_BUTTONS.len()],
}

impl InputState {
    fn set(&mut self, button: VirtualButton, pressed: bool) {
        self.pressed[button_index(button)] = pressed;
    }

    fn is_pressed(&self, button: VirtualButton) -> bool {
        self.pressed[button_index(button)]
    }

    fn clear(&mut self) {
        self.pressed.fill(false);
    }
}

#[derive(Debug, Default)]
struct HudToast {
    text: String,
    expires_at: Option<Instant>,
}

impl HudToast {
    fn show(&mut self, text: impl Into<String>) {
        self.text = text.into();
        self.expires_at = Some(Instant::now() + HUD_TOAST_DURATION);
    }

    fn is_visible(&self) -> bool {
        self.expires_at
            .is_some_and(|expires_at| Instant::now() < expires_at)
    }

    fn draw(&mut self, ctx: &egui::Context) {
        if !self.is_visible() {
            self.expires_at = None;
            return;
        }

        egui::Area::new(egui::Id::new("state_hud_toast"))
            .anchor(egui::Align2::LEFT_TOP, egui::vec2(12.0, 12.0))
            .interactable(false)
            .show(ctx, |ui| {
                egui::Frame::default()
                    .fill(egui::Color32::from_rgba_premultiplied(18, 18, 18, 220))
                    .stroke(egui::Stroke::new(1.0, egui::Color32::from_gray(82)))
                    .inner_margin(egui::Margin::symmetric(12, 8))
                    .show(ui, |ui| {
                        ui.label(
                            egui::RichText::new(&self.text)
                                .strong()
                                .size(HUD_TOAST_FONT_SIZE)
                                .color(egui::Color32::WHITE),
                        );
                    });
            });
    }
}

#[derive(Debug)]
struct Options {
    rom_path: Option<PathBuf>,
    system: Option<SystemKind>,
    cheat_path: Option<PathBuf>,
    no_audio: bool,
    select_rom: bool,
}

fn main() {
    if let Err(err) = run() {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let options = parse_args(std::env::args().skip(1))?;
    let Some(rom_path) = resolve_rom_path(&options) else {
        println!("ROM selection canceled");
        return Ok(());
    };

    let core = CoreInstance::load_rom(&rom_path, options.system)?;
    let system = core.system();
    let (cheat_path, legacy_cheat_path) = match &options.cheat_path {
        Some(path) => (path.clone(), None),
        None => (
            default_cheat_path(system, &rom_path),
            Some(legacy_cheat_path(&rom_path)),
        ),
    };
    let cheats = load_cheats(
        &cheat_path,
        options.cheat_path.is_some(),
        legacy_cheat_path.as_deref(),
    )?;

    println!("Loaded      : {}", rom_path.display());
    println!("System      : {}", core.system().label());
    println!("Title       : {}", core.title());
    println!("Cheats      : {}", cheat_path.display());
    for region in core.memory_regions() {
        println!(
            "Memory      : {} ({}, {} bytes)",
            region.id, region.label, region.len
        );
    }
    println!("State keys  : Cmd+1..9 load, Cmd+Shift+1..9 save");
    println!("Controls    : arrows move, Enter start, Shift/Backspace select");
    println!("Cheat panel : Tab toggle");

    run_sdl_loop(core, cheats, &cheat_path, &options)?;
    Ok(())
}

fn parse_args<I>(args: I) -> Result<Options, Box<dyn Error>>
where
    I: IntoIterator<Item = String>,
{
    let mut args = args.into_iter().peekable();
    if matches!(args.peek().map(String::as_str), Some("run")) {
        args.next();
    }

    let mut rom_path = None;
    let mut system = None;
    let mut cheat_path = None;
    let mut no_audio = false;
    let mut select_rom = false;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => {
                print_usage();
                std::process::exit(0);
            }
            "--system" => {
                let value = args.next().ok_or("--system requires a value")?;
                system = Some(
                    SystemKind::parse(&value).ok_or_else(|| format!("unknown system '{value}'"))?,
                );
            }
            "--cheats" => {
                let value = args.next().ok_or("--cheats requires a JSON file path")?;
                cheat_path = Some(PathBuf::from(value));
            }
            "--no-audio" => {
                no_audio = true;
            }
            "--select" => {
                select_rom = true;
            }
            _ if arg.starts_with('-') => return Err(format!("unknown option: {arg}").into()),
            _ => {
                if rom_path.is_some() {
                    return Err("multiple ROM paths provided".into());
                }
                rom_path = Some(PathBuf::from(arg));
            }
        }
    }

    Ok(Options {
        rom_path,
        system,
        cheat_path,
        no_audio,
        select_rom,
    })
}

fn print_usage() {
    println!("Usage:");
    println!(
        "  revive [rom] [--system nes|snes|megadrive|pce|gb|gbc|gba] [--cheats file.json] [--no-audio]"
    );
    println!(
        "  revive run [rom] [--system nes|snes|megadrive|pce|gb|gbc|gba] [--cheats file.json] [--no-audio]"
    );
    println!("  revive --select");
    println!();
    println!("If no ROM path is provided, a local file selection dialog opens.");
    println!("Supported ROM extensions: .nes, .sfc, .smc, .md, .gen, .pce, .gb, .gbc, .gba, .bin");
}

fn resolve_rom_path(options: &Options) -> Option<PathBuf> {
    if options.select_rom || options.rom_path.is_none() {
        select_rom_path()
    } else {
        options.rom_path.clone()
    }
}

fn select_rom_path() -> Option<PathBuf> {
    rfd::FileDialog::new()
        .set_title("Select ROM")
        .add_filter(
            "ROM files",
            &[
                "nes", "sfc", "smc", "md", "gen", "pce", "gb", "gbc", "gba", "bin",
            ],
        )
        .add_filter("NES", &["nes"])
        .add_filter("SNES", &["sfc", "smc"])
        .add_filter("Mega Drive", &["md", "gen", "bin"])
        .add_filter("PC Engine", &["pce"])
        .add_filter("Game Boy", &["gb", "gbc"])
        .add_filter("Game Boy Advance", &["gba"])
        .pick_file()
}

fn default_cheat_path(system: SystemKind, rom_path: &Path) -> PathBuf {
    PathBuf::from("cheats")
        .join(system_dir(system))
        .join(rom_file_stem(rom_path))
        .join("cheats.json")
}

fn legacy_cheat_path(rom_path: &Path) -> PathBuf {
    PathBuf::from("cheats").join(format!("{}.json", rom_file_stem(rom_path)))
}

fn rom_file_stem(rom_path: &Path) -> String {
    rom_path
        .file_stem()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or("game")
        .to_string()
}

fn system_dir(system: SystemKind) -> &'static str {
    match system {
        SystemKind::Nes => "nes",
        SystemKind::Snes => "snes",
        SystemKind::MegaDrive => "megadrive",
        SystemKind::Pce => "pce",
        SystemKind::GameBoy => "gb",
        SystemKind::GameBoyColor => "gbc",
        SystemKind::GameBoyAdvance => "gba",
    }
}

fn load_cheats(
    path: &Path,
    required: bool,
    legacy_path: Option<&Path>,
) -> Result<CheatManager, Box<dyn Error>> {
    if path.exists() {
        let manager = CheatManager::load_from_file(path)?;
        println!("Loaded cheats: {}", manager.entries.len());
        return Ok(manager);
    }
    if let Some(legacy_path) = legacy_path.filter(|legacy_path| legacy_path.exists()) {
        let manager = CheatManager::load_from_file(legacy_path)?;
        println!(
            "Loaded legacy cheats: {} ({})",
            manager.entries.len(),
            legacy_path.display()
        );
        return Ok(manager);
    }
    if required {
        return Err(format!("cheat file does not exist: {}", path.display()).into());
    }
    Ok(CheatManager::new())
}

fn run_sdl_loop(
    mut core: CoreInstance,
    mut cheats: CheatManager,
    cheat_path: &Path,
    options: &Options,
) -> Result<(), Box<dyn Error>> {
    sdl2::hint::set("SDL_DISABLE_IMMINTRIN_H", "1");
    sdl2::hint::set("SDL_MAC_CTRL_CLICK_EMULATE_RIGHT_CLICK", "0");

    let sdl = sdl2::init().map_err(sdl_error)?;
    let video = sdl.video().map_err(sdl_error)?;
    let gl_attr = video.gl_attr();
    gl_attr.set_context_profile(GLProfile::Core);
    gl_attr.set_context_version(3, 2);
    gl_attr.set_double_buffer(true);
    gl_attr.set_multisample_samples(0);

    let (frame_width, frame_height) = {
        let frame = core.frame();
        (frame.width, frame.height)
    };
    let mut game_w = frame_width as u32 * DEFAULT_SCALE;
    let mut game_h = frame_height as u32 * DEFAULT_SCALE;

    let window_title = format!("Revive - {} - {}", core.system().label(), core.title());
    let mut window = video
        .window(&window_title, game_w, game_h)
        .position_centered()
        .resizable()
        .opengl()
        .build()
        .map_err(|err| io::Error::other(err.to_string()))?;

    let gl_context = window
        .gl_create_context()
        .map_err(|err| io::Error::other(err.to_string()))?;
    window
        .gl_make_current(&gl_context)
        .map_err(|err| io::Error::other(err.to_string()))?;
    gl::load_with(|name| video.gl_get_proc_address(name) as *const _);
    let _ = video.gl_set_swap_interval(SwapInterval::Immediate);

    bring_window_to_front(&mut window);
    let (mut painter, mut egui_state) =
        egui_sdl2_gl::with_sdl2(&window, ShaderVersion::Default, DpiScaling::Default);
    let egui_ctx = egui::Context::default();
    let text_input = video.text_input();
    let mut text_input_active = false;
    text_input.stop();

    let mut game_renderer = GlGameRenderer::new();
    let mut texture_size = (frame_width, frame_height);

    let audio_queue = if options.no_audio {
        None
    } else {
        Some(open_audio_queue(&sdl, &mut core)?)
    };
    let mut audio_queue = audio_queue;
    let mut audio_scratch = Vec::new();
    let mut event_pump = sdl.event_pump().map_err(sdl_error)?;
    let mut frame_clock = FrameClock::new(core.system());
    let mut input_state = InputState::default();
    let mut cheat_panel = CheatPanel::new();
    let mut hud_toast = HudToast::default();
    let mut prev_panel_visible = cheat_panel.is_visible();
    let mut panel_width_px = PANEL_WIDTH_DEFAULT as u32;
    let input_debug = std::env::var_os("REVIVE_INPUT_DEBUG").is_some();
    let mut front_retry_frames = 12u8;

    'running: loop {
        let should_enable_text_input = cheat_panel.is_visible();
        if should_enable_text_input != text_input_active {
            if should_enable_text_input {
                text_input.start();
            } else {
                text_input.stop();
            }
            text_input_active = should_enable_text_input;
        }
        egui_state.input.time = Some(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs_f64(),
        );

        for event in event_pump.poll_iter() {
            if cheat_panel.is_visible() {
                if let Some(filtered) = filter_event_for_ascii_text_input(&event) {
                    egui_state.process_input(&window, filtered, &mut painter);
                }
            }

            match &event {
                Event::Quit { .. } => break 'running,
                Event::Window {
                    win_event: WindowEvent::FocusGained,
                    ..
                } => {
                    if input_debug {
                        eprintln!("input: focus gained");
                    }
                }
                Event::Window {
                    win_event: WindowEvent::FocusLost,
                    ..
                } => {
                    if input_debug {
                        eprintln!("input: focus lost");
                    }
                    input_state.clear();
                }
                Event::KeyDown {
                    keycode: Some(Keycode::Escape),
                    repeat: false,
                    ..
                } if cheat_panel.is_visible() => {
                    cheat_panel.hide();
                    continue;
                }
                Event::KeyDown {
                    keycode: Some(Keycode::Escape),
                    ..
                } => break 'running,
                Event::KeyDown {
                    keycode: Some(key),
                    scancode,
                    keymod,
                    repeat: false,
                    ..
                } => {
                    let key = *key;
                    let scancode = *scancode;
                    let keymod = *keymod;
                    if key == Keycode::Tab {
                        if cheat_panel.is_visible() {
                            cheat_panel.hide();
                        } else {
                            let live_memory = MemorySnapshot::capture(&core);
                            cheat_panel.toggle(&live_memory);
                        }
                        continue;
                    }
                    if handle_state_key(&mut core, key, scancode, keymod, &mut hud_toast) {
                        continue;
                    }
                    if cheat_panel.is_visible() && egui_ctx.wants_keyboard_input() {
                        continue;
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
                Event::KeyUp {
                    keycode: Some(key),
                    repeat: false,
                    ..
                } => {
                    let key = *key;
                    // Mirror KeyDown: only swallow the release when egui
                    // actually owns keyboard focus (e.g. a cheat text
                    // field is active). Dropping every KeyUp while the
                    // panel is open left game buttons stuck down.
                    if cheat_panel.is_visible() && egui_ctx.wants_keyboard_input() {
                        continue;
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
                _ => {}
            }
        }

        if cheat_panel.is_visible() != prev_panel_visible {
            let new_w = if cheat_panel.is_visible() {
                game_w + panel_width_px
            } else {
                game_w
            };
            let _ = window.set_size(new_w, game_h);
            prev_panel_visible = cheat_panel.is_visible();
        }

        if cheat_panel.is_visible() && egui_ctx.wants_keyboard_input() {
            input_state.clear();
            release_keyboard_input(&mut core);
        } else {
            sync_keyboard_input(&mut core, &event_pump, &input_state);
        }
        apply_cheats(&mut core, &cheats);
        if !cheat_panel.is_paused() {
            core.step_frame()?;
        }
        apply_cheats(&mut core, &cheats);

        if let Some(queue) = audio_queue.as_mut() {
            feed_audio(queue, &mut core, &mut audio_scratch)?;
        } else {
            core.drain_audio_i16(&mut audio_scratch);
        }

        {
            let frame = core.frame();
            if (frame.width, frame.height) != texture_size {
                texture_size = (frame.width, frame.height);
                game_w = frame.width as u32 * DEFAULT_SCALE;
                game_h = frame.height as u32 * DEFAULT_SCALE;
                let new_w = if cheat_panel.is_visible() {
                    game_w + panel_width_px
                } else {
                    game_w
                };
                let _ = window.set_size(new_w, game_h);
            }
            game_renderer.upload_frame(frame.data, frame.width, frame.height, frame.format);
        }

        let (win_w, win_h) = window.size();
        unsafe {
            gl::ClearColor(0.0, 0.0, 0.0, 1.0);
            gl::Clear(gl::COLOR_BUFFER_BIT);
        }
        let panel_px = if cheat_panel.is_visible() {
            panel_width_px
        } else {
            0
        };
        let game_vp_w = win_w.saturating_sub(panel_px);
        game_renderer.draw(0, 0, game_vp_w as i32, win_h as i32);

        let draw_ui = cheat_panel.is_visible() || hud_toast.is_visible();
        if draw_ui {
            unsafe {
                gl::Viewport(0, 0, win_w as i32, win_h as i32);
                gl::Enable(gl::BLEND);
                gl::BlendFunc(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA);
                gl::Enable(gl::SCISSOR_TEST);
            }
            painter.update_screen_rect((win_w, win_h));
            egui_state.input.screen_rect = Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(win_w as f32, win_h as f32),
            ));

            let mut pending_writes = Vec::new();
            let full_output = egui_ctx.run(egui_state.input.take(), |ctx| {
                if cheat_panel.is_visible() {
                    let live_memory = MemorySnapshot::capture(&core);
                    let panel_resp = egui::SidePanel::right("cheat_panel")
                        .resizable(true)
                        .min_width(PANEL_WIDTH_MIN)
                        .default_width(PANEL_WIDTH_DEFAULT)
                        .show(ctx, |ui| {
                            egui::ScrollArea::vertical()
                                .auto_shrink([false, false])
                                .show(ui, |ui| {
                                    pending_writes = cheat_panel.show_panel(
                                        ui,
                                        &live_memory,
                                        &mut cheats,
                                        Some(cheat_path),
                                    );
                                });
                        });
                    let actual_w = panel_resp.response.rect.width() as u32;
                    if actual_w != panel_width_px {
                        panel_width_px = actual_w;
                        let _ = window.set_size(game_w + panel_width_px, game_h);
                    }
                }
                hud_toast.draw(ctx);
            });

            let prims = egui_ctx.tessellate(full_output.shapes, full_output.pixels_per_point);
            painter.paint_jobs(None, full_output.textures_delta, prims);
            egui_state.process_output(&window, &full_output.platform_output);

            for write in pending_writes {
                core.write_memory_byte(&write.region, write.offset, write.value);
            }
        }

        window.gl_swap_window();
        if front_retry_frames > 0 {
            bring_window_to_front(&mut window);
            front_retry_frames -= 1;
        }
        frame_clock.wait();
    }

    if let Err(err) = core.flush_persistent_save() {
        eprintln!("warning: failed to flush persistent save: {err}");
    }

    Ok(())
}

fn bring_window_to_front(window: &mut Window) {
    window.show();
    window.raise();
    platform_bring_window_to_front(window);
}

#[cfg(target_os = "macos")]
fn platform_bring_window_to_front(window: &Window) {
    macos_frontmost::activate_window(window);
}

#[cfg(not(target_os = "macos"))]
fn platform_bring_window_to_front(_window: &Window) {}

fn open_audio_queue(
    sdl: &sdl2::Sdl,
    core: &mut CoreInstance,
) -> Result<AudioQueue<i16>, Box<dyn Error>> {
    let audio = sdl.audio().map_err(sdl_error)?;
    let spec = core.audio_spec();
    let desired = AudioSpecDesired {
        freq: Some(spec.sample_rate_hz as i32),
        channels: Some(spec.channels),
        samples: Some(1024),
    };
    let queue = audio
        .open_queue::<i16, _>(None, &desired)
        .map_err(|err| io::Error::other(err.to_string()))?;
    let obtained = queue.spec();
    core.configure_audio_output(obtained.freq.max(8_000) as u32);
    queue.resume();
    println!(
        "Audio       : {} Hz, {} ch",
        obtained.freq, obtained.channels
    );
    Ok(queue)
}

fn feed_audio(
    queue: &mut AudioQueue<i16>,
    core: &mut CoreInstance,
    scratch: &mut Vec<i16>,
) -> Result<(), Box<dyn Error>> {
    let spec = queue.spec();
    let channels = usize::from(spec.channels.max(1));
    let queued_i16 = queue.size() as usize / std::mem::size_of::<i16>();
    let queued_frames = queued_i16 / channels;
    let target_frames = ((spec.freq.max(8_000) as usize) / 30).clamp(512, 2048);

    core.drain_audio_i16(scratch);
    if queued_frames < target_frames && !scratch.is_empty() {
        queue
            .queue_audio(scratch)
            .map_err(|err| io::Error::other(err.to_string()))?;
    }
    Ok(())
}

fn apply_cheats(core: &mut CoreInstance, cheats: &CheatManager) {
    for entry in cheats.enabled_entries() {
        core.write_memory_byte(&entry.region, entry.offset as usize, entry.value);
    }
}

fn handle_state_key(
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
    keymod.intersects(Mod::LGUIMOD | Mod::RGUIMOD)
}

fn state_save_modifier(keymod: Mod) -> bool {
    keymod.intersects(Mod::LSHIFTMOD | Mod::RSHIFTMOD)
}

fn sync_keyboard_input(
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

fn release_keyboard_input(core: &mut CoreInstance) {
    for button in INPUT_BUTTONS {
        core.set_button(1, button, false);
    }
}

fn keycode_button(system: SystemKind, key: Keycode) -> Option<VirtualButton> {
    match system {
        SystemKind::Nes => nes_keycode_button(key),
        SystemKind::Snes => snes_keycode_button(key),
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

fn button_label(button: VirtualButton) -> &'static str {
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

struct FrameClock {
    last_frame: Instant,
    frame_duration: Duration,
}

impl FrameClock {
    fn new(system: SystemKind) -> Self {
        let fps = match system {
            SystemKind::Nes => 60.0988,
            SystemKind::Snes => 60.0988,
            SystemKind::MegaDrive => 59.9227,
            SystemKind::Pce => 60.0,
            SystemKind::GameBoy | SystemKind::GameBoyColor | SystemKind::GameBoyAdvance => 59.7275,
        };
        Self {
            last_frame: Instant::now(),
            frame_duration: Duration::from_secs_f64(1.0 / fps),
        }
    }

    fn wait(&mut self) {
        let target = self.last_frame + self.frame_duration;
        let now = Instant::now();
        if now < target {
            std::thread::sleep(target - now);
            self.last_frame = target;
        } else {
            self.last_frame = now;
        }
    }
}

fn sdl_error(message: String) -> io::Error {
    io::Error::other(message)
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

#[cfg(target_os = "macos")]
mod macos_frontmost {
    use std::ffi::{c_char, c_void, CString};

    use sdl2::video::Window;

    const SDL_SYSWM_COCOA: u32 = 4;
    const NS_APPLICATION_ACTIVATION_POLICY_REGULAR: isize = 0;
    const NS_APPLICATION_ACTIVATE_ALL_WINDOWS: usize = 1 << 0;
    const NS_APPLICATION_ACTIVATE_IGNORING_OTHER_APPS: usize = 1 << 1;

    #[repr(C)]
    union SdlSysWmInfoData {
        cocoa: CocoaInfo,
        dummy: [u8; 64],
        _align: [u64; 8],
    }

    #[repr(C)]
    #[derive(Copy, Clone)]
    struct CocoaInfo {
        window: *mut c_void,
    }

    #[repr(C)]
    struct SdlSysWmInfo {
        version: sdl2::sys::SDL_version,
        subsystem: u32,
        info: SdlSysWmInfoData,
    }

    #[link(name = "objc")]
    unsafe extern "C" {
        fn SDL_GetWindowWMInfo(
            window: *mut sdl2::sys::SDL_Window,
            info: *mut SdlSysWmInfo,
        ) -> sdl2::sys::SDL_bool;

        fn objc_getClass(name: *const c_char) -> *mut c_void;
        fn sel_registerName(name: *const c_char) -> *mut c_void;
        fn objc_msgSend();
    }

    pub fn activate_window(window: &Window) {
        let Some(ns_window) = ns_window(window) else {
            return;
        };
        unsafe {
            activate_application();
            send_void_no_args(ns_window, sel("makeMainWindow"));
            send_void_no_args(ns_window, sel("makeKeyWindow"));
            send_void(
                ns_window,
                sel("makeKeyAndOrderFront:"),
                std::ptr::null_mut(),
            );
            send_void_no_args(ns_window, sel("orderFrontRegardless"));
        }
    }

    fn ns_window(window: &Window) -> Option<*mut c_void> {
        unsafe {
            let mut info: SdlSysWmInfo = std::mem::zeroed();
            sdl2::sys::SDL_GetVersion(&mut info.version);
            if SDL_GetWindowWMInfo(window.raw(), &mut info) == sdl2::sys::SDL_bool::SDL_FALSE {
                return None;
            }
            if info.subsystem != SDL_SYSWM_COCOA {
                return None;
            }
            let ns_window = info.info.cocoa.window;
            (!ns_window.is_null()).then_some(ns_window)
        }
    }

    unsafe fn activate_application() {
        let ns_application = objc_getClass(cstr("NSApplication").as_ptr());
        if ns_application.is_null() {
            return;
        }
        let app = send_id(ns_application, sel("sharedApplication"));
        if app.is_null() {
            return;
        }
        let _ = send_isize_bool(
            app,
            sel("setActivationPolicy:"),
            NS_APPLICATION_ACTIVATION_POLICY_REGULAR,
        );
        send_bool(app, sel("activateIgnoringOtherApps:"), true);

        let ns_running_application = objc_getClass(cstr("NSRunningApplication").as_ptr());
        if ns_running_application.is_null() {
            return;
        }
        let running_app = send_id(ns_running_application, sel("currentApplication"));
        if running_app.is_null() {
            return;
        }
        let _ = send_usize_bool(
            running_app,
            sel("activateWithOptions:"),
            NS_APPLICATION_ACTIVATE_ALL_WINDOWS | NS_APPLICATION_ACTIVATE_IGNORING_OTHER_APPS,
        );
    }

    unsafe fn send_id(receiver: *mut c_void, selector: *mut c_void) -> *mut c_void {
        let send: unsafe extern "C" fn(*mut c_void, *mut c_void) -> *mut c_void =
            std::mem::transmute(objc_msgSend as *const ());
        send(receiver, selector)
    }

    unsafe fn send_bool(receiver: *mut c_void, selector: *mut c_void, value: bool) {
        let send: unsafe extern "C" fn(*mut c_void, *mut c_void, bool) =
            std::mem::transmute(objc_msgSend as *const ());
        send(receiver, selector, value);
    }

    unsafe fn send_isize_bool(receiver: *mut c_void, selector: *mut c_void, value: isize) -> bool {
        let send: unsafe extern "C" fn(*mut c_void, *mut c_void, isize) -> bool =
            std::mem::transmute(objc_msgSend as *const ());
        send(receiver, selector, value)
    }

    unsafe fn send_usize_bool(receiver: *mut c_void, selector: *mut c_void, value: usize) -> bool {
        let send: unsafe extern "C" fn(*mut c_void, *mut c_void, usize) -> bool =
            std::mem::transmute(objc_msgSend as *const ());
        send(receiver, selector, value)
    }

    unsafe fn send_void(receiver: *mut c_void, selector: *mut c_void, value: *mut c_void) {
        let send: unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void) =
            std::mem::transmute(objc_msgSend as *const ());
        send(receiver, selector, value);
    }

    unsafe fn send_void_no_args(receiver: *mut c_void, selector: *mut c_void) {
        let send: unsafe extern "C" fn(*mut c_void, *mut c_void) =
            std::mem::transmute(objc_msgSend as *const ());
        send(receiver, selector);
    }

    fn sel(name: &str) -> *mut c_void {
        unsafe { sel_registerName(cstr(name).as_ptr()) }
    }

    fn cstr(value: &str) -> CString {
        CString::new(value).expect("Objective-C selector/class names must not contain NUL")
    }
}
