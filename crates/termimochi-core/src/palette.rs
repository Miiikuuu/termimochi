use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt, fs,
    io::Read,
    path::{Path, PathBuf},
    str::FromStr,
};

use crate::Rgb;

const MAX_PALETTE_BYTES: usize = 1024 * 1024;
const MAX_PALETTE_NAME_CHARS: usize = 128;

pub const REQUIRED_COLOR_KEYS: [&str; 18] = [
    "Foreground",
    "Background",
    "Color0",
    "Color1",
    "Color2",
    "Color3",
    "Color4",
    "Color5",
    "Color6",
    "Color7",
    "Color8",
    "Color9",
    "Color10",
    "Color11",
    "Color12",
    "Color13",
    "Color14",
    "Color15",
];

pub const OPTIONAL_COLOR_KEYS: [&str; 10] = [
    "Cursor",
    "CursorForeground",
    "TitlebarBackground",
    "TitlebarForeground",
    "BellForeground",
    "BellBackground",
    "SuperuserForeground",
    "SuperuserBackground",
    "RemoteForeground",
    "RemoteBackground",
];

pub const KNOWN_COLOR_KEYS: [&str; 28] = [
    "Foreground",
    "Background",
    "Color0",
    "Color1",
    "Color2",
    "Color3",
    "Color4",
    "Color5",
    "Color6",
    "Color7",
    "Color8",
    "Color9",
    "Color10",
    "Color11",
    "Color12",
    "Color13",
    "Color14",
    "Color15",
    "Cursor",
    "CursorForeground",
    "TitlebarBackground",
    "TitlebarForeground",
    "BellForeground",
    "BellBackground",
    "SuperuserForeground",
    "SuperuserBackground",
    "RemoteForeground",
    "RemoteBackground",
];

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Variant {
    Light,
    Dark,
}

impl Variant {
    pub const ALL: [Self; 2] = [Self::Light, Self::Dark];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Light => "light",
            Self::Dark => "dark",
        }
    }

    #[must_use]
    pub const fn section_name(self) -> &'static str {
        match self {
            Self::Light => "Light",
            Self::Dark => "Dark",
        }
    }
}

