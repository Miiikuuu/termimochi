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

use crate::{
    layout::{LayoutSettings, PreviewCursorBlink, PreviewCursorShape},
    typography::{DEFAULT_CELL_SCALE, TypographySettings},
};

static NEXT_TRANSACTION: AtomicU64 = AtomicU64::new(0);
const CURRENT_RECEIPT_VERSION: u8 = 2;

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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    backup_fingerprint: Option<ContentFingerprint>,
    installed_length: u64,
    installed_hash: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct ContentFingerprint {
    length: u64,
    hash: u64,
}

impl ContentFingerprint {
    fn for_contents(contents: &[u8]) -> Self {
        let (length, hash) = fingerprint(contents);
        Self { length, hash }
    }

    fn matches(self, contents: &[u8]) -> bool {
        self == Self::for_contents(contents)
    }
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

/// Read-only configuration snapshot. A desktop launch cannot identify the
/// active tab in another application; `source_label` distinguishes a launching
/// profile from Ptyxis' configured default and notices describe approximations.
#[derive(Clone, Debug)]
pub(crate) struct CurrentTerminalAppearance {
    pub(crate) palette: Option<PtyxisPalette>,
    pub(crate) preferred_variant: Option<Variant>,
    pub(crate) typography: TypographySettings,
    pub(crate) layout: LayoutSettings,
    pub(crate) bold_is_bright: bool,
    pub(crate) cjk_ambiguous_width: i32,
    pub(crate) source_label: String,
    pub(crate) notices: Vec<String>,
}

/// Import palette, font, spacing, cursor, and configured window grid together.
/// All reads are limited to appearance settings: no shell startup files,
/// command history, session contents, or arbitrary commands are evaluated.
pub(crate) fn import_current_appearance() -> CurrentTerminalAppearance {
    let desktop = find_settings("org.gnome.desktop.interface", None);
    let inherited = env::var("PTYXIS_PROFILE").ok();
    let terminal_program = env::var("TERM_PROGRAM").ok();
    let foreign_terminal = has_foreign_terminal_hint(
        inherited.as_deref(),
        terminal_program.as_deref(),
        env::var_os("GNOME_TERMINAL_SCREEN").is_some(),
    );
    if foreign_terminal {
        return fallback_appearance(
            desktop.as_ref(),
            "The launching terminal does not provide a supported appearance profile. Using the desktop font and preview defaults.",
        );
    }
    let Some(global) = find_settings("org.gnome.Ptyxis", None) else {
        return fallback_appearance(
            desktop.as_ref(),
            "Ptyxis appearance settings are unavailable. Using the desktop font and preview defaults.",
        );
    };
    let mut notices = Vec::new();
    let profile_uuid = current_profile_uuid_from(&global, inherited.as_deref());
    let launching_profile = profile_uuid
        .as_deref()
        .is_some_and(|uuid| inherited.as_deref() == Some(uuid));
    if inherited.is_some() && !launching_profile {
        notices.push(
            "The launching Ptyxis profile is no longer available; using a configured profile."
                .to_owned(),
        );
    }
    let profile = profile_uuid.as_ref().and_then(|uuid| {
        let path = format!("/org/gnome/Ptyxis/Profiles/{uuid}/");
        find_settings("org.gnome.Ptyxis.Profile", Some(&path))
    });
    let palette = match profile.as_ref() {
        Some(profile) => match read_profile_palette(profile) {
            Ok(palette) => Some(palette),
            Err(error) => {
                notices.push(format!("Could not read the Ptyxis palette: {error}"));
                None
            }
        },
        None => {
            notices.push(
                "Ptyxis has no readable configured profile; using preview colors and cell spacing."
                    .to_owned(),
            );
            None
        }
    };
    let default_profile = setting_string(&global, "default-profile-uuid");
    let source_label = if launching_profile {
        "Ptyxis · Launching profile"
    } else if profile_uuid.is_none() {
        "Ptyxis · No profile"
    } else if profile_uuid != default_profile {
        "Ptyxis · Configured profile"
    } else {
        "Ptyxis · Default profile"
    };
    if !launching_profile && profile.is_some() {
        notices.push("Imported Ptyxis' configured profile. Desktop launches cannot identify another window's active tab or temporary zoom.".to_owned());
    }
    let style = setting_string(&global, "interface-style");
    let desktop_style = desktop
        .as_ref()
        .and_then(|settings| setting_string(settings, "color-scheme"));
    let preferred_variant = Some(resolve_variant(style.as_deref(), desktop_style.as_deref()));
    let typography = read_typography(Some(&global), profile.as_ref(), desktop.as_ref());
    let layout = read_layout(&global, desktop.as_ref(), &mut notices);
    let bold_is_bright = profile
        .as_ref()
        .and_then(|settings| setting_boolean(settings, "bold-is-bright"))
        .unwrap_or(false);
    let cjk_ambiguous_width = match profile
        .as_ref()
        .and_then(|settings| setting_string(settings, "cjk-ambiguous-width"))
        .as_deref()
    {
        Some("wide") => 2,
        _ => 1,
    };
    if profile
        .as_ref()
        .and_then(|settings| setting_double(settings, "opacity"))
        .is_some_and(|opacity| opacity < 1.0)
    {
        notices.push(
            "The profile uses transparency; the preview shows its opaque base colors.".to_owned(),
        );
    }
    CurrentTerminalAppearance {
        palette,
        preferred_variant,
        typography,
        layout,
        bold_is_bright,
        cjk_ambiguous_width,
        source_label: source_label.to_owned(),
        notices,
    }
}

fn fallback_appearance(desktop: Option<&gio::Settings>, notice: &str) -> CurrentTerminalAppearance {
    CurrentTerminalAppearance {
        palette: None,
        preferred_variant: None,
        typography: read_typography(None, None, desktop),
        layout: LayoutSettings::default(),
        bold_is_bright: false,
        cjk_ambiguous_width: 1,
        source_label: "Preview defaults".to_owned(),
        notices: vec![notice.to_owned()],
    }
}

fn has_foreign_terminal_hint(
    profile: Option<&str>,
    terminal_program: Option<&str>,
    gnome_terminal: bool,
) -> bool {
    if profile.is_some_and(valid_profile_uuid) {
        return false;
    }
    gnome_terminal
        || terminal_program.is_some_and(|program| {
            !program.trim().is_empty() && !program.eq_ignore_ascii_case("ptyxis")
        })
}

fn read_profile_palette(profile: &gio::Settings) -> Result<PtyxisPalette, PaletteImportError> {
    if !profile
        .settings_schema()
        .is_some_and(|schema| schema.has_key("palette"))
    {
        return Err(PaletteImportError::MissingSetting {
            schema: "org.gnome.Ptyxis.Profile",
            key: "palette",
        });
    }
    let palette_id = setting_string(profile, "palette").unwrap_or_default();
    let file_name = palette_file_name(&palette_id)
        .ok_or_else(|| PaletteImportError::InvalidPaletteId(palette_id.clone()))?;

    let text = match read_palette_from_data_dirs(&file_name, &ptyxis_palette_directories())? {
        Some(text) => text,
        None => extract_builtin_palette(&file_name, &palette_id)?,
    };
    PtyxisPalette::from_text(&text).map_err(PaletteImportError::InvalidPalette)
}

fn read_typography(
    global: Option<&gio::Settings>,
    profile: Option<&gio::Settings>,
    desktop: Option<&gio::Settings>,
) -> TypographySettings {
    let use_system_font = global.and_then(|settings| setting_boolean(settings, "use-system-font"));
    let custom_font_name = global.and_then(|settings| setting_string(settings, "font-name"));
    let system_font_name =
        desktop.and_then(|settings| setting_string(settings, "monospace-font-name"));
    let font_name = select_font_name(
        use_system_font,
        custom_font_name.as_deref(),
        system_font_name.as_deref(),
    )
    .unwrap_or_default();

    let line_height = profile
        .and_then(|settings| setting_double(settings, "cell-height-scale"))
        .unwrap_or(DEFAULT_CELL_SCALE);
    let cell_width = profile
        .and_then(|settings| setting_double(settings, "cell-width-scale"))
        .unwrap_or(DEFAULT_CELL_SCALE);

    TypographySettings::from_font_name(font_name, line_height, cell_width)
}

fn resolve_variant(style: Option<&str>, desktop_style: Option<&str>) -> Variant {
    match style {
        Some("light") => Variant::Light,
        Some("dark") => Variant::Dark,
        _ if desktop_style == Some("prefer-dark") => Variant::Dark,
        _ => Variant::Light,
    }
}

fn read_layout(
    global: &gio::Settings,
    desktop: Option<&gio::Settings>,
    notices: &mut Vec<String>,
) -> LayoutSettings {
    let defaults = LayoutSettings::default();
    let configured = (
        setting_uint(global, "default-columns").unwrap_or(80),
        setting_uint(global, "default-rows").unwrap_or(24),
    );
    let restored = setting_value(global, "window-size").and_then(|value| value.get::<(u32, u32)>());
    let (columns, rows) = select_window_grid(
        setting_boolean(global, "restore-window-size").unwrap_or(false),
        restored,
        configured,
    );
    let disable_padding = setting_boolean(global, "disable-padding").unwrap_or(false);
    let shape = setting_string(global, "cursor-shape");
    let blink = setting_string(global, "cursor-blink-mode");
    let scrollbar = setting_string(global, "scrollbar-policy");
    let overlay = desktop.and_then(|settings| setting_boolean(settings, "overlay-scrolling"));
    let layout = LayoutSettings::new(
        if disable_padding { 0 } else { 6 },
        columns as usize,
        rows as usize,
        match shape.as_deref() {
            Some("ibeam") => PreviewCursorShape::IBeam,
            Some("underline") => PreviewCursorShape::Underline,
            _ => PreviewCursorShape::Block,
        },
        match blink.as_deref() {
            Some("on") => PreviewCursorBlink::On,
            Some("off") => PreviewCursorBlink::Off,
            _ => PreviewCursorBlink::System,
        },
        // Ptyxis reveals its tab bar only with more than one tab. This is a
        // single-tab preview, not a reconstruction of another window's tabs.
        false,
        match scrollbar.as_deref() {
            Some("always") => true,
            Some("never") => false,
            _ => !overlay.unwrap_or(true),
        },
        defaults.window_spacing,
    );
    if layout.columns != columns as usize || layout.rows != rows as usize {
        notices.push(format!(
            "The configured {columns} × {rows} terminal grid was limited to {} × {} for the preview.",
            layout.columns, layout.rows
        ));
    }
    if !disable_padding {
        notices.push("Ptyxis' asymmetric terminal padding is represented by a 6 px inset. Window spacing and a single-tab view remain preview settings.".to_owned());
    }
    layout
}

fn select_window_grid(
    restore: bool,
    restored: Option<(u32, u32)>,
    configured: (u32, u32),
) -> (u32, u32) {
    if restore
        && let Some((columns, rows)) = restored
        && columns > 0
        && rows > 0
    {
        (columns, rows)
    } else {
        configured
    }
}

pub(crate) fn find_settings(schema_id: &str, path: Option<&str>) -> Option<gio::Settings> {
    let source = gio::SettingsSchemaSource::default()?;
    let schema = source.lookup(schema_id, true)?;
    Some(gio::Settings::new_full(
        &schema,
        gio::SettingsBackend::NONE,
        path,
    ))
}

fn setting_boolean(settings: &gio::Settings, key: &str) -> Option<bool> {
    setting_value(settings, key).and_then(|value| value.get())
}

fn setting_double(settings: &gio::Settings, key: &str) -> Option<f64> {
    setting_value(settings, key).and_then(|value| value.get())
}

fn setting_string(settings: &gio::Settings, key: &str) -> Option<String> {
    setting_value(settings, key)
        .and_then(|value| value.get::<String>())
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn setting_uint(settings: &gio::Settings, key: &str) -> Option<u32> {
    setting_value(settings, key).and_then(|value| value.get())
}

fn setting_value(settings: &gio::Settings, key: &str) -> Option<glib::Variant> {
    settings
        .settings_schema()
        .filter(|schema| schema.has_key(key))
        .map(|_| settings.value(key))
}

fn select_font_name<'a>(
    use_system_font: Option<bool>,
    custom_font_name: Option<&'a str>,
    system_font_name: Option<&'a str>,
) -> Option<&'a str> {
    let selected = if use_system_font.unwrap_or(true) {
        system_font_name
    } else {
        custom_font_name
    };
    selected.map(str::trim).filter(|value| !value.is_empty())
}

