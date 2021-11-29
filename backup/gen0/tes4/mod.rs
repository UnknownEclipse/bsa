use std::{
    cmp::Ordering,
    convert::{TryFrom, TryInto},
    fmt::Debug,
    marker::PhantomData,
};

use bitflags::bitflags;
use bytemuck::{Pod, Zeroable};
use thiserror::Error;

use crate::common::Sealed;

mod archive;
pub mod bsa;
mod writer;

pub use bsa::RawBsa;

use writer::BsaWriter;

pub type Tes4Archive<R> = archive::BsaArchive<Tes4, R>;
pub type Tes5Archive<R> = archive::BsaArchive<Tes5, R>;
pub type SseArchive<R> = archive::BsaArchive<Sse, R>;
pub type FnvArchive<R> = Tes5Archive<R>;
pub type Fo3Archive<R> = Tes5Archive<R>;

pub type Tes4Writer = BsaWriter<Tes4>;
pub type Tes5Writer = BsaWriter<Tes5>;
pub type SseWriter = BsaWriter<Sse>;
pub type FnvWriter = Tes5Writer;
pub type Fo3Writer = Tes5Writer;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Compression {
    Zlib,
    Lz4,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Version {
    V103 = 103,
    V104 = 104,
    V105 = 105,
}

impl Version {
    pub fn compression(&self) -> Compression {
        match self {
            Version::V103 | Version::V104 => Compression::Zlib,
            Version::V105 => Compression::Lz4,
        }
    }

    pub fn can_embed_filenames(&self) -> bool {
        matches!(self, Version::V104 | Version::V105)
    }
}

pub trait FolderRecord: Pod + Debug {
    fn new(hash: Hash, count: u32, offset: u32) -> Self;
    fn hash(&self) -> Hash;
    fn count(&self) -> u32;
    fn offset(&self) -> u32;
}

#[derive(Debug, Clone, Copy, Zeroable, Pod)]
#[repr(C)]
pub struct FileRecord {
    hash: [u8; 8],
    size: [u8; 4],
    offset: [u8; 4],
}

impl FileRecord {
    const NEGATE_COMPRESSION: u32 = 1 << 30;

    #[inline]
    pub fn new(hash: Hash, mut size: u32, offset: u32, negate_compression: bool) -> FileRecord {
        assert_eq!(size & Self::NEGATE_COMPRESSION, 0, "invalid file size");

        if negate_compression {
            size &= Self::NEGATE_COMPRESSION;
        }

        let hash = hash.to_bytes();
        let size = size.to_le_bytes();
        let offset = offset.to_le_bytes();

        FileRecord { hash, size, offset }
    }

    #[inline]
    pub fn hash(&self) -> Hash {
        Hash::from_bytes(self.hash)
    }

    #[inline]
    pub fn size(&self) -> u32 {
        u32::from_le_bytes(self.size) & !Self::NEGATE_COMPRESSION
    }

    #[inline]
    pub fn negate_compression(&self) -> bool {
        u32::from_le_bytes(self.size) & Self::NEGATE_COMPRESSION != 0
    }

    #[inline]
    pub fn offset(&self) -> u32 {
        u32::from_le_bytes(self.offset)
    }
}

#[derive(Debug, Clone, Copy, Zeroable, Pod)]
#[repr(C)]
pub struct Tes4FolderRecord {
    hash: [u8; 8],
    count: [u8; 4],
    offset: [u8; 4],
}

impl FolderRecord for Tes4FolderRecord {
    #[inline]
    fn new(hash: Hash, count: u32, offset: u32) -> Tes4FolderRecord {
        let hash = hash.to_bytes();
        let count = count.to_le_bytes();
        let offset = offset.to_le_bytes();

        Tes4FolderRecord {
            hash,
            count,
            offset,
        }
    }

    #[inline]
    fn hash(&self) -> Hash {
        Hash::from_bytes(self.hash)
    }

    #[inline]
    fn count(&self) -> u32 {
        u32::from_le_bytes(self.count)
    }

    #[inline]
    fn offset(&self) -> u32 {
        u32::from_le_bytes(self.offset)
    }
}

#[derive(Debug, Clone, Copy, Zeroable, Pod)]
#[repr(C)]
pub struct SseFolderRecord {
    hash: [u8; 8],
    count: [u8; 4],
    _pad1: [u8; 4],
    offset: [u8; 4],
    _pad2: [u8; 4],
}

impl FolderRecord for SseFolderRecord {
    #[inline]
    fn new(hash: Hash, count: u32, offset: u32) -> SseFolderRecord {
        let hash = hash.to_bytes();
        let count = count.to_le_bytes();
        let offset = offset.to_le_bytes();

        SseFolderRecord {
            hash,
            count,
            _pad1: [0; 4],
            offset,
            _pad2: [0; 4],
        }
    }

    #[inline]
    fn hash(&self) -> Hash {
        Hash::from_bytes(self.hash)
    }

    #[inline]
    fn count(&self) -> u32 {
        u32::from_le_bytes(self.count)
    }

    #[inline]
    fn offset(&self) -> u32 {
        u32::from_le_bytes(self.offset)
    }
}

bitflags! {
    pub struct ArchiveFlags: u32 {
        const INCLUDE_DIRNAMES = 0x1;
        const INCLUDE_FILENAMES = 0x2;
        const COMPRESSED = 0x4;
        const RETAIN_DIRNAMES = 0x8;
        const RETAIN_FILENAMES = 0x10;
        const RETAIN_FILENAME_OFFSETS = 0x20;
        const XBOX360 = 0x40;
        const RETAIN_STRINGS = 0x80;
        const EMBED_FILENAMES = 0x100;
        const XMEM = 0x200;
    }
}

bitflags! {
    #[derive(Default)]
    pub struct FileFlags: u16 {
        const MESHES = 0x1;
        const TEXTURES = 0x2;
        const MENUS = 0x4;
        const SOUNDS = 0x8;
        const VOICES = 0x10;
        const SHADERS = 0x20;
        const TREES = 0x40;
        const FONTS = 0x80;
        const MISC = 0x100;
    }
}

pub trait Bsa: Sealed {
    type FolderRecord: FolderRecord;
    // type ArchiveFlags: ArchiveFlags;

    const CAN_EMBED_FILENAMES: bool;
    const COMPRESSION: Compression;
    const VERSION: Version;
}

pub struct Tes4;

impl Sealed for Tes4 {}

impl Bsa for Tes4 {
    type FolderRecord = Tes4FolderRecord;
    // type ArchiveFlags = Tes4ArchiveFlags;

    const CAN_EMBED_FILENAMES: bool = false;
    const COMPRESSION: Compression = Compression::Zlib;
    const VERSION: Version = Version::V103;
}

pub struct Tes5;

impl Sealed for Tes5 {}

impl Bsa for Tes5 {
    type FolderRecord = Tes4FolderRecord;
    // type ArchiveFlags = Tes5ArchiveFlags;

    const CAN_EMBED_FILENAMES: bool = true;
    const COMPRESSION: Compression = Compression::Zlib;
    const VERSION: Version = Version::V104;
}

pub struct Sse;

impl Sealed for Sse {}

impl Bsa for Sse {
    type FolderRecord = SseFolderRecord;
    // type ArchiveFlags = Tes5ArchiveFlags;

    const CAN_EMBED_FILENAMES: bool = true;
    const COMPRESSION: Compression = Compression::Lz4;
    const VERSION: Version = Version::V105;
}

// #[derive(Debug, Clone, Copy, PartialEq, Eq)]
// #[repr(transparent)]
// pub struct Hash(u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(C)]
pub struct Hash {
    last: u8,
    last2: u8,
    len: u8,
    first: u8,
    crc: u32,
}

fn crc32(bytes: &[u8]) -> u32 {
    const K: u32 = 0x1003f;
    let mut crc: u32 = 0;
    for &byte in bytes {
        crc = crc.wrapping_mul(K) + byte as u32;
    }
    crc
}

impl Hash {
    pub fn from_dirname(name: &[u8]) -> Option<Hash> {
        if name.len() >= u8::MAX as usize || !name.is_ascii() || name.contains(&b'\0') {
            return None;
        }

        let mut last = 0;
        let mut last2 = 0;
        let mut first = 0;
        let mut crc = 0;

        if 3 <= name.len() {
            last2 = name[name.len() - 2];
            crc = crc32(&name[1..name.len() - 2])
        }
        if !name.is_empty() {
            first = name[0];
            last = name[name.len() - 1];
        }

        let len = name.len() as u8;
        // let crc =if 3 <= name.len() crc32(&name[1..name.len() - 2]);

        Some(Hash {
            last,
            last2,
            len,
            first,
            crc,
        })
    }

    pub fn from_filename(base_name: &[u8]) -> Option<Hash> {
        let (stem, extension) = path::split_extension(base_name);

        if !stem.is_empty() && stem.len() < 260 && extension.len() < 16 {
            let mut h = Hash::from_dirname(stem).unwrap();
            h.crc = h.crc.wrapping_add(crc32(extension));

            let i = match extension {
                b"" => Some(0u8),
                b".nif" => Some(1),
                b".kf" => Some(2),
                b".dds" => Some(3),
                b".wav" => Some(4),
                b".adp" => Some(5),
                _ => None,
            };

            if let Some(i) = i {
                h.first += 32 * (i & 0xfc);
                h.last += (i & 0xfe).wrapping_shl(6);
                h.last2 += i.wrapping_shl(7);
            }

            Some(h)
        } else {
            None
        }
    }

    #[inline]
    pub fn from_bytes(bytes: [u8; 8]) -> Hash {
        let last = bytes[0];
        let last2 = bytes[1];
        let len = bytes[2];
        let first = bytes[3];
        let crc = bytes[4..8].try_into().unwrap();
        let crc = u32::from_le_bytes(crc);
        Hash {
            last,
            last2,
            len,
            first,
            crc,
        }
    }

    pub fn to_bytes(self) -> [u8; 8] {
        let crc = self.crc.to_le_bytes();
        [
            self.last, self.last2, self.len, self.first, crc[0], crc[1], crc[2], crc[3],
        ]
    }

    #[inline]
    fn to_u64(self) -> u64 {
        u64::from_le_bytes(self.to_bytes())
    }
}

impl PartialOrd for Hash {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.to_u64().partial_cmp(&other.to_u64())
    }
}

