use anyhow::{bail, Context, Result};
use clap::Parser;
use playdia_core::cd::{XaDemux, XaPacket};
use playdia_core::content::{parse_raw_sector, DiscImage};
use playdia_core::video::ak8000;
use playdia_core::video::rgb555_to_rgb888;
use playdia_core::video::structure::{video_fragment, VIDEO_PACKET_CAP};
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Parser)]
#[command(about = "Decode or validate AK8000 pictures directly from a disc")]
struct Cli {
    input: PathBuf,
    /// One-based picture packet index, including interactive F2 packets.
    #[arg(long, default_value_t = 1, value_parser = clap::value_parser!(u64).range(1..))]
    packet: u64,
    /// Write the selected native 248x216 picture as PPM.
    #[arg(long, required_unless_present = "check_all")]
    output: Option<PathBuf>,
    /// Validate every picture without rendering or following scene commands.
    #[arg(long, conflicts_with = "output")]
    check_all: bool,
    /// Input is an assembled picture packet instead of a disc image.
    #[arg(long)]
    assembled: bool,
}

fn dump(data: &[u8], output: &Path) -> Result<()> {
    let rgb = ak8000::decode(data).map_err(|e| anyhow::anyhow!("decode failed: {e:?}"))?;
    let mut file = std::io::BufWriter::new(std::fs::File::create(output)?);
    write!(file, "P6\n{} {}\n255\n", ak8000::WIDTH, ak8000::HEIGHT)?;
    for bytes in rgb.as_chunks::<2>().0 {
        let (r, g, b) = rgb555_to_rgb888(u16::from_le_bytes(*bytes));
        file.write_all(&[r, g, b])?;
    }
    file.flush()?;
    println!(
        "wrote {} ({}x{})",
        output.display(),
        ak8000::WIDTH,
        ak8000::HEIGHT
    );
    Ok(())
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    if cli.assembled {
        let data = std::fs::read(&cli.input)?;
        if let Some(output) = &cli.output {
            return dump(&data, output);
        }
        ak8000::validate(&data).map_err(|e| anyhow::anyhow!("decode failed: {e:?}"))?;
        println!("packets=1 valid=1 failed=0");
        return Ok(());
    }
    let disc = DiscImage::from_path(&cli.input).context("read disc")?;
    let track = disc.stream_track().context("disc has no stream track")?;
    let mut demux = XaDemux::new();
    let mut buffer = Vec::new();
    let mut overflow = false;
    let (mut count, mut valid, mut failed) = (0u64, 0u64, 0u64);
    for (lba, raw) in track.data.as_chunks::<2352>().0.iter().enumerate() {
        let Some(sector) = parse_raw_sector(raw) else {
            continue;
        };
        let packet = demux.push(&sector);
        match &packet {
            XaPacket::Video { data, .. }
            | XaPacket::FrameEnd { data, .. }
            | XaPacket::Interactive { data, .. } => {
                if matches!(packet, XaPacket::Video { .. }) || !buffer.is_empty() {
                    let fragment = video_fragment(data);
                    if buffer.len() + fragment.len() > VIDEO_PACKET_CAP {
                        overflow = true;
                    } else if !overflow {
                        buffer.extend_from_slice(fragment);
                    }
                }
                if !matches!(packet, XaPacket::Video { .. }) && !buffer.is_empty() {
                    count += 1;
                    if cli.check_all {
                        if !overflow && ak8000::validate(&buffer).is_ok() {
                            valid += 1;
                        } else {
                            failed += 1;
                            if failed <= 20 {
                                println!("failed packet={count} lba={lba} overflow={overflow} error={:?}", ak8000::validate(&buffer).err());
                            }
                        }
                    } else if count == cli.packet {
                        if overflow {
                            bail!("picture packet {count} exceeds size limit");
                        }
                        return dump(
                            &buffer,
                            cli.output.as_deref().context("output path required")?,
                        );
                    }
                    buffer.clear();
                    overflow = false;
                }
            }
            XaPacket::SceneReset { .. } => {
                buffer.clear();
                overflow = false;
            }
            _ => {}
        }
    }
    if !cli.check_all {
        bail!("packet {} not found; scanned {count}", cli.packet);
    }
    println!("packets={count} valid={valid} failed={failed}");
    if failed != 0 {
        bail!("{failed} pictures failed validation");
    }
    Ok(())
}
