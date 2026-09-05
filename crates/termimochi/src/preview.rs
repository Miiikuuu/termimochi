#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PreviewScenario {
    Shell,
    Codex,
    GitStatus,
    GitDiff,
    Tests,
    Code,
    Htop,
    Typography,
    CurrentFolder,
}

impl PreviewScenario {
    pub(crate) const ALL: [Self; 9] = [
        Self::Shell,
        Self::Codex,
        Self::GitStatus,
        Self::GitDiff,
        Self::Tests,
        Self::Code,
        Self::Htop,
        Self::Typography,
        Self::CurrentFolder,
    ];

    pub(crate) fn from_index(index: u32) -> Self {
        usize::try_from(index)
            .ok()
            .and_then(|index| Self::ALL.get(index))
            .copied()
            .unwrap_or(Self::Shell)
    }

    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Shell => "Shell",
            Self::Codex => "Codex",
            Self::GitStatus => "Git Status",
            Self::GitDiff => "Git Diff",
            Self::Tests => "Tests",
            Self::Code => "Code",
            Self::Htop => "htop",
            Self::Typography => "Typography",
            Self::CurrentFolder => "Current Folder",
        }
    }

    pub(crate) const fn terminal_title(self) -> &'static str {
        match self {
            Self::Shell => "bash",
            Self::Codex => "codex",
            Self::GitStatus => "git status",
            Self::GitDiff => "git diff",
            Self::Tests => "test matrix",
            Self::Code => "syntax sampler",
            Self::Htop => "htop",
            Self::Typography => "type specimen",
            Self::CurrentFolder => "current folder",
        }
    }

    /// A deterministic terminal transcript. VTE, rather than GTK labels,
    /// interprets these SGR sequences using the palette currently under edit.
    pub(crate) const fn transcript(self) -> &'static str {
        match self {
            Self::Shell => SHELL,
            Self::Codex => CODEX,
            Self::GitStatus => GIT_STATUS,
            Self::GitDiff => GIT_DIFF,
            Self::Tests => TESTS,
            Self::Code => CODE,
            Self::Htop => HTOP,
            Self::Typography => TYPOGRAPHY,
            Self::CurrentFolder => "\x1b[?25lReading current folder…\r\n\x1b[0m",
        }
    }

    /// The ordered chunks used for every VTE refresh. Keeping the clear and
    /// replacement transcript together makes scenario transitions testable
    /// without requiring a display server.
    pub(crate) fn refresh_chunks(self) -> [&'static [u8]; 2] {
        [PREVIEW_HOME_AND_CLEAR, self.transcript().as_bytes()]
    }
}

pub(crate) const PREVIEW_COLUMNS: usize = 58;
pub(crate) const PREVIEW_ROWS: usize = 11;
pub(crate) const PREVIEW_HOME_AND_CLEAR: &[u8] = b"\x1b[3J\x1b[H\x1b[2J";
pub(crate) const PREVIEW_SHOW_CURSOR: &[u8] = b"\x1b[?25h";
pub(crate) const PREVIEW_INPUT_PROMPT: &str = "\r\x1b[2K\x1b[36m›\x1b[0m ";
const PREVIEW_INPUT_LIMIT: usize = 256;
const PREVIEW_INPUT_HISTORY_LIMIT: usize = 4;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PreviewInputEvent<'a> {
    Text(&'a str),
    Backspace,
    Submit,
    Reset,
    FocusForward,
    FocusBackward,
    Ignore,
}

impl<'a> PreviewInputEvent<'a> {
    pub(crate) fn from_commit(committed: &'a str) -> Self {
        match committed {
            "\x08" | "\x7f" => Self::Backspace,
            "\r" | "\n" => Self::Submit,
            "\x1b" => Self::Reset,
            "\t" => Self::FocusForward,
            "\x1b[Z" => Self::FocusBackward,
            _ if committed.contains('\x1b') || committed.contains('\u{009b}') => Self::Ignore,
            _ => Self::Text(committed),
        }
    }
}

/// Local, non-executing input for the VTE specimen. Keeping user text outside
/// the palette model means preview interaction can never dirty the document or
/// enter its Undo history.
#[derive(Debug, Default, Eq, PartialEq)]
pub(crate) struct PreviewInput {
    active: bool,
    submitted: Vec<String>,
    text: String,
}

impl PreviewInput {
    pub(crate) fn is_active(&self) -> bool {
        self.active
    }

    pub(crate) fn text(&self) -> &str {
        &self.text
    }

    pub(crate) fn submitted(&self) -> &[String] {
        &self.submitted
    }

