use std::{
    convert::TryFrom,
    num::{NonZeroU32, NonZeroU64},
};

use bytemuck::{Pod, Zeroable};

use crate::ReadError;

pub const BA2_MAGIC: [u8; 4] = *b"BTDX";
pub const GENERAL_CHUNK_SIZE: u16 = 0x10;
pub const DX10_CHUNK_SIZE: u16 = 0x18;
pub const CHUNK_DATA_SENTINEL: u32 = 0xBAADF00D;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Version {
    V1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Format {
    General,
    DirectX,
}

#[derive(Debug, Clone, Copy, Zeroable, Pod)]
#[repr(C)]
pub struct RawHeader {
    magic: [u8; 4],
    version: [u8; 4],
    format: [u8; 4],
    file_count: [u8; 4],
    string_table_offset: [u8; 8],
}

impl From<Header> for RawHeader {
    fn from(header: Header) -> Self {
        let magic = BA2_MAGIC;
        let version = (header.version as u32).to_le_bytes();
        let format = *match header.format {
            Format::General => b"GNRL",
            Format::DirectX => b"DX10",
        };
        let file_count = header.file_count.to_le_bytes();
        let string_table_offset = header.string_table_offset.map(|off| off.get()).unwrap_or(0);
        let string_table_offset = string_table_offset.to_le_bytes();

        RawHeader {
            magic,
            version,
            format,
            file_count,
            string_table_offset,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Header {
    pub version: Version,
    pub format: Format,
    pub file_count: u32,
    pub string_table_offset: Option<NonZeroU64>,
}

impl TryFrom<RawHeader> for Header {
    type Error = ReadError;

    fn try_from(header: RawHeader) -> Result<Self, Self::Error> {
        if header.magic != BA2_MAGIC {
            return Err(ReadError::InvalidMagic(header.magic));
        }

        let version = u32::from_le_bytes(header.version);
        let version = match version {
            1 => Version::V1,
            _ => return Err(ReadError::InvalidVersion(version)),
        };

        let format = match &header.format {
            b"GNRL" => Format::General,
            b"DX10" => Format::DirectX,
            _ => return Err(ReadError::UnsupportedFormat(header.format)),
        };

        let file_count = u32::from_le_bytes(header.file_count);

        let string_table_offset = u64::from_le_bytes(header.string_table_offset);
        let string_table_offset = NonZeroU64::new(string_table_offset);

        Ok(Header {
            version,
            format,
            file_count,
            string_table_offset,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Zeroable, Pod)]
#[repr(transparent)]
pub struct DataFileIndex(u8);

#[derive(Debug, Clone, Copy, Zeroable, Pod)]
#[repr(C)]
pub struct RawGeneralChunkHeader {
    id: Hash,
    data_file_index: u8,
    chunk_count: u8,
    chunk_size: [u8; 2],
}

impl From<GeneralChunkHeader> for RawGeneralChunkHeader {
    fn from(header: GeneralChunkHeader) -> Self {
        RawGeneralChunkHeader {
            id: header.id,
            data_file_index: header.data_file_index.0,
            chunk_count: header.chunk_count,
            chunk_size: GENERAL_CHUNK_SIZE.to_le_bytes(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct GeneralChunkHeader {
    pub id: Hash,
    pub data_file_index: DataFileIndex,
    pub chunk_count: u8,
}

impl TryFrom<RawGeneralChunkHeader> for GeneralChunkHeader {
    type Error = ReadError;

    fn try_from(header: RawGeneralChunkHeader) -> Result<Self, Self::Error> {
        let chunk_size = u16::from_le_bytes(header.chunk_size);
        if chunk_size != GENERAL_CHUNK_SIZE {
            Err(ReadError::InvalidChunkSize(chunk_size, Format::General))
        } else {
            Ok(GeneralChunkHeader {
                id: header.id,
                data_file_index: DataFileIndex(header.data_file_index),
                chunk_count: header.chunk_count,
            })
        }
    }
}

#[derive(Debug, Clone, Copy, Zeroable, Pod)]
#[repr(C)]
pub struct RawDirectXChunkHeader {
    id: Hash,
    data_file_index: u8,
    chunk_count: u8,
    chunk_size: [u8; 2],
    height: [u8; 2],
    width: [u8; 2],
    mip_count: u8,
    format: u8,
    flags: u8,
    tile_mode: u8,
}

impl From<DirectXChunkHeader> for RawDirectXChunkHeader {
    fn from(header: DirectXChunkHeader) -> Self {
        RawDirectXChunkHeader {
            id: header.id,
            data_file_index: header.data_file_index.0,
            chunk_count: header.chunk_count,
            chunk_size: DX10_CHUNK_SIZE.to_le_bytes(),
            height: header.height.to_le_bytes(),
            width: header.width.to_le_bytes(),
            mip_count: header.mip_count,
            format: header.format,
            flags: header.flags,
            tile_mode: header.tile_mode,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct DirectXChunkHeader {
    pub id: Hash,
    pub data_file_index: DataFileIndex,
    pub chunk_count: u8,
    pub height: u16,
    pub width: u16,
    pub mip_count: u8,
    pub format: u8,
    pub flags: u8,
    pub tile_mode: u8,
}

impl TryFrom<RawDirectXChunkHeader> for DirectXChunkHeader {
    type Error = ReadError;

    fn try_from(header: RawDirectXChunkHeader) -> Result<Self, Self::Error> {
        let chunk_size = u16::from_le_bytes(header.chunk_size);
        if chunk_size != DX10_CHUNK_SIZE {
            Err(ReadError::InvalidChunkSize(chunk_size, Format::General))
        } else {
            Ok(DirectXChunkHeader {
                id: header.id,
                data_file_index: DataFileIndex(header.data_file_index),
                chunk_count: header.chunk_count,
                height: u16::from_le_bytes(header.height),
                width: u16::from_le_bytes(header.width),
                mip_count: header.mip_count,
                format: header.format,
                flags: header.flags,
                tile_mode: header.tile_mode,
            })
        }
    }
}

#[derive(Debug, Clone, Copy, Zeroable, Pod)]
#[repr(C)]
pub struct RawGeneralChunkData {
    data_file_offset: [u8; 8],
    compressed_size: [u8; 4],
    decompressed_size: [u8; 4],
    sentinel: [u8; 4],
}

impl From<GeneralChunkData> for RawGeneralChunkData {
    fn from(data: GeneralChunkData) -> Self {
        let data_file_offset = data.data_file_offset.to_le_bytes();
        let compressed_size = data
            .compressed_size
            .map(|val| val.get())
            .unwrap_or(0)
            .to_le_bytes();
        let decompressed_size = data.decompressed_size.to_le_bytes();
        let sentinel = CHUNK_DATA_SENTINEL.to_le_bytes();

        RawGeneralChunkData {
            data_file_offset,
            compressed_size,
            decompressed_size,
            sentinel,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct GeneralChunkData {
    pub data_file_offset: u64,
    pub compressed_size: Option<NonZeroU32>,
    pub decompressed_size: u32,
}

impl TryFrom<RawGeneralChunkData> for GeneralChunkData {
    type Error = ReadError;

    fn try_from(data: RawGeneralChunkData) -> Result<Self, Self::Error> {
        let sentinel = u32::from_le_bytes(data.sentinel);
        if sentinel != CHUNK_DATA_SENTINEL {
            Err(ReadError::InvalidChunkSentinel(sentinel))
        } else {
            Ok(GeneralChunkData {
                data_file_offset: u64::from_le_bytes(data.data_file_offset),
                compressed_size: NonZeroU32::new(u32::from_le_bytes(data.compressed_size)),
                decompressed_size: u32::from_le_bytes(data.decompressed_size),
            })
        }
    }
}

#[derive(Debug, Clone, Copy, Zeroable, Pod)]
#[repr(C)]
pub struct RawDirectXChunkData {
    data_file_offset: [u8; 8],
    compressed_size: [u8; 4],
    decompressed_size: [u8; 4],
    mip_first: [u8; 2],
    mip_last: [u8; 2],
    sentinel: [u8; 4],
}

impl From<DirectXChunkData> for RawDirectXChunkData {
    fn from(data: DirectXChunkData) -> Self {
        let data_file_offset = data.data_file_offset.to_le_bytes();
        let compressed_size = data
            .compressed_size
            .map(|val| val.get())
            .unwrap_or(0)
            .to_le_bytes();
        let decompressed_size = data.decompressed_size.to_le_bytes();
        let sentinel = CHUNK_DATA_SENTINEL.to_le_bytes();

        RawDirectXChunkData {
            data_file_offset,
            compressed_size,
            decompressed_size,
            mip_first: data.mip_first.to_le_bytes(),
            mip_last: data.mip_last.to_le_bytes(),
            sentinel,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct DirectXChunkData {
    pub data_file_offset: u64,
    pub compressed_size: Option<NonZeroU32>,
    pub decompressed_size: u32,
    pub mip_first: u16,
    pub mip_last: u16,
}

impl TryFrom<RawDirectXChunkData> for DirectXChunkData {
    type Error = ReadError;

    fn try_from(data: RawDirectXChunkData) -> Result<Self, Self::Error> {
        let sentinel = u32::from_le_bytes(data.sentinel);
        if sentinel != CHUNK_DATA_SENTINEL {
            Err(ReadError::InvalidChunkSentinel(sentinel))
        } else {
            Ok(DirectXChunkData {
                data_file_offset: u64::from_le_bytes(data.data_file_offset),
                compressed_size: NonZeroU32::new(u32::from_le_bytes(data.compressed_size)),
                decompressed_size: u32::from_le_bytes(data.decompressed_size),
                mip_first: u16::from_le_bytes(data.mip_first),
                mip_last: u16::from_le_bytes(data.mip_last),
            })
        }
    }
}

#[derive(Debug, Clone, Copy, Zeroable, Pod)]
#[repr(C)]
pub struct Hash {
    file: [u8; 4],
    extension: [u8; 4],
    directory: [u8; 4],
}

impl Hash {
    // pub unsafe fn from_filename_bytes(bytes: &[u8]) -> Hash {}

    pub fn file(&self) -> u32 {
        u32::from_le_bytes(self.file)
    }

    pub fn extension(&self) -> u32 {
        u32::from_le_bytes(self.extension)
    }

    pub fn directory(&self) -> u32 {
        u32::from_le_bytes(self.directory)
    }
}

pub mod path {
    //! This module implements path manipulation routines for archive paths.
    //!
    //! # The Format
    //! Archive paths use a very simplistic format. These routines are unsafe to ensure
    //! the user is aware that bad things will happen when using non-validated paths.
    //!
    //! The basic path format is as follows:
    //! 1. All paths are encoded in the Windows-1252 encoding (represented as byte
    //! slices).
    //! 2. All paths use the backslash separator.
    //! 3. All paths are case-insensitive.
    //! 4. Paths may not contain a nul byte.
    //! 5. Paths must be relative and may not contain any directory traversal components
    //! such as '..' or '.'.
    //! 6. Paths may not be longer than Windows' MAX_PATH. (260 bytes)
    //!
    //! # Important Notes
    //! The path format used by Bethesda is very rudimentary. As such, these routines
    //! are unsafe when used with paths that are not in the correct format.
    //! To normalize a path, use the [normalize] function.

    use std::path::{Component, Path};

    pub const MAX_PATH: usize = 255;

    pub fn normalize<P: AsRef<Path>>(path: P) -> Option<Vec<u8>> {
        fn inner(path: &Path) -> Option<Vec<u8>> {
            let mut buf = Vec::new();

            for component in path.components() {
                let component = match component {
                    Component::Normal(s) => s.to_str()?,
                    _ => return None,
                };

                if !buf.is_empty() {
                    buf.push(b'\\');
                }
                for ch in component.chars() {
                    buf.push(windows_1252::encode(ch).ok()?);
                }
            }

            if MAX_PATH <= buf.len() {
                None
            } else {
                Some(buf)
            }
        }
        inner(path.as_ref())
    }

    /// # Safety
    /// 1. The path must be normalized.
    pub unsafe fn split(path: &[u8]) -> (&[u8], &[u8]) {
        let mut i = path.len();
        while 0 < i {
            if path[i - 1] == b'\\' {
                let parent = &path[..i - 1];
                let name = &path[i..];
                return (parent, name);
            }
            i -= 1;
        }
        (b"", path)
    }

    /// # Safety
    /// 1. The path may not contain any separators and must be in the Bethesda path
    /// format described in [self].
    pub unsafe fn split_extension(name: &[u8]) -> (&[u8], &[u8]) {
        let mut i = name.len();
        while 0 < i {
            if name[i - 1] == b'.' {
                return name.split_at(i - 1);
            }
            i -= 1;
        }
        (name, b"")
    }

    #[cfg(test)]
    mod tests {
        use crate::raw::path::split_extension;

        use super::split;

        #[test]
        fn test_split() {
            type BStr = &'static [u8];
            type Case = (BStr, (BStr, BStr));
            let cases: &[Case] = &[
                (b"", (b"", b"")),
                (b"hello", (b"", b"hello")),
                (b"textures\\dirt.dds", (b"textures", b"dirt.dds")),
                (
                    b"textures\\ground\\dirt.dds",
                    (b"textures\\ground", b"dirt.dds"),
                ),
            ];
            for &(path, result) in cases {
                assert_eq!(unsafe { split(path) }, result);
            }
        }

        #[test]
        fn test_split_extension() {
            type BStr = &'static [u8];
            type Case = (BStr, (BStr, BStr));
            let cases: &[Case] = &[
                (b"", (b"", b"")),
                (b"hello", (b"hello", b"")),
                (b"dirt.dds", (b"dirt", b".dds")),
            ];
            for &(path, result) in cases {
                assert_eq!(unsafe { split_extension(path) }, result);
            }
        }
    }
}
