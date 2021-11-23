pub fn decode(bytes: &[u8]) -> String {
    todo!()
}

#[inline]
pub fn to_lowercase(byte: u8) -> u8 {
    match byte {
        b'A'..=b'Z' => byte + (b'a' - b'A'),
        0x8a | 0x8c | 0x8e => byte + 0x10,
        0xc0..=0xd6 | 0xd8..=0xdf => byte + 0x20,
        _ => byte,
    }
}
