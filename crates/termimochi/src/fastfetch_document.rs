//! Bounded, lossless JSONC edits. Parsing never runs the imported document.
use crate::greeting_fields::{self, FieldStyle};
use jsonc_parser::{
    ParseOptions,
    cst::{CstInputValue, CstNode, CstRootNode},
};
use serde_json::{Value, json};
use std::collections::BTreeSet;

pub(crate) const LIMIT: u64 = 512 * 1024;
pub(crate) const MAX_MODULES: usize = 128;

fn options() -> ParseOptions {
    ParseOptions {
        allow_comments: true,
        allow_trailing_commas: true,
        allow_loose_object_property_names: false,
        allow_missing_commas: false,
        allow_single_quoted_strings: false,
        allow_hexadecimal_numbers: false,
        allow_unary_plus_numbers: false,
    }
}

// Check nesting before entering the recursive third-party parser. Comments and
// escaped string contents are not structural tokens.
fn preflight(text: &str) -> Result<(), String> {
    if text.len() as u64 > LIMIT {
        return Err("Fastfetch configuration exceeds 512 KiB.".into());
    }
    let mut chars = text.chars().peekable();
    let mut string = false;
    let mut depth = 0usize;
    while let Some(ch) = chars.next() {
        if string {
            if ch == '\\' {
                chars.next();
            } else if ch == '"' {
                string = false;
            }
        } else if ch == '"' {
            string = true;
        } else if ch == '/' && chars.peek() == Some(&'/') {
            chars.next();
            for ch in chars.by_ref() {
                if ch == '\n' {
                    break;
                }
            }
        } else if ch == '/' && chars.peek() == Some(&'*') {
            chars.next();
            while let Some(ch) = chars.next() {
                if ch == '*' && chars.peek() == Some(&'/') {
                    chars.next();
                    break;
                }
            }
        } else if ch == '{' || ch == '[' {
            depth += 1;
            if depth > 32 {
                return Err("Configuration nesting exceeds 32 levels.".into());
            }
        } else if ch == '}' || ch == ']' {
            depth = depth.saturating_sub(1);
        }
    }
    Ok(())
}

fn unique_properties(node: &CstNode) -> Result<(), String> {
    if let Some(object) = node.as_object() {
        let mut names = BTreeSet::new();
        for prop in object.properties() {
            let name = prop
                .name()
                .and_then(|name| name.decoded_value().ok())
                .ok_or("Invalid property name")?;
            if !names.insert(name.clone()) {
                return Err(format!(
                    "Duplicate JSONC property: {name}. Nothing was imported."
                ));
            }
            if let Some(value) = prop.value() {
                unique_properties(&value)?;
            }
        }
    } else if let Some(array) = node.as_array() {
        for value in array.elements() {
            unique_properties(&value)?;
        }
    }
    Ok(())
}

pub(crate) fn parse(source: &str) -> Result<CstRootNode, String> {
    preflight(source)?;
    let root = CstRootNode::parse(source, &options()).map_err(|e| format!("Invalid JSONC: {e}"))?;
    let node = root.value().ok_or("Empty configuration")?;
    if node.as_object().is_none() {
        return Err("Fastfetch configuration must be an object.".into());
    }
    unique_properties(&node)?;
    let value = root.to_serde_value().ok_or("Invalid JSONC values")?;
    if let Some(modules) = value.get("modules") {
        let modules = modules.as_array().ok_or("modules must be an array")?;
        if modules.len() > MAX_MODULES {
            return Err("At most 128 Fastfetch fields can be imported.".into());
        }
        if modules.iter().any(|m| {
            let name = m.as_str().or_else(|| m.get("type").and_then(Value::as_str));
            !name.is_some_and(|s| {
                !s.is_empty()
                    && s.len() <= 64
                    && s.bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
            })
        }) {
            return Err("Each module needs a short ASCII type name.".into());
        }
    }
    Ok(root)
}

pub(crate) fn value(source: &str) -> Result<Value, String> {
    parse(source)?
        .to_serde_value()
        .ok_or("Invalid JSONC value".into())
}

