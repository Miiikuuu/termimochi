//! Read-only import, rendered by Starship in a read-only, offline sandbox.
//! Only reviewed built-in modules and declarative options cross the boundary.

use std::{
    collections::BTreeSet,
    env, fs,
    io::Read,
    path::{Path, PathBuf},
    process::Command,
    time::{Duration, Instant},
};

use gtk::glib;
use toml::{Table, Value};

use crate::preview_context::run_bounded;

const CONFIG_LIMIT: u64 = 256 * 1024;
const MODULES: &[&str] = &[
    "username",
    "hostname",
    "directory",
    "git_branch",
    "git_status",
    "golang",
    "nodejs",
    "python",
    "rust",
    "conda",
    "cmd_duration",
    "line_break",
    "jobs",
    "time",
    "status",
    "character",
];

#[derive(Clone, Debug)]
pub(crate) struct ImportedPrompt {
    pub(crate) path: PathBuf,
    pub(crate) source: Option<String>,
    pub(crate) ansi: Option<String>,
    pub(crate) detail: String,
}

impl ImportedPrompt {
    pub(crate) fn load(directory: &Path, columns: usize) -> Self {
        let path = config_path(
            env::var_os("STARSHIP_CONFIG").as_deref().map(Path::new),
            &glib::user_config_dir(),
            &env::current_dir().unwrap_or_else(|_| glib::home_dir()),
        );
        Self::from_path(path, directory, columns)
    }

    fn from_path(path: PathBuf, directory: &Path, columns: usize) -> Self {
        let source = read_config(&path);
        let result = source.as_ref().map_err(Clone::clone).and_then(|source| {
            let (safe, notices) = prepare_config(source)?;
            let ansi = render(&safe, directory, columns)?;
            let mut detail = "Read-only Starship configuration. Previous command status, duration and jobs are not available from another shell.".to_owned();
            if !notices.is_empty() {
                detail.push_str("\nSkipped for this preview: ");
                detail.push_str(&notices.join(", "));
                detail.push('.');
            }
            Ok((ansi, detail))
        });
        match result {
            Ok((ansi, detail)) => Self {
                path,
                source: source.ok(),
                ansi: Some(ansi),
                detail,
            },
            Err(detail) => Self {
                path,
                source: source.ok(),
                ansi: None,
                detail,
            },
        }
    }

    pub(crate) fn label(&self) -> &'static str {
        if self.ansi.is_some() {
            "Your Starship · Read-only"
        } else {
            "Starship preview unavailable"
        }
    }
}

/// Render edits through exactly the same allowlist/sandbox as imported data.
/// The filtered text is for preview only, never the document to export.
pub(crate) fn render_copy(
    source: &str,
    directory: &Path,
    columns: usize,
    scenario: u32,
) -> Result<(String, Vec<String>), String> {
    let (safe, notices) = prepare_config(source)?;
    let fixture = match scenario {
        0 => None,
        1 => Some((
            "Cargo.toml",
            "[package]\nname = 'rust-preview'\nversion = '0.1.0'\nedition = '2024'\n",
        )),
        2 => Some((
            "package.json",
            "{\"name\":\"node-preview\",\"version\":\"0.1.0\"}\n",
        )),
        3 => Some((
            "pyproject.toml",
            "[project]\nname = 'python-preview'\nversion = '0.1.0'\n",
        )),
        4 => Some(("go.mod", "module example.com/preview\n\ngo 1.18\n")),
        _ => return Err("Unknown preview sample.".into()),
    };
    let sample = if let Some((name, contents)) = fixture {
        let sample = tempfile::Builder::new()
            .prefix("termimochi-language-sample-")
            .tempdir()
            .map_err(|e| e.to_string())?;
        fs::write(sample.path().join(name), contents).map_err(|e| e.to_string())?;
        Some(sample)
    } else {
        None
    };
    render_at(
        &safe,
        sample.as_ref().map_or(directory, |s| s.path()),
        columns,
        sample
            .as_ref()
            .map(|_| Path::new("/tmp/termimochi-language-preview")),
    )
    .map(|ansi| (ansi, notices))
}

