//! Windowed Playdia HLE player (minifb + rodio).
//!
//! Same core as `playdia play`, but with an interactive window for viewing.

use anyhow::{bail, Context, Result};
use clap::Parser;
use playdia_core::player::{DiscPlayer, PlayerStop};
use playdia_core::InputButtons;
use rodio::buffer::SamplesBuffer;
use rodio::{DeviceSinkBuilder, Player};
use std::num::NonZero;
use std::time::{Duration, Instant};

#[derive(Parser, Debug)]
#[command(
    name = "playdia-emu",
    about = "Playdia standalone emulator window (HLE disc player)"
)]
struct Cli {
    /// Path to .cue (preferred) or raw .bin/.iso
    disc: std::path::PathBuf,
    /// Window scale factor (native is 320x240)
    #[arg(long, default_value_t = 3)]
    scale: u32,
    /// Target FPS
    #[arg(long, default_value_t = 30)]
    fps: u32,
    /// Experimental AC decode instead of DC-only reconstruction
    #[arg(long)]
    full_decode: bool,
    /// Mute host audio
    #[arg(long)]
    mute: bool,
    /// Quit after N host frames (0 = until window closed)
    #[arg(long, default_value_t = 0)]
    frames: u32,
    /// Save PPM when quitting via --frames
    #[arg(long)]
    dump_ppm: Option<std::path::PathBuf>,
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

fn rgb555_to_u32(fb: &[u16]) -> Vec<u32> {
    fb.iter()
        .map(|p| {
            let r = (p & 0x1F) as u32;
            let g = ((p >> 5) & 0x1F) as u32;
            let b = ((p >> 10) & 0x1F) as u32;
            (r << 19) | (g << 11) | (b << 3) | 0x0008_0808
        })
        .collect()
}

fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let cli = Cli::parse();
    if !cli.disc.exists() {
        bail!("disc not found: {}", cli.disc.display());
    }

    log::info!("Loading {}", cli.disc.display());
    let mut player = DiscPlayer::new();
    if cli.full_decode {
        player.video.params.ac_dequant = 1;
        player.video.params.use_eob = true;
    }
    player
        .load_path(&cli.disc)
        .context("failed to load disc")?;

    let scale = cli.scale.clamp(1, 8) as usize;
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
    window.set_target_fps(cli.fps.max(1) as usize);

    let audio = if cli.mute {
        None
    } else {
        match DeviceSinkBuilder::open_default_sink() {
            Ok(handle) => {
                let player = Player::connect_new(handle.mixer());
                Some((handle, player))
            }
            Err(e) => {
                log::warn!("audio unavailable: {e}");
                None
            }
        }
    };

    let frame_dt = Duration::from_millis((1000 / cli.fps.max(1)) as u64);
    log::info!(
        "Window {}x{} fps={}  Esc=quit  Arrows/WASD  Z/J=A  X/K=B  Enter=Start  Space=Select",
        playdia_core::FB_WIDTH * scale,
        playdia_core::FB_HEIGHT * scale,
        cli.fps
    );

    let mut frames = 0u32;
    while window.is_open() && !window.is_key_down(minifb::Key::Escape) {
        let t0 = Instant::now();
        player.set_input(key_buttons(&window));
        let stop = player.run_frame();

        let buf = rgb555_to_u32(player.framebuffer());
        window
            .update_with_buffer(&buf, playdia_core::FB_WIDTH, playdia_core::FB_HEIGHT)
            .context("update window")?;

        if let Some((_handle, player_out)) = audio.as_ref() {
            let pcm = player.drain_audio();
            if !pcm.is_empty() && player_out.len() < 16 {
                let samples: Vec<f32> = pcm
                    .iter()
                    .map(|&s| s as f32 / i16::MAX as f32)
                    .collect();
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
        if cli.frames > 0 && frames >= cli.frames {
            if let Some(path) = &cli.dump_ppm {
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
