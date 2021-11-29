use std::{
    borrow::Cow,
    convert::TryInto,
    fs::{self, File},
    io::{self, BufRead, BufReader, BufWriter, Write},
    marker::PhantomData,
    mem,
    path::{self, Path, PathBuf},
};

use flate2::bufread::ZlibDecoder;
use lz4_flex::frame::FrameDecoder;
use memchr::memchr;
use memmap2::Mmap;
use rayon::iter::{
    IndexedParallelIterator, IntoParallelIterator, IntoParallelRefIterator, ParallelIterator,
};
use threadpool::ThreadPool;

use super::{Bsa, FileRecord, FolderRecord, Header, RawHeader};
use crate::{
    common::{windows_1252, Bytes},
    tes4::Compression,
    ArchiveReadError, Result,
};

pub struct RawBsa<'a, A>
where
    A: Bsa,
{
    pub header: Header<A>,
    pub folder_records: &'a [A::FolderRecord],
    pub file_record_blocks: FileRecordBlocks<'a, A>,
    pub file_names_block: Option<FileNamesBlock<'a>>,
    pub file_blocks: FileBlocks<'a, A>,
}

impl<A> RawBsa<'_, A>
where
    A: Bsa,
{
    pub fn new(bytes: &[u8]) -> Result<RawBsa<A>> {
        let mut r = Bytes(bytes);

        let raw_header: RawHeader = *r.read::<RawHeader>()?;
        let header: Header<A> = raw_header
            .try_into()
            .map_err(|_| ArchiveReadError::BadHeader)
            .unwrap();

        let folder_records = r.read_slice(header.folder_count as usize)?;

        let mut file_record_blocks_len = mem::size_of::<FileRecord>() as u32 * header.file_count;
        if header.include_dirnames() {
            file_record_blocks_len += header.folder_count + header.total_folder_name_length;
        }
        let file_record_blocks = r.read_bytes(file_record_blocks_len as usize)?;
        let file_record_blocks = FileRecordBlocks {
            bytes: file_record_blocks,
            _marker: PhantomData::default(),
            include_dirnames: header.include_dirnames(),
            offset_delta: mem::size_of::<RawHeader>() as u32
                + mem::size_of::<A::FolderRecord>() as u32 * header.folder_count,
            total_file_name_len: header.total_file_name_length,
        };

        let file_names_block = if header.include_filenames() {
            let bytes = r.read_bytes(header.total_file_name_length as usize)?;
            Some(FileNamesBlock { bytes })
        } else {
            None
        };

        let file_blocks = FileBlocks {
            _marker: PhantomData,
            data: r.0,
            default_compressed: header.compressed(),
            embedded_names: header.embed_filenames(),
            offset_delta: (bytes.len() - r.len()) as u32,
        };

        Ok(RawBsa {
            file_record_blocks,
            file_names_block,
            folder_records,
            header,
            file_blocks,
        })
    }

    pub fn extract_st<P>(&self, dir: P) -> Result<()>
    where
        P: AsRef<Path>,
    {
        self.extract_st_inner(dir.as_ref())
    }

    fn extract_st_inner(&self, dir: &Path) -> Result<()> {
        let mut file_names = self.file_names_block.as_ref().map(|block| block.iter());

        for folder_record in self.folder_records {
            let file_records_block = self.file_record_blocks.get(folder_record)?;
            let folder_name: &str = file_records_block
                .name
                .as_ref()
                .ok_or(ArchiveReadError::BadArchive)?;

            let folder_name = if !path::is_separator(b'\\' as char) {
                let mut name = folder_name.to_owned();
                unsafe {
                    for byte in name.as_bytes_mut() {
                        if *byte == b'\\' {
                            *byte = path::MAIN_SEPARATOR as u8;
                        }
                    }
                }
                Cow::Owned(name)
            } else {
                Cow::Borrowed(folder_name)
            };

            let folder_path = dir.join(folder_name.as_ref());
            fs::create_dir_all(&folder_path)?;

            for file_record in file_records_block.file_records {
                let file_name = file_names
                    .as_mut()
                    .ok_or(ArchiveReadError::BadArchive)?
                    .next()
                    .ok_or(ArchiveReadError::BadArchive)
                    .unwrap()?;
                let file_block = self.file_blocks.get(file_record)?;

                let mut compressed = self.header.compressed();
                if file_record.negate_compression() {
                    compressed = !compressed;
                }

                let path = folder_path.join(file_name.as_ref());
                let mut f = fs::File::create(path)?;

                if compressed {
                    match A::COMPRESSION {
                        Compression::Zlib => {
                            let mut decoder = ZlibDecoder::new(file_block.raw_data);
                            io::copy(&mut decoder, &mut f)?;
                        }
                        Compression::Lz4 => {
                            let mut decoder = FrameDecoder::new(file_block.raw_data);
                            io::copy(&mut decoder, &mut f)?;
                        }
                    }
                } else {
                    f.write_all(file_block.raw_data)?;
                }
            }
        }

        Ok(())
    }
}

