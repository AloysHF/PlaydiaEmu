//! Core tests: bus, content, state, SH-1 smoke, machine headless.

use playdia_core::bus::{Bus, RAM_BASE};
use playdia_core::content::{load_bios, DiscImage, LoadError, RAW_SECTOR};
use playdia_core::diagnostics::Diagnostics;
use playdia_core::machine::{Machine, MachineConfig, RunStop};
use playdia_core::sh1::Sh1;
use playdia_core::state::{crc32, decode_state, encode_state, ContentIdentity};
use std::io::Write;

#[test]
fn bios_size_check() {
    assert!(matches!(
        load_bios(&[0u8; 10], false),
        Err(LoadError::BadBiosSize(10))
    ));
    assert!(load_bios(&[], true).unwrap().len() == 0x8_0000);
}

#[test]
fn disc_size_rejects_odd() {
    assert!(DiscImage::from_bytes(vec![0u8; 100]).is_err());
}

#[test]
fn disc_accepts_raw_and_cooked() {
    let raw = vec![0u8; RAW_SECTOR * 4];
    let d = DiscImage::from_bytes(raw).unwrap();
    assert_eq!(d.kind, playdia_core::DiscKind::SingleRaw);
    assert_eq!(d.total_sectors, 4);

    let cooked = vec![0u8; 2048 * 3];
    let d = DiscImage::from_bytes(cooked).unwrap();
    assert_eq!(d.kind, playdia_core::DiscKind::SingleCooked);
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
    assert_eq!(d.kind, playdia_core::DiscKind::CueMultiTrack);
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
    assert_eq!(d.kind, playdia_core::DiscKind::SingleRaw);
    assert_eq!(d.total_sectors, 2);
    assert_eq!(d.single.as_ref().unwrap().data, raw);
}

#[test]
fn bus_ram_rw() {
    let mut bus = Bus::new(vec![0; 0x8_0000]);
    let mut diag = Diagnostics::default();
    bus.write32(RAM_BASE, 0x1122_3344, &mut diag);
    assert_eq!(bus.read32(RAM_BASE, &mut diag), 0x1122_3344);
    assert!(diag.is_quiet());
}

#[test]
fn bus_unmapped_diag() {
    let mut bus = Bus::new(vec![0; 0x8_0000]);
    let mut diag = Diagnostics::default();
    let _ = bus.read8(0x1234_5678, &mut diag);
    assert_eq!(diag.unmapped_reads, 1);
}

#[test]
fn state_roundtrip() {
    let id = ContentIdentity {
        disc_crc: 0xAABB_CCDD,
        bios_crc: Some(1),
        disc_sectors: 10,
    };
    let payload = b"hello-playdia-state".to_vec();
    let blob = encode_state(&id, &payload);
    let out = decode_state(&blob, &id).unwrap();
    assert_eq!(out, payload);
    let bad = ContentIdentity {
        disc_crc: 1,
        bios_crc: Some(1),
        disc_sectors: 10,
    };
    assert!(decode_state(&blob, &bad).is_err());
}

#[test]
fn sh1_executes_mov_and_add() {
    let mut bus = Bus::new(vec![0; 0x8_0000]);
    let mut diag = Diagnostics::default();
    // BIOS is ROM; place the program in on-chip RAM.
    //   0xE001  MOV.W #1,R0
    //   0x7002  ADD #2,R0
    //   0xAFFE  BRA self
    let words: [u16; 3] = [0xE001, 0x7002, 0xAFFE];
    for (i, w) in words.iter().enumerate() {
        let addr = RAM_BASE + (i as u32) * 2;
        bus.write16(addr, *w, &mut diag);
    }
    let mut cpu = Sh1::new();
    cpu.reset(RAM_BASE);
    cpu.run(&mut bus, &mut diag, 8);
    assert_eq!(cpu.r[0], 3);
    assert_eq!(cpu.stopped, None);
}

