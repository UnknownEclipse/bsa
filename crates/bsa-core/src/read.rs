use std::{
    borrow::Cow,
    fs::{self, File},
    io::Write,
    path::Path,
};

use crate::Result;

/// The `Archive` trait allows generic read access to a BSA or BA2 archive.
///
/// # Examples
/// Check if a file is contained in an archive.
/// ```
/// use bse_core::Archive;
///
/// fn contains_file<A: Archive>(a: &A, filename: &str) -> bool {
///     a.by_name(filename).is_some()
/// }
/// ```
///
/// # Notes
/// The api defined by the `Archive` trait is intentionally minimal. Every BSA/BA2
/// format has its own capabilities, and may provide a more advanced api where
/// necessary. The `Archive` trait should be used when generic, simple access is needed.
pub trait Archive {
    /// An `Index` is a lightweight type used to refer to entries in the archive.
    type Index: Copy + Eq;

    /// Extract all files in the archive to a directory.
    fn extract<P: AsRef<Path>>(&self, dir: P) -> Result<()> {
        default_extract(self, dir.as_ref())
    }

    /// Get an entry by index.
    ///
    /// Returns an [Entry](Self::Entry) given an `index`.
    fn by_index(&self, index: Self::Index) -> Entry<Self>;

    /// Get an entry by name.
    ///
    /// Returns an [Entry](Self::Entry) with the given name, or [None] if no entry with
    /// that name is present.
    fn by_name<S: AsRef<str>>(&self, name: S) -> Option<Entry<Self>>;

    /// Return an iterator over all entries in an archive.
    fn entries(&self) -> Entries<Self>;
}

pub struct Entries<'a, A>
where
    A: ?Sized + Archive,
{
    imp: &'a dyn EntriesImpl<A>,
    index: Option<A::Index>,
}

impl<A> Entries<'_, A>
where
    A: ?Sized + Archive,
{
    pub fn new(imp: &dyn EntriesImpl<A>, index: Option<A::Index>) -> Entries<A> {
        Entries { imp, index }
    }
}

impl<'a, A: ?Sized + Archive> Iterator for Entries<'a, A> {
    type Item = Entry<'a, A>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(cur) = self.index {
            self.index = self.imp.next(cur);
            Some(Entry::new(self.imp, cur))
        } else {
            None
        }
    }
}

pub struct Entry<'a, A: ?Sized + Archive> {
    imp: &'a dyn EntriesImpl<A>,
    index: A::Index,
}

impl<A: ?Sized + Archive> Entry<'_, A> {
    pub fn new(imp: &dyn EntriesImpl<A>, index: A::Index) -> Entry<A> {
        Entry { imp, index }
    }

    /// Get the index of this entry.
    pub fn index(&self) -> A::Index {
        self.index
    }

    /// Get the name of this entry.
    ///
    /// # Notes
    /// The returned name is already normalized, so it is safe to use it directly.
    pub fn name(&self) -> Cow<str> {
        self.imp.name(self.index)
    }

    /// Extract this entry to a file.
    pub fn extract<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        self.imp.extract(self.index, path.as_ref())
    }

    /// Extract this entry to a provided writer.
    pub fn extract_to<W: Write>(&self, out: &mut W) -> Result<()> {
        self.imp.extract_to(self.index, out)
    }
}

/// This is a helper trait for implementing the [Entries] type.
///
/// If you aren't implementing an archive format, pretend this does not exist. If you
/// are, then read on.
///
/// When implementing an `Archive`, you must implement `EntriesImpl<Self>` for that
/// archive type. Then, in your implementation of [entries()][Archive::entries()],
/// call `Entries::new(self, start_index)]`, where `start_index` is a sentinel index
/// that represents the index before the first. In your implementation of `EntriesImpl`,
/// the [get()][EntriesImpl::get] function should simply return an [Entry]. The
/// [increment()][EntriesImpl::increment] function should modify the index provided
/// to point to the next entry. If the previous entry was the last one in the archive,
/// return true, otherwise return false.
pub trait EntriesImpl<A: ?Sized + Archive> {
    fn next(&self, index: A::Index) -> Option<A::Index>;

    fn name(&self, index: A::Index) -> Cow<str>;

    fn extract(&self, index: A::Index, path: &Path) -> Result<()> {
        let mut f = File::create(path)?;
        self.extract_to(index, &mut f)?;
        Ok(())
    }

    fn extract_to(&self, index: A::Index, writer: &mut dyn Write) -> Result<()>;
}

// /// This is a helper trait for implementing the [Entry] type.
// ///
// /// If you aren't implementing an archive format, pretend this does not exist. If you
// /// are, then read on.
// ///
// /// `EntryImpl` should be implemented for the archive type. Each operation accepts
// /// the index of this entry to perform the required operations.
// pub trait EntryImpl<A: ?Sized + Archive> {
//     fn name(&self, index: A::Index) -> &str;

//     fn extract(&self, index: A::Index, path: &Path) -> Result<()> {
//         let mut f = File::create(path)?;
//         self.extract_to(index, &mut f)?;
//         Ok(())
//     }

//     fn extract_to(&self, index: A::Index, writer: &mut dyn Write) -> Result<()>;
// }

// /// The `ArchiveEntry` trait represents an individual file in an `Archive`.
// pub trait ArchiveEntry<'a>: Sized + Copy {
//     /// The index type of this entry. This must be the same as the parent archive's
//     /// [Archive::Index] type.
//     type Index: Copy + Eq;

//     /// Get the index of this entry.
//     fn index(&self) -> Self::Index;

//     /// Get the name of this entry.
//     ///
//     /// # Notes
//     /// The returned name is already normalized, so it is safe to use it directly.
//     fn name(&self) -> &str;

//     /// Extract this entry to a file.
//     fn extract<P: AsRef<Path>>(&self, path: P) -> Result<()> {
//         default_entry_extract(self, path.as_ref())
//     }

//     /// Extract this entry to a provided writer.
//     fn extract_to<W: Write>(&self, writer: &mut W) -> Result<()>;
// }

fn default_extract<A: ?Sized + Archive>(archive: &A, path: &Path) -> Result<()> {
    let path = fs::canonicalize(path)?;
    for entry in archive.entries() {
        let dst = path.join(entry.name().as_ref());
        if let Some(parent) = dst.parent() {
            fs::create_dir_all(parent)?;
        }
        entry.extract(dst)?;
    }
    Ok(())
}
