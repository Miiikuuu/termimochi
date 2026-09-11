//! Runs before GTK inside a newly opened terminal, never inside a user shell.
use super::*;
use std::{
    io::{self, IsTerminal, Read, Write},
    process::Stdio,
    time::{Duration, Instant},
};

const QUERY_ID: u32 = 317041;

struct TtyMode(String);
impl TtyMode {
    fn enter() -> Result<Self, String> {
        if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
            return Err(
                "A real terminal TTY is required; redirected output is not a capability test."
                    .into(),
            );
        }
        let saved = Command::new("/usr/bin/stty")
            .arg("-g")
            .stdin(Stdio::inherit())
            .output()
            .map_err(|e| e.to_string())?;
        if !saved.status.success() {
            return Err("Cannot read terminal input mode.".into());
        }
        let saved = String::from_utf8(saved.stdout)
            .map_err(|e| e.to_string())?
            .trim()
            .to_owned();
        if saved.is_empty() || !saved.bytes().all(|b| b.is_ascii_hexdigit() || b == b':') {
            return Err("Invalid saved terminal mode.".into());
        }
        let guard = Self(saved);
        if !Command::new("/usr/bin/stty")
            .args(["-echo", "-icanon", "min", "0", "time", "1"])
            .status()
            .map_err(|e| e.to_string())?
            .success()
        {
            return Err("Cannot prepare terminal query input.".into());
        }
        Ok(guard)
    }
}
impl Drop for TtyMode {
    fn drop(&mut self) {
        let _ = Command::new("/usr/bin/stty").arg(&self.0).status();
    }
}

fn csi(bytes: &[u8], final_byte: u8) -> Vec<String> {
    let text = String::from_utf8_lossy(bytes);
    text.split("\x1b[")
        .skip(1)
        .filter_map(|tail| {
            let end = tail.bytes().position(|b| (0x40..=0x7e).contains(&b))?;
            (tail.as_bytes()[end] == final_byte && end < 128).then(|| tail[..end].to_owned())
        })
        .collect()
}

/// DA's first parameter is terminal identity, not a feature flag. Only VT220+
/// feature parameters can advertise Sixel. Timeouts remain unverified.
pub(super) fn parse_response(bytes: &[u8], id: u32) -> Assessment {
    let bytes = &bytes[..bytes.len().min(4096)];
    let mut result = Assessment::default();
    let expected = format!("\x1b_Gi={id};");
    let text = String::from_utf8_lossy(bytes);
    if let Some(tail) = text
        .split(&expected)
        .nth(1)
        .and_then(|s| s.split_once("\x1b\\"))
    {
        result.kitty = if tail.0 == "OK" {
            Support::Advertised
        } else {
            Support::Unavailable
        };
    }
    let da = csi(bytes, b'c')
        .into_iter()
        .filter_map(|p| {
            let values: Option<Vec<u32>> = p
                .strip_prefix('?')?
                .split(';')
                .map(|n| n.parse().ok())
                .collect();
            values
        })
        .next();
    if let Some(da) = da {
        if result.kitty == Support::Unverified {
            result.kitty = Support::Unavailable;
        }
        if da.first().is_some_and(|v| *v >= 62) {
            result.sixel = if da[1..].contains(&4) {
                Support::Advertised
            } else {
                Support::Unavailable
            };
        }
    }
    result.cells = csi(bytes, b't').into_iter().find_map(|p| {
        let values: Vec<_> = p.split(';').filter_map(|n| n.parse::<u32>().ok()).collect();
        if values.len() == 3 && values[0] == 6 && values[1..].iter().all(|v| (1..=256).contains(v))
        {
            Some([values[2], values[1]])
        } else {
            None
        }
    });
    result
}

fn query() -> Result<Assessment, String> {
    print!("\x1b_Gi={QUERY_ID},s=1,v=1,a=q,t=d,f=24;AAAA\x1b\\\x1b[c\x1b[16t");
    io::stdout().flush().map_err(|e| e.to_string())?;
    let started = Instant::now();
    let mut bytes = Vec::new();
    while started.elapsed() < Duration::from_millis(900) && bytes.len() < 4096 {
        let mut buffer = [0; 256];
        let n = io::stdin().read(&mut buffer).map_err(|e| e.to_string())?;
        bytes.extend_from_slice(&buffer[..n]);
    }
    Ok(parse_response(&bytes, QUERY_ID))
}

fn publish(directory: &Path, assessment: &Assessment) -> Result<(), String> {
    typography_preset::write_private(
        &directory.join("result.json"),
        &serde_json::to_vec(assessment).map_err(|e| e.to_string())?,
    )
}

