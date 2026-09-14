//! Per-field recovery of recorded writes, with durable completion for retries.
//! Preview/reference fields (before == after) are never part of the restore set.
use crate::typography_preset::{read_private, write_checked_with_limit};
use gtk::{gio, glib, prelude::*};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, path::Path};

pub(crate) struct Field {
    pub settings: gio::Settings,
    pub key: &'static str,
    pub before: Option<glib::Variant>,
    pub after: Option<glib::Variant>,
    pub reviewed: Option<glib::Variant>,
}

#[derive(Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Progress {
    receipt: String,
    done: BTreeSet<String>,
}

pub(crate) fn restore(
    directory: Option<&Path>,
    name: &str,
    receipt: &impl Serialize,
    fields: Vec<Field>,
) -> Result<(), String> {
    let fingerprint = glib::compute_checksum_for_data(
        glib::ChecksumType::Sha256,
        &serde_json::to_vec(receipt).map_err(|e| e.to_string())?,
    )
    .unwrap()
    .to_string();
    let path =
        directory.map(|directory| directory.join(format!("{name}-restore-{fingerprint}.json")));
    let mut expected = path.as_deref().map(read_private).transpose()?.flatten();
    let mut progress: Progress = match &expected {
        Some(bytes) => {
            serde_json::from_slice(bytes).map_err(|e| format!("Invalid restore progress: {e}"))?
        }
        None => Progress {
            receipt: fingerprint.clone(),
            ..Default::default()
        },
    };
    if progress.receipt != fingerprint
        || progress
            .done
            .iter()
            .any(|key| !fields.iter().any(|f| f.key == key))
    {
        return Err(
            "Restore progress does not match this application receipt. Nothing was changed.".into(),
        );
    }
    let mut results = Vec::new();
    let mut blocked = false;
    for field in fields {
        if field.before == field.after {
            continue;
        }
        if progress.done.contains(field.key) {
            results.push(format!(
                "{}: restored previously; not touched again",
                field.key
            ));
            continue;
        }
        let result = restore_field(&field);
        match result {
            Ok(()) => {
                progress.done.insert(field.key.into());
                if let Some(path) = &path {
                    let bytes = serde_json::to_vec_pretty(&progress).map_err(|e| e.to_string())?;
                    write_checked_with_limit(path, &bytes, &expected, 1024 * 1024)
                        .map_err(|e| format!("{} restored, but progress could not be recorded: {e}. Other fields were not attempted.", field.key))?;
                    expected = Some(bytes);
                }
                results.push(format!(
                    "{}: restored (including unset inheritance)",
                    field.key
                ));
            }
            Err(error) => {
                blocked = true;
                results.push(format!("{}: kept — {error}", field.key));
            }
        }
    }
    if blocked {
        Err(format!(
            "Per-field recovery incomplete; each field result is listed below.\n{}",
            results.join("\n")
        ))
    } else {
        Ok(())
    }
}

fn restore_field(field: &Field) -> Result<(), String> {
    let settings = &field.settings;
    let schema = settings
        .settings_schema()
        .filter(|s| s.has_key(field.key))
        .ok_or("setting unavailable")?;
    let current = settings.user_value(field.key);
    if current == field.before {
        return Ok(());
    }
    if current != field.after {
        return Err("changed outside TermiMochi".into());
    }
    if current != field.reviewed {
        return Err("changed during confirmation; review again".into());
    }
    if !settings.is_writable(field.key) {
        return Err("setting locked".into());
    }
    if field
        .before
        .as_ref()
        .is_some_and(|v| !schema.key(field.key).range_check(v))
    {
        return Err("invalid backup value".into());
    }
    // One setting per transaction: never write a snapshot of unrelated keys.
    settings.delay();
    if settings.user_value(field.key) != current {
        return Err("changed before restoration".into());
    }
    match &field.before {
        Some(value) => settings
            .set_value(field.key, value)
            .map_err(|e| e.to_string())?,
        None => settings.reset(field.key),
    }
    settings.apply();
    gio::Settings::sync();
    if settings.user_value(field.key) != field.before {
        return Err("restore did not persist".into());
    }
    Ok(())
}