pub(crate) fn current_profile_uuid_from(
    settings: &gio::Settings,
    inherited: Option<&str>,
) -> Option<String> {
    let default = setting_string(settings, "default-profile-uuid");
    let profiles = setting_value(settings, "profile-uuids")
        .and_then(|value| value.get::<Vec<String>>())
        .unwrap_or_default();
    select_profile_uuid(
        inherited,
        default.as_deref(),
        profiles.iter().map(|value| value.as_str()),
    )
}

fn select_profile_uuid<'a>(
    inherited: Option<&'a str>,
    default: Option<&'a str>,
    profiles: impl IntoIterator<Item = &'a str>,
) -> Option<String> {
    let profiles: Vec<_> = profiles
        .into_iter()
        .filter(|value| valid_profile_uuid(value))
        .collect();
    let available = |value: &&str| valid_profile_uuid(value) && profiles.contains(value);
    inherited
        .filter(available)
        .or_else(|| default.filter(available))
        .or_else(|| profiles.first().copied())
        .map(ToOwned::to_owned)
}

pub(crate) fn valid_profile_uuid(value: &str) -> bool {
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
    MissingSetting {
        schema: &'static str,
        key: &'static str,
    },
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
            Self::MissingSetting { schema, key } => {
                write!(formatter, "{schema} does not provide setting {key}")
            }
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
        if !matches!(receipt.version, 1 | CURRENT_RECEIPT_VERSION) {
            return Err(InstallError::UnsupportedReceipt(receipt.version));
        }
        self.validate_receipt_paths(&receipt)?;
        self.validate_receipt_backup_metadata(&receipt)?;
        Ok(Some(receipt))
    }

    fn validate_receipt_backup_metadata(
        &self,
        receipt: &InstallReceipt,
    ) -> Result<(), InstallError> {
        match (
            receipt.version,
            receipt.backup.as_ref(),
            receipt.backup_fingerprint,
        ) {
            // Version 1 did not fingerprint backups. A receipt for a newly
            // created file remains safe because rollback verifies the installed
            // target before removing it; an old backup must be restored by hand.
            (1, None, None) => Ok(()),
            (1, Some(backup), _) => Err(InstallError::UnverifiedLegacyBackup(backup.clone())),
            (CURRENT_RECEIPT_VERSION, None, None) | (CURRENT_RECEIPT_VERSION, Some(_), Some(_)) => {
                Ok(())
            }
            _ => Err(InstallError::InvalidBackupMetadata(self.receipt_path())),
        }
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
        self.install_inner(file_name, contents, None)
    }

    pub(crate) fn install_reviewed(
        &self,
        file_name: &str,
        contents: &[u8],
        expected: &Option<Vec<u8>>,
    ) -> Result<InstallOutcome, InstallError> {
        self.install_inner(file_name, contents, Some(expected))
    }

    fn install_inner(
        &self,
        file_name: &str,
        contents: &[u8],
        expected: Option<&Option<Vec<u8>>>,
    ) -> Result<InstallOutcome, InstallError> {
        validate_file_name(file_name)?;
        let target = self.palette_dir.join(file_name);
        let check_review = || -> Result<(), InstallError> {
            if let Some(expected) = expected {
                let actual = crate::typography_preset::read_private_with_limit(&target, 256 * 1024)
                    .map_err(|source| {
                        InstallError::io(
                            "verify reviewed palette",
                            &target,
                            io::Error::other(source),
                        )
                    })?;
                if &actual != expected {
                    return Err(InstallError::TargetModified(target.clone()));
                }
            }
            Ok(())
        };
        check_review()?;
        fs::create_dir_all(&self.palette_dir).map_err(|source| {
            InstallError::io("create Ptyxis palette directory", &self.palette_dir, source)
        })?;
        fs::create_dir_all(&self.state_dir).map_err(|source| {
            InstallError::io("create TermiMochi state directory", &self.state_dir, source)
        })?;

        let previous = match fs::read(&target) {
            Ok(bytes) if bytes == contents => return Ok(InstallOutcome::Unchanged(target)),
            Ok(bytes) => Some(bytes),
            Err(error) if error.kind() == io::ErrorKind::NotFound => None,
            Err(source) => return Err(InstallError::io("read existing palette", target, source)),
        };

        let (backup, backup_fingerprint) = if let Some(previous) = previous.as_deref() {
            let backup_dir = self.state_dir.join("ptyxis-backups");
            fs::create_dir_all(&backup_dir).map_err(|source| {
                InstallError::io("create backup directory", &backup_dir, source)
            })?;
            let backup_path = backup_dir.join(unique_backup_name(file_name));
            write_atomically(&backup_path, previous)
                .map_err(|source| InstallError::io("write palette backup", &backup_path, source))?;
            (
                Some(backup_path),
                Some(ContentFingerprint::for_contents(previous)),
            )
        } else {
            (None, None)
        };

        let (installed_length, installed_hash) = fingerprint(contents);
        let receipt = InstallReceipt {
            version: CURRENT_RECEIPT_VERSION,
            target: target.clone(),
            backup,
            backup_fingerprint,
            installed_length,
            installed_hash,
        };

        check_review()?;
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

        let outcome = if let Some((backup, original)) = self.verified_backup(&receipt)? {
            write_atomically(&receipt.target, &original).map_err(|source| {
                InstallError::io("restore palette backup", &receipt.target, source)
            })?;
            RollbackOutcome::Restored {
                target: receipt.target.clone(),
                backup: backup.to_owned(),
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
        if let Some((_backup, original)) = self.verified_backup(receipt)? {
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

    fn verified_backup<'a>(
        &self,
        receipt: &'a InstallReceipt,
    ) -> Result<Option<(&'a Path, Vec<u8>)>, InstallError> {
        let Some(backup) = receipt.backup.as_deref() else {
            return Ok(None);
        };
        let expected = receipt
            .backup_fingerprint
            .ok_or_else(|| InstallError::UnverifiedLegacyBackup(backup.to_owned()))?;
        let contents = fs::read(backup)
            .map_err(|source| InstallError::io("read palette backup", backup, source))?;
        if !expected.matches(&contents) {
            return Err(InstallError::BackupModified(backup.to_owned()));
        }
        Ok(Some((backup, contents)))
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
    BackupModified(PathBuf),
    UnverifiedLegacyBackup(PathBuf),
    InvalidBackupMetadata(PathBuf),
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
            Self::BackupModified(path) => write!(
                formatter,
                "backup {} no longer matches its rollback receipt; refusing to restore it",
                path.display()
            ),
            Self::UnverifiedLegacyBackup(path) => write!(
                formatter,
                "backup {} predates integrity records; refusing automatic restore",
                path.display()
            ),
            Self::InvalidBackupMetadata(path) => write!(
                formatter,
                "rollback receipt {} has inconsistent backup integrity metadata",
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
    fn profile_selection_prefers_the_launching_terminal_then_configured_profiles() {
        let profiles = ["first", "second"];
        assert_eq!(
            select_profile_uuid(Some("second"), Some("first"), profiles),
            Some("second".to_owned())
        );
        assert_eq!(
            select_profile_uuid(None, Some("second"), profiles),
            Some("second".to_owned())
        );
        assert_eq!(
            select_profile_uuid(None, None, profiles),
            Some("first".to_owned())
        );
        assert_eq!(
            select_profile_uuid(Some("../bad"), Some("also bad"), profiles),
            Some("first".to_owned())
        );
        assert_eq!(select_profile_uuid(None, None, ["../bad"]), None);
    }

    #[test]
    fn deleted_profiles_do_not_silently_read_a_new_profiles_schema_defaults() {
        assert_eq!(
            select_profile_uuid(Some("deleted"), Some("default"), ["default", "other"]),
            Some("default".to_owned())
        );
        assert_eq!(
            select_profile_uuid(None, Some("deleted"), ["first", "other"]),
            Some("first".to_owned())
        );
        assert_eq!(
            select_profile_uuid(Some("deleted"), Some("deleted"), []),
            None
        );
    }

    #[test]
    fn unsupported_terminal_hints_do_not_import_an_unrelated_ptyxis_profile() {
        assert!(has_foreign_terminal_hint(None, Some("WezTerm"), false));
        assert!(has_foreign_terminal_hint(None, None, true));
        assert!(!has_foreign_terminal_hint(None, Some("Ptyxis"), false));
        assert!(!has_foreign_terminal_hint(None, None, false));
        assert!(!has_foreign_terminal_hint(
            Some("valid-profile"),
            Some("tmux"),
            false
        ));
        assert!(has_foreign_terminal_hint(
            Some("../bad"),
            Some("tmux"),
            false
        ));
    }

    #[test]
    fn restored_terminal_grid_is_used_only_when_valid_and_enabled() {
        assert_eq!(
            select_window_grid(true, Some((109, 23)), (80, 24)),
            (109, 23)
        );
        assert_eq!(
            select_window_grid(false, Some((109, 23)), (80, 24)),
            (80, 24)
        );
        assert_eq!(select_window_grid(true, Some((0, 23)), (80, 24)), (80, 24));
        assert_eq!(select_window_grid(true, Some((109, 0)), (80, 24)), (80, 24));
        assert_eq!(select_window_grid(true, None, (80, 24)), (80, 24));
    }

    #[test]
    fn terminal_system_variant_uses_desktop_preference_not_workbench_chrome() {
        assert_eq!(
            resolve_variant(Some("system"), Some("prefer-dark")),
            Variant::Dark
        );
        assert_eq!(
            resolve_variant(Some("system"), Some("default")),
            Variant::Light
        );
        assert_eq!(
            resolve_variant(Some("dark"), Some("prefer-light")),
            Variant::Dark
        );
        assert_eq!(
            resolve_variant(Some("light"), Some("prefer-dark")),
            Variant::Light
        );
        assert_eq!(resolve_variant(None, None), Variant::Light);
    }

    #[test]
    fn typography_font_source_obeys_ptyxis_system_font_precedence() {
        assert_eq!(
            select_font_name(Some(true), Some("Custom Mono 12"), Some("System Mono 11")),
            Some("System Mono 11")
        );
        assert_eq!(
            select_font_name(Some(false), Some("Custom Mono 12"), Some("System Mono 11")),
            Some("Custom Mono 12")
        );
    }

    #[test]
    fn typography_font_source_defaults_to_system_without_global_settings() {
        assert_eq!(
            select_font_name(None, Some("Custom Mono 12"), Some("System Mono 11")),
            Some("System Mono 11")
        );
        assert_eq!(
            select_font_name(None, None, Some("  System Mono 11  ")),
            Some("System Mono 11")
        );
    }

    #[test]
    fn typography_font_source_does_not_revive_an_inactive_or_blank_setting() {
        assert_eq!(
            select_font_name(Some(true), Some("Custom Mono 12"), None),
            None
        );
        assert_eq!(
            select_font_name(Some(false), None, Some("System Mono 11")),
            None
        );
        assert_eq!(select_font_name(Some(true), None, Some("  ")), None);
        assert_eq!(select_font_name(Some(false), Some("\t"), None), None);
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
        assert_eq!(receipt.version, CURRENT_RECEIPT_VERSION);
        assert_eq!(
            receipt.backup_fingerprint,
            Some(ContentFingerprint::for_contents(b"original"))
        );
        assert_eq!(installer.last_receipt().unwrap(), Some(receipt.clone()));

        assert!(matches!(
            installer.rollback().unwrap(),
            RollbackOutcome::Restored { .. }
        ));
        assert_eq!(fs::read(&target).unwrap(), b"original");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rollback_refuses_a_same_length_tampered_backup() {
        let (root, installer) = fixture();
        fs::create_dir_all(installer.palette_dir()).unwrap();
        let target = installer.palette_dir().join("mochi.palette");
        fs::write(&target, b"original").unwrap();
        let InstallOutcome::Installed(receipt) =
            installer.install("mochi.palette", b"replacement").unwrap()
        else {
            panic!("expected an installation");
        };
        let backup = receipt.backup().unwrap();
        assert_eq!(b"original".len(), b"tampered".len());
        fs::write(backup, b"tampered").unwrap();

        assert!(matches!(
            installer.rollback(),
            Err(InstallError::BackupModified(path)) if path == backup
        ));
        assert_eq!(fs::read(&target).unwrap(), b"replacement");
        assert!(installer.receipt_path().exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rollback_refuses_a_truncated_backup() {
        let (root, installer) = fixture();
        fs::create_dir_all(installer.palette_dir()).unwrap();
        let target = installer.palette_dir().join("mochi.palette");
        fs::write(&target, b"original").unwrap();
        let InstallOutcome::Installed(receipt) =
            installer.install("mochi.palette", b"replacement").unwrap()
        else {
            panic!("expected an installation");
        };
        let backup = receipt.backup().unwrap();
        fs::write(backup, b"cut").unwrap();

        assert!(matches!(
            installer.rollback(),
            Err(InstallError::BackupModified(path)) if path == backup
        ));
        assert_eq!(fs::read(&target).unwrap(), b"replacement");
        assert!(installer.receipt_path().exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rollback_refuses_a_missing_backup() {
        let (root, installer) = fixture();
        fs::create_dir_all(installer.palette_dir()).unwrap();
        let target = installer.palette_dir().join("mochi.palette");
        fs::write(&target, b"original").unwrap();
        let InstallOutcome::Installed(receipt) =
            installer.install("mochi.palette", b"replacement").unwrap()
        else {
            panic!("expected an installation");
        };
        fs::remove_file(receipt.backup().unwrap()).unwrap();

        match installer.rollback() {
            Err(InstallError::Io {
                operation, source, ..
            }) => {
                assert_eq!(operation, "read palette backup");
                assert_eq!(source.kind(), io::ErrorKind::NotFound);
            }
            other => panic!("expected a missing-backup error, got {other:?}"),
        }
        assert_eq!(fs::read(&target).unwrap(), b"replacement");
        assert!(installer.receipt_path().exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn legacy_receipts_only_rollback_when_no_backup_is_needed() {
        let (root, installer) = fixture();
        fs::create_dir_all(installer.palette_dir()).unwrap();
        fs::create_dir_all(&installer.state_dir).unwrap();
        let target = installer.palette_dir().join("new.palette");
        fs::write(&target, b"installed").unwrap();
        let (installed_length, installed_hash) = fingerprint(b"installed");
        installer
            .write_receipt(&InstallReceipt {
                version: 1,
                target: target.clone(),
                backup: None,
                backup_fingerprint: None,
                installed_length,
                installed_hash,
            })
            .unwrap();

        assert_eq!(
            installer.rollback().unwrap(),
            RollbackOutcome::Removed {
                target: target.clone()
            }
        );
        assert!(!target.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn legacy_receipts_never_restore_an_unverified_backup() {
        let (root, installer) = fixture();
        fs::create_dir_all(installer.palette_dir()).unwrap();
        let backup_dir = installer.state_dir.join("ptyxis-backups");
        fs::create_dir_all(&backup_dir).unwrap();
        let target = installer.palette_dir().join("mochi.palette");
        let backup = backup_dir.join("legacy-mochi.palette.bak");
        fs::write(&target, b"installed").unwrap();
        fs::write(&backup, b"legacy original").unwrap();
        let (installed_length, installed_hash) = fingerprint(b"installed");
        installer
            .write_receipt(&InstallReceipt {
                version: 1,
                target: target.clone(),
                backup: Some(backup.clone()),
                backup_fingerprint: None,
                installed_length,
                installed_hash,
            })
            .unwrap();

        assert!(matches!(
            installer.last_receipt(),
            Err(InstallError::UnverifiedLegacyBackup(path)) if path == backup
        ));
        assert_eq!(fs::read(&target).unwrap(), b"installed");
        assert_eq!(fs::read(&backup).unwrap(), b"legacy original");
        assert!(installer.receipt_path().exists());
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
            backup_fingerprint: None,
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
            backup_fingerprint: None,
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

    #[test]
    fn current_receipts_require_backup_paths_and_fingerprints_to_agree() {
        let (root, installer) = fixture();
        fs::create_dir_all(&installer.state_dir).unwrap();
        let target = installer.palette_dir.join("safe.palette");
        let backup = installer
            .state_dir
            .join("ptyxis-backups")
            .join("safe.palette.bak");
        let expected = ContentFingerprint::for_contents(b"original");

        for (backup, backup_fingerprint) in [(Some(backup), None), (None, Some(expected))] {
            installer
                .write_receipt(&InstallReceipt {
                    version: CURRENT_RECEIPT_VERSION,
                    target: target.clone(),
                    backup,
                    backup_fingerprint,
                    installed_length: 0,
                    installed_hash: fingerprint(b"").1,
                })
                .unwrap();
            assert!(matches!(
                installer.last_receipt(),
                Err(InstallError::InvalidBackupMetadata(path))
                    if path == installer.receipt_path()
            ));
        }
        fs::remove_dir_all(root).unwrap();
    }
}
