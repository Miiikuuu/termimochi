use super::*;
use crate::greeting_image::source::{EditableArtwork, SourceImage};

pub(crate) fn fixture() -> EditableArtwork {
    let frames = (0..3).map(|i| {
        let mut image = RgbaImage::from_pixel(32, 24, image::Rgba([255; 4]));
        for y in 6..18 {
            for x in (5 + i * 7)..(10 + i * 7) {
                image.put_pixel(x, y, image::Rgba([200, 30, 70, 255]));
            }
        }
        image::Frame::from_parts(
            image,
            0,
            0,
            image::Delay::from_numer_denom_ms(80 + i * 20, 1),
        )
    });
    let mut bytes = Vec::new();
    {
        let mut encoder = GifEncoder::new(&mut bytes);
        encoder.set_repeat(Repeat::Finite(2)).unwrap();
        encoder.encode_frames(frames).unwrap();
    }
    EditableArtwork::new(
        SourceImage::new(bytes).unwrap(),
        Options {
            columns: 32,
            style: Style::Detail,
            cell_ratio: 0.5,
            foreground: [0; 3],
            background: [255; 3],
            invert: false,
            edits: Adjustments {
                trim: true,
                removal: Removal {
                    enabled: true,
                    ..Default::default()
                },
                ..Default::default()
            },
        },
    )
    .unwrap()
}

pub(crate) fn fixture_bytes() -> Vec<u8> {
    fixture().image.bytes().to_vec()
}

// A flat-color GIF fits in one Kitty packet per frame and cannot exercise
// continuation commands. Grayscale noise remains lossless in the GIF palette.
pub(crate) fn chunked_fixture() -> EditableArtwork {
    let mut seed = 17u32;
    let mut background = RgbaImage::new(128, 128);
    for pixel in background.pixels_mut() {
        seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
        let gray = ((seed >> 16) % 240) as u8;
        *pixel = image::Rgba([gray, gray, gray, 255]);
    }
    let frames = (0..3).map(|i| {
        let mut pixels = background.clone();
        for y in 40..88 {
            for x in (10 + i * 38)..(30 + i * 38) {
                pixels.put_pixel(x, y, image::Rgba([200, 30, 70, 255]));
            }
        }
        image::Frame::from_parts(pixels, 0, 0, image::Delay::from_numer_denom_ms(170, 1))
    });
    let mut bytes = Vec::new();
    {
        let mut encoder = GifEncoder::new(&mut bytes);
        encoder.set_repeat(Repeat::Infinite).unwrap();
        encoder.encode_frames(frames).unwrap();
    }
    let mut options = fixture().options().unwrap();
    options.edits = Default::default();
    EditableArtwork::new(SourceImage::new(bytes).unwrap(), options).unwrap()
}

#[test]
fn gif_edits_keep_a_fixed_canvas_timing_transparency_and_source() {
    let source = fixture();
    let before = source.clone();
    let prepared = prepare(&source).unwrap();
    assert_eq!(source, before);
    assert_eq!(prepared.frames.len(), 3);
    assert_eq!(
        prepared.frame_after(0, Duration::from_millis(85)),
        (1, Duration::from_millis(5))
    );
    assert_eq!(
        prepared.frame_after(0, Duration::from_millis(319)),
        (0, Duration::from_millis(19))
    );
    assert_eq!(
        prepared.frame_after(2, Duration::from_millis(600_121)),
        (0, Duration::from_millis(1))
    );
    let dimensions = prepared.frames[0].pixels.dimensions();
    assert!(dimensions.0 < 32 && dimensions.1 < 24);
    let mut positions = Vec::new();
    for (i, frame) in prepared.frames.iter().enumerate() {
        assert_eq!(frame.pixels.dimensions(), dimensions);
        assert_eq!(frame.delay_ms, 80 + i as u32 * 20);
        assert_eq!(frame.pixels.get_pixel(0, 0)[3], 0);
        let visible: Vec<_> = frame
            .pixels
            .enumerate_pixels()
            .filter(|(_, _, p)| p[3] > 0)
            .collect();
        assert_eq!(visible.len(), 60, "no residue from the previous frame");
        assert!(visible.iter().all(|(_, _, p)| p.0 == [200, 30, 70, 255]));
        positions.push(visible.iter().map(|(x, _, _)| *x).min().unwrap());
    }
    assert_eq!(positions, vec![1, 8, 15]);
    let decoded = decode(&prepared.gif).unwrap();
    assert!(matches!(decoded.repeat, Repeat::Finite(2)));
    for (preview, exported) in prepared.frames.iter().zip(decoded.frames) {
        assert_eq!(preview.pixels, exported.pixels);
        assert_eq!(preview.delay_ms, exported.delay_ms);
    }
    let still = source.image.decode().unwrap();
    assert_eq!(still.format, "GIF still");
    assert!(
        !still
            .convert(source.options().unwrap())
            .unwrap()
            .artwork
            .plain
            .trim()
            .is_empty()
    );
    let encoded = serde_json::to_vec(&source).unwrap();
    let restored: EditableArtwork = serde_json::from_slice(&encoded).unwrap();
    assert_eq!(prepare(&restored).unwrap().gif, prepared.gif);
}

