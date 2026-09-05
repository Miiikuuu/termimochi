//! A lossless, in-memory copy of the user's configuration. Only reviewed module
//! properties can be edited; export never uses the sandbox's filtered config.
use std::{
    collections::VecDeque,
    path::{Path, PathBuf},
};

use crate::starship_modules::{MODULES, ModuleSpec, TextField};
use toml_edit::{DocumentMut, Item, Value, value};

const LIMIT: usize = 256 * 1024;
const HISTORY_LIMIT: usize = 64;
pub(crate) const VERSION_LAYOUTS: [&str; 3] = [
    "[$symbol$version]($style) ",
    "[$symbol]($style) ",
    "[$version]($style) ",
];
pub(crate) const VERSION_FORMATS: [&str; 3] = ["v${raw}", "v${major}.${minor}", "v${major}"];

pub(crate) enum ModuleEdit {
    Symbol(String),
    Style(String),
    Color(String),
    Bold(bool),
    Layout(u32),
    Version(u32),
    Enabled(bool),
    Reset,
    Format(String),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EditFocus {
    Symbol,
    Style,
    Format,
}

struct Snapshot {
    contents: String,
    module: usize,
    symbol: usize,
    style: usize,
    focus: EditFocus,
}

pub(crate) struct StarshipDraft {
    pub(crate) source_path: PathBuf,
    original: String,
    contents: String,
    exported: String,
    undo: VecDeque<Snapshot>,
    redo: Vec<Snapshot>,
    text_group: Option<&'static str>,
    pub(crate) module: usize,
    pub(crate) symbol: usize,
    pub(crate) style: usize,
    pub(crate) sample_focus: EditFocus,
}

impl StarshipDraft {
    pub(crate) fn new(source_path: PathBuf, source: String) -> Result<Self, String> {
        if source.len() > LIMIT {
            return Err("Configuration exceeds 256 KiB.".into());
        }
        let doc = source
            .parse::<DocumentMut>()
            .map_err(|e| format!("Invalid TOML: {e}"))?;
        for module in MODULES {
            if doc
                .get(module.id)
                .is_some_and(|rust| rust.as_table_like().is_none())
            {
                return Err(format!("[{}] must be a table.", module.id));
            }
            for field in module
                .symbols
                .iter()
                .chain(module.styles)
                .map(|f| f.key)
                .chain(["format", "version_format"])
            {
                if doc
                    .get(module.id)
                    .and_then(|r| r.get(field))
                    .is_some_and(|v| v.as_str().is_none())
                {
                    return Err(format!("{}.{field} must be text.", module.id));
                }
            }
            if doc
                .get(module.id)
                .and_then(|r| r.get("disabled"))
                .is_some_and(|v| v.as_bool().is_none())
            {
                return Err(format!("{}.disabled must be true or false.", module.id));
            }
        }
        Ok(Self {
            source_path,
            contents: source.clone(),
            exported: source.clone(),
            original: source,
            undo: VecDeque::new(),
            redo: Vec::new(),
            text_group: None,
            module: 0,
            symbol: 0,
            style: 0,
            sample_focus: EditFocus::Symbol,
        })
    }

