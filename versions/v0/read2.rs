use std::{
    collections::{hash_map, HashMap},
    io::{self, Cursor, Read, Seek},
    path::Path,
};

use flate2::read::ZlibDecoder;
use lz4_flex::frame::FrameDecoder;

use crate::Result;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Compression {
    Zlib,
    Lz4,
}

enum RawEntryData<'a> {
    Reader(io::Take<&'a mut dyn Read>),
    Owned(Cursor<Vec<u8>>),
}

impl Read for RawEntryData<'_> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let r: &mut dyn Read = match self {
            Self::Reader(r) => r,
            Self::Owned(r) => r,
        };
        r.read(buf)
    }
}

enum EntryData<'a> {
    Raw(RawEntryData<'a>),
    Zlib(ZlibDecoder<RawEntryData<'a>>),
    Lz4(FrameDecoder<RawEntryData<'a>>),
}

impl Read for EntryData<'_> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let r: &mut dyn Read = match self {
            Self::Raw(r) => r,
            Self::Zlib(r) => r,
            Self::Lz4(r) => r,
        };
        r.read(buf)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EntryId(usize);

pub struct ArchiveFile<'a> {
    data: EntryData<'a>,
    name: &'a str,
}

impl Read for ArchiveFile<'_> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.data.read(buf)
    }
}

struct RawEntry {
    offset: u32,
    size: u32,
    compression: Option<Compression>,
}

pub struct Archive<R>
where
    R: Read + Seek,
{
    reader: R,
    entries: Vec<RawEntry>,
    indices: HashMap<String, usize>,
}

impl<R> Archive<R>
where
    R: Read + Seek,
{
    pub fn by_name(&mut self, name: &str) -> Result<ArchiveFile<'_>> {
        todo!()
    }

    pub fn by_name_raw(&mut self, name: &str) -> Result<ArchiveFile<'_>> {
        todo!()
    }

    pub fn extract<P>(&mut self, dir: P) -> Result<()>
    where
        P: AsRef<Path>,
    {
        todo!()
    }
}

pub struct Entries<'a> {
    iter: hash_map::Iter<'a, String, usize>,
}

// impl<'a> Iterator for Entries<'a> {
//     type Item = (&'a str,);

//     fn next(&mut self) -> Option<Self::Item> {
//         todo!()
//     }
// }
