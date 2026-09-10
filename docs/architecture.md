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
The terminal's scene/comparison dropdowns are transparent at rest, with subtle
hover/open feedback. Its viewport suppresses GTK undershoot/overshoot decorations
so the enclosing AdwToolbarView cannot draw an interior seam after scrolling;
the terminal shell's exterior shadow remains unchanged. The opt-in
`terminal_scroll_edges_and_idle_controls_blend_into_both_themes` test checks
rendered edge pixels and idle button transparency in light/dark palettes under
the same toolbar ancestor. Run it separately at 1x and with `GDK_SCALE=2`.

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

`window/output_bar.rs` owns one persistent contextual output area below the
left inspector: preset/workspace Save, the module's reviewed Apply/Install or
Export, and secondary commands. Module bodies no longer repeat these actions.
The preview owns transient toasts so they cannot cover the left output bar.
`window/color_targets.rs` resolves Designer prompt tones and Greeting global /
per-field roles to existing palette slots, including VTE's bold-to-bright mapping. It
reuses palette history; independent imported styles are preserved and routed
to their source editor rather than approximated. Repeated native fields retain
their source indices; weak button references select only the mapped field list.
Your Starship role navigation selects the exact reviewed module and style field
without changing either the source or the retained preview transcript.
See [workbench design](workbench-design.md).

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
Background folder refreshes never replace an existing draft. In Prompt,
Save/Ctrl+S and Save As/Ctrl+Shift+S target the workspace, not external Starship.
Apply and Export retain the reviewed source-write workflow; Designer continues
exporting a separate complete document. Set
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
1 MiB by default (24 MiB for Greeting/Workspace embedded image sources), rejects symlinks/hard links and foreign destinations, detects external
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
Built-in art includes a compact 38 × 12 printable-ASCII TermiMochi mark based on the
app icon (letter/punctuation texture with a negative-space terminal chevron and underscore), Terminal, and pinned
upstream Ubuntu/Arch/Debian/Fedora/Linux Mint artwork. The upstream MIT license
and provenance are also compiled into GResources. Fastfetch `$1`…`$9` palette
markers and `$$` escapes are decoded without changing geometry; user Custom text
does not use that substitution. Custom artwork is limited to 96 lines, 160 cells
per line and 16 KiB, with the same terminal-control and bidi protections.
Minimal card suppresses
art without deleting the selected/custom logo. GTK DragSource/DropTarget on the
field handles support stable insertion at the target's upper/lower edge, with
drop feedback and a single undo transaction; arrow buttons are the keyboard
alternative. External text drops and invalid drafts cannot reorder fields.
The optional `preview_columns` (0/80/100/120) overrides only the greeting VTE grid,
not the Layout document or the external terminal's dimensions. Fixed grids can
be panned. Greeting height can grow to retain complete artwork inside the
terminal's own viewport, without enlarging the surrounding pane. Live Preview
controls and diagnostics remain fixed; the editor and diagnostics scroll separately.
`window/preview_scroll.rs` combines VTE history units and canvas pixels into one
local adjustment used by the optional Layout scrollbar. A capture-phase controller
contains wheel/touchpad gestures even at boundaries and handles Shift+wheel panning.
The VTE pixel-unit option is respected; surface deltas retain fractional precision.
Typing queues cursor visibility after asynchronous VTE feeds; deliberate scrolling
cancels that reveal, and Original/Edited comparison restores the local position.
Vertical canvas changes invalidate Inspect hit targets and stop opening motion.
Real X11 wheel tests cover tall artwork, history, edge containment, horizontal
panning and fixed chrome at 1x/2x. Official presets reserve at least 36 columns for
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
Shell startup filenames, symlinks and hard links are excluded. This export path
is separate from the explicit, restorable apply workflow described below.