fn config_path(explicit: Option<&Path>, config_dir: &Path, cwd: &Path) -> PathBuf {
    match explicit.filter(|path| !path.as_os_str().is_empty()) {
        Some(path) if path.is_absolute() => path.to_owned(),
        Some(path) => cwd.join(path),
        None => config_dir.join("starship.toml"),
    }
}

fn read_config(path: &Path) -> Result<String, String> {
    if !fs::metadata(path)
        .map_err(|error| format!("Cannot read starship.toml: {error}"))?
        .is_file()
    {
        return Err("Starship configuration must be a regular file.".to_owned());
    }
    let file =
        fs::File::open(path).map_err(|error| format!("Cannot read starship.toml: {error}"))?;
    if !file.metadata().is_ok_and(|metadata| metadata.is_file()) {
        return Err("Starship configuration must be a regular file.".to_owned());
    }
    let mut bytes = Vec::new();
    file.take(CONFIG_LIMIT + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("Cannot read Starship configuration: {error}"))?;
    if bytes.len() as u64 > CONFIG_LIMIT {
        return Err("Starship configuration exceeds the 256 KiB preview limit.".to_owned());
    }
    String::from_utf8(bytes).map_err(|_| "Starship configuration is not UTF-8.".to_owned())
}

fn prepare_config(source: &str) -> Result<(String, Vec<String>), String> {
    let original: Table =
        toml::from_str(source).map_err(|error| format!("Invalid Starship TOML: {error}"))?;
    let mut safe = Table::new();
    let mut skipped = BTreeSet::new();
    for key in ["palette", "palettes", "add_newline"] {
        if let Some(value) = original.get(key) {
            safe.insert(key.to_owned(), value.clone());
        }
    }
    let format = match original.get("format") {
        Some(Value::String(format)) if !format.is_empty() => format.as_str(),
        None | Some(Value::String(_)) => "$all",
        _ => return Err("Starship format must be a string.".to_owned()),
    };
    safe.insert(
        "format".to_owned(),
        Value::String(filter_format(format, &mut skipped)),
    );
    safe.insert("right_format".to_owned(), Value::String(String::new()));
    if original
        .get("right_format")
        .and_then(Value::as_str)
        .is_some_and(|value| !value.is_empty())
    {
        skipped.insert("right prompt".to_owned());
    }
    safe.insert("scan_timeout".to_owned(), Value::Integer(100));
    safe.insert("command_timeout".to_owned(), Value::Integer(250));
    safe.insert("follow_symlinks".to_owned(), Value::Boolean(false));
    for name in MODULES {
        let mut module = Table::new();
        if let Some(value) = original.get(*name) {
            let values = value
                .as_table()
                .ok_or_else(|| format!("Starship [{name}] must be a table."))?;
            for (key, value) in values {
                if safe_option(key) {
                    module.insert(key.clone(), value.clone());
                } else {
                    skipped.insert(format!("{name}.{key}"));
                }
            }
        }
        if *name == "cmd_duration" {
            module.insert("show_notifications".to_owned(), Value::Boolean(false));
        }
        if *name == "python" {
            // Never accept python_binary or pyenv commands from imported TOML.
            module.insert(
                "python_binary".to_owned(),
                Value::Array(vec![Value::String("python3".to_owned())]),
            );
            module.insert("pyenv_version_name".to_owned(), Value::Boolean(false));
        }
        safe.insert((*name).to_owned(), Value::Table(module));
    }
    if original.contains_key("custom") {
        skipped.insert("custom command modules".to_owned());
    }
    toml::to_string(&safe)
        .map(|text| (text, skipped.into_iter().collect()))
        .map_err(|error| format!("Cannot prepare Starship preview: {error}"))
}

