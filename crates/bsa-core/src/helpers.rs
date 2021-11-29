use std::io::{self, Read};

pub fn read_vec(r: &mut dyn Read, n: usize) -> io::Result<Vec<u8>> {
    let mut buf = Vec::with_capacity(n);
    unsafe {
        buf.set_len(n);
    }
    r.read_exact(&mut buf)?;
    Ok(buf)
}
