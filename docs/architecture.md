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
                                                           |-> Prompt
                                                           +-> Greeting
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

The application chrome uses a continuous white editing and preview workspace,
with a near-white navigation rail and soft gray control surfaces. `chrome.css`
scopes the editor controls and neutral focus states to the left workspace;
terminal palettes, swatches and semantic diagnostic colors are independent.
`style.rs` supplies application-local accent overrides
for both legacy named colors and GTK 4.16+ CSS variables without changing desktop
preferences. Navigation icons use original 24 px filled SVG contours so GTK's
symbolic recoloring preserves their geometry at normal and high-DPI scales.

The opt-in `light_chrome_pages_and_native_controls` GTK test checks both CSS
paths, entry contrast, neutral accents and unchanged palette data while visiting
the Palette, Typography, Layout and Prompt editors and their native menus. Run it separately from other graphical
tests; set `TERMIMOCHI_CHROME_SCREENSHOT_DIR` on X11 to capture each page and
popover through the pointer driver's named test-window bounds:

```bash
GDK_BACKEND=x11 GTK_A11Y=none \
  cargo test -p termimochi light_chrome_pages_and_native_controls -- \
  --ignored --test-threads=1
```

The desktop crate owns only presentation and file interaction. Every edit is
written to the in-memory core model, after which the preview and diagnostics
are recomputed. A narrow Activity Rail switches only the left module stack
between Palette (`Ctrl+1`), Typography (`Ctrl+2`), Layout (`Ctrl+3`), Prompt
(`Ctrl+4`) and Greeting (`Ctrl+5`). The right-side Live Preview and diagnostics are constructed once
and remain mounted. Palette, Typography and Layout share the selected scenario
and active light/dark variant. Current Folder is the default, including a
read-only Starship import when available. Prompt separates Your Starship from
Designer, which has a selector for the same folder snapshot and repeatable project,
failure, SSH, root and alignment samples. Greeting explicitly selects its fresh-terminal
scene; the existing four modules and point-to-edit
navigation switch only the editor: neither clears VTE, resets scratch input nor
queues a prompt render. The active preview source is independent of the left
editor source selector, including during inspection and asynchronous rendering.
Explicit prompt edits or module/field selections retain the existing scoped ANSI
transcript as an immutable base and append examples below it. A scenario change
or Reset Preview Session starts a new session without discarding prompt edits.

`preview_inspect.rs` records semantic targets alongside the unmodified ANSI
feed. VTE 0.76+ supplies the actual visible range and cell text for hit testing;
no hidden hyperlinks, guessed RGB matches or second terminal renderer are used.
The visible range is anchored against the retained ring because GTK adjustments
and VTE absolute rows can have different origins after reset. Unicode and soft
wraps are matched through actual extracted text, with ambiguous/stale matches
rejected. ANSI slots respect SGR and bold-is-bright; literal RGB does not pretend
to be a base-palette slot. Designer runs identify their module; imported prompts
open the configuration editor without changing the displayed scene. A passive capture controller waits through
the system double-click interval and rejects drags, modified clicks, scrolls,
selection and intervening redraws. Inspect is explicitly opt-in and off at
startup. A non-targetable overlay shows the hit cell or padding strip, with its
named destination docked in the lower-left corner; hover alone never changes
the editor. The hint does not follow the mouse or flip around near an edge,
and neither overlay participates in the terminal's size request. Pointer
updates are coalesced to the display frame; unchanged hits do not repaint the
highlight or remeasure the hint. The VTE origin is cached until content or
scrolling changes.
Blank and unmatched cells have no fallback target. Turning Inspect off, Escape,
scrolling or redrawing clears feedback and invalidates pending clicks. Press
and release must resolve to the same target. No document state or shell config
is changed.

