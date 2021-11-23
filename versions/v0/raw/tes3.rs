use std::cmp::Ordering;

use bytemuck::{Pod, Zeroable};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Pod, Zeroable)]
#[repr(transparent)]
pub struct Hash([u8; 8]);

impl Hash {
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if !bytes.is_ascii() {
            None
        } else {
            Some(compute_hash(bytes))
        }
    }
}

impl PartialOrd for Hash {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        let lhs = hash_key(*self);
        let rhs = hash_key(*other);
        lhs.partial_cmp(&rhs)
    }
}

impl Ord for Hash {
    #[inline]
    fn cmp(&self, other: &Self) -> Ordering {
        let lhs = hash_key(*self);
        let rhs = hash_key(*other);
        lhs.cmp(&rhs)
    }
}

impl From<Hash> for u64 {
    #[inline]
    fn from(h: Hash) -> Self {
        u64::from_le_bytes(h.0)
    }
}

impl From<u64> for Hash {
    #[inline]
    fn from(h: u64) -> Self {
        Hash(h.to_le_bytes())
    }
}

fn hash_key(hash: Hash) -> u64 {
    let hash: u64 = hash.into();
    hash.rotate_right(32)
}

fn compute_hash(bytes: &[u8]) -> Hash {
    let norm_byte = |byte: u8| {
        if byte == b'/' {
            b'\\'
        } else {
            byte.to_ascii_lowercase()
        }
    };

    let (first, second) = bytes.split_at(bytes.len() / 2);

    let mut low = [0; 4];
    for (i, &byte) in first.iter().enumerate() {
        let byte = norm_byte(byte);
        low[i % 4] ^= byte;
    }
    let low = u32::from_le_bytes(low);

    let mut high = 0;
    for (i, &byte) in second.iter().enumerate() {
        let byte = norm_byte(byte);
        let temp = (byte as u32) << ((i % 4) << 3);
        high = (high ^ temp).rotate_right(temp & 0x1f);
    }

    let hash = (low as u64) | ((high as u64) << 32);
    Hash::from(hash)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Pod, Zeroable)]
#[repr(C)]
pub struct Record {
    size: [u8; 4],
    offset: [u8; 4],
}

impl Record {
    #[inline]
    pub fn new(size: u32, offset: u32) -> Record {
        Self {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Pod, Zeroable)]
#[repr(C)]
pub struct Header {
    magic: [u8; 4],
    hash_table_offset: [u8; 4],
    file_count: [u8; 4],
}

impl Header {
    #[inline]
    pub fn new(hash_table_offset: u32, file_count: u32) -> Self {
        Self {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Pod, Zeroable)]
#[repr(transparent)]
pub struct NameOffset([u8; 4]);

impl From<NameOffset> for u32 {
    #[inline]
    fn from(value: NameOffset) -> Self {
        u32::from_le_bytes(value.0)
    }
}

impl From<u32> for NameOffset {
    #[inline]
    fn from(value: u32) -> Self {
        NameOffset(value.to_le_bytes())
    }
}