#[test]
fn gif_offsets_keep_previous_and_background_disposal_are_composited() {
    let mut bytes = Vec::new();
    {
        let mut encoder = gif::Encoder::new(
            &mut bytes,
            8,
            8,
            &[0, 0, 0, 255, 0, 0, 0, 255, 0, 0, 0, 255],
        )
        .unwrap();
        for (left, top, color, dispose) in [
            (1, 1, 1, gif::DisposalMethod::Keep),
            (4, 1, 2, gif::DisposalMethod::Previous),
            (1, 4, 3, gif::DisposalMethod::Background),
        ] {
            let frame = gif::Frame {
                left,
                top,
                width: 2,
                height: 2,
                dispose,
                transparent: Some(0),
                delay: 10,
                buffer: std::borrow::Cow::Owned(vec![color; 4]),
                ..Default::default()
            };
            encoder.write_frame(&frame).unwrap();
        }
    }
    let animation = decode(&bytes).unwrap();
    assert_eq!(animation.frames.len(), 3);
    assert_eq!(
        animation.frames[0].pixels.get_pixel(1, 1).0,
        [255, 0, 0, 255]
    );
    assert_eq!(
        animation.frames[1].pixels.get_pixel(4, 1).0,
        [0, 255, 0, 255]
    );
    assert_eq!(animation.frames[2].pixels.get_pixel(4, 1)[3], 0);
    assert_eq!(
        animation.frames[2].pixels.get_pixel(1, 1).0,
        [255, 0, 0, 255]
    );
    assert_eq!(
        animation.frames[2].pixels.get_pixel(1, 4).0,
        [0, 0, 255, 255]
    );
}

fn repeated_frames(width: u16, height: u16, count: usize, delay: u16) -> Vec<u8> {
    let mut bytes = Vec::new();
    {
        let mut encoder =
            gif::Encoder::new(&mut bytes, width, height, &[255, 0, 0, 0, 255, 0]).unwrap();
        let frame = gif::Frame {
            width: 1,
            height: 1,
            delay,
            buffer: std::borrow::Cow::Borrowed(&[0]),
            ..Default::default()
        };
        for _ in 0..count {
            encoder.write_frame(&frame).unwrap();
        }
    }
    bytes
}

#[test]
fn gif_bad_files_and_budgets_fail_before_unbounded_allocation() {
    for bytes in [
        b"GIF89a".to_vec(),
        repeated_frames(1025, 1, 1, 1),
        repeated_frames(2, 2, 121, 1),
        repeated_frames(1024, 1024, 17, 1),
        repeated_frames(2, 2, 1, 6500),
    ] {
        assert!(decode(&bytes).is_err());
    }
    let bytes = repeated_frames(2, 2, 1, 0);
    assert_eq!(decode(&bytes).unwrap().frames[0].delay_ms, 20);
    assert!(decode(&bytes[..bytes.len() - 1]).is_err());
    let mut trailing = bytes.clone();
    trailing.extend_from_slice(b"TRAILING");
    assert!(decode(&trailing).is_err());
    assert!(decode(&vec![0; FILE_LIMIT as usize + 1]).is_err());
    for index in 0..bytes.len() {
        let mut damaged = bytes.clone();
        damaged[index] ^= 0xff;
        // Some bit changes yield valid GIFs; either result must remain bounded.
        let _ = decode(&damaged);
    }
}