The opt-in `point_to_edit_real_vte_navigation` test exercises a real GTK window,
wrapping, scrollback and scene preservation. Set `TERMIMOCHI_POINTER_TEST=1`
under X11 to additionally use `scripts/preview-pointer-driver.py` for real
hover, opt-in/off behavior, blank-space safety, single-click, word selection and
drag-selection events (Python 3, libX11 and libXtst required). Hover regression
checks monitor actual frames during small pointer movements: the hint must not
move, disappear or repeatedly update its text, and the grid must stay still.
Long/short target names and a pending hover cancelled by turning Inspect off
are also covered. Run this separately from display-independent tests:

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
features are reported. There is no unsandboxed fallback. Importing and previewing
never write the original configuration or shell startup files.

`starship_draft.rs` backs **Your Starship**, which opens the current configuration
directly after loading, without a separate copy mode. Edits remain in memory.
`starship_modules.rs` declares the 15 reviewed editor
modules and their supported symbol/style fields; this is not an unrestricted
Starship config editor. `toml_edit` patches only the selected property in the full source
document, retaining the remaining settings, comments and layout. The full
document is exported, never the filtered sandbox input. Unsupported and custom
modules remain in the export but are not executed in the preview. Literal meta
symbols are escaped for Starship; directory/path substitution symbols remain
raw. Character symbols and Git status fields retain their format syntax,
including embedded styles and count variables. Controls/directional overrides,
unbalanced edited formats and malformed styles are rejected. Color changes
retain unrelated style attributes; version/layout choices are language-only.
Typed edits form undo groups; undo/redo restores the affected module/field.
Reset restores only the selected module to its loaded state, preserving other edits. Draft
history, invalid fields and unsaved changes participate in the toolbar and
close confirmation independently of Designer.

`starship_editor.rs` supplies the editing controls and basic-font/emoji/Nerd Font
shortcuts. The Nerd Font shortcut includes the selected module's actual preset
glyph, using the preview's font family and weight at a fixed specimen size.
The tooltip distinguishes primary-font support, fallback and missing glyphs;
missing glyphs show a warning icon instead of an unreadable box. Glyph checks
are cached by module, font and font-map revision. Viewing a specimen never edits
the draft; clicking applies the original preset with its spacing intact. The
opt-in `nerd_font_preset_preview_tracks_module_and_font` GTK test covers module
and font changes, missing glyphs, exact preset application and undo.
Modules without symbols, versions or standalone styles do not show
those controls. Multiple fields (Git statuses, user/root styles, success/error
characters) use explicit selectors. Invalid text blocks navigation, save and export
until repaired or undone; navigation by itself never edits the draft.
Font warnings live in the shared diagnostics report. A 220 ms edit
debounce and single-flight bounded worker render the full copied prompt through
the existing allowlist/sandbox; obsolete results cannot overwrite newer edits.
Optional Rust, Node.js, Python and Go samples mount a temporary project manifest
read-only at a stable sandbox path without creating files in the user's folder.
They use the installed toolchain, not fabricated version output. The copied
top-level format and detection rules still decide which modules are visible.
That prompt starts a simulated terminal session. `starship_scene.rs` generates
a command appropriate to the selected module/field, optional command output,
and the resulting complete prompt. Examples include entering a Python project,
staging Git changes, and a failed command showing the edited error character.
The original root format, palette and multiline structure are preserved; a
selected module omitted from that format is inserted for simulation only.
Disabled modules stay disabled. Context uses fixed example values, not live
versions, Git status, time or shell state. Unsupported expressions report a
simulation-only error instead of executing runtime expressions.

Only palette data and reviewed string fields enter the scene generator.
Generated configurations contain declarative `env_var` formatters with fixed
defaults, never imported runtime/custom modules or commands. Variable expansion
is bounded, rejects recursion/unknown names, and disallows injecting any other
root module reference. Starship renders it in the existing read-only/offline
sandbox and its output is ANSI-sanitized. This is an editing aid, not a claim
that the selected module is currently active in the user's shell.