    /// Appends printable text committed by VTE's input method. Escape
    /// introducers reject the complete commit so arrow/function-key sequences
    /// cannot leave fragments such as `[D` in the scratch line. Other control
    /// characters are ignored, making multi-line clipboard input harmless.
    pub(crate) fn commit(&mut self, committed: &str) -> bool {
        if committed.contains('\x1b') || committed.contains('\u{009b}') {
            return false;
        }

        let remaining = PREVIEW_INPUT_LIMIT.saturating_sub(self.text.chars().count());
        if remaining == 0 {
            return false;
        }

        let accepted: String = committed
            .chars()
            .filter(|character| !character.is_control())
            .take(remaining)
            .collect();
        if accepted.is_empty() {
            return false;
        }

        self.active = true;
        self.text.push_str(&accepted);
        true
    }

    pub(crate) fn backspace(&mut self) -> bool {
        pop_last_visual_character(&mut self.text)
    }

    pub(crate) fn submit(&mut self) {
        self.active = true;
        if self.text.is_empty() {
            return;
        }
        if self.submitted.len() == PREVIEW_INPUT_HISTORY_LIMIT {
            self.submitted.remove(0);
        }
        self.submitted.push(std::mem::take(&mut self.text));
    }

    pub(crate) fn reset(&mut self) {
        self.active = false;
        self.submitted.clear();
        self.text.clear();
    }
}

fn pop_last_visual_character(text: &mut String) -> bool {
    if text.is_empty() {
        return false;
    }

    while let Some(character) = text.pop() {
        if is_grapheme_extension(character) {
            continue;
        }

        // Regional indicators are rendered in pairs as one flag.
        if is_regional_indicator(character)
            && text
                .chars()
                .rev()
                .take_while(|character| is_regional_indicator(*character))
                .count()
                % 2
                == 1
        {
            text.pop();
        }

        // Consume the preceding member of a joined Emoji sequence, including
        // any variation selector or skin-tone modifier attached to it.
        if text.ends_with('\u{200d}') {
            text.pop();
            continue;
        }
        break;
    }
    true
}

fn is_grapheme_extension(character: char) -> bool {
    matches!(
        character as u32,
        0x0300..=0x036f
            | 0x1ab0..=0x1aff
            | 0x1dc0..=0x1dff
            | 0x20d0..=0x20ff
            | 0xfe00..=0xfe0f
            | 0x1f3fb..=0x1f3ff
            | 0xe0020..=0xe007f
    )
}

fn is_regional_indicator(character: char) -> bool {
    matches!(character as u32, 0x1f1e6..=0x1f1ff)
}

const SHELL: &str = concat!(
    "\x1b[?25l",
    "\x1b[1;32mmochi@linux\x1b[0m:\x1b[1;34m~/TermiMochi\x1b[0m$ cargo test\r\n",
    "\x1b[33m   Compiling\x1b[0m termimochi-core v0.1.0\r\n",
    "\x1b[36m    Finished\x1b[0m test profile in 0.62s\r\n",
    "\x1b[32mtest result: ok.\x1b[0m all checks passed; 0 failed\r\n",
    "normal  \x1b[30m██\x1b[31m██\x1b[32m██\x1b[33m██\x1b[34m██\x1b[35m██\x1b[36m██\x1b[37m██\x1b[0m\r\n",
    "bright  \x1b[90m██\x1b[91m██\x1b[92m██\x1b[93m██\x1b[94m██\x1b[95m██\x1b[96m██\x1b[97m██\x1b[0m\r\n",
    "styles  \x1b[1mbold\x1b[0m \x1b[2mdim\x1b[0m \x1b[3mitalic\x1b[0m \x1b[4munderline\x1b[0m \x1b[7mreverse\x1b[0m\r\n",
    "English · UTF-8 · arrows ← ↑ ↓ → · (｡•̀ᴗ-)✧\x1b[0m"
);

const CODEX: &str = concat!(
    "\x1b[?25l",
    "\x1b[1mCodex\x1b[0m · \x1b[34m~/Projects/TermiMochi\x1b[0m · main\r\n",
    "\x1b[90mSession ready · palette review\x1b[0m\r\n",
    "\r\n",
    "\x1b[36m•\x1b[0m Reading preview.rs and style.rs\r\n",
    "\x1b[32m✓\x1b[0m Read preview.rs · style.rs\r\n",
    "\x1b[34m→\x1b[0m Ran cargo test --workspace\r\n",
    "\x1b[32m✓\x1b[0m Workspace tests passed · 0 failed\r\n",
    "\r\n",
    "\x1b[39;40m╭─ Composer ──────────────────────────────────────────╮\x1b[0m\r\n",
    "\x1b[39;40m│ › Review this palette for contrast…                 │\x1b[0m\r\n",
    "\x1b[39;40m╰ Enter · Shift+Enter newline · Esc stop ─────────────╯\x1b[0m"
);

