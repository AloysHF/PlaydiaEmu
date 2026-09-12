use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use playdia_core::{
    machine::{Machine, MachineConfig, RunStop},
    player::{DiscPlayer, PlayerStop},
    DiscKind, InputButtons, FB_HEIGHT, FB_WIDTH,
};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "playdia", about = "Playdia emulator (HLE player + LLE shell)")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Inspect a CUE/BIN or raw disc image.
    Inspect { disc: PathBuf },
    /// HLE disc player: stream Track 2 video/audio without BIOS.
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
        /// Use AC decode (experimental). Default is DC-only reconstruction.
        #[arg(long)]
        full_decode: bool,
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
        Cmd::Inspect { disc } => {
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
        }
        Cmd::Play {
            disc,
            frames,
            dump_ppm,
            dump_every,
            dump_dir,
            full_decode,
        } => {
            let mut p = DiscPlayer::new();
            if full_decode {
                p.video.params.ac_dequant = 1;
                p.video.params.use_eob = false;
            }
            p.load_path(&disc).context("load disc")?;
            if let Some(dir) = dump_every.map(|_| dump_dir.clone()) {
                std::fs::create_dir_all(&dir).ok();
            }
            let mut dumped = 0u32;
            for i in 0..frames {
                let stop = p.run_frame();
                if let Some(n) = dump_every {
                    if n > 0
                        && p.video.frames_decoded > 0
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
        }
        Cmd::Headless {
            disc,
            bios,
            frames,
            allow_placeholder_bios,
            audio_test_tone,
            save_state,
            load_state,
            dump_fb,
        } => {
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
        }
    }
    Ok(())
}
