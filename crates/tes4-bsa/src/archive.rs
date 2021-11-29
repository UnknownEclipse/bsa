use std::{
    io::{Read, Seek},
    marker::PhantomData,
    path::Path,
};

use bsa_core::{Archive, Entries, Entry, ReadError, Result};

use crate::{raw_archive::RawArchive, read_at::ReadAt, Bsa};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Index {
    pub(crate) folder: u32,
    pub(crate) file: u32,
}

pub struct BsaArchive<A, R>
where
    A: Bsa,
    R: Read + Seek,
{
    inner: RawArchive<R>,
    _marker: PhantomData<A>,
}

impl<A, R> BsaArchive<A, R>
where
    A: Bsa,
    R: Read + Seek,
{
    pub fn new(r: R) -> Result<BsaArchive<A, R>> {
        let raw = RawArchive::new(r)?;
        if raw.version != A::VERSION {
            Err(ReadError::InvalidHeader.into())
        } else {
            Ok(BsaArchive {
                inner: raw,
                _marker: PhantomData,
            })
        }
    }

    pub fn extract1<P: AsRef<Path>>(&self, dir: P) -> Result<()> {
        self.inner.extract1(dir.as_ref())
    }

    pub fn extract2<P: AsRef<Path>>(&self, dir: P) -> Result<()> {
        self.inner.extract2(dir.as_ref())
    }

    pub fn extract3<P: AsRef<Path>>(&self, dir: P) -> Result<()> {
        self.inner.extract3(dir.as_ref())
    }
}

impl<A: Bsa, R: ReadAt + Read + Seek + Sync> BsaArchive<A, R> {
    pub fn extract4<P: AsRef<Path>>(&self, out: P) -> Result<()> {
        self.inner.extract4(out.as_ref())
    }
}

impl<A, R> Archive for BsaArchive<A, R>
where
    A: Bsa,
    R: Read + Seek,
{
    type Index = Index;

    fn by_index(&self, index: Self::Index) -> Entry<Self> {
        if let Some(dir) = self.inner.dirs.get(index.folder as usize) {
            if dir.files.get(index.file as usize).is_none() {
                panic!("index out of range");
            }
        } else {
            panic!("index out of range");
        }
        Entry::new(&self.inner, index)
    }

    fn by_name<S: AsRef<str>>(&self, name: S) -> Option<Entry<Self>> {
        let index = self.inner.find_file_by_name(name.as_ref())?;
        Some(Entry::new(&self.inner, index))
    }

    fn entries(&self) -> Entries<Self> {
        for (i, dir) in self.inner.dirs.iter().enumerate() {
            if dir.files.is_empty() {
                continue;
            } else {
                let folder = i as u32;
                let file = 0;
                let index = Index { folder, file };
                return Entries::new(&self.inner, Some(index));
            }
        }
        Entries::new(&self.inner, None)
    }

    fn extract<P: AsRef<Path>>(&self, dir: P) -> Result<()> {
        self.inner.extract1(dir.as_ref())
    }
}
