//! Explicit Bash opt-in. No document import or ordinary Apply enables startup.
use crate::{
    fastfetch_apply::Target,
    fastfetch_document,
    typography_preset::{read_private_with_limit, retain_backup, write_checked_with_limit},
};
use std::{
    ops::Range,
    path::{Path, PathBuf},
};

const LIMIT: u64 = 1024 * 1024;
const BEGIN: &str = "# >>> TermiMochi Greeting v1 >>>";
const END: &str = "# <<< TermiMochi Greeting v1 <<<\n";
const CONFIG: &str = "# config-base64: ";

fn block(config: &Path) -> Result<String, String> {
    let path = config.to_str().ok_or("Configuration path must be UTF-8.")?;
    if !config.is_absolute() || path.chars().any(char::is_control) {
        return Err("Choose an absolute configuration path without control characters.".into());
    }
    let encoded = gtk::glib::base64_encode(path.as_bytes());
    let quoted = path.replace('\'', "'\\''");
    Ok(format!(
        "\n{BEGIN}\n{CONFIG}{encoded}\n\
if [[ $- == *i* && -t 1 && -z ${{SSH_CONNECTION-}} && -z ${{SSH_TTY-}} && -z ${{TMUX-}} && -z ${{_TERMIMOCHI_GREETING_SHOWN-}} && -x /usr/bin/fastfetch ]]; then\n\
    _TERMIMOCHI_GREETING_PARENT=\n\
    IFS= read -r _TERMIMOCHI_GREETING_PARENT < \"/proc/$PPID/comm\" || :\n\
    case $_TERMIMOCHI_GREETING_PARENT in\n\
        bash|sh|dash|zsh|fish|ksh) ;;\n\
        *) _TERMIMOCHI_GREETING_SHOWN=1\n\
           /usr/bin/fastfetch --config '{quoted}' ;;\n\
    esac\n\
    unset _TERMIMOCHI_GREETING_PARENT\n\
fi\n{END}"
    ))
}

fn managed(text: &str) -> Result<Option<(Range<usize>, PathBuf)>, String> {
    if !text.contains(BEGIN) && !text.contains(END.trim_end()) {
        return Ok(None);
    }
    let invalid = "The TermiMochi startup block was edited or duplicated. Nothing was changed; review .bashrc manually.";
    if text.matches(BEGIN).count() != 1 || text.matches(END.trim_end()).count() != 1 {
        return Err(invalid.into());
    }
    let start = text.find(&format!("\n{BEGIN}\n")).ok_or(invalid)?;
    let stop = text[start..]
        .find(END)
        .map(|i| start + i + END.len())
        .ok_or(invalid)?;
    let saved = &text[start..stop];
    let encoded = saved
        .lines()
        .nth(2)
        .and_then(|line| line.strip_prefix(CONFIG))
        .ok_or(invalid)?;
    let bytes = gtk::glib::base64_decode(encoded);
    if gtk::glib::base64_encode(&bytes).as_str() != encoded {
        return Err(invalid.into());
    }
    let path = PathBuf::from(String::from_utf8(bytes).map_err(|_| invalid)?);
    if saved != block(&path)? {
        return Err(invalid.into());
    }
    Ok(Some((start..stop, path)))
}

fn read(path: &Path) -> Result<(Option<Vec<u8>>, String), String> {
    if !path.is_absolute() || path.file_name().is_none_or(|s| s != ".bashrc") {
        return Err("Only the explicitly reviewed .bashrc can be changed.".into());
    }
    let bytes = read_private_with_limit(path, LIMIT)?;
    let text = String::from_utf8(bytes.clone().unwrap_or_default())
        .map_err(|_| ".bashrc must be UTF-8.")?;
    Ok((bytes, text))
}

pub(crate) fn current(path: &Path) -> Result<Option<PathBuf>, String> {
    managed(&read(path)?.1).map(|entry| entry.map(|(_, path)| path))
}

pub(crate) struct Plan {
    pub path: PathBuf,
    pub before: Option<Vec<u8>>,
    pub after: Vec<u8>,
    pub snippet: String,
    target: Option<Target>,
}

fn check_syntax(text: &str) -> Result<(), String> {
    use std::{
        io::Write,
        process::{Command, Stdio},
    };
    // Parse the captured bytes, never execute or source the user's file.
    let mut child = Command::new("/bin/bash")
        .args(["--noprofile", "--norc", "-n"])
        .env_remove("BASH_ENV")
        .env_remove("ENV")
        .env_remove("SHELLOPTS")
        .env_remove("BASHOPTS")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("Cannot check Bash syntax: {e}"))?;
    let written = child.stdin.take().unwrap().write_all(text.as_bytes());
    if !child.wait().map_err(|e| e.to_string())?.success() {
        return Err(".bashrc has invalid Bash syntax. Fix it before enabling Greeting; nothing was changed.".into());
    }
    written.map_err(|e| e.to_string())
}

