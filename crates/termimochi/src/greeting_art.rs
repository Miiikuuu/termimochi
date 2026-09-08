//! Bounded text artwork. Only reviewed SGR reaches VTE or generated exports.
use crate::greeting::{ART_MAX_BYTES, ART_MAX_COLUMNS, ART_MAX_ROWS};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::Path;
use unicode_width::UnicodeWidthStr;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Artwork {
    pub ansi: String,
    pub plain: String,
    pub filtered: bool,
    pub expanded_tabs: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct LogoSnapshot {
    pub descriptor: Value,
    pub artwork: Artwork,
    #[serde(default)]
    pub marked_source: Option<String>,
}

impl LogoSnapshot {
    pub fn validate(&self) -> Result<(), String> {
        self.artwork.validate()?;
        if let Some(source) = &self.marked_source {
            let safe = Artwork::parse_source(source, true)?;
            if safe.filtered
                || safe.ansi != *source
                || marked_art(source, &self.descriptor["color"], &mut Vec::new())?.plain
                    != self.artwork.plain
            {
                return Err("Invalid stored Fastfetch marker source.".into());
            }
        }
        Ok(())
    }
}

// Reopen a compact style after each newline, so clipping/composing one logo
// row cannot bleed into the information column or lose a multiline color.
#[derive(Default, Clone)]
struct Style([Option<Vec<u16>>; 8]);
impl Style {
    fn apply(&mut self, codes: &[u16]) -> Option<()> {
        let mut index = 0;
        while index < codes.len() {
            let n = codes[index];
            match n {
                0 => self.0.fill(None),
                1 => self.0[0] = Some(vec![n]),
                2 => self.0[7] = Some(vec![n]),
                3 => self.0[1] = Some(vec![n]),
                4 => self.0[2] = Some(vec![n]),
                7 => self.0[3] = Some(vec![n]),
                9 => self.0[4] = Some(vec![n]),
                22 => {
                    self.0[0] = None;
                    self.0[7] = None;
                }
                23 => self.0[1] = None,
                24 => self.0[2] = None,
                27 => self.0[3] = None,
                29 => self.0[4] = None,
                30..=37 | 90..=97 => self.0[5] = Some(vec![n]),
                40..=47 | 100..=107 => self.0[6] = Some(vec![n]),
                39 => self.0[5] = None,
                49 => self.0[6] = None,
                38 | 48 => {
                    let count = match codes.get(index + 1)? {
                        5 => 3,
                        2 => 5,
                        _ => return None,
                    };
                    let color = codes.get(index..index + count)?;
                    if color[2..].iter().any(|n| *n > 255) {
                        return None;
                    }
                    self.0[if n == 38 { 5 } else { 6 }] = Some(color.to_vec());
                    index += count - 1;
                }
                _ => return None,
            }
            index += 1;
        }
        Some(())
    }
    fn prefix(&self) -> String {
        let codes: Vec<_> = self
            .0
            .iter()
            .flatten()
            .flatten()
            .map(u16::to_string)
            .collect();
        if codes.is_empty() {
            String::new()
        } else {
            format!("\x1b[{}m", codes.join(";"))
        }
    }
}

fn sgr(text: &str) -> Option<Vec<u16>> {
    if text.len() > 128 {
        return None;
    }
    let text = if text.contains(':') {
        // Common ISO truecolor form includes an optional empty color-space ID.
        text.replace("38:2::", "38:2:")
            .replace("48:2::", "48:2:")
            .replace(':', ";")
    } else {
        text.to_owned()
    };
    let values: Vec<u16> = text
        .split(';')
        .map(|s| {
            if s.is_empty() {
                Some(0)
            } else {
                s.parse().ok()
            }
        })
        .collect::<Option<_>>()?;
    if values.len() > 32 {
        return None;
    }
    Style::default().apply(&values)?;
    Some(values)
}

impl Artwork {
    pub fn parse(input: &str) -> Result<Self, String> {
        Self::parse_source(input, false)
    }
    // Marker sources keep their native style transitions; Fastfetch resolves
    // missing palette slots from its host defaults. Geometry is checked after
    // decoding markers, not by counting $1 as two printable cells.
    fn parse_source(input: &str, marked: bool) -> Result<Self, String> {
        if input.len() > ART_MAX_BYTES {
            return Err("Artwork exceeds 16 KiB.".into());
        }
        let mut art = Self::default();
        let mut style = Style::default();
        let normalized = input
            .strip_prefix('\u{feff}')
            .unwrap_or(input)
            .replace("\r\n", "\n");
        let mut chars = normalized.chars().peekable();
        let mut line = String::new();
        while let Some(ch) = chars.next() {
            let escape = match ch {
                '\x1b' => chars.next(),
                '\u{009b}' => Some('['),
                '\u{009d}' => Some(']'),
                '\u{0090}' => Some('P'),
                '\u{0098}' => Some('X'),
                '\u{009e}' => Some('^'),
                '\u{009f}' => Some('_'),
                _ => None,
            };
            if ch == '\x1b' || escape.is_some() {
                if escape == Some('[') {
                    let mut sequence = String::new();
                    let mut final_byte = None;
                    for next in chars.by_ref() {
                        if ('@'..='~').contains(&next) {
                            final_byte = Some(next);
                            break;
                        }
                        sequence.push(next);
                    }
                    if final_byte == Some('m')
                        && let Some(codes) = sgr(&sequence)
                    {
                        style.apply(&codes).expect("reviewed SGR");
                        art.ansi.push_str(&format!(
                            "\x1b[{}m",
                            codes
                                .iter()
                                .map(u16::to_string)
                                .collect::<Vec<_>>()
                                .join(";")
                        ));
                    } else {
                        art.filtered = true;
                    }
                } else {
                    art.filtered = true;
                    if matches!(escape, Some(']' | 'P' | 'X' | '^' | '_')) {
                        let mut previous_escape = false;
                        for next in chars.by_ref() {
                            if matches!(next, '\x07' | '\u{009c}')
                                || previous_escape && next == '\\'
                            {
                                break;
                            }
                            previous_escape = next == '\x1b';
                        }
                    } else if escape.is_some_and(|c| (' '..='/').contains(&c)) {
                        for next in chars.by_ref() {
                            if ('0'..='~').contains(&next) {
                                break;
                            }
                        }
                    }
                }
            } else if ch == '\n' {
                let prefix = if marked {
                    String::new()
                } else {
                    style.prefix()
                };
                if !prefix.is_empty() {
                    art.ansi.push_str("\x1b[0m");
                }
                art.ansi.push('\n');
                if chars.peek().is_some() {
                    art.ansi.push_str(&prefix);
                }
                art.plain.push('\n');
                line.clear();
            } else if ch == '\t' {
                let spaces = " ".repeat(if marked { 4 } else { 8 - line.width() % 8 });
                art.ansi.push_str(&spaces);
                art.plain.push_str(&spaces);
                line.push_str(&spaces);
                art.expanded_tabs = true;
            } else if ch.is_control()
                || matches!(ch,'\u{061c}'|'\u{200e}'..='\u{200f}'|'\u{2028}'..='\u{202e}'|'\u{2066}'..='\u{2069}')
            {
                art.filtered = true;
            } else {
                art.ansi.push(ch);
                art.plain.push(ch);
                line.push(ch);
            }
        }
        if !marked && !style.prefix().is_empty() && !art.ansi.ends_with('\n') {
            art.ansi.push_str("\x1b[0m");
        }
        if art.ansi.len() > ART_MAX_BYTES {
            return Err("Normalized artwork exceeds 16 KiB.".into());
        }
        if !marked {
            art.check_size()?;
        }
        Ok(art)
    }
    fn check_size(&self) -> Result<(), String> {
        if self.ansi.len() > ART_MAX_BYTES
            || self.plain.len() > ART_MAX_BYTES
            || self.plain.lines().count() > ART_MAX_ROWS
            || self.plain.lines().any(|s| s.width() > ART_MAX_COLUMNS)
        {
            return Err(
                "Artwork supports 64 lines, 120 cells per line and 16 KiB after normalization."
                    .into(),
            );
        }
        Ok(())
    }
    pub fn validate(&self) -> Result<(), String> {
        self.check_size()?;
        let check = Self::parse(&self.ansi)?;
        if check.filtered || check.ansi != self.ansi || check.plain != self.plain {
            return Err("Invalid or unsafe stored artwork.".into());
        }
        Ok(())
    }
    pub fn notices(&self) -> Vec<&'static str> {
        let mut notes = Vec::new();
        if self.filtered {
            notes.push("Artwork controls were filtered. Only colors and supported text styles are retained; cursor movement, clearing, links, clipboard/title commands and unsupported SGR are omitted. This is a restricted ANSI preview, not a screen emulator.");
        }
        if self.expanded_tabs {
            notes.push("Artwork tabs were expanded for stable alignment: 8-column stops for artwork files, 4 spaces for Fastfetch marker sources.");
        }
        notes
    }
}