`greeting_official.rs` adds five pinned, audited Fastfetch 2.57.1 presets:
Neofetch, Screenfetch, Paleofetch and examples 8/9. `official_preset` defaults to
None and `official_items` defaults empty for existing documents. Each official
item identifies an exact module array index plus its enabled state: module
objects, duplicate types, decorative rows and formats are retained rather than
mapped lossily to the custom ten-field catalog. Validation requires every
original ID exactly once. The dynamic GTK field list supports switches, arrows,
drag insertion, single-transaction Undo/Redo and portable preset/workspace saves.
The custom field list is retained separately while exploring official presets.

TermiMochi's own `resources/termimochi-greeting.jsonc` uses that same full native
pipeline: 19 configurable modules including title/separator, host/packages,
desktop/display, hardware, memory/disk and a color strip. Its compact ASCII logo
follows the existing brand vector silhouette and negative-space `>_`, with visible
`o/l/c` texture and punctuation contours like the upstream ASCII artwork, not
Unicode blocks or Braille. A regression test restricts the asset to printable
ASCII plus line feeds; cell geometry and native export remain identical. The
legacy `official_preset`/`official_items` field names are retained for document
compatibility; the brand preset appends stable ID 6 (`termimochi`) while upstream
IDs 1–5 remain unchanged. Display order is independent: TermiMochi comes first.
`GreetingSettings::starter()` selects it at 100 columns, disabled until enabled,
only for new editor sessions. `Default`/legacy deserialization and saved custom
documents are unchanged. Brand and upstream presets share toggles, drag ordering,
field editing, width handling, exports and Undo/Redo. The opt-in GTK test
`termimochi_brand_starter_preview_and_saved_custom_preservation` checks 80/100/120
columns, restart/save behavior and preservation of existing custom greetings.

Native preview accepts generated designer configurations or a restricted
projection of imported JSONC, never the complete imported document. Command
modules, user-selected image paths and network-enabled examples cannot enter
this execution path. Fields run through `/usr/bin/fastfetch` in `/usr/bin/bwrap` with a
read-only root, private temporary directory and PID namespace, no network, cleared
environment and no unsandboxed fallback. The shared concurrent-pipe helper limits
stdout to 352 KiB (320 KiB artwork plus 32 KiB information) and runtime to 2.2
seconds, kills/reaps timeouts, and reports failures.
Module SGR output is sanitized before grapheme-aware clipping and composition
with the bundled logo. Desktop/terminal detection and read-only disk flags are
sandbox observations, explicitly disclosed in the UI; module detection errors
are shown. Export keeps the original module formats and runs normally in the user's
terminal. Official module requests are coalesced into at most one running worker;
source-key checks prevent stale preset results replacing newer edits. Palette/font
changes, widths and artwork edits reuse the result without launching another probe.

`greeting_fields.rs` defines validated, serde-default `FieldStyle` overrides:
literal label/inline icon, ANSI key/output color slots and a reviewed format
catalog. IDs address custom field identities, pinned preset/index pairs or
imported array indices, so duplicates remain distinct and reset is lossless.
`window/greeting/fields.rs` attaches one reusable popover to field-name buttons.
Atomic entry notifications avoid committing the intermediate deletion during a
paste; invalid drafts block save/apply and cannot be hidden by editing a color.
Styles use the same generated module objects for native preview and export.
Unstyled custom designs retain the lightweight snapshot path. Native rendering
requires system Fastfetch 2.x >= 2.57 and Bubblewrap; a failed version/dependency
check never falls back to executing the imported file or an unsandboxed process.

`fastfetch_document.rs` uses jsonc-parser's lossless CST. It enforces 512 KiB,
32 nesting levels, 128 modules, strict JSON syntax except comments/trailing
commas, unique object keys and bounded ASCII module type names. Batch field
edits modify only selected scalar properties, retaining comments, whitespace,
unknown properties and all untouched modules. `imported_source` and overrides
are portable document data; an external write target is not serialized.
The safe preview projection allows reviewed local module types, bounded scalar
formats, explicitly named built-in/small logos and bounded text artwork with SGR only.
Disk probing is pinned to `/`. Commands, network/custom modules, imported file
paths and unsupported display/general options are omitted and reported, while
export/apply retains them. Imported layout is read-only rather than pretending
that designer layout controls can represent arbitrary upstream configurations.

