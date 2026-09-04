# TermiMochi

> Make terminal themes cute **and** usable.

TermiMochi is a native Rust + GTK4/libadwaita workbench for terminal palettes.
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
- Read-only startup import from the current or default Ptyxis profile.
- Light/dark variant editor with an embedded HSV picker and exact HEX/RGB input.
- Document-wide undo/redo with coalesced drag and text-edit transactions.
- VTE-rendered Shell, Codex, Git status/diff, test-output, syntax, htop and
  font-coverage previews.
- Embedded multi-resolution application icon and Linux desktop metadata.
- Atomic open, save and save-as workflows.
- One-click Ptyxis installation with backup, change detection and rollback.
- Kitty, Ghostty, WezTerm and Alacritty exports for the active variant.
- Human-readable and JSON CLI output.

## Requirements

- Rust 1.92 or newer.
- GTK 4.10 or newer.
- libadwaita 1.4 or newer.
- VTE for GTK4.
- GLib resource compiler (`glib-compile-resources`).

Ubuntu/Debian development packages:

```bash
sudo apt install libgtk-4-dev libadwaita-1-dev libvte-2.91-gtk4-dev libgio-2.0-dev-bin
```

## Run

```bash
cargo run -p termimochi
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
