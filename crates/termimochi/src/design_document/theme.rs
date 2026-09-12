//! Sparse theme intent. Editors remain the only mutable values; these immutable
//! snapshots describe authored overrides, never materialized preview defaults.
use super::*;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

pub(crate) type Fields = BTreeMap<String, Value>;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ThemeIntent {
    pub name: String,
    pub light: bool,
    pub typography: Fields,
    pub layout: Fields,
    pub colors: BTreeMap<String, String>,
    #[serde(default)]
    pub dark_colors: BTreeMap<String, String>,
    pub inherit: BTreeSet<String>,
    pub prompt_enabled: bool,
    pub sources: Vec<NativeSource>,
}

pub(crate) fn fields<T: Serialize>(value: &T) -> Fields {
    serde_json::to_value(value)
        .expect("settings serialize")
        .as_object()
        .unwrap()
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

fn merged<T: Serialize + serde::de::DeserializeOwned>(
    base: &T,
    patch: &Fields,
) -> Result<T, String> {
    let mut value = serde_json::to_value(base).map_err(|e| e.to_string())?;
    let map = value.as_object_mut().ok_or("Settings must be an object")?;
    for (key, value) in patch {
        if !map.contains_key(key) {
            return Err(format!("Unknown theme field: {key}"));
        }
        map.insert(key.clone(), value.clone());
    }
    serde_json::from_value(value).map_err(|e| e.to_string())
}

pub(crate) fn kitty_field(group: &str, field: &str) -> Option<&'static str> {
    match (group, field) {
        ("typography", "family") => Some("font_family"),
        ("typography", "size") => Some("font_size"),
        ("typography", "line_height") => Some("modify_font cell_height"),
        ("typography", "cell_width") => Some("modify_font cell_width"),
        ("layout", "content_padding") => Some("window_padding_width"),
        ("layout", "window_spacing") => Some("window_margin_width"),
        ("layout", "columns") => Some("initial_window_width"),
        ("layout", "rows") => Some("initial_window_height"),
        ("layout", "cursor_shape") => Some("cursor_shape"),
        ("layout", "cursor_blink") => Some("cursor_blink_interval"),
        ("layout", field) => crate::layout::window_top::kitty_field(field),
        _ => None,
    }
}

impl ThemeIntent {
    pub fn new(name: &str, light: bool) -> Self {
        Self {
            name: name.into(),
            light,
            typography: Fields::new(),
            layout: Fields::new(),
            colors: BTreeMap::new(),
            dark_colors: BTreeMap::new(),
            inherit: BTreeSet::new(),
            prompt_enabled: false,
            sources: Vec::new(),
        }
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.name.trim().is_empty()
            || self.name.len() > 160
            || self.name.chars().count() > 128
            || self.name.chars().any(char::is_control)
        {
            return Err("Theme name must contain 1–160 printable bytes.".into());
        }
        merged(&TypographySettings::default(), &self.typography)?.validate()?;
        merged(&LayoutSettings::default(), &self.layout)?.validate()?;
        for (k, v) in self.colors.iter().chain(&self.dark_colors) {
            let known = termimochi_core::KNOWN_COLOR_KEYS.contains(&k.as_str());
            if !known || v.parse::<termimochi_core::Rgb>().is_err() {
                return Err("Invalid theme color override".into());
            }
        }
        for field in &self.inherit {
            let Some((group, key)) = field.split_once('.') else {
                return Err("Invalid inherited field".into());
            };
            let valid = match group {
                "typography" => fields(&TypographySettings::default()).contains_key(key),
                "layout" => fields(&LayoutSettings::default()).contains_key(key),
                _ => false,
            };
            if !valid {
                return Err("Unknown inherited theme field".into());
            }
        }
        if self.sources.len() > 64
            || self.sources.iter().map(|s| s.text.len()).sum::<usize>() > 12 * 1024 * 1024
        {
            return Err("Too much retained theme provenance".into());
        }
        Ok(())
    }
    pub fn project(&self, workspace: &mut Workspace) {
        workspace.typography =
            merged(&workspace.typography, &self.typography).expect("validated sparse font");
        workspace.layout =
            merged(&workspace.layout, &self.layout).expect("validated sparse layout");
        workspace.light = self.light;
        if let Ok(mut palette) = workspace.palette() {
            let _ = palette.set_name(&self.name);
            for (kind, colors) in [
                (Variant::Light, &self.colors),
                (Variant::Dark, &self.dark_colors),
            ] {
                if let Some(variant) = palette.variant_mut(kind) {
                    for (key, value) in colors {
                        if let Ok(color) = value.parse() {
                            let _ = variant.set(key, color);
                        }
                    }
                }
            }
            workspace.palette = palette.to_palette_string();
        }
    }
    pub fn retain(&mut self, source: NativeSource) {
        if !self.sources.contains(&source) {
            self.sources.push(source);
        }
    }
}

