use anyhow::{bail, Context, Result};
use clap::Parser;
use playdia_core::player::{DiscPlayer, PlayerStop};
use playdia_core::{InputButtons, FB_HEIGHT, FB_WIDTH};
use rodio::buffer::SamplesBuffer;
use rodio::{DeviceSinkBuilder, Player};
use std::num::NonZero;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

#[derive(Parser)]
#[command(
    name = "playdia-emu",
    about = "Playdia standalone emulator (HLE disc player)",
    version
)]
struct Cli {
    /// Path to .cue (preferred) or raw .bin/.iso
    disc: Option<PathBuf>,
    /// Window scale factor (native is 320x240)
    #[arg(short, long, default_value_t = 1)]
    scale: u32,
    /// Run in fullscreen mode
    #[arg(short, long)]
    fullscreen: bool,
    /// Target FPS for the window frontend
    #[arg(long, default_value_t = 30)]
    fps: u32,
    /// Master audio volume (0-100)
    #[arg(short, long, default_value_t = 100, value_parser = clap::value_parser!(u8).range(0..=100))]
    volume: u8,
    /// Run without opening a window
    #[arg(long)]
    headless: bool,
    /// Number of frames to run (headless defaults to 180; window 0 = until closed)
    #[arg(long)]
    frames: Option<u32>,
    /// Take a screenshot after N frames and exit (saves as PNG)
    #[arg(short = 'S', long = "screenshot", value_name = "PATH")]
    screenshot: Option<PathBuf>,
    /// Number of frames to run before taking screenshot
    #[arg(long = "screenshot-frames", default_value_t = 30)]
    screenshot_frames: u32,
    /// Dump every Nth decoded frame as PPM into this directory (headless)
    #[arg(long)]
    dump_every: Option<u32>,
    /// Directory for periodic dumps (with --dump-every)
    #[arg(long, default_value = "tmp/out")]
    dump_dir: PathBuf,
    /// Compatibility flag; native AK8000 decoding is already enabled
    #[arg(long)]
    full_decode: bool,
    /// Press a button at a host frame, e.g. --press-at 120:a (headless)
    #[arg(long = "press-at", value_parser = parse_press_at)]
    press_at: Vec<(u32, InputButtons)>,
}

fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let cli = Cli::parse();
    let Some(disc) = cli.disc.clone() else {
        bail!("provide a disc path (optional --headless)");
    };

    // Screenshot implies headless, matching spmp8000-emu / dingoo-emu.
    let headless = cli.headless || cli.screenshot.is_some();
    let frames = if cli.screenshot.is_some() && cli.frames.is_none() {
        cli.screenshot_frames
    } else {
        cli.frames.unwrap_or(if headless { 180 } else { 0 })
    };

    if headless {
        run_hle_headless(
            &disc,
            frames,
            cli.full_decode,
            cli.screenshot.as_deref(),
            cli.dump_every,
            &cli.dump_dir,
            &cli.press_at,
        )
    } else {
        run_window(&disc, frames, &cli)
    }
}

fn run_hle_headless(
    disc: &Path,
    frames: u32,
    full_decode: bool,
    screenshot: Option<&Path>,
    dump_every: Option<u32>,
    dump_dir: &Path,
    press_at: &[(u32, InputButtons)],
) -> Result<()> {
    let mut p = DiscPlayer::new();
    if full_decode {
        p.video.params.ac_dequant = 1;
    }
    p.load_path(disc).context("load disc")?;
    if let Some(n) = dump_every {
        if n > 0 {
            std::fs::create_dir_all(dump_dir).ok();
        }
    }
    let mut dumped = 0u32;
    for i in 0..frames {
        let mut held = InputButtons::default();
        for &(at, buttons) in press_at {
            if at == i {
                held = buttons;
            }
        }
        p.set_input(held);
        let previous_frames = p.video.frames_decoded;
        let stop = p.run_frame();
        if let Some(n) = dump_every {
            if n > 0
                && p.video.frames_decoded > previous_frames
                && p.video.frames_decoded.is_multiple_of(n as u64)
            {
                let path = dump_dir.join(format!("frame_{:05}.ppm", dumped));
                p.video.dump_ppm(&path)?;
                println!("wrote {}", path.display());
                dumped += 1;
            }
        }
        if stop != PlayerStop::Ok {
            log::warn!("stop={stop:?} at host_frame {}", p.frame);
            break;
        }
        if i % 30 == 0 {
            println!("host_frame={} {}", i, p.stats_line());
        }
    }
    if let Some(path) = screenshot {
        save_screenshot_png(p.framebuffer(), path)?;
        println!("wrote {}", path.display());
    }
    let audio = p.drain_audio();
    println!(
        "DONE {} audio_samples={} track_idx={} interactive={}",
        p.stats_line(),
        audio.len(),
        p.track_index,
        p.interactive.len()
    );
    if p.video.frames_decoded == 0 {
        bail!("no video frames decoded (failed={})", p.video.frames_failed);
    }
    Ok(())
}

