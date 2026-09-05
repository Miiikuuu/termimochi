//! Read-only, bounded context probes for the Current Folder preview.
//!
//! Call `load` on a worker thread. No shell startup file or project source is
//! read, and tool versions are queried outside the project directory.

use std::{
    env, fs,
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

use gtk::glib;

use crate::{prompt::PromptPreviewContext, starship_import::ImportedPrompt};

const PROBE_BUDGET: Duration = Duration::from_millis(900);
const COMMAND_BUDGET: Duration = Duration::from_millis(300);
const OUTPUT_LIMIT: usize = 32 * 1024;

#[derive(Clone, Debug)]
pub(crate) struct CurrentPreviewContext {
    pub(crate) directory: PathBuf,
    pub(crate) shell: String,
    pub(crate) detail: String,
    pub(crate) username: String,
    pub(crate) hostname: String,
    /// Complete sanitized path for `pwd`; never abbreviates home or drops tails.
    pub(crate) absolute_path: String,
    /// Complete sanitized prompt path, with only the home prefix abbreviated.
    pub(crate) path: String,
    pub(crate) git_branch: Option<String>,
    pub(crate) git_status: Option<String>,
    pub(crate) git_status_lines: Vec<String>,
    pub(crate) entries: Vec<String>,
    pub(crate) rust_version: Option<String>,
    pub(crate) node_version: Option<String>,
    pub(crate) python_version: Option<String>,
    pub(crate) go_version: Option<String>,
    pub(crate) is_root: bool,
    pub(crate) is_ssh: bool,
    pub(crate) imported_prompt: ImportedPrompt,
    time: String,
}

impl CurrentPreviewContext {
    #[cfg(test)]
    pub(crate) fn load(directory: PathBuf) -> Self {
        Self::load_for_width(directory, 80)
    }

    pub(crate) fn load_for_width(directory: PathBuf, columns: usize) -> Self {
        let deadline = Instant::now() + PROBE_BUDGET;
        let now = glib::DateTime::now_local().ok();
        let read_at = now
            .as_ref()
            .and_then(|date| date.format("%H:%M:%S").ok())
            .map_or_else(|| "now".to_owned(), |value| value.to_string());
        let time = now
            .as_ref()
            .and_then(|date| date.format("%H:%M").ok())
            .map_or_else(String::new, |value| value.to_string());
        let shell = env::var_os("SHELL")
            .and_then(|value| Path::new(&value).file_name().map(|name| name.to_owned()))
            .map(|value| display_text(&value.to_string_lossy(), 24))
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "shell".to_owned());
        let mut context = Self {
            absolute_path: sanitized_text(&directory.to_string_lossy()),
            path: display_path(&directory, &glib::home_dir()),
            shell,
            detail: String::new(),
            username: display_text(&glib::user_name().to_string_lossy(), 48),
            hostname: display_text(&glib::host_name(), 48),
            git_branch: None,
            git_status: None,
            git_status_lines: Vec::new(),
            entries: Vec::new(),
            rust_version: None,
            node_version: None,
            python_version: None,
            go_version: None,
            is_root: effective_uid().is_some_and(|uid| uid == 0),
            is_ssh: ["SSH_CONNECTION", "SSH_CLIENT", "SSH_TTY"]
                .into_iter()
                .any(|key| env::var_os(key).is_some_and(|value| !value.is_empty())),
            directory,
            imported_prompt: ImportedPrompt {
                path: PathBuf::new(),
                source: None,
                ansi: None,
                detail: "Folder unavailable; Starship was not run.".to_owned(),
            },
            time,
        };
        if !context.directory.is_absolute() || !context.directory.is_dir() {
            context.detail = format!("Read {read_at} · Folder unavailable");
            return context;
        }
        context.entries = directory_entries(&context.directory);

        let mut details = vec![format!("Read {read_at}")];
        match probe(
            "git",
            &[
                "--no-optional-locks",
                "-c",
                "core.fsmonitor=false",
                "-c",
                "core.untrackedCache=false",
                "status",
                "--porcelain=v1",
                "--branch",
                "--untracked-files=normal",
            ],
            &context.directory,
            &context.directory,
            deadline,
        ) {
            Ok(output) => {
                let git = parse_git_status(&output);
                context.git_branch = git.branch;
                context.git_status = git.summary;
                context.git_status_lines = git.lines;
                details.push(if context.git_status.is_some() {
                    "Git changes detected".to_owned()
                } else {
                    "Git clean".to_owned()
                });
            }
            Err(ProbeError::Failed) => details.push("Git unavailable in this folder".to_owned()),
            Err(error) => details.push(format!("Git {}", error.label())),
        }

        let languages = detected_languages(&context.directory);
        let mut absent = Vec::new();
        for language in Language::ALL {
            if !languages.contains(&language) {
                absent.push(language.label());
                continue;
            }
            let (program, arguments) = language.command();
            let version = probe(
                program,
                arguments,
                Path::new("/"),
                &context.directory,
                deadline,
            )
            .ok()
            .and_then(|output| parse_version(&output));
            details.push(format!(
                "{} {}",
                language.label(),
                if version.is_some() {
                    "detected"
                } else {
                    "detected; version unavailable"
                }
            ));
            match language {
                Language::Rust => context.rust_version = version,
                Language::Node => context.node_version = version,
                Language::Python => context.python_version = version,
                Language::Go => context.go_version = version,
            }
        }
        if !absent.is_empty() {
            details.push(format!("{} not detected", absent.join("/")));
        }
        details.push(
            "Installed tool versions; previous exit status, jobs and timing unavailable".to_owned(),
        );
        context.detail = details.join(" · ");
        context.imported_prompt = ImportedPrompt::load(&context.directory, columns);
        context
    }

    pub(crate) fn as_prompt_context(&self) -> PromptPreviewContext<'_> {
        PromptPreviewContext {
            username: &self.username,
            hostname: &self.hostname,
            path: &self.path,
            git_branch: self.git_branch.as_deref(),
            git_status: self.git_status.as_deref(),
            rust_version: self.rust_version.as_deref(),
            node_version: self.node_version.as_deref(),
            python_version: self.python_version.as_deref(),
            go_version: self.go_version.as_deref(),
            command_duration: None,
            jobs: None,
            time: (!self.time.is_empty()).then_some(self.time.as_str()),
            // The previous shell's exit status is not inherited by GUI apps.
            exit_status: 0,
            is_root: self.is_root,
            is_ssh: self.is_ssh,
        }
    }
}

