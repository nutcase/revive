use std::error::Error;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use revive_cheat::CheatManager;
use revive_core::{CoreInstance, FrameView, SystemKind, VirtualButton};
use sdl2::audio::{AudioQueue, AudioSpecDesired};
use sdl2::event::Event;
use sdl2::keyboard::{Keycode, Mod};
use sdl2::pixels::{Color, PixelFormatEnum};
use sdl2::render::{Canvas, Texture};
use sdl2::video::Window;

const DEFAULT_SCALE: u32 = 3;

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
    let cheats = load_cheats(options.cheat_path.as_deref())?;

    println!("Loaded      : {}", rom_path.display());
    println!("System      : {}", core.system().label());
    println!("Title       : {}", core.title());
    if let Some(path) = &options.cheat_path {
        println!("Cheats      : {}", path.display());
    }
    for region in core.memory_regions() {
        println!(
            "Memory      : {} ({}, {} bytes)",
            region.id, region.label, region.len
        );
    }
    println!("State keys  : Ctrl/Cmd+0..9 save, 0..9 load");

    run_sdl_loop(core, cheats, &options)?;
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

fn load_cheats(path: Option<&Path>) -> Result<Option<CheatManager>, Box<dyn Error>> {
    let Some(path) = path else {
        return Ok(None);
    };
    if !path.exists() {
        return Err(format!("cheat file does not exist: {}", path.display()).into());
    }
    let manager = CheatManager::load_from_file(path)?;
    println!("Loaded cheats: {}", manager.entries.len());
    Ok(Some(manager))
}

fn run_sdl_loop(
    mut core: CoreInstance,
    cheats: Option<CheatManager>,
    options: &Options,
) -> Result<(), Box<dyn Error>> {
    sdl2::hint::set("SDL_DISABLE_IMMINTRIN_H", "1");
    sdl2::hint::set("SDL_MAC_CTRL_CLICK_EMULATE_RIGHT_CLICK", "0");

    let sdl = sdl2::init().map_err(sdl_error)?;
    let video = sdl.video().map_err(sdl_error)?;

    let (frame_width, frame_height) = {
        let frame = core.frame();
        (frame.width, frame.height)
    };

    let window_title = format!("Revive - {} - {}", core.system().label(), core.title());
    let window = video
        .window(
            &window_title,
            frame_width as u32 * DEFAULT_SCALE,
            frame_height as u32 * DEFAULT_SCALE,
        )
        .position_centered()
        .resizable()
        .build()
        .map_err(|err| io::Error::other(err.to_string()))?;

    let mut canvas = build_canvas(window)?;
    let texture_creator = canvas.texture_creator();
    let mut texture = texture_creator
        .create_texture_streaming(
            PixelFormatEnum::RGB24,
            frame_width as u32,
            frame_height as u32,
        )
        .map_err(|err| io::Error::other(err.to_string()))?;
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

    'running: loop {
        for event in event_pump.poll_iter() {
            match event {
                Event::Quit { .. } => break 'running,
                Event::KeyDown {
                    keycode: Some(Keycode::Escape),
                    ..
                } => break 'running,
                Event::KeyDown {
                    keycode: Some(key),
                    keymod,
                    repeat: false,
                    ..
                } => {
                    if handle_state_key(&mut core, key, keymod) {
                        continue;
                    }
                    if let Some(button) = map_key(core.system(), key) {
                        core.set_button(1, button, true);
                    }
                }
                Event::KeyUp {
                    keycode: Some(key),
                    repeat: false,
                    ..
                } => {
                    if let Some(button) = map_key(core.system(), key) {
                        core.set_button(1, button, false);
                    }
                }
                _ => {}
            }
        }

        apply_cheats(&mut core, cheats.as_ref());
        core.step_frame()?;
        apply_cheats(&mut core, cheats.as_ref());

        if let Some(queue) = audio_queue.as_mut() {
            feed_audio(queue, &mut core, &mut audio_scratch)?;
        }

        let frame = core.frame();
        if (frame.width, frame.height) != texture_size {
            texture = texture_creator
                .create_texture_streaming(
                    PixelFormatEnum::RGB24,
                    frame.width as u32,
                    frame.height as u32,
                )
                .map_err(|err| io::Error::other(err.to_string()))?;
            texture_size = (frame.width, frame.height);
            let _ = canvas.window_mut().set_size(
                frame.width as u32 * DEFAULT_SCALE,
                frame.height as u32 * DEFAULT_SCALE,
            );
        }
        present_frame(&mut canvas, &mut texture, frame)?;
        frame_clock.wait();
    }

    if let Err(err) = core.flush_persistent_save() {
        eprintln!("warning: failed to flush persistent save: {err}");
    }

    Ok(())
}

fn build_canvas(window: Window) -> Result<Canvas<Window>, Box<dyn Error>> {
    match window.into_canvas().accelerated().present_vsync().build() {
        Ok(mut canvas) => {
            canvas.set_draw_color(Color::RGB(0, 0, 0));
            Ok(canvas)
        }
        Err(err) => Err(io::Error::other(err.to_string()).into()),
    }
}

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
    let target_frames = ((spec.freq.max(8_000) as usize) / 10).clamp(1024, 8192);

    if queued_frames < target_frames {
        core.drain_audio_i16(scratch);
        if !scratch.is_empty() {
            queue
                .queue_audio(scratch)
                .map_err(|err| io::Error::other(err.to_string()))?;
        }
    }
    Ok(())
}

