//! Declarative greetings. No shell, Fastfetch config, network or user command
//! is executed. Machine facts are bounded, read-only Linux snapshots.
use crate::document_store::Document;
use crate::greeting_official::{OfficialItem, OfficialPreset};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fs::File, io::Read};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

pub(crate) const PRESET_NAME: &str = "greeting.termimochi-greeting.json";
const KEY_WIDTH: usize = 10;
const MOCHI: &str = include_str!("../resources/termimochi-ascii.txt");
const TERMINAL: &str = " .------------.\n |  >_        |\n |            |\n '------------'";
pub(crate) const ART_MAX_ROWS: usize = 64;
pub(crate) const ART_MAX_COLUMNS: usize = 120;
pub(crate) const ART_MAX_BYTES: usize = 16 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Logo {
    Mochi,
    Terminal,
    Ubuntu,
    Arch,
    Debian,
    Fedora,
    LinuxMint,
    Custom,
    None,
}
impl Logo {
    pub const ALL: [Self; 9] = [
        Self::Mochi,
        Self::Ubuntu,
        Self::Arch,
        Self::Debian,
        Self::Fedora,
        Self::LinuxMint,
        Self::Terminal,
        Self::Custom,
        Self::None,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Mochi => "TermiMochi",
            Self::Ubuntu => "Ubuntu",
            Self::Arch => "Arch Linux",
            Self::Debian => "Debian",
            Self::Fedora => "Fedora",
            Self::LinuxMint => "Linux Mint",
            Self::Terminal => "Terminal",
            Self::Custom => "Custom text",
            Self::None => "None",
        }
    }
    pub fn index(self) -> u32 {
        Self::ALL.iter().position(|value| *value == self).unwrap() as u32
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Position {
    Left,
    Top,
    Right,
    Card,
}
impl Position {
    pub const ALL: [Self; 4] = [Self::Left, Self::Top, Self::Right, Self::Card];
    pub fn label(self) -> &'static str {
        match self {
            Self::Left => "Horizontal",
            Self::Top => "Vertical",
            Self::Right => "Horizontal · right",
            Self::Card => "Minimal card",
        }
    }
    pub fn index(self) -> u32 {
        Self::ALL.iter().position(|value| *value == self).unwrap() as u32
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Opening {
    #[default]
    None,
    Fade,
    Lines,
    Shimmer,
}
impl Opening {
    pub const ALL: [Self; 4] = [Self::None, Self::Fade, Self::Lines, Self::Shimmer];
    pub fn label(self) -> &'static str {
        match self {
            Self::None => "None",
            Self::Fade => "Fade in",
            Self::Lines => "Line by line",
            Self::Shimmer => "Shimmer",
        }
    }
    pub fn index(self) -> u32 {
        Self::ALL.iter().position(|value| *value == self).unwrap() as u32
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Info {
    Os,
    Kernel,
    Shell,
    Terminal,
    Cpu,
    Memory,
    Uptime,
    Date,
    Gpu,
    Disk,
}
impl Info {
    pub const ALL: [Self; 10] = [
        Self::Os,
        Self::Kernel,
        Self::Shell,
        Self::Terminal,
        Self::Cpu,
        Self::Memory,
        Self::Uptime,
        Self::Date,
        Self::Gpu,
        Self::Disk,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Os => "OS",
            Self::Kernel => "Kernel",
            Self::Shell => "Shell",
            Self::Terminal => "Terminal",
            Self::Cpu => "CPU",
            Self::Memory => "Memory",
            Self::Uptime => "Uptime",
            Self::Date => "Date",
            Self::Gpu => "GPU",
            Self::Disk => "Disk (/)",
        }
    }
    pub fn module(self) -> &'static str {
        match self {
            Self::Os => "os",
            Self::Kernel => "kernel",
            Self::Shell => "shell",
            Self::Terminal => "terminal",
            Self::Cpu => "cpu",
            Self::Memory => "memory",
            Self::Uptime => "uptime",
            Self::Date => "datetime",
            Self::Gpu => "gpu",
            Self::Disk => "disk",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Item {
    pub kind: Info,
    pub enabled: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct GreetingSettings {
    pub enabled: bool,
    pub logo: Logo,
    pub custom_logo: String,
    pub position: Position,
    /// 0..15 follow ANSI palette slots; 16 follows terminal foreground.
    pub accent: u8,
    pub gap: u8,
    pub message: String,
    #[serde(deserialize_with = "deserialize_items")]
    pub items: Vec<Item>,
    /// 0 follows Layout. Fixed widths affect only this preview, never Layout.
    #[serde(default)]
    pub preview_columns: u16,
    /// Opening motion is a workbench effect, not a Fastfetch feature.
    #[serde(default)]
    pub opening: Opening,
    #[serde(default)]
    pub official_preset: Option<OfficialPreset>,
    #[serde(default)]
    pub official_items: Vec<OfficialItem>,
}

fn deserialize_items<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Vec<Item>, D::Error> {
    let mut items = Vec::<Item>::deserialize(deserializer)?;
    // Only migrate the exact legacy eight-item set. Broken/partial lists still
    // fail validation, and old workspaces do not unexpectedly show new fields.
    if items.len() == 8
        && Info::ALL[..8]
            .iter()
            .all(|kind| items.iter().filter(|i| i.kind == *kind).count() == 1)
    {
        items.extend([Info::Gpu, Info::Disk].map(|kind| Item {
            kind,
            enabled: false,
        }));
    }
    Ok(items)
}
impl Default for GreetingSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            logo: Logo::Mochi,
            custom_logo: "(づ｡◕‿‿◕｡)づ".into(),
            position: Position::Left,
            accent: 6,
            gap: 3,
            message: "Welcome back (•‿•)".into(),
            preview_columns: 0,
            opening: Opening::None,
            official_preset: None,
            official_items: Vec::new(),
            items: Info::ALL
                .into_iter()
                .map(|kind| Item {
                    kind,
                    enabled: !matches!(kind, Info::Kernel | Info::Terminal | Info::Uptime),
                })
                .collect(),
        }
    }
}
impl GreetingSettings {
    pub fn validate(&self) -> Result<(), String> {
        if self.accent > 16 || self.gap > 8 || ![0, 80, 100, 120].contains(&self.preview_columns) {
            return Err("Greeting color or spacing is outside the supported range.".into());
        }
        if self.message.chars().count() > 160 || self.message.chars().any(unsafe_char) {
            return Err("Use up to 160 characters for the welcome text, without line breaks or terminal controls.".into());
        }
        if self.custom_logo.len() > ART_MAX_BYTES
            || self.custom_logo.lines().count() > ART_MAX_ROWS
            || self
                .custom_logo
                .lines()
                .any(|line| line.width() > ART_MAX_COLUMNS)
            || self
                .custom_logo
                .chars()
                .any(|ch| ch != '\n' && unsafe_char(ch))
        {
            return Err("Custom artwork supports 64 lines, 120 cells per line and 16 KiB. Terminal controls are not allowed.".into());
        }
        if self.items.len() != Info::ALL.len()
            || Info::ALL
                .iter()
                .any(|kind| self.items.iter().filter(|item| item.kind == *kind).count() != 1)
        {
            return Err("Greeting information items must occur exactly once.".into());
        }
        if let Some(preset) = self.official_preset {
            preset.validate_items(&self.official_items)?;
        } else if !self.official_items.is_empty() {
            return Err("Official fields require a named upstream preset.".into());
        }
        Ok(())
    }
    pub fn artwork(&self) -> String {
        if self.position == Position::Card {
            return String::new();
        }
        let source = match self.logo {
            Logo::Mochi => MOCHI,
            Logo::Ubuntu => include_str!("../resources/fastfetch/logos/ubuntu.txt"),
            Logo::Arch => include_str!("../resources/fastfetch/logos/arch.txt"),
            Logo::Debian => include_str!("../resources/fastfetch/logos/debian.txt"),
            Logo::Fedora => include_str!("../resources/fastfetch/logos/fedora.txt"),
            Logo::LinuxMint => include_str!("../resources/fastfetch/logos/linuxmint.txt"),
            Logo::Terminal => TERMINAL,
            Logo::Custom => return self.custom_logo.clone(),
            Logo::None => "",
        };
        // Upstream $1…$9 are palette markers, not artwork cells. Preserve the
        // exact shape while the workbench's selected ANSI accent supplies color.
        let mut chars = source.chars().peekable();
        let mut plain = String::new();
        while let Some(ch) = chars.next() {
            if ch == '$' && chars.peek() == Some(&'$') {
                chars.next();
                plain.push('$');
            } else if ch == '$' && chars.peek().is_some_and(|c| ('1'..='9').contains(c)) {
                chars.next();
            } else {
                plain.push(ch);
            }
        }
        plain
    }
    pub fn use_official(&mut self, preset: OfficialPreset) {
        self.enabled = true;
        self.official_preset = Some(preset);
        self.official_items = preset.items();
        self.logo = Logo::Ubuntu;
        self.position = Position::Left;
        self.message.clear();
        self.gap = 3;
        self.accent = 1;
    }
    pub fn position_at_width(&self, columns: usize) -> Position {
        let width = columns.clamp(12, 240).saturating_sub(1);
        let logo_width = self.artwork().lines().map(str::width).max().unwrap_or(0);
        let minimum_info = if self.official_preset.is_some() {
            36
        } else {
            20
        };
        if matches!(self.position, Position::Left | Position::Right)
            && logo_width > 0
            && width < logo_width + usize::from(self.gap) + minimum_info
        {
            Position::Top
        } else {
            self.position
        }
    }
    pub fn color_code(&self) -> u8 {
        match self.accent {
            0..=7 => 30 + self.accent,
            8..=15 => 90 + self.accent - 8,
            _ => 39,
        }
    }
    fn colored(&self, text: &str) -> String {
        format!("\x1b[{}m{text}\x1b[0m", self.color_code())
    }

    /// Insert at an edge rather than swapping: all intervening fields retain
    /// their relative order. Used by pointer dragging and keyboard controls.
    pub fn move_item(&mut self, source: Info, target: Info, after: bool) -> bool {
        if source == target {
            return false;
        }
        let Some(from) = self.items.iter().position(|i| i.kind == source) else {
            return false;
        };
        let Some(to) = self.items.iter().position(|i| i.kind == target) else {
            return false;
        };
        let insertion = to + usize::from(after);
        let destination = insertion - usize::from(from < insertion);
        if destination == from {
            return false;
        }
        let item = self.items.remove(from);
        self.items.insert(destination, item);
        true
    }

    /// Bounded output with grapheme-aware cell sizing. Narrow terminals stack
    /// the artwork above the information; this does not mutate the saved layout.
    pub fn render(&self, context: &GreetingContext, columns: usize) -> String {
        self.render_with_official(context, columns, None)
    }
    pub fn render_with_official(
        &self,
        context: &GreetingContext,
        columns: usize,
        official: Option<&str>,
    ) -> String {
        if !self.enabled || self.validate().is_err() {
            return String::new();
        }
        let width = columns.clamp(12, 240).saturating_sub(1);
        let artwork = self.artwork();
        let logo: Vec<_> = artwork.lines().collect();
        let logo_width = logo.iter().map(|line| line.width()).max().unwrap_or(0);
        let gap = usize::from(self.gap);
        let position = self.position_at_width(columns);
        let side = matches!(position, Position::Left | Position::Right) && logo_width > 0;
        let info_width = if side {
            width - logo_width - gap
        } else {
            width
        };
        let mut info = Vec::<(String, usize)>::new();
        if !self.message.is_empty() {
            let text = clip(&self.message, info_width);
            let cells = text.width();
            info.push((self.colored(&text), cells));
            if self.position == Position::Card {
                info.push((self.colored(&"─".repeat(cells)), cells));
            }
        }
        if self.official_preset.is_some() {
            for line in official.unwrap_or("Reading official preset…").split("\r\n") {
                let (text, cells) = clip_ansi(line, info_width);
                info.push((text, cells));
            }
            // Fastfetch already ends its modules with LF; do not add that as
            // an extra empty data row alongside the logo.
            if official.is_some_and(|text| text.ends_with("\r\n")) {
                info.pop();
            }
        } else {
            for item in self.items.iter().filter(|item| item.enabled) {
                let label = format!("{:<KEY_WIDTH$}", format!("{}:", item.kind.label()));
                for value in context.lines(item.kind) {
                    let value = clip(value, info_width.saturating_sub(KEY_WIDTH));
                    info.push((
                        format!("{}{value}", self.colored(&label)),
                        KEY_WIDTH + value.width(),
                    ));
                }
            }
        }
        let mut lines = Vec::new();
        if side {
            let info_cells = info.iter().map(|(_, cells)| *cells).max().unwrap_or(0);
            for index in 0..logo.len().max(info.len()) {
                let art = logo.get(index).copied().unwrap_or("");
                let (text, cells) = info.get(index).cloned().unwrap_or_default();
                lines.push(if self.position == Position::Left {
                    format!(
                        "{}{}{text}",
                        self.colored(art),
                        " ".repeat(logo_width - art.width() + gap)
                    )
                } else {
                    format!(
                        "{text}{}{}",
                        " ".repeat(info_cells - cells + gap),
                        self.colored(art)
                    )
                });
            }
        } else {
            for line in logo {
                lines.push(self.colored(&clip(line, width)));
            }
            if !lines.is_empty() && !info.is_empty() {
                lines.push(String::new());
            }
            lines.extend(info.into_iter().map(|(text, _)| text));
        }
        if lines.is_empty() {
            return String::new();
        }
        format!("{}\x1b[0m\r\n\r\n", lines.join("\r\n"))
    }

    /// Only reviewed built-ins and literal Custom text are emitted. No Command
    /// module, logo command/path, dynamic format or shell startup hook is used.
    pub fn fastfetch_config(&self) -> Result<String, String> {
        use serde_json::json;
        self.validate()?;
        let mut config = if let Some(preset) = self.official_preset {
            preset.selected_config(&self.official_items)?
        } else {
            json!({})
        };
        let mut modules = Vec::new();
        if self.enabled {
            if !self.message.is_empty() {
                modules.push(json!({"type":"custom", "key":" ", "format": self.colored(&self.message.replace('{', "{{"))}));
                if self.position == Position::Card {
                    modules.push(json!({"type":"custom", "format": self.colored(&"─".repeat(self.message.width().min(78)))}));
                }
            }
            if self.official_preset.is_some() {
                modules.extend(config["modules"].as_array().unwrap().iter().cloned());
            } else {
                for item in self.items.iter().filter(|item| item.enabled) {
                    let mut module = json!({"type":item.kind.module(), "key":item.kind.label()});
                    if item.kind == Info::Date {
                        module["format"] = json!("{year}-{month-pretty}-{day-pretty}");
                    }
                    if item.kind == Info::Cpu {
                        module["format"] = json!("{name}");
                    }
                    if item.kind == Info::Gpu {
                        module["format"] = json!("{name}");
                    }
                    if item.kind == Info::Disk {
                        module["folders"] = json!("/");
                        module["format"] = json!("{size-used} / {size-total}");
                    }
                    modules.push(module);
                }
            }
        }
        // An empty modules array selects Fastfetch defaults; a literal empty
        // Custom module is intentional when the greeting is switched off.
        if modules.is_empty() {
            modules.push(json!({"type":"custom", "format":""}));
        }
        let art = self
            .artwork()
            .lines()
            .map(|line| self.colored(line))
            .collect::<Vec<_>>()
            .join("\n");
        let logo = if !self.enabled || art.is_empty() {
            json!({"type":"none"})
        } else {
            json!({"type":"data-raw", "source":art,
                "position": match self.position { Position::Left => "left", Position::Right => "right", _ => "top" },
                "padding":{"left":0,"right":self.gap,"top":0}, "printRemaining":true})
        };
        config["$schema"] =
            json!("https://github.com/fastfetch-cli/fastfetch/raw/dev/doc/json_schema.json");
        config["logo"] = logo;
        if self.official_preset.is_none() {
            config["display"] = json!({"separator":": ","key":{"width":KEY_WIDTH}});
        }
        config["display"]["brightColor"] = json!(false);
        config["display"]["color"] = json!({"keys":self.color_code().to_string()});
        config["modules"] = json!(modules);
        serde_json::to_string_pretty(&config).map_err(|error| error.to_string())
    }
}

/// Clip text without counting trusted SGR sequences as printable cells. State
/// is reset per row so an official module's style cannot leak into the prompt.
pub(crate) fn clip_ansi(text: &str, width: usize) -> (String, usize) {
    let safe = crate::starship_import::terminal_safe_ansi(text);
    let mut out = String::new();
    let mut rest = safe.as_str();
    let mut used = 0;
    while !rest.is_empty() {
        if let Some(sequence) = rest.strip_prefix("\x1b[") {
            let end = sequence.find('m').expect("sanitized SGR");
            out.push_str(&rest[..end + 3]);
            rest = &rest[end + 3..];
        } else {
            let end = rest.find('\x1b').unwrap_or(rest.len());
            for grapheme in rest[..end].graphemes(true) {
                let cells = grapheme.width();
                if used + cells > width {
                    out.push_str("\x1b[0m");
                    return (out, used);
                }
                out.push_str(grapheme);
                used += cells;
            }
            rest = &rest[end..];
        }
    }
    out.push_str("\x1b[0m");
    (out, used)
}

fn unsafe_char(ch: char) -> bool {
    ch.is_control()
        || matches!(ch, '\u{2028}' | '\u{2029}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
}
pub(crate) fn clip(text: &str, width: usize) -> String {
    let clean: String = text.chars().filter(|ch| !unsafe_char(*ch)).collect();
    let mut result = String::new();
    let mut used = 0;
    for grapheme in clean.graphemes(true) {
        let cells = grapheme.width();
        if used + cells > width {
            break;
        }
        result.push_str(grapheme);
        used += cells;
    }
    result
}

#[derive(Clone, Debug, Default)]
pub(crate) struct GreetingContext {
    values: BTreeMap<Info, String>,
    gpus: Vec<String>,
}
impl GreetingContext {
    pub fn value(&self, kind: Info) -> &str {
        self.values
            .get(&kind)
            .map(String::as_str)
            .unwrap_or("Unavailable")
    }
    fn lines(&self, kind: Info) -> Vec<&str> {
        if kind == Info::Gpu && !self.gpus.is_empty() {
            self.gpus.iter().map(String::as_str).collect()
        } else {
            vec![self.value(kind)]
        }
    }
    /// Called on the existing folder worker, not on the GTK main thread.
    pub fn load() -> Self {
        let os = read("/etc/os-release", 8192);
        let cpu = read("/proc/cpuinfo", 65536);
        let memory = read("/proc/meminfo", 8192);
        let uptime = read("/proc/uptime", 128)
            .split_whitespace()
            .next()
            .and_then(|v| v.parse::<f64>().ok());
        let mut values = BTreeMap::new();
        if let Some(value) = os
            .lines()
            .find_map(|line| line.strip_prefix("PRETTY_NAME="))
        {
            values.insert(Info::Os, clip(value.trim_matches('"'), 100));
        }
        let kernel = read("/proc/sys/kernel/osrelease", 256);
        if !kernel.trim().is_empty() {
            values.insert(Info::Kernel, clip(kernel.trim(), 100));
        }
        for line in cpu.lines() {
            if let Some((key, value)) = line.split_once(':')
                && ["model name", "Hardware"].contains(&key.trim())
            {
                values.insert(Info::Cpu, clip(value.trim(), 120));
                break;
            }
        }
        if let Some(value) = memory_value(&memory) {
            values.insert(Info::Memory, value);
        }
        let gpus = gpu_snapshot();
        if !gpus.is_empty() {
            values.insert(Info::Gpu, clip(&gpus.join(" · "), 240));
        }
        // The root filesystem matches the explicit Fastfetch disk.folders
        // selection. Never enumerate removable/network mounts or project paths.
        use gtk::gio::prelude::*;
        if let Ok(info) = gtk::gio::File::for_path("/").query_filesystem_info(
            "filesystem::size,filesystem::free",
            gtk::gio::Cancellable::NONE,
        ) {
            let total = info.attribute_uint64("filesystem::size");
            let free = info.attribute_uint64("filesystem::free");
            if total > 0 {
                values.insert(
                    Info::Disk,
                    format!(
                        "{:.1} / {:.1} GiB",
                        total.saturating_sub(free) as f64 / 1073741824.0,
                        total as f64 / 1073741824.0
                    ),
                );
            }
        }
        if let Some(seconds) = uptime.filter(|v| v.is_finite() && *v >= 0.0) {
            let minutes = seconds as u64 / 60;
            values.insert(Info::Uptime, format!("{}h {}m", minutes / 60, minutes % 60));
        }
        if let Some(shell) = std::env::var_os("SHELL").and_then(|value| {
            std::path::Path::new(&value)
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
        }) {
            values.insert(Info::Shell, clip(&shell, 40));
        }
        if let Ok(terminal) = std::env::var("TERM_PROGRAM") {
            values.insert(Info::Terminal, clip(&terminal, 40));
        }
        if let Ok(now) = gtk::glib::DateTime::now_local()
            && let Ok(date) = now.format("%Y-%m-%d")
        {
            values.insert(Info::Date, date.to_string());
        }
        Self { values, gpus }
    }
}

fn gpu_snapshot() -> Vec<String> {
    let Ok(entries) = std::fs::read_dir("/sys/class/drm") else {
        return Vec::new();
    };
    let mut devices: Vec<_> = entries
        .take(128)
        .filter_map(Result::ok)
        .filter(|entry| {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            name.strip_prefix("card").is_some_and(|suffix| {
                !suffix.is_empty() && suffix.bytes().all(|b| b.is_ascii_digit())
            })
        })
        .collect();
    devices.sort_by_key(|entry| entry.file_name());
    let mut ids = read("/usr/share/hwdata/pci.ids", 4 * 1024 * 1024);
    if ids.is_empty() {
        ids = read("/usr/share/misc/pci.ids", 4 * 1024 * 1024);
    }
    let names: Vec<_> = devices
        .iter()
        .take(8)
        .filter_map(|entry| {
            let device = entry.path().join("device");
            let vendor = read(device.join("vendor").to_str()?, 32);
            let product = read(device.join("device").to_str()?, 32);
            let vendor = vendor.trim().strip_prefix("0x")?;
            let product = product.trim().strip_prefix("0x")?;
            if vendor.len() != 4
                || product.len() != 4
                || !vendor
                    .bytes()
                    .chain(product.bytes())
                    .all(|b| b.is_ascii_hexdigit())
            {
                return None;
            }
            Some(
                pci_name(&ids, vendor, product)
                    .unwrap_or_else(|| format!("PCI {vendor}:{product}")),
            )
        })
        .collect();
    names
}

fn pci_name(ids: &str, vendor: &str, device: &str) -> Option<String> {
    let mut in_vendor = false;
    for line in ids.lines() {
        if line.starts_with('#') || line.is_empty() {
            continue;
        }
        if !line.starts_with('\t') {
            if in_vendor {
                break;
            }
            in_vendor = line
                .get(..4)
                .is_some_and(|id| id.eq_ignore_ascii_case(vendor))
                && line.as_bytes().get(4) == Some(&b' ');
        } else if in_vendor
            && line
                .get(1..5)
                .is_some_and(|id| id.eq_ignore_ascii_case(device))
            && line.as_bytes().get(5) == Some(&b' ')
        {
            return Some(clip(line[5..].trim(), 120));
        }
    }
    None
}
fn read(path: &str, limit: u64) -> String {
    let mut text = String::new();
    if let Ok(file) = File::open(path) {
        let _ = file.take(limit).read_to_string(&mut text);
    }
    text
}
fn memory_value(text: &str) -> Option<String> {
    let find = |key: &str| {
        text.lines()
            .find_map(|line| line.strip_prefix(key))?
            .split_whitespace()
            .next()?
            .parse::<u64>()
            .ok()
    };
    let total = find("MemTotal:")?;
    let available = find("MemAvailable:")?;
    Some(format!(
        "{:.1} / {:.1} GiB",
        total.saturating_sub(available) as f64 / 1048576.0,
        total as f64 / 1048576.0
    ))
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct GreetingPreset {
    kind: String,
    version: u8,
    pub greeting: GreetingSettings,
}
impl GreetingPreset {
    pub fn new(greeting: GreetingSettings) -> Self {
        Self {
            kind: "termimochi-greeting".into(),
            version: 1,
            greeting,
        }
    }
}
impl Document for GreetingPreset {
    const SUFFIX: &'static str = ".termimochi-greeting.json";
    fn validate(&self) -> Result<(), String> {
        if self.kind != "termimochi-greeting" || self.version != 1 {
            return Err("Unsupported greeting preset.".into());
        }
        self.greeting.validate()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn full_size_artwork_and_larger_custom_documents_preserve_geometry() {
        let mut settings = GreetingSettings {
            enabled: true,
            logo: Logo::Ubuntu,
            ..Default::default()
        };
        let art = settings.artwork();
        assert_eq!(art.lines().count(), 20);
        assert!(art.lines().map(str::width).max().unwrap() >= 40);
        assert!(!art.contains("$1"));
        settings.use_official(OfficialPreset::Neofetch);
        assert_eq!(settings.position_at_width(80), Position::Top);
        assert_eq!(settings.position_at_width(100), Position::Left);
        assert_eq!(settings.position_at_width(120), Position::Left);
        settings.logo = Logo::Debian;
        assert!(settings.artwork().starts_with("        _,met$$$$$gg."));
        settings.logo = Logo::Custom;
        settings.custom_logo = "$1 literal $$".into();
        assert_eq!(settings.artwork(), "$1 literal $$");
        settings.custom_logo = vec!["x".repeat(120); 64].join("\n");
        settings.validate().unwrap();
        assert_eq!(settings.artwork().lines().count(), 64);
        for invalid in [
            "x".repeat(121),
            "x\n".repeat(65),
            "\x1b]52;c;bad\x07".into(),
            "x".repeat(ART_MAX_BYTES + 1),
        ] {
            settings.custom_logo = invalid;
            assert!(settings.validate().is_err());
        }
    }
    #[test]
    fn official_settings_roundtrip_and_export_keep_uneditable_formats_and_duplicates() {
        for preset in OfficialPreset::ALL {
            let mut settings = GreetingSettings::default();
            settings.use_official(preset);
            settings.official_items.reverse();
            settings.official_items[1].enabled = false;
            let encoded =
                crate::document_store::encode(&GreetingPreset::new(settings.clone())).unwrap();
            assert_eq!(
                crate::document_store::decode::<GreetingPreset>(&encoded)
                    .unwrap()
                    .greeting,
                settings
            );
            let output: serde_json::Value =
                serde_json::from_str(&settings.fastfetch_config().unwrap()).unwrap();
            assert_eq!(
                output["modules"],
                preset.selected_config(&settings.official_items).unwrap()["modules"]
            );
            assert_eq!(output["logo"]["type"], "data-raw");
            settings.official_items.pop();
            assert!(settings.validate().is_err());
        }
    }
    #[test]
    fn ansi_rows_are_clipped_by_graphemes_without_cursor_or_osc_controls() {
        let (line, width) = clip_ansi("\x1b[31mA中文🦀\x1b[0m\x1b]52;c;secret\x07\x1b[2J", 5);
        assert_eq!(width, 5);
        assert_eq!(line, "\x1b[31mA中文\x1b[0m");
        assert_eq!(clip_ansi("\x1b[1mhello", 0).1, 0);
        for columns in [80, 100, 120] {
            let mut settings = GreetingSettings::default();
            settings.use_official(OfficialPreset::Neofetch);
            let text = settings.render_with_official(
                &GreetingContext::default(),
                columns,
                Some("\x1b[31mOS: Test\r\nGPU: Test\r\n"),
            );
            assert!(text.contains("OS: Test"));
            assert!(text.contains("GPU: Test"));
            assert!(!text.contains("Reading official"));
        }
    }
    #[test]
    #[ignore = "requires system Fastfetch; compares bundled artwork against real built-in logos"]
    fn upstream_logo_geometry_matches_real_fastfetch() {
        for (logo, name) in [
            (Logo::Ubuntu, "ubuntu"),
            (Logo::Arch, "arch"),
            (Logo::Debian, "debian"),
            (Logo::Fedora, "fedora"),
            (Logo::LinuxMint, "linuxmint"),
        ] {
            let settings = GreetingSettings {
                logo,
                ..Default::default()
            };
            let mut command = std::process::Command::new("/usr/bin/fastfetch");
            command
                .args([
                    "--config",
                    "none",
                    "--logo",
                    name,
                    "--logo-type",
                    "builtin",
                    "--structure",
                    "break",
                    "--logo-print-remaining",
                    "true",
                    "--pipe",
                    "false",
                ])
                .current_dir("/");
            let output = crate::preview_context::run_bounded(
                &mut command,
                std::time::Instant::now() + std::time::Duration::from_secs(3),
            )
            .unwrap();
            let safe = crate::starship_import::terminal_safe_ansi(&output);
            let mut sgr = false;
            let plain: String = safe
                .chars()
                .filter(|ch| {
                    if *ch == '\x1b' {
                        sgr = true;
                        return false;
                    }
                    if sgr {
                        if *ch == 'm' {
                            sgr = false;
                        }
                        return false;
                    }
                    true
                })
                .collect();
            let expected = settings.artwork();
            assert_eq!(
                plain
                    .lines()
                    .take(expected.lines().count())
                    .map(str::trim_end)
                    .collect::<Vec<_>>(),
                expected.lines().map(str::trim_end).collect::<Vec<_>>(),
                "{name}"
            );
        }
    }
    #[test]
    fn legacy_presets_preserve_order_and_new_options_roundtrip() {
        let mut original = GreetingSettings::default();
        original.items.reverse();
        let mut json = serde_json::to_value(&original).unwrap();
        json.as_object_mut().unwrap().remove("opening");
        json.as_object_mut().unwrap().remove("preview_columns");
        json["items"]
            .as_array_mut()
            .unwrap()
            .retain(|item| item["kind"] != "gpu" && item["kind"] != "disk");
        let loaded: GreetingSettings = serde_json::from_value(json.clone()).unwrap();
        loaded.validate().unwrap();
        assert_eq!(loaded.preview_columns, 0);
        assert_eq!(loaded.opening, Opening::None);
        assert_eq!(loaded.items[0].kind, Info::Date);
        assert_eq!(
            loaded.items[8],
            Item {
                kind: Info::Gpu,
                enabled: false
            }
        );
        assert_eq!(
            loaded.items[9],
            Item {
                kind: Info::Disk,
                enabled: false
            }
        );
        json["items"].as_array_mut().unwrap().pop();
        assert!(
            serde_json::from_value::<GreetingSettings>(json)
                .unwrap()
                .validate()
                .is_err()
        );
        for columns in [0, 80, 100, 120] {
            for opening in Opening::ALL {
                original.preview_columns = columns;
                original.opening = opening;
                original.logo = Logo::Ubuntu;
                original.position = Position::Card;
                let bytes =
                    crate::document_store::encode(&GreetingPreset::new(original.clone())).unwrap();
                assert_eq!(
                    crate::document_store::decode::<GreetingPreset>(&bytes)
                        .unwrap()
                        .greeting,
                    original
                );
            }
        }
        for columns in [1, 79, 81, 240, u16::MAX] {
            original.preview_columns = columns;
            assert!(original.validate().is_err());
        }
    }

    #[test]
    fn reordering_is_stable_for_every_source_target_and_edge() {
        let base = GreetingSettings::default();
        for source in Info::ALL {
            for target in Info::ALL {
                for after in [false, true] {
                    let mut moved = base.clone();
                    moved.move_item(source, target, after);
                    moved.validate().unwrap();
                    let rest: Vec<_> = moved.items.iter().filter(|i| i.kind != source).collect();
                    assert_eq!(
                        rest,
                        base.items
                            .iter()
                            .filter(|i| i.kind != source)
                            .collect::<Vec<_>>()
                    );
                    if source != target {
                        let from = moved.items.iter().position(|i| i.kind == source).unwrap();
                        let to = moved.items.iter().position(|i| i.kind == target).unwrap();
                        assert_eq!(
                            if after { to + 1 } else { from + 1 },
                            if after { from } else { to }
                        );
                    }
                    assert!(!moved.move_item(source, target, after));
                }
            }
        }
    }

    #[test]
    fn pci_ids_match_only_exact_vendor_and_device_not_subsystems() {
        let ids = "# comment\n8086  Intel Corporation\n\t1234  First GPU\n\t\t9999 0000  Subsystem\n\tabcd  Second GPU\n10de  NVIDIA\n\tabcd  Other GPU\n";
        assert_eq!(pci_name(ids, "8086", "abcd").as_deref(), Some("Second GPU"));
        assert_eq!(pci_name(ids, "10DE", "ABCD").as_deref(), Some("Other GPU"));
        assert!(pci_name(ids, "8086", "9999").is_none());
        assert!(pci_name(ids, "8086", "0000").is_none());
        assert!(pci_name(ids, "0000", "abcd").is_none());
    }

    #[test]
    fn multiple_gpus_render_as_individual_rows_and_follow_the_field_toggle() {
        let context = GreetingContext {
            gpus: vec!["Integrated GPU".into(), "Discrete GPU".into()],
            ..Default::default()
        };
        let mut settings = GreetingSettings {
            enabled: true,
            logo: Logo::None,
            ..Default::default()
        };
        let text = settings.render(&context, 80);
        assert_eq!(text.matches("GPU:").count(), 2);
        assert!(text.contains("Integrated GPU"));
        assert!(text.contains("Discrete GPU"));
        settings
            .items
            .iter_mut()
            .find(|i| i.kind == Info::Gpu)
            .unwrap()
            .enabled = false;
        assert!(!settings.render(&context, 80).contains("GPU"));
    }

    #[test]
    fn new_modules_export_live_data_without_baking_in_theme_or_motion() {
        let mut settings = GreetingSettings {
            enabled: true,
            logo: Logo::Ubuntu,
            preview_columns: 120,
            opening: Opening::Shimmer,
            ..Default::default()
        };
        let source = settings.fastfetch_config().unwrap();
        let json: serde_json::Value = serde_json::from_str(&source).unwrap();
        assert!(!source.contains("shimmer"));
        assert!(!source.contains("preview_columns"));
        assert_eq!(json["display"]["brightColor"], false);
        let modules = json["modules"].as_array().unwrap();
        assert!(
            modules
                .iter()
                .any(|m| m["type"] == "gpu" && m["format"] == "{name}")
        );
        assert!(
            modules
                .iter()
                .any(|m| m["type"] == "disk" && m["folders"] == "/")
        );
        for accent in 0..=16 {
            settings.accent = accent;
            let json: serde_json::Value =
                serde_json::from_str(&settings.fastfetch_config().unwrap()).unwrap();
            assert_eq!(
                json["display"]["color"]["keys"],
                settings.color_code().to_string()
            );
        }
        settings.position = Position::Card;
        let json: serde_json::Value =
            serde_json::from_str(&settings.fastfetch_config().unwrap()).unwrap();
        assert_eq!(json["logo"]["type"], "none");
        assert!(json["modules"][1]["format"].as_str().unwrap().contains('─'));
    }
    #[test]
    fn greeting_roundtrip_disabled_default_and_validation() {
        let default = GreetingSettings::default();
        assert!(default.render(&GreetingContext::default(), 80).is_empty());
        let preset = GreetingPreset::new(default.clone());
        assert_eq!(
            crate::document_store::decode::<GreetingPreset>(
                &crate::document_store::encode(&preset).unwrap()
            )
            .unwrap(),
            preset
        );
        for bad in ["\x1b]52;c;secret\x07", "hello\rworld", "\u{202e}hidden"] {
            let mut settings = default.clone();
            settings.message = bad.into();
            assert!(settings.validate().is_err());
        }
        let mut settings = default;
        settings.items[1] = settings.items[0].clone();
        assert!(settings.validate().is_err());
    }
    #[test]
    fn artwork_width_and_multilingual_clipping_are_bounded() {
        assert_eq!(clip("A中文🦀Z", 5), "A中文");
        assert_eq!(clip("e\u{301}中", 1), "e\u{301}");
        let mut settings = GreetingSettings {
            enabled: true,
            ..Default::default()
        };
        settings.custom_logo = "中".repeat(61);
        assert!(settings.validate().is_err());
        settings.custom_logo = "safe".into();
        for item in &mut settings.items {
            item.enabled = true;
        }
        for columns in [12, 24, 40, 80, 100, 120] {
            for (position, logo) in Position::ALL
                .into_iter()
                .flat_map(|p| Logo::ALL.map(|l| (p, l)))
            {
                settings.position = position;
                settings.logo = logo;
                let text = settings.render(&GreetingContext::default(), columns);
                assert!(text.contains("Welcome"));
                assert!(text.ends_with("\x1b[0m\r\n\r\n"));
                assert!(text.len() < 8000);
                // Count actual printable cells, including the longest key.
                let mut escape = false;
                let plain: String = text
                    .chars()
                    .filter(|ch| {
                        if *ch == '\x1b' {
                            escape = true;
                            return false;
                        }
                        if escape {
                            if *ch == 'm' {
                                escape = false;
                            }
                            return false;
                        }
                        true
                    })
                    .collect();
                for line in plain.lines() {
                    assert!(
                        line.trim_end_matches('\r').width() < columns,
                        "{columns}: {line:?}"
                    );
                }
            }
        }
    }
    #[test]
    fn memory_snapshot_uses_available_memory_and_handles_bad_data() {
        assert_eq!(
            memory_value("MemTotal: 8388608 kB\nMemAvailable: 2097152 kB"),
            Some("6.0 / 8.0 GiB".into())
        );
        assert!(memory_value("MemTotal: no").is_none());
    }
    #[test]
    fn export_contains_only_literal_art_and_reviewed_modules_in_order() {
        let mut settings = GreetingSettings {
            enabled: true,
            logo: Logo::Custom,
            custom_logo: "$1 $(must-not-run)".into(),
            message: "Hello {literal} $(safe)".into(),
            ..Default::default()
        };
        settings.items.reverse();
        let json: serde_json::Value =
            serde_json::from_str(&settings.fastfetch_config().unwrap()).unwrap();
        assert_eq!(json["logo"]["type"], "data-raw");
        assert!(
            json["logo"]["source"]
                .as_str()
                .unwrap()
                .contains("$1 $(must-not-run)")
        );
        assert_eq!(json["modules"][0]["type"], "custom");
        let types: Vec<_> = json["modules"]
            .as_array()
            .unwrap()
            .iter()
            .skip(1)
            .map(|v| v["type"].as_str().unwrap())
            .collect();
        assert_eq!(
            types,
            settings
                .items
                .iter()
                .filter(|i| i.enabled)
                .map(|i| i.kind.module())
                .collect::<Vec<_>>()
        );
        settings.enabled = false;
        let json: serde_json::Value =
            serde_json::from_str(&settings.fastfetch_config().unwrap()).unwrap();
        assert_eq!(json["logo"]["type"], "none");
        assert_eq!(json["modules"].as_array().unwrap().len(), 1);
        assert_eq!(json["modules"][0]["format"], "");
    }

    #[test]
    #[ignore = "requires Fastfetch; set TERMIMOCHI_FASTFETCH_TEST_BIN to an executable"]
    fn real_fastfetch_accepts_export_and_preserves_literal_text() {
        use std::{
            process::{Command, Stdio},
            time::{Duration, Instant},
        };
        let executable =
            std::env::var_os("TERMIMOCHI_FASTFETCH_TEST_BIN").unwrap_or_else(|| "fastfetch".into());
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("greeting.fastfetch.jsonc");
        let mut settings = GreetingSettings {
            enabled: true,
            logo: Logo::Custom,
            custom_logo: "$1 (•‿•)".into(),
            message: "Hello {$TERMIMOCHI_LITERAL_SECRET} {name} $(touch should-not-exist)".into(),
            ..Default::default()
        };
        for (position, logo) in Position::ALL
            .into_iter()
            .flat_map(|p| Logo::ALL.map(|l| (p, l)))
        {
            settings.position = position;
            settings.logo = logo;
            std::fs::write(&path, settings.fastfetch_config().unwrap()).unwrap();
            let mut child = Command::new(&executable)
                .args(["--config"])
                .arg(&path)
                .args(["--pipe", "true", "--show-errors", "true"])
                .current_dir(root.path())
                .env("TERMIMOCHI_LITERAL_SECRET", "SHOULD_NOT_EXPAND")
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap();
            let deadline = Instant::now() + Duration::from_secs(5);
            while child.try_wait().unwrap().is_none() {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    panic!("Fastfetch timed out");
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            let output = child.wait_with_output().unwrap();
            let text = String::from_utf8_lossy(&output.stdout);
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(
                output.stderr.is_empty(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(
                text.contains(&settings.message),
                "literal text missing: {text}"
            );
            assert!(!text.contains("SHOULD_NOT_EXPAND"));
            if position != Position::Card && logo == Logo::Custom {
                assert!(text.contains("$1 (•‿•)"));
            }
            assert!(!text.contains("Unknown"), "{text}");
            assert!(!root.path().join("should-not-exist").exists());
        }
    }
}
