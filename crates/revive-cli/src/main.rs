use std::error::Error;
use std::io;
use std::path::{Path, PathBuf};

mod audio;
mod cheat_panel;
mod frame_clock;
mod gl_game;
mod hud;
mod input;
mod state;
mod window;

use audio::{feed_audio, open_audio_queue};
use cheat_panel::{CheatPanel, MemorySnapshot};
use egui_sdl2_gl::gl;
use egui_sdl2_gl::{DpiScaling, ShaderVersion};
use frame_clock::FrameClock;
use gl_game::GlGameRenderer;
use hud::HudToast;
use input::{
    button_label, keycode_button, release_keyboard_input, sync_keyboard_input, InputState,
};
use revive_cheat::CheatManager;
use revive_core::{CoreInstance, SystemKind, ROM_EXTENSIONS};
use sdl2::event::{Event, WindowEvent};
use sdl2::keyboard::Keycode;
use sdl2::video::{GLProfile, SwapInterval};
use state::{handle_state_key, state_key_help};
use window::bring_window_to_front;

const DEFAULT_SCALE: u32 = 3;
const PANEL_WIDTH_DEFAULT: f32 = 420.0;
const PANEL_WIDTH_MIN: f32 = 300.0;

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
    println!("State keys  : {}", state_key_help());
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
        "  revive [rom] [--system nes|snes|sg1000|sms|megadrive|pce|gb|gbc|gba] [--cheats file.json] [--no-audio]"
    );
    println!(
        "  revive run [rom] [--system nes|snes|sg1000|sms|megadrive|pce|gb|gbc|gba] [--cheats file.json] [--no-audio]"
    );
    println!("  revive --select");
    println!();
    println!("If no ROM path is provided, a local file selection dialog opens.");
    println!("Supported ROM extensions: .{}", ROM_EXTENSIONS.join(", ."));
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
        .add_filter("ROM files", ROM_EXTENSIONS)
        .add_filter(SystemKind::Nes.label(), SystemKind::Nes.dialog_extensions())
        .add_filter(
            SystemKind::Snes.label(),
            SystemKind::Snes.dialog_extensions(),
        )
        .add_filter(
            SystemKind::Sg1000.label(),
            SystemKind::Sg1000.dialog_extensions(),
        )
        .add_filter(
            SystemKind::MasterSystem.label(),
            SystemKind::MasterSystem.dialog_extensions(),
        )
        .add_filter(
            SystemKind::MegaDrive.label(),
            SystemKind::MegaDrive.dialog_extensions(),
        )
        .add_filter(SystemKind::Pce.label(), SystemKind::Pce.dialog_extensions())
        .add_filter("Game Boy", &["gb", "gbc"])
        .add_filter(
            SystemKind::GameBoyAdvance.label(),
            SystemKind::GameBoyAdvance.dialog_extensions(),
        )
        .pick_file()
}

fn default_cheat_path(system: SystemKind, rom_path: &Path) -> PathBuf {
    PathBuf::from("cheats")
        .join(system.storage_dir())
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

fn apply_cheats(core: &mut CoreInstance, cheats: &CheatManager) {
    for entry in cheats.enabled_entries() {
        core.write_memory_byte(&entry.region, entry.offset as usize, entry.value);
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
