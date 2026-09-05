//! Semantic hit testing without changing the visible terminal transcript.
//!
//! VTE supplies the actual text before/under the pointer. Matching that text
//! against the recorded feed avoids guessing Unicode cell widths, soft wraps,
//! or scrollback offsets. Whitespace is ignored because VTE trims row padding.
use crate::prompt::PromptSegmentKind;

pub(crate) const ANSI_NAMES: [&str; 16] = [
    "Black",
    "Red",
    "Green",
    "Yellow",
    "Blue",
    "Magenta",
    "Cyan",
    "White",
    "Bright Black",
    "Bright Red",
    "Bright Green",
    "Bright Yellow",
    "Bright Blue",
    "Bright Magenta",
    "Bright Cyan",
    "Bright White",
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PreviewTarget {
    Typography,
    Ansi(u8),
    Cursor,
    Padding,
    TabBar,
    Prompt,
    PromptSegment(PromptSegmentKind),
    PromptCharacter,
}

impl PreviewTarget {
    pub(crate) fn label(self) -> String {
        match self {
            Self::Typography => "Typography · Font Family".into(),
            Self::Ansi(index) => ANSI_NAMES.get(usize::from(index)).map_or_else(
                || format!("Palette · Color{index}"),
                |name| format!("Palette · {name} / Color{index}"),
            ),
            Self::Cursor => "Layout · Cursor".into(),
            Self::Padding => "Layout · Content Padding".into(),
            Self::TabBar => "Layout · Tab Bar".into(),
            Self::Prompt => "Prompt · Your Starship (read-only)".into(),
            Self::PromptSegment(kind) => format!("Prompt · {} Accent", kind.label()),
            Self::PromptCharacter => "Prompt · Character".into(),
        }
    }
}

#[derive(Debug, Default)]
pub(crate) struct PreviewMap {
    text: String,
    // One target per UTF-8 byte, not per terminal column.
    targets: Vec<PreviewTarget>,
    foreground: Option<u8>,
    bold: bool,
    escape: String,
}

impl PreviewMap {
    pub(crate) fn record(
        &mut self,
        text: &str,
        scope: Option<PreviewTarget>,
        bold_is_bright: bool,
    ) {
        for ch in text.chars() {
            if !self.escape.is_empty() {
                self.escape.push(ch);
                if self.escape.len() == 2 && ch != '[' {
                    // Input is restricted to CSI-only specimens and sanitized
                    // snapshots. Fail closed if a future feed violates that.
                    self.text.clear();
                    self.targets.clear();
                    self.escape.clear();
                    continue;
                }
                if self.escape.len() > 2 && ('@'..='~').contains(&ch) {
                    if ch == 'm' {
                        let parameters = self.escape[2..self.escape.len() - 1].to_owned();
                        self.sgr(&parameters);
                    }
                    self.escape.clear();
                }
                continue;
            }
            if ch == '\x1b' {
                self.escape.push(ch);
            } else if !ch.is_whitespace() && !ch.is_control() {
                let target = scope.unwrap_or_else(|| {
                    self.foreground.map_or(PreviewTarget::Typography, |index| {
                        PreviewTarget::Ansi(if bold_is_bright && self.bold && index < 8 {
                            index + 8
                        } else {
                            index
                        })
                    })
                });
                self.text.push(ch);
                self.targets
                    .extend(std::iter::repeat_n(target, ch.len_utf8()));
            }
        }
    }

    fn sgr(&mut self, parameters: &str) {
        let values: Vec<_> = parameters
            .split(';')
            .map(|part| part.parse::<u16>().unwrap_or(0))
            .collect();
        let mut i = 0;
        while i < values.len() {
            match values[i] {
                0 => {
                    self.foreground = None;
                    self.bold = false;
                }
                1 => self.bold = true,
                22 => self.bold = false,
                30..=37 => self.foreground = Some((values[i] - 30) as u8),
                90..=97 => self.foreground = Some((values[i] - 90 + 8) as u8),
                39 => self.foreground = None,
                38 | 48 | 58 => {
                    let foreground = values[i] == 38;
                    match values.get(i + 1) {
                        Some(5) => {
                            if foreground {
                                self.foreground = values
                                    .get(i + 2)
                                    .filter(|index| **index < 16)
                                    .map(|index| *index as u8);
                            }
                            i += 2;
                        }
                        Some(2) => {
                            // Literal RGB and 256-color entries beyond 15 do
                            // not belong to any editable base-palette slot.
                            if foreground {
                                self.foreground = None;
                            }
                            i += 4;
                        }
                        _ => {
                            if foreground {
                                self.foreground = None;
                            }
                        }
                    }
                }
                _ => {}
            }
            i += 1;
        }
    }

    pub(crate) fn resolve(&self, buffer: &str, prefix: &str, cell: &str) -> Option<PreviewTarget> {
        let buffer = compact(buffer);
        let prefix = compact(prefix);
        let cell = compact(cell);
        if cell.is_empty() || buffer.is_empty() || !buffer.starts_with(&prefix) {
            return None;
        }
        // The retained buffer is a suffix once the scrollback limit is hit.
        // Repeated prompts remain distinguishable through their full context.
        let offset = if let Some(start) = self.text.rfind(&buffer) {
            start.checked_add(prefix.len())?
        } else {
            // A reset can leave an older screen in the retained ring. Only
            // the newest complete copy belongs to this feed's semantic map.
            prefix.len().checked_sub(buffer.rfind(&self.text)?)?
        };
        if !self.text.get(offset..)?.starts_with(&cell) {
            return None;
        }
        let target = *self.targets.get(offset)?;
        self.targets
            .get(offset..offset + cell.len())?
            .iter()
            .all(|other| *other == target)
            .then_some(target)
    }
}

fn compact(text: &str) -> String {
    text.chars()
        .filter(|ch| !ch.is_whitespace() && !ch.is_control())
        .collect()
}

/// A click must stay a click for its whole lifetime, even if a drag returns to
/// its starting point. Delaying navigation also preserves double/triple-click
/// selection. The event controller never claims VTE's pointer events.
#[derive(Default)]
pub(crate) struct PreviewClick {
    start: Option<(f64, f64)>,
    dragged: bool,
}

impl PreviewClick {
    pub(crate) fn press(&mut self, x: f64, y: f64) {
        self.start = Some((x, y));
        self.dragged = false;
    }

    pub(crate) fn motion(&mut self, x: f64, y: f64, threshold: f64) {
        if let Some((sx, sy)) = self.start {
            self.dragged |= (x - sx).abs() > threshold || (y - sy).abs() > threshold;
        }
    }

    pub(crate) fn release(&mut self, x: f64, y: f64, threshold: f64) -> bool {
        self.motion(x, y, threshold);
        self.start.take().is_some() && !self.dragged
    }

    pub(crate) fn cancel(&mut self) {
        self.start = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indexed_colors_and_bold_bright_follow_sgr_across_chunks() {
        let mut map = PreviewMap::default();
        map.record("\x1b[1;3", None, true);
        map.record("2mPASS\x1b[22m ok\x1b[0m plain", None, true);
        assert_eq!(
            map.resolve("PASS ok plain", "", "P"),
            Some(PreviewTarget::Ansi(10))
        );
        assert_eq!(
            map.resolve("PASS ok plain", "PASS ", "o"),
            Some(PreviewTarget::Ansi(2))
        );
        assert_eq!(
            map.resolve("PASS ok plain", "PASS ok ", "p"),
            Some(PreviewTarget::Typography)
        );
    }

    #[test]
    fn all_base_slots_and_explicit_rgb_are_not_confused() {
        for index in 0..16 {
            let mut map = PreviewMap::default();
            map.record(&format!("\x1b[38;5;{index}mX"), None, false);
            assert_eq!(map.resolve("X", "", "X"), Some(PreviewTarget::Ansi(index)));
        }
        for sgr in ["38;2;31;32;33", "38;5;160", "48;2;31;32;33", "48;5;2"] {
            let mut map = PreviewMap::default();
            map.record(&format!("\x1b[{sgr}mX"), None, true);
            assert_eq!(map.resolve("X", "", "X"), Some(PreviewTarget::Typography));
        }
    }

    #[test]
    fn prompt_scopes_override_their_color_and_stop_at_command_boundary() {
        let mut map = PreviewMap::default();
        let target = PreviewTarget::PromptSegment(PromptSegmentKind::Directory);
        map.record("\x1b[34m~/中文/project\x1b[0m ", Some(target), false);
        map.record("pwd\r\n", None, false);
        assert_eq!(map.resolve("~/中文/project pwd", "~/", "中"), Some(target));
        assert_eq!(
            map.resolve("~/中文/project pwd", "~/中文/project ", "p"),
            Some(PreviewTarget::Typography)
        );
    }

    #[test]
    fn wrapping_unicode_and_retained_scrollback_use_actual_vte_text() {
        let mut map = PreviewMap::default();
        map.record("old\r\n", None, false);
        map.record(
            "\x1b[35m你好 👩‍💻 e\u{301} branch\x1b[0m",
            Some(PreviewTarget::Prompt),
            false,
        );
        assert_eq!(
            map.resolve("你好 \n👩‍💻 e\u{301}\nbranch", "你好 \n", "👩‍💻"),
            Some(PreviewTarget::Prompt)
        );
        assert_eq!(
            map.resolve("你好 👩‍💻 e\u{301} branch", "你好 👩‍💻 ", "e\u{301}"),
            Some(PreviewTarget::Prompt)
        );
    }

    #[test]
    fn stale_or_unmatched_text_never_jumps_to_an_unrelated_control() {
        let mut map = PreviewMap::default();
        map.record("new", Some(PreviewTarget::Prompt), false);
        assert_eq!(map.resolve("old", "", "o"), None);
        assert_eq!(map.resolve("new", "ne", "x"), None);
        assert_eq!(map.resolve("new", "", " "), None);
        assert_eq!(map.resolve("new", "bad", "n"), None);
    }

    #[test]
    fn an_older_retained_screen_cannot_steal_a_new_prompt_target() {
        let mut map = PreviewMap::default();
        map.record("new prompt", Some(PreviewTarget::Prompt), false);
        assert_eq!(
            map.resolve("old scene\nnew prompt", "old scene\n", "n"),
            Some(PreviewTarget::Prompt)
        );
        assert_eq!(map.resolve("old scene\nnew prompt", "", "o"), None);
    }

    #[test]
    fn dragging_away_and_back_or_cancelling_is_not_a_click() {
        let mut click = PreviewClick::default();
        click.press(10.0, 10.0);
        assert!(click.release(12.0, 10.0, 8.0));
        click.press(10.0, 10.0);
        click.motion(50.0, 10.0, 8.0);
        assert!(!click.release(10.0, 10.0, 8.0));
        click.press(10.0, 10.0);
        click.cancel();
        assert!(!click.release(10.0, 10.0, 8.0));
    }
}
