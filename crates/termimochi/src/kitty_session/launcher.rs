//! One published desktop file points at an immutable, reviewed launcher receipt.
//! No shell startup file, default terminal or system Kitty configuration is written.
use super::*;
use gtk::{gio, glib};
use std::os::unix::fs::PermissionsExt;

pub const ARG: &str = "--open-kitty-launcher";
pub const REPAIR_ARG: &str = "--repair-kitty-launcher";
const RECORD: &str = "launcher.json";
const RUNTIME_LIMIT: u64 = 512 * 1024 * 1024;

#[cfg(test)]
#[path = "launcher_tests.rs"]
mod tests;

// Called only on a worker (or before the launcher's error-window main loop).
// Retain diagnostics even when Kitty fails before showing a window.
fn spawn_session(mut command: Command, owner: Option<tempfile::TempDir>) -> Result<(), String> {
    let logs = glib::user_cache_dir().join("termimochi/launcher-logs");
    safe_root(&logs)?;
    fs::create_dir_all(&logs).map_err(|e| e.to_string())?;
    let log = tempfile::Builder::new()
        .prefix("launch-")
        .suffix(".log")
        .tempfile_in(logs)
        .map_err(|e| e.to_string())?;
    let (file, path) = log.keep().map_err(|e| e.to_string())?;
    command
        .stdout(file.try_clone().map_err(|e| e.to_string())?)
        .stderr(file);
    let mut child = command
        .spawn()
        .map_err(|e| format!("{e}. Log: {}", path.display()))?;
    std::thread::sleep(std::time::Duration::from_millis(250));
    if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
        return Err(format!(
            "Kitty exited before the session could be checked ({status}). Log: {}",
            path.display()
        ));
    }
    std::thread::spawn(move || {
        let _ = child.wait();
        drop(owner);
    });
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ShellMode {
    PersonalBash,
    Controlled,
}

fn digest(bytes: &[u8]) -> String {
    glib::compute_checksum_for_data(glib::ChecksumType::Sha256, bytes)
        .expect("SHA256 is available")
        .into()
}

/// Desktop Exec quoting is not shell quoting; KeyFile performs the outer
/// string escaping. Literal percent signs must survive field-code expansion.
fn exec_arg(text: &str) -> Result<String, String> {
    if text.chars().any(char::is_control) {
        return Err("Launcher arguments cannot contain control characters.".into());
    }
    let mut out = String::from("\"");
    for ch in text.chars() {
        if matches!(ch, '"' | '`' | '$' | '\\') {
            out.push('\\');
        }
        if ch == '%' {
            out.push('%');
        }
        out.push(ch);
    }
    out.push('"');
    Ok(out)
}

fn desktop_path(data: &Path, id: &str) -> PathBuf {
    data.join("applications")
        .join(format!("io.github.miiikuuu.termimochi.theme-{id}.desktop"))
}

fn runtime_executable() -> Result<PathBuf, String> {
    #[cfg(test)]
    let executable = std::env::var_os("TERMIMOCHI_SVG_WORKER_BIN")
        .map(PathBuf::from)
        .unwrap_or(std::env::current_exe().map_err(|e| e.to_string())?);
    #[cfg(not(test))]
    let executable = std::env::current_exe().map_err(|e| e.to_string())?;
    Ok(executable)
}

fn runtime_bytes() -> Result<Vec<u8>, String> {
    typography_preset::read_private_with_limit(&runtime_executable()?, RUNTIME_LIMIT)?
        .ok_or("TermiMochi executable is missing.".into())
}

fn bash_sources() -> Vec<PathBuf> {
    vec![
        PathBuf::from("/etc/bash.bashrc"),
        glib::home_dir().join(".bashrc"),
    ]
}

fn source_snapshot(path: &Path) -> Result<Option<Vec<u8>>, String> {
    match fs::metadata(path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.to_string()),
        Ok(m) if !m.is_file() || m.len() > 1024 * 1024 => Err(format!(
            "Bash startup must be a regular file below 1 MiB: {}",
            path.display()
        )),
        Ok(_) => fs::read(path).map(Some).map_err(|e| e.to_string()),
    }
}

