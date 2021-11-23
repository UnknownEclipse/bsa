use std::{
    collections::HashMap,
    fs::{self, File},
    io::{self, BufReader, Read, Seek, Write},
    mem,
    path::{Path, PathBuf},
};

use crate::{
    raw::tes3::{Hash, Header, NameOffset, Record},
    Error, Result,
};

/// Used to stream data only when the time comes to write to a destination.
type WriteDataFn = Box<dyn Data>;

pub struct Archive {
    entries: HashMap<Vec<u8>, WriteDataFn>,
}

impl Archive {
    pub fn add_from_file(&mut self, name: String, path: &Path) -> Result<()> {
        self.add_inner(
            name,
            Box::new(FileData {
                path: path.to_owned(),
            }),
        )
    }

    // pub fn add_from_reader<R>(&mut self, name: String, r: R) -> Result<()>
    // where
    //     R: 'static + Read + Seek,
    // {
    //     self.add_inner(
    //         name,
    //         Box::new(FileData {
    //             path: path.to_owned(),
    //         }),
    //     )
    // }

    fn add_inner(&mut self, name: String, data: Box<dyn Data>) -> Result<()> {
        let name = normalize_name(name)?;
        self.entries.insert(name, data);
        Ok(())
    }

    pub fn write(self, w: &mut dyn Write) -> Result<()> {
        let file_count = self.entries.len();

        let mut name_offsets = Vec::with_capacity(file_count);
        let mut hashes = Vec::with_capacity(file_count);

        let mut entries: Vec<_> = self
            .entries
            .into_iter()
            .map(|(name, data)| (Hash::from_bytes(&name).unwrap(), name, data))
            .collect();
        entries.sort_by_key(|(hash, _, _)| *hash);

        let mut names = Vec::new();

        for (hash, name, _) in entries.iter() {
            hashes.push(*hash);
            name_offsets.push(NameOffset::from(names.len() as u32));

            names.extend_from_slice(name);
            names.push(b'\0');
        }

        let mut records = Vec::with_capacity(file_count);
        let mut data_offset = 0;
        entries.sort_by(|a, b| a.1.cmp(&b.1));
        for (_, _, data) in entries.iter() {
            let len = data.data_len()? as u32;
            records.push(Record::new(len, data_offset));
            data_offset += len;
        }

        let hash_table_offset =
            names.len() + file_count * (mem::size_of::<NameOffset>() + mem::size_of::<Record>());

        let header = Header::new(hash_table_offset as u32, file_count as u32);

        w.write_all(bytemuck::bytes_of(&header))?;
        w.write_all(bytemuck::cast_slice(records.as_slice()))?;
        w.write_all(bytemuck::cast_slice(name_offsets.as_slice()))?;
        w.write_all(&names)?;
        w.write_all(bytemuck::cast_slice(hashes.as_slice()))?;
        for (_, _, mut data) in entries {
            data.write(w)?;
        }
        Ok(())
    }
}

trait Data {
    fn write(&mut self, w: &mut dyn Write) -> Result<()>;

    fn data_len(&self) -> Result<usize>;
}

struct FileData {
    path: PathBuf,
}

impl Data for FileData {
    fn write(&mut self, w: &mut dyn Write) -> Result<()> {
        let f = File::open(&self.path)?;
        let mut r = BufReader::new(f);
        io::copy(&mut r, w)?;
        Ok(())
    }

    fn data_len(&self) -> Result<usize> {
        let meta = fs::metadata(&self.path)?;
        Ok(meta.len() as usize)
    }
}

fn normalize_name(name: String) -> Result<Vec<u8>> {
    if !name.is_ascii() {
        return Err(Error::InvalidFileName);
    }
    let mut bytes = name.into_bytes();
    for byte in bytes.iter_mut() {
        if *byte == b'/' {
            *byte = b'\\';
        } else if *byte == b'\0' {
            return Err(Error::InvalidFileName);
        } else {
            byte.make_ascii_lowercase()
        }
    }
    Ok(bytes)
}
