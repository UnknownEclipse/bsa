use std::io;

use thiserror::Error;

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
    #[error("invalid header")]
    InvalidHeader,

    #[error("eof")]
    Eof,

    #[error("embedded nul")]
    EmbeddedNul,

    #[error("missing nul")]
    MissingNul,
}
