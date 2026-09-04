use std::{
    env,
    ffi::{OsStr, OsString},
    io::{self, Write},
    path::{Path, PathBuf},
    process::ExitCode,
    str::FromStr,
};

use serde_json::json;
use termimochi_core::{
    ExportFormat, PtyxisPalette, Severity, Target, Variant, contrast_ratio, export_palette,
    lint_palette, paths_refer_to_same_file, write_atomically,
};

const HELP: &str = "\
TermiMochi · terminal palette diagnostics

Usage:
  termimochi-cli inspect <PALETTE>
  termimochi-cli lint <PALETTE> [--target general|codex] [--json]
  termimochi-cli export <PALETTE> --variant light|dark \
    --format kitty|ghostty|wezterm|alacritty [--output PATH]
";

#[derive(Debug)]
enum CliError {
    Message(String),
    Output(io::Error),
}

impl CliError {
    fn message(message: impl Into<String>) -> Self {
        Self::Message(message.into())
    }
}

impl std::fmt::Display for CliError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Message(message) => formatter.write_str(message),
            Self::Output(error) => write!(formatter, "cannot write output: {error}"),
        }
    }
}

fn main() -> ExitCode {
    let mut stdout = io::stdout().lock();
    match run(env::args_os().skip(1).collect(), &mut stdout) {
        Ok(code) => ExitCode::from(code),
        Err(CliError::Output(error)) if error.kind() == io::ErrorKind::BrokenPipe => {
            ExitCode::SUCCESS
        }
        Err(error) => {
            let _ = writeln!(io::stderr().lock(), "TermiMochi: {error}");
            ExitCode::from(2)
        }
    }
}

fn run(arguments: Vec<OsString>, output: &mut dyn Write) -> Result<u8, CliError> {
    let Some(command) = arguments.first() else {
        emit(output, HELP)?;
        return Ok(0);
    };
    if is_help(command) {
        emit(output, HELP)?;
        return Ok(0);
    }
    let command = command
        .to_str()
        .ok_or_else(|| CliError::message("command is not valid UTF-8"))?;

    match command {
        "inspect" => inspect(&arguments[1..], output),
        "lint" => lint(&arguments[1..], output),
        "export" => export(&arguments[1..], output),
        _ => Err(CliError::message(format!(
            "unknown command {command:?}\n\n{HELP}"
        ))),
    }
}

fn inspect(arguments: &[OsString], output: &mut dyn Write) -> Result<u8, CliError> {
    if subcommand_help(arguments) {
        emit(output, HELP)?;
        return Ok(0);
    }
    let [path] = arguments else {
        return Err(CliError::message(
            "inspect expects exactly one palette path",
        ));
    };
    let palette = load(path)?;
    let mut rendered = format!("Name: {}\n", palette.name());
    if let Some(source) = palette.source() {
        rendered.push_str(&format!("Source: {}\n", source.display()));
    }

    for variant_kind in Variant::ALL {
        let Some(variant) = palette.variant(variant_kind) else {
            continue;
        };
        rendered.push_str(&format!("\n[{}]\n", variant_kind.section_name()));
        if let (Some(foreground), Some(background)) =
            (variant.get("Foreground"), variant.get("Background"))
        {
            rendered.push_str(&format!(
                "Body: {foreground} on {background} ({:.2}:1)\n",
                contrast_ratio(foreground, background)
            ));
        }
        if let (Some(foreground), Some(color0)) = (variant.get("Foreground"), variant.get("Color0"))
        {
            rendered.push_str(&format!(
                "Codex composer simulation: {foreground} on {color0} ({:.2}:1)\n",
                contrast_ratio(foreground, color0)
            ));
        }
        rendered.push_str(&format!("Known colors: {}\n", variant.colors().len()));
        let missing = variant.missing_required_keys();
        if missing.is_empty() {
            rendered.push_str("Required colors: complete\n");
        } else {
            rendered.push_str(&format!("Required colors: {}\n", missing.join(", ")));
        }
    }
    emit(output, &rendered)?;
    Ok(0)
}

