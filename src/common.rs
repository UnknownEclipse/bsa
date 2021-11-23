use std::{
    convert::TryFrom,
    fmt::Display,
    io::{self, Read},
    iter::FusedIterator,
    mem::MaybeUninit,
    slice, str,
};

use bytemuck::{cast_slice_mut, Pod};
use thiserror::Error;

mod bytes;

pub use bytes::Bytes;

/// Efficiently read a buffer into a vector.
pub fn read_vec(r: &mut dyn Read, len: usize) -> io::Result<Vec<u8>> {
    let mut buf = Vec::new();
    read_vec_in(r, len, &mut buf)?;
    Ok(buf)
    // let mut v = Vec::with_capacity(len);
    // unsafe {
    //     // SAFETY:
    //     // 1. The vector has been created using `Vec::with_capacity(len)`.
    //     // 2. We are going to fill the uninitialized memory using `read_exact`. If the
    //     //    reader is malicious, and reports a higher number of bytes read than
    //     //    actually were, the worst that happens is that there is some bogus data,
    //     //    which should be fine. (Will likely cause an error later down the line, but
    //     //    no different than a corrupt file would anyway).
    //     debug_assert!(v.capacity() >= len);
    //     v.set_len(len);
    // }
    // r.read_exact(&mut v)?;
    // Ok(v)
}

pub fn read_vec_in(r: &mut dyn Read, len: usize, buf: &mut Vec<u8>) -> io::Result<()> {
    buf.clear();
    buf.reserve(len);

    unsafe {
        // SAFETY:
        // 1. The vector has been created using `Vec::with_capacity(len)`.
        // 2. We are going to fill the uninitialized memory using `read_exact`. If the
        //    reader is malicious, and reports a higher number of bytes read than
        //    actually were, the worst that happens is that there is some bogus data,
        //    which should be fine. (Will likely cause an error later down the line, but
        //    no different than a corrupt file would anyway).
        debug_assert!(buf.capacity() >= len);
        buf.set_len(len);
    }
    r.read_exact(buf)?;
    Ok(())
}

// pub fn cast_vec<T>(mut v: Vec<u8>) -> Vec<T>
// where
//     T: Pod,
// {
//     let ptr = v.as_mut_ptr();
//     let len = v.len();
//     let cap = v.capacity();

//     assert_eq!(ptr as usize % mem::align_of::<T>(), 0, "invalid alignment");
//     assert_eq!(len % mem::size_of::<T>(), 0, "invalid size");
//     assert_eq!(cap % mem::size_of::<T>(), 0, "invalid capacity");

//     mem::forget(v);

//     let ptr = ptr as *mut T;
//     let len = len / mem::size_of::<T>();
//     let cap = len / mem::size_of::<T>();

//     unsafe { Vec::from_raw_parts(ptr, len, cap) }
// }

#[allow(clippy::uninit_assumed_init)]
pub fn read_pod<T: Pod, R: Read + ?Sized>(r: &mut R) -> io::Result<T> {
    let mut value: T = unsafe { MaybeUninit::uninit().assume_init() };
    r.read_exact(bytemuck::bytes_of_mut(&mut value))?;
    Ok(value)
}

#[allow(clippy::uninit_assumed_init)]
pub fn read_pod_vec_in<T: Pod, R: Read + ?Sized>(
    r: &mut R,
    n: usize,
    buf: &mut Vec<T>,
) -> io::Result<()> {
    buf.clear();
    buf.reserve(n);
    unsafe { buf.set_len(n) };
    r.read_exact(cast_slice_mut(buf))?;
    Ok(())
}

#[allow(clippy::uninit_assumed_init)]
pub fn read_pod_vec<T: Pod, R: Read + ?Sized>(r: &mut R, n: usize) -> io::Result<Vec<T>> {
    let mut buf = Vec::new();
    read_pod_vec_in(r, n, &mut buf)?;
    Ok(buf)
}

pub trait Sealed {}

#[allow(dead_code)]
pub mod windows_1252 {
    use std::{borrow::Cow, str};

    #[allow(dead_code)]
    const TABLE: [u16; 128] = [
        0x20AC, 0x0081, 0x201A, 0x0192, 0x201E, 0x2026, 0x2020, 0x2021, 0x02C6, 0x2030, 0x0160,
        0x2039, 0x0152, 0x008D, 0x017D, 0x008F, 0x0090, 0x2018, 0x2019, 0x201C, 0x201D, 0x2022,
        0x2013, 0x2014, 0x02DC, 0x2122, 0x0161, 0x203A, 0x0153, 0x009D, 0x017E, 0x0178, 0x00A0,
        0x00A1, 0x00A2, 0x00A3, 0x00A4, 0x00A5, 0x00A6, 0x00A7, 0x00A8, 0x00A9, 0x00AA, 0x00AB,
        0x00AC, 0x00AD, 0x00AE, 0x00AF, 0x00B0, 0x00B1, 0x00B2, 0x00B3, 0x00B4, 0x00B5, 0x00B6,
        0x00B7, 0x00B8, 0x00B9, 0x00BA, 0x00BB, 0x00BC, 0x00BD, 0x00BE, 0x00BF, 0x00C0, 0x00C1,
        0x00C2, 0x00C3, 0x00C4, 0x00C5, 0x00C6, 0x00C7, 0x00C8, 0x00C9, 0x00CA, 0x00CB, 0x00CC,
        0x00CD, 0x00CE, 0x00CF, 0x00D0, 0x00D1, 0x00D2, 0x00D3, 0x00D4, 0x00D5, 0x00D6, 0x00D7,
        0x00D8, 0x00D9, 0x00DA, 0x00DB, 0x00DC, 0x00DD, 0x00DE, 0x00DF, 0x00E0, 0x00E1, 0x00E2,
        0x00E3, 0x00E4, 0x00E5, 0x00E6, 0x00E7, 0x00E8, 0x00E9, 0x00EA, 0x00EB, 0x00EC, 0x00ED,
        0x00EE, 0x00EF, 0x00F0, 0x00F1, 0x00F2, 0x00F3, 0x00F4, 0x00F5, 0x00F6, 0x00F7, 0x00F8,
        0x00F9, 0x00FA, 0x00FB, 0x00FC, 0x00FD, 0x00FE, 0x00FF,
    ];