`greeting_official/layout.rs` resolves native Fastfetch's relative cursor moves
and horizontal positioning into a bounded 240-column / 256-row static grid.
Simply deleting those moves puts module text below or inside the already-painted
logo. The worker now caches typed drawing operations, replayed at the current
preview width (including right-aligned logos) without launching another process
on resize. Grapheme cells and compact reviewed SGR state preserve wide glyphs,
color and transparent padding; overflow clips instead of wrapping into fields.
Only text, CRLF and SGR leave the compositor. Imported artwork still cannot
introduce cursor controls, and native OSC/DCS, erasure and mode changes are not
forwarded to VTE. GUI regression coverage checks actual VTE rows at 80/100/120
columns, left/right/top artwork, scrolling and window resize at 1x/2x.
Custom designer exports also derive their 1-based value-column stop from the
edited labels' cell widths, including icons and separators, so long labels are
not overwritten by values. Existing imported files are not silently rewritten.

`greeting_art.rs` bounds visible UTF-8 text to 16 KiB, 96 rows and 160 terminal
cells per row, with a separate 320 KiB input/normalized ANSI budget for per-cell
image colors. Incremental expansion checks prevent newlines/styles from growing
beyond these limits. An SGR whitelist retains 16/256/RGB foreground/background colors,
bold/dim, italic, underline, reverse and strike. OSC/DCS/APC/C1 controls, cursor
movement, erasure, blink/conceal and bidi controls are removed with a notice;
this deliberately is not an ANSI screen emulator. Tabs expand at 8-cell stops,
CRLF/BOM normalize, and style state resets/reopens at line boundaries for safe
composition. Serialized ANSI/plain pairs are revalidated on load. GTK edits
plain text only; removing imported styles is explicit and undoable.

Static SVG sources share the editable image-source envelope and conversion UI.
`greeting_image/svg.rs` preflights XML in a disposable renderer process, then uses
resvg with both image resolvers disabled. The private executable entry point runs
before GTK initialization. Bubblewrap mounts only runtime/font directories, the
executable and captured input, without network or home/project access; prlimit
and the bounded parent pipe reader enforce memory/CPU/time/output limits.
Premultiplied RGBA is returned at a maximum 1024-pixel longest side and reused
by the existing conversion worker. Only final character art enters Fastfetch
exports. See `image-conversion.md` for supported SVG features and explicit limits.

Fastfetch `data` and `file` decode `$1`–`$9` and `$$`; raw types keep dollars
literal. Sanitized marker text and explicit safe color slots are passed as `data`
so native host-default colors (including unset slots) are preserved, not guessed.
Marker source tabs follow Fastfetch's four-space expansion. Built-in and small
types remain distinct. Reviewed padding/position,
printRemaining and color slots are projected. At explicit config import only,
literal regular text files contained inside the canonical config directory are
snapshotted (no leaf symlink/hardlink, special files or word expansion). Preview
passes embedded sanitized text, never a filesystem source, to Fastfetch. The
saved snapshot/descriptor survives portable presets but mismatches are rejected.
Relative resolution differs from Fastfetch's CWD semantics and is reported;
original config/path/controls remain untouched. Explicit artwork import replaces
only the logo type/source through the lossless CST, embedding safe `data-raw`.
Exports of just the logo use checked TXT/ANS writes, extension guards and backups;
native built-in logo export runs on a bounded worker without system modules.
Tests cover encoding, widths, controls, snapshots, aliases/FIFOs, conflicts,
multiline color, actual Fastfetch projection, GTK cancel/undo and restart.

