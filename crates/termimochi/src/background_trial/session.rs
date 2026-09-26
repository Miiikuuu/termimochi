//! Immutable enhanced-Ptyxis appearances. The desktop entry is the single
//! checked publication pointer; complete versions survive updates/recovery.
//! Portable themes never carry runtime paths or permission to publish/run.
use super::*;
use crate::{
    kitty_session::{Executable, identifier, safe_root},
    typography_preset as store,
};
use serde::{Deserialize, Serialize};
use std::process::Command;

pub const ARG: &str = "--open-enhanced-ptyxis";
const LIMIT: u64 = 64 * 1024 * 1024;
const RUNTIME_LIMIT: u64 = 512 * 1024 * 1024;
const RECORD: &str = "manifest.json";
const FOLDER: &str = "termimochi/ptyxis-background-sessions";

fn digest(bytes: &[u8]) -> String {
    glib::compute_checksum_for_data(glib::ChecksumType::Sha256, bytes)
        .unwrap()
        .into()
}
fn read(path: &Path) -> Result<Option<Vec<u8>>, String> {
    store::read_private_with_limit(
        path,
        if path.file_name().is_some_and(|n| n == "termimochi") {
            RUNTIME_LIMIT
        } else {
            LIMIT
        },
    )
}
fn write(path: &Path, bytes: &[u8], expected: &Option<Vec<u8>>) -> Result<(), String> {
    safe_root(path.parent().ok_or("Missing destination directory")?)?;
    store::write_checked_with_limit(
        path,
        bytes,
        expected,
        if path.file_name().is_some_and(|n| n == "termimochi") {
            RUNTIME_LIMIT
        } else {
            LIMIT
        },
    )
}
fn desktop_path(data: &Path, id: &str) -> PathBuf {
    data.join("applications")
        .join(format!("io.github.miiikuuu.termimochi.ptyxis-{id}.desktop"))
}
fn directory(data: &Path, id: &str, version: &str) -> Result<PathBuf, String> {
    identifier(id)?;
    identifier(version)?;
    let path = data.join(FOLDER).join(id).join("versions").join(version);
    safe_root(&path)?;
    Ok(path)
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Runtime {
    prefix: PathBuf,
    terminal: Executable,
    agent: Executable,
    schemas: String,
}
impl Runtime {
    fn capture(prefix: &Path) -> Result<Self, String> {
        let prefix = validate_runtime(prefix)?;
        if fs::read(prefix.join("share/termimochi-background-canvas-version"))
            .ok()
            .as_deref()
            != Some(b"1\n")
        {
            return Err("Update the optional enhanced runtime with scripts/build-background-runtime.sh. System Ptyxis is never substituted.".into());
        }
        Ok(Self {
            terminal: Executable::capture(prefix.join("bin/ptyxis"))?,
            agent: Executable::capture(prefix.join("libexec/ptyxis-agent"))?,
            schemas: digest(
                &fs::read(prefix.join("share/glib-2.0/schemas/gschemas.compiled"))
                    .map_err(|e| e.to_string())?,
            ),
            prefix,
        })
    }
    fn check(&self) -> Result<(), String> {
        self.terminal.check()?;
        self.agent.check()?;
        let now = Self::capture(&self.prefix)?;
        if now.schemas != self.schemas {
            return Err(
                "Enhanced Ptyxis schemas changed. Review and apply the saved theme again.".into(),
            );
        }
        Ok(())
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Record {
    schema: u8,
    id: String,
    name: String,
    version: String,
    runtime: Runtime,
    hashes: BTreeMap<String, String>,
    previous: Option<Vec<u8>>,
}

#[derive(Clone)]
pub(crate) struct Entry {
    data: PathBuf,
    record: Record,
    manifest: Vec<u8>,
    desktop: Vec<u8>,
}
impl Entry {
    pub fn name(&self) -> &str {
        &self.record.name
    }
    pub fn id(&self) -> &str {
        &self.record.id
    }
    pub fn version(&self) -> &str {
        &self.record.version
    }
    pub fn has_previous(&self) -> bool {
        self.record.previous.is_some()
    }
    fn directory(&self) -> Result<PathBuf, String> {
        directory(&self.data, self.id(), self.version())
    }
    fn generated_desktop(&self) -> Result<Vec<u8>, String> {
        let key = glib::KeyFile::new();
        let group = "Desktop Entry";
        key.set_string(group, "Type", "Application");
        key.set_string(
            group,
            "Name",
            &format!("TermiMochi · {} (Enhanced Ptyxis)", self.name()),
        );
        key.set_string(
            group,
            "Comment",
            "Independent background appearance · clean Bash; system Ptyxis unchanged",
        );
        key.set_string(group, "Icon", "utilities-terminal");
        key.set_boolean(group, "Terminal", false);
        key.set_string(group, "Categories", "System;TerminalEmulator;");
        let exe = self.directory()?.join("termimochi");
        let args = [
            exe.to_string_lossy().into_owned(),
            ARG.into(),
            self.data.to_string_lossy().into_owned(),
            self.id().into(),
        ];
        let exec = args
            .iter()
            .map(|s| crate::kitty_session::launcher::exec_arg(s))
            .collect::<Result<Vec<_>, _>>()?
            .join(" ");
        key.set_string(group, "Exec", &exec);
        key.set_string(group, "X-Termimochi-Enhanced-Version", self.version());
        key.set_string(
            group,
            "X-Termimochi-Enhanced-Checksum",
            &digest(&self.manifest),
        );
        Ok(key.to_data().as_bytes().to_vec())
    }
    fn artifacts(&self) -> Result<(), String> {
        let dir = self.directory()?;
        if read(&dir.join(RECORD))?.as_deref() != Some(&self.manifest) {
            return Err("Published manifest changed. Nothing was launched or overwritten.".into());
        }
        let keys: Vec<_> = self.record.hashes.keys().map(String::as_str).collect();
        if keys != ["appearance.json", "background.png", "termimochi"] {
            return Err("Invalid enhanced appearance artifacts.".into());
        }
        for (name, expected) in &self.record.hashes {
            if read(&dir.join(name))?.as_deref().map(digest).as_ref() != Some(expected) {
                return Err(format!(
                    "Managed {name} changed or is missing. Reopen the saved theme; external edits are not trusted."
                ));
            }
        }
        Executable::capture(dir.join("termimochi"))?;
        Ok(())
    }
    fn active(&self) -> Result<(), String> {
        if read(&desktop_path(&self.data, self.id()))?.as_ref() != Some(&self.desktop) {
            return Err("The entry changed since review. Reopen its current library entry.".into());
        }
        self.artifacts()
    }
    pub fn details(&self) -> String {
        format!(
            "Enhanced Ptyxis · {}\nVersion: {}\nShortcut: {}\nManaged files: {}\nRuntime: {}\n\nBackground, preview palette, font and grid snapshot. Clean Bash in your home folder; personal startup/Conda, Prompt/Greeting and other layout fields are not included. No live updates. System Ptyxis and the default terminal stay unchanged. Keep the optional runtime installed at the reviewed path.",
            self.name(),
            self.version(),
            desktop_path(&self.data, self.id()).display(),
            self.directory().unwrap_or_default().display(),
            self.record.runtime.prefix.display()
        )
    }
    /// A separate retained helper owns the private bus/terminal. Closing the
    /// editor does not terminate this shell or delete its session resources.
    pub fn launch(&self) -> Result<(), String> {
        self.active()?;
        self.record.runtime.check()?;
        let mut command = Command::new(self.directory()?.join("termimochi"));
        crate::kitty_session::clean_environment(&mut command);
        for key in [
            "XDG_CONFIG_HOME",
            "XDG_DATA_HOME",
            "XDG_CACHE_HOME",
            "XDG_STATE_HOME",
            "GDK_BACKEND",
            "LIBGL_ALWAYS_SOFTWARE",
        ] {
            if let Some(v) = std::env::var_os(key) {
                command.env(key, v);
            }
        }
        command.arg(ARG).arg(&self.data).arg(self.id());
        let logs = self.data.join(FOLDER).join(self.id()).join("logs");
        safe_root(&logs)?;
        fs::create_dir_all(&logs).map_err(|e| e.to_string())?;
        let log = tempfile::Builder::new()
            .prefix("open-")
            .suffix(".log")
            .tempfile_in(&logs)
            .map_err(|e| e.to_string())?;
        let (file, path) = log.keep().map_err(|e| e.to_string())?;
        command
            .stdout(file.try_clone().map_err(|e| e.to_string())?)
            .stderr(file);
        let mut child = command
            .spawn()
            .map_err(|e| format!("Open failed: {e}. Log: {}", path.display()))?;
        std::thread::sleep(std::time::Duration::from_millis(300));
        if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
            return Err(format!(
                "Launcher exited ({status}). Entry remains saved. Log: {}",
                path.display()
            ));
        }
        std::thread::spawn(move || {
            let _ = child.wait();
        });
        Ok(())
    }
    pub fn restore(&self) -> Result<Option<Entry>, String> {
        self.active()?;
        let previous = self
            .record
            .previous
            .as_deref()
            .map(|b| from_desktop(&self.data, self.id(), b))
            .transpose()?;
        if let Some(previous) = &previous {
            previous.artifacts()?;
        }
        let path = desktop_path(&self.data, self.id());
        let backups = self.data.join(FOLDER).join(self.id()).join("backups");
        safe_root(&backups)?;
        store::retain_backup(&backups, &self.desktop)?;
        if let Some(before) = &self.record.previous {
            write(&path, before, &Some(self.desktop.clone()))?;
        } else {
            // Revalidate the exact managed target immediately before removal.
            self.active()?;
            fs::remove_file(&path).map_err(|e| e.to_string())?;
        }
        // Versions and any live session remain intact, including deactivation.
        Ok(previous)
    }
    fn run(&self) -> Result<(), String> {
        self.active()?;
        self.record.runtime.check()?;
        let dir = self.directory()?;
        let appearance: Appearance = serde_json::from_slice(
            &read(&dir.join("appearance.json"))?.ok_or("Missing appearance")?,
        )
        .map_err(|e| e.to_string())?;
        validate_appearance(&appearance)?;
        let png = read(&dir.join("background.png"))?.ok_or("Missing background")?;
        let mut prepared = prepare_png_at(
            self.record.runtime.prefix.clone(),
            png,
            false,
            0.0,
            appearance,
        )?;
        prepared
            .env
            .insert("TERMIMOCHI_RESEARCH_CANVAS".into(), "1".into());
        // Restore real HOME/XDG only in the shell, not in the Ptyxis process.
        // No personal startup file is read, and no exported shell function or
        // inherited PROMPT_COMMAND is allowed to initialize it.
        let mut script =
            String::from("unset PROMPT_COMMAND BASH_ENV ENV HISTFILE\nPS1='\\u@\\h:\\w\\$ '\n");
        for key in [
            "HOME",
            "XDG_CONFIG_HOME",
            "XDG_DATA_HOME",
            "XDG_STATE_HOME",
            "XDG_CACHE_HOME",
            "XDG_RUNTIME_DIR",
            "DBUS_SESSION_BUS_ADDRESS",
        ] {
            if let Some(value) = std::env::var_os(key) {
                script.push_str(&format!(
                    "export {key}={}\n",
                    quote(&value.to_string_lossy())
                ));
            } else {
                script.push_str(&format!("unset {key}\n"));
            }
        }
        script.push_str("cd -- \"$HOME\"\n");
        fs::write(prepared.root.path().join("startup.bash"), script).map_err(|e| e.to_string())?;
        let keypath = prepared
            .root
            .path()
            .join("config/glib-2.0/settings/keyfile");
        let hook = format!(
            "source {}",
            quote(&prepared.root.path().join("startup.bash").to_string_lossy())
        );
        let command = format!(
            "/usr/bin/env {} /usr/bin/bash --noprofile --norc -i",
            quote(&format!("PROMPT_COMMAND={hook}"))
        );
        set_custom_command(&keypath, &command)?;
        let child =
            prepared.start_named(&format!("TermiMochi · {} — Enhanced Ptyxis", self.name()))?;
        child
            .wait(gio::Cancellable::NONE)
            .map_err(|e| e.to_string())?;
        if !child.is_successful() {
            return Err("Enhanced Ptyxis exited with an error. Reopen the saved theme to review runtime availability; the entry is retained.".into());
        }
        Ok(())
    }
}

fn set_custom_command(path: &Path, command: &str) -> Result<(), String> {
    let key = glib::KeyFile::new();
    key.load_from_file(path, glib::KeyFileFlags::NONE)
        .map_err(|e| e.to_string())?;
    // The GSettings keyfile stores a raw GVariant, not an escaped KeyFile
    // string. set_string() would double escape quotes in PROMPT_COMMAND.
    key.set_value(
        &format!("{SCHEMA_PATH}/Profiles/{PROFILE}"),
        "custom-command",
        &variant(command),
    );
    fs::write(path, key.to_data()).map_err(|e| e.to_string())
}

fn validate_appearance(a: &Appearance) -> Result<(), String> {
    if a.background.is_some()
        || a.font.is_empty()
        || a.font.len() > 1024
        || a.font.chars().any(char::is_control)
        || !(20..=240).contains(&a.columns)
        || !(10..=100).contains(&a.rows)
    {
        return Err("Invalid published appearance snapshot".into());
    }
    termimochi_core::PtyxisPalette::from_text(a.palette.as_deref().ok_or("Missing palette")?)
        .map_err(|e| e.to_string())?;
    Ok(())
}

fn from_desktop(data: &Path, id: &str, bytes: &[u8]) -> Result<Entry, String> {
    let key = glib::KeyFile::new();
    key.load_from_data(
        std::str::from_utf8(bytes).map_err(|e| e.to_string())?,
        glib::KeyFileFlags::NONE,
    )
    .map_err(|e| e.to_string())?;
    let version = key
        .string("Desktop Entry", "X-Termimochi-Enhanced-Version")
        .map_err(|_| "This shortcut is not a managed enhanced-Ptyxis entry.")?;
    let expected = key
        .string("Desktop Entry", "X-Termimochi-Enhanced-Checksum")
        .map_err(|e| e.to_string())?;
    let dir = directory(data, id, &version)?;
    let manifest = read(&dir.join(RECORD))?.ok_or("Missing enhanced entry manifest")?;
    if digest(&manifest) != expected {
        return Err("Enhanced entry manifest changed. Refusing to trust external edits.".into());
    }
    let record: Record = serde_json::from_slice(&manifest).map_err(|e| e.to_string())?;
    if record.schema != 1
        || record.id != id
        || record.version != version
        || record.name.is_empty()
        || record.name.len() > 512
        || record.name.chars().any(char::is_control)
    {
        return Err("Enhanced entry identity mismatch".into());
    }
    let entry = Entry {
        data: data.into(),
        record,
        manifest,
        desktop: bytes.into(),
    };
    if entry.generated_desktop()? != bytes {
        return Err(
            "App-menu shortcut was changed externally. Nothing was overwritten or launched.".into(),
        );
    }
    Ok(entry)
}
pub fn current(data: &Path, id: &str) -> Result<Option<Entry>, String> {
    identifier(id)?;
    safe_root(&data.join("applications"))?;
    read(&desktop_path(data, id))?
        .as_deref()
        .map(|b| from_desktop(data, id, b))
        .transpose()
}
pub fn list(data: &Path) -> Result<(Vec<Entry>, Vec<String>), String> {
    let root = data.join(FOLDER);
    safe_root(&root)?;
    if !root.exists() {
        return Ok((vec![], vec![]));
    }
    let mut entries = Vec::new();
    let mut issues = Vec::new();
    for item in fs::read_dir(root).map_err(|e| e.to_string())?.take(1024) {
        let item = item.map_err(|e| e.to_string())?;
        let id = item.file_name().to_string_lossy().into_owned();
        match current(data, &id) {
            Ok(Some(entry)) => entries.push(entry),
            Ok(None) => {}
            Err(e) => issues.push(format!("{id}: {e}")),
        }
    }
    entries.sort_by(|a, b| a.name().cmp(b.name()));
    Ok((entries, issues))
}

pub(crate) struct Plan {
    data: PathBuf,
    id: String,
    name: String,
    runtime: Runtime,
    files: BTreeMap<String, Vec<u8>>,
    expected: Option<Vec<u8>>,
    prior: Option<Entry>,
}
impl Plan {
    pub fn prepare(
        data: &Path,
        id: &str,
        name: &str,
        appearance: Appearance,
    ) -> Result<Self, String> {
        let prefix = runtime()?;
        #[cfg(test)]
        let exe = std::env::var_os("TERMIMOCHI_SVG_WORKER_BIN")
            .map(PathBuf::from)
            .unwrap_or(std::env::current_exe().map_err(|e| e.to_string())?);
        #[cfg(not(test))]
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        // Cargo build outputs can be hard-linked into deps/. Reading that
        // executable is safe; published destinations still reject aliases.
        use std::io::Read;
        let identity = Executable::capture(exe.clone())?;
        let mut helper = Vec::new();
        fs::File::open(&exe)
            .map_err(|e| e.to_string())?
            .take(RUNTIME_LIMIT + 1)
            .read_to_end(&mut helper)
            .map_err(|e| e.to_string())?;
        identity.check()?;
        if helper.len() as u64 > RUNTIME_LIMIT {
            return Err("Launcher executable exceeds 512 MiB".into());
        }
        Self::prepare_at(
            data,
            id,
            name,
            appearance,
            Runtime::capture(&prefix)?,
            helper,
        )
    }
    fn prepare_at(
        data: &Path,
        id: &str,
        name: &str,
        mut appearance: Appearance,
        runtime: Runtime,
        helper: Vec<u8>,
    ) -> Result<Self, String> {
        directory(data, id, "check")?;
        if name.is_empty() || name.len() > 512 || name.chars().any(char::is_control) {
            return Err("Invalid entry name".into());
        }
        let prior = current(data, id)?;
        if let Some(entry) = &prior {
            entry.artifacts()?;
        }
        let png = canvas(&appearance)?;
        appearance.background = None;
        appearance.columns = appearance.columns.clamp(20, 240);
        appearance.rows = appearance.rows.clamp(10, 100);
        validate_appearance(&appearance)?;
        let files = BTreeMap::from([
            (
                "appearance.json".into(),
                serde_json::to_vec_pretty(&appearance).map_err(|e| e.to_string())?,
            ),
            ("background.png".into(), png),
            ("termimochi".into(), helper),
        ]);
        Ok(Self {
            data: data.into(),
            id: id.into(),
            name: name.into(),
            runtime,
            files,
            expected: prior.as_ref().map(|e| e.desktop.clone()),
            prior,
        })
    }
    pub fn details(&self) -> String {
        format!(
            "{}\n{}\nShortcut: {}\nVersion assets: {}\nRuntime: {}\nUses a snapshot of the visible palette/font/grid, including preview defaults where inherited. Grid is limited to 20–240 columns and 10–100 rows. Prompt/Greeting, personal Bash/Conda and other layout settings are not applied. Window resizing can stretch the background.\n\n{}",
            self.name,
            if self.prior.is_some() {
                "Updates this independent entry; the previous version is retained for Restore."
            } else {
                "Creates an independent app-menu shortcut. Save alone never does this."
            },
            desktop_path(&self.data, &self.id).display(),
            self.data.join(FOLDER).join(&self.id).display(),
            self.runtime.prefix.display(),
            String::from_utf8_lossy(&self.files["appearance.json"])
        )
    }
    pub fn publish(&self) -> Result<Entry, String> {
        self.runtime.check()?;
        let destination = desktop_path(&self.data, &self.id);
        if read(&destination)? != self.expected {
            return Err(
                "Entry changed since review. Nothing was overwritten; reopen Apply.".into(),
            );
        }
        if let Some(prior) = &self.prior {
            prior.active()?;
        }
        let versions = self.data.join(FOLDER).join(&self.id).join("versions");
        safe_root(&versions)?;
        fs::create_dir_all(&versions).map_err(|e| e.to_string())?;
        let folder = tempfile::Builder::new()
            .prefix("version-")
            .tempdir_in(&versions)
            .map_err(|e| e.to_string())?;
        let version = folder
            .path()
            .file_name()
            .unwrap()
            .to_string_lossy()
            .into_owned();
        for (name, bytes) in &self.files {
            write(&folder.path().join(name), bytes, &None)?;
        }
        fs::set_permissions(
            folder.path().join("termimochi"),
            fs::Permissions::from_mode(0o700),
        )
        .map_err(|e| e.to_string())?;
        let record = Record {
            schema: 1,
            id: self.id.clone(),
            name: self.name.clone(),
            version,
            runtime: self.runtime.clone(),
            hashes: self
                .files
                .iter()
                .map(|(k, v)| (k.clone(), digest(v)))
                .collect(),
            previous: self.expected.clone(),
        };
        let manifest = serde_json::to_vec_pretty(&record).map_err(|e| e.to_string())?;
        write(&folder.path().join(RECORD), &manifest, &None)?;
        let mut entry = Entry {
            data: self.data.clone(),
            record,
            manifest,
            desktop: vec![],
        };
        entry.desktop = entry.generated_desktop()?;
        entry.artifacts()?;
        if let Some(prior) = &self.prior {
            prior.active()?;
        }
        if let Some(bytes) = &self.expected {
            let backups = versions.parent().unwrap().join("backups");
            safe_root(&backups)?;
            store::retain_backup(&backups, bytes)?;
        }
        // Single commit point: launcher + library refer to the same entry.
        // Complete versions are kept even if the final checked switch fails.
        let _retained = folder.keep();
        write(&destination, &entry.desktop, &self.expected)?;
        Ok(entry)
    }
}

pub fn dispatch(args: &[std::ffi::OsString]) -> glib::ExitCode {
    use adw::prelude::*;
    let result = if args.len() == 4 {
        current(Path::new(&args[2]), &args[3].to_string_lossy())
            .and_then(|e| {
                e.ok_or("Entry removed. Open the theme in TermiMochi to apply it again.".into())
            })
            .and_then(|e| e.run())
    } else {
        Err("Invalid enhanced Ptyxis entry arguments".into())
    };
    let Err(error) = result else {
        return glib::ExitCode::SUCCESS;
    };
    eprintln!("TermiMochi enhanced entry: {error}");
    let app = adw::Application::builder()
        .application_id("io.github.miiikuuu.termimochi.EnhancedEntryError")
        .flags(gio::ApplicationFlags::NON_UNIQUE)
        .build();
    app.connect_activate(move |app| {
        let win = adw::ApplicationWindow::builder().application(app).title("Enhanced Ptyxis needs attention").default_width(520).default_height(240).build();
        let body = gtk::Box::new(gtk::Orientation::Vertical, 16);
        body.set_margin_top(24); body.set_margin_bottom(24);
        body.set_margin_start(24); body.set_margin_end(24);
        body.append(&gtk::Label::builder().label(format!("{error}\n\nOpen the saved theme in TermiMochi to review and apply a new version. Daily settings were not changed.")).wrap(true).selectable(true).build());
        let close = gtk::Button::with_label("Close");
        let weak = win.downgrade();
        close.connect_clicked(move |_| { if let Some(w) = weak.upgrade() { w.close(); } });
        body.append(&close);
        win.set_content(Some(&body)); win.present();
    });
    app.run_with_args::<&str>(&[]);
    glib::ExitCode::FAILURE
}

#[cfg(test)]
mod tests;
