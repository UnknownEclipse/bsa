mod bsa;
// mod bsa_ref;

use std::cmp::Ordering;

pub use bsa::Bsa;
use bytemuck::{Pod, Zeroable};

use crate::read::EntryIndex;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct BsaIndex(u32);

impl EntryIndex for BsaIndex {}

#[derive(Debug, Clone, Copy, Pod, Zeroable)]
#[repr(C)]
struct Header {
    magic: [u8; 4],
    hash_table_offset: [u8; 4],
    file_count: [u8; 4],
}

impl Header {
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
struct Record {
    size: [u8; 4],
    offset: [u8; 4],
}

impl Record {
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
struct NameOffset([u8; 4]);

impl NameOffset {
    #[inline]
    pub fn get(&self) -> u32 {
        u32::from_le_bytes(self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Pod, Zeroable)]
#[repr(transparent)]
struct NameHash([u8; 8]);

impl NameHash {
    #[inline]
    pub fn get(&self) -> u64 {
        u64::from_le_bytes(self.0)
    }

    fn compare_key(&self) -> u64 {
        let value = self.get();
        value >> 32 | value << 32
    }
}

impl PartialOrd for NameHash {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.compare_key().partial_cmp(&other.compare_key())
    }
}

impl Ord for NameHash {
    fn cmp(&self, other: &Self) -> Ordering {
        self.compare_key().cmp(&other.compare_key())
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