`greeting_image.rs` converts local PNG/JPEG/WebP, static SVG and a GIF still into
that same `Artwork` type. Four raster `image` codecs are enabled; SVG uses the
isolated renderer described above. APNG and animated WebP remain deferred.
Reads are bounded to
16 MiB and reject non-regular files, leaf symlinks/hardlinks and file races.
Magic bytes select the decoder. RIFF chunk bounds are checked before WebP
metadata reads. Before pixel decoding, dimensions are limited to 8192 per side,
16 million pixels and 64 MiB of decoded pixel data; decoder allocation limits
also apply where supported (not a total process-memory guarantee).
EXIF orientation is honored, alpha is premultiplied before resampling, and the
decoded snapshot is reduced to a maximum 1024-pixel side for repeated conversions.
The grid preserves source aspect using the current VTE cell ratio, within
160 columns, 96 rows and 15360 cells for ASCII; Detail/Half blocks keep the
120-column, 64-row, 3072-cell budget. The default ANSI Detail mode samples 8×8
subcells and fits procedural quadrant, eighth-block and diagonal masks by
weighted RGB error with transparency penalties. Common shapes win ties, and
inverse transparency cannot accidentally draw the terminal default foreground.
Half blocks still encode two vertical samples per cell. ASCII samples 4×4
subcells, combines Sobel/non-maximum-suppression/connected hysteresis contours
with tonal fill, suppresses neutral paper noise and strengthens pastel ink.
The selected mode chooses balanced, contours-only or tone-only output. Cropping,
outer-margin trim, exposure, contrast, saturation and smoothing precede rendering.
See [image-conversion.md](image-conversion.md) for independent implementation
details, limitations and public algorithm references.
This is intentionally a stylized foreground-only rendition; only Detail/Half
blocks aim to preserve source RGB colors. Invisible RGB cannot bleed into edges.
Fully transparent images are rejected; partial alpha uses the captured terminal
background. Every generated result passes the existing SGR/geometry validator.

`window/greeting/image_import.rs` owns a cancelable modal with a read-only VTE
preview, maximum-width/style/density controls and one coalescing worker. A
same-size Original/Converted switch uses a premultiplied RGBA thumbnail from the
already decoded snapshot; no repeated file reads or terminal graphics protocol
are introduced. The Import Artwork action is a visible,
labeled button, separate from the secondary Export / Edit menu. The
ASCII width defaults to 64 and is remembered separately from symbol width.
An independently scrolling sidebar contains four adjustment recipes, color and
structure modes, tone sliders and a crop expander. Reset retains width and style;
manual edits mark the recipe Custom. The original-color reference follows the
same crop and generation as converted artwork. `greeting_image/source.rs` retains
the bounded original image and a versioned conversion recipe beside final ANSI.
Canonical base64 serialization omits the original path; image bytes are shared
with Arc across Undo/Redo. All numeric recipe fields validate before use, and
full decoding happens only in the bounded image worker. Source metadata is not
stripped, so sharing a preset/workspace also shares that data. External Fastfetch
exports never include the source or recipe. Edit Artwork restores controls and
captured cell/color parameters; replacement with unrelated artwork or plain text
detaches the source. Legacy images offer an explicit reimport rather than a
fabricated reconstruction. Source, recipe and output commit in one history step.

`greeting_image/animation.rs` preflights GIF containers before bounded frame
decompression/compositing. A serialized worker pipeline unites crop/trim bounds,
locks the background key, applies edits, and encodes/re-decodes the GIF so the
pixel preview matches export quantization. Data budgets and cooperative deadlines
are documented in `image-conversion.md`. Ordinary ANSI uses the first visible
composited frame. The existing editable source envelope preserves all GIF bytes.

`greeting_image/pixel_export.rs` creates fresh private export bundles without
touching active configuration or documents. Static PNG uses kitty-direct;
animation uses a generated self-contained Kitty stream through Fastfetch's raw
logo interface. Explicit full-frame replacement avoids icat transparent-frame
ghosting. The stream has bounded 4096-byte Base64 payload chunks, integer-only
controls, quiet responses and fresh image-number allocation; no imported control
streams or external file-transfer paths enter it. Animated bundles also include
the processed GIF, PNG still and unchanged ANSI fallback configuration.
`window/greeting/pixel_export.rs` provides opt-in playback, pause and frame seeking
without modifying source/recipe/history. Main VTE rendering and Apply stay ANSI.