#[test]
fn parse_mode2_form2_video_sector() {
    use playdia_core::content::{parse_raw_sector, RAW_SECTOR};
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
fn machine_placeholder_boot_runs() {
    let cfg = MachineConfig {
        allow_placeholder_bios: true,
        enable_xa_stream: false,
        audio_test_tone: false,
    };
    let mut m = Machine::new(cfg);
    m.reset();
    for _ in 0..10 {
        assert_eq!(m.run_frame(), RunStop::Ok);
    }
    assert_eq!(m.cpu.stopped, None);
}

#[test]
fn machine_raw_disc_stream_counts() {
    // F1 video data sectors: mode2 Form1, channel 0, submode Data (0x08), marker 0xF1.
    let mut raw = vec![0u8; RAW_SECTOR * 4];
    for i in 0..4 {
        let off = i * RAW_SECTOR;
        raw[off + 15] = 2; // mode 2
        raw[off + 16] = 0; // file
        raw[off + 17] = 0; // channel
        raw[off + 18] = 0x08; // Data bit, not audio 0x04
        raw[off + 19] = 0;
        raw[off + 20] = 0;
        raw[off + 21] = 0;
        raw[off + 22] = 0x08;
        raw[off + 23] = 0;
        raw[off + 24] = 0xF1; // video marker
        for b in 1..64 {
            raw[off + 24 + b] = b as u8;
        }
    }
    // One audio Form2 sector at the end.
    let off = 3 * RAW_SECTOR;
    raw[off + 15] = 2;
    raw[off + 18] = 0x04 | 0x20; // audio + form2
    raw[off + 22] = 0x04 | 0x20;
    let cfg = MachineConfig {
        allow_placeholder_bios: true,
        enable_xa_stream: true,
        audio_test_tone: false,
    };
    let mut m = Machine::new(cfg);
    m.load_disc_bytes(raw).unwrap();
    m.reset();
    let _ = m.run_frame();
    let _ = m.run_frame();
    let stats = m.disc_stats().unwrap();
    assert!(stats.video_sectors >= 2, "got {}", stats.video_sectors);
    assert!(stats.audio_sectors >= 1, "got {}", stats.audio_sectors);
}

#[test]
fn save_state_on_machine() {
    let cfg = MachineConfig {
        allow_placeholder_bios: true,
        enable_xa_stream: false,
        audio_test_tone: false,
    };
    let mut m = Machine::new(cfg);
    m.reset();
    let _ = m.run_frame();
    m.video.framebuffer[0] = 0x0081_8283;
    m.video.framebuffer[320 * 240 - 1] = 0x0001_FE07;
    let blob = m.save_state();
    let mut m2 = Machine::new(MachineConfig {
        allow_placeholder_bios: true,
        enable_xa_stream: false,
        audio_test_tone: false,
    });
    // identity needs to match — load same empty bios/disc defaults
    m2.reset();
    // disc crc both 0, bios crc of zeros
    m2.load_state(&blob).unwrap();
    assert_eq!(m2.cpu.pc, m.cpu.pc);
    assert_eq!(m2.frame, m.frame);
    assert_eq!(m2.framebuffer(), m.framebuffer());
    assert_eq!(m2.video.crc(), m.video.crc());

    // Old two-byte framebuffer states must fail before mutating the machine.
    let mut old = blob;
    old[8..10].copy_from_slice(&1u16.to_le_bytes());
    let before = m2.save_state();
    assert!(matches!(
        m2.load_state(&old),
        Err(playdia_core::state::SaveStateError::BadVersion(1))
    ));
    assert_eq!(m2.save_state(), before);
}

#[test]
fn crc32_stable() {
    assert_eq!(crc32(b""), 0);
    assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
}

#[test]
fn xa_adpcm_silence_produces_samples() {
    use playdia_core::audio::AudioDecoder;
    use playdia_core::cd::XaPacket;
    // 18*128 zeroed Form2 payload → silence, still expands to samples.
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
    use playdia_core::cd::XaPacket;
    use playdia_core::video::VideoDecoder;
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
    // Packet does not start 00 80 04 → no successful decode
    assert_eq!(v.frames_decoded, 0);
}
