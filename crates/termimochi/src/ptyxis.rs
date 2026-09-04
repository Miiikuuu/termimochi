use std::{
    env,
    error::Error,
    ffi::OsString,
    fmt, fs, io,
    path::{Component, Path, PathBuf},
    process::Command,
    string::FromUtf8Error,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use gtk::{
    gio::{self, prelude::*},
    glib,
};
use serde::{Deserialize, Serialize};
use termimochi_core::{PaletteError, PtyxisPalette, Variant, write_atomically};

static NEXT_TRANSACTION: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug)]
pub(crate) struct PtyxisInstaller {
    palette_dir: PathBuf,
    state_dir: PathBuf,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) struct InstallReceipt {
    version: u8,
    target: PathBuf,
    backup: Option<PathBuf>,
    installed_length: u64,
    installed_hash: u64,
}

impl InstallReceipt {
    pub(crate) fn target(&self) -> &Path {
        &self.target
    }

    pub(crate) fn backup(&self) -> Option<&Path> {
        self.backup.as_deref()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum InstallOutcome {
    Installed(InstallReceipt),
    Unchanged(PathBuf),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum RollbackOutcome {
    Restored { target: PathBuf, backup: PathBuf },
    Removed { target: PathBuf },
}

#[derive(Clone, Debug)]
pub(crate) struct CurrentPtyxisPalette {
    palette: PtyxisPalette,
    preferred_variant: Option<Variant>,
}

impl CurrentPtyxisPalette {
    pub(crate) fn into_parts(self) -> (PtyxisPalette, Option<Variant>) {
        (self.palette, self.preferred_variant)
    }
}

/// Read the palette selected by the Ptyxis profile that launched us. When the
/// app is opened from the desktop, use Ptyxis' default profile instead.
pub(crate) fn import_current_palette() -> Result<CurrentPtyxisPalette, PaletteImportError> {
    let global = open_settings(
        "org.gnome.Ptyxis",
        None,
        &["default-profile-uuid", "profile-uuids", "interface-style"],
    )?;
    let profile_uuid = current_profile_uuid(&global).ok_or(PaletteImportError::NoProfile)?;
    let profile_path = format!("/org/gnome/Ptyxis/Profiles/{profile_uuid}/");
    let profile = open_settings(
        "org.gnome.Ptyxis.Profile",
        Some(&profile_path),
        &["palette"],
    )?;
    let palette_id = profile.string("palette").trim().to_owned();
    let file_name = palette_file_name(&palette_id)
        .ok_or_else(|| PaletteImportError::InvalidPaletteId(palette_id.clone()))?;

    let text = match read_palette_from_data_dirs(&file_name, &ptyxis_palette_directories())? {
        Some(text) => text,
        None => extract_builtin_palette(&file_name, &palette_id)?,
    };
    let palette = PtyxisPalette::from_text(&text).map_err(PaletteImportError::InvalidPalette)?;
    let preferred_variant = match global.string("interface-style").as_str() {
        "light" => Some(Variant::Light),
        "dark" => Some(Variant::Dark),
        _ => None,
    };

    Ok(CurrentPtyxisPalette {
        palette,
        preferred_variant,
    })
}

fn open_settings(
    schema_id: &'static str,
    path: Option<&str>,
    required_keys: &[&'static str],
) -> Result<gio::Settings, PaletteImportError> {
    let source =
        gio::SettingsSchemaSource::default().ok_or(PaletteImportError::MissingSchema(schema_id))?;
    let schema = source
        .lookup(schema_id, true)
        .ok_or(PaletteImportError::MissingSchema(schema_id))?;
    for key in required_keys {
        if !schema.has_key(key) {
            return Err(PaletteImportError::MissingSetting {
                schema: schema_id,
                key,
            });
        }
    }
    Ok(gio::Settings::new_full(
        &schema,
        gio::SettingsBackend::NONE,
        path,
    ))
}

fn current_profile_uuid(settings: &gio::Settings) -> Option<String> {
    env::var("PTYXIS_PROFILE")
        .ok()
        .filter(|value| valid_profile_uuid(value))
        .or_else(|| {
            let value = settings.string("default-profile-uuid").to_string();
            valid_profile_uuid(&value).then_some(value)
        })
        .or_else(|| {
            settings
                .strv("profile-uuids")
                .first()
                .map(ToString::to_string)
                .filter(|value| valid_profile_uuid(value))
        })
}

fn valid_profile_uuid(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

fn palette_file_name(palette_id: &str) -> Option<String> {
    if palette_id.is_empty() || palette_id.chars().any(char::is_control) {
        return None;
    }
    let path = Path::new(palette_id);
    let mut components = path.components();
    if !matches!(components.next(), Some(Component::Normal(_))) || components.next().is_some() {
        return None;
    }
    Some(
        if path.extension().is_some_and(|value| value == "palette") {
            palette_id.to_owned()
        } else {
            format!("{palette_id}.palette")
        },
    )
}

fn ptyxis_palette_directories() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    let home = env::var_os("HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from);
    if let Some(data_home) = absolute_env_path("XDG_DATA_HOME") {
        roots.push(data_home);
    } else if let Some(home) = &home {
        roots.push(home.join(".local/share"));
    }

    if let Some(data_dirs) = env::var_os("XDG_DATA_DIRS").filter(|value| !value.is_empty()) {
        roots.extend(env::split_paths(&data_dirs).filter(|path| path.is_absolute()));
    } else {
        roots.extend([
            PathBuf::from("/usr/local/share"),
            PathBuf::from("/usr/share"),
        ]);
    }

    if let Some(home) = home {
        roots.push(home.join(".var/app/org.gnome.Ptyxis/data"));
    }
    roots.dedup();
    roots
        .into_iter()
        .map(|root| root.join("org.gnome.Ptyxis/palettes"))
        .collect()
}

fn read_palette_from_data_dirs(
    file_name: &str,
    directories: &[PathBuf],
) -> Result<Option<String>, PaletteImportError> {
    for directory in directories {
        let path = directory.join(file_name);
        match fs::read_to_string(&path) {
            Ok(text) => return Ok(Some(text)),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(source) => return Err(PaletteImportError::io(path, source)),
        }
    }
    Ok(None)
}

fn extract_builtin_palette(
    file_name: &str,
    palette_id: &str,
) -> Result<String, PaletteImportError> {
    let ptyxis = glib::find_program_in_path("ptyxis")
        .ok_or_else(|| PaletteImportError::PaletteNotFound(palette_id.to_owned()))?;
    let gresource = glib::find_program_in_path("gresource")
        .ok_or_else(|| PaletteImportError::PaletteNotFound(palette_id.to_owned()))?;
    let resource_path = format!("/org/gnome/Ptyxis/palettes/{file_name}");
    let output = Command::new(&gresource)
        .arg("extract")
        .arg(&ptyxis)
        .arg(resource_path)
        .output()
        .map_err(|source| PaletteImportError::Command {
            program: gresource,
            source,
        })?;
    if !output.status.success() || output.stdout.is_empty() {
        return Err(PaletteImportError::PaletteNotFound(palette_id.to_owned()));
    }
    String::from_utf8(output.stdout).map_err(PaletteImportError::InvalidEncoding)
}

#[derive(Debug)]
pub(crate) enum PaletteImportError {
    MissingSchema(&'static str),
    MissingSetting {
        schema: &'static str,
        key: &'static str,
    },
    NoProfile,
    InvalidPaletteId(String),
    PaletteNotFound(String),
    Io {
        path: PathBuf,
        source: io::Error,
    },
    Command {
        program: PathBuf,
        source: io::Error,
    },
    InvalidEncoding(FromUtf8Error),
    InvalidPalette(PaletteError),
}

impl PaletteImportError {
    fn io(path: impl Into<PathBuf>, source: io::Error) -> Self {
        Self::Io {
            path: path.into(),
            source,
        }
    }
}

impl fmt::Display for PaletteImportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingSchema(schema) => write!(formatter, "missing GSettings schema {schema}"),
            Self::MissingSetting { schema, key } => {
                write!(formatter, "{schema} does not provide setting {key}")
            }
            Self::NoProfile => formatter.write_str("Ptyxis has no configured profile"),
            Self::InvalidPaletteId(id) => write!(formatter, "invalid Ptyxis palette id {id:?}"),
            Self::PaletteNotFound(id) => write!(formatter, "Ptyxis palette {id:?} was not found"),
            Self::Io { path, source } => {
                write!(formatter, "cannot read {}: {source}", path.display())
            }
            Self::Command { program, source } => {
                write!(formatter, "cannot run {}: {source}", program.display())
            }
            Self::InvalidEncoding(source) => {
                write!(formatter, "Ptyxis palette is not UTF-8: {source}")
            }
            Self::InvalidPalette(source) => write!(formatter, "invalid Ptyxis palette: {source}"),
        }
    }
}

impl Error for PaletteImportError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io { source, .. } | Self::Command { source, .. } => Some(source),
            Self::InvalidEncoding(source) => Some(source),
            Self::InvalidPalette(source) => Some(source),
            _ => None,
        }
    }
}