impl fmt::Display for Variant {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for Variant {
    type Err = PaletteError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.to_ascii_lowercase().as_str() {
            "light" => Ok(Self::Light),
            "dark" => Ok(Self::Dark),
            _ => Err(PaletteError::new(format!("unknown variant {value:?}"))),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ThemeVariant {
    kind: Variant,
    colors: BTreeMap<String, Rgb>,
    properties: BTreeMap<String, String>,
}

impl ThemeVariant {
    fn from_pairs(
        kind: Variant,
        pairs: &[(String, String)],
        line_numbers: &BTreeMap<(String, String), usize>,
    ) -> Result<Self, PaletteError> {
        let mut colors = BTreeMap::new();
        let mut properties = BTreeMap::new();

        for (key, raw_value) in pairs {
            if is_known_color_key(key) {
                let color = raw_value.parse::<Rgb>().map_err(|error| {
                    let line = line_numbers
                        .get(&(kind.section_name().to_owned(), key.clone()))
                        .copied();
                    PaletteError::at_optional_line(
                        format!("[{}] {key}: {error}", kind.section_name()),
                        line,
                    )
                })?;
                colors.insert(key.clone(), color);
            } else {
                properties.insert(key.clone(), raw_value.clone());
            }
        }

        Ok(Self {
            kind,
            colors,
            properties,
        })
    }

    #[must_use]
    pub const fn kind(&self) -> Variant {
        self.kind
    }

    #[must_use]
    pub fn get(&self, key: &str) -> Option<Rgb> {
        self.colors.get(key).copied()
    }

    pub fn set(&mut self, key: &str, color: Rgb) -> Result<(), PaletteError> {
        if !is_known_color_key(key) {
            return Err(PaletteError::new(format!("unknown color key {key:?}")));
        }
        self.colors.insert(key.to_owned(), color);
        Ok(())
    }

    #[must_use]
    pub fn colors(&self) -> &BTreeMap<String, Rgb> {
        &self.colors
    }

    #[must_use]
    pub fn missing_required_keys(&self) -> Vec<&'static str> {
        REQUIRED_COLOR_KEYS
            .iter()
            .copied()
            .filter(|key| !self.colors.contains_key(*key))
            .collect()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PtyxisPalette {
    name: String,
    variants: BTreeMap<Variant, ThemeVariant>,
    palette_properties: BTreeMap<String, String>,
    other_sections: BTreeMap<String, Vec<(String, String)>>,
    source: Option<PathBuf>,
}

impl PtyxisPalette {
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, PaletteError> {
        let path = path.as_ref();
        let file = fs::File::open(path).map_err(|error| {
            PaletteError::new(format!("cannot read {}: {error}", path.display()))
        })?;
        let mut text = String::new();
        file.take((MAX_PALETTE_BYTES + 1) as u64)
            .read_to_string(&mut text)
            .map_err(|error| {
                PaletteError::new(format!("cannot read {}: {error}", path.display()))
            })?;
        if text.len() > MAX_PALETTE_BYTES {
            return Err(PaletteError::new(format!(
                "{} is larger than the 1 MiB palette limit",
                path.display()
            )));
        }
        let mut palette = Self::from_text(&text)?;
        palette.source = Some(path.to_owned());
        Ok(palette)
    }

    pub fn from_text(text: &str) -> Result<Self, PaletteError> {
        if text.len() > MAX_PALETTE_BYTES {
            return Err(PaletteError::new("palette is larger than the 1 MiB limit"));
        }
        if text.contains('\0') {
            return Err(PaletteError::new(
                "palette contains a NUL control character",
            ));
        }
        let text = text.strip_prefix('\u{feff}').unwrap_or(text);
        let parsed = ParsedIni::parse(text)?;
        let palette_pairs = parsed
            .sections
            .get("Palette")
            .ok_or_else(|| PaletteError::new("missing [Palette] section"))?;

        let name = palette_pairs
            .iter()
            .find_map(|(key, value)| (key == "Name").then(|| value.trim()))
            .ok_or_else(|| PaletteError::new("missing [Palette] Name"))?;
        validate_palette_name(name)?;
        let name = name.to_owned();

        let palette_properties = palette_pairs
            .iter()
            .filter(|(key, _)| key != "Name")
            .cloned()
            .collect();

        let mut variants = BTreeMap::new();
        for kind in Variant::ALL {
            if let Some(pairs) = parsed.sections.get(kind.section_name()) {
                variants.insert(
                    kind,
                    ThemeVariant::from_pairs(kind, pairs, &parsed.line_numbers)?,
                );
            }
        }

        if variants.is_empty() {
            return Err(PaletteError::new("palette must contain [Light] or [Dark]"));
        }

        let other_sections = parsed
            .sections
            .into_iter()
            .filter(|(name, _)| !matches!(name.as_str(), "Palette" | "Light" | "Dark"))
            .collect();

        Ok(Self {
            name,
            variants,
            palette_properties,
            other_sections,
            source: None,
        })
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn set_name(&mut self, name: impl Into<String>) -> Result<(), PaletteError> {
        let name = name.into();
        let name = name.trim();
        validate_palette_name(name)?;
        self.name = name.to_owned();
        Ok(())
    }

    #[must_use]
    pub fn variant(&self, kind: Variant) -> Option<&ThemeVariant> {
        self.variants.get(&kind)
    }

    pub fn variant_mut(&mut self, kind: Variant) -> Option<&mut ThemeVariant> {
        self.variants.get_mut(&kind)
    }

    #[must_use]
    pub fn variants(&self) -> &BTreeMap<Variant, ThemeVariant> {
        &self.variants
    }

    #[must_use]
    pub fn source(&self) -> Option<&Path> {
        self.source.as_deref()
    }

    pub fn set_source(&mut self, source: Option<PathBuf>) {
        self.source = source;
    }

    /// Serialize to a deterministic Ptyxis-compatible GKeyFile/INI document.
    #[must_use]
    pub fn to_palette_string(&self) -> String {
        let mut output = String::new();
        output.push_str("[Palette]\nName=");
        output.push_str(&self.name);
        output.push('\n');
        for (key, value) in &self.palette_properties {
            output.push_str(key);
            output.push('=');
            output.push_str(value);
            output.push('\n');
        }

        for kind in Variant::ALL {
            let Some(variant) = self.variants.get(&kind) else {
                continue;
            };
            output.push_str("\n[");
            output.push_str(kind.section_name());
            output.push_str("]\n");

            for key in REQUIRED_COLOR_KEYS.into_iter().chain(OPTIONAL_COLOR_KEYS) {
                if let Some(color) = variant.get(key) {
                    output.push_str(key);
                    output.push('=');
                    output.push_str(&color.to_hex());
                    output.push('\n');
                }
            }
            for (key, value) in &variant.properties {
                output.push_str(key);
                output.push('=');
                output.push_str(value);
                output.push('\n');
            }
        }

        for (section, pairs) in &self.other_sections {
            output.push_str("\n[");
            output.push_str(section);
            output.push_str("]\n");
            for (key, value) in pairs {
                output.push_str(key);
                output.push('=');
                output.push_str(value);
                output.push('\n');
            }
        }
        output
    }
}

fn validate_palette_name(name: &str) -> Result<(), PaletteError> {
    if name.is_empty() {
        return Err(PaletteError::new("palette name cannot be empty"));
    }
    if name.chars().any(char::is_control) {
        return Err(PaletteError::new(
            "palette name cannot contain control characters",
        ));
    }
    if name.chars().count() > MAX_PALETTE_NAME_CHARS {
        return Err(PaletteError::new(format!(
            "palette name cannot exceed {MAX_PALETTE_NAME_CHARS} characters"
        )));
    }
    Ok(())
}

#[derive(Debug)]
struct ParsedIni {
    sections: BTreeMap<String, Vec<(String, String)>>,
    line_numbers: BTreeMap<(String, String), usize>,
}

impl ParsedIni {
    fn parse(text: &str) -> Result<Self, PaletteError> {
        let mut sections = BTreeMap::<String, Vec<(String, String)>>::new();
        let mut line_numbers = BTreeMap::new();
        let mut seen_sections = BTreeSet::new();
        let mut current_section: Option<String> = None;

        for (index, raw_line) in text.lines().enumerate() {
            let line_number = index + 1;
            let line = raw_line.trim();
            if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
                continue;
            }

            if line.starts_with('[') {
                if !line.ends_with(']') || line.len() < 3 {
                    return Err(PaletteError::at_line("invalid section header", line_number));
                }
                let name = line[1..line.len() - 1].trim();
                if name.is_empty() || name.contains(['[', ']']) {
                    return Err(PaletteError::at_line("invalid section header", line_number));
                }
                if !seen_sections.insert(name.to_owned()) {
                    return Err(PaletteError::at_line(
                        format!("duplicate section [{name}]"),
                        line_number,
                    ));
                }
                sections.insert(name.to_owned(), Vec::new());
                current_section = Some(name.to_owned());
                continue;
            }

            let section = current_section.as_ref().ok_or_else(|| {
                PaletteError::at_line("property appears before any section", line_number)
            })?;
            let (key, value) = line.split_once('=').ok_or_else(|| {
                PaletteError::at_line("expected a key=value property", line_number)
            })?;
            let key = key.trim();
            let value = value.trim();
            if key.is_empty() {
                return Err(PaletteError::at_line(
                    "property key cannot be empty",
                    line_number,
                ));
            }
            let identity = (section.clone(), key.to_owned());
            if line_numbers.insert(identity, line_number).is_some() {
                return Err(PaletteError::at_line(
                    format!("duplicate property {key:?} in [{section}]"),
                    line_number,
                ));
            }
            sections
                .get_mut(section)
                .expect("current section is always registered")
                .push((key.to_owned(), value.to_owned()));
        }

        Ok(Self {
            sections,
            line_numbers,
        })
    }
}

#[must_use]
fn is_known_color_key(key: &str) -> bool {
    KNOWN_COLOR_KEYS.contains(&key)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PaletteError {
    message: String,
    line: Option<usize>,
}

impl PaletteError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            line: None,
        }
    }

    fn at_line(message: impl Into<String>, line: usize) -> Self {
        Self {
            message: message.into(),
            line: Some(line),
        }
    }

    fn at_optional_line(message: impl Into<String>, line: Option<usize>) -> Self {
        Self {
            message: message.into(),
            line,
        }
    }
}

impl fmt::Display for PaletteError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(line) = self.line {
            write!(formatter, "line {line}: {}", self.message)
        } else {
            formatter.write_str(&self.message)
        }
    }
}

