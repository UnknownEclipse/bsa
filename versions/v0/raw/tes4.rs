use std::{
    convert::{TryFrom, TryInto},
    marker::PhantomData,
};

use bitflags::bitflags;
use bytemuck::{Pod, Zeroable};

use crate::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Version {
    V103 = 103,
    V104 = 104,
    V105 = 105,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Compression {
    Zlib,
    Lz4,
}

pub trait Bsa {
    type FolderRecord: FolderRecord;

    const VERSION: Version;
    const COMPRESSION: Compression;
    const CAN_EMBED_FILENAMES: bool;
}

pub struct Tes4;

impl Bsa for Tes4 {
    type FolderRecord = Tes4FolderRecord;
    const VERSION: Version = Version::V103;
    const COMPRESSION: Compression = Compression::Zlib;
    const CAN_EMBED_FILENAMES: bool = false;
}

pub struct Tes5;

impl Bsa for Tes5 {
    type FolderRecord = Tes4FolderRecord;
    const VERSION: Version = Version::V104;
    const COMPRESSION: Compression = Compression::Zlib;
    const CAN_EMBED_FILENAMES: bool = true;
}

pub struct Sse;

impl Bsa for Sse {
    type FolderRecord = SseFolderRecord;
    const VERSION: Version = Version::V105;
    const COMPRESSION: Compression = Compression::Lz4;
    const CAN_EMBED_FILENAMES: bool = true;
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

#[derive(Debug, Default, Clone, Copy, Pod, Zeroable)]
#[repr(C)]
pub struct RawHeader {
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
    /// Get the header's archive flags.
    pub fn archive_flags(&self) -> ArchiveFlags {
        self.archive_flags
    }

    /// Get the header's folder count.
    pub fn folder_count(&self) -> u32 {
        self.folder_count
    }

    /// Get the header's file count.
    pub fn file_count(&self) -> u32 {
        self.file_count
    }

    /// Get the header's total folder name length.
    pub fn total_folder_name_length(&self) -> u32 {
        self.total_folder_name_length
    }

    /// Get the header's total file name length.
    pub fn total_file_name_length(&self) -> u32 {
        self.total_file_name_length
    }

    /// Get the header's file flags.
    pub fn file_flags(&self) -> FileFlags {
        self.file_flags
    }

    pub fn has_embedded_names(&self) -> bool {
        A::CAN_EMBED_FILENAMES && self.archive_flags().contains(ArchiveFlags::EMBED_FILENAMES)
    }

    pub fn compression(&self) -> Option<Compression> {
        if self.archive_flags().contains(ArchiveFlags::COMPRESSED) {
            Some(A::COMPRESSION)
        } else {
            None
        }
    }

    pub fn include_dirnames(&self) -> bool {
        self.archive_flags()
            .contains(ArchiveFlags::INCLUDE_DIRNAMES)
    }

    pub fn include_filenames(&self) -> bool {
        self.archive_flags()
            .contains(ArchiveFlags::INCLUDE_FILENAMES)
    }
}

impl<A> TryFrom<RawHeader> for Header<A>
where
    A: Bsa,
{
    type Error = Error;

    fn try_from(value: RawHeader) -> Result<Self, Self::Error> {
        if &value.magic != b"BSA\0" {
            return Err(Error::InvalidHeader);
        }
        if u32::from_le_bytes(value.version) != A::VERSION as u32 {
            return Err(Error::InvalidHeader);
        }
        if u32::from_le_bytes(value.offset) != 36 {
            return Err(Error::InvalidHeader);
        }

        let folder_count = u32::from_le_bytes(value.folder_count);
        let file_count = u32::from_le_bytes(value.file_count);
        let total_folder_name_length = u32::from_le_bytes(value.total_folder_name_length);
        let total_file_name_length = u32::from_le_bytes(value.total_file_name_length);

        let archive_flags = u32::from_le_bytes(value.archive_flags);
        let archive_flags = ArchiveFlags::from_bits(archive_flags).ok_or(Error::InvalidHeader)?;

        let file_flags = u16::from_le_bytes(value.file_flags);
        let file_flags = FileFlags::from_bits(file_flags).ok_or(Error::InvalidHeader)?;

        Ok(Header {
            archive_flags,
            total_file_name_length,
            total_folder_name_length,
            folder_count,
            file_count,
            file_flags,
            _marker: Default::default(),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Pod, Zeroable)]
#[repr(transparent)]
pub struct Hash([u8; 8]);

impl Hash {
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        todo!()
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

#[derive(Debug, Clone, Copy, Pod, Zeroable)]
#[repr(C)]
pub struct Tes4FolderRecord {
    hash: Hash,
    count: [u8; 4],
    offset: [u8; 4],
}

pub trait FolderRecord: Pod {
    fn new(hash: Hash, count: u32, offset: u32) -> Self;
    fn hash(&self) -> Hash;
    fn count(&self) -> u32;
    fn offset(&self) -> u32;
}

impl FolderRecord for Tes4FolderRecord {
    #[inline]
    fn new(hash: Hash, count: u32, offset: u32) -> Self {
        Self {
            hash,
            count: count.to_le_bytes(),
            offset: offset.to_le_bytes(),
        }
    }

    #[inline]
    fn hash(&self) -> Hash {
        self.hash
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

#[derive(Debug, Clone, Copy, Pod, Zeroable)]
#[repr(C)]
pub struct SseFolderRecord {
    hash: Hash,
    count: [u8; 4],
    _pad1: [u8; 4],
    offset: [u8; 4],
    _pad2: [u8; 4],
}

impl FolderRecord for SseFolderRecord {
    #[inline]
    fn new(hash: Hash, count: u32, offset: u32) -> Self {
        Self {
            hash,
            count: count.to_le_bytes(),
            offset: offset.to_le_bytes(),
            _pad1: Default::default(),
            _pad2: Default::default(),
        }
    }

    #[inline]
    fn hash(&self) -> Hash {
        self.hash
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
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
#[repr(C)]
pub struct FileRecord {
    hash: Hash,
    size: [u8; 4],
    offset: [u8; 4],
}

impl FileRecord {
    #[inline]
    pub fn new(hash: Hash, size: u32, offset: u32) -> Self {
        Self {
            hash,
            size: size.to_le_bytes(),
            offset: offset.to_le_bytes(),
        }
    }

    #[inline]
    pub fn hash(&self) -> Hash {
        self.hash
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

const INVERT_COMPRESSION: u32 = 1 << 30;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Pod, Zeroable)]
#[repr(transparent)]
pub struct FileSize([u8; 4]);

impl FileSize {
    pub fn new(len: usize, invert_compression: bool) -> Option<Self> {
        let value: u32 = len.try_into().ok()?;
        if value & INVERT_COMPRESSION != 0 {
            None
        } else {
            Some(Self((value | INVERT_COMPRESSION).to_le_bytes()))
        }
    }

    #[inline]
    pub fn compression_inverted(&self) -> bool {
        let value = u32::from_le_bytes(self.0);
        value & INVERT_COMPRESSION != 0
    }

    #[inline]
    pub fn get(&self) -> u32 {
        let value = u32::from_le_bytes(self.0);
        value & !INVERT_COMPRESSION
    }
}
