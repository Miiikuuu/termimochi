//! Procedural, font-independent block coverage masks, independently authored.
//! Candidate foreground/background colors minimize weighted RGB squared error.
use super::{Character, Color, QUADRANTS};
use std::sync::LazyLock;

struct Symbol {
    character: char,
    mask: u64,
}

static SYMBOLS: LazyLock<Vec<Symbol>> = LazyLock::new(|| {
    let mut symbols = Vec::new();
    let mut add = |character, coverage: &dyn Fn(usize, usize) -> bool| {
        let mask = (0..64).fold(0, |bits, i| bits | (u64::from(coverage(i % 8, i / 8)) << i));
        if mask != 0 && !symbols.iter().any(|s: &Symbol| s.mask == mask) {
            symbols.push(Symbol { character, mask });
        }
    };
    // Prefer common block glyphs on ties. Complementary masks are equivalent
    // when both foreground and background are available.
    add('█', &|_, _| true);
    for (mask, &character) in QUADRANTS.iter().enumerate().skip(1) {
        add(character, &|x, y| {
            mask & (1 << (usize::from(y >= 4) * 2 + usize::from(x >= 4))) != 0
        });
    }
    for (n, character) in ['▁', '▂', '▃', '▄', '▅', '▆', '▇'].into_iter().enumerate()
    {
        add(character, &|_, y| y >= 7 - n);
    }
    for (n, character) in ['▏', '▎', '▍', '▌', '▋', '▊', '▉'].into_iter().enumerate()
    {
        add(character, &|x, _| x <= n);
    }
    add('▔', &|_, y| y == 0);
    add('▕', &|x, _| x == 7);
    for (character, kind) in [('◢', 0), ('◣', 1), ('◤', 2), ('◥', 3)] {
        add(character, &|x, y| match kind {
            0 => x + y >= 7,
            1 => y >= x,
            2 => x + y <= 7,
            _ => x >= y,
        });
    }
    symbols
});

pub(super) fn matched_cell(samples: [Option<Color>; 64]) -> Character {
    if samples.iter().all(Option::is_none) {
        return (' ', None, None);
    }
    let mut best = (f64::INFINITY, (' ', None, None));
    for symbol in SYMBOLS.iter() {
        let mut sums = [[0u32; 3]; 2];
        let mut squared = [[0u64; 3]; 2];
        let mut count = [0u32; 2];
        let mut empty = [0u32; 2];
        for (i, sample) in samples.iter().enumerate() {
            let group = usize::from(symbol.mask & (1 << i) != 0);
            if let Some(color) = sample {
                count[group] += 1;
                for channel in 0..3 {
                    sums[group][channel] += u32::from(color[channel]);
                    squared[group][channel] += u64::from(color[channel]).pow(2);
                }
            } else {
                empty[group] += 1;
            }
        }
        let colors: [Option<Color>; 2] = std::array::from_fn(|g| {
            (count[g] > 0)
                .then(|| std::array::from_fn(|c| ((sums[g][c] + count[g] / 2) / count[g]) as u8))
        });
        // A default foreground is not transparent: inverse silhouettes would
        // paint the supposedly empty area with the terminal's text color.
        if colors[1].is_none() && colors[0].is_some() {
            continue;
        }
        let mut score = 0.0;
        for g in 0..2 {
            if let Some(color) = colors[g] {
                for (c, weight) in [2.0, 4.0, 1.0].into_iter().enumerate() {
                    score += weight
                        * (squared[g][c] as f64
                            - 2.0 * f64::from(color[c]) * f64::from(sums[g][c])
                            + f64::from(count[g]) * f64::from(color[c]).powi(2));
                }
                score += f64::from(empty[g]) * 7.0 * 255.0 * 255.0;
            }
        }
        if score < best.0 {
            best = (score, (symbol.character, colors[1], colors[0]));
        }
    }
    best.1
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn eighth_blocks_and_diagonals_beat_a_quadrant_only_grid() {
        let a = [220, 30, 50];
        let b = [20, 110, 220];
        for (expected, mask) in [
            (
                '▁',
                SYMBOLS.iter().find(|s| s.character == '▁').unwrap().mask,
            ),
            (
                '◢',
                SYMBOLS.iter().find(|s| s.character == '◢').unwrap().mask,
            ),
        ] {
            let samples = std::array::from_fn(|i| Some(if mask & (1 << i) != 0 { a } else { b }));
            let (character, fg, bg) = matched_cell(samples);
            assert_eq!(character, expected);
            assert_eq!((fg, bg), (Some(a), Some(b)));
        }
        assert_eq!(matched_cell([None; 64]), (' ', None, None));
    }
}
