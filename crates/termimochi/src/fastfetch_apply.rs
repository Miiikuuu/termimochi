//! Explicit, conflict-checked Fastfetch file transactions and durable rollback.
//! Nothing here edits a shell startup file or runs the resulting configuration.
use crate::{
    fastfetch_document::{self, LIMIT},
    typography_preset::{read_private_with_limit, retain_backup, write_checked_with_limit},
};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

const RECEIPT: &str = "last-fastfetch-apply.json";
// Two bounded byte arrays, including worst-case pretty-printed JSON expansion.
const RECEIPT_LIMIT: u64 = LIMIT * 32;

#[derive(Clone, Debug)]
pub(crate) struct Target {
    pub path: PathBuf,
    pub expected: Option<Vec<u8>>,
}
impl Target {
    pub fn open(path: PathBuf) -> Result<Self, String> {
        validate_path(&path)?;
        let expected = read_private_with_limit(&path, LIMIT)?;
        Ok(Self { path, expected })
    }
    pub fn source(&self) -> Result<String, String> {
        String::from_utf8(
            self.expected
                .clone()
                .ok_or("No Fastfetch configuration exists at this path.")?,
        )
        .map_err(|_| "Configuration must be UTF-8.".into())
    }
    pub fn check(&self) -> Result<(), String> {
        validate_path(&self.path)?;
        if read_private_with_limit(&self.path, LIMIT)? != self.expected {
            return Err("Fastfetch configuration changed outside TermiMochi. Reload it and review the changes again; nothing was overwritten.".into());
        }
        Ok(())
    }
}

pub(crate) fn default_path() -> PathBuf {
    let directory = gtk::glib::user_config_dir().join("fastfetch");
    let jsonc = directory.join("config.jsonc");
    if std::fs::symlink_metadata(&jsonc).is_err()
        && std::fs::symlink_metadata(directory.join("config.json")).is_ok()
    {
        directory.join("config.json")
    } else {
        jsonc
    }
}

