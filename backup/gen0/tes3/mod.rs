mod archive;
mod bsa;
mod writer;

use std::cmp::Ordering;

use bytemuck::{Pod, Zeroable};

pub use self::archive::Tes3Archive;
pub use self::writer::Tes3Writer;

use crate::read::EntryIndex;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct BsaIndex(u32);

impl EntryIndex for BsaIndex {}

#[derive(Debug, Clone, Copy, Pod, Zeroable)]
#[repr(C)]
pub struct Header {
    magic: [u8; 4],
    hash_table_offset: [u8; 4],
    file_count: [u8; 4],
}

impl Header {
    #[inline]
    pub fn new(hash_table_offset: u32, file_count: u32) -> Header {
        Header {
            magic: 0x100u32.to_le_bytes(),
            hash_table_offset: hash_table_offset.to_le_bytes(),
            file_count: file_count.to_le_bytes(),
        }
    }

    #[inline]
    pub fn magic(&self) -> u32 {
        u32::from_le_bytes(self.magic)
    }

    #[inline]
    pub fn hash_table_offset(&self) -> u32 {
        u32::from_le_bytes(self.hash_table_offset)
    }

    #[inline]
    pub fn file_count(&self) -> u32 {
        u32::from_le_bytes(self.file_count)
    }
}

#[derive(Debug, Clone, Copy, Pod, Zeroable)]
#[repr(C)]
pub struct Record {
    size: [u8; 4],
    offset: [u8; 4],
}

impl Record {
    pub fn new(size: u32, offset: u32) -> Record {
        Record {
            size: size.to_le_bytes(),
            offset: offset.to_le_bytes(),
        }
    }

    #[inline]
    pub fn size(&self) -> u32 {
        u32::from_le_bytes(self.size)
    }

    #[inline]
    pub fn offset(&self) -> u32 {
        u32::from_le_bytes(self.offset)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Pod, Zeroable)]
#[repr(transparent)]
pub struct NameOffset([u8; 4]);

impl NameOffset {
    pub fn new(off: u32) -> NameOffset {
        NameOffset(off.to_le_bytes())
    }

    #[inline]
    pub fn get(&self) -> u32 {
        u32::from_le_bytes(self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Pod, Zeroable)]
#[repr(transparent)]
pub struct NameHash([u8; 8]);

impl NameHash {
    #[inline]
    pub fn get(&self) -> u64 {
        u64::from_le_bytes(self.0)
    }

    fn compare_key(&self) -> u64 {
        self.get().rotate_right(32)
    }
}

impl PartialOrd for NameHash {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.compare_key().partial_cmp(&other.compare_key())
    }
}

impl Ord for NameHash {
    fn cmp(&self, other: &Self) -> Ordering {
        self.partial_cmp(other).unwrap()
    }
}

impl From<u64> for NameHash {
    fn from(value: u64) -> Self {
        Self(value.to_le_bytes())
    }
}

impl From<NameHash> for u64 {
    fn from(value: NameHash) -> Self {
        value.get()
    }
}

fn compute_hash(name: &str) -> Option<NameHash> {
    let norm_byte = |byte: u8| -> Option<u8> {
        match byte {
            b'/' => Some(b'\\'),
            b'\0' => None,
            byte if byte.is_ascii() => Some(byte.to_ascii_lowercase()),
            _ => None,
        }
    };

    let bytes = name.as_bytes();
    let (first, second) = bytes.split_at(bytes.len() / 2);

    let mut low = [0; 4];
    for (i, &byte) in first.iter().enumerate() {
        let byte = norm_byte(byte)?;
        low[i % 4] ^= byte;
    }
    let low = u32::from_le_bytes(low);

    let mut high = 0;
    for (i, &byte) in second.iter().enumerate() {
        let byte = norm_byte(byte)?;
        let temp = (byte as u32) << ((i % 4) << 3);
        high = (high ^ temp).rotate_right(temp & 0x1f);
    }

    let hash = (low as u64) | ((high as u64) << 32);
    Some(NameHash::from(hash))
}

#[cfg(test)]
mod tests;
