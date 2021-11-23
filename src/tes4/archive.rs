use std::{
    convert::{TryFrom, TryInto},
    fs,
    io::{self, Cursor, Read, Seek, SeekFrom},
    iter::Enumerate,
    marker::PhantomData,
    mem,
    path::{self, Component, Path, PathBuf},
    slice,
    sync::mpsc::channel,
};

use flate2::bufread::ZlibDecoder;
use lz4_flex::frame;
use memchr::memchr;
use threadpool::ThreadPool;

use super::{Bsa, Compression, FolderRecord, Hash};
use crate::{
    common::{read_vec, read_vec_in, windows_1252, Bytes},
    read::{EntryData, EntryIndex, RawEntryData},
    tes4::{FileRecord, Header, RawHeader},
    ArchiveReadError, Result,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArchiveIndex {
    folder: u32,
    file: u32,
}

impl EntryIndex for ArchiveIndex {}

#[doc(hidden)]
pub struct BsaArchive<A, R>
where
    A: Bsa,
    R: Read + Seek,
{
    reader: R,
    dirs: Vec<Dir>,
    header: Header<A>,
    _marker: PhantomData<A>,
}

impl<A, R> BsaArchive<A, R>
where
    A: Bsa,
    R: Read + Seek,
{
    pub fn new(mut r: R) -> Result<BsaArchive<A, R>> {
        // Keep track of the current position. We read the archive in a streaming
        // fashion (meaning no offsets are used), so we use the read_position to
        // ensure that all offsets are valid.
        let mut read_position = 0;

        let mut raw_header = RawHeader::default();
        r.read_exact(&mut bytemuck::bytes_of_mut(&mut raw_header))?;
        read_position += 36;

        let header = Header::<A>::try_from(raw_header).map_err(|_| ArchiveReadError::BadHeader)?;

        let size_of_folder_records =
            header.folder_count as usize * mem::size_of::<A::FolderRecord>();
        let folder_records = read_vec(&mut r, size_of_folder_records)?;
        let folder_records: &[A::FolderRecord] = bytemuck::cast_slice(&folder_records);
        read_position += size_of_folder_records;

        let mut file_records = Vec::new();
        let mut dirs = Vec::with_capacity(header.folder_count as usize);

        for folder_record in folder_records {
            let folder_offset = folder_record
                .offset()
                .checked_sub(header.total_file_name_length)
                .ok_or(ArchiveReadError::BadOffset)?;

            if folder_offset as usize != read_position {
                return Err(ArchiveReadError::BadOffset.into());
            }

            let name = if header.include_dirnames() {
                let name = read_bzstring(&mut r)?;
                read_position += name.len() + 2;
                Some(name)
            } else {
                None
            };

            let size_of_file_records =
                folder_record.count() as usize * mem::size_of::<FileRecord>();

            read_vec_in(&mut r, size_of_file_records, &mut file_records)?;
            read_position += size_of_file_records;
            let file_records: &[FileRecord] = bytemuck::cast_slice(&file_records);

            let mut files = Vec::with_capacity(folder_record.count() as usize);

            for file_record in file_records {
                let mut compressed = header.compressed();
                if file_record.negate_compression() {
                    compressed = !compressed;
                }
                let file = File {
                    compressed,
                    hash: file_record.hash(),
                    name: None,
                    offset: file_record.offset(),
                    raw_size: file_record.size(),
                };
                files.push(file);
            }

            let dir = Dir {
                name,
                files,
                hash: folder_record.hash(),
            };
            dirs.push(dir);
        }

        if header.include_filenames() {
            let offset = compute_file_name_block_offset(&header);
            if offset != read_position {
                return Err(ArchiveReadError::BadOffset.into());
            }

            let names_block = read_vec(&mut r, header.total_file_name_length as usize)?;
            let mut names = names_block.as_slice();

            for dir in &mut dirs {
                for file in &mut dir.files {
                    let len = memchr(b'\0', names).ok_or(ArchiveReadError::MissingNul)?;
                    let (name, rest) = names.split_at(len + 1);
                    names = rest;
                    let mut name = name.to_owned();
                    name.pop();
                    let name = windows_1252::to_string(name);
                    file.name = Some(name);
                }
            }
        }

        Ok(BsaArchive {
            dirs,
            reader: r,
            header,
            _marker: PhantomData::default(),
        })
    }

    pub fn open_raw(&mut self, index: ArchiveIndex) -> Result<RawEntryData<'_>> {
        let folder = &self.dirs[index.folder as usize];
        let file = &folder.files[index.file as usize];

        let offset = file.offset as u64;
        let len = file.raw_size as u64;

        self.reader.seek(SeekFrom::Start(offset))?;
        let reader: &mut dyn Read = &mut self.reader;
        Ok(RawEntryData::from_stream(reader.take(len)))
    }

    pub fn open(&mut self, index: ArchiveIndex) -> Result<EntryData<'_>> {
        let folder = &self.dirs[index.folder as usize];
        let file = &folder.files[index.file as usize];

        if file.compressed {
            self.reader.seek(SeekFrom::Start(file.offset as u64))?;

            if self.header.embed_filenames() {
                let _embedded_name = read_bstring(&mut self.reader)?;
            }

            let mut uncompressed_len = [0; 4];
            self.reader.read_exact(&mut uncompressed_len)?;
            let uncompressed_len = u32::from_le_bytes(uncompressed_len);

            let r: &mut dyn Read = &mut self.reader;
            let raw = RawEntryData::from_stream(r.take(file.raw_size as u64));

            Ok(match A::COMPRESSION {
                Compression::Zlib => EntryData::new_zlib(raw, uncompressed_len),
                Compression::Lz4 => EntryData::new_lz4(raw, uncompressed_len),
            })
        } else {
            Ok(EntryData::new_uncompressed(self.open_raw(index)?))
        }
    }

    fn read_file_block(&mut self, index: ArchiveIndex) -> Result<FileBlock> {
        let folder = &self.dirs[index.folder as usize];
        let file = &folder.files[index.file as usize];

        let mut len = file.raw_size as u64;
        let offset = file.offset as u64;
        self.reader.seek(SeekFrom::Start(offset))?;

        let embedded_name = if self.header.embed_filenames() {
            let name = read_bstring(&mut self.reader)?;
            len -= name.len() as u64 + 1;
            Some(name)
        } else {
            None
        };

        let uncompressed_len = if file.compressed {
            let mut buf = [0; 4];
            self.reader.read_exact(&mut buf)?;
            len -= 4;
            Some(u32::from_le_bytes(buf))
        } else {
            None
        };

        let data: &mut dyn Read = &mut self.reader;
        let data = data.take(len);

        Ok(FileBlock {
            _embedded_name: embedded_name,
            uncompressed_len,
            data,
        })
    }

    fn read_file_block2(&mut self, index: ArchiveIndex) -> Result<FileBlock2> {
        let folder = &self.dirs[index.folder as usize];
        let file = &folder.files[index.file as usize];

        let mut len = file.raw_size as usize;

        let buf = read_vec(&mut self.reader, len)?;

        let mut r = Bytes(&buf);

        let embedded_name = if self.header.embed_filenames() {
            Some(r.read_bstring()?.into_owned())
        } else {
            None
        };

        let uncompressed_len = if file.compressed {
            let buf = r.read_bytes(4)?.try_into().unwrap();
            Some(u32::from_le_bytes(buf))
        } else {
            None
        };

        let offset = buf.len() - r.len();
        mem::drop(r);

        let mut data = buf;
        data.rotate_left(offset);
        data.resize(data.len() - offset, 0);

        Ok(FileBlock2 {
            _embedded_name: embedded_name,
            uncompressed_len,
            data,
        })
    }

    pub fn entries(&self) -> Entries<'_, A, R> {
        Entries {
            archive: self,
            dir_index: 0,
            files: self
                .dirs
                .first()
                .map(|dir| dir.files.iter().enumerate())
                .unwrap_or_else(|| [].iter().enumerate()),
        }
    }

    /// Extract the contents of the archive into a directory located at `path`.
    ///
    /// # Performance
    /// This will often be faster than opening each entry individually.
    /// For a compressed archive, decompression will be handled in parallel by a
    /// threadpool. Uncompressed archives simply have their data copied directly.
    pub fn extract<P>(&mut self, path: P) -> Result<()>
    where
        P: AsRef<Path>,
    {
        self.extract_inner_threaded(path.as_ref())
    }

    pub fn extract2<P, F>(&mut self, path: P) -> Result<()>
    where
        P: AsRef<Path>,
    {
        self.extract_inner_threaded(path.as_ref())
    }

    fn extract_inner_threaded(&mut self, dir: &Path) -> Result<()> {
        let to_extract: Vec<_> = self
            .entries()
            .map(|entry| (entry.index(), dir.join(entry.path())))
            .collect();

        let pool = ThreadPool::new(num_cpus::get());
        let (errors_tx, errors_rx) = channel();

        for (index, path) in to_extract {
            let FileBlock {
                uncompressed_len,
                mut data,
                ..
            } = self.read_file_block(index)?;

            if uncompressed_len.is_some() {
                let mut buf = Vec::new();
                data.read_to_end(&mut buf)?;
                let data = buf;

                let errors_tx = errors_tx.clone();

                pool.execute(move || match decompress_to(data, &path, A::COMPRESSION) {
                    Ok(()) => {}
                    Err(e) => errors_tx.send(e).unwrap(),
                });
            } else {
                let mut f = fs::File::create(path)?;
                io::copy(&mut data, &mut f)?;
            }
        }

        pool.join();
        if let Ok(e) = errors_rx.try_recv() {
            return Err(e);
        };
        Ok(())
    }

    fn extract_inner_threaded2(&mut self, dir: &Path) -> Result<()> {
        let to_extract: Vec<_> = self
            .entries()
            .map(|entry| (entry.index(), dir.join(entry.path())))
            .collect();

        let pool = ThreadPool::new(num_cpus::get());
        let (errors_tx, errors_rx) = channel();

        for (index, path) in to_extract {
            let FileBlock {
                uncompressed_len,
                mut data,
                ..
            } = self.read_file_block(index)?;

            if uncompressed_len.is_some() {
                let mut buf = Vec::new();
                data.read_to_end(&mut buf)?;
                let data = buf;

                let errors_tx = errors_tx.clone();

                pool.execute(move || match decompress_to(data, &path, A::COMPRESSION) {
                    Ok(()) => {}
                    Err(e) => errors_tx.send(e).unwrap(),
                });
            } else {
                let mut f = fs::File::create(path)?;
                io::copy(&mut data, &mut f)?;
            }
        }
        pool.join();
        if let Ok(e) = errors_rx.try_recv() {
            return Err(e);
        };
        Ok(())
    }

    fn extract_inner(&mut self, dir: &Path) -> Result<()> {
        let to_extract: Vec<_> = self
            .entries()
            .map(|entry| (entry.index(), dir.join(entry.path())))
            .collect();

        for (index, path) in to_extract {
            let FileBlock {
                uncompressed_len,
                mut data,
                ..
            } = self.read_file_block(index)?;

            if uncompressed_len.is_some() {
                let mut buf = Vec::new();
                data.read_to_end(&mut buf)?;
                let data = buf;
                decompress_to(data, &path, A::COMPRESSION)?;
            } else {
                let mut f = fs::File::create(path)?;
                io::copy(&mut data, &mut f)?;
            }
        }

        Ok(())
    }
}