impl Ord for Hash {
    fn cmp(&self, other: &Self) -> Ordering {
        self.to_u64().cmp(&other.to_u64())
    }
}

#[derive(Debug, Clone, Copy, Default, Zeroable, Pod)]
#[repr(C)]
struct RawHeader {
    magic: [u8; 4],
    version: [u8; 4],
    offset: [u8; 4],
    archive_flags: [u8; 4],
    folder_count: [u8; 4],
    file_count: [u8; 4],
    total_folder_name_length: [u8; 4],
    total_file_name_length: [u8; 4],
    file_flags: [u8; 2],
    _pad: [u8; 2],
}

#[derive(Debug, Clone, Copy)]
pub struct Header<A>
where
    A: Bsa,
{
    archive_flags: ArchiveFlags,
    folder_count: u32,
    file_count: u32,
    total_folder_name_length: u32,
    total_file_name_length: u32,
    file_flags: FileFlags,
    _marker: PhantomData<A>,
}

impl<A> Header<A>
where
    A: Bsa,
{
    pub fn include_dirnames(&self) -> bool {
        self.archive_flags.contains(ArchiveFlags::INCLUDE_DIRNAMES)
    }

    pub fn include_filenames(&self) -> bool {
        self.archive_flags.contains(ArchiveFlags::INCLUDE_FILENAMES)
    }

    pub fn compressed(&self) -> bool {
        self.archive_flags.contains(ArchiveFlags::COMPRESSED)
    }

    pub fn embed_filenames(&self) -> bool {
        A::CAN_EMBED_FILENAMES && self.archive_flags.contains(ArchiveFlags::EMBED_FILENAMES)
    }
}

