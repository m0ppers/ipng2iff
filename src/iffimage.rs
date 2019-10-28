use png::ColorType;
use png::DecodingError as PngDecodeError;
use std::error::Error;
use std::fmt;
use std::fs::File;
use std::io::Error as IoError;
use std::io::Result as IoResult;
use std::io::Write;
use std::path::Path;

#[derive(Debug)]
pub enum IffConvertError {
    WrongColorType(ColorType),
    NoPalette,
    EmptyPalette,
    TooManyColors(usize),
    InvalidPixel([u8; 3]),
}

impl fmt::Display for IffConvertError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            IffConvertError::WrongColorType(c) => f.write_fmt(format_args!(
                "Invalid ColorType {:?}. Can only work with indexed!",
                c
            )),
            IffConvertError::NoPalette => f.write_str("No palette found"),
            IffConvertError::EmptyPalette => f.write_str("Palette found, but is empty :S"),
            IffConvertError::TooManyColors(c) => {
                f.write_fmt(format_args!("Too many colors: {}", c))
            }
            IffConvertError::InvalidPixel(c) => {
                f.write_fmt(format_args!("Too many colors: {:?}", c))
            }
        }
    }
}

#[derive(Debug)]
pub enum IffLoadError {
    IoError(IoError),
    PngDecodeError(PngDecodeError),
    IffConvertError(IffConvertError),
}

impl From<IoError> for IffLoadError {
    fn from(error: IoError) -> Self {
        IffLoadError::IoError(error)
    }
}

impl From<PngDecodeError> for IffLoadError {
    fn from(error: PngDecodeError) -> Self {
        IffLoadError::PngDecodeError(error)
    }
}

impl From<IffConvertError> for IffLoadError {
    fn from(error: IffConvertError) -> Self {
        IffLoadError::IffConvertError(error)
    }
}

impl fmt::Display for IffLoadError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            IffLoadError::IoError(e) => f.write_fmt(format_args!("IoError {}", e)),
            IffLoadError::PngDecodeError(e) => f.write_fmt(format_args!("PngDecodeError {}", e)),
            IffLoadError::IffConvertError(e) => f.write_fmt(format_args!("IffConvertError {}", e)),
        }
    }
}

impl Error for IffLoadError {}

#[derive(Default)]
pub struct IffImage {
    bmhd: BitmapHeader,
    cmap: ColorMap,
    pixels: Vec<u8>,
}

#[derive(Default)]
struct BitmapHeader {
    width: u16,
    height: u16,
    x: i16,
    y: i16,
    bitplanes: u8,
    masking: u8,
    compression: u8,
    _pad1: u8,
    transparent_color: u16,
    x_aspect: u8,
    y_aspect: u8,
    page_width: u16,
    page_height: u16,
}

#[derive(Default)]
struct Color {
    r: u8,
    g: u8,
    b: u8,
}

#[derive(Default)]
struct ColorMap {
    colors: Vec<Color>,
}

impl IffImage {
    pub fn from_png_file<P: AsRef<Path>>(path: P) -> Result<IffImage, IffLoadError> {
        let decoder = png::Decoder::new(File::open(path)?);
        let (info, mut reader) = decoder.read_info()?;

        let frame_info = reader.info();
        if frame_info.color_type != ColorType::Indexed {
            return Err(From::from(IffConvertError::WrongColorType(
                frame_info.color_type,
            )));
        }

        let palette = match &frame_info.palette {
            None => return Err(From::from(IffConvertError::NoPalette)),
            Some(palette) => {
                if palette.len() == 0 {
                    return Err(From::from(IffConvertError::EmptyPalette));
                }
                palette
            }
        };

        // hmmm always RGB?
        let num_colors = palette.len() / 3;
        if num_colors > 255 {
            return Err(From::from(IffConvertError::TooManyColors(num_colors)));
        }

        let bitplanes = (num_colors as f32).log2().ceil() as u8;
        let cmap = ColorMap {
            colors: palette
                .chunks(3)
                .map(|c| Color {
                    r: c[0],
                    g: c[1],
                    b: c[2],
                })
                .collect::<Vec<_>>(),
        };

        // Allocate the output buffer.
        let mut buf = vec![0; info.buffer_size()];
        reader.next_frame(&mut buf)?;

        // hmm this nested result is really suboptimal...need an early return
        let pixels =
            buf.chunks(3).map(|pixel| {
                match cmap.colors.iter().position(|color| {
                    pixel[0] == color.r && pixel[1] == color.g && pixel[2] == color.b
                }) {
                    None => {
                        return Err(IffConvertError::InvalidPixel([
                            pixel[0], pixel[1], pixel[2],
                        ]))
                    }
                    Some(index) => Ok(index as u8),
                }
            });
        match pixels.clone().find(|p| p.is_err()) {
            Some(e) => return Err(From::from(e.err().unwrap())),
            None => (),
        }

        let pixels = pixels.map(|pixel| pixel.unwrap()).collect::<Vec<_>>();

        Ok(IffImage {
            bmhd: BitmapHeader {
                width: info.width as u16,
                height: info.height as u16,
                bitplanes,
                page_width: info.width as u16,
                page_height: info.height as u16,
                ..Default::default()
            },
            cmap,
            pixels,
            ..Default::default()
        })
    }

