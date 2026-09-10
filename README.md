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
- Typography presets: **Save → Save Typography Preset** (or `Ctrl+S` on Typography) remembers
  all five font settings for the next launch. Open and export portable
  `.termimochi-font.json` files; typography edits have their own Undo/Redo.
  The bottom bar's **Apply…** confirms the app-wide font and target-profile spacing,
  saves a private backup, and blocks external-change conflicts. The Typography
  **⋮** menu offers **Restore Previous Typography**. Saving alone never applies.
- Live layout controls for content padding, rows and columns, cursor behavior,
  tab and scrollbar chrome, and preview-window spacing.
- Layout presets: **Save → Save Layout Preset** (`Ctrl+S` on Layout) restores all eight
  settings next launch, with independent Undo/Redo and portable
  `.termimochi-layout.json` import/export. The bottom bar's **Apply…** backs up and
  applies global cursor, scrollbar and new-window grid settings; exact padding,
  tab bar and preview-window spacing remain preview-only. The **⋮** menu offers
  **Restore Previous Layout**. Remembered window sizing is disabled only after
  explicit Apply confirmation, so the chosen grid can take effect.
- Complete setups: the bottom bar's **Save → Save Workspace**
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
  Custom art supports **96 lines × 160 cells, up to 16 KiB**, including CJK,
  emoji and kaomoji, with bounded grapheme-aware clipping
  and a stacked fallback in narrow previews. Text controls and invalid artwork
  are rejected, with independent Undo/Redo and unsaved-change protection.