fn present_frame(
    canvas: &mut Canvas<Window>,
    texture: &mut Texture<'_>,
    frame: FrameView<'_>,
) -> Result<(), Box<dyn Error>> {
    let row_bytes = frame.width * 3;
    texture
        .with_lock(None, |target: &mut [u8], pitch: usize| {
            for row in 0..frame.height {
                let src_start = row * row_bytes;
                let dst_start = row * pitch;
                let src = &frame.data[src_start..src_start + row_bytes];
                let dst = &mut target[dst_start..dst_start + row_bytes];
                dst.copy_from_slice(src);
            }
        })
        .map_err(|err| io::Error::other(err.to_string()))?;

    canvas.clear();
    canvas
        .copy(texture, None, None)
        .map_err(|err| io::Error::other(err.to_string()))?;
    canvas.present();
    Ok(())
}

fn apply_cheats(core: &mut CoreInstance, cheats: Option<&CheatManager>) {
    let Some(cheats) = cheats else {
        return;
    };
    for entry in cheats.enabled_entries() {
        core.write_memory_byte(&entry.region, entry.offset as usize, entry.value);
    }
}

fn handle_state_key(core: &mut CoreInstance, key: Keycode, keymod: Mod) -> bool {
    let Some(slot) = state_slot_from_key(key) else {
        return false;
    };
    if state_save_modifier(keymod) {
        match core.save_state_to_slot(slot) {
            Ok(()) => println!("Saved state slot {slot}"),
            Err(err) => eprintln!("failed to save state slot {slot}: {err}"),
        }
    } else {
        match core.load_state_from_slot(slot) {
            Ok(()) => println!("Loaded state slot {slot}"),
            Err(err) => eprintln!("failed to load state slot {slot}: {err}"),
        }
    }
    true
}

fn state_slot_from_key(key: Keycode) -> Option<u8> {
    match key {
        Keycode::Num0 | Keycode::Kp0 => Some(0),
        Keycode::Num1 | Keycode::Kp1 => Some(1),
        Keycode::Num2 | Keycode::Kp2 => Some(2),
        Keycode::Num3 | Keycode::Kp3 => Some(3),
        Keycode::Num4 | Keycode::Kp4 => Some(4),
        Keycode::Num5 | Keycode::Kp5 => Some(5),
        Keycode::Num6 | Keycode::Kp6 => Some(6),
        Keycode::Num7 | Keycode::Kp7 => Some(7),
        Keycode::Num8 | Keycode::Kp8 => Some(8),
        Keycode::Num9 | Keycode::Kp9 => Some(9),
        _ => None,
    }
}

fn state_save_modifier(keymod: Mod) -> bool {
    keymod.intersects(Mod::LCTRLMOD | Mod::RCTRLMOD | Mod::LGUIMOD | Mod::RGUIMOD)
}

fn map_key(system: SystemKind, key: Keycode) -> Option<VirtualButton> {
    match system {
        SystemKind::Nes => map_nes_key(key),
        SystemKind::Snes => map_snes_key(key),
        SystemKind::MegaDrive => map_megadrive_key(key),
        SystemKind::Pce => map_pce_key(key),
        SystemKind::GameBoy | SystemKind::GameBoyColor => map_gameboy_key(key),
        SystemKind::GameBoyAdvance => map_gameboy_advance_key(key),
    }
}

fn map_nes_key(key: Keycode) -> Option<VirtualButton> {
    match key {
        Keycode::Up => Some(VirtualButton::Up),
        Keycode::Down => Some(VirtualButton::Down),
        Keycode::Left => Some(VirtualButton::Left),
        Keycode::Right => Some(VirtualButton::Right),
        Keycode::Z => Some(VirtualButton::A),
        Keycode::X => Some(VirtualButton::B),
        Keycode::Return => Some(VirtualButton::Start),
        Keycode::Space | Keycode::RShift | Keycode::LShift => Some(VirtualButton::Select),
        _ => None,
    }
}

fn map_snes_key(key: Keycode) -> Option<VirtualButton> {
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
        Keycode::RShift | Keycode::LShift => Some(VirtualButton::Select),
        _ => None,
    }
}

fn map_megadrive_key(key: Keycode) -> Option<VirtualButton> {
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

fn map_pce_key(key: Keycode) -> Option<VirtualButton> {
    match key {
        Keycode::Up => Some(VirtualButton::Up),
        Keycode::Down => Some(VirtualButton::Down),
        Keycode::Left => Some(VirtualButton::Left),
        Keycode::Right => Some(VirtualButton::Right),
        Keycode::Z => Some(VirtualButton::A),
        Keycode::X => Some(VirtualButton::B),
        Keycode::Return | Keycode::Space => Some(VirtualButton::Start),
        Keycode::RShift | Keycode::LShift => Some(VirtualButton::Select),
        _ => None,
    }
}

fn map_gameboy_key(key: Keycode) -> Option<VirtualButton> {
    match key {
        Keycode::Up => Some(VirtualButton::Up),
        Keycode::Down => Some(VirtualButton::Down),
        Keycode::Left => Some(VirtualButton::Left),
        Keycode::Right => Some(VirtualButton::Right),
        Keycode::X => Some(VirtualButton::A),
        Keycode::Z => Some(VirtualButton::B),
        Keycode::Return | Keycode::Space => Some(VirtualButton::Start),
        Keycode::Backspace | Keycode::RShift | Keycode::LShift => Some(VirtualButton::Select),
        _ => None,
    }
}

fn map_gameboy_advance_key(key: Keycode) -> Option<VirtualButton> {
    match key {
        Keycode::A => Some(VirtualButton::L),
        Keycode::S => Some(VirtualButton::R),
        _ => map_gameboy_key(key),
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
