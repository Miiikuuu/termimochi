//! Explicit Ptyxis typography transactions. Four allow-listed settings only;
//! palette, desktop settings, startup files and terminal commands are untouched.
use std::path::{Path, PathBuf};

use gtk::{gio, glib, prelude::*};
use serde::{Deserialize, Serialize};

use crate::{
    ptyxis::{current_profile_uuid_from, find_settings, valid_profile_uuid},
    typography::TypographySettings,
    typography_preset::{read_private, retain_backup, write_private},
};

const RECEIPT: &str = "last-typography-apply.json";
const GLOBAL_KEYS: [&str; 2] = ["use-system-font", "font-name"];
const PROFILE_KEYS: [&str; 2] = ["cell-height-scale", "cell-width-scale"];

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Values {
    use_system_font: Option<bool>,
    font_name: Option<String>,
    line_height: Option<f64>,
    cell_width: Option<f64>,
}

impl Values {
    fn for_settings(settings: &TypographySettings) -> Self {
        Self {
            use_system_font: Some(false),
            font_name: Some(settings.font_description().to_string().to_string()),
            line_height: Some(settings.line_height),
            cell_width: Some(settings.cell_width),
        }
    }

    fn validate(&self) -> Result<(), String> {
        if self
            .font_name
            .as_ref()
            .is_some_and(|font| font.len() > 512 || font.chars().any(char::is_control))
        {
            return Err("Invalid font in the typography backup.".into());
        }
        if [self.line_height, self.cell_width]
            .into_iter()
            .flatten()
            .any(|value| !value.is_finite() || !(1.0..=2.0).contains(&value))
        {
            return Err("Invalid cell spacing in the typography backup.".into());
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Snapshot {
    user: Values,
    effective: Values,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Receipt {
    version: u8,
    profile_uuid: String,
    profile_label: String,
    before: Snapshot,
    after: Values,
}

impl Receipt {
    fn validate(&self) -> Result<(), String> {
        if ![1, 2].contains(&self.version) || !valid_profile_uuid(&self.profile_uuid) {
            return Err("Unsupported typography backup or invalid profile identifier.".into());
        }
        self.before.user.validate()?;
        self.before.effective.validate()?;
        self.after.validate()?;
        if self.version == 1
            && (self.after.use_system_font != Some(false)
                || self.after.font_name.is_none()
                || self.after.line_height.is_none()
                || self.after.cell_width.is_none())
        {
            return Err("Incomplete typography apply record.".into());
        }
        Ok(())
    }
}

#[derive(Clone)]
pub(crate) struct TypographyTarget {
    global: gio::Settings,
    profile: gio::Settings,
    pub uuid: String,
    pub label: String,
}

impl TypographyTarget {
    pub fn discover() -> Result<Self, String> {
        let global = find_settings("org.gnome.Ptyxis", None)
            .ok_or("Ptyxis settings are unavailable. You can still save and export a preset.")?;
        let inherited = std::env::var("PTYXIS_PROFILE").ok();
        let uuid = current_profile_uuid_from(&global, inherited.as_deref())
            .ok_or("Ptyxis has no configured profile.")?;
        Self::for_uuid(&uuid)
    }

    pub(crate) fn for_uuid(uuid: &str) -> Result<Self, String> {
        if !valid_profile_uuid(uuid) {
            return Err("Invalid Ptyxis profile identifier.".into());
        }
        let global =
            find_settings("org.gnome.Ptyxis", None).ok_or("Ptyxis settings are unavailable.")?;
        let path = format!("/org/gnome/Ptyxis/Profiles/{uuid}/");
        let profile = find_settings("org.gnome.Ptyxis.Profile", Some(&path))
            .ok_or("Ptyxis profile settings are unavailable.")?;
        let label = profile
            .settings_schema()
            .filter(|schema| schema.has_key("label"))
            .map(|_| profile.string("label").to_string())
            .unwrap_or_else(|| uuid.to_owned());
        let target = Self {
            global,
            profile,
            uuid: uuid.into(),
            label,
        };
        target.check()?;
        Ok(target)
    }

    fn check(&self) -> Result<(), String> {
        let profiles = self
            .global
            .settings_schema()
            .filter(|schema| schema.has_key("profile-uuids"))
            .and_then(|_| self.global.value("profile-uuids").get::<Vec<String>>())
            .ok_or("Ptyxis does not provide a supported profile list.")?;
        if !profiles.contains(&self.uuid) {
            return Err("The selected Ptyxis profile was removed. Nothing was changed.".into());
        }
        for (settings, keys) in [(&self.global, GLOBAL_KEYS), (&self.profile, PROFILE_KEYS)] {
            let schema = settings
                .settings_schema()
                .ok_or("Missing settings schema.")?;
            for key in keys {
                if !schema.has_key(key) || !settings.is_writable(key) {
                    return Err(format!(
                        "Ptyxis setting {key} is unavailable or locked. Nothing was changed."
                    ));
                }
            }
        }
        Ok(())
    }

    fn values(&self, user: bool) -> Result<Values, String> {
        fn get<T: glib::variant::FromVariant>(
            settings: &gio::Settings,
            key: &str,
            user: bool,
        ) -> Result<Option<T>, String> {
            let value = if user {
                settings.user_value(key)
            } else {
                Some(settings.value(key))
            };
            value
                .map(|value| {
                    value
                        .get()
                        .ok_or_else(|| format!("Unexpected type for Ptyxis setting {key}."))
                })
                .transpose()
        }
        Ok(Values {
            use_system_font: get(&self.global, GLOBAL_KEYS[0], user)?,
            font_name: get(&self.global, GLOBAL_KEYS[1], user)?,
            line_height: get(&self.profile, PROFILE_KEYS[0], user)?,
            cell_width: get(&self.profile, PROFILE_KEYS[1], user)?,
        })
    }

    fn snapshot(&self) -> Result<Snapshot, String> {
        self.check()?;
        Ok(Snapshot {
            user: self.values(true)?,
            effective: self.values(false)?,
        })
    }

    fn write(&self, values: &Values) -> Result<(), String> {
        self.check()?;
        values.validate()?;
        let global = [
            values.use_system_font.map(|value| value.to_variant()),
            values.font_name.as_ref().map(|value| value.to_variant()),
        ];
        let profile = [
            values.line_height.map(|value| value.to_variant()),
            values.cell_width.map(|value| value.to_variant()),
        ];
        // Validate every value before queuing either settings group.
        for (settings, keys, values) in [
            (&self.global, GLOBAL_KEYS, &global),
            (&self.profile, PROFILE_KEYS, &profile),
        ] {
            let schema = settings.settings_schema().unwrap();
            for (key, value) in keys.into_iter().zip(values) {
                if let Some(value) = value
                    && !schema.key(key).range_check(value)
                {
                    return Err(format!(
                        "Ptyxis does not support the requested value for {key}."
                    ));
                }
            }
        }
        self.global.delay();
        self.profile.delay();
        for (settings, keys, values) in [
            (&self.global, GLOBAL_KEYS, global),
            (&self.profile, PROFILE_KEYS, profile),
        ] {
            for (key, value) in keys.into_iter().zip(values) {
                if settings.user_value(key) == value {
                    continue;
                }
                let result = match value {
                    Some(value) => settings
                        .set_value(key, &value)
                        .map_err(|error| error.to_string()),
                    None => {
                        settings.reset(key);
                        Ok(())
                    }
                };
                if let Err(error) = result {
                    self.global.revert();
                    self.profile.revert();
                    return Err(error);
                }
            }
        }
        // GSettings batches each group; the backup precedes both groups and
        // verification/recovery handles a failure between them.
        self.global.apply();
        self.profile.apply();
        gio::Settings::sync();
        if self.values(true)? != *values {
            return Err("Ptyxis did not retain all requested settings.".into());
        }
        Ok(())
    }

    pub fn prepare(&self, settings: &TypographySettings) -> Result<ApplyRequest, String> {
        settings.validate()?;
        Ok(ApplyRequest {
            target: self.clone(),
            receipt: Receipt {
                version: 1,
                profile_uuid: self.uuid.clone(),
                profile_label: self.label.clone(),
                before: self.snapshot()?,
                after: Values::for_settings(settings),
            },
        })
    }

    /// Sparse theme edit: composite font fields come from the actual target,
    /// never from the disposable preview. Unowned user values stay unset.
    pub fn prepare_theme(
        &self,
        settings: &TypographySettings,
        fields: &crate::design_document::theme::Fields,
    ) -> Result<ApplyRequest, String> {
        let mut request = self.prepare(settings)?;
        let before = &request.receipt.before;
        let mut after = before.user.clone();
        if ["family", "size", "weight"]
            .iter()
            .any(|k| fields.contains_key(*k))
        {
            let font = if before.effective.use_system_font == Some(true) {
                find_settings("org.gnome.desktop.interface", None)
                    .map(|s| s.string("monospace-font-name").to_string())
                    .ok_or("Cannot resolve inherited system font; no font was applied.")?
            } else {
                before
                    .effective
                    .font_name
                    .clone()
                    .ok_or("Cannot read target font")?
            };
            let mut description = gtk::pango::FontDescription::from_string(&font);
            let requested = settings.font_description();
            if fields.contains_key("family") {
                description.set_family(&settings.family);
            }
            if fields.contains_key("size") {
                description.set_size(requested.size());
            }
            if fields.contains_key("weight") {
                description.set_weight(requested.weight());
            }
            after.font_name = Some(description.to_string());
            after.use_system_font = Some(false);
        }
        if fields.contains_key("line_height") {
            after.line_height = Some(settings.line_height);
        }
        if fields.contains_key("cell_width") {
            after.cell_width = Some(settings.cell_width);
        }
        request.receipt.version = 2;
        request.receipt.after = after;
        request.receipt.validate()?;
        Ok(request)
    }
}

pub(crate) struct ApplyRequest {
    target: TypographyTarget,
    receipt: Receipt,
}

impl ApplyRequest {
    pub fn detail(&self) -> String {
        let before = &self.receipt.before.effective;
        let before_font = if before.use_system_font == Some(true) {
            "System monospace font"
        } else {
            before.font_name.as_deref().unwrap_or("Unknown")
        };
        format!(
            "Ptyxis-wide (all windows and profiles):\nFont: {before_font} → {}\n\nProfile: {} ({})\nLine height: {:.2} → {:.2}\nCell width: {:.2} → {:.2}\n\nA private backup is saved first. Palette, desktop font and shell files are not changed.",
            self.receipt
                .after
                .font_name
                .as_deref()
                .unwrap_or("Inherited (unchanged)"),
            self.target.label,
            self.target.uuid,
            before.line_height.unwrap_or(1.0),
            self.receipt
                .after
                .line_height
                .or(before.line_height)
                .unwrap_or(1.0),
            before.cell_width.unwrap_or(1.0),
            self.receipt
                .after
                .cell_width
                .or(before.cell_width)
                .unwrap_or(1.0)
        )
    }

    pub fn apply(self, directory: &Path) -> Result<Option<PathBuf>, String> {
        if self.target.snapshot()? != self.receipt.before {
            return Err("Ptyxis settings changed while confirmation was open. Review the changes again; nothing was overwritten.".into());
        }
        self.receipt.validate()?;
        if self.receipt.before.user == self.receipt.after {
            return Ok(None); // Preserve the useful rollback record on repeated Apply.
        }
        let bytes = serde_json::to_vec_pretty(&self.receipt).map_err(|error| error.to_string())?;
        let backup = retain_backup(&directory.join("typography-backups"), &bytes)?;
        write_private(&directory.join(RECEIPT), &bytes)?;
        if self.target.snapshot()? != self.receipt.before {
            return Err("Ptyxis settings changed before applying. Nothing was overwritten; backup retained.".into());
        }
        if let Err(error) = self.target.write(&self.receipt.after) {
            let recovery = restore_compatible(&self.target, &self.receipt)
                .and_then(|_| self.target.write(&self.receipt.before.user));
            return Err(format!(
                "{error}\nRecovery: {}\nBackup: {}",
                recovery
                    .map(|_| "previous settings restored".to_owned())
                    .unwrap_or_else(|error| error),
                backup.display()
            ));
        }
        Ok(Some(backup))
    }
}

fn restore_compatible(target: &TypographyTarget, receipt: &Receipt) -> Result<(), String> {
    let current = target.values(true)?;
    let before = &receipt.before.user;
    let after = &receipt.after;
    if !(current.use_system_font == before.use_system_font
        || current.use_system_font == after.use_system_font)
        || !(current.font_name == before.font_name || current.font_name == after.font_name)
        || !(current.line_height == before.line_height || current.line_height == after.line_height)
        || !(current.cell_width == before.cell_width || current.cell_width == after.cell_width)
    {
        return Err("Typography changed outside TermiMochi after applying. Restore is blocked to protect those changes.".into());
    }
    Ok(())
}

pub(crate) struct RestoreRequest {
    target: TypographyTarget,
    receipt: Receipt,
    current: Snapshot,
}

impl RestoreRequest {
    pub fn load(directory: &Path) -> Result<Self, String> {
        let bytes =
            read_private(&directory.join(RECEIPT))?.ok_or("No typography backup exists yet.")?;
        let receipt: Receipt = serde_json::from_slice(&bytes).map_err(|error| error.to_string())?;
        receipt.validate()?;
        let target = TypographyTarget::for_uuid(&receipt.profile_uuid)?;
        Self::prepare(target, receipt)
    }

    fn prepare(target: TypographyTarget, receipt: Receipt) -> Result<Self, String> {
        restore_compatible(&target, &receipt)?;
        let current = target.snapshot()?;
        Ok(Self {
            target,
            receipt,
            current,
        })
    }

    pub fn detail(&self) -> String {
        format!(
            "Restore the previous Ptyxis-wide font and spacing for profile {} ({})?\n\nUnset values are restored as unset, preserving system-font inheritance. Your saved TermiMochi preset is kept. External changes block restoration.",
            self.receipt.profile_label, self.receipt.profile_uuid
        )
    }

    pub fn restore(self) -> Result<(), String> {
        if self.target.snapshot()? != self.current {
            return Err(
                "Ptyxis settings changed while confirmation was open. Nothing was overwritten."
                    .into(),
            );
        }
        restore_compatible(&self.target, &self.receipt)?;
        self.target.write(&self.receipt.before.user)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::typography::PreviewFontWeight;

    #[test]
    fn sparse_theme_size_keeps_target_family_weight_and_unset_spacing() {
        let target = target();
        let root = tempfile::tempdir().unwrap();
        target.global.set_boolean("use-system-font", false).unwrap();
        target
            .global
            .set_string("font-name", "Target Mono Bold 12")
            .unwrap();
        let before = target.snapshot().unwrap();
        let patch = [("size".into(), serde_json::json!(18.0))]
            .into_iter()
            .collect();
        let mut preview = desired();
        preview.size = 18.0;
        let request = target.prepare_theme(&preview, &patch).unwrap();
        assert_eq!(
            request.receipt.after.font_name.as_deref(),
            Some("Target Mono Bold 18")
        );
        assert_eq!(request.receipt.after.line_height, None);
        assert_eq!(request.receipt.after.cell_width, None);
        let receipt = request.receipt.clone();
        request.apply(root.path()).unwrap();
        assert_eq!(target.profile.user_value("cell-height-scale"), None);
        RestoreRequest::prepare(target.clone(), receipt)
            .unwrap()
            .restore()
            .unwrap();
        assert_eq!(target.snapshot().unwrap(), before);
    }

    fn target() -> TypographyTarget {
        let backend = gio::memory_settings_backend_new();
        // CI need not install Ptyxis. Use an isolated schema and memory
        // backend; these tests never connect to the user's dconf database.
        let schemas = tempfile::tempdir().unwrap();
        std::fs::write(schemas.path().join("font-test.gschema.xml"), r#"<schemalist>
          <schema id="org.gnome.Ptyxis" path="/org/gnome/Ptyxis/">
            <key name="profile-uuids" type="as"><default>[]</default></key>
            <key name="use-system-font" type="b"><default>true</default></key>
            <key name="font-name" type="s"><default>'Monospace 10'</default></key>
          </schema>
          <schema id="org.gnome.Ptyxis.Profile">
            <key name="cell-height-scale" type="d"><default>1.0</default><range min="1.0" max="2.0"/></key>
            <key name="cell-width-scale" type="d"><default>1.0</default><range min="1.0" max="2.0"/></key>
            <key name="palette" type="s"><default>'test-palette'</default></key>
          </schema>
        </schemalist>"#).unwrap();
        assert!(
            std::process::Command::new("glib-compile-schemas")
                .arg(schemas.path())
                .status()
                .unwrap()
                .success()
        );
        let source =
            gio::SettingsSchemaSource::from_directory(schemas.path(), None, false).unwrap();
        let global = gio::Settings::new_full(
            &source.lookup("org.gnome.Ptyxis", true).unwrap(),
            Some(&backend),
            None,
        );
        global.set_strv("profile-uuids", ["font-test"]).unwrap();
        let profile = gio::Settings::new_full(
            &source.lookup("org.gnome.Ptyxis.Profile", true).unwrap(),
            Some(&backend),
            Some("/org/gnome/Ptyxis/Profiles/font-test/"),
        );
        TypographyTarget {
            global,
            profile,
            uuid: "font-test".into(),
            label: "Test profile".into(),
        }
    }

    fn desired() -> TypographySettings {
        TypographySettings::new(
            "Iosevka Nerd Font Mono",
            14.5,
            PreviewFontWeight::Semibold,
            1.25,
            1.1,
        )
    }

    #[test]
    fn apply_and_restore_preserve_unset_values_and_other_settings() {
        let target = target();
        let directory = tempfile::tempdir().unwrap();
        let before = target.snapshot().unwrap();
        let palette = target.profile.value("palette");
        let request = target.prepare(&desired()).unwrap();
        let receipt = request.receipt.clone();
        assert_eq!(
            target.snapshot().unwrap(),
            before,
            "preparing confirmation is read-only"
        );
        let backup = request.apply(directory.path()).unwrap().unwrap();
        assert!(backup.is_file());
        assert_eq!(
            target.values(true).unwrap(),
            Values::for_settings(&desired())
        );
        assert_eq!(target.profile.value("palette"), palette);
        RestoreRequest::prepare(target.clone(), receipt)
            .unwrap()
            .restore()
            .unwrap();
        assert_eq!(target.snapshot().unwrap(), before);
    }

    #[test]
    fn external_changes_before_apply_or_restore_are_protected() {
        let target = target();
        let directory = tempfile::tempdir().unwrap();
        let request = target.prepare(&desired()).unwrap();
        target
            .global
            .set_string("font-name", "External Mono 12")
            .unwrap();
        assert!(request.apply(directory.path()).is_err());
        assert!(!directory.path().join(RECEIPT).exists());
        let request = target.prepare(&desired()).unwrap();
        let receipt = request.receipt.clone();
        request.apply(directory.path()).unwrap();
        target
            .global
            .set_string("font-name", "External Mono 16")
            .unwrap();
        target.global.apply();
        assert!(RestoreRequest::prepare(target.clone(), receipt).is_err());
        assert_eq!(target.global.string("font-name"), "External Mono 16");
    }

    #[test]
    fn failed_backup_and_removed_profile_make_no_settings_writes() {
        let target = target();
        let directory = tempfile::tempdir().unwrap();
        let before = target.snapshot().unwrap();
        let blocked = directory.path().join("not-a-directory");
        std::fs::write(&blocked, "keep").unwrap();
        assert!(target.prepare(&desired()).unwrap().apply(&blocked).is_err());
        assert_eq!(target.snapshot().unwrap(), before);
        let request = target.prepare(&desired()).unwrap();
        target
            .global
            .set_strv("profile-uuids", [] as [&str; 0])
            .unwrap();
        assert!(request.apply(directory.path()).is_err());
        assert_eq!(target.values(true).unwrap(), before.user);
    }

    #[test]
    fn repeated_apply_keeps_backup_and_persisted_receipt_restores_after_restart() {
        let target = target();
        let directory = tempfile::tempdir().unwrap();
        target.global.set_boolean("use-system-font", true).unwrap();
        target
            .global
            .set_string("font-name", "My Previous Font Bold 12")
            .unwrap();
        target.profile.set_double("cell-height-scale", 1.5).unwrap();
        let before = target.snapshot().unwrap();
        target
            .prepare(&desired())
            .unwrap()
            .apply(directory.path())
            .unwrap();
        let persisted = read_private(&directory.path().join(RECEIPT))
            .unwrap()
            .unwrap();
        assert_eq!(
            target
                .prepare(&desired())
                .unwrap()
                .apply(directory.path())
                .unwrap(),
            None
        );
        assert_eq!(
            read_private(&directory.path().join(RECEIPT))
                .unwrap()
                .unwrap(),
            persisted
        );
        // Recreate both settings objects, just as a later app process does.
        let reconnected = TypographyTarget {
            global: gio::Settings::new_full(
                &target.global.settings_schema().unwrap(),
                target.global.backend().as_ref(),
                None,
            ),
            profile: gio::Settings::new_full(
                &target.profile.settings_schema().unwrap(),
                target.profile.backend().as_ref(),
                Some("/org/gnome/Ptyxis/Profiles/font-test/"),
            ),
            uuid: target.uuid.clone(),
            label: target.label.clone(),
        };
        let receipt: Receipt = serde_json::from_slice(&persisted).unwrap();
        receipt.validate().unwrap();
        RestoreRequest::prepare(reconnected, receipt)
            .unwrap()
            .restore()
            .unwrap();
        assert_eq!(target.snapshot().unwrap(), before);
    }

    #[test]
    fn restore_rechecks_confirmation_and_rejects_bad_receipts() {
        let target = target();
        let directory = tempfile::tempdir().unwrap();
        let request = target.prepare(&desired()).unwrap();
        let receipt = request.receipt.clone();
        request.apply(directory.path()).unwrap();
        let restore = RestoreRequest::prepare(target.clone(), receipt.clone()).unwrap();
        target.profile.set_double("cell-width-scale", 1.6).unwrap();
        target.profile.apply();
        assert!(restore.restore().is_err());
        assert_eq!(target.profile.double("cell-width-scale"), 1.6);
        let mut invalid = receipt.clone();
        invalid.version = 3;
        assert!(invalid.validate().is_err());
        invalid = receipt.clone();
        invalid.profile_uuid = "../../other".into();
        assert!(invalid.validate().is_err());
        invalid = receipt;
        invalid.after.cell_width = Some(0.1);
        assert!(invalid.validate().is_err());
    }
}
