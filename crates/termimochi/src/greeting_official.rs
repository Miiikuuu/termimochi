//! Pinned, reviewed upstream presets. No user-supplied Fastfetch document is
//! accepted. Preserve module objects exactly; order/toggles reference their IDs.
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    process::Command,
    time::{Duration, Instant},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum OfficialPreset {
    Neofetch,
    Screenfetch,
    Paleofetch,
    Icons,
    Bars,
}
impl OfficialPreset {
    pub const ALL: [Self; 5] = [
        Self::Neofetch,
        Self::Screenfetch,
        Self::Paleofetch,
        Self::Icons,
        Self::Bars,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Neofetch => "Neofetch",
            Self::Screenfetch => "Screenfetch",
            Self::Paleofetch => "Paleofetch",
            Self::Icons => "Icons · Example 8",
            Self::Bars => "Bars · Example 9",
        }
    }
    pub fn source(self) -> &'static str {
        match self {
            Self::Neofetch => include_str!("../resources/fastfetch/presets/neofetch.jsonc"),
            Self::Screenfetch => include_str!("../resources/fastfetch/presets/screenfetch.jsonc"),
            Self::Paleofetch => include_str!("../resources/fastfetch/presets/paleofetch.jsonc"),
            Self::Icons => include_str!("../resources/fastfetch/presets/example-8.jsonc"),
            Self::Bars => include_str!("../resources/fastfetch/presets/example-9.jsonc"),
        }
    }
    pub fn index(self) -> u32 {
        Self::ALL.iter().position(|p| *p == self).unwrap() as u32 + 1
    }
    pub fn config(self) -> Value {
        parse_jsonc(self.source()).expect("reviewed bundled preset is valid JSONC")
    }
    pub fn items(self) -> Vec<OfficialItem> {
        (0..self.config()["modules"].as_array().unwrap().len())
            .map(|id| OfficialItem {
                id: id as u16,
                enabled: true,
            })
            .collect()
    }
    pub fn module_label(self, id: u16) -> String {
        let config = self.config();
        let module = &config["modules"][usize::from(id)];
        let kind = module
            .as_str()
            .or_else(|| module["type"].as_str())
            .unwrap_or("unknown");
        let label = match kind {
            "os" => "OS",
            "cpu" => "CPU",
            "gpu" => "GPU",
            "de" => "Desktop Environment",
            "wm" => "Window Manager",
            "wmtheme" => "Window Manager Theme",
            "terminalfont" => "Terminal Font",
            "display" => "Resolution",
            "colors" => "Color Swatches",
            "break" => "Blank Line",
            other => other,
        };
        let label = module["key"].as_str().unwrap_or(label);
        let mut chars = label.chars();
        chars
            .next()
            .map(|first| first.to_uppercase().collect::<String>() + chars.as_str())
            .unwrap_or_default()
    }
    pub fn validate_items(self, items: &[OfficialItem]) -> Result<(), String> {
        let count = self.items().len();
        if items.len() != count
            || (0..count).any(|id| items.iter().filter(|i| usize::from(i.id) == id).count() != 1)
        {
            return Err("Official preset fields must occur exactly once; unknown or missing IDs are not accepted.".into());
        }
        Ok(())
    }
    pub fn selected_config(self, items: &[OfficialItem]) -> Result<Value, String> {
        self.validate_items(items)?;
        let mut config = self.config();
        let source = config["modules"].as_array().unwrap();
        let modules: Vec<_> = items
            .iter()
            .filter(|i| i.enabled)
            .map(|i| source[usize::from(i.id)].clone())
            .collect();
        config["modules"] = if modules.is_empty() {
            json!([{"type":"custom", "format":""}])
        } else {
            json!(modules)
        };
        Ok(config)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct OfficialItem {
    pub id: u16,
    pub enabled: bool,
}

// A string-aware comment pass, used ONLY on compile-time upstream assets.
fn parse_jsonc(source: &str) -> Result<Value, String> {
    let mut out = String::new();
    let mut chars = source.chars().peekable();
    let mut string = false;
    let mut escape = false;
    while let Some(ch) = chars.next() {
        if string {
            out.push(ch);
            if escape {
                escape = false;
            } else if ch == '\\' {
                escape = true;
            } else if ch == '"' {
                string = false;
            }
        } else if ch == '"' {
            string = true;
            out.push(ch);
        } else if ch == '/' && chars.peek() == Some(&'/') {
            chars.next();
            for ch in chars.by_ref() {
                if ch == '\n' {
                    out.push('\n');
                    break;
                }
            }
        } else {
            out.push(ch);
        }
    }
    serde_json::from_str(&out).map_err(|e| e.to_string())
}

/// Render only pinned module objects. User text/art never enters this process.
/// The layout is composed locally around these ANSI lines, using the same logo
/// data and exported module configuration. No project directory is mounted RW.
pub(crate) fn render(
    preset: OfficialPreset,
    items: &[OfficialItem],
    accent: u8,
) -> Result<String, String> {
    let mut config = preset.selected_config(items)?;
    config["logo"] = json!({"type":"none"});
    config["display"]["pipe"] = json!(false);
    config["display"]["hideCursor"] = json!(false);
    config["display"]["disableLinewrap"] = json!(false);
    config["display"]["brightColor"] = json!(false);
    config["display"]["showErrors"] = json!(true);
    config["display"]["color"] = json!({"keys":accent.to_string()});
    config["general"] = json!({"processingTimeout":200});
    let scratch = tempfile::Builder::new()
        .prefix("termimochi-fastfetch-")
        .tempdir()
        .map_err(|e| e.to_string())?;
    let path = scratch.path().join("config.jsonc");
    std::fs::write(
        &path,
        serde_json::to_vec(&config).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    if !std::path::Path::new("/usr/bin/fastfetch").is_file()
        || !std::path::Path::new("/usr/bin/bwrap").is_file()
    {
        return Err(
            "Install Fastfetch and Bubblewrap to preview official presets. Export still works."
                .into(),
        );
    }
    let mut command = Command::new("/usr/bin/bwrap");
    command
        .env_clear()
        .args([
            "--die-with-parent",
            "--new-session",
            "--unshare-net",
            "--unshare-pid",
            "--unshare-ipc",
            "--cap-drop",
            "ALL",
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
            "--ro-bind",
        ])
        .arg(&path)
        .args([
            "/tmp/config.jsonc",
            "--chdir",
            "/",
            "--",
            "/usr/bin/fastfetch",
            "--config",
            "/tmp/config.jsonc",
        ])
        .env("HOME", gtk::glib::home_dir())
        .env("USER", gtk::glib::user_name())
        .env("LOGNAME", gtk::glib::user_name())
        .env("PATH", "/usr/bin:/bin")
        .env("LANG", "C.UTF-8")
        .env("LC_ALL", "C.UTF-8")
        .env("TERM", "xterm-256color");
    for key in [
        "SHELL",
        "TERM_PROGRAM",
        "TERM_PROGRAM_VERSION",
        "XDG_CURRENT_DESKTOP",
        "DESKTOP_SESSION",
    ] {
        if let Some(value) = std::env::var_os(key) {
            command.env(key, value);
        }
    }
    let output = crate::preview_context::run_bounded(
        &mut command,
        Instant::now() + Duration::from_millis(2200),
    )
    .map_err(|e| {
        format!("Official preview unavailable ({e:?}). No unsandboxed fallback was run.")
    })?;
    Ok(crate::starship_import::terminal_safe_ansi(&output))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bundled_presets_have_only_reviewed_local_modules_and_preserve_every_object() {
        let allowed = [
            "title",
            "separator",
            "os",
            "host",
            "kernel",
            "uptime",
            "packages",
            "shell",
            "display",
            "de",
            "wm",
            "wmtheme",
            "theme",
            "icons",
            "terminal",
            "terminalfont",
            "cpu",
            "gpu",
            "memory",
            "break",
            "colors",
            "battery",
            "disk",
            "swap",
        ];
        for preset in OfficialPreset::ALL {
            let original = preset.config();
            let mut items = preset.items();
            for module in original["modules"].as_array().unwrap() {
                let kind = module.as_str().or_else(|| module["type"].as_str()).unwrap();
                assert!(
                    allowed.contains(&kind),
                    "Review new module {kind} before exposing it"
                );
            }
            assert!(original.get("general").is_none());
            assert_eq!(preset.selected_config(&items).unwrap(), original);
            items.reverse();
            items[1].enabled = false;
            let edited = preset.selected_config(&items).unwrap();
            assert_eq!(
                edited["modules"],
                json!(
                    items
                        .iter()
                        .filter(|i| i.enabled)
                        .map(|i| &original["modules"][usize::from(i.id)])
                        .collect::<Vec<_>>()
                )
            );
            items[0].id = u16::MAX;
            assert!(preset.selected_config(&items).is_err());
            let mut items = preset.items();
            items[1].id = items[0].id;
            assert!(preset.validate_items(&items).is_err());
            for item in &mut items {
                item.enabled = false;
            }
            assert!(preset.selected_config(&items).is_err());
            let mut items = preset.items();
            for item in &mut items {
                item.enabled = false;
            }
            assert_eq!(
                preset.selected_config(&items).unwrap()["modules"],
                json!([{"type":"custom", "format":""}])
            );
        }
    }
    #[test]
    fn comments_do_not_corrupt_urls_or_escaped_strings() {
        assert_eq!(
            parse_jsonc(
                "{\"url\":\"https://example.test/a//b\", // comment\n\"text\":\"a\\\"//b\"}"
            )
            .unwrap()["url"],
            "https://example.test/a//b"
        );
    }
    #[test]
    #[ignore = "requires system Fastfetch and Bubblewrap; never runs an unsandboxed fallback"]
    fn real_official_presets_render_offline_and_remain_terminal_safe() {
        for preset in OfficialPreset::ALL {
            let output = render(preset, &preset.items(), 31).unwrap();
            assert!(!output.trim().is_empty(), "{preset:?}");
            assert!(!output.contains("\x1b]"));
            assert!(!output.contains("\x1b[?"));
            assert!(output.len() < 32768);
            assert!(
                output.contains("Memory")
                    || output.contains("RAM")
                    || preset == OfficialPreset::Icons,
                "{preset:?}: {output}"
            );
            if std::env::var_os("TERMIMOCHI_OFFICIAL_OUTPUT").is_some() {
                println!("{preset:?}: {output}");
            }
            let mut settings = crate::greeting::GreetingSettings::default();
            settings.use_official(preset);
            settings.message = "Literal {$TERMIMOCHI_SECRET} $(must-not-run)".into();
            let root = tempfile::tempdir().unwrap();
            let path = root.path().join("config.jsonc");
            std::fs::write(&path, settings.fastfetch_config().unwrap()).unwrap();
            let mut command = Command::new("/usr/bin/fastfetch");
            command
                .args(["--config"])
                .arg(path)
                .args(["--pipe", "false"])
                .current_dir(root.path())
                .env("TERMIMOCHI_SECRET", "SHOULD_NOT_EXPAND");
            let exported = crate::preview_context::run_bounded(
                &mut command,
                Instant::now() + Duration::from_secs(3),
            )
            .unwrap();
            assert!(exported.contains(&settings.message));
            assert!(!exported.contains("SHOULD_NOT_EXPAND"));
            assert!(!root.path().join("must-not-run").exists());
        }
    }
}
