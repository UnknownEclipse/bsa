use std::borrow::Borrow;

use windows_1252::EncodeWin1252Error;

use crate::str::BsStr;

#[repr(transparent)]
#[derive(Debug, Default, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BsString(Vec<u8>);

impl BsString {
    /// # Safety
    /// 1. `bytes` must be encoded in the Windows-1252 encoding.
    /// 2. `bytes` must contain no nul bytes.
    /// 3. `bytes` must be entirely lowercase.
    pub unsafe fn from_bytes_unchecked(bytes: Vec<u8>) -> BsString {
        BsString(bytes)
    }

    /// Creates a new [BsString] with `bytes`. All letters are normalized to be
    /// lowercase, and if a nul byte is encountered, the string is truncated at that
    /// point.
    pub fn from_bytes_lossy(bytes: Vec<u8>) -> BsString {
        todo!()
    }

    // /// Creates a new [BsString] from a provided string. All letters are normalized to
    // /// be lowercase. If a nul byte is encountered, the string is truncated at that
    // /// point. If a character is encountered that cannot be stored
    // pub fn from_string_lossy(s: String) -> Result<BsString, EncodeWin1252Error> {
    //     if s.is_ascii() {}
    // }

    pub fn new() -> BsString {
        Default::default()
    }
}

impl Borrow<BsStr> for BsString {
    fn borrow(&self) -> &BsStr {
        unsafe { BsStr::from_bytes_unchecked(&self.0) }
    }
}

impl AsRef<BsStr> for BsString {
    fn as_ref(&self) -> &BsStr {
        unsafe { BsStr::from_bytes_unchecked(&self.0) }
    }
}