fn startup(deployment: &Deployment, mode: ShellMode) -> Result<String, String> {
    if mode == ShellMode::Controlled {
        return bootstrap(&deployment.directory, &deployment.dependencies);
    }
    let mut script = String::from(
        "# Explicit personal Bash session. This executes local startup code, not imported code.\nunset PROMPT_COMMAND BASH_ENV ENV\nexport TERMIMOCHI_THEME_SESSION=1\n",
    );
    for path in bash_sources() {
        let quoted = path_quote(&path)?;
        script.push_str(&format!(
            "if [[ -r {quoted} ]]; then source {quoted} >/dev/null; fi\n"
        ));
    }
    // Keep aliases, functions, PATH, history and (if unowned) the user's prompt.
    // Only the theme-owned Prompt replaces the local prompt hooks.
    if deployment.dependencies.starship.is_some() {
        script.push_str("unset PROMPT_COMMAND\n");
    }
    script.push_str(&theme_initialization(
        &deployment.directory,
        &deployment.dependencies,
    )?);
    script.push('\n');
    Ok(script)
}

fn session_command(
    deployment: &Deployment,
    mode: ShellMode,
    script: &Path,
) -> Result<Command, String> {
    let mut command = deployment.command()?;
    command.env("PROMPT_COMMAND", format!("source {}", path_quote(script)?));
    if mode == ShellMode::PersonalBash {
        for key in [
            "PATH",
            "SSH_AUTH_SOCK",
            "SSH_AGENT_PID",
            "XDG_CONFIG_HOME",
            "XDG_DATA_HOME",
            "XDG_CACHE_HOME",
            "XDG_STATE_HOME",
            "XDG_CURRENT_DESKTOP",
            "XDG_SESSION_TYPE",
        ] {
            if let Some(value) = std::env::var_os(key) {
                command.env(key, value);
            }
        }
    }
    Ok(command)
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Record {
    schema: u8,
    data: PathBuf,
    deployment: Deployment,
    mode: ShellMode,
    runtime: Executable,
    startup_hash: String,
    before: Option<Vec<u8>>,
    #[serde(default)]
    theme_manifest_hash: Option<String>,
}

#[derive(Clone)]
pub struct Installed {
    pub path: PathBuf,
    pub checksum: String,
}

pub type ListedLauncher = (String, Result<Installed, String>);

pub struct LauncherPlan {
    deployment: Deployment,
    data: PathBuf,
    mode: ShellMode,
    before: Option<Vec<u8>>,
    binary: Vec<u8>,
    sources: Vec<(PathBuf, Option<Vec<u8>>)>,
    repair: Option<Plan>,
}

impl LauncherPlan {
    pub fn prepare(deployment: Deployment, mode: ShellMode) -> Result<Self, String> {
        let data = glib::user_data_dir();
        safe_root(&data.join("applications"))?;
        let before = read(&desktop_path(&data, &deployment.id))?;
        if before.is_some() {
            // Only a byte-identical, locally generated entry can be replaced.
            Installed::discover(&deployment.id)?.ok_or(
                "The app-menu entry is not managed by TermiMochi; choose a different theme ID.",
            )?;
        }
        let sources = if mode == ShellMode::PersonalBash {
            bash_sources()
                .into_iter()
                .map(|p| source_snapshot(&p).map(|b| (p, b)))
                .collect::<Result<_, _>>()?
        } else {
            vec![]
        };
        let plan = Self {
            deployment,
            data,
            mode,
            before,
            binary: runtime_bytes()?,
            sources,
            repair: None,
        };
        plan.check()?;
        Ok(plan)
    }
    fn check(&self) -> Result<(), String> {
        self.deployment.check()?;
        let root = self.data.join("termimochi/kitty-sessions");
        let active = current(&root, &self.deployment.id)?
            .ok_or("Entry is deactivated. Publish it again before creating a launcher.")?;
        if active.version != self.deployment.version {
            return Err("The active theme version changed. Reopen its result and review the launcher again.".into());
        }
        if let Some(repair) = &self.repair {
            if let Some((path, bytes)) = &repair.previous_manifest
                && read(path)?.as_ref() != Some(bytes)
            {
                return Err(
                    "Theme manifest changed during repair review. Nothing was replaced.".into(),
                );
            }
            active.check_artifacts()?;
        }
        if read(&desktop_path(&self.data, &self.deployment.id))? != self.before {
            return Err(
                "The app-menu launcher changed since review. Nothing was overwritten.".into(),
            );
        }
        for (path, before) in &self.sources {
            if source_snapshot(path)? != *before {
                return Err(
                    "Bash startup changed after review. Prepare and test the launcher again."
                        .into(),
                );
            }
        }
        Ok(())
    }
    pub fn review(&self) -> String {
        format!(
            "Application: {} — Kitty\nVersion: {} (pinned; later theme changes require Update App Launcher)\nDestination: {}\nShell: {:?}\n\n{}\n\nA private TermiMochi runtime is retained with this launcher, so closing or moving the editor does not remove it. Kitty, Bash, Fastfetch and Starship still need to remain installed. External changes block updates and recovery. No default terminal, daily kitty.conf or startup file is modified.\n\nGenerated startup:\n{}",
            self.deployment.name,
            self.deployment.version,
            desktop_path(&self.data, &self.deployment.id).display(),
            self.mode,
            if self.mode == ShellMode::PersonalBash {
                "Personal Bash explicitly executes /etc/bash.bashrc and ~/.bashrc. Aliases, PATH and history are retained; .profile and login scripts are not sourced. Startup stdout is hidden to avoid duplicate greetings; errors remain visible. The theme-owned Prompt runs after local initialization. Custom hooks that write directly to /dev/tty, exec another shell or reset colors can still conflict: verify this trial. Future local startup edits affect future launches."
            } else {
                "Isolated Bash does not load personal startup files, aliases or initialization."
            },
            format_args!(
                "{}\n{}",
                startup(&self.deployment, self.mode).unwrap_or_else(|e| e),
                self.repair
                    .as_ref()
                    .map(|p| p.notes.join("\n"))
                    .unwrap_or_default()
            )
        )
    }
    pub fn summary(&self) -> String {
        format!(
            "{}{} — Kitty · {}",
            if self.repair.is_some() {
                "Repair: "
            } else {
                ""
            },
            self.deployment.name,
            match self.mode {
                ShellMode::PersonalBash => "My Bash Environment",
                ShellMode::Controlled => "Isolated Bash",
            }
        )
    }
    pub fn trial(&self) -> Result<(), String> {
        self.check()?;
        let parent = glib::user_cache_dir().join("termimochi/launcher-trials");
        safe_root(&parent)?;
        fs::create_dir_all(&parent).map_err(|e| e.to_string())?;
        let owner = tempfile::Builder::new()
            .prefix("trial-")
            .tempdir_in(parent)
            .map_err(|e| e.to_string())?;
        let path = owner.path().join("startup.bash");
        let deployment = if let Some(repair) = &self.repair {
            let directory = owner.path().join("session");
            fs::create_dir(&directory).map_err(|e| e.to_string())?;
            repair.materialize(&directory, self.deployment.version.clone(), None)?
        } else {
            self.deployment.clone()
        };
        write_new(&path, startup(&deployment, self.mode)?.as_bytes())?;
        spawn_session(session_command(&deployment, self.mode, &path)?, Some(owner))
    }
    pub fn install(&self) -> Result<Installed, String> {
        self.check()?;
        let parent = self
            .data
            .join("termimochi/kitty-launchers")
            .join(&self.deployment.id);
        safe_root(&parent)?;
        fs::create_dir_all(&parent).map_err(|e| e.to_string())?;
        let folder = tempfile::Builder::new()
            .prefix("version-")
            .tempdir_in(&parent)
            .map_err(|e| e.to_string())?;
        let runtime_dir = self
            .data
            .join("termimochi/launcher-runtimes")
            .join(digest(&self.binary));
        safe_root(&runtime_dir)?;
        let mut executable = runtime_dir.join("termimochi");
        let mut old = typography_preset::read_private_with_limit(&executable, RUNTIME_LIMIT)?;
        if self.repair.is_some() && old.as_ref().is_some_and(|bytes| bytes != &self.binary) {
            // Explicit repair retains the changed runtime, never overwrites it.
            let fresh = tempfile::Builder::new()
                .prefix("repair-")
                .tempdir_in(&runtime_dir)
                .map_err(|e| e.to_string())?;
            executable = fresh.keep().join("termimochi");
            old = None;
        }
        match old {
            Some(bytes) if bytes != self.binary => {
                return Err("Managed launcher runtime changed; it was not overwritten.".into());
            }
            None => {
                typography_preset::write_checked_with_limit(
                    &executable,
                    &self.binary,
                    &None,
                    RUNTIME_LIMIT,
                )?;
                fs::set_permissions(&executable, fs::Permissions::from_mode(0o700))
                    .map_err(|e| e.to_string())?;
            }
            _ => {}
        }
        let runtime = Executable::capture(executable)?;
        let mut deployment = self.deployment.clone();
        // The settle helper must also survive rebuilding/moving the editor.
        if deployment.dependencies.helper.is_some() {
            deployment.dependencies.helper = Some(runtime.clone());
        }
        if let Some(repair) = &self.repair {
            let mut repair = repair.clone();
            repair.dependencies = deployment.dependencies.clone();
            let directory = folder.path().join("session");
            fs::create_dir(&directory).map_err(|e| e.to_string())?;
            deployment = repair.materialize(&directory, deployment.version.clone(), None)?;
        }
        let script = startup(&deployment, self.mode)?;
        write_new(&folder.path().join("startup.bash"), script.as_bytes())?;
        let record = Record {
            schema: 1,
            data: self.data.clone(),
            deployment,
            mode: self.mode,
            runtime,
            startup_hash: digest(script.as_bytes()),
            before: self.before.clone(),
            theme_manifest_hash: Some(digest(
                &read(
                    &current(
                        &self.data.join("termimochi/kitty-sessions"),
                        &self.deployment.id,
                    )?
                    .ok_or("Theme was deactivated during publication.")?
                    .directory
                    .join(MANIFEST),
                )?
                .ok_or("Theme manifest disappeared during publication.")?,
            )),
        };
        let bytes = serde_json::to_vec_pretty(&record).map_err(|e| e.to_string())?;
        let installed = Installed {
            path: folder.path().join(RECORD),
            checksum: digest(&bytes),
        };
        write_new(&installed.path, &bytes)?;
        let desktop = installed.desktop(&record)?;
        // Keep the complete immutable receipt before the single publication
        // switch. Failed/interrupted publication leaves only inert artifacts.
        let _retained = folder.keep();
        self.check()?;
        let destination = desktop_path(&self.data, &self.deployment.id);
        if let Some(before) = &self.before {
            typography_preset::retain_backup(&parent.join("backups"), before)?;
        }
        typography_preset::write_checked_with_limit(&destination, &desktop, &self.before, LIMIT)?;
        Ok(installed)
    }
}

impl Installed {
    /// Repair only this managed entry. Never execute a broken startup script or
    /// accept edited configuration/assets as a new trusted baseline.
    pub fn prepare_repair(&self) -> Result<LauncherPlan, String> {
        let record = self.record()?;
        if record.data != glib::user_data_dir() {
            return Err("Launcher belongs to another user data directory.".into());
        }
        self.prepare_repair_at(&record.data)
    }

    fn prepare_repair_at(&self, data: &Path) -> Result<LauncherPlan, String> {
        let record = self.record()?;
        if Self::discover_at(data, &record.deployment.id)?
            .as_ref()
            .map(|v| &v.path)
            != Some(&self.path)
        {
            return Err(
                "The launcher changed or was removed. Reopen its current library entry.".into(),
            );
        }
        let root = data.join("termimochi/kitty-sessions");
        let mut deployment = current(&root, &record.deployment.id)?
            .ok_or("This theme is deactivated. Re-publish it before repairing its launcher.")?;
        deployment.check_artifacts()?;
        let manifest_path = deployment.directory.join(MANIFEST);
        let manifest = read(&manifest_path)?.ok_or("Missing theme manifest.")?;
        if deployment.version == record.deployment.version {
            let changed = record.theme_manifest_hash.as_ref().map_or_else(
                || {
                    record.deployment.directory == deployment.directory
                        && record.deployment.hashes != deployment.hashes
                },
                |expected| expected != &digest(&manifest),
            );
            if changed {
                return Err("The published theme manifest was modified. Re-publish from the saved theme; launcher repair will not trust edited artifacts.".into());
            }
        }
        let mut notes = vec![format!(
            "Targeted repair: {} — Kitty. Launcher version {} → active theme {}. Only this app-menu entry switches; theme source/version, other launchers and daily configuration are unchanged. The previous entry is retained for Undo Launcher Update.",
            deployment.name, record.deployment.version, deployment.version
        )];
        notes.push("Startup is regenerated from verified theme artifacts. Missing runtime is restored; a changed runtime is kept and a separate clean copy is retained. Undo returns the previous entry, which may still be broken.".into());
        for (name, executable) in [
            ("kitty", Some(&mut deployment.dependencies.kitty)),
            ("bash", Some(&mut deployment.dependencies.bash)),
            ("starship", deployment.dependencies.starship.as_mut()),
            ("fastfetch", deployment.dependencies.fastfetch.as_mut()),
        ] {
            if let Some(executable) = executable {
                let refreshed = if executable.path.exists() {
                    Executable::capture(executable.path.clone())?
                } else {
                    Executable::find(name)?
                };
                notes.push(format!(
                    "Review {name}: {} {:?} → {} {:?}",
                    executable.path.display(),
                    executable.identity,
                    refreshed.path.display(),
                    refreshed.identity
                ));
                *executable = refreshed;
            }
        }
        if deployment.dependencies.helper.is_some() {
            deployment.dependencies.helper = Some(Executable::capture(runtime_executable()?)?);
        }
        let mut files = BTreeMap::new();
        for name in deployment
            .hashes
            .keys()
            .filter(|name| name.as_str() != "session.bash")
        {
            let bytes =
                read(&deployment.directory.join(name))?.ok_or("Missing managed artifact.")?;
            if hash(&bytes) != deployment.hashes[name] {
                return Err("Theme artifact changed while preparing repair.".into());
            }
            files.insert(name.clone(), bytes);
        }
        let logo = files
            .get("fastfetch.jsonc")
            .map(|bytes| {
                crate::fastfetch_document::value(
                    std::str::from_utf8(bytes).map_err(|e| e.to_string())?,
                )
            })
            .transpose()?
            .and_then(|value| value["logo"]["source"].as_str().map(str::to_owned))
            .and_then(|source| {
                let path = Path::new(&source);
                (path.parent() == Some(deployment.directory.as_path()))
                    .then(|| path.file_name().map(|n| n.to_string_lossy().into_owned()))
                    .flatten()
            });
        if logo.as_ref().is_some_and(|name| !files.contains_key(name)) {
            return Err("Greeting references an unrecorded asset; repair is blocked.".into());
        }
        let repair = Plan {
            root,
            deployment_id: deployment.id.clone(),
            name: deployment.name.clone(),
            revision: deployment.revision,
            notes,
            animated: false,
            pixel: logo.is_some(),
            files,
            dependencies: deployment.dependencies.clone(),
            expected: None,
            previous_manifest: Some((manifest_path, manifest)),
            previous_summary: None,
            logo,
        };
        let sources = if record.mode == ShellMode::PersonalBash {
            bash_sources()
                .into_iter()
                .map(|p| source_snapshot(&p).map(|b| (p, b)))
                .collect::<Result<_, _>>()?
        } else {
            vec![]
        };
        let plan = LauncherPlan {
            before: read(&desktop_path(data, &deployment.id))?,
            deployment,
            data: data.into(),
            mode: record.mode,
            binary: runtime_bytes()?,
            sources,
            repair: Some(repair),
        };
        plan.check()?;
        Ok(plan)
    }

    pub fn name(&self) -> Result<String, String> {
        self.record()
            .map(|r| format!("{} — Kitty", r.deployment.name))
    }
    fn record(&self) -> Result<Record, String> {
        let bytes = read(&self.path)?
            .ok_or("Launcher receipt is missing. Recreate the launcher in TermiMochi.")?;
        if digest(&bytes) != self.checksum {
            return Err("Launcher receipt changed; refusing to run it.".into());
        }
        let record: Record = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
        identifier(&record.deployment.id)?;
        let parent = record
            .data
            .join("termimochi/kitty-launchers")
            .join(&record.deployment.id);
        if record.schema != 1
            || self.path.file_name().and_then(|n| n.to_str()) != Some(RECORD)
            || self.path.parent().and_then(Path::parent) != Some(parent.as_path())
        {
            return Err("Invalid launcher receipt location or version.".into());
        }
        safe_root(self.path.parent().unwrap())?;
        Ok(record)
    }
    fn desktop(&self, record: &Record) -> Result<Vec<u8>, String> {
        let runtime = record
            .runtime
            .path
            .to_str()
            .ok_or("Unsupported executable path")?;
        if runtime.contains('=') {
            return Err("Desktop executable paths cannot contain '='.".into());
        }
        let key = glib::KeyFile::new();
        key.set_string("Desktop Entry", "Type", "Application");
        key.set_string(
            "Desktop Entry",
            "Name",
            &format!("{} — Kitty", record.deployment.name),
        );
        key.set_string(
            "Desktop Entry",
            "Comment",
            "Open a reviewed TermiMochi theme in Kitty",
        );
        key.set_string("Desktop Entry", "Icon", "utilities-terminal");
        key.set_string("Desktop Entry", "Categories", "System;TerminalEmulator;");
        key.set_boolean("Desktop Entry", "Terminal", false);
        key.set_string(
            "Desktop Entry",
            "Exec",
            &format!(
                "{} {ARG} {} {}",
                exec_arg(runtime)?,
                exec_arg(self.path.to_str().ok_or("Unsupported receipt path")?)?,
                self.checksum
            ),
        );
        key.set_string(
            "Desktop Entry",
            "X-TermiMochi-Receipt",
            self.path.to_str().unwrap(),
        );
        key.set_string("Desktop Entry", "X-TermiMochi-Checksum", &self.checksum);
        Ok(key.to_data().as_bytes().to_vec())
    }
    pub fn discover(id: &str) -> Result<Option<Self>, String> {
        Self::discover_at(&glib::user_data_dir(), id)
    }
    fn discover_at(data: &Path, id: &str) -> Result<Option<Self>, String> {
        identifier(id)?;
        let path = desktop_path(data, id);
        let Some(bytes) = read(&path)? else {
            return Ok(None);
        };
        let key = glib::KeyFile::new();
        key.load_from_data(
            std::str::from_utf8(&bytes).map_err(|e| e.to_string())?,
            glib::KeyFileFlags::NONE,
        )
        .map_err(|e| e.to_string())?;
        let installed = Self {
            path: PathBuf::from(
                key.string("Desktop Entry", "X-TermiMochi-Receipt")
                    .map_err(|e| e.to_string())?
                    .as_str(),
            ),
            checksum: key
                .string("Desktop Entry", "X-TermiMochi-Checksum")
                .map_err(|e| e.to_string())?
                .into(),
        };
        let expected_root = data.join("termimochi/kitty-launchers").join(id);
        if installed.path.parent().and_then(Path::parent) != Some(expected_root.as_path()) {
            return Err("The desktop entry is not a managed launcher for this theme.".into());
        }
        let record = installed.record()?;
        if record.data != data || record.deployment.id != id || installed.desktop(&record)? != bytes
        {
            return Err(
                "The desktop entry was modified externally. It was not overwritten.".into(),
            );
        }
        Ok(Some(installed))
    }
    /// Include launchers whose independent entry is now deactivated. They must
    /// remain removable from the GUI after restarting the editor.
    pub fn list() -> Result<Vec<ListedLauncher>, String> {
        let data = glib::user_data_dir();
        let parent = data.join("applications");
        safe_root(&parent)?;
        if !parent.exists() {
            return Ok(vec![]);
        }
        let mut entries = vec![];
        for entry in fs::read_dir(parent).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let name = entry.file_name();
            let Some(id) = name
                .to_str()
                .and_then(|s| s.strip_prefix("io.github.miiikuuu.termimochi.theme-"))
                .and_then(|s| s.strip_suffix(".desktop"))
            else {
                continue;
            };
            let installed =
                Self::discover_at(&data, id).and_then(|v| v.ok_or("Launcher disappeared.".into()));
            let label = installed
                .as_ref()
                .ok()
                .and_then(|v| v.record().ok())
                .map(|r| format!("{} — Kitty", r.deployment.name))
                .unwrap_or_else(|| id.to_owned());
            entries.push((label, installed));
        }
        entries.sort_by(|a, b| a.0.cmp(&b.0));
        Ok(entries)
    }
    pub fn open(&self) -> Result<(), String> {
        let record = self.record()?;
        if record.data != glib::user_data_dir() {
            return Err("Launcher belongs to a different user data directory.".into());
        }
        if Self::discover_at(&record.data, &record.deployment.id)?
            .as_ref()
            .map(|v| &v.path)
            != Some(&self.path)
        {
            return Err(
                "This launcher is no longer active. Reopen its current entry from the app menu."
                    .into(),
            );
        }
        let root = record.data.join("termimochi/kitty-sessions");
        let active = current(&root, &record.deployment.id)?
            .ok_or("Theme entry is deactivated. Re-publish it in TermiMochi.")?;
        if active.version != record.deployment.version {
            return Err("The theme version changed. Choose Update App Launcher in TermiMochi and test the new version.".into());
        }
        record.runtime.check()?;
        let path = self.path.parent().unwrap().join("startup.bash");
        if read(&path)?.as_deref().map(digest).as_deref() != Some(&record.startup_hash) {
            return Err("Launcher startup changed; refusing to execute it.".into());
        }
        spawn_session(
            session_command(&record.deployment, record.mode, &path)?,
            None,
        )
    }
    /// Undo this one desktop-file publication. Artifacts remain recoverable.
    pub fn restore(&self) -> Result<(), String> {
        let record = self.record()?;
        let path = desktop_path(&record.data, &record.deployment.id);
        let after = self.desktop(&record)?;
        if read(&path)? != Some(after.clone()) {
            return Err("Launcher changed since review. Nothing was removed.".into());
        }
        if let Some(before) = &record.before {
            typography_preset::write_checked_with_limit(&path, before, &Some(after), LIMIT)
        } else {
            fs::remove_file(&path).map_err(|e| e.to_string())
        }
    }
}

