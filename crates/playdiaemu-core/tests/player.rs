//! HLE player tests against synthetic MODE2 discs (no copyrighted content).
mod common;

use playdiaemu_core::content::{DiscImage, Track};
use playdiaemu_core::player::{DiscPlayer, PlayerStop};
use playdiaemu_core::{DiscKind, InputButtons, FB_HEIGHT, FB_WIDTH};

fn synthetic_stream_disc() -> Vec<u8> {
    // 8 raw sectors: lead-in file0 + 6脳F1 + F2
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
        cmd[off + 2] = if button == 0 { 6 } else { 4 };
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

fn noninteractive_stream_disc() -> DiscImage {
    let mut disc = interactive_disc(0x44);
    disc.tracks[1].data[2352 + 24] = 0;
    disc
}

#[test]
fn player_advances_75_cd_sectors_per_second() {
    let mut player = DiscPlayer::new();
    player.disc = Some(noninteractive_stream_disc());

    for expected in [2, 5, 7, 10] {
        assert_eq!(player.run_frame(), PlayerStop::Ok);
        assert_eq!(player.track_index, expected);
    }
    for _ in 4..30 {
        assert_eq!(player.run_frame(), PlayerStop::Ok);
    }

    assert_eq!(player.track_index, 75);
}

#[test]
fn save_state_preserves_cd_sector_phase() {
    let disc = noninteractive_stream_disc();
    let mut first = DiscPlayer::new();
    first.disc = Some(disc.clone());
    assert_eq!(first.run_frame(), PlayerStop::Ok);
    assert_eq!(first.track_index, 2);

    let state = first.save_state();
    let mut restored = DiscPlayer::new();
    restored.disc = Some(disc);
    restored.load_state(&state).unwrap();
    assert_eq!(restored.run_frame(), PlayerStop::Ok);
    assert_eq!(restored.track_index, 5);
}

#[test]
fn interactive_jump_uses_disc_lba_and_discards_prefetch() {
    let mut player = DiscPlayer::new();
    player.disc = Some(interactive_disc(0x40));
    assert_eq!(player.run_frame(), PlayerStop::Ok);
    assert_eq!(player.interactive[0].0, 11);
    assert_eq!(player.demux.interactive_cmds, 1);
    // destinations[0] uses S=4 F=6 -> LBA 180; track_index = 180 - base(10) = 170.
    assert_eq!(player.track_index, 170);
    assert_eq!(player.run_frame(), PlayerStop::Ok);
    assert_eq!(player.track_index, 173);
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
    // B1/default slot is Start (destinations[0]: S=4 F=6 → LBA 180).
    player.set_input(InputButtons {
        start: true,
        ..Default::default()
    });
    player.run_frame();
    assert!(!player.is_waiting_for_input());
    assert_eq!(player.track_index, 173);
}

#[test]
fn interactive_video_is_finished_and_presented_before_control() {
    for command in [0x44, 0x80] {
        let mut disc = interactive_disc(command);
        let raw = &mut disc.tracks[1].data;
        let f1 = &mut raw[24..2072];
        let picture = common::picture(|row, _, bits| {
            if row < 2 {
                common::escape(bits, 0, 0);
            }
            common::put(bits, 1, 2);
        });
        assert!(picture.len() > 2047 && picture.len() < 2047 + 2013);
        f1[0] = 0xF1;
        f1[1..].copy_from_slice(&picture[..2047]);
        let tail = &mut raw[2352 + 24 + 0x23..2352 + 2072];
        tail.fill(0xFF);
        tail[..picture.len() - 2047].copy_from_slice(&picture[2047..]);
        let mut player = DiscPlayer::new();
        player.disc = Some(disc);
        player.run_frame();
        assert_eq!(player.video.last_packet_len, 2047 + 2013);
        assert!(player.video.acc.is_empty());
        assert_eq!(player.video.frames_decoded, 1);
        assert!(!player.video.present_next());
        assert_eq!(player.is_waiting_for_input(), command == 0x44);
        assert_eq!(player.demux.interactive_cmds, 1);
    }
}

#[test]
fn b_choices_reach_complete_pictures_without_replaying_the_prompt() {
    for single_track in [false, true] {
        let mut disc = interactive_disc(0x44);
        for (index, target, level) in [(0, [0, 2, 8], 0), (30, [0, 3, 1], 8), (70, [0, 3, 1], 16)] {
            let picture = common::picture(|row, _, bits| {
                if row < 2 {
                    common::escape(bits, 0, level);
                }
                common::put(bits, 1, 2);
            });
            let raw = &mut disc.tracks[1].data;
            let f1 = &mut raw[index * 2352 + 24..index * 2352 + 2072];
            f1[0] = 0xF1;
            f1[1..].copy_from_slice(&picture[..2047]);
            let f2 = &mut raw[(index + 1) * 2352..(index + 2) * 2352];
            f2[18] = 0x09;
            f2[24] = 0xF2;
            f2[25] = 0x44;
            // Every other slot returns to the title; B (slot 6) advances.
            for slot in 0..7 {
                let off = 27 + slot * 4;
                f2[off..off + 3].copy_from_slice(&[0, 2, 2]);
            }
            f2[27 + 6 * 4..27 + 6 * 4 + 3].copy_from_slice(&target);
            let tail = &mut f2[24 + 0x23..2072];
            tail.fill(0xFF);
            tail[..picture.len() - 2047].copy_from_slice(&picture[2047..]);
        }
        let mut player = DiscPlayer::new();
        if single_track {
            player
                .load_bytes(
                    disc.tracks
                        .into_iter()
                        .flat_map(|track| track.data)
                        .collect(),
                )
                .unwrap();
            player.sector_cursor = 10;
        } else {
            player.disc = Some(disc);
        }
        assert_eq!(player.run_frame(), PlayerStop::Ok);
        let title_crc = player.frame_crc();
        // Slot 6 (B) carries the advance target in this fixture.
        let b = InputButtons {
            b: true,
            ..Default::default()
        };
        player.set_input(b);
        assert_eq!(player.run_frame(), PlayerStop::Ok);
        assert_eq!(player.interactive.last().unwrap().0, 41);
        assert!(player.is_waiting_for_input());
        assert_ne!(player.frame_crc(), title_crc);
        let menu_crc = player.frame_crc();
        // Level-triggered selection: release so the open menu is not auto-advanced.
        player.set_input(InputButtons::default());
        player.run_frame();
        assert_eq!(player.frame_crc(), menu_crc);
        assert_eq!(player.interactive.len(), 2);
        player.set_input(b);
        assert_eq!(player.run_frame(), PlayerStop::Ok);
        assert_eq!(player.interactive.last().unwrap().0, 81);
        assert_ne!(player.frame_crc(), menu_crc);
        assert_eq!(player.video.frames_decoded, 3);
        assert_eq!(player.video.frames_failed, 0);
    }
}

#[test]
fn horizontal_choices_use_reference_button_slots() {
    // Reference slots: Left=B4(3), Right=B5(5→index 4).
    for (buttons, slot) in [
        (
            InputButtons {
                left: true,
                ..Default::default()
            },
            3,
        ),
        (
            InputButtons {
                right: true,
                ..Default::default()
            },
            4,
        ),
    ] {
        let mut disc = interactive_disc(0x44);
        for index in 0..7 {
            disc.tracks[1].data[2352 + 29 + index * 4] = 4 + index as u8;
        }
        let mut player = DiscPlayer::new();
        player.disc = Some(disc);
        player.run_frame();
        player.set_input(buttons);
        player.run_frame();
        assert_eq!(player.track_index, 160 + slot * 5 + 3);
        assert!(!player.is_waiting_for_input());
    }
}

#[test]
fn choice_times_out_to_default_destination() {
    let mut player = DiscPlayer::new();
    player.disc = Some(interactive_disc(0x44));
    player.run_frame();
    assert!(player.is_waiting_for_input());
    for _ in 0..300 {
        player.set_input(InputButtons::default());
        player.run_frame();
    }
    assert!(!player.is_waiting_for_input());
    // Default timeout target is destinations[0] (S=4 F=6 → LBA 180 → index 170 + 3).
    assert_eq!(player.track_index, 173);
}

#[test]
fn choice_held_before_menu_still_selects() {
    let mut player = DiscPlayer::new();
    player.disc = Some(interactive_disc(0x44));
    // Player is already holding A when the prompt opens.
    player.set_input(InputButtons {
        a: true,
        ..Default::default()
    });
    player.run_frame();
    assert!(player.is_waiting_for_input());
    player.set_input(InputButtons {
        a: true,
        ..Default::default()
    });
    player.run_frame();
    assert!(
        !player.is_waiting_for_input(),
        "held A (slot 5) must select without a new edge"
    );
}

#[test]
fn f2_80_timeout_destination_is_used_before_default() {
    let mut player = DiscPlayer::new();
    let mut disc = interactive_disc(0x80);
    // F2 80 slot0: S=4 F=4 → LBA 170 (different from default choice slot0 LBA 180).
    disc.tracks[1].data[2352 + 27] = 0;
    disc.tracks[1].data[2352 + 28] = 4;
    disc.tracks[1].data[2352 + 29] = 4;
    disc.tracks[1].data[2352 + 30] = 0;
    player.disc = Some(disc);
    player.run_frame();
    assert!(!player.is_waiting_for_input());
    assert_eq!(player.demux.interactive_cmds, 1);

    // Next F2 at the same stream position is a choice; timeout_dest must stick.
    let mut choice = interactive_disc(0x44);
    choice.tracks[1].data[2352 + 24] = 0xF2;
    choice.tracks[1].data[2352 + 25] = 0x44;
    choice.tracks[1].data[2352 + 18] = 0x09;
    player.sector_cursor = 0;
    player.track_index = 0;
    player.demux = playdiaemu_core::cd::XaDemux::new();
    player.interactive.clear();
    // Keep timeout_dest from the 0x80 command above; waiting is already None.
    player.disc = Some(choice);
    player.run_frame();
    assert!(player.is_waiting_for_input());
    for _ in 0..300 {
        player.set_input(InputButtons::default());
        player.run_frame();
    }
    assert!(!player.is_waiting_for_input());
    // Timeout dest S=4 F=4 → LBA 170 → track 160 + 2 = 162 (not default 172).
    assert_eq!(player.track_index, 162);
}

#[test]
fn player_save_state_roundtrips_playback_position() {
    let mut p1 = DiscPlayer::new();
    p1.load_bytes(synthetic_stream_disc()).unwrap();
    for _ in 0..8 {
        let _ = p1.run_frame();
    }
    // Put non-zero pixels so length/shift bugs cannot hide behind zeros.
    p1.video.framebuffer[0] = 0x00A1B2C3;
    p1.video.framebuffer[FB_WIDTH * FB_HEIGHT - 1] = 0x0011FE07;
    let fb0 = p1.framebuffer().to_vec();
    let stats0 = p1.stats_line();
    let blob = p1.save_state();

    let mut p2 = DiscPlayer::new();
    p2.load_bytes(synthetic_stream_disc()).unwrap();
    p2.load_state(&blob).unwrap();
    assert_eq!(p2.frame, p1.frame);
    assert_eq!(p2.sector_cursor, p1.sector_cursor);
    assert_eq!(p2.framebuffer(), fb0.as_slice());
    assert_eq!(p2.video.frames_decoded, p1.video.frames_decoded);
    assert_eq!(p2.stats_line(), stats0);

    // Wrong content must fail without mutating the player.
    let mut p3 = DiscPlayer::new();
    let mut other = synthetic_stream_disc();
    other[0] = 0xFF;
    // Ensure size stays valid for from_bytes
    p3.load_bytes(other).unwrap();
    let before = p3.save_state();
    assert!(p3.load_state(&blob).is_err());
    assert_eq!(p3.save_state(), before);
}
