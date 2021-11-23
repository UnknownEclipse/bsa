use std::{io, num::TryFromIntError};

use lz4_flex::frame;
use thiserror::Error;

mod archive;
mod common;
pub mod read;
mod tes3;
mod tes4;
pub mod write;
mod writer;

pub use writer::ArchiveWriter;

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[non_exhaustive]
#[derive(Debug, Error)]
pub enum Error {
    #[error(transparent)]
    Read(#[from] ArchiveReadError),

    #[error(transparent)]
    Write(#[from] ArchiveWriteError),

    #[error(transparent)]
    Io(#[from] io::Error),

    #[error(transparent)]
    WalkDir(#[from] walkdir::Error),

    #[error(transparent)]
    Lz4(#[from] frame::Error),

    #[error(transparent)]
    ReadBytes(#[from] ReadBytesError),
}

#[non_exhaustive]
#[derive(Debug, Error)]
pub enum ArchiveReadError {
    #[error("invalid archive header")]
    BadHeader,

    #[error("missing null terminator in archive string")]
    MissingNul,

    #[error("embedded null in archive string")]
    EmbeddedNul,

    #[error("invalid string encoding")]
    BadEncoding,

    #[error("bad offset in archive")]
    BadOffset,

    #[error("unsupported archive format")]
    UnsupportedFormat,

    #[error("invalid magic")]
    InvalidMagic,

    #[error("invalid version")]
    InvalidVersion,

    #[error("invalid flags")]
    InvalidFlags,

    #[error("bad sentinel")]
    BadSentinel,

    #[error("bad archive")]
    BadArchive,

    #[error("file is not found in the archive")]
    FileNotFound,

    #[error(transparent)]
    Overflow(#[from] TryFromIntError),
}

#[non_exhaustive]
#[derive(Debug, Error)]
pub enum ReadBytesError {
    #[error("eof")]
    Eof,
}

#[non_exhaustive]
#[derive(Debug, Error)]
pub enum ArchiveWriteError {
    #[error("invalid file name")]
    InvalidFileName,

    #[error("compression is not supported for this archive format")]
    CompressionUnsupported,

    #[error("archive is too large for this format")]
    ArchiveTooLarge,

    #[error("file is too large for this archive format")]
    FileTooLarge,

    #[error("file already exists in archive")]
    FileExists,
}

#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Format {
    Tes3,
    Tes4,
    Tes5,
    Sse,
    Fo4,
}
