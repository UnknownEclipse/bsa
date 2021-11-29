use std::{
    fs::File,
    io::{self, Read},
};

pub trait ReadAt {
    fn read_at(&self, buf: &mut [u8], pos: u64) -> io::Result<usize>;

    fn read_exact_at(&self, mut buf: &mut [u8], mut pos: u64) -> io::Result<()> {
        loop {
            match self.read_at(buf, pos) {
                Ok(0) => break,
                Ok(n) => {
                    let tmp = buf;
                    buf = &mut tmp[n..];
                    pos += n as u64
                }
                Err(ref e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(e) => return Err(e),
            }
        }

        if !buf.is_empty() {
            Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "failed to fill whole buffer",
            ))
        } else {
            Ok(())
        }
    }
}

#[cfg(unix)]
impl ReadAt for File {
    fn read_at(&self, buf: &mut [u8], pos: u64) -> io::Result<usize> {
        use std::os::unix::fs::FileExt;

        FileExt::read_at(self, buf, pos)
    }
}

impl ReadAt for &[u8] {
    fn read_at(&self, buf: &mut [u8], pos: u64) -> io::Result<usize> {
        if self.len() as u64 <= pos {
            return Ok(0);
        }
        let mut tmp = &self[pos as usize..];
        tmp.read(buf)
    }
}

impl ReadAt for Vec<u8> {
    fn read_at(&self, buf: &mut [u8], pos: u64) -> io::Result<usize> {
        if self.len() as u64 <= pos {
            return Ok(0);
        }
        let mut tmp = &self[pos as usize..];
        tmp.read(buf)
    }
}
