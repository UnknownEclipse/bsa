use std::io::{Seek, Write};

use crate::{
    write::{ArchiveWrite, FileData, SseWriter, Tes3Writer, Tes4Writer, Tes5Writer},
    Format, Result,
};

pub struct ArchiveWriter {
    inner: ArchiveWriterInner,
}

impl ArchiveWriter {
    #[inline]
    pub fn new(format: Format) -> ArchiveWriter {
        match format {
            Format::Tes3 => Tes3Writer::new().into(),
            Format::Tes4 => Tes4Writer::new().into(),
            Format::Tes5 => Tes5Writer::new().into(),
            Format::Sse => SseWriter::new().into(),
            Format::Fo4 => todo!("fallout 4 writer"),
        }
    }

    #[inline]
    pub fn format(&self) -> Format {
        match &self.inner {
            ArchiveWriterInner::Tes3(_) => Format::Tes3,
            ArchiveWriterInner::Tes4(_) => Format::Tes4,
            ArchiveWriterInner::Tes5(_) => Format::Tes5,
            ArchiveWriterInner::Sse(_) => Format::Sse,
        }
    }
}

impl ArchiveWrite for ArchiveWriter {
    #[inline]
    fn set_compressed(&mut self, compressed: bool) -> Result<()> {
        match &mut self.inner {
            ArchiveWriterInner::Tes3(w) => w.set_compressed(compressed),
            ArchiveWriterInner::Tes4(w) => w.set_compressed(compressed),
            ArchiveWriterInner::Tes5(w) => w.set_compressed(compressed),
            ArchiveWriterInner::Sse(w) => w.set_compressed(compressed),
        }
    }

    #[inline]
    fn add<D>(&mut self, path: &std::path::Path, data: D) -> Result<()>
    where
        D: FileData,
    {
        match &mut self.inner {
            ArchiveWriterInner::Tes3(w) => w.add(path, data),
            ArchiveWriterInner::Tes4(w) => w.add(path, data),
            ArchiveWriterInner::Tes5(w) => w.add(path, data),
            ArchiveWriterInner::Sse(w) => w.add(path, data),
        }
    }

    fn write_to<W>(self, w: &mut W) -> Result<()>
    where
        W: Write + Seek,
    {
        match self.inner {
            ArchiveWriterInner::Tes3(inner) => inner.write_to(w),
            ArchiveWriterInner::Tes4(inner) => inner.write_to(w),
            ArchiveWriterInner::Tes5(inner) => inner.write_to(w),
            ArchiveWriterInner::Sse(inner) => inner.write_to(w),
        }
    }
}

impl From<Tes3Writer> for ArchiveWriter {
    #[inline]
    fn from(w: Tes3Writer) -> Self {
        ArchiveWriter {
            inner: ArchiveWriterInner::Tes3(w),
        }
    }
}

impl From<Tes4Writer> for ArchiveWriter {
    #[inline]
    fn from(w: Tes4Writer) -> Self {
        ArchiveWriter {
            inner: ArchiveWriterInner::Tes4(w),
        }
    }
}

impl From<Tes5Writer> for ArchiveWriter {
    #[inline]
    fn from(w: Tes5Writer) -> Self {
        ArchiveWriter {
            inner: ArchiveWriterInner::Tes5(w),
        }
    }
}

impl From<SseWriter> for ArchiveWriter {
    #[inline]
    fn from(w: SseWriter) -> Self {
        ArchiveWriter {
            inner: ArchiveWriterInner::Sse(w),
        }
    }
}

enum ArchiveWriterInner {
    Tes3(Tes3Writer),
    Tes4(Tes4Writer),
    Tes5(Tes5Writer),
    Sse(SseWriter),
}
