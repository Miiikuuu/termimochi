//! One-shot native output adaptation, not a shell/resize hook or design model.
//! Fastfetch detects real facts and emits the existing logo protocol; the shared
//! bounded ANSI tools wrap its information before the final native output.
use super::*;
use crate::{greeting::wrap_ansi, greeting_official::NativeOutput};
use serde_json::{Value, json};
use std::{
    io::Write,
    time::{Duration, Instant},
};

pub(crate) const ARG: &str = "--termimochi-native-greeting";

fn columns() -> Result<usize, String> {
    let size = crate::preview_context::run_bounded(
        Command::new("/usr/bin/stty").args(["-F", "/dev/tty", "size"]),
        Instant::now() + Duration::from_secs(1),
    )
    .map_err(|_| "Cannot measure the native Greeting terminal width.".to_owned())?;
    size.split_whitespace()
        .nth(1)
        .and_then(|s| s.parse::<usize>().ok())
        .filter(|n| (2..=4096).contains(n))
        .ok_or_else(|| "Invalid native Greeting terminal width.".into())
}

fn display(config: &mut Value) {
    // Explicit across Fastfetch versions. Information is already wrapped before
    // the native write; later terminal resize must not inherit no-wrap mode.
    config["display"]["pipe"] = json!(false);
    config["display"]["disableLinewrap"] = json!(false);
    config["display"]["hideCursor"] = json!(false);
    config["display"]["showErrors"] = json!(true);
}

fn compose(mut config: Value, information: &str, columns: usize) -> Result<Value, String> {
    let width = columns.saturating_sub(1).max(2);
    let logo = &config["logo"];
    let art_width = match logo["type"].as_str() {
        Some("none") => 0,
        Some("data" | "data-raw") => logo["source"]
            .as_str()
            .unwrap_or("")
            .lines()
            .map(|s| crate::greeting::clip_ansi(s, usize::MAX).1)
            .max()
            .unwrap_or(0),
        _ => logo["width"].as_u64().unwrap_or(0) as usize,
    };
    if art_width > width {
        return Err(format!(
            "Greeting artwork occupies {art_width} columns, but this terminal has {columns}. Widen the terminal and reopen the session; the saved artwork was not resized."
        ));
    }
    let gap = logo["padding"]["right"].as_u64().unwrap_or(0) as usize;
    let left = logo["padding"]["left"].as_u64().unwrap_or(0) as usize;
    let side = matches!(logo["position"].as_str(), Some("left" | "right"))
        && art_width > 0
        && width
            >= art_width
                .saturating_add(gap)
                .saturating_add(left)
                .saturating_add(20);
    let info_width = if side {
        width - art_width - gap - left
    } else {
        width
    };
    if !side && config["logo"]["type"] != "none" {
        config["logo"]["position"] = json!("top");
        // Fastfetch uses padding.right as the vertical gap for a top logo.
        config["logo"]["padding"]["right"] = json!(1);
        config["logo"]["padding"]["left"] = json!(0);
    }
    let mut modules = Vec::new();
    for line in information.split_terminator("\r\n") {
        for (row, _) in wrap_ansi(line, info_width) {
            if modules.len() >= 4096 {
                return Err(
                    "Wrapped Greeting exceeds 4096 rows; output was not silently truncated.".into(),
                );
            }
            // Literal format, not a new source of executable/custom user code.
            modules.push(json!({"type":"custom", "format":row.replace('{', "{{")}));
        }
    }
    if modules.is_empty() {
        modules.push(json!({"type":"custom","format":""}));
    }
    config["modules"] = json!(modules);
    display(&mut config);
    Ok(config)
}

fn fetch(executable: &Path, config: &Value, limit: usize) -> Result<String, String> {
    // Fastfetch uses the extension to distinguish JSONC from a named preset.
    let mut file = tempfile::Builder::new()
        .suffix(".jsonc")
        .tempfile()
        .map_err(|e| e.to_string())?;
    serde_json::to_writer(&mut file, config).map_err(|e| e.to_string())?;
    file.flush().map_err(|e| e.to_string())?;
    crate::preview_context::run_bounded_with_limit(
        // Fastfetch skips exactly this wrapper when detecting the real shell.
        Command::new(executable)
            .env("FFTS_IGNORE_PARENT", "1")
            .arg("--config")
            .arg(file.path()),
        Instant::now() + Duration::from_secs(10),
        limit,
    )
    .map_err(|e| format!("Native Greeting output failed: {e:?}"))
}

