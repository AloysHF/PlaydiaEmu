use anyhow::{Context, Result};
use clap::Parser;
use playdia_core::content::{DiscImage, Track};
use playdia_core::video::parse_packet_header;
use playdia_core::video::structure::{
    is_video_padding, scan_picture_rows, video_fragment, VIDEO_PACKET_CAP,
};
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
    /// Rank video packets with low byte entropy or long 55/AA byte runs.
    #[arg(long)]
    video_candidates: bool,
    /// Scan MSB-first row sequences and padding-anchored picture terminators.
    #[arg(long)]
    video_rows: bool,
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
        if cli.video_headers || cli.video_candidates || cli.video_rows {
            inspect_video_headers(t, cli.video_candidates, cli.video_rows);
        }
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct VideoCandidate {
    index: u64,
    first_lba: usize,
    length: usize,
    raw_byte38: u8,
    entropy: f64,
    alternating_byte_run: usize,
}

fn body_entropy(data: &[u8]) -> f64 {
    let mut counts = [0usize; 256];
    for &byte in data {
        counts[byte as usize] += 1;
    }
    let total = data.len() as f64;
    counts
        .iter()
        .filter(|&&count| count != 0)
        .map(|&count| {
            let p = count as f64 / total;
            -p * p.log2()
        })
        .sum()
}

fn longest_alternating_byte_run(data: &[u8]) -> usize {
    let mut previous = 0;
    let mut current = 0;
    let mut longest = 0;
    for &byte in data {
        current = if (byte == 0x55 || byte == 0xAA) && byte == previous {
            current + 1
        } else if byte == 0x55 || byte == 0xAA {
            1
        } else {
            0
        };
        longest = longest.max(current);
        previous = byte;
    }
    longest
}

fn inspect_video_headers(track: &Track, show_candidates: bool, show_rows: bool) {
    let mut packet = Vec::new();
    let mut first_lba = 0usize;
    let mut overflow = false;
    let mut packets = 0u64;
    let mut valid = 0u64;
    let mut identical_quantizers = 0u64;
    let mut codes = BTreeMap::<u8, u64>::new();
    let mut lengths = BTreeMap::<usize, u64>::new();
    let mut lowest_entropy = Vec::<VideoCandidate>::new();
    let mut longest_runs = Vec::<VideoCandidate>::new();
    let mut row_sequences = 0u64;
    let mut ambiguous_sequences = 0u64;
    let mut f2_terminators = 0u64;
    let mut padding_sectors = 0u64;
    let mut padding_with_video = 0u64;
    for (lba, raw) in track.data.as_chunks::<2352>().0.iter().enumerate() {
        if raw[15] != 2 || raw[16] != 1 || raw[17] != 0 || raw[18] & 0x08 == 0 {
            continue;
        }
        match raw[24] {
            0xF1 if packet.len() + 2047 <= VIDEO_PACKET_CAP && !overflow => {
                if packet.is_empty() {
                    first_lba = lba;
                }
                packet.extend_from_slice(video_fragment(&raw[24..24 + 2048]));
            }
            0xF1 => overflow = true,
            0xF2 if raw[18] & 1 == 0 => {
                if !packet.is_empty() || overflow {
                    packets += 1;
                    let f1_bits = packet.len() * 8;
                    let tail = video_fragment(&raw[24..24 + 2048]);
                    if packet.len() + tail.len() <= VIDEO_PACKET_CAP && !overflow {
                        packet.extend_from_slice(tail);
                    } else {
                        overflow = true;
                    }
                    if !overflow {
                        if show_rows {
                            if let Some(rows) = scan_picture_rows(&packet) {
                                row_sequences += 1;
                                ambiguous_sequences += u64::from(rows.ambiguous_rows != 0);
                                f2_terminators += u64::from(rows.terminator_bit + 14 > f1_bits);
                            }
                        }
                        let len = packet.iter().rposition(|&b| b != 0xFF).map_or(0, |p| p + 1);
                        *lengths.entry(len).or_default() += 1;
                        if let Some(header) = parse_packet_header(&packet) {
                            valid += 1;
                            identical_quantizers +=
                                u64::from(header.quant_luma == header.quant_chroma);
                            *codes.entry(header.raw_row_prefix[2]).or_default() += 1;
                            if show_candidates && len > 40 + 256 {
                                let body = &packet[40..len];
                                let candidate = VideoCandidate {
                                    index: packets,
                                    first_lba,
                                    length: len,
                                    raw_byte38: header.raw_row_prefix[2],
                                    entropy: body_entropy(body),
                                    alternating_byte_run: longest_alternating_byte_run(body),
                                };
                                lowest_entropy.push(candidate);
                                lowest_entropy.sort_by(|a, b| a.entropy.total_cmp(&b.entropy));
                                lowest_entropy.truncate(8);
                                longest_runs.push(candidate);
                                longest_runs.sort_by(|a, b| {
                                    b.alternating_byte_run.cmp(&a.alternating_byte_run)
                                });
                                longest_runs.truncate(8);
                            }
                        }
                    }
                    packet.clear();
                    overflow = false;
                }
            }
            0xF3 if is_video_padding(&raw[24..24 + 2048]) => {
                padding_sectors += 1;
                padding_with_video += u64::from(!packet.is_empty());
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
    if show_rows {
        println!("video_row_sequences={row_sequences} ambiguous_sequences={ambiguous_sequences} terminators_in_f2={f2_terminators}");
        println!("video_padding_sectors={padding_sectors} padding_with_pending_video={padding_with_video}");
        println!(
            "Row markers are structural candidates; entropy and original pixels are not validated."
        );
    }
    let mut common_codes: Vec<_> = codes.into_iter().collect();
    common_codes.sort_by_key(|&(code, count)| (std::cmp::Reverse(count), code));
    for (code, count) in common_codes.into_iter().take(10) {
        println!("video_raw_byte38={code:02x} packets={count}");
    }
    let mut common_lengths: Vec<_> = lengths.into_iter().collect();
    common_lengths.sort_by_key(|&(length, count)| (std::cmp::Reverse(count), length));
    for (length, count) in common_lengths.into_iter().take(10) {
        println!("video_packet_bytes={length} packets={count}");
    }
    if show_candidates {
        for candidate in lowest_entropy {
            println!(
                "low_entropy_packet={} track_lba={} bytes={} raw_byte38={:02x} entropy={:.3} alternating_run={}",
                candidate.index,
                candidate.first_lba,
                candidate.length,
                candidate.raw_byte38,
                candidate.entropy,
                candidate.alternating_byte_run
            );
        }
        for candidate in longest_runs {
            println!(
                "long_alternating_run_packet={} track_lba={} bytes={} raw_byte38={:02x} entropy={:.3} alternating_run={}",
                candidate.index,
                candidate.first_lba,
                candidate.length,
                candidate.raw_byte38,
                candidate.entropy,
                candidate.alternating_byte_run
            );
        }
    }
}
