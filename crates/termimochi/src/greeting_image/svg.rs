//! Static SVG rasterization in a disposable, offline, resource-limited process.
use super::*;
use resvg::{tiny_skia, usvg};
use serde::{Deserialize, Serialize};
use std::{
    io::{Read, Write},
    process::Command,
    time::{Duration, Instant},
};

pub(super) const SVG_LIMIT: usize = 2 * 1024 * 1024;
const RASTER_SIDE: u32 = 1024;
const OUTPUT_LIMIT: usize = 6 * 1024 * 1024;
pub(crate) const WORKER_ARG: &str = "--termimochi-svg-worker";

pub(super) fn looks_like_svg(bytes: &[u8]) -> bool {
    let bytes = bytes.strip_prefix(b"\xef\xbb\xbf").unwrap_or(bytes);
    bytes.iter().find(|b| !b.is_ascii_whitespace()) == Some(&b'<')
}

fn preflight(bytes: &[u8]) -> Result<roxmltree::Document<'_>, String> {
    if bytes.is_empty() || bytes.len() > SVG_LIMIT {
        return Err("SVG sources must be nonempty and at most 2 MiB.".into());
    }
    let text = std::str::from_utf8(bytes)
        .map_err(|_| "SVG must use UTF-8 encoding.")?
        .trim_start_matches('\u{feff}');
    if text.contains("<!DOCTYPE") || text.contains("<!ENTITY") {
        return Err("SVG document types and custom entities are not allowed.".into());
    }
    let document = roxmltree::Document::parse_with_options(
        text,
        roxmltree::ParsingOptions {
            allow_dtd: false,
            nodes_limit: 10_000,
            ..Default::default()
        },
    )
    .map_err(|e| format!("Invalid or overly complex SVG: {e}"))?;
    let root = document.root_element();
    if root.tag_name().name() != "svg"
        || root.tag_name().namespace() != Some("http://www.w3.org/2000/svg")
    {
        return Err("Use an SVG document with the standard SVG namespace.".into());
    }
    for node in document.descendants() {
        if node.ancestors().take(66).count() > 64 {
            return Err("SVG nesting exceeds 64 levels.".into());
        }
        if node.is_pi() {
            return Err("SVG stylesheet processing instructions are not supported.".into());
        }
        if !node.is_element() {
            continue;
        }
        let name = node.tag_name().name();
        if matches!(
            name,
            "script"
                | "foreignObject"
                | "animate"
                | "animateMotion"
                | "animateTransform"
                | "set"
                | "discard"
        ) {
            return Err("Use a static SVG without scripts, animation or embedded HTML.".into());
        }
        if name == "image" || name == "feImage" {
            return Err("SVG embedded or linked images are not supported yet. Export vector shapes or a still PNG instead.".into());
        }
        if name == "style" {
            for child in node.children().filter(|n| n.is_text()) {
                check_references(child.text().unwrap_or(""))?;
            }
        }
        for attribute in node.attributes() {
            if attribute.name().starts_with("on")
                || attribute.name() == "base"
                    && attribute.namespace() == Some("http://www.w3.org/XML/1998/namespace")
            {
                return Err("SVG event handlers and external base paths are not allowed.".into());
            }
            let value = attribute.value().trim();
            if attribute.name() == "href" && !value.is_empty() && !value.starts_with('#') {
                return Err(
                    "SVG external references are not allowed. Use local #id references.".into(),
                );
            }
            check_references(value)?;
        }
    }
    Ok(document)
}

fn check_references(value: &str) -> Result<(), String> {
    // Do not allow CSS escapes/at-rules to hide a resource URL or animation.
    if value.contains('\\') || value.contains('@') {
        return Err(
            "SVG CSS escapes and at-rules are not supported. Use static inline styles.".into(),
        );
    }
    let lower = value.to_ascii_lowercase();
    for tail in lower.split("url(").skip(1) {
        let target = tail
            .split_once(')')
            .ok_or("Invalid SVG URL reference.")?
            .0
            .trim()
            .trim_matches(['\'', '"'])
            .trim();
        if !target.starts_with('#') {
            return Err(
                "SVG external resources are not allowed. Use local url(#id) references.".into(),
            );
        }
    }
    Ok(())
}

