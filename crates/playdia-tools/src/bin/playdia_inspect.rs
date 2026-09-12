use anyhow::{Context, Result};
use clap::Parser;
use playdia_core::{content::DiscImage, Machine, MachineConfig};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "playdia-inspect", about = "Inspect Playdia CDS-XA images")]
struct Cli {
    disc: PathBuf,
    /// Print first N video/audio sector LBAs.
    #[arg(long, default_value_t = 8)]
    sample: usize,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let disc = DiscImage::from_path(&cli.disc).context("read disc")?;
    println!("size_bytes={}", disc.data.len());
    println!("raw_mode={}", disc.raw);
    println!("total_sectors={}", disc.total_sectors);
    println!("crc32={:08x}", disc.crc);

    let mut m = Machine::new(MachineConfig::default());
    m.load_disc_path(&cli.disc)?;
    if let Some(stats) = m.disc_stats() {
        println!("video_sectors={}", stats.video_sectors);
        println!("audio_sectors={}", stats.audio_sectors);
        println!("other_sectors={}", stats.other_sectors);
        println!("last_video_lba={:?}", stats.last_video_lba);
        println!("last_audio_lba={:?}", stats.last_audio_lba);
    }

    // Sample subheaders
    let mut shown_v = 0;
    let mut shown_a = 0;
    for lba in 0..disc.total_sectors {
        let Some(sec) = disc.read_sector(lba) else {
            continue;
        };
        if sec.is_video() && shown_v < cli.sample {
            println!(
                "video lba={lba} ch={} submode=0x{:02x} coding=0x{:02x} bytes={}",
                sec.channel_id,
                sec.submode,
                sec.coding,
                sec.data.len()
            );
            shown_v += 1;
        }
        if sec.is_audio() && shown_a < cli.sample {
            println!(
                "audio lba={lba} ch={} submode=0x{:02x} coding=0x{:02x} bytes={}",
                sec.channel_id,
                sec.submode,
                sec.coding,
                sec.data.len()
            );
            shown_a += 1;
        }
        if shown_v >= cli.sample && shown_a >= cli.sample {
            break;
        }
    }
    Ok(())
}
