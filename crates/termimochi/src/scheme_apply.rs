//! One reviewed plan, independent module transactions, one durable result.
//! Portable workspaces never carry destinations or permission to apply.
pub(crate) mod activation;
#[cfg(test)]
mod tests;

use crate::{
    fastfetch_apply, layout_apply,
    ptyxis::{InstallOutcome, PtyxisInstaller},
    starship_file::FileSnapshot,
    typography_apply, typography_preset,
};
use serde::{Deserialize, Serialize};
use std::{
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

const REPORT_LIMIT: u64 = 2 * 1024 * 1024;
const FILE_LIMIT: u64 = 256 * 1024;
static NEXT: AtomicU64 = AtomicU64::new(0);

pub(crate) enum Action {
    Palette {
        installer: PtyxisInstaller,
        name: String,
        before: Option<Vec<u8>>,
        contents: String,
    },
    Activate(activation::Activation),
    Typography(typography_apply::ApplyRequest),
    Layout(layout_apply::ApplyRequest),
    Starship {
        file: FileSnapshot,
        contents: String,
    },
    ExportStarship(String),
    Fastfetch {
        target: fastfetch_apply::Target,
        source: String,
    },
}

pub(crate) struct Item {
    pub id: &'static str,
    pub title: String,
    pub detail: String,
    pub versions: Option<(String, String)>,
    pub action: Option<Action>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum Status {
    Pending,
    Applied,
    Exported,
    NotEnabled,
    Unsupported,
    Failed,
    Skipped,
    Unchanged,
    Restored,
    RestoreBlocked,
}

impl Status {
    pub fn label(self) -> &'static str {
        match self {
            Self::Pending => "Interrupted / review recovery",
            Self::Applied => "Applied",
            Self::Exported => "Exported only",
            Self::NotEnabled => "Installed · not enabled by this action",
            Self::Unsupported => "Unsupported / preview only",
            Self::Failed => "Failed",
            Self::Skipped => "Not selected",
            Self::Unchanged => "Already matches",
            Self::Restored => "Restored",
            Self::RestoreBlocked => "Restore blocked · external changes kept",
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
enum Undo {
    Palette {
        directory: PathBuf,
        id: String,
    },
    Activate,
    Typography,
    Layout,
    Starship {
        path: PathBuf,
        before: String,
        after: String,
    },
    Fastfetch,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ResultItem {
    pub id: String,
    pub title: String,
    pub detail: String,
    pub status: Status,
    pub path: Option<PathBuf>,
    undo: Option<Undo>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Report {
    version: u8,
    pub target: String,
    pub profile_uuid: Option<String>,
    pub items: Vec<ResultItem>,
}

pub(crate) struct Plan {
    pub directory: PathBuf,
    pub target: String,
    pub profile_uuid: Option<String>,
    pub items: Vec<Item>,
}

impl Plan {
    pub fn new(root: &Path, target: String, profile_uuid: Option<String>) -> Self {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        Self {
            directory: root
                .join("scheme-applies")
                .join(format!("{stamp}-{}", NEXT.fetch_add(1, Ordering::Relaxed))),
            target,
            profile_uuid,
            items: Vec::new(),
        }
    }

    /// Preparing/reviewing/cancelling is read-only. Only this call creates state.
    /// A journal entry precedes each module, so interrupted runs remain discoverable.
    pub fn apply(self, selected: &[bool]) -> Result<(PathBuf, Report), String> {
        if selected.len() != self.items.len()
            || !self
                .items
                .iter()
                .zip(selected)
                .any(|(item, selected)| *selected && item.action.is_some())
        {
            return Err("Select at least one supported change.".into());
        }
        use std::os::unix::fs::DirBuilderExt;
        let parent = self
            .directory
            .parent()
            .ok_or("Missing application directory.")?;
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&self.directory)
            .map_err(|e| e.to_string())?;
        let mut report = Report {
            version: 1,
            target: self.target,
            profile_uuid: self.profile_uuid,
            items: self
                .items
                .iter()
                .map(|item| ResultItem {
                    id: item.id.into(),
                    title: item.title.clone(),
                    detail: item.detail.clone(),
                    status: if item.action.is_some() {
                        Status::Skipped
                    } else {
                        Status::Unsupported
                    },
                    undo: None,
                    path: item.action.as_ref().and_then(Action::path),
                })
                .collect(),
        };
        report.save(&self.directory)?;
        typography_preset::write_private(
            &parent.join("latest"),
            self.directory.file_name().unwrap().as_encoded_bytes(),
        )?;
        let mut palette_ready = false;
        for (index, (item, selected)) in self.items.into_iter().zip(selected).enumerate() {
            let Some(action) = item.action.filter(|_| *selected) else {
                continue;
            };
            if matches!(action, Action::Activate(_)) && !palette_ready {
                report.items[index].status = Status::Failed;
                report.items[index].detail.push_str(
                    "\nPalette installation was not selected or failed. Activation was skipped.",
                );
                report.save(&self.directory)?;
                continue;
            }
            report.items[index].status = Status::Pending;
            report.items[index].undo = action.undo();
            report.save(&self.directory)?;
            let result = action.apply(&self.directory);
            let row = &mut report.items[index];
            match result {
                Ok((status, changed, detail)) => {
                    row.status = status;
                    row.detail.push_str(&format!("\n{detail}"));
                    if !changed {
                        row.undo = None;
                    }
                    if row.id == "palette" {
                        palette_ready = true;
                    }
                }
                Err(error) => {
                    row.status = Status::Failed;
                    row.detail.push_str(&format!("\n{error}"));
                    // Keep recovery only when a module actually prepared a receipt.
                    if !row
                        .undo
                        .as_ref()
                        .is_some_and(|undo| undo.has_receipt(&self.directory))
                    {
                        row.undo = None;
                    }
                }
            }
            report.save(&self.directory).map_err(|e| format!("Changes may already be present. Result recording failed: {e}. Reopen Last Application to recover the journal at {}.", self.directory.display()))?;
        }
        Ok((self.directory, report))
    }
}

impl Action {
    fn path(&self) -> Option<PathBuf> {
        match self {
            Self::Palette {
                installer, name, ..
            } => Some(installer.palette_dir().join(name)),
            Self::Starship { file, .. } => Some(file.path.clone()),
            Self::Fastfetch { target, .. } => Some(target.path.clone()),
            _ => None,
        }
    }
    fn undo(&self) -> Option<Undo> {
        Some(match self {
            Self::Palette {
                installer, name, ..
            } => Undo::Palette {
                directory: installer.palette_dir().into(),
                id: name.trim_end_matches(".palette").into(),
            },
            Self::Activate(_) => Undo::Activate,
            Self::Typography(_) => Undo::Typography,
            Self::Layout(_) => Undo::Layout,
            Self::Starship { file, contents } => Undo::Starship {
                path: file.path.clone(),
                before: file.contents.clone(),
                after: contents.clone(),
            },
            Self::ExportStarship(_) => return None,
            Self::Fastfetch { .. } => Undo::Fastfetch,
        })
    }

    fn apply(self, directory: &Path) -> Result<(Status, bool, String), String> {
        match self {
            Self::Palette {
                installer,
                name,
                before,
                contents,
            } => {
                let target = installer.palette_dir().join(&name);
                if typography_preset::read_private_with_limit(&target, FILE_LIMIT)? != before {
                    return Err(
                        "Palette file changed during review. Nothing was overwritten.".into(),
                    );
                }
                let installer =
                    PtyxisInstaller::new(installer.palette_dir().into(), directory.join("palette"));
                let changed = matches!(
                    installer
                        .install_reviewed(&name, contents.as_bytes(), &before)
                        .map_err(|e| e.to_string())?,
                    InstallOutcome::Installed(_)
                );
                Ok((
                    Status::NotEnabled,
                    changed,
                    format!(
                        "Installed at {}. See the separate activation result.",
                        target.display()
                    ),
                ))
            }
            Self::Activate(request) => {
                let changed = request.apply(directory)?;
                Ok((
                    if changed {
                        Status::Applied
                    } else {
                        Status::Unchanged
                    },
                    changed,
                    "Palette selection and Light/Dark reviewed above are active in Ptyxis.".into(),
                ))
            }
            Self::Typography(request) => {
                let changed = request.apply(directory)?.is_some();
                Ok((
                    if changed {
                        Status::Applied
                    } else {
                        Status::Unchanged
                    },
                    changed,
                    "Ptyxis font and profile spacing match the reviewed settings.".into(),
                ))
            }
            Self::Layout(request) => {
                let changed = request.apply(directory)?.is_some();
                Ok((
                    if changed {
                        Status::Applied
                    } else {
                        Status::Unchanged
                    },
                    changed,
                    "Supported layout settings match. Open a new window to verify the grid.".into(),
                ))
            }
            Self::Starship { file, contents } => {
                file.verify()?;
                let changed = file.contents != contents;
                if changed {
                    file.save(&contents)?;
                }
                Ok((
                    if changed {
                        Status::Applied
                    } else {
                        Status::Unchanged
                    },
                    changed,
                    format!(
                        "Wrote {}. Effective only in shells already configured to use this file; startup is unchanged.",
                        file.path.display()
                    ),
                ))
            }
            Self::ExportStarship(contents) => {
                let path = directory.join("starship.toml");
                crate::starship_draft::StarshipDraft::new(path.clone(), contents.clone())?;
                typography_preset::write_checked_with_limit(
                    &path,
                    contents.as_bytes(),
                    &None,
                    FILE_LIMIT,
                )?;
                Ok((
                    Status::Exported,
                    false,
                    format!(
                        "Export: {}. Not enabled. Review and install it separately; .bashrc and .zshrc are untouched.",
                        path.display()
                    ),
                ))
            }
            Self::Fastfetch { mut target, source } => {
                let changed = target.expected.as_deref() != Some(source.as_bytes());
                fastfetch_apply::apply(&mut target, &source, directory)?;
                Ok((
                    if changed {
                        Status::Applied
                    } else {
                        Status::Unchanged
                    },
                    changed,
                    format!(
                        "Wrote {}. Run fastfetch with this config to verify. No startup hook was added; no commands were executed.",
                        target.path.display()
                    ),
                ))
            }
        }
    }
}

impl Undo {
    fn has_receipt(&self, directory: &Path) -> bool {
        let name = match self {
            Self::Palette { .. } => "palette/last-ptyxis-install.json",
            Self::Activate => "activation.json",
            Self::Typography => "last-typography-apply.json",
            Self::Layout => "last-layout-apply.json",
            Self::Fastfetch => "last-fastfetch-apply.json",
            Self::Starship { path, after, .. } => return FileSnapshot::bind(path, after).is_ok(),
        };
        directory.join(name).is_file()
    }

    fn restore(&self, directory: &Path) -> Result<(), String> {
        match self {
            Self::Palette {
                directory: palettes,
                id,
            } => {
                let installer = PtyxisInstaller::new(palettes.clone(), directory.join("palette"));
                let receipt = installer
                    .last_receipt()
                    .map_err(|e| e.to_string())?
                    .ok_or("No palette receipt.")?;
                typography_preset::read_private_with_limit(receipt.target(), FILE_LIMIT)?;
                if receipt.backup().is_none() && activation::palette_in_use(id) {
                    return Err("A Ptyxis profile still uses this palette. Restore its selection first; the palette was kept.".into());
                }
                installer.rollback().map(|_| ()).map_err(|e| e.to_string())
            }
            Self::Activate => activation::restore(directory),
            Self::Typography => typography_apply::RestoreRequest::load(directory)?.restore(),
            Self::Layout => layout_apply::RestoreRequest::load(directory)?.restore(),
            Self::Starship {
                path,
                before,
                after,
            } => {
                let file = FileSnapshot::bind(path, after)?;
                file.save(before).map(|_| ())
            }
            Self::Fastfetch => {
                fastfetch_apply::restore(&fastfetch_apply::prepare_restore(directory)?, directory)
            }
        }
    }
}

impl Report {
    fn save(&self, directory: &Path) -> Result<(), String> {
        let path = directory.join("report.json");
        let expected = typography_preset::read_private_with_limit(&path, REPORT_LIMIT)?;
        typography_preset::write_checked_with_limit(
            &path,
            &serde_json::to_vec_pretty(self).map_err(|e| e.to_string())?,
            &expected,
            REPORT_LIMIT,
        )
    }

    pub fn latest(root: &Path) -> Result<(PathBuf, Self), String> {
        let parent = root.join("scheme-applies");
        let id = typography_preset::read_private(&parent.join("latest"))?
            .ok_or("No scheme has been applied yet.")?;
        let id = std::str::from_utf8(&id).map_err(|e| e.to_string())?;
        if id.is_empty() || id.len() > 80 || !id.bytes().all(|b| b.is_ascii_digit() || b == b'-') {
            return Err("Invalid application identifier.".into());
        }
        let directory = parent.join(id);
        let report = Self::load(&directory)?;
        Ok((directory, report))
    }

    pub fn load(directory: &Path) -> Result<Self, String> {
        let bytes = typography_preset::read_private_with_limit(
            &directory.join("report.json"),
            REPORT_LIMIT,
        )?
        .ok_or("Missing application report.")?;
        let report: Self = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
        if report.version != 1 || report.items.len() > 16 {
            return Err("Unsupported application report.".into());
        }
        Ok(report)
    }

    pub fn can_restore(&self) -> bool {
        self.items.iter().any(|item| item.undo.is_some())
    }

    /// Reverse dependency order. Failure in one module does not discard recovery
    /// for others. Successfully restored rows are never restored a second time.
    pub fn restore(&mut self, directory: &Path) -> Result<(), String> {
        let mut activation_blocked = false;
        for index in (0..self.items.len()).rev() {
            let Some(undo) = self.items[index].undo.clone() else {
                continue;
            };
            let result = if matches!(undo, Undo::Palette { .. }) && activation_blocked {
                Err("Palette retained because its activation could not be restored.".into())
            } else {
                undo.restore(directory)
            };
            let row = &mut self.items[index];
            match result {
                Ok(()) => {
                    row.status = Status::Restored;
                    row.undo = None;
                    row.detail.push_str("\nPrevious external values restored. Workspace edits and exports are kept.");
                }
                Err(error) => {
                    activation_blocked |= matches!(undo, Undo::Activate);
                    row.status = Status::RestoreBlocked;
                    row.detail.push_str(&format!("\nRestore blocked: {error}"));
                }
            }
            self.save(directory)?;
        }
        Ok(())
    }
}
