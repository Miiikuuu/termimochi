use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicU64, Ordering},
};

use serde_json::Value;

static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

const VALID_PALETTE: &str = r#"[Palette]
Name=CLI Contract

[Light]
Foreground=#000000
Background=#FFFFFF
Cursor=#000000
CursorForeground=#FFFFFF
Color0=#FFFFFF
Color1=#810000
Color2=#006400
Color3=#725B00
Color4=#003EAA
Color5=#700070
Color6=#00606A
Color7=#D0D0D0
Color8=#505050
Color9=#A00000
Color10=#007000
Color11=#806600
Color12=#0050C0
Color13=#800080
Color14=#007080
Color15=#F8F8F8
"#;

const INVALID_CONTRAST_PALETTE: &str = r#"[Palette]
Name=Low Contrast

[Light]
Foreground=#777777
Background=#FFFFFF
Color0=#777777
Color1=#810000
Color2=#006400
Color3=#725B00
Color4=#003EAA
Color5=#700070
Color6=#00606A
Color7=#D0D0D0
Color8=#505050
Color9=#A00000
Color10=#007000
Color11=#806600
Color12=#0050C0
Color13=#800080
Color14=#007080
Color15=#F8F8F8
"#;

struct FixtureDir(PathBuf);

impl FixtureDir {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "termimochi-cli-contract-{}-{}",
            std::process::id(),
            NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).expect("create CLI test fixture directory");
        Self(path)
    }

    fn write(&self, name: &str, contents: &str) -> PathBuf {
        let path = self.0.join(name);
        fs::write(&path, contents).expect("write CLI test fixture");
        path
    }
}

impl Drop for FixtureDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn run(arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_termimochi-cli"))
        .args(arguments)
        .output()
        .expect("run termimochi-cli")
}

fn run_with_path(command: &str, path: &Path, arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_termimochi-cli"))
        .arg(command)
        .arg(path)
        .args(arguments)
        .output()
        .expect("run termimochi-cli")
}

fn stdout(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).expect("stdout is UTF-8")
}

fn stderr(output: &Output) -> String {
    String::from_utf8(output.stderr.clone()).expect("stderr is UTF-8")
}

#[test]
fn help_and_usage_errors_have_stable_streams_and_exit_codes() {
    let help = run(&[]);
    assert_eq!(help.status.code(), Some(0));
    assert!(stdout(&help).contains("Usage:"));
    assert!(help.stderr.is_empty());

    let unknown = run(&["unknown"]);
    assert_eq!(unknown.status.code(), Some(2));
    assert!(unknown.stdout.is_empty());
    assert!(stderr(&unknown).contains("unknown command"));

    let incomplete = run(&["export", "missing.palette", "--variant"]);
    assert_eq!(incomplete.status.code(), Some(2));
    assert!(incomplete.stdout.is_empty());
    assert!(stderr(&incomplete).contains("--variant expects a value"));
}

#[test]
fn inspect_reports_palette_contract_on_stdout() {
    let fixture = FixtureDir::new();
    let palette = fixture.write("valid.palette", VALID_PALETTE);

    let output = run_with_path("inspect", &palette, &[]);

    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());
    let rendered = stdout(&output);
    assert!(rendered.contains("Name: CLI Contract"));
    assert!(rendered.contains("[Light]"));
    assert!(rendered.contains("Required colors: complete"));
}

#[test]
fn lint_json_distinguishes_pass_failure_and_parse_error() {
    let fixture = FixtureDir::new();
    let valid = fixture.write("valid.palette", VALID_PALETTE);
    let invalid = fixture.write("invalid.palette", INVALID_CONTRAST_PALETTE);
    let malformed = fixture.write("malformed.palette", "[Palette]\nName=Broken\n");

    let passed = run_with_path("lint", &valid, &["--target", "general", "--json"]);
    assert_eq!(passed.status.code(), Some(0));
    assert!(passed.stderr.is_empty());
    let passed_json: Value = serde_json::from_slice(&passed.stdout).expect("valid lint JSON");
    assert_eq!(passed_json["palette"], "CLI Contract");
    assert_eq!(passed_json["target"], "general");
    assert_eq!(passed_json["passed"], true);

    let failed = run_with_path("lint", &invalid, &["--target", "codex", "--json"]);
    assert_eq!(failed.status.code(), Some(1));
    assert!(failed.stderr.is_empty());
    let failed_json: Value = serde_json::from_slice(&failed.stdout).expect("valid lint JSON");
    assert_eq!(failed_json["passed"], false);
    assert!(
        failed_json["issues"]
            .as_array()
            .expect("issues array")
            .iter()
            .any(|issue| issue["code"] == "body-text-low-contrast")
    );

    let parse_error = run_with_path("lint", &malformed, &["--json"]);
    assert_eq!(parse_error.status.code(), Some(2));
    assert!(parse_error.stdout.is_empty());
    assert!(stderr(&parse_error).contains("TermiMochi:"));
}

#[test]
fn every_export_format_has_a_process_level_contract() {
    let fixture = FixtureDir::new();
    let palette = fixture.write("valid.palette", VALID_PALETTE);
    let cases = [
        ("kitty", "foreground #000000", "color15 #F8F8F8"),
        ("ghostty", "foreground = #000000", "palette = 15=#F8F8F8"),
        ("wezterm", "foreground = '#000000'", "brights = {"),
        ("alacritty", "[colors.primary]", "white = \"#F8F8F8\""),
    ];

    for (format, first_contract, second_contract) in cases {
        let output = run_with_path(
            "export",
            &palette,
            &["--variant", "light", "--format", format],
        );
        assert_eq!(
            output.status.code(),
            Some(0),
            "{format}: {}",
            stderr(&output)
        );
        assert!(output.stderr.is_empty(), "{format}");
        let rendered = stdout(&output);
        assert!(rendered.contains(first_contract), "{format}");
        assert!(rendered.contains(second_contract), "{format}");
    }
}

#[test]
fn export_output_file_is_written_without_stdout_noise() {
    let fixture = FixtureDir::new();
    let palette = fixture.write("valid.palette", VALID_PALETTE);
    let destination = fixture.0.join("theme.toml");

    let output = Command::new(env!("CARGO_BIN_EXE_termimochi-cli"))
        .arg("export")
        .arg(&palette)
        .args(["--variant", "light", "--format", "alacritty", "--output"])
        .arg(&destination)
        .output()
        .expect("run termimochi-cli");

    assert_eq!(output.status.code(), Some(0));
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
    assert!(
        fs::read_to_string(destination)
            .expect("read exported file")
            .contains("[colors.bright]")
    );
}
