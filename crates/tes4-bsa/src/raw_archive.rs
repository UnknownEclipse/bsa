use std::{
    borrow::Cow,
    cell::RefCell,
    fs,
    io::{self, Cursor, Read, Seek, SeekFrom, Write},
    mem,
    ops::{Deref, DerefMut},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicUsize, Ordering},
        mpsc::channel,
        Arc, Mutex, RwLock,
    },
};

use bitflags::bitflags;
use bsa_core::{detail::EntriesImpl, helpers::read_vec, ReadError};
use bytes::Bytes;
use flate2::bufread::ZlibDecoder;
use lz4_flex::frame::FrameDecoder;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use threadpool::ThreadPool;

use crate::{
    archive::Index,
    bytes::BytesExt,
    common::read_vec_at,
    hash::{hash_file_path, Hash},
    read_at::ReadAt,
    Bsa, BsaArchive, Compression, Result, Version,
};

pub struct RawArchive<R>
where
    R: ?Sized,
{
    pub version: Version,
    pub embed_file_names: bool,
    pub dirs: Vec<Dir>,
    pub reader: RefCell<R>,
}

pub struct Dir {
    pub name: String,
    pub hash: Hash,
    pub files: Vec<File>,
}

pub struct File {
    pub name: String,
    pub hash: Hash,
    pub block_len: u32,
    pub block_offset: u32,
    pub compression: Option<Compression>,
}

