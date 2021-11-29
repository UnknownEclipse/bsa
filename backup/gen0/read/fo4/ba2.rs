use std::{
    convert::TryFrom,
    io::{Read, Seek, SeekFrom, Take},
    num::NonZeroU32,
};

use crate::{
    common::{read_pod, read_pod_vec, read_vec, windows_1252},
    read::{
        fo4::{Format, GeneralChunkHeader, Header, RawHeader},
        EntryData, RawEntryData,
    },
    ArchiveReadError, Result,
};

use super::{ChunkData, ChunkHeader, Dx10ChunkData, Dx10ChunkHeader, GeneralChunkData};

pub struct Archive<R>
where
    R: Read + Seek,
{
    reader: R,
    header: Header,
    // chunks: Chunks,
}

#[derive(Debug)]
enum EntryInner {
    General {
        data_offset: u64,
        compressed_size: Option<NonZeroU32>,
        uncompressed_size: u32,
    },
}

#[derive(Debug)]
struct Entry {
    name: Option<String>,
    inner: EntryInner,
}

enum Chunks {
    General(Vec<(GeneralChunkHeader, Vec<GeneralChunkData>)>),
    Dx10(Vec<(Dx10ChunkHeader, Vec<Dx10ChunkData>)>),
}

impl<R> Archive<R>
where
    R: Read + Seek,
{
    pub fn new(mut r: R) -> Result<Archive<R>> {
        let raw_header: RawHeader = read_pod(&mut r)?;
        let header = Header::try_from(raw_header)?;
        let chunks = read_chunks(&mut r, &header)?;
        let strings = read_string_table(&mut r, &header)?;
        let mut strings = strings.map(IntoIterator::into_iter);

        let mut entries = Vec::with_capacity(header.file_count as usize);

        match chunks {
            Chunks::General(chunks) => {
                for (_header, data) in chunks {
                    let name = strings.as_mut().map(|strings| strings.next().unwrap());

                    let data = data.into_iter().next().unwrap();
                    let inner = EntryInner::General {
                        compressed_size: data.compressed_size(),
                        uncompressed_size: data.uncompressed_size(),
                        data_offset: data.data_file_offset(),
                    };

                    entries.push(Entry { inner, name });
                }
            }
            Chunks::Dx10(_) => eprintln!("todo!"),
        }
        // println!("{:?}", entries);
        // todo!()

        Ok(Archive { reader: r, header })
    }

    fn open_entry(&mut self, entry: &Entry) -> Result<EntryData> {
        match &entry.inner {
            EntryInner::General {
                compressed_size,
                uncompressed_size,
                ..
            } => {
                let raw = RawEntryData::from_stream(self.raw_entry_data(entry)?);
                if compressed_size.is_some() {
                    Ok(EntryData::new_zlib(raw, *uncompressed_size))
                } else {
                    Ok(EntryData::new_uncompressed(raw))
                }
            }
        }
    }

    fn raw_entry_data(&mut self, entry: &Entry) -> Result<Take<&mut dyn Read>> {
        match &entry.inner {
            EntryInner::General {
                data_offset,
                compressed_size,
                uncompressed_size,
            } => {
                let r = &mut self.reader;
                r.seek(SeekFrom::Start(*data_offset))?;
                let r = r as &mut dyn Read;
                Ok(r.take(
                    compressed_size
                        .map(|s| s.get())
                        .unwrap_or(*uncompressed_size) as u64,
                ))
            }
        }
    }
}

fn read_chunks<R: Read>(r: &mut R, header: &Header) -> Result<Chunks> {
    match header.format {
        Format::General => Ok(Chunks::General(read_chunks_n(r, header)?)),
        Format::Dx10 => Ok(Chunks::Dx10(read_chunks_n(r, header)?)),
    }
}

fn read_chunks_n<R: Read, H: ChunkHeader, D: ChunkData>(
    r: &mut R,
    header: &Header,
) -> Result<Vec<(H, Vec<D>)>> {
    let mut chunks = Vec::with_capacity(header.file_count as usize);

    for _ in 0..header.file_count {
        let header: H = read_pod(r)?;
        let data: Vec<D> = read_pod_vec(r, header.chunk_count() as usize)?;

        for data_chunk in data.iter() {
            if data_chunk.sentinel() != 0xBAADF00D {
                return Err(ArchiveReadError::BadSentinel.into());
            }
        }

        chunks.push((header, data));
    }

    Ok(chunks)
}

fn read_chunks_one<R: Read, H: ChunkHeader, D: ChunkData>(
    r: &mut R,
    header: &Header,
) -> Result<Vec<(H, D)>> {
    let mut chunks = Vec::with_capacity(header.file_count as usize);

    for _ in 0..header.file_count {
        let header: H = read_pod(r)?;
        if header.chunk_count() != 1 {
            return Err(ArchiveReadError::BadArchive.into());
        }

        let data: D = read_pod(r)?;

        if data.sentinel() != 0xBAADF00D {
            return Err(ArchiveReadError::BadSentinel.into());
        }

        chunks.push((header, data));
    }

    Ok(chunks)
}

fn read_string_table<R: Read + Seek>(r: &mut R, hdr: &Header) -> Result<Option<Vec<String>>> {
    let offset = match hdr.string_table_offset {
        Some(off) => off.get(),
        _ => return Ok(None),
    };
    r.seek(SeekFrom::Start(offset))?;
    let mut strings = Vec::with_capacity(hdr.file_count as usize);
    for _ in 0..hdr.file_count {
        let s = read_wstring(r)?;
        strings.push(s);
    }
    Ok(Some(strings))
}

fn read_wstring<R: Read>(r: &mut R) -> Result<String> {
    let mut buf = [0; 2];
    r.read_exact(&mut buf)?;
    let len = u16::from_le_bytes(buf) as usize;
    let buf = read_vec(r, len)?;

    Ok(windows_1252::to_string(buf))
    // Ok(String::from_utf8(buf).map_err(|_| InvalidArchiveError::BadEncoding)?)
}
