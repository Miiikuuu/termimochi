//! Glyph diagnostics for characters actually fed as prompt text, not a generic
//! Nerd Font score or symbols belonging to inactive configuration modules.
use crate::starship_modules::{MODULES, ModuleSpec};
use gtk::{
    glib,
    pango::{self, prelude::*},
};
use std::collections::{BTreeMap, BTreeSet};
use termimochi_core::{Issue, Severity};

#[derive(Clone, Debug, PartialEq)]
struct CheckKey {
    font: String,
    font_map_serial: u32,
    characters: Vec<(char, String)>,
}

#[derive(Default)]
pub(crate) struct PromptDiagnostics {
    key: Option<CheckKey>,
    issues: Vec<Issue>,
}

impl PromptDiagnostics {
    pub(crate) fn check(
        &mut self,
        context: &pango::Context,
        font: &pango::FontDescription,
        used: &BTreeSet<char>,
        source: Option<&str>,
    ) -> Vec<Issue> {
        let key = CheckKey {
            font: font.to_string().to_string(),
            font_map_serial: context.font_map().map_or(0, |map| map.serial()),
            characters: requirements(used, source),
        };
        if self.key.as_ref() == Some(&key) {
            return self.issues.clone();
        }
        self.issues.clear();
        if !key.characters.is_empty() {
            if let Some(face) = context.load_font(font) {
                let layout = pango::Layout::new(context);
                layout.set_font_description(Some(font));
                let family = font.family().unwrap_or_else(|| "selected font".into());
                let mut failures: BTreeMap<(String, bool), Vec<char>> = BTreeMap::new();
                for (ch, module) in &key.characters {
                    let own_glyph = face.has_char(*ch);
                    // Normal CJK/emoji fallback is expected; don't call it a
                    // failure just because the monospace face lacks the glyph.
                    layout.set_text(&ch.to_string());
                    if let Some(missing) =
                        classify(*ch, own_glyph, layout.unknown_glyphs_count() > 0)
                    {
                        failures
                            .entry((module.clone(), missing))
                            .or_default()
                            .push(*ch);
                    }
                }
                self.issues = failures
                    .into_iter()
                    .map(|((module, missing), chars)| {
                        glyph_issue(&module, &chars, missing, &family)
                    })
                    .collect();
            } else {
                self.issues.push(Issue {
                    severity: Severity::Warning, code: "prompt-font-unavailable",
                    message: "Prompt · Font check unavailable\nThe selected preview font could not be loaded. Open Fonts to choose an installed terminal font.".into(),
                    variant: None, ratio: None,
                });
            }
        }
        self.key = Some(key);
        self.issues.clone()
    }
}

pub(crate) fn is_prompt_character(ch: char) -> bool {
    !ch.is_ascii() && !ch.is_whitespace() && !ch.is_control() && !glib::Unichar::is_zero_width(ch)
}

fn private_use(ch: char) -> bool {
    matches!(ch, '\u{e000}'..='\u{f8ff}' | '\u{f0000}'..='\u{ffffd}' | '\u{100000}'..='\u{10fffd}')
}

/// Some(true): missing even with fallback. Some(false): PUA requires fallback.
fn classify(ch: char, primary: bool, unknown: bool) -> Option<bool> {
    if unknown {
        Some(true)
    } else if private_use(ch) && !primary {
        Some(false)
    } else {
        None
    }
}

fn requirements(used: &BTreeSet<char>, source: Option<&str>) -> Vec<(char, String)> {
    if used.is_empty() {
        return Vec::new();
    }
    let config = source.and_then(|source| toml::from_str::<toml::Table>(source).ok());
    used.iter()
        .copied()
        .filter(|ch| is_prompt_character(*ch))
        .take(128)
        .map(|ch| {
            let mut owners = BTreeSet::new();
            if let Some(config) = &config {
                for spec in MODULES {
                    let module = config.get(spec.id).and_then(toml::Value::as_table);
                    if module
                        .and_then(|m| m.get("disabled"))
                        .and_then(toml::Value::as_bool)
                        .unwrap_or(spec.disabled)
                    {
                        continue;
                    }
                    let contains = spec.symbols.iter().any(|field| {
                        module
                            .and_then(|m| m.get(field.key))
                            .and_then(toml::Value::as_str)
                            .unwrap_or(field.default)
                            .contains(ch)
                    }) || module
                        .and_then(|m| m.get("format"))
                        .and_then(toml::Value::as_str)
                        .is_some_and(|f| f.contains(ch));
                    if contains {
                        owners.insert(spec.label);
                    }
                }
            }
            let owner = if owners.len() == 1 {
                *owners.first().unwrap()
            } else {
                "Prompt"
            };
            (ch, owner.to_owned())
        })
        .collect()
}

