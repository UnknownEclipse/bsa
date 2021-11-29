use std::{borrow::Cow, mem, str};

use crate::string::BsString;

#[repr(transparent)]
pub struct BsStr([u8]);

impl BsStr {
    /// # Safety
    /// 1. `bytes` must be encoded in the Windows-1252 encoding.
    /// 2. `bytes` must contain no nul bytes.
    /// 3. `bytes` must be entirely lowercase.
    pub unsafe fn from_bytes_unchecked(bytes: &[u8]) -> &BsStr {
        mem::transmute(bytes)
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    pub fn to_str(&self) -> Cow<str> {
        if self.0.is_ascii() {
            Cow::Borrowed(unsafe { str::from_utf8_unchecked(&self.0) })
        } else {
            let s = self
                .as_bytes()
                .iter()
                .map(|&byte| windows_1252::decode(byte))
                .collect();

            Cow::Owned(s)
        }
    }
}

impl ToOwned for BsStr {
    type Owned = BsString;

    fn to_owned(&self) -> Self::Owned {
        todo!()
    }
}
