use smallvec::SmallVec;

use super::GeneralChunkInner;

#[derive(Debug, Clone, Copy)]
pub struct GeneralEntry<'a> {
    name: Option<&'a str>,
    inner: &'a GeneralChunkInner,
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

pub struct DirectXChunks<'a> {
    entry: DirectXEntry<'a>,
    chunks: slice::Iter<'a, DirectXChunkData>,
}

impl<'a> Iterator for DirectXChunks<'a> {
    type Item = DirectXChunk<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let chunk = self.chunks.next()?;
        Some(DirectXChunk { inner: chunk })
    }
}

#[derive(Debug, Clone, Copy)]
pub struct DirectXChunk<'a> {
    inner: &'a DirectXChunkData,
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

enum ChunksInner<'a> {
    General(GeneralChunks<'a>),
    DirectX(DirectXChunks<'a>),
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

enum ChunkInner<'a> {
    General(GeneralChunk<'a>),
    DirectX(DirectXChunk<'a>),
}

pub struct Chunk<'a> {
    inner: ChunkInner<'a>,
}
