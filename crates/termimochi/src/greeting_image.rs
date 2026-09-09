//! Local, bounded PNG/JPEG/WebP decoding into portable, reviewed ANSI artwork.
//! Editable sources are portable snapshots; external exports contain only ANSI.
use crate::{
    greeting::{ART_MAX_COLUMNS, ART_MAX_ROWS},
    greeting_art::Artwork,
};
use image::{DynamicImage, ImageDecoder, ImageFormat, Limits, RgbaImage, imageops::FilterType};
use std::{fmt::Write, io::Cursor, path::Path};

mod background;
mod processing;
pub(crate) mod source;
mod symbols;
pub(crate) use background::{Removal, RemovalReport, straight_color};
pub(crate) use processing::{Adjustments, Ink, Structure};

pub(crate) const FILE_LIMIT: u64 = 16 * 1024 * 1024;
const MAX_DIMENSION: u32 = 8192;
const MAX_PIXELS: u64 = 16_000_000;
const MAX_DECODE_BYTES: u64 = 64 * 1024 * 1024;
const MAX_CELLS: u32 = 3072;
const ASCII_MAX_CELLS: u32 = 160 * 96;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Style {
    Ascii,
    HalfBlocks,
    Detail,
}

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Options {
    pub columns: u32,
    pub style: Style,
    pub cell_ratio: f64,
    pub background: [u8; 3],
    pub foreground: [u8; 3],
    pub invert: bool,
    pub edits: Adjustments,
}

pub(crate) struct DecodedImage {
    // Premultiplied alpha prevents invisible RGB from making dark resize halos.
    pixels: RgbaImage,
    pub dimensions: (u32, u32),
    pub format: &'static str,
}

pub(crate) struct ConvertedImage {
    pub artwork: Artwork,
    pub columns: u32,
    pub rows: u32,
    pub source_dimensions: (u32, u32),
    pub format: &'static str,
    pub reference: RgbaImage,
    pub removal: RemovalReport,
}

pub(crate) fn is_image(path: &Path) -> bool {
    path.extension()
        .and_then(|s| s.to_str())
        .is_some_and(|ext| {
            ["png", "jpg", "jpeg", "webp"]
                .iter()
                .any(|name| ext.eq_ignore_ascii_case(name))
        })
}

#[cfg(test)]
pub(crate) fn load(path: &Path) -> Result<DecodedImage, String> {
    if !is_image(path) {
        return Err(
            "Choose PNG, JPG or WebP. SVG, GIF and image-display protocols are not supported yet."
                .into(),
        );
    }
    let bytes = crate::typography_preset::read_private_with_limit(path, FILE_LIMIT)?
        .ok_or("Image file no longer exists.")?;
    decode(&bytes)
}

fn limits() -> Limits {
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_DIMENSION);
    limits.max_image_height = Some(MAX_DIMENSION);
    limits.max_alloc = Some(MAX_DECODE_BYTES);
    limits
}

fn check_dimensions(width: u32, height: u32, bytes: u64) -> Result<(), String> {
    if width == 0
        || height == 0
        || width > MAX_DIMENSION
        || height > MAX_DIMENSION
        || u64::from(width) * u64::from(height) > MAX_PIXELS
        || bytes > MAX_DECODE_BYTES
    {
        return Err("Image exceeds the decode limits: 8192 px per side, 16 megapixels and 64 MiB of pixel data.".into());
    }
    Ok(())
}

pub(crate) fn decode(bytes: &[u8]) -> Result<DecodedImage, String> {
    if bytes.len() as u64 > FILE_LIMIT {
        return Err("Image exceeds 16 MiB.".into());
    }
    let format = image::guess_format(bytes)
        .map_err(|_| "Not a supported image. Choose PNG, JPG or WebP.")?;
    let cursor = Cursor::new(bytes);
    let error = |e: image::ImageError| format!("Image could not be decoded: {e}");
    let animated = "Animated images are not supported yet. Export a still PNG, JPG or WebP first.";
    match format {
        ImageFormat::Png => {
            let decoder =
                image::codecs::png::PngDecoder::with_limits(cursor, limits()).map_err(error)?;
            if decoder.is_apng().map_err(error)? {
                return Err(animated.into());
            }
            decode_pixels(decoder, "PNG")
        }
        ImageFormat::Jpeg => decode_pixels(
            image::codecs::jpeg::JpegDecoder::new(cursor).map_err(error)?,
            "JPG",
        ),
        ImageFormat::WebP => {
            // Reject oversized/truncated RIFF chunks before a decoder can read
            // optional metadata. A filename extension alone is never trusted.
            let mut offset = 12usize;
            while offset < bytes.len() {
                let header = bytes
                    .get(offset..offset + 8)
                    .ok_or("Truncated WebP chunk.")?;
                let length = u32::from_le_bytes(header[4..8].try_into().unwrap()) as usize;
                offset = offset
                    .checked_add(8)
                    .and_then(|n| n.checked_add(length))
                    .and_then(|n| n.checked_add(length % 2))
                    .ok_or("WebP chunk is too large.")?;
                if offset > bytes.len() {
                    return Err("Truncated WebP chunk.".into());
                }
            }
            let decoder = image::codecs::webp::WebPDecoder::new(cursor).map_err(error)?;
            if decoder.has_animation() {
                return Err(animated.into());
            }
            decode_pixels(decoder, "WebP")
        }
        _ => Err("Choose PNG, JPG or WebP. SVG and animated images are not supported yet.".into()),
    }
}