pub(crate) fn file_art(path: &Path) -> Result<Artwork, String> {
    let ext = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if !matches!(ext.as_str(), "txt" | "ans") {
        return Err("Choose a .txt or .ans artwork file.".into());
    }
    let bytes = crate::typography_preset::read_private_with_limit(path, ART_MAX_BYTES as u64)?
        .ok_or("Artwork file no longer exists.")?;
    let input = String::from_utf8(bytes).map_err(
        |_| "Artwork must use UTF-8 or ASCII. Legacy CP437 ANSI files need conversion first.",
    )?;
    if ext == "txt" && input.contains('\x1b') {
        return Err("This text file contains ANSI escapes. Import it as .ans to filter controls and retain colors.".into());
    }
    Artwork::parse(&input)
}

pub(crate) fn color(value: &Value) -> Option<String> {
    let text = value.as_str()?;
    if text.len() > 128 {
        return None;
    }
    let code = if let Some(hex) = text.strip_prefix('#') {
        let hex = if hex.len() == 3 && hex.is_ascii() {
            hex.chars().flat_map(|c| [c, c]).collect()
        } else {
            hex.to_owned()
        };
        if hex.len() != 6 || !hex.is_ascii() {
            return None;
        }
        format!(
            "38;2;{};{};{}",
            u8::from_str_radix(&hex[..2], 16).ok()?,
            u8::from_str_radix(&hex[2..4], 16).ok()?,
            u8::from_str_radix(&hex[4..], 16).ok()?
        )
    } else if let Some(index) = text.strip_prefix('@') {
        format!("38;5;{}", index.parse::<u8>().ok()?)
    } else if text
        .chars()
        .all(|c| c.is_ascii_digit() || matches!(c, ';' | ':'))
    {
        text.to_owned()
    } else {
        let mut codes = Vec::new();
        let mut bright = false;
        let mut background = false;
        let parts: Vec<_> = text.split('_').collect();
        for part in &parts[..parts.len().saturating_sub(1)] {
            match *part {
                "bright" | "light" => bright = true,
                "bg" => background = true,
                "reset" => codes.push(0),
                "bold" => codes.push(1),
                "dim" => codes.push(2),
                "italic" => codes.push(3),
                "underline" => codes.push(4),
                "inverse" => codes.push(7),
                "strike" => codes.push(9),
                _ => return None,
            }
        }
        let last = *parts.last()?;
        let slot = [
            "black", "red", "green", "yellow", "blue", "magenta", "cyan", "white",
        ]
        .iter()
        .position(|s| *s == last);
        codes.push(if last == "default" {
            if background { 49 } else { 39 }
        } else {
            slot? as u16
                + if background {
                    if bright { 100 } else { 40 }
                } else if bright {
                    90
                } else {
                    30
                }
        });
        codes
            .iter()
            .map(u16::to_string)
            .collect::<Vec<_>>()
            .join(";")
    };
    let codes = sgr(&code)?;
    Some(
        codes
            .iter()
            .map(u16::to_string)
            .collect::<Vec<_>>()
            .join(";"),
    )
}

