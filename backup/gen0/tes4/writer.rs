use std::{
    collections::HashMap,
    convert::TryInto,
    io::{self, Cursor, Seek, SeekFrom, Write},
    marker::PhantomData,
    mem,
    path::Path,
};

use bytemuck::{bytes_of, cast_slice};
use flate2::bufread::ZlibEncoder;
use lz4_flex::frame::FrameEncoder;

use crate::{
    tes4::{Compression, FileRecord, FolderRecord, RawHeader},
    write::{ArchiveWrite, FileData},
    ArchiveWriteError, Result,
};

use super::{ArchiveFlags, Bsa, FileFlags, Hash, Header};

pub struct BsaWriter<A>
where
    A: Bsa,
{
    total_dir_name_len: u32,
    total_file_name_len: u32,
    file_count: u32,
    archive_flags: ArchiveFlags,
    file_flags: FileFlags,
    dirs: HashMap<Hash, Dir>,
    _marker: PhantomData<A>,
}

impl<A> BsaWriter<A>
where
    A: Bsa,
{
    pub fn new() -> BsaWriter<A> {
        BsaWriter {
            total_dir_name_len: 0,
            total_file_name_len: 0,
            file_count: 0,
            archive_flags: ArchiveFlags::INCLUDE_DIRNAMES | ArchiveFlags::INCLUDE_FILENAMES,
            file_flags: FileFlags::empty(),
            dirs: HashMap::new(),
            _marker: PhantomData::default(),
        }
    }

    pub fn set_embed_filenames(&mut self, embed: bool) -> Result<()> {
        if embed {
            self.archive_flags.insert(ArchiveFlags::EMBED_FILENAMES)
        } else {
            self.archive_flags.remove(ArchiveFlags::EMBED_FILENAMES)
        }
        Ok(())
    }

    fn write_to_inner(self, mut w: &mut dyn WriteSeek) -> Result<()> {
        let header: Header<A> = Header {
            archive_flags: self.archive_flags,
            folder_count: self.dirs.len().try_into().unwrap(),
            file_count: self.file_count,
            total_folder_name_length: self.total_dir_name_len,
            total_file_name_length: self.total_file_name_len,
            file_flags: self.file_flags,
            _marker: PhantomData::default(),
        };
        let raw_header = RawHeader::from(header);

        w.write_all(bytes_of(&raw_header))?;

        let compressed = self.archive_flags.contains(ArchiveFlags::COMPRESSED);
        let embed_names =
            A::CAN_EMBED_FILENAMES && self.archive_flags.contains(ArchiveFlags::EMBED_FILENAMES);

        let mut dirs: Vec<_> = self
            .dirs
            .into_iter()
            .map(|(hash, dir)| RawDir {
                name: dir.name,
                hash,
                files: dir
                    .files
                    .into_iter()
                    .map(|(hash, file)| RawFile {
                        name: file.name,
                        hash,
                        data: file.data,
                    })
                    .collect(),
            })
            .collect();
        dirs.sort_unstable_by_key(|dir| dir.hash);
        for dir in &mut dirs {
            dir.files.sort_unstable_by_key(|file| file.hash);
        }

        let mut folder_records = Vec::new();
        let mut file_record_block_offset = mem::size_of::<RawHeader>() as u32
            + self.total_file_name_len
            + (dirs.len() as u32 * mem::size_of::<A::FolderRecord>() as u32);
        for dir in &dirs {
            let folder_record = A::FolderRecord::new(
                dir.hash,
                dir.files.len().try_into().unwrap(),
                file_record_block_offset,
            );
            folder_records.push(folder_record);
            file_record_block_offset += dir.name.len() as u32 + 1;
            file_record_block_offset += (mem::size_of::<FileRecord>() * dir.files.len()) as u32;
        }
        w.write_all(cast_slice(&folder_records))?;

        let file_record_blocks_len = dirs.len()
            + self.total_dir_name_len as usize
            + mem::size_of::<FileRecord>() * self.file_count as usize;

        let zeroed_file_record_blocks = vec![0; file_record_blocks_len];
        w.write_all(&zeroed_file_record_blocks)?;

        let mut file_names_block = Vec::with_capacity(self.total_file_name_len as usize);
        for dir in &mut dirs {
            for file in &mut dir.files {
                file_names_block.extend_from_slice(&file.name);
            }
        }
        w.write_all(&file_names_block)?;

        let mut file_sizes = Vec::new();

        let mut buffer = Vec::new();
        for dir in &mut dirs {
            for file in &mut dir.files {
                if embed_names {
                    let name_len = dir.name.len() + file.name.len() - 1;
                    let mut name = vec![name_len as u8];
                    name.extend_from_slice(dir.name.split_last().unwrap().1);
                    name.push(b'\\');
                    name.extend_from_slice(file.name.split_last().unwrap().1);
                    w.write_all(&name)?;
                }

                if compressed {
                    buffer.clear();
                    let mut temp = Cursor::new(buffer);

                    let uncompressed_len: u32 = file.data.write_to(&mut temp)? as u32;
                    w.write_all(&uncompressed_len.to_le_bytes())?;
                    temp.rewind()?;

                    let compressed_len = match A::COMPRESSION {
                        Compression::Zlib => {
                            let level = flate2::Compression::new(1);
                            let mut encoder = ZlibEncoder::new(&mut temp, level);
                            io::copy(&mut encoder, w)?
                        }
                        Compression::Lz4 => {
                            let mut encoder = FrameEncoder::new(&mut w);
                            let n = io::copy(&mut temp, &mut encoder)?;
                            encoder.finish()?;
                            n
                        }
                    };
                    buffer = temp.into_inner();
                    file_sizes.push(compressed_len);
                } else {
                    let n = file.data.write_to(&mut w)?;
                    file_sizes.push(n);
                }
            }
        }

        {
            let off = mem::size_of::<RawHeader>() + dirs.len() * mem::size_of::<A::FolderRecord>();
            let off = off as u64;
            w.seek(SeekFrom::Start(off))?;
        }

        let file_block_offset = mem::size_of::<RawHeader>()
            + dirs.len() * mem::size_of::<A::FolderRecord>()
            + dirs.len()
            + self.file_count as usize * mem::size_of::<FileRecord>()
            + self.total_dir_name_len as usize
            + self.total_file_name_len as usize;
        let mut file_block_offset = file_block_offset as u32;
        let mut file_sizes = file_sizes.into_iter();

        let mut file_records = Vec::new();
        for dir in &dirs {
            w.write_all(&[dir.name.len() as u8])?;
            w.write_all(&dir.name)?;
            file_records.clear();

            for file in &dir.files {
                let size = file_sizes.next().unwrap() as u32;
                let file_record = FileRecord::new(file.hash, size, file_block_offset, false);
                file_records.push(file_record);

                file_block_offset += size;
                if embed_names {
                    file_block_offset += dir.name.len() as u32 - 1;
                    file_block_offset += file.name.len() as u32 - 1;
                    file_block_offset += 2;
                }
                if compressed {
                    file_block_offset += 4;
                }
            }
            w.write_all(cast_slice(&file_records))?;
        }

        Ok(())
    }

    fn add(&mut self, path: &Path, data: Box<dyn FileData>) -> Result<()> {
        let path = super::path::normalize(path)?;

        let (dirname, filename) = super::path::split(&path);
        let filename = filename.ok_or(ArchiveWriteError::InvalidFileName).unwrap();

        let dir_hash = Hash::from_dirname(dirname)
            .ok_or(ArchiveWriteError::InvalidFileName)
            .unwrap();
        let file_hash = Hash::from_filename(filename)
            .ok_or(ArchiveWriteError::InvalidFileName)
            .unwrap();

        let extension = super::path::split_extension(filename).1;

        self.file_flags |= match extension {
            b".dds" => FileFlags::TEXTURES,
            b".nif" => FileFlags::MESHES,
            _ => todo!(
                "unsupported extension {}",
                String::from_utf8_lossy(extension)
            ),
        };

        let mut dir_is_new = false;
        let dir = self.dirs.entry(dir_hash).or_insert_with(|| {
            dir_is_new = true;
            let mut dirname = dirname.to_owned();
            dirname.push(b'\0');
            Dir {
                name: dirname.to_owned(),
                files: Default::default(),
            }
        });
        if dir_is_new {
            self.total_dir_name_len += dirname.len() as u32 + 1;
        }

        let mut owned_filename = filename.to_owned();
        owned_filename.push(b'\0');
        let file = File {
            name: owned_filename,
            data,
        };

        if dir.files.insert(file_hash, file).is_none() {
            self.file_count += 1;
            self.total_file_name_len += filename.len() as u32 + 1;
        }
        Ok(())
    }
}