Selection and edit changes share the 220 ms debounce/single-flight worker.
Unchanged starting prompts are cached. GTK captures an immutable draft snapshot;
scene generation and rendering run in the worker. Generation checks discard
stale starting prompts and transitions. Each frame contains an original/edited
pair built from the same module, fields, focus and deterministic context. The
original document is the configuration loaded when editing began, not a second
live shell probe. Original / Edited switches cached results without starting a
renderer, editing the document or appending a frame; scratch input and scroll
position are retained. Designer compares against its settings at session start.
The common transcript and initial prompt are identical in both views. A selection adds a command frame;
edits replace the active frame, with at most six retained frames. A new copy
clears history. Undo/redo restores the edited field and focus. Only the newest
simulated prompt in the selected comparison view participates in scene glyph diagnostics, with findings labelled
as simulated; obsolete glyphs in older command lines are not warnings.
No displayed command is executed, including Git, SSH or file-operation examples.
None of the temporary formats or example values enter the draft, export or user files.
`starship_file.rs` binds the draft to a size-limited disk snapshot (resolved path,
device/inode and exact bytes). Save Changes confirms the target, makes a private
unique backup beside it, then atomically replaces the file with preserved Unix
permissions. A changed/deleted/replaced source or retargeted symlink blocks save;
checks are repeated immediately before replacement. Existing symlinks are kept
and their resolved .toml target is updated. Read-only files, hard-link aliases,
non-.toml targets and shell startup aliases are refused. The full draft is saved,
never the sandbox-filtered or simulated configuration. A successful save updates
the disk baseline without discarding editing undo history.

Save As protects the active configuration, including symlink and hardlink aliases;
writing it requires Save Changes instead. Other existing destinations are backed
up before replacement. Saving separately does not switch the active shell config.
Restore Previous Version discovers the latest regular, parseable backup after
restart, confirms restoration, verifies both snapshots and backs up the current
file first. Restore updates the draft and can be undone in memory. Reload from
Disk requires confirmation for unsaved/invalid input and does not write files.
Background folder refreshes never replace an existing draft. In Prompt, toolbar
Save/Ctrl+S and Save As/Ctrl+Shift+S target the prompt rather than the theme;
Designer continues exporting a separate complete document. Set
`TERMIMOCHI_COPY_TEST=1` for the graphical test above to additionally exercise
editing controls, real rendering, undo, validation, superseded render results,
actual confirmation/cancellation, backed-up save, restore and external conflicts
(Starship and Bubblewrap required). Run at both `GDK_SCALE=1` and `GDK_SCALE=2`.

`prompt_diagnostics.rs` checks non-ASCII characters actually fed as prompt text,
not unused config symbols or a font-name heuristic. Unique characters are
bounded and attributed to a module only when the source has an unambiguous
match. Pango checks the selected face and available fallback fonts separately:
unknown glyphs are warnings; private-use icons supported only by fallback are
informational compatibility notes. Supported CJK/emoji fallback is normal and
does not produce a warning. These checks do not certify emoji sequence shaping
or alignment in a different terminal. Findings include code points, explain the
font context, and link to Typography or the affected module/field without silently
altering a symbol or the original configuration. Font/content changes refresh
the report and resolved findings disappear. A successful copy render keeps its
matching source for attribution while newer edits are pending. Coverage is
cached by font, font-map generation and character requirements; unchanged
reports retain their row widgets so caret/selection events do not cause churn.
The graphical copy test also covers real missing Rust/Python glyphs, supported
emoji, disabled modules, plain-text repair, module/field navigation, cross-module
history, independent resets, report actions and stable unchanged rows.

The Typography module uses installed monospace families and inspects the
selected Pango face directly for the Nerd icons shown in the specimen, without
accepting fallback-font coverage. The preview consumes the same 16-color model
as the linter. Its focusable scratch prompt echoes sanitized local input for
interaction and cursor testing. It has no shell/PTY child, so typing a command
never executes it; sample output is explicitly separate from live context.

