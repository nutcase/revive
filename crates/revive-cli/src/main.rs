use std::error::Error;
use std::io;
use std::path::{Path, PathBuf};

mod audio;
mod cheat_panel;
mod egui_input;
mod events;
mod frame_clock;
mod hud;
mod input;
mod render;
mod session;
mod state;
mod wgpu_game;
mod window;

use audio::{feed_audio, open_audio_output};
use cheat_panel::{CheatPanel, MemorySnapshot};
use egui_input::EguiInput;
use events::{process_sdl_events, EventLoopAction};
use frame_clock::FrameClock;
use hud::HudToast;
use input::{release_keyboard_input, sync_keyboard_input, InputState};
use render::{RenderState, UiRenderData};
use revive_cheat::CheatManager;
use revive_core::{CoreInstance, SystemKind, ROM_EXTENSIONS};
use session::{print_session_banner, CheatPaths};
use window::bring_window_to_front;

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

    let core = CoreInstance::load_rom_with_audio(&rom_path, options.system, !options.no_audio)?;
    let cheat_paths = CheatPaths::resolve(options.cheat_path.as_deref(), core.system(), &rom_path);
    let cheats = cheat_paths.load(options.cheat_path.is_some())?;

    print_session_banner(&core, &rom_path, cheat_paths.active());
    run_sdl_loop(core, cheats, cheat_paths.active(), &options)?;
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

fn run_sdl_loop(
    mut core: CoreInstance,
    mut cheats: CheatManager,
    cheat_path: &Path,
    options: &Options,
) -> Result<(), Box<dyn Error>> {
    sdl3::hint::set("SDL_DISABLE_IMMINTRIN_H", "1");
    sdl3::hint::set("SDL_MAC_CTRL_CLICK_EMULATE_RIGHT_CLICK", "0");

    let sdl = sdl3::init().map_err(sdl_error)?;
    let mut video = sdl.video().map_err(sdl_error)?;

    let (frame_width, frame_height) = {
        let frame = core.frame();
        (frame.width, frame.height)
    };

    let window_title = format!("Revive - {} - {}", core.system().label(), core.title());
    let (initial_w, initial_h) = RenderState::initial_window_size(frame_width, frame_height);
    let mut window_builder = video.window(&window_title, initial_w, initial_h);
    window_builder
        .position_centered()
        .resizable()
        .high_pixel_density();
    #[cfg(target_os = "macos")]
    window_builder.metal_view();
    let mut window = window_builder
        .build()
        .map_err(|err| io::Error::other(err.to_string()))?;

    bring_window_to_front(&mut window);
    let mut egui_input = EguiInput::new(&window);
    let egui_ctx = egui_input.context();
    let text_input = video.text_input();
    let mut text_input_active = false;
    text_input.stop(&window);

    let mut render_state = RenderState::new(
        &window,
        frame_width,
        frame_height,
        PANEL_WIDTH_DEFAULT as u32,
    )?;
    render_state.resize_window_for_panel(&mut window, false);

    let audio_output = if options.no_audio {
        None
    } else {
        Some(open_audio_output(&sdl, &mut core)?)
    };
    let audio_output = audio_output;
    let mut audio_scratch = Vec::new();
    let mut event_pump = sdl.event_pump().map_err(sdl_error)?;
    let mut frame_clock = FrameClock::new(core.system());
    let mut input_state = InputState::default();
    let mut cheat_panel = CheatPanel::new();
    let mut hud_toast = HudToast::default();
    let mut prev_panel_visible = cheat_panel.is_visible();
    let input_debug = std::env::var_os("REVIVE_INPUT_DEBUG").is_some();
    let mut front_retry_frames = 12u8;

    'running: loop {
        let should_enable_text_input = cheat_panel.is_visible();
        if should_enable_text_input != text_input_active {
            if should_enable_text_input {
                text_input.start(&window);
            } else {
                text_input.stop(&window);
            }
            text_input_active = should_enable_text_input;
        }

        if matches!(
            process_sdl_events(
                &mut event_pump,
                &video,
                &mut egui_input,
                &egui_ctx,
                &mut core,
                &mut cheat_panel,
                &mut input_state,
                &mut hud_toast,
                input_debug,
            ),
            EventLoopAction::Exit
        ) {
            break 'running;
        }

        if cheat_panel.is_visible() != prev_panel_visible {
            render_state.resize_window_for_panel(&mut window, cheat_panel.is_visible());
            prev_panel_visible = cheat_panel.is_visible();
        }

        if cheat_panel.is_visible() && egui_ctx.egui_wants_keyboard_input() {
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

        if let Some(output) = audio_output.as_ref() {
            feed_audio(output, &mut core, &mut audio_scratch);
        } else {
            core.drain_audio_i16(&mut audio_scratch);
        }

        render_state.upload_core_frame(&mut core, &mut window, cheat_panel.is_visible());

        let draw_ui = cheat_panel.is_visible() || hud_toast.is_visible();
        if draw_ui {
            let ctx = egui_input.begin_frame(&window);
            let mut pending_writes = Vec::new();
            if cheat_panel.is_visible() {
                let live_memory = MemorySnapshot::capture(&core);
                #[allow(deprecated)]
                let panel_resp = egui::SidePanel::right("cheat_panel")
                    .resizable(true)
                    .min_width(PANEL_WIDTH_MIN)
                    .default_width(PANEL_WIDTH_DEFAULT)
                    .show(&ctx, |ui| {
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
                let actual_w = (panel_resp.response.rect.width() * ctx.pixels_per_point()) as u32;
                if actual_w != render_state.panel_width_px() {
                    render_state.set_panel_width_px(actual_w);
                    render_state.resize_window_for_panel(&mut window, true);
                }
            }
            hud_toast.draw(&ctx);

            let full_output = egui_input.end_frame(&mut video);
            let primitives = egui_input.tessellate(&full_output);
            render_state.present_frame(
                &window,
                cheat_panel.is_visible(),
                Some(UiRenderData {
                    textures_delta: &full_output.textures_delta,
                    primitives: &primitives,
                    pixels_per_point: ctx.pixels_per_point(),
                }),
            )?;

            for write in pending_writes {
                core.write_memory_byte(&write.region, write.offset, write.value);
            }
        } else {
            render_state.present_frame(&window, false, None)?;
        }

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

fn sdl_error(message: sdl3::Error) -> io::Error {
    io::Error::other(message.to_string())
}
