use anyhow::{bail, Context, Result};
use clap::Parser;
use playdia_core::machine::{Machine, MachineConfig, RunStop};
use playdia_core::player::{DiscPlayer, PlayerStop};
use playdia_core::InputButtons;
use rodio::buffer::SamplesBuffer;
use rodio::{DeviceSinkBuilder, Player};
use std::num::NonZero;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

#[derive(Parser)]
#[command(
    name = "playdia-emu",
    about = "Playdia standalone emulator (HLE disc player + LLE shell)"
)]
struct Cli {
    /// Path to .cue (preferred) or raw .bin/.iso
    disc: Option<PathBuf>,
    /// Window scale factor (native is 320x240)
    #[arg(long, default_value_t = 3)]
    scale: u32,
    /// Target FPS for the window frontend
    #[arg(long, default_value_t = 30)]
    fps: u32,
    /// Mute host audio (window mode)
    #[arg(long)]
    mute: bool,
    /// Run without opening a window
    #[arg(long)]
    headless: bool,
    /// Number of frames to run (headless defaults: HLE 180, LLE 60; window 0 = until closed)
    #[arg(long)]
    frames: Option<u32>,
    /// Save final framebuffer as PPM (HLE window/HLE headless)
    #[arg(long)]
    dump_ppm: Option<PathBuf>,
    /// Dump every Nth decoded frame as PPM into this directory (HLE headless)
    #[arg(long)]
    dump_every: Option<u32>,
    /// Directory for periodic dumps (with --dump-every)
    #[arg(long, default_value = "tmp/out")]
    dump_dir: PathBuf,
    /// Compatibility flag; native AK8000 decoding is already enabled
    #[arg(long)]
    full_decode: bool,
    /// Press a button at a host frame, e.g. --press-at 120:a (HLE headless)
    #[arg(long = "press-at", value_parser = parse_press_at)]
    press_at: Vec<(u32, InputButtons)>,
    /// Use the LLE machine (SH-1 + bus) instead of the HLE disc player
    #[arg(long)]
    lle: bool,
    /// Path to a 512 KiB BIOS EPROM (LLE)
    #[arg(long)]
    bios: Option<PathBuf>,
    /// Allow empty placeholder BIOS for LLE experiments
    #[arg(long)]
    allow_placeholder_bios: bool,
    /// Emit a test tone instead of disc audio (LLE)
    #[arg(long)]
    audio_test_tone: bool,
    /// Write save state after the run (LLE)
    #[arg(long)]
    save_state: Option<PathBuf>,
    /// Load save state before the run (LLE)
    #[arg(long)]
    load_state: Option<PathBuf>,
    /// Dump raw 320x240 XRGB8888 framebuffer words (LLE)
    #[arg(long)]
    dump_fb: Option<PathBuf>,
}

fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let cli = Cli::parse();
    let Some(disc) = cli.disc.clone() else {
        bail!("provide a disc path (optional --headless / --lle)");
    };

    let use_lle = cli.lle
        || cli.bios.is_some()
        || cli.allow_placeholder_bios
        || cli.audio_test_tone
        || cli.save_state.is_some()
        || cli.load_state.is_some()
        || cli.dump_fb.is_some();
    let headless = cli.headless || use_lle;
    let frames = cli.frames.unwrap_or(if use_lle {
        60
    } else if headless {
        180
    } else {
        0
    });

    if use_lle {
        run_lle(&disc, frames, &cli)
    } else if headless {
        run_hle_headless(
            &disc,
            frames,
            cli.full_decode,
            cli.dump_ppm.as_ref(),
            cli.dump_every,
            &cli.dump_dir,
            &cli.press_at,
        )
    } else {
        run_window(
            &disc,
            cli.scale,
            cli.fps,
            cli.full_decode,
            cli.mute,
            frames,
            cli.dump_ppm.as_ref(),
        )
    }
}