/// Launch dispatch runs before normal document opening. Errors are visible even
/// when invoked from the desktop without an attached terminal.
pub fn dispatch(args: &[std::ffi::OsString]) -> glib::ExitCode {
    use adw::prelude::*;
    let result = if args.len() == 4 {
        Installed {
            path: PathBuf::from(&args[2]),
            checksum: args[3].to_string_lossy().into_owned(),
        }
        .open()
    } else {
        Err("Invalid Kitty launcher arguments.".into())
    };
    let Err(error) = result else {
        return glib::ExitCode::SUCCESS;
    };
    eprintln!("TermiMochi launcher: {error}");
    let app = adw::Application::builder()
        .application_id("io.github.miiikuuu.termimochi.LauncherError")
        .flags(gio::ApplicationFlags::NON_UNIQUE)
        .build();
    let args_for_repair: Vec<_> = args.iter().skip(2).cloned().collect();
    app.connect_activate(move |app| {
        let window = adw::ApplicationWindow::builder()
            .application(app)
            .title("Theme launcher needs attention")
            .default_width(500)
            .default_height(220)
            .build();
        let body = gtk::Box::new(gtk::Orientation::Vertical, 16);
        body.set_margin_top(24);
        body.set_margin_bottom(24);
        body.set_margin_start(24);
        body.set_margin_end(24);
        body.append(
            &gtk::Label::builder()
                .label(&error)
                .wrap(true)
                .selectable(true)
                .build(),
        );
        let open = gtk::Button::with_label("Repair This Launcher…");
        let repair_args = args_for_repair.clone();
        open.connect_clicked(move |_| {
            if let Ok(exe) = std::env::current_exe() {
                let _ = Command::new(exe).arg(REPAIR_ARG).args(&repair_args).spawn();
            }
        });
        body.append(&open);
        let close = gtk::Button::with_label("Close");
        let weak = window.downgrade();
        close.connect_clicked(move |_| {
            if let Some(window) = weak.upgrade() {
                window.close();
            }
        });
        body.append(&close);
        window.set_content(Some(&body));
        window.present();
    });
    app.run_with_args::<&str>(&[]);
    glib::ExitCode::FAILURE
}