const GIT_STATUS: &str = concat!(
    "\x1b[?25l",
    "\x1b[1;32mmochi@linux\x1b[0m:\x1b[1;34m~/TermiMochi\x1b[0m$ git status --short\r\n",
    "\x1b[32mM \x1b[0m README.md\r\n",
    "\x1b[33m M\x1b[0m crates/termimochi/src/window.rs\r\n",
    "\x1b[31m D\x1b[0m old-preview.rs\r\n",
    "\x1b[31m??\x1b[0m crates/termimochi/src/preview.rs\r\n\r\n",
    "\x1b[1mOn branch main\x1b[0m\r\n",
    "Your branch is ahead by \x1b[36m2 commits\x1b[0m.\r\n",
    "\x1b[90mnothing has been written by this preview\x1b[0m"
);

const GIT_DIFF: &str = concat!(
    "\x1b[?25l",
    "\x1b[1;32mmochi@linux\x1b[0m:\x1b[1;34m~/TermiMochi\x1b[0m$ git diff -- src/preview.rs\r\n",
    "\x1b[1mdiff --git a/src/preview.rs b/src/preview.rs\x1b[0m\r\n",
    "\x1b[1mindex 21bf62a..915aa31 100644\x1b[0m\r\n",
    "\x1b[1m--- a/src/preview.rs\x1b[0m\r\n",
    "\x1b[1m+++ b/src/preview.rs\x1b[0m\r\n",
    "\x1b[36m@@ -59,3 +59,4 @@ match self {\x1b[0m\r\n",
    "             Self::Git => GIT,\r\n",
    "\x1b[31m-            Self::Tests => TESTS,\x1b[0m\r\n",
    "\x1b[32m+            Self::GitDiff => GIT_DIFF,\x1b[41m  \x1b[0m\r\n",
    "\x1b[32m+            Self::Tests => TESTS, // keep selector order.\x1b[0m\r\n",
    "         }\x1b[0m"
);

const HTOP: &str = concat!(
    "\x1b[?25l",
    "\x1b[30;46m  1  [|||||||||||||                 42.0%] \x1b[0m Tasks: \x1b[32m128\x1b[0m\r\n",
    "\x1b[30;42m Mem[|||||||||||||||||          5.8G/16G] \x1b[0m Load: 0.42\r\n",
    "\x1b[30;43m Swp[|                         120M/8G] \x1b[0m Uptime: 3h21m\r\n\r\n",
    "\x1b[30;47m  PID USER       CPU%  MEM%   TIME+  Command              \x1b[0m\r\n",
    "\x1b[30;46m 7842 mochi      12.4   3.2   1:42  termimochi           \x1b[0m\r\n",
    " 1851 mochi       \x1b[31m7.1\x1b[0m   1.8   8:04  ptyxis\r\n",
    " 2016 mochi       \x1b[33m2.3\x1b[0m   0.9   2:18  codex\r\n\r\n",
    "\x1b[30;42m F1 Help \x1b[30;46m F2 Setup \x1b[30;44m F3 Search \x1b[30;41m F9 Kill \x1b[0m"
);

const TESTS: &str = concat!(
    "\x1b[?25l",
    "\x1b[1mTest / build status sampler\x1b[0m\r\n",
    "\x1b[34m$\x1b[0m cargo test --workspace\r\n",
    "\x1b[32mPASS\x1b[0m preview::all_ansi_slots\r\n",
    "\x1b[32mPASS\x1b[0m history::undo_redo\r\n",
    "\x1b[36mSKIP\x1b[0m export::remote_profile\r\n",
    "result: \x1b[32m2 passed\x1b[0m; 0 failed; \x1b[36m1 ignored\x1b[0m\r\n",
    "\r\n",
    "\x1b[34m$\x1b[0m pytest -q && npm test\r\n",
    "\x1b[32mPASS 18 passed\x1b[0m  \x1b[33mWARN 1 warning\x1b[0m\r\n",
    "sample: \x1b[31mFAIL\x1b[0m \x1b[91mERROR\x1b[0m \x1b[34mINFO\x1b[0m \x1b[33mWARN\x1b[0m\r\n",
    "\x1b[35mTODO FIXME\x1b[0m  \x1b[90mDEBUG TRACE\x1b[0m"
);

