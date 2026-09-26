//! Optional enhanced runtime. Trials are temporary; `session` publishes only
//! explicitly reviewed, independent entries, never the system Ptyxis profile.
use crate::workspace::Workspace;
use gtk::{gio, glib};
use std::os::unix::fs::PermissionsExt;
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

pub(crate) mod session;
pub(crate) mod session_ui;
mod ui;
pub(crate) use ui::present;
pub(crate) use ui::present_theme;
pub(crate) const ARG: &str = "--ptyxis-background-trial";
const ID: &str = "io.github.Miiikuuu.Termimochi.PtyxisBackgroundResearch";
const SCHEMA_PATH: &str = "io/github/Miiikuuu/Termimochi/PtyxisBackgroundResearch";
const PROFILE: &str = "12345678-1234-4321-9876-0123456789ab";

#[derive(Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Appearance {
    background: Option<crate::background::BackgroundSettings>,
    canvas: [u32; 2],
    palette: Option<String>,
    font: String,
    columns: usize,
    rows: usize,
    light: bool,
}
impl Default for Appearance {
    fn default() -> Self {
        Self {
            background: None,
            canvas: [1000, 600],
            palette: None,
            font: "Monospace 12".into(),
            columns: 100,
            rows: 30,
            light: false,
        }
    }
}
impl Appearance {
    pub fn from_workspace(workspace: &Workspace) -> Self {
        Self {
            background: workspace.background.clone(),
            canvas: [
                (workspace.layout.columns as u32).saturating_mul(10),
                (workspace.layout.rows as u32).saturating_mul(20),
            ],
            palette: Some(workspace.palette.clone()),
            font: workspace.typography.font_description().to_string(),
            columns: workspace.layout.columns,
            rows: workspace.layout.rows,
            light: workspace.light,
        }
    }
    pub fn with_cells(mut self, cells: [u32; 2]) -> Self {
        self.canvas = [
            (self.columns as u32).saturating_mul(cells[0].max(1)),
            (self.rows as u32).saturating_mul(cells[1].max(1)),
        ];
        self
    }
}

pub(super) struct Prepared {
    // Retain assets until the supervisor has reaped terminal and shell children.
    pub root: tempfile::TempDir,
    executable: PathBuf,
    env: BTreeMap<String, String>,
}
fn variant(text: &str) -> String {
    glib::variant::ToVariant::to_variant(text)
        .print(true)
        .to_string()
}
fn quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', "'\\''"))
}

pub(super) fn runtime() -> Result<PathBuf, String> {
    let prefix = std::env::var_os("TERMIMOCHI_BACKGROUND_PREFIX").map(PathBuf::from)
        .ok_or("Optional runtime unavailable. Start with scripts/run-background-trial.sh; the system Ptyxis is not substituted.")?;
    validate_runtime(&prefix)
}

fn validate_runtime(prefix: &Path) -> Result<PathBuf, String> {
    if !prefix.is_absolute() {
        return Err("Background runtime prefix must be absolute.".into());
    }
    let prefix = prefix.canonicalize().map_err(|e| e.to_string())?;
    for relative in [
        "bin/ptyxis",
        "libexec/ptyxis-agent",
        "share/glib-2.0/schemas/gschemas.compiled",
    ] {
        if !prefix.join(relative).is_file() {
            return Err(format!("Incomplete optional runtime: {relative}"));
        }
    }
    let schema =
        fs::read_to_string(prefix.join(format!("share/glib-2.0/schemas/{ID}.gschema.xml")))
            .map_err(|_| "This is not the isolated background runtime.".to_string())?;
    if !schema.contains(ID) {
        return Err("Background runtime schema mismatch.".into());
    }
    Ok(prefix)
}

pub(super) fn prepare(
    path: &Path,
    contain: bool,
    tint: f64,
    appearance: Appearance,
) -> Result<Prepared, String> {
    if !tint.is_finite() || !(0.0..=1.0).contains(&tint) {
        return Err("Tint must be between 0 and 1.".into());
    }
    let prefix = runtime()?;
    prepare_at(prefix, path, contain, tint, appearance)
}

fn prepare_at(
    prefix: PathBuf,
    path: &Path,
    contain: bool,
    tint: f64,
    appearance: Appearance,
) -> Result<Prepared, String> {
    let bytes =
        crate::typography_preset::read_private_with_limit(path, crate::greeting_image::FILE_LIMIT)?
            .ok_or("The image no longer exists.")?;
    let png = crate::greeting_image::background_png(&bytes)?;
    prepare_png_at(prefix, png, contain, tint, appearance)
}

