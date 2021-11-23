use std::{
    io::{Read, Seek},
    path::PathBuf,
};

use crate::Result;
pub mod tes3;
pub mod tes4;

pub trait Archive<R>: Sized
where
    R: Read + Seek,
{
    type Entry: Entry;
    type Entries: Iterator<Item = Result<Self::Entry>>;

    fn new(r: R) -> Result<Self>;

    fn into_entries(self) -> Result<Self::Entries>;

    fn get(&self, name: &str) -> Result<Option<Self::Entry>>;
}

pub trait Entry {
    fn directory_name(&self) -> Result<&str>;

    fn name(&self) -> Result<&str>;

    fn path(&self) -> Result<PathBuf>;

    fn data(&self) -> Result<EntryData<'_>>;
}

pub struct EntryData<'a> {
    inner: Box<dyn Read + 'a>,
}

impl<'a> EntryData<'a> {
    pub fn new<R>(r: R) -> Self
    where
        R: Read + 'a,
    {
        Self { inner: Box::new(r) }
    }
}

impl Read for EntryData<'_> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.inner.read(buf)
    }
}