fn decompress_to(raw: Vec<u8>, path: &Path, compression: Compression) -> Result<()> {
    let parent = path.parent().unwrap();
    fs::create_dir_all(parent)?;
    let mut out = fs::File::create(&path)?;

    match compression {
        Compression::Zlib => {
            let mut decoder = ZlibDecoder::new(Cursor::new(raw));
            io::copy(&mut decoder, &mut out)?;
        }
        Compression::Lz4 => {
            let mut decoder = frame::FrameDecoder::new(Cursor::new(raw));
            io::copy(&mut decoder, &mut out)?;
        }
    }
    Ok(())
}

struct FileBlock<'a> {
    _embedded_name: Option<String>,
    uncompressed_len: Option<u32>,
    data: io::Take<&'a mut dyn Read>,
}

struct FileBlock2 {
    _embedded_name: Option<String>,
    uncompressed_len: Option<u32>,
    data: Vec<u8>,
}

pub struct Entries<'a, A, R>
where
    A: Bsa,
    R: Read + Seek,
{
    archive: &'a BsaArchive<A, R>,
    dir_index: usize,
    files: Enumerate<slice::Iter<'a, File>>,
}

impl<'a, A, R> Iterator for Entries<'a, A, R>
where
    A: Bsa,
    R: Read + Seek,
{
    type Item = Entry<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some((i, file)) = self.files.next() {
                let dir = &self.archive.dirs[self.dir_index];
                let index = ArchiveIndex {
                    file: i as u32,
                    folder: self.dir_index as u32,
                };
                return Some(Entry { dir, file, index });
            }
            self.dir_index += 1;
            if self.archive.dirs.len() <= self.dir_index {
                return None;
            }
            self.files = self.archive.dirs[self.dir_index].files.iter().enumerate();
        }
    }
}