    pub(crate) fn contents(&self) -> &str {
        &self.contents
    }
    pub(crate) fn dirty(&self) -> bool {
        self.contents != self.exported
    }
    pub(crate) fn mark_exported(&mut self, contents: String) {
        self.exported = contents;
    }
    pub(crate) fn replace_contents(&mut self, contents: String) -> Result<(), String> {
        Self::new(self.source_path.clone(), contents.clone())?;
        self.finish_text_edit();
        self.commit(contents, None);
        Ok(())
    }
    pub(crate) fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }
    pub(crate) fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
    pub(crate) fn undo(&mut self) {
        self.finish_text_edit();
        if let Some(previous) = self.undo.pop_back() {
            let mut next = self.snapshot();
            next.module = previous.module;
            next.symbol = previous.symbol;
            next.style = previous.style;
            next.focus = previous.focus;
            self.redo.push(next);
            self.restore(previous);
        }
    }
    pub(crate) fn redo(&mut self) {
        self.finish_text_edit();
        if let Some(next) = self.redo.pop() {
            let mut previous = self.snapshot();
            previous.module = next.module;
            previous.symbol = next.symbol;
            previous.style = next.style;
            previous.focus = next.focus;
            self.undo.push_back(previous);
            self.restore(next);
        }
    }
    pub(crate) fn field(&self, key: &str) -> Option<String> {
        field(&self.contents, self.spec().id, key)
    }
    pub(crate) fn spec(&self) -> &'static ModuleSpec {
        &MODULES[self.module]
    }
    pub(crate) fn symbol_spec(&self) -> Option<&'static TextField> {
        self.spec().symbols.get(self.symbol)
    }
    pub(crate) fn style_spec(&self) -> Option<&'static TextField> {
        self.spec().styles.get(self.style)
    }
    pub(crate) fn select_module(&mut self, index: usize) -> Result<(), String> {
        if index >= MODULES.len() {
            return Err("Unknown module.".into());
        }
        self.finish_text_edit();
        self.module = index;
        self.symbol = 0;
        self.style = 0;
        self.sample_focus = if self.spec().symbols.is_empty() {
            EditFocus::Style
        } else {
            EditFocus::Symbol
        };
        Ok(())
    }
    fn snapshot(&self) -> Snapshot {
        Snapshot {
            contents: self.contents.clone(),
            module: self.module,
            symbol: self.symbol,
            style: self.style,
            focus: self.sample_focus,
        }
    }
    fn restore(&mut self, snapshot: Snapshot) {
        self.contents = snapshot.contents;
        self.module = snapshot.module;
        self.symbol = snapshot.symbol;
        self.style = snapshot.style;
        self.sample_focus = snapshot.focus;
    }
    pub(crate) fn enabled(&self) -> bool {
        !self
            .document()
            .get(self.spec().id)
            .and_then(|r| r.get("disabled"))
            .and_then(Item::as_bool)
            .unwrap_or(self.spec().disabled)
    }
    pub(crate) fn style(&self) -> String {
        self.style_spec()
            .map(|f| {
                self.field(f.key).unwrap_or_else(|| {
                    if matches!(
                        f.key,
                        "repo_root_style"
                            | "before_repo_root_style"
                            | "success_style"
                            | "failure_style"
                    ) {
                        self.field("style")
                            .unwrap_or_else(|| self.spec().styles[0].default.into())
                    } else {
                        f.default.into()
                    }
                })
            })
            .unwrap_or_default()
    }
    pub(crate) fn layout_index(&self) -> u32 {
        choice_index(self.field("format"), &VERSION_LAYOUTS)
    }
    pub(crate) fn version_index(&self) -> u32 {
        choice_index(self.field("version_format"), &VERSION_FORMATS)
    }
    pub(crate) fn literal_symbol(&self) -> String {
        let Some(spec) = self.symbol_spec() else {
            return String::new();
        };
        let symbol = self.field(spec.key).unwrap_or_else(|| spec.default.into());
        if !spec.escaped {
            return symbol;
        }
        let mut chars = symbol.chars();
        let mut out = String::new();
        while let Some(ch) = chars.next() {
            if ch == '\\' {
                out.extend(chars.next());
            } else {
                out.push(ch);
            }
        }
        out
    }

    pub(crate) fn color(&self) -> Option<String> {
        let style = self.style();
        let foreground = style
            .split_whitespace()
            .rfind(|t| !attribute(t) && !t.starts_with("bg:"))?;
        let name = foreground.strip_prefix("fg:").unwrap_or(foreground);
        let doc = self.document();
        let palette = doc.get("palette").and_then(Item::as_str);
        let resolved = palette.and_then(|p| doc.get("palettes")?.get(p)?.get(name)?.as_str());
        Some(resolved.unwrap_or(name).to_owned())
    }

    pub(crate) fn edit(&mut self, edit: ModuleEdit) -> Result<bool, String> {
        self.edit_grouped(edit, None)
    }
    pub(crate) fn edit_text(
        &mut self,
        edit: ModuleEdit,
        field: &'static str,
    ) -> Result<bool, String> {
        self.edit_grouped(edit, Some(field))
    }
    pub(crate) fn finish_text_edit(&mut self) {
        self.text_group = None;
    }
    fn edit_grouped(
        &mut self,
        edit: ModuleEdit,
        group: Option<&'static str>,
    ) -> Result<bool, String> {
        let mut doc = self.document();
        let module = self.spec().id;
        self.sample_focus = match &edit {
            ModuleEdit::Symbol(_) => EditFocus::Symbol,
            ModuleEdit::Style(_) | ModuleEdit::Color(_) | ModuleEdit::Bold(_) => EditFocus::Style,
            ModuleEdit::Layout(_) | ModuleEdit::Version(_) | ModuleEdit::Format(_) => {
                EditFocus::Format
            }
            _ => self.sample_focus,
        };
        match edit {
            ModuleEdit::Reset => {
                let original: DocumentMut = self.original.parse().expect("validated original");
                if let Some(item) = original.get(module) {
                    doc[module] = item.clone();
                } else {
                    doc.remove(module);
                }
                // Preserve byte-exact no-op/reset behavior once every module matches.
                let contents = doc.to_string();
                let unchanged = toml::from_str::<toml::Table>(&contents).ok()
                    == toml::from_str::<toml::Table>(&self.original).ok();
                let contents = if unchanged {
                    self.original.clone()
                } else {
                    contents
                };
                return Ok(self.commit(contents, None));
            }
            ModuleEdit::Symbol(text) => {
                let spec = self
                    .symbol_spec()
                    .ok_or("This module has no symbol field.")?;
                validate_text(&text, 64)?;
                let mut escaped = String::new();
                for ch in text.chars() {
                    if spec.escaped && "\\$[]()".contains(ch) {
                        escaped.push('\\');
                    }
                    escaped.push(ch);
                }
                if spec.formatted {
                    validate_format(&escaped)?;
                }
                set(&mut doc, module, spec.key, Some(Value::from(escaped)));
            }
            ModuleEdit::Style(style) => {
                self.validate_style(&style)?;
                set(
                    &mut doc,
                    module,
                    self.style_spec()
                        .ok_or("This module has no style field.")?
                        .key,
                    Some(Value::from(style)),
                );
            }
            ModuleEdit::Color(color) => {
                if !is_hex(&color) {
                    return Err("Use a six-digit color, such as #eb6f92.".into());
                }
                let mut tokens = self
                    .style()
                    .split_whitespace()
                    .filter(|t| attribute(t) || t.starts_with("bg:"))
                    .map(str::to_owned)
                    .collect::<Vec<_>>();
                tokens.push(format!("fg:{color}"));
                set(
                    &mut doc,
                    module,
                    self.style_spec()
                        .ok_or("This module has no style field.")?
                        .key,
                    Some(Value::from(tokens.join(" "))),
                );
            }
            ModuleEdit::Bold(bold) => {
                let mut tokens = self
                    .style()
                    .split_whitespace()
                    .filter(|t| *t != "bold")
                    .map(str::to_owned)
                    .collect::<Vec<_>>();
                if bold {
                    tokens.insert(0, "bold".into());
                }
                set(
                    &mut doc,
                    module,
                    self.style_spec()
                        .ok_or("This module has no style field.")?
                        .key,
                    Some(Value::from(tokens.join(" "))),
                );
            }
            ModuleEdit::Layout(index) => {
                if !self.spec().version {
                    return Err("This module has no version layout.".into());
                }
                self.set_choice(&mut doc, "format", index, &VERSION_LAYOUTS)?;
            }
            ModuleEdit::Version(index) => {
                if !self.spec().version {
                    return Err("This module has no version field.".into());
                }
                self.set_choice(&mut doc, "version_format", index, &VERSION_FORMATS)?
            }
            ModuleEdit::Enabled(enabled) => {
                set(&mut doc, module, "disabled", Some(Value::from(!enabled)))
            }
            ModuleEdit::Format(text) => {
                validate_text(&text, 1024)?;
                validate_format(&text)?;
                set(&mut doc, module, "format", Some(Value::from(text)));
            }
        }
        let contents = doc.to_string();
        if contents.len() > LIMIT {
            return Err("Configuration exceeds 256 KiB.".into());
        }
        Ok(self.commit(contents, group))
    }

    fn set_choice(
        &self,
        doc: &mut DocumentMut,
        key: &str,
        index: u32,
        choices: &[&str],
    ) -> Result<(), String> {
        let selected = if index == 0 {
            field(&self.original, self.spec().id, key)
        } else {
            Some(
                choices
                    .get(index as usize - 1)
                    .ok_or("Unknown module option.")?
                    .to_string(),
            )
        };
        set(doc, self.spec().id, key, selected.map(Value::from));
        Ok(())
    }
    fn document(&self) -> DocumentMut {
        self.contents.parse().expect("draft is valid TOML")
    }
    fn commit(&mut self, contents: String, group: Option<&'static str>) -> bool {
        if contents == self.contents {
            return false;
        }
        if group.is_none() || self.text_group != group {
            if self.undo.len() == HISTORY_LIMIT {
                self.undo.pop_front();
            }
            self.undo.push_back(self.snapshot());
        }
        self.contents = contents;
        self.text_group = group;
        self.redo.clear();
        true
    }
    fn validate_style(&self, style: &str) -> Result<(), String> {
        validate_text(style, 160)?;
        let doc = self.document();
        for token in style.split_whitespace() {
            let color = token
                .strip_prefix("fg:")
                .or_else(|| token.strip_prefix("bg:"))
                .unwrap_or(token);
            let named = doc
                .get("palette")
                .and_then(Item::as_str)
                .and_then(|p| doc.get("palettes")?.get(p)?.get(color)?.as_str())
                .is_some();
            if !attribute(token)
                && !named
                && !is_hex(color)
                && color.parse::<u8>().is_err()
                && ![
                    "black",
                    "red",
                    "green",
                    "yellow",
                    "blue",
                    "purple",
                    "magenta",
                    "cyan",
                    "white",
                    "bright-black",
                    "bright-red",
                    "bright-green",
                    "bright-yellow",
                    "bright-blue",
                    "bright-purple",
                    "bright-magenta",
                    "bright-cyan",
                    "bright-white",
                ]
                .contains(&color)
            {
                return Err(format!(
                    "Unknown style: {token}. Use a palette color or #RRGGBB."
                ));
            }
        }
        Ok(())
    }

    pub(crate) fn validate_destination(&self, destination: &Path) -> Result<(), String> {
        if destination
            .extension()
            .is_none_or(|ext| !ext.eq_ignore_ascii_case("toml"))
        {
            return Err("Choose a .toml file for Save As.".into());
        }
        match termimochi_core::paths_refer_to_same_file(&self.source_path, destination) {
            Ok(false) => Ok(()),
            Ok(true) => Err("Use Save Changes to update the current configuration with confirmation and backup; Save As needs a different file.".into()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                // Preserve Save As as a recovery route when the original was
                // deleted externally, while protecting its original pathname.
                let resolved = |path: &Path| -> Result<std::path::PathBuf, String> {
                    if path.exists() { return std::fs::canonicalize(path).map_err(|e| e.to_string()); }
                    let parent = path.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
                    let parent = std::fs::canonicalize(parent).map_err(|e| e.to_string())?;
                    Ok(parent.join(path.file_name().ok_or("Choose a configuration filename.")?))
                };
                if resolved(&self.source_path)? == resolved(destination)? {
                    Err("Save As needs a different file from the original configuration.".into())
                } else { Ok(()) }
            }
            Err(e) => Err(format!("Cannot verify the export path: {e}")),
        }
    }
}