fn rasterize(bytes: &[u8]) -> Result<DecodedImage, String> {
    let document = preflight(bytes)?;
    let mut options = usvg::Options {
        font_family: "DejaVu Sans".into(),
        image_href_resolver: usvg::ImageHrefResolver {
            resolve_data: Box::new(|_, _, _| None),
            resolve_string: Box::new(|_, _| None),
        },
        ..Default::default()
    };
    if document.descendants().any(|n| n.has_tag_name("text")) {
        // No user-selected font paths or fonts embedded in SVG are loaded.
        options.fontdb_mut().load_fonts_dir("/usr/share/fonts");
        options
            .fontdb_mut()
            .load_fonts_dir("/usr/local/share/fonts");
        options.fontdb_mut().set_sans_serif_family("DejaVu Sans");
    }
    let tree = usvg::Tree::from_str(document.input_text(), &options)
        .map_err(|e| format!("SVG could not be rendered: {e}"))?;
    let size = tree.size();
    let dimensions = (size.width().ceil() as u32, size.height().ceil() as u32);
    check_dimensions(
        dimensions.0,
        dimensions.1,
        u64::from(dimensions.0) * u64::from(dimensions.1) * 4,
    )?;
    let scale = RASTER_SIDE as f32 / size.width().max(size.height());
    let width = (size.width() * scale)
        .round()
        .clamp(1.0, RASTER_SIDE as f32) as u32;
    let height = (size.height() * scale)
        .round()
        .clamp(1.0, RASTER_SIDE as f32) as u32;
    let mut pixmap =
        tiny_skia::Pixmap::new(width, height).ok_or("Unable to allocate SVG canvas.")?;
    resvg::render(
        &tree,
        tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    // tiny-skia already produces premultiplied RGBA. Multiplying alpha again
    // would darken antialiased edges and break the existing removal pipeline.
    let pixels =
        RgbaImage::from_raw(width, height, pixmap.take()).ok_or("Invalid SVG pixel buffer.")?;
    Ok(DecodedImage {
        pixels,
        dimensions,
        format: "SVG",
    })
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "status", deny_unknown_fields)]
enum Response {
    Ok {
        width: u32,
        height: u32,
        pixel_width: u32,
        pixel_height: u32,
        rgba: String,
    },
    Error {
        message: String,
    },
}

pub(crate) fn worker_entry() -> gtk::glib::ExitCode {
    let result = (|| {
        let mut bytes = Vec::new();
        std::fs::File::open("/input.svg")
            .map_err(|e| e.to_string())?
            .take(SVG_LIMIT as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        rasterize(&bytes)
    })();
    let response = match result {
        Ok(image) => Response::Ok {
            width: image.dimensions.0,
            height: image.dimensions.1,
            pixel_width: image.pixels.width(),
            pixel_height: image.pixels.height(),
            rgba: gtk::glib::base64_encode(image.pixels.as_raw()).into(),
        },
        Err(message) => Response::Error { message },
    };
    if serde_json::to_writer(std::io::stdout().lock(), &response).is_ok() {
        gtk::glib::ExitCode::SUCCESS
    } else {
        gtk::glib::ExitCode::FAILURE
    }
}

fn worker_program() -> Result<std::path::PathBuf, String> {
    let executable = std::env::current_exe().map_err(|e| e.to_string())?;
    #[cfg(test)]
    {
        let path = std::env::var_os("TERMIMOCHI_SVG_WORKER_BIN")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| {
                executable
                    .parent()
                    .unwrap()
                    .parent()
                    .unwrap()
                    .join("termimochi")
            });
        path.canonicalize().map_err(|_| "Build the SVG helper with cargo build -p termimochi, or set TERMIMOCHI_SVG_WORKER_BIN for tests.".into())
    }
    #[cfg(not(test))]
    Ok(executable)
}

pub(super) fn decode(bytes: &[u8]) -> Result<DecodedImage, String> {
    decode_with_worker(bytes, &worker_program()?, Duration::from_secs(5))
}

