//! Explicit pixel-image bundles. No terminal probing, subprocesses or activation.
use super::*;
use crate::greeting::{GreetingSettings, Position};
use jsonc_parser::cst::CstInputValue;
use std::{
    io::Write as IoWrite,
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::PathBuf,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Protocol {
    Kitty,
    Sixel,
    KittyAnimation,
}
impl Protocol {
    pub fn logo_type(self) -> &'static str {
        match self {
            Self::Kitty => "kitty-direct",
            Self::Sixel | Self::KittyAnimation => "raw",
        }
    }
    pub fn source_name(self) -> &'static str {
        match self {
            Self::Kitty => "logo.png",
            Self::Sixel => "logo.sixel",
            Self::KittyAnimation => "logo.kitty",
        }
    }
    pub fn requirements(self) -> &'static str {
        match self {
            Self::Kitty => {
                "Requires a terminal supporting Kitty graphics with direct PNG file transfer. This is not a terminal capability test."
            }
            Self::Sixel => {
                "Requires a Sixel-capable terminal. Transparent pixels are preserved; soft alpha uses a 50% threshold and colors use a 256-color palette. Size follows the current preview font/DPI; re-export after changing the target font or DPI. No ImageMagick required."
            }
            Self::KittyAnimation => {
                "Requires a terminal supporting Kitty graphics animation. Full-frame replacement prevents transparent-frame trails. GIF palette and transparency limits still apply."
            }
        }
    }
}

pub(crate) struct PixelImage {
    pub pixels: RgbaImage, // straight alpha, ready for PNG and GTK
    png: Vec<u8>,
    pub animation: Option<animation::Prepared>,
}

pub(crate) fn prepare(source: &source::EditableArtwork) -> Result<PixelImage, String> {
    if source.image.is_gif() {
        let animation = animation::prepare(source)?;
        let pixels = animation
            .frames
            .iter()
            .find(|f| f.pixels.pixels().any(|p| p[3] > 1))
            .ok_or("GIF has no visible frames after conversion.")?
            .pixels
            .clone();
        return encoded(pixels, Some(animation));
    }
    let options = source.options()?;
    let decoded = source.image.decode()?;
    let mut pixels = processing::crop_image(&decoded.pixels, options.edits);
    let removal = background::remove(
        &mut pixels,
        options.edits.removal,
        if options.edits.removal.enabled && options.edits.removal.color.is_none() {
            background::detect(&decoded.pixels)
        } else {
            None
        },
    );
    if let Some(issue) = removal.issue {
        return Err(issue.into());
    }
    processing::adjust(&mut pixels, options.edits);
    if options.edits.smoothing > 0.0 {
        pixels = image::imageops::blur(&pixels, (options.edits.smoothing * 1.2) as f32);
    }
    let mut visible = false;
    for pixel in pixels.pixels_mut() {
        if let Some(color) = straight_color(pixel) {
            let color = processing::ink_color(color, options);
            pixel.0[..3].copy_from_slice(&color);
            visible = true;
        } else {
            pixel.0 = [0; 4];
        }
    }
    if !visible {
        return Err("The image is entirely transparent. Edit Artwork to recover it.".into());
    }
    encoded(pixels, None)
}

fn encoded(
    pixels: RgbaImage,
    animation: Option<animation::Prepared>,
) -> Result<PixelImage, String> {
    let mut png = Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(pixels.clone())
        .write_to(&mut png, ImageFormat::Png)
        .map_err(|e| e.to_string())?;
    // Decoding already caps the longest side at 1024; encoding adds no metadata.
    if png.get_ref().len() > 8 * 1024 * 1024 {
        return Err("Processed PNG exceeds 8 MiB.".into());
    }
    Ok(PixelImage {
        pixels,
        png: png.into_inner(),
        animation,
    })
}

pub(crate) fn area(
    dimensions: (u32, u32),
    columns: u32,
    cell_ratio: f64,
) -> Result<(u32, u32), String> {
    if !(8..=120).contains(&columns)
        || dimensions.0 == 0
        || dimensions.1 == 0
        || !cell_ratio.is_finite()
        || !(0.1..=2.0).contains(&cell_ratio)
    {
        return Err("Invalid pixel-image layout.".into());
    }
    let aspect = f64::from(dimensions.1) / f64::from(dimensions.0) * cell_ratio;
    let width = f64::from(columns).min(64.0 / aspect).floor().max(1.0) as u32;
    Ok((
        width,
        (f64::from(width) * aspect).round().clamp(1.0, 64.0) as u32,
    ))
}