fn field(source: &str, module: &str, key: &str) -> Option<String> {
    source
        .parse::<DocumentMut>()
        .ok()?
        .get(module)?
        .get(key)?
        .as_str()
        .map(str::to_owned)
}
fn choice_index(value: Option<String>, choices: &[&str]) -> u32 {
    choices
        .iter()
        .position(|choice| Some(*choice) == value.as_deref())
        .map_or(0, |i| i as u32 + 1)
}
fn set(doc: &mut DocumentMut, module: &str, key: &str, new: Option<Value>) {
    if new.is_none() {
        if let Some(rust) = doc.get_mut(module).and_then(Item::as_table_like_mut) {
            rust.remove(key);
        }
        return;
    }
    if doc.get(module).is_none() {
        doc[module] = Item::Table(toml_edit::Table::new());
    }
    let rust = doc[module]
        .as_table_like_mut()
        .expect("validated module table");
    let mut new = new.unwrap();
    if let Some(old) = rust.get(key).and_then(Item::as_value) {
        if old.as_str().zip(new.as_str()).is_some_and(|(a, b)| a == b)
            || old
                .as_bool()
                .zip(new.as_bool())
                .is_some_and(|(a, b)| a == b)
        {
            return;
        }
        *new.decor_mut() = old.decor().clone();
    }
    rust.insert(key, value(new));
}
fn validate_text(text: &str, limit: usize) -> Result<(), String> {
    if text.chars().count() > limit {
        return Err(format!("Use at most {limit} characters."));
    }
    if text.chars().any(|ch| ch.is_control() || matches!(ch, '\u{061c}' | '\u{200e}' | '\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')) {
        return Err("Control characters and directional overrides are not allowed.".into());
    }
    Ok(())
}
fn validate_format(text: &str) -> Result<(), String> {
    let mut brackets = Vec::new();
    let mut escaped = false;
    for ch in text.chars() {
        if escaped {
            escaped = false;
            continue;
        }
        match ch {
            '\\' => escaped = true,
            '[' | '(' | '{' => brackets.push(ch),
            ']' | ')' | '}' => {
                let expected = match ch {
                    ']' => '[',
                    ')' => '(',
                    _ => '{',
                };
                if brackets.pop() != Some(expected) {
                    return Err(
                        "Unbalanced Starship format. Escape literal brackets with a backslash."
                            .into(),
                    );
                }
            }
            _ => (),
        }
    }
    if escaped || !brackets.is_empty() {
        return Err("Finish the Starship format or escape literal brackets.".into());
    }
    Ok(())
}
fn is_hex(text: &str) -> bool {
    text.len() == 7 && text.starts_with('#') && text[1..].bytes().all(|ch| ch.is_ascii_hexdigit())
}
fn attribute(text: &str) -> bool {
    matches!(
        text,
        "bold"
            | "italic"
            | "underline"
            | "dimmed"
            | "inverted"
            | "hidden"
            | "blink"
            | "strikethrough"
            | "none"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    const SOURCE: &str = "# My prompt\npalette = 'cute'\nformat = '''\n[╭─](bold pink)$directory$rust\n[╰─]($style)$character\n'''\n[palettes.cute]\npink = '#f275a0'\n[rust]\nsymbol = ' rs ' # keep me\nstyle = 'bold pink'\nformat = '[$symbol$version]($style)'\n[custom.secret]\ncommand = 'never execute this'\n[python]\nsymbol = 'py '\n";
    fn draft() -> StarshipDraft {
        StarshipDraft::new("/tmp/source.toml".into(), SOURCE.into()).unwrap()
    }
    #[test]
    fn every_reviewed_module_and_field_edits_without_touching_its_neighbors() {
        for (index, spec) in MODULES.iter().enumerate() {
            let mut draft = draft();
            draft.select_module(index).unwrap();
            for (i, field) in spec.symbols.iter().enumerate() {
                draft.symbol = i;
                draft.edit(ModuleEdit::Symbol("test ".into())).unwrap();
                assert_eq!(
                    draft.field(field.key).as_deref(),
                    Some("test "),
                    "{}.{}",
                    spec.id,
                    field.key
                );
            }
            for (i, field) in spec.styles.iter().enumerate() {
                draft.style = i;
                draft
                    .edit(ModuleEdit::Style("italic bg:blue red".into()))
                    .unwrap();
                draft.edit(ModuleEdit::Color("#abcdef".into())).unwrap();
                draft.edit(ModuleEdit::Bold(true)).unwrap();
                assert_eq!(
                    draft.field(field.key).as_deref(),
                    Some("bold italic bg:blue fg:#abcdef")
                );
            }
            draft.edit(ModuleEdit::Enabled(false)).unwrap();
            assert!(!draft.enabled());
            draft.edit(ModuleEdit::Enabled(true)).unwrap();
            assert!(draft.enabled());
            draft
                .edit(ModuleEdit::Format("[$symbol]($style) ".into()))
                .unwrap();
            if spec.version {
                draft.edit(ModuleEdit::Layout(2)).unwrap();
                draft.edit(ModuleEdit::Version(3)).unwrap();
            } else {
                assert!(draft.edit(ModuleEdit::Layout(1)).is_err());
                assert!(draft.edit(ModuleEdit::Version(1)).is_err());
            }
            let mut before: toml::Table = toml::from_str(SOURCE).unwrap();
            let mut after: toml::Table = toml::from_str(draft.contents()).unwrap();
            before.remove(spec.id);
            after.remove(spec.id);
            assert_eq!(before, after, "{} changed another module", spec.id);
            draft.edit(ModuleEdit::Reset).unwrap();
            assert_eq!(draft.contents(), SOURCE, "{} reset", spec.id);
        }
    }
    #[test]
    fn module_reset_and_global_history_preserve_other_edits_and_restore_selection() {
        let mut draft = draft();
        draft.edit(ModuleEdit::Symbol("crab ".into())).unwrap();
        let rust = draft.contents().to_owned();
        let python = crate::starship_modules::module_index("python").unwrap();
        draft.select_module(python).unwrap();
        draft
            .edit_text(ModuleEdit::Symbol("python ".into()), "symbol")
            .unwrap();
        draft
            .edit_text(ModuleEdit::Symbol("python3 ".into()), "symbol")
            .unwrap();
        draft.select_module(0).unwrap();
        draft.undo();
        assert_eq!(draft.module, python);
        assert_eq!(draft.contents(), rust);
        draft.redo();
        assert_eq!(draft.field("symbol").as_deref(), Some("python3 "));
        draft.edit(ModuleEdit::Reset).unwrap();
        assert_eq!(draft.contents(), rust, "reset only the selected module");
        draft.undo();
        assert_eq!(draft.field("symbol").as_deref(), Some("python3 "));
        draft.select_module(0).unwrap();
        draft.edit(ModuleEdit::Reset).unwrap();
        let doc: toml::Table = toml::from_str(draft.contents()).unwrap();
        assert_eq!(doc["python"]["symbol"].as_str(), Some("python3 "));
    }
    #[test]
    fn formatted_symbols_keep_styles_and_counters_while_path_symbols_stay_raw() {
        let mut draft = draft();
        for (module, text) in [
            ("character", "[>](bold green)"),
            ("git_status", "!${count}"),
        ] {
            draft
                .select_module(crate::starship_modules::module_index(module).unwrap())
                .unwrap();
            draft.edit(ModuleEdit::Symbol(text.into())).unwrap();
            assert_eq!(draft.literal_symbol(), text);
            assert_eq!(
                draft.field(draft.symbol_spec().unwrap().key).as_deref(),
                Some(text)
            );
            let before = draft.contents().to_owned();
            assert!(
                draft
                    .edit(ModuleEdit::Symbol("[unfinished".into()))
                    .is_err()
            );
            assert_eq!(draft.contents(), before);
        }
        draft
            .select_module(crate::starship_modules::module_index("directory").unwrap())
            .unwrap();
        draft.edit(ModuleEdit::Symbol("[$]/\\".into())).unwrap();
        assert_eq!(draft.field("read_only").as_deref(), Some("[$]/\\"));
        assert_eq!(draft.literal_symbol(), "[$]/\\");
    }
    #[test]
    fn module_defaults_validation_and_table_forms_are_safe() {
        for (i, spec) in MODULES.iter().enumerate() {
            let mut draft = StarshipDraft::new("source.toml".into(), "# empty\n".into()).unwrap();
            draft.select_module(i).unwrap();
            assert_eq!(draft.enabled(), !spec.disabled, "{} default", spec.id);
            assert!(!draft.dirty() && !draft.can_undo());
            for source in [
                format!("{} = 1", spec.id),
                format!("[{}]\ndisabled = 'yes'", spec.id),
            ] {
                assert!(StarshipDraft::new("source.toml".into(), source).is_err());
            }
            for source in [
                format!("{} = {{ disabled = false }}\n", spec.id),
                format!("{}.disabled = false\n", spec.id),
            ] {
                let mut draft = StarshipDraft::new("source.toml".into(), source.clone()).unwrap();
                draft.select_module(i).unwrap();
                draft.edit(ModuleEdit::Enabled(false)).unwrap();
                draft.edit(ModuleEdit::Reset).unwrap();
                assert_eq!(draft.contents(), source);
            }
        }
        let mut draft = draft();
        assert!(draft.select_module(usize::MAX).is_err());
        for format in ["[oops", "$symbol\n", "$symbol\u{202e}", "trailing\\"] {
            assert!(draft.edit(ModuleEdit::Format(format.into())).is_err());
        }
        assert_eq!(draft.contents(), SOURCE);
    }
    #[test]
    fn edits_only_rust_preserving_comments_and_unreviewed_modules() {
        let mut draft = draft();
        assert_eq!(draft.contents(), SOURCE);
        draft.edit(ModuleEdit::Symbol(" 🦀 ".into())).unwrap();
        assert!(draft.contents().contains("# keep me"));
        let mut before: toml::Table = toml::from_str(SOURCE).unwrap();
        let mut after: toml::Table = toml::from_str(draft.contents()).unwrap();
        before.remove("rust");
        after.remove("rust");
        assert_eq!(before, after);
        assert!(draft.contents().ends_with(
            "[custom.secret]\ncommand = 'never execute this'\n[python]\nsymbol = 'py '\n"
        ));
    }
    #[test]
    fn no_op_undo_redo_and_export_snapshot() {
        let mut draft = draft();
        assert!(!draft.edit(ModuleEdit::Symbol(" rs ".into())).unwrap());
        draft.edit(ModuleEdit::Symbol("🦀 ".into())).unwrap();
        let exported = draft.contents().to_owned();
        draft.edit(ModuleEdit::Bold(false)).unwrap();
        draft.mark_exported(exported);
        assert!(draft.dirty());
        draft.undo();
        assert!(!draft.dirty());
        draft.redo();
        assert!(draft.dirty());
        draft.edit(ModuleEdit::Reset).unwrap();
        assert_eq!(draft.contents(), SOURCE);
        draft.undo();
        assert_ne!(draft.contents(), SOURCE);
    }
    #[test]
    fn typing_is_grouped_and_history_stays_bounded() {
        let mut draft = draft();
        for symbol in ["", "c", "cr", "crab"] {
            draft
                .edit_text(ModuleEdit::Symbol(symbol.into()), "symbol")
                .unwrap();
        }
        draft.undo();
        assert_eq!(draft.contents(), SOURCE);
        draft.redo();
        assert_eq!(draft.literal_symbol(), "crab");
        draft.finish_text_edit();
        for i in 0..100 {
            draft.edit(ModuleEdit::Symbol(i.to_string())).unwrap();
        }
        assert_eq!(draft.undo.len(), HISTORY_LIMIT);
        for i in 0..100 {
            draft
                .edit_text(ModuleEdit::Symbol(format!("x{i}")), "symbol")
                .unwrap();
        }
        assert_eq!(draft.undo.len(), HISTORY_LIMIT);
        draft.undo();
        assert_eq!(draft.literal_symbol(), "99");
    }
    #[test]
    fn controls_and_malformed_config_fail_without_mutation() {
        for source in [
            "not TOML",
            "rust = []",
            "[rust]\nsymbol = 1",
            "[rust]\ndisabled = 'yes'",
        ] {
            assert!(StarshipDraft::new("source.toml".into(), source.into()).is_err());
        }
        let mut draft = draft();
        for symbol in ["\x1b[2J", "bad\ntext", "\u{202e}"] {
            assert!(draft.edit(ModuleEdit::Symbol(symbol.into())).is_err());
        }
        assert!(draft.edit(ModuleEdit::Style("bold nope".into())).is_err());
        assert!(draft.edit(ModuleEdit::Color("#bad".into())).is_err());
        assert!(draft.edit(ModuleEdit::Layout(999)).is_err());
        assert_eq!(draft.contents(), SOURCE);
    }
    #[test]
    fn palette_color_and_weight_keep_background_and_other_attributes() {
        let mut draft = draft();
        assert_eq!(draft.color().as_deref(), Some("#f275a0"));
        draft
            .edit(ModuleEdit::Style("bold italic bg:blue pink".into()))
            .unwrap();
        draft.edit(ModuleEdit::Color("#123456".into())).unwrap();
        draft.edit(ModuleEdit::Bold(false)).unwrap();
        assert_eq!(draft.style(), "italic bg:blue fg:#123456");
    }
    #[test]
    fn optional_styles_inherit_the_module_style_until_explicitly_edited() {
        let mut draft = draft();
        for id in ["directory", "status"] {
            draft
                .select_module(crate::starship_modules::module_index(id).unwrap())
                .unwrap();
            draft
                .edit(ModuleEdit::Style("bold italic bg:blue pink".into()))
                .unwrap();
            draft.style = if id == "directory" { 2 } else { 1 };
            assert_eq!(draft.style(), "bold italic bg:blue pink");
            draft.edit(ModuleEdit::Color("#123456".into())).unwrap();
            assert_eq!(draft.style(), "bold italic bg:blue fg:#123456");
            assert_eq!(
                draft.field("style").as_deref(),
                Some("bold italic bg:blue pink")
            );
        }
    }
    #[test]
    fn literal_symbols_and_version_choices_are_safe() {
        let mut draft = draft();
        draft
            .edit(ModuleEdit::Symbol("[$x](red)\\'🦀 ".into()))
            .unwrap();
        assert_eq!(draft.literal_symbol(), "[$x](red)\\'🦀 ");
        draft.edit(ModuleEdit::Layout(2)).unwrap();
        draft.edit(ModuleEdit::Version(2)).unwrap();
        assert_eq!(draft.field("format").as_deref(), Some(VERSION_LAYOUTS[1]));
        draft.edit(ModuleEdit::Layout(0)).unwrap();
        assert_eq!(draft.field("format"), field(SOURCE, "rust", "format"));
        draft.edit(ModuleEdit::Version(0)).unwrap();
        assert!(draft.field("version_format").is_none());
    }
    #[test]
    fn handles_absent_inline_and_dotted_rust_tables() {
        for source in [
            "format = '$rust'\n",
            "rust = { symbol = 'rs ' }\n",
            "rust.symbol = 'rs '\n",
        ] {
            let mut draft = StarshipDraft::new("source.toml".into(), source.into()).unwrap();
            draft.edit(ModuleEdit::Symbol("🦀 ".into())).unwrap();
            let doc: toml::Table = toml::from_str(draft.contents()).unwrap();
            assert_eq!(doc["rust"]["symbol"].as_str(), Some("🦀 "));
        }
    }
    #[test]
    fn export_blocks_original_symlinks_and_hardlinks() {
        let temp = tempfile::tempdir().unwrap();
        let original = temp.path().join("starship.toml");
        std::fs::write(&original, SOURCE).unwrap();
        let draft = StarshipDraft::new(original.clone(), SOURCE.into()).unwrap();
        assert!(draft.validate_destination(&original).is_err());
        let link = temp.path().join("alias.toml");
        std::os::unix::fs::symlink(&original, &link).unwrap();
        assert!(draft.validate_destination(&link).is_err());
        let hard = temp.path().join("hard.toml");
        std::fs::hard_link(&original, &hard).unwrap();
        assert!(draft.validate_destination(&hard).is_err());
        assert!(
            draft
                .validate_destination(&temp.path().join(".bashrc"))
                .is_err()
        );
        assert!(
            draft
                .validate_destination(&temp.path().join("copy.toml"))
                .is_ok()
        );
        assert_eq!(std::fs::read_to_string(original).unwrap(), SOURCE);
    }
    #[test]
    fn save_as_recovers_a_draft_after_its_original_is_deleted() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("starship.toml");
        let draft = StarshipDraft::new(source.clone(), SOURCE.into()).unwrap();
        assert!(draft.validate_destination(&source).is_err());
        assert!(
            draft
                .validate_destination(&temp.path().join("recovered.toml"))
                .is_ok()
        );
        assert!(!source.exists());
    }
}
