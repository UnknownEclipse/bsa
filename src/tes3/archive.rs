use std::{
    io::{Read, Seek, SeekFrom},
    ops::Range,
    str,
};

use memchr::memchr;

use crate::{
    read::{EntryData, RawEntryData},
    tes3::NameOffset,
    ArchiveReadError, Result,
};

use super::bsa::{Bsa, BsaIndex};

pub struct Tes3Archive<R>
where
    R: Read + Seek,
{
    reader: R,
    bsa: Bsa<'static>,
}

impl<R> Tes3Archive<R>
where
    R: Read + Seek,
{
    pub fn new(mut r: R) -> Result<Tes3Archive<R>> {
        let bsa = Bsa::new(&mut r)?;
        Ok(Tes3Archive { reader: r, bsa })
    }

    pub fn entries(&self) -> Entries {
        Entries {
            bsa: &self.bsa,
            indices: 0..self.bsa.records.len() as u32,
        }
    }

    pub fn open_by_index(&mut self, index: BsaIndex) -> Result<EntryData> {
        let off = self.bsa.absolute_offset(index) as u64;
        let len = self.bsa.file_len(index) as u64;
        self.reader.seek(SeekFrom::Start(off))?;
        let r = &mut self.reader as &mut dyn Read;
        Ok(EntryData::new_uncompressed(RawEntryData::from_stream(
            r.take(len),
        )))
    }

    pub fn open_by_name(&mut self, name: &str) -> Result<EntryData> {
        let index = self
            .get_by_name(name)
            .ok_or(ArchiveReadError::FileNotFound)??
            .index();
        self.open_by_index(index)
    }

    pub fn get_by_name(&self, name: &str) -> Option<Result<Entry>> {
        let index = BsaIndex(self.bsa.hash_table.find_by_name(name)? as u32);
        Some(entry_for(&self.bsa, index))
    }
}

pub struct Entries<'a> {
    bsa: &'a Bsa<'static>,
    indices: Range<u32>,
}

impl<'a> Iterator for Entries<'a> {
    type Item = Result<Entry<'a>>;

    fn next(&mut self) -> Option<Self::Item> {
        let index = BsaIndex(self.indices.next()?);
        Some(entry_for(self.bsa, index))
    }
}

#[derive(Debug)]
pub struct Entry<'a> {
    index: BsaIndex,
    size: u32,
    name: &'a str,
}

impl<'a> Entry<'a> {
    #[inline]
    pub fn index(&self) -> BsaIndex {
        self.index
    }

    #[inline]
    pub fn name(&self) -> &'a str {
        self.name
    }
}

struct File {
    size: u32,
    offset: u32,
}

fn read_name(names: &[u8], off: NameOffset) -> Result<&str> {
    let off = off.get() as usize;
    if names.len() <= off {
        return Err(ArchiveReadError::BadOffset.into());
    }

    let names = &names[off..];
    let len = memchr(b'\0', names).ok_or(ArchiveReadError::MissingNul)?;
    let name = &names[..len];

    if !name.is_ascii() {
        Err(ArchiveReadError::BadEncoding.into())
    } else {
        Ok(unsafe { str::from_utf8_unchecked(name) })
    }
}

fn entry_for<'a>(bsa: &'a Bsa, index: BsaIndex) -> Result<Entry<'a>> {
    let size = bsa.file_len(index);
    let name = bsa.name(index)?;
    Ok(Entry { size, name, index })
}
