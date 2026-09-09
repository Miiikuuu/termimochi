//! Resolve native Fastfetch positioning into static, bounded rows. Cursor
//! commands are interpreted here, never forwarded to the interactive VTE.
use crate::greeting_art::{Style, sgr};
use std::sync::Arc;
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

const MAX_ROWS: usize = 256;

#[derive(Debug)]
enum Op {
    Text(String, Arc<str>),
    Newline,
    Move(char, usize),
}

#[derive(Debug)]
pub(crate) struct NativeOutput(Vec<Op>);

impl NativeOutput {
    pub(super) fn parse(raw: &str) -> Self {
        let mut ops = Vec::new();
        let mut style = Style::default();
        let mut chars = raw.chars().peekable();
        while let Some(ch) = chars.next() {
            let escape = match ch {
                '\x1b' => chars.next(),
                '\u{009b}' => Some('['),
                '\u{009d}' => Some(']'),
                '\u{0090}' => Some('P'),
                '\u{0098}' => Some('X'),
                '\u{009e}' => Some('^'),
                '\u{009f}' => Some('_'),
                _ => None,
            };
            if ch == '\x1b' || escape.is_some() {
                match escape {
                    Some('[') => {
                        let mut params = String::new();
                        for end in chars.by_ref() {
                            if ('@'..='~').contains(&end) {
                                if end == 'm' {
                                    if let Some(codes) = sgr(&params) {
                                        style.apply(&codes);
                                    }
                                } else if matches!(end, 'A' | 'B' | 'C' | 'D' | 'G')
                                    && params.len() <= 10
                                    && params.bytes().all(|b| b.is_ascii_digit())
                                    && let Ok(n) = params
                                        .parse::<u32>()
                                        .or_else(|e| if params.is_empty() { Ok(1) } else { Err(e) })
                                {
                                    ops.push(Op::Move(end, n.max(1) as usize));
                                }
                                break;
                            }
                            params.push(end);
                        }
                    }
                    Some(']' | 'P' | 'X' | '^' | '_') => {
                        let mut escaped = false;
                        for next in chars.by_ref() {
                            if matches!(next, '\x07' | '\u{009c}') || escaped && next == '\\' {
                                break;
                            }
                            escaped = next == '\x1b';
                        }
                    }
                    Some(' '..='/') => {
                        for next in chars.by_ref() {
                            if ('0'..='~').contains(&next) {
                                break;
                            }
                        }
                    }
                    _ => {}
                }
            } else if ch == '\n' {
                // Fastfetch writes LF to a terminal with ONLCR enabled.
                ops.push(Op::Newline);
            } else if ch == '\r' {
                ops.push(Op::Move('G', 1));
            } else if !ch.is_control() {
                let mut text = String::from(ch);
                while chars.peek().is_some_and(|c| !c.is_control()) {
                    text.push(chars.next().unwrap());
                }
                let text = crate::starship_import::terminal_safe_ansi(&text);
                ops.push(Op::Text(text, style.prefix().into()));
            }
        }
        Self(ops)
    }

    /// Replay at the current grid width, including Fastfetch's right-aligned
    /// `CSI 9999999 C` logo. Resizing needs no new process or stale-width cache.
    pub(crate) fn render(&self, columns: usize) -> String {
        let columns = columns.clamp(1, 240);
        let mut rows = vec![vec![Cell::Empty; columns]];
        let (mut row, mut col) = (0usize, 0usize);
        for op in &self.0 {
            match op {
                Op::Newline => {
                    row = (row + 1).min(MAX_ROWS - 1);
                    col = 0;
                }
                Op::Move(command, n) => match command {
                    'A' => row = row.saturating_sub(*n),
                    'B' => row = row.saturating_add(*n).min(MAX_ROWS - 1),
                    'C' => col = col.saturating_add(*n).min(columns - 1),
                    'D' => col = col.saturating_sub(*n),
                    'G' => col = (n - 1).min(columns - 1),
                    _ => unreachable!(),
                },
                Op::Text(text, style) => {
                    if rows.len() <= row {
                        rows.resize_with(row + 1, || vec![Cell::Empty; columns]);
                    }
                    let cells = &mut rows[row];
                    for glyph in text.graphemes(true) {
                        let width = glyph.width();
                        if width == 0 {
                            continue;
                        }
                        // Clip overflow without wrapping it into another field.
                        if col + width <= columns {
                            for at in col..col + width {
                                clear(cells, at);
                            }
                            cells[col] = Cell::Glyph(glyph.into(), style.clone(), width);
                            for cell in &mut cells[col + 1..col + width] {
                                *cell = Cell::Continuation(col);
                            }
                        }
                        col = col.saturating_add(width);
                    }
                }
            }
        }
        // A final LF terminates a row; additional LFs are intentional Break
        // modules. Keep those blank rows without inventing an extra cursor row.
        if rows.len() < row {
            rows.resize_with(row, || vec![Cell::Empty; columns]);
        }
        let mut output = String::new();
        for cells in rows {
            let length = cells
                .iter()
                .rposition(|c| !matches!(c, Cell::Empty))
                .map_or(0, |n| n + 1);
            let mut previous: &str = "";
            for cell in &cells[..length] {
                let (text, style) = match cell {
                    Cell::Empty => (" ", ""),
                    Cell::Glyph(text, style, _) => (text.as_str(), style.as_ref()),
                    Cell::Continuation(_) => continue,
                };
                if style != previous {
                    output.push_str("\x1b[0m");
                    output.push_str(style);
                    previous = style;
                }
                output.push_str(text);
            }
            output.push_str("\x1b[0m\r\n");
        }
        output
    }
}