fn decode_with_worker(
    bytes: &[u8],
    executable: &Path,
    budget: Duration,
) -> Result<DecodedImage, String> {
    if bytes.len() > SVG_LIMIT {
        return Err("SVG sources are limited to 2 MiB.".into());
    }
    let mut input = tempfile::NamedTempFile::new().map_err(|e| e.to_string())?;
    input.write_all(bytes).map_err(|e| e.to_string())?;
    let mut command = Command::new("/usr/bin/bwrap");
    command
        .args([
            "--die-with-parent",
            "--new-session",
            "--unshare-all",
            "--cap-drop",
            "ALL",
            "--ro-bind",
            "/usr",
            "/usr",
            "--ro-bind-try",
            "/lib",
            "/lib",
            "--ro-bind-try",
            "/lib64",
            "/lib64",
            "--ro-bind-try",
            "/etc/fonts",
            "/etc/fonts",
            "--ro-bind",
        ])
        .arg(executable)
        .arg("/app")
        .arg("--ro-bind")
        .arg(input.path())
        .arg("/input.svg")
        .args([
            "--proc",
            "/proc",
            "--dev",
            "/dev",
            "--tmpfs",
            "/tmp",
            "--chdir",
            "/tmp",
            "--",
            "/usr/bin/prlimit",
            "--as=1073741824",
            "--cpu=3",
            "--fsize=0",
            "--nofile=128",
            "--",
            "/app",
            WORKER_ARG,
        ])
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("LANG", "C.UTF-8")
        .env("LC_ALL", "C.UTF-8");
    let output = crate::preview_context::run_bounded_with_limit(&mut command,
        Instant::now() + budget, OUTPUT_LIMIT)
        .map_err(|e| format!("SVG renderer unavailable or resource limit reached ({e:?}). Requires Bubblewrap and prlimit; no unsandboxed fallback was run."))?;
    match serde_json::from_str::<Response>(&output).map_err(|_| "Invalid SVG renderer response.")? {
        Response::Error { message } => Err(message),
        Response::Ok {
            width,
            height,
            pixel_width,
            pixel_height,
            rgba,
        } => {
            check_dimensions(width, height, u64::from(width) * u64::from(height) * 4)?;
            if pixel_width == 0
                || pixel_height == 0
                || pixel_width > RASTER_SIDE
                || pixel_height > RASTER_SIDE
            {
                return Err("Invalid SVG raster dimensions.".into());
            }
            let bytes = gtk::glib::base64_decode(&rgba);
            let pixels = RgbaImage::from_raw(pixel_width, pixel_height, bytes)
                .ok_or("Invalid SVG raster data.")?;
            if pixels.pixels().any(|p| p.0[..3].iter().any(|v| *v > p[3])) {
                return Err("Invalid SVG alpha data.".into());
            }
            Ok(DecodedImage {
                pixels,
                dimensions: (width, height),
                format: "SVG",
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="64" height="32"><defs><linearGradient id="g"><stop stop-color="#f00"/><stop offset="1" stop-color="#00f"/></linearGradient></defs><rect x="8" y="4" width="48" height="24" rx="4" fill="url(#g)" opacity="0.5"/></svg>"##;
    #[test]
    fn svg_shapes_gradients_transparency_and_small_vector_resolution() {
        let image = rasterize(SVG.as_bytes()).unwrap();
        assert_eq!(image.dimensions, (64, 32));
        assert_eq!(image.pixels.dimensions(), (1024, 512));
        assert_eq!(image.pixels.get_pixel(0, 0)[3], 0);
        let middle = image.pixels.get_pixel(512, 256);
        assert!((126..=129).contains(&middle[3]));
        assert!(middle[0] > 40 && middle[2] > 40);
        assert!(middle[0] <= middle[3]);
        for style in [Style::Ascii, Style::Detail, Style::HalfBlocks] {
            let output = image
                .convert(Options {
                    columns: 64,
                    style,
                    cell_ratio: 0.5,
                    background: [255; 3],
                    foreground: [20; 3],
                    invert: false,
                    edits: Adjustments::default(),
                })
                .unwrap();
            output.artwork.validate().unwrap();
            assert_eq!(output.format, "SVG");
            assert!(!output.artwork.ansi.contains("<svg"));
        }
    }
    #[test]
    fn svg_rejects_external_active_malformed_and_oversized_inputs() {
        for child in [
            r#"<script/>"#,
            r#"<animate/>"#,
            r#"<foreignObject/>"#,
            r#"<image href="file:///etc/passwd"/>"#,
            r#"<use href="https://example.invalid/a.svg#x"/>"#,
            r#"<rect fill="url(file:///etc/passwd)"/>"#,
            r#"<style>@import 'remote.css';</style>"#,
            r#"<rect onclick="bad()"/>"#,
            r#"<image href="data:image/png;base64,AAAA"/>"#,
        ] {
            let text = format!(r#"<svg xmlns="http://www.w3.org/2000/svg">{child}</svg>"#);
            assert!(preflight(text.as_bytes()).is_err(), "{child}");
        }
        for text in [
            "<!DOCTYPE svg><svg/>",
            "<html/>",
            "<svg/>",
            "<svg",
            "<?xml-stylesheet href='x'?><svg/>",
        ] {
            assert!(preflight(text.as_bytes()).is_err());
        }
        assert!(preflight(&vec![b' '; SVG_LIMIT + 1]).is_err());
        let nested = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg">{}x{}</svg>"#,
            "<g>".repeat(65),
            "</g>".repeat(65)
        );
        assert!(preflight(nested.as_bytes()).is_err());
        assert!(rasterize(SVG.replace("width=\"64\"", "width=\"999999\"").as_bytes()).is_err());
    }
    #[test]
    #[ignore = "requires built TermiMochi SVG helper, Bubblewrap and prlimit"]
    fn svg_sandbox_preserves_pixels_and_rejects_external_files() {
        let expected = rasterize(SVG.as_bytes()).unwrap();
        let actual = decode(SVG.as_bytes()).unwrap();
        assert_eq!(actual.pixels, expected.pixels);
        assert_eq!(actual.dimensions, expected.dimensions);
        let invalid = SVG.replace("url(#g)", "url(file:///etc/passwd)");
        assert!(
            decode(invalid.as_bytes())
                .err()
                .unwrap()
                .contains("external")
        );
    }

    #[test]
    fn svg_viewbox_clipping_use_and_system_text_render() {
        let image = rasterize(br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="10 20 160 80"><defs><clipPath id="clip"><circle cx="50" cy="60" r="25"/></clipPath><rect id="r" x="10" y="20" width="80" height="80" fill="#1a6"/></defs><g clip-path="url(#clip)"><use href="#r"/></g><text x="90" y="65" font-family="DejaVu Sans" font-size="30">SVG</text></svg>"##).unwrap();
        assert_eq!(image.dimensions, (160, 80));
        assert_eq!(image.pixels.get_pixel(0, 0)[3], 0);
        assert!(
            image
                .pixels
                .enumerate_pixels()
                .any(|(x, _, p)| x > 520 && p[3] != 0)
        );
    }

    #[test]
    #[ignore = "requires Bubblewrap and prlimit; isolated timeout/output-limit/missing-helper checks"]
    fn svg_worker_failures_are_bounded_without_unsandboxed_fallback() {
        use std::os::unix::fs::PermissionsExt;
        let root = tempfile::tempdir().unwrap();
        let program = root.path().join("worker");
        std::fs::write(&program, "#!/usr/bin/sh\nexec /usr/bin/sleep 20\n").unwrap();
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o700)).unwrap();
        let started = Instant::now();
        let error = decode_with_worker(SVG.as_bytes(), &program, Duration::from_millis(150))
            .err()
            .unwrap();
        assert!(error.contains("Timeout"), "{error}");
        assert!(started.elapsed() < Duration::from_secs(2));
        std::fs::write(
            &program,
            "#!/usr/bin/sh\nexec /usr/bin/head -c 7000000 /dev/zero\n",
        )
        .unwrap();
        assert!(decode_with_worker(SVG.as_bytes(), &program, Duration::from_secs(2)).is_err());
        assert!(
            decode_with_worker(
                SVG.as_bytes(),
                &root.path().join("missing"),
                Duration::from_secs(2)
            )
            .is_err()
        );
    }
}