fn decode_pixels(
    mut decoder: impl ImageDecoder,
    format: &'static str,
) -> Result<DecodedImage, String> {
    let (width, height) = decoder.dimensions();
    check_dimensions(width, height, decoder.total_bytes())?;
    decoder.set_limits(limits()).map_err(|e| e.to_string())?;
    let orientation = decoder.orientation().map_err(|e| e.to_string())?;
    let mut image = DynamicImage::from_decoder(decoder).map_err(|e| e.to_string())?;
    image.apply_orientation(orientation);
    let dimensions = (image.width(), image.height());
    let mut pixels = image.into_rgba8();
    for pixel in pixels.pixels_mut() {
        for channel in 0..3 {
            pixel[channel] = ((u16::from(pixel[channel]) * u16::from(pixel[3]) + 127) / 255) as u8;
        }
    }
    let scale = (1024.0 / f64::from(dimensions.0.max(dimensions.1))).min(1.0);
    if scale < 1.0 {
        pixels = image::imageops::resize(
            &pixels,
            (f64::from(dimensions.0) * scale).round().max(1.0) as u32,
            (f64::from(dimensions.1) * scale).round().max(1.0) as u32,
            FilterType::Triangle,
        );
    }
    Ok(DecodedImage {
        pixels,
        dimensions,
        format,
    })
}

fn grid(dimensions: (u32, u32), options: Options) -> Result<(u32, u32), String> {
    let (max_columns, max_rows, max_cells) = grid_limits(options.style);
    if !(1..=max_columns).contains(&options.columns)
        || !options.cell_ratio.is_finite()
        || !(0.1..=2.0).contains(&options.cell_ratio)
        || dimensions.0 == 0
        || dimensions.1 == 0
    {
        return Err("Invalid character grid settings.".into());
    }
    let aspect = f64::from(dimensions.1) / f64::from(dimensions.0) * options.cell_ratio;
    let width = f64::from(options.columns)
        .min(f64::from(max_rows) / aspect)
        .min((f64::from(max_cells) / aspect).sqrt());
    let columns = width.floor().max(1.0) as u32;
    let rows = (f64::from(columns) * aspect)
        .round()
        .clamp(1.0, f64::from(max_rows)) as u32;
    Ok((columns, rows.min(max_cells / columns)))
}

pub(crate) fn grid_limits(style: Style) -> (u32, u32, u32) {
    if style == Style::Ascii {
        (ART_MAX_COLUMNS as u32, ART_MAX_ROWS as u32, ASCII_MAX_CELLS)
    } else {
        (120, 64, MAX_CELLS)
    }
}

fn composite(pixel: &image::Rgba<u8>, background: [u8; 3]) -> Option<[u8; 3]> {
    if pixel[3] <= 1 {
        return None;
    }
    Some(std::array::from_fn(|i| {
        (u16::from(pixel[i]) + (u16::from(background[i]) * u16::from(255 - pixel[3]) + 127) / 255)
            .min(255) as u8
    }))
}

type Color = [u8; 3];
type Character = (char, Option<Color>, Option<Color>);
const QUADRANTS: [char; 16] = [
    ' ', '▘', '▝', '▀', '▖', '▌', '▞', '▛', '▗', '▚', '▐', '▜', '▄', '▙', '▟', '█',
];

fn luma(rgb: Color) -> f64 {
    (f64::from(rgb[0]) * 2126.0 + f64::from(rgb[1]) * 7152.0 + f64::from(rgb[2]) * 722.0) / 10000.0
}

fn mean(colors: impl Iterator<Item = Color>) -> Color {
    let (sum, count) = colors.fold(([0u32; 3], 0), |(mut sum, count), color| {
        for i in 0..3 {
            sum[i] += u32::from(color[i]);
        }
        (sum, count + 1)
    });
    std::array::from_fn(|i| ((sum[i] + count / 2) / count.max(1)) as u8)
}

// Fit all two-color partitions of a 2×2 cell. Unlike an averaged half block,
// this preserves vertical boundaries and corners as well as horizontal ones.
#[cfg(test)]
fn detail_cell(samples: [Option<Color>; 4]) -> Character {
    symbols::matched_cell(std::array::from_fn(|i| {
        samples[(i / 8 / 4) * 2 + i % 8 / 4]
    }))
}

