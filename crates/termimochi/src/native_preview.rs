//! Actual terminal surface host. Immutable theme projections feed private
//! adapters; no native runtime value is ever serialized into a design.
use crate::{
    design_document::{DesignDocument, TargetHint},
    kitty_session::{self, Ownership},
    workspace::Workspace,
};
use gtk::{glib, glib::translate::*};
use std::{
    collections::BTreeMap,
    ffi::CString,
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

const PROFILE: &str = "12345678-1234-4321-9876-0123456789ab";

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Snapshot {
    pub design: DesignDocument,
    pub workspace: Workspace,
}
impl Snapshot {
    pub fn target(&self) -> Result<TargetHint, String> {
        self.design
            .target_hint
            .ok_or("Choose a theme target before starting a native terminal.".into())
    }
    fn owned(&self) -> Ownership {
        let scope = self.design.scope();
        Ownership {
            palette: scope.palette,
            typography: scope.typography,
            layout: scope.layout,
            prompt: scope.prompt && self.design.theme.as_ref().is_none_or(|t| t.prompt_enabled),
            greeting: (scope.greeting || scope.artwork) && self.workspace.greeting.enabled,
        }
    }
    fn kitty(&self) -> Result<String, String> {
        let native = if self.design.theme.is_some() {
            Some(self.design.theme_kitty_configuration(&self.workspace)?)
        } else {
            self.design
                .native
                .as_ref()
                .filter(|s| s.format == crate::design_document::NativeFormat::Kitty)
                .map(|s| s.text.clone())
        };
        kitty_session::appearance_configuration(&self.workspace, self.owned(), native.as_deref())
            .map(|r| r.0)
    }
}

/// Only local installation paths. The launcher sets this for this App process,
/// never via the desktop session's activation environment.
fn prefix() -> Result<PathBuf, String> {
    let path = std::env::var_os("TERMIMOCHI_NATIVE_PREFIX")
        .map(PathBuf::from)
        .ok_or("Start the enabled build with scripts/run-native-preview.sh.")?;
    if !path.is_absolute() {
        return Err("Native dependency prefix must be absolute.".into());
    }
    path.canonicalize().map_err(|e| e.to_string())
}
fn quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}
fn variant_string(s: &str) -> String {
    glib::variant::ToVariant::to_variant(s)
        .print(true)
        .to_string()
}
fn atomic(path: &Path, contents: &[u8]) -> Result<(), String> {
    let mut file = tempfile::NamedTempFile::new_in(path.parent().ok_or("Missing parent")?)
        .map_err(|e| e.to_string())?;
    std::io::Write::write_all(&mut file, contents).map_err(|e| e.to_string())?;
    file.persist(path).map_err(|e| e.to_string())?;
    Ok(())
}
fn run(command: &mut Command) -> Result<(), String> {
    let label = command.get_program().to_string_lossy().into_owned();
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("{label}: {e}"))?;
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
            return if status.success() {
                Ok(())
            } else {
                Err(format!(
                    "{label} failed ({status}); native state is not confirmed."
                ))
            };
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(format!("{label} timed out; native state is not confirmed."));
        }
        std::thread::sleep(Duration::from_millis(15));
    }
}

