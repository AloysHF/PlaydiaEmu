use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use playdia_core::{
    machine::{Machine, MachineConfig, RunStop},
    InputButtons, FB_HEIGHT, FB_WIDTH,
};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "playdia", about = "Playdia emulator (standalone/headless)")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Inspect a disc image (sector counts, XA stats).
    Inspect {
        /// Path to raw/cooked CDS-XA image.
        disc: PathBuf,
    },
    /// Run headless for N frames and print diagnostics / CRCs.
    Headless {
        disc: PathBuf,
        /// Optional 512KiB BIOS EPROM dump.
        #[arg(long)]
        bios: Option<PathBuf>,
        #[arg(long, default_value_t = 60)]
        frames: u32,
        /// Allow missing BIOS (empty EPROM + RAM idle loop).
        #[arg(long)]
        allow_placeholder_bios: bool,
        /// Force audio test tone instead of silent XA ADPCM placeholder.
        #[arg(long)]
        audio_test_tone: bool,
        /// Write save-state after run.
        #[arg(long)]
        save_state: Option<PathBuf>,
        /// Load save-state before run.
        #[arg(long)]
        load_state: Option<PathBuf>,
        /// Dump final framebuffer as raw RGB555.
        #[arg(long)]
        dump_fb: Option<PathBuf>,
    },
}

fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Inspect { disc } => {
            let m = Machine::new(MachineConfig::default());
            let mut m = m;
            m.load_disc_path(&disc).context("load disc")?;
            println!(
                "sectors={} raw={}",
                m.disc.as_ref().unwrap().total_sectors,
                m.disc.as_ref().unwrap().raw
            );
            if let Some(stats) = m.disc_stats() {
                println!(
                    "video_sectors={} audio_sectors={} other_sectors={}",
                    stats.video_sectors, stats.audio_sectors, stats.other_sectors
                );
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
            println!(
                "diag unmapped_r={} unmapped_w={} unknown_op={} notes={:?}",
                m.diag.unmapped_reads, m.diag.unmapped_writes, m.diag.unknown_opcodes, m.diag.notes
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
            let _ = (FB_WIDTH, FB_HEIGHT, InputButtons::default());
        }
    }
    Ok(())
}
