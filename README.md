# TermiMochi

> Make terminal themes cute **and** usable.

TermiMochi is a native Rust + GTK4/libadwaita workbench for terminal palettes
and Starship prompts, with a terminal greeting designer.
It treats colors as semantic relationships and checks how real terminal apps
combine them, instead of judging a theme only by its swatches.

The first target is the Codex composer contrast trap in a light Ptyxis theme:
Codex can draw composer text with `Foreground` on a `Color0` surface. A palette
that looks balanced elsewhere can therefore become almost unreadable.

## Current MVP

- Pure Rust, UI-independent palette core.
- Strict Ptyxis `.palette` parsing with unknown-property preservation.
- WCAG relative luminance and contrast ratios.
- General terminal and Codex-specific diagnostics.
- Read-only Ptyxis appearance snapshots: palette, font, cell spacing, cursor,
  configured grid, scrollbar policy and character-width behavior, with an
  explicit launching/default-profile source and fallback details.
- Light/dark variant editor with an embedded HSV picker and exact HEX/RGB input.
- Document-wide undo/redo with coalesced drag and text-edit transactions.
- VS Code-style Activity Rail for switching the left-side Palette (`Ctrl+1`),
  Typography (`Ctrl+2`), Layout (`Ctrl+3`), Prompt (`Ctrl+4`) and Greeting (`Ctrl+5`) modules while
  the right-side Live Preview and diagnostics remain mounted.
- Live VTE typography controls for installed monospace families, point size,
  weight, line height and cell width, with fallback-safe Nerd icon checks.
- Typography presets: **Save Preset** (or `Ctrl+S` on Typography) remembers
  all five font settings for the next launch. Open and export portable
  `.termimochi-font.json` files; typography edits have their own Undo/Redo.
  **Apply to Ptyxis** confirms the app-wide font and target-profile spacing,
  saves a private backup, and blocks external-change conflicts. The Typography
  save menu offers **Restore Previous Typography**. Saving alone never applies.
- Live layout controls for content padding, rows and columns, cursor behavior,
  tab and scrollbar chrome, and preview-window spacing.
- Layout presets: **Save Preset** (`Ctrl+S` on Layout) restores all eight
  settings next launch, with independent Undo/Redo and portable
  `.termimochi-layout.json` import/export. **Apply to Ptyxis** backs up and
  applies global cursor, scrollbar and new-window grid settings; exact padding,
  tab bar and preview-window spacing remain preview-only. The save menu offers
  **Restore Previous Layout**. Remembered window sizing is disabled only after
  explicit Apply confirmation, so the chosen grid can take effect.
- Complete setups: the top save menu's **Complete Setup → Save Workspace**
  stores colors, typography, layout, greetings, Designer modules and imported Starship
  text in one `.termimochi.json` file. Use **Open Workspace…** (or launch
  `termimochi my-setup.termimochi.json`) to restore it. Opening never applies
  terminal settings. Workspace prompts reopen detached; export them with
  **Save As** instead of overwriting this machine's Starship configuration.
  Module shortcuts still save their own files; workspace files are explicit,
  not automatically reopened on startup.
