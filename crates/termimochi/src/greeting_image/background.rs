//! Bounded color-key background removal, not semantic foreground segmentation.
//! Operates on the private premultiplied snapshot; never writes a source image.
use super::Color;
use image::{Rgba, RgbaImage};

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Removal {
    pub enabled: bool,
    pub color: Option<Color>,
    pub tolerance: f64,
    pub softness: f64,
    pub connected: bool,
}

impl Default for Removal {
    fn default() -> Self {
        Self {
            enabled: false,
            color: None,
            tolerance: 8.0,
            softness: 4.0,
            connected: true,
        }
    }
}

impl Removal {
    pub fn validate(self) -> Result<(), String> {
        if !self.tolerance.is_finite()
            || !(0.0..=50.0).contains(&self.tolerance)
            || !self.softness.is_finite()
            || !(0.0..=25.0).contains(&self.softness)
        {
            return Err("Invalid background tolerance or softness.".into());
        }
        Ok(())
    }
}

#[derive(Default)]
pub(crate) struct RemovalReport {
    pub color: Option<Color>,
    pub changed_pixels: usize,
    pub issue: Option<&'static str>,
}

pub(crate) fn straight_color(pixel: &Rgba<u8>) -> Option<Color> {
    (pixel[3] > 1).then(|| {
        std::array::from_fn(|i| {
            ((u32::from(pixel[i]) * 255 + u32::from(pixel[3]) / 2) / u32::from(pixel[3])).min(255)
                as u8
        })
    })
}

fn border(width: u32, height: u32) -> Vec<(u32, u32)> {
    let mut pixels = Vec::new();
    for x in 0..width {
        pixels.push((x, 0));
        if height > 1 {
            pixels.push((x, height - 1));
        }
    }
    for y in 1..height.saturating_sub(1) {
        pixels.push((0, y));
        if width > 1 {
            pixels.push((width - 1, y));
        }
    }
    pixels
}

fn distance(a: Color, b: Color) -> f64 {
    (0..3)
        .map(|i| f64::from(a[i].abs_diff(b[i])))
        .fold(0.0, f64::max)
}

pub(super) fn detect(image: &RgbaImage) -> Option<Color> {
    if image.width() == 0 || image.height() == 0 {
        return None;
    }
    let edge = border(image.width(), image.height());
    let mut colors: Vec<_> = edge
        .iter()
        .filter_map(|&(x, y)| {
            let p = image.get_pixel(x, y);
            (p[3] >= 250).then(|| straight_color(p).unwrap())
        })
        .collect();
    if colors.len() * 10 < edge.len() * 9 || colors.is_empty() {
        return None;
    }
    let median = std::array::from_fn(|i| {
        colors.sort_unstable_by_key(|c| c[i]);
        colors[colors.len() / 2][i]
    });
    (colors
        .iter()
        .filter(|&&c| distance(c, median) <= 18.0)
        .count()
        * 10
        >= edge.len() * 9)
        .then_some(median)
}