fn lint(arguments: &[OsString], output: &mut dyn Write) -> Result<u8, CliError> {
    if subcommand_help(arguments) {
        emit(output, HELP)?;
        return Ok(0);
    }
    let Some(path) = arguments.first() else {
        return Err(CliError::message("lint expects a palette path"));
    };
    let mut target = Target::Codex;
    let mut json_output = false;
    let mut index = 1;
    while index < arguments.len() {
        let option = utf8_option(&arguments[index], "lint option")?;
        match option {
            "--json" => json_output = true,
            "--target" => {
                index += 1;
                let value = arguments
                    .get(index)
                    .ok_or_else(|| CliError::message("--target expects general or codex"))?;
                let value = value
                    .to_str()
                    .ok_or_else(|| CliError::message("lint target is not valid UTF-8"))?;
                target = Target::from_str(value).map_err(CliError::message)?;
            }
            _ => return Err(CliError::message(format!("unknown lint option {option:?}"))),
        }
        index += 1;
    }

    let palette = load(path)?;
    let report = lint_palette(&palette, target);
    let rendered = if json_output {
        let issues: Vec<_> = report
            .issues
            .iter()
            .map(|issue| {
                let mut value = json!({
                    "severity": issue.severity.as_str(),
                    "code": issue.code,
                    "message": issue.message,
                });
                if let Some(variant) = issue.variant {
                    value["variant"] = json!(variant.as_str());
                }
                if let Some(ratio) = issue.ratio {
                    value["ratio"] = json!((ratio * 100.0).round() / 100.0);
                }
                value
            })
            .collect();
        format!(
            "{}\n",
            serde_json::to_string_pretty(&json!({
                "palette": report.palette_name,
                "target": report.target.as_str(),
                "passed": !report.has_errors(),
                "issues": issues,
            }))
            .expect("JSON values are serializable")
        )
    } else {
        let mut rendered = format!(
            "TermiMochi · {} · target={}\n",
            report.palette_name, report.target
        );
        for issue in &report.issues {
            let mark = match issue.severity {
                Severity::Error => "✗",
                Severity::Warning => "!",
                Severity::Note => "✓",
            };
            let scope = issue
                .variant
                .map_or_else(String::new, |variant| format!("[{variant}] "));
            let ratio = issue
                .ratio
                .map_or_else(String::new, |ratio| format!(" ({ratio:.2}:1)"));
            rendered.push_str(&format!("{mark} {scope}{}{ratio}\n", issue.message));
        }
        rendered
    };
    emit(output, &rendered)?;

    Ok(u8::from(report.has_errors()))
}

fn load(path: &OsStr) -> Result<PtyxisPalette, CliError> {
    PtyxisPalette::from_file(PathBuf::from(path))
        .map_err(|error| CliError::message(error.to_string()))
}

fn export(arguments: &[OsString], output_writer: &mut dyn Write) -> Result<u8, CliError> {
    if subcommand_help(arguments) {
        emit(output_writer, HELP)?;
        return Ok(0);
    }
    let Some(path) = arguments.first() else {
        return Err(CliError::message("export expects a palette path"));
    };
    let mut variant = None;
    let mut format = None;
    let mut output = None;
    let mut index = 1;
    while index < arguments.len() {
        let option = utf8_option(&arguments[index], "export option")?;
        index += 1;
        let value = arguments
            .get(index)
            .ok_or_else(|| CliError::message(format!("{option} expects a value")))?;
        match option {
            "--variant" => {
                let value = value
                    .to_str()
                    .ok_or_else(|| CliError::message("variant is not valid UTF-8"))?;
                variant = Some(
                    Variant::from_str(value)
                        .map_err(|error| CliError::message(error.to_string()))?,
                );
            }
            "--format" => {
                let value = value
                    .to_str()
                    .ok_or_else(|| CliError::message("format is not valid UTF-8"))?;
                format = Some(
                    ExportFormat::from_str(value)
                        .map_err(|error| CliError::message(error.to_string()))?,
                );
            }
            "--output" => output = Some(PathBuf::from(value)),
            _ => {
                return Err(CliError::message(format!(
                    "unknown export option {option:?}"
                )));
            }
        }
        index += 1;
    }

    let variant =
        variant.ok_or_else(|| CliError::message("export requires --variant light|dark"))?;
    let format = format.ok_or_else(|| {
        CliError::message("export requires --format kitty|ghostty|wezterm|alacritty")
    })?;
    let palette = load(path)?;
    let exported = export_palette(&palette, variant, format)
        .map_err(|error| CliError::message(error.to_string()))?;
    if let Some(output) = output {
        let source = Path::new(path);
        if paths_refer_to_same_file(source, &output).map_err(|error| {
            CliError::message(format!(
                "cannot verify output path {}: {error}",
                output.display()
            ))
        })? {
            return Err(CliError::message(format!(
                "refusing to overwrite source palette {}",
                source.display()
            )));
        }
        write_atomically(&output, exported.contents.as_bytes()).map_err(|error| {
            CliError::message(format!("cannot write {}: {error}", output.display()))
        })?;
    } else {
        emit(output_writer, &exported.contents)?;
    }
    Ok(0)
}

