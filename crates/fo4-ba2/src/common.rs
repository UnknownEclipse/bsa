use std::{
    io::{self, Read},
    mem::MaybeUninit,
};

use bytemuck::{bytes_of_mut, cast_slice_mut, Pod};
use smallvec::{Array, SmallVec};

use crate::Result;

#[allow(clippy::uninit_assumed_init)]
pub fn read_pod<R, T>(r: &mut R) -> io::Result<T>
where
    R: ?Sized + Read,
    T: Pod,
{
    unsafe {
        let mut value = MaybeUninit::uninit().assume_init();
        let buf = bytes_of_mut(&mut value);
        r.read_exact(buf)?;
        Ok(value)
    }
}

pub fn read_vec<R, T>(r: &mut R, n: usize) -> io::Result<Vec<T>>
where
    R: ?Sized + Read,
    T: Pod,
{
    let mut v = Vec::with_capacity(n);
    unsafe {
        v.set_len(n);
    }
    let buf = cast_slice_mut(&mut v);
    r.read_exact(buf)?;
    Ok(v)
}

pub fn read_smallvec<R, A, T>(r: &mut R, n: usize) -> io::Result<SmallVec<A>>
where
    A: Array<Item = T>,
    T: Pod,
    R: ?Sized + Read,
{
    let mut v = SmallVec::with_capacity(n);
    unsafe {
        v.set_len(n);
    }
    let buf = cast_slice_mut(&mut v);
    r.read_exact(buf)?;
    Ok(v)
}

pub fn read_wstring<R>(r: &mut R) -> Result<String>
where
    R: ?Sized + Read,
{
    let mut len = [0; 2];
    r.read_exact(&mut len)?;
    let len = u16::from_le_bytes(len) as usize;
    let bytes = read_vec(r, len)?;
    Ok(windows_1252::decode_string(bytes))
}