- Greeting starts with **Preset → TermiMochi** selected for new users; turn on
  the header switch to preview it. This complete native preset combines the
  brand mark, user/host title, separator, system/desktop/hardware fields and a
  bottom color strip. It defaults to a 100-column canvas and stays side-by-side
  at 80/100/120 columns. Existing saved custom presets are not replaced; select
  **TermiMochi** explicitly to switch, with Undo/Redo and Save Preset available.
  Choose **Preset → Neofetch** for a complete Ubuntu welcome page.
  Five pinned upstream presets are also included:
  **Neofetch, Screenfetch, Paleofetch, Icons (Example 8), Bars (Example 9)** from
  [Fastfetch 2.57.1](https://github.com/fastfetch-cli/fastfetch/tree/2.57.1/presets).
  All original fields, decorative rows and module formats are retained, including
  host, packages, resolution and desktop information. Every field can be toggled
  or reordered with drag handles/arrows. Switching presets is undoable; **Custom
  design** returns to the separate basic field controls. Network-, command- and
  external-image-dependent upstream examples are not offered in this collection.
- Greeting designer: enable the header switch to combine a welcome message,
  full-size **Ubuntu / Arch Linux / Debian / Fedora / Linux Mint** upstream artwork,
  a compact **38 × 12** ASCII TermiMochi mark based on the app icon, Terminal art or custom text.
  The TermiMochi mark uses visible `o`, `l`, `c` and punctuation textures rather than solid Unicode
  blocks. The brand silhouette and negative-space `>_` remain recognizable.
  The basic designer offers ten optional
  system-information fields: OS, Kernel, Shell, Terminal, CPU, GPU, Memory,
  root Disk, Uptime and Date. Drag the field handles to reorder; arrow buttons
  remain available for keyboard use, with independent Undo/Redo. Choose horizontal,
  vertical, right-side artwork or a borderless minimal card. ANSI accents inherit
  the current terminal palette instead of baking in theme colors.
  The fresh-terminal preview shows the greeting once,
  followed by the current prompt; switching to other editors keeps it mounted.
  Use **Preview Source → Reset Preview Session** to return to command scenarios.
  Official shapes retain their original character geometry (Ubuntu is 43 × 20
  cells); palette markers are decoded and recolored with the selected ANSI accent.
  Custom art supports **64 lines × 120 cells, up to 16 KiB**, including CJK,
  emoji and kaomoji, with bounded grapheme-aware clipping
  and a stacked fallback in narrow previews. Text controls and invalid artwork
  are rejected, with independent Undo/Redo and unsaved-change protection.
- **Artwork Files → Import Artwork…** accepts UTF-8/ASCII `.txt` and `.ans`
  logos. ANSI imports retain 16/256/RGB colors and supported text styles; cursor
  movement, screen clearing, clipboard/title commands, links and unsupported
  controls are filtered and reported in **Compatibility**. Tabs expand to
  8-column stops. Legacy CP437 artwork needs conversion to UTF-8 first.
  Colored imports show a read-only plain-text editor; **Edit as Plain Text**
  removes colors for manual editing, with Undo to restore them. **Export Plain
  Text… / Export ANSI…** exports only the logo, not machine information.
  Overwrites require confirmation, retain a backup and reject external changes.
  PNG/SVG conversion and Kitty/Sixel image logos are not supported yet.
- Click a system field's name to edit its **label, inline icon, name/content
  colors and display format** in a compact popover. CPU/GPU summaries, memory
  and disk percentages/bars, and date/time formats share the same native
  Fastfetch configuration in preview and export. **Reset Field** restores the
  original; edits support Undo/Redo and portable preset/workspace saves.
  **Compatibility** expands only when needed: it reports unavailable native
  rendering, offline detection differences, missing icon glyphs and imported
  settings that are retained but not simulated.
- **Fastfetch Configuration → Load Current** reads the standard user
  `fastfetch/config.jsonc` (or existing `config.json`); **Import…** selects another
  `config.jsonc`, `config.json` or `.fastfetch.jsonc` file. Imports retain JSONC
  comments, formatting, duplicate module types and unrecognized options.
  Supported fields can be edited; other modules remain read-only. Imported
  layout settings stay in the source, so designer layout controls are hidden.
  Preview uses a restricted local projection, never the entire imported file:
  command/network/custom modules and external image logos are not run.
  Unsupported display settings, paths and modules are listed in Compatibility.
  Text logos support `builtin` / `small`, `data` / `data-raw` and local `file` /
  `file-raw`, including `$1`–`$9` color slots and `$$` escaping in non-raw sources.
  On config import, literal text-file references **inside the config directory**
  are read once into a portable preview snapshot. Path expressions, aliases and
  outside references are refused; no path is passed to Fastfetch for expansion.
  This preview resolves relative paths from the config directory, while Fastfetch
  itself uses its working directory. The original path remains in config exports.
  Use **Import Artwork…** to replace it with embedded, sanitized `data-raw` text
  for portable Fastfetch output. Original imported ANSI controls remain in the
  source until this explicit replacement; the preview alone does not clean it.
- **Review & Apply…** shows the exact destination and Before/After text, then
  requires **Back Up & Apply**. External changes block replacement. **Restore
  Previous…** restores the last successful apply, including after restarting
  TermiMochi; backups and the checked rollback record live under
  `XDG_STATE_HOME/termimochi/fastfetch-state`. Restore also checks for external
  edits and retains a recovery copy. Imported unsupported settings remain in
  the applied file and can run when **you** invoke Fastfetch; review them first.
  Loading, saving a TermiMochi preset and applying a Fastfetch file are separate
  actions. None edits `.bashrc` or enables automatic shell startup.
- Greeting width can follow Layout or use an exact **80 / 100 / 120 columns**,
  without changing the Layout document. Pan wider grids with Shift+wheel.
  A short, once-per-window hint appears when the preview is too wide: drag
  the center divider or widen the window for more room. It dismisses on resize/panning or
  after seven seconds, without interrupting terminal input or selection.
  Official layouts stack when there is insufficient room for readable fields.
  Taller artwork expands the greeting canvas inside an independently scrolling
  terminal. **Live Preview**, its controls and **Preview Checks** stay fixed;
  wheel gestures never spill into the editor or logs at the terminal's edges.
  The optional Layout scrollbar spans both full-size artwork and terminal history.
  Typing reveals the input line without moving the surrounding interface. The Fastfetch
  export captures the currently resolved horizontal/stacked composition; it
  does not resize your terminal or implement runtime-responsive JSONC.
  Optional **Fade in / Line by line / Shimmer** openings can be replayed with
  the play button. Motion is **preview only**, respects system reduced motion,
  and never changes terminal text; Fastfetch exports remain static.
- **Save Preset** (`Ctrl+S` on Greeting) restores the greeting next launch.
  Import/export `.termimochi-greeting.json` presets, or **Export Fastfetch…** to
  `config.jsonc` or a separate `.fastfetch.jsonc` file. Basic, unstyled custom preview
  needs no Fastfetch installation; field overrides, official presets and imports
  need system **Fastfetch 2.57+ (2.x) + Bubblewrap** for native preview.
  To use the exported configuration, install Fastfetch and run
  `fastfetch --config /path/to/config.jsonc`.
  Basic custom preview reads bounded local snapshots; native module output is rendered
  by the system Fastfetch in a **read-only, offline sandbox**, not an attached shell.
  Desktop/terminal detection and disk mount flags may differ in that sandbox;
  detection errors remain visible rather than silently removing fields.
  Designer export includes only reviewed
  built-in modules and literal text, never commands or a startup hook. Imported
  export preserves the original document with only the selected field overrides.
  Existing destinations require an explicit **Back Up & Replace** confirmation;
  private backups are kept in `termimochi-backups` beside the export. Symlinks,
  hard links and concurrent external changes are rejected. Choosing your active
  Fastfetch config changes future Fastfetch runs, but `.bashrc` is never modified.
  Existing workspaces without a Greeting section load with it disabled; older
  eight-field presets preserve their order and add GPU/Disk switched off.
- Optional point-to-edit in Live Preview: enable **Inspect** (off by default)
  for a hover highlight and a named destination before clicking. Colored output selects its ANSI slot,
  ordinary text for Typography, a prompt for Prompt, or the cursor/padding/tab
  title for Layout. Designer prompt parts select their own accent controls.
  Inspection preserves the scene and local input; dragging and double-clicking
  still select text normally. Blank/unrecognized areas do not navigate or
  implicitly select Background. Turn Inspect off, or press Escape in the
  preview, to return to ordinary interaction. Inspection never writes configuration files.
- Ordered Starship prompt builder with a categorized module library for
  context, Git, Rust, Node.js, Python, Go and runtime state. Essential,
  Developer, Remote and Blank are starting points rather than locked
  modes; modules can be added, removed, reordered and recolored. One/two-line
  layout, prompt spacing, ASCII symbols and SSH-only hostnames export to an
  independent `starship.toml` without editing shell startup files.
- A default Current Folder preview with asynchronously collected directory,
  Git and installed-tool context; optional Shell, Codex, Git status/diff,
  test-output, syntax, htop and multilingual alignment samples. VTE renders
  both modes, with local typing and cursor motion that never execute input.
- Read-only import of your existing Starship prompt, preserving its format,
  custom color palette, symbols and multiline structure. Your Starship and
  Designer are separate modes; importing never overwrites the original file.
- **Your Starship** opens your existing configuration directly in a shared editor for
  15 reviewed modules: languages, Git, directory, user/host, prompt symbols,
  Conda and runtime state. Choose the module and, where applicable, its symbol
  or style field; edit text, color, bold, format and visibility. Language modules
  also offer version layouts and sandboxed Rust/Node.js/Python/Go samples.
  Cross-module undo/redo and module-only reset are included. Edits stay in memory
  until saved; other modules and comments are retained. **Save Changes** confirms
  writing back and creates a private backup first. **Save As** saves separately;
  **Restore Previous Version** backs up the current file before restoring.
  External changes block writing back. Shell startup files are never changed.
- Selecting a module or field adds a **simulated command and its complete prompt**:
  enter a language project, stage Git changes, or show a failed command.
  The existing preview is retained above the examples. Edits update that prompt
  in place; recent command lines remain visible. **Original / Edited** compares
  the loaded and edited prompt in the same context, without changing commands,
  paths or example versions. Switching editor pages preserves the transcript,
  scroll position and local input. Preview Source offers **Reset Preview Session**.
  Context uses example values, and omitted modules are included for preview only.
  Displayed commands are never executed and the simulation never enters the export.
- Prompt glyph checks in the diagnostics report: missing characters and
  private-use icons relying on font fallback are distinguished, with direct
  **Fonts** and module-aware **Edit** actions. Ordinary supported CJK/emoji fallback is
  not flagged as an incompatibility.
- Nerd Font symbol shortcuts include the module's glyph beside the label,
  with primary-font, fallback and missing-glyph details on hover.
- Embedded multi-resolution application icon and Linux desktop metadata.
- Atomic open, save and save-as workflows.
- One-click Ptyxis installation with backup, change detection and rollback.
- Kitty, Ghostty, WezTerm and Alacritty exports for the active variant.
- Human-readable and JSON CLI output.

## Requirements

- Rust 1.92 or newer.
- GTK 4.10 or newer.
- libadwaita 1.4 or newer.
- VTE 0.76 or newer, for GTK4.
- GLib resource compiler (`glib-compile-resources`).
- Optional: Starship and Bubblewrap (`bwrap`) for safe existing-prompt previews.
- Optional: Fastfetch to use exported greetings; system Fastfetch and Bubblewrap
  are required for live official-preset preview. Custom-design preview works without them.

Ubuntu/Debian development packages:

```bash
sudo apt install libgtk-4-dev libadwaita-1-dev libvte-2.91-gtk4-dev libgio-2.0-dev-bin
```

## Run

```bash
cargo run -p termimochi
```

Preview Source identifies the imported appearance and any approximations.
Launching from Ptyxis can select its inherited profile; a desktop launch uses
the configured default, not another window's active tab. Appearance import
currently targets Ptyxis; other terminal profiles are not imported. Temporary
zoom and transparency are not copied. Arbitrary shell `PS1` scripts are not imported.

Current Folder uses the app's working directory and can be refreshed. Its
context comes from bounded, read-only probes, not an attached shell session;
previous command status and timing are unavailable. Sample scenarios provide
repeatable success, failure, SSH and alignment checks in Designer.

Your Starship reads `STARSHIP_CONFIG`, or `starship.toml` in the user's XDG
configuration directory (normally `~/.config`). **Reload from Disk** rereads it,
with confirmation before discarding unsaved edits. In Prompt, `Ctrl+S` opens the
save confirmation and `Ctrl+Shift+S` opens Save As. Backups are kept beside the
resolved configuration as `.NAME.termimochi-backup-TIMESTAMP-RANDOM.bak`, with
owner-only permissions; Restore Previous Version also works after restarting.
The installed Starship renderer runs asynchronously with a read-only filesystem,
isolated temporary cache and no network. Only reviewed built-in modules and
declarative options are passed through; custom commands, unreviewed modules,
command overrides and right prompts are skipped and reported. If Starship or
Bubblewrap is unavailable, the app reports the limitation and uses a basic
context prompt; it never runs an unsandboxed fallback. Shell startup files are
not read or executed. Designer still exports a separate complete configuration,
not a merge of the imported file. Nerd Font symbols need a font with those glyphs;
importing preserves symbols but does not silently change your font.

## Install for the current user

The installer builds both release binaries, installs the desktop metadata and
all hicolor icons, and refreshes the desktop and icon caches. It does not need
`sudo` with the default paths:

```bash
./scripts/install.sh install
```

Binaries default to `$HOME/.local/bin`. Desktop data follows `XDG_DATA_HOME`
when it is set and otherwise uses `PREFIX/share`. Paths can contain spaces:

```bash
./scripts/install.sh install \
  --prefix "$HOME/Applications/TermiMochi Local" \
  --data-home "$HOME/Applications/TermiMochi Data"
```

Preview every filesystem change without building or writing anything:

```bash
./scripts/install.sh install --dry-run
```

If an installation using the former `io.github.termimochi.TermiMochi` ID is
found, its exact desktop and icon files are moved to a timestamped backup under
`XDG_STATE_HOME/termimochi/legacy-backups`. Uninstall TermiMochi with the same
path options used during installation:

```bash
./scripts/install.sh uninstall
```

The uninstaller removes only TermiMochi's exact installed targets and keeps
legacy backups. The isolated installation test uses temporary XDG directories:

```bash
./scripts/test-install.sh
```

Open a palette directly:

```bash
cargo run -p termimochi -- themes/fog-paper.palette
```

Use the reference-compatible CLI:

```bash
cargo run -p termimochi-cli -- \
  lint themes/fog-paper.palette --target codex
```

Export either variant without opening the GUI:

```bash
cargo run -p termimochi-cli -- \
  export themes/fog-paper-codex.palette \
  --variant dark --format wezterm --output fog-paper-dark.lua
```

The desktop app's install action writes to Ptyxis' per-user palette directory.
It creates a backup before replacing a file and records a verified one-step
rollback; rollback stops if another program changed the installed file.

## Verify

```bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
python3 -m unittest discover -s scripts -p 'test_desktop_resources.py' -v
python3 scripts/validate_desktop_resources.py
```

The desktop/resource checks need `desktop-file-validate`, `appstreamcli`,
`xmllint`, GLib's resource tools, and Python 3. They check the complete resource
paths and embedded bytes, shared desktop icons, bundled licenses, and palette
copies. Private action icons are embedded only; no fixed resource count is used.
CI also runs the isolated installer, a GUI smoke test, crate packaging and tests
of the unpacked packages, plus a separate RustSec dependency audit.

## Repository structure

```text
crates/termimochi-core  palette model, parser, contrast engine, diagnostics
crates/termimochi-cli   human and JSON command-line interface
crates/termimochi       GTK4/libadwaita desktop application
data/                   application metadata and hicolor icon assets
themes/                 executable example palettes and regression fixtures
```

See [docs/architecture.md](docs/architecture.md) for the design boundaries and
next milestones.

## License

MIT