pub(crate) fn prepare(path: PathBuf, target: Option<Target>) -> Result<Plan, String> {
    let (before, text) = read(&path)?;
    let existing = managed(&text)?;
    let mut after = text.clone();
    let snippet;
    if let Some(target) = &target {
        target.check()?;
        fastfetch_document::parse(&target.source()?)?;
        if existing.is_some() {
            return Err("Greeting startup is already enabled. Disable it first to change its configuration path.".into());
        }
        if text.lines().any(|line| {
            let line = line.trim();
            !line.starts_with('#') && (line.contains("fastfetch") || line.contains("neofetch"))
        }) {
            return Err("An existing Fastfetch/Neofetch reference was found in .bashrc. Review it manually first to avoid duplicate greetings.".into());
        }
        snippet = block(&target.path)?;
        after.push_str(&snippet);
        check_syntax(&after)?;
    } else {
        let (range, _) = existing.ok_or("TermiMochi greeting startup is already off.")?;
        snippet = after[range.clone()].to_owned();
        let separator = if range.end < after.len()
            && range.start > 0
            && !after[..range.start].ends_with('\n')
        {
            "\n"
        } else {
            ""
        };
        after.replace_range(range, separator);
    }
    if after.len() as u64 > LIMIT {
        return Err(".bashrc exceeds 1 MiB.".into());
    }
    Ok(Plan {
        path,
        before,
        after: after.into_bytes(),
        snippet,
        target,
    })
}