pub(crate) struct Session {
    pub root: tempfile::TempDir,
    pub start: Snapshot,
    pub argv: Vec<String>,
    pub env: BTreeMap<String, String>,
    prefix: PathBuf,
    // Retained until supervisor has reaped every child, including animations.
    _bootstrap: Option<kitty_session::PreparedSession>,
    managed_settings: Mutex<Option<Vec<String>>>,
}
impl Session {
    pub fn prepare(start: Snapshot) -> Result<Arc<Self>, String> {
        let target = start.target()?;
        let prefix = prefix()?;
        let root = tempfile::Builder::new()
            .prefix("termimochi-native-")
            .tempdir()
            .map_err(|e| e.to_string())?;
        for dir in [
            "home",
            "config",
            "data",
            "state",
            "cache",
            "runtime",
            "data/org.gnome.Ptyxis/palettes",
        ] {
            fs::create_dir_all(root.path().join(dir)).map_err(|e| e.to_string())?;
        }
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(
            root.path().join("runtime"),
            fs::Permissions::from_mode(0o700),
        )
        .map_err(|e| e.to_string())?;
        let mut env = BTreeMap::new();
        for (key, value) in std::env::vars() {
            if matches!(key.as_str(), "LANG" | "LANGUAGE" | "USER" | "LOGNAME")
                || key.starts_with("LC_")
            {
                env.insert(key, value);
            }
        }
        for (key, dir) in [
            ("HOME", "home"),
            ("XDG_CONFIG_HOME", "config"),
            ("XDG_DATA_HOME", "data"),
            ("XDG_STATE_HOME", "state"),
            ("XDG_CACHE_HOME", "cache"),
            ("XDG_RUNTIME_DIR", "runtime"),
        ] {
            env.insert(
                key.into(),
                root.path().join(dir).to_string_lossy().into_owned(),
            );
        }
        for (key, value) in [
            ("PATH", "/usr/local/bin:/usr/bin:/bin"),
            ("SHELL", "/usr/bin/bash"),
            ("INPUTRC", "/dev/null"),
            ("GSETTINGS_BACKEND", "keyfile"),
            ("GTK_IM_MODULE", "wayland"),
            ("GSK_RENDERER", "cairo"),
            ("LIBGL_ALWAYS_SOFTWARE", "1"),
            ("GTK_A11Y", "none"),
            ("GIO_USE_VFS", "local"),
            ("TERMIMOCHI_PRIVATE_SESSION", "1"),
        ] {
            env.insert(key.into(), value.into());
        }
        env.insert(
            "LD_LIBRARY_PATH".into(),
            format!(
                "{}/lib:{}/usr/lib/x86_64-linux-gnu",
                prefix.display(),
                prefix.display()
            ),
        );
        env.insert(
            "GSETTINGS_SCHEMA_DIR".into(),
            prefix
                .join("share/glib-2.0/schemas")
                .to_string_lossy()
                .into_owned(),
        );
        // No activation paths, no systemd user manager, no daily bus fallback.
        let bus = format!(
            "<busconfig><type>session</type><listen>unix:path={}/runtime/bus</listen><auth>EXTERNAL</auth><policy context=\"default\"><allow send_destination=\"*\"/><allow receive_sender=\"*\"/><allow own=\"*\"/></policy></busconfig>",
            root.path().display()
        );
        fs::write(root.path().join("bus.conf"), bus).map_err(|e| e.to_string())?;
        let owned = start.owned();
        let bootstrap = if owned.prompt || owned.greeting {
            if target == TargetHint::Ptyxis && owned.greeting {
                let output = start.workspace.greeting.presentation.resolve(
                    &start.workspace.greeting,
                    crate::pixel_trial::Terminal::Ptyxis,
                )?;
                if output.protocol.is_some() {
                    return Err("Ptyxis pixel output is not verified. Choose Character output or disable Greeting for this real session; the artwork is retained.".into());
                }
            }
            Some(
                kitty_session::Plan::prepare(
                    root.path(),
                    &start.design.id,
                    "Native preview",
                    0,
                    &start.workspace,
                    owned,
                    None,
                    [8, 16],
                )?
                .temporary_trial(root.path())?,
            )
        } else {
            None
        };
        let hook = bootstrap
            .as_ref()
            .map(|p| format!("source {}", quote(&p.bootstrap_path().to_string_lossy())))
            .unwrap_or_else(|| {
                "unset PROMPT_COMMAND; PS1='preview \\w $ '; HISTFILE=/dev/null".into()
            });
        env.insert("PROMPT_COMMAND".into(), hook);
        let executable = match target {
            TargetHint::Kitty => {
                glib::find_program_in_path("kitty").ok_or("Kitty is not installed.")?
            }
            TargetHint::Ptyxis => prefix.join("bin/ptyxis"),
        };
        if !executable.is_file() {
            return Err(format!("Missing local terminal: {}", executable.display()));
        }
        let mut argv = vec![
            "/usr/bin/python3".into(),
            prefix
                .join("libexec/session-supervisor.py")
                .to_string_lossy()
                .into_owned(),
            root.path().to_string_lossy().into_owned(),
            executable.to_string_lossy().into_owned(),
        ];
        fs::write(
            root.path().join("rc-auth.py"),
            include_str!("native_rc_auth.py"),
        )
        .map_err(|e| e.to_string())?;
        match target {
            TargetHint::Kitty => {
                argv.extend([
                    "--config".into(),
                    root.path()
                        .join("kitty.conf")
                        .to_string_lossy()
                        .into_owned(),
                    "--start-as=maximized".into(),
                    "--override".into(),
                    "allow_remote_control=password".into(),
                    "--override".into(),
                    format!(
                        "remote_control_password=\"\" {}/rc-auth.py",
                        root.path().display()
                    ),
                    "--listen-on".into(),
                    format!("unix:{}/runtime/kitty.sock", root.path().display()),
                    "--override".into(),
                    "shell_integration=disabled".into(),
                    "--override".into(),
                    "shell=/usr/bin/bash --noprofile --norc -i".into(),
                    "--override".into(),
                    "linux_display_server=wayland".into(),
                ]);
            }
            TargetHint::Ptyxis => argv.extend([
                "--standalone".into(),
                "--new-window".into(),
                "--maximize".into(),
            ]),
        }
        let session = Arc::new(Self {
            root,
            start,
            argv,
            env,
            prefix,
            _bootstrap: bootstrap,
            managed_settings: Mutex::new(None),
        });
        session.apply(&session.start, false)?;
        Ok(session)
    }
    fn settings(
        &self,
        schema: &str,
        profile: bool,
        values: Vec<(&str, String)>,
    ) -> Result<(), String> {
        let mut command = Command::new(self.prefix.join("bin/termimochi-settings-helper"));
        command
            .env_clear()
            .envs(&self.env)
            .arg(self.root.path().join("config/glib-2.0/settings/keyfile"))
            .arg(schema)
            .arg(if profile {
                format!("/org/gnome/Ptyxis/Profiles/{PROFILE}/")
            } else {
                "-".into()
            });
        for (key, value) in values {
            command.arg(key).arg(value);
        }
        run(&mut command)
    }
    pub fn apply(&self, snapshot: &Snapshot, live: bool) -> Result<String, String> {
        if snapshot.design.id != self.start.design.id
            || snapshot.target()? != self.start.target()?
        {
            return Err("Native session belongs to another theme or target; stop it before starting this theme.".into());
        }
        match snapshot.target()? {
            TargetHint::Kitty => {
                let config = snapshot.kitty()?;
                atomic(&self.root.path().join("kitty.conf"), config.as_bytes())?;
                if live {
                    let kitty = &self.argv[3];
                    run(Command::new(kitty)
                        .env_clear()
                        .envs(&self.env)
                        .args([
                            "@",
                            "--to",
                            &format!("unix:{}/runtime/kitty.sock", self.root.path().display()),
                            "load-config",
                        ])
                        .arg(self.root.path().join("kitty.conf")))?;
                }
            }
            TargetHint::Ptyxis => {
                self.apply_ptyxis(snapshot)?;
                *self.managed_settings.lock().map_err(|e| e.to_string())? =
                    Some(self.ptyxis_values()?);
            }
        }
        let needs_restart = snapshot.workspace.designer != self.start.workspace.designer
            || snapshot.workspace.starship != self.start.workspace.starship
            || snapshot.workspace.greeting != self.start.workspace.greeting
            || snapshot.owned().prompt != self.start.owned().prompt;
        Ok(if needs_restart { "Appearance sent · Prompt/Greeting changes require Stop and Start; running programs are untouched." } else { "Appearance sent · actual pixels need visual review. Native Preferences changes are temporary; Resync restores theme-owned settings." }.into())
    }
    // Observe only keys this adapter manages, never terminal history or the
    // daily settings backend. Executed on the same bounded worker as sync.
    fn ptyxis_values(&self) -> Result<Vec<String>, String> {
        let file = glib::KeyFile::new();
        file.load_from_file(
            self.root.path().join("config/glib-2.0/settings/keyfile"),
            glib::KeyFileFlags::NONE,
        )
        .map_err(|e| e.to_string())?;
        let mut result = Vec::new();
        for (group, keys) in [
            (
                "org/gnome/Ptyxis".to_owned(),
                &[
                    "font-name",
                    "use-system-font",
                    "interface-style",
                    "cursor-shape",
                    "cursor-blink-mode",
                    "scrollbar-policy",
                ][..],
            ),
            (
                format!("org/gnome/Ptyxis/Profiles/{PROFILE}"),
                &["palette", "cell-height-scale", "cell-width-scale"][..],
            ),
        ] {
            for key in keys {
                result.push(
                    file.value(&group, key)
                        .map(|v| v.to_string())
                        .unwrap_or_default(),
                );
            }
        }
        Ok(result)
    }
    pub fn temporary_difference(&self) -> Result<bool, String> {
        if self.start.target()? != TargetHint::Ptyxis {
            return Ok(false);
        }
        let actual = self.ptyxis_values()?;
        Ok(self
            .managed_settings
            .lock()
            .map_err(|e| e.to_string())?
            .as_ref()
            .is_some_and(|expected| *expected != actual))
    }
    fn apply_ptyxis(&self, snapshot: &Snapshot) -> Result<(), String> {
        let w = &snapshot.workspace;
        let mut font = crate::typography::TypographySettings {
            family: "Monospace".into(),
            size: 11.0,
            ..Default::default()
        };
        if let Some(theme) = &snapshot.design.theme {
            let mut value = serde_json::to_value(&font).map_err(|e| e.to_string())?;
            for (k, v) in &theme.typography {
                value[k] = v.clone();
            }
            font = serde_json::from_value(value).map_err(|e| e.to_string())?;
        } else if snapshot.owned().typography {
            font = w.typography.clone();
        }
        font.validate()?;
        let mut palette = if snapshot.design.components.palette.is_some() {
            w.palette()?
        } else {
            termimochi_core::PtyxisPalette::from_text(
                &fs::read_to_string(self.prefix.join("share/termimochi-native/gnome.palette"))
                    .map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?
        };
        if let Some(theme) = &snapshot.design.theme {
            for (kind, colors) in [
                (termimochi_core::Variant::Light, &theme.colors),
                (termimochi_core::Variant::Dark, &theme.dark_colors),
            ] {
                if let Some(variant) = palette.variant_mut(kind) {
                    for (key, color) in colors {
                        variant
                            .set(key, color.parse().map_err(|e| format!("{e}"))?)
                            .map_err(|e| e.to_string())?;
                    }
                }
            }
        }
        atomic(
            &self
                .root
                .path()
                .join("data/org.gnome.Ptyxis/palettes/native.palette"),
            palette.to_palette_string().as_bytes(),
        )?;
        let mut global = vec![
            ("default-profile-uuid", quote(PROFILE)),
            ("profile-uuids", format!("['{PROFILE}']")),
            ("restore-window-size", "false".into()),
            ("restore-session", "false".into()),
            ("use-system-font", "false".into()),
            (
                "font-name",
                variant_string(&font.font_description().to_string()),
            ),
            (
                "interface-style",
                quote(if w.light { "light" } else { "dark" }),
            ),
        ];
        use crate::layout::{PreviewCursorBlink as Blink, PreviewCursorShape as Shape};
        let layout = w.layout;
        layout.validate()?;
        let specified = |field: &str| {
            snapshot
                .design
                .theme
                .as_ref()
                .map_or(snapshot.owned().layout, |t| t.layout.contains_key(field))
        };
        for (field, key, value) in [
            (
                "cursor_shape",
                "cursor-shape",
                variant_string(match layout.cursor_shape {
                    Shape::Block => "block",
                    Shape::IBeam => "ibeam",
                    Shape::Underline => "underline",
                }),
            ),
            (
                "cursor_blink",
                "cursor-blink-mode",
                variant_string(match layout.cursor_blink {
                    Blink::System => "system",
                    Blink::On => "on",
                    Blink::Off => "off",
                }),
            ),
            (
                "scrollbar",
                "scrollbar-policy",
                variant_string(if layout.scrollbar { "system" } else { "never" }),
            ),
            ("columns", "default-columns", layout.columns.to_string()),
            ("rows", "default-rows", layout.rows.to_string()),
        ] {
            global.push((
                key,
                if specified(field) {
                    value
                } else {
                    "@reset".into()
                },
            ));
        }
        self.settings("org.gnome.Ptyxis", false, global)?;
        self.settings(
            "org.gnome.Ptyxis.Profile",
            true,
            vec![
                ("palette", quote("native")),
                ("use-custom-command", "true".into()),
                (
                    "custom-command",
                    quote("/usr/bin/bash --noprofile --norc -i"),
                ),
                ("cell-height-scale", font.line_height.to_string()),
                ("cell-width-scale", font.cell_width.to_string()),
            ],
        )
    }
}

// Tiny, audited GObject FFI boundary; no parallel GTK instance or image stream.
#[link(name = "casilda-1.0")]
unsafe extern "C" {
    fn casilda_compositor_new(socket: *const libc::c_char) -> *mut gtk::ffi::GtkWidget;
    fn casilda_compositor_spawn_async(
        compositor: *mut gtk::ffi::GtkWidget,
        directory: *const libc::c_char,
        argv: *mut *mut libc::c_char,
        env: *mut *mut libc::c_char,
        flags: glib::ffi::GSpawnFlags,
        setup: glib::ffi::GSpawnChildSetupFunc,
        data: glib::ffi::gpointer,
        pid: *mut glib::ffi::GPid,
        error: *mut *mut glib::ffi::GError,
    ) -> glib::ffi::gboolean;
}
pub(crate) fn host() -> gtk::Widget {
    // Casilda returns a floating GtkWidget, sunk by from_glib_none.
    unsafe { from_glib_none(casilda_compositor_new(std::ptr::null())) }
}
pub(crate) fn spawn(host: &gtk::Widget, session: &Session) -> Result<glib::Pid, String> {
    let argv: Vec<_> = session
        .argv
        .iter()
        .map(|s| CString::new(s.as_str()).map_err(|e| e.to_string()))
        .collect::<Result<_, _>>()?;
    let env: Vec<_> = session
        .env
        .iter()
        .map(|(k, v)| CString::new(format!("{k}={v}")).map_err(|e| e.to_string()))
        .collect::<Result<_, _>>()?;
    let mut argv: Vec<_> = argv
        .iter()
        .map(|s| s.as_ptr().cast_mut())
        .chain([std::ptr::null_mut()])
        .collect();
    let mut env: Vec<_> = env
        .iter()
        .map(|s| s.as_ptr().cast_mut())
        .chain([std::ptr::null_mut()])
        .collect();
    let cwd = CString::new(
        session
            .root
            .path()
            .join("home")
            .to_string_lossy()
            .as_bytes(),
    )
    .map_err(|e| e.to_string())?;
    let mut pid = 0;
    let mut error = std::ptr::null_mut();
    let ok = unsafe {
        casilda_compositor_spawn_async(
            host.to_glib_none().0,
            cwd.as_ptr(),
            argv.as_mut_ptr(),
            env.as_mut_ptr(),
            glib::ffi::G_SPAWN_DO_NOT_REAP_CHILD,
            None,
            std::ptr::null_mut(),
            &mut pid,
            &mut error,
        )
    };
    if ok == 0 {
        return Err(unsafe { from_glib_full::<_, glib::Error>(error) }.to_string());
    }
    Ok(glib::Pid(pid))
}
