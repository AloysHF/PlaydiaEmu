use anyhow::{Context, Result};
use clap::Parser;
use playdia_core::content::DiscImage;
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "playdia-inspect",
    about = "Inspect Playdia CDS-XA CUE/BIN images"
)]
struct Cli {
    disc: PathBuf,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let disc = DiscImage::from_path(&cli.disc).context("read disc")?;
    println!("kind={:?}", disc.kind);
    println!("total_sectors={}", disc.total_sectors);
    println!("crc32={:08x}", disc.crc);
    for t in &disc.tracks {
        println!(
            "track{} sectors={} mode2={} bytes={}",
            t.number,
            t.sectors,
            t.mode2,
            t.data.len()
        );
    }
    if let Some(t) = disc.stream_track() {
        let mut f1 = 0u32;
        let mut f2 = 0u32;
        let mut f3 = 0u32;
        let mut aud = 0u32;
        for i in 0..t.sectors.min(5000) {
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
