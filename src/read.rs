use std::io::{self, Cursor, Read};

use flate2::read::ZlibDecoder;
use lz4_flex::frame::FrameDecoder;

use crate::{
    common::read_vec,
    tes4::{RawBsa, Tes5},
    Result,
};

pub mod fo4;

pub type FnvBsa = crate::tes4::bsa::OwnedBsa<Tes5>;

pub use crate::{
    tes3::Tes3Archive,
    tes4::{FnvArchive, Fo3Archive, SseArchive, Tes4Archive, Tes5Archive},
};
pub use fo4::ba2::Archive as Fo4Archive;

pub trait EntryIndex: Copy + Eq {}

pub trait ArchiveRead {
    type Index: EntryIndex;

    fn by_index(&mut self, index: Self::Index) -> Result<EntryData<'_>>;

    fn by_index_raw(&mut self, index: Self::Index) -> Result<RawEntryData<'_>>;

    fn by_name(&mut self, name: &str) -> Result<Option<EntryData<'_>>>;

    fn by_name_raw(&mut self, name: &str) -> Result<Option<RawEntryData<'_>>>;
}

enum RawEntryDataInner<'a> {
    Slice(Cursor<&'a [u8]>),
    Owned(Cursor<Vec<u8>>),
    Stream(io::Take<&'a mut dyn Read>),
}

impl<'a> RawEntryDataInner<'a> {
    pub fn to_slice(&self) -> Option<&[u8]> {
        match self {
            Self::Slice(buf) => Some(buf.get_ref()),
            Self::Owned(buf) => Some(buf.get_ref()),
            Self::Stream(_) => None,
        }
    }
}

pub struct RawEntryData<'a> {
    inner: RawEntryDataInner<'a>,
}

impl<'a> RawEntryData<'a> {
    pub(crate) fn from_stream(r: io::Take<&'a mut dyn Read>) -> Self {
        Self {
            inner: RawEntryDataInner::Stream(r),
        }
    }

    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> u64 {
        match &self.inner {
            RawEntryDataInner::Slice(buf) => buf.get_ref().len() as u64,
            RawEntryDataInner::Owned(buf) => buf.get_ref().len() as u64,
            RawEntryDataInner::Stream(r) => r.limit(),
        }
    }

    /// Get the raw data as a slice, if available. If the underlying archive
    /// is backed by a reader, this will return None.
    pub fn to_slice(&self) -> Option<&[u8]> {
        self.inner.to_slice()
    }

    /// Get the raw data as an owned vector.
    pub fn into_owned(self) -> Result<Vec<u8>> {
        let len = self.len() as usize;

        match self.inner {
            RawEntryDataInner::Slice(slice) => Ok(slice.into_inner().to_owned()),
            RawEntryDataInner::Owned(vec) => Ok(vec.into_inner()),
            RawEntryDataInner::Stream(mut r) => Ok(read_vec(&mut r, len)?),
        }
    }
}

impl Read for RawEntryData<'_> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let r: &mut dyn Read = match &mut self.inner {
            RawEntryDataInner::Slice(r) => r,
            RawEntryDataInner::Owned(r) => r,
            RawEntryDataInner::Stream(r) => r,
        };
        r.read(buf)
    }
}

enum EntryDataInner<'a> {
    Raw(RawEntryData<'a>),
    Zlib {
        reader: ZlibDecoder<RawEntryData<'a>>,
        uncompressed_len: u32,
    },
    Lz4 {
        reader: FrameDecoder<RawEntryData<'a>>,
        uncompressed_len: u32,
    },
}

pub struct EntryData<'a> {
    inner: EntryDataInner<'a>,
}

impl<'a> EntryData<'a> {
    pub(crate) fn new_uncompressed(raw: RawEntryData<'a>) -> Self {
        Self {
            inner: EntryDataInner::Raw(raw),
        }
    }

    pub(crate) fn new_zlib(raw: RawEntryData<'a>, uncompressed_len: u32) -> Self {
        Self {
            inner: EntryDataInner::Zlib {
                reader: ZlibDecoder::new(raw),
                uncompressed_len,
            },
        }
    }

    pub(crate) fn new_lz4(raw: RawEntryData<'a>, uncompressed_len: u32) -> Self {
        Self {
            inner: EntryDataInner::Lz4 {
                reader: FrameDecoder::new(raw),
                uncompressed_len,
            },
        }
    }

    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> u64 {
        match &self.inner {
            EntryDataInner::Raw(raw) => raw.len(),
            EntryDataInner::Zlib {
                uncompressed_len, ..
            } => *uncompressed_len as u64,
            EntryDataInner::Lz4 {
                uncompressed_len, ..
            } => *uncompressed_len as u64,
        }
    }

    /// Get the raw data as a slice, if available. If the underlying archive
    /// is backed by a reader, or if the data is compressed, this will return None.
    pub fn to_slice(&self) -> Option<&[u8]> {
        match &self.inner {
            EntryDataInner::Raw(raw) => raw.to_slice(),
            _ => None,
        }
    }

    /// Get the data as an owned vector.
    pub fn into_owned(self) -> Result<Vec<u8>> {
        match self.inner {
            EntryDataInner::Raw(raw) => raw.into_owned(),
            EntryDataInner::Zlib {
                mut reader,
                uncompressed_len,
            } => Ok(read_vec(&mut reader, uncompressed_len as usize)?),
            EntryDataInner::Lz4 {
                mut reader,
                uncompressed_len,
            } => Ok(read_vec(&mut reader, uncompressed_len as usize)?),
        }
    }
}

impl Read for EntryData<'_> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let r: &mut dyn Read = match &mut self.inner {
            EntryDataInner::Raw(r) => r,
            EntryDataInner::Zlib { reader, .. } => reader,
            EntryDataInner::Lz4 { reader, .. } => reader,
        };
        r.read(buf)
    }
}