fn prepare_current(appearance: Appearance) -> Result<Prepared, String> {
    let prefix = runtime()?;
    if !fs::read_to_string(prefix.join("share/termimochi-background-canvas-version"))
        .is_ok_and(|v| v == "1\n")
    {
        return Err("Background runtime needs the framing update. Build it with scripts/build-background-runtime.sh; the system Ptyxis is not used.".into());
    }
    prepare_current_at(prefix, appearance)
}
fn canvas(appearance: &Appearance) -> Result<Vec<u8>, String> {
    let background = appearance
        .background
        .as_ref()
        .ok_or("Choose a background in the theme first.")?;
    let palette = appearance
        .palette
        .as_deref()
        .ok_or("Missing theme palette")?;
    let palette = termimochi_core::PtyxisPalette::from_text(palette).map_err(|e| e.to_string())?;
    let color = palette
        .variant(if appearance.light {
            termimochi_core::Variant::Light
        } else {
            termimochi_core::Variant::Dark
        })
        .and_then(|p| p.get("Background"))
        .unwrap_or(termimochi_core::Rgb::new(0, 0, 0));
    background.canvas(
        appearance.canvas,
        [color.red(), color.green(), color.blue()],
    )
}
fn prepare_current_at(prefix: PathBuf, appearance: Appearance) -> Result<Prepared, String> {
    let png = canvas(&appearance)?;
    let mut prepared = prepare_png_at(prefix, png, false, 0.0, appearance)?;
    prepared
        .env
        .insert("TERMIMOCHI_RESEARCH_CANVAS".into(), "1".into());
    Ok(prepared)
}
fn prepare_png_at(
    prefix: PathBuf,
    png: Vec<u8>,
    contain: bool,
    tint: f64,
    appearance: Appearance,
) -> Result<Prepared, String> {
    let root = tempfile::Builder::new()
        .prefix("tm-bg-")
        .tempdir()
        .map_err(|e| e.to_string())?;
    let base = root.path();
    let mut env = BTreeMap::new();
    for (key, dir) in [
        ("HOME", "home"),
        ("XDG_CONFIG_HOME", "config"),
        ("XDG_DATA_HOME", "data"),
        ("XDG_STATE_HOME", "state"),
        ("XDG_CACHE_HOME", "cache"),
        ("XDG_RUNTIME_DIR", "runtime"),
    ] {
        fs::create_dir(base.join(dir)).map_err(|e| e.to_string())?;
        fs::set_permissions(base.join(dir), fs::Permissions::from_mode(0o700))
            .map_err(|e| e.to_string())?;
        env.insert(key.into(), base.join(dir).to_string_lossy().into_owned());
    }
    fs::write(base.join("background.png"), png).map_err(|e| e.to_string())?;
    fs::write(
        base.join("supervisor.py"),
        include_str!("../resources/native/session-supervisor.py"),
    )
    .map_err(|e| e.to_string())?;
    fs::write(base.join("startup.bash"), "PS1='trial:\\w $ '\nprintf '\\nPtyxis background trial — real shell, private settings.\\nNo personal startup files loaded. Close this window to finish.\\n\\n'\n").map_err(|e| e.to_string())?;
    let command = format!(
        "/bin/bash --noprofile --rcfile {} -i",
        quote(&base.join("startup.bash").to_string_lossy())
    );
    let palette = if let Some(text) = appearance.palette {
        termimochi_core::PtyxisPalette::from_text(&text).map_err(|e| e.to_string())?;
        let palettes = base.join(format!("data/{ID}/palettes"));
        fs::create_dir_all(&palettes).map_err(|e| e.to_string())?;
        fs::write(palettes.join("trial.palette"), text).map_err(|e| e.to_string())?;
        "trial"
    } else {
        "gnome"
    };
    let settings = format!(
        "[{SCHEMA_PATH}]\nprofile-uuids=['{PROFILE}']\ndefault-profile-uuid='{PROFILE}'\nrestore-session=false\nuse-system-font=false\nfont-name={}\ninterface-style='{}'\nwindow-size=({}, {})\n\n[{SCHEMA_PATH}/Profiles/{PROFILE}]\nuse-custom-command=true\ncustom-command={}\nlogin-shell=false\nopacity=1.0\npalette='{palette}'\n",
        variant(&appearance.font),
        if appearance.light { "light" } else { "dark" },
        appearance.columns.clamp(20, 240),
        appearance.rows.clamp(10, 100),
        variant(&command)
    );
    let keyfile = base.join("config/glib-2.0/settings");
    fs::create_dir_all(&keyfile).map_err(|e| e.to_string())?;
    fs::write(keyfile.join("keyfile"), settings).map_err(|e| e.to_string())?;
    let bus = format!(
        "<busconfig><type>session</type><listen>unix:path={}/runtime/bus</listen><auth>EXTERNAL</auth><policy context=\"default\"><allow send_destination=\"*\"/><allow receive_sender=\"*\"/><allow own=\"*\"/></policy></busconfig>",
        base.display()
    );
    fs::write(base.join("bus.conf"), bus).map_err(|e| e.to_string())?;
    for (key, value) in [
        ("GSETTINGS_BACKEND", "keyfile"),
        ("TERMIMOCHI_PRIVATE_SESSION", "1"),
        ("TERMIMOCHI_EXTERNAL_TRIAL", "1"),
        (
            "TERMIMOCHI_RESEARCH_LAYOUT",
            if contain { "contain" } else { "cover" },
        ),
        ("GSK_RENDERER", "gl"),
    ] {
        env.insert(key.into(), value.into());
    }
    env.insert(
        "TERMIMOCHI_RESEARCH_BACKGROUND".into(),
        base.join("background.png").to_string_lossy().into_owned(),
    );
    env.insert("TERMIMOCHI_RESEARCH_TINT".into(), tint.to_string());
    env.insert(
        "GSETTINGS_SCHEMA_DIR".into(),
        prefix
            .join("share/glib-2.0/schemas")
            .to_string_lossy()
            .into_owned(),
    );
    env.insert(
        "XDG_DATA_DIRS".into(),
        format!("{}/share:/usr/local/share:/usr/share", prefix.display()),
    );
    // The compositor socket belongs to the user's desktop; configuration and
    // session bus do not. Never import the desktop bus into the private bus.
    if let Some(display) = std::env::var_os("WAYLAND_DISPLAY") {
        let display = PathBuf::from(display);
        let socket = if display.is_absolute() {
            display
        } else {
            PathBuf::from(
                std::env::var_os("XDG_RUNTIME_DIR").ok_or("Missing desktop runtime directory")?,
            )
            .join(display)
        };
        env.insert(
            "WAYLAND_DISPLAY".into(),
            socket.to_string_lossy().into_owned(),
        );
    }
    Ok(Prepared {
        root,
        executable: prefix.join("bin/ptyxis"),
        env,
    })
}

