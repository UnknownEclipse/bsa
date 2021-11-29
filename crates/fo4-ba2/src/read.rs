use std::{
    cell::RefCell,
    convert::{TryFrom, TryInto},
    io::{Read, Seek, SeekFrom},
    mem,
    num::NonZeroU32,
    ops::DerefMut,
    slice,
};

use smallvec::SmallVec;

use crate::{
    chunk_data::ChunkData,
    common::{read_pod, read_smallvec, read_vec, read_wstring},
    raw::{
        DirectXChunkData, DirectXChunkHeader, Format, GeneralChunkData, GeneralChunkHeader, Header,
        RawDirectXChunkData, RawDirectXChunkHeader, RawGeneralChunkData, RawGeneralChunkHeader,
        RawHeader,
    },
    Result,
};

/// The Fallout 4 BA2 archive.
///
/// Fallout 4 BA2's contain a number of file entries, each divided into a number of
/// chunks. There are several different formats supported, currently GNRL and DX10.
/// GNRL (General) archives contain the majority of files the game uses. DX10 archives,
/// on the other hand, are only used for textures, but the split chunk strategy
/// allows the game to stream in mipmaps on demand, improving performance.
///
/// # Examples
/// Open an archive and list the files contained, skipping any files that do not have
/// a name.
/// ```
/// use std::{fs::File, path::Path};
///
/// use fo4_ba2::{Ba2, Result};
///
/// fn list_files(path: &Path) -> Result<()> {
///     let f = File::open(path)?;
///     let ba2 = Ba2::new(f)?;
///
///     for entry in ba2.entries() {
///         if let Some(name) = entry.name() {
///             println("{}", name);
///         }
///     }
/// }
/// ```
pub struct Ba2<R>
where
    R: Read + Seek,
{
    inner: Ba2Inner<R>,
}

impl<R> Ba2<R>
where
    R: Read + Seek,
{
    pub fn new(r: R) -> Result<Ba2<R>> {
        Ok(Ba2 {
            inner: Ba2Inner::new(r)?,
        })
    }

    pub fn entries(&self) -> Entries {
        let inner = match &self.inner.chunks {
            Ba2Chunks::General(chunks) => EntriesInner::General(chunks.iter()),
            Ba2Chunks::DirectX(chunks) => EntriesInner::DirectX(chunks.iter()),
        };

        Entries {
            strings: self.inner.strings.as_ref().map(|strings| strings.iter()),
            inner,
            ba2: &self.inner,
        }
    }
}

pub struct Entries<'a> {
    strings: Option<slice::Iter<'a, String>>,
    inner: EntriesInner<'a>,
    ba2: &'a Ba2Inner<dyn 'a + ReadSeek>,
}

impl<'a> Iterator for Entries<'a> {
    type Item = Entry<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let string = self
            .strings
            .as_mut()
            .and_then(|strings| strings.next())
            .map(|s| s.as_ref());

        match &mut self.inner {
            EntriesInner::General(entries) => {
                let e = entries.next()?;
                Some(Entry::General(GeneralEntry {
                    ba2: self.ba2,
                    inner: e,
                    name: string,
                }))
            }
            EntriesInner::DirectX(entries) => {
                let e = entries.next()?;
                Some(Entry::DirectX(DirectXEntry {
                    ba2: self.ba2,
                    inner: e,
                    name: string,
                }))
            }
        }
    }
}

enum EntriesInner<'a> {
    General(slice::Iter<'a, GeneralChunkInner>),
    DirectX(slice::Iter<'a, DirectXChunkInner>),
}

pub enum Entry<'a> {
    General(GeneralEntry<'a>),
    DirectX(DirectXEntry<'a>),
}

impl<'a> Entry<'a> {
    pub fn name(&self) -> Option<&'a str> {
        match self {
            Entry::General(e) => e.name(),
            Entry::DirectX(e) => e.name(),
        }
    }

    pub fn chunks(&self) -> Chunks<'a> {
        match self {
            Entry::General(e) => Chunks {
                inner: ChunksInner::General(e.chunks()),
            },
            Entry::DirectX(e) => Chunks {
                inner: ChunksInner::DirectX(e.chunks()),
            },
        }
    }
}

pub struct Chunks<'a> {
    inner: ChunksInner<'a>,
}

