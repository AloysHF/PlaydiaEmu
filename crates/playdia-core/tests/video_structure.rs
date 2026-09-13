use playdia_core::cd::{XaDemux, XaPacket};
use playdia_core::content::parse_raw_sector;
use playdia_core::video::structure::{
    read_bits, scan_picture_rows, video_fragment, PictureHeader, VIDEO_PACKET_CAP,
};
use playdia_core::video::VideoDecoder;

fn put(bits: &mut Vec<bool>, value: u32, count: usize) {
    for shift in (0..count).rev() {
        bits.push((value >> shift) & 1 != 0);
    }
}

fn picture(ambiguous: bool) -> (Vec<u8>, [usize; 27], usize) {
    let mut bits = Vec::new();
    put(&mut bits, 0x400, 19);
    put(&mut bits, 1, 3);
    put(&mut bits, 2, 2);
    put(&mut bits, 9, 8);
    for _ in 0..32 {
        put(&mut bits, 7, 8);
    }
    let mut starts = [0; 27];
    for (row, start) in starts.iter_mut().enumerate() {
        *start = bits.len();
        put(&mut bits, (0x20 << 5) | (row as u32 + 1), 19);
        for _ in 0..190 {
            put(&mut bits, 1, 2);
        }
        if row == 8 {
            // A false end code inside entropy must not truncate the picture.
            put(&mut bits, 0x21, 14);
            put(&mut bits, 3, 2);
        }
        if row == 12 && ambiguous {
            put(&mut bits, (0x20 << 5) | 14, 19);
            put(&mut bits, 3, 2);
        }
    }
    let end = bits.len();
    put(&mut bits, 0x21, 14);
    while bits.len() % 8 != 0 {
        bits.push(false);
    }
    let mut bytes: Vec<u8> = bits
        .as_chunks::<8>()
        .0
        .iter()
        .map(|b| b.iter().fold(0, |v, &bit| (v << 1) | u8::from(bit)))
        .collect();
    bytes.extend_from_slice(&[0xFF; 20]);
    (bytes, starts, end)
}

#[test]
fn rows_cross_bytes_and_ignore_false_terminators() {
    let (bytes, starts, end) = picture(false);
    let header = PictureHeader::parse(&bytes).unwrap();
    assert_eq!(
        (header.picture_type, header.quantizer_shift, header.factor),
        (1, 2, 9)
    );
    let rows = scan_picture_rows(&bytes).unwrap();
    assert_eq!(rows.starts, starts);
    assert_eq!(rows.terminator_bit, end);
    assert_eq!(rows.ambiguous_rows, 0);
    assert!(starts.iter().any(|p| p % 8 != 0));
    assert!(scan_picture_rows(&bytes[..end / 8]).is_none());
    assert_eq!(read_bits(&bytes, usize::MAX, 1), None);
}

#[test]
fn marker_collisions_are_reported() {
    let (bytes, _, _) = picture(true);
    assert_eq!(scan_picture_rows(&bytes).unwrap().ambiguous_rows, 1);
}

fn raw_sector(marker: u8) -> Vec<u8> {
    let mut raw = vec![0; 2352];
    raw[15] = 2;
    raw[16..24].copy_from_slice(&[1, 0, 8, 0, 1, 0, 8, 0]);
    raw[24] = marker;
    raw
}

#[test]
fn f2_tail_and_interleaved_f3_preserve_the_picture() {
    let (mut bytes, _, _) = picture(false);
    // Legacy preview accepts picture type 1, QBS 0 only.
    bytes[2] = 4;
    let split = bytes.len() - 200;
    let mut video = VideoDecoder::new();
    let mut demux = XaDemux::new();
    let mut f1 = raw_sector(0xF1);
    f1[25..25 + split].copy_from_slice(&bytes[..split]);
    // Use a short synthetic fragment to place the terminator in the F2 tail.
    let mut sector = parse_raw_sector(&f1).unwrap();
    sector.data.truncate(split + 1);
    video.ingest_packet(&demux.push(&sector));
    let mut filler = raw_sector(0xF3);
    filler[25..27].copy_from_slice(&[0x6F, 0xF2]);
    filler[27..2072].fill(0xFF);
    video.ingest_packet(&demux.push(&parse_raw_sector(&filler).unwrap()));
    assert_eq!(video.acc, bytes[..split]);
    let mut f2 = raw_sector(0xF2);
    f2[24 + 0x23..2072].fill(0xFF);
    f2[24 + 0x23..24 + 0x23 + bytes.len() - split].copy_from_slice(&bytes[split..]);
    video.ingest_packet(&demux.push(&parse_raw_sector(&f2).unwrap()));
    assert_eq!(video.last_packet_len, split + 2013);
    assert!(video.acc.is_empty());
    assert_eq!(demux.scene_resets, 0);
    assert_eq!(demux.frame_ends, 1);
    assert_eq!(video_fragment(&f2[24..]).len(), 2013);
}

#[test]
fn overflow_discards_the_whole_picture_and_recovers_at_f2() {
    let mut video = VideoDecoder::new();
    let mut data = vec![0xFF; 2048];
    data[0..4].copy_from_slice(&[0xF1, 0, 0x80, 4]);
    data[37..40].copy_from_slice(&[0, 0x80, 0x24]);
    let packet = XaPacket::Video {
        lba: 0,
        channel: 0,
        submode: 8,
        coding: 0,
        data,
    };
    for _ in 0..VIDEO_PACKET_CAP / 2047 + 1 {
        video.ingest_packet(&packet);
    }
    video.ingest_packet(&XaPacket::FrameEnd {
        lba: 1,
        submode: 8,
        data: vec![0xF2],
    });
    assert_eq!(video.frames_failed, 1);
    assert_eq!(video.last_packet_len, 0);
    video.ingest_packet(&packet);
    assert_eq!(video.acc.len(), 2047);
}