#[derive(Clone)]
enum Cell {
    Empty,
    Glyph(String, Arc<str>, usize),
    Continuation(usize),
}

fn clear(cells: &mut [Cell], at: usize) {
    let origin = match cells[at] {
        Cell::Continuation(origin) => origin,
        _ => at,
    };
    let width = match cells[origin] {
        Cell::Glyph(_, _, width) => width,
        _ => 1,
    };
    cells[origin..origin + width].fill(Cell::Empty);
}

#[cfg(test)]
mod tests {
    use super::*;
    fn plain(output: &str, columns: usize) -> String {
        let rendered = NativeOutput::parse(output).render(columns);
        rendered
            .split("\x1b[")
            .enumerate()
            .map(|(i, s)| {
                if i == 0 {
                    s
                } else {
                    s.split_once('m').unwrap().1
                }
            })
            .collect()
    }
    #[test]
    fn fields_return_to_the_logo_start_and_keep_padding() {
        let raw =
            "\x1b[1mAAA\nBBB\nCCC\x1b[m\x1b[1G\x1b[2A\x1b[6CONE: value1\n\x1b[6CTWO: value2\n\n";
        assert_eq!(
            plain(raw, 80),
            "AAA   ONE: value1\r\nBBB   TWO: value2\r\nCCC\r\n"
        );
    }
    #[test]
    fn right_alignment_replays_at_each_width_and_preserves_rows() {
        let raw =
            "\x1b[9999999C\x1b[6DAAA\n\x1b[9999999C\x1b[6DBBB\x1b[1G\x1b[1AONE: value\nTWO: value";
        for width in [80, 100, 120, 80] {
            let text = plain(raw, width);
            let lines: Vec<_> = text.lines().collect();
            assert_eq!(lines.len(), 2);
            assert_eq!(lines[0].find("AAA"), Some(width - 7));
            assert_eq!(lines[1].find("BBB"), Some(width - 7));
            assert!(lines[0].starts_with("ONE: value"));
            assert!(lines[1].starts_with("TWO: value"));
        }
    }
    #[test]
    fn unicode_clipping_and_overwrite_never_leave_half_a_glyph() {
        assert_eq!(plain("中🙂e\u{301}!?", 5), "中🙂e\u{301}\r\n");
        assert_eq!(plain("中🙂\x1b[2Gx", 8), " x🙂\r\n");
        assert_eq!(plain("中🙂\x1b[3GX", 8), "中X\r\n");
    }
    #[test]
    fn decorative_blank_rows_and_carriage_returns_are_preserved() {
        assert_eq!(plain("ONE\n\nTWO\n\n", 80), "ONE\r\n\r\nTWO\r\n\r\n");
        assert_eq!(plain("old\rNEW\n", 80), "NEW\r\n");
    }
    #[test]
    fn colors_cross_rows_without_bleeding_into_fields() {
        let text = NativeOutput::parse("\x1b[38;2;12;34;56mAA\nBB\x1b[m\x1b[1G\x1b[1A\x1b[4CFIELD")
            .render(80);
        assert_eq!(text.matches("38;2;12;34;56").count(), 2);
        assert!(text.contains("AA\x1b[0m  FIELD"));
        assert_eq!(crate::starship_import::terminal_safe_ansi(&text), text);
    }
    #[test]
    fn randomized_control_streams_never_escape_or_exceed_the_grid() {
        let chunks = [
            "x",
            "中",
            "🙂",
            "e\u{301}",
            "\n",
            "\r",
            "\x1b[1A",
            "\x1b[0G",
            "\x1b[9999999C",
            "\x1b[9999999D",
            "\x1b[4294967295B",
            "\x1b[999999999999999999999G",
            "\x1b[38;2;12;34;56m",
            "\x1b[48:2::255:0:255m",
            "\x1b[0m",
            "\x1b[2J",
            "\x1b]52;c;NEVER\x07",
            "\x1bPNEVER\x1b\\",
            "\u{009b}3C",
            "\u{202e}",
        ];
        let mut seed = 0x7465726d696d6f63_u64;
        for case in 0..1024 {
            let mut raw = String::new();
            for _ in 0..48 {
                seed ^= seed << 13;
                seed ^= seed >> 7;
                seed ^= seed << 17;
                raw.push_str(chunks[seed as usize % chunks.len()]);
            }
            let columns = [0, 1, 11, 40, 80, 100, 120, 240, usize::MAX][case % 9];
            let output = NativeOutput::parse(&raw).render(columns);
            assert_eq!(
                crate::starship_import::terminal_safe_ansi(&output),
                output,
                "case {case}"
            );
            assert!(!output.contains("NEVER"));
            assert!(output.lines().count() <= MAX_ROWS);
            assert!(output.len() <= 4 * 1024 * 1024);
            for line in output.lines() {
                assert!(
                    crate::greeting::clip_ansi(line, usize::MAX).1 <= columns.clamp(1, 240),
                    "case {case}"
                );
            }
        }
    }

    #[test]
    fn unsafe_controls_and_extreme_coordinates_stay_bounded() {
        let raw = "\x1b]52;c;clipboard\x07\x1bPpayload\x1b\\\u{009d}title\u{009c}\x1b[2J\x1b[?25lOK\x1b[999999999B\x1b[999999999C!";
        let output = NativeOutput::parse(raw).render(80);
        assert_eq!(crate::starship_import::terminal_safe_ansi(&output), output);
        assert_eq!(output.lines().count(), MAX_ROWS);
        assert!(!output.contains("payload") && !output.contains("clipboard"));
        assert!(output.len() < 10_000);
    }
}