impl<A> From<Header<A>> for RawHeader
where
    A: Bsa,
{
    fn from(header: Header<A>) -> Self {
        RawHeader {
            magic: *b"BSA\0",
            version: (A::VERSION as u32).to_le_bytes(),
            offset: 36u32.to_le_bytes(),
            archive_flags: header.archive_flags.bits().to_le_bytes(),
            folder_count: header.folder_count.to_le_bytes(),
            file_count: header.file_count.to_le_bytes(),
            total_folder_name_length: header.total_folder_name_length.to_le_bytes(),
            total_file_name_length: header.total_file_name_length.to_le_bytes(),
            file_flags: header.file_flags.bits().to_le_bytes(),
            _pad: Default::default(),
        }
    }
}

#[derive(Debug, Error)]
#[error("invalid archive header")]
pub struct TryFromRawHeaderError;

impl<A> TryFrom<RawHeader> for Header<A>
where
    A: Bsa,
{
    type Error = TryFromRawHeaderError;

    fn try_from(value: RawHeader) -> Result<Self, Self::Error> {
        if &value.magic != b"BSA\0" {
            return Err(TryFromRawHeaderError);
        }

        let version = u32::from_le_bytes(value.version);
        if version != A::VERSION as u32 {
            return Err(TryFromRawHeaderError);
        }

        let offset = u32::from_le_bytes(value.offset);
        if offset != 36 {
            return Err(TryFromRawHeaderError);
        }

        let archive_flags = u32::from_le_bytes(value.archive_flags);
        let archive_flags = ArchiveFlags::from_bits(archive_flags).ok_or(TryFromRawHeaderError)?;

        let folder_count = u32::from_le_bytes(value.folder_count);
        let file_count = u32::from_le_bytes(value.file_count);
        let total_folder_name_length = u32::from_le_bytes(value.total_folder_name_length);
        let total_file_name_length = u32::from_le_bytes(value.total_file_name_length);

        let file_flags = u16::from_le_bytes(value.file_flags);
        let file_flags = FileFlags::from_bits(file_flags).ok_or(TryFromRawHeaderError)?;

        Ok(Header {
            archive_flags,
            folder_count,
            file_count,
            total_file_name_length,
            total_folder_name_length,
            file_flags,
            _marker: Default::default(),
        })
    }
}

