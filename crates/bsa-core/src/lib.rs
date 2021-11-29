pub mod helpers;
pub mod path;
pub mod str;
pub mod string;

mod error;
mod read;

pub use error::{Error, ReadError, Result};
pub use read::{Archive, Entries, Entry};

pub mod detail {
    pub use super::read::EntriesImpl;
}

pub use windows_1252;
