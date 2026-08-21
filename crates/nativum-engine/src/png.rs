//! Uncompressed PNG encoder (no compression crate). Valid, deterministic files.

use crate::paint::Surface;

/// Encode `surface` as an RGBA8 PNG with filter 0 and stored-block zlib.
pub fn encode_png(surface: &Surface) -> Vec<u8> {
    let mut raw = Vec::with_capacity(((surface.width + 1) * surface.height * 4) as usize);
    for y in 0..surface.height {
        raw.push(0); // filter None
        let start = (y * surface.width * 4) as usize;
        let end = start + (surface.width * 4) as usize;
        raw.extend_from_slice(&surface.pixels[start..end]);
    }
    let mut out = Vec::new();
    out.extend_from_slice(&[137, 80, 78, 71, 13, 10, 26, 10]);
    write_chunk(&mut out, *b"IHDR", &ihdr(surface.width, surface.height));
    for piece in stored_zlib(&raw) {
        write_chunk(&mut out, *b"IDAT", &piece);
    }
    write_chunk(&mut out, *b"IEND", &[]);
    out
}

fn ihdr(w: u32, h: u32) -> [u8; 13] {
    let mut b = [0u8; 13];
    b[0..4].copy_from_slice(&w.to_be_bytes());
    b[4..8].copy_from_slice(&h.to_be_bytes());
    b[8] = 8; // bit depth
    b[9] = 6; // RGBA
    b
}

fn write_chunk(out: &mut Vec<u8>, ty: [u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    out.extend_from_slice(&ty);
    out.extend_from_slice(data);
    let mut crc = Crc32::new();
    crc.update(&ty);
    crc.update(data);
    out.extend_from_slice(&crc.finish().to_be_bytes());
}

/// Split into zlib stored blocks of at most 65535 bytes.
fn stored_zlib(data: &[u8]) -> Vec<Vec<u8>> {
    // One IDAT containing a complete zlib stream is simplest.
    let mut stream = Vec::new();
    // CMF/FLG: deflate, 32K window, no dict, fcheck.
    stream.extend_from_slice(&[0x78, 0x01]);
    let mut i = 0;
    while i < data.len() {
        let rest = data.len() - i;
        let n = rest.min(65535);
        let last = i + n == data.len();
        stream.push(if last { 0x01 } else { 0x00 });
        let n16 = n as u16;
        stream.extend_from_slice(&n16.to_le_bytes());
        stream.extend_from_slice((!n16).to_le_bytes().as_ref());
        stream.extend_from_slice(&data[i..i + n]);
        i += n;
    }
    if data.is_empty() {
        stream.push(0x01);
        stream.extend_from_slice(&0u16.to_le_bytes());
        stream.extend_from_slice(&0xFFFFu16.to_le_bytes());
    }
    stream.extend_from_slice(&adler32(data).to_be_bytes());
    vec![stream]
}

fn adler32(data: &[u8]) -> u32 {
    let mut a: u32 = 1;
    let mut b: u32 = 0;
    for &byte in data {
        a = (a + u32::from(byte)) % 65521;
        b = (b + a) % 65521;
    }
    (b << 16) | a
}

struct Crc32 {
    value: u32,
}

impl Crc32 {
    fn new() -> Self {
        Self { value: 0xFFFF_FFFF }
    }

    fn update(&mut self, data: &[u8]) {
        let mut c = self.value;
        for &byte in data {
            c ^= u32::from(byte);
            for _ in 0..8 {
                c = if c & 1 != 0 {
                    0xEDB8_8320 ^ (c >> 1)
                } else {
                    c >> 1
                };
            }
        }
        self.value = c;
    }

    fn finish(self) -> u32 {
        !self.value
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paint::Surface;
    use nativum_core::{Color, Size};

    #[test]
    fn png_signature() {
        let s = Surface::new(Size::new(2.0, 2.0), Color::rgb(255, 0, 0));
        let png = encode_png(&s);
        assert_eq!(&png[..8], &[137, 80, 78, 71, 13, 10, 26, 10]);
        assert!(png.len() > 32);
    }
}