mod path {
    use std::path::{Component, Path};

    use memchr::memchr;

    use crate::{common::windows_1252, ArchiveWriteError, Result};

    #[inline]
    fn rsplit_once(bytes: &[u8], sep: u8) -> (&[u8], Option<&[u8]>) {
        for i in (0..bytes.len()).rev() {
            if bytes[i] == sep {
                let (dir, file) = bytes.split_at(i);
                return (dir, Some(&file[1..]));
            }
        }
        (bytes, None)
    }

    /// Splits a path into a (parent, file name) pair. If the path has no file name, returns
    /// (path, None).
    ///
    /// This routine will only work correctly on paths that have been normalized by the
    /// [normalize] function.
    pub fn split(bytes: &[u8]) -> (&[u8], Option<&[u8]>) {
        rsplit_once(bytes, b'\\')
    }

    /// Splits a path into a (file name, extensino) pair. If the file has no extension,
    /// returns (file name, None).
    ///
    /// This routine will only work correctly on paths that have been normalized by the
    /// [normalize] function.
    pub fn split_extension(bytes: &[u8]) -> (&[u8], &[u8]) {
        if let Some(i) = memchr(b'.', bytes) {
            bytes.split_at(i)
        } else {
            (bytes, b"")
        }
    }

    pub fn normalize(path: &Path) -> Result<Vec<u8>> {
        fn inner(path: &Path) -> Option<Vec<u8>> {
            let mut bytes = Vec::new();
            for component in path.components() {
                if let Component::Normal(s) = component {
                    let s = s.to_str()?;
                    if !bytes.is_empty() {
                        bytes.push(b'\\');
                    }
                    for ch in s.chars() {
                        let byte = windows_1252::encode(ch)?;
                        if byte == b'\0' {
                            return None;
                        }
                        let byte = windows_1252::to_lowercase(byte);
                        bytes.push(byte);
                    }
                } else {
                    return None;
                }
            }

            if u8::MAX as usize <= bytes.len() {
                return None;
            }

            Some(bytes)
        }

        Ok(inner(path).ok_or(ArchiveWriteError::InvalidFileName)?)
    }
}

#[cfg(test)]
mod tests;
