use std::error::Error;
use std::fs::File;
use std::path::PathBuf;
use structopt::StructOpt;

mod iffimage;

#[derive(StructOpt, Debug)]
#[structopt(about = "A command line utility to convert indexed PNGs to Amiga readable IFF files")]
struct Opt {
    #[structopt(parse(from_os_str))]
    infile: PathBuf,
    #[structopt(parse(from_os_str))]
    outfile: PathBuf,
}

fn main() -> Result<(), Box<dyn Error>> {
    let opt = Opt::from_args();
    let iff = iffimage::IffImage::from_png_file(opt.infile)?;
    let mut buffer = File::create(opt.outfile)?;
    iff.write(&mut buffer)?;

    Ok(())
}
