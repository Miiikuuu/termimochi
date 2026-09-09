//! Locate a retained imported logo only when its entire visible rectangle has
//! one exact match. Native output otherwise has no reliable module provenance.
use super::*;

pub(super) fn imported_parts(text: String, artwork: Option<&str>) -> Vec<(String, GreetingPart)> {
    let fallback = || vec![(text.clone(), GreetingPart::Fields)];
    let Some(artwork) = artwork else {
        return fallback();
    };
    let art: Vec<_> = artwork.lines().map(str::trim_end).collect();
    let Some((anchor_row, anchor)) = art
        .iter()
        .enumerate()
        .filter(|(_, line)| !line.is_empty())
        .max_by_key(|(_, line)| line.width())
    else {
        return fallback();
    };
    let mut base = 0;
    let rows: Vec<_> = text
        .split_inclusive('\n')
        .map(|line| {
            let mut plain = String::new();
            let mut offsets = Vec::new();
            let mut chars = line.char_indices();
            while let Some((index, ch)) = chars.next() {
                if ch == '\x1b' {
                    for (_, next) in chars.by_ref() {
                        if next == 'm' {
                            break;
                        }
                    }
                } else if !ch.is_control() {
                    plain.push(ch);
                    offsets.extend(std::iter::repeat_n(base + index, ch.len_utf8()));
                }
            }
            offsets.push(base + line.trim_end_matches(['\r', '\n']).len());
            base += line.len();
            (plain, offsets)
        })
        .collect();
    let mut found = None;
    let mut attempts = 0;
    for (row, (plain, _)) in rows.iter().enumerate().skip(anchor_row) {
        for (at, _) in plain.match_indices(*anchor) {
            attempts += 1;
            if attempts > 128 {
                // Repeated tiny glyphs must not cause a costly rectangle
                // search on every preview redraw. Fall back without guessing.
                return fallback();
            }
            let column = plain[..at].width();
            let start = row - anchor_row;
            let mut ranges = Vec::new();
            let valid = art
                .iter()
                .enumerate()
                .filter(|(_, line)| !line.is_empty())
                .all(|(dy, line)| {
                    let Some((plain, offsets)) = rows.get(start + dy) else {
                        return false;
                    };
                    let mut width = 0;
                    let byte = plain.grapheme_indices(true).find_map(|(byte, glyph)| {
                        let matches = width == column;
                        width += glyph.width();
                        matches.then_some(byte)
                    });
                    let Some(byte) = byte else {
                        return false;
                    };
                    if !plain[byte..].starts_with(line) {
                        return false;
                    }
                    ranges.push(offsets[byte]..offsets[byte + line.len()]);
                    true
                });
            if valid {
                // Never guess when a logo also appears in a field/value.
                if found.is_some() {
                    return fallback();
                }
                found = Some(ranges);
            }
        }
    }
    let Some(ranges) = found else {
        return fallback();
    };
    let mut result = Vec::new();
    let mut offset = 0;
    for range in ranges {
        result.push((text[offset..range.start].into(), GreetingPart::Fields));
        result.push((text[range.clone()].into(), GreetingPart::Artwork));
        offset = range.end;
    }
    result.push((text[offset..].into(), GreetingPart::Fields));
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn imported_logo_exact_unicode_rectangle_and_ambiguous_fallback() {
        let raw = "OS: Linux  \x1b[31m中★\x1b[0m\r\nCPU: Rust  ▀▄\r\n";
        let parts = imported_parts(raw.into(), Some("中★\n▀▄"));
        assert_eq!(
            parts.iter().map(|(s, _)| s.as_str()).collect::<String>(),
            raw
        );
        assert_eq!(
            parts
                .iter()
                .filter(|(_, p)| *p == GreetingPart::Artwork)
                .count(),
            2
        );
        for (text, art) in [("AA AA\r\nBB BB\r\n", "AA\nBB"), ("AA\r\nCC\r\n", "AA\nBB")] {
            assert!(
                imported_parts(text.into(), Some(art))
                    .iter()
                    .all(|(_, p)| *p == GreetingPart::Fields)
            );
        }
    }
}
