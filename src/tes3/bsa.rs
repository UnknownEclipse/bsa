use std::{borrow::Cow, convert::TryInto, io::Read, mem, str};

use memchr::memchr;

use crate::{
    common::{read_pod, read_pod_vec, read_vec},
    ArchiveReadError, Result,
};

use super::{compute_hash, Header, NameHash, NameOffset, Record};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BsaIndex(pub u32);

pub struct Bsa<'a> {
    pub header: Header,
    pub records: Cow<'a, [Record]>,
    pub name_offsets: Cow<'a, [NameOffset]>,
    pub names: NameTable<'a>,
    pub hash_table: HashTable<'a>,
}

pub struct NameTable<'a> {
    bytes: Cow<'a, [u8]>,
}

pub struct NameTableIter<'a> {
    bytes: &'a [u8],
}

pub struct HashTable<'a> {
    hashes: Cow<'a, [NameHash]>,
}

impl Bsa<'_> {
    pub fn new(r: &mut dyn Read) -> Result<Bsa<'static>> {
        let header: Header = read_pod(r)?;
        if header.magic() != 0x100 {
            return Err(ArchiveReadError::InvalidMagic.into());
        }
        dbg!(header.hash_table_offset());

        let file_count = header.file_count() as usize;
        let records = read_pod_vec(r, file_count)?;
        let name_offsets = read_pod_vec(r, file_count)?;

        let prelude_len = file_count
            .checked_mul(mem::size_of::<Record>() + mem::size_of::<NameOffset>())
            .and_then(|len| len.try_into().ok())
            .ok_or(ArchiveReadError::BadOffset)?;

        let names_len = header
            .hash_table_offset()
            .checked_sub(prelude_len)
            .ok_or(ArchiveReadError::BadOffset)?;

        let names = read_vec(r, names_len as usize)?;
        let hashes = read_pod_vec(r, file_count)?;

        let bsa = Bsa {
            header,
            records: Cow::Owned(records),
            name_offsets: Cow::Owned(name_offsets),
            names: NameTable {
                bytes: Cow::Owned(names),
            },
            hash_table: HashTable {
                hashes: Cow::Owned(hashes),
            },
        };

        Ok(bsa)
    }

    pub fn data_offset(&self) -> u32 {
        self.header.hash_table_offset()
            + self.header.file_count() * (mem::size_of::<NameHash>() as u32)
    }

    pub fn name(&self, index: BsaIndex) -> Result<&str> {
        self.names.get(self.name_offsets[index.0 as usize])
    }

    pub fn absolute_offset(&self, index: BsaIndex) -> u32 {
        self.data_offset() + self.records[index.0 as usize].offset()
    }

    pub fn file_len(&self, index: BsaIndex) -> u32 {
        self.records[index.0 as usize].size()
    }

    pub fn find_by_name(&self, name: &str) -> Option<BsaIndex> {
        Some(BsaIndex(self.hash_table.find_by_name(name)? as u32))
    }

    pub fn data_slice<'a>(&self, index: BsaIndex, data: &'a [u8]) -> Result<&'a [u8]> {
        let off = self.absolute_offset(index) as usize;
        let len = self.file_len(index) as usize;
        if data.len() <= (off + len) {
            Err(ArchiveReadError::BadOffset.into())
        } else {
            Ok(&data[off..off + len])
        }
    }
}

impl<'a> NameTable<'a> {
    pub fn get(&self, off: NameOffset) -> Result<&str> {
        let off = off.get() as usize;
        if self.bytes.len() <= off {
            return Err(ArchiveReadError::BadOffset.into());
        }
        let bytes = &self.bytes[off..];
        let len = match memchr(b'\0', bytes) {
            Some(i) => i,
            None => return Err(ArchiveReadError::MissingNul.into()),
        };
        let name = &bytes[..len];
        if !name.is_ascii() {
            return Err(ArchiveReadError::BadEncoding.into());
        }
        unsafe { Ok(str::from_utf8_unchecked(name)) }
    }

    pub fn iter(&self) -> NameTableIter {
        NameTableIter { bytes: &self.bytes }
    }
}

impl<'a> IntoIterator for &'a NameTable<'_> {
    type Item = Result<&'a str>;

    type IntoIter = NameTableIter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a> Iterator for NameTableIter<'a> {
    type Item = Result<&'a str>;

    fn next(&mut self) -> Option<Self::Item> {
        fn next_name<'b>(bytes: &mut &'b [u8]) -> Result<&'b str> {
            let len = match memchr(b'\0', bytes) {
                Some(i) => i,
                None => return Err(ArchiveReadError::MissingNul.into()),
            };
            let (name, rest) = bytes.split_at(len);
            if !name.is_ascii() {
                Err(ArchiveReadError::BadEncoding.into())
            } else {
                *bytes = rest;
                unsafe { Ok(str::from_utf8_unchecked(name)) }
            }
        }

        if self.bytes.is_empty() {
            None
        } else {
            Some(next_name(&mut self.bytes))
        }
    }
}

impl HashTable<'_> {
    pub fn find_by_hash(&self, hash: NameHash) -> Option<usize> {
        self.hashes.binary_search(&hash).ok()
    }

    pub fn find_by_name(&self, name: &str) -> Option<usize> {
        let hash = compute_hash(name)?;
        self.find_by_hash(hash)
    }
}
