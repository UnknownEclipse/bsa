use std::io;

use raw::Format;
use thiserror::Error;

mod chunk_data;
mod common;
mod raw;
mod read;

pub use read::{
    Ba2, Chunk, Chunks, DirectXChunk, DirectXChunks, DirectXEntry, Entries, Entry, GeneralChunk,
    GeneralChunks, GeneralEntry,
};

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[non_exhaustive]
#[derive(Debug, Error)]
pub enum Error {
    #[error(transparent)]
    Read(#[from] ReadError),

    #[error(transparent)]
    Io(#[from] io::Error),
}

#[non_exhaustive]
#[derive(Debug, Error)]
pub enum ReadError {
    #[error("invalid archive magic: {0:?}")]
    InvalidMagic([u8; 4]),

    #[error("invalid archive version: {0}")]
    InvalidVersion(u32),

    #[error("unsupported format: {0:?}")]
    UnsupportedFormat([u8; 4]),

    #[error("invalid chunk size {0} for format {1:?}")]
    InvalidChunkSize(u16, Format),

    #[error("invalid chunk sentinel: 0x{0:x} (required to be 0xBAADF00D)")]
    InvalidChunkSentinel(u32),
}
