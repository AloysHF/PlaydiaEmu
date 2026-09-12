use anyhow::{Context, Result};
use clap::Parser;
use playdia_core::content::{DiscImage, Track};
use playdia_core::video::parse_packet_header;
use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "playdia-inspect",
    about = "Inspect Playdia CDS-XA CUE/BIN images"
)]
struct Cli {
    disc: PathBuf,
    /// Report assembled video packet headers and length frequencies.
    #[arg(long)]
    video_headers: bool,
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
        if cli.video_headers {
            inspect_video_headers(t);
        }
    }
    Ok(())
}

fn inspect_video_headers(track: &Track) {
    let mut packet = Vec::new();
    let mut overflow = false;
    let mut packets = 0u64;
    let mut valid = 0u64;
    let mut identical_quantizers = 0u64;
    let mut codes = BTreeMap::<u8, u64>::new();
    let mut lengths = BTreeMap::<usize, u64>::new();
    for raw in track.data.as_chunks::<2352>().0 {
        if raw[15] != 2 || raw[16] != 1 || raw[17] != 0 || raw[18] & 0x08 == 0 {
            continue;
        }
        match raw[24] {
            0xF1 if packet.len() + 2047 <= 256 * 1024 => {
                packet.extend_from_slice(&raw[25..24 + 2048]);
            }
            0xF1 => overflow = true,
            0xF2 if raw[18] & 1 == 0 => {
                if !packet.is_empty() || overflow {
                    packets += 1;
                    if !overflow {
                        let len = packet.iter().rposition(|&b| b != 0xFF).map_or(0, |p| p + 1);
                        *lengths.entry(len).or_default() += 1;
                        if let Some(header) = parse_packet_header(&packet) {
                            valid += 1;
                            identical_quantizers +=
                                u64::from(header.quant_luma == header.quant_chroma);
                            *codes.entry(header.segment_code).or_default() += 1;
                        }
                    }
                    packet.clear();
                    overflow = false;
                }
            }
            0xF3 => {
                packet.clear();
                overflow = false;
            }
            _ => {}
        }
    }
    println!(
        "video_packets={} valid_headers={} matching_quantizers={}",
        packets, valid, identical_quantizers
    );
    let mut common_codes: Vec<_> = codes.into_iter().collect();
    common_codes.sort_by_key(|&(code, count)| (std::cmp::Reverse(count), code));
    for (code, count) in common_codes.into_iter().take(10) {
        println!("video_segment_code={code:02x} packets={count}");
    }
    let mut common_lengths: Vec<_> = lengths.into_iter().collect();
    common_lengths.sort_by_key(|&(length, count)| (std::cmp::Reverse(count), length));
    for (length, count) in common_lengths.into_iter().take(10) {
        println!("video_packet_bytes={length} packets={count}");
    }
}
