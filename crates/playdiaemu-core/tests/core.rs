//! Core tests: content, state, audio, and video.

use playdiaemu_core::content::{DiscImage, RAW_SECTOR};
use playdiaemu_core::state::{crc32, decode_state, encode_state, ContentIdentity};
use std::io::Write;

#[test]
fn disc_size_rejects_odd() {
    assert!(DiscImage::from_bytes(vec![0u8; 100]).is_err());
}

#[test]
fn disc_accepts_raw_and_cooked() {
    let raw = vec![0u8; RAW_SECTOR * 4];
    let d = DiscImage::from_bytes(raw).unwrap();
    assert_eq!(d.kind, playdiaemu_core::DiscKind::SingleRaw);
    assert_eq!(d.total_sectors, 4);

    let cooked = vec![0u8; 2048 * 3];
    let d = DiscImage::from_bytes(cooked).unwrap();
    assert_eq!(d.kind, playdiaemu_core::DiscKind::SingleCooked);
    assert_eq!(d.total_sectors, 3);
}

#[test]
fn disc_loads_multi_track_zip() {
    let dir = std::env::temp_dir().join("playdiaemu-zip-cue-test");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let zip_path = dir.join("game.zip");

    let track1 = vec![0u8; RAW_SECTOR * 2];
    let track2 = {
        let mut t = vec![0u8; RAW_SECTOR * 3];
        t[15] = 2;
        t[18] = 0x08;
        t[22] = 0x08;
        t[24] = 0xF1;
        t
    };
    let cue = "FILE \"game (Track 1).bin\" BINARY\n  TRACK 01 MODE2/2352\n    INDEX 01 00:00:00\nFILE \"game (Track 2).bin\" BINARY\n  TRACK 02 MODE2/2352\n    INDEX 01 00:00:00\n";

    {
        let file = std::fs::File::create(&zip_path).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        let opts = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        zip.start_file("game (Track 1).bin", opts).unwrap();
        zip.write_all(&track1).unwrap();
        zip.start_file("game (Track 2).bin", opts).unwrap();
        zip.write_all(&track2).unwrap();
        zip.start_file("game.cue", opts).unwrap();
        zip.write_all(cue.as_bytes()).unwrap();
        zip.finish().unwrap();
    }

    let d = DiscImage::from_path(&zip_path).unwrap();
    assert_eq!(d.kind, playdiaemu_core::DiscKind::CueMultiTrack);
    assert_eq!(d.tracks.len(), 2);
    assert_eq!(d.total_sectors, 5);
    assert!(d.stream_track().is_some());
    let stream = d.stream_track().unwrap();
    assert_eq!(stream.number, 2);
    assert_eq!(stream.data.len(), RAW_SECTOR * 3);
}

#[test]
fn disc_loads_single_image_zip() {
    let dir = std::env::temp_dir().join("playdiaemu-zip-single-test");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let zip_path = dir.join("single.zip");
    let raw = vec![0xA5u8; RAW_SECTOR * 2];

    {
        let file = std::fs::File::create(&zip_path).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        let opts = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        zip.start_file("disc.bin", opts).unwrap();
        zip.write_all(&raw).unwrap();
        zip.finish().unwrap();
    }

    let d = DiscImage::from_path(&zip_path).unwrap();
    assert_eq!(d.kind, playdiaemu_core::DiscKind::SingleRaw);
    assert_eq!(d.total_sectors, 2);
    assert_eq!(d.single.as_ref().unwrap().data, raw);
}

#[test]
fn state_roundtrip() {
    let id = ContentIdentity {
        disc_crc: 0xAABB_CCDD,
        disc_sectors: 10,
    };
    let payload = b"hello-playdia-state".to_vec();
    let blob = encode_state(&id, &payload);
    let out = decode_state(&blob, &id).unwrap();
    assert_eq!(out, payload);
    let bad = ContentIdentity {
        disc_crc: 1,
        disc_sectors: 10,
    };
    assert!(decode_state(&blob, &bad).is_err());
}

#[test]
fn parse_mode2_form2_video_sector() {
    use playdiaemu_core::content::{parse_raw_sector, RAW_SECTOR};
    let mut raw = vec![0u8; RAW_SECTOR];
    raw[15] = 2;
    raw[16] = 0x61;
    raw[17] = 0;
    raw[18] = 0x64;
    raw[19] = 0;
    raw[20] = 0x61;
    raw[21] = 0;
    raw[22] = 0x64;
    raw[23] = 0;
    let sec = parse_raw_sector(&raw).expect("parse");
    assert_eq!(sec.file_id, 0x61);
    assert_eq!(sec.form, 2);
    assert!(sec.is_video());
}

#[test]
fn crc32_stable() {
    assert_eq!(crc32(b""), 0);
    assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
}

#[test]
fn xa_adpcm_silence_produces_samples() {
    use playdiaemu_core::audio::AudioDecoder;
    use playdiaemu_core::cd::XaPacket;
    // 18*128 zeroed Form2 payload 鈫?silence, still expands to samples.
    let data = vec![0u8; 18 * 128];
    let mut ad = AudioDecoder::new();
    ad.ingest(&XaPacket::Audio {
        lba: 0,
        channel: 0,
        submode: 0x04 | 0x20,
        coding: 0,
        data,
    });
    let pcm = ad.drain();
    assert!(!pcm.is_empty());
    assert!(pcm.iter().all(|s| *s == 0));
}

#[test]
fn video_f1_accumulates_and_frame_end_clears() {
    use playdiaemu_core::cd::XaPacket;
    use playdiaemu_core::video::VideoDecoder;
    let mut v = VideoDecoder::new();
    let mut data = vec![0xF1u8];
    data.extend_from_slice(&[0u8; 32]);
    v.ingest_packet(&XaPacket::Video {
        lba: 0,
        channel: 0,
        submode: 0x08,
        coding: 0,
        data,
    });
    assert_eq!(v.acc_sectors, 1);
    v.ingest_packet(&XaPacket::FrameEnd {
        lba: 1,
        submode: 0x08,
        data: vec![0xF2],
    });
    assert_eq!(v.acc_sectors, 0);
    // Packet does not start 00 80 04 鈫?no successful decode
    assert_eq!(v.frames_decoded, 0);
}
