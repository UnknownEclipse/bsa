use std::mem;

use crate::str::BsStr;

#[repr(transparent)]
pub struct BsPath(BsStr);

impl BsPath {
    pub fn new<S: ?Sized + AsRef<BsStr>>(s: &S) -> &BsPath {
        let s = s.as_ref();
        unsafe { mem::transmute(s) }
    }
}
