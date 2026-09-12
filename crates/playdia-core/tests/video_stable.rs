//! Decode stability: identical packets must yield identical frames.

use playdia_core::video::{decode_packet, CodecParams};

fn sample_packet() -> Vec<u8> {
    // Minimal valid header + bitstream zeros (12282-like size).
    let mut p = vec![0u8; 256];
    p[0] = 0x00;
    p[1] = 0x80;
    p[2] = 0x04;
    p[3] = 8; // QS
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
    // bitstream from 44
    for b in p.iter_mut().skip(44) {
        *b = 0x80;
    }
    p
}

#[test]
fn same_packet_same_output() {
    let pkt = sample_packet();
    let p = CodecParams::default();
    let a = decode_packet(&pkt, p).expect("a");
    let b = decode_packet(&pkt, p).expect("b");
    assert_eq!(a.0, b.0);
    assert_eq!(a.1, b.1);
}

#[test]
fn packet_header_rejects_garbage() {
    assert!(decode_packet(&[0u8; 64], CodecParams::default()).is_none());
}