`greeting_image/sixel.rs` generates bounded transparent Sixel directly, bypassing
the Fastfetch/ImageMagick alpha-loss path found by real xterm screenshots. It
resamples associated alpha into the captured physical cell geometry, thresholds
alpha, uses exact colors or a sampled NeuQuant palette, then emits run-length
encoded six-row color planes with P2=1. No imported protocol input is parsed or
forwarded. Source/raster/byte/time bounds are independent, and failed generation
creates no export directory. `logo.sixel` uses Fastfetch's raw logo interface;
the bundle retains `logo.png` and an unchanged ANSI fallback configuration.
Raw raster dimensions require re-export for different font/DPI settings.

`greeting_image/background.rs` adds opt-in color-key removal before tone edits
and resampling. Automatic mode requires a near-uniform opaque original border;
manual mode uses validated HEX or a pixel picked from the original-color crop.
Bounded iterative four-connected selection preserves enclosed same-color regions
by default; global removal is explicit. Tolerance and smoothstep softness only
reduce alpha, with matte decontamination and premultiplied RGB bounds. This is
not semantic segmentation. No-match/ambiguous detection disables acceptance;
invalid controls invalidate pending generations, and blank output is rejected.
The Picture picker accounts for centered Contain letterboxing in logical pixels;
Esc cancels picking before closing the dialog. Recipes preserve removal controls,
while Reset clears them. Source bytes remain untouched and no image recipe is saved.
Fit preview adjusts the VTE font only, based on viewport allocation changes;
it never reruns conversion or changes exported geometry. Full-size inspection
remains scrollable, and both reference/converted views use the same canvas.
The saved-status message explicitly distinguishes in-app presets from applying
Fastfetch. A high-priority save toast offers the existing review action; opening
or canceling that review never writes the external config. The
worker decodes once; generation IDs ignore stale results and a bounded output
channel prevents accumulating converted drafts. Apply stays disabled while the
current request is pending or invalid. Closing drops the channels. Accepting
rechecks the captured Greeting settings and validates both Fastfetch and preset
budgets before one undoable replacement. Only sanitized ANSI text is persisted,
never source pixels/paths. Existing imported fields/comments remain intact.
Native Fastfetch output allows 320 KiB of art plus 32 KiB of information; other
preview subprocesses retain their smaller output bounds. Fastfetch exports and
rollback receipts use the enlarged document budget consistently. Tests generate
PNG/JPEG/WebP, EXIF, APNG and malformed fixtures in memory, cover alpha/aspect/
resource limits and large portable round trips, and exercise cancel/coalescing/
stale edits/undo/save/reopen with real GTK/VTE and sandboxed Fastfetch at 1x/2x.

`window/greeting/fastfetch.rs` exposes Load Current, Import, Review & Apply and
Restore Previous. The collapsed Compatibility report distinguishes runtime,
offline detection, Pango missing/private-use fallback glyphs and retained-but-
unsimulated source settings. Actionable runtime/font issues also join existing
Preview Checks. Full read-only Before/After panes show the explicit destination;
Cancel is initially focused, and both the draft and destination bytes are
rechecked when accepting. The standard path follows the GLib/XDG config directory,
preferring config.jsonc over config.json. Shell startup files are never targets.

The separate `greeting_startup.rs` opt-in is the only Greeting action allowed to
edit `.bashrc`. `window/greeting/startup.rs` presents a default-off switch and
an explicit review of its exact managed block, config path and execution risk.
Portable documents cannot restore this authority. Checked writes retain backups,
reject concurrent changes/aliases/read-only targets and check the applied config
again before enabling. Removal recognizes the exact versioned block, preserves
outside edits, and refuses modified or duplicate markers. Existing unmanaged
Fastfetch/Neofetch references block enabling to avoid double output. The Bash
guard skips non-interactive/non-TTY, SSH, tmux and shell-parent sessions, and
uses a non-exported once-per-shell flag. Configuration paths are single-quoted
with embedded quotes escaped; no imported shell code becomes part of the hook.
Fastfetch itself can execute retained config commands after this explicit opt-in.

