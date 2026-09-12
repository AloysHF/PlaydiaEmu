//! Decode stability: identical packets must yield identical frames.

use playdia_core::video::{decode_packet_frames, CodecParams};

fn sample_packet() -> Vec<u8> {
    // Load a real packet if present (most reliable).
    let p = std::path::Path::new("tmp/out/pkt0_s150_e156.bin");
    if p.exists() {
        return std::fs::read(p).unwrap();
    }
    // Fallback synthetic.
    let mut p = vec![0u8; 512];
    p[0] = 0x00;
    p[1] = 0x80;
    p[2] = 0x04;
    p[3] = 8;
    for i in 0..16 {
        p[4 + i] = 8;
        p[20 + i] = 8;
    }
    p[36] = 0x00;
    p[37] = 0x80;
    p[38] = 0x24;
    p[40] = 90;
    p[41] = 128;
    p[42] = 128;
    for b in p.iter_mut().skip(45) {
        *b = 0x80;
    }
    p
}

#[test]
fn same_packet_same_output() {
    let pkt = sample_packet();
    let p = CodecParams::default();
    let a = decode_packet_frames(&pkt, p);
    let b = decode_packet_frames(&pkt, p);
    assert!(!a.is_empty(), "need at least one frame from sample packet");
    assert_eq!(a.len(), b.len());
    assert_eq!(a[0].0, b[0].0);
    assert_eq!(a[0].1, b[0].1);
}

#[test]
fn packet_header_rejects_garbage() {
    assert!(decode_packet_frames(&[0u8; 64], CodecParams::default()).is_empty());
}