const CODE: &str = concat!(
    "\x1b[?25l",
    "\x1b[1mSyntax sampler · semantic color roles\x1b[0m\r\n",
    "\x1b[35mpub async fn\x1b[0m \x1b[34mrender\x1b[0m(t: &\x1b[36mTheme\x1b[0m) -> \x1b[36mResult<()>\x1b[0m {\r\n",
    "  \x1b[35mlet\x1b[0m ok = \x1b[33mtrue\x1b[0m; \x1b[90m// Rust keyword, bool, comment\x1b[0m\r\n",
    "\x1b[35mfrom\x1b[0m pathlib \x1b[35mimport\x1b[0m \x1b[36mPath\x1b[0m\r\n",
    "\x1b[35mdef\x1b[0m \x1b[34mload\x1b[0m(name: \x1b[36mstr\x1b[0m) -> \x1b[36mdict\x1b[0m | \x1b[33mNone\x1b[0m:\r\n",
    "\x1b[35mexport const\x1b[0m theme: \x1b[36mTheme\x1b[0m = \x1b[35mawait\x1b[0m \x1b[34mload\x1b[0m();\r\n",
    "\x1b[35mif\x1b[0m (!theme) \x1b[35mthrow new\x1b[0m \x1b[36mError\x1b[0m(\x1b[32m\"missing\"\x1b[0m);\r\n",
    "\x1b[35mfor\x1b[0m f \x1b[35min\x1b[0m *.palette; \x1b[35mdo\x1b[0m lint \x1b[32m\"$f\"\x1b[0m; \x1b[35mdone\x1b[0m\r\n",
    "{\x1b[34m\"light\"\x1b[0m: \x1b[33mtrue\x1b[0m, \x1b[34m\"colors\"\x1b[0m: \x1b[33m16\x1b[0m, \x1b[34m\"name\"\x1b[0m: \x1b[31mnull\x1b[0m}\r\n",
    "\x1b[35mSELECT\x1b[0m name \x1b[35mFROM\x1b[0m themes \x1b[35mWHERE\x1b[0m contrast >= \x1b[33m4.5\x1b[0m;\r\n",
    "\x1b[90m# Rust · Python · TypeScript · shell · JSON · SQL\x1b[0m"
);

