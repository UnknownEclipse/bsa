use std::{
    borrow::Cow,
    cell::RefCell,
    io::{Cursor, Read, Seek, SeekFrom},
    mem,
    path::{self, PathBuf},
    str,
};

use memchr::memchr;

use crate::{
    raw::tes3::{Hash, Header, NameOffset, Record},
    Error, Result,
};

use super::EntryData;

struct File<'a> {
    pub name: &'a str,
    pub offset: u32,
    pub size: u32,
}

pub struct Archive<R>
where
    R: Read + Seek,
{
    meta: ArchiveMeta<'static>,
    reader: RefCell<R>,
}

impl<R> Archive<R>
where
    R: Read + Seek,
{
    pub fn new(mut r: R) -> Result<Self> {
        let mut header = Header::new(0, 0);
        r.read_exact(bytemuck::bytes_of_mut(&mut header))?;
        if header.magic() != 0x100 {
            return Err(Error::InvalidMagic);
        }

        let meta = ArchiveMeta::from_reader(&mut r, &header)?;
        Ok(Self {
            meta,
            reader: RefCell::new(r),
        })
    }

    pub fn entries(&self) -> Result<Entries<'_, R>> {
        Ok(Entries {
            archive: self,
            files: self.meta.files(),
        })
    }

    pub fn get(&self, name: &str) -> Result<Option<Entry<'_, R>>> {
        if !name.is_ascii() {
            return Ok(None);
        }
        let bytes = name.as_bytes();
        let hash = match Hash::from_bytes(bytes) {
            Some(h) => h,
            None => return Ok(None),
        };
        let file = self.meta.file_by_hash(hash)?;
        Ok(file.map(|f| Entry {
            file: f,
            archive: self,
        }))
    }

    pub fn read_at(&self, off: usize, len: usize) -> Result<Vec<u8>> {
        let mut buf = vec![0; len];
        let mut r = self.reader.borrow_mut();
        r.seek(SeekFrom::Start(off as u64))?;
        r.read_exact(&mut buf)?;
        Ok(buf)
    }
}

pub struct Entry<'a, R>
where
    R: Read + Seek,
{
    archive: &'a Archive<R>,
    file: File<'a>,
}

impl<'a, R> Entry<'a, R>
where
    R: Read + Seek,
{
    pub fn directory_name(&self) -> Result<&str> {
        Ok(self.file.name.rsplit_once('\\').unwrap().0)
    }

    pub fn name(&self) -> Result<&str> {
        Ok(self.file.name.rsplit_once('\\').unwrap().1)
    }

    pub fn path(&self) -> Result<PathBuf> {
        let sep = path::MAIN_SEPARATOR as u8;

        let s = self.file.name.to_owned();
        let mut v = s.into_bytes();
        for byte in v.iter_mut() {
            if *byte == b'\\' {
                *byte = sep;
            }
        }
        let s = unsafe { String::from_utf8_unchecked(v) };
        Ok(PathBuf::from(s))
    }

    pub fn data(&self) -> Result<EntryData<'_>> {
        Ok(EntryData::new(Cursor::new(self.archive.read_at(
            self.file.offset as usize,
            self.file.size as usize,
        )?)))
    }
}

pub struct Entries<'a, R>
where
    R: Read + Seek,
{
    files: ArchiveFiles<'a>,
    archive: &'a Archive<R>,
}

impl<'a, R> Iterator for Entries<'a, R>
where
    R: Read + Seek,
{
    type Item = Result<Entry<'a, R>>;

    fn next(&mut self) -> Option<Self::Item> {
        let file = match self.files.next()? {
            Err(e) => return Some(Err(e)),
            Ok(file) => file,
        };
        Some(Ok(Entry {
            file,
            archive: self.archive,
        }))
    }
}

struct ArchiveMeta<'a> {
    bytes: Cow<'a, [u8]>,
    file_count: u32,
    hash_table_offset: u32,
    data_offset: u32,
}

