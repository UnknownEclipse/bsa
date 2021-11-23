use std::{
    borrow::{BorrowMut, Cow},
    cell::RefCell,
    convert::TryInto,
    io::{Read, Seek, SeekFrom},
    marker::PhantomData,
    mem, str,
};

use crate::{
    raw::tes4::{Bsa, Compression, FileRecord, FolderRecord, Header, RawHeader},
    Error, Result,
};

pub struct Archive<A, R>
where
    A: Bsa,
    R: Read + Seek,
{
    meta: ArchiveMeta<'static, A>,
    reader: RefCell<R>,
}

impl<A, R> Archive<A, R>
where
    A: Bsa,
    R: Read + Seek,
{
    pub fn new(mut r: R) -> Result<Self> {
        let meta = ArchiveMeta::from_reader(&mut r)?;
        Ok(Self {
            meta,
            reader: RefCell::new(r),
        })
    }

    fn file_block(&self, file: &FileRecord) -> Result<FileBlock> {
        let mut r = self.reader.borrow_mut();
        r.seek(SeekFrom::Start(file.offset() as u64))?;

        let embedded_name = if self.meta.header.has_embedded_names() {
            let mut buf = [0; 1];
            r.read_exact(&mut buf)?;
            let len = buf[0] as usize;
            let mut name = vec![0; len];
            r.read_exact(&mut name)?;
            Some(String::from_utf8(name).map_err(|_| Error::InvalidEncoding)?)
        } else {
            None
        };

        let original_size = if self.meta.header.compression().is_some() {
            let mut buf = [0; 4];
            r.read_exact(&mut buf)?;
            Some(u32::from_le_bytes(buf))
        } else {
            None
        };

        let mut buf = Vec::with_capacity(file.size() as usize);
        unsafe { buf.set_len(file.size() as usize) };
        r.read_exact(&mut buf)?;

        Ok(FileBlock {
            compression: None,
            name: embedded_name,
            original_size: original_size.unwrap_or_else(|| file.size()),
            raw_data: buf,
        })
    }
}

struct FileBlock {
    name: Option<String>,
    compression: Option<Compression>,
    raw_data: Vec<u8>,
    original_size: u32,
}

struct ArchiveMeta<'a, A>
where
    A: Bsa,
{
    bytes: Cow<'a, [u8]>,
    header: Header<A>,
    _marker: PhantomData<A>,
}

impl<'a, A> ArchiveMeta<'a, A>
where
    A: Bsa,
{
    pub fn from_reader(r: &mut dyn Read) -> Result<Self> {
        let mut raw_header = RawHeader::default();
        r.read_exact(bytemuck::bytes_of_mut(&mut raw_header))?;

        let hdr: Header<A> = raw_header.try_into()?;

        let mut len = 0;
        len += mem::size_of::<A::FolderRecord>() * hdr.folder_count() as usize;
        len += mem::size_of::<FileRecord>() * hdr.file_count() as usize;
        if hdr.include_dirnames() {
            len += (hdr.total_folder_name_length() + hdr.folder_count()) as usize;
        }
        if hdr.include_filenames() {
            len += hdr.total_file_name_length() as usize;
        }
        let len = len;
        let mut buf = vec![0; len];
        r.read_exact(&mut buf)?;
        Self::new_inner(Cow::Owned(buf), hdr)
    }

    fn new_inner(bytes: Cow<'a, [u8]>, header: Header<A>) -> Result<Self> {
        todo!()
    }

    pub fn file_record_block(
        &self,
        folder: &A::FolderRecord,
    ) -> Result<(Option<&str>, &[FileRecord])> {
        let bytes: &[u8] = &self.bytes;
        if bytes.len() <= folder.offset() as usize {
            return Err(Error::InvalidOffset);
        }
        let mut bytes = &bytes[folder.offset() as usize..];

        let name = if self.header.include_dirnames() {
            Some(read_bzstring(&mut bytes)?)
        } else {
            None
        };

        let len = folder.count() as usize * mem::size_of::<FileRecord>();
        if bytes.len() < len {
            return Err(Error::Eof);
        }
        let bytes = &bytes[..len];
        let file_records = bytemuck::try_cast_slice(bytes)?;
        Ok((name, file_records))
    }
}

fn read_bzstring<'a>(bytes: &mut &'a [u8]) -> Result<&'a str> {
    if bytes.len() < 2 {
        return Err(Error::Eof);
    }
    let len = bytes[0] as usize;

    if bytes.len() - 1 < len {
        return Err(Error::Eof);
    }
    let name = &bytes[1..len + 1];
    let (&null, name) = name.split_last().unwrap();
    if null != b'\0' {
        return Err(Error::MissingNull);
    }
    if !name.is_ascii() {
        return Err(Error::InvalidEncoding);
    }
    let name = unsafe { str::from_utf8_unchecked(name) };
    Ok(name)
}
