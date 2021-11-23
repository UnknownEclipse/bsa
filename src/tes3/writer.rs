use std::{
    collections::HashMap,
    convert::TryInto,
    io::{Seek, Write},
    mem,
    path::{self, Path},
};

use bytemuck::{bytes_of, cast_slice};

use crate::{
    write::{ArchiveWrite, FileData},
    ArchiveWriteError, Result,
};

use super::{compute_hash, Header, NameHash, NameOffset, Record};

pub struct Tes3Writer {
    file_names_len: u32,
    entries: HashMap<Vec<u8>, Entry>,
}

impl Tes3Writer {
    pub fn new() -> Tes3Writer {
        Tes3Writer {
            file_names_len: 0,
            entries: HashMap::new(),
        }
    }

    fn add_inner(&mut self, path: &Path, data: Box<dyn FileData>) -> Result<()> {
        let name = path.to_str().ok_or(ArchiveWriteError::InvalidFileName)?;
        let hash = compute_hash(name).ok_or(ArchiveWriteError::InvalidFileName)?;

        let mut name = name.as_bytes().to_owned();
        for byte in name.iter_mut() {
            if path::is_separator(*byte as char) {
                *byte = b'\\';
            } else {
                *byte = byte.to_ascii_lowercase();
            }
        }
        name.push(b'\0');
        let name_len = name.len();

        let entry = Entry { hash, data };

        if self.entries.insert(name, entry).is_none() {
            let name_len = name_len
                .try_into()
                .map_err(|_| ArchiveWriteError::ArchiveTooLarge)?;

            self.file_names_len = self
                .file_names_len
                .checked_add(name_len)
                .ok_or(ArchiveWriteError::ArchiveTooLarge)?;
        }

        Ok(())
    }
}

impl Default for Tes3Writer {
    fn default() -> Self {
        Self::new()
    }
}

impl ArchiveWrite for Tes3Writer {
    fn set_compressed(&mut self, compressed: bool) -> Result<()> {
        if compressed {
            Err(ArchiveWriteError::CompressionUnsupported.into())
        } else {
            Ok(())
        }
    }

    fn add<D>(&mut self, path: &Path, data: D) -> Result<()>
    where
        D: FileData,
    {
        self.add_inner(path, Box::new(data))
    }

    fn write_to<W>(mut self, w: &mut W) -> Result<()>
    where
        W: Write + Seek,
    {
        let entries = mem::take(&mut self.entries);
        let mut entries: Vec<_> = entries.into_iter().collect();

        let hash_table_offset =
            compute_hash_table_offset(&self, &entries).ok_or(ArchiveWriteError::ArchiveTooLarge)?;

        let header = Header::new(hash_table_offset, entries.len() as u32);
        w.write_all(bytes_of(&header))?;

        entries.sort_unstable_by(|(a, _), (b, _)| a.cmp(b));

        let mut data_offset = 0;
        let mut records = Vec::new();
        let mut data_offsets = Vec::new();
        for (_, entry) in &mut entries {
            data_offsets.push(data_offset);
            let len: u32 = entry.data.len()?.try_into().unwrap();
            data_offset += len;
        }

        entries.sort_unstable_by_key(|(_, entry)| entry.hash);

        for (offset, (_name, entry)) in data_offsets.into_iter().zip(entries.iter_mut()) {
            let size = entry.data.len()?.try_into().unwrap();
            let record = Record::new(size, offset);
            records.push(record);
        }
        w.write_all(cast_slice(&records))?;

        let mut names = Vec::with_capacity(self.file_names_len as usize);
        let mut name_offsets = Vec::with_capacity(entries.len());

        for (name, _) in &entries {
            let offset = NameOffset::new(names.len() as u32);
            names.extend_from_slice(name);
            name_offsets.push(offset);
        }

        // let zeroed_records = vec![0; entries.len() * mem::size_of::<Record>()];
        // w.write_all(&zeroed_records)?;

        w.write_all(cast_slice(&name_offsets))?;
        w.write_all(&names)?;

        let hashes: Vec<_> = entries.iter().map(|(_, entry)| entry.hash).collect();
        w.write_all(cast_slice(&hashes))?;

        entries.sort_unstable_by(|(a, _), (b, _)| a.cmp(b));

        for (_, entry) in &mut entries {
            entry.data.write_to(w)?;
        }

        Ok(())
    }
}

fn compute_hash_table_offset(w: &Tes3Writer, entries: &[(Vec<u8>, Entry)]) -> Option<u32> {
    let records_len = mem::size_of::<Record>().checked_mul(entries.len())?;
    let name_offsets_len = mem::size_of::<NameOffset>().checked_mul(entries.len())?;
    let file_names_len = w.file_names_len;
    file_names_len
        .checked_add(records_len.try_into().ok()?)?
        .checked_add(name_offsets_len.try_into().ok()?)
}

struct Entry {
    hash: NameHash,
    data: Box<dyn FileData>,
}