impl Error for PaletteError {}

#[cfg(test)]
mod tests {
    use super::*;

    const FOG_PAPER: &str = include_str!("../../../themes/fog-paper.palette");
    const TINY: &str = "[Palette]\nName=Tiny\nUseSystemAccent=false\n\
        [Light]\nForeground=#000000\nBackground=#ffffff\n";

    #[test]
    fn parses_dual_variant_ptyxis_palette() {
        let palette = PtyxisPalette::from_text(FOG_PAPER).unwrap();
        assert_eq!(palette.name(), "雾纸白 · Fog Paper（清晰版）");
        assert_eq!(palette.variants().len(), 2);
        let light = palette.variant(Variant::Light).unwrap();
        assert_eq!(light.get("Background"), Some(Rgb::new(0xE7, 0xE8, 0xE5)));
        assert!(light.missing_required_keys().is_empty());
    }

    #[test]
    fn parses_and_round_trips_metadata() {
        let input = format!("{TINY}FutureSetting=keep-me\n[Extra]\nAnswer=42\n");
        let palette = PtyxisPalette::from_text(&input).unwrap();
        assert_eq!(palette.name(), "Tiny");
        assert_eq!(
            palette.variant(Variant::Light).unwrap().get("Background"),
            Some(Rgb::new(255, 255, 255))
        );

        let serialized = palette.to_palette_string();
        assert!(serialized.contains("FutureSetting=keep-me"));
        assert!(serialized.contains("[Extra]\nAnswer=42"));
        assert_eq!(
            PtyxisPalette::from_text(&serialized).unwrap().name(),
            "Tiny"
        );
    }

