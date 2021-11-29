use std::io::{Read, Seek};

use private::Sealed;

pub mod hash;

mod archive;
mod bytes;
mod common;
mod raw_archive;
mod read_at;

#[cfg(test)]
mod tests;

pub use archive::{BsaArchive, Index};
pub use bsa_core::{Error, Result};

pub type Tes4Archive<R> = BsaArchive<Tes4, R>;
pub type Fo3Archive<R> = BsaArchive<Fo3, R>;
pub type FnvArchive<R> = BsaArchive<Fnv, R>;
pub type Tes5Archive<R> = BsaArchive<Tes5, R>;
pub type SseArchive<R> = BsaArchive<Sse, R>;

trait ReadSeek: Read + Seek {}

impl<R: Read + Seek> ReadSeek for R {}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Version {
    V103,
    V104,
    V105,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Compression {
    Zlib,
    Lz4,
}

pub trait Bsa: Sealed {
    const VERSION: Version;
}

pub struct Tes4;
pub struct Fo3;
pub struct Fnv;
pub struct Tes5;
pub struct Sse;

mod private {
    pub trait Sealed {}
}

impl Sealed for Tes4 {}
impl Sealed for Fo3 {}
impl Sealed for Fnv {}
impl Sealed for Tes5 {}
impl Sealed for Sse {}

impl Bsa for Tes4 {
    const VERSION: Version = Version::V103;
}

impl Bsa for Fo3 {
    const VERSION: Version = Version::V104;
}

impl Bsa for Fnv {
    const VERSION: Version = Version::V104;
}

impl Bsa for Tes5 {
    const VERSION: Version = Version::V104;
}

impl Bsa for Sse {
    const VERSION: Version = Version::V105;
}