// ASCII is an ink rendition, not a pixel facsimile: retain sub-cell dark/light
// contours and strengthen pastel ink that otherwise disappears on the terminal.
fn ascii_cell(samples: [Option<Color>; 16], options: Options) -> Character {
    let count = samples.iter().flatten().count();
    if count == 0 {
        return (' ', None, None);
    }
    let background = luma(options.background);
    let contrast = |color: Color| {
        let difference = luma(color) - background;
        let saturation = f64::from(color.iter().max().unwrap() - color.iter().min().unwrap());
        // Ignore near-paper highlights on light backgrounds (and near-black on
        // dark ones), rather than covering the empty image margin in commas.
        if saturation < 12.0
            && ((background > 180.0 && luma(color) > background - 22.0)
                || (background < 75.0 && luma(color) < background + 22.0))
        {
            0.0
        } else {
            difference.abs().max(saturation * 0.28)
        }
    };
    let ink = samples.map(|s| s.map_or(0.0, contrast));
    let strongest = samples
        .iter()
        .flatten()
        .max_by(|a, b| contrast(**a).total_cmp(&contrast(**b)))
        .copied()
        .unwrap();
    let peak = contrast(strongest);
    if peak < 8.0 {
        return (' ', Some(mean(samples.into_iter().flatten())), None);
    }
    let average = ink.iter().sum::<f64>() / 16.0;
    let weight = (average * 0.55 + peak * 0.45) / background.max(255.0 - background).max(128.0);
    let mut density = weight.clamp(0.0, 1.0).sqrt();
    if options.invert {
        density = 1.0 - density;
    }
    const RAMP: &[u8] = b".:;irsXA253hMHGS#9B&@";
    let mut character = RAMP[(density * (RAMP.len() - 1) as f64).round() as usize] as char;
    // Covariance of the strong pixels picks long, thin contour directions.
    let threshold = peak * 0.68;
    let points: Vec<_> = ink
        .iter()
        .enumerate()
        .filter(|(_, v)| **v > threshold)
        .map(|(i, _)| ((i % 4) as f64, (i / 4) as f64))
        .collect();
    if options.edits.structure != Structure::Tone
        && options.edits.edges > 0.0
        && (2..=9).contains(&points.len())
        && peak > average * 1.5
    {
        let n = points.len() as f64;
        let cx = points.iter().map(|p| p.0).sum::<f64>() / n;
        let cy = points.iter().map(|p| p.1).sum::<f64>() / n;
        let xx = points.iter().map(|p| (p.0 - cx).powi(2)).sum::<f64>();
        let yy = points.iter().map(|p| (p.1 - cy).powi(2)).sum::<f64>();
        let xy = points.iter().map(|p| (p.0 - cx) * (p.1 - cy)).sum::<f64>();
        character = if xx > yy * 3.0 {
            '-'
        } else if yy > xx * 3.0 {
            '|'
        } else if xy.abs() > (xx + yy) * 0.3 {
            if xy > 0.0 { '\\' } else { '/' }
        } else {
            character
        };
    }
    let color = mean(
        samples
            .into_iter()
            .flatten()
            .filter(|c| contrast(*c) >= peak * 0.55),
    );
    let target = if luma(color) < background { 0.0 } else { 255.0 };
    let required = 110.0_f64.min((target - background).abs());
    let current = (luma(color) - background).abs();
    let boost = ((required - current) / (target - luma(color)).abs().max(1.0)).clamp(0.0, 0.8);
    let color = color.map(|c| (f64::from(c) * (1.0 - boost) + target * boost).round() as u8);
    (character, Some(color), None)
}

fn structured_ascii(
    samples: [Option<Color>; 16],
    edges: [processing::Edge; 16],
    options: Options,
) -> Character {
    let (mut character, mut foreground, background) = ascii_cell(samples, options);
    let mut directions = [0.0; 4];
    for edge in edges {
        directions[edge.direction] += edge.strength;
    }
    let total: f64 = directions.iter().sum();
    let direction = (0..4)
        .max_by(|&a, &b| directions[a].total_cmp(&directions[b]))
        .unwrap();
    if options.edits.structure == Structure::Outline
        || options.edits.structure == Structure::Balanced
            && total > 2.0
            && directions[direction] > total * 0.65
    {
        character = if total < 1.2 {
            ' '
        } else if directions[direction] < total * 0.45 {
            '+'
        } else {
            ['|', '/', '-', '\\'][direction]
        };
        if character != ' ' && foreground.is_none() {
            foreground = Some(options.foreground);
        }
    }
    (
        character,
        foreground.map(|c| processing::ink_color(c, options)),
        background,
    )
}

