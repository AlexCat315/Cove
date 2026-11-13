//! Encode a runtime-owned frame within the probe message budget.
use base64::{Engine, engine::general_purpose::STANDARD};
use cove_devtools_protocol::control::{Fault, MAX_CAPTURE_BYTES, RuntimeCapture, Viewport};
use image::{ImageEncoder, RgbaImage, codecs::png::PngEncoder, imageops};
use std::io::{self, Write};

pub struct FramePixels {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

pub fn encode(pixels: FramePixels, viewport: Viewport) -> Result<RuntimeCapture, Fault> {
    let mut image = RgbaImage::from_raw(pixels.width, pixels.height, pixels.rgba)
        .ok_or_else(|| Fault::new("capture_failed", "Invalid framebuffer size."))?;
    loop {
        let mut png = BoundedPng {
            bytes: Vec::new(),
            exceeded: false,
        };
        match PngEncoder::new(&mut png).write_image(
            image.as_raw(),
            image.width(),
            image.height(),
            image::ExtendedColorType::Rgba8,
        ) {
            Ok(()) => {
                return Ok(RuntimeCapture {
                    viewport,
                    png: STANDARD.encode(png.bytes),
                });
            }
            Err(_) if png.exceeded && (image.width() > 1 || image.height() > 1) => {
                image = imageops::thumbnail(
                    &image,
                    (image.width() / 2).max(1),
                    (image.height() / 2).max(1),
                );
            }
            Err(error) => return Err(Fault::new("capture_failed", error.to_string())),
        }
    }
}

struct BoundedPng {
    bytes: Vec<u8>,
    exceeded: bool,
}

impl Write for BoundedPng {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > MAX_CAPTURE_BYTES - self.bytes.len() {
            self.exceeded = true;
            return Err(io::Error::other("capture exceeds the probe message budget"));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
