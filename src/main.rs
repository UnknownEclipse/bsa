#![allow(dead_code)]

use std::{
    fs::{self, File},
    io::{self, BufReader, Cursor, Read},
    path::{Path, PathBuf},
    time::Instant,
};

use anyhow::Result;
use bsa::{
    read::{fo4::ba2::Archive as Fo4Archive, FnvArchive, FnvBsa, SseArchive, Tes3Archive},
    write::{write_dir, ArchiveWrite, ReaderData, SseWriter, Tes3Writer},
};
use walkdir::WalkDir;

fn sse_data() -> &'static Path {
    Path::new("/Applications/The Elder Scrolls V Skyrim SE.app/Contents/Resources/drive_c/Program Files (x86)/Steam/steamapps/common/Skyrim Special Edition/Data")
}

fn tes3_data() -> &'static Path {
    Path::new("/Users/benjamin/Library/Application Support/Steam/steamapps/common/The Elder Scrolls III - Morrowind/Data Files")
}

fn fnv_data() -> &'static Path {
    Path::new("/Applications/Fallout New Vegas.app/Contents/Resources/drive_c/GOG Games/Fallout New Vegas/Data")
}

fn sse_textures0() -> PathBuf {
    sse_data().join("Skyrim - Textures0.bsa")
}

fn fnv_textures() -> PathBuf {
    fnv_data().join("Fallout - Textures.bsa")
}

fn fnv_sound() -> PathBuf {
    fnv_data().join("Fallout - Sound.bsa")
}

fn tes3() -> Result<()> {
    let path = "/Users/benjamin/Library/Application Support/Steam/steamapps/common/The Elder Scrolls III - Morrowind/Data Files/Morrowind.bsa";
    let dir = "testing/tes3";

    let f = File::open(path)?;
    let f = BufReader::new(f);
    let mut archive = Tes3Archive::new(f)?;

    let mut names = Vec::new();
    for entry in archive.entries() {
        let entry = entry?;
        names.push(entry.name().replace("\\", "/"));
    }

    for name in &names {
        let mut entry = archive.open_by_name(name)?;
        let path = Path::new(dir).join(name);
        fs::create_dir_all(path.parent().unwrap())?;
        io::copy(&mut entry, &mut File::create(&path)?)?;
    }

    Ok(())
}

fn sse() -> Result<()> {
    let path = "/Applications/The Elder Scrolls V Skyrim SE.app/Contents/Resources/drive_c/Program Files (x86)/Steam/steamapps/common/Skyrim Special Edition/Data/Skyrim - Textures0.bsa";

    let dst = Path::new("testing/sse");
    fs::remove_dir_all(dst)?;

    let f = File::open(path)?;
    let r = BufReader::new(f);

    let mut bsa = SseArchive::new(r)?;

    bsa.extract(dst)?;

    Ok(())
}

fn fo4() -> Result<()> {
    let path = "/Applications/Fallout 4.app/Contents/Resources/drive_c/Program Files (x86)/Steam/steamapps/common/Fallout 4/Data/Fallout4 - Textures1.ba2";
    let dir = Path::new(path).parent().unwrap();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        if entry.file_type()?.is_file()
            && Path::new(&entry.file_name()).extension() == Some("ba2".as_ref())
        {
            // println!("{:?}", entry.path());
            let f = File::open(entry.path())?;
            let r = BufReader::new(f);
            let _ba2 = Fo4Archive::new(r)?;
        }
    }
    // let f = File::open(path)?;
    // let r = BufReader::new(f);
    // let _ba2 = Archive::new(r)?;
    Ok(())
}

fn read_sse(path: &str) -> Result<()> {
    let f = File::open(path)?;
    let r = BufReader::new(f);
    let bsa = SseArchive::new(r)?;
    for entry in bsa.entries() {
        println!("{:?}", entry.path());
    }
    Ok(())
}

fn write_tes3<P, Q>(dir: P, out: Q) -> Result<()>
where
    P: AsRef<Path>,
    Q: AsRef<Path>,
{
    let dir = dir.as_ref();

    let mut writer = Tes3Writer::new();

    for entry in WalkDir::new(dir) {
        let entry = entry?;
        if entry.file_type().is_file() {
            let path = entry.path().strip_prefix(dir)?;
            let mut r = File::open(entry.path())?;
            let mut buf = Vec::new();
            r.read_to_end(&mut buf)?;
            writer.add(path, ReaderData::new(Cursor::new(buf)))?;
        }
    }

    writer.write_to_file(out)?;

    Ok(())
}

fn assert_file_content_equal<P, Q>(a: P, b: Q) -> Result<()>
where
    P: AsRef<Path>,
    Q: AsRef<Path>,
{
    let a = fs::read(a)?;
    let b = fs::read(b)?;

    // assert_eq!(a.len(), b.len());

    for (i, (a, b)) in a.into_iter().zip(b.into_iter()).enumerate() {
        assert_eq!(a, b, "mismatch at offset {}", i);
    }

    Ok(())
}

fn write_sse<P, Q>(dir: P, dst: Q, compressed: bool, embed_names: bool) -> Result<()>
where
    P: AsRef<Path>,
    Q: AsRef<Path>,
{
    let mut writer = SseWriter::new();
    writer.set_compressed(compressed)?;
    writer.set_embed_filenames(embed_names)?;
    write_dir(writer, dir, dst)?;
    Ok(())
}

fn main() -> Result<()> {
    let f = File::open(fnv_sound())?;
    let mut archive = FnvArchive::new(f)?;

    Ok(())
}