const TYPOGRAPHY: &str = concat!(
    "\x1b[?25l",
    "\x1b[1mTypography · alignment and fallback\x1b[0m\r\n",
    "\x1b[90m            │123456789012345678│\x1b[0m\r\n",
    "────────────┼──────────────────┼────────\r\n",
    "English     │TermiMochi 123    │Latin\r\n",
    "简体中文    │终端麻薯          │CJK\r\n",
    "日本語      │かな カナ 漢字    │CJK\r\n",
    "Emoji       │🌸 🍡 🐱          │fallback\r\n",
    "Kaomoji     │(｡•̀ᴗ-)✧           │width\r\n",
    "Nerd Font   │             │5 probes\r\n",
    "Box / block │╭─╮ ├─┤ ╰─╯ ░▒▓█  │drawing\r\n",
    "\x1b[90mBoundaries should stay vertically aligned.\x1b[0m"
);

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;
    use crate::typography::NERD_FONT_PROBES;

    fn visible_text(transcript: &str) -> String {
        let bytes = transcript.as_bytes();
        let mut visible = String::with_capacity(transcript.len());
        let mut index = 0;
        let mut sgr_active = false;

        while index < bytes.len() {
            if bytes[index] == 0x1b {
                assert_eq!(
                    bytes.get(index + 1),
                    Some(&b'['),
                    "only CSI controls are allowed"
                );
                index += 2;
                if bytes
                    .get(index..index + 4)
                    .is_some_and(|sequence| sequence == b"?25l")
                {
                    index += 4;
                    continue;
                }

                let parameter_start = index;
                while bytes
                    .get(index)
                    .is_some_and(|byte| byte.is_ascii_digit() || *byte == b';')
                {
                    index += 1;
                }
                assert!(index > parameter_start, "SGR parameters cannot be empty");
                assert_eq!(
                    bytes.get(index),
                    Some(&b'm'),
                    "only SGR styling is allowed after the cursor is hidden"
                );
                let parameters = std::str::from_utf8(&bytes[parameter_start..index])
                    .expect("SGR parameters are ASCII");
                for parameter in parameters.split(';') {
                    let parameter: u8 = parameter.parse().expect("SGR parameters are numeric");
                    assert!(
                        matches!(
                            parameter,
                            0 | 1 | 2 | 3 | 4 | 7 | 30..=37 | 39 | 40..=47 | 90..=97
                        ),
                        "SGR {parameter} bypasses the editable ANSI palette"
                    );
                }
                sgr_active = parameters != "0";
                index += 1;
                continue;
            }

            let character = transcript[index..]
                .chars()
                .next()
                .expect("index stays on a UTF-8 boundary");
            if character == '\r' {
                assert!(!sgr_active, "SGR styling leaks across a preview row");
            }
            visible.push(character);
            index += character.len_utf8();
        }

        assert!(
            !sgr_active,
            "preview transcript ends with active SGR styling"
        );

        visible
    }

    fn assert_safe_controls(transcript: &str) {
        let bytes = transcript.as_bytes();
        for (index, byte) in bytes.iter().copied().enumerate() {
            if byte >= b' ' && byte != 0x7f {
                continue;
            }
            match byte {
                0x1b => {}
                b'\r' => assert_eq!(bytes.get(index + 1), Some(&b'\n'), "bare CR"),
                b'\n' => assert_eq!(
                    index.checked_sub(1).and_then(|i| bytes.get(i)),
                    Some(&b'\r'),
                    "bare LF"
                ),
                _ => panic!("unsafe control byte 0x{byte:02x}"),
            }
        }
        let _ = visible_text(transcript);
    }

    fn specimen_cell_width(text: &str) -> usize {
        text.chars()
            .map(|character| match character as u32 {
                0x0300..=0x036f | 0x200d | 0xfe00..=0xfe0f => 0,
                0x1100..=0x115f
                | 0x2329..=0x232a
                | 0x2e80..=0xa4cf
                | 0xac00..=0xd7a3
                | 0xf900..=0xfaff
                | 0xfe10..=0xfe19
                | 0xfe30..=0xfe6f
                | 0xff01..=0xff60
                | 0xffe0..=0xffe6
                | 0x1f300..=0x1faff
                | 0x20000..=0x3fffd => 2,
                _ => 1,
            })
            .sum()
    }

    #[test]
    fn every_scenario_has_a_label_title_and_deterministic_transcript() {
        let mut labels = HashSet::new();
        let mut titles = HashSet::new();
        for (index, scenario) in PreviewScenario::ALL.into_iter().enumerate() {
            assert_eq!(PreviewScenario::from_index(index as u32), scenario);
            assert!(labels.insert(scenario.label()), "duplicate scenario label");
            assert!(
                titles.insert(scenario.terminal_title()),
                "duplicate terminal title"
            );
            let transcript = scenario.transcript();
            assert!(transcript.starts_with("\x1b[?25l"));
            assert_eq!(transcript.matches("\x1b[?25l").count(), 1);
            assert!(transcript.ends_with("\x1b[0m"));
            assert_safe_controls(transcript);
            let visible = visible_text(transcript);
            let rows: Vec<_> = visible.split("\r\n").collect();
            assert!(
                rows.len() <= PREVIEW_ROWS,
                "{} exceeds the preview's {PREVIEW_ROWS} rows",
                scenario.label()
            );
            for line in rows {
                assert!(
                    line.chars().count() <= PREVIEW_COLUMNS,
                    "{} line exceeds {PREVIEW_COLUMNS} columns: {line:?}",
                    scenario.label()
                );
            }
        }
        assert_eq!(
            PreviewScenario::from_index(u32::MAX),
            PreviewScenario::Shell
        );
    }

    #[test]
    fn dense_scenarios_fill_the_grid_without_wrapping() {
        assert_eq!(PREVIEW_HOME_AND_CLEAR, b"\x1b[3J\x1b[H\x1b[2J");
        assert_eq!(PREVIEW_SHOW_CURSOR, b"\x1b[?25h");
        for scenario in [
            PreviewScenario::Codex,
            PreviewScenario::GitDiff,
            PreviewScenario::Tests,
            PreviewScenario::Code,
        ] {
            assert_eq!(
                visible_text(scenario.transcript()).split("\r\n").count(),
                PREVIEW_ROWS,
                "{} should fill the preview without scrolling",
                scenario.label()
            );
        }
    }

    #[test]
    fn selector_order_exposes_every_preview_scenario() {
        assert_eq!(
            PreviewScenario::ALL.map(PreviewScenario::label),
            [
                "Shell",
                "Codex",
                "Git Status",
                "Git Diff",
                "Tests",
                "Code",
                "htop",
                "Typography",
                "Current Folder",
            ]
        );
        assert_eq!(
            PreviewScenario::ALL.map(PreviewScenario::terminal_title),
            [
                "bash",
                "codex",
                "git status",
                "git diff",
                "test matrix",
                "syntax sampler",
                "htop",
                "type specimen",
                "current folder",
            ]
        );
    }

    #[test]
    fn every_scenario_transition_clears_the_previous_terminal_contents() {
        fn apply_refresh(screen: &mut String, scenario: PreviewScenario) {
            for chunk in scenario.refresh_chunks() {
                if chunk == PREVIEW_HOME_AND_CLEAR {
                    screen.clear();
                } else {
                    let transcript = std::str::from_utf8(chunk).expect("preview chunks are UTF-8");
                    screen.push_str(&visible_text(transcript));
                }
            }
        }

        for previous in PreviewScenario::ALL {
            for next in PreviewScenario::ALL {
                let mut screen = String::new();
                apply_refresh(&mut screen, previous);
                assert_eq!(screen, visible_text(previous.transcript()));
                apply_refresh(&mut screen, next);
                assert_eq!(
                    screen,
                    visible_text(next.transcript()),
                    "{} -> {} left stale preview content",
                    previous.label(),
                    next.label()
                );
            }
        }
    }

    #[test]
    fn codex_metadata_copy_glyphs_and_sgr_roles_are_stable() {
        let scenario = PreviewScenario::Codex;
        assert_eq!(scenario.label(), "Codex");
        assert_eq!(scenario.terminal_title(), "codex");

        let transcript = scenario.transcript();
        let visible = visible_text(transcript);
        for anchor in [
            "Codex",
            "~/Projects/TermiMochi",
            "Session ready",
            "Reading preview.rs and style.rs",
            "Read preview.rs · style.rs",
            "Ran cargo test --workspace",
            "Workspace tests passed",
            "0 failed",
            "Composer",
            "Review this palette for contrast…",
            "Enter",
            "Shift+Enter",
            "Esc",
        ] {
            assert!(visible.contains(anchor), "missing Codex anchor: {anchor}");
        }
        for glyph in ['╭', '╮', '╰', '╯', '•', '✓', '→', '›', '…', '·'] {
            assert!(visible.contains(glyph), "missing Codex glyph: {glyph}");
        }
        for sgr in [
            "\x1b[1m",
            "\x1b[32m",
            "\x1b[34m",
            "\x1b[36m",
            "\x1b[90m",
            "\x1b[39;40m",
        ] {
            assert!(transcript.contains(sgr), "missing Codex SGR: {sgr:?}");
        }
        for line in visible.split("\r\n") {
            assert!(
                line.chars().count() <= PREVIEW_COLUMNS,
                "Codex line exceeds {PREVIEW_COLUMNS} columns: {line:?}"
            );
        }
    }

    #[test]
    fn codex_composer_keeps_all_three_rows_on_color_zero() {
        let transcript = PreviewScenario::Codex.transcript();
        let rows: Vec<_> = transcript.split("\r\n").collect();
        assert_eq!(rows.len(), PREVIEW_ROWS);
        for row in &rows[rows.len() - 3..] {
            let body = row
                .strip_prefix("\x1b[39;40m")
                .and_then(|row| row.strip_suffix("\x1b[0m"))
                .expect("the complete composer row must use Foreground on Color0");
            assert!(
                !body.contains('\x1b'),
                "composer row resets before its edge"
            );
            assert_eq!(body.chars().count(), 55, "composer edges must align");
        }
        assert_eq!(transcript.matches("\x1b[39;40m").count(), 3);
    }

    #[test]
    fn git_status_keeps_porcelain_states_and_branch_summary() {
        let scenario = PreviewScenario::GitStatus;
        assert_eq!(scenario.label(), "Git Status");
        assert_eq!(scenario.terminal_title(), "git status");

        let transcript = scenario.transcript();
        let visible = visible_text(transcript);
        for anchor in [
            "git status --short",
            "M  README.md",
            " M crates/termimochi/src/window.rs",
            " D old-preview.rs",
            "?? crates/termimochi/src/preview.rs",
            "On branch main",
            "ahead by 2 commits",
            "nothing has been written by this preview",
        ] {
            assert!(
                visible.contains(anchor),
                "missing Git status anchor: {anchor}"
            );
        }
        for sgr in ["\x1b[32mM ", "\x1b[33m M", "\x1b[31m D", "\x1b[31m??"] {
            assert!(transcript.contains(sgr), "missing Git status SGR: {sgr:?}");
        }
    }

    #[test]
    fn git_diff_keeps_patch_structure_colors_and_column_boundary() {
        let scenario = PreviewScenario::GitDiff;
        assert_eq!(scenario.label(), "Git Diff");
        assert_eq!(scenario.terminal_title(), "git diff");

        let transcript = scenario.transcript();
        let visible = visible_text(transcript);
        let rows: Vec<_> = visible.split("\r\n").collect();
        assert_eq!(rows.len(), PREVIEW_ROWS);
        assert_eq!(
            rows.iter()
                .map(|row| row.chars().count())
                .collect::<Vec<_>>(),
            [52, 44, 29, 20, 20, 30, 30, 34, 41, 58, 10]
        );

        for anchor in [
            "git diff -- src/preview.rs",
            "diff --git a/src/preview.rs b/src/preview.rs",
            "index 21bf62a..915aa31 100644",
            "--- a/src/preview.rs",
            "+++ b/src/preview.rs",
            "@@ -59,3 +59,4 @@",
            "-            Self::Tests => TESTS,",
            "+            Self::GitDiff => GIT_DIFF,",
            "keep selector order.",
        ] {
            assert!(
                visible.contains(anchor),
                "missing Git diff anchor: {anchor}"
            );
        }

        let raw_rows: Vec<_> = transcript.split("\r\n").collect();
        assert!(raw_rows[1].starts_with("\x1b[1mdiff --git "));
        assert!(raw_rows[3].starts_with("\x1b[1m--- "));
        assert!(raw_rows[4].starts_with("\x1b[1m+++ "));
        assert!(raw_rows[5].starts_with("\x1b[36m@@ "));
        assert!(!raw_rows[6].contains('\x1b'));
        assert!(raw_rows[7].starts_with("\x1b[31m-"));
        assert!(raw_rows[8].starts_with("\x1b[32m+"));
        assert!(raw_rows[8].contains("\x1b[41m  \x1b[0m"));
        assert!(raw_rows[9].starts_with("\x1b[32m+"));
        assert!(!transcript.contains("\x1b[38;"));
        assert!(!transcript.contains("\x1b[48;"));
    }

    #[test]
    fn code_sampler_keeps_languages_keywords_and_color_roles() {
        let transcript = PreviewScenario::Code.transcript();
        for language in ["Rust", "Python", "TypeScript", "shell", "JSON", "SQL"] {
            assert!(
                transcript.contains(language),
                "missing language: {language}"
            );
        }
        for keyword in [
            "pub async fn",
            "let",
            "from",
            "import",
            "def",
            "export const",
            "await",
            "throw new",
            "for",
            "do",
            "done",
            "true",
            "null",
            "SELECT",
            "FROM",
            "WHERE",
        ] {
            assert!(transcript.contains(keyword), "missing keyword: {keyword}");
        }
        for sgr in [31, 32, 33, 34, 35, 36, 90] {
            assert!(
                transcript.contains(&format!("\x1b[{sgr}m")),
                "missing syntax SGR {sgr}"
            );
        }
    }

    #[test]
    fn test_sampler_keeps_common_status_vocabulary() {
        let transcript = PreviewScenario::Tests.transcript();
        for command in ["cargo test --workspace", "pytest -q", "npm test"] {
            assert!(
                transcript.contains(command),
                "missing test command: {command}"
            );
        }
        for status in [
            "PASS", "WARN", "FAIL", "ERROR", "SKIP", "TODO", "FIXME", "INFO", "DEBUG", "TRACE",
            "passed", "failed", "ignored",
        ] {
            assert!(transcript.contains(status), "missing test status: {status}");
        }
    }

    #[test]
    fn shell_exercises_all_ansi_slots_and_common_text_styles() {
        let transcript = PreviewScenario::Shell.transcript();
        for code in 30..=37 {
            assert!(transcript.contains(&format!("\x1b[{code}m")));
        }
        for code in 90..=97 {
            assert!(transcript.contains(&format!("\x1b[{code}m")));
        }
        for style in [1, 2, 3, 4, 7] {
            assert!(transcript.contains(&format!("\x1b[{style}m")));
        }
    }

    #[test]
    fn typography_specimen_covers_scripts_icons_and_aligned_cells() {
        let scenario = PreviewScenario::Typography;
        assert_eq!(scenario.label(), "Typography");
        assert_eq!(scenario.terminal_title(), "type specimen");

        let visible = visible_text(scenario.transcript());
        for anchor in [
            "English",
            "简体中文",
            "日本語",
            "Emoji",
            "Kaomoji",
            "Nerd Font",
            "Box / block",
        ] {
            assert!(visible.contains(anchor), "missing type sample: {anchor}");
        }
        for glyph in ['🌸', '🍡', '🐱', '｡', 'ᴗ'] {
            assert!(visible.contains(glyph), "missing specimen glyph: {glyph}");
        }
        for glyph in NERD_FONT_PROBES {
            assert!(
                visible.contains(glyph),
                "Nerd Font probe is missing from the specimen: {glyph}"
            );
        }

        let rows: Vec<_> = visible.split("\r\n").collect();
        assert_eq!(rows.len(), PREVIEW_ROWS);
        for row in &rows[3..10] {
            let columns: Vec<_> = row.split('│').collect();
            assert_eq!(columns.len(), 3, "specimen row lost its guides: {row:?}");
            assert_eq!(specimen_cell_width(columns[0]), 12, "left guide drifted");
            assert_eq!(specimen_cell_width(columns[1]), 18, "right guide drifted");
        }
    }

    #[test]
    fn preview_input_accepts_printable_unicode_and_filters_controls() {
        let mut input = PreviewInput::default();
        assert!(!input.is_active());
        assert!(input.commit("hello 终端 🌸"));
        assert!(input.commit("\nnext\tline\u{7f}"));
        assert!(input.is_active());
        assert_eq!(input.text(), "hello 终端 🌸nextline");
    }

    #[test]
    fn vte_commits_map_to_local_controls_without_interpreting_ansi() {
        assert_eq!(
            PreviewInputEvent::from_commit("hello 终端"),
            PreviewInputEvent::Text("hello 终端")
        );
        assert_eq!(
            PreviewInputEvent::from_commit("\x7f"),
            PreviewInputEvent::Backspace
        );
        assert_eq!(
            PreviewInputEvent::from_commit("\r"),
            PreviewInputEvent::Submit
        );
        assert_eq!(
            PreviewInputEvent::from_commit("\x1b"),
            PreviewInputEvent::Reset
        );
        assert_eq!(
            PreviewInputEvent::from_commit("\t"),
            PreviewInputEvent::FocusForward
        );
        assert_eq!(
            PreviewInputEvent::from_commit("\x1b[Z"),
            PreviewInputEvent::FocusBackward
        );
        assert_eq!(
            PreviewInputEvent::from_commit("\x1b[D"),
            PreviewInputEvent::Ignore
        );
        assert_eq!(
            PreviewInputEvent::from_commit("\u{009b}31m"),
            PreviewInputEvent::Ignore
        );
    }

    #[test]
    fn preview_input_rejects_escape_sequences_without_visible_fragments() {
        let mut input = PreviewInput::default();
        assert!(input.commit("safe"));
        assert!(!input.commit("\x1b[D"));
        assert!(!input.commit("\u{009b}31munsafe"));
        assert_eq!(input.text(), "safe");
    }

    #[test]
    fn preview_input_is_bounded_and_resettable() {
        let mut input = PreviewInput::default();
        assert!(input.commit(&"a".repeat(PREVIEW_INPUT_LIMIT + 32)));
        assert_eq!(input.text().chars().count(), PREVIEW_INPUT_LIMIT);
        assert!(!input.commit("more"));

        input.submit();
        assert!(input.is_active());
        assert!(input.text().is_empty());
        assert_eq!(input.submitted().len(), 1);
        input.reset();
        assert!(!input.is_active());
        assert!(input.text().is_empty());
        assert!(input.submitted().is_empty());
    }

    #[test]
    fn preview_input_keeps_only_recent_local_history() {
        let mut input = PreviewInput::default();
        for command in ["one", "two", "three", "four", "five"] {
            assert!(input.commit(command));
            input.submit();
        }
        assert_eq!(input.submitted(), ["two", "three", "four", "five"]);
        assert!(input.text().is_empty());
    }

    #[test]
    fn preview_backspace_removes_complete_visible_unicode_units() {
        let mut input = PreviewInput::default();
        assert!(!input.backspace());

        assert!(input.commit("A简e\u{301}👍🏽👩\u{200d}💻🇨🇳"));
        assert!(input.backspace());
        assert_eq!(input.text(), "A简e\u{301}👍🏽👩\u{200d}💻");
        assert!(input.backspace());
        assert_eq!(input.text(), "A简e\u{301}👍🏽");
        assert!(input.backspace());
        assert_eq!(input.text(), "A简e\u{301}");
        assert!(input.backspace());
        assert_eq!(input.text(), "A简");
        assert!(input.backspace());
        assert_eq!(input.text(), "A");
    }

    #[test]
    fn interactive_prompt_uses_the_editable_ansi_cyan_and_clears_its_line() {
        assert!(PREVIEW_INPUT_PROMPT.starts_with("\r\x1b[2K"));
        assert!(PREVIEW_INPUT_PROMPT.contains("\x1b[36m"));
        assert!(PREVIEW_INPUT_PROMPT.ends_with("\x1b[0m "));
    }
}