fn emit(output: &mut dyn Write, value: &str) -> Result<(), CliError> {
    output.write_all(value.as_bytes()).map_err(CliError::Output)
}

fn is_help(value: &OsStr) -> bool {
    matches!(value.to_str(), Some("-h" | "--help" | "help"))
}

fn subcommand_help(arguments: &[OsString]) -> bool {
    matches!(arguments, [argument] if is_help(argument))
}

fn utf8_option<'a>(value: &'a OsStr, kind: &str) -> Result<&'a str, CliError> {
    value
        .to_str()
        .ok_or_else(|| CliError::message(format!("{kind} is not valid UTF-8")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_TEST: AtomicU64 = AtomicU64::new(0);
    const PALETTE: &str = include_str!("../../../themes/fog-paper-codex.palette");

    fn args(values: &[&str]) -> Vec<OsString> {
        values.iter().map(OsString::from).collect()
    }

    fn fixture() -> PathBuf {
        let root = env::temp_dir().join(format!(
            "termimochi-cli-test-{}-{}",
            std::process::id(),
            NEXT_TEST.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&root).unwrap();
        root
    }

    #[test]
    fn top_level_and_subcommand_help_succeed() {
        for values in [
            &[][..],
            &["--help"][..],
            &["inspect", "--help"][..],
            &["lint", "--help"][..],
            &["export", "--help"][..],
        ] {
            let mut output = Vec::new();
            assert_eq!(run(args(values), &mut output).unwrap(), 0);
            assert!(String::from_utf8(output).unwrap().contains("Usage:"));
        }
    }

    #[test]
    fn export_refuses_to_replace_its_source() {
        let root = fixture();
        let source = root.join("source.palette");
        std::fs::write(&source, PALETTE).unwrap();
        let arguments = vec![
            OsString::from("export"),
            source.clone().into_os_string(),
            OsString::from("--variant"),
            OsString::from("light"),
            OsString::from("--format"),
            OsString::from("kitty"),
            OsString::from("--output"),
            source.clone().into_os_string(),
        ];

        let error = run(arguments, &mut Vec::new()).unwrap_err();

        assert!(error.to_string().contains("refusing to overwrite"));
        assert_eq!(std::fs::read_to_string(&source).unwrap(), PALETTE);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn non_utf8_paths_are_reported_without_panicking() {
        use std::os::unix::ffi::OsStringExt;

        let path = OsString::from_vec(b"/tmp/termimochi-\xff.palette".to_vec());
        let result = run(vec![OsString::from("inspect"), path], &mut Vec::new());
        assert!(matches!(result, Err(CliError::Message(_))));
    }

    struct BrokenPipe;

    impl Write for BrokenPipe {
        fn write(&mut self, _buffer: &[u8]) -> io::Result<usize> {
            Err(io::Error::from(io::ErrorKind::BrokenPipe))
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn broken_pipe_is_distinguishable_from_usage_errors() {
        assert!(matches!(
            run(args(&["--help"]), &mut BrokenPipe),
            Err(CliError::Output(error)) if error.kind() == io::ErrorKind::BrokenPipe
        ));
    }
}
