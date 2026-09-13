//! HLE player tests against synthetic MODE2 discs (no copyrighted content).

use playdia_core::content::{DiscImage, Track};
use playdia_core::player::{DiscPlayer, PlayerStop};
use playdia_core::{DiscKind, InputButtons};

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

fn interactive_disc(command: u8) -> DiscImage {
    const SECTOR: usize = 2352;
    let mut stream = vec![0u8; SECTOR * 220];
    for sector in stream.as_chunks_mut::<SECTOR>().0 {
        sector[15] = 2;
        sector[16] = 1;
        sector[18] = 0x08;
    }
    let cmd = &mut stream[SECTOR..SECTOR * 2];
    cmd[18] = 0x09;
    cmd[24] = 0xF2;
    cmd[25] = command;
    for button in 0..7 {
        let off = 27 + button * 4;
        cmd[off + 1] = 4;
        cmd[off + 2] = if button == 5 { 30 } else { 20 };
    }
    DiscImage {
        cue_path: None,
        tracks: vec![
            Track {
                number: 1,
                path: Default::default(),
                data: vec![0; SECTOR * 10],
                sectors: 10,
                mode2: true,
            },
            Track {
                number: 2,
                path: Default::default(),
                data: stream,
                sectors: 220,
                mode2: true,
            },
        ],
        single: None,
        total_sectors: 230,
        crc: 0,
        kind: DiscKind::CueMultiTrack,
    }
}

#[test]
fn interactive_jump_uses_disc_lba_and_discards_prefetch() {
    let mut player = DiscPlayer::new();
    player.disc = Some(interactive_disc(0x40));
    assert_eq!(player.run_frame(), PlayerStop::Ok);
    assert_eq!(player.interactive[0].0, 11);
    assert_eq!(player.demux.interactive_cmds, 1);
    assert_eq!(player.track_index, 160);
    assert_eq!(player.run_frame(), PlayerStop::Ok);
    assert_eq!(player.track_index, 168);
}

#[test]
fn interactive_choice_pauses_and_seeks_on_button_edge() {
    let mut player = DiscPlayer::new();
    player.disc = Some(interactive_disc(0x44));
    player.run_frame();
    assert!(player.is_waiting_for_input());
    assert_eq!(player.track_index, 2);
    player.run_frame();
    assert_eq!(player.track_index, 2);
    player.set_input(InputButtons {
        a: true,
        ..Default::default()
    });
    player.run_frame();
    assert!(!player.is_waiting_for_input());
    assert_eq!(player.track_index, 178);
}

#[test]
fn interactive_video_is_finished_and_presented_before_control() {
    for command in [0x44, 0x80] {
        let mut disc = interactive_disc(command);
        let raw = &mut disc.tracks[1].data;
        let f1 = &mut raw[24..2072];
        f1.fill(0x55);
        f1[0..5].copy_from_slice(&[0xF1, 0, 0x80, 4, 8]);
        f1[5..37].fill(8);
        f1[37..40].copy_from_slice(&[0, 0x80, 0x24]);
        raw[2352 + 24 + 0x23..2352 + 2072].fill(0x55);
        let mut player = DiscPlayer::new();
        player.disc = Some(disc);
        // Keep the synthetic preview small enough for one F1/F2 pair.
        player.video.params.ac_count = 0;
        player.run_frame();
        assert_eq!(player.video.last_packet_len, 2047 + 2013);
        assert!(player.video.acc.is_empty());
        assert_eq!(player.video.frames_decoded, 1);
        assert!(!player.video.present_next());
        assert_eq!(player.is_waiting_for_input(), command == 0x44);
        assert_eq!(player.demux.interactive_cmds, 1);
    }
}