fn key(directory: &Path, allowed: &[u8], timeout: Duration) -> Result<Option<u8>, String> {
    let started = Instant::now();
    while directory.is_dir() && started.elapsed() < timeout {
        let path = directory.join("gui-response");
        if let Some(response) = typography_preset::read_private_with_limit(&path, 32)? {
            let _ = std::fs::remove_file(&path);
            let stage = if allowed == b"ynq" {
                "visual:"
            } else if allowed == b"tq" {
                "unknown:"
            } else {
                "close:"
            };
            if let Some(key) = response
                .strip_prefix(stage.as_bytes())
                .filter(|v| v.len() == 1)
                .map(|v| v[0])
                .filter(|v| allowed.contains(v))
            {
                return Ok(Some(key));
            }
        }
        let mut buffer = [0; 64];
        let n = io::stdin().read(&mut buffer).map_err(|e| e.to_string())?;
        // Do not interpret protocol responses, pasted commands or escape keys as
        // a visual approval. An approval is one explicit key, optionally newline.
        let input: Vec<_> = buffer[..n]
            .iter()
            .copied()
            .filter(|b| !b.is_ascii_whitespace())
            .collect();
        if input.len() == 1 && allowed.contains(&input[0].to_ascii_lowercase()) {
            return Ok(Some(input[0].to_ascii_lowercase()));
        }
    }
    Ok(None)
}

/// The trial is artwork + fixed safe fields, NOT arbitrary imported modules.
pub(super) fn safe_config(
    source: &str,
    directory: &Path,
    protocol: Protocol,
    ansi: bool,
) -> Result<String, String> {
    let source = crate::fastfetch_document::value(source)?;
    let mut logo = source.get("logo").cloned().ok_or("Missing trial logo.")?;
    if !ansi {
        if logo.get("type").and_then(|v| v.as_str()) != Some(protocol.logo_type())
            || logo.get("source").and_then(|v| v.as_str())
                != directory.join(protocol.source_name()).to_str()
        {
            return Err("The trial logo does not match its local asset and protocol.".into());
        }
        // Rebuild only the known layout keys, not arbitrary Fastfetch options.
        let mut safe = serde_json::json!({"type":protocol.logo_type(), "source":directory.join(protocol.source_name()), "printRemaining":true});
        for name in ["width", "height"] {
            let value = logo
                .get(name)
                .and_then(|v| v.as_u64())
                .filter(|v| (1..=120).contains(v))
                .ok_or("Invalid logo geometry.")?;
            safe[name] = value.into();
        }
        logo = safe;
    } else {
        // Existing ANSI exports may use data-raw. Keep only validated logo text;
        // external raw/image files and shell logo commands cannot enter a trial.
        let kind = logo.get("type").and_then(|v| v.as_str()).unwrap_or("none");
        if !["data", "data-raw", "none", "builtin"].contains(&kind) {
            return Err(
                "This ANSI fallback uses an external logo. Export portable ANSI artwork first."
                    .into(),
            );
        }
        let text = logo.get("source").and_then(|v| v.as_str()).unwrap_or("");
        if kind == "data-raw" || kind == "data" {
            let art = crate::greeting_art::Artwork::parse(text)?;
            logo = serde_json::json!({"type":"data-raw", "source":art.ansi});
        } else {
            logo = serde_json::json!({"type":kind, "source":text});
        }
    }
    if let Some(original) = source.get("logo") {
        logo["position"] = match original.get("position").and_then(|v| v.as_str()) {
            Some("right") => "right",
            Some("top") => "top",
            _ => "left",
        }
        .into();
        let mut padding = serde_json::Map::new();
        for name in ["left", "right", "top"] {
            if let Some(value) = original.get("padding").and_then(|p| p.get(name)) {
                let value = value
                    .as_u64()
                    .filter(|v| *v <= 120)
                    .ok_or("Invalid trial logo padding.")?;
                padding.insert(name.into(), value.into());
            }
        }
        logo["padding"] = padding.into();
        logo["printRemaining"] = true.into();
    }
    serde_json::to_string_pretty(&serde_json::json!({"logo":logo,"modules":[{"type":"custom","format":"TermiMochi · Image Trial"},"separator","os","shell","colors"]})).map_err(|e| e.to_string())
}

