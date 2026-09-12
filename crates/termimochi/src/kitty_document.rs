//! Lossless Kitty source container; only validated declarative appearance keys
//! are projected to a controlled session. Includes/actions remain inert source.
use std::collections::BTreeMap;

#[derive(Clone, Debug)]
pub(crate) struct KittyDocument {
    source: String,
    properties: BTreeMap<String, String>,
    pub notices: Vec<String>,
}

pub(crate) fn category(key: &str) -> Option<usize> {
    if matches!(
        key,
        "foreground"
            | "background"
            | "cursor"
            | "cursor_text_color"
            | "selection_foreground"
            | "selection_background"
    ) || key
        .strip_prefix("color")
        .is_some_and(|n| n.parse::<u8>().is_ok_and(|n| n < 16))
    {
        Some(0)
    } else if matches!(
        key,
        "font_family"
            | "font_size"
            | "bold_font"
            | "italic_font"
            | "bold_italic_font"
            | "modify_font"
    ) {
        Some(1)
    } else if matches!(
        key,
        "window_padding_width"
            | "window_margin_width"
            | "initial_window_width"
            | "initial_window_height"
            | "cursor_shape"
            | "cursor_blink_interval"
            | "tab_bar_style"
            | "tab_bar_edge"
            | "tab_bar_min_tabs"
            | "active_tab_foreground"
            | "active_tab_background"
            | "inactive_tab_foreground"
            | "inactive_tab_background"
            | "wayland_titlebar_color"
            | "remember_window_size"
    ) {
        Some(2)
    } else {
        None
    }
}

fn valid(key: &str, value: &str) -> bool {
    if value.is_empty() || value.len() > 512 || value.chars().any(char::is_control) {
        return false;
    }
    match key {
        "font_family" | "bold_font" | "italic_font" | "bold_italic_font" => {
            !value.contains(['$', '`', '\\'])
        }
        "font_size" => value
            .parse::<f64>()
            .is_ok_and(|v| v.is_finite() && (4.0..=96.0).contains(&v)),
        "window_padding_width" | "window_margin_width" => {
            value.split_whitespace().count() <= 4
                && value.split_whitespace().all(|v| {
                    v.parse::<f64>()
                        .is_ok_and(|v| v.is_finite() && (0.0..=200.0).contains(&v))
                })
        }
        "initial_window_width" | "initial_window_height" => value
            .trim_end_matches('c')
            .parse::<u32>()
            .is_ok_and(|v| (1..=8192).contains(&v)),
        "cursor_shape" => matches!(value, "block" | "beam" | "underline"),
        "cursor_blink_interval" => value
            .parse::<f64>()
            .is_ok_and(|v| v.is_finite() && (-1.0..=10.0).contains(&v)),
        "tab_bar_style" => matches!(
            value,
            "fade" | "slant" | "separator" | "powerline" | "hidden"
        ),
        "remember_window_size" => matches!(value, "yes" | "no"),
        "tab_bar_edge" => matches!(value, "top" | "bottom"),
        "tab_bar_min_tabs" => value.parse::<u8>().is_ok_and(|n| (1..=16).contains(&n)),
        "wayland_titlebar_color" => {
            crate::layout::window_top::TitlebarColor::parse(value).is_some()
        }
        "active_tab_foreground"
        | "active_tab_background"
        | "inactive_tab_foreground"
        | "inactive_tab_background" => crate::layout::window_top::color(value).is_ok(),
        "modify_font" => {
            let mut p = value.split_whitespace();
            matches!(p.next(), Some("cell_width" | "cell_height"))
                && p.next().is_some_and(|v| {
                    v.trim_end_matches('%')
                        .parse::<f64>()
                        .is_ok_and(|v| v.is_finite() && (50.0..=200.0).contains(&v))
                })
                && p.next().is_none()
        }
        _ if category(key) == Some(0) => value.parse::<termimochi_core::Rgb>().is_ok(),
        _ => false,
    }
}

