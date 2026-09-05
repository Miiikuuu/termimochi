use crate::preview::{PREVIEW_COLUMNS, PREVIEW_ROWS};

pub(crate) const MIN_CONTENT_PADDING: i32 = 0;
pub(crate) const MAX_CONTENT_PADDING: i32 = 24;
pub(crate) const DEFAULT_CONTENT_PADDING: i32 = 5;
pub(crate) const MIN_COLUMNS: usize = 40;
pub(crate) const MAX_COLUMNS: usize = 120;
pub(crate) const MIN_ROWS: usize = 8;
pub(crate) const MAX_ROWS: usize = 36;
pub(crate) const MIN_WINDOW_SPACING: i32 = 0;
pub(crate) const MAX_WINDOW_SPACING: i32 = 32;
pub(crate) const DEFAULT_WINDOW_SPACING: i32 = 18;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum PreviewCursorShape {
    #[default]
    Block,
    IBeam,
    Underline,
}

impl PreviewCursorShape {
    pub(crate) const ALL: [Self; 3] = [Self::Block, Self::IBeam, Self::Underline];

    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Block => "Block",
            Self::IBeam => "I-Beam",
            Self::Underline => "Underline",
        }
    }

    pub(crate) fn from_index(index: u32) -> Self {
        usize::try_from(index)
            .ok()
            .and_then(|index| Self::ALL.get(index))
            .copied()
            .unwrap_or_default()
    }

    pub(crate) const fn index(self) -> u32 {
        match self {
            Self::Block => 0,
            Self::IBeam => 1,
            Self::Underline => 2,
        }
    }

    pub(crate) const fn vte_shape(self) -> vte::CursorShape {
        match self {
            Self::Block => vte::CursorShape::Block,
            Self::IBeam => vte::CursorShape::Ibeam,
            Self::Underline => vte::CursorShape::Underline,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum PreviewCursorBlink {
    #[default]
    System,
    On,
    Off,
}

impl PreviewCursorBlink {
    pub(crate) const ALL: [Self; 3] = [Self::System, Self::On, Self::Off];

    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::System => "System",
            Self::On => "On",
            Self::Off => "Off",
        }
    }

    pub(crate) fn from_index(index: u32) -> Self {
        usize::try_from(index)
            .ok()
            .and_then(|index| Self::ALL.get(index))
            .copied()
            .unwrap_or_default()
    }

    pub(crate) const fn index(self) -> u32 {
        match self {
            Self::System => 0,
            Self::On => 1,
            Self::Off => 2,
        }
    }

    pub(crate) const fn vte_mode(self) -> vte::CursorBlinkMode {
        match self {
            Self::System => vte::CursorBlinkMode::System,
            Self::On => vte::CursorBlinkMode::On,
            Self::Off => vte::CursorBlinkMode::Off,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct LayoutSettings {
    pub(crate) content_padding: i32,
    pub(crate) columns: usize,
    pub(crate) rows: usize,
    pub(crate) cursor_shape: PreviewCursorShape,
    pub(crate) cursor_blink: PreviewCursorBlink,
    pub(crate) tab_bar: bool,
    pub(crate) scrollbar: bool,
    pub(crate) window_spacing: i32,
}

impl Default for LayoutSettings {
    fn default() -> Self {
        Self {
            content_padding: DEFAULT_CONTENT_PADDING,
            columns: PREVIEW_COLUMNS,
            rows: PREVIEW_ROWS,
            cursor_shape: PreviewCursorShape::default(),
            cursor_blink: PreviewCursorBlink::default(),
            tab_bar: true,
            scrollbar: false,
            window_spacing: DEFAULT_WINDOW_SPACING,
        }
    }
}

impl LayoutSettings {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        content_padding: i32,
        columns: usize,
        rows: usize,
        cursor_shape: PreviewCursorShape,
        cursor_blink: PreviewCursorBlink,
        tab_bar: bool,
        scrollbar: bool,
        window_spacing: i32,
    ) -> Self {
        Self {
            content_padding: content_padding.clamp(MIN_CONTENT_PADDING, MAX_CONTENT_PADDING),
            columns: columns.clamp(MIN_COLUMNS, MAX_COLUMNS),
            rows: rows.clamp(MIN_ROWS, MAX_ROWS),
            cursor_shape,
            cursor_blink,
            tab_bar,
            scrollbar,
            window_spacing: window_spacing.clamp(MIN_WINDOW_SPACING, MAX_WINDOW_SPACING),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_preserve_the_existing_preview_grid() {
        let settings = LayoutSettings::default();
        assert_eq!(settings.columns, PREVIEW_COLUMNS);
        assert_eq!(settings.rows, PREVIEW_ROWS);
        assert_eq!(settings.cursor_shape, PreviewCursorShape::Block);
        assert_eq!(settings.cursor_blink, PreviewCursorBlink::System);
        assert!(settings.tab_bar);
        assert!(!settings.scrollbar);
    }

    #[test]
    fn invalid_dimensions_and_spacing_are_clamped() {
        let low = LayoutSettings::new(
            -20,
            0,
            0,
            PreviewCursorShape::IBeam,
            PreviewCursorBlink::On,
            false,
            true,
            -10,
        );
        assert_eq!(low.content_padding, MIN_CONTENT_PADDING);
        assert_eq!(low.columns, MIN_COLUMNS);
        assert_eq!(low.rows, MIN_ROWS);
        assert_eq!(low.window_spacing, MIN_WINDOW_SPACING);
        assert!(!low.tab_bar);
        assert!(low.scrollbar);

        let high = LayoutSettings::new(
            i32::MAX,
            usize::MAX,
            usize::MAX,
            PreviewCursorShape::Underline,
            PreviewCursorBlink::System,
            true,
            false,
            i32::MAX,
        );
        assert_eq!(high.content_padding, MAX_CONTENT_PADDING);
        assert_eq!(high.columns, MAX_COLUMNS);
        assert_eq!(high.rows, MAX_ROWS);
        assert_eq!(high.window_spacing, MAX_WINDOW_SPACING);
    }

    #[test]
    fn selectors_have_stable_labels_indexes_and_safe_fallbacks() {
        assert_eq!(
            PreviewCursorShape::ALL.map(PreviewCursorShape::label),
            ["Block", "I-Beam", "Underline"]
        );
        assert_eq!(
            PreviewCursorBlink::ALL.map(PreviewCursorBlink::label),
            ["System", "On", "Off"]
        );
        for shape in PreviewCursorShape::ALL {
            assert_eq!(PreviewCursorShape::from_index(shape.index()), shape);
        }
        for blink in PreviewCursorBlink::ALL {
            assert_eq!(PreviewCursorBlink::from_index(blink.index()), blink);
        }
        assert_eq!(
            PreviewCursorShape::from_index(u32::MAX),
            PreviewCursorShape::Block
        );
        assert_eq!(
            PreviewCursorBlink::from_index(u32::MAX),
            PreviewCursorBlink::System
        );
    }
}
