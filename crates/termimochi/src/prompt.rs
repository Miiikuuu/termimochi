use unicode_segmentation::UnicodeSegmentation;

pub(crate) const STARSHIP_FILE_NAME: &str = "starship.toml";
const PREVIEW_VALUE_LIMIT: usize = 64;
const DIRECTORY_COMPONENT_LIMIT: usize = 3;
const DIRECTORY_TRUNCATION_SYMBOL: &str = ".../";
const BRANCH_GRAPHEME_LIMIT: usize = 24;
// Starship takes only the first grapheme of git_branch.truncation_symbol.
const BRANCH_TRUNCATION_SYMBOL: &str = ".";

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum PromptSegmentKind {
    Username,
    Hostname,
    Directory,
    GitBranch,
    GitStatus,
    Rust,
    NodeJs,
    Python,
    Go,
    CommandDuration,
    ExitStatus,
    Jobs,
    Time,
}

impl PromptSegmentKind {
    pub(crate) const ALL: [Self; 13] = [
        Self::Username,
        Self::Hostname,
        Self::Directory,
        Self::GitBranch,
        Self::GitStatus,
        Self::Rust,
        Self::NodeJs,
        Self::Python,
        Self::Go,
        Self::CommandDuration,
        Self::ExitStatus,
        Self::Jobs,
        Self::Time,
    ];

    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Username => "User",
            Self::Hostname => "Host",
            Self::Directory => "Directory",
            Self::GitBranch => "Git Branch",
            Self::GitStatus => "Git Status",
            Self::Rust => "Rust",
            Self::NodeJs => "Node.js",
            Self::Python => "Python",
            Self::Go => "Go",
            Self::CommandDuration => "Command Time",
            Self::ExitStatus => "Exit Status",
            Self::Jobs => "Background Jobs",
            Self::Time => "Time",
        }
    }

    pub(crate) const fn category(self) -> &'static str {
        match self {
            Self::Username | Self::Hostname | Self::Directory => "Context",
            Self::GitBranch | Self::GitStatus => "Git",
            Self::Rust | Self::NodeJs | Self::Python | Self::Go => "Languages",
            Self::CommandDuration | Self::ExitStatus | Self::Jobs | Self::Time => "Runtime",
        }
    }

    pub(crate) const fn description(self) -> &'static str {
        match self {
            Self::Username => "Show the current user",
            Self::Hostname => "Show the machine name, always or only during SSH sessions",
            Self::Directory => "Show the current working directory",
            Self::GitBranch => "Show the active Git branch inside repositories",
            Self::GitStatus => "Show changed, staged, and untracked Git files",
            Self::Rust => "Show the Rust version in Rust projects",
            Self::NodeJs => "Show the Node.js version in JavaScript projects",
            Self::Python => "Show the Python version in Python projects",
            Self::Go => "Show the Go version in Go projects",
            Self::CommandDuration => "Show how long the previous command took",
            Self::ExitStatus => "Show non-zero command exit codes",
            Self::Jobs => "Show active background jobs",
            Self::Time => "Show the local time",
        }
    }

    pub(crate) const fn sample(self) -> &'static str {
        match self {
            Self::Username => "mii",
            Self::Hostname => "@workstation",
            Self::Directory => "in ~/TermiMochi",
            Self::GitBranch => "git:main",
            Self::GitStatus => "!?",
            Self::Rust => "rs v1.89",
            Self::NodeJs => "node v24",
            Self::Python => "py v3.14",
            Self::Go => "go v1.25",
            Self::CommandDuration => "took 842ms",
            Self::ExitStatus => "exit 1",
            Self::Jobs => "jobs 2",
            Self::Time => "23:42",
        }
    }

    pub(crate) const fn starship_module(self) -> &'static str {
        match self {
            Self::Username => "username",
            Self::Hostname => "hostname",
            Self::Directory => "directory",
            Self::GitBranch => "git_branch",
            Self::GitStatus => "git_status",
            Self::Rust => "rust",
            Self::NodeJs => "nodejs",
            Self::Python => "python",
            Self::Go => "golang",
            Self::CommandDuration => "cmd_duration",
            Self::ExitStatus => "status",
            Self::Jobs => "jobs",
            Self::Time => "time",
        }
    }

    const fn starship_format(self) -> &'static str {
        match self {
            Self::Username => "[$user]($style) ",
            Self::Hostname => "[@$hostname]($style) ",
            Self::Directory => "[in $path]($style) ",
            Self::GitBranch => "[git:$branch]($style) ",
            Self::GitStatus => "([$all_status$ahead_behind]($style) )",
            Self::Rust => "[rs $version]($style) ",
            Self::NodeJs => "[node $version]($style) ",
            Self::Python => "[py $version]($style) ",
            Self::Go => "[go $version]($style) ",
            Self::CommandDuration => "[took $duration]($style) ",
            Self::ExitStatus => "[exit $status]($style) ",
            Self::Jobs => "[$symbol$number]($style) ",
            Self::Time => "[$time]($style) ",
        }
    }

    pub(crate) const fn default_tone(self) -> PreviewTone {
        match self {
            Self::Username => PreviewTone::Cyan,
            Self::Hostname => PreviewTone::Green,
            Self::Directory => PreviewTone::Blue,
            Self::GitBranch => PreviewTone::Magenta,
            Self::GitStatus => PreviewTone::Yellow,
            Self::Rust => PreviewTone::Red,
            Self::NodeJs => PreviewTone::Green,
            Self::Python => PreviewTone::Yellow,
            Self::Go => PreviewTone::Cyan,
            Self::CommandDuration => PreviewTone::Yellow,
            Self::ExitStatus => PreviewTone::Red,
            Self::Jobs => PreviewTone::Cyan,
            Self::Time => PreviewTone::Blue,
        }
    }

    #[cfg(test)]
    pub(crate) fn from_index(index: u32) -> Self {
        usize::try_from(index)
            .ok()
            .and_then(|index| Self::ALL.get(index))
            .copied()
            .unwrap_or(Self::Directory)
    }

    pub(crate) const fn index(self) -> u32 {
        match self {
            Self::Username => 0,
            Self::Hostname => 1,
            Self::Directory => 2,
            Self::GitBranch => 3,
            Self::GitStatus => 4,
            Self::Rust => 5,
            Self::NodeJs => 6,
            Self::Python => 7,
            Self::Go => 8,
            Self::CommandDuration => 9,
            Self::ExitStatus => 10,
            Self::Jobs => 11,
            Self::Time => 12,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum PromptPreset {
    Essential,
    #[default]
    Developer,
    Remote,
    Blank,
}

impl PromptPreset {
    pub(crate) const ALL: [Self; 4] = [Self::Essential, Self::Developer, Self::Remote, Self::Blank];

    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Essential => "Essential",
            Self::Developer => "Developer",
            Self::Remote => "Remote",
            Self::Blank => "Blank",
        }
    }

    pub(crate) fn from_index(index: u32) -> Self {
        usize::try_from(index)
            .ok()
            .and_then(|index| Self::ALL.get(index))
            .copied()
            .unwrap_or_default()
    }

    #[cfg(test)]
    pub(crate) const fn index(self) -> u32 {
        match self {
            Self::Essential => 0,
            Self::Developer => 1,
            Self::Remote => 2,
            Self::Blank => 3,
        }
    }

    const fn segment_order(self) -> [PromptSegmentKind; 13] {
        use PromptSegmentKind as Kind;
        match self {
            Self::Essential => [
                Kind::Directory,
                Kind::GitBranch,
                Kind::ExitStatus,
                Kind::GitStatus,
                Kind::Username,
                Kind::Hostname,
                Kind::Rust,
                Kind::NodeJs,
                Kind::Python,
                Kind::Go,
                Kind::CommandDuration,
                Kind::Jobs,
                Kind::Time,
            ],
            Self::Developer => [
                Kind::Directory,
                Kind::GitBranch,
                Kind::GitStatus,
                Kind::Rust,
                Kind::NodeJs,
                Kind::Python,
                Kind::Go,
                Kind::CommandDuration,
                Kind::ExitStatus,
                Kind::Username,
                Kind::Hostname,
                Kind::Jobs,
                Kind::Time,
            ],
            Self::Remote => [
                Kind::Username,
                Kind::Hostname,
                Kind::Directory,
                Kind::GitBranch,
                Kind::GitStatus,
                Kind::Jobs,
                Kind::CommandDuration,
                Kind::ExitStatus,
                Kind::Time,
                Kind::Rust,
                Kind::NodeJs,
                Kind::Python,
                Kind::Go,
            ],
            Self::Blank => PromptSegmentKind::ALL,
        }
    }

    const fn is_initially_enabled(self, kind: PromptSegmentKind) -> bool {
        use PromptSegmentKind as Kind;
        match self {
            Self::Essential => matches!(kind, Kind::Directory | Kind::GitBranch | Kind::ExitStatus),
            Self::Developer => matches!(
                kind,
                Kind::Directory
                    | Kind::GitBranch
                    | Kind::GitStatus
                    | Kind::Rust
                    | Kind::NodeJs
                    | Kind::Python
                    | Kind::Go
                    | Kind::CommandDuration
                    | Kind::ExitStatus
            ),
            Self::Remote => matches!(
                kind,
                Kind::Username
                    | Kind::Hostname
                    | Kind::Directory
                    | Kind::GitBranch
                    | Kind::GitStatus
                    | Kind::CommandDuration
                    | Kind::ExitStatus
                    | Kind::Jobs
                    | Kind::Time
            ),
            Self::Blank => false,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PromptSegment {
    pub(crate) kind: PromptSegmentKind,
    pub(crate) enabled: bool,
    pub(crate) tone: PreviewTone,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PromptSettings {
    source_preset: Option<PromptPreset>,
    segments: Vec<PromptSegment>,
    character: PromptCharacter,
    layout: PromptLayout,
    add_newline: bool,
    hostname_mode: PromptHostnameMode,
}

impl Default for PromptSettings {
    fn default() -> Self {
        Self::from_preset(PromptPreset::default())
    }
}

impl PromptSettings {
    pub(crate) fn from_preset(preset: PromptPreset) -> Self {
        let segments = preset
            .segment_order()
            .into_iter()
            .map(|kind| PromptSegment {
                kind,
                enabled: preset.is_initially_enabled(kind),
                tone: kind.default_tone(),
            })
            .collect();
        Self {
            source_preset: Some(preset),
            segments,
            character: PromptCharacter::Chevron,
            layout: PromptLayout::SingleLine,
            add_newline: false,
            hostname_mode: if preset == PromptPreset::Remote {
                PromptHostnameMode::SshOnly
            } else {
                PromptHostnameMode::Always
            },
        }
    }

    pub(crate) fn apply_preset(&mut self, preset: PromptPreset) {
        *self = Self::from_preset(preset);
    }

    pub(crate) const fn source_preset(&self) -> Option<PromptPreset> {
        self.source_preset
    }

    pub(crate) fn segments(&self) -> &[PromptSegment] {
        &self.segments
    }

    pub(crate) fn is_enabled(&self, kind: PromptSegmentKind) -> bool {
        self.segments
            .iter()
            .find(|segment| segment.kind == kind)
            .is_some_and(|segment| segment.enabled)
    }

    pub(crate) fn enabled_count(&self) -> usize {
        self.segments
            .iter()
            .filter(|segment| segment.enabled)
            .count()
    }

    pub(crate) fn active_position(&self, kind: PromptSegmentKind) -> Option<(usize, usize)> {
        let active = self.enabled_segments().collect::<Vec<_>>();
        active
            .iter()
            .position(|segment| segment.kind == kind)
            .map(|position| (position, active.len()))
    }

    pub(crate) fn tone(&self, kind: PromptSegmentKind) -> PreviewTone {
        self.segments
            .iter()
            .find(|segment| segment.kind == kind)
            .map_or_else(|| kind.default_tone(), |segment| segment.tone)
    }

    pub(crate) const fn character(&self) -> PromptCharacter {
        self.character
    }

    pub(crate) const fn layout(&self) -> PromptLayout {
        self.layout
    }

    pub(crate) const fn add_newline(&self) -> bool {
        self.add_newline
    }

    pub(crate) const fn hostname_mode(&self) -> PromptHostnameMode {
        self.hostname_mode
    }

    pub(crate) fn set_layout(&mut self, layout: PromptLayout) -> bool {
        if self.layout == layout {
            return false;
        }
        self.layout = layout;
        self.source_preset = None;
        true
    }

    pub(crate) fn set_add_newline(&mut self, add_newline: bool) -> bool {
        if self.add_newline == add_newline {
            return false;
        }
        self.add_newline = add_newline;
        self.source_preset = None;
        true
    }

    pub(crate) fn set_hostname_mode(&mut self, mode: PromptHostnameMode) -> bool {
        if self.hostname_mode == mode {
            return false;
        }
        self.hostname_mode = mode;
        self.source_preset = None;
        true
    }

    pub(crate) fn add_module(&mut self, kind: PromptSegmentKind) -> bool {
        let Some(position) = self
            .segments
            .iter()
            .position(|segment| segment.kind == kind)
        else {
            return false;
        };
        if self.segments[position].enabled {
            return false;
        }
        let mut segment = self.segments.remove(position);
        segment.enabled = true;
        self.segments.push(segment);
        self.source_preset = None;
        true
    }

    pub(crate) fn remove_module(&mut self, kind: PromptSegmentKind) -> bool {
        let Some(segment) = self
            .segments
            .iter_mut()
            .find(|segment| segment.kind == kind)
        else {
            return false;
        };
        if !segment.enabled {
            return false;
        }
        segment.enabled = false;
        self.source_preset = None;
        true
    }

    pub(crate) fn move_module_up(&mut self, kind: PromptSegmentKind) -> bool {
        let Some(source) = self
            .segments
            .iter()
            .position(|segment| segment.kind == kind && segment.enabled)
        else {
            return false;
        };
        let Some(target) = (0..source)
            .rev()
            .find(|index| self.segments[*index].enabled)
        else {
            return false;
        };
        self.segments.swap(source, target);
        self.source_preset = None;
        true
    }

    pub(crate) fn move_module_down(&mut self, kind: PromptSegmentKind) -> bool {
        let Some(source) = self
            .segments
            .iter()
            .position(|segment| segment.kind == kind && segment.enabled)
        else {
            return false;
        };
        let Some(target) =
            (source + 1..self.segments.len()).find(|index| self.segments[*index].enabled)
        else {
            return false;
        };
        self.segments.swap(source, target);
        self.source_preset = None;
        true
    }

    pub(crate) fn set_tone(&mut self, kind: PromptSegmentKind, tone: PreviewTone) -> bool {
        let Some(segment) = self
            .segments
            .iter_mut()
            .find(|segment| segment.kind == kind)
        else {
            return false;
        };
        if segment.tone == tone {
            return false;
        }
        segment.tone = tone;
        self.source_preset = None;
        true
    }

    pub(crate) fn set_character(&mut self, character: PromptCharacter) -> bool {
        if self.character == character {
            return false;
        }
        self.character = character;
        self.source_preset = None;
        true
    }

    pub(crate) fn enabled_segments(&self) -> impl Iterator<Item = &PromptSegment> {
        self.segments.iter().filter(|segment| segment.enabled)
    }

    pub(crate) fn preview_parts(
        &self,
        context: &PromptPreviewContext<'_>,
    ) -> Vec<PromptPreviewPart> {
        self.enabled_segments()
            .filter_map(|segment| {
                let kind = segment.kind;
                let value = match kind {
                    PromptSegmentKind::Username => sanitize_preview_value(context.username, "user"),
                    PromptSegmentKind::Hostname => {
                        if self.hostname_mode == PromptHostnameMode::SshOnly && !context.is_ssh {
                            return None;
                        }
                        sanitize_preview_value(context.hostname, "host")
                    }
                    PromptSegmentKind::Directory => preview_directory(context.path),
                    PromptSegmentKind::GitBranch => preview_git_branch(context.git_branch?),
                    PromptSegmentKind::GitStatus => {
                        sanitize_preview_value(context.git_status?, "clean")
                            .replace('»', "r")
                            .replace('✘', "x")
                    }
                    PromptSegmentKind::Rust => {
                        sanitize_preview_value(context.rust_version?, "v?.?.?")
                    }
                    PromptSegmentKind::NodeJs => {
                        sanitize_preview_value(context.node_version?, "v?.?.?")
                    }
                    PromptSegmentKind::Python => {
                        sanitize_preview_value(context.python_version?, "v?.?.?")
                    }
                    PromptSegmentKind::Go => sanitize_preview_value(context.go_version?, "v?.?.?"),
                    PromptSegmentKind::CommandDuration => {
                        sanitize_preview_value(context.command_duration?, "0ms")
                    }
                    PromptSegmentKind::ExitStatus => {
                        if context.exit_status == 0 {
                            return None;
                        }
                        context.exit_status.to_string()
                    }
                    PromptSegmentKind::Jobs => sanitize_preview_value(context.jobs?, "1"),
                    PromptSegmentKind::Time => sanitize_preview_value(context.time?, "00:00"),
                };
                let text = match kind {
                    PromptSegmentKind::Hostname => format!("@{value}"),
                    PromptSegmentKind::Directory => format!("in {value}"),
                    PromptSegmentKind::GitBranch => format!("git:{value}"),
                    PromptSegmentKind::Rust => format!("rs {value}"),
                    PromptSegmentKind::NodeJs => format!("node {value}"),
                    PromptSegmentKind::Python => format!("py {value}"),
                    PromptSegmentKind::Go => format!("go {value}"),
                    PromptSegmentKind::CommandDuration => format!("took {value}"),
                    PromptSegmentKind::ExitStatus => format!("exit {value}"),
                    PromptSegmentKind::Jobs => format!("jobs {value}"),
                    _ => value,
                };
                Some(PromptPreviewPart {
                    kind,
                    text,
                    tone: if kind == PromptSegmentKind::Username && context.is_root {
                        PreviewTone::Red
                    } else {
                        segment.tone
                    },
                })
            })
            .collect()
    }

    #[cfg(test)]
    pub(crate) fn preview_text(&self, context: &PromptPreviewContext<'_>) -> String {
        let mut line = self
            .preview_parts(context)
            .into_iter()
            .map(|part| part.text)
            .collect::<Vec<_>>()
            .join(" ");
        if !line.is_empty() {
            line.push(' ');
        }
        if self.has_second_line() {
            line.push('\n');
        }
        line.push_str(self.character.symbol());
        line.push(' ');
        if self.add_newline {
            line.insert(0, '\n');
        }
        line
    }

    /// Produces an SGR-only specimen for VTE. Every context value is filtered
    /// before escape codes are added, preventing terminal-control injection.
    pub(crate) fn preview_ansi(&self, context: &PromptPreviewContext<'_>) -> String {
        self.preview_ansi_runs(context)
            .into_iter()
            .map(|(_, text)| text)
            .collect()
    }

    /// Semantic runs for point-to-edit. Joining them is exactly preview_ansi;
    /// no markers, hyperlinks, or extra styling enter the terminal/export.
    pub(crate) fn preview_ansi_runs(
        &self,
        context: &PromptPreviewContext<'_>,
    ) -> Vec<(Option<PromptSegmentKind>, String)> {
        let mut runs = Vec::new();
        let mut preview = String::new();
        if self.add_newline {
            preview.push_str("\r\n");
        }
        for part in self.preview_parts(context) {
            preview.push_str(part.tone.sgr());
            preview.push_str(&part.text);
            preview.push_str("\x1b[0m ");
            runs.push((Some(part.kind), std::mem::take(&mut preview)));
        }
        if self.has_second_line() {
            preview.push_str("\r\n");
        }
        preview.push_str(if context.exit_status == 0 {
            PreviewTone::Green.sgr()
        } else {
            PreviewTone::Red.sgr()
        });
        preview.push_str(self.character.symbol());
        preview.push_str("\x1b[0m ");
        runs.push((None, preview));
        runs
    }

    fn has_second_line(&self) -> bool {
        self.layout == PromptLayout::TwoLine && self.enabled_count() > 0
    }

    /// Serializes only a Starship configuration. This pure function performs
    /// no file I/O and never reads or modifies shell startup files.
    pub(crate) fn to_starship_toml(&self) -> String {
        let modules = self.enabled_segments().copied().collect::<Vec<_>>();
        let mut output = String::from(
            "# Generated by TermiMochi. Review before replacing an existing configuration.\n\
             # This is a complete configuration, not a merge of your current Starship settings.\n\
             # Custom commands and shell startup files are not read or modified.\n\
             # Uses standard ANSI colors and text labels; no Nerd Font is required.\n\
             \"$schema\" = 'https://starship.rs/config-schema.json'\n",
        );
        output.push_str(if self.add_newline {
            "add_newline = true\n"
        } else {
            "add_newline = false\n"
        });
        output.push_str("format = '");
        for segment in &modules {
            output.push('$');
            output.push_str(segment.kind.starship_module());
        }
        if self.has_second_line() {
            output.push_str("$line_break");
        }
        output.push_str("$character'\n");

        for segment in modules {
            output.push('\n');
            self.write_module(segment, &mut output);
        }
        output.push('\n');
        self.write_character(&mut output);
        output
    }

    fn write_module(&self, segment: PromptSegment, output: &mut String) {
        let kind = segment.kind;
        output.push('[');
        output.push_str(kind.starship_module());
        output.push_str("]\n");
        push_literal(output, "format", kind.starship_format());
        if kind == PromptSegmentKind::Username {
            push_literal(output, "style_user", segment.tone.starship_style());
            push_literal(output, "style_root", "bold red");
        } else {
            push_literal(output, "style", segment.tone.starship_style());
        }

        match kind {
            PromptSegmentKind::Username => output.push_str("show_always = true\n"),
            PromptSegmentKind::Hostname => {
                output.push_str(match self.hostname_mode {
                    PromptHostnameMode::Always => "ssh_only = false\n",
                    PromptHostnameMode::SshOnly => "ssh_only = true\n",
                });
                // Keep the same full machine name as the current-session preview.
                push_literal(output, "trim_at", "");
            }
            PromptSegmentKind::Directory => {
                output.push_str(&format!(
                    "truncation_length = {DIRECTORY_COMPONENT_LIMIT}\ntruncate_to_repo = false\n"
                ));
                push_literal(output, "truncation_symbol", DIRECTORY_TRUNCATION_SYMBOL);
            }
            PromptSegmentKind::GitBranch => {
                output.push_str(&format!("truncation_length = {BRANCH_GRAPHEME_LIMIT}\n"));
                push_literal(output, "truncation_symbol", BRANCH_TRUNCATION_SYMBOL);
            }
            PromptSegmentKind::CommandDuration => {
                output.push_str("min_time = 0\nshow_milliseconds = true\n");
            }
            PromptSegmentKind::ExitStatus => output.push_str("disabled = false\n"),
            PromptSegmentKind::Jobs => {
                push_literal(output, "symbol", "jobs ");
                output.push_str("number_threshold = 1\n");
            }
            PromptSegmentKind::Time => {
                push_literal(output, "time_format", "%H:%M");
                output.push_str("disabled = false\n");
            }
            PromptSegmentKind::GitStatus => {
                // Starship's defaults use Unicode arrows. These status labels
                // remain legible over SSH and in fonts without those glyphs.
                push_literal(output, "ahead", ">${count}");
                push_literal(output, "behind", "<${count}");
                push_literal(output, "diverged", "<>${ahead_count}/${behind_count}");
                push_literal(output, "renamed", "r");
                push_literal(output, "deleted", "x");
            }
            PromptSegmentKind::Rust
            | PromptSegmentKind::NodeJs
            | PromptSegmentKind::Python
            | PromptSegmentKind::Go => {}
        }
    }

    fn write_character(&self, output: &mut String) {
        output.push_str("[character]\n");
        let success = format!("[{}](bold green)", self.character.starship_symbol());
        let error = format!("[{}](bold red)", self.character.starship_symbol());
        push_literal(output, "success_symbol", &success);
        push_literal(output, "error_symbol", &error);
        push_literal(output, "vimcmd_symbol", "[<](bold green)");
        push_literal(output, "vimcmd_replace_one_symbol", "[r](bold purple)");
        push_literal(output, "vimcmd_replace_symbol", "[R](bold purple)");
        push_literal(output, "vimcmd_visual_symbol", "[V](bold yellow)");
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PromptLayout {
    SingleLine,
    TwoLine,
}

impl PromptLayout {
    pub(crate) const ALL: [Self; 2] = [Self::SingleLine, Self::TwoLine];

    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::SingleLine => "One line",
            Self::TwoLine => "Two lines",
        }
    }

    pub(crate) fn from_index(index: u32) -> Self {
        Self::ALL
            .get(index as usize)
            .copied()
            .unwrap_or(Self::SingleLine)
    }

    pub(crate) const fn index(self) -> u32 {
        match self {
            Self::SingleLine => 0,
            Self::TwoLine => 1,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PromptHostnameMode {
    Always,
    SshOnly,
}

impl PromptHostnameMode {
    pub(crate) const ALL: [Self; 2] = [Self::Always, Self::SshOnly];

    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Always => "Always",
            Self::SshOnly => "SSH only",
        }
    }

    pub(crate) fn from_index(index: u32) -> Self {
        Self::ALL
            .get(index as usize)
            .copied()
            .unwrap_or(Self::Always)
    }

    pub(crate) const fn index(self) -> u32 {
        match self {
            Self::Always => 0,
            Self::SshOnly => 1,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PreviewTone {
    Cyan,
    Blue,
    Magenta,
    Yellow,
    Green,
    Red,
}

impl PreviewTone {
    pub(crate) const ALL: [Self; 6] = [
        Self::Cyan,
        Self::Blue,
        Self::Magenta,
        Self::Yellow,
        Self::Green,
        Self::Red,
    ];

    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Cyan => "Cyan",
            Self::Blue => "Blue",
            Self::Magenta => "Violet",
            Self::Yellow => "Amber",
            Self::Green => "Green",
            Self::Red => "Red",
        }
    }

    pub(crate) fn from_index(index: u32) -> Self {
        usize::try_from(index)
            .ok()
            .and_then(|index| Self::ALL.get(index))
            .copied()
            .unwrap_or(Self::Blue)
    }

    pub(crate) const fn index(self) -> u32 {
        match self {
            Self::Cyan => 0,
            Self::Blue => 1,
            Self::Magenta => 2,
            Self::Yellow => 3,
            Self::Green => 4,
            Self::Red => 5,
        }
    }

    pub(crate) const fn css_class(self) -> &'static str {
        match self {
            Self::Cyan => "prompt-token-cyan",
            Self::Blue => "prompt-token-blue",
            Self::Magenta => "prompt-token-magenta",
            Self::Yellow => "prompt-token-yellow",
            Self::Green => "prompt-token-green",
            Self::Red => "prompt-token-red",
        }
    }

    const fn sgr(self) -> &'static str {
        match self {
            Self::Cyan => "\x1b[1;36m",
            Self::Blue => "\x1b[1;34m",
            Self::Magenta => "\x1b[1;35m",
            Self::Yellow => "\x1b[1;33m",
            Self::Green => "\x1b[1;32m",
            Self::Red => "\x1b[1;31m",
        }
    }

    const fn starship_style(self) -> &'static str {
        match self {
            Self::Cyan => "bold cyan",
            Self::Blue => "bold blue",
            Self::Magenta => "bold purple",
            Self::Yellow => "bold yellow",
            Self::Green => "bold green",
            Self::Red => "bold red",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PromptCharacter {
    Chevron,
    Arrow,
    Dollar,
    Lambda,
    Ascii,
}

impl PromptCharacter {
    pub(crate) const ALL: [Self; 5] = [
        Self::Chevron,
        Self::Arrow,
        Self::Dollar,
        Self::Lambda,
        Self::Ascii,
    ];

    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Chevron => "Chevron  ❯",
            Self::Arrow => "Arrow  →",
            Self::Dollar => "Dollar  $",
            Self::Lambda => "Lambda  λ",
            Self::Ascii => "ASCII  >",
        }
    }

    pub(crate) fn from_index(index: u32) -> Self {
        usize::try_from(index)
            .ok()
            .and_then(|index| Self::ALL.get(index))
            .copied()
            .unwrap_or(Self::Chevron)
    }

    pub(crate) const fn index(self) -> u32 {
        match self {
            Self::Chevron => 0,
            Self::Arrow => 1,
            Self::Dollar => 2,
            Self::Lambda => 3,
            Self::Ascii => 4,
        }
    }

    const fn symbol(self) -> &'static str {
        match self {
            Self::Chevron => "❯",
            Self::Arrow => "→",
            Self::Dollar => "$",
            Self::Lambda => "λ",
            Self::Ascii => ">",
        }
    }

    const fn starship_symbol(self) -> &'static str {
        match self {
            // Starship treats an unescaped dollar as the start of a variable
            // even inside a styled text group.
            Self::Dollar => "\\$",
            _ => self.symbol(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PromptPreviewPart {
    pub(crate) kind: PromptSegmentKind,
    pub(crate) text: String,
    pub(crate) tone: PreviewTone,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PromptPreviewContext<'a> {
    pub(crate) username: &'a str,
    pub(crate) hostname: &'a str,
    pub(crate) path: &'a str,
    pub(crate) git_branch: Option<&'a str>,
    pub(crate) git_status: Option<&'a str>,
    pub(crate) rust_version: Option<&'a str>,
    pub(crate) node_version: Option<&'a str>,
    pub(crate) python_version: Option<&'a str>,
    pub(crate) go_version: Option<&'a str>,
    pub(crate) command_duration: Option<&'a str>,
    pub(crate) jobs: Option<&'a str>,
    pub(crate) time: Option<&'a str>,
    pub(crate) exit_status: i32,
    pub(crate) is_root: bool,
    pub(crate) is_ssh: bool,
}

impl Default for PromptPreviewContext<'_> {
    fn default() -> Self {
        Self {
            username: "mochi",
            hostname: "workstation",
            path: "~/TermiMochi",
            git_branch: Some("main"),
            git_status: Some("!?"),
            rust_version: Some("v1.89"),
            node_version: Some("v24"),
            python_version: Some("v3.14"),
            go_version: Some("v1.25"),
            command_duration: Some("842ms"),
            jobs: Some("2"),
            time: Some("23:42"),
            exit_status: 0,
            is_root: false,
            is_ssh: false,
        }
    }
}

fn push_literal(output: &mut String, key: &str, value: &str) {
    assert!(
        !value.contains(['\'', '\n', '\r']),
        "internal Starship literal must not contain quotes or line breaks"
    );
    output.push_str(key);
    output.push_str(" = '");
    output.push_str(value);
    output.push_str("'\n");
}

fn sanitize_preview_value(value: &str, fallback: &str) -> String {
    let mut sanitized = value
        .chars()
        .filter(|character| !is_unsafe_preview_character(*character))
        .take(PREVIEW_VALUE_LIMIT + 1)
        .collect::<String>();
    if sanitized.chars().count() > PREVIEW_VALUE_LIMIT {
        let truncate_at = sanitized
            .char_indices()
            .nth(PREVIEW_VALUE_LIMIT)
            .map_or(sanitized.len(), |(index, _)| index);
        sanitized.truncate(truncate_at);
        sanitized.push('…');
    }
    if sanitized.trim().is_empty() {
        fallback.to_owned()
    } else {
        sanitized
    }
}

fn preview_directory(path: &str) -> String {
    // Starship counts a contracted home marker as a component, but not the
    // leading root slash. Work backwards before the preview's character limit
    // so long ancestor names cannot hide the actual working directory.
    let mut components = path.strip_prefix('/').unwrap_or(path).rsplit('/');
    let mut tail = components
        .by_ref()
        .take(DIRECTORY_COMPONENT_LIMIT)
        .collect::<Vec<_>>();
    if components.next().is_none() {
        return sanitize_preview_value(path, "~");
    }
    tail.reverse();
    sanitize_preview_value(
        &format!("{DIRECTORY_TRUNCATION_SYMBOL}{}", tail.join("/")),
        "~",
    )
}

fn preview_git_branch(branch: &str) -> String {
    let sanitized = branch
        .chars()
        .filter(|character| !is_unsafe_preview_character(*character))
        .collect::<String>();
    let mut graphemes = sanitized.graphemes(true);
    let mut preview = graphemes
        .by_ref()
        .take(BRANCH_GRAPHEME_LIMIT)
        .collect::<String>();
    if graphemes.next().is_some() {
        preview.push_str(BRANCH_TRUNCATION_SYMBOL);
    }
    if preview.trim().is_empty() {
        "HEAD".to_owned()
    } else {
        preview
    }
}

fn is_unsafe_preview_character(character: char) -> bool {
    character.is_control()
        || matches!(
            character as u32,
            0x061c | 0x200e..=0x200f | 0x202a..=0x202e | 0x2066..=0x2069
        )
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn catalog_metadata_is_stable_and_unique() {
        assert_eq!(PromptSegmentKind::ALL.len(), 13);
        assert_eq!(
            PromptPreset::ALL.map(PromptPreset::label),
            ["Essential", "Developer", "Remote", "Blank"]
        );
        let mut labels = HashSet::new();
        let mut modules = HashSet::new();
        for kind in PromptSegmentKind::ALL {
            assert_eq!(PromptSegmentKind::from_index(kind.index()), kind);
            assert!(labels.insert(kind.label()));
            assert!(modules.insert(kind.starship_module()));
            assert!(!kind.category().is_empty());
            assert!(!kind.description().is_empty());
        }
        for preset in PromptPreset::ALL {
            assert_eq!(PromptPreset::from_index(preset.index()), preset);
        }
        assert_eq!(
            PromptSegmentKind::from_index(u32::MAX),
            PromptSegmentKind::Directory
        );
        assert_eq!(PromptPreset::from_index(u32::MAX), PromptPreset::Developer);
    }

    #[test]
    fn presets_are_complete_unique_starting_points() {
        for preset in PromptPreset::ALL {
            let settings = PromptSettings::from_preset(preset);
            assert_eq!(settings.source_preset(), Some(preset));
            assert_eq!(settings.segments().len(), PromptSegmentKind::ALL.len());
            let kinds = settings
                .segments()
                .iter()
                .map(|segment| segment.kind)
                .collect::<HashSet<_>>();
            assert_eq!(kinds.len(), PromptSegmentKind::ALL.len());
        }
        let enabled = |preset| {
            PromptSettings::from_preset(preset)
                .enabled_segments()
                .map(|segment| segment.kind)
                .collect::<Vec<_>>()
        };
        use PromptSegmentKind as Kind;
        assert_eq!(
            enabled(PromptPreset::Essential),
            [Kind::Directory, Kind::GitBranch, Kind::ExitStatus]
        );
        assert_eq!(
            enabled(PromptPreset::Developer),
            [
                Kind::Directory,
                Kind::GitBranch,
                Kind::GitStatus,
                Kind::Rust,
                Kind::NodeJs,
                Kind::Python,
                Kind::Go,
                Kind::CommandDuration,
                Kind::ExitStatus,
            ]
        );
        assert_eq!(
            enabled(PromptPreset::Remote),
            [
                Kind::Username,
                Kind::Hostname,
                Kind::Directory,
                Kind::GitBranch,
                Kind::GitStatus,
                Kind::Jobs,
                Kind::CommandDuration,
                Kind::ExitStatus,
                Kind::Time,
            ]
        );
        assert!(enabled(PromptPreset::Blank).is_empty());
    }

    #[test]
    fn structural_edits_are_safe_and_become_custom() {
        let mut settings = PromptSettings::default();
        assert!(!settings.add_module(PromptSegmentKind::Directory));
        assert!(settings.add_module(PromptSegmentKind::Jobs));
        assert_eq!(settings.source_preset(), None);
        assert!(!settings.add_module(PromptSegmentKind::Jobs));
        assert!(settings.remove_module(PromptSegmentKind::Jobs));
        assert!(!settings.remove_module(PromptSegmentKind::Jobs));

        let before = settings
            .enabled_segments()
            .map(|segment| segment.kind)
            .collect::<Vec<_>>();
        assert!(!settings.move_module_up(before[0]));
        assert!(!settings.move_module_down(*before.last().expect("active module")));
        assert!(settings.move_module_down(before[0]));
        assert!(settings.move_module_up(before[0]));
        assert_eq!(
            settings
                .enabled_segments()
                .map(|segment| segment.kind)
                .collect::<Vec<_>>(),
            before
        );

        settings.apply_preset(PromptPreset::Essential);
        assert_eq!(settings.source_preset(), Some(PromptPreset::Essential));
        assert_eq!(settings.enabled_count(), 3);
    }

    #[test]
    fn appearance_edits_are_typed_and_become_custom() {
        let mut settings = PromptSettings::default();
        assert!(settings.set_tone(PromptSegmentKind::Directory, PreviewTone::Green));
        assert_eq!(
            settings.tone(PromptSegmentKind::Directory),
            PreviewTone::Green
        );
        assert_eq!(settings.source_preset(), None);
        assert!(!settings.set_tone(PromptSegmentKind::Directory, PreviewTone::Green));
        assert!(settings.set_character(PromptCharacter::Lambda));
        assert!(!settings.set_character(PromptCharacter::Lambda));
    }

    #[test]
    fn preview_follows_order_and_hides_success_status() {
        let mut settings = PromptSettings::default();
        while settings.move_module_up(PromptSegmentKind::ExitStatus) {}
        let success = PromptPreviewContext::default();
        assert!(
            !settings
                .preview_parts(&success)
                .iter()
                .any(|part| part.kind == PromptSegmentKind::ExitStatus)
        );

        let failure = PromptPreviewContext {
            exit_status: 127,
            ..success
        };
        let parts = settings.preview_parts(&failure);
        assert_eq!(parts[0].kind, PromptSegmentKind::ExitStatus);
        assert_eq!(parts[0].text, "exit 127");
        assert_eq!(parts[0].tone, PreviewTone::Red);
    }

    #[test]
    fn contextual_modules_disappear_without_context() {
        let mut settings = PromptSettings::from_preset(PromptPreset::Blank);
        for kind in [
            PromptSegmentKind::GitBranch,
            PromptSegmentKind::GitStatus,
            PromptSegmentKind::Rust,
            PromptSegmentKind::NodeJs,
            PromptSegmentKind::Python,
            PromptSegmentKind::Go,
            PromptSegmentKind::Jobs,
        ] {
            assert!(settings.add_module(kind));
        }
        let context = PromptPreviewContext {
            git_branch: None,
            git_status: None,
            rust_version: None,
            node_version: None,
            python_version: None,
            go_version: None,
            jobs: None,
            ..PromptPreviewContext::default()
        };
        assert!(settings.preview_parts(&context).is_empty());
        assert_eq!(settings.preview_text(&context), "❯ ");
    }

    #[test]
    fn preview_removes_terminal_and_bidi_injection_and_bounds_values() {
        let settings = PromptSettings::default();
        let long_path = "a".repeat(PREVIEW_VALUE_LIMIT + 20);
        let context = PromptPreviewContext {
            path: &long_path,
            git_branch: Some("ma\u{202e}ni"),
            git_status: Some("!\x1b[31m\n"),
            command_duration: Some("\r842ms\u{2066}"),
            exit_status: 1,
            ..PromptPreviewContext::default()
        };
        let plain = settings.preview_text(&context);
        assert!(!plain.contains('\x1b'));
        assert!(!plain.contains('\n'));
        assert!(!plain.contains('\r'));
        assert!(!plain.contains('\u{202e}'));
        assert!(!plain.contains('\u{2066}'));
        assert!(plain.contains(&format!("{}…", "a".repeat(PREVIEW_VALUE_LIMIT))));

        let ansi = settings.preview_ansi(&context);
        assert!(!ansi.contains("\x1b[31m\n"));
        assert!(ansi.ends_with("\x1b[0m "));
    }

    #[test]
    fn export_follows_visible_order_and_is_valid_toml() {
        let mut settings = PromptSettings::from_preset(PromptPreset::Blank);
        for kind in [
            PromptSegmentKind::Time,
            PromptSegmentKind::Directory,
            PromptSegmentKind::GitBranch,
        ] {
            assert!(settings.add_module(kind));
        }
        let exported = settings.to_starship_toml();
        assert!(exported.contains("format = '$time$directory$git_branch$character'"));
        assert!(exported.contains("[time]\n"));
        assert!(exported.contains("disabled = false\n"));
        toml::from_str::<toml::Table>(&exported).expect("export must be valid TOML");
    }

    #[test]
    fn export_uses_each_special_modules_supported_starship_keys() {
        let settings = PromptSettings::from_preset(PromptPreset::Remote);
        let exported = settings.to_starship_toml();
        assert!(exported.contains(
            "[username]\nformat = '[$user]($style) '\nstyle_user = 'bold cyan'\nstyle_root = 'bold red'\nshow_always = true"
        ));
        assert!(exported.contains("[hostname]\n"));
        assert!(exported.contains("ssh_only = true\n"));
        assert!(exported.contains("[status]\n"));
        assert!(exported.contains("disabled = false\n"));
        assert!(exported.contains("[time]\n"));
        assert!(exported.contains("time_format = '%H:%M'\n"));
        assert!(exported.contains("[jobs]\n"));
        assert!(exported.contains("symbol = 'jobs '\n"));
        assert!(exported.contains("number_threshold = 1\n"));
    }

    #[test]
    fn every_prompt_character_has_a_literal_starship_format() {
        for character in PromptCharacter::ALL {
            let mut settings = PromptSettings::from_preset(PromptPreset::Blank);
            assert!(settings.set_character(character) || character == PromptCharacter::Chevron);
            let exported = settings.to_starship_toml();
            let expected = format!(
                "success_symbol = '[{}](bold green)'",
                character.starship_symbol()
            );
            assert!(exported.contains(&expected), "{character:?}\n{exported}");
            toml::from_str::<toml::Table>(&exported)
                .unwrap_or_else(|error| panic!("{character:?} must be valid TOML: {error}"));
        }
        let mut dollar = PromptSettings::from_preset(PromptPreset::Blank);
        assert!(dollar.set_character(PromptCharacter::Dollar));
        assert!(
            dollar
                .to_starship_toml()
                .contains("success_symbol = '[\\$](bold green)'")
        );
    }

    #[test]
    fn each_catalog_module_exports_a_valid_closed_configuration() {
        for kind in PromptSegmentKind::ALL {
            let mut settings = PromptSettings::from_preset(PromptPreset::Blank);
            assert!(settings.add_module(kind));
            let exported = settings.to_starship_toml();
            assert!(
                exported.contains(&format!("format = '${}$character'", kind.starship_module()))
            );
            assert!(exported.contains(&format!("[{}]", kind.starship_module())));
            assert!(!exported.contains(".bashrc"));
            assert!(!exported.contains(".zshrc"));
            assert!(!exported.contains("starship init"));
            toml::from_str::<toml::Table>(&exported)
                .unwrap_or_else(|error| panic!("{kind:?} produced invalid TOML: {error}"));
        }
    }

    #[test]
    fn removing_a_module_removes_its_format_reference_and_section() {
        let mut settings = PromptSettings::from_preset(PromptPreset::Blank);
        assert!(settings.add_module(PromptSegmentKind::Directory));
        assert!(settings.add_module(PromptSegmentKind::Python));
        assert!(settings.remove_module(PromptSegmentKind::Python));

        let exported = settings.to_starship_toml();
        assert!(exported.contains("format = '$directory$character'"));
        assert!(!exported.contains("$python"));
        assert!(!exported.contains("[python]"));
        toml::from_str::<toml::Table>(&exported).expect("trimmed export must remain valid TOML");
    }

    #[test]
    fn preview_tones_match_their_exported_starship_styles() {
        let expected = [
            (PreviewTone::Cyan, "\x1b[1;36m", "bold cyan"),
            (PreviewTone::Blue, "\x1b[1;34m", "bold blue"),
            (PreviewTone::Magenta, "\x1b[1;35m", "bold purple"),
            (PreviewTone::Yellow, "\x1b[1;33m", "bold yellow"),
            (PreviewTone::Green, "\x1b[1;32m", "bold green"),
            (PreviewTone::Red, "\x1b[1;31m", "bold red"),
        ];
        for (tone, sgr, starship_style) in expected {
            assert_eq!(tone.sgr(), sgr);
            assert_eq!(tone.starship_style(), starship_style);
        }
    }

    #[test]
    fn empty_structure_still_exports_a_working_character_prompt() {
        let settings = PromptSettings::from_preset(PromptPreset::Blank);
        let exported = settings.to_starship_toml();
        assert_eq!(STARSHIP_FILE_NAME, "starship.toml");
        assert!(exported.starts_with("# Generated by TermiMochi."));
        assert!(exported.ends_with('\n'));
        assert!(exported.contains("format = '$character'"));
        assert!(exported.contains("[character]"));
        for kind in PromptSegmentKind::ALL {
            assert!(!exported.contains(&format!("[{}]", kind.starship_module())));
        }
        toml::from_str::<toml::Table>(&exported).expect("blank export must be valid TOML");
    }

    #[test]
    fn layout_and_spacing_have_matching_preview_and_export_semantics() {
        let mut settings = PromptSettings::from_preset(PromptPreset::Blank);
        settings.set_character(PromptCharacter::Ascii);
        settings.add_module(PromptSegmentKind::ExitStatus);
        let context = PromptPreviewContext {
            exit_status: 127,
            ..PromptPreviewContext::default()
        };
        assert_eq!(settings.preview_text(&context), "exit 127 > ");
        assert!(settings.set_layout(PromptLayout::TwoLine));
        assert!(!settings.set_layout(PromptLayout::TwoLine));
        assert!(settings.set_add_newline(true));
        assert!(!settings.set_add_newline(true));
        assert_eq!(settings.preview_text(&context), "\nexit 127 \n> ");
        assert_eq!(settings.preview_ansi(&context).matches("\r\n").count(), 2);
        let exported: toml::Table = toml::from_str(&settings.to_starship_toml()).unwrap();
        assert_eq!(
            exported["format"].as_str(),
            Some("$status$line_break$character")
        );
        assert_eq!(exported["add_newline"].as_bool(), Some(true));
        assert_eq!(settings.source_preset(), None);

        // A blank starting point must not grow an empty information row.
        settings.remove_module(PromptSegmentKind::ExitStatus);
        assert_eq!(settings.preview_text(&context), "\n> ");
        assert!(!settings.to_starship_toml().contains("$line_break"));
    }

    #[test]
    fn remote_visibility_and_root_color_match_starship_behavior() {
        let mut settings = PromptSettings::from_preset(PromptPreset::Remote);
        let local = PromptPreviewContext::default();
        assert!(
            settings
                .preview_parts(&local)
                .iter()
                .all(|part| part.kind != PromptSegmentKind::Hostname)
        );
        let remote_root = PromptPreviewContext {
            username: "root",
            hostname: "build.example.org",
            is_root: true,
            is_ssh: true,
            ..local
        };
        let parts = settings.preview_parts(&remote_root);
        assert_eq!(parts[0].text, "root");
        assert_eq!(parts[0].tone, PreviewTone::Red);
        assert_eq!(parts[1].text, "@build.example.org");
        assert!(settings.set_hostname_mode(PromptHostnameMode::Always));
        assert!(!settings.set_hostname_mode(PromptHostnameMode::Always));
        assert!(
            settings
                .preview_parts(&local)
                .iter()
                .any(|part| part.kind == PromptSegmentKind::Hostname)
        );
        let exported: toml::Table = toml::from_str(&settings.to_starship_toml()).unwrap();
        assert_eq!(exported["hostname"]["ssh_only"].as_bool(), Some(false));
        assert_eq!(exported["hostname"]["trim_at"].as_str(), Some(""));
        assert_eq!(
            exported["username"]["style_root"].as_str(),
            Some("bold red")
        );
    }

    #[test]
    fn directory_preview_preserves_root_home_and_unicode_components() {
        let mut settings = PromptSettings::from_preset(PromptPreset::Blank);
        settings.add_module(PromptSegmentKind::Directory);
        for (path, expected) in [
            ("", "~"),
            ("/", "/"),
            ("~", "~"),
            ("~/a/b", "~/a/b"),
            ("~/a/b/c", ".../a/b/c"),
            ("/a/b/c", "/a/b/c"),
            ("/a/b/c/d", ".../b/c/d"),
            ("/projects/中文/🙂/src", ".../中文/🙂/src"),
            (
                "/projects/space name/e\u{301}/src",
                ".../space name/e\u{301}/src",
            ),
            ("/projects/中文/\u{202e}src/\x1btests", ".../中文/src/tests"),
        ] {
            let context = PromptPreviewContext {
                path,
                ..PromptPreviewContext::default()
            };
            assert_eq!(
                settings.preview_parts(&context)[0].text,
                format!("in {expected}"),
                "{path:?}"
            );
        }
        let long_ancestor = format!("/{}/project/模块/src", "x".repeat(512));
        assert_eq!(preview_directory(&long_ancestor), ".../project/模块/src");
        let exported: toml::Table = toml::from_str(&settings.to_starship_toml()).unwrap();
        assert_eq!(
            exported["directory"]["truncation_length"].as_integer(),
            Some(3)
        );
        assert_eq!(
            exported["directory"]["truncate_to_repo"].as_bool(),
            Some(false)
        );
        assert_eq!(
            exported["directory"]["truncation_symbol"].as_str(),
            Some(".../")
        );
    }

    #[test]
    fn ascii_and_dollar_exports_have_no_hidden_unicode_glyphs() {
        for character in [PromptCharacter::Ascii, PromptCharacter::Dollar] {
            let mut settings = PromptSettings::from_preset(PromptPreset::Blank);
            for kind in PromptSegmentKind::ALL {
                settings.add_module(kind);
            }
            settings.set_character(character);
            let output = settings.to_starship_toml();
            assert!(
                output.is_ascii(),
                "{character:?} contains a non-ASCII fallback"
            );
            let exported: toml::Table = toml::from_str(&output).unwrap();
            assert_eq!(exported["git_status"]["ahead"].as_str(), Some(">${count}"));
            assert_eq!(exported["git_status"]["behind"].as_str(), Some("<${count}"));
            assert_eq!(exported["git_status"]["renamed"].as_str(), Some("r"));
            assert_eq!(exported["git_status"]["deleted"].as_str(), Some("x"));
            for mode in [
                "vimcmd_symbol",
                "vimcmd_replace_one_symbol",
                "vimcmd_replace_symbol",
                "vimcmd_visual_symbol",
            ] {
                assert!(exported["character"][mode].as_str().unwrap().is_ascii());
            }
        }
    }

    #[test]
    fn git_preview_uses_ascii_status_and_grapheme_safe_branch_truncation() {
        let mut settings = PromptSettings::from_preset(PromptPreset::Blank);
        settings.add_module(PromptSegmentKind::GitStatus);
        let context = PromptPreviewContext {
            git_status: Some("+»✘"),
            ..PromptPreviewContext::default()
        };
        assert_eq!(settings.preview_parts(&context)[0].text, "+rx");
        for grapheme in ["x", "中", "e\u{301}", "👩‍👩‍👧‍👦"] {
            let boundary = grapheme.repeat(BRANCH_GRAPHEME_LIMIT);
            assert_eq!(preview_git_branch(&boundary), boundary);
            assert_eq!(
                preview_git_branch(&format!("{boundary}z")),
                format!("{boundary}.")
            );
        }
        assert_eq!(preview_git_branch("ma\u{202e}in\x1b"), "main");
    }

    /// Exercise the real format parser and shell-specific escaping when
    /// Starship is installed. All configuration and cache writes are isolated.
    #[test]
    fn real_starship_matches_runtime_preview_for_bash_zsh_and_fish() {
        use std::{
            fs,
            process::Command,
            time::{SystemTime, UNIX_EPOCH},
        };

        if Command::new("starship").arg("--version").output().is_err() {
            eprintln!("skipping real renderer check: Starship is not installed");
            return;
        }
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let scratch = std::env::temp_dir().join(format!(
            "termimochi-starship-compat-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir(&scratch).unwrap();
        struct Cleanup(std::path::PathBuf);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        let _cleanup = Cleanup(scratch.clone());
        let config = scratch.join("starship.toml");
        let mut settings = PromptSettings::from_preset(PromptPreset::Blank);
        settings.add_module(PromptSegmentKind::CommandDuration);
        settings.add_module(PromptSegmentKind::ExitStatus);
        settings.add_module(PromptSegmentKind::Jobs);

        for character in PromptCharacter::ALL {
            settings.set_character(character);
            for layout in PromptLayout::ALL {
                settings.set_layout(layout);
                for add_newline in [false, true] {
                    settings.set_add_newline(add_newline);
                    fs::write(&config, settings.to_starship_toml()).unwrap();
                    for shell in ["bash", "zsh", "fish"] {
                        for status in [0, 127] {
                            let output = Command::new("starship")
                                .env_clear()
                                .env("PATH", std::env::var_os("PATH").unwrap_or_default())
                                .env("TERM", "xterm-256color")
                                .env("STARSHIP_CONFIG", &config)
                                .env("STARSHIP_CACHE", scratch.join("cache"))
                                .env("STARSHIP_SHELL", shell)
                                .current_dir(&scratch)
                                .args([
                                    "prompt",
                                    "--cmd-duration",
                                    "842",
                                    "--jobs",
                                    "2",
                                    "--status",
                                    &status.to_string(),
                                ])
                                .output()
                                .unwrap();
                            assert!(output.status.success(), "{shell}: {:?}", output.status);
                            assert!(
                                output.stderr.is_empty(),
                                "{shell}: {}",
                                String::from_utf8_lossy(&output.stderr)
                            );
                            let rendered = String::from_utf8(output.stdout).unwrap();
                            let plain = plain_starship_output(&rendered);
                            let context = PromptPreviewContext {
                                command_duration: Some("842ms"),
                                jobs: Some("2"),
                                exit_status: status,
                                ..PromptPreviewContext::default()
                            };
                            assert_eq!(
                                plain,
                                settings.preview_text(&context),
                                "{shell}/{character:?}/{layout:?}/spacing={add_newline}/status={status}"
                            );
                        }
                    }
                }
            }
        }

        // Render physical temporary paths so UTF-8 component boundaries and
        // deep directories are checked against Starship rather than a mock.
        settings.apply_preset(PromptPreset::Blank);
        settings.set_character(PromptCharacter::Ascii);
        settings.add_module(PromptSegmentKind::Directory);
        fs::write(&config, settings.to_starship_toml()).unwrap();
        for suffix in [
            "alpha/beta/gamma/delta",
            "プロジェクト/中文/🙂/src",
            "project/space name/e\u{301}/src",
        ] {
            let directory = scratch.join(suffix);
            fs::create_dir_all(&directory).unwrap();
            let output = Command::new("starship")
                .env_clear()
                .env("PATH", std::env::var_os("PATH").unwrap_or_default())
                .env("TERM", "xterm-256color")
                .env("STARSHIP_CONFIG", &config)
                .env("STARSHIP_CACHE", scratch.join("cache"))
                .env("STARSHIP_SHELL", "fish")
                .current_dir(&directory)
                .arg("prompt")
                .output()
                .unwrap();
            assert!(output.status.success());
            assert!(
                output.stderr.is_empty(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            let path = directory.to_string_lossy();
            let context = PromptPreviewContext {
                path: &path,
                ..PromptPreviewContext::default()
            };
            assert_eq!(
                plain_starship_output(&String::from_utf8(output.stdout).unwrap()),
                settings.preview_text(&context),
                "{suffix}"
            );
        }

        // A clean Git repository must not leave a phantom separator. All Git
        // work is inside this throwaway repository with user config disabled.
        if Command::new("git").arg("--version").output().is_ok() {
            let repo = scratch.join("repo");
            let git_config = scratch.join("gitconfig");
            fs::write(&git_config, "").unwrap();
            let initialized = Command::new("git")
                .env_clear()
                .env("PATH", std::env::var_os("PATH").unwrap_or_default())
                .env("GIT_CONFIG_NOSYSTEM", "1")
                .env("GIT_CONFIG_GLOBAL", &git_config)
                .args(["-c", "init.defaultBranch=main", "init", "--quiet"])
                .arg(&repo)
                .output()
                .unwrap();
            assert!(initialized.status.success());
            settings.apply_preset(PromptPreset::Blank);
            settings.set_character(PromptCharacter::Ascii);
            settings.add_module(PromptSegmentKind::GitStatus);
            fs::write(&config, settings.to_starship_toml()).unwrap();
            for dirty in [false, true] {
                if dirty {
                    fs::write(repo.join("new.txt"), "untracked fixture\n").unwrap();
                }
                let output = Command::new("starship")
                    .env_clear()
                    .env("PATH", std::env::var_os("PATH").unwrap_or_default())
                    .env("TERM", "xterm-256color")
                    .env("GIT_CONFIG_NOSYSTEM", "1")
                    .env("GIT_CONFIG_GLOBAL", &git_config)
                    .env("STARSHIP_CONFIG", &config)
                    .env("STARSHIP_CACHE", scratch.join("cache"))
                    .env("STARSHIP_SHELL", "fish")
                    .current_dir(&repo)
                    .arg("prompt")
                    .output()
                    .unwrap();
                assert!(output.status.success());
                assert!(
                    output.stderr.is_empty(),
                    "{}",
                    String::from_utf8_lossy(&output.stderr)
                );
                let context = PromptPreviewContext {
                    git_status: if dirty { Some("?") } else { None },
                    ..PromptPreviewContext::default()
                };
                assert_eq!(
                    plain_starship_output(&String::from_utf8(output.stdout).unwrap()),
                    settings.preview_text(&context)
                );
            }

            let git = |arguments: &[&str]| {
                let output = Command::new("git")
                    .env_clear()
                    .env("PATH", std::env::var_os("PATH").unwrap_or_default())
                    .env("GIT_CONFIG_NOSYSTEM", "1")
                    .env("GIT_CONFIG_GLOBAL", &git_config)
                    .current_dir(&repo)
                    .args([
                        "-c",
                        "user.name=TermiMochi Tests",
                        "-c",
                        "user.email=tests@example.invalid",
                        "-c",
                        "commit.gpgsign=false",
                        "-c",
                        "core.hooksPath=/dev/null",
                    ])
                    .args(arguments)
                    .output()
                    .unwrap();
                assert!(
                    output.status.success(),
                    "git {arguments:?}: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
            };
            let render_git = |settings: &PromptSettings, context: &PromptPreviewContext<'_>| {
                fs::write(&config, settings.to_starship_toml()).unwrap();
                let output = Command::new("starship")
                    .env_clear()
                    .env("PATH", std::env::var_os("PATH").unwrap_or_default())
                    .env("TERM", "xterm-256color")
                    .env("GIT_CONFIG_NOSYSTEM", "1")
                    .env("GIT_CONFIG_GLOBAL", &git_config)
                    .env("STARSHIP_CONFIG", &config)
                    .env("STARSHIP_CACHE", scratch.join("cache"))
                    .env("STARSHIP_SHELL", "fish")
                    .current_dir(&repo)
                    .arg("prompt")
                    .output()
                    .unwrap();
                assert!(output.status.success());
                assert!(
                    output.stderr.is_empty(),
                    "{}",
                    String::from_utf8_lossy(&output.stderr)
                );
                assert_eq!(
                    plain_starship_output(&String::from_utf8(output.stdout).unwrap()),
                    settings.preview_text(context)
                );
            };
            fs::write(repo.join("delete.txt"), "tracked deletion fixture\n").unwrap();
            git(&["add", "new.txt", "delete.txt"]);
            git(&["commit", "--quiet", "-m", "Tracked fixtures"]);
            git(&["mv", "new.txt", "renamed.txt"]);
            render_git(
                &settings,
                &PromptPreviewContext {
                    git_status: Some("r"),
                    ..PromptPreviewContext::default()
                },
            );
            fs::remove_file(repo.join("delete.txt")).unwrap();
            render_git(
                &settings,
                &PromptPreviewContext {
                    git_status: Some("xr"),
                    ..PromptPreviewContext::default()
                },
            );

            settings.remove_module(PromptSegmentKind::GitStatus);
            settings.add_module(PromptSegmentKind::GitBranch);
            for branch in [
                "a".repeat(24),
                "a".repeat(25),
                "中".repeat(25),
                "e\u{301}".repeat(25),
                format!("{}{}z", "a".repeat(16), "👩‍👩‍👧‍👦".repeat(8)),
            ] {
                git(&["branch", "-m", &branch]);
                render_git(
                    &settings,
                    &PromptPreviewContext {
                        git_branch: Some(&branch),
                        ..PromptPreviewContext::default()
                    },
                );
            }
        }
    }

    fn plain_starship_output(output: &str) -> String {
        let unwrapped = output
            .replace("\\[", "")
            .replace("\\]", "")
            .replace("%{", "")
            .replace("%}", "")
            .replace("\\$", "$");
        let mut chars = unwrapped.chars();
        let mut plain = String::new();
        while let Some(character) = chars.next() {
            if character == '\x1b' && chars.next() == Some('[') {
                for code in chars.by_ref() {
                    if ('@'..='~').contains(&code) {
                        break;
                    }
                }
            } else {
                plain.push(character);
            }
        }
        plain
    }
}