fn safe_option(key: &str) -> bool {
    matches!(
        key,
        "disabled"
            | "format"
            | "symbol"
            | "style"
            | "style_user"
            | "style_root"
            | "show_always"
            | "ssh_only"
            | "ssh_symbol"
            | "trim_at"
            | "aliases"
            | "success_symbol"
            | "error_symbol"
            | "vimcmd_symbol"
            | "vimcmd_replace_one_symbol"
            | "vimcmd_replace_symbol"
            | "vimcmd_visual_symbol"
            | "truncation_length"
            | "truncate_to_repo"
            | "truncation_symbol"
            | "home_symbol"
            | "read_only"
            | "read_only_style"
            | "repo_root_style"
            | "before_repo_root_style"
            | "repo_root_format"
            | "substitutions"
            | "fish_style_pwd_dir_length"
            | "use_logical_path"
            | "use_os_path_sep"
            | "only_attached"
            | "always_show_remote"
            | "ignore_branches"
            | "conflicted"
            | "ahead"
            | "behind"
            | "diverged"
            | "up_to_date"
            | "untracked"
            | "stashed"
            | "modified"
            | "staged"
            | "renamed"
            | "deleted"
            | "typechanged"
            | "ignore_submodules"
            | "version_format"
            | "detect_extensions"
            | "detect_files"
            | "detect_folders"
            | "detect_env_vars"
            | "not_capable_style"
            | "ignore_base"
            | "min_time"
            | "show_milliseconds"
            | "number_threshold"
            | "symbol_threshold"
            | "time_format"
            | "utc_time_offset"
            | "time_range"
            | "use_12hr"
            | "map_symbol"
            | "recognize_signal_code"
            | "pipestatus"
            | "pipestatus_format"
            | "pipestatus_separator"
            | "success_style"
            | "failure_style"
            | "not_executable_symbol"
            | "not_found_symbol"
            | "sigint_symbol"
            | "signal_symbol"
    )
}

/// Filter top-level variables, including braced names, without changing
/// Starship's styling, conditional groups, escaped literals or line breaks.
pub(crate) fn filter_format(format: &str, skipped: &mut BTreeSet<String>) -> String {
    let mut out = String::new();
    let mut all_positions = Vec::new();
    let mut explicit = BTreeSet::new();
    let mut chars = format.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            out.push(ch);
            if let Some(next) = chars.next() {
                out.push(next);
            }
        } else if ch == '$' {
            let braced = chars.peek() == Some(&'{');
            let mut name = String::new();
            if braced {
                chars.next();
                for next in chars.by_ref() {
                    if next == '}' {
                        break;
                    }
                    name.push(next);
                }
            } else {
                while chars
                    .peek()
                    .is_some_and(|next| next.is_ascii_alphanumeric() || *next == '_')
                {
                    name.push(chars.next().unwrap());
                }
            }
            if name == "all" {
                all_positions.push(out.len());
                skipped.insert("unreviewed default modules in $all".to_owned());
            } else if MODULES.contains(&name.as_str()) {
                explicit.insert(name.clone());
                out.push_str("${");
                out.push_str(&name);
                out.push('}');
            } else if !name.is_empty() {
                skipped.insert(name);
            } else {
                out.push_str("\\$");
            }
        } else {
            out.push(ch);
        }
    }
    let all = MODULES
        .iter()
        .filter(|name| !explicit.contains(**name))
        .map(|name| format!("${name}"))
        .collect::<String>();
    for position in all_positions.into_iter().rev() {
        out.insert_str(position, &all);
    }
    out
}

fn render(config: &str, directory: &Path, columns: usize) -> Result<String, String> {
    render_at(config, directory, columns, None)
}

fn render_at(
    config: &str,
    directory: &Path,
    columns: usize,
    sample_path: Option<&Path>,
) -> Result<String, String> {
    render_sandbox(config, directory, columns, sample_path, false)
}

/// Only accepts internally generated declarative scenes; never the copied
/// user's TOML. Uses the same offline/read-only process and ANSI sanitizer.
pub(crate) fn render_scene(
    scene: &crate::starship_scene::CommandScene,
    directory: &Path,
    columns: usize,
) -> Result<String, String> {
    let config = scene.config.as_ref().map_err(Clone::clone)?;
    let parsed: Table = toml::from_str(config).map_err(|e| e.to_string())?;
    if parsed
        .get("format")
        .and_then(Value::as_str)
        .is_none_or(str::is_empty)
    {
        return Ok(String::new());
    }
    render_sandbox(config, directory, columns, None, true)
}