pub(crate) fn run(directory: &Path) -> Result<(), String> {
    let bytes = read(&directory.join(MANIFEST))?.ok_or("Missing managed Greeting manifest.")?;
    let mut deployment: Deployment = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    if directory != deployment.directory || !deployment.dependencies.adaptive_greeting {
        return Err("This session does not contain a reviewed adaptive Greeting.".into());
    }
    // The already-running helper is pinned/checked by the caller (a daily
    // launcher can retain its own copy). Never execute the old editor helper
    // from this manifest or require that old editor binary to still exist.
    deployment.dependencies.helper = None;
    deployment.check()?;
    let executable = &deployment
        .dependencies
        .fastfetch
        .as_ref()
        .ok_or("Missing Fastfetch dependency.")?
        .path;
    let source = read(&directory.join("fastfetch.jsonc"))?
        .ok_or("Missing native Greeting configuration.")?;
    let config =
        crate::fastfetch_document::value(std::str::from_utf8(&source).map_err(|e| e.to_string())?)?;
    let mut fields = config.clone();
    fields["logo"] = json!({"type":"none"});
    display(&mut fields);
    let raw = fetch(executable, &fields, 128 * 1024)
        .map_err(|e| format!("Collecting Greeting fields: {e}"))?;
    let information = NativeOutput::parse(&raw).information()?;
    let mut output = compose(config, &information, columns()?)?;
    // Our generated animation explicitly uses Kitty C=1 (cursor unchanged).
    // Fastfetch raw/top assumes the stream advances past its image. Provide
    // that advance in an ephemeral copy, never in the shared immutable asset
    // (which is also used for side-by-side output).
    let mut top_asset = None;
    if output["logo"]["position"] == "top"
        && output["logo"]["type"] == "raw"
        && deployment.hashes.contains_key("logo.kitty")
    {
        let stream = read(&directory.join("logo.kitty"))?.ok_or("Missing animation stream.")?;
        let height = output["logo"]["height"]
            .as_u64()
            .filter(|n| (1..=64).contains(n))
            .ok_or("Invalid animation row reservation.")?;
        let mut file = tempfile::NamedTempFile::new().map_err(|e| e.to_string())?;
        file.write_all(&stream)
            .and_then(|()| file.write_all("\r\n".repeat(height as usize).as_bytes()))
            .map_err(|e| e.to_string())?;
        file.flush().map_err(|e| e.to_string())?;
        output["logo"]["source"] = json!(file.path());
        top_asset = Some(file);
    }
    // Fastfetch still emits the actual Kitty/raw graphics, not a GTK image.
    let native = fetch(executable, &output, LIMIT as usize)
        .map_err(|e| format!("Rendering Greeting: {e}"))?;
    drop(top_asset);
    std::io::stdout()
        .lock()
        .write_all(native.as_bytes())
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use unicode_width::UnicodeWidthStr;
    fn config() -> Value {
        json!({"logo":{"type":"raw","width":32,"height":14,"position":"left","padding":{"right":3}},"display":{},"modules":["cpu"]})
    }
    #[test]
    fn narrow_stacks_wide_wraps_only_in_information_column_without_losing_tails() {
        let original = config();
        let info = "\x1b[31mCPU: 13th Gen Intel(R) Core(TM) i7-13700H (20) @ 5.00 GHz\x1b[0m\r\nGPU: 中文🦀e\u{301} {literal} tail\r\n";
        for columns in [46, 80, 142] {
            let rendered = compose(original.clone(), info, columns).unwrap();
            assert_eq!(rendered["logo"]["width"], 32);
            assert_eq!(rendered["logo"]["height"], 14);
            assert_eq!(
                rendered["logo"]["position"],
                if columns == 46 { "top" } else { "left" }
            );
            assert_eq!(rendered["display"]["disableLinewrap"], false);
            let texts: Vec<_> = rendered["modules"]
                .as_array()
                .unwrap()
                .iter()
                .map(|m| m["format"].as_str().unwrap().replace("{{", "{"))
                .collect();
            let plain = |s: &str| crate::greeting_art::Artwork::parse(s).unwrap().plain;
            assert_eq!(plain(&texts.join("")), plain(&info.replace("\r\n", "")));
            let max = if columns == 46 { 45 } else { columns - 36 };
            assert!(texts.iter().all(|s| plain(s).width() <= max));
        }
        assert!(
            compose(original, info, 25)
                .unwrap_err()
                .contains("not resized")
        );
    }
    #[test]
    fn native_information_preserves_long_rows_and_rejects_overflow_instead_of_clipping() {
        let long = format!("CPU: {} END\r\n", "中".repeat(200));
        let info = NativeOutput::parse(&long).information().unwrap();
        assert!(info.contains("END"));
        let result = compose(config(), &info, 80).unwrap();
        assert!(result["modules"].as_array().unwrap().len() > 8);
        assert!(
            NativeOutput::parse(&"x".repeat(4097))
                .information()
                .is_err()
        );
        assert!(
            NativeOutput::parse(&"row\n".repeat(256))
                .information()
                .is_err()
        );
        assert!(
            !NativeOutput::parse("\x1b]52;c;SECRET\x07CPU: safe\n")
                .information()
                .unwrap()
                .contains("SECRET")
        );
    }
}