impl<R> RawArchive<R>
where
    R: Read + Seek,
{
    pub fn new(mut r: R) -> Result<RawArchive<R>> {
        let mut header = [0; 36];
        r.read_exact(&mut header)?;
        let header = Header::from_bytes(header).ok_or(ReadError::InvalidHeader)?;

        let folder_record_len = if header.version == Version::V105 {
            24
        } else {
            16
        };

        let folder_records = read_vec(&mut r, header.folder_count as usize * folder_record_len)?;

        let folder_records = folder_records.chunks_exact(folder_record_len).map(|bytes| {
            if header.version == Version::V105 {
                FolderRecord::from_bytes_sse(bytes.try_into().unwrap())
            } else {
                FolderRecord::from_bytes_tes4(bytes.try_into().unwrap())
            }
        });

        let file_record_blocks_len =
            header.file_count * 16 + header.folder_count + header.total_folder_name_len;

        let file_record_blocks = read_vec(&mut r, file_record_blocks_len as usize)?;
        let file_names_block = read_vec(&mut r, header.total_file_name_len as usize)?;

        let mut file_record_blocks = Bytes::new(&file_record_blocks);
        let mut file_names_block = Bytes::new(&file_names_block);

        let mut dirs = Vec::new();

        let default_compressed = header.archive_flags.contains(ArchiveFlags::COMPRESSED);

        let compression = match header.version {
            Version::V103 | Version::V104 => Compression::Zlib,
            Version::V105 => Compression::Lz4,
        };

        for folder_record in folder_records {
            let name = file_record_blocks.read_bzstring()?.replace('\\', "/");
            let file_records = file_record_blocks
                .read_bytes(folder_record.count as usize * 16)
                .map_err(|_| ReadError::Eof)?;

            let mut files = Vec::new();

            for bytes in file_records.chunks_exact(16) {
                let bytes = bytes.try_into().unwrap();
                let file_record = FileRecord::from_bytes(bytes);
                let name = file_names_block.read_zstring()?.into_owned();

                let compressed = if file_record.len & (1 << 30) != 0 {
                    !default_compressed
                } else {
                    default_compressed
                };

                let compression = if compressed { Some(compression) } else { None };

                let file = File {
                    name,
                    compression,
                    hash: file_record.hash,
                    block_len: file_record.len,
                    block_offset: file_record.offset,
                };
                files.push(file);
            }

            let dir = Dir {
                name,
                hash: folder_record.hash,
                files,
            };
            dirs.push(dir);
        }

        let embed_file_names = header.version != Version::V103
            && header.archive_flags.contains(ArchiveFlags::EMBED_FILENAMES);

        let reader = RefCell::new(r);

        Ok(RawArchive {
            version: header.version,
            embed_file_names,
            reader,
            dirs,
        })
    }

    pub fn find_file_by_name(&self, name: &str) -> Option<Index> {
        let (folder_hash, file_hash) = hash_file_path(name)?;
        let dir_index = self
            .dirs
            .binary_search_by_key(&folder_hash, |dir| dir.hash)
            .ok()?;

        let dir = &self.dirs[dir_index];
        let file_index = dir
            .files
            .binary_search_by_key(&file_hash, |file| file.hash)
            .ok()?;

        let index = Index {
            folder: dir_index as u32,
            file: file_index as u32,
        };

        Some(index)
    }

    pub fn extract1<P: AsRef<Path>>(&self, dir: P) -> Result<()> {
        self._extract1(dir.as_ref())
    }

    pub fn extract2<P: AsRef<Path>>(&self, dir: P) -> Result<()> {
        self._extract2(dir.as_ref())
    }

    pub fn extract3<P: AsRef<Path>>(&self, dir: P) -> Result<()> {
        self._extract3(dir.as_ref())
    }

    fn file_block(&self, file: &File) -> Result<FileBlock> {
        let mut r = self.reader.borrow_mut();
        let pos = SeekFrom::Start(file.block_offset as u64);
        r.seek(pos)?;

        let data = read_vec(r.deref_mut(), file.block_len as usize)?;

        FileBlock::from_bytes(data, file.compression.is_some(), self.embed_file_names)
    }

    fn get(&self, index: Index) -> (&Dir, &File) {
        let dir = &self.dirs[index.folder as usize];
        let file = &dir.files[index.file as usize];
        (dir, file)
    }

    /// Extraction by simply reading through the archive in a single threaded fashion.
    ///
    /// This is by far the slowest, as reading is dependent on both decompression
    /// finishing and the file to be written to disk. The other strategies below
    /// remove this dependency.
    fn _extract1(&self, out: &Path) -> Result<()> {
        for dir in &self.dirs {
            let folder_path = out.join(&dir.name);
            fs::create_dir_all(&folder_path)?;

            for file in &dir.files {
                let path = folder_path.join(&file.name);
                let file_block = self.file_block(file)?;
                save_file(file_block, &path, file.compression)?;
            }
        }

        Ok(())
    }

    /// Extraction by reading file data into in-memory buffers. These buffers are passed
    /// to a threadpool for to be decompressed and written to a file. (Decompression
    /// and writing occurs in the same task)
    fn _extract2(&self, out: &Path) -> Result<()> {
        let pool = ThreadPool::new(num_cpus::get());
        let (sender, receiver) = channel();

        for dir in &self.dirs {
            let folder_path = out.join(&dir.name);
            fs::create_dir_all(&folder_path)?;

            for file in &dir.files {
                let file_block = self.file_block(file)?;
                let compression = file.compression;
                let path = folder_path.join(&file.name);
                let sender = sender.clone();

                pool.execute(move || {
                    if let Err(e) = save_file(file_block, &path, compression) {
                        sender.send(e).unwrap();
                    }
                });
            }
        }

        pool.join();

        if let Ok(err) = receiver.try_recv() {
            Err(err)
        } else {
            Ok(())
        }
    }

    /// Extraction by reading all data into in-memory buffers. These buffers are passed
    /// to a threadpool for decompression. **Once reading is done** (meaning the all the
    /// data in the archive will be stored in memory), the decompressed
    /// buffers are passed back to the threadpool to be written to files. (Decompression
    /// and writing occurs in different tasks). The idea behind this strategy is to
    /// prevent mechanical drives from needing to seek to other positions, and can do
    /// only sequential reads without "getting distracted". This still needs testing,
    /// but is similar in performance to method2 on a very fast ssd (althought with
    /// much higher memory usage).
    fn _extract3(&self, out: &Path) -> Result<()> {
        let decompress_pool = ThreadPool::new(num_cpus::get());
        let (errors_tx, errors_rx) = channel();
        let (uncompressed_tx, uncompressed_rx) = channel();

        for dir in &self.dirs {
            let folder_path = out.join(&dir.name);

            for file in &dir.files {
                let file_block = self.file_block(file)?;
                let compression = file.compression;
                let path = folder_path.join(&file.name);
                let errors_tx = errors_tx.clone();
                let uncompressed_tx = uncompressed_tx.clone();

                decompress_pool.execute(move || match decompress(file_block, compression) {
                    Ok(data) => uncompressed_tx.send((path, data)).unwrap(),
                    Err(e) => errors_tx.send(e).unwrap(),
                });
            }
        }

        mem::drop(uncompressed_tx);

        let write_pool = ThreadPool::new(64);

        while let Ok((path, uncompressed_data)) = uncompressed_rx.recv() {
            let errors_tx = errors_tx.clone();

            write_pool.execute(move || {
                let data = uncompressed_data.get_ref();
                let off = uncompressed_data.position();
                let data = &data[off as usize..];

                if let Err(err) = fs::create_dir_all(path.parent().unwrap()) {
                    errors_tx.send(err.into()).unwrap();
                } else if let Err(err) = fs::write(path, data) {
                    errors_tx.send(err.into()).unwrap();
                }
            });
        }

        write_pool.join();

        if let Ok(err) = errors_rx.try_recv() {
            Err(err)
        } else {
            Ok(())
        }
    }
}