fn run_fastfetch(directory: &Path, config: &str) -> Result<(), String> {
    let path = directory.join("trial.fastfetch.jsonc");
    write(&path, config.as_bytes())?;
    let mut child = Command::new("/usr/bin/fastfetch")
        .args(["--pipe", "false", "--show-errors", "true"])
        .arg("--config")
        .arg(&path)
        .current_dir("/")
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|e| e.to_string())?;
    let started = Instant::now();
    loop {
        if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
            return if status.success() {
                Ok(())
            } else {
                Err(format!(
                    "Fastfetch exited with {status}. Output is not verified; choose ANSI fallback."
                ))
            };
        }
        if !directory.is_dir() || started.elapsed() > Duration::from_secs(12) {
            let _ = child.kill();
            let _ = child.wait();
            return Err("Trial cancelled or Fastfetch timed out. Output is not verified.".into());
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn run(directory: &Path) -> Result<(), String> {
    validate_directory(directory)?;
    let request: Request = serde_json::from_slice(
        &typography_preset::read_private(&directory.join("request.json"))?
            .ok_or("Missing trial request.")?,
    )
    .map_err(|e| e.to_string())?;
    if request.version != 1 || !(8..=120).contains(&request.columns) {
        return Err("Invalid trial request.".into());
    }
    let names = payload_names(request.protocol);
    if request.hashes.len() != names.len() {
        return Err("Invalid trial manifest.".into());
    }
    for name in names {
        if request.hashes.get(name) != Some(&hash(&read(&directory.join(name))?)) {
            return Err("Trial files changed before execution; no graphics were sent. Prepare a fresh trial.".into());
        }
    }
    let _mode = TtyMode::enter()?;
    let mut assessment = query()?;
    let support = if request.protocol == Protocol::Sixel {
        assessment.sixel
    } else {
        assessment.kitty
    };
    if !request.ansi && support == Support::Unavailable {
        assessment.done = true;
        assessment.message = "This terminal session reports no support for the selected pixel protocol. No pixel stream was sent. Choose Use Character & Try in TermiMochi.".into();
        publish(directory, &assessment)?;
        println!("\n{}\nPress Q to close.", assessment.message);
        let _ = key(directory, b"q", Duration::from_secs(45));
        return Ok(());
    }
    if !request.ansi && support == Support::Unverified {
        assessment.message = "No conclusive protocol response. In the trial terminal press T to try this unverified output, or Q to cancel. ANSI fallback is available in the app.".into();
        publish(directory, &assessment)?;
        println!("\n{}", assessment.message);
        if key(directory, b"tq", Duration::from_secs(60))? != Some(b't') {
            assessment.done = true;
            assessment.message = "Unverified trial cancelled; no pixel stream sent.".into();
            return publish(directory, &assessment);
        }
    }
    if !request.ansi
        && request.protocol == Protocol::Sixel
        && let Some(cells) = assessment.cells
    {
        let (stream, size) = pixel_export::resize_sixel(
            &read(&directory.join("logo.png"))?,
            request.columns,
            cells,
        )?;
        write(&directory.join("logo.sixel"), &stream)?;
        let source =
            String::from_utf8(read(&directory.join("config.jsonc"))?).map_err(|e| e.to_string())?;
        let root = crate::fastfetch_document::parse(&source)?;
        let logo = root
            .object_value()
            .and_then(|o| o.get("logo"))
            .and_then(|p| p.object_value())
            .ok_or("Missing logo")?;
        for (name, size) in [("width", size.0), ("height", size.1)] {
            logo.get(name)
                .ok_or("Missing geometry")?
                .set_value(jsonc_parser::cst::CstInputValue::Number(size.to_string()));
        }
        write(&directory.join("config.jsonc"), root.to_string().as_bytes())?;
    }
    let config_name = if request.ansi {
        "config-ansi.jsonc"
    } else {
        "config.jsonc"
    };
    let source =
        String::from_utf8(read(&directory.join(config_name))?).map_err(|e| e.to_string())?;
    let config = safe_config(&source, directory, request.protocol, request.ansi)?;
    assessment.message =
        "Rendering in the target terminal. Exit code alone does not verify the image.".into();
    publish(directory, &assessment)?;
    print!("\x1b[2J\x1b[H");
    io::stdout().flush().map_err(|e| e.to_string())?;
    run_fastfetch(directory, &config)?;
    assessment.asset_hash = Some(hash(&read(&directory.join(if request.ansi {
        "config-ansi.jsonc"
    } else {
        request.protocol.source_name()
    }))?));
    assessment.config_hash = Some(hash(source.as_bytes()));
    assessment.message = if request.protocol == Protocol::KittyAnimation && !request.ansi { "Does the animation move correctly, with no trails or clipping? Press Y to confirm, N if broken, or Q to leave unverified." } else { "Is the artwork visible and correctly placed, with the expected background? Press Y to confirm, N if broken, or Q to leave unverified." }.into();
    publish(directory, &assessment)?;
    println!("\n\n{}", assessment.message);
    assessment.visual = match key(directory, b"ynq", Duration::from_secs(120))? {
        Some(b'y') => Visual::Confirmed,
        Some(b'n') => Visual::Failed,
        _ => Visual::Unverified,
    };
    if request.protocol == Protocol::KittyAnimation && !request.ansi {
        assessment.animation = assessment.visual;
    }
    assessment.done = true;
    assessment.message = match assessment.visual { Visual::Confirmed => "Visually confirmed for this artwork, terminal session and output only. Return to TermiMochi to review installation.", Visual::Failed => "Visual check failed. Use Character & Try or change target/protocol; installation is blocked.", Visual::Unverified => "Visual appearance was not confirmed. Installation is blocked; retry or choose Character." }.into();
    publish(directory, &assessment)?;
    println!("\n{}\nPress Q to close.", assessment.message);
    let _ = key(directory, b"q", Duration::from_secs(45));
    Ok(())
}

pub(crate) fn worker_entry() -> glib::ExitCode {
    let Some(directory) = std::env::args_os().nth(2).map(PathBuf::from) else {
        return glib::ExitCode::FAILURE;
    };
    match run(&directory) {
        Ok(()) => glib::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("TermiMochi trial: {error}");
            if validate_directory(&directory).is_ok() {
                let _ = publish(
                    &directory,
                    &Assessment {
                        done: true,
                        message: error,
                        ..Default::default()
                    },
                );
            }
            glib::ExitCode::FAILURE
        }
    }
}
