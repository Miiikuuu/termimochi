//! Reviewed field edits, shared by the designer, native preview and export.
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct FieldStyle {
    pub label: Option<String>,
    pub icon: Option<String>,
    pub key_color: Option<u8>,
    pub value_color: Option<u8>,
    pub format: Option<String>,
}

pub(crate) fn module_kind(module: &Value) -> &str {
    module
        .as_str()
        .or_else(|| module.get("type").and_then(Value::as_str))
        .unwrap_or("unknown")
}

pub(crate) fn editable(kind: &str) -> bool {
    matches!(
        kind,
        "os" | "kernel"
            | "shell"
            | "terminal"
            | "cpu"
            | "gpu"
            | "memory"
            | "disk"
            | "uptime"
            | "datetime"
            | "host"
            | "packages"
            | "display"
            | "de"
            | "wm"
            | "wmtheme"
            | "theme"
            | "icons"
            | "terminalfont"
            | "font"
            | "locale"
    )
}

pub(crate) fn formats(kind: &str) -> Vec<(&'static str, &'static str)> {
    match kind {
        "cpu" => vec![
            ("Name only", "{name}"),
            (
                "Cores & frequency",
                "{cores-physical}C / {cores-logical}T @ {freq-max}",
            ),
        ],
        "gpu" => vec![
            ("Name only", "{name}"),
            ("Name & driver", "{name} · {driver}"),
        ],
        "memory" => vec![
            ("Used / total", "{used} / {total}"),
            ("Percentage", "{percentage}"),
            ("Usage bar", "{percentage-bar}"),
            ("Bar & values", "{percentage-bar} {used} / {total}"),
        ],
        "disk" => vec![
            ("Used / total", "{size-used} / {size-total}"),
            ("Percentage", "{size-percentage}"),
            ("Usage bar", "{size-percentage-bar}"),
            (
                "Bar & values",
                "{size-percentage-bar} {size-used} / {size-total}",
            ),
        ],
        "os" => vec![
            ("Name only", "{name}"),
            ("Name & version", "{name} {version}"),
        ],
        "shell" | "terminal" => vec![
            ("Name only", "{pretty-name}"),
            ("Name & version", "{pretty-name} {version}"),
        ],
        "datetime" => vec![
            ("Date", "{year}-{month-pretty}-{day-pretty}"),
            ("Time", "{hour-pretty}:{minute-pretty}:{second-pretty}"),
        ],
        _ => vec![],
    }
}

pub(crate) fn safe_text(text: &str, limit: usize) -> bool {
    text.chars().count() <= limit
        && !text.chars().any(|ch| {
            ch.is_control() || matches!(ch, '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
        })
}

pub(crate) fn color_code(slot: u8) -> u8 {
    match slot {
        0..=7 => 30 + slot,
        8..=15 => 90 + slot - 8,
        _ => 39,
    }
}

impl FieldStyle {
    pub fn validate(&self, kind: &str) -> Result<(), String> {
        if !editable(kind) {
            return Err("This field is preserved read-only.".into());
        }
        if self.label.as_ref().is_some_and(|s| !safe_text(s, 64))
            || self.icon.as_ref().is_some_and(|s| !safe_text(s, 8))
            || [self.key_color, self.value_color]
                .into_iter()
                .flatten()
                .any(|c| c > 16)
            || self
                .format
                .as_ref()
                .is_some_and(|s| !formats(kind).iter().any(|(_, f)| s == f))
        {
            return Err("Use a short label/icon, a theme color and a supported display format. Control characters are not allowed.".into());
        }
        Ok(())
    }
    pub fn apply(&self, module: &mut Value) -> Result<(), String> {
        let kind = module_kind(module).to_owned();
        self.validate(&kind)?;
        if module.is_string() {
            *module = json!({"type":kind});
        }
        if self.label.is_some() || self.icon.is_some() {
            let label = self
                .label
                .as_deref()
                .or_else(|| module["key"].as_str())
                .unwrap_or(&kind);
            let icon = self.icon.as_deref().unwrap_or("");
            let key = if icon.is_empty() {
                label.to_owned()
            } else {
                format!("{icon} {label}")
            };
            module["key"] = json!(key.replace('{', "{{"));
        }
        if let Some(slot) = self.key_color {
            module["keyColor"] = json!(color_code(slot).to_string());
        }
        if let Some(slot) = self.value_color {
            module["outputColor"] = json!(color_code(slot).to_string());
        }
        if let Some(format) = &self.format {
            module["format"] = json!(format);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn edits_only_touch_reviewed_properties_and_preserve_module_options() {
        let mut module =
            json!({"type":"disk", "folders":"/data", "percent":{"type":9}, "future":true});
        let style = FieldStyle {
            label: Some("Storage".into()),
            icon: Some("▣".into()),
            key_color: Some(4),
            value_color: Some(16),
            format: Some("{size-percentage-bar}".into()),
        };
        style.apply(&mut module).unwrap();
        assert_eq!(module["folders"], "/data");
        assert_eq!(module["future"], true);
        assert_eq!(module["key"], "▣ Storage");
        assert_eq!(module["keyColor"], "34");
        assert_eq!(module["outputColor"], "39");
        assert!(style.validate("command").is_err());
    }
    #[test]
    fn controls_oversized_fields_and_unreviewed_formats_are_rejected() {
        for label in [
            "\x1b]52;c;bad\x07".into(),
            "x".repeat(65),
            "a\u{202e}b".into(),
        ] {
            assert!(
                FieldStyle {
                    label: Some(label),
                    ..Default::default()
                }
                .validate("cpu")
                .is_err()
            );
        }
        assert!(
            FieldStyle {
                format: Some("{$SECRET}".into()),
                ..Default::default()
            }
            .validate("cpu")
            .is_err()
        );
        assert!(
            FieldStyle {
                key_color: Some(99),
                ..Default::default()
            }
            .validate("cpu")
            .is_err()
        );
    }
}
