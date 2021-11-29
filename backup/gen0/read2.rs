use std::{
    borrow::Cow,
    io::{Cursor, Read},
    path::Path,
};

use flate2::bufread::ZlibDecoder;
use lz4_flex::frame::FrameDecoder;

use crate::Result;

pub trait ArchiveRead {
    type Index: Copy + Eq;
    type Entry: ArchiveReadEntry;

    /// Return the data of the file at `index`.
    fn by_index(&mut self, index: Self::Index) -> Result<FileData>;

    fn find_by_name<S: AsRef<str>>(&self, name: S) -> Option<Self::Index>;

    fn by_name<S: AsRef<str>>(&mut self, name: S) -> Option<Result<FileData>> {
        let index = self.find_by_name(name)?;
        Some(self.by_index(index))
    }

    fn entry(&self, index: Self::Index) -> &Self::Entry;

    fn extract_to<P: AsRef<Path>>(&mut self, dir: P) -> Result<()>;
}

pub trait ArchiveReadEntry {}

pub struct FileData<'a> {
    inner: DataInner<'a>,
}

enum DataInner<'a> {
    Raw(Cursor<Cow<'a, [u8]>>),
    Zlib(ZlibDecoder<Cursor<Cow<'a, [u8]>>>),
    Lz4(FrameDecoder<Cursor<Cow<'a, [u8]>>>),
    Dyn(Box<dyn 'a + Read>),
}

impl Read for FileData<'_> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match &mut self.inner {
            DataInner::Raw(r) => r.read(buf),
            DataInner::Zlib(r) => r.read(buf),
            DataInner::Lz4(r) => r.read(buf),
            DataInner::Dyn(r) => r.read(buf),
        }
    }
}

impl<'a> From<Cow<'a, [u8]>> for FileData<'a> {
    fn from(raw: Cow<'a, [u8]>) -> Self {
        FileData {
            inner: DataInner::Raw(Cursor::new(raw)),
        }
    }
}

impl<'a> From<Vec<u8>> for FileData<'a> {
    fn from(raw: Vec<u8>) -> Self {
        FileData {
            inner: DataInner::Raw(Cursor::new(raw.into())),
        }
    }
}

impl<'a> From<&'a [u8]> for FileData<'a> {
    fn from(raw: &'a [u8]) -> Self {
        FileData {
            inner: DataInner::Raw(Cursor::new(raw.into())),
        }
    }
}

impl<'a, R> From<Box<R>> for FileData<'a>
where
    R: 'a + Read,
{
    fn from(r: Box<R>) -> Self {
        FileData {
            inner: DataInner::Dyn(r),
        }
    }
}

impl<'a> From<ZlibDecoder<Cursor<Cow<'a, [u8]>>>> for FileData<'a> {
    fn from(zlib: ZlibDecoder<Cursor<Cow<'a, [u8]>>>) -> Self {
        FileData {
            inner: DataInner::Zlib(zlib),
        }
    }
}

impl<'a> From<FrameDecoder<Cursor<Cow<'a, [u8]>>>> for FileData<'a> {
    fn from(lz4: FrameDecoder<Cursor<Cow<'a, [u8]>>>) -> Self {
        FileData {
            inner: DataInner::Lz4(lz4),
        }
    }
}
