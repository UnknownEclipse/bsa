use std::{fs::File, path::Path};

use bsa::read::tes3::Archive;
use color_eyre::Result;

pub fn main() -> Result<()> {
    let path = "/Users/benjamin/Library/Application Support/Steam/steamapps/common/The Elder Scrolls III - Morrowind/Data Files/Morrowind.bsa";
    let f = File::open(path)?;
    let archive = Archive::new(f)?;

    for entry in archive.entries()? {
        let entry = entry?;
        println!("{:?}", entry.path()?);
    }

    assert_eq!(
        &archive
            .get("textures/vfx_alpha_steam00.dds")
            .unwrap()
            .unwrap()
            .path()?,
        Path::new("textures/vfx_alpha_steam00.dds")
    );
    Ok(())
}
