//! A target-specific, versioned Kitty + Bash session. Portable design data never
//! supplies executable paths or publication permission. No daily rc/config is
//! read by Kitty/Bash; this is a controlled shell, NOT a filesystem sandbox.
use crate::{greeting_image::pixel_export, typography_preset, workspace::Workspace};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs,
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
    process::Command,
};

const LIMIT: u64 = 40 * 1024 * 1024;
const MANIFEST: &str = "manifest.json";
pub(crate) mod launcher;

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Ownership {
    pub palette: bool,
    pub typography: bool,
    pub layout: bool,
    pub prompt: bool,
    pub greeting: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Executable {
    path: PathBuf,
    identity: [u64; 5],
}
impl Executable {
    fn capture(path: PathBuf) -> Result<Self, String> {
        let m = fs::metadata(&path)
            .map_err(|e| format!("Required executable {}: {e}", path.display()))?;
        if !path.is_absolute() || !m.is_file() || m.mode() & 0o111 == 0 {
            return Err("Required program is not an executable regular file.".into());
        }
        Ok(Self {
            path,
            identity: [
                m.dev(),
                m.ino(),
                m.len(),
                m.mtime() as u64,
                m.mtime_nsec() as u64,
            ],
        })
    }
    fn find(name: &str) -> Result<Self, String> {
        Self::capture(
            gtk::glib::find_program_in_path(name).ok_or_else(|| {
                format!("Install {name} before using a controlled Kitty session.")
            })?,
        )
    }
    fn check(&self) -> Result<(), String> {
        if Self::capture(self.path.clone())?.identity != self.identity {
            return Err(format!(
                "{} changed or moved; prepare and review the session again.",
                self.path.display()
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Dependencies {
    kitty: Executable,
    bash: Executable,
    helper: Option<Executable>,
    starship: Option<Executable>,
    fastfetch: Option<Executable>,
}
impl Dependencies {
    fn check(&self) -> Result<(), String> {
        for executable in [
            Some(&self.kitty),
            Some(&self.bash),
            self.helper.as_ref(),
            self.starship.as_ref(),
            self.fastfetch.as_ref(),
        ]
        .into_iter()
        .flatten()
        {
            executable.check()?;
        }
        Ok(())
    }
}

/// A frozen, local plan. It contains only explicitly owned Workspace components.
/// `revision` is checked by the UI session before trial approval/publication.
pub(crate) struct Plan {
    root: PathBuf,
    pub deployment_id: String,
    pub name: String,
    pub revision: u64,
    pub notes: Vec<String>,
    pub animated: bool,
    pub pixel: bool,
    files: BTreeMap<String, Vec<u8>>,
    dependencies: Dependencies,
    expected: Option<Vec<u8>>,
    previous_manifest: Option<(PathBuf, Vec<u8>)>,
    previous_summary: Option<(String, String)>,
    logo: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Deployment {
    schema: u8,
    pub id: String,
    pub name: String,
    pub version: String,
    pub previous: Option<String>,
    pub revision: u64,
    pub notes: Vec<String>,
    directory: PathBuf,
    dependencies: Dependencies,
    hashes: BTreeMap<String, u64>,
}

pub(crate) struct PreparedSession {
    owner: std::sync::Arc<tempfile::TempDir>,
    pub deployment: Deployment,
}

fn hash(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf29ce484222325u64, |h, b| {
        (h ^ u64::from(*b)).wrapping_mul(0x100000001b3)
    })
}
fn identifier(value: &str) -> Result<(), String> {
    if value.is_empty()
        || value.len() > 96
        || !value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    {
        return Err("Invalid managed session identifier.".into());
    }
    Ok(())
}
fn read(path: &Path) -> Result<Option<Vec<u8>>, String> {
    typography_preset::read_private_with_limit(path, LIMIT)
}
fn write_new(path: &Path, bytes: &[u8]) -> Result<(), String> {
    typography_preset::write_checked_with_limit(path, bytes, &None, LIMIT)
}
fn quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}
fn path_quote(path: &Path) -> Result<String, String> {
    let value = path
        .to_str()
        .filter(|s| !s.chars().any(char::is_control))
        .ok_or("Unsupported path encoding in session launcher.")?;
    Ok(quote(value))
}
fn safe_root(root: &Path) -> Result<(), String> {
    if !root.is_absolute() || root == Path::new("/") {
        return Err("Managed session root must be an explicit absolute directory.".into());
    }
    for ancestor in root.ancestors() {
        if let Ok(m) = fs::symlink_metadata(ancestor)
            && (!m.is_dir() || m.file_type().is_symlink())
        {
            return Err(format!(
                "Managed session directory is aliased: {}",
                ancestor.display()
            ));
        }
    }
    Ok(())
}

pub(crate) fn appearance_configuration(
    workspace: &Workspace,
    owned: Ownership,
    native_kitty: Option<&str>,
) -> Result<(String, Vec<String>), String> {
    let mut notes = Vec::new();
    let mut config = String::new();
    if let Some(source) = native_kitty {
        let doc = crate::kitty_document::KittyDocument::parse(source)?;
        notes.extend(doc.notices.clone());
        for line in doc.safe_configuration().lines() {
            let key = line.split_whitespace().next().unwrap_or("");
            let authorized = match crate::kitty_document::category(key) {
                Some(0) => owned.palette,
                Some(1) => owned.typography,
                Some(2) => owned.layout,
                _ => false,
            };
            if authorized {
                config.push_str(line);
                config.push('\n');
            }
        }
    } else {
        if owned.palette {
            config.push_str(
                &termimochi_core::export_palette(
                    &workspace.palette()?,
                    workspace.variant(),
                    termimochi_core::ExportFormat::Kitty,
                )
                .map_err(|e| e.to_string())?
                .contents,
            );
            notes.push("Palette: terminal foreground/background, cursor and 16 ANSI colors. Ptyxis-only titlebar/status colors are not Kitty settings.".into());
        }
        if owned.typography {
            let f = &workspace.typography;
            f.validate()?;
            if f.family.contains(['$', '`', '\\']) {
                return Err(
                    "This font name cannot safely be mapped into Kitty configuration.".into(),
                );
            }
            config.push_str(&format!("font_family {}\nfont_size {}\nmodify_font cell_height {}%\nmodify_font cell_width {}%\n", f.family, f.size, f.line_height * 100.0, f.cell_width * 100.0));
            if f.weight != crate::typography::PreviewFontWeight::Regular {
                notes.push(format!("Requested {} default weight: Kitty font matching is family-based here; select the exact weighted family to make this equivalent. Weight is not silently claimed as applied.", f.weight.label()));
            }
        }
        if owned.layout {
            use crate::layout::{PreviewCursorBlink as B, PreviewCursorShape as S};
            let l = workspace.layout;
            l.validate()?;
            let shape = match l.cursor_shape {
                S::Block => "block",
                S::IBeam => "beam",
                S::Underline => "underline",
            };
            let blink = match l.cursor_blink {
                B::Off => "0",
                B::On => "0.5",
                B::System => "-1",
            };
            config.push_str(&format!("window_padding_width {}\nwindow_margin_width {}\ninitial_window_width {}c\ninitial_window_height {}c\nremember_window_size no\ncursor_shape {shape}\ncursor_blink_interval {blink}\ntab_bar_style {}\n", l.content_padding, l.window_spacing, l.columns, l.rows, if l.tab_bar { "fade" } else { "hidden" }));
            notes.push("Layout: grid and cursor mapped; spacing is Kitty points (not VTE pixels). Scrollbar is unsupported. Tab bar is visible only when multiple tabs exist.".into());
        }
    }
    Ok((config, notes))
}

impl Plan {
    #[allow(clippy::too_many_arguments)]
    pub fn prepare(
        root: &Path,
        deployment_id: &str,
        name: &str,
        revision: u64,
        workspace: &Workspace,
        owned: Ownership,
        native_kitty: Option<&str>,
        cells: [u32; 2],
    ) -> Result<Self, String> {
        Self::prepare_inner(
            root,
            deployment_id,
            name,
            revision,
            workspace,
            owned,
            native_kitty,
            cells,
            None,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn prepare_inner(
        root: &Path,
        deployment_id: &str,
        name: &str,
        revision: u64,
        workspace: &Workspace,
        owned: Ownership,
        native_kitty: Option<&str>,
        cells: [u32; 2],
        dependencies_for_test: Option<Dependencies>,
    ) -> Result<Self, String> {
        identifier(deployment_id)?;
        safe_root(root)?;
        if name.is_empty() || name.len() > 256 || name.chars().any(char::is_control) {
            return Err("Invalid session name.".into());
        }
        let mut notes = vec!["Controlled Kitty + Bash: original aliases, Conda/ROS initialization and rc/profile files are not loaded. Typed commands execute normally; this is not a filesystem sandbox. No default terminal or daily configuration is changed.".into()];
        let mut files = BTreeMap::new();
        let (mut config, appearance_notes) =
            appearance_configuration(workspace, owned, native_kitty)?;
        notes.extend(appearance_notes);
        if let Some(source) = native_kitty {
            files.insert("kitty.source.conf".into(), source.as_bytes().to_vec());
        }
        // Never load a user/system Kitty file. Overrides are parsed as data.
        config.push_str("shell_integration disabled\nallow_remote_control no\n");
        files.insert("kitty.conf".into(), config.into_bytes());
        if owned.prompt {
            let source = if workspace.use_designer {
                workspace.designer.to_starship_toml()
            } else {
                workspace
                    .starship
                    .clone()
                    .ok_or("Current Prompt has no Starship source.")?
            };
            let (safe, skipped) = crate::starship_import::prepare_config(&source)?;
            notes.extend(skipped.into_iter().map(|n| format!("Prompt · {n}: source retained, omitted from the controlled session. Trial confirms only this safe projection.")));
            files.insert("starship.source.toml".into(), source.into_bytes());
            files.insert("starship.toml".into(), safe.into_bytes());
        }
        let active_greeting = owned.greeting && workspace.greeting.enabled;
        let mut logo = None;
        let mut animated = false;
        let mut pixel = false;
        if active_greeting {
            let settings = &workspace.greeting;
            let source = settings.fastfetch_config()?;
            let (mut safe, skipped) = crate::fastfetch_document::preview_config_with_logo(
                &source,
                settings.source_logo.as_ref(),
            )?;
            notes.extend(skipped.into_iter().map(|n| format!("Greeting · {n}")));
            files.insert("fastfetch.source.jsonc".into(), source.into_bytes());
            let spec = settings
                .presentation
                .resolve(settings, crate::pixel_trial::Terminal::Kitty)?;
            if let Some(protocol) = spec.protocol {
                if protocol == pixel_export::Protocol::Sixel {
                    return Err("This controlled Kitty adapter supports Kitty static/animation, not Sixel. Explicitly choose Kitty output or create a Character copy.".into());
                }
                pixel = true;
                animated = protocol == pixel_export::Protocol::KittyAnimation;
                let image = pixel_export::prepare(
                    settings
                        .editable_artwork
                        .as_ref()
                        .ok_or("Missing editable artwork source.")?,
                )?;
                let temporary = tempfile::tempdir().map_err(|e| e.to_string())?;
                let bundle = pixel_export::export_bundle(
                    temporary.path(),
                    settings,
                    &image,
                    protocol,
                    spec.columns,
                    cells,
                )?;
                let bytes = read(&bundle.join("config.jsonc"))?
                    .ok_or("Missing generated configuration.")?;
                let generated = crate::fastfetch_document::value(
                    std::str::from_utf8(&bytes).map_err(|e| e.to_string())?,
                )?;
                safe["logo"] = generated["logo"].clone();
                let asset = protocol.source_name().to_owned();
                files.insert(
                    asset.clone(),
                    read(&bundle.join(&asset))?.ok_or("Missing processed image.")?,
                );
                logo = Some(asset);
            }
            safe["display"]["pipe"] = serde_json::json!(false);
            files.insert(
                "fastfetch.jsonc".into(),
                serde_json::to_vec_pretty(&safe).map_err(|e| e.to_string())?,
            );
        }
        let dependencies = if let Some(dependencies) = dependencies_for_test {
            dependencies
        } else {
            Dependencies {
                kitty: Executable::find("kitty")?,
                bash: Executable::capture(PathBuf::from("/usr/bin/bash"))?,
                helper: if pixel {
                    #[cfg(test)]
                    let path = std::env::var_os("TERMIMOCHI_SVG_WORKER_BIN")
                        .map(PathBuf::from)
                        .unwrap_or(std::env::current_exe().map_err(|e| e.to_string())?);
                    #[cfg(not(test))]
                    let path = std::env::current_exe().map_err(|e| e.to_string())?;
                    Some(Executable::capture(path)?)
                } else {
                    None
                },
                starship: owned
                    .prompt
                    .then(|| Executable::find("starship"))
                    .transpose()?,
                fastfetch: active_greeting
                    .then(|| Executable::find("fastfetch"))
                    .transpose()?,
            }
        };
        let expected = read(&root.join(deployment_id).join("current.json"))?;
        let prior_version = expected
            .as_ref()
            .map(|bytes| serde_json::from_slice::<Option<String>>(bytes).map_err(|e| e.to_string()))
            .transpose()?
            .flatten();
        let (previous_manifest, previous_summary) = if let Some(version) = prior_version {
            let project = root.join(deployment_id);
            let (previous, bytes) = load_version_snapshot(&project, deployment_id, &version)?;
            if previous.schema != 1 {
                return Err("The existing entry has an unsupported manifest. Review its recovery before updating this ID.".into());
            }
            let path = previous.directory.join(MANIFEST);
            (Some((path, bytes)), Some((previous.name, previous.version)))
        } else {
            (None, None)
        };
        Ok(Self {
            root: root.into(),
            deployment_id: deployment_id.into(),
            name: name.into(),
            revision,
            notes,
            animated,
            pixel,
            files,
            dependencies,
            expected,
            previous_manifest,
            previous_summary,
            logo,
        })
    }

    pub fn review_text(&self) -> String {
        let destination = self.root.join(&self.deployment_id);
        let replacement = self.previous_summary.as_ref().map_or_else(|| "Create a new independent entry; no prior active version.".to_owned(), |(name, version)| format!("Update existing independent entry: {name}\nCurrent version to replace: {version}\nThe previous version and assets remain available for recovery.\nIf an app-menu launcher exists, review Update App Launcher for this new version; the old launcher will not silently switch."));
        let mut text = format!(
            "Kitty · {}\nDeployment ID: {}\nEntry destination: {}\nVersion artifacts: {}\n{}\nDisplay: {}\nOwned output: {}\n\n{}",
            self.name,
            self.deployment_id,
            destination.join("current.json").display(),
            destination.join("versions").display(),
            replacement,
            if self.animated {
                "Animation · real motion confirmation required"
            } else if self.pixel {
                "Image · real visual confirmation required"
            } else {
                "Character terminal session"
            },
            self.files
                .keys()
                .filter(|k| !k.contains(".source."))
                .cloned()
                .collect::<Vec<_>>()
                .join(", "),
            self.notes.join("\n\n")
        );
        for (name, bytes) in &self.files {
            if name.ends_with(".conf") || name.ends_with(".toml") || name.ends_with(".jsonc") {
                text.push_str(&format!(
                    "\n\n--- {name} ---\n{}",
                    String::from_utf8_lossy(bytes)
                ));
            }
        }
        text
    }
    pub fn temporary_trial(&self, parent: &Path) -> Result<PreparedSession, String> {
        safe_root(parent)?;
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        let owner = tempfile::Builder::new()
            .prefix("kitty-trial-")
            .tempdir_in(parent)
            .map_err(|e| e.to_string())?;
        let deployment = self.materialize(owner.path(), "trial".into(), None)?;
        Ok(PreparedSession {
            owner: std::sync::Arc::new(owner),
            deployment,
        })
    }
    fn materialize(
        &self,
        directory: &Path,
        version: String,
        previous: Option<String>,
    ) -> Result<Deployment, String> {
        self.dependencies.check()?;
        let mut files = self.files.clone();
        if let Some(asset) = &self.logo {
            let config =
                String::from_utf8(files["fastfetch.jsonc"].clone()).map_err(|e| e.to_string())?;
            files.insert(
                "fastfetch.jsonc".into(),
                pixel_export::absolute_configuration(&config, &directory.join(asset))?.into_bytes(),
            );
        }
        files.insert(
            "session.bash".into(),
            bootstrap(directory, &self.dependencies)?.into_bytes(),
        );
        let hashes = files.iter().map(|(k, v)| (k.clone(), hash(v))).collect();
        for (name, bytes) in files {
            write_new(&directory.join(name), &bytes)?;
        }
        let deployment = Deployment {
            schema: 1,
            id: self.deployment_id.clone(),
            name: self.name.clone(),
            version,
            previous,
            revision: self.revision,
            notes: self.notes.clone(),
            directory: directory.into(),
            dependencies: self.dependencies.clone(),
            hashes,
        };
        write_new(
            &directory.join(MANIFEST),
            &serde_json::to_vec_pretty(&deployment).map_err(|e| e.to_string())?,
        )?;
        Ok(deployment)
    }
    /// The caller must have confirmed this frozen plan, including any projection
    /// losses. A new complete version is durable before the checked entry switch.
    pub fn publish(&self) -> Result<Deployment, String> {
        safe_root(&self.root)?;
        let project = self.root.join(&self.deployment_id);
        safe_root(&project)?;
        if read(&project.join("current.json"))? != self.expected {
            return Err("This project's entry changed since review. Reopen the use flow; the existing entry is untouched.".into());
        }
        if let Some((path, expected)) = &self.previous_manifest
            && read(path)?.as_ref() != Some(expected)
        {
            return Err("The reviewed existing entry manifest changed. Review the replacement again; the current entry was not switched.".into());
        }
        let versions = project.join("versions");
        safe_root(&versions)?;
        fs::create_dir_all(&versions).map_err(|e| e.to_string())?;
        let staged = tempfile::Builder::new()
            .prefix("version-")
            .tempdir_in(&versions)
            .map_err(|e| e.to_string())?;
        let version = staged
            .path()
            .file_name()
            .unwrap()
            .to_string_lossy()
            .into_owned();
        let previous = self
            .expected
            .as_ref()
            .map(|bytes| serde_json::from_slice::<Option<String>>(bytes).map_err(|e| e.to_string()))
            .transpose()?
            .flatten();
        let deployment = self.materialize(staged.path(), version.clone(), previous)?;
        // Keep complete version even if the final switch conflicts: no dangling
        // references or removal of assets potentially opened by another process.
        let _retained_version = staged.keep();
        let entry = serde_json::to_vec(&version).map_err(|e| e.to_string())?;
        typography_preset::write_checked_with_limit(
            &project.join("current.json"),
            &entry,
            &self.expected,
            LIMIT,
        )?;
        Ok(deployment)
    }
}

fn bootstrap(directory: &Path, deps: &Dependencies) -> Result<String, String> {
    let mut script = String::from(
        "# Generated controlled session; never source imported shell code.\nunset PROMPT_COMMAND BASH_ENV ENV HISTFILE\nPS1='\\u@\\h:\\w\\$ '\n",
    );
    script.push_str(&theme_initialization(directory, deps)?);
    Ok(script)
}

fn theme_initialization(directory: &Path, deps: &Dependencies) -> Result<String, String> {
    let mut script = String::new();
    if let Some(helper) = &deps.helper {
        script.push_str(&format!(
            "{} {}\n",
            path_quote(&helper.path)?,
            crate::pixel_trial::startup::ARG
        ));
    }
    if let Some(fetch) = &deps.fastfetch {
        script.push_str(&format!("{} --config {} || printf '%s\\n' 'TermiMochi: Greeting failed; the shell remains usable.' >&2\n", path_quote(&fetch.path)?, path_quote(&directory.join("fastfetch.jsonc"))?));
    }
    if let Some(starship) = &deps.starship {
        script.push_str(&format!("export STARSHIP_CONFIG={}\nexport STARSHIP_CACHE={}\neval \"$({} init bash --print-full-init)\"\nif declare -F starship_precmd >/dev/null; then starship_precmd; else printf '%s\\n' 'TermiMochi: Starship initialization failed.' >&2; fi\n", path_quote(&directory.join("starship.toml"))?, path_quote(&directory.join("cache"))?, path_quote(&starship.path)?));
    }
    Ok(script)
}

/// Allowlist only. In particular BASH_FUNC_*, PS*, PROMPT_COMMAND, BASH_ENV,
/// ENV, SHELLOPTS, KITTY_CONFIG_DIRECTORY and injected library variables die here.
pub(crate) fn clean_environment(command: &mut Command) {
    command.env_clear();
    for (key, value) in std::env::vars_os() {
        let name = key.to_string_lossy();
        if matches!(
            name.as_ref(),
            "HOME"
                | "USER"
                | "LOGNAME"
                | "DISPLAY"
                | "WAYLAND_DISPLAY"
                | "XAUTHORITY"
                | "XDG_RUNTIME_DIR"
                | "DBUS_SESSION_BUS_ADDRESS"
                | "LANG"
                | "LANGUAGE"
        ) || name.starts_with("LC_")
        {
            command.env(key, value);
        }
    }
    command
        .env("PATH", "/usr/local/bin:/usr/bin:/bin")
        .env("INPUTRC", "/dev/null")
        .env("SHELL", "/usr/bin/bash");
}

impl Deployment {
    fn check(&self) -> Result<(), String> {
        if self.schema != 1 {
            return Err("Unsupported controlled-session manifest.".into());
        }
        identifier(&self.id)?;
        identifier(&self.version)?;
        safe_root(&self.directory)?;
        self.dependencies.check()?;
        for (name, expected) in &self.hashes {
            if Path::new(name).file_name().and_then(|n| n.to_str()) != Some(name.as_str()) {
                return Err("Invalid managed artifact name.".into());
            }
            if read(&self.directory.join(name))?.as_deref().map(hash) != Some(*expected) {
                return Err(format!(
                    "Managed {name} changed or is missing. Re-publish after review; no external changes were overwritten."
                ));
            }
        }
        Ok(())
    }
    pub fn command(&self) -> Result<Command, String> {
        self.check()?;
        let bytes =
            read(&self.directory.join("kitty.conf"))?.ok_or("Missing Kitty configuration.")?;
        let source = std::str::from_utf8(&bytes).map_err(|e| e.to_string())?;
        let mut command = Command::new(&self.dependencies.kitty.path);
        clean_environment(&mut command);
        command.args([
            "--config",
            "NONE",
            "--title",
            &format!("TermiMochi · {}", self.name),
        ]);
        for line in source
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
        {
            let (key, value) = line
                .split_once(char::is_whitespace)
                .ok_or("Invalid generated Kitty configuration.")?;
            command
                .arg("--override")
                .arg(format!("{key}={}", value.trim()));
        }
        // Startup hook is generated locally, not inherited/imported. --norc
        // suppresses even the distro's /etc/bash.bashrc; the hook clears itself.
        command.env(
            "PROMPT_COMMAND",
            format!(
                "source {}",
                path_quote(&self.directory.join("session.bash"))?
            ),
        );
        command
            .arg(&self.dependencies.bash.path)
            .args(["--noprofile", "--norc", "-i"]);
        command.current_dir(gtk::glib::home_dir());
        Ok(command)
    }
    pub fn launch(&self) -> Result<(), String> {
        let mut child = self
            .command()?
            .spawn()
            .map_err(|e| format!("Kitty failed to start: {e}"))?;
        std::thread::spawn(move || {
            let _ = child.wait();
        });
        Ok(())
    }
}
impl PreparedSession {
    pub fn launch(&self) -> Result<(), String> {
        let mut child = self
            .deployment
            .command()?
            .spawn()
            .map_err(|e| format!("Kitty failed to start: {e}"))?;
        // A dialog closing must never unlink a still-running terminal's assets.
        let owner = self.owner.clone();
        std::thread::spawn(move || {
            let _ = child.wait();
            drop(owner);
        });
        Ok(())
    }
}

#[cfg(test)]
fn list(root: &Path) -> Result<Vec<Deployment>, String> {
    let (entries, issues) = list_with_issues(root)?;
    if issues.is_empty() {
        Ok(entries)
    } else {
        Err(issues.join("\n"))
    }
}

pub(crate) fn list_with_issues(root: &Path) -> Result<(Vec<Deployment>, Vec<String>), String> {
    safe_root(root)?;
    if !root.exists() {
        return Ok((vec![], vec![]));
    }
    let mut result = Vec::new();
    let mut issues = Vec::new();
    for entry in fs::read_dir(root).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let id = entry.file_name().to_string_lossy().into_owned();
        if identifier(&id).is_ok() && entry.file_type().map_err(|e| e.to_string())?.is_dir() {
            match current(root, &id) {
                Ok(Some(deployment)) => result.push(deployment),
                Ok(None) => {}
                Err(error) => issues.push(format!("{id}: {error}")),
            }
        }
    }
    result.sort_by(|a, b| a.name.cmp(&b.name));
    Ok((result, issues))
}

pub(crate) fn current(root: &Path, id: &str) -> Result<Option<Deployment>, String> {
    identifier(id)?;
    safe_root(root)?;
    let project = root.join(id);
    safe_root(&project)?;
    let Some(bytes) = read(&project.join("current.json"))? else {
        return Ok(None);
    };
    let Some(version): Option<String> =
        serde_json::from_slice(&bytes).map_err(|e| e.to_string())?
    else {
        return Ok(None);
    };
    Ok(Some(load_version(&project, id, &version)?))
}
fn load_version(project: &Path, id: &str, version: &str) -> Result<Deployment, String> {
    load_version_snapshot(project, id, version).map(|(deployment, _)| deployment)
}

fn load_version_snapshot(
    project: &Path,
    id: &str,
    version: &str,
) -> Result<(Deployment, Vec<u8>), String> {
    identifier(version)?;
    let directory = project.join("versions").join(version);
    safe_root(&directory)?;
    let bytes = read(&directory.join(MANIFEST))?.ok_or(
        "Managed session manifest is missing. Re-publish the project to repair the entry.",
    )?;
    let deployment: Deployment = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    if deployment.id != id || deployment.version != version || deployment.directory != directory {
        return Err("Managed session identity/path mismatch.".into());
    }
    Ok((deployment, bytes))
}

pub(crate) fn restore(
    root: &Path,
    id: &str,
    expected_version: &str,
) -> Result<Option<Deployment>, String> {
    let current = current(root, id)?.ok_or("No current session entry to restore.")?;
    if current.version != expected_version {
        return Err("Session entry changed; refresh before recovery.".into());
    }
    current.check()?;
    let Some(previous) = &current.previous else {
        let expected = Some(serde_json::to_vec(&current.version).map_err(|e| e.to_string())?);
        // An inert tombstone removes the newly created entry from the library,
        // without deleting its manifest, assets or potential live references.
        typography_preset::write_checked_with_limit(
            &root.join(id).join("current.json"),
            b"null",
            &expected,
            LIMIT,
        )?;
        return Ok(None);
    };
    let project = root.join(id);
    let restored = load_version(&project, id, previous)?;
    restored.check()?;
    let expected = Some(serde_json::to_vec(&current.version).map_err(|e| e.to_string())?);
    typography_preset::write_checked_with_limit(
        &project.join("current.json"),
        &serde_json::to_vec(previous).map_err(|e| e.to_string())?,
        &expected,
        LIMIT,
    )?;
    Ok(Some(restored))
}

#[cfg(test)]
mod tests;