impl DecodedImage {
    pub fn convert(&self, options: Options) -> Result<ConvertedImage, String> {
        options.edits.validate()?;
        let reference = processing::crop_image(&self.pixels, options.edits);
        let (columns, rows) = grid(reference.dimensions(), options)?;
        let mut prepared = reference.clone();
        let removal = background::remove(
            &mut prepared,
            options.edits.removal,
            if options.edits.removal.enabled && options.edits.removal.color.is_none() {
                background::detect(&self.pixels)
            } else {
                None
            },
        );
        processing::adjust(&mut prepared, options.edits);
        let (sx, sy) = match options.style {
            Style::Ascii => (4, 4),
            Style::HalfBlocks => (1, 2),
            Style::Detail => (8, 8),
        };
        let mut sampled =
            image::imageops::resize(&prepared, columns * sx, rows * sy, FilterType::Triangle);
        if options.edits.smoothing > 0.0 {
            sampled = image::imageops::blur(&sampled, (options.edits.smoothing * 1.2) as f32);
        }
        let edges = if options.style == Style::Ascii {
            processing::edges(&sampled, options)
        } else {
            Vec::new()
        };
        let mut ansi = String::new();
        let mut visible = false;
        for y in 0..rows {
            let mut previous = (None, None);
            for x in 0..columns {
                let sample = |dx, dy| {
                    composite(
                        sampled.get_pixel(x * sx + dx, y * sy + dy),
                        options.background,
                    )
                    .map(|color| {
                        if options.style == Style::Ascii || options.edits.ink == Ink::Color {
                            color
                        } else if options.edits.ink == Ink::Gray {
                            [luma(color).round() as u8; 3]
                        } else {
                            let bg = luma(options.background);
                            let density =
                                ((luma(color) - bg).abs() / bg.max(255.0 - bg)).clamp(0.0, 1.0);
                            std::array::from_fn(|i| {
                                (f64::from(options.background[i]) * (1.0 - density)
                                    + f64::from(options.foreground[i]) * density)
                                    .round() as u8
                            })
                        }
                    })
                };
                let (character, foreground, background) = match options.style {
                    Style::Detail => symbols::matched_cell(std::array::from_fn(|i| {
                        sample(i as u32 % 8, i as u32 / 8)
                    })),
                    Style::Ascii => structured_ascii(
                        std::array::from_fn(|i| sample(i as u32 % 4, i as u32 / 4)),
                        std::array::from_fn(|i| {
                            edges[((y * 4 + i as u32 / 4) * columns * 4 + x * 4 + i as u32 % 4)
                                as usize]
                        }),
                        options,
                    ),
                    Style::HalfBlocks => {
                        let top = sample(0, 0);
                        let bottom = sample(0, 1);
                        match (top, bottom) {
                            (Some(a), Some(b)) if a == b => ('█', Some(a), None),
                            (Some(a), Some(b)) => ('▀', Some(a), Some(b)),
                            (Some(a), None) => ('▀', Some(a), None),
                            (None, Some(b)) => ('▄', Some(b), None),
                            (None, None) => (' ', None, None),
                        }
                    }
                };
                visible |= foreground.is_some() || background.is_some();
                for (color, old, channel) in
                    [(foreground, previous.0, 38), (background, previous.1, 48)]
                {
                    if color != old {
                        if let Some([r, g, b]) = color {
                            write!(ansi, "\x1b[{channel};2;{r};{g};{b}m").unwrap();
                        } else {
                            write!(ansi, "\x1b[{}m", channel + 1).unwrap();
                        }
                    }
                }
                ansi.push(character);
                previous = (foreground, background);
            }
            ansi.push_str("\x1b[0m");
            if y + 1 < rows {
                ansi.push('\n');
            }
        }
        if !visible {
            if options.edits.removal.enabled {
                return Err("No visible artwork remains. Lower background tolerance, turn removal off or Reset adjustments.".into());
            }
            return Err(
                "Image is fully transparent; there is no visible artwork to import.".into(),
            );
        }
        let artwork = Artwork::parse(&ansi)?;
        artwork.validate()?;
        if artwork.plain.trim().is_empty() {
            return Err("No visible characters remain. Reduce cropping, adjust exposure or choose a different structure mode.".into());
        }
        Ok(ConvertedImage {
            artwork,
            columns,
            rows,
            source_dimensions: self.dimensions,
            format: self.format,
            reference,
            removal,
        })
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::{
        greeting::{GreetingPreset, GreetingSettings},
        greeting_art,
    };

    pub(crate) fn fixture() -> RgbaImage {
        RgbaImage::from_fn(240, 160, |x, y| {
            image::Rgba([
                (x * 37 + y * 5) as u8,
                (x * 7 + y * 23) as u8,
                (x * 19 + y * 3) as u8,
                if x < 12 { 0 } else { 255 },
            ])
        })
    }

    fn encoded(pixels: RgbaImage, format: ImageFormat) -> Vec<u8> {
        let mut buffer = Cursor::new(Vec::new());
        let image = DynamicImage::ImageRgba8(pixels);
        let image = if format == ImageFormat::Jpeg {
            DynamicImage::ImageRgb8(image.into_rgb8())
        } else {
            image
        };
        image.write_to(&mut buffer, format).unwrap();
        buffer.into_inner()
    }

    fn options(style: Style) -> Options {
        Options {
            columns: 48,
            style,
            cell_ratio: 0.5,
            background: [245, 246, 247],
            foreground: [35, 38, 42],
            invert: false,
            edits: Adjustments::default(),
        }
    }

    #[test]
    fn background_removal_flows_through_every_style_without_changing_reference() {
        let source = RgbaImage::from_fn(96, 96, |x, y| {
            if (24..72).contains(&x)
                && (24..72).contains(&y)
                && !((36..60).contains(&x) && (36..60).contains(&y))
            {
                image::Rgba([30, 55, 90, 255])
            } else {
                image::Rgba([255; 4])
            }
        });
        for format in [ImageFormat::Png, ImageFormat::Jpeg, ImageFormat::WebP] {
            let decoded = decode(&encoded(source.clone(), format)).unwrap();
            for style in [Style::Ascii, Style::Detail, Style::HalfBlocks] {
                let normal = decoded.convert(options(style)).unwrap();
                let mut opts = options(style);
                opts.edits.removal.enabled = true;
                let removed = decoded.convert(opts).unwrap();
                assert!(removed.removal.changed_pixels > 0);
                assert!(removed.removal.issue.is_none());
                assert_eq!(removed.reference, normal.reference);
                assert_eq!(
                    (removed.columns, removed.rows),
                    (normal.columns, normal.rows)
                );
                assert_ne!(removed.artwork, normal.artwork);
                assert!(
                    removed
                        .artwork
                        .plain
                        .lines()
                        .next()
                        .unwrap()
                        .trim()
                        .is_empty()
                );
                assert_eq!(
                    Artwork::parse(&removed.artwork.ansi).unwrap(),
                    removed.artwork
                );
                let mut settings = crate::greeting::GreetingSettings::starter();
                settings.import_artwork(removed.artwork.clone()).unwrap();
                assert_eq!(
                    crate::fastfetch_document::value(&settings.fastfetch_config().unwrap())
                        .unwrap()["logo"]["source"],
                    removed.artwork.ansi
                );
                assert_eq!(
                    decoded.convert(options(style)).unwrap().artwork,
                    normal.artwork
                );
            }
        }
        let blank = decode(&encoded(
            RgbaImage::from_pixel(16, 16, image::Rgba([255; 4])),
            ImageFormat::Png,
        ))
        .unwrap();
        let mut opts = options(Style::Detail);
        opts.edits.removal.enabled = true;
        assert!(
            blank.convert(opts).is_err(),
            "Never accept an entirely erased logo"
        );
    }

    #[test]
    fn png_jpg_webp_decode_real_bytes_and_preserve_safe_character_grids() {
        for format in [ImageFormat::Png, ImageFormat::Jpeg, ImageFormat::WebP] {
            let source = decode(&encoded(fixture(), format)).unwrap();
            assert_eq!(source.dimensions, (240, 160));
            for style in [Style::Ascii, Style::HalfBlocks, Style::Detail] {
                let art = source.convert(options(style)).unwrap();
                assert_eq!((art.columns, art.rows), (48, 16));
                assert!(art.artwork.ansi.contains("38;2;"));
                assert!(!art.artwork.filtered);
                assert!(!art.artwork.ansi.contains("\x1b]"));
                art.artwork.validate().unwrap();
                if style == Style::Ascii {
                    assert!(art.artwork.plain.is_ascii());
                }
            }
        }
    }

    #[test]
    fn half_blocks_keep_top_bottom_rgb_and_transparent_cells() {
        let pixels = RgbaImage::from_raw(
            2,
            2,
            vec![
                255, 0, 0, 255, 17, 88, 99, 0, 0, 0, 255, 255, 0, 255, 0, 255,
            ],
        )
        .unwrap();
        let source = decode(&encoded(pixels, ImageFormat::Png)).unwrap();
        let art = source
            .convert(Options {
                columns: 2,
                ..options(Style::HalfBlocks)
            })
            .unwrap()
            .artwork;
        assert_eq!(art.plain, "▀▄");
        assert!(art.ansi.contains("38;2;255;0;0m"));
        assert!(art.ansi.contains("48;2;0;0;255m"));
        assert!(art.ansi.contains("38;2;0;255;0m"));
        assert!(art.ansi.contains("\x1b[49m"));
        assert!(art.ansi.ends_with("\x1b[0m"));
    }

    #[test]
    fn alpha_resampling_does_not_leak_hidden_rgb_and_matte_is_explicit() {
        let image = RgbaImage::from_raw(2, 1, vec![0, 0, 255, 0, 255, 0, 0, 255]).unwrap();
        let source = decode(&encoded(image, ImageFormat::Png)).unwrap();
        assert_eq!(source.pixels.get_pixel(0, 0).0, [0; 4]);
        let art = source
            .convert(Options {
                columns: 1,
                background: [0, 0, 0],
                ..options(Style::HalfBlocks)
            })
            .unwrap();
        assert!(!art.artwork.ansi.contains(";0;0;255m"));
        assert_eq!(
            composite(&image::Rgba([128, 0, 0, 128]), [255; 3]),
            Some([255, 127, 127])
        );
        let transparent = RgbaImage::from_pixel(3, 3, image::Rgba([17, 33, 255, 0]));
        assert!(
            decode(&encoded(transparent, ImageFormat::Png))
                .unwrap()
                .convert(options(Style::Ascii))
                .err()
                .unwrap()
                .contains("transparent")
        );
    }

    #[test]
    fn jpeg_exif_orientation_is_applied_before_grid_sizing() {
        let jpeg = encoded(
            RgbaImage::from_pixel(2, 3, image::Rgba([255, 0, 0, 255])),
            ImageFormat::Jpeg,
        );
        let exif = b"Exif\0\0II\x2a\0\x08\0\0\0\x01\0\x12\x01\x03\0\x01\0\0\0\x06\0\0\0\0\0\0\0";
        let mut rotated = jpeg[..2].to_vec();
        rotated.extend_from_slice(b"\xff\xe1");
        rotated.extend_from_slice(&((exif.len() + 2) as u16).to_be_bytes());
        rotated.extend_from_slice(exif);
        rotated.extend_from_slice(&jpeg[2..]);
        assert_eq!(decode(&rotated).unwrap().dimensions, (3, 2));
    }

    #[test]
    fn bad_files_oversized_headers_and_unsupported_formats_fail_without_panics() {
        for bytes in [
            b"".as_slice(),
            b"<svg xmlns='http://www.w3.org/2000/svg'/>",
            b"GIF89a",
            b"\x89PNG\r\n\x1a\n",
            b"RIFF\xff\xff\xff\xffWEBPVP8L\xff\xff\xff\xff",
        ] {
            assert!(decode(bytes).is_err());
        }
        assert!(decode(&vec![0; FILE_LIMIT as usize + 1]).is_err());
        assert!(check_dimensions(8193, 1, 32772).is_err());
        assert!(check_dimensions(5000, 4000, 80_000_000).is_err());
        assert!(check_dimensions(100, 100, MAX_DECODE_BYTES + 1).is_err());
        assert!(check_dimensions(0, 100, 0).is_err());
        let wide = RgbaImage::from_pixel(8193, 1, image::Rgba([0, 0, 0, 255]));
        assert!(decode(&encoded(wide, ImageFormat::Png)).is_err());
        for ratio in [f64::NAN, f64::INFINITY, 0.0, -1.0, 2.1] {
            assert!(
                grid(
                    (100, 100),
                    Options {
                        cell_ratio: ratio,
                        ..options(Style::Ascii)
                    }
                )
                .is_err()
            );
        }
        assert!(
            grid(
                (100, 100),
                Options {
                    columns: ART_MAX_COLUMNS as u32 + 1,
                    ..options(Style::Ascii)
                }
            )
            .is_err()
        );
    }

    #[test]
    fn animated_png_and_webp_are_explicitly_rejected() {
        let mut apng = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut apng, 1, 1);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_animated(2, 0).unwrap();
            let mut writer = encoder.write_header().unwrap();
            writer.write_image_data(&[255, 0, 0, 255]).unwrap();
            writer.write_image_data(&[0, 255, 0, 255]).unwrap();
        }
        assert!(decode(&apng).err().unwrap().contains("Animated"));
        fn chunk(tag: &[u8; 4], payload: &[u8]) -> Vec<u8> {
            let mut bytes = tag.to_vec();
            bytes.extend_from_slice(&(payload.len() as u32).to_le_bytes());
            bytes.extend_from_slice(payload);
            if payload.len() % 2 == 1 {
                bytes.push(0);
            }
            bytes
        }
        let webp = encoded(
            RgbaImage::from_pixel(1, 1, image::Rgba([255, 0, 0, 255])),
            ImageFormat::WebP,
        );
        let mut content = b"WEBP".to_vec();
        content.extend(chunk(b"VP8X", &[2, 0, 0, 0, 0, 0, 0, 0, 0, 0]));
        content.extend(chunk(b"ANIM", &[0; 6]));
        let mut frame = vec![0; 16];
        frame.extend_from_slice(&webp[12..]);
        content.extend(chunk(b"ANMF", &frame));
        let mut animated = b"RIFF".to_vec();
        animated.extend_from_slice(&(content.len() as u32).to_le_bytes());
        animated.extend(content);
        assert!(decode(&animated).err().unwrap().contains("Animated"));
    }

    #[test]
    fn local_file_loading_checks_types_aliases_and_size_without_touching_sources() {
        let root = tempfile::tempdir().unwrap();
        let bytes = encoded(fixture(), ImageFormat::Png);
        let path = root.path().join("art.PnG");
        std::fs::write(&path, &bytes).unwrap();
        load(&path).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        let alias = root.path().join("link.png");
        std::os::unix::fs::symlink(&path, &alias).unwrap();
        assert!(load(&alias).is_err());
        assert!(load(root.path()).is_err());
        assert!(load(&root.path().join("missing.png")).is_err());
        let oversized = root.path().join("huge.png");
        std::fs::File::create(&oversized)
            .unwrap()
            .set_len(FILE_LIMIT + 1)
            .unwrap();
        assert!(load(&oversized).is_err());
        let renamed = root.path().join("not-an-image.jpg");
        std::fs::write(&renamed, b"<svg/>").unwrap();
        assert!(load(&renamed).is_err());
    }

    #[test]
    fn large_colored_art_roundtrips_ansi_fastfetch_presets_and_workspaces() {
        let source = decode(&encoded(fixture(), ImageFormat::Png)).unwrap();
        for style in [Style::Ascii, Style::HalfBlocks, Style::Detail] {
            let converted = source
                .convert(Options {
                    columns: grid_limits(style).0,
                    ..options(style)
                })
                .unwrap();
            assert!(converted.columns * converted.rows <= grid_limits(style).2);
            let art = converted.artwork;
            assert!(art.ansi.len() > 32 * 1024);
            assert!(art.ansi.len() <= crate::greeting::ART_MAX_ANSI_BYTES);
            assert!(art.plain.len() <= crate::greeting::ART_MAX_BYTES);
            let root = tempfile::tempdir().unwrap();
            let path = root.path().join("logo.ans");
            greeting_art::export_file(&path, &art.ansi, true, &None).unwrap();
            assert_eq!(greeting_art::file_art(&path).unwrap(), art);
            let mut designer = GreetingSettings::starter();
            designer.import_artwork(art.clone()).unwrap();
            let config = designer.fastfetch_config().unwrap();
            let value = crate::fastfetch_document::value(&config).unwrap();
            assert_eq!(value["logo"]["type"], "data-raw");
            assert_eq!(value["logo"]["source"], art.ansi);
            let mut imported = GreetingSettings { imported_source:Some("// keep my comments\n{\"logo\":{\"type\":\"none\"},\"modules\":[\"cpu\"],\"customKey\":true}".into()), ..Default::default() };
            imported.import_artwork(art.clone()).unwrap();
            assert!(
                imported
                    .fastfetch_config()
                    .unwrap()
                    .contains("// keep my comments")
            );
            assert!(imported.fastfetch_config().unwrap().contains("customKey"));
            for settings in [designer, imported] {
                let preset = GreetingPreset::new(settings.clone());
                let bytes = crate::document_store::encode(&preset).unwrap();
                assert_eq!(
                    crate::document_store::decode::<GreetingPreset>(&bytes).unwrap(),
                    preset
                );
                let palette = termimochi_core::PtyxisPalette::from_text(include_str!(
                    "../resources/themes/fog-paper.palette"
                ))
                .unwrap();
                let mut workspace = crate::workspace::Workspace::new(
                    &palette,
                    termimochi_core::Variant::Light,
                    Default::default(),
                    Default::default(),
                    Default::default(),
                    None,
                    true,
                );
                workspace.greeting = settings;
                let bytes = crate::document_store::encode(&workspace).unwrap();
                assert_eq!(
                    crate::document_store::decode::<crate::workspace::Workspace>(&bytes).unwrap(),
                    workspace
                );
            }
        }
    }

    #[test]
    fn maximum_ascii_and_imported_config_survive_save_and_restart() {
        use crate::document_store::{Document, DocumentStore};
        let row = format!("{}\x1b[0m", "\x1b[38;2;255;254;253m@".repeat(160));
        let art = Artwork::parse(&vec![row; 96].join("\n")).unwrap();
        let mut settings = GreetingSettings {
            imported_source: Some("{}".into()),
            ..Default::default()
        };
        settings.import_artwork(art).unwrap();
        let source = settings.imported_source.as_mut().unwrap();
        let padding = crate::fastfetch_document::LIMIT as usize - source.len();
        source.insert_str(0, &format!("//{}\n", "x".repeat(padding - 3)));
        assert_eq!(source.len() as u64, crate::fastfetch_document::LIMIT);
        let preset = GreetingPreset::new(settings.clone());
        let bytes = crate::document_store::encode(&preset).unwrap();
        // The imported document, logo descriptor and ANSI snapshot are all
        // retained. Together they can legitimately exceed the old 1 MiB cap.
        assert!(bytes.len() > 1024 * 1024);
        assert!(bytes.len() as u64 <= GreetingPreset::MAX_BYTES);
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("dense.termimochi-greeting.json");
        DocumentStore::<GreetingPreset>::open(path.clone())
            .unwrap()
            .save(&preset)
            .unwrap();
        assert_eq!(
            DocumentStore::<GreetingPreset>::open(path)
                .unwrap()
                .document()
                .unwrap(),
            Some(preset)
        );
        let palette = termimochi_core::PtyxisPalette::from_text(include_str!(
            "../resources/themes/fog-paper.palette"
        ))
        .unwrap();
        let mut workspace = crate::workspace::Workspace::new(
            &palette,
            termimochi_core::Variant::Light,
            Default::default(),
            Default::default(),
            Default::default(),
            None,
            true,
        );
        workspace.greeting = settings;
        let bytes = crate::document_store::encode(&workspace).unwrap();
        assert!(bytes.len() > 1024 * 1024);
        assert_eq!(
            crate::document_store::decode::<crate::workspace::Workspace>(&bytes).unwrap(),
            workspace
        );
        let too_large = vec![b' '; GreetingPreset::MAX_BYTES as usize + 1];
        assert!(crate::document_store::decode::<GreetingPreset>(&too_large).is_err());
    }

    #[test]
    fn aspect_ratio_and_dense_output_stay_bounded_for_every_width_and_style() {
        for dimensions in [(1, 1), (1, 8192), (8192, 1), (4000, 3000), (3000, 4000)] {
            for columns in 1..=120 {
                let (width, height) = grid(
                    dimensions,
                    Options {
                        columns,
                        ..options(Style::HalfBlocks)
                    },
                )
                .unwrap();
                assert!((1..=columns).contains(&width));
                assert!((1..=64).contains(&height));
                assert!(width * height <= MAX_CELLS);
            }
        }
        let source = decode(&encoded(fixture(), ImageFormat::Png)).unwrap();
        let normal = source.convert(options(Style::Ascii)).unwrap();
        let inverted = source
            .convert(Options {
                invert: true,
                ..options(Style::Ascii)
            })
            .unwrap();
        assert_ne!(normal.artwork.plain, inverted.artwork.plain);
        assert_eq!(
            normal.artwork,
            source.convert(options(Style::Ascii)).unwrap().artwork
        );
    }

    #[test]
    #[ignore = "requires system Fastfetch and Bubblewrap; checks large colored image output"]
    fn real_fastfetch_preserves_large_converted_ansi_without_truncation() {
        let image = decode(&encoded(fixture(), ImageFormat::Png)).unwrap();
        for style in [Style::Ascii, Style::HalfBlocks, Style::Detail] {
            let art = image
                .convert(Options {
                    columns: grid_limits(style).0,
                    ..options(style)
                })
                .unwrap()
                .artwork;
            assert!(art.ansi.len() > 32 * 1024);
            let result = crate::greeting_official::render_config(serde_json::json!({"logo":{"type":"data-raw","source":art.ansi,"printRemaining":true,"padding":{"left":0,"right":0,"top":0}},"modules":[{"type":"custom","format":""}]})).unwrap();
            // Native output can include trailing blank terminal rows, which
            // are not part of the bounded logo artwork.
            let output = Artwork::parse(result.trim_end()).unwrap();
            assert_eq!(output.plain.trim_end(), art.plain.trim_end());
            assert!(output.ansi.contains("38;2;"));
            if style != Style::Ascii {
                assert!(output.ansi.contains("48;2;"));
            }
        }
        let background = RgbaImage::from_fn(96, 96, |x, y| {
            if (24..72).contains(&x) && (24..72).contains(&y) {
                image::Rgba([30, 55, 90, 255])
            } else {
                image::Rgba([255; 4])
            }
        });
        let image = decode(&encoded(background, ImageFormat::Png)).unwrap();
        for style in [Style::Ascii, Style::HalfBlocks, Style::Detail] {
            let mut opts = options(style);
            opts.edits.removal.enabled = true;
            let removed = image.convert(opts).unwrap();
            assert!(removed.removal.issue.is_none());
            let art = removed.artwork;
            let result = crate::greeting_official::render_config(serde_json::json!({"logo":{"type":"data-raw","source":art.ansi,"printRemaining":true,"padding":{"left":0,"right":0,"top":0}},"modules":[{"type":"custom","format":""}]})).unwrap();
            let output = Artwork::parse(result.trim_end()).unwrap();
            assert_eq!(output.plain.trim_end(), art.plain.trim_end());
            assert!(output.plain.lines().next().unwrap().trim().is_empty());
            assert!(
                !output.ansi.lines().next().unwrap().contains("48;2;"),
                "Empty cells must not paint a background"
            );
        }
    }

    #[test]
    fn quadrants_preserve_every_corner_and_transparency_mask() {
        let ink = [22, 53, 91];
        let paper = [249, 251, 255];
        for (mask, glyph) in QUADRANTS.iter().enumerate().skip(1) {
            let transparent = std::array::from_fn(|i| (mask & (1 << i) != 0).then_some(ink));
            let (character, fg, bg) = detail_cell(transparent);
            assert_eq!((character, fg, bg), (*glyph, Some(ink), None));
            let opaque =
                std::array::from_fn(|i| Some(if mask & (1 << i) != 0 { ink } else { paper }));
            let (character, fg, bg) = detail_cell(opaque);
            let encoded = QUADRANTS.iter().position(|c| *c == character).unwrap();
            for (i, color) in opaque.iter().enumerate() {
                assert_eq!(*color, if encoded & (1 << i) != 0 { fg } else { bg });
            }
        }
        assert_eq!(detail_cell([None; 4]), (' ', None, None));
    }

    #[test]
    fn ascii_resolution_is_not_capped_by_the_two_color_cell_budget() {
        for columns in [80, 100, 120, 128, 160] {
            assert_eq!(
                grid(
                    (1440, 1440),
                    Options {
                        columns,
                        ..options(Style::Ascii)
                    }
                )
                .unwrap(),
                (columns, columns / 2)
            );
        }
        for columns in 1..=160 {
            for dimensions in [(1, 8192), (8192, 1), (4000, 3000), (1440, 1440)] {
                let (width, height) = grid(
                    dimensions,
                    Options {
                        columns,
                        ..options(Style::Ascii)
                    },
                )
                .unwrap();
                assert!(width <= columns && height <= 96);
                assert!(width * height <= ASCII_MAX_CELLS);
            }
        }
        let source = decode(&encoded(fixture(), ImageFormat::Png)).unwrap();
        let art = source
            .convert(Options {
                columns: 160,
                ..options(Style::Ascii)
            })
            .unwrap();
        assert_eq!(art.columns, 160);
        assert!(art.rows > 48 && art.artwork.plain.is_ascii());
        art.artwork.validate().unwrap();
    }

    #[test]
    fn ascii_keeps_pastel_ink_and_thin_outlines_without_paper_noise() {
        let options = options(Style::Ascii);
        let white = Some([255; 3]);
        let ink = Some([50, 40, 35]);
        for color in [[195, 230, 204], [249, 161, 164]] {
            let (glyph, fg, bg) = ascii_cell([Some(color); 16], options);
            assert_ne!(glyph, ' ');
            assert_ne!(glyph, ',');
            assert!((luma(fg.unwrap()) - luma(options.background)).abs() >= 109.0);
            assert_eq!(bg, None);
        }
        assert_eq!(ascii_cell([white; 16], options).0, ' ');
        let vertical = std::array::from_fn(|i| if i % 4 == 1 { ink } else { white });
        let horizontal = std::array::from_fn(|i| if i / 4 == 1 { ink } else { white });
        let diagonal = std::array::from_fn(|i| if i % 4 == i / 4 { ink } else { white });
        assert_eq!(ascii_cell(vertical, options).0, '|');
        assert_eq!(ascii_cell(horizontal, options).0, '-');
        assert_eq!(ascii_cell(diagonal, options).0, '\\');
        let dark = Options {
            background: [16; 3],
            ..options
        };
        assert_eq!(ascii_cell([Some([0; 3]); 16], dark).0, ' ');
        assert!(luma(ascii_cell([Some([75, 110, 125]); 16], dark).1.unwrap()) >= 125.0);
    }
}
