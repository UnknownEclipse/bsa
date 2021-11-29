use std::{fs::File, time::Instant};

use bsa_core::Result;
use tes4_bsa::FnvArchive;

fn main() -> Result<()> {
    let path = "/Applications/Fallout New Vegas.app/Contents/Resources/drive_c/GOG Games/Fallout New Vegas/Data/Fallout - Textures2.bsa";

    let f = File::open(path)?;
    let bsa = FnvArchive::new(f)?;

    // let start = Instant::now();
    // bsa.extract1("testing/out1")?;
    // let end = Instant::now();
    // println!("#1 took {}ms", (end - start).as_millis());

    let start = Instant::now();
    bsa.extract2("testing/out2")?;
    let end = Instant::now();
    println!("#2 took {}ms", (end - start).as_millis());

    let start = Instant::now();
    bsa.extract3("testing/out3")?;
    let end = Instant::now();
    println!("#3 took {}ms", (end - start).as_millis());

    #[cfg(unix)]
    {
        let start = Instant::now();
        bsa.extract4("testing/out4")?;
        let end = Instant::now();
        println!("#4 took {}ms", (end - start).as_millis());
    }

    Ok(())
}