impl<A> Default for BsaWriter<A>
where
    A: Bsa,
{
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl<A> ArchiveWrite for BsaWriter<A>
where
    A: Bsa,
{
    #[inline]
    fn write_to<W>(self, w: &mut W) -> Result<()>
    where
        W: Write + Seek,
    {
        self.write_to_inner(w)
    }

    #[inline]
    fn set_compressed(&mut self, compressed: bool) -> Result<()> {
        if compressed {
            self.archive_flags.insert(ArchiveFlags::COMPRESSED)
        } else {
            self.archive_flags.remove(ArchiveFlags::COMPRESSED)
        }
        Ok(())
    }

    #[inline]
    fn add<D>(&mut self, path: &Path, data: D) -> Result<()>
    where
        D: FileData,
    {
        self.add(path, Box::new(data))
    }
}

trait WriteSeek: Write + Seek {}

impl<W: Write + Seek> WriteSeek for W {}

struct Dir {
    name: Vec<u8>,
    files: HashMap<Hash, File>,
}

struct RawDir {
    name: Vec<u8>,
    hash: Hash,
    files: Vec<RawFile>,
}

struct RawFile {
    name: Vec<u8>,
    hash: Hash,
    data: Box<dyn FileData>,
}

struct File {
    name: Vec<u8>,
    data: Box<dyn FileData>,
}