fn run_window(disc: &Path, max_frames: u32, cli: &Cli) -> Result<()> {
    if !disc.exists() {
        bail!("disc not found: {}", disc.display());
    }

    log::info!("Loading {}", disc.display());
    let mut player = DiscPlayer::new();
    if cli.full_decode {
        player.video.params.ac_dequant = 1;
    }
    player.load_path(disc).context("failed to load disc")?;

    let scale = cli.scale.clamp(1, 8) as usize;
    let (window_width, window_height) = if cli.fullscreen {
        screen_size()
    } else {
        (FB_WIDTH * scale, FB_HEIGHT * scale)
    };
    let mut window = minifb::Window::new(
        "PlaydiaEmu",
        window_width,
        window_height,
        minifb::WindowOptions {
            resize: !cli.fullscreen,
            borderless: cli.fullscreen,
            scale_mode: minifb::ScaleMode::AspectRatioStretch,
            ..Default::default()
        },
    )
    .context("failed to create window")?;
    window.set_target_fps(cli.fps.max(1) as usize);
    if cli.fullscreen {
        window.topmost(true);
        window.set_position(0, 0);
    }

    let audio = if cli.volume == 0 {
        None
    } else {
        match DeviceSinkBuilder::open_default_sink() {
            Ok(handle) => {
                let out = Player::connect_new(handle.mixer());
                out.set_volume(f32::from(cli.volume) / 100.0);
                Some((handle, out))
            }
            Err(e) => {
                log::warn!("audio unavailable: {e}");
                None
            }
        }
    };

    let frame_dt = Duration::from_millis((1000 / cli.fps.max(1)) as u64);
    log::info!(
        "Window {}x{} fps={} volume={}  Esc=quit  Arrows/WASD  Z/J=A  X/K=B  Enter=Start  Space=Select",
        window_width,
        window_height,
        cli.fps,
        cli.volume
    );

    let mut frames = 0u32;
    while window.is_open() && !window.is_key_down(minifb::Key::Escape) {
        let t0 = Instant::now();
        player.set_input(key_buttons(&window));
        let stop = player.run_frame();

        window
            .update_with_buffer(player.framebuffer(), FB_WIDTH, FB_HEIGHT)
            .context("update window")?;

        if let Some((_handle, player_out)) = audio.as_ref() {
            let pcm = player.drain_audio();
            if !pcm.is_empty() && player_out.len() < 16 {
                let samples: Vec<f32> = pcm.iter().map(|&s| s as f32 / i16::MAX as f32).collect();
                player_out.append(SamplesBuffer::new(
                    NonZero::new(2).unwrap(),
                    NonZero::new(44100).unwrap(),
                    samples,
                ));
            }
        } else {
            let _ = player.drain_audio();
        }

        frames += 1;
        if max_frames > 0 && frames >= max_frames {
            if let Some(path) = cli.screenshot.as_deref() {
                save_screenshot_png(player.framebuffer(), path)?;
                log::info!("wrote {}", path.display());
            }
            break;
        }
        if stop == PlayerStop::EndOfDisc && frames > 8 {
            log::info!("end of disc");
            break;
        }

        let elapsed = t0.elapsed();
        if elapsed < frame_dt {
            std::thread::sleep(frame_dt - elapsed);
        }
        if frames.is_multiple_of(60) {
            log::info!("f{} {}", frames, player.stats_line());
        }
    }

    log::info!("exit {}", player.stats_line());
    Ok(())
}

fn save_screenshot_png(framebuffer: &[u32], path: &Path) -> Result<()> {
    let mut img = image::RgbaImage::new(FB_WIDTH as u32, FB_HEIGHT as u32);
    for (i, &px) in framebuffer.iter().enumerate() {
        let r = ((px >> 16) & 0xFF) as u8;
        let g = ((px >> 8) & 0xFF) as u8;
        let b = (px & 0xFF) as u8;
        let x = (i % FB_WIDTH) as u32;
        let y = (i / FB_WIDTH) as u32;
        img.put_pixel(x, y, image::Rgba([r, g, b, 0xFF]));
    }
    img.save(path).context("save screenshot")?;
    Ok(())
}

#[cfg(target_os = "windows")]
fn screen_size() -> (usize, usize) {
    #[allow(non_snake_case)]
    extern "system" {
        fn GetSystemMetrics(index: i32) -> i32;
    }
    const SM_CXSCREEN: i32 = 0;
    const SM_CYSCREEN: i32 = 1;
    unsafe {
        (
            GetSystemMetrics(SM_CXSCREEN) as usize,
            GetSystemMetrics(SM_CYSCREEN) as usize,
        )
    }
}

#[cfg(not(target_os = "windows"))]
fn screen_size() -> (usize, usize) {
    (FB_WIDTH * 4, FB_HEIGHT * 4)
}

fn key_buttons(window: &minifb::Window) -> InputButtons {
    InputButtons {
        up: window.is_key_down(minifb::Key::Up) || window.is_key_down(minifb::Key::W),
        down: window.is_key_down(minifb::Key::Down) || window.is_key_down(minifb::Key::S),
        left: window.is_key_down(minifb::Key::Left) || window.is_key_down(minifb::Key::A),
        right: window.is_key_down(minifb::Key::Right) || window.is_key_down(minifb::Key::D),
        a: window.is_key_down(minifb::Key::Z) || window.is_key_down(minifb::Key::J),
        b: window.is_key_down(minifb::Key::X) || window.is_key_down(minifb::Key::K),
        start: window.is_key_down(minifb::Key::Enter),
        select: window.is_key_down(minifb::Key::Space),
    }
}

fn parse_press_at(value: &str) -> Result<(u32, InputButtons), String> {
    let (frame, name) = value
        .split_once(':')
        .ok_or_else(|| "expected FRAME:BUTTON".to_owned())?;
    let frame = frame
        .parse::<u32>()
        .map_err(|_| "frame must be a non-negative integer".to_owned())?;
    let mut buttons = InputButtons::default();
    match name.to_ascii_lowercase().as_str() {
        "up" => buttons.up = true,
        "down" => buttons.down = true,
        "left" => buttons.left = true,
        "right" => buttons.right = true,
        "a" => buttons.a = true,
        "b" => buttons.b = true,
        "start" => buttons.start = true,
        _ => return Err("button must be up, down, left, right, a, b, or start".to_owned()),
    }
    Ok((frame, buttons))
}