impl<R: ReadAt + Sync> RawArchive<R> {
    pub fn extract4<P: AsRef<Path>>(&self, out: P) -> Result<()> {
        self._extract4(out.as_ref())
    }

    fn _extract4(&self, out: &Path) -> Result<()> {
        let reader = self.reader.borrow();
        let reader = reader.deref();
        let embed_filenames = self.embed_file_names;

        self.dirs
            .par_iter()
            .flat_map(|dir| dir.files.par_iter().map(|file| (dir.name.clone(), file)))
            .try_for_each(|(dirname, file)| -> Result<()> {
                let data = read_vec_at(reader, file.block_len as usize, file.block_offset as u64)?;
                let file_block =
                    FileBlock::from_bytes(data, file.compression.is_some(), embed_filenames)?;
                let mut path = out.join(dirname);
                fs::create_dir_all(&path)?;
                path.push(&file.name);

                save_file(file_block, &path, file.compression)?;

                Ok(())
            })?;

        Ok(())
    }
}

impl<A, R> EntriesImpl<BsaArchive<A, R>> for RawArchive<R>
where
    A: Bsa,
    R: Read + Seek,
{
    fn next(&self, mut index: Index) -> Option<Index> {
        loop {
            let dirs = &self.dirs;
            if index.folder as usize >= dirs.len() {
                return None;
            }
            let dir = &dirs[index.folder as usize];
            index.file += 1;
            if (index.file as usize) < dir.files.len() {
                return Some(index);
            } else {
                index.folder += 1;
                index.file = 0;
            }
        }
    }

    fn name(&self, index: Index) -> Cow<str> {
        let (dir, file) = self.get(index);
        let mut name = dir.name.clone();
        name.push('/');
        name.push_str(&file.name);
        name.into()
    }

    fn extract_to(&self, index: Index, writer: &mut dyn Write) -> Result<()> {
        let (_, file) = self.get(index);
        let file_block = self.file_block(file)?;
        save_file_to(file_block, file.compression, writer)
    }

    fn extract(&self, index: Index, path: &Path) -> Result<()> {
        let (_, file) = self.get(index);
        let file_block = self.file_block(file)?;
        save_file(file_block, path, file.compression)
    }
}

fn save_file(file_block: FileBlock, path: &Path, compression: Option<Compression>) -> Result<()> {
    let mut f = fs::File::create(path)?;
    save_file_to(file_block, compression, &mut f)
}

fn decompress(file_block: FileBlock, compression: Option<Compression>) -> Result<Cursor<Vec<u8>>> {
    let uncompressed_len = file_block.uncompressed_len;

    match compression {
        Some(Compression::Zlib) => {
            let mut decoder = ZlibDecoder::new(file_block.into_raw_data());
            let buf = read_vec(&mut decoder, uncompressed_len.unwrap() as usize)?;
            Ok(Cursor::new(buf))
        }
        Some(Compression::Lz4) => {
            let mut decoder = FrameDecoder::new(file_block.into_raw_data());
            let buf = read_vec(&mut decoder, uncompressed_len.unwrap() as usize)?;
            Ok(Cursor::new(buf))
        }
        None => Ok(file_block.into_raw_data()),
    }
}

fn save_file_to<W: ?Sized + Write>(
    file_block: FileBlock,
    compression: Option<Compression>,
    out: &mut W,
) -> Result<()> {
    match compression {
        Some(Compression::Zlib) => {
            let mut decoder = ZlibDecoder::new(file_block.into_raw_data());
            io::copy(&mut decoder, out)?;
        }
        Some(Compression::Lz4) => {
            let mut decoder = FrameDecoder::new(file_block.into_raw_data());
            io::copy(&mut decoder, out)?;
        }
        None => out.write_all(file_block.raw_data())?,
    }

    Ok(())
}

struct FileBlock {
    embedded_name_len: Option<u8>,
    uncompressed_len: Option<u32>,
    data: Vec<u8>,
}

impl FileBlock {
    pub fn from_bytes(data: Vec<u8>, compressed: bool, embed_filenames: bool) -> Result<FileBlock> {
        let mut r = Bytes::new(&data);

        let embedded_name_len = if embed_filenames {
            let len = r.read_bytes(1).map_err(|_| ReadError::Eof)?[0];
            let len = len as usize;
            let bytes = r.read_bytes(len).map_err(|_| ReadError::Eof)?;
            if bytes.contains(&b'\0') {
                return Err(ReadError::EmbeddedNul.into());
            }
            Some(len as u8)
        } else {
            None
        };

        let uncompressed_len = if compressed {
            let buf = r
                .read_bytes(4)
                .map_err(|_| ReadError::Eof)?
                .try_into()
                .unwrap();
            Some(u32::from_le_bytes(buf))
        } else {
            None
        };

        Ok(FileBlock {
            uncompressed_len,
            embedded_name_len,
            data,
        })
    }