pub(crate) fn marked_art(
    text: &str,
    colors: &Value,
    notes: &mut Vec<String>,
) -> Result<Artwork, String> {
    let safe = Artwork::parse_source(text, true)?;
    let mut palette = [const { None }; 9];
    if let Some(values) = colors.as_object() {
        for (key, value) in values {
            if let Ok(index) = key.parse::<usize>()
                && (1..=9).contains(&index)
            {
                palette[index - 1] = color(value);
                if palette[index - 1].is_none() {
                    notes.push(format!("Logo color {key}: unsupported color/style; retained in the source, omitted in preview."));
                }
            } else {
                notes.push(
                    "Logo color slots must be 1–9; unknown slots are retained but not simulated."
                        .into(),
                );
            }
        }
    } else if !colors.is_null() {
        notes.push("Logo colors must be an object; retained but not simulated.".into());
    }
    let mut chars = safe.ansi.chars().peekable();
    let mut expanded = String::new();
    while let Some(ch) = chars.next() {
        if ch == '$' && chars.peek() == Some(&'$') {
            chars.next();
            expanded.push('$');
        } else if ch == '$' && chars.peek().is_some_and(|c| ('1'..='9').contains(c)) {
            let index = chars.next().unwrap().to_digit(10).unwrap() as usize - 1;
            if let Some(code) = &palette[index] {
                expanded.push_str(&format!("\x1b[{code}m"));
            }
        } else {
            expanded.push(ch);
        }
        if expanded.len() > ART_MAX_BYTES {
            return Err("Expanded text logo exceeds 16 KiB.".into());
        }
    }
    let mut art = Artwork::parse(&expanded)?;
    art.filtered |= safe.filtered;
    art.expanded_tabs |= safe.expanded_tabs;
    Ok(art)
}

