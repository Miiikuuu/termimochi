//! Explicit reviewed post-apply launch in the chosen terminal. Never types into
//! an existing tab, rewrites a startup file, or guesses a pixel fallback.
use crate::fastfetch_apply::Target;
use crate::pixel_trial::Terminal;
use std::{path::Path, process::Command};

// Only fixed shell code is interpreted. The reviewed path is a positional
// argument, not interpolated code. Neither bashrc nor BASH_ENV is sourced.
const RUN_ONCE: &str = r#"/usr/bin/fastfetch --config "$1"
result=$?
if [ "$result" -ne 0 ]; then printf '\nFastfetch exited with status %s. The saved configuration was not rolled back.\n' "$result"; fi
printf '\nPress Enter to close this preview… '
IFS= read -r answer
exit "$result"
"#;

fn command_in(path: &Path, terminal: Terminal, executable: &Path) -> Command {
    let mut command = Command::new(executable);
    crate::kitty_session::clean_environment(&mut command);
    let directory = path.parent().unwrap_or(Path::new("/"));
    match terminal {
        Terminal::Ptyxis => {
            command
                .args(["--new-window", "--title=Fastfetch · TermiMochi"])
                .arg(format!("--working-directory={}", directory.display()))
                .arg("--");
        }
        Terminal::Kitty => {
            command
                .args([
                    "--config",
                    "NONE",
                    "--override",
                    "shell_integration=disabled",
                    "--title",
                    "Fastfetch · TermiMochi",
                    "--directory",
                ])
                .arg(directory);
        }
        Terminal::Xterm => {
            command.args(["-T", "Fastfetch · TermiMochi", "-e"]);
        }
    }
    // A Ptyxis service may already be running with its own environment. Clean
    // again INSIDE the terminal, using only locally captured allowed variables.
    let environment: Vec<_> = command
        .get_envs()
        .filter_map(|(k, v)| {
            v.map(|v| {
                let mut value = k.to_os_string();
                value.push("=");
                value.push(v);
                value
            })
        })
        .collect();
    command
        .args([
            "/usr/bin/env",
            "-u",
            "BASH_ENV",
            "-u",
            "ENV",
            "--ignore-environment",
        ])
        .args(environment)
        .arg(if terminal == Terminal::Kitty {
            "TERM=xterm-kitty"
        } else {
            "TERM=xterm-256color"
        })
        .args([
            "/bin/bash",
            "--noprofile",
            "--norc",
            "-c",
            RUN_ONCE,
            "termimochi-fastfetch",
        ])
        .arg(path)
        .current_dir(directory);
    command
}

#[cfg(test)]
fn command(path: &Path) -> Command {
    command_in(path, Terminal::Ptyxis, Path::new("/usr/bin/ptyxis"))
}

fn validate_output(target: &Target, terminal: Terminal) -> Result<(), String> {
    let config = crate::fastfetch_document::value(&target.source()?)?;
    let (kind, source) = crate::greeting_art::source_kind(&config["logo"]);
    match kind {
        "kitty"|"kitty-direct"|"kitty-icat" if terminal!=Terminal::Kitty => return Err("This configuration contains Kitty graphics. Choose Kitty; no character fallback has been substituted.".into()),
        "sixel" if terminal!=Terminal::Xterm => return Err("This configuration requests Sixel. Choose the Sixel test target (Xterm); support in other targets is not verified.".into()),
        "raw"|"file-raw"|"data-raw" => {
            let bytes=if kind=="data-raw" {source.as_bytes().to_vec()} else {
                let path=Path::new(source); let path=if path.is_absolute(){path.to_owned()}else{target.path.parent().ok_or("Missing config directory.")?.join(path)};
                crate::typography_preset::read_private_with_limit(&path,40*1024*1024)?.ok_or("Raw logo asset is missing; reopen or repair the source before running.")?
            };
            if bytes.starts_with(b"\x1b_G") {
                if terminal!=Terminal::Kitty {return Err("This raw logo contains Kitty graphics/animation. Open it in Kitty, not Ptyxis or Xterm.".into());}
            } else if bytes.starts_with(b"\x1bP0;1;0q") || bytes.starts_with(b"\x1bPq") {
                if terminal!=Terminal::Xterm {return Err("This raw logo contains Sixel; choose the Xterm target.".into());}
            } else if !(kind!="raw" && std::str::from_utf8(&bytes).ok().and_then(|s|crate::greeting_art::Artwork::parse(s).ok()).is_some_and(|a|!a.filtered)) {
                return Err("The raw logo protocol is unverified. Use the artwork import/target trial flow; this launcher will not guess or silently change output.".into());
            }
        }
        "auto" if !source.is_empty() && !crate::greeting_art::builtin_name(source) => return Err("File-logo Auto output is not verified for this target. Choose an explicit Character/Image output and test it first.".into()),
        "auto"|"none"|"builtin"|"small"|"data"|"file"|"kitty"|"kitty-direct"|"kitty-icat"|"sixel" => {},
        _ => return Err(format!("Logo output type {kind:?} is not supported by this terminal launcher. Its source is unchanged.")),
    }
    Ok(())
}