    pub fn write(&self, writer: &mut dyn Write) -> IoResult<()> {
        let ilbm = self.get_ilbm();
        writer.write(b"FORM")?;
        writer.write(&(ilbm.len() as u32).to_be_bytes())?;
        writer.write(&ilbm)?;
        Ok(())
    }

    fn get_bmhd(&self) -> Vec<u8> {
        let mut v = vec![];
        v.extend_from_slice(&self.bmhd.width.to_be_bytes());
        v.extend_from_slice(&self.bmhd.height.to_be_bytes());
        v.extend_from_slice(&self.bmhd.x.to_be_bytes());
        v.extend_from_slice(&self.bmhd.y.to_be_bytes());
        v.push(self.bmhd.bitplanes);
        v.push(self.bmhd.masking);
        v.push(self.bmhd.compression);
        v.push(0); // pad
        v.extend_from_slice(&self.bmhd.transparent_color.to_be_bytes());
        v.push(self.bmhd.x_aspect);
        v.push(self.bmhd.y_aspect);
        v.extend_from_slice(&self.bmhd.page_width.to_be_bytes());
        v.extend_from_slice(&self.bmhd.page_height.to_be_bytes());
        v
    }

    fn get_cmap(&self) -> Vec<u8> {
        self.cmap.colors.iter().fold(vec![], |mut v, color| {
            v.push(color.r);
            v.push(color.g);
            v.push(color.b);
            v
        })
    }

    fn get_body(&self) -> Vec<u8> {
        let mut row = vec![0u8; (self.bmhd.width / 8) as usize];
        let mut v = vec![];

        let mut row_pixel_index = 0;
        // UFF...SMEEELLLLLSSSS!!!
        for _y in 0..self.bmhd.height {
            for bpl in 0..self.bmhd.bitplanes {
                let mut pixel_index = row_pixel_index;
                for row_index in 0..self.bmhd.width / 8 {
                    let mut value = 0u8;
                    for bit in 0..8 {
                        // println!(
                        //     "pixel: {} {} {}",
                        //     self.pixels[pixel_index],
                        //     2i32.pow(bpl as u32),
                        //     ((self.pixels[pixel_index]) & (2 ^ (bpl + 1)))
                        // );
                        if ((self.pixels[pixel_index]) & 2i32.pow(bpl as u32) as u8) != 0 {
                            // 7-bit because of big endian
                            value |= 1 << (7 - bit);
                        }
                        // println!("Value: {:b}", value);
                        pixel_index += 1;
                    }
                    row[row_index as usize] = value;
                }
                v.extend_from_slice(&row);
            }
            row_pixel_index += self.bmhd.width as usize;
        }
        v
    }

    fn get_ilbm(&self) -> Vec<u8> {
        let mut v = vec![];
        v.extend_from_slice(b"ILBM");
        v.extend_from_slice(b"BMHD");
        let bmhd = self.get_bmhd();
        v.extend_from_slice(&(bmhd.len() as u32).to_be_bytes());
        v.extend_from_slice(&bmhd);

        v.extend_from_slice(b"CMAP");
        let cmap = self.get_cmap();
        v.extend_from_slice(&(cmap.len() as u32).to_be_bytes());
        v.extend_from_slice(&cmap);

        v.extend_from_slice(b"BODY");
        let body = self.get_body();
        v.extend_from_slice(&(body.len() as u32).to_be_bytes());
        v.extend_from_slice(&body);
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn one_bitplanes_body() {
        let image = IffImage {
            bmhd: BitmapHeader {
                width: 8,
                height: 1,
                bitplanes: 1,
                ..Default::default()
            },
            pixels: vec![0, 1, 0, 1, 0, 1, 0, 1],
            cmap: {
                ColorMap {
                    colors: vec![
                        Color { r: 0, g: 0, b: 0 },
                        Color {
                            r: 0xff,
                            g: 0xff,
                            b: 0xff,
                        },
                    ],
                }
            },
            ..Default::default()
        };
        let body = image.get_body();
        assert_eq!(body.len(), 1);
        assert_eq!(body[0], 0b1010101);
    }

    #[test]
    fn two_bitplanes_body() {
        let image = IffImage {
            bmhd: BitmapHeader {
                width: 8,
                height: 1,
                bitplanes: 2,
                ..Default::default()
            },
            pixels: vec![0, 1, 2, 2, 1, 0, 0, 1],
            cmap: {
                ColorMap {
                    colors: vec![
                        Color { r: 0, g: 0, b: 0 },
                        Color { r: 0, g: 0, b: 0 },
                        Color { r: 0, g: 0, b: 0 },
                    ],
                }
            },
            ..Default::default()
        };
        let body = image.get_body();
        assert_eq!(body.len(), 2);
        assert_eq!(body[0], 0b01001001);
        assert_eq!(body[1], 0b00110000);
    }
}