- The **Import Artwork…** button in Greeting accepts **PNG / JPG / WebP / SVG** images
  and UTF-8/ASCII `.txt` / `.ans` logos. Images open a live conversion draft:
  start with **ANSI Detail** at 64 columns for recognizable color areas and corners,
  or choose **ASCII** / **Half blocks**. Adjust **Max columns**, optionally invert
  ASCII density, and toggle **Original / Converted** at the same display size.
  ASCII starts at **64 columns**, independently supports **160 × 96 cells**,
  and remembers its width when switching styles. **Fit preview** scales only
  the display, not the generated text; turn it off to inspect full-size glyphs.
  The independently scrolling editor offers **Balanced / Illustration / Line Art /
  Photo** recipes, exposure, contrast, saturation, noise smoothing, color modes,
  ASCII structure/edge sensitivity, manual percentage crops and outer-margin trim.
  **Background → Remove background** adds automatic or picked solid-color
  removal, tolerance and edge softness. Edge-connected removal protects enclosed
  details by default; this is not AI removal of complex photo backgrounds.
  **Reset adjustments** keeps the character mode and width. Original shows the
  same crop without tone edits. Recipes are conversion drafts, not saved source
  images: reimport the original to adjust them after closing. See the
  [image conversion guide](docs/image-conversion.md) for usage and algorithm references.
  Click **Use Artwork** to replace only the logo;
  Cancel leaves the greeting unchanged and Undo restores the previous artwork.
  The artwork toolbar keeps **Import Artwork…** readable at narrow widths, with
  export/text editing in the adjacent **⋮** menu. Colored artwork has a fitted
  ANSI thumbnail; click it or the expand icon for a larger **Artwork Preview**.
  Turn **Fit** off there to inspect full-size characters with scrolling. Plain
  custom text remains editable, and thumbnail scaling never changes exports.
  The aspect ratio follows the current terminal cell proportions. ANSI Detail
  and Half blocks approximate source RGB colors unless adjustments are enabled; ASCII strengthens pastel ink contrast
  and thin contours for readability. Transparent cells remain empty and partial alpha blends
  with the current terminal background. Conversion runs in the background.
  Images are limited to 16 MiB, 8192 px per side and 16 megapixels. Detail/Half
  blocks fit within 120 columns, 64 rows and 3072 cells; ASCII has a separate
  15360-cell budget. The actual fitted grid is shown, including width reductions
  for extreme aspect ratios. Wide exports need a sufficiently wide terminal.
  Photo orientation is honored.
  **Edit Artwork…** reopens the embedded original image and saved conversion
  settings, even after moving or deleting the original file. **Use Artwork**
  commits one undoable change; Cancel keeps the previous source and result.
  Presets/workspaces include the original image bytes (including any original
  metadata), but not its filesystem path. Keep this in mind when sharing them.
  Fastfetch/TXT/ANSI exports contain only the rendered artwork, never source
  images or recipes. Existing artwork without a source offers **Reimport
  Original…**; old conversion settings cannot be reconstructed from ANSI alone.
  **Save Preset** remembers it inside TermiMochi for next launch; it does not apply
  anything externally. Its confirmation offers **Review & Apply…**, also available
  as **Apply…** in the fixed bottom bar, to back up and update Fastfetch only after
  explicit confirmation. Run `fastfetch` again to see the result; startup files
  remain unchanged. **Export Fastfetch Configuration…**
  embeds the same colored artwork in `config.jsonc` without changing shell startup.
  ANSI imports retain 16/256/RGB colors and supported text styles; cursor
  movement, screen clearing, clipboard/title commands, links and unsupported
  controls are filtered and reported in **Compatibility**. Tabs expand to
  8-column stops. Legacy CP437 artwork needs conversion to UTF-8 first.
  Colored imports show a read-only plain-text editor; **Edit as Plain Text**
  removes colors for manual editing, with Undo to restore them. **Export / Edit → Export Plain
  Text… / Export ANSI…** exports only the logo, not machine information.
  Overwrites require confirmation, retain a backup and reject external changes.
  Plain text is limited to 16 KiB; ANSI artwork has a separate 320 KiB color-data
  budget with the same visible geometry limits. Static SVG shapes, gradients,
  clipping, masks and text are rasterized locally before character conversion.
  SVG sources are limited to 2 MiB; linked/embedded images, scripts, animation,
  DTDs and external resources are rejected. Text uses system fonts; convert text
  to paths for portable results. SVG rendering requires Bubblewrap and `prlimit`
  and runs with time/memory limits, without an unsandboxed fallback.
  **Bottom bar ⋮ → Export → Export Image Greeting…** separately exports a static
  **Kitty (direct PNG)** or **Sixel** bundle from the editable image source.
  The processed PNG retains crop, background removal and color edits. Choose a
  parent folder; a new folder contains `logo.png`, `config.jsonc` and instructions.
  Run `fastfetch --config config.jsonc` from that folder in a compatible terminal.
  This does not install or activate the bundle. Its checkerboard preview shows
  pixels, not terminal protocol support; Live Preview and normal Apply remain
  ANSI. Sixel additionally includes a generated transparent `logo.sixel` and
  `config-ansi.jsonc` fallback, avoiding ImageMagick's black-background conversion.
  Its fixed pixel size follows the preview font/DPI; re-export after changing the
  target font or DPI. Sixel uses up to 256 colors and binary transparency.
  GIF imports retain their editable animation source while ordinary ANSI uses
  the first nontransparent frame. In **Export Image Greeting…**, select
  **Kitty · Animated GIF**, then use **Play / Pause** or the frame scrubber.
  Playback is opt-in. Export includes the processed GIF, a generated Kitty
  animation stream, PNG still and an ANSI fallback configuration. All frames
  share one crop canvas; full-frame replacement prevents transparent-frame trails.
  GIF is limited to 16 MiB, 1024×1024, 120 frames, 64 MiB of decoded frame pixels
  and 60 seconds per cycle. Importing binary terminal protocol streams and
  animated PNG/WebP remain unsupported.
  See [image export requirements](docs/image-conversion.md#pixel-image-bundles).
- Click a system field's name to edit its **label, inline icon, name/content
  colors and display format** in a compact popover. CPU/GPU summaries, memory
  and disk percentages/bars, and date/time formats share the same native
  Fastfetch configuration in preview and export. **Reset Field** restores the
  original; edits support Undo/Redo and portable preset/workspace saves.
  **Compatibility** expands only when needed: it reports unavailable native
  rendering, offline detection differences, missing icon glyphs and imported
  settings that are retained but not simulated.
- **Bottom bar ⋮ → Sources → Load Current Configuration** reads the standard user
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
  the applied file and can run when Fastfetch is invoked; review them first.
  **Run Fastfetch in a new terminal after applying** opens a new **Ptyxis** window
  and executes the applied config once. It is on by default for designer configs
  and off for imported configs, which may contain commands or network modules.
  Press Enter in the result window to close it. The run is outside the offline
  preview sandbox and uses Ptyxis's configured appearance, not an unsaved app
  theme. Missing launch tools do not undo a successful config apply. Cancel,
  failed/conflicting apply, Save Preset and Restore do not launch a terminal.
  A successful apply also saves the current Greeting preset for the next launch.
  If that local save fails, a persistent notice reports the partial success;
  conflicting presets are never overwritten. Startup and window refocus compare
  the preview with the last applied file (or the default Fastfetch config).
  **Load Applied** loads and saves that version locally, with confirmation before
  replacing unsaved edits. These checks do not execute imported commands or
  silently replace drafts. **Save Preset** alone still changes only TermiMochi.
  None of these actions edits `.bashrc` or enables automatic shell startup.
- **Bottom bar ⋮ → Terminal Startup…** is a separate, default-off
  opt-in for new local Bash terminals. It shows the exact managed block and
  `.bashrc` destination before enabling, keeps a backup, and rejects aliases,
  read-only files, concurrent changes and pre-existing Fastfetch/Neofetch
  references. Disabling removes only the unchanged TermiMochi block and preserves
  other edits. It skips SSH, tmux, nested shells and non-interactive output, and
  runs at most once per shell. The selected applied config may execute commands
  or network modules automatically; the confirmation explicitly warns about this.
  Other shells are not configured. Portable presets cannot enable this setting.
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
- **Save → Save Greeting Preset** (`Ctrl+S` on Greeting) restores the greeting next launch.
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
  Fastfetch config changes future Fastfetch runs; Review & Apply can also run it
  once immediately. `.bashrc` is never modified.
  Existing workspaces without a Greeting section load with it disabled; older
  eight-field presets preserve their order and add GPU/Disk switched off.
- Optional point-to-edit in Live Preview: enable **Inspect** (off by default)
  for a hover highlight and a named destination before clicking. Colored output selects its ANSI slot,
  ordinary text for Typography, a prompt for Prompt, or the cursor/padding/tab
  title for Layout. Designer prompt parts select their own accent controls.
  Greeting artwork, welcome messages and basic fields reveal their corresponding
  controls. Official/native fields reveal the field list; imported artwork is
  identified only when its retained source matches one complete visible rectangle.
  Ambiguous native content stays at the field-list level instead of guessing.
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

## Editor organization

The activity rail switches properties without replacing the terminal preview.
The fixed bottom bar keeps **Save**, the current module's **Apply… / Install… /
Export…**, and **⋮** accessible while properties scroll. **Save** stores presets
or a complete workspace; external changes still require the existing review and
backup flow. Secondary imports, exports, reloads and restores live under **⋮**.
Prompt's Save menu and `Ctrl+S` both save a workspace; `Ctrl+Shift+S` saves a
workspace under a new name. `Ctrl+O` opens a workspace, not a palette. Only the
explicit **Apply…** / **Export…** actions write Starship files.

Palette's target selector can locate **Prompt Designer** segment colors and
**Greeting** accent/text and individual field name/content colors in the shared
terminal palette. Repeated native fields retain distinct source indices. Editing
a shared slot changes every element referencing it; **Edit source…** opens the
module that owns its binding. **Your Starship** lists reviewed module style
fields and jumps directly to their existing color editor. Literal RGB, complex
SGR styles and ambiguous inherited native colors remain source-owned; they are
never replaced with a guessed palette slot. Source navigation does not edit data.
Artwork adjustments remain in Edit Artwork.

## Requirements

- Rust 1.92 or newer.
- GTK 4.10 or newer.
- libadwaita 1.4 or newer.
- VTE 0.76 or newer, for GTK4.
- GLib resource compiler (`glib-compile-resources`).
- Optional: Starship and Bubblewrap (`bwrap`) for safe existing-prompt previews.
- Optional: Fastfetch to use exported greetings; system Fastfetch and Bubblewrap
  are required for live official-preset preview. Custom-design preview works without them.
- Optional: Bubblewrap and `prlimit` (util-linux) for sandboxed static SVG import.

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
with confirmation before discarding unsaved edits. In Prompt, **Apply…** opens
the backed-up save confirmation; **⋮ → Save Starship As…** exports separately.
`Ctrl+S` saves a local workspace instead. Backups are kept beside the
resolved configuration as `.NAME.termimochi-backup-TIMESTAMP-RANDOM.bak`, with
owner-only permissions; Restore Previous Version also works after restarting.
The installed Starship renderer runs asynchronously with a read-only filesystem,
isolated temporary cache and no network. Only reviewed built-in modules and
declarative options are passed through; custom commands, unreviewed modules,
command overrides and right prompts are skipped and reported. If Starship or
Bubblewrap is unavailable, the app reports the limitation and uses a basic
context prompt; it never runs an unsandboxed fallback. The prompt preview does
not read or execute shell startup files. Designer still exports a separate complete configuration,
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

For the full opt-in native/GUI regression matrix, start a **dedicated Xvfb**
display, then run:

```bash
Xvfb :91 -screen 0 2880x1900x24 -nolisten tcp -noreset
# In another terminal:
python3 scripts/test-regression.py --display :91 --scale 1 --scale 2
```

This requires Fastfetch, Bubblewrap, `prlimit`, Ptyxis, `dbus-run-session`, `xwininfo`,
and the X11 libraries used by the pointer driver. Each case uses a separate
process, private D-Bus session and temporary XDG directories; the terminal-launch
test opens Ptyxis on the test display only. The runner pins the test and SVG-worker builds,
enforces per-case timeouts and writes logs plus `results.json` under `/tmp`.
Use `--filter extreme_settings` for the cross-module extreme-value/rapid-switch
test. Ordinary tests additionally cover a 672-case artwork/layout matrix and
1,024 seeded terminal-control streams. These checks supplement, not replace,
manual testing on real desktop compositors.

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