    #[test]
    fn rejects_missing_palette_section() {
        let error = PtyxisPalette::from_text("[Light]\nForeground=#000000\n").unwrap_err();
        assert!(error.to_string().contains("missing [Palette]"));
    }

    #[test]
    fn rejects_invalid_color_with_context() {
        let error =
            PtyxisPalette::from_text("[Palette]\nName=Broken\n[Light]\nForeground=tomato\n")
                .unwrap_err();
        assert!(error.to_string().contains("[Light] Foreground"));
        assert!(error.to_string().contains("invalid RGB hex color"));
    }

    #[test]
    fn rejects_duplicate_keys() {
        let error = PtyxisPalette::from_text(
            "[Palette]\nName=One\nName=Two\n[Light]\nForeground=#000000\n",
        )
        .unwrap_err();
        assert!(error.to_string().contains("duplicate property"));
    }

    #[test]
    fn accepts_utf8_bom() {
        let palette = PtyxisPalette::from_text(&format!("\u{feff}{TINY}")).unwrap();
        assert_eq!(palette.name(), "Tiny");
    }

    #[test]
    fn rejects_nul_and_unsafe_names() {
        for input in [
            "[Palette]\nName=Tiny\0Hidden\n[Light]\nForeground=#000000\n",
            "[Palette]\nName=Tiny\tHidden\n[Light]\nForeground=#000000\n",
        ] {
            assert!(PtyxisPalette::from_text(input).is_err());
        }

        let mut palette = PtyxisPalette::from_text(TINY).unwrap();
        for name in ["", "   ", "Line 1\n[Injected]", "Name\0Hidden"] {
            assert!(palette.set_name(name).is_err(), "accepted {name:?}");
        }
        assert!(palette.set_name("A".repeat(129)).is_err());
        assert_eq!(palette.name(), "Tiny");
    }

    #[test]
    fn set_name_trims_outer_whitespace() {
        let mut palette = PtyxisPalette::from_text(TINY).unwrap();
        palette.set_name("  Soft Blue  ").unwrap();
        assert_eq!(palette.name(), "Soft Blue");
    }

    #[test]
    fn rejects_oversized_documents() {
        let text = format!("{TINY}Unknown={}", "x".repeat(MAX_PALETTE_BYTES));
        let error = PtyxisPalette::from_text(&text).unwrap_err();
        assert!(error.to_string().contains("1 MiB"));
    }
}