Ptyxis `.palette` files do not contain typography settings. Typography is a
separate document with independent dirty state and Undo/Redo. On Typography,
Ctrl+S and Save Preset write a versioned `typography.termimochi-font.json` under
the XDG TermiMochi state directory, restored on the next launch. Without a saved
preset, initial values come from the read-only Ptyxis appearance snapshot.
The Open button imports portable presets into the preview; Export Preset writes
a copy without applying it. Reload Saved Preset recovers from external changes.
Unsaved typography participates in close and replacement confirmations.
Missing font families remain selected rather than being silently rewritten;
preview fallback is allowed, but applying a missing family is refused.

`typography_preset.rs` validates the document kind, version, exact fields and
numeric bounds before loading. Private, atomic writes reject symlinks, hard-link
aliases, read-only targets, oversized files and externally changed contents.
Existing presets are backed up before replacement. Font changes never dirty the
palette or Starship document, and ordinary preview/navigation never saves them.

`typography_apply.rs` explicitly writes only four allow-listed Ptyxis settings:
global `use-system-font` and `font-name`, and `cell-height-scale` and
`cell-width-scale` on the identified launching/configured profile. The confirmation
shows the global impact, profile label/identifier and before/after values. It
captures both effective and user-set values, preserving unset/inherited settings
for restoration. A durable private backup and recovery receipt precede all writes.
Each GSettings group is batched, then verified; failure attempts conflict-aware
recovery. External changes, removed profiles, unsupported values and locked keys
block writing. Repeated identical Apply preserves the existing rollback record.
Restore uses the recorded profile, not whichever profile is now the default,
and rechecks settings after confirmation. Desktop font, palette and shell startup
files are never written. Backend tests use their own schemas and a memory backend,
never the user's dconf database; GTK tests save only into temporary directories.

Layout is a separate document with its own baseline and Undo/Redo. Save Preset
and Ctrl+S on Layout persist all eight fields to
`layout.termimochi-layout.json` beside the typography preset; startup restores
it without marking the palette as modified. Open/export use the same suffix.
`document_store.rs` validates versioned documents before use, bounds input to
1 MiB, rejects symlinks/hard links and foreign destinations, detects external
edits and writes atomically with private backups.

`layout_apply.rs` only writes six global Ptyxis keys: `cursor-shape`,
`cursor-blink-mode`, `scrollbar-policy`, `default-columns`, `default-rows` and
`restore-window-size`. The confirmation discloses that these affect all
profiles, and disables remembered sizing so the chosen grid controls new
windows. Existing windows are not resized. Exact pixel padding, tab-bar
visibility and preview-window spacing are saved but not applied; native Ptyxis
does not expose equivalent settings for these controls. Typography, palette,
desktop and shell settings are untouched. One delayed GSettings batch is
verified after application; a separate layout receipt preserves unset values,
allows rollback after restart, and refuses conflicting external changes.
Tests use isolated schemas and an in-memory settings backend.

`workspace.rs` defines portable `.termimochi.json` complete setups. The common
save menu exposes Open Workspace, Save Workspace and Save Workspace As on
every module. A workspace contains the entire palette and active variant,
typography, layout, greeting settings, ordered Designer modules (including disabled modules),
lossless imported Starship text, and the active prompt source. It contains no
file destinations, profile IDs or terminal deployment actions. Whole-document
validation precedes all UI mutations. Imported Starship data reopens detached
from local files; Save routes to separate export and native reload/restore are
disabled. Missing imported content stays absent instead of being replaced by
asynchronous discovery of the host configuration. Preview rendering uses the
existing restricted renderer: custom commands are preserved as text, not run.

A complete workspace savepoint covers all five modules for close warnings,
without pretending any module's standalone file or actual terminal settings
were saved. Module shortcuts retain their contextual behavior. Workspace
files are opened explicitly, while typography/layout presets restore at launch.
The opt-in `layout_and_workspace_save_restore_and_safety` GTK/VTE test covers
contextual actions, live cursor settings, module history isolation, restart,
cancelled apply/discard, external conflicts, atomic validation, detached
Starship and asynchronous startup recovery. Run separately on 1x and 2x:

```bash
GDK_SCALE=1 cargo test -p termimochi layout_and_workspace_save_restore_and_safety -- \
  --ignored --test-threads=1
```