pub struct Entry<'a> {
    dir: &'a Dir,
    file: &'a File,
    index: ArchiveIndex,
}

impl<'a> Entry<'a> {
    /// Get the entry's index.
    pub fn index(&self) -> ArchiveIndex {
        self.index
    }

    pub fn path(&self) -> PathBuf {
        let mut dir = self.dir.name.clone().unwrap();
        let file = self.file.name.as_ref().unwrap();

        unsafe {
            // SAFETY: We are only replacing ascii bytes with other ascii bytes.
            for byte in dir.as_bytes_mut() {
                if *byte == b'\\' {
                    *byte = path::MAIN_SEPARATOR as u8;
                }
            }
        }

        let mut path = PathBuf::from(dir);
        path.push(file);

        assert!(path.is_relative());
        assert!(path
            .components()
            .all(|component| matches!(component, Component::Normal(_))));

        path
    }

    #[inline]
    pub fn file_name(&self) -> &str {
        self.file.name.as_ref().unwrap()
    }

    #[inline]
    pub fn dir_name(&self) -> &str {
        self.dir.name.as_ref().unwrap()
    }

    #[inline]
    pub fn file_hash(&self) -> Hash {
        self.file.hash
    }

    #[inline]
    pub fn dir_hash(&self) -> Hash {
        self.dir.hash
    }
}

