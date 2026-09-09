//! Explicit post-apply launch in a new Ptyxis window. Never types into a tab.
use crate::fastfetch_apply::Target;
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

fn command(path: &Path) -> Command {
    let mut command = Command::new("/usr/bin/ptyxis");
    command
        .args(["--new-window", "--title=Fastfetch · TermiMochi"])
        .arg(format!(
            "--working-directory={}",
            gtk::glib::home_dir().display()
        ))
        .args([
            "--",
            "/usr/bin/env",
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
        .arg(path);
    command
}

pub(crate) fn launch(target: &Target) -> Result<(), String> {
    target.check()?;
    target.source()?;
    #[cfg(test)]
    {
        // GUI tests must never open a window in the user's running terminal.
        LAUNCHES.with(|runs| runs.borrow_mut().push(target.path.clone()));
        Ok(())
    }
    #[cfg(not(test))]
    {
        if !Path::new("/usr/bin/fastfetch").is_file() || !Path::new("/usr/bin/ptyxis").is_file() {
            return Err("Automatic launch requires system Fastfetch and Ptyxis. The configuration is saved; you can run fastfetch manually.".into());
        }
        let mut child = command(&target.path).spawn().map_err(|e| {
            format!("Could not open Ptyxis: {e}. The configuration is still saved.")
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