impl PtyxisInstaller {
    pub(crate) fn from_environment() -> Result<Self, InstallError> {
        let home = env::var_os("HOME")
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .ok_or(InstallError::MissingHome)?;
        let data_home =
            absolute_env_path("XDG_DATA_HOME").unwrap_or_else(|| home.join(".local").join("share"));
        let state_home = absolute_env_path("XDG_STATE_HOME")
            .unwrap_or_else(|| home.join(".local").join("state"));
        Ok(Self::new(
            data_home.join("org.gnome.Ptyxis").join("palettes"),
            state_home.join("termimochi"),
        ))
    }

    pub(crate) fn new(palette_dir: PathBuf, state_dir: PathBuf) -> Self {
        Self {
            palette_dir,
            state_dir,
        }
    }

    pub(crate) fn palette_dir(&self) -> &Path {
        &self.palette_dir
    }

    pub(crate) fn receipt_path(&self) -> PathBuf {
        self.state_dir.join("last-ptyxis-install.json")
    }

    pub(crate) fn last_receipt(&self) -> Result<Option<InstallReceipt>, InstallError> {
        let path = self.receipt_path();
        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(source) => return Err(InstallError::io("read rollback receipt", path, source)),
        };
        let receipt = serde_json::from_slice::<InstallReceipt>(&bytes)
            .map_err(|source| InstallError::InvalidReceipt { path, source })?;
        if receipt.version != 1 {
            return Err(InstallError::UnsupportedReceipt(receipt.version));
        }
        self.validate_receipt_paths(&receipt)?;
        Ok(Some(receipt))
    }

    fn validate_receipt_paths(&self, receipt: &InstallReceipt) -> Result<(), InstallError> {
        let target_name = receipt
            .target
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| InstallError::InvalidReceiptPath {
                field: "target",
                path: receipt.target.clone(),
            })?;
        if receipt.target.parent() != Some(self.palette_dir.as_path())
            || validate_file_name(target_name).is_err()
        {
            return Err(InstallError::InvalidReceiptPath {
                field: "target",
                path: receipt.target.clone(),
            });
        }

        if let Some(backup) = &receipt.backup {
            let expected_parent = self.state_dir.join("ptyxis-backups");
            let valid_name = backup
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| !name.starts_with('.') && name.ends_with(".palette.bak"));
            if backup.parent() != Some(expected_parent.as_path()) || !valid_name {
                return Err(InstallError::InvalidReceiptPath {
                    field: "backup",
                    path: backup.clone(),
                });
            }
        }
        Ok(())
    }

    pub(crate) fn install(
        &self,
        file_name: &str,
        contents: &[u8],
    ) -> Result<InstallOutcome, InstallError> {
        validate_file_name(file_name)?;
        fs::create_dir_all(&self.palette_dir).map_err(|source| {
            InstallError::io("create Ptyxis palette directory", &self.palette_dir, source)
        })?;
        fs::create_dir_all(&self.state_dir).map_err(|source| {
            InstallError::io("create TermiMochi state directory", &self.state_dir, source)
        })?;

        let target = self.palette_dir.join(file_name);
        let previous = match fs::read(&target) {
            Ok(bytes) if bytes == contents => return Ok(InstallOutcome::Unchanged(target)),
            Ok(bytes) => Some(bytes),
            Err(error) if error.kind() == io::ErrorKind::NotFound => None,
            Err(source) => return Err(InstallError::io("read existing palette", target, source)),
        };

        let backup = if let Some(previous) = previous.as_deref() {
            let backup_dir = self.state_dir.join("ptyxis-backups");
            fs::create_dir_all(&backup_dir).map_err(|source| {
                InstallError::io("create backup directory", &backup_dir, source)
            })?;
            let backup_path = backup_dir.join(unique_backup_name(file_name));
            write_atomically(&backup_path, previous)
                .map_err(|source| InstallError::io("write palette backup", &backup_path, source))?;
            Some(backup_path)
        } else {
            None
        };

        let (installed_length, installed_hash) = fingerprint(contents);
        let receipt = InstallReceipt {
            version: 1,
            target: target.clone(),
            backup,
            installed_length,
            installed_hash,
        };

        write_atomically(&target, contents)
            .map_err(|source| InstallError::io("install palette", &target, source))?;
        if let Err(error) = self.write_receipt(&receipt) {
            self.undo_uncommitted_install(&receipt)?;
            return Err(error);
        }

        Ok(InstallOutcome::Installed(receipt))
    }

    pub(crate) fn rollback(&self) -> Result<RollbackOutcome, InstallError> {
        let receipt = self.last_receipt()?.ok_or(InstallError::NoRollback)?;
        let installed = fs::read(&receipt.target).map_err(|source| {
            InstallError::io("read installed palette", &receipt.target, source)
        })?;
        let (length, hash) = fingerprint(&installed);
        if length != receipt.installed_length || hash != receipt.installed_hash {
            return Err(InstallError::TargetModified(receipt.target));
        }

        let outcome = if let Some(backup) = &receipt.backup {
            let original = fs::read(backup)
                .map_err(|source| InstallError::io("read palette backup", backup, source))?;
            write_atomically(&receipt.target, &original).map_err(|source| {
                InstallError::io("restore palette backup", &receipt.target, source)
            })?;
            RollbackOutcome::Restored {
                target: receipt.target.clone(),
                backup: backup.clone(),
            }
        } else {
            fs::remove_file(&receipt.target).map_err(|source| {
                InstallError::io("remove newly installed palette", &receipt.target, source)
            })?;
            RollbackOutcome::Removed {
                target: receipt.target.clone(),
            }
        };

        let journal = self.receipt_path();
        fs::remove_file(&journal)
            .map_err(|source| InstallError::io("remove rollback receipt", journal, source))?;
        Ok(outcome)
    }

    fn write_receipt(&self, receipt: &InstallReceipt) -> Result<(), InstallError> {
        let mut json = serde_json::to_vec_pretty(receipt)
            .expect("the rollback receipt always serializes to JSON");
        json.push(b'\n');
        let path = self.receipt_path();
        write_atomically(&path, &json)
            .map_err(|source| InstallError::io("write rollback receipt", path, source))
    }

    fn undo_uncommitted_install(&self, receipt: &InstallReceipt) -> Result<(), InstallError> {
        if let Some(backup) = &receipt.backup {
            let original = fs::read(backup)
                .map_err(|source| InstallError::io("read palette backup", backup, source))?;
            write_atomically(&receipt.target, &original).map_err(|source| {
                InstallError::io(
                    "restore palette after failed transaction",
                    &receipt.target,
                    source,
                )
            })
        } else {
            fs::remove_file(&receipt.target).map_err(|source| {
                InstallError::io(
                    "remove palette after failed transaction",
                    &receipt.target,
                    source,
                )
            })
        }
    }
}