pub struct FileRecordBlocks<'a, A>
where
    A: Bsa,
{
    bytes: &'a [u8],
    offset_delta: u32,
    total_file_name_len: u32,
    include_dirnames: bool,
    _marker: PhantomData<A>,
}

impl<'a, A> FileRecordBlocks<'a, A>
where
    A: Bsa,
{
    pub fn get(&self, folder_record: &A::FolderRecord) -> Result<FileRecordBlock> {
        let offset = folder_record
            .offset()
            .checked_sub(self.total_file_name_len)
            .and_then(|offset| offset.checked_sub(self.offset_delta))
            .ok_or(ArchiveReadError::BadOffset)
            .unwrap();
        let offset = offset as usize;

        if self.bytes.len() < offset {
            return Err(ArchiveReadError::BadOffset.into());
        }

        let mut bytes = Bytes(&self.bytes[offset..]);
        let name = if self.include_dirnames {
            Some(read_bzstring(&mut bytes)?)
        } else {
            None
        };

        let size_of_file_records = (folder_record.count() as usize)
            .checked_mul(mem::size_of::<FileRecord>())
            .ok_or(ArchiveReadError::BadArchive)?;

        if bytes.len() < size_of_file_records {
            return Err(ArchiveReadError::BadArchive.into());
        }

        let file_records = &bytes.0[..size_of_file_records];
        let file_records = bytemuck::cast_slice(file_records);

        Ok(FileRecordBlock { name, file_records })
    }
}

pub struct FileRecordBlock<'a> {
    name: Option<Cow<'a, str>>,
    file_records: &'a [FileRecord],
}

pub struct FileNamesBlock<'a> {
    bytes: &'a [u8],
}

impl FileNamesBlock<'_> {
    pub fn iter(&self) -> FileNamesBlockIter {
        FileNamesBlockIter { bytes: self.bytes }
    }
}

pub struct FileNamesBlockIter<'a> {
    bytes: &'a [u8],
}

impl<'a> FileNamesBlockIter<'a> {
    fn read_name(&mut self) -> Result<Cow<'a, str>> {
        let len = memchr(b'\0', self.bytes).ok_or(ArchiveReadError::MissingNul)?;
        let name = &self.bytes[..len];
        self.bytes = &self.bytes[len + 1..];
        Ok(windows_1252::to_str(name))
    }
}

impl<'a> Iterator for FileNamesBlockIter<'a> {
    type Item = Result<Cow<'a, str>>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.bytes.is_empty() {
            None
        } else {
            Some(self.read_name())
        }
    }
}

pub struct FileBlocks<'a, A>
where
    A: Bsa,
{
    data: &'a [u8],
    offset_delta: u32,
    default_compressed: bool,
    embedded_names: bool,
    _marker: PhantomData<A>,
}

impl<A> FileBlocks<'_, A>
where
    A: Bsa,
{
    pub fn get(&self, file_record: &FileRecord) -> Result<FileBlock> {
        let offset = file_record
            .offset()
            .checked_sub(self.offset_delta)
            .ok_or(ArchiveReadError::BadOffset)?;
        let offset = offset as usize;

        let len = file_record.size() as usize;
        let mut data = Bytes(self.data);
        data.skip(offset)?;
        let mut data = Bytes(data.read_bytes(len)?);

        let embedded_name = if self.embedded_names {
            Some(read_bstring(&mut data)?)
        } else {
            None
        };

        let mut compressed = self.default_compressed;
        if file_record.negate_compression() {
            compressed = !compressed;
        }

        let uncompressed_len = if compressed {
            let buf = data.read_bytes(4)?.try_into().unwrap();
            Some(u32::from_le_bytes(buf))
        } else {
            None
        };

        let raw_data = data.0;

        Ok(FileBlock {
            embedded_name,
            uncompressed_len,
            raw_data,
        })
    }
}

pub struct FileBlock<'a> {
    pub embedded_name: Option<Cow<'a, str>>,
    pub uncompressed_len: Option<u32>,
    pub raw_data: &'a [u8],
}

fn read_bstring<'a>(bytes: &mut Bytes<'a>) -> Result<Cow<'a, str>> {
    let bytes = read_bstring_bytes(bytes)?;
    if bytes.contains(&b'\0') {
        return Err(ArchiveReadError::EmbeddedNul.into());
    }
    Ok(windows_1252::to_str(bytes))
}

fn read_bzstring<'a>(bytes: &mut Bytes<'a>) -> Result<Cow<'a, str>> {
    let bytes = read_bstring_bytes(bytes)?;

    match memchr(b'\0', bytes) {
        Some(i) => {
            if i != bytes.len() - 1 {
                return Err(ArchiveReadError::EmbeddedNul.into());
            }
        }
        None => return Err(ArchiveReadError::MissingNul.into()),
    }
    let bytes = &bytes[..bytes.len() - 1];
    Ok(windows_1252::to_str(bytes))
}