impl KittyDocument {
    pub fn parse(source: &str) -> Result<Self, String> {
        if source.len() > 1024 * 1024 || source.contains('\0') {
            return Err("Invalid or oversized Kitty source.".into());
        }
        let mut properties = BTreeMap::new();
        let mut notices = Vec::new();
        for (index, line) in source.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let Some((key, value)) = line.split_once(char::is_whitespace) else {
                notices.push(format!(
                    "Kitty line {} is retained but not projected.",
                    index + 1
                ));
                continue;
            };
            let value = value.trim();
            if category(key).is_some() && valid(key, value) {
                // modify_font permits independent repeated axes. Preserve both.
                let map_key = if key == "modify_font" {
                    format!("modify_font {}", value.split_whitespace().next().unwrap())
                } else {
                    key.into()
                };
                properties.insert(map_key, value.into());
            } else {
                // An unreadable later override is not permission to claim the
                // earlier value is still effective (notably expanded includes).
                // Retain source, but leave this field inherited/unverified.
                if category(key).is_some() {
                    let map_key = if key == "modify_font" {
                        format!(
                            "modify_font {}",
                            value.split_whitespace().next().unwrap_or("")
                        )
                    } else {
                        key.into()
                    };
                    properties.remove(&map_key);
                }
                notices.push(format!("Kitty line {} · {key}: retained in source, not executed by this controlled appearance adapter.", index + 1));
            }
        }
        if source.trim().is_empty() {
            return Err("No Kitty settings found.".into());
        }
        Ok(Self {
            source: source.into(),
            properties,
            notices,
        })
    }
    #[cfg(test)]
    pub fn source(&self) -> &str {
        &self.source
    }
    pub fn properties(&self) -> &BTreeMap<String, String> {
        &self.properties
    }
    /// Read-only bridge. Missing native settings borrow preview values; they do
    /// not become exported native fields merely because the editor needs them.
    pub fn project_workspace(
        &self,
        reference: &crate::workspace::Workspace,
    ) -> Result<crate::workspace::Workspace, String> {
        let mut result = reference.clone();
        crate::layout::window_top::reset_reference(&mut result.layout);
        let mut palette = result.palette()?;
        let variant = result.variant();
        for (key, value) in &self.properties {
            if let Some(palette_key) = palette_key(key) {
                palette
                    .variant_mut(variant)
                    .ok_or("Missing preview palette variant.")?
                    .set(
                        &palette_key,
                        value
                            .parse()
                            .map_err(|e: termimochi_core::ColorParseError| e.to_string())?,
                    )
                    .map_err(|e| e.to_string())?;
            }
            match key.as_str() {
                "font_family" => result.typography.family = value.clone(),
                "font_size" => {
                    result.typography.size = value.parse::<f64>().unwrap().clamp(
                        crate::typography::MIN_FONT_SIZE,
                        crate::typography::MAX_FONT_SIZE,
                    )
                }
                "modify_font cell_height" => {
                    result.typography.line_height = (value
                        .split_whitespace()
                        .nth(1)
                        .unwrap()
                        .trim_end_matches('%')
                        .parse::<f64>()
                        .unwrap()
                        / 100.0)
                        .clamp(1.0, 2.0)
                }
                "modify_font cell_width" => {
                    result.typography.cell_width = (value
                        .split_whitespace()
                        .nth(1)
                        .unwrap()
                        .trim_end_matches('%')
                        .parse::<f64>()
                        .unwrap()
                        / 100.0)
                        .clamp(1.0, 2.0)
                }
                "window_padding_width" => {
                    result.layout.content_padding = value
                        .split_whitespace()
                        .next()
                        .unwrap()
                        .parse::<f64>()
                        .unwrap()
                        .round()
                        .clamp(0.0, 24.0) as i32
                }
                "window_margin_width" => {
                    result.layout.window_spacing = value
                        .split_whitespace()
                        .next()
                        .unwrap()
                        .parse::<f64>()
                        .unwrap()
                        .round()
                        .clamp(0.0, 32.0) as i32
                }
                "initial_window_width" if value.ends_with('c') => {
                    result.layout.columns = value
                        .trim_end_matches('c')
                        .parse::<usize>()
                        .unwrap()
                        .clamp(crate::layout::MIN_COLUMNS, crate::layout::MAX_COLUMNS)
                }
                "initial_window_height" if value.ends_with('c') => {
                    result.layout.rows = value
                        .trim_end_matches('c')
                        .parse::<usize>()
                        .unwrap()
                        .clamp(crate::layout::MIN_ROWS, crate::layout::MAX_ROWS)
                }
                "cursor_shape" => {
                    result.layout.cursor_shape = match value.as_str() {
                        "beam" => crate::layout::PreviewCursorShape::IBeam,
                        "underline" => crate::layout::PreviewCursorShape::Underline,
                        _ => crate::layout::PreviewCursorShape::Block,
                    }
                }
                "cursor_blink_interval" => {
                    result.layout.cursor_blink = if value == "0" {
                        crate::layout::PreviewCursorBlink::Off
                    } else if value.starts_with('-') {
                        crate::layout::PreviewCursorBlink::System
                    } else {
                        crate::layout::PreviewCursorBlink::On
                    }
                }
                "tab_bar_style" => {
                    result.layout.tab_bar = value != "hidden";
                    result.layout.tab_style =
                        crate::layout::window_top::TabStyle::parse(value).unwrap_or_default();
                }
                "tab_bar_edge" => {
                    result.layout.tab_edge = if value == "top" {
                        crate::layout::window_top::TabEdge::Top
                    } else {
                        crate::layout::window_top::TabEdge::Bottom
                    }
                }
                "tab_bar_min_tabs" => result.layout.tab_min_tabs = value.parse().unwrap(),
                "wayland_titlebar_color" => {
                    result.layout.titlebar_color =
                        crate::layout::window_top::TitlebarColor::parse(value).unwrap()
                }
                "active_tab_foreground" => {
                    result.layout.tab_active_fg = crate::layout::window_top::color(value).unwrap()
                }
                "active_tab_background" => {
                    result.layout.tab_active_bg = crate::layout::window_top::color(value).unwrap()
                }
                "inactive_tab_foreground" => {
                    result.layout.tab_inactive_fg = crate::layout::window_top::color(value).unwrap()
                }
                "inactive_tab_background" => {
                    result.layout.tab_inactive_bg = crate::layout::window_top::color(value).unwrap()
                }
                _ => {}
            }
        }
        result.palette = palette.to_palette_string();
        Ok(result)
    }
    /// Explicit changes are measured against this session's immutable opening
    /// projection, never against guessed defaults. Only changed missing fields
    /// are added; an untouched native font_size 50 retains its exact bytes even
    /// though the bounded GTK reference displays 24.
    pub fn reconcile_with_reference(
        &self,
        current: &crate::workspace::Workspace,
        reference: &crate::workspace::Workspace,
    ) -> Result<String, String> {
        let now = workspace_properties(current)?;
        let before = workspace_properties(reference)?;
        let edits = now
            .into_iter()
            .filter(|(key, value)| before.get(key) != Some(value))
            .collect();
        self.edited(&edits)
    }
    pub fn ownership(&self) -> (bool, bool, bool) {
        let has = |n| {
            self.properties
                .keys()
                .any(|k| category(k.split_whitespace().next().unwrap()) == Some(n))
        };
        (has(0), has(1), has(2))
    }
    pub fn safe_configuration(&self) -> String {
        self.properties
            .iter()
            .map(|(k, v)| format!("{} {v}\n", k.split_whitespace().next().unwrap()))
            .collect()
    }
    pub fn edited(&self, edits: &BTreeMap<String, String>) -> Result<String, String> {
        for (key, value) in edits {
            if !valid(key.split_whitespace().next().unwrap_or(""), value) {
                return Err(format!("Unsupported Kitty edit: {key}"));
            }
        }
        let mut remaining = edits.clone();
        let mut result = String::new();
        for line in self.source.split_inclusive('\n') {
            let mut words = line.split_whitespace();
            let key = words.next().unwrap_or("");
            let map_key = if key == "modify_font" {
                format!("modify_font {}", words.next().unwrap_or(""))
            } else {
                key.into()
            };
            if let Some(value) = edits.get(&map_key) {
                // All repeated occurrences of an explicitly edited scalar get
                // the same value; untouched repeat/include/comment bytes survive.
                result.push_str(&format!("{key} {value}\n"));
                remaining.remove(&map_key);
            } else {
                result.push_str(line);
            }
        }
        if !remaining.is_empty() && !result.ends_with('\n') {
            result.push('\n');
        }
        for (key, value) in remaining {
            result.push_str(&format!(
                "{} {value}\n",
                key.split_whitespace().next().unwrap_or("")
            ));
        }
        Ok(result)
    }
}