impl<'a> ArchiveMeta<'a> {
    pub fn from_reader(r: &mut dyn Read, header: &Header) -> Result<Self> {
        let len = header.hash_table_offset() + header.file_count() * mem::size_of::<Hash>() as u32;
        let mut buf = vec![0; len as usize];
        r.read_exact(&mut buf)?;
        Self::new_inner(Cow::Owned(buf), header)
    }

    #[allow(dead_code)]
    pub fn from_bytes(buf: &'a [u8], header: &Header) -> Result<Self> {
        Self::new_inner(Cow::Borrowed(buf), header)
    }

    fn new_inner(bytes: Cow<'a, [u8]>, header: &Header) -> Result<Self> {
        let data_offset = header
            .file_count()
            .checked_mul(mem::size_of::<Hash>() as u32)
            .ok_or(Error::InvalidOffset)?
            .checked_add(header.hash_table_offset())
            .ok_or(Error::InvalidOffset)?;

        Ok(Self {
            bytes,
            file_count: header.file_count(),
            hash_table_offset: header.hash_table_offset(),
            data_offset,
        })
    }

    pub fn hashes(&self) -> &[Hash] {
        let off = self.hash_table_offset as usize;
        let len = self.file_count as usize * mem::size_of::<Hash>();
        bytemuck::cast_slice(&self.bytes[off..off + len])
    }

    pub fn name_offsets(&self) -> &[NameOffset] {
        let off = self.file_count as usize * mem::size_of::<Record>();
        let len = self.file_count as usize * mem::size_of::<NameOffset>();
        bytemuck::cast_slice(&self.bytes[off..off + len])
    }

    pub fn records(&self) -> &[Record] {
        let off = 0;
        let len = self.file_count as usize * mem::size_of::<Record>();
        bytemuck::cast_slice(&self.bytes[off..off + len])
    }

    pub fn name(&self, off: NameOffset) -> Result<&str> {
        let off: u32 = off.into();
        let off = off as usize;

        let names_offset =
            self.file_count as usize * (mem::size_of::<Record>() + mem::size_of::<NameOffset>());
        let names_len = (self.hash_table_offset as usize)
            .checked_sub(names_offset)
            .ok_or(Error::InvalidOffset)?;
        let names = &self.bytes[names_offset..names_offset + names_len];
        if names.len() <= off {
            return Err(Error::InvalidOffset);
        }
        let names = &names[off..];
        let len = memchr(b'\0', names).ok_or(Error::MissingNull)?;
        let name = &names[..len];
        if !name.is_ascii() {
            return Err(Error::InvalidEncoding);
        }
        Ok(unsafe { str::from_utf8_unchecked(name) })
    }

    pub fn file(&self, index: usize) -> Result<File> {
        let record = self.records()[index];
        let name_offset = self.name_offsets()[index];
        let name = self.name(name_offset)?;

        Ok(File {
            name,
            offset: record
                .offset()
                .checked_add(self.data_offset)
                .ok_or(Error::InvalidOffset)?,
            size: record.size(),
        })
    }

    pub fn file_by_hash(&self, hash: Hash) -> Result<Option<File>> {
        let hashes = self.hashes();
        if let Ok(index) = hashes.binary_search(&hash) {
            Ok(Some(self.file(index)?))
        } else {
            Ok(None)
        }
    }
    pub fn files(&self) -> ArchiveFiles<'_> {
        ArchiveFiles {
            meta: self,
            index: 0,
        }
    }
}

struct ArchiveFiles<'a> {
    meta: &'a ArchiveMeta<'a>,
    index: u32,
}

impl<'a> ArchiveFiles<'a> {
    fn cur_file(&self) -> Result<File<'a>> {
        self.meta.file(self.index as usize)
    }
}

impl<'a> Iterator for ArchiveFiles<'a> {
    type Item = Result<File<'a>>;

    fn next(&mut self) -> Option<Self::Item> {
        self.index = self.index.saturating_add(1);
        if self.meta.file_count <= self.index {
            return None;
        }
        Some(self.cur_file())
    }
}
