mod audio;
mod bus;
mod cartridge;
mod cpu;
mod dma;
mod gameboy;
mod joypad;
mod model;
mod ppu;
mod serial;
mod timer;

use clap::Parser;
use gameboy::GameBoy;
use joypad::Button;
use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::pixels::PixelFormatEnum;
use sdl2::rect::Rect;
use std::time::{Duration, Instant};

/// 🦀 FerrisBoy — Game Boy (DMG) and Game Boy Color (CGB) emulator
#[derive(Parser, Debug)]
#[command(author, version, about)]
struct Args {
    /// Path to the ROM file (.gb / .gbc)
    rom: String,

    /// Display scale factor (default: 3)
    #[arg(short, long, default_value_t = 3)]
    scale: u32,

    /// Force DMG (original Game Boy) mode even for CGB ROMs
    #[arg(long)]
    force_dmg: bool,

    /// Skip boot ROM animation
    #[arg(long)]
    skip_boot: bool,

    /// Path to boot ROM (DMG: 256 bytes, CGB: ~2304 bytes)
    #[arg(long)]
    boot_rom: Option<String>,
}

const GB_WIDTH:  u32 = 160;
const GB_HEIGHT: u32 = 144;
const TARGET_FPS: u64 = 60;
const FRAME_DURATION: Duration = Duration::from_nanos(1_000_000_000 / TARGET_FPS);

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let args = Args::parse();

    let rom_data = std::fs::read(&args.rom).unwrap_or_else(|e| {
        eprintln!("Failed to read ROM '{}': {}", args.rom, e);
        std::process::exit(1);
    });

    let boot_rom = args.boot_rom.as_ref().map(|p| {
        std::fs::read(p).unwrap_or_else(|e| { eprintln!("Boot ROM error: {}", e); std::process::exit(1); })
    });

    let mut gb = GameBoy::new(rom_data, boot_rom, args.skip_boot, args.force_dmg);

    log::info!("🦀 FerrisBoy starting");
    log::info!("  Title : {}", gb.cart_title());
    log::info!("  Model : {}", gb.model_name());
    log::info!("  MBC   : {}", gb.mbc_type());

    // ── SDL2 ────────────────────────────────────────────────────────────────
    let sdl = sdl2::init().expect("SDL2 init failed");
    let video = sdl.video().expect("SDL2 video failed");

    let win_w = GB_WIDTH  * args.scale;
    let win_h = GB_HEIGHT * args.scale;
    let title = format!("🦀 FerrisBoy — {} [{}]", gb.cart_title(), gb.model_name());

    let window = video.window(&title, win_w, win_h)
        .position_centered().build().expect("Window failed");

    let mut canvas = window.into_canvas()
        .accelerated().present_vsync().build().expect("Canvas failed");

    let tc = canvas.texture_creator();
    let mut texture = tc.create_texture_streaming(PixelFormatEnum::RGB24, GB_WIDTH, GB_HEIGHT)
        .expect("Texture failed");

    let mut events = sdl.event_pump().expect("Event pump failed");
    let dest = Rect::new(0, 0, win_w, win_h);

    let mut frame_start = Instant::now();
    let mut fps_timer   = Instant::now();
    let mut frame_count = 0u32;

    'main: loop {
        for event in events.poll_iter() {
            match event {
                Event::Quit { .. } => break 'main,
                Event::KeyDown { keycode: Some(kc), .. } => {
                    if kc == Keycode::Escape { break 'main; }
                    gb.key_down(map_key(kc));
                }
                Event::KeyUp { keycode: Some(kc), .. } => gb.key_up(map_key(kc)),
                _ => {}
            }
        }

        gb.run_frame();

        texture.update(None, gb.framebuffer(), (GB_WIDTH * 3) as usize)
            .expect("Texture update failed");
        canvas.clear();
        canvas.copy(&texture, None, Some(dest)).expect("Render failed");
        canvas.present();

        frame_count += 1;
        if fps_timer.elapsed() >= Duration::from_secs(1) {
            let t = format!("🦀 FerrisBoy — {} [{}] | {} fps",
                gb.cart_title(), gb.model_name(), frame_count);
            canvas.window_mut().set_title(&t).ok();
            frame_count = 0;
            fps_timer = Instant::now();
        }

        let elapsed = frame_start.elapsed();
        if elapsed < FRAME_DURATION { std::thread::sleep(FRAME_DURATION - elapsed); }
        frame_start = Instant::now();
    }
}

fn map_key(kc: Keycode) -> Button {
    match kc {
        Keycode::Z                    => Button::A,
        Keycode::X                    => Button::B,
        Keycode::Return               => Button::Start,
        Keycode::RShift | Keycode::Space => Button::Select,
        Keycode::Up                   => Button::Up,
        Keycode::Down                 => Button::Down,
        Keycode::Left                 => Button::Left,
        Keycode::Right                => Button::Right,
        _                             => Button::None,
    }
}
