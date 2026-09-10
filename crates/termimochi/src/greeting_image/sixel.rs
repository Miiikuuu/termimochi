//! Locally generated, bounded SIXEL. Never parses or forwards imported controls.
//! P2=1 leaves unpainted pixels transparent; zero-alpha pixels never enter a plane.
use super::{RgbaImage, straight_color};
use std::{
    collections::BTreeMap,
    fmt::Write,
    time::{Duration, Instant},
};

const MAX_SIDE: u32 = 2048;
const MAX_BYTES: usize = 8 * 1024 * 1024;

pub(super) fn encode(source: &RgbaImage, size: (u32, u32)) -> Result<Vec<u8>, String> {
    if size.0 == 0 || size.1 == 0 || size.0 > MAX_SIDE || size.1 > MAX_SIDE {
        return Err("Sixel is limited to 2048 × 2048 pixels. Reduce columns or font size.".into());
    }
    if source.width() == 0
        || source.height() == 0
        || source.width() > 1024
        || source.height() > 1024
    {
        return Err("Invalid Sixel source dimensions.".into());
    }
    let started = Instant::now();
    let deadline = || {
        if started.elapsed() > Duration::from_secs(12) {
            Err("Sixel export took too long. Reduce columns or font size.".to_string())
        } else {
            Ok(())
        }
    };
    // Resize associated alpha to avoid dark/white halos from invisible RGB.
    let mut associated = source.clone();
    for p in associated.pixels_mut() {
        for c in 0..3 {
            p[c] = ((u16::from(p[c]) * u16::from(p[3]) + 127) / 255) as u8;
        }
    }
    let mut pixels = image::imageops::resize(
        &associated,
        size.0,
        size.1,
        image::imageops::FilterType::Triangle,
    );
    for p in pixels.pixels_mut() {
        if p[3] < 128 {
            p.0 = [0; 4];
        } else {
            let rgb = straight_color(p).ok_or("Invalid Sixel alpha.")?;
            p.0 = [rgb[0], rgb[1], rgb[2], 255];
        }
    }
    // Exact palette for flat artwork. Complex images use the existing NeuQuant
    // crate with bounded training samples; transparent RGB cannot bias colors.
    let mut exact = BTreeMap::new();
    for p in pixels.pixels().filter(|p| p[3] != 0) {
        let next = exact.len();
        exact.entry([p[0], p[1], p[2]]).or_insert(next);
        if exact.len() > 256 {
            break;
        }
    }
    if exact.is_empty() {
        return Err("Sixel contains no visible pixels.".into());
    }
    let quantizer = if exact.len() > 256 {
        let step = pixels
            .pixels()
            .filter(|p| p[3] != 0)
            .count()
            .div_ceil(65536);
        let mut sample = Vec::with_capacity(65536 * 4);
        for p in pixels.pixels().filter(|p| p[3] != 0).step_by(step) {
            sample.extend_from_slice(&p.0);
        }
        Some(color_quant::NeuQuant::new(10, 256, &sample))
    } else {
        None
    };
    let palette: Vec<[u8; 3]> = if let Some(q) = &quantizer {
        q.color_map_rgb().as_chunks::<3>().0.to_vec()
    } else {
        let mut result = vec![[0; 3]; exact.len()];
        for (color, index) in &exact {
            result[*index] = *color;
        }
        result
    };
    let mut indexed = Vec::with_capacity(pixels.width() as usize * pixels.height() as usize);
    for row in pixels.rows() {
        deadline()?;
        for p in row {
            indexed.push(if p[3] == 0 {
                256u16
            } else if let Some(q) = &quantizer {
                q.index_of(&p.0) as u16
            } else {
                exact[&[p[0], p[1], p[2]]] as u16
            });
        }
    }
    let mut output = format!("\x1bP0;1;0q\"1;1;{};{}", size.0, size.1);
    for (i, rgb) in palette.iter().enumerate() {
        let percent = |c| (u32::from(c) * 100 + 127) / 255;
        write!(
            output,
            "#{i};2;{};{};{}",
            percent(rgb[0]),
            percent(rgb[1]),
            percent(rgb[2])
        )
        .unwrap();
    }
    let w = size.0 as usize;
    let mut planes = vec![0u8; palette.len() * w];
    for y in (0..size.1 as usize).step_by(6) {
        deadline()?;
        planes.fill(0);
        for dy in 0..6.min(size.1 as usize - y) {
            for x in 0..w {
                let index = indexed[(y + dy) * w + x] as usize;
                if index != 256 {
                    planes[index * w + x] |= 1 << dy;
                }
            }
        }
        let mut painted = false;
        for (i, plane) in planes.chunks_exact(w).enumerate() {
            let Some(last) = plane.iter().rposition(|v| *v != 0) else {
                continue;
            };
            if painted {
                output.push('$');
            }
            painted = true;
            write!(output, "#{i}").unwrap();
            let mut x = 0;
            while x <= last {
                let value = plane[x];
                let mut count = 1;
                while x + count <= last && plane[x + count] == value {
                    count += 1;
                }
                let c = char::from(63 + value);
                if count >= 4 {
                    write!(output, "!{count}{c}").unwrap();
                } else {
                    for _ in 0..count {
                        output.push(c);
                    }
                }
                x += count;
            }
            if output.len() > MAX_BYTES {
                return Err("Sixel exceeds 8 MiB. Reduce columns or image detail.".into());
            }
        }
        if y + 6 < size.1 as usize {
            output.push('-');
        }
    }
    if output.len() + 2 > MAX_BYTES {
        return Err("Sixel exceeds 8 MiB. Reduce columns or image detail.".into());
    }
    output.push_str("\x1b\\");
    Ok(output.into_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transparent_planes_keep_black_ink_and_six_row_boundaries() {
        let mut image = RgbaImage::from_pixel(8, 7, image::Rgba([255, 255, 255, 0]));
        image.put_pixel(0, 0, image::Rgba([0, 0, 0, 255]));
        image.put_pixel(7, 6, image::Rgba([200, 30, 70, 255]));
        let bytes = encode(&image, (8, 7)).unwrap();
        let text = std::str::from_utf8(&bytes).unwrap();
        assert!(text.starts_with("\x1bP0;1;0q\"1;1;8;7#0;2;0;0;0#1;2;78;12;27"));
        assert!(text.ends_with("#0@-#1!7?@\x1b\\"), "{text:?}");
        assert_eq!(image.get_pixel(1, 0).0, [255, 255, 255, 0]);
        assert!(encode(&image, (2049, 7)).is_err());
        assert!(encode(&image, (0, 7)).is_err());
        assert!(encode(&RgbaImage::new(8, 7), (8, 7)).is_err());
    }

    #[test]
    fn soft_alpha_threshold_is_explicit_and_never_becomes_a_black_matte() {
        let image = RgbaImage::from_fn(4, 1, |x, _| match x {
            0 => image::Rgba([255, 255, 255, 0]),
            1 => image::Rgba([200, 30, 70, 127]),
            2 => image::Rgba([200, 30, 70, 128]),
            _ => image::Rgba([0, 0, 0, 255]),
        });
        let stream = String::from_utf8(encode(&image, (4, 1)).unwrap()).unwrap();
        assert!(stream.ends_with("#0??@$#1???@\x1b\\"), "{stream:?}");
        assert_eq!(image.get_pixel(1, 0)[3], 127);
    }

    #[test]
    fn complex_palette_and_alpha_are_bounded_and_deterministic() {
        let image = RgbaImage::from_fn(128, 96, |x, y| {
            image::Rgba([
                x as u8,
                (y * 2) as u8,
                (x + y) as u8,
                if x < 8 { 0 } else { 255 },
            ])
        });
        let a = encode(&image, (256, 192)).unwrap();
        assert_eq!(a, encode(&image, (256, 192)).unwrap());
        let text = std::str::from_utf8(&a).unwrap();
        assert_eq!(text.matches(";2;").count(), 256);
        assert_eq!(text.bytes().filter(|b| *b == 27).count(), 2);
        assert!(a.len() < MAX_BYTES);
    }
}