fn validate_path(path: &Path) -> Result<(), String> {
    if !path.is_absolute()
        || !path
            .file_name()
            .and_then(|s| s.to_str())
            .is_some_and(|name| {
                matches!(name, "config.jsonc" | "config.json") || name.ends_with(".fastfetch.jsonc")
            })
    {
        return Err("Choose config.jsonc, config.json or a .fastfetch.jsonc file. Shell startup files cannot be targets.".into());
    }
    Ok(())
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Receipt {
    version: u8,
    path: PathBuf,
    before: Option<Vec<u8>>,
    after: Vec<u8>,
    restored: bool,
}

/// Read-only discovery, never permission to overwrite a previously applied file.
/// Restored receipts still identify the configuration the user was working on.
pub(crate) fn last_path(state: &Path) -> Result<Option<PathBuf>, String> {
    let Some(bytes) = read_private_with_limit(&state.join(RECEIPT), RECEIPT_LIMIT)? else {
        return Ok(None);
    };
    let receipt: Receipt = serde_json::from_slice(&bytes)
        .map_err(|e| format!("Invalid Fastfetch rollback record: {e}"))?;
    if receipt.version != 1
        || receipt.after.len() as u64 > LIMIT
        || receipt
            .before
            .as_ref()
            .is_some_and(|b| b.len() as u64 > LIMIT)
    {
        return Err("Invalid Fastfetch rollback record.".into());
    }
    validate_path(&receipt.path)?;
    Ok(Some(receipt.path))
}

pub(crate) fn apply(
    target: &mut Target,
    source: &str,
    state: &Path,
) -> Result<Option<PathBuf>, String> {
    fastfetch_document::parse(source)?;
    target.check()?;
    let bytes = source.as_bytes();
    if target.expected.as_deref() == Some(bytes) {
        return Ok(None);
    }
    let backup = target
        .expected
        .as_ref()
        .map(|before| retain_backup(&state.join("fastfetch-backups"), before))
        .transpose()?;
    let receipt = Receipt {
        version: 1,
        path: target.path.clone(),
        before: target.expected.clone(),
        after: bytes.to_vec(),
        restored: false,
    };
    let receipt_path = state.join(RECEIPT);
    let expected_receipt = read_private_with_limit(&receipt_path, RECEIPT_LIMIT)?;
    let encoded = serde_json::to_vec_pretty(&receipt).map_err(|e| e.to_string())?;
    // Prepare durable rollback data before changing the target. If the target
    // write fails, restore the previous receipt, not just the previous config.
    write_checked_with_limit(&receipt_path, &encoded, &expected_receipt, RECEIPT_LIMIT)?;
    if let Err(error) = write_checked_with_limit(&target.path, bytes, &target.expected, LIMIT) {
        if read_private_with_limit(&target.path, LIMIT)
            .ok()
            .flatten()
            .as_deref()
            == Some(bytes)
        {
            target.expected = Some(bytes.to_vec());
            return Err(format!(
                "The new configuration is present, but write finalization failed: {error}. The new rollback record was kept; review the file or use Restore Previous."
            ));
        }
        let rollback = if let Some(previous) = &expected_receipt {
            write_checked_with_limit(&receipt_path, previous, &Some(encoded), RECEIPT_LIMIT)
        } else if read_private_with_limit(&receipt_path, RECEIPT_LIMIT)? == Some(encoded) {
            std::fs::remove_file(&receipt_path).map_err(|e| e.to_string())
        } else {
            Err("Rollback record changed concurrently.".into())
        };
        return Err(match rollback {
            Ok(()) => error,
            Err(rollback) => format!(
                "{error} The previous rollback record could not be restored: {rollback}. Original-file backups remain in {}.",
                state.join("fastfetch-backups").display()
            ),
        });
    }
    target.expected = Some(bytes.to_vec());
    Ok(backup)
}

#[derive(Clone)]
pub(crate) struct RestorePlan {
    pub target: Target,
    pub before: Option<Vec<u8>>,
    receipt_bytes: Vec<u8>,
}
pub(crate) fn prepare_restore(state: &Path) -> Result<RestorePlan, String> {
    let bytes = read_private_with_limit(&state.join(RECEIPT), RECEIPT_LIMIT)?
        .ok_or("No previous Fastfetch apply to restore.")?;
    let receipt: Receipt = serde_json::from_slice(&bytes)
        .map_err(|e| format!("Invalid Fastfetch rollback record: {e}"))?;
    if receipt.version != 1
        || receipt.restored
        || receipt.after.len() as u64 > LIMIT
        || receipt
            .before
            .as_ref()
            .is_some_and(|b| b.len() as u64 > LIMIT)
    {
        return Err("No valid pending Fastfetch restore is available.".into());
    }
    let target = Target {
        path: receipt.path,
        expected: Some(receipt.after),
    };
    target.check()?;
    Ok(RestorePlan {
        target,
        before: receipt.before,
        receipt_bytes: bytes,
    })
}
pub(crate) fn restore(plan: &RestorePlan, state: &Path) -> Result<(), String> {
    plan.target.check()?;
    let path = state.join(RECEIPT);
    if read_private_with_limit(&path, RECEIPT_LIMIT)?.as_ref() != Some(&plan.receipt_bytes) {
        return Err("Rollback record changed after confirmation. Review again.".into());
    }
    if let Some(current) = &plan.target.expected {
        retain_backup(&state.join("fastfetch-backups"), current)?;
    }
    if let Some(before) = &plan.before {
        write_checked_with_limit(&plan.target.path, before, &plan.target.expected, LIMIT)?;
    } else {
        // Only remove the exact file created by our apply, after a second
        // identity/type/content check. The replaced content is backed up above.
        plan.target.check()?;
        std::fs::remove_file(&plan.target.path).map_err(|e| e.to_string())?;
    }
    let mut receipt: Receipt =
        serde_json::from_slice(&plan.receipt_bytes).map_err(|e| e.to_string())?;
    receipt.restored = true;
    write_checked_with_limit(
        &path,
        &serde_json::to_vec_pretty(&receipt).map_err(|e| e.to_string())?,
        &Some(plan.receipt_bytes.clone()),
        RECEIPT_LIMIT,
    )
    .map_err(|e| {
        format!("Configuration restored, but the rollback record could not be finalized: {e}")
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn discovery_is_read_only_and_validates_receipts_including_restored_targets() {
        let root = tempfile::tempdir().unwrap();
        let state = root.path().join("state");
        assert_eq!(last_path(&state).unwrap(), None);
        assert!(!state.exists());
        let path = root.path().join("custom.fastfetch.jsonc");
        std::fs::write(&path, "// before\n{}").unwrap();
        let mut target = Target::open(path.clone()).unwrap();
        apply(&mut target, "{\"modules\":[\"os\"]}", &state).unwrap();
        let bytes = std::fs::read(state.join(RECEIPT)).unwrap();
        assert_eq!(last_path(&state).unwrap(), Some(path.clone()));
        assert_eq!(std::fs::read(state.join(RECEIPT)).unwrap(), bytes);
        restore(&prepare_restore(&state).unwrap(), &state).unwrap();
        assert_eq!(last_path(&state).unwrap(), Some(path.clone()));
        // Deleted targets remain discoverable, so the UI can report the loss.
        std::fs::remove_file(&path).unwrap();
        assert_eq!(last_path(&state).unwrap(), Some(path));
        let mut receipt: Receipt = serde_json::from_slice(&bytes).unwrap();
        for invalid in [
            PathBuf::from("relative/config.jsonc"),
            root.path().join(".bashrc"),
        ] {
            receipt.path = invalid;
            std::fs::write(state.join(RECEIPT), serde_json::to_vec(&receipt).unwrap()).unwrap();
            assert!(last_path(&state).is_err());
        }
        std::fs::write(state.join(RECEIPT), "broken").unwrap();
        assert!(last_path(&state).is_err());
    }
    #[test]
    fn failed_second_apply_keeps_the_previous_restore_available() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let state = dir.path().join("state");
        let path = dir.path().join("config.jsonc");
        std::fs::write(&path, "// original\n{}").unwrap();
        let mut target = Target::open(path.clone()).unwrap();
        apply(&mut target, "{\"modules\":[\"os\"]}", &state).unwrap();
        let receipt = std::fs::read(state.join(RECEIPT)).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o400)).unwrap();
        assert!(apply(&mut target, "{\"modules\":[\"cpu\"]}", &state).is_err());
        assert_eq!(std::fs::read(state.join(RECEIPT)).unwrap(), receipt);
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        restore(&prepare_restore(&state).unwrap(), &state).unwrap();
        assert_eq!(std::fs::read_to_string(path).unwrap(), "// original\n{}");
    }
    #[test]
    fn maximum_size_documents_fit_the_durable_receipt() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.jsonc");
        let state = dir.path().join("state");
        let before = format!("//{}\n{{}}", "a".repeat(LIMIT as usize - 5));
        let after = format!("//{}\n{{}}", "b".repeat(LIMIT as usize - 5));
        assert_eq!(before.len() as u64, LIMIT);
        std::fs::write(&path, &before).unwrap();
        let mut target = Target::open(path.clone()).unwrap();
        apply(&mut target, &after, &state).unwrap();
        restore(&prepare_restore(&state).unwrap(), &state).unwrap();
        assert_eq!(std::fs::read_to_string(path).unwrap(), before);
    }
    #[test]
    fn apply_restore_survive_restart_and_preserve_exact_comments() {
        let dir = tempfile::tempdir().unwrap();
        let state = dir.path().join("state");
        let path = dir.path().join("config.jsonc");
        let before = "// personal comments\n{\"modules\":[\"os\"],}\n";
        std::fs::write(&path, before).unwrap();
        let mut target = Target::open(path.clone()).unwrap();
        let after = "{\"modules\":[\"cpu\"]}";
        let backup = apply(&mut target, after, &state).unwrap().unwrap();
        assert_eq!(std::fs::read_to_string(backup).unwrap(), before);
        let plan = prepare_restore(&state).unwrap();
        restore(&plan, &state).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), before);
        assert!(prepare_restore(&state).is_err());
    }
    #[test]
    fn conflicts_aliases_readonly_and_startup_targets_are_refused() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let state = dir.path().join("state");
        let path = dir.path().join("config.jsonc");
        assert!(Target::open(dir.path().join(".bashrc")).is_err());
        let mut target = Target::open(path.clone()).unwrap();
        std::fs::write(&path, "outside").unwrap();
        assert!(apply(&mut target, "{}", &state).is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "outside");
        let alias = dir.path().join("alias.fastfetch.jsonc");
        std::os::unix::fs::symlink(&path, &alias).unwrap();
        assert!(Target::open(alias).is_err());
        let alias = dir.path().join("hard.fastfetch.jsonc");
        std::fs::hard_link(&path, &alias).unwrap();
        assert!(Target::open(path.clone()).is_err());
        std::fs::remove_file(alias).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o400)).unwrap();
        let mut target = Target::open(path.clone()).unwrap();
        assert!(apply(&mut target, "{}", &state).is_err());
        assert_eq!(std::fs::read_to_string(path).unwrap(), "outside");
    }
    #[test]
    fn restore_checks_late_changes_and_only_removes_a_created_target() {
        let dir = tempfile::tempdir().unwrap();
        let state = dir.path().join("state");
        let path = dir.path().join("config.jsonc");
        let mut target = Target::open(path.clone()).unwrap();
        apply(&mut target, "{}", &state).unwrap();
        let plan = prepare_restore(&state).unwrap();
        std::fs::write(&path, "external").unwrap();
        assert!(restore(&plan, &state).is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "external");
        std::fs::write(&path, "{}").unwrap();
        restore(&plan, &state).unwrap();
        assert!(!path.exists());
        assert!(dir.path().exists());
    }
}