    pub fn raw_data(&self) -> &[u8] {
        let mut offset = 0;
        if let Some(len) = self.embedded_name_len {
            offset += 1;
            offset += len as usize;
        }
        if self.uncompressed_len.is_some() {
            offset += 4;
        }
        &self.data[offset..]
    }

    pub fn into_raw_data(self) -> Cursor<Vec<u8>> {
        let mut offset = 0;
        if let Some(len) = self.embedded_name_len {
            offset += 1;
            offset += len as usize;
        }
        if self.uncompressed_len.is_some() {
            offset += 4;
        }

        let mut data = Cursor::new(self.data);
        data.set_position(offset as u64);
        data
    }
}

const MAGIC: &[u8] = b"BSA\0";

struct Header {
    pub version: Version,
    pub archive_flags: ArchiveFlags,
    pub folder_count: u32,
    pub file_count: u32,
    pub total_folder_name_len: u32,
    pub total_file_name_len: u32,
    pub file_flags: FileFlags,
}

impl Header {
    pub fn from_bytes(bytes: [u8; 36]) -> Option<Header> {
        let mut chunks = bytes.chunks(4);

        let magic = chunks.next().unwrap();
        if magic != MAGIC {
            return None;
        }

        let mut next_u32 = || u32::from_le_bytes(chunks.next().unwrap().try_into().unwrap());
        let version = next_u32();
        let version = match version {
            103 => Version::V103,
            104 => Version::V104,
            105 => Version::V105,
            _ => return None,
        };
        let offset = next_u32();
        if offset != 36 {
            return None;
        }
        let archive_flags = next_u32();
        let archive_flags = ArchiveFlags::from_bits(archive_flags)?;
        let folder_count = next_u32();
        let file_count = next_u32();
        let total_folder_name_len = next_u32();
        let total_file_name_len = next_u32();
        let file_flags = (next_u32() & 0xFFFF) as u16;
        let file_flags = FileFlags::from_bits(file_flags)?;

        Some(Header {
            version,
            archive_flags,
            folder_count,
            file_count,
            total_file_name_len,
            total_folder_name_len,
            file_flags,
        })
    }
}

struct FolderRecord {
    pub hash: Hash,
    pub count: u32,
    pub offset: u32,
}

impl FolderRecord {
    pub fn from_bytes_tes4(bytes: [u8; 16]) -> FolderRecord {
        let hash = Hash::from_bytes(bytes[..8].try_into().unwrap());
        let count = u32::from_le_bytes(bytes[8..12].try_into().unwrap());
        let offset = u32::from_le_bytes(bytes[12..].try_into().unwrap());
        FolderRecord {
            hash,
            count,
            offset,
        }
    }

    pub fn from_bytes_sse(bytes: [u8; 24]) -> FolderRecord {
        let hash = Hash::from_bytes(bytes[..8].try_into().unwrap());
        let count = u32::from_le_bytes(bytes[8..12].try_into().unwrap());
        let offset = u32::from_le_bytes(bytes[16..20].try_into().unwrap());
        FolderRecord {
            hash,
            count,
            offset,
        }
    }
}

struct FileRecord {
    pub hash: Hash,
    pub len: u32,
    pub offset: u32,
}

impl FileRecord {
    pub fn from_bytes(bytes: [u8; 16]) -> FileRecord {
        let hash = Hash::from_bytes(bytes[..8].try_into().unwrap());
        let len = u32::from_le_bytes(bytes[8..12].try_into().unwrap());
        let offset = u32::from_le_bytes(bytes[12..].try_into().unwrap());
        FileRecord { hash, len, offset }
    }
}

bitflags! {
    pub struct ArchiveFlags: u32 {
        const INCLUDE_DIRNAMES = 0x1;
        const INCLUDE_FILENAMES = 0x2;
        const COMPRESSED = 0x4;
        const RETAIN_DIRNAMES = 0x8;
        const RETAIN_FILENAMES = 0x10;
        const RETAIN_FILENAME_OFFSETS = 0x20;
        const XBOX360 = 0x40;
        const RETAIN_STRINGS = 0x80;
        const EMBED_FILENAMES = 0x100;
        const XMEM = 0x200;
    }
}

bitflags! {
    pub struct FileFlags: u16 {
        const MESHES = 0x1;
        const TEXTURES = 0x2;
        const MENUS = 0x4;
        const SOUNDS = 0x8;
        const VOICES = 0x10;
        const SHADERS = 0x20;
        const TREES = 0x40;
        const FONTS = 0x80;
        const MISC = 0x100;
    }
}