fn absolute_env_path(name: &str) -> Option<PathBuf> {
    env::var_os(name)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
}

fn validate_file_name(file_name: &str) -> Result<(), InstallError> {
    let path = Path::new(file_name);
    let mut components = path.components();
    let one_normal_component =
        matches!(components.next(), Some(Component::Normal(_))) && components.next().is_none();
    let is_palette = path
        .extension()
        .is_some_and(|extension| extension == "palette");
    if file_name.starts_with('.') || !one_normal_component || !is_palette {
        return Err(InstallError::InvalidFileName(file_name.to_owned()));
    }
    Ok(())
}

fn unique_backup_name(file_name: &str) -> OsString {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let nonce = NEXT_TRANSACTION.fetch_add(1, Ordering::Relaxed);
    format!("{timestamp}-{nonce}-{file_name}.bak").into()
}

fn fingerprint(contents: &[u8]) -> (u64, u64) {
    // FNV-1a is used only as a stable accidental-change guard, not for security.
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in contents {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    (contents.len() as u64, hash)
}

#[derive(Debug)]
pub(crate) enum InstallError {
    MissingHome,
    InvalidFileName(String),
    InvalidReceiptPath {
        field: &'static str,
        path: PathBuf,
    },
    NoRollback,
    TargetModified(PathBuf),
    UnsupportedReceipt(u8),
    InvalidReceipt {
        path: PathBuf,
        source: serde_json::Error,
    },
    Io {
        operation: &'static str,
        path: PathBuf,
        source: io::Error,
    },
}

impl InstallError {
    fn io(operation: &'static str, path: impl Into<PathBuf>, source: io::Error) -> Self {
        Self::Io {
            operation,
            path: path.into(),
            source,
        }
    }
}

impl fmt::Display for InstallError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingHome => formatter.write_str("HOME is not available"),
            Self::InvalidFileName(name) => write!(formatter, "invalid palette file name {name:?}"),
            Self::InvalidReceiptPath { field, path } => write!(
                formatter,
                "rollback receipt {field} is outside its managed directory: {}",
                path.display()
            ),
            Self::NoRollback => formatter.write_str("there is no Ptyxis installation to roll back"),
            Self::TargetModified(path) => write!(
                formatter,
                "{} changed after installation; refusing to overwrite it",
                path.display()
            ),
            Self::UnsupportedReceipt(version) => {
                write!(formatter, "unsupported rollback receipt version {version}")
            }
            Self::InvalidReceipt { path, source } => {
                write!(
                    formatter,
                    "invalid rollback receipt {}: {source}",
                    path.display()
                )
            }
            Self::Io {
                operation,
                path,
                source,
            } => write!(formatter, "cannot {operation} {}: {source}", path.display()),
        }
    }
}

