#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PreviewScenario {
    Shell,
    Codex,
    GitStatus,
    GitDiff,
    Tests,
    Code,
    Htop,
    Glyphs,
}

impl PreviewScenario {
    pub(crate) const ALL: [Self; 8] = [
        Self::Shell,
        Self::Codex,
        Self::GitStatus,
        Self::GitDiff,
        Self::Tests,
        Self::Code,
        Self::Htop,
        Self::Glyphs,
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
            Self::Glyphs => "Glyphs",
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
            Self::Glyphs => "font coverage",
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
            Self::Glyphs => GLYPHS,
        }
    }
}

pub(crate) const PREVIEW_COLUMNS: usize = 58;
pub(crate) const PREVIEW_ROWS: usize = 11;
pub(crate) const PREVIEW_HOME_AND_CLEAR: &[u8] = b"\x1b[H\x1b[2J";

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

const GLYPHS: &str = concat!(
    "\x1b[?25l",
    "\x1b[1mCJK / Emoji / Kaomoji Glyph Coverage\x1b[0m\r\n\r\n",
    "Simplified   终端麻薯，让配色清晰可读。\r\n",
    "Traditional  雲霧紙白，終端配色工作臺。\r\n",
    "Japanese     ターミナルの配色を確認します。\r\n",
    "Korean       터미널 색상과 글꼴을 확인합니다.\r\n\r\n",
    "Emoji     🌸 🍡 🐱 🚀 ✅ ⚠️  💻 🎨\r\n",
    "Kaomoji   (｡•̀ᴗ-)✧  ʕ•ᴥ•ʔ  (づ｡◕‿‿◕｡)づ\r\n",
    "Box       ╭──────╮  ├──────┤  ╰──────╯\r\n",
    "Blocks    ▁▂▃▄▅▆▇█  ░▒▓█  ←↑↓→  ◆◇○●\x1b[0m"
);

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

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
        assert_eq!(PREVIEW_HOME_AND_CLEAR, b"\x1b[H\x1b[2J");
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
                "Glyphs",
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
                "font coverage",
            ]
        );
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
}
