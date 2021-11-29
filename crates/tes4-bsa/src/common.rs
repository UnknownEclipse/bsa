use std::io::{self, Read};

use crate::read_at::ReadAt;

pub fn read_vec(r: &mut dyn Read, n: usize) -> io::Result<Vec<u8>> {
    let mut buf = Vec::with_capacity(n);
    unsafe {
        buf.set_len(n);
    }
    r.read_exact(&mut buf)?;
    Ok(buf)
}

pub fn read_vec_at(r: &dyn ReadAt, n: usize, off: u64) -> io::Result<Vec<u8>> {
    let mut buf = Vec::with_capacity(n);
    unsafe {
        buf.set_len(n);
    }
    r.read_exact_at(&mut buf, off)?;
    Ok(buf)
}
