//! Parameter scanner: cache real packets once, sweep CodecParams, score vs reference.
#![allow(clippy::chunks_exact_to_as_chunks)]

use anyhow::{Context, Result};
use clap::Parser;
use playdia_core::player::DiscPlayer;
use playdia_core::video::{rgb555_to_rgb888, CodecParams, VideoDecoder};
use std::path::{Path, PathBuf};

#[derive(Parser)]
#[command(name = "playdia-scan", about = "Sweep video codec params vs a reference frame")]
struct Cli {
    disc: PathBuf,
    reference: PathBuf,
    #[arg(long, default_value_t = 300)]
    max_frames: u32,
    #[arg(long, default_value_t = 12)]
    top: usize,
    #[arg(long)]
    dump_best: Option<PathBuf>,
}

fn load_reference_gray(path: &Path) -> Result<(Vec<f32>, usize, usize)> {
    let data = std::fs::read(path).context("read reference")?;
    if path.extension().and_then(|e| e.to_str()) == Some("ppm")
        || data.starts_with(b"P6")
        || data.starts_with(b"P5")
    {
        return load_ppm_gray(&data);
    }
    if data.len() == 192 * 144 * 3 {
        let mut g = Vec::with_capacity(192 * 144);
        for px in data.chunks_exact(3) {
            g.push(0.299 * px[0] as f32 + 0.587 * px[1] as f32 + 0.114 * px[2] as f32);
        }
        return Ok((g, 192, 144));
    }
    if data.len() == 192 * 144 {
        return Ok((data.iter().map(|&b| b as f32).collect(), 192, 144));
    }
    anyhow::bail!("unsupported reference {} bytes", data.len());
}

