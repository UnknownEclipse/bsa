use std::{
    collections::HashMap,
    convert::TryInto,
    io::{Read, Seek, SeekFrom},
    str,
};

use memchr::memchr;

use crate::{
    common::read_vec,
    read::{
        tes3::{BsaIndex, NameOffset, Record},
        Archive, EntryData, RawEntryData,
    },
    InvalidArchiveError, Result,
};

pub struct Bsa<R>
where
    R: Read + Seek,
{
    reader: R,
    files: Vec<File>,
    names: HashMap<String, BsaIndex>,
}

impl<R> Bsa<R>
where
    R: Read + Seek,
{
    pub fn new(mut r: R) -> Result<Self> {
        let mut header_buf = [0; 12];
        r.read_exact(&mut header_buf)?;

        let magic = u32::from_le_bytes(header_buf[..4].try_into().unwrap());
        let hash_table_offset = u32::from_le_bytes(header_buf[4..8].try_into().unwrap());
        let file_count = u32::from_le_bytes(header_buf[8..].try_into().unwrap());

        if magic != 0x100 {
            return Err(InvalidArchiveError::InvalidMagic.into());
        }

        let meta_len = 8u32
            .checked_mul(file_count)
            .ok_or(InvalidArchiveError::BadOffset)?
            .checked_add(12)
            .ok_or(InvalidArchiveError::BadOffset)?;

        let meta_buf = read_vec(&mut r, meta_len as usize)?;
        let meta = meta_buf.as_slice();

        let (records, meta) = meta.split_at(8 * file_count as usize);
        let records: &[Record] = bytemuck::cast_slice(records);

        let (name_offsets, meta) = meta.split_at(4 * file_count as usize);
        let name_offsets: &[NameOffset] = bytemuck::cast_slice(name_offsets);

        let names_len = meta_len
            .checked_sub(hash_table_offset)
            .ok_or(InvalidArchiveError::BadOffset)?;

        let (name_block, meta) = meta.split_at(names_len as usize);
        if meta.len() != 8 * file_count as usize {
            return Err(InvalidArchiveError::BadOffset.into());
        }

        let mut files = Vec::with_capacity(file_count as usize);
        let mut names = HashMap::with_capacity(file_count as usize);

        for i in 0..file_count as usize {
            let record = records[i];
            let name_offset = name_offsets[i];
            let name = read_name(name_block, name_offset)?;
            let file = File {
                offset: record.offset(),
                size: record.size(),
            };
            let index = BsaIndex(i as u32);
            files.push(file);
            names.insert(name.to_string(), index);
        }

        Ok(Self {
            reader: r,
            files,
            names,
        })
    }
}

impl<R> Archive for Bsa<R>
where
    R: Read + Seek,
{
    type Index = BsaIndex;

    fn by_index(&mut self, index: Self::Index) -> Result<EntryData<'_>> {
        Ok(EntryData::new_uncompressed(self.by_index_raw(index)?))
    }

    fn by_index_raw(&mut self, index: Self::Index) -> Result<RawEntryData<'_>> {
        let index = index.0 as usize;
        let file = &self.files[index];

        let off = file.offset as u64;
        let size = file.size as u64;

        self.reader.seek(SeekFrom::Start(off))?;
        let r: &mut dyn Read = &mut self.reader;
        Ok(RawEntryData::from_stream(r.take(size)))
    }

    fn by_name(&mut self, name: &str) -> Result<Option<EntryData<'_>>> {
        match self.names.get(name) {
            Some(&index) => Ok(Some(self.by_index(index)?)),
            _ => Ok(None),
        }
    }

    fn by_name_raw(&mut self, name: &str) -> Result<Option<RawEntryData<'_>>> {
        match self.names.get(name) {
            Some(&index) => Ok(Some(self.by_index_raw(index)?)),
            _ => Ok(None),
        }
    }
}

struct File {
    size: u32,
    offset: u32,
}

fn read_name(names: &[u8], off: NameOffset) -> Result<&str> {
    let off = off.get() as usize;
    if names.len() <= off {
        return Err(InvalidArchiveError::BadOffset.into());
    }

    let names = &names[off..];
    let len = memchr(b'\0', names).ok_or(InvalidArchiveError::MissingNul)?;
    let name = &names[..len];

    if !name.is_ascii() {
        Err(InvalidArchiveError::BadEncoding.into())
    } else {
        Ok(unsafe { str::from_utf8_unchecked(name) })
    }
}
