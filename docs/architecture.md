# TermiMochi architecture

TermiMochi treats a terminal theme as semantic data rather than a bag of hex
values. Parsing, validation, target rules and exporters remain independent from
GTK so they can be tested, reused by the CLI and embedded elsewhere.

```text
termimochi-core
  Ptyxis document -> palette model -> contrast engine -> target rules -> issues
          |                                                        |
          +---------------- safe serialization                     +-> stable codes

termimochi-cli                         termimochi (GTK/libadwaita)
  lint / JSON / export                    editor -> preview -> diagnostics
                                                        |-> atomic save / export
                                                        +-> Ptyxis install / rollback
```

## Core contracts

- `Rgb` parses six-digit sRGB values and implements WCAG relative luminance.
- `PtyxisPalette` requires a named `[Palette]` section and at least one of
  `[Light]` or `[Dark]`.
- Required colors are `Foreground`, `Background` and `Color0`–`Color15`.
- Recognized optional semantic colors match the original Python reference.
- Unknown properties and sections survive a load/save round trip.
- Diagnostics use stable codes so GUI text can be localized without changing
  integrations.

## Target adapters

The first adapter records the failure that motivated the project:

```text
Codex composer text       <- Ptyxis Foreground
Codex composer background <- Ptyxis Color0
```

The rule checks this relationship rather than a preferred hard-coded color, so
it works for any palette and either light or dark variants.

## UI boundary

The desktop crate owns only presentation and file interaction. Every edit is
written to the in-memory core model, after which the preview and diagnostics
are recomputed. A VTE widget renders deterministic ANSI transcripts for Shell,
Codex, Git, htop and Vim plus a CJK/Emoji/kaomoji coverage scene. It consumes
the same 16-color model as the linter, but never launches a shell or external
program while the user edits a palette.

Startup import is also read-only. It prefers the `PTYXIS_PROFILE` inherited
from the launching terminal, then falls back to Ptyxis' default profile. The
selected palette is parsed as an unsaved snapshot, so ordinary Save opens a
file chooser instead of modifying Ptyxis behind the user's back.

Ptyxis deployment is deliberately separate from ordinary save. Installation
uses a sibling temporary file, backs up an existing palette and writes a receipt
under the user's state directory. Rollback checks the installed byte length and
stable content hash before restoring or removing anything, so external edits
are never silently overwritten.

Kitty, Ghostty, WezTerm and Alacritty serialization lives in the core crate.
The GUI and CLI therefore share the exact same mapping of foreground,
background, cursor and all 16 ANSI colors.

## Next milestones

1. Add lossless comments and source-order preservation.
2. Add Flatpak packaging and release automation.
3. Add configurable font selection and automated missing-glyph diagnostics.