pub(crate) fn source_kind(logo: &Value) -> (&str, &str) {
    let source = logo
        .as_str()
        .or_else(|| logo["source"].as_str())
        .unwrap_or("");
    let kind = logo["type"].as_str().unwrap_or("auto");
    (kind, source)
}
pub(crate) fn builtin_name(source: &str) -> bool {
    source.len() <= 80
        && source
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-'))
}
pub(crate) fn is_file(logo: &Value) -> bool {
    let (kind, source) = source_kind(logo);
    matches!(kind, "file" | "file-raw") || kind == "auto" && !builtin_name(source)
}

/// Only literal, contained references are read, and only when the user loads
/// that config. Native Fastfetch never receives a file path or word expansion.
pub(crate) fn snapshot_file(
    source: &str,
    config_path: &Path,
) -> Result<Option<LogoSnapshot>, String> {
    let value = crate::fastfetch_document::value(source)?;
    let logo = &value["logo"];
    if !is_file(logo) {
        return Ok(None);
    }
    let (kind, name) = source_kind(logo);
    if name.is_empty()
        || name == "-"
        || name
            .chars()
            .any(|c| c.is_control() || "$`~|&;<>(){}*?[]\\".contains(c))
    {
        return Err("Logo path expressions are not expanded. Import the artwork file explicitly to embed it.".into());
    }
    let root = config_path
        .parent()
        .ok_or("Missing config directory")?
        .canonicalize()
        .map_err(|e| e.to_string())?;
    let path = root.join(name);
    let canonical = path.canonicalize().map_err(
        |_| "Referenced logo is missing. Import the artwork file explicitly to embed it.",
    )?;
    if !canonical.starts_with(&root) {
        return Err("Referenced artwork is outside the configuration directory. Import the artwork explicitly to embed it.".into());
    }
    let bytes = crate::typography_preset::read_private_with_limit(&path, ART_MAX_BYTES as u64)?
        .ok_or("Missing logo")?;
    let input = String::from_utf8(bytes).map_err(|_| "Referenced logo must use UTF-8 or ASCII.")?;
    let artwork = if kind == "file-raw" {
        Artwork::parse(&input)?
    } else {
        marked_art(&input, &logo["color"], &mut Vec::new())?
    };
    Ok(Some(LogoSnapshot {
        descriptor: logo.clone(),
        artwork,
        marked_source: if kind == "file-raw" {
            None
        } else {
            Some(Artwork::parse_source(&input, true)?.ansi)
        },
    }))
}

pub(crate) fn export_file(
    path: &Path,
    contents: &str,
    ansi: bool,
    expected: &Option<Vec<u8>>,
) -> Result<(), String> {
    let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
    if !ext.eq_ignore_ascii_case(if ansi { "ans" } else { "txt" }) {
        return Err(
            "Use .txt for plain artwork or .ans for ANSI artwork. Other files cannot be targets."
                .into(),
        );
    }
    let art = Artwork::parse(contents)?;
    if art.filtered || !ansi && art.ansi.contains('\x1b') {
        return Err("Unsafe or mismatched artwork export.".into());
    }
    if &crate::typography_preset::read_private_with_limit(path, ART_MAX_BYTES as u64)? != expected {
        return Err("Artwork changed on disk. Choose the file again before replacing it.".into());
    }
    if let Some(bytes) = expected {
        crate::typography_preset::retain_backup(
            &path
                .parent()
                .ok_or("Missing directory")?
                .join("termimochi-backups"),
            bytes,
        )?;
    }
    crate::typography_preset::write_checked_with_limit(
        path,
        contents.as_bytes(),
        expected,
        ART_MAX_BYTES as u64,
    )
}

