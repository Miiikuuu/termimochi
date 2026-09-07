//! Versioned font presets. Only explicit saves write state; startup only reads.
use std::{
    fs::{self, File},
    io::{Read, Write},
    os::unix::fs::{MetadataExt, PermissionsExt},
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::typography::TypographySettings;

const LIMIT: u64 = 64 * 1024;
pub(crate) const PRESET_NAME: &str = "typography.termimochi-font.json";

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Preset {
    kind: String,
    version: u8,
    typography: TypographySettings,
}

pub(crate) fn encode(settings: &TypographySettings) -> Result<Vec<u8>, String> {
    settings.validate()?;
    serde_json::to_vec_pretty(&Preset {
        kind: "termimochi-typography".into(),
        version: 1,
        typography: settings.clone(),
    })
    .map_err(|error| error.to_string())
}

pub(crate) fn decode(bytes: &[u8]) -> Result<TypographySettings, String> {
    if bytes.len() as u64 > LIMIT {
        return Err("Typography preset exceeds 64 KiB.".into());
    }
    let preset: Preset = serde_json::from_slice(bytes).map_err(|error| error.to_string())?;
    if preset.kind != "termimochi-typography" || preset.version != 1 {
        return Err("This is not a supported TermiMochi typography preset.".into());
    }
    preset.typography.validate()?;
    Ok(preset.typography)
}

pub(crate) fn state_directory() -> PathBuf {
    std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .unwrap_or_else(|| gtk::glib::home_dir().join(".local/state"))
        .join("termimochi")
}

/// Do not follow symlinks, overwrite hard-link aliases, or read unbounded files.
pub(crate) fn read_private(path: &Path) -> Result<Option<Vec<u8>>, String> {
    read_private_with_limit(path, LIMIT)
}

pub(crate) fn read_private_with_limit(path: &Path, limit: u64) -> Result<Option<Vec<u8>>, String> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.to_string()),
    };
    if !metadata.is_file() || metadata.nlink() != 1 || metadata.len() > limit {
        return Err(format!(
            "Expected a regular, unaliased file of at most {} KiB.",
            limit / 1024
        ));
    }
    let file = File::open(path).map_err(|error| error.to_string())?;
    let opened = file.metadata().map_err(|error| error.to_string())?;
    if (metadata.dev(), metadata.ino()) != (opened.dev(), opened.ino()) {
        return Err("The file changed while it was being read. Retry.".into());
    }
    let mut bytes = Vec::new();
    file.take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    if bytes.len() as u64 > limit {
        return Err(format!("File exceeds {} KiB.", limit / 1024));
    }
    Ok(Some(bytes))
}

pub(crate) fn write_private(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let expected = read_private(path)?;
    write_checked(path, bytes, &expected)
}

fn write_checked(path: &Path, bytes: &[u8], expected: &Option<Vec<u8>>) -> Result<(), String> {
    write_checked_with_limit(path, bytes, expected, LIMIT)
}

pub(crate) fn write_checked_with_limit(
    path: &Path,
    bytes: &[u8],
    expected: &Option<Vec<u8>>,
    limit: u64,
) -> Result<(), String> {
    if bytes.len() as u64 > limit {
        return Err("The document exceeds its size limit.".into());
    }
    let parent = path.parent().ok_or("File has no parent directory.")?;
    fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    // Recheck file type even for app-owned state: a symlink must not authorize
    // replacing another configuration or silently break a user-created alias.
    if &read_private_with_limit(path, limit)? != expected {
        return Err("The destination changed before saving. Nothing was overwritten.".into());
    }
    if let Ok(metadata) = fs::metadata(path)
        && metadata.permissions().readonly()
    {
        return Err("The destination is read-only.".into());
    }
    let mut temporary = tempfile::Builder::new()
        .prefix(".termimochi-document-")
        .tempfile_in(parent)
        .map_err(|error| error.to_string())?;
    temporary
        .as_file()
        .set_permissions(fs::Permissions::from_mode(0o600))
        .map_err(|error| error.to_string())?;
    temporary
        .write_all(bytes)
        .and_then(|_| temporary.as_file().sync_all())
        .map_err(|error| error.to_string())?;
    if &read_private_with_limit(path, limit)? != expected {
        return Err("The destination changed before replacement. Nothing was overwritten.".into());
    }
    temporary.persist(path).map_err(|error| error.to_string())?;
    File::open(parent)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| error.to_string())
}

