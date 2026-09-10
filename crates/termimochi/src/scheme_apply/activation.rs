//! The only new external setting transaction: palette selection and variant.
//! Uses the existing private backup writer; preserves unset GSettings values.
use gtk::{gio, prelude::*};
use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::{ptyxis, typography_preset};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Values {
    palette: Option<String>,
    style: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Receipt {
    version: u8,
    uuid: String,
    before: Values,
    effective: Values,
    after: Values,
}

pub(crate) struct Activation {
    global: gio::Settings,
    profile: gio::Settings,
    receipt: Receipt,
}

pub(crate) fn profile() -> Result<(String, String), String> {
    let global = ptyxis::find_settings("org.gnome.Ptyxis", None)
        .ok_or("Ptyxis settings are unavailable; Kitty and other terminals are export-only.")?;
    let uuid =
        ptyxis::current_profile_uuid_from(&global, std::env::var("PTYXIS_PROFILE").ok().as_deref())
            .ok_or("Ptyxis has no configured profile.")?;
    // Discovery is read-only and must not make unrelated font/layout actions
    // depend on whether palette or appearance settings are writable.
    let profile = ptyxis::find_settings(
        "org.gnome.Ptyxis.Profile",
        Some(&format!("/org/gnome/Ptyxis/Profiles/{uuid}/")),
    )
    .ok_or("Ptyxis profile settings are unavailable.")?;
    let label = profile
        .settings_schema()
        .filter(|s| s.has_key("label"))
        .map(|_| profile.string("label").to_string())
        .unwrap_or_default();
    Ok((uuid, label))
}

fn settings(uuid: &str) -> Result<(gio::Settings, gio::Settings), String> {
    if !ptyxis::valid_profile_uuid(uuid) {
        return Err("Invalid Ptyxis profile UUID.".into());
    }
    let global = ptyxis::find_settings("org.gnome.Ptyxis", None)
        .ok_or("Ptyxis settings are unavailable.")?;
    let profile = ptyxis::find_settings(
        "org.gnome.Ptyxis.Profile",
        Some(&format!("/org/gnome/Ptyxis/Profiles/{uuid}/")),
    )
    .ok_or("Ptyxis profile settings are unavailable.")?;
    check(&global, &profile, uuid)?;
    Ok((global, profile))
}

fn check(global: &gio::Settings, profile: &gio::Settings, uuid: &str) -> Result<(), String> {
    if !global
        .settings_schema()
        .is_some_and(|s| s.has_key("profile-uuids"))
        || !global.strv("profile-uuids").iter().any(|id| id == uuid)
    {
        return Err("The reviewed Ptyxis profile no longer exists.".into());
    }
    for (settings, key) in [(global, "interface-style"), (profile, "palette")] {
        if !settings.settings_schema().is_some_and(|s| s.has_key(key)) || !settings.is_writable(key)
        {
            return Err(format!("Ptyxis {key} is unavailable or locked."));
        }
    }
    Ok(())
}

fn values(global: &gio::Settings, profile: &gio::Settings, user: bool) -> Values {
    let get = |settings: &gio::Settings, key| {
        if user {
            settings.user_value(key).and_then(|v| v.get::<String>())
        } else {
            Some(settings.string(key).to_string())
        }
    };
    Values {
        palette: get(profile, "palette"),
        style: get(global, "interface-style"),
    }
}

fn write(global: &gio::Settings, profile: &gio::Settings, values: &Values) -> Result<(), String> {
    for (settings, key, value) in [
        (profile, "palette", &values.palette),
        (global, "interface-style", &values.style),
    ] {
        if let Some(value) = value {
            settings.set_string(key, value).map_err(|e| e.to_string())?;
        } else {
            settings.reset(key);
        }
    }
    gio::Settings::sync();
    if &self::values(global, profile, true) != values {
        return Err("Ptyxis did not retain the palette selection.".into());
    }
    Ok(())
}

impl Activation {
    pub fn prepare(uuid: &str, palette_id: &str, light: bool) -> Result<Self, String> {
        let (global, profile) = settings(uuid)?;
        let after = Values {
            palette: Some(palette_id.into()),
            style: Some(if light { "light" } else { "dark" }.into()),
        };
        for (settings, key, value) in [
            (&global, "interface-style", &after.style),
            (&profile, "palette", &after.palette),
        ] {
            if !settings
                .settings_schema()
                .unwrap()
                .key(key)
                .range_check(&value.as_ref().unwrap().to_variant())
            {
                return Err(format!("Unsupported Ptyxis value for {key}."));
            }
        }
        let receipt = Receipt {
            version: 1,
            uuid: uuid.into(),
            before: values(&global, &profile, true),
            effective: values(&global, &profile, false),
            after,
        };
        Ok(Self {
            global,
            profile,
            receipt,
        })
    }

    pub fn detail(&self) -> String {
        format!(
            "Profile {}: palette {} → {}\nPtyxis-wide appearance (all profiles): {} → {}. Does not change the default profile.",
            self.receipt.uuid,
            self.receipt.effective.palette.as_deref().unwrap_or("unset"),
            self.receipt.after.palette.as_deref().unwrap(),
            self.receipt.effective.style.as_deref().unwrap_or("unset"),
            self.receipt.after.style.as_deref().unwrap()
        )
    }

    pub fn apply(self, directory: &Path) -> Result<bool, String> {
        check(&self.global, &self.profile, &self.receipt.uuid)?;
        if values(&self.global, &self.profile, true) != self.receipt.before
            || values(&self.global, &self.profile, false) != self.receipt.effective
        {
            return Err("Palette selection changed during review. Nothing was overwritten.".into());
        }
        if self.receipt.before == self.receipt.after {
            return Ok(false);
        }
        let bytes = serde_json::to_vec_pretty(&self.receipt).map_err(|e| e.to_string())?;
        typography_preset::retain_backup(&directory.join("activation-backups"), &bytes)?;
        typography_preset::write_private(&directory.join("activation.json"), &bytes)?;
        if values(&self.global, &self.profile, true) != self.receipt.before {
            return Err("Palette selection changed before applying.".into());
        }
        if let Err(error) = write(&self.global, &self.profile, &self.receipt.after) {
            let recovery = restore(directory);
            return Err(format!(
                "{error}; recovery: {}",
                recovery.map(|_| "restored".into()).unwrap_or_else(|e| e)
            ));
        }
        Ok(true)
    }
}

pub(crate) fn restore(directory: &Path) -> Result<(), String> {
    let bytes = typography_preset::read_private(&directory.join("activation.json"))?
        .ok_or("No palette activation receipt exists for this application.")?;
    let receipt: Receipt = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    if receipt.version != 1 {
        return Err("Unsupported activation receipt.".into());
    }
    let (global, profile) = settings(&receipt.uuid)?;
    let current = values(&global, &profile, true);
    if (current.palette != receipt.after.palette && current.palette != receipt.before.palette)
        || (current.style != receipt.after.style && current.style != receipt.before.style)
    {
        return Err("Palette selection changed outside TermiMochi. Restore is blocked.".into());
    }
    for (settings, key, value) in [
        (&global, "interface-style", &receipt.before.style),
        (&profile, "palette", &receipt.before.palette),
    ] {
        if value.as_ref().is_some_and(|v| {
            !settings
                .settings_schema()
                .unwrap()
                .key(key)
                .range_check(&v.to_variant())
        }) {
            return Err("Invalid activation backup.".into());
        }
    }
    write(&global, &profile, &receipt.before)
}

/// Do not remove a newly installed palette if another profile has since selected it.
pub(crate) fn palette_in_use(id: &str) -> bool {
    let Some(global) = ptyxis::find_settings("org.gnome.Ptyxis", None) else {
        return false;
    };
    if !global
        .settings_schema()
        .is_some_and(|s| s.has_key("profile-uuids"))
    {
        return true;
    }
    global.strv("profile-uuids").iter().any(|uuid| {
        if !ptyxis::valid_profile_uuid(uuid) {
            return true;
        }
        ptyxis::find_settings(
            "org.gnome.Ptyxis.Profile",
            Some(&format!("/org/gnome/Ptyxis/Profiles/{uuid}/")),
        )
        .is_none_or(|p| {
            !p.settings_schema().is_some_and(|s| s.has_key("palette")) || p.string("palette") == id
        })
    })
}