fn load_ppm_gray(data: &[u8]) -> Result<(Vec<f32>, usize, usize)> {
    let mut i = 0;
    let mut tokens = Vec::new();
    while tokens.len() < 4 && i < data.len() {
        while i < data.len() && data[i].is_ascii_whitespace() {
            i += 1;
        }
        if i < data.len() && data[i] == b'#' {
            while i < data.len() && data[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        let start = i;
        while i < data.len() && !data[i].is_ascii_whitespace() {
            i += 1;
        }
        tokens.push(&data[start..i]);
    }
    let magic = std::str::from_utf8(tokens[0])?;
    let w: usize = std::str::from_utf8(tokens[1])?.parse()?;
    let h: usize = std::str::from_utf8(tokens[2])?.parse()?;
    while i < data.len() && data[i].is_ascii_whitespace() {
        i += 1;
    }
    let pix = &data[i..];
    let mut gray = vec![0f32; w * h];
    if magic == "P5" {
        for (k, g) in gray.iter_mut().enumerate().take((w * h).min(pix.len())) {
            *g = pix[k] as f32;
        }
    } else {
        for (k, g) in gray
            .iter_mut()
            .enumerate()
            .take((w * h).min(pix.len() / 3))
        {
            let o = k * 3;
            *g = 0.299 * pix[o] as f32 + 0.587 * pix[o + 1] as f32 + 0.114 * pix[o + 2] as f32;
        }
    }
    Ok((gray, w, h))
}

fn resize_to_192x144(src: &[f32], sw: usize, sh: usize) -> Vec<f32> {
    let mut out = vec![0f32; 192 * 144];
    for y in 0..144 {
        let sy0 = y * sh / 144;
        let sy1 = ((y + 1) * sh / 144).max(sy0 + 1);
        for x in 0..192 {
            let sx0 = x * sw / 192;
            let sx1 = ((x + 1) * sw / 192).max(sx0 + 1);
            let mut acc = 0.0;
            let mut n = 0.0;
            for yy in sy0..sy1.min(sh) {
                for xx in sx0..sx1.min(sw) {
                    acc += src[yy * sw + xx];
                    n += 1.0;
                }
            }
            out[y * 192 + x] = if n > 0.0 { acc / n } else { 0.0 };
        }
    }
    out
}

fn fb_luma_center(fb: &[u16]) -> Vec<f32> {
    let (ox, oy) = ((320 - 192) / 2, (240 - 144) / 2);
    let mut g = vec![0f32; 192 * 144];
    for y in 0..144 {
        for x in 0..192 {
            let px = fb[(oy + y) * 320 + (ox + x)];
            let (r, gr, b) = rgb555_to_rgb888(px);
            g[y * 192 + x] = 0.299 * r as f32 + 0.587 * gr as f32 + 0.114 * b as f32;
        }
    }
    g
}

fn pearson(a: &[f32], b: &[f32]) -> f32 {
    let n = a.len().min(b.len());
    if n < 2 {
        return 0.0;
    }
    let ma = a[..n].iter().sum::<f32>() / n as f32;
    let mb = b[..n].iter().sum::<f32>() / n as f32;
    let mut num = 0.0;
    let mut da = 0.0;
    let mut db = 0.0;
    for i in 0..n {
        let xa = a[i] - ma;
        let xb = b[i] - mb;
        num += xa * xb;
        da += xa * xa;
        db += xb * xb;
    }
    let den = (da * db).sqrt();
    if den < 1e-6 {
        0.0
    } else {
        num / den
    }
}

fn score_packet(pkt: &[u8], p: CodecParams, ref192: &[f32]) -> f32 {
    let mut dec = VideoDecoder::new();
    dec.params = p;
    match playdia_core::video::decode_packet(pkt, p) {
        Some((rgb, _blocks)) => {
            // blit like VideoDecoder
            let (ox, oy) = ((320 - 192) / 2, (240 - 144) / 2);
            dec.framebuffer.fill(0);
            for y in 0..144 {
                for x in 0..192 {
                    let i = (y * 192 + x) * 2;
                    let px = u16::from_le_bytes([rgb[i], rgb[i + 1]]);
                    dec.framebuffer[(oy + y) * 320 + (ox + x)] = px;
                }
            }
            let g = fb_luma_center(&dec.framebuffer);
            pearson(&g, ref192)
        }
        None => f32::MIN,
    }
}

fn collect_packets(disc: &Path, max_frames: u32) -> Result<Vec<Vec<u8>>> {
    // Walk player without caring about decode params; capture packets via a
    // thin hook: re-read stream track F1-F2 groups.
    let mut player = DiscPlayer::new();
    player.load_path(disc)?;
    // Disable decode for speed — we only need the stream. Monkey: use params
    // that fail fast? Better: parse track directly.
    let image = playdia_core::DiscImage::from_path(disc)?;
    let Some(track) = image.stream_track().cloned() else {
        anyhow::bail!("no stream track");
    };
    let mut packets = Vec::new();
    let mut acc = Vec::new();
    let mut started = false;
    // skip lead-in file 0
    let mut i = 0u32;
    while i < track.sectors && track.data[i as usize * 2352 + 16] != 1 {
        i += 1;
    }
    while i < track.sectors && (packets.len() as u32) < max_frames {
        let o = i as usize * 2352;
        i += 1;
        let sm = track.data[o + 18];
        let mk = track.data[o + 24];
        if sm & 0x04 != 0 {
            continue;
        }
        if sm & 0x08 == 0 {
            continue;
        }
        match mk {
            0xF1 => {
                started = true;
                acc.extend_from_slice(&track.data[o + 25..o + 24 + 2048]);
            }
            0xF2 => {
                if started && acc.len() >= 44 {
                    packets.push(std::mem::take(&mut acc));
                } else {
                    acc.clear();
                }
                started = false;
            }
            0xF3 => {
                acc.clear();
                started = false;
            }
            _ => {}
        }
    }
    let _ = player;
    Ok(packets)
}

fn build_grid() -> Vec<CodecParams> {
    let mut v = Vec::new();
    // Refined around first-pass winners + zigzag.
    for bs_offset in [40usize, 42, 43, 44, 45, 46, 47, 48] {
        for ac_count in [5usize, 10, 20, 30, 40, 50, 63] {
            for ac_dequant in [0u8, 1] {
                for use_eob in [false] {
                    for dc_accum in [true] {
                        for dc_scale in [8i32] {
                            for scan_order in [0u8, 1, 2] {
                                v.push(CodecParams {
                                    bs_offset,
                                    ac_count,
                                    ac_dequant,
                                    use_eob,
                                    dc_mode_accum: dc_accum,
                                    dc_scale,
                                    scan_order,
                                    ..CodecParams::default()
                                });
                            }
                        }
                    }
                }
            }
        }
    }
    v
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let (rg, rw, rh) = load_reference_gray(&cli.reference)?;
    let ref192 = resize_to_192x144(&rg, rw, rh);

    eprintln!("collecting packets...");
    let packets = collect_packets(&cli.disc, cli.max_frames)?;
    eprintln!("packets={}", packets.len());
    if packets.is_empty() {
        anyhow::bail!("no packets");
    }

    let combos = build_grid();
    eprintln!("combos={}", combos.len());

    let mut results: Vec<(f32, CodecParams, usize)> = Vec::new();
    for (ci, p) in combos.iter().enumerate() {
        let mut best = f32::MIN;
        let mut best_i = 0usize;
        // Sample up to 40 packets for speed
        let step = (packets.len() / 40).max(1);
        for (i, pkt) in packets.iter().step_by(step).enumerate() {
            let s = score_packet(pkt, *p, &ref192);
            if s > best {
                best = s;
                best_i = i;
            }
        }
        results.push((best, *p, best_i));
        if ci % 25 == 0 {
            let mx = results.iter().map(|r| r.0).fold(f32::MIN, f32::max);
            eprintln!("combo {ci}/{} best={:.4}", combos.len(), mx);
        }
    }

    results.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
    println!("TOP");
    for (score, p, idx) in results.iter().take(cli.top) {
        println!(
            "pearson={score:.4} bs_off={} ac={} deq={} eob={} dc_acc={} sc={} pkt={idx}",
            p.bs_offset, p.ac_count, p.ac_dequant, p.use_eob, p.dc_mode_accum, p.dc_scale
        );
    }

    if let Some(path) = &cli.dump_best {
        let best = results[0];
        let pkt = &packets[best.2];
        let mut dec = VideoDecoder::new();
        dec.params = best.1;
        if let Some((rgb, _)) = playdia_core::video::decode_packet(pkt, best.1) {
            let (ox, oy) = ((320 - 192) / 2, (240 - 144) / 2);
            for y in 0..144 {
                for x in 0..192 {
                    let i = (y * 192 + x) * 2;
                    let px = u16::from_le_bytes([rgb[i], rgb[i + 1]]);
                    dec.framebuffer[(oy + y) * 320 + (ox + x)] = px;
                }
            }
            dec.dump_ppm(path)?;
            println!("dumped {}", path.display());
        }
    }
    Ok(())
}
