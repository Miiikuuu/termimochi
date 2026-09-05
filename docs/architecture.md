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
  lint / JSON / export                    Activity Rail -> left module stack
                                                           |-> Palette
                                                           |-> Typography
                                                           |-> Layout
                                                           +-> Prompt
                                         persistent preview -> diagnostics
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
are recomputed. A narrow Activity Rail switches only the left module stack
between Palette (`Ctrl+1`), Typography (`Ctrl+2`), Layout (`Ctrl+3`) and Prompt
(`Ctrl+4`). The right-side Live Preview and diagnostics are constructed once
and remain mounted. Palette, Typography and Layout share the selected scenario
and active light/dark variant. Current Folder is the default, including a
read-only Starship import when available. Prompt separates Your Starship from
Designer, which has a selector for the same folder snapshot and repeatable project,
failure, SSH, root and alignment samples. Switching modules rebuilds the VTE
contents; transcript and scroll position are presentation state, not state
promised across the Prompt boundary when using the Activity Rail. Point-to-edit
navigation is an exception: it switches only the editor, preserving the active
preview scene, scroll position and scratch input until an explicit scene change.

`preview_inspect.rs` records semantic targets alongside the unmodified ANSI
feed. VTE 0.76+ supplies the actual visible range and cell text for hit testing;
no hidden hyperlinks, guessed RGB matches or second terminal renderer are used.
The visible range is anchored against the retained ring because GTK adjustments
and VTE absolute rows can have different origins after reset. Unicode and soft
wraps are matched through actual extracted text, with ambiguous/stale matches
rejected. ANSI slots respect SGR and bold-is-bright; literal RGB does not pretend
to be a base-palette slot. Designer runs identify their module; imported prompts
only open the read-only source panel. A passive capture controller waits through
the system double-click interval and rejects drags, modified clicks, scrolls,
selection and intervening redraws. Inspect is explicitly opt-in and off at
startup. A non-targetable overlay shows the hit cell or padding strip and its
named destination; hover alone never changes the editor. Pointer updates are
coalesced and the VTE origin is cached until content or scrolling changes.
Blank and unmatched cells have no fallback target. Turning Inspect off, Escape,
scrolling or redrawing clears feedback and invalidates pending clicks. Press
and release must resolve to the same target. No document state or shell config
is changed.

The opt-in `point_to_edit_real_vte_navigation` test exercises a real GTK window,
wrapping, scrollback and scene preservation. Set `TERMIMOCHI_POINTER_TEST=1`
under X11 to additionally use `scripts/preview-pointer-driver.py` for real
hover, opt-in/off behavior, blank-space safety, single-click, word selection and drag-selection events (Python 3, libX11 and
libXtst required). Run this separately from display-independent tests:

```bash
GDK_BACKEND=x11 GTK_A11Y=none TERMIMOCHI_POINTER_TEST=1 \
  cargo test -p termimochi point_to_edit_real_vte_navigation -- \
  --ignored --test-threads=1
```

A VTE widget renders both the Current Folder snapshot and deterministic
samples for Shell, Codex, Git status/diff, test output, syntax, system activity
and multilingual alignment. `preview_context.rs` gathers the app's working
directory, username, hostname and Git status on a worker thread. Language
markers enable bounded installed-tool version probes in a neutral directory,
with a cleaned environment. Command probes share a deadline and cap output;
displayed names are sanitized and directory/status lists are limited. The
snapshot records its read time and missing information. It does not observe
another shell's last exit status, jobs or duration, and does not inspect shell
startup files or execute project scripts. Refresh collects a new snapshot.

`starship_import.rs` resolves `STARSHIP_CONFIG` or the XDG configuration path,
validates a size-limited UTF-8 TOML document and prepares a temporary copy with
only reviewed built-in modules and declarative options. Top-level format
variables cannot activate unreviewed modules; custom commands and executable
overrides are never forwarded. Starship renders the copy inside Bubblewrap with
a read-only filesystem, private temporary cache, no network, a cleaned environment
and bounded runtime/output. Git hooks and optional writes are disabled. VTE gets
only text, line breaks and SGR styles; OSC and cursor-control sequences are stripped.
Rendered ANSI is cached in the folder snapshot, so typing and color adjustments
never spawn a renderer. Missing sandbox/renderer, malformed files and skipped
features are reported. There is no unsandboxed fallback. Original configuration
and shell startup files are untouched, and imported prompts have no export action.

The Typography module uses installed monospace families and inspects the
selected Pango face directly for the Nerd icons shown in the specimen, without
accepting fallback-font coverage. The preview consumes the same 16-color model
as the linter. Its focusable scratch prompt echoes sanitized local input for
interaction and cursor testing. It has no shell/PTY child, so typing a command
never executes it; sample output is explicitly separate from live context.

Ptyxis `.palette` files do not contain typography settings, so those controls
intentionally affect only the live preview and do not enter palette save or
undo history. Their initial values come from the unified Ptyxis appearance
snapshot: the configured font (or desktop monospace font when
`use-system-font` is enabled) and the selected profile's cell height/width
scales. Missing settings fall back independently.
Layout controls follow the same preview-only boundary: they adjust the VTE
grid, cursor and scrollback chrome plus the GTK preview spacing without marking
the palette document as modified.

Prompt composition is a separate in-memory document with its own Undo/Redo
history. The same VTE renders current-folder or sample contexts for an ordered
set of Starship modules. Starting points only initialize that
structure; adding, removing, reordering or changing a typed accent produces a
custom document. The static module catalog keeps Starship names unique and
uses conditional preview contexts for Git and language runtimes. One/two-line
layout and blank-line spacing map to Starship's format settings; ASCII prompt
symbols, SSH-only hostnames and root styling keep preview and export behavior
aligned across supported shells. Export writes a complete, independent
configuration to a user-selected local `.toml` path through the atomic writer.
Designer does not merge with the read-only imported configuration, invoke a
shell, or change `.bashrc`, `.zshrc` or other startup files.
A successful export becomes the Prompt savepoint, so
closing the window warns separately about unexported Prompt edits and unsaved
palette edits.

Startup import is also read-only. `CurrentTerminalAppearance` reads palette,
font, cell scales, cursor shape/blink, configured grid, scrollbar policy,
bold/bright behavior and CJK ambiguous width together. It prefers an available
`PTYXIS_PROFILE` inherited from the launching terminal, then falls back to a
configured Ptyxis profile. Preview Source distinguishes launching, default,
fallback and unavailable settings. A desktop launch cannot identify another
window's active tab or temporary zoom. Transparency is shown as opaque base
colors; asymmetric padding and oversized grids are approximated with a notice.
Unsupported terminal hints use preview defaults rather than claiming a Ptyxis
match. This importer does not copy existing prompt text or a running session.
The selected palette is an in-memory snapshot, so ordinary Save opens a file
chooser instead of modifying the source profile.

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
3. Add explicit terminal-profile typography installation/export without
   weakening the palette format boundary.