pub(crate) fn workspace_properties(
    workspace: &crate::workspace::Workspace,
) -> Result<BTreeMap<String, String>, String> {
    use crate::layout::{PreviewCursorBlink as B, PreviewCursorShape as S};
    let mut values = BTreeMap::new();
    let palette = workspace.palette()?;
    let variant = palette
        .variant(workspace.variant())
        .ok_or("Missing palette variant.")?;
    for key in ["foreground", "background", "cursor", "cursor_text_color"]
        .into_iter()
        .map(String::from)
        .chain((0..16).map(|n| format!("color{n}")))
    {
        if let Some(color) = palette_key(&key).and_then(|key| variant.get(&key)) {
            values.insert(key, color.to_hex());
        }
    }
    let f = &workspace.typography;
    let l = workspace.layout;
    for (key, value) in [
        ("font_family", f.family.clone()),
        ("font_size", f.size.to_string()),
        (
            "modify_font cell_width",
            format!("cell_width {}%", f.cell_width * 100.0),
        ),
        (
            "modify_font cell_height",
            format!("cell_height {}%", f.line_height * 100.0),
        ),
        ("window_padding_width", l.content_padding.to_string()),
        ("window_margin_width", l.window_spacing.to_string()),
        ("initial_window_width", format!("{}c", l.columns)),
        ("initial_window_height", format!("{}c", l.rows)),
        (
            "cursor_shape",
            match l.cursor_shape {
                S::Block => "block",
                S::IBeam => "beam",
                S::Underline => "underline",
            }
            .into(),
        ),
        (
            "cursor_blink_interval",
            match l.cursor_blink {
                B::Off => "0",
                B::On => "0.5",
                B::System => "-1",
            }
            .into(),
        ),
        (
            "tab_bar_style",
            if l.tab_bar { "fade" } else { "hidden" }.into(),
        ),
    ] {
        values.insert(key.into(), value);
    }
    values.extend(crate::layout::window_top::properties(&workspace.layout));
    Ok(values)
}

