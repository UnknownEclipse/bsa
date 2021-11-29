use std::io::{self, Cursor, Read};

use flate2::bufread::ZlibDecoder;

pub struct ChunkData {
    inner: ChunkDataInner,
}

impl ChunkData {
    pub(crate) fn uncompressed(buf: Vec<u8>) -> ChunkData {
        ChunkData {
            inner: ChunkDataInner::Vec(Cursor::new(buf)),
        }
    }

    pub(crate) fn compressed(buf: Vec<u8>) -> ChunkData {
        ChunkData {
            inner: ChunkDataInner::Zlib(ZlibDecoder::new(Cursor::new(buf))),
        }
    }
}

impl Read for ChunkData {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match &mut self.inner {
            ChunkDataInner::Vec(r) => r.read(buf),
            ChunkDataInner::Zlib(r) => r.read(buf),
        }
    }

    fn read_to_end(&mut self, buf: &mut Vec<u8>) -> io::Result<usize> {
        match &mut self.inner {
            ChunkDataInner::Vec(r) => r.read_to_end(buf),
            ChunkDataInner::Zlib(r) => r.read_to_end(buf),
        }
    }

    fn read_exact(&mut self, buf: &mut [u8]) -> io::Result<()> {
        match &mut self.inner {
            ChunkDataInner::Vec(r) => r.read_exact(buf),
            ChunkDataInner::Zlib(r) => r.read_exact(buf),
        }
    }
}

enum ChunkDataInner {
    Vec(Cursor<Vec<u8>>),
    Zlib(ZlibDecoder<Cursor<Vec<u8>>>),
}
