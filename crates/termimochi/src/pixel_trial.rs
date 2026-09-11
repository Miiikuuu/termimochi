//! Real-terminal trials are temporary and safe-field-only. Managed installation
//! is a separate reviewed Fastfetch transaction; exports are not capability proof.
use crate::{
    fastfetch_apply,
    greeting::GreetingSettings,
    greeting_image::pixel_export::{self, PixelImage, Protocol},
    typography_preset,
};
use gtk::glib;
use serde::{Deserialize, Serialize};
use std::{
    os::unix::fs::{MetadataExt, PermissionsExt},
    path::{Path, PathBuf},
    process::Command,
};

mod runner;
pub(crate) mod startup;
#[cfg(test)]
pub(crate) mod tests;
pub(crate) use runner::worker_entry;
pub(crate) const WORKER_ARG: &str = "--termimochi-pixel-trial";
const LIMIT: u64 = 40 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum Terminal {
    Kitty,
    Ptyxis,
    Xterm,
}
impl Terminal {
    pub const ALL: [Self; 3] = [Self::Kitty, Self::Ptyxis, Self::Xterm];
    pub fn label(self) -> &'static str {
        match self {
            Self::Kitty => "Kitty",
            Self::Ptyxis => "Ptyxis",
            Self::Xterm => "Xterm",
        }
    }
    fn program(self) -> &'static str {
        match self {
            Self::Kitty => "kitty",
            Self::Ptyxis => "ptyxis",
            Self::Xterm => "xterm",
        }
    }
    pub fn executable(self) -> Option<PathBuf> {
        glib::find_program_in_path(self.program())
    }
    pub fn command(self, executable: &Path, helper: &Path, directory: &Path) -> Command {
        let mut command = Command::new(executable);
        match self {
            Self::Kitty => {
                command.args(["--title", "TermiMochi Image Trial", "--directory", "/"]);
            }
            Self::Ptyxis => {
                command.args([
                    "--new-window",
                    "--title=TermiMochi Image Trial",
                    "--working-directory=/",
                    "--",
                ]);
            }
            Self::Xterm => {
                command.args(["-T", "TermiMochi Image Trial", "-e"]);
            }
        }
        command
            .arg(helper)
            .arg(WORKER_ARG)
            .arg(directory)
            .current_dir("/");
        command.env_remove("BASH_ENV").env_remove("ENV");
        command
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum Support {
    #[default]
    Unverified,
    Advertised,
    Unavailable,
}
impl Support {
    pub fn label(self) -> &'static str {
        match self {
            Self::Unverified => "Unverified",
            Self::Advertised => "Protocol response received · visual check still required",
            Self::Unavailable => "Unavailable in this terminal session",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum Visual {
    #[default]
    Unverified,
    Confirmed,
    Failed,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Assessment {
    pub kitty: Support,
    pub sixel: Support,
    pub animation: Visual,
    pub visual: Visual,
    pub done: bool,
    pub message: String,
    pub cells: Option<[u32; 2]>,
    pub asset_hash: Option<u64>,
    pub config_hash: Option<u64>,
}
impl Default for Assessment {
    fn default() -> Self {
        Self {
            kitty: Support::Unverified,
            sixel: Support::Unverified,
            animation: Visual::Unverified,
            visual: Visual::Unverified,
            done: false,
            message: "Waiting for the target terminal…".into(),
            cells: None,
            asset_hash: None,
            config_hash: None,
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    version: u8,
    protocol: Protocol,
    ansi: bool,
    columns: u32,
    hashes: std::collections::BTreeMap<String, u64>,
}

#[derive(Clone, Copy)]
pub(crate) struct TrialOptions {
    pub protocol: Protocol,
    pub terminal: Terminal,
    pub ansi: bool,
    pub columns: u32,
    pub cells: [u32; 2],
}

fn payload_names(protocol: Protocol) -> Vec<&'static str> {
    let mut names = vec![
        "config.jsonc",
        "config-ansi.jsonc",
        "logo.png",
        protocol.source_name(),
    ];
    names.sort_unstable();
    names.dedup();
    names
}

pub(crate) struct Trial {
    _owner: tempfile::TempDir,
    pub directory: PathBuf,
    pub protocol: Protocol,
    pub terminal: Terminal,
    pub ansi: bool,
}

pub(crate) fn read(path: &Path) -> Result<Vec<u8>, String> {
    typography_preset::read_private_with_limit(path, LIMIT)?
        .ok_or_else(|| format!("Missing trial file: {}", path.display()))
}
fn write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let before = typography_preset::read_private_with_limit(path, LIMIT)?;
    typography_preset::write_checked_with_limit(path, bytes, &before, LIMIT)
}
fn hash(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf29ce484222325u64, |h, b| {
        (h ^ u64::from(*b)).wrapping_mul(0x100000001b3)
    })
}

// Pixel installation must not silently fall back to a builtin logo because an
// imported display.pipe flag (or NO_COLOR) suppressed graphics. Review shows this
// explicit output-setting change; the portable exporter remains unchanged.
fn require_pixel_output(source: &str) -> Result<String, String> {
    use jsonc_parser::cst::CstInputValue;
    let root = crate::fastfetch_document::parse(source)?;
    let object = root.object_value().ok_or("Missing configuration object.")?;
    if let Some(display) = object.get("display") {
        let display = display.object_value().ok_or("Invalid display settings.")?;
        if let Some(pipe) = display.get("pipe") {
            pipe.set_value(CstInputValue::Bool(false));
        } else {
            display.append("pipe", CstInputValue::Bool(false));
        }
    } else {
        object.append(
            "display",
            CstInputValue::Object(vec![("pipe".into(), CstInputValue::Bool(false))]),
        );
    }
    Ok(root.to_string())
}

impl Trial {
    pub fn respond(&self, key: u8) -> Result<(), String> {
        let assessment = self.assessment()?;
        if assessment.done {
            return Err("This trial has finished. Start a fresh trial.".into());
        }
        let stage = if assessment.message.starts_with("No conclusive") {
            "unknown"
        } else if assessment.asset_hash.is_some()
            && (assessment.message.starts_with("Does the animation")
                || assessment.message.starts_with("Is the artwork"))
        {
            "visual"
        } else {
            return Err("Wait until the terminal has rendered before giving feedback.".into());
        };
        if !(stage == "visual" && b"ynq".contains(&key)
            || stage == "unknown" && b"tq".contains(&key))
        {
            return Err("Feedback does not match the trial stage.".into());
        }
        typography_preset::write_checked_with_limit(
            &self.directory.join("gui-response"),
            format!("{stage}:{}", key as char).as_bytes(),
            &None,
            32,
        )
    }
    pub fn prepare(
        parent: &Path,
        settings: &GreetingSettings,
        image: &PixelImage,
        options: TrialOptions,
    ) -> Result<Self, String> {
        let TrialOptions {
            protocol,
            terminal,
            ansi,
            columns,
            cells,
        } = options;
        // ANSI fallback must work even if generation of a pixel stream fails.
        let protocol = if ansi { Protocol::Kitty } else { protocol };
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        let owner = tempfile::Builder::new()
            .prefix("trial-")
            .permissions(std::fs::Permissions::from_mode(0o700))
            .tempdir_in(parent)
            .map_err(|e| e.to_string())?;
        let directory =
            pixel_export::export_bundle(owner.path(), settings, image, protocol, columns, cells)?;
        let config =
            String::from_utf8(read(&directory.join("config.jsonc"))?).map_err(|e| e.to_string())?;
        let config =
            pixel_export::absolute_configuration(&config, &directory.join(protocol.source_name()))?;
        let config = require_pixel_output(&config)?;
        let config = if !ansi && protocol != Protocol::Sixel {
            #[cfg(test)]
            let helper = std::env::var_os("TERMIMOCHI_SVG_WORKER_BIN")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("/test/termimochi"));
            #[cfg(not(test))]
            let helper = std::env::current_exe().map_err(|e| e.to_string())?;
            startup::configuration(&config, &helper)?
        } else {
            config
        };
        write(&directory.join("config.jsonc"), config.as_bytes())?;
        write(
            &directory.join("config-ansi.jsonc"),
            settings.fastfetch_config()?.as_bytes(),
        )?;
        write(
            &directory.join("request.json"),
            &serde_json::to_vec(&Request {
                version: 1,
                protocol,
                ansi,
                columns,
                hashes: payload_names(protocol)
                    .into_iter()
                    .map(|name| Ok((name.to_owned(), hash(&read(&directory.join(name))?))))
                    .collect::<Result<_, String>>()?,
            })
            .map_err(|e| e.to_string())?,
        )?;
        Ok(Self {
            _owner: owner,
            directory,
            protocol,
            terminal,
            ansi,
        })
    }

    pub fn launch(&self, helper: &Path) -> Result<(), String> {
        let executable = self
            .terminal
            .executable()
            .ok_or_else(|| format!("{} is not installed.", self.terminal.label()))?;
        if !Path::new("/usr/bin/fastfetch").is_file() {
            return Err("System Fastfetch is required for a real terminal trial.".into());
        }
        let mut child = self
            .terminal
            .command(&executable, helper, &self.directory)
            .spawn()
            .map_err(|e| e.to_string())?;
        std::thread::spawn(move || {
            let _ = child.wait();
        });
        Ok(())
    }

    pub fn assessment(&self) -> Result<Assessment, String> {
        match typography_preset::read_private(&self.directory.join("result.json"))? {
            Some(bytes) => serde_json::from_slice(&bytes).map_err(|e| e.to_string()),
            None => Ok(Assessment::default()),
        }
    }

    pub fn install_plan(
        &self,
        root: &Path,
        target: fastfetch_apply::Target,
    ) -> Result<InstallPlan, String> {
        let assessment = self.assessment()?;
        if !assessment.done || assessment.visual != Visual::Confirmed {
            return Err("First confirm that this exact output looks correct in the target terminal. Export alone is not verification.".into());
        }
        let names = if self.ansi {
            vec!["config-ansi.jsonc"]
        } else {
            let mut names = vec!["logo.png", self.protocol.source_name(), "config-ansi.jsonc"];
            names.sort_unstable();
            names.dedup();
            names
        };
        let mut assets = Vec::new();
        for name in names {
            assets.push((name.to_owned(), read(&self.directory.join(name))?));
        }
        let tested = read(&self.directory.join(if self.ansi {
            "config-ansi.jsonc"
        } else {
            self.protocol.source_name()
        }))?;
        if assessment.asset_hash != Some(hash(&tested)) {
            return Err("Artwork changed after testing. Run the trial again.".into());
        }
        let tested_config = read(&self.directory.join(if self.ansi {
            "config-ansi.jsonc"
        } else {
            "config.jsonc"
        }))?;
        if assessment.config_hash != Some(hash(&tested_config)) {
            return Err("Configuration changed after testing. Run the trial again.".into());
        }
        let name = format!(
            "{}-{}",
            self._owner.path().file_name().unwrap().to_string_lossy(),
            self.directory.file_name().unwrap().to_string_lossy()
        );
        let directory = root.join(name);
        target.check()?;
        let config = if self.ansi {
            String::from_utf8(tested).map_err(|e| e.to_string())?
        } else {
            let source = String::from_utf8(tested_config).map_err(|e| e.to_string())?;
            pixel_export::absolute_configuration(
                &source,
                &directory.join(self.protocol.source_name()),
            )?
        };
        Ok(InstallPlan {
            directory,
            target,
            config,
            assets,
        })
    }
}

pub(crate) struct InstallPlan {
    pub directory: PathBuf,
    pub target: fastfetch_apply::Target,
    pub config: String,
    assets: Vec<(String, Vec<u8>)>,
}
impl InstallPlan {
    /// Reuse verified pixels/layout, but review the current complete fields.
    /// Trial confirmation never authorizes imported commands or a destination.
    pub fn with_current_fields(mut self, current: &str) -> Result<Self, String> {
        use jsonc_parser::cst::CstInputValue;
        let tested = crate::fastfetch_document::value(&self.config)?;
        let root = crate::fastfetch_document::parse(current)?;
        let object = root.object_value().ok_or("Missing configuration object.")?;
        fn input(v: &serde_json::Value) -> CstInputValue {
            match v {
                serde_json::Value::Null => CstInputValue::Null,
                serde_json::Value::Bool(v) => CstInputValue::Bool(*v),
                serde_json::Value::Number(v) => CstInputValue::Number(v.to_string()),
                serde_json::Value::String(v) => CstInputValue::String(v.clone()),
                serde_json::Value::Array(v) => CstInputValue::Array(v.iter().map(input).collect()),
                serde_json::Value::Object(v) => {
                    CstInputValue::Object(v.iter().map(|(k, v)| (k.clone(), input(v))).collect())
                }
            }
        }
        let logo = input(&tested["logo"]);
        if let Some(old) = object.get("logo") {
            old.set_value(logo);
        } else {
            object.append("logo", logo);
        }
        let mut config = require_pixel_output(&root.to_string())?;
        if let Some(guard) = tested
            .get("general")
            .and_then(|g| g.get("preRun"))
            .and_then(|v| v.as_str())
        {
            let root = crate::fastfetch_document::parse(&config)?;
            let object = root.object_value().unwrap();
            if object.get("general").is_none() {
                object.append("general", CstInputValue::Object(vec![]));
            }
            let general = object
                .get("general")
                .and_then(|p| p.object_value())
                .ok_or("Invalid general settings")?;
            let guard = guard.split(";\n").next().unwrap_or(guard);
            let values = crate::fastfetch_document::value(current)?;
            let previous = values
                .get("general")
                .and_then(|g| g.get("preRun"))
                .map(|v| {
                    v.as_str()
                        .ok_or("general.preRun must be text; it has not been replaced.")
                })
                .transpose()?
                .unwrap_or("");
            let command = if previous == guard || previous.starts_with(&format!("{guard};\n")) {
                previous.to_owned()
            } else if previous.is_empty() {
                guard.to_owned()
            } else {
                format!("{guard};\n{previous}")
            };
            if let Some(p) = general.get("preRun") {
                p.set_value(CstInputValue::String(command));
            } else {
                general.append("preRun", CstInputValue::String(command));
            }
            config = root.to_string();
        }
        self.config = config;
        Ok(self)
    }
    pub fn apply(mut self, state: &Path) -> Result<fastfetch_apply::Target, String> {
        self.target.check()?;
        crate::fastfetch_document::parse(&self.config)?;
        std::fs::create_dir_all(self.directory.parent().ok_or("Missing asset parent.")?)
            .map_err(|e| e.to_string())?;
        use std::os::unix::fs::DirBuilderExt;
        if self.directory.exists() {
            let metadata = std::fs::symlink_metadata(&self.directory).map_err(|e| e.to_string())?;
            if !metadata.is_dir() || metadata.mode() & 0o077 != 0 {
                return Err("Managed asset directory changed.".into());
            }
            for (name, bytes) in &self.assets {
                if read(&self.directory.join(name))? != *bytes {
                    return Err("Managed artwork changed externally; nothing was applied.".into());
                }
            }
        } else {
            std::fs::DirBuilder::new()
                .mode(0o700)
                .create(&self.directory)
                .map_err(|e| e.to_string())?;
        }
        // Retain immutable assets on any uncertain failure: the active file or a
        // backup may reference them. Never garbage-collect assets during restore.
        let result = (|| {
            for (name, bytes) in self.assets {
                if self.directory.join(&name).exists() {
                    continue;
                }
                typography_preset::write_checked_with_limit(
                    &self.directory.join(name),
                    &bytes,
                    &None,
                    LIMIT,
                )?;
            }
            // A retained bundle remains immutable across later field edits.
            if !self.directory.join("config.jsonc").exists() {
                write(&self.directory.join("config.jsonc"), self.config.as_bytes())?;
            }
            fastfetch_apply::apply(&mut self.target, &self.config, state)?;
            Ok(self.target)
        })();
        result.map_err(|e: String| {
            format!(
                "{e}\nManaged assets retained at {}. Existing configuration backups are kept.",
                self.directory.display()
            )
        })
    }
}

pub(crate) fn validate_directory(directory: &Path) -> Result<(), String> {
    let metadata = std::fs::symlink_metadata(directory).map_err(|e| e.to_string())?;
    if !directory.is_absolute()
        || !metadata.is_dir()
        || !directory
            .file_name()
            .and_then(|s| s.to_str())
            .is_some_and(|s| s.starts_with("termimochi-image-"))
        || !directory
            .parent()
            .and_then(Path::file_name)
            .and_then(|s| s.to_str())
            .is_some_and(|s| s.starts_with("trial-"))
        || metadata.mode() & 0o077 != 0
        || std::fs::canonicalize(directory).map_err(|e| e.to_string())? != directory
    {
        return Err("Expected a private, real trial directory.".into());
    }
    Ok(())
}