pub(crate) fn text_logo(
    logo: &Value,
    snapshot: Option<&LogoSnapshot>,
    notes: &mut Vec<String>,
) -> Result<Artwork, String> {
    let (kind, source) = source_kind(logo);
    let cached = snapshot.filter(|s| &s.descriptor == logo);
    let art = if is_file(logo) {
        let cached = cached.ok_or("Referenced text logo is not loaded. Only literal files inside the configuration directory are read on import; use Artwork Files to embed another file.")?;
        cached.validate()?;
        notes.push("File logo preview uses a saved snapshot resolved relative to the configuration directory. Fastfetch itself resolves paths from its working directory. The original path is retained in export; import artwork explicitly to embed a portable copy.".into());
        // Validate/report palette properties even for a cached file.
        marked_art("", &logo["color"], notes)?;
        cached.artwork.clone()
    } else if kind == "data" {
        marked_art(source, &logo["color"], notes)?
    } else if kind == "data-raw" {
        Artwork::parse(source)?
    } else {
        return Err("Choose a text logo before exporting artwork.".into());
    };
    notes.extend(art.notices().into_iter().map(str::to_owned));
    if let Some(cached) = cached {
        for note in cached.artwork.notices() {
            if !notes.iter().any(|s| s == note) {
                notes.push(note.into());
            }
        }
    }
    Ok(art)
}

