//! PNG strikes from OpenType's CBDT/CBLC tables, decoded without system libpng.
use crate::{ColorGlyphBitmap, ColorGlyphSource, FontFace, FreeTypeColorGlyphFace};
use cove_core::{GlyphLineMetrics, GlyphMetrics};
use std::io::Cursor;
use std::rc::Rc;
use ttf_parser::{Face, GlyphId, RasterGlyphImage, RasterImageFormat};

pub(crate) fn source(font: &FontFace) -> Option<ColorGlyphSource> {
    parse(font)?.tables().cbdt?;
    let metrics = font.clone();
    let lines = font.clone();
    let raster = font.clone();
    Some(ColorGlyphSource {
        size: font.size(),
        raster_scale: font.raster_scale(),
        metrics: Rc::new(move |glyph| strike(&metrics, glyph).map(|(_, metrics)| metrics)),
        line_metrics: Rc::new(move || {
            let Some(face) = parse(&lines) else {
                return lines.line_metrics();
            };
            let unit = lines.size() / f32::from(face.units_per_em());
            let ascent = f32::from(face.ascender()) * unit;
            let descent = -f32::from(face.descender()) * unit;
            GlyphLineMetrics {
                ascent,
                descent,
                line_height: ascent + descent + f32::from(face.line_gap()).max(0.0) * unit,
            }
        }),
        rasterize: Rc::new(move |glyph| rasterize(&raster, glyph)),
    })
}

fn parse(font: &FontFace) -> Option<Face<'_>> {
    Face::parse(font.font_bytes(), u32::from(font.face_index())).ok()
}

fn strike(font: &FontFace, glyph: u32) -> Option<(RasterGlyphImage<'_>, GlyphMetrics)> {
    let face = parse(font)?;
    let id = GlyphId(u16::try_from(glyph).ok()?);
    let scale = font.raster_scale();
    let desired = font.size() * scale;
    let image = face.tables().cbdt?.get(id, desired.ceil() as u16)?;
    if image.format != RasterImageFormat::PNG || image.pixels_per_em == 0 {
        return None;
    }
    let unit = font.size() / f32::from(image.pixels_per_em);
    let metrics = GlyphMetrics {
        glyph_index: glyph,
        width: (f32::from(image.width) * unit * scale).round().max(1.0) / scale,
        height: (f32::from(image.height) * unit * scale).round().max(1.0) / scale,
        bearing_x: f32::from(image.x) * unit,
        bearing_y: (f32::from(image.y) + f32::from(image.height)) * unit,
        advance: f32::from(face.glyph_hor_advance(id)?) * font.size()
            / f32::from(face.units_per_em()),
    };
    Some((image, metrics))
}

fn rasterize(font: &FontFace, glyph: u32) -> Option<ColorGlyphBitmap> {
    let (image, metrics) = strike(font, glyph)?;
    let mut decoder = png::Decoder::new(Cursor::new(image.data));
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info().ok()?;
    let (source_width, source_height) = (usize::from(image.width), usize::from(image.height));
    if reader.info().width as usize != source_width
        || reader.info().height as usize != source_height
    {
        return None;
    }
    let mut pixels = vec![0; reader.output_buffer_size()?];
    let output = reader.next_frame(&mut pixels).ok()?;
    pixels.truncate(output.buffer_size());
    let pixels = match output.color_type {
        png::ColorType::Rgba => pixels,
        png::ColorType::Rgb => pixels
            .chunks_exact(3)
            .flat_map(|p| [p[0], p[1], p[2], 255])
            .collect(),
        png::ColorType::Grayscale => pixels.into_iter().flat_map(|g| [g, g, g, 255]).collect(),
        png::ColorType::GrayscaleAlpha => pixels
            .chunks_exact(2)
            .flat_map(|p| [p[0], p[0], p[0], p[1]])
            .collect(),
        png::ColorType::Indexed => return None,
    };
    let scale = font.raster_scale();
    let width = (metrics.width * scale).round() as usize;
    let height = (metrics.height * scale).round() as usize;
    let pixels = if (width, height) == (source_width, source_height) {
        pixels
    } else {
        FreeTypeColorGlyphFace::resample(&pixels, source_width, source_height, width, height)?
    };
    Some(ColorGlyphBitmap {
        pixels,
        width,
        height,
        metrics,
    })
}