fn display_path(path: &Path, home: &Path) -> String {
    let value = if let Ok(relative) = path.strip_prefix(home) {
        if relative.as_os_str().is_empty() {
            "~".to_owned()
        } else {
            format!("~/{}", relative.display())
        }
    } else {
        path.display().to_string()
    };
    sanitized_text(&value)
}

pub(crate) fn display_text(value: &str, limit: usize) -> String {
    let sanitized = sanitized_text(value);
    let mut characters = sanitized.chars();
    let mut result: String = characters.by_ref().take(limit).collect();
    if characters.next().is_some() {
        result.push('…');
    }
    result
}

/// Remove terminal escape sequences as units. Removing just ESC would leave
/// text such as `[31m` in path names and could change their visible structure.
fn sanitized_text(value: &str) -> String {
    enum State {
        Text,
        Escape,
        Intermediate,
        Csi,
        String,
        StringEscape,
    }
    let mut state = State::Text;
    let mut result = String::with_capacity(value.len());
    for character in value.chars() {
        match state {
            State::Text => match character {
                '\u{1b}' => state = State::Escape,
                '\u{9b}' => state = State::Csi,
                '\u{90}' | '\u{98}' | '\u{9d}' | '\u{9e}' | '\u{9f}' => state = State::String,
                character
                    if character.is_control()
                        || matches!(
                            character,
                            '\u{061c}' | '\u{200e}' | '\u{200f}'
                                | '\u{2028}'..='\u{202e}' | '\u{2066}'..='\u{2069}'
                        ) => {}
                _ => result.push(character),
            },
            State::Escape => {
                state = match character {
                    '[' => State::Csi,
                    ']' | 'P' | 'X' | '^' | '_' => State::String,
                    ' '..='/' => State::Intermediate,
                    _ => State::Text,
                };
            }
            State::Intermediate => {
                if ('0'..='~').contains(&character) {
                    state = State::Text;
                }
            }
            State::Csi => {
                if ('@'..='~').contains(&character) {
                    state = State::Text;
                }
            }
            State::String => match character {
                '\u{7}' | '\u{9c}' => state = State::Text,
                '\u{1b}' => state = State::StringEscape,
                _ => {}
            },
            State::StringEscape => {
                state = if character == '\\' || character == '\u{9c}' || character == '\u{7}' {
                    State::Text
                } else if character == '\u{1b}' {
                    State::StringEscape
                } else {
                    State::String
                };
            }
        }
    }
    result
}

