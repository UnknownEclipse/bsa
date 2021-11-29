use std::{
    convert::TryFrom,
    fmt::Debug,
    num::{NonZeroU32, NonZeroU64},
};

use bytemuck::{Pod, Zeroable};

use crate::ArchiveReadError;

pub mod ba2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Format {
    General,
    Dx10,
}

#[derive(Debug, Clone, Copy, Zeroable, Pod)]
#[repr(C)]
struct RawHeader {
    magic: [u8; 4],
    version: [u8; 4],
    format: [u8; 4],
    file_count: [u8; 4],
    string_table_offset: [u8; 8],
}

#[derive(Debug, Clone, Copy)]
struct Header {
    format: Format,
    file_count: u32,
    string_table_offset: Option<NonZeroU64>,
}

impl TryFrom<RawHeader> for Header {
    type Error = ArchiveReadError;

    fn try_from(value: RawHeader) -> Result<Self, Self::Error> {
        if &value.magic != b"BTDX" {
            return Err(ArchiveReadError::InvalidMagic);
        }

        let version = u32::from_le_bytes(value.version);
        if version != 1 {
            return Err(ArchiveReadError::InvalidVersion);
        }

        let format = match &value.format {
            b"GNRL" => Format::General,
            b"DX10" => Format::Dx10,
            _ => return Err(ArchiveReadError::UnsupportedFormat),
        };

        let file_count = u32::from_le_bytes(value.file_count);
        let string_table_offset = u64::from_le_bytes(value.string_table_offset);
        let string_table_offset = NonZeroU64::try_from(string_table_offset).ok();

        Ok(Header {
            file_count,
            format,
            string_table_offset,
        })
    }
}

#[derive(Debug, Clone, Copy, Zeroable, Pod)]
#[repr(C)]
struct ChunkHeaderHead {
    id: Hash,
    data_file_index: u8,
    chunk_count: u8,
    chunk_size: u16,
}

#[derive(Debug, Clone, Copy, Zeroable, Pod)]
#[repr(C)]
struct GeneralChunkHeader {
    head: ChunkHeaderHead,
}

#[derive(Debug, Clone, Copy, Zeroable, Pod)]
#[repr(C)]
struct Dx10ChunkHeader {
    head: ChunkHeaderHead,
    height: [u8; 2],
    width: [u8; 2],
    mip_count: u8,
    format: u8,
    flags: u8,
    tile_mode: u8,
}

#[derive(Debug, Clone, Copy, Zeroable, Pod)]
#[repr(C)]
struct GeneralChunkData {
    data_file_offset: [u8; 8],
    compressed_size: [u8; 4],
    uncompressed_size: [u8; 4],
    sentinel: [u8; 4],
}

#[derive(Debug, Clone, Copy, Zeroable, Pod)]
#[repr(C)]
struct Dx10ChunkData {
    data_file_offset: [u8; 8],
    compressed_size: [u8; 4],
    uncompressed_size: [u8; 4],
    mip_first: [u8; 2],
    mip_last: [u8; 2],
    sentinel: [u8; 4],
}

#[derive(Debug, Clone, Copy, Zeroable, Pod)]
#[repr(C)]
struct Hash {
    file: [u8; 4],
    ext: [u8; 4],
    dir: [u8; 4],
}

trait ChunkHeader: Pod + Debug {
    fn id(&self) -> Hash;
    fn data_file_index(&self) -> u8;
    fn chunk_count(&self) -> u8;
    fn chunk_size(&self) -> u16;
}

trait ChunkData: Pod + Debug {
    fn data_file_offset(&self) -> u64;
    fn compressed_size(&self) -> Option<NonZeroU32>;
    fn uncompressed_size(&self) -> u32;
    fn sentinel(&self) -> u32;
}

impl ChunkHeader for GeneralChunkHeader {
    fn id(&self) -> Hash {
        self.head.id
    }

    fn data_file_index(&self) -> u8 {
        self.head.data_file_index
    }

    fn chunk_count(&self) -> u8 {
        self.head.chunk_count
    }

    fn chunk_size(&self) -> u16 {
        self.head.chunk_size
    }
}

impl ChunkHeader for Dx10ChunkHeader {
    fn id(&self) -> Hash {
        self.head.id
    }

    fn data_file_index(&self) -> u8 {
        self.head.data_file_index
    }

    fn chunk_count(&self) -> u8 {
        self.head.chunk_count
    }

    fn chunk_size(&self) -> u16 {
        self.head.chunk_size
    }
}

impl ChunkData for GeneralChunkData {
    fn data_file_offset(&self) -> u64 {
        u64::from_le_bytes(self.data_file_offset)
    }

    fn compressed_size(&self) -> Option<NonZeroU32> {
        let size = u32::from_le_bytes(self.compressed_size);
        if size == 0 {
            None
        } else {
            Some(NonZeroU32::try_from(size).unwrap())
        }
    }

    fn uncompressed_size(&self) -> u32 {
        u32::from_le_bytes(self.uncompressed_size)
    }

    fn sentinel(&self) -> u32 {
        u32::from_le_bytes(self.sentinel)
    }
}

impl ChunkData for Dx10ChunkData {
    fn data_file_offset(&self) -> u64 {
        u64::from_le_bytes(self.data_file_offset)
    }

    fn compressed_size(&self) -> Option<NonZeroU32> {
        let size = u32::from_le_bytes(self.compressed_size);
        if size == 0 {
            None
        } else {
            Some(NonZeroU32::try_from(size).unwrap())
        }
    }

    fn uncompressed_size(&self) -> u32 {
        u32::from_le_bytes(self.uncompressed_size)
    }

    fn sentinel(&self) -> u32 {
        u32::from_le_bytes(self.sentinel)
    }
}