#[cfg(test)]
fn launch(target: &Target) -> Result<(), String> {
    launch_in(target, Terminal::Ptyxis)
}

/// Caller presents exact source and collects explicit execution consent. This
/// function rechecks it; saving or importing alone never calls this boundary.
pub(crate) fn launch_in(target: &Target, terminal: Terminal) -> Result<(), String> {
    target.check()?;
    validate_output(target, terminal)?;
    #[cfg(test)]
    {
        // GUI tests must never open a window in the user's running terminal.
        LAUNCHES.with(|runs| runs.borrow_mut().push(target.path.clone()));
        Ok(())
    }
    #[cfg(not(test))]
    {
        if !Path::new("/usr/bin/fastfetch").is_file() {
            return Err("Opening this result requires system Fastfetch. The saved configuration is unchanged.".into());
        }
        let executable=terminal.executable().ok_or_else(||format!("{} is unavailable. Install it or choose a compatible target; the configuration is still saved.",terminal.label()))?;
        target.check()?;
        let mut child = command_in(&target.path, terminal, &executable)
            .spawn()
            .map_err(|e| {
                format!(
                    "Could not open {}: {e}. The configuration is still saved.",
                    terminal.label()
                )
            })?;
        // Reap the launcher without blocking GTK. Fastfetch's own errors are
        // displayed in the new terminal, which stays open for inspection.
        std::thread::spawn(move || {
            let _ = child.wait();
        });
        Ok(())
    }
}