impl Error for InstallError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InvalidReceipt { source, .. } => Some(source),
            Self::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const IMPORTABLE_PALETTE: &str = "\
[Palette]\n\
Name=Imported\n\
\n\
[Dark]\n\
Foreground=#eeeeee\n\
Background=#111111\n\
Cursor=#eeeeee\n\
Color0=#111111\n\
Color1=#aa0000\n\
Color2=#00aa00\n\
Color3=#aa5500\n\
Color4=#0000aa\n\
Color5=#aa00aa\n\
Color6=#00aaaa\n\
Color7=#aaaaaa\n\
Color8=#555555\n\
Color9=#ff5555\n\
Color10=#55ff55\n\
Color11=#ffff55\n\
Color12=#5555ff\n\
Color13=#ff55ff\n\
Color14=#55ffff\n\
Color15=#ffffff\n";

    fn fixture() -> (PathBuf, PtyxisInstaller) {
        let root = env::temp_dir().join(format!(
            "termimochi-ptyxis-test-{}-{}",
            std::process::id(),
            NEXT_TRANSACTION.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        let installer = PtyxisInstaller::new(root.join("palettes"), root.join("state"));
        (root, installer)
    }

    #[test]
    fn palette_ids_cannot_escape_the_palette_directory() {
        assert_eq!(
            palette_file_name("Tokyo Night"),
            Some("Tokyo Night.palette".to_owned())
        );
        assert_eq!(
            palette_file_name("custom.palette"),
            Some("custom.palette".to_owned())
        );
        for id in [
            "",
            ".",
            "../outside",
            "folder/theme",
            "/tmp/theme",
            "bad\nid",
        ] {
            assert_eq!(palette_file_name(id), None);
        }
    }

    #[test]
    fn current_palette_is_imported_as_an_unsaved_snapshot() {
        let root = env::temp_dir().join(format!(
            "termimochi-ptyxis-import-test-{}-{}",
            std::process::id(),
            NEXT_TRANSACTION.fetch_add(1, Ordering::Relaxed)
        ));
        let first = root.join("first");
        let second = root.join("second");
        fs::create_dir_all(&first).unwrap();
        fs::create_dir_all(&second).unwrap();
        fs::write(second.join("active.palette"), "not selected").unwrap();
        fs::write(first.join("active.palette"), IMPORTABLE_PALETTE).unwrap();

        let text = read_palette_from_data_dirs("active.palette", &[first.clone(), second.clone()])
            .unwrap()
            .unwrap();
        let palette = PtyxisPalette::from_text(&text).unwrap();
        assert_eq!(palette.name(), "Imported");
        assert!(palette.source().is_none());

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn profile_ids_are_restricted_to_one_safe_settings_segment() {
        assert!(valid_profile_uuid("1d5138953bf016aed77141946a9863c5"));
        assert!(valid_profile_uuid("profile-1_test"));
        for id in ["", "../profile", "profile/child", "profile one"] {
            assert!(!valid_profile_uuid(id));
        }
    }

    #[test]
    fn new_install_can_be_removed_by_rollback() {
        let (root, installer) = fixture();
        let outcome = installer.install("mochi.palette", b"new palette").unwrap();
        let InstallOutcome::Installed(receipt) = outcome else {
            panic!("expected an installation");
        };
        assert!(receipt.backup().is_none());
        assert_eq!(fs::read(receipt.target()).unwrap(), b"new palette");

        assert_eq!(
            installer.rollback().unwrap(),
            RollbackOutcome::Removed {
                target: receipt.target().to_owned()
            }
        );
        assert!(!receipt.target().exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn replacement_is_backed_up_and_restored() {
        let (root, installer) = fixture();
        fs::create_dir_all(installer.palette_dir()).unwrap();
        let target = installer.palette_dir().join("mochi.palette");
        fs::write(&target, b"original").unwrap();

        let InstallOutcome::Installed(receipt) =
            installer.install("mochi.palette", b"replacement").unwrap()
        else {
            panic!("expected an installation");
        };
        assert_eq!(fs::read(&target).unwrap(), b"replacement");
        assert_eq!(fs::read(receipt.backup().unwrap()).unwrap(), b"original");

        assert!(matches!(
            installer.rollback().unwrap(),
            RollbackOutcome::Restored { .. }
        ));
        assert_eq!(fs::read(&target).unwrap(), b"original");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rollback_refuses_to_overwrite_external_changes() {
        let (root, installer) = fixture();
        let InstallOutcome::Installed(receipt) =
            installer.install("mochi.palette", b"installed").unwrap()
        else {
            panic!("expected an installation");
        };
        fs::write(receipt.target(), b"changed elsewhere").unwrap();

        assert!(matches!(
            installer.rollback(),
            Err(InstallError::TargetModified(_))
        ));
        assert_eq!(fs::read(receipt.target()).unwrap(), b"changed elsewhere");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn identical_install_is_a_no_op() {
        let (root, installer) = fixture();
        fs::create_dir_all(installer.palette_dir()).unwrap();
        let target = installer.palette_dir().join("mochi.palette");
        fs::write(&target, b"same").unwrap();

        assert_eq!(
            installer.install("mochi.palette", b"same").unwrap(),
            InstallOutcome::Unchanged(target)
        );
        assert!(installer.last_receipt().unwrap().is_none());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn traversal_and_wrong_extensions_are_rejected() {
        let (root, installer) = fixture();
        for name in [
            "../oops.palette",
            "/tmp/oops.palette",
            ".hidden.palette",
            "oops.txt",
        ] {
            assert!(matches!(
                installer.install(name, b"data"),
                Err(InstallError::InvalidFileName(_))
            ));
        }
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn receipt_paths_must_stay_in_managed_directories() {
        let (root, installer) = fixture();
        fs::create_dir_all(&installer.state_dir).unwrap();
        let outside = root.join("outside.palette");
        fs::write(&outside, b"do not touch").unwrap();
        let (installed_length, installed_hash) = fingerprint(b"do not touch");
        let receipt = InstallReceipt {
            version: 1,
            target: outside.clone(),
            backup: None,
            installed_length,
            installed_hash,
        };
        installer.write_receipt(&receipt).unwrap();

        assert!(matches!(
            installer.last_receipt(),
            Err(InstallError::InvalidReceiptPath {
                field: "target",
                ..
            })
        ));
        assert_eq!(fs::read(&outside).unwrap(), b"do not touch");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn receipt_backups_must_stay_in_the_backup_directory() {
        let (root, installer) = fixture();
        fs::create_dir_all(&installer.state_dir).unwrap();
        let target = installer.palette_dir.join("safe.palette");
        let receipt = InstallReceipt {
            version: 1,
            target,
            backup: Some(root.join("outside.palette.bak")),
            installed_length: 0,
            installed_hash: fingerprint(b"").1,
        };
        installer.write_receipt(&receipt).unwrap();

        assert!(matches!(
            installer.last_receipt(),
            Err(InstallError::InvalidReceiptPath {
                field: "backup",
                ..
            })
        ));
        fs::remove_dir_all(root).unwrap();
    }
}