fn glyph_issue(module: &str, chars: &[char], missing: bool, family: &str) -> Issue {
    let codes = chars
        .iter()
        .take(4)
        .map(|ch| format!("U+{:04X}", *ch as u32))
        .collect::<Vec<_>>()
        .join(", ");
    let codes = if chars.len() > 4 {
        format!("{codes}, +{} more", chars.len() - 4)
    } else {
        codes
    };
    let title = if missing {
        "Missing glyph"
    } else {
        "Icon uses fallback"
    };
    let repair = if MODULES.iter().any(|m| m.label == module) {
        format!(
            "Choose a compatible font, or open Edit to customize {module} in your copy using plain text."
        )
    } else {
        "Open Fonts to choose a compatible terminal font.".into()
    };
    let detail = if missing {
        format!(
            "{codes} cannot be rendered with the current preview font ({family}) or its available fallback fonts. {repair}"
        )
    } else {
        format!(
            "{family} does not contain {codes}; an available fallback font renders it here. Other terminals may show a different icon or a missing-glyph box. {repair}"
        )
    };
    Issue {
        severity: if missing {
            Severity::Warning
        } else {
            Severity::Note
        },
        code: match (module == "Rust", missing) {
            (true, true) => "rust-symbol-missing",
            (true, false) => "rust-symbol-fallback",
            (false, true) => "prompt-glyph-missing",
            (false, false) => "prompt-icon-fallback",
        },
        message: format!("{module} · {title} ({codes})\n{detail}"),
        variant: None,
        ratio: None,
    }
}

pub(crate) fn is_font_issue(issue: &Issue) -> bool {
    matches!(
        issue.code,
        "rust-symbol-missing"
            | "rust-symbol-fallback"
            | "prompt-glyph-missing"
            | "prompt-icon-fallback"
            | "prompt-font-unavailable"
    )
}
pub(crate) fn issue_module(issue: &Issue) -> Option<&'static ModuleSpec> {
    if !is_font_issue(issue) {
        return None;
    }
    let (label, _) = issue.message.split_once(" · ")?;
    MODULES.iter().find(|module| module.label == label)
}
#[cfg(test)]
pub(crate) fn is_rust_issue(issue: &Issue) -> bool {
    matches!(issue.code, "rust-symbol-missing" | "rust-symbol-fallback")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn active_rust_glyph_is_identified_and_unused_symbols_are_ignored() {
        let source = "[rust]\nsymbol = ' \u{e7a8} '\n[python]\nsymbol = ' \u{e73c} '\n";
        let used = "\u{e7a8}\u{e7a8} rs".chars().collect();
        assert_eq!(
            requirements(&used, Some(source)),
            vec![('\u{e7a8}', "Rust".into())]
        );
        assert!(requirements(&BTreeSet::new(), Some(source)).is_empty());
    }
    #[test]
    fn generic_diagnostics_identify_non_symbol_fields_and_default_icons() {
        for (source, glyph, label, id) in [
            (
                "[git_status]\nmodified='\u{10fffd}'",
                '\u{10fffd}',
                "Git Status",
                "git_status",
            ),
            (
                "[directory]\nread_only='\u{10fffd}'",
                '\u{10fffd}',
                "Directory",
                "directory",
            ),
            (
                "[character]\nerror_symbol='[\u{10fffd}](red)'",
                '\u{10fffd}',
                "Prompt Symbol",
                "character",
            ),
            (
                "[python]\nsymbol='\u{10fffd}'",
                '\u{10fffd}',
                "Python",
                "python",
            ),
            ("# defaults", '\u{e0a0}', "Git Branch", "git_branch"),
        ] {
            assert_eq!(
                requirements(&[glyph].into_iter().collect(), Some(source)),
                vec![(glyph, label.into())]
            );
            assert_eq!(
                issue_module(&glyph_issue(label, &[glyph], true, "Monospace"))
                    .unwrap()
                    .id,
                id
            );
        }
        assert!(issue_module(&glyph_issue("Prompt", &['\u{10fffd}'], true, "Monospace")).is_none());
    }
    #[test]
    fn actual_coverage_not_font_name_decides_compatibility() {
        assert_eq!(classify('\u{e7a8}', true, false), None);
        assert_eq!(classify('\u{e7a8}', false, true), Some(true));
        assert_eq!(classify('\u{e7a8}', false, false), Some(false));
        assert_eq!(classify('🦀', false, false), None);
        assert_eq!(classify('中', false, false), None);
        assert_eq!(classify('🦀', false, true), Some(true));
    }
    #[test]
    fn missing_and_fallback_reports_are_distinct_and_actionable() {
        let missing = glyph_issue("Rust", &['\u{e7a8}'], true, "Monospace");
        assert_eq!(missing.severity, Severity::Warning);
        assert!(missing.message.contains("U+E7A8"));
        assert!(missing.message.contains("plain text"));
        assert!(is_rust_issue(&missing));
        let fallback = glyph_issue("Prompt", &['\u{e0b0}'], false, "Monospace");
        assert_eq!(fallback.severity, Severity::Note);
        assert!(fallback.message.contains("renders it here"));
        assert!(!is_rust_issue(&fallback));
    }
    #[test]
    fn formatting_controls_and_ambiguous_module_ownership_are_not_misreported() {
        for ch in ['\u{200d}', '\u{fe0f}', '\n', ' ', '\u{1b}', 'r', 's'] {
            assert!(!is_prompt_character(ch));
        }
        let used = ['\u{e7a8}'].into_iter().collect();
        let shared = "[rust]\nsymbol='\u{e7a8}'\n[python]\nsymbol='\u{e7a8}'";
        assert_eq!(
            requirements(&used, Some(shared)),
            vec![('\u{e7a8}', "Prompt".into())]
        );
        assert_eq!(
            requirements(&used, Some("invalid")),
            vec![('\u{e7a8}', "Prompt".into())]
        );
    }
}
