mod common;

use common::{escape, picture, put};
use playdia_core::cd::XaPacket;
use playdia_core::video::ak8000::{decode, ErrorKind, HEIGHT, WIDTH};
use playdia_core::video::VideoDecoder;
use playdia_core::video::{decode_packet_frames, CodecParams};

fn gray(rgb: &[u8], x: usize, y: usize) -> u16 {
    let offset = (y * WIDTH + x) * 2;
    u16::from_le_bytes([rgb[offset], rgb[offset + 1]])
}

fn gray555(value: u16) -> u16 {
    let channel = value >> 3;
    channel | (channel << 5) | (channel << 10)
}

#[test]
fn dc_prediction_uses_the_current_macroblock_first_luma() {
    let packet = picture(|_, index, bits| {
        let delta = match index {
            0 => 16,
            1 => 8,
            2 => -8,
            6 => 8,
            _ => 0,
        };
        if delta != 0 {
            escape(bits, 0, delta);
        }
        put(bits, 1, 2);
    });
    let rgb = decode(&packet).unwrap();
    assert_eq!((WIDTH, HEIGHT, rgb.len()), (248, 216, 248 * 216 * 2));
    for (x, y, expected) in [
        (0, 0, 144),
        (4, 0, 152),
        (0, 4, 136),
        (4, 4, 144),
        (8, 0, 152),
        (12, 0, 152),
    ] {
        assert_eq!(gray(&rgb, x, y), gray555(expected), "pixel {x},{y}");
    }
    assert_eq!(gray(&rgb, 0, 8), gray555(144));
    let frames = decode_packet_frames(&packet, CodecParams::default());
    assert_eq!(frames.len(), 1);
    assert_eq!(frames[0].1, 27 * 186);
}

#[test]
fn full_blocks_end_without_an_eob_and_negative_escape_is_signed() {
    let packet = picture(|_, index, bits| {
        if index == 0 {
            escape(bits, 0, -64);
            for _ in 1..16 {
                put(bits, 0b110, 3);
            }
        } else {
            put(bits, 1, 2);
        }
    });
    let rgb = decode(&packet).unwrap();
    assert_eq!(gray(&rgb, 8, 0), gray555(64));
    assert_eq!(gray(&rgb, 247, 215), gray555(64));
}

#[test]
fn rejects_coefficient_overflow_and_truncated_picture() {
    let packet = picture(|_, index, bits| {
        if index == 0 {
            escape(bits, 14, 1);
            escape(bits, 1, 1);
        }
        put(bits, 1, 2);
    });
    let error = decode(&packet).unwrap_err();
    assert_eq!(
        (error.kind, error.row, error.block),
        (ErrorKind::CoefficientOverflow, 1, 0)
    );
    let valid = picture(|_, _, bits| put(bits, 1, 2));
    for length in [0, 35, 36, 400, valid.len() - 1] {
        assert!(decode(&valid[..length]).is_err());
    }
    let mut bad = valid.clone();
    bad[2] |= 1;
    assert_eq!(decode(&bad).unwrap_err().kind, ErrorKind::UnsupportedHeader);
    let mut padded = valid.clone();
    padded.extend_from_slice(&[0xFF; 64]);
    assert_eq!(decode(&valid).unwrap(), decode(&padded).unwrap());
    padded.push(0x42);
    assert!(decode(&padded).is_err());
}

#[test]
fn rare_run_five_symbol_reaches_the_last_coefficient() {
    let packet = picture(|_, index, bits| {
        if index == 0 {
            put(bits, 0b00000000100010, 14);
            escape(bits, 9, 1);
        } else {
            put(bits, 1, 2);
        }
    });
    // Run 5 then run 9 fills positions 5 and 15, with no EOB.
    assert!(decode(&packet).is_ok());
}

#[test]
fn invalid_picture_preserves_the_last_presented_frame() {
    let mut video = VideoDecoder::new();
    let valid = picture(|_, _, bits| put(bits, 1, 2));
    let mut invalid = valid.clone();
    invalid[36] ^= 1;
    for packet in [&valid, &invalid] {
        let mut data = vec![0xF1];
        data.extend_from_slice(packet);
        video.ingest_packet(&XaPacket::Video {
            lba: 0,
            channel: 0,
            submode: 8,
            coding: 0,
            data,
        });
        let mut end = vec![0xFF; 2048];
        end[0] = 0xF2;
        video.ingest_packet(&XaPacket::FrameEnd {
            lba: 1,
            submode: 8,
            data: end,
        });
        if video.frames_decoded == 0 {
            assert!(video.present_next());
        }
    }
    assert_eq!((video.frames_decoded, video.frames_failed), (1, 1));
    assert!(!video.present_next());
    assert_eq!(video.framebuffer[12 * 320 + 36], gray555(128));
    assert_eq!(video.framebuffer[0], 0);
}

#[test]
fn native_export_retains_eight_bit_channels() {
    let packet = picture(|_, index, bits| {
        if index == 0 {
            escape(bits, 0, 3);
        }
        put(bits, 1, 2);
    });
    let rgb = playdia_core::video::ak8000::decode_rgb888(&packet).unwrap();
    assert_eq!(rgb.len(), WIDTH * HEIGHT * 3);
    assert!(rgb.iter().all(|&channel| channel == 131));
    assert_eq!(gray(&decode(&packet).unwrap(), 0, 0), gray555(128));
}
