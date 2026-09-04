use std::{fmt, str::FromStr};

use crate::{PtyxisPalette, ThemeVariant, Variant, contrast_ratio};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Severity {
    Error,
    Warning,
    Note,
}

impl Severity {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Warning => "warning",
            Self::Note => "note",
        }
    }
}

impl fmt::Display for Severity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Target {
    General,
    #[default]
    Codex,
}

impl Target {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::General => "general",
            Self::Codex => "codex",
        }
    }
}

impl fmt::Display for Target {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for Target {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.to_ascii_lowercase().as_str() {
            "general" => Ok(Self::General),
            "codex" => Ok(Self::Codex),
            _ => Err(format!("unsupported lint target: {value}")),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Issue {
    pub severity: Severity,
    pub code: &'static str,
    pub message: String,
    pub variant: Option<Variant>,
    pub ratio: Option<f64>,
}

impl Issue {
    fn new(
        severity: Severity,
        code: &'static str,
        message: impl Into<String>,
        variant: Variant,
        ratio: Option<f64>,
    ) -> Self {
        Self {
            severity,
            code,
            message: message.into(),
            variant: Some(variant),
            ratio,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct LintReport {
    pub palette_name: String,
    pub target: Target,
    pub issues: Vec<Issue>,
}

impl LintReport {
    #[must_use]
    pub fn has_errors(&self) -> bool {
        self.issues
            .iter()
            .any(|issue| issue.severity == Severity::Error)
    }

    pub fn issues_for(&self, variant: Variant) -> impl Iterator<Item = &Issue> {
        self.issues
            .iter()
            .filter(move |issue| issue.variant == Some(variant))
    }
}

#[must_use]
pub fn lint_palette(palette: &PtyxisPalette, target: Target) -> LintReport {
    let mut issues = Vec::new();

    for (&kind, variant) in palette.variants() {
        let start = issues.len();
        check_structure(variant, &mut issues);
        check_readability(variant, &mut issues);
        if target == Target::Codex {
            check_codex(variant, &mut issues);
        }
        if issues.len() == start {
            issues.push(Issue::new(
                Severity::Note,
                "variant-passed",
                "all enabled checks passed",
                kind,
                None,
            ));
        }
    }

    LintReport {
        palette_name: palette.name().to_owned(),
        target,
        issues,
    }
}

fn check_structure(variant: &ThemeVariant, issues: &mut Vec<Issue>) {
    for key in variant.missing_required_keys() {
        issues.push(Issue::new(
            Severity::Error,
            "missing-required-color",
            format!("missing required color {key}"),
            variant.kind(),
            None,
        ));
    }
}

fn check_readability(variant: &ThemeVariant, issues: &mut Vec<Issue>) {
    let Some(foreground) = variant.get("Foreground") else {
        return;
    };
    let Some(background) = variant.get("Background") else {
        return;
    };

    let body_ratio = contrast_ratio(foreground, background);
    if body_ratio < 4.5 {
        issues.push(Issue::new(
            Severity::Error,
            "body-text-low-contrast",
            "Foreground and Background need at least 4.5:1 contrast",
            variant.kind(),
            Some(body_ratio),
        ));
    }

    if let Some(cursor) = variant.get("Cursor") {
        let cursor_ratio = contrast_ratio(cursor, background);
        if cursor_ratio < 3.0 {
            issues.push(Issue::new(
                Severity::Warning,
                "cursor-low-contrast",
                "Cursor is difficult to distinguish from Background",
                variant.kind(),
                Some(cursor_ratio),
            ));
        }
    }

    for index in (1..7).chain(9..15) {
        let key = format!("Color{index}");
        let Some(color) = variant.get(&key) else {
            continue;
        };
        let ratio = contrast_ratio(color, background);
        if ratio < 3.0 {
            issues.push(Issue::new(
                Severity::Warning,
                "ansi-color-low-contrast",
                format!("{key} may be hard to read on Background"),
                variant.kind(),
                Some(ratio),
            ));
        }
    }
}

fn check_codex(variant: &ThemeVariant, issues: &mut Vec<Issue>) {
    let Some(foreground) = variant.get("Foreground") else {
        return;
    };
    let Some(composer_background) = variant.get("Color0") else {
        return;
    };
    let ratio = contrast_ratio(foreground, composer_background);
    if ratio < 4.5 {
        issues.push(Issue::new(
            Severity::Error,
            "codex-composer-low-contrast",
            "Codex composer text may disappear: Foreground versus Color0 needs at least 4.5:1 contrast",
            variant.kind(),
            Some(ratio),
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ORIGINAL: &str = include_str!("../fixtures/fog-paper.palette");
    const CODEX: &str = include_str!("../fixtures/fog-paper-codex.palette");

    #[test]
    fn original_fog_paper_catches_codex_composer_failure() {
        let palette = PtyxisPalette::from_text(ORIGINAL).unwrap();
        let report = lint_palette(&palette, Target::Codex);
        let failures: Vec<_> = report
            .issues_for(Variant::Light)
            .filter(|issue| issue.code == "codex-composer-low-contrast")
            .collect();
        assert_eq!(failures.len(), 1);
        assert_eq!(failures[0].severity, Severity::Error);
        assert!(failures[0].ratio.unwrap() < 1.2);
    }

    #[test]
    fn codex_variant_fixes_composer_failure() {
        let palette = PtyxisPalette::from_text(CODEX).unwrap();
        let report = lint_palette(&palette, Target::Codex);
        assert!(
            report
                .issues_for(Variant::Light)
                .all(|issue| issue.code != "codex-composer-low-contrast")
        );
    }

    #[test]
    fn missing_required_colors_are_errors() {
        let palette = PtyxisPalette::from_text(
            "[Palette]\nName=Tiny\n[Light]\nForeground=#000000\nBackground=#FFFFFF\n",
        )
        .unwrap();
        let report = lint_palette(&palette, Target::General);
        assert_eq!(
            report
                .issues
                .iter()
                .filter(|issue| issue.code == "missing-required-color")
                .count(),
            16
        );
        assert!(report.has_errors());
    }
}
