//! Explicit, backed-up writes for the subset of Layout supported by Ptyxis.
use crate::{
    layout::{LayoutSettings, PreviewCursorBlink, PreviewCursorShape},
    ptyxis::find_settings,
    typography_preset::{read_private, retain_backup, write_private},
};
use gtk::{gio, glib, prelude::*};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

const KEYS: [&str; 6] = [
    "cursor-shape",
    "cursor-blink-mode",
    "scrollbar-policy",
    "default-columns",
    "default-rows",
    "restore-window-size",
];
const RECEIPT: &str = "last-layout-apply.json";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
enum Value {
    Boolean(bool),
    Number(u32),
    Text(String),
}

impl Value {
    fn variant(&self) -> glib::Variant {
        match self {
            Self::Boolean(value) => value.to_variant(),
            Self::Number(value) => value.to_variant(),
            Self::Text(value) => value.to_variant(),
        }
    }
    fn read(value: &glib::Variant) -> Result<Self, String> {
        if let Some(value) = value.get::<bool>() {
            Ok(Self::Boolean(value))
        } else if let Some(value) = value.get::<u32>() {
            Ok(Self::Number(value))
        } else if let Some(value) = value.get::<String>() {
            Ok(Self::Text(value))
        } else {
            Err("Unsupported Ptyxis layout setting type.".into())
        }
    }
    fn label(&self) -> String {
        match self {
            Self::Boolean(value) => value.to_string(),
            Self::Number(value) => value.to_string(),
            Self::Text(value) => value.clone(),
        }
    }
}

type Values = BTreeMap<String, Option<Value>>;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Snapshot {
    user: Values,
    effective: Values,
}

fn validate(values: &Values, allow_unset: bool) -> Result<(), String> {
    if values.len() != KEYS.len() || KEYS.iter().any(|key| !values.contains_key(*key)) {
        return Err("The layout record has missing or unsupported settings.".into());
    }
    for (key, value) in values {
        let valid = match (key.as_str(), value) {
            (_, None) => allow_unset,
            ("cursor-shape", Some(Value::Text(value))) => {
                ["block", "ibeam", "underline"].contains(&value.as_str())
            }
            ("cursor-blink-mode", Some(Value::Text(value))) => {
                ["system", "on", "off"].contains(&value.as_str())
            }
            ("scrollbar-policy", Some(Value::Text(value))) => {
                ["system", "always", "never"].contains(&value.as_str())
            }
            ("default-columns" | "default-rows", Some(Value::Number(value))) => {
                (1..=65535).contains(value)
            }
            ("restore-window-size", Some(Value::Boolean(_))) => true,
            _ => false,
        };
        if !valid {
            return Err(format!("Invalid layout value for {key}."));
        }
    }
    Ok(())
}

fn requested(layout: &LayoutSettings) -> Values {
    let shape = match layout.cursor_shape {
        PreviewCursorShape::Block => "block",
        PreviewCursorShape::IBeam => "ibeam",
        PreviewCursorShape::Underline => "underline",
    };
    let blink = match layout.cursor_blink {
        PreviewCursorBlink::System => "system",
        PreviewCursorBlink::On => "on",
        PreviewCursorBlink::Off => "off",
    };
    [
        Value::Text(shape.into()),
        Value::Text(blink.into()),
        Value::Text(if layout.scrollbar { "always" } else { "never" }.into()),
        Value::Number(layout.columns as u32),
        Value::Number(layout.rows as u32),
        Value::Boolean(false),
    ]
    .into_iter()
    .zip(KEYS)
    .map(|(value, key)| (key.into(), Some(value)))
    .collect()
}

fn check(settings: &gio::Settings) -> Result<(), String> {
    let schema = settings
        .settings_schema()
        .ok_or("Ptyxis settings are unavailable.")?;
    for key in KEYS {
        if !schema.has_key(key) || !settings.is_writable(key) {
            return Err(format!(
                "Ptyxis setting {key} is missing or locked. Nothing was changed."
            ));
        }
    }
    Ok(())
}

