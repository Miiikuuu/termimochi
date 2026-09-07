# TermiMochi

> Make terminal themes cute **and** usable.

TermiMochi is a native Rust + GTK4/libadwaita workbench for terminal palettes
and Starship prompts.
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
  Typography (`Ctrl+2`), Layout (`Ctrl+3`) and Prompt (`Ctrl+4`) modules while
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
```

## Workspace

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