fn effective_uid() -> Option<u32> {
    let mut contents = String::new();
    fs::File::open("/proc/self/status")
        .ok()?
        .take(4096)
        .read_to_string(&mut contents)
        .ok()?;
    contents
        .lines()
        .find_map(|line| line.strip_prefix("Uid:"))?
        .split_whitespace()
        .nth(1)?
        .parse()
        .ok()
}

fn directory_entries(directory: &Path) -> Vec<String> {
    let Ok(entries) = fs::read_dir(directory) else {
        return Vec::new();
    };
    let mut names = entries
        .take(128)
        .filter_map(Result::ok)
        .map(|entry| display_text(&entry.file_name().to_string_lossy(), 48))
        .filter(|name| !name.starts_with('.') && !name.is_empty())
        .collect::<Vec<_>>();
    names.sort_unstable();
    names.truncate(8);
    names
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Language {
    Rust,
    Node,
    Python,
    Go,
}

impl Language {
    const ALL: [Self; 4] = [Self::Rust, Self::Node, Self::Python, Self::Go];

    const fn marker(self) -> &'static str {
        match self {
            Self::Rust => "Cargo.toml",
            Self::Node => "package.json",
            Self::Python => "pyproject.toml",
            Self::Go => "go.mod",
        }
    }

    const fn label(self) -> &'static str {
        match self {
            Self::Rust => "Rust",
            Self::Node => "Node",
            Self::Python => "Python",
            Self::Go => "Go",
        }
    }

    const fn command(self) -> (&'static str, &'static [&'static str]) {
        match self {
            Self::Rust => ("rustc", &["--version"]),
            Self::Node => ("node", &["--version"]),
            Self::Python => ("python3", &["-I", "--version"]),
            Self::Go => ("go", &["version"]),
        }
    }
}

fn detected_languages(directory: &Path) -> Vec<Language> {
    Language::ALL
        .into_iter()
        .filter(|language| directory.join(language.marker()).is_file())
        .collect()
}

fn parse_version(output: &str) -> Option<String> {
    output.split_whitespace().find_map(|token| {
        let version = token
            .strip_prefix("go")
            .unwrap_or(token)
            .trim_start_matches('v');
        (version.as_bytes().first().is_some_and(u8::is_ascii_digit)
            && version.contains('.')
            && version
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'+')))
        .then(|| format!("v{}", display_text(version, 32)))
    })
}

#[derive(Debug, Eq, PartialEq)]
struct GitContext {
    branch: Option<String>,
    summary: Option<String>,
    lines: Vec<String>,
}

