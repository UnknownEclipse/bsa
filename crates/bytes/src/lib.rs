use std::{error::Error, fmt::Display, ops::Deref};

#[derive(Debug)]
pub struct EofError;

impl Display for EofError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("unexpected eof")
    }
}

impl Error for EofError {}

#[derive(Debug, Clone, Copy)]
pub struct Bytes<'a>(&'a [u8]);

impl<'a> Bytes<'a> {
    #[inline]
    pub fn new(buf: &'a [u8]) -> Bytes<'a> {
        Bytes(buf)
    }

    #[inline]
    pub fn skip(&mut self, n: usize) -> Result<(), EofError> {
        if self.len() < n {
            Err(EofError)
        } else {
            self.0 = &self.0[n..];
            Ok(())
        }
    }

    #[inline]
    pub fn read_bytes(&mut self, n: usize) -> Result<&'a [u8], EofError> {
        if self.len() < n {
            Err(EofError)
        } else {
            let (bytes, rest) = self.split_at(n);
            self.0 = rest;
            Ok(bytes)
        }
    }

    #[inline]
    pub fn peek_bytes(&self, n: usize) -> Result<&'a [u8], EofError> {
        let mut temp = *self;
        temp.read_bytes(n)
    }
}

impl<'a> Deref for Bytes<'a> {
    type Target = &'a [u8];

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<'a> AsRef<[u8]> for Bytes<'a> {
    #[inline]
    fn as_ref(&self) -> &'a [u8] {
        self.0
    }
}