fn render_sandbox(
    config: &str,
    directory: &Path,
    columns: usize,
    sample_path: Option<&Path>,
    allow_empty: bool,
) -> Result<String, String> {
    let sandbox_directory = sample_path.unwrap_or(directory);
    let starship = trusted_program("starship", directory)
        .ok_or("Starship is not installed. Install it to preview your configuration.")?;
    let bwrap = trusted_program("bwrap", directory).ok_or(
        "Bubblewrap is unavailable; imported prompts are not run without the read-only sandbox.",
    )?;
    let scratch = tempfile::Builder::new()
        .prefix("termimochi-starship-")
        .tempdir()
        .map_err(|error| format!("Cannot create isolated Starship preview: {error}"))?;
    let config_path = scratch.path().join("starship.toml");
    fs::write(&config_path, config).map_err(|error| format!("Cannot prepare preview: {error}"))?;
    let mut command = Command::new(bwrap);
    command
        .env_clear()
        .args([
            "--die-with-parent",
            "--new-session",
            "--unshare-net",
            "--unshare-pid",
            "--unshare-ipc",
            "--unshare-uts",
            "--cap-drop",
            "ALL",
        ])
        .args([
            "--ro-bind",
            "/",
            "/",
            "--proc",
            "/proc",
            "--dev",
            "/dev",
            "--tmpfs",
            "/tmp",
            "--tmpfs",
            "/run",
        ]);
    // A selected /tmp project must remain readable after hiding the host's
    // temporary files. It is still mounted read-only.
    if directory.starts_with("/tmp") && directory != Path::new("/tmp") {
        command
            .arg("--ro-bind")
            .arg(directory)
            .arg(sandbox_directory);
    }
    command
        .arg("--ro-bind")
        .arg(&config_path)
        .arg("/tmp/starship.toml")
        .arg("--chdir")
        .arg(sandbox_directory)
        .arg("--")
        .arg(starship)
        .args(["prompt", "--terminal-width"])
        .arg(columns.clamp(12, 500).to_string())
        .arg("--path")
        .arg(sandbox_directory)
        .env("HOME", glib::home_dir())
        .env("USER", glib::user_name())
        .env("LOGNAME", glib::user_name())
        .env("PATH", safe_path(directory))
        .env("TERM", "xterm-256color")
        .env("COLORTERM", "truecolor")
        .env("LANG", "C.UTF-8")
        .env("LC_ALL", "C.UTF-8")
        .env("STARSHIP_CONFIG", "/tmp/starship.toml")
        .env("STARSHIP_CACHE", "/tmp/starship-cache")
        .env("STARSHIP_LOG", "error")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_COUNT", "2")
        .env("GIT_CONFIG_KEY_0", "core.fsmonitor")
        .env("GIT_CONFIG_VALUE_0", "false")
        .env("GIT_CONFIG_KEY_1", "core.untrackedCache")
        .env("GIT_CONFIG_VALUE_1", "false")
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("RUSTUP_AUTO_INSTALL", "0")
        .env("GOTOOLCHAIN", "local")
        .env("PYTHONSAFEPATH", "1")
        .env("PYTHONNOUSERSITE", "1");
    for key in [
        "VIRTUAL_ENV",
        "CONDA_DEFAULT_ENV",
        "CONDA_PREFIX",
        "CONDA_SHLVL",
        "SSH_CONNECTION",
        "SSH_CLIENT",
        "SSH_TTY",
        "RUSTUP_HOME",
        "RUSTUP_TOOLCHAIN",
        "CARGO_HOME",
    ] {
        if let Some(value) = env::var_os(key) {
            command.env(key, value);
        }
    }
    let output = run_bounded(&mut command, Instant::now() + Duration::from_millis(1500)).map_err(
        |error| {
            format!(
                "Starship sandbox could not render ({error:?}). No unsandboxed fallback was run."
            )
        },
    )?;
    // Shell command substitution strips final newlines before assigning PS1.
    // Preserve leading/interior line breaks and the character's trailing space.
    let ansi = terminal_safe_ansi(output.trim_end_matches('\n'));
    if ansi.trim().is_empty() && !allow_empty {
        return Err(
            "Starship returned an empty prompt. Check its format or use Designer.".to_owned(),
        );
    }
    Ok(ansi)
}