    #[inline]
    pub fn decode(byte: u8) -> char {
        if byte.is_ascii() {
            char::from(byte)
        } else {
            let ch = TABLE[byte as usize - 128];
            let ch = ch as u32;
            unsafe { char::from_u32_unchecked(ch) }
        }
    }

    #[inline]
    pub fn encode(ch: char) -> Option<u8> {
        if ch.is_ascii() {
            Some(ch as u8)
        } else {
            let index = TABLE.iter().position(|&c| c as u32 == ch as u32)?;
            let byte = index + 128;
            let byte = byte as u8;
            Some(byte)
        }
    }

    pub fn decode_into(s: &str, buf: &mut Vec<u8>) -> Option<()> {
        for ch in s.chars() {
            buf.push(encode(ch)?);
        }
        Some(())
    }

    #[inline]
    pub fn to_string(win1252: Vec<u8>) -> String {
        if win1252.is_ascii() {
            unsafe { String::from_utf8_unchecked(win1252) }
        } else {
            let mut s = String::with_capacity(win1252.len());
            for byte in win1252 {
                s.push(decode(byte));
            }
            s
        }
    }

    #[inline]
    pub fn to_str(win1252: &[u8]) -> Cow<str> {
        if win1252.is_ascii() {
            unsafe { str::from_utf8_unchecked(win1252).into() }
        } else {
            let mut s = String::with_capacity(win1252.len());
            for &byte in win1252 {
                s.push(decode(byte));
            }
            s.into()
        }
    }

    #[inline]
    pub fn from_string(s: String) -> Option<Vec<u8>> {
        if s.is_ascii() {
            Some(s.into_bytes())
        } else {
            let mut buf = Vec::with_capacity(s.len());
            for ch in s.chars() {
                let byte = encode(ch)?;
                buf.push(byte);
            }
            Some(buf)
        }
    }

    #[inline]
    pub fn from_str(s: &str) -> Option<Cow<[u8]>> {
        if s.is_ascii() {
            Some(Cow::Borrowed(s.as_bytes()))
        } else {
            let mut buf = Vec::with_capacity(s.len());
            for ch in s.chars() {
                let byte = encode(ch)?;
                buf.push(byte);
            }
            Some(Cow::Owned(buf))
        }
    }

    #[inline]
    pub fn to_lowercase(byte: u8) -> u8 {
        // https://en.wikipedia.org/wiki/Windows-1252#Character_set
        match byte {
            // ASCII
            b'A'..=b'Z' => byte + (b'a' - b'A'),
            // 'Š', 'Œ', 'Ž'
            0x8a | 0x8c | 0x9e => byte + 0x10,
            // 'À' through 'Þ'
            0xc0..=0xd0 => byte + 0x20,
            _ => byte,
        }
    }
}

#[derive(Default, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Win1252String(Vec<u8>);

pub struct Chars<'a> {
    bytes: slice::Iter<'a, u8>,
}

impl Iterator for Chars<'_> {
    type Item = char;

    fn next(&mut self) -> Option<Self::Item> {
        Some(windows_1252::decode(*self.bytes.next()?))
    }
}

impl FusedIterator for Chars<'_> {}

impl ExactSizeIterator for Chars<'_> {
    #[inline]
    fn len(&self) -> usize {
        self.bytes.len()
    }
}

impl Win1252String {
    #[inline]
    pub fn new() -> Win1252String {
        Default::default()
    }

    #[inline]
    pub fn from_bytes(bytes: Vec<u8>) -> Win1252String {
        Self(bytes)
    }

    #[inline]
    pub fn chars(&self) -> Chars {
        Chars {
            bytes: self.0.iter(),
        }
    }

    #[inline]
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    #[inline]
    pub fn into_bytes(self) -> Vec<u8> {
        self.0
    }

    #[inline]
    pub fn into_string(self) -> String {
        if self.0.is_ascii() {
            unsafe { String::from_utf8_unchecked(self.0) }
        } else {
            self.chars().collect()
        }
    }
}

#[derive(Debug, Error)]
#[error("unable to encode character {0} at index {1} as windows-1252")]
pub struct Win1252StringTryFromStringError(char, usize);

impl TryFrom<String> for Win1252String {
    type Error = Win1252StringTryFromStringError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        if value.is_ascii() {
            Ok(Win1252String(value.into_bytes()))
        } else {
            let mut buf = Vec::new();
            for (i, ch) in value.char_indices() {
                if let Some(byte) = windows_1252::encode(ch) {
                    buf.push(byte);
                } else {
                    return Err(Win1252StringTryFromStringError(ch, i));
                }
            }
            Ok(Win1252String(buf))
        }
    }
}

impl Display for Win1252String {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let bytes = self.as_bytes();
        if bytes.is_ascii() {
            let s = unsafe { str::from_utf8_unchecked(bytes) };
            f.write_str(s)
        } else {
            let s: String = self.chars().collect();
            f.write_str(&s)
        }
    }
}
