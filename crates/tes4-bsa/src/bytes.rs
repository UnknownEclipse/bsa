use std::{borrow::Cow, str};

use bsa_core::ReadError;
use bytes::Bytes;

pub trait BytesExt<'a> {
    fn read_bzstring(&mut self) -> Result<Cow<'a, str>, ReadError>;
    fn read_bstring(&mut self) -> Result<Cow<'a, str>, ReadError>;
    fn read_zstring(&mut self) -> Result<Cow<'a, str>, ReadError>;
}

impl<'a> BytesExt<'a> for Bytes<'a> {
    fn read_bzstring(&mut self) -> Result<Cow<'a, str>, ReadError> {
        let len = self.read_bytes(1).map_err(|_| ReadError::Eof)?[0] as usize;
        let bytes = self.read_bytes(len).map_err(|_| ReadError::Eof)?;

        match bytes.iter().position(|&byte| byte == b'\0') {
            Some(i) if i + 1 == bytes.len() => Ok(decode_string(&bytes[..bytes.len() - 1])),
            Some(_) => Err(ReadError::EmbeddedNul),
            None => Err(ReadError::MissingNul),
        }
    }

    fn read_bstring(&mut self) -> Result<Cow<'a, str>, ReadError> {
        let len = self.read_bytes(1).map_err(|_| ReadError::Eof)?[0] as usize;
        let bytes = self.read_bytes(len).map_err(|_| ReadError::Eof)?;

        if bytes.contains(&b'\0') {
            Err(ReadError::EmbeddedNul)
        } else {
            Ok(decode_string(bytes))
        }
    }

    fn read_zstring(&mut self) -> Result<Cow<'a, str>, ReadError> {
        match self.iter().position(|&byte| byte == b'\0') {
            Some(len) => {
                let bytes = &self[..len];
                self.skip(len + 1).unwrap();
                Ok(decode_string(bytes))
            }
            None => Err(ReadError::MissingNul),
        }
    }
}

fn decode_string(bytes: &[u8]) -> Cow<str> {
    if bytes.is_ascii() {
        str::from_utf8(bytes).unwrap().into()
    } else {
        windows_1252::decode_string(bytes.to_owned()).into()
    }
}
