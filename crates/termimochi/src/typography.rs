use gtk::pango;
use gtk::pango::prelude::*;

pub(crate) const DEFAULT_FONT_FAMILY: &str = "Monospace";
pub(crate) const DEFAULT_FONT_SIZE: f64 = 10.5;
pub(crate) const DEFAULT_CELL_SCALE: f64 = 1.0;
pub(crate) const MIN_FONT_SIZE: f64 = 8.0;
pub(crate) const MAX_FONT_SIZE: f64 = 24.0;
pub(crate) const MIN_CELL_SCALE: f64 = 1.0;
pub(crate) const MAX_CELL_SCALE: f64 = 2.0;

/// Stable private-use glyphs shared by current Nerd Font releases. Checking
/// the loaded face itself (instead of a shaped layout) prevents a fallback
/// font from making an unpatched family look compatible.
pub(crate) const NERD_FONT_PROBES: [char; 5] = [
    '\u{e0a0}', // Powerline branch
    '\u{e0b0}', // Powerline hard divider
    '\u{f120}', // terminal
    '\u{f121}', // code
    '\u{f17c}', // Linux
];
const TERMINAL_TEXT_PROBES: [char; 5] = ['A', 'M', '0', '{', '|'];

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum PreviewFontWeight {
    #[default]
    Regular,
    Medium,
    Semibold,
    Bold,
}