fn read_bstring_bytes<'a>(bytes: &mut Bytes<'a>) -> Result<&'a [u8]> {
    fn inner<'a>(bytes: &mut Bytes<'a>) -> Result<&'a [u8]> {
        let len: u8 = *bytes.read()?;
        Ok(bytes.read_bytes(len as usize)?)
    }

    Ok(inner(bytes).map_err(|_| ArchiveReadError::BadArchive)?)
}

pub struct OwnedBsa<A>
where
    A: Bsa,
{
    data: OwnedData,
    raw: RawBsa<'static, A>,
}

impl<A> OwnedBsa<A>
where
    A: Bsa,
{
    pub fn open<P>(path: P) -> Result<OwnedBsa<A>>
    where
        P: AsRef<Path>,
    {
        let f = File::open(path)?;
        let map = unsafe { Mmap::map(&f)? };
        let raw: RawBsa<A> = RawBsa::new(&map)?;
        let raw: RawBsa<'static, A> = unsafe { mem::transmute(raw) };

        Ok(OwnedBsa {
            data: OwnedData::Mmap(map),
            raw,
        })
    }

    pub fn open_memory(buf: Vec<u8>) -> Result<OwnedBsa<A>> {
        let buf = buf.into_boxed_slice();
        let raw: RawBsa<A> = RawBsa::new(&buf)?;
        let raw: RawBsa<'static, A> = unsafe { mem::transmute(raw) };

        Ok(OwnedBsa {
            data: OwnedData::Box(buf),
            raw,
        })
    }

    pub fn extract_st<P: AsRef<Path>>(&self, dir: P) -> Result<()> {
        self.raw.extract_st(dir)
    }

    pub fn extract_mt<P: AsRef<Path>>(&self, dir: P) -> Result<()> {
        let dir = dir.as_ref();

        let mut file_names = self.raw.file_names_block.as_ref().map(|block| block.iter());

        for folder_record in self.raw.folder_records {
            let file_records_block = self.raw.file_record_blocks.get(folder_record)?;
            let folder_name: &str = file_records_block
                .name
                .as_ref()
                .ok_or(ArchiveReadError::BadArchive)?;

            let folder_name = if !path::is_separator(b'\\' as char) {
                let mut name = folder_name.to_owned();
                unsafe {
                    for byte in name.as_bytes_mut() {
                        if *byte == b'\\' {
                            *byte = path::MAIN_SEPARATOR as u8;
                        }
                    }
                }
                Cow::Owned(name)
            } else {
                Cow::Borrowed(folder_name)
            };

            let folder_path = dir.join(folder_name.as_ref());
            fs::create_dir_all(&folder_path)?;

            let mut files = Vec::new();

            for file_record in file_records_block.file_records {
                let file_name = file_names
                    .as_mut()
                    .ok_or(ArchiveReadError::BadArchive)?
                    .next()
                    .ok_or(ArchiveReadError::BadArchive)
                    .unwrap()?;
                let file_block = self.raw.file_blocks.get(file_record)?;

                let mut compressed = self.raw.header.compressed();
                if file_record.negate_compression() {
                    compressed = !compressed;
                }

                files.push((file_block, compressed, folder_path.join(file_name.as_ref())));
            }

            files
                .into_par_iter()
                .try_for_each(|(file_block, compressed, path)| -> Result<()> {
                    let mut f = BufWriter::new(fs::File::create(path)?);
                    let data = file_block.raw_data;

                    if compressed {
                        match A::COMPRESSION {
                            Compression::Zlib => {
                                let mut decoder = ZlibDecoder::new(data);
                                io::copy(&mut decoder, &mut f)?;
                            }
                            Compression::Lz4 => {
                                let mut decoder = FrameDecoder::new(data);
                                io::copy(&mut decoder, &mut f)?;
                            }
                        }
                    } else {
                        f.write_all(data)?;
                    }
                    Ok(())
                })?;
        }

        Ok(())
    }

    // fn decompress_file(&self, file_record: FileRecord, path: PathBuf) -> Result<()> {
    //     let mut f = File::create(path)?;
    //     if compressed {
    //         match A::COMPRESSION {
    //             Compression::Zlib => {
    //                 let mut decoder = ZlibDecoder::new(file_block.raw_data);
    //                 io::copy(&mut decoder, &mut f)?;
    //             }
    //             Compression::Lz4 => {
    //                 let mut decoder = FrameDecoder::new(file_block.raw_data);
    //                 io::copy(&mut decoder, &mut f)?;
    //             }
    //         }
    //     } else {
    //         f.write_all(file_block.raw_data)?;
    //     }
    // }
}

enum OwnedData {
    Mmap(Mmap),
    Box(Box<[u8]>),
}