fn configuration(
    settings: &GreetingSettings,
    protocol: Protocol,
    size: (u32, u32),
) -> Result<String, String> {
    settings.validate()?;
    if !settings.enabled {
        return Err("Enable Greeting before exporting an image greeting.".into());
    }
    let source = settings.fastfetch_config()?;
    let root = crate::fastfetch_document::parse(&source)?;
    let object = root.object_value().ok_or("Missing configuration object")?;
    let number = |n: u32| CstInputValue::Number(n.to_string());
    let logo = CstInputValue::Object(vec![
        (
            "type".into(),
            CstInputValue::String(protocol.logo_type().into()),
        ),
        (
            "source".into(),
            CstInputValue::String(protocol.source_name().into()),
        ),
        ("width".into(), number(size.0)),
        ("height".into(), number(size.1)),
        (
            "position".into(),
            CstInputValue::String(
                match settings.position {
                    Position::Left => "left",
                    Position::Right => "right",
                    _ => "top",
                }
                .into(),
            ),
        ),
        (
            "padding".into(),
            CstInputValue::Object(vec![
                ("left".into(), number(0)),
                ("right".into(), number(u32::from(settings.gap))),
                ("top".into(), number(0)),
            ]),
        ),
        ("printRemaining".into(), CstInputValue::Bool(true)),
    ]);
    if let Some(property) = object.get("logo") {
        property.set_value(logo);
    } else {
        object.append("logo", logo);
    }
    let source = root.to_string();
    crate::fastfetch_document::parse(&source)?;
    Ok(source)
}

