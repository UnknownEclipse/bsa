use std::{borrow::Cow, mem, ops::Deref};

use bytemuck::Pod;
use memchr::memchr;

use crate::{ArchiveReadError, ReadBytesError, Result};

use super::windows_1252;

pub struct Bytes<'a>(pub &'a [u8]);

impl<'a> Bytes<'a> {
    #[inline]
    pub fn skip(&mut self, n: usize) -> Result<(), ReadBytesError> {
        if self.len() < n {
            Err(ReadBytesError::Eof)
        } else {
            self.0 = &self.0[n..];
            Ok(())
        }
    }

    #[inline]
    pub fn read_bytes(&mut self, n: usize) -> Result<&'a [u8], ReadBytesError> {
        if self.len() < n {
            dbg!(n - self.len());
            Err(ReadBytesError::Eof)
        } else {
            let (bytes, rest) = self.0.split_at(n);
            self.0 = rest;
            Ok(bytes)
        }
    }

    #[inline]
    pub fn read<T: Pod>(&mut self) -> Result<&'a T, ReadBytesError> {
        let bytes = self.read_bytes(mem::size_of::<T>())?;
        Ok(bytemuck::from_bytes(bytes))
    }

    #[inline]
    pub fn read_slice<T: Pod>(&mut self, n: usize) -> Result<&'a [T], ReadBytesError> {
        let bytes = self.read_bytes(mem::size_of::<T>() * n)?;
        Ok(bytemuck::cast_slice(bytes))
    }

    #[inline]
    pub fn read_bstring(&mut self) -> Result<Cow<'a, str>> {
        let bytes = read_bstring_bytes(self)?;
        if bytes.contains(&b'\0') {
            return Err(ArchiveReadError::EmbeddedNul.into());
        }
        Ok(windows_1252::to_str(bytes))
    }

    pub fn read_bzstring(&mut self) -> Result<Cow<'a, str>> {
        let bytes = read_bstring_bytes(self)?;

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
}

impl<'a> Deref for Bytes<'a> {
    type Target = [u8];

    fn deref(&self) -> &'a Self::Target {
        self.0
    }
}

fn read_bstring_bytes<'a>(bytes: &mut Bytes<'a>) -> Result<&'a [u8]> {
    fn inner<'a>(bytes: &mut Bytes<'a>) -> Result<&'a [u8]> {
        let len: u8 = *bytes.read()?;
        Ok(bytes.read_bytes(len as usize)?)
    }

    Ok(inner(bytes).map_err(|_| ArchiveReadError::BadArchive)?)
}