fn snapshot(settings: &gio::Settings) -> Result<Snapshot, String> {
    check(settings)?;
    let user = KEYS
        .into_iter()
        .map(|key| {
            Ok((
                key.into(),
                settings
                    .user_value(key)
                    .as_ref()
                    .map(Value::read)
                    .transpose()?,
            ))
        })
        .collect::<Result<Values, String>>()?;
    let effective = KEYS
        .into_iter()
        .map(|key| Ok((key.into(), Some(Value::read(&settings.value(key))?))))
        .collect::<Result<Values, String>>()?;
    validate(&user, true)?;
    validate(&effective, false)?;
    Ok(Snapshot { user, effective })
}

fn write(settings: &gio::Settings, values: &Values) -> Result<(), String> {
    check(settings)?;
    validate(values, true)?;
    let schema = settings.settings_schema().unwrap();
    for (key, value) in values {
        if let Some(value) = value
            && !schema.key(key).range_check(&value.variant())
        {
            return Err(format!("Ptyxis does not support this {key} value."));
        }
    }
    settings.delay();
    for (key, value) in values {
        if settings.user_value(key) == value.as_ref().map(Value::variant) {
            continue;
        }
        if let Some(value) = value {
            if let Err(error) = settings.set_value(key, &value.variant()) {
                settings.revert();
                return Err(error.to_string());
            }
        } else {
            settings.reset(key);
        }
    }
    settings.apply();
    gio::Settings::sync();
    if snapshot(settings)?.user != *values {
        return Err("Ptyxis did not retain all requested layout values.".into());
    }
    Ok(())
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Receipt {
    version: u8,
    before: Snapshot,
    after: Values,
}

impl Receipt {
    fn validate(&self) -> Result<(), String> {
        if ![1, 2].contains(&self.version) {
            return Err("Unsupported layout backup version.".into());
        }
        validate(&self.before.user, true)?;
        validate(&self.before.effective, false)?;
        validate(&self.after, self.version == 2)
    }
}

fn compatible(settings: &gio::Settings, receipt: &Receipt) -> Result<(), String> {
    let current = snapshot(settings)?.user;
    if KEYS.iter().any(|key| {
        current[*key] != receipt.before.user[*key] && current[*key] != receipt.after[*key]
    }) {
        return Err(
            "Layout changed outside TermiMochi. Restore is blocked to protect those changes."
                .into(),
        );
    }
    Ok(())
}

pub(crate) struct ApplyRequest {
    settings: gio::Settings,
    receipt: Receipt,
}

impl ApplyRequest {
    pub fn discover_theme(
        layout: &LayoutSettings,
        fields: &crate::design_document::theme::Fields,
    ) -> Result<Self, String> {
        Self::discover(layout)?.with_theme_fields(fields)
    }
    fn with_theme_fields(
        mut self,
        fields: &crate::design_document::theme::Fields,
    ) -> Result<Self, String> {
        let request = &mut self;
        let desired = request.receipt.after.clone();
        request.receipt.after = request.receipt.before.user.clone();
        for (field, key) in [
            ("cursor_shape", "cursor-shape"),
            ("cursor_blink", "cursor-blink-mode"),
            ("scrollbar", "scrollbar-policy"),
            ("columns", "default-columns"),
            ("rows", "default-rows"),
        ] {
            if fields.contains_key(field) {
                request
                    .receipt
                    .after
                    .insert(key.into(), desired[key].clone());
            }
        }
        if fields.contains_key("columns") || fields.contains_key("rows") {
            request
                .receipt
                .after
                .insert("restore-window-size".into(), Some(Value::Boolean(false)));
        }
        request.receipt.version = 2;
        request.receipt.validate()?;
        Ok(self)
    }
    pub fn discover(layout: &LayoutSettings) -> Result<Self, String> {
        Self::prepare(
            find_settings("org.gnome.Ptyxis", None)
                .ok_or("Ptyxis settings are unavailable. You can still save the layout preset.")?,
            layout,
        )
    }
    fn prepare(settings: gio::Settings, layout: &LayoutSettings) -> Result<Self, String> {
        layout.validate()?;
        let receipt = Receipt {
            version: 1,
            before: snapshot(&settings)?,
            after: requested(layout),
        };
        Ok(Self { settings, receipt })
    }
    pub fn detail(&self) -> String {
        let changes: Vec<_> = KEYS
            .iter()
            .map(|key| {
                format!(
                    "{key}: {} → {}",
                    self.receipt.before.effective[*key]
                        .as_ref()
                        .unwrap()
                        .label(),
                    self.receipt.after[*key]
                        .as_ref()
                        .map(Value::label)
                        .unwrap_or_else(|| "Inherited (unchanged)".into())
                )
            })
            .collect();
        format!(
            "Scope: all Ptyxis windows and profiles.\n{}\n\nSpecified grid size applies to new windows and disables remembered sizing. Unspecified grid fields keep their current values; existing windows are not resized.\n\nPreview only (saved, not applied): exact content padding, tab bar and window spacing.\n\nA private backup is created before changing settings. Fonts, colors and shell files are untouched.",
            changes.join("\n")
        )
    }
    pub fn apply(self, directory: &Path) -> Result<Option<PathBuf>, String> {
        if snapshot(&self.settings)? != self.receipt.before {
            return Err(
                "Ptyxis layout changed while confirmation was open. Review it again.".into(),
            );
        }
        if self.receipt.before.user == self.receipt.after {
            return Ok(None);
        }
        self.receipt.validate()?;
        let bytes = serde_json::to_vec_pretty(&self.receipt).map_err(|error| error.to_string())?;
        let backup = retain_backup(&directory.join("layout-backups"), &bytes)?;
        write_private(&directory.join(RECEIPT), &bytes)?;
        if snapshot(&self.settings)? != self.receipt.before {
            return Err("Ptyxis layout changed before applying. Nothing was overwritten.".into());
        }
        if let Err(error) = write(&self.settings, &self.receipt.after) {
            let recovery = compatible(&self.settings, &self.receipt)
                .and_then(|_| write(&self.settings, &self.receipt.before.user));
            return Err(format!(
                "{error}\nRecovery: {}\nBackup: {}",
                recovery
                    .map(|_| "previous settings restored".into())
                    .unwrap_or_else(|error| error),
                backup.display()
            ));
        }
        Ok(Some(backup))
    }
}

pub(crate) struct RestoreRequest {
    settings: gio::Settings,
    receipt: Receipt,
    current: Snapshot,
}

impl RestoreRequest {
    pub fn load(directory: &Path) -> Result<Self, String> {
        let bytes =
            read_private(&directory.join(RECEIPT))?.ok_or("No layout backup exists yet.")?;
        let receipt: Receipt = serde_json::from_slice(&bytes).map_err(|error| error.to_string())?;
        let settings =
            find_settings("org.gnome.Ptyxis", None).ok_or("Ptyxis settings are unavailable.")?;
        Self::prepare(settings, receipt)
    }
    fn prepare(settings: gio::Settings, receipt: Receipt) -> Result<Self, String> {
        receipt.validate()?;
        compatible(&settings, &receipt)?;
        let current = snapshot(&settings)?;
        Ok(Self {
            settings,
            receipt,
            current,
        })
    }
    pub fn restore(self) -> Result<(), String> {
        if snapshot(&self.settings)? != self.current {
            return Err(
                "Ptyxis layout changed during confirmation. Nothing was overwritten.".into(),
            );
        }
        compatible(&self.settings, &self.receipt)?;
        write(&self.settings, &self.receipt.before.user)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sparse_theme_cursor_preserves_unset_grid_and_other_values() {
        let settings = settings();
        let root = tempfile::tempdir().unwrap();
        let before = snapshot(&settings).unwrap();
        let layout = LayoutSettings {
            cursor_shape: PreviewCursorShape::IBeam,
            ..Default::default()
        };
        let fields = [("cursor_shape".into(), serde_json::json!("i_beam"))]
            .into_iter()
            .collect();
        let request = ApplyRequest::prepare(settings.clone(), &layout)
            .unwrap()
            .with_theme_fields(&fields)
            .unwrap();
        assert_eq!(request.receipt.after["default-columns"], None);
        assert_eq!(request.receipt.after["restore-window-size"], None);
        request.apply(root.path()).unwrap();
        assert_eq!(settings.string("cursor-shape"), "ibeam");
        assert_eq!(settings.user_value("default-columns"), None);
        assert_eq!(
            snapshot(&settings).unwrap().user["scrollbar-policy"],
            before.user["scrollbar-policy"]
        );
    }
    fn settings() -> gio::Settings {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("layout.gschema.xml"), r#"<schemalist><schema id="org.gnome.Ptyxis" path="/org/gnome/Ptyxis/">
        <key name="cursor-shape" type="s"><default>'block'</default><choices><choice value="block"/><choice value="ibeam"/><choice value="underline"/></choices></key>
        <key name="cursor-blink-mode" type="s"><default>'system'</default><choices><choice value="system"/><choice value="on"/><choice value="off"/></choices></key>
        <key name="scrollbar-policy" type="s"><default>'system'</default><choices><choice value="system"/><choice value="always"/><choice value="never"/></choices></key>
        <key name="default-columns" type="u"><default>80</default><range min="1" max="65535"/></key>
        <key name="default-rows" type="u"><default>24</default><range min="1" max="65535"/></key>
        <key name="restore-window-size" type="b"><default>true</default></key>
        <key name="font-name" type="s"><default>'Monospace 11'</default></key>
        <key name="disable-padding" type="b"><default>false</default></key>
        </schema></schemalist>"#).unwrap();
        assert!(
            std::process::Command::new("glib-compile-schemas")
                .arg(root.path())
                .status()
                .unwrap()
                .success()
        );
        let source = gio::SettingsSchemaSource::from_directory(root.path(), None, false).unwrap();
        gio::Settings::new_full(
            &source.lookup("org.gnome.Ptyxis", false).unwrap(),
            Some(&gio::memory_settings_backend_new()),
            None,
        )
    }
    #[test]
    fn apply_restore_restart_and_noop_preserve_unset_and_unrelated_settings() {
        let settings = settings();
        let root = tempfile::tempdir().unwrap();
        let before = snapshot(&settings).unwrap();
        let layout = LayoutSettings {
            columns: 101,
            rows: 31,
            cursor_shape: PreviewCursorShape::IBeam,
            cursor_blink: PreviewCursorBlink::Off,
            scrollbar: true,
            content_padding: 24,
            ..Default::default()
        };
        let request = ApplyRequest::prepare(settings.clone(), &layout).unwrap();
        assert_eq!(snapshot(&settings).unwrap(), before);
        request.apply(root.path()).unwrap().unwrap();
        assert_eq!(settings.uint("default-columns"), 101);
        assert!(!settings.boolean("restore-window-size"));
        assert_eq!(settings.string("font-name"), "Monospace 11");
        assert!(!settings.boolean("disable-padding"));
        let bytes = read_private(&root.path().join(RECEIPT)).unwrap().unwrap();
        assert!(
            ApplyRequest::prepare(settings.clone(), &layout)
                .unwrap()
                .apply(root.path())
                .unwrap()
                .is_none()
        );
        assert_eq!(
            read_private(&root.path().join(RECEIPT)).unwrap().unwrap(),
            bytes
        );
        let receipt: Receipt = serde_json::from_slice(&bytes).unwrap();
        let reopened = gio::Settings::new_full(
            &settings.settings_schema().unwrap(),
            settings.backend().as_ref(),
            None,
        );
        RestoreRequest::prepare(reopened, receipt)
            .unwrap()
            .restore()
            .unwrap();
        assert_eq!(snapshot(&settings).unwrap(), before);
    }
    #[test]
    fn external_changes_failed_backups_and_bad_records_do_not_overwrite() {
        let settings = settings();
        let root = tempfile::tempdir().unwrap();
        let request = ApplyRequest::prepare(settings.clone(), &LayoutSettings::default()).unwrap();
        settings.set_string("cursor-shape", "underline").unwrap();
        assert!(request.apply(root.path()).is_err());
        assert!(!root.path().join(RECEIPT).exists());
        let before = snapshot(&settings).unwrap();
        let blocked = root.path().join("blocked");
        std::fs::write(&blocked, "keep").unwrap();
        assert!(
            ApplyRequest::prepare(settings.clone(), &LayoutSettings::default())
                .unwrap()
                .apply(&blocked)
                .is_err()
        );
        assert_eq!(snapshot(&settings).unwrap(), before);
        let request = ApplyRequest::prepare(settings.clone(), &LayoutSettings::default()).unwrap();
        let mut receipt = request.receipt.clone();
        request.apply(root.path()).unwrap();
        let restore = RestoreRequest::prepare(settings.clone(), receipt.clone()).unwrap();
        settings.set_uint("default-columns", 107).unwrap();
        settings.apply();
        assert!(restore.restore().is_err());
        assert_eq!(settings.uint("default-columns"), 107);
        receipt
            .after
            .insert("font-name".into(), Some(Value::Text("wrong".into())));
        assert!(receipt.validate().is_err());
    }
}