fn safe_path(directory: &Path) -> std::ffi::OsString {
    let project = directory
        .canonicalize()
        .unwrap_or_else(|_| directory.to_owned());
    let paths = env::var_os("PATH").unwrap_or_default();
    env::join_paths(env::split_paths(&paths).filter(|path| {
        path.is_absolute()
            && path
                .canonicalize()
                .is_ok_and(|path| !path.starts_with(&project))
    }))
    .unwrap_or_default()
}

fn trusted_program(name: &str, directory: &Path) -> Option<PathBuf> {
    let project = directory
        .canonicalize()
        .unwrap_or_else(|_| directory.to_owned());
    env::split_paths(&safe_path(directory))
        .map(|path| path.join(name))
        .find(|path| {
            path.is_file()
                && path
                    .canonicalize()
                    .is_ok_and(|path| !path.starts_with(&project))
        })
}
#[cfg(test)]
pub(crate) fn can_render_samples(directory: &Path) -> bool {
    trusted_program("starship", directory).is_some()
        && trusted_program("bwrap", directory).is_some()
}

/// VTE receives only text, LF and SGR color/style sequences. Imported strings
/// cannot set titles, inject clipboard OSCs, move the cursor or erase content.
fn terminal_safe_ansi(output: &str) -> String {
    let mut result = String::new();
    let mut chars = output.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\u{1b}' {
            match chars.next() {
                Some('[') => {
                    let mut sequence = String::new();
                    for ch in chars.by_ref() {
                        if ('@'..='~').contains(&ch) {
                            if ch == 'm'
                                && sequence.len() <= 128
                                && sequence
                                    .chars()
                                    .all(|c| c.is_ascii_digit() || c == ';' || c == ':')
                            {
                                result.push_str("\x1b[");
                                result.push_str(&sequence);
                                result.push('m');
                            }
                            break;
                        }
                        sequence.push(ch);
                    }
                }
                Some(']' | 'P' | '_' | '^' | 'X') => {
                    let mut escape = false;
                    for ch in chars.by_ref() {
                        if ch == '\u{7}' || (escape && ch == '\\') {
                            break;
                        }
                        escape = ch == '\u{1b}';
                    }
                }
                _ => {}
            }
        } else if ch == '\n' {
            result.push_str("\r\n");
        } else if !ch.is_control()
            && !matches!(ch, '\u{061c}' | '\u{200e}'..='\u{200f}' | '\u{2028}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
        {
            result.push(ch);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edited_rust_symbols_render_literally_without_executing_custom_modules() {
        use crate::starship_draft::{ModuleEdit, StarshipDraft};
        let temp = tempfile::tempdir().unwrap();
        if trusted_program("starship", temp.path()).is_none()
            || trusted_program("bwrap", temp.path()).is_none()
        {
            return;
        }
        let marker = temp.path().join("must-not-run");
        let source = format!(
            "format = '$rust${{custom.danger}}'\n[rust]\nsymbol = 'rs '\nformat = '[$symbol$version]($style)'\n[custom.danger]\nwhen = true\ncommand = \"touch {}\"\n",
            marker.display()
        );
        let mut draft = StarshipDraft::new(temp.path().join("source.toml"), source).unwrap();
        for symbol in [" rs ", " 🦀 ", "[$x](red)\\' "] {
            draft.edit(ModuleEdit::Symbol(symbol.into())).unwrap();
            let (ansi, notices) = render_copy(draft.contents(), temp.path(), 80, 1).unwrap();
            assert!(
                ansi.contains(symbol),
                "{symbol:?} was not literal: {ansi:?}"
            );
            assert!(!notices.is_empty());
            assert!(!marker.exists());
            assert!(draft.contents().contains("[custom.danger]"));
        }
    }

    const CUTE: &str = r##"
palette = "cute"
format = "[╭─](bold pink)$directory$python${custom.danger}\n[╰─](bold pink)$character\n"
add_newline = true
[palettes.cute]
pink = "#f275a0"
lavender = "#c4a7e7"
[directory]
style = "bold lavender"
truncation_length = 3
truncate_to_repo = false
[character]
success_symbol = "[❯](bold pink)"
error_symbol = "[❯](bold red)"
[python]
symbol = " "
style = "bold lavender"
format = "[$symbol$version]($style)"
python_binary = "/untrusted-python"
[custom.danger]
when = "touch /never-run-this"
command = "touch /never-run-this-either"
shell = ["bash", "-c"]
"##;

    #[test]
    fn config_location_honors_explicit_relative_and_xdg_paths() {
        let cwd = Path::new("/workspace");
        let xdg = Path::new("/settings");
        assert_eq!(
            config_path(None, xdg, cwd),
            Path::new("/settings/starship.toml")
        );
        assert_eq!(
            config_path(Some(Path::new("")), xdg, cwd),
            Path::new("/settings/starship.toml")
        );
        assert_eq!(
            config_path(Some(Path::new("my.toml")), xdg, cwd),
            Path::new("/workspace/my.toml")
        );
        assert_eq!(
            config_path(Some(Path::new("/absolute.toml")), xdg, cwd),
            Path::new("/absolute.toml")
        );
    }

    #[test]
    fn import_preserves_declarative_appearance_and_strips_execution_options() {
        let (safe, notes) = prepare_config(CUTE).unwrap();
        let table: Table = toml::from_str(&safe).unwrap();
        assert_eq!(table["palette"].as_str(), Some("cute"));
        assert_eq!(table["palettes"]["cute"]["pink"].as_str(), Some("#f275a0"));
        assert_eq!(table["directory"]["style"].as_str(), Some("bold lavender"));
        assert!(
            table["format"]
                .as_str()
                .unwrap()
                .contains("[╰─](bold pink)${character}")
        );
        assert!(!safe.contains("touch"));
        assert!(!safe.contains("/untrusted-python"));
        assert!(!table.contains_key("custom"));
        assert!(notes.contains(&"custom command modules".to_owned()));
        assert!(notes.contains(&"python.python_binary".to_owned()));
        assert!(
            !table["cmd_duration"]["show_notifications"]
                .as_bool()
                .unwrap()
        );
    }

    #[test]
    fn top_level_variables_cannot_enable_unreviewed_modules() {
        let mut notes = BTreeSet::new();
        assert_eq!(
            filter_format(
                r"\$aws$directory${custom.test}$env_var$aws$character",
                &mut notes
            ),
            r"\$aws${directory}${character}"
        );
        assert!(notes.contains("aws"));
        assert!(notes.contains("custom.test"));
        assert!(notes.contains("env_var"));
        let all = filter_format("$all", &mut BTreeSet::new());
        assert!(!all.contains("$custom"));
        assert!(!all.contains("$aws"));
        assert!(all.ends_with("$character"));
        let rearranged = filter_format("$all$directory$character", &mut BTreeSet::new());
        assert_eq!(rearranged.matches("directory").count(), 1);
        assert_eq!(rearranged.matches("character").count(), 1);
    }

    #[test]
    fn terminal_filter_retains_truecolor_and_removes_non_style_controls() {
        let malicious = "\x1b[1;38;2;242;117;160m╭─\x1b[0m\n\x1b]52;c;c2FmZQ==\x07safe\x1b[2J\x1b[H\u{202e}text";
        assert_eq!(
            terminal_safe_ansi(malicious),
            "\x1b[1;38;2;242;117;160m╭─\x1b[0m\r\nsafetext"
        );
        assert_eq!(
            terminal_safe_ansi("a\x1b]0;title\x1b\\b\x1bPignored\x1b\\c"),
            "abc"
        );
    }

    #[test]
    fn malformed_oversized_and_non_file_configs_fail_without_rendering() {
        assert!(prepare_config("format = [").is_err());
        assert!(prepare_config("format = 42").is_err());
        assert!(prepare_config("[directory]\nstyle='blue'\n[character]\ndisabled=true").is_ok());
        let temp = tempfile::tempdir().unwrap();
        assert!(read_config(temp.path()).is_err());
        assert!(read_config(&temp.path().join("absent.toml")).is_err());
        let large = temp.path().join("large.toml");
        fs::write(&large, vec![b' '; CONFIG_LIMIT as usize + 1]).unwrap();
        assert!(read_config(&large).unwrap_err().contains("256 KiB"));
    }

    #[test]
    fn real_renderer_preserves_cute_colors_and_never_executes_imported_commands() {
        let temp = tempfile::tempdir().unwrap();
        let project = temp.path().join("project");
        fs::create_dir(&project).unwrap();
        fs::write(project.join("pyproject.toml"), "").unwrap();
        if trusted_program("starship", &project).is_none()
            || trusted_program("bwrap", &project).is_none()
        {
            eprintln!("Skipping imported renderer integration: Starship/Bubblewrap not installed");
            return;
        }
        let marker = temp.path().join("must-not-exist");
        let source = CUTE
            .replace("/never-run-this-either", &marker.to_string_lossy())
            .replace("/never-run-this", &marker.to_string_lossy());
        let path = temp.path().join("original.toml");
        fs::write(&path, &source).unwrap();
        let imported = ImportedPrompt::from_path(path.clone(), &project, 60);
        let ansi = imported.ansi.as_ref().expect(&imported.detail);
        assert!(ansi.contains("╭─"), "{ansi:?}");
        assert!(ansi.contains("╰─"), "{ansi:?}");
        assert!(ansi.contains("❯"), "{ansi:?}");
        assert!(ansi.contains("38;2;242;117;160"), "{ansi:?}");
        assert!(ansi.contains("38;2;196;167;231"), "{ansi:?}");
        assert!(ansi.contains("\r\n"));
        assert!(
            !ansi.ends_with("\r\n"),
            "commands must start beside the final prompt symbol"
        );
        assert!(
            !ansi.contains("\\["),
            "shell wrappers must not leak into VTE"
        );
        assert!(!marker.exists());
        assert_eq!(fs::read_to_string(path).unwrap(), source);
    }
}
#[test]
fn all_language_samples_and_reviewed_editor_fields_cross_the_safe_preview_boundary() {
    use crate::starship_draft::{ModuleEdit, StarshipDraft};
    use crate::starship_modules::MODULES as EDITABLE;
    let temp = tempfile::tempdir().unwrap();
    for (i, spec) in EDITABLE.iter().enumerate() {
        let mut draft =
            StarshipDraft::new(temp.path().join("source.toml"), "format='$all'\n".into()).unwrap();
        draft.select_module(i).unwrap();
        for j in 0..spec.symbols.len() {
            draft.symbol = j;
            draft.edit(ModuleEdit::Symbol("test ".into())).unwrap();
        }
        for j in 0..spec.styles.len() {
            draft.style = j;
            draft.edit(ModuleEdit::Style("bold red".into())).unwrap();
        }
        draft.edit(ModuleEdit::Enabled(true)).unwrap();
        let (safe, notices) = prepare_config(draft.contents()).unwrap();
        let safe: toml::Table = toml::from_str(&safe).unwrap();
        let full: toml::Table = toml::from_str(draft.contents()).unwrap();
        for field in spec.symbols.iter().chain(spec.styles) {
            assert_eq!(
                safe[spec.id][field.key], full[spec.id][field.key],
                "{}.{}",
                spec.id, field.key
            );
            assert!(!notices.contains(&format!("{}.{}", spec.id, field.key)));
        }
    }
    if trusted_program("starship", temp.path()).is_none()
        || trusted_program("bwrap", temp.path()).is_none()
    {
        return;
    }
    for (scenario, module) in [(1, "rust"), (2, "nodejs"), (3, "python"), (4, "golang")] {
        let source = format!("format='${module}'\n[{module}]\nformat='[$symbol]($style)'\n");
        let mut draft = StarshipDraft::new(temp.path().join("source.toml"), source).unwrap();
        draft
            .select_module(crate::starship_modules::module_index(module).unwrap())
            .unwrap();
        draft
            .edit(ModuleEdit::Symbol("[$literal] 🦀".into()))
            .unwrap();
        let (ansi, _) = render_copy(draft.contents(), temp.path(), 80, scenario).unwrap();
        assert!(ansi.contains("[$literal] 🦀"), "{module}: {ansi:?}");
    }
    assert!(render_copy("format='$all'", temp.path(), 80, u32::MAX).is_err());
    assert!(!temp.path().join("Cargo.toml").exists());
    assert!(!temp.path().join("package.json").exists());
    assert!(!temp.path().join("pyproject.toml").exists());
    assert!(!temp.path().join("go.mod").exists());
}
