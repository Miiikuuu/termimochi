//! Independent image preparation and connected-edge extraction. See
//! docs/image-conversion.md for design references; no upstream code is embedded.
use super::{Color, Options, Removal, composite, luma};
use image::RgbaImage;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Ink {
    #[default]
    Color,
    Gray,
    Theme,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Structure {
    #[default]
    Balanced,
    Outline,
    Tone,
}

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Adjustments {
    pub exposure: f64,
    pub contrast: f64,
    pub saturation: f64,
    pub smoothing: f64,
    pub edges: f64,
    pub trim: bool,
    // Percent removed from left, top, right, bottom, before auto-trimming.
    pub crop: [u8; 4],
    pub ink: Ink,
    pub structure: Structure,
    pub removal: Removal,
}

impl Default for Adjustments {
    fn default() -> Self {
        Self {
            exposure: 0.0,
            contrast: 1.0,
            saturation: 1.0,
            smoothing: 0.0,
            edges: 0.7,
            trim: false,
            crop: [0; 4],
            ink: Ink::Color,
            structure: Structure::Balanced,
            removal: Removal::default(),
        }
    }
}

impl Adjustments {
    pub fn validate(self) -> Result<(), String> {
        self.removal.validate()?;
        if [
            (self.exposure, -2.0, 2.0),
            (self.contrast, 0.5, 2.0),
            (self.saturation, 0.0, 2.0),
            (self.smoothing, 0.0, 1.0),
            (self.edges, 0.0, 2.0),
        ]
        .into_iter()
        .any(|(value, low, high)| !value.is_finite() || !(low..=high).contains(&value))
            || self.crop.iter().any(|value| *value > 45)
        {
            return Err("Invalid image adjustments. Reset the controls and try again.".into());
        }
        Ok(())
    }

    pub fn preset(index: u32) -> Self {
        match index {
            1 => Self {
                trim: true,
                contrast: 1.15,
                saturation: 1.1,
                smoothing: 0.35,
                edges: 1.2,
                ..Self::default()
            },
            2 => Self {
                trim: true,
                smoothing: 0.25,
                edges: 1.4,
                structure: Structure::Outline,
                ink: Ink::Theme,
                ..Self::default()
            },
            3 => Self {
                smoothing: 0.15,
                edges: 0.25,
                structure: Structure::Tone,
                ..Self::default()
            },
            _ => Self::default(),
        }
    }
}

// Only a near-uniform image border can serve as an inferred paper color.
// Alpha and edge-connected flood fill avoid treating enclosed white details
// as margin. This trims framing, not a semantic subject/background removal.
pub(super) fn crop_image(image: &RgbaImage, edits: Adjustments) -> RgbaImage {
    let x = image.width() * u32::from(edits.crop[0]) / 100;
    let y = image.height() * u32::from(edits.crop[1]) / 100;
    let w = (image.width() - x - image.width() * u32::from(edits.crop[2]) / 100).max(1);
    let h = (image.height() - y - image.height() * u32::from(edits.crop[3]) / 100).max(1);
    let cropped = image::imageops::crop_imm(image, x, y, w, h).to_image();
    if !edits.trim {
        return cropped;
    }
    let mut border = Vec::new();
    for x in 0..w {
        border.push((x, 0));
        if h > 1 {
            border.push((x, h - 1));
        }
    }
    for y in 1..h.saturating_sub(1) {
        border.push((0, y));
        if w > 1 {
            border.push((w - 1, y));
        }
    }
    let mut colors: Vec<_> = border
        .iter()
        .filter_map(|&(x, y)| {
            let p = cropped.get_pixel(x, y);
            (p[3] >= 250).then_some([p[0], p[1], p[2]])
        })
        .collect();
    let paper = if colors.len() * 10 >= border.len() * 9 {
        let median: Color = std::array::from_fn(|channel| {
            colors.sort_unstable_by_key(|p| p[channel]);
            colors[colors.len() / 2][channel]
        });
        let matching = colors
            .iter()
            .filter(|p| (0..3).all(|i| p[i].abs_diff(median[i]) <= 18))
            .count();
        (matching * 10 >= border.len() * 9).then_some(median)
    } else {
        None
    };
    let empty = |x, y| {
        let p = cropped.get_pixel(x, y);
        p[3] <= 8
            || paper
                .is_some_and(|color| p[3] >= 250 && (0..3).all(|i| p[i].abs_diff(color[i]) <= 18))
    };
    let mut visited = vec![false; (w * h) as usize];
    let mut queue = Vec::new();
    for (x, y) in border {
        let index = (y * w + x) as usize;
        if !visited[index] && empty(x, y) {
            visited[index] = true;
            queue.push((x, y));
        }
    }
    while let Some((x, y)) = queue.pop() {
        for (nx, ny) in [
            (x.saturating_sub(1), y),
            ((x + 1).min(w - 1), y),
            (x, y.saturating_sub(1)),
            (x, (y + 1).min(h - 1)),
        ] {
            let index = (ny * w + nx) as usize;
            if !visited[index] && empty(nx, ny) {
                visited[index] = true;
                queue.push((nx, ny));
            }
        }
    }
    let (mut left, mut top, mut right, mut bottom) = (w, h, 0, 0);
    for y in 0..h {
        for x in 0..w {
            if !visited[(y * w + x) as usize] {
                left = left.min(x);
                top = top.min(y);
                right = right.max(x);
                bottom = bottom.max(y);
            }
        }
    }
    if left > right || top > bottom {
        return cropped;
    }
    let pad = ((right - left + 1).max(bottom - top + 1) / 50).max(1);
    left = left.saturating_sub(pad);
    top = top.saturating_sub(pad);
    right = (right + pad).min(w - 1);
    bottom = (bottom + pad).min(h - 1);
    image::imageops::crop_imm(&cropped, left, top, right - left + 1, bottom - top + 1).to_image()
}

pub(super) fn adjust(image: &mut RgbaImage, edits: Adjustments) {
    if edits.exposure == 0.0 && edits.contrast == 1.0 && edits.saturation == 1.0 {
        return;
    }
    for pixel in image.pixels_mut() {
        let alpha = f64::from(pixel[3]);
        if alpha == 0.0 {
            continue;
        }
        let color: [f64; 3] = std::array::from_fn(|i| {
            let value = (f64::from(pixel[i]) / alpha).clamp(0.0, 1.0);
            ((value.powf(2.2) * 2.0_f64.powf(edits.exposure)).powf(1.0 / 2.2) - 0.5)
                * edits.contrast
                + 0.5
        });
        let gray = color[0] * 0.2126 + color[1] * 0.7152 + color[2] * 0.0722;
        for i in 0..3 {
            pixel[i] = ((gray + (color[i] - gray) * edits.saturation).clamp(0.0, 1.0) * alpha)
                .round() as u8;
        }
    }
}

pub(super) fn ink_color(color: Color, options: Options) -> Color {
    match options.edits.ink {
        Ink::Color => color,
        Ink::Gray => [luma(color).round() as u8; 3],
        Ink::Theme => options.foreground,
    }
}

#[derive(Clone, Copy, Default)]
pub(super) struct Edge {
    pub strength: f64,
    pub direction: usize,
}

// Sobel -> non-maximum suppression -> percentile threshold -> connected weak
// edges. Both thresholds are bounded, and hysteresis is iterative, not recursive.
pub(super) fn edges(image: &RgbaImage, options: Options) -> Vec<Edge> {
    let (w, h) = (image.width() as usize, image.height() as usize);
    let mut output = vec![Edge::default(); w * h];
    if w < 3 || h < 3 || options.edits.edges == 0.0 {
        return output;
    }
    let values: Vec<_> = image
        .pixels()
        .map(|p| luma(composite(p, options.background).unwrap_or(options.background)))
        .collect();
    let mut gradients = vec![Edge::default(); w * h];
    for y in 1..h - 1 {
        for x in 1..w - 1 {
            let p = |dx: isize, dy: isize| {
                values[((y as isize + dy) as usize) * w + (x as isize + dx) as usize]
            };
            let gx = p(1, -1) + 2.0 * p(1, 0) + p(1, 1) - p(-1, -1) - 2.0 * p(-1, 0) - p(-1, 1);
            let gy = p(-1, 1) + 2.0 * p(0, 1) + p(1, 1) - p(-1, -1) - 2.0 * p(0, -1) - p(1, -1);
            let angle = gy.atan2(gx).to_degrees().rem_euclid(180.0);
            let direction = if !(22.5..157.5).contains(&angle) {
                0
            } else if angle < 67.5 {
                1
            } else if angle < 112.5 {
                2
            } else {
                3
            };
            gradients[y * w + x] = Edge {
                strength: gx.hypot(gy),
                direction,
            };
        }
    }
    let mut magnitudes = Vec::new();
    for y in 1..h - 1 {
        for x in 1..w - 1 {
            let i = y * w + x;
            let e = gradients[i];
            let (a, b) = match e.direction {
                0 => (i - 1, i + 1),
                1 => (i - w - 1, i + w + 1),
                2 => (i - w, i + w),
                _ => (i - w + 1, i + w - 1),
            };
            if e.strength >= gradients[a].strength
                && e.strength >= gradients[b].strength
                && e.strength > 4.0
            {
                output[i] = e;
                magnitudes.push(e.strength);
            }
        }
    }
    if magnitudes.is_empty() {
        return output;
    }
    magnitudes.sort_unstable_by(f64::total_cmp);
    let high = (magnitudes[magnitudes.len() * 3 / 4] * 0.65 / options.edits.edges.max(0.1))
        .clamp(12.0, 350.0);
    let low = high * 0.4;
    let mut keep = vec![false; w * h];
    let mut queue = Vec::new();
    for (i, e) in output.iter().enumerate() {
        if e.strength >= high {
            keep[i] = true;
            queue.push(i);
        }
    }
    while let Some(i) = queue.pop() {
        let (x, y) = (i % w, i / w);
        for ny in y.saturating_sub(1)..=(y + 1).min(h - 1) {
            for nx in x.saturating_sub(1)..=(x + 1).min(w - 1) {
                let j = ny * w + nx;
                if !keep[j] && output[j].strength >= low {
                    keep[j] = true;
                    queue.push(j);
                }
            }
        }
    }
    for (e, keep) in output.iter_mut().zip(keep) {
        if keep {
            e.strength = (e.strength / high).min(2.0);
        } else {
            e.strength = 0.0;
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::greeting_image::{Style, decode};
    use image::{DynamicImage, ImageFormat, Rgba};
    use std::io::Cursor;

    fn options() -> Options {
        Options {
            columns: 48,
            style: Style::Ascii,
            cell_ratio: 0.5,
            background: [255; 3],
            foreground: [24, 35, 48],
            invert: false,
            edits: Adjustments::default(),
        }
    }

    #[test]
    fn crop_trims_only_outer_margin_and_preserves_source_and_holes() {
        let mut source = RgbaImage::from_pixel(100, 100, Rgba([255; 4]));
        for y in 30..70 {
            for x in 20..80 {
                source.put_pixel(x, y, Rgba([20, 50, 90, 255]));
            }
        }
        source.put_pixel(50, 50, Rgba([255; 4]));
        let before = source.clone();
        let edits = Adjustments {
            trim: true,
            ..Default::default()
        };
        let cropped = crop_image(&source, edits);
        assert_eq!(cropped.dimensions(), (62, 42));
        assert_eq!(cropped.get_pixel(31, 21), &Rgba([255; 4]));
        assert_eq!(source, before);
        let manual = crop_image(
            &source,
            Adjustments {
                crop: [10, 20, 30, 40],
                ..Default::default()
            },
        );
        assert_eq!(manual.dimensions(), (60, 40));
        let transparent = RgbaImage::from_fn(100, 100, |x, y| {
            if (20..80).contains(&x) && (30..70).contains(&y) {
                Rgba([50, 30, 20, 255])
            } else {
                Rgba([0; 4])
            }
        });
        assert_eq!(crop_image(&transparent, edits).dimensions(), (62, 42));
        let complex = RgbaImage::from_fn(100, 100, |x, y| {
            if (x + y) % 2 == 0 {
                Rgba([255; 4])
            } else {
                Rgba([0, 0, 0, 255])
            }
        });
        assert_eq!(crop_image(&complex, edits), complex);
        for size in [(1, 1), (1, 8), (8, 1)] {
            let tiny = RgbaImage::from_pixel(size.0, size.1, Rgba([255; 4]));
            assert_eq!(crop_image(&tiny, edits), tiny);
        }
    }

    #[test]
    fn adjustments_are_bounded_neutral_and_alpha_safe() {
        let source = RgbaImage::from_raw(2, 1, vec![80, 40, 20, 128, 0, 0, 0, 0]).unwrap();
        let mut neutral = source.clone();
        adjust(&mut neutral, Adjustments::default());
        assert_eq!(neutral, source);
        for exposure in [-2.0, 0.0, 2.0] {
            for contrast in [0.5, 2.0] {
                for saturation in [0.0, 2.0] {
                    let edits = Adjustments {
                        exposure,
                        contrast,
                        saturation,
                        ..Default::default()
                    };
                    edits.validate().unwrap();
                    let mut adjusted = source.clone();
                    adjust(&mut adjusted, edits);
                    for p in adjusted.pixels() {
                        assert!((0..3).all(|i| p[i] <= p[3]));
                    }
                    assert_eq!(adjusted.get_pixel(1, 0), &Rgba([0; 4]));
                }
            }
        }
        let mut bright = source.clone();
        adjust(
            &mut bright,
            Adjustments {
                exposure: 1.0,
                ..Default::default()
            },
        );
        assert!(bright.get_pixel(0, 0)[0] > source.get_pixel(0, 0)[0]);
        for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -10.0, 10.0] {
            for edits in [
                Adjustments {
                    exposure: value,
                    ..Default::default()
                },
                Adjustments {
                    contrast: value,
                    ..Default::default()
                },
                Adjustments {
                    saturation: value,
                    ..Default::default()
                },
                Adjustments {
                    smoothing: value,
                    ..Default::default()
                },
                Adjustments {
                    edges: value,
                    ..Default::default()
                },
            ] {
                assert!(edits.validate().is_err());
            }
        }
        assert!(
            Adjustments {
                crop: [46, 0, 0, 0],
                ..Default::default()
            }
            .validate()
            .is_err()
        );
    }

    #[test]
    fn connected_edges_detect_orientation_and_ignore_flat_images() {
        for direction in [0, 2] {
            let source = RgbaImage::from_fn(32, 32, |x, y| {
                if (if direction == 0 { x } else { y }) < 16 {
                    Rgba([0, 0, 0, 255])
                } else {
                    Rgba([255; 4])
                }
            });
            let result = edges(&source, options());
            let retained: Vec<_> = result.iter().filter(|e| e.strength > 0.0).collect();
            assert!(retained.len() >= 28);
            assert!(retained.iter().all(|e| e.direction == direction));
            assert!(
                edges(
                    &source,
                    Options {
                        edits: Adjustments {
                            edges: 0.0,
                            ..Default::default()
                        },
                        ..options()
                    }
                )
                .iter()
                .all(|e| e.strength == 0.0)
            );
        }
        assert!(
            edges(
                &RgbaImage::from_pixel(32, 32, Rgba([120, 120, 120, 255])),
                options()
            )
            .iter()
            .all(|e| e.strength == 0.0)
        );
    }

    #[test]
    fn recipes_produce_distinct_portable_outputs_without_mutating_the_reference() {
        let source = RgbaImage::from_fn(96, 96, |x, y| {
            if x > 20 && x < 75 && y > 15 && y < 80 {
                Rgba([50 + (x % 20) as u8, 90, 145, 255])
            } else {
                Rgba([255; 4])
            }
        });
        let mut bytes = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(source)
            .write_to(&mut bytes, ImageFormat::Png)
            .unwrap();
        let decoded = decode(&bytes.into_inner()).unwrap();
        let mut outputs = Vec::new();
        for index in 0..4 {
            let art = decoded
                .convert(Options {
                    edits: Adjustments::preset(index),
                    ..options()
                })
                .unwrap();
            art.artwork.validate().unwrap();
            assert!(art.artwork.plain.is_ascii());
            assert!(art.artwork.plain.lines().all(|line| line.len() <= 48));
            assert!(art.reference.pixels().any(|pixel| pixel == &Rgba([255; 4])));
            outputs.push(art.artwork);
        }
        for i in 1..outputs.len() {
            assert_ne!(outputs[i], outputs[i - 1]);
        }
        assert!(outputs[2].plain.chars().all(|c| " |/-\\+\n".contains(c)));
        assert!(outputs[2].ansi.contains("38;2;24;35;48m"));
        let neutral = decoded.convert(options()).unwrap().reference;
        let bright = decoded
            .convert(Options {
                edits: Adjustments {
                    exposure: 1.0,
                    ..Default::default()
                },
                ..options()
            })
            .unwrap()
            .reference;
        assert_eq!(neutral, bright);
    }
}