impl<'a> Iterator for Chunks<'a> {
    type Item = Chunk<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let inner = match &mut self.inner {
            ChunksInner::General(chunks) => ChunkInner::General(chunks.next()?),
            ChunksInner::DirectX(chunks) => ChunkInner::DirectX(chunks.next()?),
        };
        Some(Chunk { inner })
    }
}

pub struct Chunk<'a> {
    inner: ChunkInner<'a>,
}

impl Chunk<'_> {
    pub fn data(&self) -> Result<ChunkData> {
        match self.inner {
            ChunkInner::General(chunk) => chunk.open(),
            ChunkInner::DirectX(chunk) => chunk.open(),
        }
    }
}

struct Ba2Inner<R>
where
    R: ?Sized + Read + Seek,
{
    chunks: Ba2Chunks,
    strings: Option<Vec<String>>,
    reader: RefCell<R>,
}

impl<R> Ba2Inner<R>
where
    R: Read + Seek,
{
    pub fn new(mut r: R) -> Result<Ba2Inner<R>> {
        let mut header = [0; mem::size_of::<RawHeader>()];
        r.read_exact(&mut header)?;
        let header: RawHeader = bytemuck::cast(header);
        let header = Header::try_from(header)?;

        let chunks = match header.format {
            Format::General => {
                let chunks = read_general_chunks(&mut r, header.file_count as usize)?;
                Ba2Chunks::General(chunks)
            }
            Format::DirectX => {
                let chunks = read_directx_chunks(&mut r, header.file_count as usize)?;
                Ba2Chunks::DirectX(chunks)
            }
        };

        let strings = if let Some(offset) = header.string_table_offset {
            let off = offset.get();
            Some(read_string_table(&mut r, off, header.file_count)?)
        } else {
            None
        };

        let reader = RefCell::new(r);

        for s in strings.as_ref().unwrap() {
            println!("{}", s)
        }

        Ok(Ba2Inner {
            chunks,
            reader,
            strings,
        })
    }
}

impl Ba2Inner<dyn '_ + ReadSeek> {
    pub fn chunk_data(
        &self,
        offset: u64,
        compressed_len: Option<NonZeroU32>,
        uncompressed_len: u32,
    ) -> Result<ChunkData> {
        let mut r = self.reader.borrow_mut();
        r.seek(SeekFrom::Start(offset))?;

        let raw_len = if let Some(len) = compressed_len {
            len.get()
        } else {
            uncompressed_len
        };
        let raw_len = raw_len as usize;

        let buf = read_vec(r.deref_mut(), raw_len)?;

        let data = if compressed_len.is_some() {
            ChunkData::compressed(buf)
        } else {
            ChunkData::uncompressed(buf)
        };
        Ok(data)
    }
}

trait ReadSeek: Read + Seek {}

impl<R> ReadSeek for R where R: Read + Seek {}

fn read_general_chunk<R>(r: &mut R) -> Result<GeneralChunkInner>
where
    R: ?Sized + Read + Seek,
{
    let header: RawGeneralChunkHeader = read_pod(r)?;
    let header = GeneralChunkHeader::try_from(header)?;

    let raw_data: SmallVec<[RawGeneralChunkData; 1]> =
        read_smallvec(r, header.chunk_count as usize)?;

    let mut data = SmallVec::with_capacity(header.chunk_count as usize);
    for chunk in raw_data {
        data.push(chunk.try_into()?);
    }

    Ok(GeneralChunkInner { header, data })
}

fn read_general_chunks<R>(r: &mut R, n: usize) -> Result<Vec<GeneralChunkInner>>
where
    R: ?Sized + Read + Seek,
{
    let mut chunks = Vec::new();
    for _ in 0..n {
        chunks.push(read_general_chunk(r)?);
    }
    Ok(chunks)
}

fn read_directx_chunk<R>(r: &mut R) -> Result<DirectXChunkInner>
where
    R: ?Sized + Read + Seek,
{
    let header: RawDirectXChunkHeader = read_pod(r)?;
    let header = DirectXChunkHeader::try_from(header)?;

    let raw_data: SmallVec<[RawDirectXChunkData; 1]> =
        read_smallvec(r, header.chunk_count as usize)?;

    let mut data = SmallVec::with_capacity(header.chunk_count as usize);
    for chunk in raw_data {
        data.push(chunk.try_into()?);
    }

    Ok(DirectXChunkInner { header, data })
}