impl DesignDocument {
    pub fn new_theme(target: TargetHint, name: &str, light: bool) -> Self {
        Self {
            schema: "termimochi-design".into(),
            version: 3,
            id: gtk::glib::uuid_string_random().into(),
            kind: Kind::Project,
            components: Components::default(),
            native: None,
            target_hint: Some(target),
            theme: Some(ThemeIntent::new(name, light)),
        }
    }
    /// In-memory migration only. ID is retained; native sources are inert.
    pub fn into_theme(mut self, target: TargetHint, name: &str) -> Result<Self, String> {
        if self.theme.is_some() {
            if self.target_hint != Some(target) {
                return Err("Changing terminal requires a theme copy".into());
            }
            return Ok(self);
        }
        let mut theme = ThemeIntent::new(
            name,
            self.components.palette.as_ref().is_none_or(|p| p.light),
        );
        if let Some(font) = self.components.typography.take() {
            theme.typography = fields(&font);
        }
        if let Some(layout) = self.components.layout.take() {
            theme.layout = fields(&layout);
        }
        theme.prompt_enabled = self.components.prompt.is_some();
        if self.components.artwork.is_some() {
            self.components.greeting = self.greeting_output();
            self.components.artwork = None;
        }
        if let Some(source) = self.native.take() {
            theme.retain(source.clone());
            if source.format == NativeFormat::Kitty && target == TargetHint::Kitty {
                self.native = Some(source);
            }
        }
        self.version = 3;
        self.kind = Kind::Project;
        self.target_hint = Some(target);
        self.theme = Some(theme);
        self.validate()?;
        Ok(self)
    }
    pub fn theme_scope(&self) -> Scope {
        let t = self.theme.as_ref().expect("theme");
        let mut s = Scope {
            palette: self.components.palette.is_some()
                || !t.colors.is_empty()
                || !t.dark_colors.is_empty(),
            typography: !t.typography.is_empty(),
            layout: !t.layout.is_empty(),
            prompt: self.components.prompt.is_some(),
            greeting: self.components.greeting.is_some(),
            artwork: false,
        };
        if let Some(native) = self.theme_native_source()
            && let Ok(doc) = crate::kitty_document::KittyDocument::parse(&native)
        {
            let (p, f, l) = doc.ownership();
            s.palette |= p;
            s.typography |= f;
            s.layout |= l;
        }
        s
    }
    pub fn theme_native_source(&self) -> Option<String> {
        let native = self
            .native
            .as_ref()
            .filter(|n| n.format == NativeFormat::Kitty)?;
        let theme = self.theme.as_ref()?;
        let excluded: BTreeSet<_> = theme
            .inherit
            .iter()
            .filter_map(|f| f.split_once('.').and_then(|(g, k)| kitty_field(g, k)))
            .collect();
        Some(
            native
                .text
                .split_inclusive('\n')
                .filter(|line| {
                    let mut p = line.split_whitespace();
                    let key = p.next().unwrap_or("");
                    let key = if key == "modify_font" {
                        format!("modify_font {}", p.next().unwrap_or(""))
                    } else {
                        key.into()
                    };
                    !excluded.contains(key.as_str())
                })
                .collect(),
        )
    }
    /// Compare actual editor values to their immutable opening projection. Page
    /// changes/Inspect/redraw do not produce edits. Undo to an unspecified opening
    /// value therefore restores inheritance, not a default-valued override.
    pub fn capture_theme(&self, opening: &Workspace, current: &Workspace) -> Result<Self, String> {
        let mut result = self.clone();
        let theme = result.theme.as_mut().ok_or("Not a theme")?;
        for (group, before, now, patch) in [
            (
                "typography",
                fields(&opening.typography),
                fields(&current.typography),
                &mut theme.typography,
            ),
            (
                "layout",
                fields(&opening.layout),
                fields(&current.layout),
                &mut theme.layout,
            ),
        ] {
            for (key, value) in now {
                if before.get(&key) != Some(&value) {
                    patch.insert(key.clone(), value);
                    theme.inherit.remove(&format!("{group}.{key}"));
                }
            }
        }
        let before = opening.palette()?;
        let now = current.palette()?;
        if before.name() != now.name() {
            theme.name = now.name().to_owned();
        }
        for (variant, patch) in [
            (Variant::Light, &mut theme.colors),
            (Variant::Dark, &mut theme.dark_colors),
        ] {
            if let (Some(old), Some(now)) = (before.variant(variant), now.variant(variant)) {
                for (key, color) in now.colors() {
                    if old.get(key) != Some(*color) {
                        patch.insert(key.clone(), color.to_hex());
                    }
                }
            }
        }
        theme.light = current.light;
        if current.designer != opening.designer
            || current.starship != opening.starship
            || current.use_designer != opening.use_designer
        {
            if result.components.prompt.is_none() {
                theme.prompt_enabled = true;
            }
            result.components.prompt = Some(PromptComponent {
                designer: current.designer.clone(),
                starship: current.starship.clone(),
                use_designer: current.use_designer,
            });
        }
        if current.greeting != opening.greeting {
            result.components.greeting = Some(current.greeting.clone());
        }
        result.validate()?;
        Ok(result)
    }
    pub fn theme_kitty_configuration(&self, effective: &Workspace) -> Result<String, String> {
        let theme = self.theme.as_ref().ok_or("Not a theme")?;
        let source = self.theme_native_source().unwrap_or_else(|| {
            "# TermiMochi theme: unspecified fields inherit Kitty defaults\n".into()
        });
        let native = crate::kitty_document::KittyDocument::parse(&source)?;
        let all = crate::kitty_document::workspace_properties(effective)?;
        let mut edits = BTreeMap::new();
        for (key, value) in &all {
            if crate::kitty_document::category(key) == Some(0)
                && (self.components.palette.is_some()
                    || crate::kitty_document::palette_key(key).is_some_and(|k| {
                        if theme.light {
                            theme.colors.contains_key(&k)
                        } else {
                            theme.dark_colors.contains_key(&k)
                        }
                    }))
            {
                edits.insert(key.clone(), value.clone());
            }
        }
        for (group, patch) in [("typography", &theme.typography), ("layout", &theme.layout)] {
            for field in patch.keys() {
                if let Some(key) = kitty_field(group, field)
                    && let Some(value) = all.get(key)
                {
                    edits.insert(key.into(), value.clone());
                }
            }
        }
        if theme.layout.contains_key("columns") || theme.layout.contains_key("rows") {
            edits.insert("remember_window_size".into(), "no".into());
        }
        native.edited(&edits)
    }