impl Prepared {
    pub fn start(&self) -> Result<gio::Subprocess, String> {
        self.start_impl("Ptyxis Background Trial", false)
    }
    fn start_named(&self, title: &str) -> Result<gio::Subprocess, String> {
        self.start_impl(title, true)
    }
    fn start_impl(&self, title: &str, clean: bool) -> Result<gio::Subprocess, String> {
        let launcher = gio::SubprocessLauncher::new(gio::SubprocessFlags::STDIN_PIPE);
        if clean {
            // Desktop launches do not pass through Entry::launch(). Apply the
            // same allowlist here too; no inherited Bash hooks/functions.
            for (key, _) in std::env::vars_os() {
                launcher.unsetenv(&key);
            }
            let mut environment = std::process::Command::new("unused");
            crate::kitty_session::clean_environment(&mut environment);
            for (key, value) in environment.get_envs() {
                if let Some(value) = value {
                    launcher.setenv(key, value, true);
                }
            }
            for key in [
                "GDK_BACKEND",
                "LIBGL_ALWAYS_SOFTWARE",
                "GTK_IM_MODULE",
                "XMODIFIERS",
            ] {
                if let Some(value) = std::env::var_os(key) {
                    launcher.setenv(key, value, true);
                }
            }
        }
        launcher.set_cwd(self.root.path().join("home"));
        for key in [
            "DBUS_SESSION_BUS_ADDRESS",
            "DBUS_STARTER_ADDRESS",
            "DBUS_STARTER_BUS_TYPE",
            "AT_SPI_BUS_ADDRESS",
            "WAYLAND_SOCKET",
            "BASH_ENV",
            "ENV",
            "GSETTINGS_SCHEMA_DIR",
            "GSETTINGS_BACKEND",
            "GTK_A11Y",
            "PTYXIS_PROFILE",
        ] {
            launcher.unsetenv(key);
        }
        for (key, value) in &self.env {
            launcher.setenv(key, value, true);
        }
        launcher
            .spawn(&[
                std::ffi::OsStr::new("/usr/bin/python3"),
                self.root.path().join("supervisor.py").as_os_str(),
                self.root.path().as_os_str(),
                self.executable.as_os_str(),
                std::ffi::OsStr::new("--standalone"),
                std::ffi::OsStr::new(&format!("--title={title}")),
            ])
            .map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{DynamicImage, ImageFormat, Rgba, RgbaImage};
    fn saved_appearance() -> Appearance {
        let bg = crate::background::tests::fixture();
        let json = serde_json::to_vec(&bg).unwrap();
        Appearance {
            background: Some(serde_json::from_slice(&json).unwrap()),
            palette: Some(include_str!("../resources/themes/fog-paper.palette").into()),
            light: true,
            canvas: [800, 400],
            ..Default::default()
        }
    }
    #[test]
    fn saved_background_uses_exact_canvas_and_private_settings() {
        let prefix = tempfile::tempdir().unwrap();
        let appearance = saved_appearance();
        let palette =
            termimochi_core::PtyxisPalette::from_text(appearance.palette.as_ref().unwrap())
                .unwrap();
        let c = palette
            .variant(termimochi_core::Variant::Light)
            .unwrap()
            .get("Background")
            .unwrap();
        let expected = appearance
            .background
            .as_ref()
            .unwrap()
            .canvas(appearance.canvas, [c.red(), c.green(), c.blue()])
            .unwrap();
        let prepared = prepare_current_at(prefix.path().into(), appearance).unwrap();
        assert_eq!(
            fs::read(prepared.root.path().join("background.png")).unwrap(),
            expected
        );
        assert_eq!(prepared.env["TERMIMOCHI_RESEARCH_CANVAS"], "1");
        assert_eq!(prepared.env["TERMIMOCHI_RESEARCH_TINT"], "0");
        assert_eq!(prepared.env["GSETTINGS_BACKEND"], "keyfile");
    }
    #[test]
    #[ignore = "private Xvfb/HOME/XDG plus optional enhanced Ptyxis runtime; actual saved background pixels and reopen"]
    fn saved_background_native_ptyxis_and_reopen() {
        assert_eq!(std::env::var("GSETTINGS_BACKEND").as_deref(), Ok("memory"));
        assert!(!matches!(
            std::env::var("DISPLAY").as_deref(),
            Ok(":0" | ":1") | Err(_)
        ));
        struct Child(gio::Subprocess);
        impl Drop for Child {
            fn drop(&mut self) {
                self.0.send_signal(15);
                let _ = self.0.wait(gio::Cancellable::NONE);
            }
        }
        let evidence = PathBuf::from(std::env::var_os("XDG_CACHE_HOME").unwrap());
        for phase in ["saved-background", "reopened-background"] {
            let prepared = prepare_current(saved_appearance()).unwrap();
            let root = prepared.root.path().to_path_buf();
            let child = Child(prepared.start().unwrap());
            let output=std::process::Command::new("/usr/bin/python3").arg("-c").arg(r#"
import os,sys,time,json
from PIL import ImageGrab
deadline=time.monotonic()+20
while True:
    shot=ImageGrab.grab(xdisplay=os.environ['DISPLAY']).convert('RGB')
    colors=shot.getcolors(shot.width*shot.height)
    counts=[sum(n for n,c in colors if max(abs(c[j]-expected[j]) for j in range(3))<4) for expected in [(30,80,150),(170,130,20)]]
    if min(counts)>1000: break
    if time.monotonic()>deadline:
        shot.save(sys.argv[1]); raise AssertionError(('native background missing',counts))
    time.sleep(.2)
shot.save(sys.argv[1]); print(json.dumps({'visible_color_pixels':counts,'screenshot':sys.argv[1]}))
"#).arg(evidence.join(format!("{phase}.png"))).output().unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            println!("{}", String::from_utf8_lossy(&output.stdout));
            drop(child);
            drop(prepared);
            assert!(!root.exists());
        }
    }
    fn png(width: u32, height: u32) -> Vec<u8> {
        let mut buffer = std::io::Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(RgbaImage::from_pixel(
            width,
            height,
            Rgba([200, 50, 25, 128]),
        ))
        .write_to(&mut buffer, ImageFormat::Png)
        .unwrap();
        buffer.into_inner()
    }
    #[test]
    fn background_static_decode_is_bounded_and_preserves_alpha() {
        let result = crate::greeting_image::background_png(&png(4000, 2000)).unwrap();
        let image = image::load_from_memory(&result).unwrap().into_rgba8();
        assert_eq!(image.dimensions(), (1920, 960));
        let pixel = image.get_pixel(400, 400);
        assert!((i32::from(pixel[0]) - 200).abs() <= 2);
        assert_eq!(pixel[3], 128);
        for invalid in [
            b"<svg/>".as_slice(),
            b"GIF89a",
            b"invalid",
            b"\x89PNG\r\n\x1a\n",
        ] {
            assert!(crate::greeting_image::background_png(invalid).is_err());
        }
    }
    #[test]
    fn trial_materializes_only_private_files_and_cleans_up() {
        let input = tempfile::tempdir().unwrap();
        let path = input.path().join("image with spaces.png");
        fs::write(&path, png(20, 10)).unwrap();
        let appearance = Appearance {
            font: "Mono 'quoted' 14".into(),
            ..Default::default()
        };
        let prepared =
            prepare_at(input.path().to_path_buf(), &path, true, 0.35, appearance).unwrap();
        let root = prepared.root.path().to_path_buf();
        assert_ne!(prepared.env["HOME"], std::env::var("HOME").unwrap());
        assert_eq!(prepared.env["GSETTINGS_BACKEND"], "keyfile");
        assert_eq!(prepared.env["TERMIMOCHI_RESEARCH_LAYOUT"], "contain");
        let keyfile = fs::read_to_string(root.join("config/glib-2.0/settings/keyfile")).unwrap();
        assert!(keyfile.contains("--noprofile --rcfile"));
        assert!(keyfile.contains(SCHEMA_PATH));
        assert!(!keyfile.contains("[org/gnome/Ptyxis]"));
        assert!(root.join("background.png").is_file());
        assert_eq!(
            fs::metadata(root.join("runtime"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
        assert!(
            !fs::read_to_string(root.join("bus.conf"))
                .unwrap()
                .contains("servicedir")
        );
        drop(prepared);
        assert!(!root.exists());
        assert_eq!(fs::read(&path).unwrap(), png(20, 10));
    }
    #[test]
    fn workspace_snapshot_preserves_palette_font_and_grid_without_mutation() {
        let palette = termimochi_core::PtyxisPalette::from_text(include_str!(
            "../resources/themes/fog-paper.palette"
        ))
        .unwrap();
        let mut workspace = Workspace::new(
            &palette,
            termimochi_core::Variant::Light,
            Default::default(),
            Default::default(),
            Default::default(),
            None,
            false,
        );
        workspace.layout.columns = 120;
        workspace.layout.rows = 36;
        let before = workspace.clone();
        let appearance = Appearance::from_workspace(&workspace);
        assert_eq!(
            appearance.font,
            workspace.typography.font_description().to_string()
        );
        let input = tempfile::tempdir().unwrap();
        let path = input.path().join("background.png");
        fs::write(&path, png(2, 2)).unwrap();
        let prepared = prepare_at(input.path().into(), &path, false, 0.5, appearance).unwrap();
        assert_eq!(
            fs::read_to_string(
                prepared
                    .root
                    .path()
                    .join(format!("data/{ID}/palettes/trial.palette"))
            )
            .unwrap(),
            workspace.palette
        );
        let settings = fs::read_to_string(
            prepared
                .root
                .path()
                .join("config/glib-2.0/settings/keyfile"),
        )
        .unwrap();
        assert!(settings.contains("window-size=(120, 36)"));
        assert!(settings.contains("interface-style='light'"));
        assert!(settings.contains("palette='trial'"));
        assert_eq!(workspace, before);
    }
    #[test]
    fn trial_rejects_invalid_tint_and_aliased_sources() {
        for tint in [f64::NAN, -0.01, 1.01, f64::INFINITY] {
            assert!(prepare(Path::new("/unused"), false, tint, Default::default()).is_err());
        }
        let input = tempfile::tempdir().unwrap();
        let image = input.path().join("image.png");
        fs::write(&image, png(1, 1)).unwrap();
        let alias = input.path().join("alias.png");
        std::os::unix::fs::symlink(&image, &alias).unwrap();
        assert!(prepare_at(input.path().into(), &alias, false, 0.35, Default::default()).is_err());
        assert!(
            prepare_at(
                input.path().into(),
                &input.path().join("missing"),
                false,
                0.35,
                Default::default()
            )
            .is_err()
        );
    }
}