/// Never pass a path, auto-detected source or unreviewed control to Fastfetch.
pub(crate) fn preview_logo(
    logo: &Value,
    snapshot: Option<&LogoSnapshot>,
    notes: &mut Vec<String>,
) -> Value {
    use serde_json::json;
    let (kind, source) = source_kind(logo);
    let mut result = if logo.is_null() || kind == "none" || source == "none" && kind == "auto" {
        json!({"type":"none"})
    } else if matches!(kind, "builtin" | "small" | "auto") && builtin_name(source) {
        json!({"type":if kind=="small" {"small"} else {"builtin"}, "source":source})
    } else {
        match text_logo(logo, snapshot, notes) {
            Ok(art) if !art.plain.is_empty() => {
                let marked = if kind == "data" {
                    Artwork::parse_source(source, true).ok().map(|a| a.ansi)
                } else {
                    snapshot
                        .filter(|s| &s.descriptor == logo)
                        .and_then(|s| s.marked_source.clone())
                };
                if let Some(text) = marked {
                    json!({"type":"data","source":text})
                } else {
                    json!({"type":"data-raw","source":art.ansi})
                }
            }
            Ok(_) => {
                notes.push("Empty text logo is hidden in preview; Fastfetch may select a default logo for an empty source.".into());
                json!({"type":"none"})
            }
            Err(error) => {
                notes.push(format!(
                    "Logo: {error} Image protocols and path expressions are not executed."
                ));
                json!({"type":"none"})
            }
        }
    };
    if matches!(result["type"].as_str(), Some("data" | "builtin" | "small")) {
        if let Some(colors) = logo["color"].as_object() {
            for (key, value) in colors {
                if key.len() == 1
                    && matches!(key.as_bytes()[0], b'1'..=b'9')
                    && let Some(code) = color(value)
                {
                    result["color"][key] = json!(code);
                } else {
                    notes.push(format!(
                        "Logo color {key}: retained, unsupported in preview."
                    ));
                }
            }
        } else if !logo["color"].is_null() {
            notes.push("Logo colors must be an object; retained but not simulated.".into());
        }
    }
    if let Some(object) = logo.as_object() {
        for (key, value) in object {
            match key.as_str() {
                "type" | "source" | "color" => {}
                "position"
                    if value
                        .as_str()
                        .is_some_and(|s| matches!(s, "left" | "right" | "top")) =>
                {
                    result[key] = value.clone()
                }
                "printRemaining" if value.is_boolean() => result[key] = value.clone(),
                "padding" if value.is_object() => {
                    for (side, n) in value.as_object().unwrap() {
                        if matches!(side.as_str(), "left" | "right" | "top")
                            && n.as_u64()
                                .is_some_and(|n| n <= if side == "top" { 64 } else { 32 })
                        {
                            result["padding"][side] = n.clone();
                        } else {
                            notes.push(format!("logo.padding.{side}: retained, not simulated."));
                        }
                    }
                }
                _ => notes.push(format!("logo.{key}: retained, not simulated.")),
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn restricted_sgr_roundtrip_multiline_and_style_resets() {
        for input in [
            "\x1b[31mred\nred\n",
            "\x1b[1;2;3;4;7;9;92;104mAB\nCD",
            "\x1b[38;5;123m中🙂\x1b[39m plain",
            "\x1b[38:2::12:34:56mRGB\nRGB\x1b[0m",
            "\x1b[48;2;255;0;1mBG\x1b[49m!",
            "\x1b[mreset",
            "\x1b[0m\n",
            "",
            "plain\n",
        ] {
            let art = Artwork::parse(input).unwrap();
            assert!(!art.filtered, "{input:?}");
            art.validate().unwrap();
            let reparsed = Artwork::parse(&art.ansi).unwrap();
            assert_eq!(reparsed.ansi, art.ansi);
            assert_eq!(reparsed.plain, art.plain);
            assert_eq!(art.ansi.lines().count(), art.plain.lines().count());
        }
        let art = Artwork::parse("\x1b[31mA\nB").unwrap();
        assert_eq!(art.ansi, "\x1b[31mA\x1b[0m\n\x1b[31mB\x1b[0m");
        let mut forged = art.clone();
        forged.plain = "wrong".into();
        assert!(forged.validate().is_err());
        forged = art;
        forged.ansi = "\x1b]52;c;clipboard\x07".into();
        assert!(forged.validate().is_err());
    }
    #[test]
    fn dangerous_controls_and_hidden_payloads_are_filtered() {
        for attack in [
            "\x1b]52;c;SECRET\x07",
            "\x1b]0;SECRET\x1b\\",
            "\x1bPSECRET\x1b\\",
            "\x1b_SECRET\x1b\\",
            "\u{009d}52;c;SECRET\u{009c}",
            "\x1b[2J",
            "\x1b[999A",
            "\u{009b}2J",
            "\x1b[8m",
            "\x1b[5m",
            "\x1b[38;5;999m",
            "\x1b[?25l",
            "\x1b(B",
            "\x07",
            "\u{202e}",
            "\x1b]8;;https://invalid.test\x07",
        ] {
            let art = Artwork::parse(&format!("A{attack}B")).unwrap();
            assert_eq!(art.plain, "AB", "{attack:?}");
            assert_eq!(art.ansi, "AB", "{attack:?}");
            assert!(art.filtered);
            assert!(!art.notices().is_empty());
            art.validate().unwrap();
        }
    }
    #[test]
    fn normalization_and_unicode_cell_limits_are_bounded() {
        let art = Artwork::parse("\u{feff}中\tX\r\nY").unwrap();
        assert_eq!(art.plain, "中      X\nY");
        assert!(art.expanded_tabs);
        art.validate().unwrap();
        for input in [
            "x".repeat(121),
            "中".repeat(61),
            vec!["x"; 65].join("\n"),
            "x".repeat(ART_MAX_BYTES + 1),
            "\x1b[31m".repeat(4000),
        ] {
            assert!(Artwork::parse(&input).is_err());
        }
        Artwork::parse(&vec!["x".repeat(120); 64].join("\n"))
            .unwrap()
            .validate()
            .unwrap();
    }
    #[test]
    fn placeholders_raw_dollars_and_supported_color_formats() {
        let art = marked_art(
            "$1A$2B$3C$$1$9D",
            &json!({"1":"red","2":"#abc","3":"@123","9":"bold_bg_blue"}),
            &mut Vec::new(),
        )
        .unwrap();
        assert_eq!(art.plain, "ABC$1D");
        assert!(art.ansi.contains("38;2;170;187;204"));
        assert!(art.ansi.contains("38;5;123"));
        assert!(art.ansi.contains("1;44"));
        art.validate().unwrap();
        assert_eq!(
            marked_art("$1A$$$2B", &Value::Null, &mut Vec::new())
                .unwrap()
                .plain,
            "A$B"
        );
        assert_eq!(
            text_logo(
                &json!({"type":"data-raw","source":"$1$$"}),
                None,
                &mut Vec::new()
            )
            .unwrap()
            .plain,
            "$1$$"
        );
        let mut notes = Vec::new();
        marked_art(
            "$1x",
            &json!({"1":"blink_red","2":"$(touch NEVER)","10":"red"}),
            &mut notes,
        )
        .unwrap();
        assert_eq!(notes.len(), 3);
        for invalid in [
            "#xxyyzz",
            "@256",
            "38;2;999;1;1",
            "hidden_blue",
            "\x1b[31m",
            "31m",
        ] {
            assert!(color(&json!(invalid)).is_none());
        }
    }
    #[test]
    fn import_and_export_check_encoding_aliases_conflicts_and_backups() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("logo.ans");
        std::fs::write(&path, "\x1b[32m安全\x1b[2J").unwrap();
        let art = file_art(&path).unwrap();
        assert!(art.filtered);
        art.validate().unwrap();
        let txt = dir.path().join("logo.txt");
        export_file(&txt, &art.plain, false, &None).unwrap();
        assert_eq!(file_art(&txt).unwrap().plain, art.plain);
        export_file(&txt, "updated", false, &Some(art.plain.as_bytes().to_vec())).unwrap();
        assert_eq!(
            std::fs::read_dir(dir.path().join("termimochi-backups"))
                .unwrap()
                .count(),
            1
        );
        assert!(export_file(&txt, "bad", false, &None).is_err());
        assert_eq!(std::fs::read_to_string(&txt).unwrap(), "updated");
        assert!(export_file(&dir.path().join(".bashrc"), "bad", false, &None).is_err());
        assert!(export_file(&txt, "\x1b[31mred", false, &Some(b"updated".to_vec())).is_err());
        let invalid = dir.path().join("cp437.ans");
        std::fs::write(&invalid, [0xdb, 0xdb]).unwrap();
        assert!(file_art(&invalid).unwrap_err().contains("CP437"));
        std::fs::write(&txt, "\x1b[31mred").unwrap();
        assert!(file_art(&txt).is_err());
        let alias = dir.path().join("alias.ans");
        std::os::unix::fs::symlink(&path, &alias).unwrap();
        assert!(file_art(&alias).is_err());
        let hard = dir.path().join("hard.ans");
        std::fs::hard_link(&path, &hard).unwrap();
        assert!(file_art(&hard).is_err());
        let fifo = dir.path().join("pipe.ans");
        assert!(
            std::process::Command::new("mkfifo")
                .arg(&fifo)
                .status()
                .unwrap()
                .success()
        );
        assert!(file_art(&fifo).is_err());
    }
    #[test]
    fn file_snapshots_are_contained_portable_and_never_pass_paths() {
        let dir = tempfile::tempdir().unwrap();
        let cfg = dir.path().join("config.jsonc");
        std::fs::write(dir.path().join("logo.txt"), "$1A$$\nB").unwrap();
        let source=json!({"logo":{"type":"file","source":"logo.txt","color":{"1":"red"}},"modules":["os"]}).to_string();
        let snapshot = snapshot_file(&source, &cfg).unwrap().unwrap();
        assert_eq!(snapshot.artwork.plain, "A$\nB");
        let (preview, notes) =
            crate::fastfetch_document::preview_config_with_logo(&source, Some(&snapshot)).unwrap();
        assert_eq!(preview["logo"]["type"], "data");
        assert!(!preview.to_string().contains("logo.txt"));
        assert!(!notes.is_empty());
        std::fs::write(dir.path().join("logo.txt"), "CHANGED").unwrap();
        assert_eq!(snapshot.artwork.plain, "A$\nB");
        let outside = tempfile::NamedTempFile::new().unwrap();
        for name in [
            outside.path().to_str().unwrap(),
            "$(touch NEVER)",
            "missing.txt",
            "~/.bashrc",
            "-",
            "$SECRET",
            "../logo.txt",
        ] {
            let source = json!({"logo":{"type":"file","source":name}}).to_string();
            assert!(snapshot_file(&source, &cfg).is_err(), "{name}");
            assert_eq!(
                crate::fastfetch_document::preview_config(&source)
                    .unwrap()
                    .0["logo"]["type"],
                "none"
            );
        }
        let alias = dir.path().join("link.txt");
        std::os::unix::fs::symlink(outside.path(), &alias).unwrap();
        assert!(
            snapshot_file(
                &json!({"logo":{"type":"file","source":"link.txt"}}).to_string(),
                &cfg
            )
            .is_err()
        );
        let raw = json!({"logo":{"type":"file-raw","source":"logo.txt"}}).to_string();
        std::fs::write(dir.path().join("logo.txt"), "$1raw").unwrap();
        assert_eq!(
            snapshot_file(&raw, &cfg).unwrap().unwrap().artwork.plain,
            "$1raw"
        );
    }
    #[test]
    fn builtin_small_and_padding_are_preserved_without_auto_path_expansion() {
        for kind in ["builtin", "small"] {
            let logo = json!({"type":kind,"source":"nixos","padding":{"left":2,"right":4,"top":1},"position":"right","printRemaining":true,"color":{"1":"#123456"}});
            let mut notes = Vec::new();
            let preview = preview_logo(&logo, None, &mut notes);
            assert_eq!(preview["type"], kind);
            assert_eq!(preview["padding"], logo["padding"]);
            assert!(notes.is_empty());
        }
        for kind in ["auto", "kitty", "sixel", "raw", "command-raw"] {
            let preview = preview_logo(
                &json!({"type":kind,"source":"$(touch NEVER)"}),
                None,
                &mut Vec::new(),
            );
            assert_eq!(preview["type"], "none");
            assert!(!preview.to_string().contains("NEVER"));
        }
        assert_eq!(
            preview_logo(&json!("ubuntu"), None, &mut Vec::new())["type"],
            "builtin"
        );
        assert_eq!(
            preview_logo(&json!("none"), None, &mut Vec::new())["type"],
            "none"
        );
    }
    #[test]
    fn model_colored_preview_export_persistence_and_lossless_embedding() {
        use crate::greeting::{GreetingContext, GreetingSettings, Position};
        let art = Artwork::parse("\x1b[31mA\nB\x1b[38;2;1;2;3m中").unwrap();
        let mut settings = GreetingSettings::starter();
        let items = settings.official_items.clone();
        settings.import_artwork(art.clone()).unwrap();
        assert_eq!(settings.official_items, items);
        for position in [Position::Left, Position::Right, Position::Top] {
            settings.position = position;
            assert!(
                settings
                    .render_with_official(&GreetingContext::default(), 80, Some("Fields\r\n"))
                    .contains("38;2;1;2;3")
            );
            assert_eq!(
                crate::fastfetch_document::value(&settings.fastfetch_config().unwrap()).unwrap()["logo"]
                    ["source"],
                art.ansi
            );
        }
        let stored = serde_json::to_string(&settings).unwrap();
        let loaded: GreetingSettings = serde_json::from_str(&stored).unwrap();
        loaded.validate().unwrap();
        assert_eq!(loaded, settings);
        let source = "// untouched\n{\"logo\": {\"type\":\"file\", /* logo note */ \"source\":\"logo.txt\",\"padding\":{\"right\":4}}, \"future\":7, \"modules\":[\"cpu\", {\"type\":\"command\",\"text\":\"NEVER\"}]}";
        let mut imported = GreetingSettings {
            imported_source: Some(source.into()),
            ..Default::default()
        };
        imported.import_artwork(art).unwrap();
        imported.validate().unwrap();
        let saved = imported.fastfetch_config().unwrap();
        for fragment in [
            "// untouched",
            "/* logo note */",
            "\"future\":7",
            "{\"type\":\"command\",\"text\":\"NEVER\"}",
        ] {
            assert!(saved.contains(fragment));
        }
        let value = crate::fastfetch_document::value(&saved).unwrap();
        assert_eq!(value["logo"]["type"], "data-raw");
        assert_eq!(value["logo"]["padding"]["right"], 4);
        imported.source_logo.as_mut().unwrap().descriptor = json!("wrong");
        assert!(imported.validate().is_err());
    }
}