pub(crate) fn edit_fields<'a>(
    source: &str,
    styles: impl IntoIterator<Item = (usize, &'a FieldStyle)>,
) -> Result<String, String> {
    let root = parse(source)?;
    for (index, style) in styles {
        let modules = root
            .object_value()
            .and_then(|o| o.get("modules"))
            .and_then(|p| p.value())
            .and_then(|n| n.as_array())
            .ok_or("No explicit module list to edit")?;
        let node = modules
            .elements()
            .get(index)
            .cloned()
            .ok_or("Unknown field")?;
        let mut edited = node.to_serde_value().ok_or("Invalid module")?;
        style.apply(&mut edited)?;
        let object = if let Some(object) = node.as_object() {
            object
        } else {
            node.as_string_lit()
                .ok_or("Invalid module")?
                .replace_with(CstInputValue::Object(vec![(
                    "type".into(),
                    CstInputValue::String(greeting_fields::module_kind(&edited).into()),
                )]))
                .and_then(|n| n.as_object())
                .ok_or("Could not edit module")?
        };
        for (property, changed) in [
            ("key", style.label.is_some() || style.icon.is_some()),
            ("keyColor", style.key_color.is_some()),
            ("outputColor", style.value_color.is_some()),
            ("format", style.format.is_some()),
        ] {
            if changed {
                let input = CstInputValue::String(
                    edited[property]
                        .as_str()
                        .ok_or("Invalid field edit")?
                        .into(),
                );
                if let Some(prop) = object.get(property) {
                    prop.set_value(input);
                } else {
                    object.append(property, input);
                }
            }
        }
    }
    let result = root.to_string();
    parse(&result)?;
    Ok(result)
}

#[cfg(test)]
fn edit_field(source: &str, index: usize, style: &FieldStyle) -> Result<String, String> {
    edit_fields(source, [(index, style)])
}

/// Safe preview is a projection; the original JSONC is always used for export.
/// Only local built-ins and reviewed scalar formatting enter Fastfetch. Unknown
/// modules/properties, commands, image/file logos and networks are never run.
#[cfg(test)]
pub(crate) fn preview_config(source: &str) -> Result<(Value, Vec<String>), String> {
    preview_config_with_logo(source, None)
}
pub(crate) fn preview_config_with_logo(
    source: &str,
    snapshot: Option<&crate::greeting_art::LogoSnapshot>,
) -> Result<(Value, Vec<String>), String> {
    let input = value(source)?;
    let mut notes = Vec::new();
    let mut modules = Vec::new();
    if let Some(items) = input["modules"].as_array() {
        for (index, item) in items.iter().enumerate() {
            let kind = greeting_fields::module_kind(item);
            if !greeting_fields::editable(kind)
                && !matches!(kind, "title" | "separator" | "break" | "colors")
            {
                notes.push(format!("Field {} · {kind}: preserved, not run in preview. Command/network/custom modules require separate review.", index + 1));
                continue;
            }
            let mut module = json!({"type":kind});
            if let Some(object) = item.as_object() {
                for (key, value) in object {
                    if key == "type" {
                        continue;
                    }
                    if matches!(
                        key.as_str(),
                        "key" | "keyIcon" | "keyColor" | "outputColor" | "format"
                    ) && value
                        .as_str()
                        .is_some_and(|s| greeting_fields::safe_text(s, 1024))
                        || key == "keyWidth" && value.as_u64().is_some_and(|v| v <= 80)
                    {
                        module[key] = value.clone();
                    } else {
                        notes.push(format!(
                            "Field {} · {kind}.{key}: preserved in export; not simulated.",
                            index + 1
                        ));
                    }
                }
            }
            // Disk previews target only the root volume; imported paths never
            // cause a probe of a user-specified mount or network filesystem.
            if kind == "disk" {
                module["folders"] = json!("/");
            }
            modules.push(module);
        }
    } else {
        notes.push("No explicit modules array: Fastfetch defaults are preserved; add explicit fields externally to edit them here.".into());
        modules = vec![
            json!("os"),
            json!("kernel"),
            json!("shell"),
            json!("cpu"),
            json!("memory"),
        ];
    }
    if modules.is_empty() {
        modules.push(json!({"type":"custom","format":"No supported fields to preview"}));
    }
    let mut config = json!({"modules":modules, "logo":{"type":"none"}});
    if let Some(display) = input["display"].as_object() {
        for (key, item) in display {
            if key == "separator"
                && item
                    .as_str()
                    .is_some_and(|s| greeting_fields::safe_text(s, 16))
            {
                config["display"][key] = item.clone();
            } else {
                notes.push(format!(
                    "display.{key}: retained in export; offline preview uses safe defaults."
                ));
            }
        }
    } else if input.get("display").is_some() {
        notes.push("display: expected an object; preserved but not simulated.".into());
    }
    if let Some(logo) = input.get("logo") {
        config["logo"] = crate::greeting_art::preview_logo(logo, snapshot, &mut notes);
    }
    if let Some(object) = input.as_object() {
        for key in object
            .keys()
            .filter(|k| !matches!(k.as_str(), "$schema" | "modules" | "display" | "logo"))
        {
            notes.push(format!(
                "{key}: preserved read-only; not passed to the preview process."
            ));
        }
    }
    Ok((config, notes))
}

