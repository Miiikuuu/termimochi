//! Read-only saved-configuration snapshots. No shell, running-session query,
//! remote control, deployment binding or destination is created here.
use crate::design_document::{DesignDocument, NativeFormat, NativeSource, TargetHint};
use crate::document_store::Document;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum ReadStatus {
    Read,
    Inherited,
    Unparsed,
    Unrecognized,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct FieldReport {
    pub field: String,
    pub status: ReadStatus,
    pub source: String,
}
impl FieldReport {
    fn new(field: impl Into<String>, status: ReadStatus, source: impl Into<String>) -> Self {
        Self {
            field: field.into(),
            status,
            source: source.into(),
        }
    }
    pub fn label(&self) -> String {
        format!("{} · {:?} — {}", self.field, self.status, self.source)
    }
}

pub(crate) fn ptyxis(id: &str) -> Result<DesignDocument, String> {
    use crate::design_document::{PaletteComponent, theme::fields};
    use gtk::gio::prelude::*;
    let appearance = crate::ptyxis::import_profile_appearance(id)?;
    let global = crate::ptyxis::find_settings("org.gnome.Ptyxis", None)
        .ok_or("Ptyxis global settings unavailable")?;
    let profile = crate::ptyxis::find_settings(
        "org.gnome.Ptyxis.Profile",
        Some(&format!("/org/gnome/Ptyxis/Profiles/{id}/")),
    )
    .ok_or("Ptyxis profile unavailable")?;
    let has = |s: &gtk::gio::Settings, key: &str| {
        s.settings_schema()
            .is_some_and(|schema| schema.has_key(key))
    };
    let mut design = DesignDocument::new_theme(
        TargetHint::Ptyxis,
        "My Ptyxis theme",
        appearance.preferred_variant != Some(termimochi_core::Variant::Dark),
    );
    let t = design.theme.as_mut().unwrap();
    t.import_report.push(FieldReport::new(
        "Snapshot",
        ReadStatus::Read,
        appearance.source_label,
    ));
    if let Some(p) = appearance.palette {
        design.components.palette = Some(PaletteComponent {
            source: p.to_palette_string(),
            light: t.light,
        });
        t.import_report.push(FieldReport::new(
            "Palette",
            ReadStatus::Read,
            format!("profile {id} / palette"),
        ));
    } else {
        t.import_report.push(FieldReport::new(
            "Palette",
            ReadStatus::Unparsed,
            "Palette could not be read; displayed colors are inherited preview references",
        ));
    }
    let font_fields = fields(&appearance.typography);
    let font_source = if has(&global, "use-system-font") && global.boolean("use-system-font") {
        crate::ptyxis::find_settings("org.gnome.desktop.interface", None)
            .filter(|s| {
                has(s, "monospace-font-name") && !s.string("monospace-font-name").trim().is_empty()
            })
            .map(|_| "desktop.interface / monospace-font-name (system font)")
    } else {
        (has(&global, "font-name") && !global.string("font-name").trim().is_empty())
            .then_some("Ptyxis global / font-name")
    };
    for field in ["family", "size", "weight"] {
        if let Some(source) = font_source {
            t.typography
                .insert(field.into(), font_fields[field].clone());
            t.import_report.push(FieldReport::new(
                format!("typography.{field}"),
                ReadStatus::Read,
                source,
            ));
        } else {
            t.import_report.push(FieldReport::new(
                format!("typography.{field}"),
                ReadStatus::Inherited,
                "Font source unavailable; preview reference only",
            ));
        }
    }
    for (field, key) in [
        ("cell_width", "cell-width-scale"),
        ("line_height", "cell-height-scale"),
    ] {
        if has(&profile, key) {
            t.typography
                .insert(field.into(), font_fields[field].clone());
            t.import_report.push(FieldReport::new(
                format!("typography.{field}"),
                ReadStatus::Read,
                format!("profile {id} / {key}"),
            ));
        }
    }
    let layout_fields = fields(&appearance.layout);
    for (field, key) in [
        ("columns", "default-columns"),
        ("rows", "default-rows"),
        ("cursor_shape", "cursor-shape"),
        ("cursor_blink", "cursor-blink-mode"),
    ] {
        if has(&global, key) {
            t.layout.insert(field.into(), layout_fields[field].clone());
            t.import_report.push(FieldReport::new(
                format!("layout.{field}"),
                ReadStatus::Read,
                format!(
                    "Ptyxis global / {key}; restored window-size takes precedence when enabled"
                ),
            ));
        }
    }
    // Padding, single-tab visibility and overlay scrollbar policy are preview
    // approximations. Do not turn them into authored layout defaults.
    for notice in appearance.notices {
        t.import_report
            .push(FieldReport::new("Limit", ReadStatus::Unparsed, notice));
    }
    t.import_report.push(FieldReport::new(
        "Prompt / Greeting",
        ReadStatus::Inherited,
        "Not associated with this profile; optional candidates must be explicitly selected",
    ));
    design.validate()?;
    Ok(design)
}

const MAX_FILES: usize = 32;
const MAX_DEPTH: usize = 8;
const MAX_BYTES: usize = 512 * 1024;
struct Reader {
    stack: BTreeSet<PathBuf>,
    sources: Vec<NativeSource>,
    report: Vec<FieldReport>,
    expanded: String,
    bytes: usize,
    files: usize,
    entries: usize,
    snapshots: Vec<crate::starship_file::FileSnapshot>,
}
impl Reader {
    fn note(&mut self, field: &str, source: String) {
        if self.report.len() < 200 {
            self.report
                .push(FieldReport::new(field, ReadStatus::Unparsed, source));
        }
    }
    fn file(&mut self, path: &Path, depth: usize) -> Result<(), String> {
        if depth > MAX_DEPTH || self.files >= MAX_FILES {
            return Err("Kitty include limit exceeded (32 files / depth 8)".into());
        }
        let canonical = std::fs::canonicalize(path).map_err(|e| e.to_string())?;
        if sensitive_path(&canonical) {
            return Err("Refusing shell startup/history or credential source in a terminal appearance import".into());
        }
        if !self.stack.insert(canonical.clone()) {
            self.note("include", format!("Cycle skipped: {}", path.display()));
            return Ok(());
        }
        let file = crate::starship_file::FileSnapshot::read(path)?;
        self.files += 1;
        self.bytes += file.contents.len();
        if self.bytes > MAX_BYTES {
            return Err("Kitty sources exceed 512 KiB".into());
        }
        self.sources.push(NativeSource {
            format: NativeFormat::Kitty,
            text: file.contents.clone(),
        });
        self.report.push(FieldReport::new(
            "File",
            ReadStatus::Read,
            path.display().to_string(),
        ));
        // Kitty continuation lines start with a backslash, not end with one.
        let mut logical = Vec::<String>::new();
        for line in file.contents.lines() {
            if let Some(rest) = line.trim_start().strip_prefix('\\') {
                if let Some(previous) = logical.last_mut() {
                    previous.push_str(rest.trim_start());
                } else {
                    self.note(
                        "continuation",
                        format!("Orphan continuation in {}", path.display()),
                    );
                }
            } else {
                logical.push(line.into());
            }
        }
        for line in logical {
            let trimmed = line.trim();
            let (key, value) = trimmed
                .split_once(char::is_whitespace)
                .unwrap_or((trimmed, ""));
            if matches!(key, "include" | "globinclude") {
                self.expanded
                    .push_str(&format!("# retained directive: {line}\n"));
                let value = value.trim();
                if value.contains(['$', '~', '`']) || value.is_empty() {
                    self.note(
                        key,
                        format!("Environment-dependent path not expanded: {value}"),
                    );
                    continue;
                }
                let target = path.parent().unwrap_or(Path::new(".")).join(value);
                let targets = if key == "globinclude" {
                    self.glob(&target)?
                } else {
                    vec![target]
                };
                if targets.is_empty() {
                    self.note(key, format!("No matching files: {value}"));
                }
                for target in targets {
                    if !target.exists() {
                        self.note(key, format!("Missing file: {}", target.display()));
                        continue;
                    }
                    self.file(&target, depth + 1)?;
                }
            } else if matches!(key, "geninclude" | "envinclude") {
                self.expanded
                    .push_str(&format!("# skipped execution/environment source: {line}\n"));
                self.note(key, format!("Not executed or evaluated: {value}"));
            } else {
                self.expanded.push_str(&line);
                self.expanded.push('\n');
            }
            if self.expanded.len() > MAX_BYTES {
                return Err("Expanded Kitty config exceeds 512 KiB".into());
            }
        }
        // Read-only conflict check, without retaining any write permission.
        file.verify()?;
        self.snapshots.push(file);
        self.stack.remove(&canonical);
        Ok(())
    }
    fn glob(&mut self, path: &Path) -> Result<Vec<PathBuf>, String> {
        let pattern = glob::Pattern::new(path.to_str().ok_or("Non-UTF8 include pattern")?)
            .map_err(|e| e.to_string())?;
        let mut base = PathBuf::new();
        for component in path.components() {
            if component
                .as_os_str()
                .to_string_lossy()
                .contains(['*', '?', '['])
            {
                break;
            }
            base.push(component);
        }
        if base == path {
            return Ok(if path.exists() {
                vec![path.into()]
            } else {
                vec![]
            });
        }
        let mut matches = Vec::new();
        self.walk(&base, &pattern, 0, &mut matches)?;
        matches.sort();
        Ok(matches)
    }
    fn walk(
        &mut self,
        dir: &Path,
        pattern: &glob::Pattern,
        depth: usize,
        result: &mut Vec<PathBuf>,
    ) -> Result<(), String> {
        if depth > MAX_DEPTH {
            return Err("Glob directory depth exceeds 8".into());
        }
        if !dir.exists() {
            return Ok(());
        }
        for entry in std::fs::read_dir(dir).map_err(|e| e.to_string())? {
            self.entries += 1;
            if self.entries > 2048 {
                return Err("Glob traversal exceeds 2048 entries".into());
            }
            let entry = entry.map_err(|e| e.to_string())?;
            let kind = entry.file_type().map_err(|e| e.to_string())?;
            // Never descend through directory symlinks into unbounded trees.
            if kind.is_symlink() && entry.path().is_dir() {
                self.note(
                    "globinclude",
                    format!(
                        "Directory symlink not traversed: {}",
                        entry.path().display()
                    ),
                );
            } else if kind.is_dir() {
                self.walk(&entry.path(), pattern, depth + 1, result)?;
            } else if pattern.matches_path_with(
                &entry.path(),
                glob::MatchOptions {
                    case_sensitive: true,
                    require_literal_separator: true,
                    require_literal_leading_dot: true,
                },
            ) {
                result.push(entry.path());
                if result.len() > MAX_FILES {
                    return Err("Glob matches more than 32 files".into());
                }
            }
        }
        Ok(())
    }
}

pub(crate) fn kitty(path: &Path) -> Result<DesignDocument, String> {
    let mut r = Reader {
        stack: BTreeSet::new(),
        sources: vec![],
        report: vec![],
        expanded: String::new(),
        bytes: 0,
        files: 0,
        entries: 0,
        snapshots: vec![],
    };
    r.file(path, 0)?;
    for snapshot in &r.snapshots {
        snapshot.verify()?;
    }
    let parsed = crate::kitty_document::KittyDocument::parse(&r.expanded)?;
    for key in parsed.properties().keys() {
        r.report.push(FieldReport::new(
            key,
            ReadStatus::Read,
            "Flattened saved config; last supported occurrence wins",
        ));
    }
    for notice in &parsed.notices {
        if r.report.len() < 230 {
            r.report.push(FieldReport::new(
                "Native field",
                ReadStatus::Unrecognized,
                notice,
            ));
        }
    }
    r.report.push(FieldReport::new(
        "Runtime",
        ReadStatus::Unparsed,
        "Command-line overrides, live remote settings and terminal environment were not captured",
    ));
    r.report.push(FieldReport::new(
        "Prompt / Greeting",
        ReadStatus::Inherited,
        "No startup scripts executed; optional files are not evidence that Kitty uses them",
    ));
    let mut design = DesignDocument::from_kitty_source(r.expanded)?
        .into_theme(TargetHint::Kitty, "My Kitty theme")?;
    let t = design.theme.as_mut().unwrap();
    t.sources = r.sources;
    t.import_report = r.report;
    design.validate()?;
    Ok(design)
}

fn sensitive_path(path: &Path) -> bool {
    path.components().any(|component| {
        matches!(
            component.as_os_str().to_str(),
            Some(
                ".ssh"
                    | ".gnupg"
                    | ".aws"
                    | ".bashrc"
                    | ".profile"
                    | ".bash_profile"
                    | ".zshrc"
                    | ".bash_history"
                    | ".zsh_history"
                    | ".netrc"
                    | ".env"
                    | "credentials"
                    | "id_rsa"
                    | "id_ed25519"
            )
        )
    })
}

pub(crate) fn kitty_candidates() -> Vec<PathBuf> {
    let mut paths = vec![];
    if let Some(dir) = std::env::var_os("KITTY_CONFIG_DIRECTORY") {
        paths.push(PathBuf::from(dir).join("kitty.conf"));
    }
    paths.push(gtk::glib::user_config_dir().join("kitty/kitty.conf"));
    paths.push(gtk::glib::home_dir().join(".config/kitty/kitty.conf"));
    let mut seen = BTreeSet::new();
    paths
        .into_iter()
        .filter(|p| p.is_file() && seen.insert(p.clone()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn kitty_includes_order_sources_and_no_execution() {
        let root = tempfile::tempdir().unwrap();
        let file = root.path().join("kitty.conf");
        std::fs::create_dir(root.path().join("parts")).unwrap();
        std::fs::write(&file,"background #111111\ninclude colors.conf\nglobinclude parts/*.conf\nfont_size 17\ngeninclude touch must-not-exist\nenvinclude KITTY_CONF_*\ninclude missing.conf\n").unwrap();
        std::fs::write(
            root.path().join("colors.conf"),
            "background #222222\ninclude kitty.conf\ninclude ${UNKNOWN}.conf\n",
        )
        .unwrap();
        std::fs::write(
            root.path().join("parts/a.conf"),
            "font_size 12\nforeground #aabbcc\n",
        )
        .unwrap();
        std::fs::write(
            root.path().join("parts/b.conf"),
            "font_size 14\nforeground #ddeeff\nunknown_future_setting retained\n",
        )
        .unwrap();
        let before = std::fs::read(&file).unwrap();
        let design = kitty(&file).unwrap();
        let source = design.theme_native_source().unwrap();
        let parsed = crate::kitty_document::KittyDocument::parse(&source).unwrap();
        assert_eq!(parsed.properties()["background"], "#222222");
        assert_eq!(parsed.properties()["foreground"], "#ddeeff");
        assert_eq!(parsed.properties()["font_size"], "17");
        assert!(!parsed.safe_configuration().contains("include"));
        assert!(!parsed.safe_configuration().contains("do-not-run"));
        let theme = design.theme.as_ref().unwrap();
        let report = theme
            .import_report
            .iter()
            .map(FieldReport::label)
            .collect::<Vec<_>>()
            .join("\n");
        for expected in [
            "Cycle skipped",
            "Missing file",
            "Environment-dependent",
            "geninclude",
            "Not executed",
            "unknown_future_setting",
        ] {
            assert!(report.contains(expected), "{report}");
        }
        assert!(
            theme
                .sources
                .iter()
                .any(|s| s.text.contains("geninclude touch"))
        );
        assert_eq!(std::fs::read(&file).unwrap(), before);
        assert!(!root.path().join("must-not-exist").exists());
        assert!(!design.scope().prompt && !design.scope().greeting);
        assert!(
            theme.typography.is_empty(),
            "Native parser remains the sole owner; no duplicated font defaults"
        );
        assert_eq!(
            serde_json::from_slice::<DesignDocument>(&serde_json::to_vec(&design).unwrap())
                .unwrap(),
            design
        );
    }
    #[test]
    fn kitty_recursive_glob_and_continuation() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("parts/deep")).unwrap();
        std::fs::write(
            root.path().join("parts/deep/a.conf"),
            "font_size 16\nfont_family JetBrains\n  \\ Mono\n",
        )
        .unwrap();
        let path = root.path().join("kitty.conf");
        std::fs::write(&path, "globinclude parts/**/*.conf\n").unwrap();
        std::os::unix::fs::symlink(root.path().join("parts"), root.path().join("parts/loop"))
            .unwrap();
        let d = kitty(&path).unwrap();
        assert!(
            d.theme
                .as_ref()
                .unwrap()
                .import_report
                .iter()
                .any(|r| r.source.contains("Directory symlink not traversed"))
        );
        let p =
            crate::kitty_document::KittyDocument::parse(&d.theme_native_source().unwrap()).unwrap();
        assert_eq!(p.properties()["font_size"], "16");
        // Continuations remove whitespace after the backslash, like Kitty.
        assert_eq!(p.properties()["font_family"], "JetBrainsMono");
    }
    #[test]
    fn unread_later_override_cannot_masquerade_as_earlier_value() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("kitty.conf");
        std::fs::write(
            &path,
            "font_size 14\ninclude later.conf\nbackground #ffffff\n",
        )
        .unwrap();
        std::fs::write(root.path().join("later.conf"), "font_size $DYNAMIC_SIZE\n").unwrap();
        let d = kitty(&path).unwrap();
        let p =
            crate::kitty_document::KittyDocument::parse(&d.theme_native_source().unwrap()).unwrap();
        assert!(!p.properties().contains_key("font_size"));
        assert!(p.notices.iter().any(|n| n.contains("font_size")));
        assert!(!d.scope().typography);
    }
    #[test]
    fn kitty_limits_missing_and_special_files() {
        let root = tempfile::tempdir().unwrap();
        assert!(kitty(&root.path().join("missing")).is_err());
        assert!(kitty(root.path()).is_err());
        let shell = root.path().join(".bashrc");
        std::fs::write(&shell, "background #ffffff").unwrap();
        let alias = root.path().join("alias.conf");
        std::os::unix::fs::symlink(&shell, &alias).unwrap();
        assert!(kitty(&alias).unwrap_err().contains("Refusing shell"));
        for i in 0..10 {
            std::fs::write(
                root.path().join(format!("{i}.conf")),
                format!("include {}.conf\n", i + 1),
            )
            .unwrap();
        }
        assert!(
            kitty(&root.path().join("0.conf"))
                .unwrap_err()
                .contains("limit")
        );
        let large = root.path().join("large.conf");
        std::fs::write(&large, " ".repeat(256 * 1024 + 1)).unwrap();
        assert!(kitty(&large).is_err());
        let files = root.path().join("many");
        std::fs::create_dir(&files).unwrap();
        for i in 0..33 {
            std::fs::write(files.join(format!("{i}.conf")), "font_size 12\n").unwrap();
        }
        std::fs::write(root.path().join("glob.conf"), "globinclude many/*.conf\n").unwrap();
        assert!(
            kitty(&root.path().join("glob.conf"))
                .unwrap_err()
                .contains("32")
        );
    }
}