    pub fn convert_theme_copy(
        &self,
        target: TargetHint,
        reference: &Workspace,
    ) -> Result<Self, String> {
        let mut copy = self.clone();
        let effective = self.project_preview(reference);
        let native = self
            .theme_native_source()
            .and_then(|s| crate::kitty_document::KittyDocument::parse(&s).ok());
        let theme = copy
            .theme
            .as_mut()
            .ok_or("Only themes can convert terminal")?;
        if let Some(native) = native {
            for (key, value) in native.properties() {
                if let Some(key) = crate::kitty_document::palette_key(key) {
                    if theme.light {
                        &mut theme.colors
                    } else {
                        &mut theme.dark_colors
                    }
                    .entry(key)
                    .or_insert(value.clone());
                }
            }
            for (group, values, patch) in [
                (
                    "typography",
                    fields(&effective.typography),
                    &mut theme.typography,
                ),
                ("layout", fields(&effective.layout), &mut theme.layout),
            ] {
                for (field, value) in values {
                    if kitty_field(group, &field)
                        .is_some_and(|k| native.properties().contains_key(k))
                    {
                        patch.entry(field).or_insert(value);
                    }
                }
            }
        }
        if let Some(native) = copy.native.take() {
            theme.retain(native);
        }
        copy.id = gtk::glib::uuid_string_random().into();
        copy.target_hint = Some(target);
        copy.validate()?;
        Ok(copy)
    }
}