pub(crate) fn replace_logo_art(
    source: &str,
    art: &crate::greeting_art::Artwork,
) -> Result<String, String> {
    art.validate()?;
    let root = parse(source)?;
    let object = root.object_value().ok_or("Missing configuration object")?;
    let input = CstInputValue::Object(vec![
        ("type".into(), CstInputValue::String("data-raw".into())),
        ("source".into(), CstInputValue::String(art.ansi.clone())),
    ]);
    if let Some(prop) = object.get("logo") {
        if let Some(logo) = prop.value().and_then(|v| v.as_object()) {
            for (key, text) in [("type", "data-raw"), ("source", art.ansi.as_str())] {
                let input = CstInputValue::String(text.into());
                if let Some(p) = logo.get(key) {
                    p.set_value(input);
                } else {
                    logo.append(key, input);
                }
            }
        } else {
            prop.set_value(input);
        }
    } else {
        object.append("logo", input);
    }
    let result = root.to_string();
    parse(&result)?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn jsonc_edits_preserve_comments_unknown_settings_and_untouched_modules() {
        let source = "// keep\r\n{\r\n  \"future\": {\"x\":1,}, /* keep block */\r\n  \"modules\": [ { \"type\":\"cpu\", \"format\":\"{name}\", \"future\":7 }, /* separator */ {\"type\":\"command\",\"text\":\"touch NEVER\"}, ],\r\n}\r\n";
        assert_eq!(parse(source).unwrap().to_string(), source);
        let edited = edit_field(
            source,
            0,
            &FieldStyle {
                label: Some("Processor".into()),
                ..Default::default()
            },
        )
        .unwrap();
        for retained in [
            "// keep\r\n",
            "/* keep block */",
            "/* separator */",
            "\"future\":7",
            "{\"type\":\"command\",\"text\":\"touch NEVER\"}",
        ] {
            assert!(edited.contains(retained), "{retained}");
        }
        assert_eq!(value(&edited).unwrap()["modules"][0]["key"], "Processor");
        assert!(edit_field(source, 1, &FieldStyle::default()).is_err());
        let (preview, notes) = preview_config(&edited).unwrap();
        assert!(!preview.to_string().contains("NEVER"));
        assert!(!preview.to_string().contains("future"));
        assert!(!notes.is_empty());
    }
    #[test]
    fn hostile_ambiguous_or_deep_documents_are_rejected_before_mutation() {
        for source in [
            "{\"modules\":[],\"modules\":[]}".to_owned(),
            "{\"x\":1,\"\\u0078\":2}".into(),
            "{modules:[]}".into(),
            "{\"modules\":[1]}".into(),
            "{\"x\":1 \"y\":2}".into(),
            " ".repeat(LIMIT as usize + 1),
            format!("{{\"x\":{}0{}}}", "[".repeat(40), "]".repeat(40)),
        ] {
            assert!(parse(&source).is_err(), "{source}");
        }
        assert!(parse("{/* [] {{{ */ \"url\":\"https://example.com/\\\"//\",}").is_ok());
    }
    #[test]
    fn image_command_and_custom_modules_are_never_projected() {
        let source = r#"{"logo":{"type":"file","source":"$(touch NEVER)"},"general":{"preRun":"NEVER"},"modules":["publicip",{"type":"command","text":"NEVER"},{"type":"custom","format":"{$SECRET}"},"memory"]}"#;
        let (preview, notes) = preview_config(source).unwrap();
        assert_eq!(preview["modules"].as_array().unwrap().len(), 1);
        assert!(!preview.to_string().contains("NEVER"));
        assert!(!preview.to_string().contains("SECRET"));
        assert_eq!(notes.len(), 5);
        assert_eq!(parse(source).unwrap().to_string(), source);
    }
    #[test]
    fn designer_artwork_can_be_imported_but_osc_and_cursor_controls_cannot() {
        let settings = crate::greeting::GreetingSettings {
            enabled: true,
            ..Default::default()
        };
        let source = settings.fastfetch_config().unwrap();
        let original = value(&source).unwrap();
        let (preview, _) = preview_config(&source).unwrap();
        assert_eq!(preview["logo"]["source"], original["logo"]["source"]);
        for text in ["\x1b]52;c;secret\x07", "\x1b[2J", "bad\u{202e}text"] {
            let source =
                json!({"logo":{"type":"data-raw","source":text},"modules":["os"]}).to_string();
            let (preview, notes) = preview_config(&source).unwrap();
            assert!(!preview.to_string().contains("secret"));
            assert!(!preview.to_string().contains("202e"));
            assert!(notes.iter().any(|n| n.contains("filtered")));
            if text.starts_with("bad") {
                assert_eq!(preview["logo"]["source"], "badtext");
            } else {
                assert_eq!(preview["logo"]["type"], "none");
            }
        }
    }
}