fn parse_git_status(output: &str) -> GitContext {
    let mut branch = None;
    let mut ahead = None;
    let mut behind = None;
    let mut flags = [false; 6]; // conflict, untracked, modified, staged, renamed, deleted
    let mut lines = Vec::new();
    for line in output.lines() {
        if let Some(header) = line.strip_prefix("## ") {
            let name = header
                .strip_prefix("No commits yet on ")
                .or_else(|| header.strip_prefix("Initial commit on "))
                .unwrap_or(header)
                .split("...")
                .next()
                .unwrap_or(header)
                .split(" [")
                .next()
                .unwrap_or(header);
            branch = Some(if name.starts_with("HEAD (no branch)") {
                "detached".to_owned()
            } else {
                // Prompt formatting owns grapheme-aware branch truncation.
                // Cutting scalar values here can split an emoji sequence.
                sanitized_text(name)
            });
            ahead = divergence_count(header, "ahead ");
            behind = divergence_count(header, "behind ");
            continue;
        }
        let status = line.as_bytes();
        if status.len() < 3 || status[2] != b' ' {
            continue;
        }
        let (index, worktree) = (status[0], status[1]);
        flags[0] |= index == b'U' || worktree == b'U' || matches!(&status[..2], b"AA" | b"DD");
        flags[1] |= index == b'?';
        flags[2] |= worktree == b'M';
        flags[3] |= matches!(index, b'A' | b'M' | b'T');
        flags[4] |= index == b'R' || worktree == b'R';
        flags[5] |= index == b'D' || worktree == b'D';
        if lines.len() < 6 {
            lines.push(display_text(line, 64));
        }
    }
    // Match Starship's $all_status ordering, not porcelain's column order.
    let mut summary = [0, 5, 4, 2, 3, 1]
        .into_iter()
        .zip(["=", "x", "r", "!", "+", "?"])
        .filter_map(|(index, symbol)| flags[index].then_some(symbol))
        .collect::<String>();
    match (ahead, behind) {
        (Some(ahead), Some(behind)) => summary.push_str(&format!("<>{ahead}/{behind}")),
        (Some(ahead), None) => summary.push_str(&format!(">{ahead}")),
        (None, Some(behind)) => summary.push_str(&format!("<{behind}")),
        _ => {}
    }
    GitContext {
        branch,
        summary: (!summary.is_empty()).then_some(summary),
        lines,
    }
}

fn divergence_count(header: &str, prefix: &str) -> Option<u32> {
    let count = header.split_once(prefix)?.1;
    count
        .split(|character: char| !character.is_ascii_digit())
        .next()?
        .parse::<u32>()
        .ok()
        .filter(|value| *value > 0)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ProbeError {
    Unavailable,
    Failed,
    Timeout,
    OutputLimit,
}

impl ProbeError {
    const fn label(self) -> &'static str {
        match self {
            Self::Unavailable => "unavailable",
            Self::Failed => "probe failed",
            Self::Timeout => "probe timed out",
            Self::OutputLimit => "output limit reached",
        }
    }
}

fn probe(
    program: &str,
    arguments: &[&str],
    working_directory: &Path,
    project_directory: &Path,
    deadline: Instant,
) -> Result<String, ProbeError> {
    if Instant::now() >= deadline {
        return Err(ProbeError::Timeout);
    }
    let executable = find_program(program, project_directory).ok_or(ProbeError::Unavailable)?;
    let mut command = Command::new(executable);
    command
        .args(arguments)
        .current_dir(working_directory)
        .env_clear()
        .env("PATH", env::var_os("PATH").unwrap_or_default())
        .env("HOME", glib::home_dir())
        .env("LANG", "C.UTF-8")
        .env("LC_ALL", "C.UTF-8")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("RUSTUP_AUTO_INSTALL", "0")
        .env("GOTOOLCHAIN", "local");
    run_bounded(&mut command, deadline.min(Instant::now() + COMMAND_BUDGET))
}

fn find_program(program: &str, project_directory: &Path) -> Option<PathBuf> {
    let path = env::var_os("PATH")?;
    let project = project_directory.canonicalize().ok()?;
    env::split_paths(&path)
        .filter(|directory| directory.is_absolute())
        .map(|directory| directory.join(program))
        .find(|candidate| {
            candidate.is_file()
                && candidate
                    .canonicalize()
                    .is_ok_and(|resolved| !resolved.starts_with(&project))
        })
}

