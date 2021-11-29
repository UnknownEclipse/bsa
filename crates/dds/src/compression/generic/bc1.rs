struct Pixel {
    r: u8,
    g: u8,
    b: u8,
}

struct Texel {
    pub pixels: [[Pixel; 4]; 4],
}

// #[inline]
// fn decompress_texel(bytes: [u8; 8]) -> Texel {}