pub(crate) fn palette_key(key: &str) -> Option<String> {
    match key {
        "foreground" => Some("Foreground".into()),
        "background" => Some("Background".into()),
        "cursor" => Some("Cursor".into()),
        "cursor_text_color" => Some("CursorForeground".into()),
        _ => key
            .strip_prefix("color")
            .filter(|n| n.parse::<u8>().is_ok_and(|n| n < 16))
            .map(|n| format!("Color{n}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn source_repeat_and_unknowns_survive_without_running_includes() {
        let source = "# imported\nbackground #ffffff\ninclude ../daily.conf\nmap f1 launch touch /sentinel\nbackground #000000\n";
        let doc = KittyDocument::parse(source).unwrap();
        assert_eq!(doc.source(), source);
        assert_eq!(doc.ownership(), (true, false, false));
        assert_eq!(doc.properties()["background"], "#000000");
        assert!(!doc.safe_configuration().contains("include"));
        assert!(!doc.safe_configuration().contains("sentinel"));
        assert_eq!(doc.edited(&BTreeMap::new()).unwrap(), source);
        let edited = doc
            .edited(&BTreeMap::from([("background".into(), "#aabbcc".into())]))
            .unwrap();
        assert_eq!(edited.matches("background #aabbcc").count(), 2);
        assert!(edited.contains("include ../daily.conf\nmap f1 launch touch /sentinel\n"));
    }
    #[test]
    fn missing_fields_stay_missing_and_injection_is_not_projected() {
        let doc = KittyDocument::parse("font_size 14\nfont_family $(touch x)\nwatcher /evil.py\n")
            .unwrap();
        assert_eq!(doc.ownership(), (false, true, false));
        assert_eq!(doc.safe_configuration(), "font_size 14\n");
        assert!(
            doc.edited(&BTreeMap::from([(
                "font_size".into(),
                "14\ninclude x".into()
            )]))
            .is_err()
        );
    }

    #[test]
    fn sparse_native_edit_never_writes_reference_defaults_and_clamps_only_preview() {
        let reference = crate::workspace::Workspace::new(
            &termimochi_core::PtyxisPalette::from_text(include_str!(
                "../resources/themes/fog-paper.palette"
            ))
            .unwrap(),
            termimochi_core::Variant::Light,
            Default::default(),
            Default::default(),
            Default::default(),
            None,
            true,
        );
        let source = "# keep exactly\nbackground #abcdef\nfont_size 50\nmodify_font cell_width 120%\nmodify_font cell_height 130%\ninclude ../outside\n";
        let doc = KittyDocument::parse(source).unwrap();
        let baseline = doc.project_workspace(&reference).unwrap();
        assert_eq!(baseline.typography.size, 24.0);
        assert_eq!(
            doc.reconcile_with_reference(&baseline, &baseline).unwrap(),
            source
        );
        let mut edited = baseline.clone();
        let mut palette = edited.palette().unwrap();
        palette
            .variant_mut(edited.variant())
            .unwrap()
            .set("Background", "#010203".parse().unwrap())
            .unwrap();
        edited.palette = palette.to_palette_string();
        edited.typography.cell_width = 1.4;
        let output = doc.reconcile_with_reference(&edited, &baseline).unwrap();
        assert!(output.contains("background #010203"));
        assert!(output.contains("font_size 50"));
        assert!(output.contains("modify_font cell_width 140%\nmodify_font cell_height 130%"));
        assert!(!output.contains("font_family"));
        assert!(!output.contains("foreground"));
        edited.typography.family = "Liberation Mono".into();
        let added = doc.reconcile_with_reference(&edited, &baseline).unwrap();
        assert!(added.contains("font_family Liberation Mono"));
        assert!(added.contains("include ../outside"));
    }
}
