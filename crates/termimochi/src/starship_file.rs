//! Explicit, backed-up writes. Editing and previewing never call this module's
//! save path. Snapshots prevent overwriting a configuration changed elsewhere.
use std::{
    fs::{self, File, Metadata},
    io::{Read, Write},
    os::unix::fs::{MetadataExt, PermissionsExt},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

const LIMIT: u64 = 256 * 1024;

#[derive(Clone, Debug)]
pub(crate) struct FileSnapshot {
    pub path: PathBuf,
    target: PathBuf,
    pub contents: String,
    identity: (u64, u64),
}

#[derive(Debug)]
pub(crate) struct SavedFile {
    pub snapshot: FileSnapshot,
    pub backup: PathBuf,
}

fn identity(metadata: &Metadata) -> (u64, u64) {
    (metadata.dev(), metadata.ino())
}

fn read_regular(path: &Path) -> Result<(String, Metadata), String> {
    let metadata = fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    if !metadata.is_file() || metadata.len() > LIMIT {
        return Err("Expected a regular configuration file of at most 256 KiB.".into());
    }
    let file = File::open(path).map_err(|e| e.to_string())?;
    let opened = file.metadata().map_err(|e| e.to_string())?;
    if identity(&metadata) != identity(&opened) || !opened.is_file() {
        return Err("The configuration changed while being read. Reload it and retry.".into());
    }
    let mut contents = String::new();
    file.take(LIMIT + 1)
        .read_to_string(&mut contents)
        .map_err(|e| e.to_string())?;
    if contents.len() as u64 > LIMIT {
        return Err("Configuration exceeds 256 KiB.".into());
    }
    Ok((contents, opened))
}

impl FileSnapshot {
    pub fn read(path: &Path) -> Result<Self, String> {
        let target = fs::canonicalize(path).map_err(|e| e.to_string())?;
        let (contents, metadata) = read_regular(&target)?;
        Ok(Self {
            path: path.to_owned(),
            target,
            contents,
            identity: identity(&metadata),
        })
    }

    pub fn bind(path: &Path, contents: &str) -> Result<Self, String> {
        let snapshot = Self::read(path)?;
        if snapshot.contents != contents {
            return Err("The configuration changed outside TermiMochi. Reload it before saving, or use Save As.".into());
        }
        Ok(snapshot)
    }

    pub fn verify(&self) -> Result<(), String> {
        let current = Self::read(&self.path)
            .map_err(|e| format!("Cannot verify the original configuration: {e}"))?;
        if current.target != self.target
            || current.identity != self.identity
            || current.contents != self.contents
        {
            return Err("The configuration changed outside TermiMochi. Nothing was overwritten. Reload it or use Save As.".into());
        }
        Ok(())
    }

    pub fn save(&self, contents: &str) -> Result<SavedFile, String> {
        self.verify()?;
        // An alias named .toml must never authorize writing a shell startup file.
        if [&self.path, &self.target].iter().any(|path| {
            path.extension()
                .is_none_or(|e| !e.eq_ignore_ascii_case("toml"))
        }) {
            return Err("Writing back requires a .toml configuration and target. Use Save As; shell startup files are never changed.".into());
        }
        crate::starship_draft::StarshipDraft::new(self.path.clone(), contents.into())?;
        let metadata = fs::metadata(&self.target).map_err(|e| e.to_string())?;
        if metadata.permissions().readonly() || metadata.nlink() != 1 {
            return Err(
                "The configuration is read-only or has hard-link aliases. Use Save As instead."
                    .into(),
            );
        }
        let parent = self
            .target
            .parent()
            .ok_or("Configuration has no parent directory.")?;
        // Backups are private, unique and persistent across application restarts.
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_nanos();
        let prefix = format!("{}{:020}-", self.backup_prefix()?, stamp);
        let mut backup = tempfile::Builder::new()
            .prefix(&prefix)
            .suffix(".bak")
            .tempfile_in(parent)
            .map_err(|e| format!("Cannot create backup; configuration unchanged: {e}"))?;
        backup
            .write_all(self.contents.as_bytes())
            .and_then(|_| backup.as_file().sync_all())
            .map_err(|e| format!("Cannot complete backup; configuration unchanged: {e}"))?;
        let (_, backup) = backup
            .keep()
            .map_err(|e| format!("Cannot retain backup; configuration unchanged: {e}"))?;
        let result: Result<FileSnapshot, String> = (|| {
            File::open(parent)
                .and_then(|directory| directory.sync_all())
                .map_err(|e| format!("Cannot sync the backup directory: {e}"))?;
            let mut temporary = tempfile::Builder::new()
                .prefix(".termimochi-starship-write-")
                .tempfile_in(parent)
                .map_err(|e| e.to_string())?;
            temporary
                .as_file()
                .set_permissions(fs::Permissions::from_mode(metadata.mode() & 0o777))
                .map_err(|e| e.to_string())?;
            temporary
                .write_all(contents.as_bytes())
                .and_then(|_| temporary.as_file().sync_all())
                .map_err(|e| e.to_string())?;
            let new_identity =
                identity(&temporary.as_file().metadata().map_err(|e| e.to_string())?);
            // Recheck after preparing both files, immediately before replacement.
            self.verify()?;
            let current = fs::metadata(&self.target).map_err(|e| e.to_string())?;
            if current.permissions().readonly() || current.nlink() != 1 {
                return Err("Configuration permissions or links changed before saving. Nothing was overwritten.".into());
            }
            temporary.persist(&self.target).map_err(|e| e.to_string())?;
            Ok(FileSnapshot {
                path: self.path.clone(),
                target: self.target.clone(),
                contents: contents.into(),
                identity: new_identity,
            })
        })();
        match result {
            Ok(snapshot) => Ok(SavedFile { snapshot, backup }),
            Err(error) => Err(format!("{error}\nBackup retained at {}", backup.display())),
        }
    }

    fn backup_prefix(&self) -> Result<String, String> {
        let name = self
            .target
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or("Configuration filename must be valid UTF-8 for backups.")?;
        Ok(format!(".{name}.termimochi-backup-"))
    }

    pub fn latest_backup(&self) -> Result<Self, String> {
        self.verify()?;
        let prefix = self.backup_prefix()?;
        let parent = self
            .target
            .parent()
            .ok_or("Configuration has no parent directory.")?;
        let mut latest = None;
        for entry in fs::read_dir(parent).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let name = entry.file_name();
            let Some(name) = name.to_str() else {
                continue;
            };
            if name.starts_with(&prefix)
                && name.ends_with(".bak")
                && entry.file_type().map_err(|e| e.to_string())?.is_file()
            {
                let path = entry.path();
                if latest.as_ref().is_none_or(|previous| &path > previous) {
                    latest = Some(path);
                }
            }
        }
        let path = latest.ok_or("No TermiMochi backup exists for this configuration yet.")?;
        let (contents, metadata) = read_regular(&path)?;
        crate::starship_draft::StarshipDraft::new(self.path.clone(), contents.clone())?;
        Ok(Self {
            target: path.clone(),
            path,
            contents,
            identity: identity(&metadata),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const OLD: &str = "# keep comments\n[rust]\nsymbol='rs '\n";
    const NEW: &str = "# keep comments\n[rust]\nsymbol='🦀 '\n";

    fn fixture() -> (tempfile::TempDir, PathBuf, FileSnapshot) {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("starship.toml");
        fs::write(&path, OLD).unwrap();
        let snapshot = FileSnapshot::bind(&path, OLD).unwrap();
        (temp, path, snapshot)
    }
    #[test]
    fn save_preserves_exact_backup_permissions_and_allows_safe_restore_after_restart() {
        let (_temp, path, snapshot) = fixture();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o640)).unwrap();
        let saved = snapshot.save(NEW).unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), NEW);
        assert_eq!(fs::read_to_string(&saved.backup).unwrap(), OLD);
        assert_eq!(fs::metadata(&path).unwrap().mode() & 0o777, 0o640);
        assert_eq!(fs::metadata(&saved.backup).unwrap().mode() & 0o777, 0o600);
        let reopened = FileSnapshot::read(&path).unwrap();
        let previous = reopened.latest_backup().unwrap();
        assert_eq!(previous.contents, OLD);
        let restored = reopened.save(&previous.contents).unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), OLD);
        assert_eq!(fs::read_to_string(restored.backup).unwrap(), NEW);
        assert_eq!(restored.snapshot.latest_backup().unwrap().contents, NEW);
    }
    #[test]
    fn external_edits_deletion_replacement_and_invalid_toml_never_get_overwritten() {
        let (_temp, path, snapshot) = fixture();
        fs::write(&path, "# external edit").unwrap();
        assert!(snapshot.save(NEW).unwrap_err().contains("changed outside"));
        assert_eq!(fs::read_to_string(&path).unwrap(), "# external edit");
        fs::remove_file(&path).unwrap();
        assert!(snapshot.save(NEW).is_err());
        assert!(!path.exists());
        fs::write(&path, OLD).unwrap();
        let snapshot = FileSnapshot::read(&path).unwrap();
        assert!(snapshot.save("[invalid").is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), OLD);
    }
    #[test]
    fn symlinks_keep_the_link_and_retargeting_is_refused() {
        let (temp, path, _) = fixture();
        let alias = temp.path().join("alias.toml");
        std::os::unix::fs::symlink(&path, &alias).unwrap();
        let snapshot = FileSnapshot::read(&alias).unwrap();
        snapshot.save(NEW).unwrap();
        assert!(fs::symlink_metadata(&alias).unwrap().is_symlink());
        assert_eq!(fs::read_to_string(&path).unwrap(), NEW);
        let snapshot = FileSnapshot::read(&alias).unwrap();
        let other = temp.path().join("other.toml");
        fs::write(&other, NEW).unwrap();
        fs::remove_file(&alias).unwrap();
        std::os::unix::fs::symlink(&other, &alias).unwrap();
        assert!(snapshot.save(OLD).is_err());
        assert_eq!(fs::read_to_string(&other).unwrap(), NEW);
    }
    #[test]
    fn read_only_hard_links_and_shell_file_aliases_are_refused() {
        let (temp, path, snapshot) = fixture();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o400)).unwrap();
        assert!(snapshot.save(NEW).is_err());
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        let hard = temp.path().join("hard.toml");
        fs::hard_link(&path, &hard).unwrap();
        assert!(snapshot.save(NEW).is_err());
        assert_eq!(fs::read_to_string(&hard).unwrap(), OLD);
        let shell = temp.path().join(".bashrc");
        fs::write(&shell, OLD).unwrap();
        let alias = temp.path().join("shell.toml");
        std::os::unix::fs::symlink(&shell, &alias).unwrap();
        assert!(FileSnapshot::read(&alias).unwrap().save(NEW).is_err());
        assert_eq!(fs::read_to_string(&shell).unwrap(), OLD);
    }
    #[test]
    fn bad_or_replaced_backups_cannot_silently_restore() {
        let (temp, path, snapshot) = fixture();
        let saved = snapshot.save(NEW).unwrap();
        let backup = saved.snapshot.latest_backup().unwrap();
        fs::write(&backup.path, "# tampered").unwrap();
        assert!(backup.verify().is_err());
        fs::write(&backup.path, "[broken").unwrap();
        assert!(saved.snapshot.latest_backup().is_err());
        fs::remove_file(&saved.backup).unwrap();
        std::os::unix::fs::symlink(&path, &saved.backup).unwrap();
        assert!(saved.snapshot.latest_backup().is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), NEW);
        assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 2);
    }
    #[test]
    fn replacing_a_file_with_identical_bytes_still_requires_reload() {
        let (temp, path, snapshot) = fixture();
        let replacement = temp.path().join("replacement.toml");
        fs::write(&replacement, OLD).unwrap();
        fs::rename(replacement, &path).unwrap();
        assert!(snapshot.save(NEW).is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), OLD);
        assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 1);
    }
    #[test]
    fn backup_creation_failure_leaves_original_untouched() {
        let temp = tempfile::tempdir().unwrap();
        // Valid original filename, but its prefixed backup exceeds NAME_MAX.
        let path = temp.path().join(format!("{}.toml", "a".repeat(230)));
        fs::write(&path, OLD).unwrap();
        let snapshot = FileSnapshot::read(&path).unwrap();
        assert!(
            snapshot
                .save(NEW)
                .unwrap_err()
                .contains("Cannot create backup")
        );
        assert_eq!(fs::read_to_string(&path).unwrap(), OLD);
        assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 1);
    }
}
