use std::io;

use thiserror::Error;

mod raw;
pub mod read;
pub mod read2;
pub mod write;

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("invalid header magic number")]
    InvalidMagic,

    #[error("missing null terminator")]
    MissingNull,

    #[error("invalid offset")]
    InvalidOffset,

    #[error("invalid string encoding")]
    InvalidEncoding,

    #[error("invalid filename")]
    InvalidFileName,

    #[error("invalid header")]
    InvalidHeader,

    #[error("invalid filename")]
    ExceededMaxSize,

    #[error("unexpected eof")]
    Eof,

    #[error(transparent)]
    FromBytes(#[from] bytemuck::PodCastError),

    #[error(transparent)]
    Io(#[from] io::Error),
}
