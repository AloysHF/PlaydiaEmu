use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use playdia_core::player::{DiscPlayer, PlayerStop};
use playdia_core::{
    machine::{Machine, MachineConfig, RunStop},
    DiscKind, InputButtons, FB_HEIGHT, FB_WIDTH,
};
use rodio::buffer::SamplesBuffer;
use rodio::{DeviceSinkBuilder, Player};
use std::num::NonZero;
use std::path::PathBuf;
use std::time::{Duration, Instant};

#[derive(Parser)]
#[command(
    name = "playdia-emu",
    about = "Playdia standalone emulator (HLE disc player + LLE shell)"
)]
struct Cli {
    /// Open a window and play this disc (used when no subcommand is given)
    disc: Option<PathBuf>,
    /// Window scale factor (native is 320x240)
    #[arg(long, default_value_t = 3)]
    scale: u32,
    /// Target FPS for the window frontend
    #[arg(long, default_value_t = 30)]
    fps: u32,
    /// Apply experimental quantization to the approximate AC coefficients
    #[arg(long)]
    full_decode: bool,
    /// Mute host audio (window mode)
    #[arg(long)]
    mute: bool,
    /// Quit window mode after N host frames (0 = until closed)
    #[arg(long, default_value_t = 0)]
    frames: u32,
    /// Save PPM when quitting window mode via --frames
    #[arg(long)]
    dump_ppm: Option<PathBuf>,
    #[command(subcommand)]
    cmd: Option<Cmd>,
}

#[derive(Subcommand)]
enum Cmd {
    /// Inspect a CUE/BIN or raw disc image.
    Inspect { disc: PathBuf },
    /// HLE disc player: stream Track 2 video/audio without BIOS (headless).
    Play {
        /// Path to .cue (preferred) or raw .bin/.iso
        disc: PathBuf,
        #[arg(long, default_value_t = 180)]
        frames: u32,
        /// Dump final framebuffer as PPM after the run.
        #[arg(long)]
        dump_ppm: Option<PathBuf>,
        /// Dump every Nth decoded frame as PPM into this directory.
        #[arg(long)]
        dump_every: Option<u32>,
        /// Dump directory for periodic frames (with --dump-every).
        #[arg(long, default_value = "tmp/out")]
        dump_dir: PathBuf,
        /// Apply experimental quantization to the approximate AC coefficients.
        #[arg(long)]
        full_decode: bool,
        /// Press a button at a host frame, e.g. --press-at 120:a.
        #[arg(long = "press-at", value_parser = parse_press_at)]
        press_at: Vec<(u32, InputButtons)>,
    },
    /// LLE-oriented headless (SH-1 + bus). Prefer `play` for disc playback.
    Headless {
        disc: PathBuf,
        #[arg(long)]
        bios: Option<PathBuf>,
        #[arg(long, default_value_t = 60)]
        frames: u32,
        #[arg(long)]
        allow_placeholder_bios: bool,
        #[arg(long)]
        audio_test_tone: bool,
        #[arg(long)]
        save_state: Option<PathBuf>,
        #[arg(long)]
        load_state: Option<PathBuf>,
        #[arg(long)]
        dump_fb: Option<PathBuf>,
    },
}

fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let cli = Cli::parse();
    match cli.cmd {
        None => {
            let Some(disc) = cli.disc else {
                bail!("provide a disc path, or a subcommand (inspect / play / headless)");
            };
            run_window(
                &disc,
                cli.scale,
                cli.fps,
                cli.full_decode,
                cli.mute,
                cli.frames,
                cli.dump_ppm.as_ref(),
            )
        }
        Some(Cmd::Inspect { disc }) => {
            let d = playdia_core::DiscImage::from_path(&disc).context("load disc")?;
            println!("kind={:?}", d.kind);
            println!("total_sectors={}", d.total_sectors);
            println!("crc32={:08x}", d.crc);
            for t in &d.tracks {
                println!(
                    "track{} sectors={} mode2={} bytes={}",
                    t.number,
                    t.sectors,
                    t.mode2,
                    t.data.len()
                );
            }
            // Sample first track sectors for ISO + stream track markers.
            if let Some(t) = d.data_track() {
                if t.sectors > 16 {
                    let sec = &t.data[16 * 2352..17 * 2352];
                    let sig = &sec[25..30];
                    println!(
                        "pvd_sig={:?} volume={:?}",
                        String::from_utf8_lossy(sig),
                        String::from_utf8_lossy(&sec[40..72])
                    );
                }
            }
            if let Some(t) = d.stream_track() {
                let mut f1 = 0u32;
                let mut f2 = 0u32;
                let mut f3 = 0u32;
                let mut aud = 0u32;
                for i in 0..t.sectors.min(8000) {
                    let o = i as usize * 2352;
                    if o + 25 >= t.data.len() {
                        break;
                    }
                    let sm = t.data[o + 18];
                    let mk = t.data[o + 24];
                    if sm & 0x04 != 0 {
                        aud += 1;
                    } else if sm & 0x08 != 0 {
                        match mk {
                            0xF1 => f1 += 1,
                            0xF2 => f2 += 1,
                            0xF3 => f3 += 1,
                            _ => {}
                        }
                    }
                }
                println!(
                    "stream_track={} f1={} f2={} f3={} audio={}",
                    t.number, f1, f2, f3, aud
                );
            }
            Ok(())
        }
        Some(Cmd::Play {
            disc,
            frames,
            dump_ppm,
            dump_every,
            dump_dir,
            full_decode,
            press_at,
        }) => {
            let mut p = DiscPlayer::new();
            if full_decode {
                p.video.params.ac_dequant = 1;
            }
            p.load_path(&disc).context("load disc")?;
            if let Some(dir) = dump_every.map(|_| dump_dir.clone()) {
                std::fs::create_dir_all(&dir).ok();
            }
            let mut dumped = 0u32;
            for i in 0..frames {
                let mut held = InputButtons::default();
                for &(at, buttons) in &press_at {
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
                p.video.dump_ppm(&path)?;
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
        Some(Cmd::Headless {
            disc,
            bios,
            frames,
            allow_placeholder_bios,
            audio_test_tone,
            save_state,
            load_state,
            dump_fb,
        }) => {
            let cfg = MachineConfig {
                allow_placeholder_bios,
                enable_xa_stream: true,
                audio_test_tone,
            };
            let mut m = Machine::new(cfg);
            if let Some(bios) = bios {
                m.load_bios_path(&bios).context("load bios")?;
            } else if !allow_placeholder_bios {
                bail!("--bios is required unless --allow-placeholder-bios is set");
            }
            m.load_disc_path(&disc).context("load disc")?;
            m.reset();
            if let Some(path) = load_state {
                let bytes = std::fs::read(&path).context("read state")?;
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
            if let Some(path) = dump_fb {
                let bytes: Vec<u8> = m
                    .framebuffer()
                    .iter()
                    .flat_map(|p| p.to_le_bytes())
                    .collect();
                std::fs::write(path, bytes)?;
            }
            if let Some(path) = save_state {
                std::fs::write(path, m.save_state())?;
            }
            let _ = (
                FB_WIDTH,
                FB_HEIGHT,
                InputButtons::default(),
                DiscKind::SingleRaw,
            );
            Ok(())
        }
    }
}

fn run_window(
    disc: &std::path::Path,
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

        let buf = rgb555_to_u32(player.framebuffer());
        window
            .update_with_buffer(&buf, playdia_core::FB_WIDTH, playdia_core::FB_HEIGHT)
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

fn rgb555_to_u32(fb: &[u16]) -> Vec<u32> {
    fb.iter()
        .map(|p| {
            let r = (p & 0x1F) as u32;
            let g = ((p >> 5) & 0x1F) as u32;
            let b = ((p >> 10) & 0x1F) as u32;
            let r8 = (r << 3) | (r >> 2);
            let g8 = (g << 3) | (g >> 2);
            let b8 = (b << 3) | (b >> 2);
            (r8 << 16) | (g8 << 8) | b8
        })
        .collect()
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