pub(crate) fn export_bundle(
    parent: &Path,
    settings: &GreetingSettings,
    image: &PixelImage,
    protocol: Protocol,
    columns: u32,
    cell_size: [u32; 2],
) -> Result<PathBuf, String> {
    if !parent.is_absolute() || !parent.is_dir() {
        return Err("Choose an existing local folder.".into());
    }
    if cell_size.iter().any(|v| !(1..=256).contains(v)) {
        return Err("Invalid terminal cell pixel size.".into());
    }
    let cell_ratio = f64::from(cell_size[0]) / f64::from(cell_size[1]);
    let size = area(image.pixels.dimensions(), columns, cell_ratio)?;
    let config = configuration(settings, protocol, size)?;
    let animated = protocol == Protocol::KittyAnimation;
    if animated && image.animation.is_none() {
        return Err(
            "Animated export requires a GIF source. Choose a static protocol for this image."
                .into(),
        );
    }
    let ansi_fallback = (animated || protocol == Protocol::Sixel)
        .then(|| settings.fastfetch_config())
        .transpose()?;
    let sixel_stream = if protocol == Protocol::Sixel {
        Some(sixel::encode(
            &image.pixels,
            (size.0 * cell_size[0], size.1 * cell_size[1]),
        )?)
    } else {
        None
    };
    let kitty_stream = if animated {
        Some(
            image
                .animation
                .as_ref()
                .unwrap()
                .kitty_stream(size.0, size.1)?,
        )
    } else {
        None
    };
    // Each export gets a fresh owner-only directory. Existing files and active
    // Fastfetch configuration are never replaced. Failure drops this new bundle.
    let directory = tempfile::Builder::new()
        .prefix("termimochi-image-")
        .permissions(std::fs::Permissions::from_mode(0o700))
        .tempdir_in(parent)
        .map_err(|e| e.to_string())?;
    let mut guide = format!(
        "# TermiMochi pixel-image greeting\n\nProtocol: {}.\n{}\n\nOpen a terminal IN THIS FOLDER and run:\n\n    fastfetch --config config.jsonc\n\nKeep all files in the bundle together. The relative logo path is resolved from the working directory, not the config filename. You can move the whole folder, then run from its new location.\n\nLogo area: {} columns x {} rows (based on the preview font). The target terminal font, DPI and protocol can change placement; see the protocol-specific notes below before changing dimensions.\n\nlogo.png is a processed static image, not character art: crop, background removal, tone/color and smoothing are retained. Character density, ASCII contours/inversion and opening animations do not apply. Pixel size is bounded to a 1024-pixel longest side. The re-encoded PNG contains no original metadata or editable source.\n\nImported configuration settings, including commands and network modules, are retained. Review config.jsonc before running it. TermiMochi did not run this bundle, install it or change shell startup files. The in-app terminal preview and ordinary Apply still use ANSI artwork.\n",
        protocol.logo_type(),
        protocol.requirements(),
        size.0,
        size.1
    );
    if protocol == Protocol::Sixel {
        guide.push_str(&format!("\n## Transparent Sixel\n\nconfig.jsonc displays logo.sixel using Fastfetch's raw logo mode. TermiMochi generated this stream directly: transparent pixels are not painted (P2=1), avoiding the ImageMagick black-background conversion. No ImageMagick or extra converter is needed. Keep all bundle files together.\n\nThe raster is {} × {} pixels, based on {} × {} pixel cells in the captured preview. Raw Sixel has fixed pixel dimensions: changing logo.width/height alone does NOT resize it. Re-export for a different font, DPI or logo size. Sixel supports binary transparency, not soft alpha: alpha below 128 is cleared, otherwise opaque. Up to 256 colors are quantized; the PNG preview/reference retains full color and soft alpha and may differ at edges.\n\nUse fastfetch --config config-ansi.jsonc for portable static character artwork. No existing exports or active configuration were changed.\n", size.0*cell_size[0], size.1*cell_size[1], cell_size[0], cell_size[1]));
    }
    if animated {
        guide.push_str("\n## GIF animation\n\nconfig.jsonc uses Fastfetch's raw logo type and logo.kitty, a self-contained Kitty graphics stream generated by TermiMochi. Full-frame replacement avoids transparent-frame trails in some icat versions; no kitten executable is needed. Only use this configuration in a Kitty-animation-capable terminal. Keep all files together and run from this folder. If unsupported or placement is incorrect, run: fastfetch --config config-ansi.jsonc\n\nlogo.gif is the processed animation for sharing, and logo.png is its still fallback. Both are separate from the protocol stream. To change animation size, export again: dimensions are embedded in logo.kitty as well as the configuration. Do not treat arbitrary raw protocol files as safe artwork.\n\nAll frames share one crop/trim canvas and one inferred background key. GIF quantization limits colors to a palette and thresholds alpha at 128; soft PNG transparency is not retained. Delays below 20 ms are raised to 20 ms. The preview uses the re-decoded exported GIF. The terminal stream loops continuously; standalone logo.gif retains its encoded repeat setting. No opening effects are added. config-ansi.jsonc uses the accepted ANSI artwork. Original embedded metadata is not copied.\n");
    } else if image.animation.is_some() {
        guide.push_str("\nThis export is a still PNG from a GIF source. Choose Kitty · Animated GIF to retain motion.\n");
    }
    let mut files = vec![
        ("config.jsonc", config.as_bytes()),
        ("logo.png", image.png.as_slice()),
        ("README.md", guide.as_bytes()),
    ];
    if animated {
        files.push(("logo.gif", &image.animation.as_ref().unwrap().gif));
        files.push(("logo.kitty", kitty_stream.as_ref().unwrap().as_slice()));
    }
    if let Some(stream) = &sixel_stream {
        files.push(("logo.sixel", stream.as_slice()));
    }
    if let Some(fallback) = &ansi_fallback {
        files.push(("config-ansi.jsonc", fallback.as_bytes()));
    }
    for (name, bytes) in files {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(directory.path().join(name))
            .map_err(|e| e.to_string())?;
        file.write_all(bytes)
            .and_then(|()| file.sync_all())
            .map_err(|e| e.to_string())?;
    }
    Ok(directory.keep())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::greeting_image::source::{EditableArtwork, SourceImage};

    fn fixture() -> (GreetingSettings, EditableArtwork) {
        let mut image = RgbaImage::from_pixel(24, 24, image::Rgba([255; 4]));
        for y in 6..18 {
            for x in 6..18 {
                image.put_pixel(x, y, image::Rgba([200, 30, 70, 255]));
            }
        }
        let mut encoded = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(image)
            .write_to(&mut encoded, ImageFormat::Png)
            .unwrap();
        let options = Options {
            columns: 32,
            style: Style::Detail,
            cell_ratio: 0.5,
            foreground: [0; 3],
            background: [255; 3],
            invert: false,
            edits: Adjustments {
                removal: Removal {
                    enabled: true,
                    ..Default::default()
                },
                ..Default::default()
            },
        };
        let editable =
            EditableArtwork::new(SourceImage::new(encoded.into_inner()).unwrap(), options).unwrap();
        let settings = GreetingSettings {
            enabled: true,
            editable_artwork: Some(editable.clone()),
            ..Default::default()
        };
        (settings, editable)
    }

    #[test]
    fn png_preserves_removal_and_color_without_mutating_source() {
        let (_, source) = fixture();
        let before = source.clone();
        let image = prepare(&source).unwrap();
        assert_eq!(image.pixels.get_pixel(0, 0)[3], 0);
        assert_eq!(image.pixels.get_pixel(12, 12).0, [200, 30, 70, 255]);
        assert_eq!(
            image::load_from_memory(&image.png).unwrap().into_rgba8(),
            image.pixels
        );
        assert_eq!(source, before);
        for dimensions in [(1, 1024), (1024, 1), (1024, 1024)] {
            let (w, h) = area(dimensions, 120, 0.5).unwrap();
            assert!((1..=120).contains(&w) && (1..=64).contains(&h));
        }
        assert!(area((24, 24), 0, 0.5).is_err());
        assert!(area((24, 24), 32, f64::NAN).is_err());
    }

    #[test]
    fn bundles_preserve_fields_comments_and_never_replace_existing_files() {
        let root = tempfile::tempdir().unwrap();
        let (mut settings, source) = fixture();
        settings.imported_source = Some("// retained\n{\"future\":true,\"modules\":[\"os\",{\"type\":\"command\",\"text\":\"NEVER\"}]}".into());
        let image = prepare(&source).unwrap();
        std::fs::write(root.path().join("config.jsonc"), "untouched").unwrap();
        for protocol in [Protocol::Kitty, Protocol::Sixel] {
            let folder =
                export_bundle(root.path(), &settings, &image, protocol, 32, [10, 20]).unwrap();
            let config = std::fs::read_to_string(folder.join("config.jsonc")).unwrap();
            assert_eq!(
                folder.metadata().unwrap().permissions().mode() & 0o777,
                0o700
            );
            assert_eq!(
                folder
                    .join("logo.png")
                    .metadata()
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
            assert!(config.contains("// retained"));
            let value = crate::fastfetch_document::value(&config).unwrap();
            assert_eq!(value["logo"]["type"], protocol.logo_type());
            assert_eq!(value["logo"]["source"], protocol.source_name());
            assert_eq!(value["logo"]["width"], 32);
            assert_eq!(value["logo"]["height"], 16);
            assert_eq!(value["modules"][1]["text"], "NEVER");
            assert_eq!(value["future"], true);
            assert_eq!(std::fs::read(folder.join("logo.png")).unwrap(), image.png);
            if protocol == Protocol::Sixel {
                let raw = folder.join("logo.sixel");
                assert!(
                    std::fs::read(&raw)
                        .unwrap()
                        .starts_with(b"\x1bP0;1;0q\"1;1;320;320")
                );
                assert_eq!(raw.metadata().unwrap().permissions().mode() & 0o777, 0o600);
                assert_eq!(
                    std::fs::read_to_string(folder.join("config-ansi.jsonc")).unwrap(),
                    settings.fastfetch_config().unwrap()
                );
                assert_eq!(std::fs::read_dir(&folder).unwrap().count(), 5);
            }
            assert_eq!(
                std::fs::read_to_string(root.path().join("config.jsonc")).unwrap(),
                "untouched"
            );
        }
        assert!(!root.path().join("NEVER").exists());
        let count = std::fs::read_dir(root.path()).unwrap().count();
        assert!(
            export_bundle(
                root.path(),
                &settings,
                &image,
                Protocol::Sixel,
                120,
                [32, 64]
            )
            .is_err()
        );
        assert!(
            export_bundle(root.path(), &settings, &image, Protocol::Sixel, 32, [0, 20]).is_err()
        );
        assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), count);
        settings.enabled = false;
        assert!(
            export_bundle(
                root.path(),
                &settings,
                &image,
                Protocol::Kitty,
                32,
                [10, 20]
            )
            .is_err()
        );
        assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), count);
    }

    #[test]
    #[ignore = "requires native Fastfetch; verifies emitted bytes, not visual terminal compatibility"]
    fn pixel_bundles_native_fastfetch_emits_protocols() {
        let root = tempfile::tempdir().unwrap();
        let (mut settings, source) = fixture();
        settings.imported_source =
            Some(r#"{"modules":[{"type":"custom","format":"PIXEL_EXPORT_OK"}]}"#.into());
        let image = prepare(&source).unwrap();
        for protocol in [Protocol::Kitty, Protocol::Sixel] {
            let folder =
                export_bundle(root.path(), &settings, &image, protocol, 32, [10, 20]).unwrap();
            let result = std::process::Command::new("python3")
                .arg(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../../scripts/test-pixel-protocol.py"
                ))
                .arg(&folder)
                .arg(if protocol == Protocol::Sixel {
                    "sixel"
                } else {
                    protocol.logo_type()
                })
                .output()
                .unwrap();
            assert!(
                result.status.success(),
                "{protocol:?}: {}",
                String::from_utf8_lossy(&result.stderr)
            );
            assert!(String::from_utf8_lossy(&result.stdout).contains("PASS"));
        }
    }

    #[test]
    #[ignore = "requires isolated Xvfb, Kitty, SIXEL-enabled xterm and Python Pillow; captures actual pixels"]
    fn pixel_bundles_visual_terminal_rendering() {
        let root = tempfile::tempdir().unwrap();
        let (_, template) = fixture();
        let pixels = RgbaImage::from_fn(128, 96, |x, y| {
            if !(16..112).contains(&x)
                || !(12..84).contains(&y)
                || ((58..70).contains(&x) && (42..54).contains(&y))
            {
                image::Rgba([255; 4])
            } else {
                image::Rgba(match (x < 64, y < 48) {
                    (true, true) => [200, 30, 70, 255],
                    (false, true) => [30, 180, 100, 255],
                    (true, false) => [40, 100, 220, 255],
                    (false, false) => [230, 180, 30, 255],
                })
            }
        });
        let mut bytes = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(pixels)
            .write_to(&mut bytes, ImageFormat::Png)
            .unwrap();
        let mut options = template.options().unwrap();
        options.edits.removal.connected = false;
        let chart = EditableArtwork::new(
            source::SourceImage::new(bytes.into_inner()).unwrap(),
            options,
        )
        .unwrap();
        options.edits.removal.enabled = false;
        let brand = EditableArtwork::new(
            source::SourceImage::new(
                include_bytes!(
                    "../../resources/icons/256x256/apps/io.github.miiikuuu.termimochi.png"
                )
                .to_vec(),
            )
            .unwrap(),
            options,
        )
        .unwrap();
        let mut failures = Vec::new();
        for (name, source) in [("chart", chart), ("brand", brand)] {
            let image = prepare(&source).unwrap();
            for protocol in [Protocol::Kitty, Protocol::Sixel] {
                for position in [Position::Left, Position::Right, Position::Top] {
                    if name == "brand" && position != Position::Left {
                        continue;
                    }
                    let settings = GreetingSettings {
                        enabled: true,
                        position,
                        gap: 3,
                        imported_source: Some(r#"{"modules":[{"type":"custom","format":"PIXEL_EXPORT_OK"},{"type":"custom","format":"OS: TEST LINUX"},{"type":"custom","format":"CPU: TEST CPU"},{"type":"custom","format":"Shell: bash"}]}"#.into()),
                        ..Default::default()
                    };
                    let folder =
                        export_bundle(root.path(), &settings, &image, protocol, 24, [10, 20])
                            .unwrap();
                    let result = std::process::Command::new("python3")
                        .arg(concat!(
                            env!("CARGO_MANIFEST_DIR"),
                            "/../../scripts/test-pixel-rendering.py"
                        ))
                        .arg(&folder)
                        .arg(if protocol == Protocol::Sixel {
                            "sixel"
                        } else {
                            protocol.logo_type()
                        })
                        .arg(name)
                        .output()
                        .unwrap();
                    if !result.status.success() {
                        failures.push(format!(
                            "{name} {protocol:?} {position:?}: {}\n{}",
                            String::from_utf8_lossy(&result.stdout),
                            String::from_utf8_lossy(&result.stderr)
                        ));
                    }
                    println!("{}", String::from_utf8_lossy(&result.stdout));
                }
            }
        }
        assert!(failures.is_empty(), "{}", failures.join("\n"));
    }
}
