//! Checked, versioned local documents. Opening never writes or applies settings.
use crate::typography_preset::{read_private_with_limit, retain_backup, write_checked_with_limit};
use serde::{Serialize, de::DeserializeOwned};
use std::{
    marker::PhantomData,
    path::{Path, PathBuf},
};

const LIMIT: u64 = 1024 * 1024;

pub(crate) trait Document: Serialize + DeserializeOwned + Clone {
    const SUFFIX: &'static str;
    fn validate(&self) -> Result<(), String>;
}

pub(crate) fn encode<T: Document>(document: &T) -> Result<Vec<u8>, String> {
    document.validate()?;
    let bytes = serde_json::to_vec_pretty(document).map_err(|error| error.to_string())?;
    if bytes.len() as u64 > LIMIT {
        return Err("Document exceeds 1 MiB.".into());
    }
    Ok(bytes)
}

pub(crate) fn decode<T: Document>(bytes: &[u8]) -> Result<T, String> {
    if bytes.len() as u64 > LIMIT {
        return Err("Document exceeds 1 MiB.".into());
    }
    let document: T = serde_json::from_slice(bytes).map_err(|error| error.to_string())?;
    document.validate()?;
    Ok(document)
}

pub(crate) struct DocumentStore<T: Document> {
    pub path: PathBuf,
    expected: Option<Vec<u8>>,
    marker: PhantomData<T>,
}

impl<T: Document> DocumentStore<T> {
    pub fn open(path: PathBuf) -> Result<Self, String> {
        let expected = read_private_with_limit(&path, LIMIT)?;
        if let Some(bytes) = &expected {
            decode::<T>(bytes)?;
        }
        Ok(Self {
            path,
            expected,
            marker: PhantomData,
        })
    }

    pub fn document(&self) -> Result<Option<T>, String> {
        self.expected.as_deref().map(decode).transpose()
    }

    pub fn save(&mut self, document: &T) -> Result<(), String> {
        if !matches_path::<T>(&self.path) {
            return Err(format!(
                "Use a filename ending in {}. Other configurations are never overwritten.",
                T::SUFFIX
            ));
        }
        let bytes = encode(document)?;
        if read_private_with_limit(&self.path, LIMIT)? != self.expected {
            return Err(
                "The document changed outside this window. Reopen it or save a separate copy."
                    .into(),
            );
        }
        if self.expected.as_deref() == Some(bytes.as_slice()) {
            return Ok(());
        }
        if let Some(before) = &self.expected {
            retain_backup(
                &self
                    .path
                    .parent()
                    .ok_or("Missing document directory")?
                    .join("termimochi-backups"),
                before,
            )?;
        }
        write_checked_with_limit(&self.path, &bytes, &self.expected, LIMIT)?;
        self.expected = Some(bytes);
        Ok(())
    }
}

pub(crate) fn matches_path<T: Document>(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.ends_with(T::SUFFIX))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::{LayoutPreset, LayoutSettings};
    #[test]
    fn layout_restart_conflict_and_alias_protection() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("saved.termimochi-layout.json");
        let mut store = DocumentStore::<LayoutPreset>::open(path.clone()).unwrap();
        let mut preset = LayoutPreset::new(LayoutSettings::default());
        preset.layout.columns = 101;
        store.save(&preset).unwrap();
        assert_eq!(
            DocumentStore::<LayoutPreset>::open(path.clone())
                .unwrap()
                .document()
                .unwrap(),
            Some(preset.clone())
        );
        std::fs::write(&path, "external").unwrap();
        assert!(store.save(&preset).is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "external");
        let link = root.path().join("alias.termimochi-layout.json");
        std::os::unix::fs::symlink(&path, &link).unwrap();
        assert!(DocumentStore::<LayoutPreset>::open(link).is_err());
    }
    #[test]
    fn invalid_layout_values_and_foreign_destinations_are_rejected() {
        let mut preset = LayoutPreset::new(LayoutSettings::default());
        preset.layout.rows = 0;
        assert!(encode(&preset).is_err());
        assert!(decode::<LayoutPreset>(b"{}").is_err());
        let root = tempfile::tempdir().unwrap();
        let mut store = DocumentStore::<LayoutPreset>::open(root.path().join(".bashrc")).unwrap();
        assert!(
            store
                .save(&LayoutPreset::new(LayoutSettings::default()))
                .is_err()
        );
        assert!(!root.path().join(".bashrc").exists());
    }

    #[test]
    fn bounded_documents_reject_links_directories_and_late_creation() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("layout.termimochi-layout.json");
        let preset = LayoutPreset::new(LayoutSettings::default());
        let mut pending = DocumentStore::<LayoutPreset>::open(path.clone()).unwrap();
        let bytes = encode(&preset).unwrap();
        std::fs::write(&path, &bytes).unwrap();
        assert!(
            pending.save(&preset).is_err(),
            "new files cannot overwrite late external creation"
        );
        let alias = root.path().join("hardlink.termimochi-layout.json");
        std::fs::hard_link(&path, &alias).unwrap();
        assert!(DocumentStore::<LayoutPreset>::open(alias).is_err());
        assert!(DocumentStore::<LayoutPreset>::open(root.path().into()).is_err());
        let oversized = root.path().join("oversized.termimochi-layout.json");
        std::fs::File::create(&oversized)
            .unwrap()
            .set_len(LIMIT + 1)
            .unwrap();
        assert!(DocumentStore::<LayoutPreset>::open(oversized).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
    }

    #[test]
    fn overwrite_keeps_private_backup_and_failed_save_keeps_original() {
        use std::os::unix::fs::PermissionsExt;
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("layout.termimochi-layout.json");
        let mut store = DocumentStore::open(path.clone()).unwrap();
        let mut preset = LayoutPreset::new(LayoutSettings::default());
        store.save(&preset).unwrap();
        let original = std::fs::read(&path).unwrap();
        preset.layout.columns = 99;
        store.save(&preset).unwrap();
        let backup = std::fs::read_dir(root.path().join("termimochi-backups"))
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        assert_eq!(std::fs::read(&backup).unwrap(), original);
        for file in [&path, &backup] {
            assert_eq!(
                std::fs::metadata(file).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        let saved = std::fs::read(&path).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o400)).unwrap();
        preset.layout.rows = 30;
        assert!(store.save(&preset).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), saved);
    }
}