Successful apply (including an unchanged target) also persists the accepted
Greeting via `DocumentStore`, advancing its baseline only after a checked local
save. A local conflict cannot undo the external apply or be mistaken for complete
success: a persistent notice offers Retry Save. Cancel and failed external apply
never save the preset. Save Preset itself remains local-only.

`window/greeting/sync.rs` coalesces read-only comparisons on startup, refocus and
Greeting edits. Bounded reads and JSONC comparison run off GTK, with at most one
worker per window and stale-result rejection. Explicitly loaded targets take
precedence over the local last-apply receipt and the default path. Receipt
discovery never restores a portable preset's write authority; applying still
requires explicit review. Comparison ignores comments, formatting and `$schema`,
and accepts the designer's adaptive stacked export. Load Applied reads fresh,
rechecks the file and draft after confirmation, loads without executing imported
modules, then saves the local preset. It never writes external Fastfetch. Unsaved
or invalid drafts require confirmation even if a workspace was saved earlier.

`fastfetch_apply.rs` writes private backups and a checked durable rollback receipt
under `state_directory()/fastfetch-state`. Apply prepares rollback before an
atomic write. A failed target write restores the previous receipt; a finalization
error after replacement keeps the new receipt and explicitly reports that the
new bytes are present. Restore checks both receipt and target, backs up current
bytes, then restores the exact original or removes only the exact newly created
config. All writes reject symlinks, hard links, readonly files and detected
concurrent changes. Unsupported imported settings may execute later when the
user invokes Fastfetch externally; the review dialog explicitly discloses this.

`fastfetch_run.rs` implements the separate post-apply action. The review checkbox
defaults on for designer configurations and off for imported documents. Only a
successful apply may launch; unchanged configurations can be explicitly reviewed
and run again. Restore never launches. The target is rechecked before opening a
new Ptyxis window. A fixed bash wrapper passes the absolute config path as `$1`,
never interpolates it into shell code, clears BASH_ENV/ENV and skips shell startup
files. It runs system Fastfetch once, shows failure status and waits for Enter.
The external run intentionally uses normal host detection and may execute retained
imported commands after opt-in; it is not the restricted in-app preview renderer.
No commands are injected into existing tabs, no startup hooks are installed and
launch failure cannot roll back an otherwise successful configuration write.
GTK apply tests record launch requests without touching the user's terminal;
opt-in real launcher tests use temporary configs, an isolated X display and a
private D-Bus session.

`window/greeting/art_preview.rs` renders artwork in a read-only VTE thumbnail,
with the active palette and typography. Width/height-driven font fitting and
centering preserve the entire character grid without changing the stored ANSI.
The plain text editor remains separate; imported text snapshots and colored
custom artwork use the thumbnail. Clicking expands a read-only window with Fit
and full-size scrolling. Every queued VTE frame clears/homes before feeding, so
old generations cannot leave remnants. A full-width import button and compact
overflow menu keep the toolbar within the editor column.

`greeting_field_editor_import_review_apply_restore_and_invalid_input` is a separate
opt-in GTK test for popover lifecycle, live output, undo/redo, invalid drafts,
font notices, lossless import, command non-execution, cancellation, late file
conflicts, apply, portable save/workspaces and restart/restore. Run at 1x and 2x;
`TERMIMOCHI_FIELDS_SCREENSHOT` optionally captures the field editor. The native
`reviewed_field_formats_render_without_unresolved_placeholders` test checks the
entire reviewed format catalog using offline Fastfetch. Pure tests cover bounded
JSONC parsing, retained comments, omitted unsafe preview settings, failed second
applies, maximum-sized receipts and exact rollback behavior.

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