impl PreviewFontWeight {
    pub(crate) const ALL: [Self; 4] = [Self::Regular, Self::Medium, Self::Semibold, Self::Bold];

    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Regular => "Regular",
            Self::Medium => "Medium",
            Self::Semibold => "Semibold",
            Self::Bold => "Bold",
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
            Self::Regular => 0,
            Self::Medium => 1,
            Self::Semibold => 2,
            Self::Bold => 3,
        }
    }

    fn from_pango_weight(weight: pango::Weight) -> Self {
        match weight {
            pango::Weight::Medium => Self::Medium,
            pango::Weight::Semibold => Self::Semibold,
            pango::Weight::Bold
            | pango::Weight::Ultrabold
            | pango::Weight::Heavy
            | pango::Weight::Ultraheavy => Self::Bold,
            _ => Self::Regular,
        }
    }

    const fn pango_weight(self) -> pango::Weight {
        match self {
            Self::Regular => pango::Weight::Normal,
            Self::Medium => pango::Weight::Medium,
            Self::Semibold => pango::Weight::Semibold,
            Self::Bold => pango::Weight::Bold,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct TypographySettings {
    pub(crate) family: String,
    pub(crate) size: f64,
    pub(crate) weight: PreviewFontWeight,
    pub(crate) line_height: f64,
    pub(crate) cell_width: f64,
}

impl Default for TypographySettings {
    fn default() -> Self {
        Self {
            family: DEFAULT_FONT_FAMILY.to_owned(),
            size: DEFAULT_FONT_SIZE,
            weight: PreviewFontWeight::Regular,
            line_height: DEFAULT_CELL_SCALE,
            cell_width: DEFAULT_CELL_SCALE,
        }
    }
}

impl TypographySettings {
    /// Parse the Pango font strings used by GNOME and Ptyxis settings while
    /// keeping the preview's point-size contract explicit. Absolute pixel
    /// sizes and omitted sizes fall back instead of being mislabeled as pt.
    pub(crate) fn from_font_name(font_name: &str, line_height: f64, cell_width: f64) -> Self {
        let description = pango::FontDescription::from_string(font_name.trim());
        let family = description
            .family()
            .filter(|family| !family.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_FONT_FAMILY.into());
        let size = if description.size() > 0 && !description.is_size_absolute() {
            f64::from(description.size()) / f64::from(pango::SCALE)
        } else {
            DEFAULT_FONT_SIZE
        };
        Self::new(
            &family,
            size,
            PreviewFontWeight::from_pango_weight(description.weight()),
            line_height,
            cell_width,
        )
    }

    pub(crate) fn new(
        family: &str,
        size: f64,
        weight: PreviewFontWeight,
        line_height: f64,
        cell_width: f64,
    ) -> Self {
        let family = family.trim();
        Self {
            family: if family.is_empty() {
                DEFAULT_FONT_FAMILY.to_owned()
            } else {
                family.to_owned()
            },
            size: finite_clamp(size, MIN_FONT_SIZE, MAX_FONT_SIZE, DEFAULT_FONT_SIZE),
            weight,
            line_height: finite_clamp(
                line_height,
                MIN_CELL_SCALE,
                MAX_CELL_SCALE,
                DEFAULT_CELL_SCALE,
            ),
            cell_width: finite_clamp(
                cell_width,
                MIN_CELL_SCALE,
                MAX_CELL_SCALE,
                DEFAULT_CELL_SCALE,
            ),
        }
    }

    pub(crate) fn font_description(&self) -> pango::FontDescription {
        let mut description = pango::FontDescription::new();
        description.set_family(&self.family);
        description.set_size((self.size * f64::from(pango::SCALE)).round() as i32);
        description.set_weight(self.weight.pango_weight());
        description
    }
}

fn finite_clamp(value: f64, minimum: f64, maximum: f64, fallback: f64) -> f64 {
    if value.is_finite() {
        value.clamp(minimum, maximum)
    } else {
        fallback
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum NerdFontSupport {
    Unresolved,
    Missing,
    Partial { present: usize, total: usize },
    Ready,
}

impl NerdFontSupport {
    fn from_count(present: usize, total: usize) -> Self {
        if total > 0 && present >= total {
            Self::Ready
        } else if present > 0 {
            Self::Partial { present, total }
        } else {
            Self::Missing
        }
    }

    pub(crate) fn label(self) -> String {
        match self {
            Self::Unresolved => "Nerd Font Unavailable".to_owned(),
            Self::Missing => format!("Nerd Font 0/{}", NERD_FONT_PROBES.len()),
            Self::Partial { present, total } => format!("Nerd Font {present}/{total}"),
            Self::Ready => {
                let total = NERD_FONT_PROBES.len();
                format!("Nerd Font {total}/{total}")
            }
        }
    }

    pub(crate) fn detail(self) -> String {
        match self {
            Self::Unresolved => {
                "The selected font face could not be resolved for a direct glyph check".to_owned()
            }
            Self::Missing => format!(
                "The selected font has none of the {} tested Nerd Font glyphs",
                NERD_FONT_PROBES.len()
            ),
            Self::Partial { present, total } => {
                format!("The selected font has {present} of {total} tested Nerd Font glyphs")
            }
            Self::Ready => format!(
                "The selected font has all {} tested Nerd Font glyphs",
                NERD_FONT_PROBES.len()
            ),
        }
    }
}

pub(crate) fn is_usable_terminal_family(
    context: &pango::Context,
    family: &pango::FontFamily,
) -> bool {
    if !family.is_monospace() {
        return false;
    }
    let family_name = family.name();
    let settings = TypographySettings::new(
        &family_name,
        DEFAULT_FONT_SIZE,
        PreviewFontWeight::Regular,
        DEFAULT_CELL_SCALE,
        DEFAULT_CELL_SCALE,
    );
    let Some(font) = context.load_font(&settings.font_description()) else {
        return false;
    };
    let Some(resolved_family) = font.face().map(|face| face.family()) else {
        return false;
    };
    resolved_family.name() == family_name
        && TERMINAL_TEXT_PROBES
            .into_iter()
            .all(|character| font.has_char(character))
}

pub(crate) fn detect_nerd_font_support(
    context: &pango::Context,
    description: &pango::FontDescription,
) -> NerdFontSupport {
    let Some(font) = context.load_font(description) else {
        return NerdFontSupport::Unresolved;
    };

    let requested_family = description.family().unwrap_or_default();
    let resolved_family = font
        .face()
        .map(|face| face.family().name())
        .or_else(|| font.describe().family())
        .unwrap_or_default();
    let is_generic = requested_family.eq_ignore_ascii_case(DEFAULT_FONT_FAMILY);
    if !is_generic && !requested_family.eq_ignore_ascii_case(&resolved_family) {
        return NerdFontSupport::Unresolved;
    }

    let present = NERD_FONT_PROBES
        .into_iter()
        .filter(|character| font.has_char(*character))
        .count();
    NerdFontSupport::from_count(present, NERD_FONT_PROBES.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_build_an_exact_pango_description() {
        let settings = TypographySettings::new(
            "JetBrainsMono Nerd Font",
            13.5,
            PreviewFontWeight::Semibold,
            1.25,
            1.1,
        );
        let description = settings.font_description();

        assert_eq!(
            description.family().as_deref(),
            Some("JetBrainsMono Nerd Font")
        );
        assert_eq!(description.size(), 13_824);
        assert_eq!(description.weight(), pango::Weight::Semibold);
        assert_eq!(settings.line_height, 1.25);
        assert_eq!(settings.cell_width, 1.1);
    }

    #[test]
    fn invalid_or_extreme_values_are_normalized() {
        let low = TypographySettings::new("  ", -20.0, PreviewFontWeight::Regular, 0.1, f64::NAN);
        assert_eq!(low.family, DEFAULT_FONT_FAMILY);
        assert_eq!(low.size, MIN_FONT_SIZE);
        assert_eq!(low.line_height, MIN_CELL_SCALE);
        assert_eq!(low.cell_width, DEFAULT_CELL_SCALE);

        let high =
            TypographySettings::new(" Mono ", 500.0, PreviewFontWeight::Bold, f64::INFINITY, 9.0);
        assert_eq!(high.family, "Mono");
        assert_eq!(high.size, MAX_FONT_SIZE);
        assert_eq!(high.line_height, DEFAULT_CELL_SCALE);
        assert_eq!(high.cell_width, MAX_CELL_SCALE);
    }

    #[test]
    fn weight_selector_has_stable_labels_and_a_safe_fallback() {
        assert_eq!(
            PreviewFontWeight::ALL.map(PreviewFontWeight::label),
            ["Regular", "Medium", "Semibold", "Bold"]
        );
        assert_eq!(
            PreviewFontWeight::from_index(u32::MAX),
            PreviewFontWeight::Regular
        );
        for weight in PreviewFontWeight::ALL {
            assert_eq!(PreviewFontWeight::from_index(weight.index()), weight);
        }
    }

    #[test]
    fn pango_font_names_import_family_size_and_nearest_supported_weight() {
        let settings = TypographySettings::from_font_name("JetBrains Mono Bold 12", 1.15, 1.05);
        assert_eq!(settings.family, "JetBrains Mono");
        assert_eq!(settings.size, 12.0);
        assert_eq!(settings.weight, PreviewFontWeight::Bold);
        assert_eq!(settings.line_height, 1.15);
        assert_eq!(settings.cell_width, 1.05);

        assert_eq!(
            TypographySettings::from_font_name("Monospace Medium 11", 1.0, 1.0).weight,
            PreviewFontWeight::Medium
        );
        assert_eq!(
            TypographySettings::from_font_name("Monospace Semibold 11", 1.0, 1.0).weight,
            PreviewFontWeight::Semibold
        );
        assert_eq!(
            TypographySettings::from_font_name("Monospace Light 11", 1.0, 1.0).weight,
            PreviewFontWeight::Regular
        );
    }

    #[test]
    fn incomplete_or_absolute_pango_sizes_use_the_preview_default() {
        let omitted = TypographySettings::from_font_name("Monospace", 1.0, 1.0);
        let absolute = TypographySettings::from_font_name("Monospace 18px", 1.0, 1.0);
        let empty = TypographySettings::from_font_name("", 1.0, 1.0);
        assert_eq!(omitted.size, DEFAULT_FONT_SIZE);
        assert_eq!(absolute.size, DEFAULT_FONT_SIZE);
        assert_eq!(empty.family, DEFAULT_FONT_FAMILY);
        assert_eq!(empty.size, DEFAULT_FONT_SIZE);
    }

    #[test]
    fn nerd_font_status_distinguishes_missing_partial_and_complete() {
        let missing = NerdFontSupport::from_count(0, 5);
        let partial = NerdFontSupport::from_count(2, 5);
        let ready = NerdFontSupport::from_count(5, 5);
        assert_eq!(missing, NerdFontSupport::Missing);
        assert_eq!(NerdFontSupport::Unresolved.label(), "Nerd Font Unavailable");
        assert_eq!(
            partial,
            NerdFontSupport::Partial {
                present: 2,
                total: 5
            }
        );
        assert_eq!(ready, NerdFontSupport::Ready);
        assert_eq!(missing.label(), "Nerd Font 0/5");
        assert_eq!(partial.label(), "Nerd Font 2/5");
        assert_eq!(ready.label(), "Nerd Font 5/5");
        assert_eq!(NerdFontSupport::from_count(9, 5), NerdFontSupport::Ready);
        assert_eq!(NerdFontSupport::from_count(0, 0), NerdFontSupport::Missing);
    }
}