pub(crate) fn apply(plan: &Plan, state: &Path) -> Result<Option<PathBuf>, String> {
    if let Some(target) = &plan.target {
        target.check()?;
    }
    if read(&plan.path)?.0 != plan.before {
        return Err(
            ".bashrc changed while reviewing. Review again; nothing was overwritten.".into(),
        );
    }
    let backup = plan
        .before
        .as_ref()
        .map(|before| retain_backup(&state.join("startup-backups"), before))
        .transpose()?;
    write_checked_with_limit(&plan.path, &plan.after, &plan.before, LIMIT)?;
    Ok(backup)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires system Fastfetch and Python PTY; only temporary Bash/config files"]
    fn real_bash_startup_runs_once_and_skips_remote_nested_and_noninteractive() {
        let root = tempfile::tempdir().unwrap();
        let rc = root.path().join(".bashrc");
        let config = root
            .path()
            .join("quote ' $(touch SHOULD_NOT_RUN).fastfetch.jsonc");
        let sentinel = root.path().join("ran");
        let source = serde_json::json!({"logo":{"type":"none"},"modules":[{"type":"command","text":format!("printf x >> '{}'", sentinel.display())}]}).to_string();
        std::fs::write(&config, source).unwrap();
        let plan = prepare(rc.clone(), Some(Target::open(config).unwrap())).unwrap();
        apply(&plan, root.path()).unwrap();
        let script = r#"
import os, pathlib, pty, subprocess, sys
rc, marker = sys.argv[1:]
cases = [('local', {}, True, True, False, b'x'),
         ('ssh', {'SSH_CONNECTION':'remote'}, True, True, False, b''),
         ('ssh_tty', {'SSH_TTY':'/dev/pts/1'}, True, True, False, b''),
         ('tmux', {'TMUX':'session'}, True, True, False, b''),
         ('script', {}, False, True, False, b''),
         ('pipe', {}, True, False, False, b''),
         ('nested', {}, True, True, True, b'')]
for name, extra, interactive, terminal, nested, expected in cases:
    path = pathlib.Path(marker)
    if path.exists(): path.unlink()
    env = os.environ.copy()
    for key in ('SSH_CONNECTION','SSH_TTY','TMUX','BASH_ENV','ENV','_TERMIMOCHI_GREETING_SHOWN'):
        env.pop(key, None)
    env['SHLVL'] = '9' # Desktop-launched terminals need not have SHLVL=1.
    env.update(extra)
    master, slave = pty.openpty()
    try:
        code = 'source "$1"; source "$1"; exit'
        flag = '-ic' if interactive else '-c'
        if nested:
            flag = '-c'
            code = '/bin/bash --noprofile --norc -ic \'source "$1"\' nested "$1"; :'
        result = subprocess.run(['/bin/bash','--noprofile','--norc',flag,code,'startup-test',rc],
            stdin=slave, stdout=slave if terminal else subprocess.PIPE,
            stderr=slave, env=env, timeout=15)
        assert result.returncode == 0, (name, result.returncode)
    finally:
        os.close(slave)
        os.close(master)
    actual = path.read_bytes() if path.exists() else b''
    assert actual == expected, (name, actual, expected)
    assert not pathlib.Path('SHOULD_NOT_RUN').exists(), 'config path was interpreted as shell code'
print('local once; SSH, tmux, nested, script and pipe guards passed')
"#;
        let output = std::process::Command::new("python3")
            .args(["-c", script])
            .arg(rc)
            .arg(sentinel)
            .current_dir(root.path())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    #[test]
    fn startup_opt_in_is_reversible_and_preserves_other_edits() {
        let root = tempfile::tempdir().unwrap();
        let rc = root.path().join(".bashrc");
        let config = root.path().join("odd ' $name.fastfetch.jsonc");
        std::fs::write(&config, "{\"modules\":[\"os\"]}").unwrap();
        let before = "# user's setup\nalias hi='echo hi'";
        std::fs::write(&rc, before).unwrap();
        assert_eq!(current(&rc).unwrap(), None);
        let plan = prepare(rc.clone(), Some(Target::open(config.clone()).unwrap())).unwrap();
        assert_eq!(std::fs::read_to_string(&rc).unwrap(), before);
        assert!(plan.snippet.contains("'\\''"));
        let backup = apply(&plan, root.path()).unwrap().unwrap();
        assert_eq!(std::fs::read_to_string(backup).unwrap(), before);
        assert_eq!(current(&rc).unwrap(), Some(config));
        assert!(
            std::process::Command::new("/bin/bash")
                .args(["--noprofile", "--norc", "-n"])
                .arg(&rc)
                .env_remove("BASH_ENV")
                .env_remove("ENV")
                .status()
                .unwrap()
                .success()
        );
        let after = format!("{}# added later\n", std::fs::read_to_string(&rc).unwrap());
        std::fs::write(&rc, after).unwrap();
        let plan = prepare(rc.clone(), None).unwrap();
        apply(&plan, root.path()).unwrap();
        assert_eq!(
            std::fs::read_to_string(&rc).unwrap(),
            format!("{before}\n# added later\n")
        );
        assert_eq!(current(&rc).unwrap(), None);
    }
    #[test]
    fn startup_conflicts_aliases_and_modified_blocks_are_never_overwritten() {
        use std::os::unix::fs::PermissionsExt;
        let root = tempfile::tempdir().unwrap();
        let rc = root.path().join(".bashrc");
        let config = root.path().join("config.jsonc");
        std::fs::write(&config, "{}").unwrap();
        let target = Target::open(config.clone()).unwrap();
        let plan = prepare(rc.clone(), Some(target.clone())).unwrap();
        std::fs::write(&rc, "# external").unwrap();
        assert!(apply(&plan, root.path()).is_err());
        let plan = prepare(rc.clone(), Some(target.clone())).unwrap();
        std::fs::write(&config, "// conflict\n{}").unwrap();
        assert!(apply(&plan, root.path()).is_err());
        std::fs::write(&config, "{}").unwrap();
        std::fs::set_permissions(&rc, std::fs::Permissions::from_mode(0o400)).unwrap();
        assert!(apply(&plan, root.path()).is_err());
        std::fs::set_permissions(&rc, std::fs::Permissions::from_mode(0o600)).unwrap();
        std::fs::write(&rc, "fastfetch\n").unwrap();
        assert!(prepare(rc.clone(), Some(target.clone())).is_err());
        std::fs::write(&rc, "if broken; then\n").unwrap();
        assert!(prepare(rc.clone(), Some(target.clone())).is_err());
        let sentinel = root.path().join("DO_NOT_EXECUTE");
        std::fs::write(&rc, format!("touch '{}'\n", sentinel.display())).unwrap();
        prepare(rc.clone(), Some(target.clone())).unwrap();
        assert!(!sentinel.exists(), "syntax checking must never run .bashrc");
        let body = block(&config).unwrap();
        for invalid in [body.replace("-t 1", "-t 2"), format!("{body}{body}")] {
            std::fs::write(&rc, &invalid).unwrap();
            assert!(prepare(rc.clone(), None).is_err());
            assert_eq!(std::fs::read_to_string(&rc).unwrap(), invalid);
        }
        std::fs::remove_file(&rc).unwrap();
        std::os::unix::fs::symlink(&config, &rc).unwrap();
        assert!(prepare(rc, Some(target)).is_err());
    }
}
