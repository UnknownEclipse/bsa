use std::{
    env,
    fs::{self, File},
    io::{self, BufWriter, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};

use tempfile::NamedTempFile;
use walkdir::WalkDir;

pub use crate::{
    tes3::Tes3Writer,
    tes4::{FnvWriter, Fo3Writer, SseWriter, Tes4Writer, Tes5Writer},
    Result,
};
use crate::{writer::ArchiveWriter, Format};

pub trait ArchiveWrite: Sized {
    /// Set the compression of the archive.
    ///
    /// # Errors
    /// 1. If the archive format does not support compression.
    fn set_compressed(&mut self, compressed: bool) -> Result<()>;

    /// Add a file to the archive with the given path.
    ///
    /// # Errors
    /// 1. If the path is absolute, or contains invalid characters.
    fn add<D>(&mut self, path: &Path, data: D) -> Result<()>
    where
        D: FileData;

    /// Write an archive to a writer.
    fn write_to<W>(self, w: &mut W) -> Result<()>
    where
        W: Write + Seek;

    /// Write an archive to a file at a given path.
    ///
    /// The default implementation of this operation is atomic, meaning that if an
    /// error occurs while writing to the file, the file is not modified. The file
    /// will also not be accessible while changes are occurring. This is done by
    /// creating a temporary file in the same directory as the target, writing to
    /// *that*, and renaming the file to the desired path.
    #[inline]
    fn write_to_file<P>(self, p: P) -> Result<()>
    where
        P: AsRef<Path>,
    {
        write_to_file(self, p.as_ref())
    }

    #[inline]
    fn add_from_dir<P>(&mut self, dir: P) -> Result<()>
    where
        P: AsRef<Path>,
    {
        add_from_dir(self, dir.as_ref())
    }
}

#[allow(clippy::len_without_is_empty)]
pub trait FileData: 'static {
    fn len(&mut self) -> Result<u64>;

    fn write_to(&mut self, w: &mut dyn Write) -> Result<u64>;
}

pub struct ReaderData<R>(R)
where
    R: 'static + Read + Seek;

impl<R> ReaderData<R>
where
    R: 'static + Read + Seek,
{
    #[inline]
    pub fn new(r: R) -> ReaderData<R> {
        ReaderData(r)
    }

    #[inline]
    pub fn into_inner(self) -> R {
        self.0
    }

    #[inline]
    pub fn get_ref(&self) -> &R {
        &self.0
    }

    #[inline]
    pub fn get_mut(&mut self) -> &mut R {
        &mut self.0
    }
}

impl<R> FileData for ReaderData<R>
where
    R: 'static + Read + Seek,
{
    #[inline]
    fn len(&mut self) -> Result<u64> {
        Ok(self.0.seek(SeekFrom::End(0))?)
    }

    fn write_to(&mut self, w: &mut dyn Write) -> Result<u64> {
        self.0.seek(SeekFrom::Start(0))?;
        Ok(io::copy(self.get_mut(), w)?)
    }
}

impl FileData for File {
    fn len(&mut self) -> Result<u64> {
        Ok(self.metadata()?.len())
    }

    fn write_to(&mut self, w: &mut dyn Write) -> Result<u64> {
        self.seek(SeekFrom::Start(0))?;
        Ok(io::copy(self, w)?)
    }
}

impl FileData for Vec<u8> {
    fn len(&mut self) -> Result<u64> {
        Ok((&*self).len() as u64)
    }

    fn write_to(&mut self, w: &mut dyn Write) -> Result<u64> {
        w.write_all(self.as_slice())?;
        self.len()
    }
}

/// A [FileData] type that holds only a path. This is inherently racy, so it should be
/// replaced by a better solution when one arises. This is also why the type is not
/// public.
///
/// Possible alternatives:
/// 1. Use a directory file descriptor
/// 2. Just use a file (there's a limit on most platforms, so this isn't feasible)
struct RacyFsFileData {
    path: PathBuf,
}

impl FileData for RacyFsFileData {
    fn len(&mut self) -> Result<u64> {
        Ok(fs::metadata(&self.path)?.len())
    }

    fn write_to(&mut self, w: &mut dyn Write) -> Result<u64> {
        let mut f = File::open(&self.path)?;
        let n = io::copy(&mut f, w)?;
        Ok(n)
    }
}

pub fn pack_directory<P, Q>(format: Format, dir: P, archive: Q) -> Result<()>
where
    P: AsRef<Path>,
    Q: AsRef<Path>,
{
    fn inner(format: Format, dir: &Path, archive: &Path) -> Result<()> {
        let mut writer = ArchiveWriter::new(format);
        writer.add_from_dir(dir)?;
        writer.write_to_file(archive)?;
        Ok(())
    }
    inner(format, dir.as_ref(), archive.as_ref())
}

fn write_dir_inner<W>(mut writer: W, dir: &Path, dst: &Path) -> Result<()>
where
    W: ArchiveWrite,
{
    for entry in WalkDir::new(dir) {
        let entry = entry.map_err(|e| e.into_io_error().unwrap())?;
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path().strip_prefix(dir).unwrap();
        let data = RacyFsFileData {
            path: entry.path().to_owned(),
        };
        writer.add(path, data)?;
    }
    writer.write_to_file(dst)?;
    Ok(())
}

pub fn write_dir<P, Q, W>(writer: W, dir: P, dst: Q) -> Result<()>
where
    P: AsRef<Path>,
    Q: AsRef<Path>,
    W: ArchiveWrite,
{
    write_dir_inner(writer, dir.as_ref(), dst.as_ref())
}

fn write_to_file<W: ArchiveWrite>(writer: W, path: &Path) -> Result<()> {
    let mut f = if let Some(dir) = path.parent() {
        NamedTempFile::new_in(dir)?
    } else {
        NamedTempFile::new_in(env::current_dir()?)?
    };

    {
        let mut w = BufWriter::new(&mut f);
        writer.write_to(&mut w)?;
    }

    f.persist(path).map_err(|err| err.error)?;
    Ok(())
}

fn add_from_dir<W>(w: &mut W, dir: &Path) -> Result<()>
where
    W: ArchiveWrite,
{
    for entry in WalkDir::new(dir) {
        let entry = entry?;
        if entry.file_type().is_file() {
            let data = RacyFsFileData {
                path: entry.path().to_owned(),
            };
            w.add(entry.path(), data)?;
        }
    }
    Ok(())
}