`window/preview_hint.rs` overlays a small, non-targetable resize hint on the
preview pane when its horizontal adjustment still overflows after a 500 ms
settle delay. It appears once per window, expires after seven seconds, and
dismisses on divider movement, horizontal panning, resolved overflow or unmap.
The overlay does not participate in size measurement or change terminal input,
grid sizing, document history or saved configuration. Timers and signal handlers
use weak widget references; fades respect reduced motion. The opt-in
`preview_fit_hint_is_once_nonblocking_and_dismisses_on_resize_pan_or_timeout`
GTK/VTE test covers its lifecycle, layout stability and native divider dragging
with `TERMIMOCHI_POINTER_TEST=1`; run separately at `GDK_SCALE=1` and `2`.

Greeting (`Ctrl+5`) is a separate editor in `window/greeting.rs`, backed by the
declarative model and renderer in `greeting.rs`. Its header switch defaults off;
the optional workspace field also defaults off for older documents. Presets
use `greeting.termimochi-greeting.json` beside the typography/layout presets.
Independent coalesced text history, contextual Open/Save/Save As, rollback to
the saved preset, external-change protection and close warnings follow the
existing document workflow. Invalid pasted artwork is never fed into VTE or
exported; undo discards the whole paste transaction rather than an intermediate
TextBuffer deletion.

Entering Greeting selects a fresh-terminal preview; its render state is separate
from the left module, so changing colors, fonts and layout retains that preview.
The opening output is followed by the selected prompt and harmless local scratch
input. Reset Preview Session returns to the previous scenario mechanism.
Inspect maps greeting output to its own editor. Rendering is coalesced and uses
the active ANSI palette and font. Unicode graphemes are clipped by cell width;
wide side-by-side artwork stacks above the information in narrow viewports.
Built-in art includes a TermiMochi mark based on the app icon, Terminal, and pinned
upstream Ubuntu/Arch/Debian/Fedora/Linux Mint artwork. The upstream MIT license
and provenance are also compiled into GResources. Fastfetch `$1`…`$9` palette
markers and `$$` escapes are decoded without changing geometry; user Custom text
does not use that substitution. Custom artwork is limited to 64 lines, 120 cells
per line and 16 KiB, with the same terminal-control and bidi protections.
Minimal card suppresses
art without deleting the selected/custom logo. GTK DragSource/DropTarget on the
field handles support stable insertion at the target's upper/lower edge, with
drop feedback and a single undo transaction; arrow buttons are the keyboard
alternative. External text drops and invalid drafts cannot reorder fields.
The optional `preview_columns` (0/80/100/120) overrides only the greeting VTE grid,
not the Layout document or the external terminal's dimensions. Fixed grids can
be panned. Greeting height can grow to retain complete artwork and is scrolled
by the outer preview pane. Official presets reserve at least 36 columns for
fields, stacking on narrower grids. Export snapshots that resolved placement
before opening the save dialog; it is a static layout, not runtime responsiveness.
Reset
Preview Session restores Layout sizing. Older exact eight-item lists migrate
by appending disabled GPU and Disk entries, preserving order and appearance.

The optional `opening` defaults to None. Fade, line reveal and shimmer run as
a non-targetable Cairo overlay on the existing VTE, using the GTK frame clock.
No animation frame feeds escape sequences or changes terminal allocation.
Redraw/typing, pointer interaction and reflow cancel the mask; generation checks
invalidate earlier runs. The 850 ms motion respects GTK reduced motion and can
be replayed explicitly. It is stored in presets/workspaces, not exported to
Fastfetch, which receives a static configuration.

The existing folder worker also takes a bounded snapshot of `/etc/os-release`,
Linux procfs kernel/CPU/memory/uptime fields, the inherited shell/terminal name
and local date. GPU names use bounded DRM sysfs entries and the local PCI ID
database (falling back to numeric PCI IDs); the root disk uses GIO filesystem
size/free attributes. No process, network connection, user script, config file or
Fastfetch plugin is launched for these fields. Missing facts say Unavailable.
Values are refreshed explicitly, not polled on the UI thread; live Fastfetch
values, terminal detection, font fallback and some formatting may differ.

