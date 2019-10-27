use std::error::Error;
use std::fs::File;

mod iffimage;

fn main() -> Result<(), Box<dyn Error>> {
    let iff = iffimage::IffImage::from_png_file("test.png")?;
    let mut buffer = File::create("test.iff")?;
    iff.write(&mut buffer)?;

    Ok(())
}
