use std::str;

use memchr::memchr;

use super::{compute_hash, Header, NameHash, NameOffset, Record};
use crate::{
    read::{EntryData, RawEntryData},
    InvalidArchiveError, Result,
};

pub struct BsaRef<'a> {
    header: Header,
    records: &'a [Record],
    name_offsets: &'a [NameOffset],
    names: NameTable<'a>,
    hash_table: HashTable<'a>,
    data: &'a [u8],
}

impl<'a> BsaRef<'a> {
    pub fn new(buf: &'a [u8]) -> Result<Self> {
        let data = buf;

        if buf.len() < 12 {
            return Err(InvalidArchiveError::BadHeader.into());
        }

        let (header, buf) = buf.split_at(12);

        todo!()
    }

    pub fn get_raw(&self, index: usize) -> Result<RawEntryData<'a>> {
        let record = self.records[index];
        let off = record.offset() as usize;
        let len = record.size() as usize;
    }

    pub fn get(&self, index: usize) -> Result<EntryData<'a>> {
        let data = self.get_raw(index)?;
        Ok(EntryData::new_raw(data))
    }
}

#[derive(Debug, Clone, Copy)]
struct NameTable<'a>(&'a [u8]);

impl<'a> NameTable<'a> {
    pub fn new(bytes: &'a [u8]) -> Self {
        Self(bytes)
    }

    pub fn get(&self, off: NameOffset) -> Result<&'a str> {
        let off = off.get() as usize;
        let bytes = self.0;

        if bytes.len() <= off {
            return Err(InvalidArchiveError::BadOffset.into());
        }

        let bytes = &bytes[off..];
        let len = memchr(b'\0', bytes).ok_or(InvalidArchiveError::MissingNul)?;
        let name = &bytes[..len];

        if !name.is_ascii() {
            Err(InvalidArchiveError::BadEncoding.into())
        } else {
            unsafe { Ok(str::from_utf8_unchecked(name)) }
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct HashTable<'a>(&'a [NameHash]);

impl<'a> HashTable<'a> {
    pub fn new(hashes: &'a [NameHash]) -> Self {
        HashTable(hashes)
    }

    pub fn get_by_hash(&self, hash: NameHash) -> Option<usize> {
        self.0.binary_search(&hash).ok()
    }

    pub fn get_by_name(&self, name: &str) -> Option<usize> {
        self.get_by_hash(compute_hash(name)?)
    }
}