#[cfg(test)]
thread_local! { pub(crate) static LAUNCHES: std::cell::RefCell<Vec<std::path::PathBuf>> = const { std::cell::RefCell::new(Vec::new()) }; }

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn paths_are_arguments_and_startup_files_are_not_loaded() {
        let file = Path::new("/tmp/a '$(touch sentinel)' ;/config.jsonc");
        let command = command(file);
        let args: Vec<_> = command.get_args().collect();
        assert_eq!(command.get_program(), "/usr/bin/ptyxis");
        assert_eq!(args.last().unwrap(), &file.as_os_str());
        assert!(args.contains(&std::ffi::OsStr::new("--new-window")));
        assert!(args.contains(&std::ffi::OsStr::new("--noprofile")));
        assert!(args.contains(&std::ffi::OsStr::new("--norc")));
        assert!(args.contains(&std::ffi::OsStr::new("BASH_ENV")));
        assert!(!RUN_ONCE.contains("sentinel"));
        assert!(RUN_ONCE.contains("IFS= read -r"));
    }
    #[test]
    fn launch_rechecks_the_applied_target_and_never_uses_missing_config() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.jsonc");
        let missing = Target::open(path.clone()).unwrap();
        assert!(launch(&missing).is_err());
        std::fs::write(&path, "{\"logo\":\"none\",\"modules\":[]}").unwrap();
        let target = Target::open(path.clone()).unwrap();
        launch(&target).unwrap();
        assert_eq!(
            LAUNCHES.with(|runs| runs.borrow().clone()),
            vec![path.clone()]
        );
        std::fs::write(&path, "// external change\n{}").unwrap();
        assert!(launch(&target).is_err());
        assert_eq!(LAUNCHES.with(|runs| runs.borrow().len()), 1);
    }
    #[test]
    fn selected_terminal_controls_arguments_and_inner_environment_is_clean() {
        let path = Path::new("/tmp/中文 '$(touch sentinel)' ;/config.jsonc");
        for (terminal, program, flag) in [
            (Terminal::Kitty, "/usr/bin/kitty", "--config"),
            (Terminal::Xterm, "/usr/bin/xterm", "-e"),
            (Terminal::Ptyxis, "/usr/bin/ptyxis", "--new-window"),
        ] {
            let command = command_in(path, terminal, Path::new(program));
            let args: Vec<_> = command.get_args().map(|a| a.to_string_lossy()).collect();
            assert_eq!(command.get_program(), program);
            assert!(args.iter().any(|a| a == flag));
            assert_eq!(args.last().unwrap(), &path.to_string_lossy());
            assert!(!args.iter().any(|a| a == "--hold" || a == "--rcfile"));
            assert!(args.iter().any(|a| a == "--ignore-environment"));
            assert!(!args.iter().any(|a| a.starts_with("PROMPT_COMMAND=")
                || a.starts_with("BASH_FUNC_")
                || a.starts_with("BASH_ENV=")));
            if terminal == Terminal::Kitty {
                assert!(args.windows(2).any(|a| a == ["--config", "NONE"]));
            }
        }
        assert_eq!(RUN_ONCE.matches("/usr/bin/fastfetch").count(), 1);
        assert!(!RUN_ONCE.contains(".bashrc") && !RUN_ONCE.contains("exec bash"));
    }

    #[test]
    fn incompatible_or_unknown_raw_output_never_launches_a_wrong_target() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("config.jsonc");
        for kind in ["kitty", "kitty-direct", "kitty-icat"] {
            std::fs::write(
                &path,
                serde_json::json!({"logo":{"type":kind,"source":"/image.png"},"modules":[]})
                    .to_string(),
            )
            .unwrap();
            let target = Target::open(path.clone()).unwrap();
            assert!(validate_output(&target, Terminal::Kitty).is_ok());
            assert!(validate_output(&target, Terminal::Ptyxis).is_err());
            assert!(validate_output(&target, Terminal::Xterm).is_err());
        }
        let raw = root.path().join("logo.kitty");
        std::fs::write(&raw, b"\x1b_Ga=T;payload\x1b\\").unwrap();
        std::fs::write(
            &path,
            serde_json::json!({"logo":{"type":"raw","source":"logo.kitty"},"modules":[]})
                .to_string(),
        )
        .unwrap();
        let target = Target::open(path.clone()).unwrap();
        assert!(validate_output(&target, Terminal::Kitty).is_ok());
        assert!(validate_output(&target, Terminal::Ptyxis).is_err());
        std::fs::write(&raw, b"unknown raw stream").unwrap();
        assert!(validate_output(&target, Terminal::Kitty).is_err());
        std::fs::write(&path,serde_json::json!({"logo":{"type":"data-raw","source":"\u{1b}[31mANSI\u{1b}[0m"},"modules":[]}).to_string()).unwrap();
        let target = Target::open(path).unwrap();
        assert!(validate_output(&target, Terminal::Ptyxis).is_ok());
        assert!(validate_output(&target, Terminal::Xterm).is_ok());
    }
    #[test]
    #[ignore = "requires system Fastfetch; runs the fixed wrapper with a temporary config and no terminal launcher"]
    fn real_fastfetch_runs_once_and_keeps_failure_visible() {
        use std::io::Write;
        let dir = tempfile::tempdir().unwrap();
        let path = dir
            .path()
            .join("literal '$(touch sentinel)' ;.fastfetch.jsonc");
        let hook = dir.path().join("untrusted-startup");
        std::fs::write(&hook, "touch STARTUP_WAS_RUN\n").unwrap();
        std::fs::write(&path,r#"{"logo":{"type":"none"},"modules":[{"type":"custom","format":"TERMIMOCHI_RUN_ONCE"}]}"#).unwrap();
        let mut child = Command::new("/usr/bin/env")
            .args([
                "-u",
                "BASH_ENV",
                "-u",
                "ENV",
                "/bin/bash",
                "--noprofile",
                "--norc",
                "-c",
                RUN_ONCE,
                "termimochi-fastfetch",
            ])
            .arg(&path)
            .env("BASH_ENV", &hook)
            .env("ENV", &hook)
            .current_dir(dir.path())
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        child.stdin.take().unwrap().write_all(b"\n").unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(output.status.success());
        let text = String::from_utf8(output.stdout).unwrap();
        assert_eq!(text.matches("TERMIMOCHI_RUN_ONCE").count(), 1);
        assert!(text.contains("Press Enter"));
        assert!(!dir.path().join("sentinel").exists());
        assert!(!dir.path().join("STARTUP_WAS_RUN").exists());
        let output = Command::new("/bin/bash")
            .args([
                "--noprofile",
                "--norc",
                "-c",
                RUN_ONCE,
                "termimochi-fastfetch",
            ])
            .arg(dir.path().join("missing.jsonc"))
            .env_remove("BASH_ENV")
            .env_remove("ENV")
            .stdin(std::process::Stdio::null())
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stdout).contains("Fastfetch exited with status"));
    }

    #[test]
    #[ignore = "requires an isolated X11 display, Ptyxis and Fastfetch; uses a private D-Bus session"]
    fn real_ptyxis_opens_new_window_and_enter_closes_once() {
        use std::{
            os::unix::fs::PermissionsExt,
            os::unix::process::CommandExt,
            time::{Duration, Instant},
        };
        let dir = tempfile::tempdir().unwrap();
        std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
        let path = dir.path().join("config.jsonc");
        std::fs::write(&path,r#"{"logo":{"type":"builtin","source":"ubuntu"},"modules":[{"type":"custom","format":"TermiMochi: Fastfetch ran once after applying."},"os","shell","terminal"]}"#).unwrap();
        let plan = command(&path);
        let args: Vec<_> = plan
            .get_args()
            .map(|arg| {
                if arg == "--title=Fastfetch · TermiMochi" {
                    std::ffi::OsString::from("--title=TermiMochi point-to-edit test")
                } else {
                    arg.to_os_string()
                }
            })
            .collect();
        let mut child = Command::new("dbus-run-session")
            .args(["--", "/usr/bin/ptyxis", "--standalone"])
            .args(args)
            .env("XDG_CONFIG_HOME", dir.path().join("config"))
            .env("XDG_DATA_HOME", dir.path().join("data"))
            .env("GIO_USE_VFS", "local")
            .process_group(0)
            .spawn()
            .unwrap();
        let group = child.id();
        let check = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let deadline = Instant::now() + Duration::from_secs(15);
            loop {
                let output = Command::new("xwininfo")
                    .args(["-root", "-tree"])
                    .output()
                    .unwrap();
                if String::from_utf8_lossy(&output.stdout).contains("TermiMochi point-to-edit test")
                {
                    break;
                }
                assert!(
                    Instant::now() < deadline,
                    "Ptyxis did not open the test window"
                );
                std::thread::sleep(Duration::from_millis(50));
            }
            std::thread::sleep(Duration::from_millis(1200));
            let driver = concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../scripts/preview-pointer-driver.py"
            );
            if let Some(file) = std::env::var_os("TERMIMOCHI_FASTFETCH_RUN_SCREENSHOT") {
                assert!(
                    Command::new("python3")
                        .arg(driver)
                        .args(["capture", "50", "150"])
                        .env("TERMIMOCHI_INSPECT_SCREENSHOT", file)
                        .status()
                        .unwrap()
                        .success()
                );
            }
            assert!(
                Command::new("python3")
                    .arg(driver)
                    .args(["click_enter", "50", "150"])
                    .status()
                    .unwrap()
                    .success()
            );
            let deadline = Instant::now() + Duration::from_secs(10);
            loop {
                if let Some(status) = child.try_wait().unwrap() {
                    assert!(status.success());
                    break;
                }
                assert!(
                    Instant::now() < deadline,
                    "The one-shot terminal did not close after Enter"
                );
                std::thread::sleep(Duration::from_millis(50));
            }
        }));
        // This process group belongs solely to the private test session.
        if check.is_err() {
            let _ = Command::new("/bin/kill")
                .args(["-TERM", "--", &format!("-{group}")])
                .status();
        }
        let _ = child.wait();
        if let Err(error) = check {
            std::panic::resume_unwind(error);
        }
    }
}
