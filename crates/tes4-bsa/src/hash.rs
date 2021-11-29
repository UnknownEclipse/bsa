//! This module implements the TES4 hashing algorithm and the `Hash` type.

/// A computed filename hash.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Hash {
    pub(crate) last: u8,
    pub(crate) last2: u8,
    pub(crate) len: u8,
    pub(crate) first: u8,
    pub(crate) crc: u32,
}

impl Hash {
    pub fn from_bytes(bytes: [u8; 8]) -> Hash {
        Hash {
            last: bytes[0],
            last2: bytes[1],
            len: bytes[2],
            first: bytes[3],
            crc: u32::from_le_bytes(bytes[4..].try_into().unwrap()),
        }
    }

    pub fn from_u64(value: u64) -> Hash {
        Hash::from_bytes(value.to_le_bytes())
    }

    pub fn to_bytes(self) -> [u8; 8] {
        let crc = self.crc.to_le_bytes();
        [
            self.last, self.last2, self.len, self.first, crc[0], crc[1], crc[2], crc[3],
        ]
    }

    pub fn to_u64(self) -> u64 {
        u64::from_le_bytes(self.to_bytes())
    }
}

impl PartialOrd for Hash {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.to_u64().partial_cmp(&other.to_u64())
    }
}

impl Ord for Hash {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.to_u64().cmp(&other.to_u64())
    }
}

const MAX_PATH: usize = 260;

/// Computes the hash of a directory name without normalization.
///
/// # Safety
/// This function is marked `unsafe` because it is important that the directory name
/// be in the correct format for the correct results. The following assumptions must
/// be verified by the caller:
/// 1. The name is in the Windows-1252 encoding
/// 2. The name contains no embedded nuls
/// 3. The name is entirely lowercase.
/// 4. The name uses *ONLY* '\\' as a directory separator.
/// 5. The path is lexically normalized, relative, and contains no '..' or '.'
/// components.
/// 6. The path is not empty and shorter than Windows' `MAX_PATH` length (260 bytes)
pub unsafe fn hash_directory_name_unchecked(name: &[u8]) -> Hash {
    let length = name.len() as u8;

    let mut last = 0;
    let mut last2 = 0;
    let mut first = 0;
    let mut crc = 0;

    if 3 <= name.len() {
        last2 = name[name.len() - 2];
        crc = crc32(&name[1..name.len() - 2]);
    }
    if !name.is_empty() {
        first = name[0];
        last = name[name.len() - 1];
    }

    Hash {
        last,
        last2,
        first,
        crc,
        len: length,
    }
}

/// Computes the hash of a file name without normalization.
///
/// # Safety
/// This function is marked `unsafe` because it is important that the file name
/// be in the correct format for the correct results. The following assumptions must
/// be verified by the caller:
/// 1. Both `stem` and `extension` are in the Windows-1252 encoding
/// 2. Neither `stem` nor `extension` contain any embedded nuls.
/// 3. Both `stem` and `extension` are lowercase.
/// 4. Neither `stem` nor `extension` contain any directory separators.
/// 5. `extension` is shorter than 16 bytes.
/// 6. `stem` is not empty, and shorter than Windows' `MAX_PATH` length (260 bytes).
pub unsafe fn hash_file_name_unchecked(stem: &[u8], extension: &[u8]) -> Hash {
    let mut hash = hash_directory_name_unchecked(stem);
    hash.crc = hash.crc.wrapping_add(crc32(extension));

    let i = match extension {
        b"" => Some(0),
        b".nif" => Some(1),
        b".kf" => Some(2),
        b".dds" => Some(3),
        b".wav" => Some(4),
        b".adp" => Some(5),
        _ => None,
    };

    if let Some(i) = i {
        hash.first = hash.first.wrapping_add(32 * (i & 0xfc));
        hash.last = hash.last.wrapping_add((i & 0xfe) << 6);
        hash.last2 = hash.last2.wrapping_add(i << 7);
    }

    hash
}

/// Computes the hash of a directory name, with normalization.
///
/// This function compute the hash of a directory path with normalization. If an invalid
/// portion is found, such as a unicode character that cannot be encoded as
/// Windows-1252, a '..' component, or a leading separator, returns [None].
pub fn hash_directory_name(path: &str) -> Option<Hash> {
    let path = normalize_path(path)?;
    if path.is_empty() || MAX_PATH <= path.len() {
        None
    } else {
        let hash = unsafe { hash_directory_name_unchecked(&path) };
        Some(hash)
    }
}

/// Computes the hash of a file name, with normalization.
///
/// This function compute the hash of a file name with normalization. If an invalid
/// portion is found, such as a unicode character that cannot be encoded as
/// Windows-1252 or an embedded separator, returns [None].
pub fn hash_file_name(name: &str) -> Option<Hash> {
    if name.contains(|ch| ch == '\\' || ch == '/') {
        return None;
    }
    let chars = name.chars().flat_map(char::to_lowercase);

    let mut name = Vec::new();
    for ch in chars {
        let byte = windows_1252::encode(ch).ok()?;
        name.push(byte);
    }
    let (stem, extension) = split_extension(&name);
    dbg!(stem);
    dbg!(extension);
    if stem.is_empty() || MAX_PATH <= stem.len() || 16 <= extension.len() {
        None
    } else {
        let hash = unsafe { hash_file_name_unchecked(stem, extension) };
        Some(hash)
    }
}

/// Computes the hashes of a file path.
///
/// Returns [None] if the path is not valid, otherwise returns a tuple of
/// `(directory_hash, file_hash)`.
pub fn hash_file_path(path: &str) -> Option<(Hash, Hash)> {
    let path = normalize_path(path)?;

    let (directory, file_name) = split_path(&path);
    let (stem, extension) = split_extension(file_name);

    if directory.is_empty()
        || stem.is_empty()
        || MAX_PATH <= stem.len()
        || MAX_PATH <= directory.len()
        || 16 <= extension.len()
    {
        None
    } else {
        unsafe {
            let folder_hash = hash_directory_name_unchecked(directory);
            let file_hash = hash_file_name_unchecked(stem, extension);
            Some((folder_hash, file_hash))
        }
    }
}

fn normalize_path(path: &str) -> Option<Vec<u8>> {
    let is_separator = |ch: char| ch == '\\' || ch == '/';

    let mut buf = Vec::new();

    if path.starts_with(is_separator) {
        return None;
    }

    for component in path.split(|ch| ch == '\\' || ch == '/') {
        if component.is_empty() {
            continue;
        }
        if component == "." || component == ".." {
            return None;
        }
        let name = component;
        if !buf.is_empty() {
            buf.push(b'\\');
        }
        for ch in name.chars() {
            for ch in ch.to_lowercase() {
                let byte = windows_1252::encode(ch).ok()?;
                buf.push(byte);
            }
        }
    }
    Some(buf)
}

fn split_extension(name: &[u8]) -> (&[u8], &[u8]) {
    for (i, &byte) in name.iter().enumerate().rev() {
        if byte == b'.' {
            return name.split_at(i);
        }
    }
    (name, b"")
}

fn split_path(path: &[u8]) -> (&[u8], &[u8]) {
    for (i, &byte) in path.iter().enumerate().rev() {
        if byte == b'\\' {
            let parent = &path[..i];
            let name = &path[i + 1..];
            return (parent, name);
        }
    }
    (b"", path)
}

fn crc32(bytes: &[u8]) -> u32 {
    const K: u32 = 0x1003f;
    let mut crc = 0u32;
    for &byte in bytes {
        crc = byte as u32 + crc.wrapping_mul(K);
    }
    crc
}
