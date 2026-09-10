//! A bounded, input-preserving guard for explicitly installed Kitty greetings.
use std::{
    io::IsTerminal,
    path::Path,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

pub(crate) const ARG: &str = "--termimochi-settle-kitty";
const QUIET: Duration = Duration::from_millis(300);
const LIMIT: Duration = Duration::from_millis(1500);

#[derive(Default)]
struct Stability {
    size: Option<(u16, u16)>,
    changed: Duration,
}
impl Stability {
    fn observe(&mut self, elapsed: Duration, size: Option<(u16, u16)>) -> bool {
        if size.is_none() || size != self.size {
            self.size = size;
            self.changed = elapsed;
        }
        size.is_some() && elapsed.saturating_sub(self.changed) >= QUIET
    }
}

fn parse_size(bytes: &[u8]) -> Option<(u16, u16)> {
    let text = std::str::from_utf8(bytes).ok()?;
    let mut parts = text.split_whitespace();
    let rows = parts.next()?.parse().ok()?;
    let columns = parts.next()?.parse().ok()?;
    (parts.next().is_none() && rows > 0 && columns > 0).then_some((rows, columns))
}

pub(crate) fn run() {
    // No queries or reads from the keyboard: early typed input remains in the
    // shell's input queue. No GTK, configuration reads, clear or redraw commands.
    let kitty = std::env::var_os("KITTY_WINDOW_ID").is_some()
        || std::env::var("TERM").is_ok_and(|s| s == "xterm-kitty");
    if !kitty || !std::io::stdout().is_terminal() {
        return;
    }
    let Ok(tty) = std::fs::File::open("/dev/tty") else {
        return;
    };
    let started = Instant::now();
    let mut stability = Stability::default();
    while started.elapsed() < LIMIT {
        let Ok(input) = tty.try_clone() else {
            return;
        };
        let size = Command::new("/usr/bin/stty")
            .arg("size")
            .stdin(input)
            .stderr(Stdio::null())
            .output()
            .ok()
            .filter(|r| r.status.success())
            .and_then(|r| parse_size(&r.stdout));
        if stability.observe(started.elapsed(), size) {
            return;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    eprintln!("TermiMochi: terminal is still resizing; verify the greeting after startup.");
}

pub(super) fn configuration(source: &str, helper: &Path) -> Result<String, String> {
    use jsonc_parser::cst::CstInputValue;
    let helper = helper
        .to_str()
        .filter(|s| !s.chars().any(char::is_control))
        .filter(|_| helper.is_absolute())
        .ok_or("Invalid startup helper path.")?;
    let root = crate::fastfetch_document::parse(source)?;
    let object = root.object_value().ok_or("Missing configuration object.")?;
    if object.get("general").is_none() {
        object.append("general", CstInputValue::Object(vec![]));
    }
    let general = object
        .get("general")
        .and_then(|p| p.object_value())
        .ok_or("Invalid Fastfetch general settings.")?;
    let values = crate::fastfetch_document::value(source)?;
    let previous = match values.get("general").and_then(|v| v.get("preRun")) {
        None => "",
        Some(value) => value.as_str().ok_or("Invalid Fastfetch preRun command.")?,
    };
    let guard = format!("'{}' {ARG}", helper.replace('\'', "'\\''"));
    let command = if previous == guard || previous.starts_with(&format!("{guard};\n")) {
        previous.to_owned()
    } else if previous.is_empty() {
        guard
    } else {
        format!("{guard};\n{previous}")
    };
    if let Some(property) = general.get("preRun") {
        property.set_value(CstInputValue::String(command));
    } else {
        general.append("preRun", CstInputValue::String(command));
    }
    let result = root.to_string();
    crate::fastfetch_document::parse(&result)?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires the normal worker binary and Python PTY; queued input and resize safety"]
    fn startup_guard_preserves_queued_input_termios_and_handles_resize() {
        let helper = std::env::var_os("TERMIMOCHI_SVG_WORKER_BIN").unwrap();
        let script = r#"
import fcntl, os, pty, select, struct, subprocess, sys, termios, time
helper = sys.argv[1]
pid, fd = pty.fork()
if pid == 0:
    os.environ['KITTY_WINDOW_ID'] = 'test'
    fcntl.ioctl(0, termios.TIOCSWINSZ, struct.pack('HHHH', 36, 100, 0, 0))
    original = termios.tcgetattr(0)
    print('GUARD_START', flush=True)
    started = time.monotonic()
    subprocess.run([helper, '--termimochi-settle-kitty'], check=True, timeout=3)
    elapsed = time.monotonic() - started
    assert termios.tcgetattr(0) == original, 'TTY input mode changed'
    assert sys.stdin.readline() == 'typed early\n', 'Queued keyboard input was consumed'
    assert 0.45 <= elapsed < 2.5, elapsed
    print('INPUT_RETAINED', flush=True)
    os._exit(0)
data = b''
deadline = time.monotonic() + 4
while b'GUARD_START' not in data:
    assert time.monotonic() < deadline
    if select.select([fd], [], [], 0.05)[0]: data += os.read(fd, 4096)
os.write(fd, b'typed early\n')
time.sleep(0.1)
fcntl.ioctl(fd, termios.TIOCSWINSZ, struct.pack('HHHH', 30, 90, 0, 0))
time.sleep(0.1)
fcntl.ioctl(fd, termios.TIOCSWINSZ, struct.pack('HHHH', 32, 80, 0, 0))
while time.monotonic() < deadline:
    if select.select([fd], [], [], 0.05)[0]:
        try: chunk = os.read(fd, 4096)
        except OSError: break
        if not chunk: break
        data += chunk
    if b'INPUT_RETAINED' in data: break
assert b'INPUT_RETAINED' in data, data
assert os.waitpid(pid, 0)[1] == 0, data
os.close(fd)
result = subprocess.run([helper, '--termimochi-settle-kitty'], input=b'untouched',
    stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=2,
    env=dict(os.environ, KITTY_WINDOW_ID='test'))
assert result.returncode == 0 and not result.stdout and not result.stderr, result
"#;
        let output = Command::new("python3")
            .arg("-c")
            .arg(script)
            .arg(helper)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    #[test]
    fn size_changes_restart_quiet_window_and_invalid_sizes_never_pass() {
        let mut state = Stability::default();
        assert!(!state.observe(Duration::ZERO, Some((36, 100))));
        assert!(!state.observe(Duration::from_millis(299), Some((36, 100))));
        assert!(!state.observe(Duration::from_millis(300), Some((32, 80))));
        assert!(!state.observe(Duration::from_millis(599), Some((32, 80))));
        assert!(state.observe(Duration::from_millis(600), Some((32, 80))));
        assert!(!state.observe(Duration::from_millis(700), None));
        assert_eq!(parse_size(b"36 100\n"), Some((36, 100)));
        for text in ["0 100", "-1 100", "99999 100", "36 100 x", "bad"] {
            assert!(parse_size(text.as_bytes()).is_none());
        }
    }
    #[test]
    fn reviewed_guard_preserves_existing_commands_comments_and_quotes_once() {
        let source = "// keep\n{\"general\":{\"preRun\":\"printf original\",\"thread\":false},\"modules\":[\"os\"]}";
        let helper = Path::new("/tmp/user's helper $(literal)");
        let result = configuration(source, helper).unwrap();
        assert!(result.contains("// keep"));
        let value = crate::fastfetch_document::value(&result).unwrap();
        assert_eq!(value["general"]["thread"], false);
        assert_eq!(
            value["general"]["preRun"],
            "'/tmp/user'\\''s helper $(literal)' --termimochi-settle-kitty;\nprintf original"
        );
        assert_eq!(configuration(&result, helper).unwrap(), result);
        assert!(configuration(source, Path::new("relative")).is_err());
        assert!(configuration("{\"general\":{\"preRun\":false}}", helper).is_err());
    }
}