Fastfetch export follows the upstream [configuration schema](https://github.com/fastfetch-cli/fastfetch/blob/dev/doc/json_schema.json).
Custom design emits only ten reviewed system modules, literal Custom text and `data-raw`
artwork are emitted. Literal opening braces are escaped against Fastfetch's
format parser; artwork dollar/color placeholders remain literal. No Command
module, external art path or shell startup hook is emitted. GPU exports names;
Disk explicitly targets `/`, matching the snapshot. ANSI slot references and
`brightColor: false` preserve theme inheritance. Exports allow `config.jsonc` or
the separate `.fastfetch.jsonc` suffix, with checked atomic writes and private
backups. An existing destination requires a second Back Up & Replace dialog;
its bytes are captured before confirmation and rechecked before writing. JSONC
comments are preserved in backups without executing or importing the file.
Shell startup filenames, symlinks and hard links are excluded. Arbitrary
Fastfetch configs are not imported or executed in the first version.

`greeting_official.rs` adds five pinned, audited Fastfetch 2.57.1 presets:
Neofetch, Screenfetch, Paleofetch and examples 8/9. `official_preset` defaults to
None and `official_items` defaults empty for existing documents. Each official
item identifies an exact module array index plus its enabled state: module
objects, duplicate types, decorative rows and formats are retained rather than
mapped lossily to the custom ten-field catalog. Validation requires every
original ID exactly once. The dynamic GTK field list supports switches, arrows,
drag insertion, single-transaction Undo/Redo and portable preset/workspace saves.
The custom field list is retained separately while exploring official presets.

Only compile-time bundled JSONC is parsed; arbitrary user Fastfetch documents,
commands, image paths and network-enabled examples cannot enter this execution
path. Official fields run through `/usr/bin/fastfetch` in `/usr/bin/bwrap` with a
read-only root, private temporary directory and PID namespace, no network, cleared
environment and no unsandboxed fallback. The shared concurrent-pipe helper limits
stdout to 32 KiB and runtime to 2.2 seconds, kills/reaps timeouts, and reports failures.
Module SGR output is sanitized before grapheme-aware clipping and composition
with the bundled logo. Desktop/terminal detection and read-only disk flags are
sandbox observations, explicitly disclosed in the UI; module detection errors
are shown. Export keeps the original module formats and runs normally in the user's
terminal. Official module requests are coalesced into at most one running worker;
source-key checks prevent stale preset results replacing newer edits. Palette/font
changes, widths and artwork edits reuse the result without launching another probe.

`greeting_editor_preview_persistence_and_workspace_safety` is an opt-in GTK/VTE
test covering controls, widths, motion/reduced motion, Unicode text, invalid-paste undo, module
isolation, cancelled discard/reload, save conflicts, restart and Workspace
round trips. Set `TERMIMOCHI_GREETING_DRAG_TEST=1` for real X11 pointer dragging
and its undo/redo assertions. Run it separately at 1x and 2x. The additional opt-in
`real_fastfetch_accepts_export_and_preserves_literal_text` test checks exported
configurations using a real executable (override with
`TERMIMOCHI_FASTFETCH_TEST_BIN`). It verifies all artwork positions and ensures
environment-variable-like welcome text stays literal. Neither test changes
the user's shell startup files or terminal settings.
`upstream_logo_geometry_matches_real_fastfetch` compares all five decoded logos
against actual built-in Fastfetch output, including Debian dollar escaping.
`real_official_presets_render_offline_and_remain_terminal_safe` exercises all five
presets in the sandbox. `official_greeting_presets_preserve_fields_history_workspace_and_full_art`
adds GTK/VTE checks for all official fields, quick switching, drag undo/redo,
80/100/120 columns, expanded canvas, preset restart and Workspace restoration.

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