#[test]
fn gif_bundle_has_animation_and_safe_ansi_fallback_without_activation() {
    let source = fixture();
    let mut settings = crate::greeting::GreetingSettings::starter();
    settings
        .import_artwork(
            source
                .image
                .decode()
                .unwrap()
                .convert(source.options().unwrap())
                .unwrap()
                .artwork,
        )
        .unwrap();
    settings.enabled = true;
    settings.editable_artwork = Some(source.clone());
    let image = pixel_export::prepare(&source).unwrap();
    let root = tempfile::tempdir().unwrap();
    let folder = pixel_export::export_bundle(
        root.path(),
        &settings,
        &image,
        pixel_export::Protocol::KittyAnimation,
        32,
        [10, 20],
    )
    .unwrap();
    let value = crate::fastfetch_document::value(
        &std::fs::read_to_string(folder.join("config.jsonc")).unwrap(),
    )
    .unwrap();
    assert_eq!(value["logo"]["type"], "raw");
    assert_eq!(value["logo"]["source"], "logo.kitty");
    assert_eq!(
        decode(&std::fs::read(folder.join("logo.gif")).unwrap())
            .unwrap()
            .frames
            .len(),
        3
    );
    assert_eq!(
        std::fs::read_to_string(folder.join("config-ansi.jsonc")).unwrap(),
        settings.fastfetch_config().unwrap()
    );
    assert_eq!(std::fs::read_dir(&folder).unwrap().count(), 6);
    let stream = std::fs::read_to_string(folder.join("logo.kitty")).unwrap();
    assert_eq!(stream.matches("a=f").count(), 2);
    assert_eq!(stream.matches("X=1").count(), 2);
    assert!(
        !stream.contains("t=f"),
        "Animation stream must not read external file paths"
    );
    for protocol in [pixel_export::Protocol::Kitty, pixel_export::Protocol::Sixel] {
        let still =
            pixel_export::export_bundle(root.path(), &settings, &image, protocol, 32, [10, 20])
                .unwrap();
        assert!(!still.join("logo.gif").exists());
        assert!(still.join("logo.png").exists());
    }
}

#[test]
#[ignore = "requires Kitty, Fastfetch, Python Pillow and isolated Xvfb; renders real animation and captures changing frames"]
fn gif_bundle_native_kitty_animation() {
    let source = fixture();
    let image = pixel_export::prepare(&source).unwrap();
    let settings = crate::greeting::GreetingSettings {
        enabled: true,
        imported_source: Some(
            r#"{"modules":[{"type":"custom","format":"PIXEL_EXPORT_OK"}]}"#.into(),
        ),
        ..Default::default()
    };
    let root = tempfile::tempdir().unwrap();
    let folder = pixel_export::export_bundle(
        root.path(),
        &settings,
        &image,
        pixel_export::Protocol::KittyAnimation,
        32,
        [10, 20],
    )
    .unwrap();
    let output = std::process::Command::new("python3")
        .arg(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../scripts/test-kitty-animation.py"
        ))
        .arg(&folder)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    println!("{}", String::from_utf8_lossy(&output.stdout));
}

#[test]
fn gif_kitty_chunked_payload_roundtrips_exact_pixels() {
    let mut seed = 17u32;
    let mut pixels = RgbaImage::new(128, 128);
    for pixel in pixels.pixels_mut() {
        seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
        *pixel = image::Rgba([seed as u8, (seed >> 8) as u8, (seed >> 16) as u8, 255]);
    }
    let prepared = Prepared {
        frames: vec![
            Frame {
                pixels: pixels.clone(),
                delay_ms: 80,
            },
            Frame {
                pixels: pixels.clone(),
                delay_ms: 100,
            },
        ],
        gif: vec![],
    };
    assert!(prepared.kitty_stream(0, 10).is_err());
    assert!(prepared.kitty_stream(32, 65).is_err());
    let bytes = prepared.kitty_stream(32, 16).unwrap();
    let stream = std::str::from_utf8(&bytes).unwrap();
    let mut payload = String::new();
    let mut chunks = 0;
    let mut frames = 0;
    let mut controls = 0;
    for packet in stream.split_terminator("\x1b\\") {
        let packet = packet.strip_prefix("\x1b_G").unwrap();
        let Some((header, data)) = packet.split_once(';') else {
            assert!(payload.is_empty());
            assert!(packet.starts_with("a=a,"));
            controls += 1;
            continue;
        };
        assert!(!data.is_empty() && data.len() <= 4096);
        assert!(
            data.bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"+/=".contains(&b))
        );
        if payload.is_empty() {
            assert!(header.starts_with(if frames == 0 { "a=T," } else { "a=f," }));
        } else {
            assert!(header.starts_with(if frames == 0 { "q=2,m=" } else { "a=f,q=2,m=" }));
        }
        chunks += 1;
        payload.push_str(data);
        if header.ends_with("m=0") {
            let png = gtk::glib::base64_decode(&payload);
            assert_eq!(
                image::load_from_memory_with_format(&png, ImageFormat::Png)
                    .unwrap()
                    .to_rgba8(),
                pixels
            );
            payload.clear();
            frames += 1;
        } else {
            assert!(header.ends_with("m=1"));
            assert_eq!(data.len(), 4096);
        }
    }
    assert!(payload.is_empty());
    assert!(chunks > 4, "fixture must exercise continuation packets");
    assert_eq!(frames, 2);
    assert_eq!(controls, 2);
}
