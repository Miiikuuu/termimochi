//! Resolve only proven palette bindings; never guess imported RGB or SGR styles.
use super::*;
use crate::{greeting::GreetingSettings, greeting_fields, starship_modules::MODULES};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Source {
    Designer(PromptSegmentKind),
    Greeting,
    Field(usize),
    Starship { module: usize, style: usize },
}

#[derive(Clone)]
pub(super) struct Role {
    pub label: String,
    pub key: Option<String>,
    pub source: Source,
}

pub(super) fn slot_key(slot: u8) -> String {
    if slot == 16 {
        "Foreground".into()
    } else {
        format!("Color{slot}")
    }
}

fn ansi_key(value: &serde_json::Value) -> Option<String> {
    let code = value.as_str()?.parse::<u8>().ok()?;
    match code {
        30..=37 => Some(slot_key(code - 30)),
        90..=97 => Some(slot_key(code - 90 + 8)),
        39 => Some(slot_key(16)),
        _ => None,
    }
}

pub(super) fn greeting_roles(settings: &GreetingSettings) -> Vec<Role> {
    let imported = settings
        .imported_source
        .as_ref()
        .and_then(|source| crate::fastfetch_document::value(source).ok());
    let official = settings.official_preset.map(|preset| preset.config());
    let global = |key: bool| -> Option<String> {
        if settings.imported_source.is_none() {
            return Some(slot_key(if key { settings.accent } else { 16 }));
        }
        let display = &imported.as_ref()?["display"];
        // Missing imported key colors can depend on the native distro logo.
        // Complex global strings / bright-key behavior stay source-owned.
        if key && display["brightColor"].as_bool() != Some(false) {
            return None;
        }
        let colors = &display["color"];
        if let Some(color) = colors.get(if key { "keys" } else { "output" }) {
            return ansi_key(color);
        }
        if !key && (colors.is_null() || colors.is_object()) {
            return Some(slot_key(16));
        }
        None
    };
    let mut roles = vec![
        Role {
            label: "Accent".into(),
            key: global(true),
            source: Source::Greeting,
        },
        Role {
            label: "Content text".into(),
            key: global(false),
            source: Source::Greeting,
        },
    ];
    let indices: Vec<usize> = if let Some(config) = imported.as_ref() {
        (0..config["modules"].as_array().map_or(0, Vec::len)).collect()
    } else if settings.official_preset.is_some() {
        settings
            .official_items
            .iter()
            .filter(|item| item.enabled)
            .map(|item| usize::from(item.id))
            .collect()
    } else {
        settings
            .items
            .iter()
            .filter(|item| item.enabled)
            .filter_map(|item| {
                crate::greeting::Info::ALL
                    .iter()
                    .position(|kind| *kind == item.kind)
            })
            .collect()
    };
    for index in indices {
        let id = settings.style_id(index);
        // Parse native JSONC once for the whole role list, not once per field.
        let module = if let Some(config) = imported.as_ref().or(official.as_ref()) {
            config["modules"].get(index).cloned()
        } else {
            settings.module_for_field_id(&id)
        };
        let Some(module) = module else {
            continue;
        };
        let kind = greeting_fields::module_kind(&module);
        if !greeting_fields::editable(kind) {
            continue;
        }
        let style = settings.field_styles.get(&id);
        // Kind plus source index disambiguates repeated native modules and
        // avoids rendering arbitrary imported control sequences in labels.
        for key in [true, false] {
            let override_slot = style.and_then(|style| {
                if key {
                    style.key_color
                } else {
                    style.value_color
                }
            });
            let binding = if let Some(slot) = override_slot {
                if key
                    && imported.as_ref().is_some_and(|config| {
                        config["display"]["brightColor"].as_bool() != Some(false)
                    })
                {
                    None
                } else {
                    Some(slot_key(slot))
                }
            } else if let Some(value) = module.get(if key { "keyColor" } else { "outputColor" }) {
                // Imported key styles can inherit native bold/bright behavior.
                if key
                    && imported.as_ref().is_some_and(|config| {
                        config["display"]["brightColor"].as_bool() != Some(false)
                    })
                {
                    None
                } else {
                    ansi_key(value)
                }
            } else {
                global(key)
            };
            roles.push(Role {
                label: format!(
                    "{} · {} · {}",
                    kind.to_uppercase(),
                    index + 1,
                    if key { "Name" } else { "Content" }
                ),
                key: binding,
                source: Source::Field(index),
            });
        }
    }
    roles
}

pub(super) fn starship_roles() -> Vec<Role> {
    MODULES
        .iter()
        .enumerate()
        .flat_map(|(module, spec)| {
            spec.styles
                .iter()
                .enumerate()
                .map(move |(style, field)| Role {
                    label: format!("{} · {}", spec.label, field.label),
                    key: None,
                    source: Source::Starship { module, style },
                })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::greeting_fields::FieldStyle;

    #[test]
    fn field_overrides_do_not_disable_unrelated_shared_colors() {
        let mut settings = GreetingSettings::default();
        let index = crate::greeting::Info::ALL
            .iter()
            .position(|kind| *kind == crate::greeting::Info::Os)
            .unwrap();
        settings.field_styles.insert(
            settings.style_id(index),
            FieldStyle {
                key_color: Some(4),
                ..Default::default()
            },
        );
        let roles = greeting_roles(&settings);
        assert_eq!(roles[0].key, Some("Color6".into()));
        let names: Vec<_> = roles
            .iter()
            .filter(|r| r.source == Source::Field(index))
            .collect();
        assert_eq!(names[0].key, Some("Color4".into()));
        assert_eq!(names[1].key, Some("Foreground".into()));
    }

    #[test]
    fn imported_rgb_and_repeated_fields_keep_distinct_source_ownership() {
        let settings = GreetingSettings {
            imported_source: Some(r##"{"display":{"brightColor":false,"color":{"keys":"36"}},"modules":[{"type":"os","keyColor":"#abcdef"},{"type":"os","keyColor":"34","outputColor":"91"}]}"##.into()),
            ..Default::default()
        };
        let roles = greeting_roles(&settings);
        assert_eq!(roles[2].source, Source::Field(0));
        assert_eq!(roles[2].key, None);
        assert_eq!(roles[4].source, Source::Field(1));
        assert_eq!(roles[4].key, Some("Color4".into()));
        assert_eq!(roles[5].key, Some("Color9".into()));
        assert_ne!(roles[2].label, roles[4].label);
        for value in ["#abcdef", "1;34", "38;5;4", "999", ""] {
            assert!(ansi_key(&serde_json::json!(value)).is_none());
        }
    }
}