pub(crate) fn run_bounded(command: &mut Command, deadline: Instant) -> Result<String, ProbeError> {
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| ProbeError::Unavailable)?;
    let stdout = child.stdout.take().ok_or(ProbeError::Failed)?;
    let (sender, receiver) = mpsc::sync_channel(1);
    // Read concurrently so a full pipe cannot hold up the timeout loop. The
    // extra byte detects overflow while keeping memory bounded.
    thread::spawn(move || {
        let mut bytes = Vec::new();
        let result = stdout
            .take((OUTPUT_LIMIT + 1) as u64)
            .read_to_end(&mut bytes)
            .map(|_| bytes);
        let _ = sender.send(result);
    });
    let mut output = None;
    loop {
        if let Ok(result) = receiver.try_recv() {
            output = Some(result.map_err(|_| ProbeError::Failed));
        }
        if output.as_ref().is_some_and(|result| {
            result
                .as_ref()
                .is_ok_and(|bytes| bytes.len() > OUTPUT_LIMIT)
        }) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(ProbeError::OutputLimit);
        }
        match child.try_wait() {
            Ok(Some(status)) => {
                if !status.success() {
                    return Err(ProbeError::Failed);
                }
                if let Some(result) = output {
                    return result.map(|bytes| String::from_utf8_lossy(&bytes).into_owned());
                }
            }
            Ok(None) => {}
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(ProbeError::Failed);
            }
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(ProbeError::Timeout);
        }
        thread::sleep(Duration::from_millis(3));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_folder_keeps_unobserved_prompt_state_empty() {
        let current = CurrentPreviewContext::load(PathBuf::from(
            "/termimochi-nonexistent-preview-context-test",
        ));
        let prompt = current.as_prompt_context();
        assert!(current.detail.contains("Folder unavailable"));
        assert!(prompt.git_branch.is_none());
        assert!(prompt.rust_version.is_none());
        assert!(prompt.node_version.is_none());
        assert!(prompt.python_version.is_none());
        assert!(prompt.go_version.is_none());
        assert!(prompt.jobs.is_none());
        assert!(prompt.command_duration.is_none());
        assert!(current.entries.is_empty());
        assert_eq!(
            current.absolute_path,
            "/termimochi-nonexistent-preview-context-test"
        );
    }

    #[test]
    fn language_detection_only_checks_regular_project_markers() {
        let fixture = glib::mkdtemp(env::temp_dir().join("termimochi-context-XXXXXX")).unwrap();
        let node = fixture.join("package.json");
        let fake_rust = fixture.join("Cargo.toml");
        let python = fixture.join("pyproject.toml");
        fs::write(&node, "This marker is deliberately not parsed.").unwrap();
        fs::create_dir(&fake_rust).unwrap();
        fs::write(&python, "Neither are project commands executed.").unwrap();
        let detected = detected_languages(&fixture);
        fs::remove_file(&node).unwrap();
        fs::remove_file(&python).unwrap();
        fs::remove_dir(&fake_rust).unwrap();
        fs::remove_dir(&fixture).unwrap();
        assert_eq!(detected, [Language::Node, Language::Python]);
    }

    #[test]
    fn real_git_porcelain_preserves_branch_changes_and_divergence() {
        let git = parse_git_status(
            "## feature/preview...origin/feature/preview [ahead 2, behind 1]\n M src/main.rs\nA  tests/context.rs\n?? notes.txt\n",
        );
        assert_eq!(git.branch.as_deref(), Some("feature/preview"));
        assert_eq!(git.summary.as_deref(), Some("!+?<>2/1"));
        assert_eq!(git.lines.len(), 3);
        let clean = parse_git_status("## main...origin/main\n");
        assert_eq!(clean.summary, None);
        assert!(clean.lines.is_empty());
        let unborn = parse_git_status("## No commits yet on new-branch\n?? one.txt\n");
        assert_eq!(unborn.branch.as_deref(), Some("new-branch"));
        let renamed_deleted =
            parse_git_status("## main\nR  before.txt -> after.txt\n D removed.txt\n");
        assert_eq!(renamed_deleted.summary.as_deref(), Some("xr"));
    }

    #[test]
    fn context_text_cannot_inject_terminal_controls_and_is_bounded() {
        let git = parse_git_status("## safe\n?? \u{1b}[31mred\u{202e}.txt\n");
        assert_eq!(git.lines[0], "?? red.txt");
        assert!(!git.lines[0].contains('\u{1b}'));
        assert!(!git.lines[0].contains('\u{202e}'));
        let long = display_text(&"中".repeat(100), 4);
        assert_eq!(long, "中中中中…");
        assert_eq!(
            display_path(Path::new("/home/tester/app"), Path::new("/home/tester")),
            "~/app"
        );
        assert_eq!(
            display_path(Path::new("/home/tester2/app"), Path::new("/home/tester")),
            "/home/tester2/app"
        );
        assert_eq!(
            display_path(Path::new("/home/tester"), Path::new("/home/tester")),
            "~"
        );
        assert_eq!(display_path(Path::new("/"), Path::new("/home/tester")), "/");
    }

    #[test]
    fn long_ancestors_never_remove_the_directory_tail_from_prompt_context() {
        // An overlong ancestor is guaranteed not to exist, so load only builds
        // the context and does not run tools or inspect a user's real files.
        let ancestor = "long-ancestor-".repeat(32);
        let directory = glib::home_dir().join(&ancestor).join("one/two/项目");
        let current = CurrentPreviewContext::load(directory.clone());
        assert_eq!(current.absolute_path, directory.to_string_lossy());
        assert_eq!(current.path, format!("~/{ancestor}/one/two/项目"));
        assert!(current.path.chars().count() > 160);
        assert_eq!(current.as_prompt_context().path, current.path);
        assert!(current.as_prompt_context().path.ends_with("/one/two/项目"));
    }

    #[test]
    fn full_paths_strip_ansi_controls_and_bidi_without_abbreviating_pwd() {
        let directory = PathBuf::from(
            "/termimochi-\u{1b}[31mnonexistent\u{1b}[0m/\u{1b}]8;;https://example.invalid\u{7}leaf\u{1b}]8;;\u{1b}\\\u{202e}\n\u{2028}",
        );
        let current = CurrentPreviewContext::load(directory);
        assert_eq!(current.absolute_path, "/termimochi-nonexistent/leaf");
        assert_eq!(current.path, "/termimochi-nonexistent/leaf");
        assert_eq!(
            sanitized_text("/safe/\u{9b}31mvisible\u{9b}0m\u{9d}0;hidden title\u{9c}/end"),
            "/safe/visible/end"
        );
    }

    #[test]
    fn installed_version_outputs_are_normalized_without_fabrication() {
        for (output, expected) in [
            ("rustc 1.92.0 (abc 2025-12-11)", "v1.92.0"),
            ("v24.5.1\n", "v24.5.1"),
            ("Python 3.14.0\n", "v3.14.0"),
            ("go version go1.25.0 linux/amd64\n", "v1.25.0"),
        ] {
            assert_eq!(parse_version(output).as_deref(), Some(expected));
        }
        assert_eq!(parse_version("tool not installed"), None);
    }

    #[test]
    fn child_probes_timeout_and_reject_unbounded_output() {
        let start = Instant::now();
        assert_eq!(
            run_bounded(
                Command::new("/usr/bin/sleep").arg("2"),
                start + Duration::from_millis(40)
            ),
            Err(ProbeError::Timeout)
        );
        assert!(start.elapsed() < Duration::from_secs(1));
        assert_eq!(
            run_bounded(
                &mut Command::new("/usr/bin/yes"),
                Instant::now() + Duration::from_secs(1)
            ),
            Err(ProbeError::OutputLimit)
        );
    }

    #[test]
    fn failed_or_missing_probe_never_supplies_a_version() {
        assert_eq!(
            run_bounded(
                &mut Command::new("/usr/bin/false"),
                Instant::now() + Duration::from_secs(1)
            ),
            Err(ProbeError::Failed)
        );
        assert_eq!(
            run_bounded(
                &mut Command::new("/termimochi-nonexistent-test-command"),
                Instant::now() + Duration::from_secs(1)
            ),
            Err(ProbeError::Unavailable)
        );
    }
}