fn run_hle_headless(
    disc: &Path,
    frames: u32,
    full_decode: bool,
    dump_ppm: Option<&PathBuf>,
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
    if let Some(path) = dump_ppm {
        p.video.dump_ppm(path)?;
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

fn run_lle(disc: &Path, frames: u32, cli: &Cli) -> Result<()> {
    let cfg = MachineConfig {
        allow_placeholder_bios: cli.allow_placeholder_bios,
        enable_xa_stream: true,
        audio_test_tone: cli.audio_test_tone,
    };
    let mut m = Machine::new(cfg);
    if let Some(bios) = cli.bios.as_deref() {
        m.load_bios_path(bios).context("load bios")?;
    } else if !cli.allow_placeholder_bios {
        bail!("--bios is required unless --allow-placeholder-bios is set");
    }
    m.load_disc_path(disc).context("load disc")?;
    m.reset();
    if let Some(path) = cli.load_state.as_deref() {
        let bytes = std::fs::read(path).context("read state")?;
        m.load_state(&bytes).context("load state")?;
    }
    for _ in 0..frames {
        let stop = m.run_frame();
        if stop != RunStop::Ok {
            log::warn!("stop={stop:?} at frame {}", m.frame);
            break;
        }
    }
    let fb_crc = {
        let bytes: Vec<u8> = m
            .framebuffer()
            .iter()
            .flat_map(|p| p.to_le_bytes())
            .collect();
        playdia_core::state::crc32(&bytes)
    };
    let audio = m.drain_audio();
    println!(
        "frames={} fb_crc={fb_crc:08x} audio_samples={} cpu_pc={:08x} stopped={:?}",
        m.frame,
        audio.len(),
        m.cpu.pc,
        m.cpu.stopped
    );
    if let Some(path) = cli.dump_fb.as_deref() {
        let bytes: Vec<u8> = m
            .framebuffer()
            .iter()
            .flat_map(|p| p.to_le_bytes())
            .collect();
        std::fs::write(path, bytes)?;
    }
    if let Some(path) = cli.save_state.as_deref() {
        std::fs::write(path, m.save_state())?;
    }
    Ok(())
}

fn run_window(
    disc: &Path,
    scale: u32,
    fps: u32,
    full_decode: bool,
    mute: bool,
    max_frames: u32,
    dump_ppm: Option<&PathBuf>,
) -> Result<()> {
    if !disc.exists() {
        bail!("disc not found: {}", disc.display());
    }

    log::info!("Loading {}", disc.display());
    let mut player = DiscPlayer::new();
    if full_decode {
        player.video.params.ac_dequant = 1;
    }
    player.load_path(disc).context("failed to load disc")?;

    let scale = scale.clamp(1, 8) as usize;
    let mut window = minifb::Window::new(
        "PlaydiaEmu",
        playdia_core::FB_WIDTH * scale,
        playdia_core::FB_HEIGHT * scale,
        minifb::WindowOptions {
            resize: true,
            scale_mode: minifb::ScaleMode::AspectRatioStretch,
            ..Default::default()
        },
    )
    .context("failed to create window")?;
    window.set_target_fps(fps.max(1) as usize);

    let audio = if mute {
        None
    } else {
        match DeviceSinkBuilder::open_default_sink() {
            Ok(handle) => {
                let out = Player::connect_new(handle.mixer());
                Some((handle, out))
            }
            Err(e) => {
                log::warn!("audio unavailable: {e}");
                None
            }
        }
    };

    let frame_dt = Duration::from_millis((1000 / fps.max(1)) as u64);
    log::info!(
        "Window {}x{} fps={}  Esc=quit  Arrows/WASD  Z/J=A  X/K=B  Enter=Start  Space=Select",
        playdia_core::FB_WIDTH * scale,
        playdia_core::FB_HEIGHT * scale,
        fps
    );

    let mut frames = 0u32;
    while window.is_open() && !window.is_key_down(minifb::Key::Escape) {
        let t0 = Instant::now();
        player.set_input(key_buttons(&window));
        let stop = player.run_frame();

        window
            .update_with_buffer(
                player.framebuffer(),
                playdia_core::FB_WIDTH,
                playdia_core::FB_HEIGHT,
            )
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
            if let Some(path) = dump_ppm {
                player.video.dump_ppm(path)?;
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