pub(super) fn remove(
    image: &mut RgbaImage,
    settings: Removal,
    automatic: Option<Color>,
) -> RemovalReport {
    if !settings.enabled {
        return RemovalReport::default();
    }
    let Some(color) = settings.color.or(automatic) else {
        return RemovalReport {
            issue: Some("No uniform background detected. Pick a color from Original."),
            ..Default::default()
        };
    };
    let (w, h) = image.dimensions();
    let threshold = settings.tolerance * 2.55;
    let band = settings.softness * 2.55;
    let eligible = |x, y| {
        straight_color(image.get_pixel(x, y)).is_none_or(|c| distance(c, color) <= threshold + band)
    };
    let mut selected = vec![!settings.connected; (w * h) as usize];
    if settings.connected && w > 0 && h > 0 {
        let mut queue = Vec::new();
        for (x, y) in border(w, h) {
            let index = (y * w + x) as usize;
            if !selected[index] && eligible(x, y) {
                selected[index] = true;
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
                if !selected[index] && eligible(nx, ny) {
                    selected[index] = true;
                    queue.push((nx, ny));
                }
            }
        }
    }
    let mut changed = 0;
    for (pixel, selected) in image.pixels_mut().zip(selected) {
        if !selected {
            continue;
        }
        let Some(straight) = straight_color(pixel) else {
            continue;
        };
        let d = distance(straight, color);
        let keep = if d <= threshold {
            0.0
        } else if band > 0.0 {
            ((d - threshold) / band).clamp(0.0, 1.0)
        } else {
            1.0
        };
        let keep = keep * keep * (3.0 - 2.0 * keep);
        if keep >= 1.0 {
            continue;
        }
        let alpha = f64::from(pixel[3]);
        let new_alpha = (alpha * keep).round() as u8;
        // Subtract the keyed matte contribution in the transition band, then
        // keep RGB premultiplied. This limits white/color fringes on dark themes.
        for i in 0..3 {
            pixel[i] = ((f64::from(straight[i]) - f64::from(color[i]) * (1.0 - keep)) * alpha
                / 255.0)
                .round()
                .clamp(0.0, f64::from(new_alpha)) as u8;
        }
        if new_alpha < pixel[3] {
            changed += 1;
        }
        pixel[3] = new_alpha;
    }
    RemovalReport {
        color: Some(color),
        changed_pixels: changed,
        issue: (changed == 0)
            .then_some("No background pixels matched. Adjust the color or tolerance."),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> RgbaImage {
        RgbaImage::from_fn(15, 15, |x, y| {
            if (4..=10).contains(&x)
                && (4..=10).contains(&y)
                && (x == 4 || x == 10 || y == 4 || y == 10)
            {
                Rgba([30, 55, 90, 255])
            } else {
                Rgba([255; 4])
            }
        })
    }
    fn enabled() -> Removal {
        Removal {
            enabled: true,
            ..Default::default()
        }
    }

    #[test]
    fn connected_removal_preserves_enclosed_same_color_details() {
        let source = fixture();
        let mut image = source.clone();
        let report = remove(&mut image, enabled(), detect(&source));
        assert_eq!(report.color, Some([255; 3]));
        assert!(report.issue.is_none());
        assert_eq!(image.get_pixel(0, 0), &Rgba([0; 4]));
        assert_eq!(image.get_pixel(7, 7), source.get_pixel(7, 7));
        assert_eq!(image.get_pixel(4, 4), source.get_pixel(4, 4));
        assert_eq!(report.changed_pixels, 15 * 15 - 7 * 7);
        let mut all = source.clone();
        remove(
            &mut all,
            Removal {
                connected: false,
                ..enabled()
            },
            detect(&source),
        );
        assert_eq!(all.get_pixel(7, 7), &Rgba([0; 4]));
        assert_eq!(source, fixture());
    }

    #[test]
    fn automatic_detection_is_conservative_and_off_is_exact() {
        let source = fixture();
        let mut image = source.clone();
        let report = remove(&mut image, Removal::default(), Some([255; 3]));
        assert_eq!(image, source);
        assert!(report.issue.is_none());
        let complex = RgbaImage::from_fn(16, 16, |x, y| {
            if (x + y) % 2 == 0 {
                Rgba([255; 4])
            } else {
                Rgba([0, 0, 0, 255])
            }
        });
        assert_eq!(detect(&complex), None);
        let mut copy = complex.clone();
        assert!(remove(&mut copy, enabled(), None).issue.is_some());
        assert_eq!(copy, complex);
        let transparent = RgbaImage::new(10, 10);
        assert_eq!(detect(&transparent), None);
        for (w, h) in [(1, 1), (1, 15), (15, 1)] {
            let image = RgbaImage::from_pixel(w, h, Rgba([123, 145, 67, 255]));
            assert_eq!(detect(&image), Some([123, 145, 67]));
        }
    }

    #[test]
    fn custom_key_softness_and_alpha_remain_bounded() {
        let mut hard = RgbaImage::from_raw(
            3,
            1,
            vec![255, 255, 255, 255, 230, 230, 230, 255, 30, 55, 90, 255],
        )
        .unwrap();
        let mut soft = hard.clone();
        let settings = Removal {
            color: Some([255; 3]),
            tolerance: 0.0,
            softness: 20.0,
            connected: false,
            ..enabled()
        };
        remove(&mut soft, settings, None);
        assert_eq!(soft.get_pixel(0, 0), &Rgba([0; 4]));
        assert!((1..255).contains(&soft.get_pixel(1, 0)[3]));
        assert_eq!(soft.get_pixel(2, 0), hard.get_pixel(2, 0));
        remove(
            &mut hard,
            Removal {
                softness: 0.0,
                ..settings
            },
            None,
        );
        assert_eq!(hard.get_pixel(1, 0)[3], 255);
        for tolerance in [0.0, 8.0, 50.0] {
            for softness in [0.0, 4.0, 25.0] {
                let mut image = RgbaImage::from_raw(
                    4,
                    1,
                    vec![
                        128, 128, 128, 128, 110, 100, 90, 128, 0, 0, 0, 0, 4, 8, 12, 16,
                    ],
                )
                .unwrap();
                let old = image.clone();
                remove(
                    &mut image,
                    Removal {
                        tolerance,
                        softness,
                        ..settings
                    },
                    None,
                );
                for (pixel, before) in image.pixels().zip(old.pixels()) {
                    assert!(pixel[3] <= before[3]);
                    assert!((0..3).all(|i| pixel[i] <= pixel[3]));
                }
            }
        }
        assert_eq!(
            straight_color(&Rgba([64, 32, 16, 128])),
            Some([128, 64, 32])
        );
        assert_eq!(straight_color(&Rgba([0; 4])), None);
    }

    #[test]
    fn invalid_controls_are_rejected() {
        for value in [f64::NAN, f64::INFINITY, -1.0, 100.0] {
            assert!(
                Removal {
                    tolerance: value,
                    ..Default::default()
                }
                .validate()
                .is_err()
            );
            assert!(
                Removal {
                    softness: value,
                    ..Default::default()
                }
                .validate()
                .is_err()
            );
        }
    }
}
