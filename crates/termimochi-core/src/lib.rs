//! UI-independent palette model and readability engine for TermiMochi.

mod color;
mod export;
mod file;
mod lint;
mod palette;

pub use color::{ColorParseError, Rgb, contrast_ratio};
pub use export::{ExportError, ExportFormat, ExportedTheme, export_palette};
pub use file::{paths_refer_to_same_file, write_atomically};
pub use lint::{Issue, LintReport, Severity, Target, lint_palette};
pub use palette::{
    KNOWN_COLOR_KEYS, OPTIONAL_COLOR_KEYS, PaletteError, PtyxisPalette, REQUIRED_COLOR_KEYS,
    ThemeVariant, Variant,
};
