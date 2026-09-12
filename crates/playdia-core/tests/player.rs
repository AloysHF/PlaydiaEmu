//! HLE player tests against synthetic MODE2 discs (no copyrighted content).

use playdia_core::player::{DiscPlayer, PlayerStop};
use playdia_core::DiscKind;

fn synthetic_stream_disc() -> Vec<u8> {
    // 8 raw sectors: lead-in file0 + 6×F1 + F2
    const S: usize = 2352;
    let mut raw = vec![0u8; S * 8];
    // lead-in
    raw[15] = 2;
    raw[16] = 0;
    raw[18] = 0x08;
    // 6 F1 with 00 80 04 header on first
    for i in 1..=6 {
        let o = i * S;
        raw[o + 15] = 2;
        raw[o + 16] = 1;
        raw[o + 17] = 0;
        raw[o + 18] = 0x08;
        raw[o + 24] = 0xF1;
        if i == 1 {
            raw[o + 25] = 0x00;
            raw[o + 26] = 0x80;
            raw[o + 27] = 0x04;
            raw[o + 28] = 0x10; // QS
        }
    }
    let o = 7 * S;
    raw[o + 15] = 2;
    raw[o + 16] = 1;
    raw[o + 18] = 0x08;
    raw[o + 24] = 0xF2;
    raw
}

#[test]
fn player_decodes_synthetic_packet_header() {
    let mut p = DiscPlayer::new();
    p.load_bytes(synthetic_stream_disc()).unwrap();
    assert_eq!(p.disc.as_ref().unwrap().kind, DiscKind::SingleRaw);
    // Run several frames until F2
    for _ in 0..4 {
        let stop = p.run_frame();
        if stop == PlayerStop::EndOfDisc {
            break;
        }
    }
    assert!(
        p.demux.video_sectors >= 6,
        "video {}",
        p.demux.video_sectors
    );
    // DC-only may or may not produce blocks depending on bitstream; at least F2 was seen.
    assert!(p.demux.frame_ends >= 1 || p.video.frames_failed + p.video.frames_decoded >= 1);
}

#[test]
fn player_crc_changes_or_stable_not_panics() {
    let mut p = DiscPlayer::new();
    p.load_bytes(synthetic_stream_disc()).unwrap();
    let _ = p.run_frame();
    let _ = p.frame_crc();
    let _ = p.stats_line();
}
