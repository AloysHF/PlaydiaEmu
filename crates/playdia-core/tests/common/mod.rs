pub fn put(bits: &mut Vec<bool>, value: u32, count: usize) {
    for shift in (0..count).rev() {
        bits.push(value & (1 << shift) != 0);
    }
}

pub fn escape(bits: &mut Vec<bool>, run: u32, level: i32) {
    put(bits, 8, 6);
    put(bits, run, 4);
    put(bits, level as u32 & 1023, 10);
}

pub fn picture(mut block: impl FnMut(usize, usize, &mut Vec<bool>)) -> Vec<u8> {
    let mut bits = Vec::new();
    put(&mut bits, 0x400, 19);
    put(&mut bits, 1, 3);
    put(&mut bits, 0, 2);
    put(&mut bits, 4, 8);
    for _ in 0..32 {
        put(&mut bits, 64, 8);
    }
    for row in 0..27 {
        put(&mut bits, (0x20 << 5) | (row as u32 + 1), 19);
        for index in 0..186 {
            block(row, index, &mut bits);
        }
    }
    put(&mut bits, 0x21, 14);
    while !bits.len().is_multiple_of(8) {
        bits.push(false);
    }
    bits.as_chunks::<8>()
        .0
        .iter()
        .map(|byte| byte.iter().fold(0, |v, &b| (v << 1) | u8::from(b)))
        .collect()
}