pub(crate) fn retain_backup(directory: &Path, bytes: &[u8]) -> Result<PathBuf, String> {
    fs::create_dir_all(directory).map_err(|error| error.to_string())?;
    let mut backup = tempfile::Builder::new()
        .prefix("termimochi-")
        .suffix(".json")
        .tempfile_in(directory)
        .map_err(|error| error.to_string())?;
    backup
        .write_all(bytes)
        .and_then(|_| backup.as_file().sync_all())
        .map_err(|error| error.to_string())?;
    let (_, path) = backup.keep().map_err(|error| error.to_string())?;
    File::open(directory)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| error.to_string())?;
    Ok(path)
}

#[derive(Debug)]
pub(crate) struct PresetStore {
    pub path: PathBuf,
    expected: Option<Vec<u8>>,
}

impl PresetStore {
    pub fn open(path: PathBuf) -> Result<Self, String> {
        let expected = read_private(&path)?;
        Ok(Self { path, expected })
    }

    pub fn settings(&self) -> Result<Option<TypographySettings>, String> {
        self.expected.as_deref().map(decode).transpose()
    }

    pub fn save(&mut self, settings: &TypographySettings) -> Result<(), String> {
        let bytes = encode(settings)?;
        if read_private(&self.path)? != self.expected {
            return Err(
                "The saved preset changed outside this window. Use Reload Saved Preset before saving.".into(),
            );
        }
        if self.expected.as_deref() == Some(bytes.as_slice()) {
            return Ok(());
        }
        if let Some(previous) = &self.expected {
            retain_backup(
                &self
                    .path
                    .parent()
                    .ok_or("Missing parent directory")?
                    .join("typography-backups"),
                previous,
            )?;
        }
        if read_private(&self.path)? != self.expected {
            return Err("The preset changed before saving. Nothing was overwritten.".into());
        }
        write_checked(&self.path, &bytes, &self.expected)?;
        self.expected = Some(bytes);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::typography::PreviewFontWeight;

    #[test]
    fn preset_roundtrip_and_restart_preserve_every_field() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join(PRESET_NAME);
        let settings = TypographySettings::new(
            "Iosevka Nerd Font Mono",
            13.5,
            PreviewFontWeight::Semibold,
            1.25,
            1.1,
        );
        let mut store = PresetStore::open(path.clone()).unwrap();
        assert_eq!(store.settings().unwrap(), None);
        store.save(&settings).unwrap();
        assert_eq!(
            PresetStore::open(path.clone()).unwrap().settings().unwrap(),
            Some(settings)
        );
        assert_eq!(
            fs::metadata(path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }

    #[test]
    fn invalid_versions_values_and_foreign_files_are_rejected() {
        let valid = String::from_utf8(encode(&TypographySettings::default()).unwrap()).unwrap();
        for invalid in [
            valid.replace("\"version\": 1", "\"version\": 2"),
            valid.replace("10.5", "0.5"),
            "{}".into(),
            valid.replace("Monospace", ""),
            valid.replace("\"regular\"", "\"heavy\""),
        ] {
            assert!(decode(invalid.as_bytes()).is_err());
        }
        assert!(decode(&vec![b' '; LIMIT as usize + 1]).is_err());
    }

    #[test]
    fn external_changes_readonly_and_symlinks_do_not_get_overwritten() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join(PRESET_NAME);
        let mut store = PresetStore::open(path.clone()).unwrap();
        store.save(&TypographySettings::default()).unwrap();
        fs::write(&path, "external").unwrap();
        assert!(store.save(&TypographySettings::default()).is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), "external");
        let link = directory.path().join("alias.json");
        std::os::unix::fs::symlink(&path, &link).unwrap();
        assert!(PresetStore::open(link).is_err());
        fs::set_permissions(&path, fs::Permissions::from_mode(0o400)).unwrap();
        assert!(write_private(&path, b"new").is_err());
    }
}
