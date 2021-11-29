use std::fs::File;

use fo4_ba2::{Ba2, Error};

fn main() -> Result<(), Error> {
    let path = "/Applications/Fallout 4.app/Contents/Resources/drive_c/Program Files (x86)/Steam/steamapps/common/Fallout 4/Data/Fallout4 - Textures2.ba2";
    let f = File::open(path)?;

    let _ba2 = Ba2::new(f)?;
    Ok(())
}