fn read_directx_chunks<R>(r: &mut R, n: usize) -> Result<Vec<DirectXChunkInner>>
where
    R: ?Sized + Read + Seek,
{
    let mut chunks = Vec::new();
    for _ in 0..n {
        chunks.push(read_directx_chunk(r)?);
    }
    Ok(chunks)
}

fn read_string_table<R>(r: &mut R, off: u64, file_count: u32) -> Result<Vec<String>>
where
    R: ?Sized + Read + Seek,
{
    let mut strings = Vec::with_capacity(file_count as usize);
    r.seek(SeekFrom::Start(off))?;
    for _ in 0..file_count {
        let string = read_wstring(r)?;
        strings.push(string);
    }
    Ok(strings)
}

enum Ba2Chunks {
    General(Vec<GeneralChunkInner>),
    DirectX(Vec<DirectXChunkInner>),
}

#[derive(Debug)]
struct GeneralChunkInner {
    header: GeneralChunkHeader,
    data: SmallVec<[GeneralChunkData; 1]>,
}

#[derive(Debug)]
struct DirectXChunkInner {
    header: DirectXChunkHeader,
    data: SmallVec<[DirectXChunkData; 1]>,
}

#[derive(Clone, Copy)]
pub struct GeneralEntry<'a> {
    name: Option<&'a str>,
    inner: &'a GeneralChunkInner,
    ba2: &'a Ba2Inner<dyn 'a + ReadSeek>,
}

impl<'a> GeneralEntry<'a> {
    pub fn name(&self) -> Option<&'a str> {
        self.name
    }

    pub fn chunks(&self) -> GeneralChunks<'a> {
        GeneralChunks {
            entry: *self,
            chunks: self.inner.data.iter(),
        }
    }
}

pub struct GeneralChunks<'a> {
    entry: GeneralEntry<'a>,
    chunks: slice::Iter<'a, GeneralChunkData>,
}

impl<'a> Iterator for GeneralChunks<'a> {
    type Item = GeneralChunk<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let chunk = self.chunks.next()?;
        Some(GeneralChunk {
            inner: chunk,
            ba2: self.entry.ba2,
        })
    }
}

#[derive(Clone, Copy)]
pub struct GeneralChunk<'a> {
    inner: &'a GeneralChunkData,
    ba2: &'a Ba2Inner<dyn 'a + ReadSeek>,
}

impl GeneralChunk<'_> {
    pub fn open(&self) -> Result<ChunkData> {
        let offset = self.inner.data_file_offset;
        let compressed_len = self.inner.compressed_size;
        let uncompressed_len = self.inner.decompressed_size;

        let ba2: &Ba2Inner<dyn ReadSeek> = self.ba2;
        ba2.chunk_data(offset, compressed_len, uncompressed_len)
    }
}

#[derive(Clone, Copy)]
pub struct DirectXEntry<'a> {
    name: Option<&'a str>,
    inner: &'a DirectXChunkInner,
    ba2: &'a Ba2Inner<dyn 'a + ReadSeek>,
}

impl<'a> DirectXEntry<'a> {
    pub fn name(&self) -> Option<&'a str> {
        self.name
    }

    pub fn chunks(&self) -> DirectXChunks<'a> {
        DirectXChunks {
            entry: *self,
            chunks: self.inner.data.iter(),
        }
    }
}

pub struct DirectXChunks<'a> {
    entry: DirectXEntry<'a>,
    chunks: slice::Iter<'a, DirectXChunkData>,
}

impl<'a> Iterator for DirectXChunks<'a> {
    type Item = DirectXChunk<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let chunk = self.chunks.next()?;
        Some(DirectXChunk {
            inner: chunk,
            ba2: self.entry.ba2,
        })
    }
}

#[derive(Clone, Copy)]
pub struct DirectXChunk<'a> {
    inner: &'a DirectXChunkData,
    ba2: &'a Ba2Inner<dyn 'a + ReadSeek>,
}

impl DirectXChunk<'_> {
    pub fn open(&self) -> Result<ChunkData> {
        let offset = self.inner.data_file_offset;
        let compressed_len = self.inner.compressed_size;
        let uncompressed_len = self.inner.decompressed_size;

        self.ba2
            .chunk_data(offset, compressed_len, uncompressed_len)
    }
}

enum ChunkInner<'a> {
    General(GeneralChunk<'a>),
    DirectX(DirectXChunk<'a>),
}

enum ChunksInner<'a> {
    General(GeneralChunks<'a>),
    DirectX(DirectXChunks<'a>),
}