fn read_bzstring(r: &mut dyn Read) -> Result<String> {
    let name = read_bstring_inner(r)?;

    if let Some(i) = memchr(b'\0', &name) {
        if i != name.len() - 1 {
            return Err(ArchiveReadError::EmbeddedNul.into());
        }
    } else {
        return Err(ArchiveReadError::MissingNul.into());
    }

    let mut name = name;
    name.pop(); // remove nul
    Ok(windows_1252::to_string(name))
}

fn read_bstring(r: &mut dyn Read) -> Result<String> {
    let name = read_bstring_inner(r)?;

    if name.contains(&b'\0') {
        return Err(ArchiveReadError::EmbeddedNul.into());
    }

    Ok(windows_1252::to_string(name))
}

fn read_bstring_inner(r: &mut dyn Read) -> Result<Vec<u8>> {
    let mut len = [0; 1];
    r.read_exact(&mut len)?;
    let len = len[0] as usize;
    Ok(read_vec(r, len)?)
}

#[inline]
fn compute_file_name_block_offset<A>(header: &Header<A>) -> usize
where
    A: Bsa,
{
    let mut offset = mem::size_of::<RawHeader>();
    offset += header.folder_count as usize * mem::size_of::<A::FolderRecord>();
    offset += header.file_count as usize * mem::size_of::<FileRecord>();
    if header.include_dirnames() {
        offset += header.total_folder_name_length as usize;
        offset += header.folder_count as usize; // length prefixes
    }
    offset
}

#[derive(Debug)]
struct Dir {
    name: Option<String>,
    hash: Hash,
    files: Vec<File>,
}

#[derive(Debug)]
struct File {
    hash: Hash,
    name: Option<String>,
    offset: u32,
    raw_size: u32,
    compressed: bool,
}
