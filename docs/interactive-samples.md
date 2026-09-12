# Interactive samples

Implementation scope: `TermiMochi_Interactive_Samples_Codex_Task.md`.
The theme/workspace is still the only mutable design model. Sample sessions are
window-local runtime state and are never serialized, applied or validated as native.

## Preservation map (before implementation)

| Existing function | Owner / new entry | Regression |
| --- | --- | --- |
| Full composition, VTE, image overlay | Full, default interactive samples | Independent tabs, clear, typography, image anchors |
| Terminal/Prompt/Greeting scenes | Scene selector, unchanged | Scene and editor independence |
| Scenario catalog, current-folder snapshot | Sample menu; explicit snapshot stays in Terminal | Shared scenario fixtures; no command triggers snapshot |
| Palette, Typography, Layout, Prompt, Greeting | Existing theme editors | No session state in save/undo/apply |
| Pictures/GIF and worker cache | Existing Greeting projection | No decoding on tab switch; clear removes anchors |
| PNG/JPG/WebP/SVG processing, background removal, editable source recipes | Existing Greeting artwork editor and bounded/sandboxed workers, unchanged | Non-ignored processing tests; all format-specific ignored native tests not rerun |
| ANSI/pixel/GIF export, managed image installation and startup integration | Existing advanced output/target adapters, unchanged | Existing non-ignored export/safety tests; sample animation never marks protocol verification |
| Font family/Nerd Font, layout presets, sparse overrides | Existing Typography/Layout pages and preset actions | Workspace unit tests plus selected top/Save/typography GUI coverage |
| Native Starship/Fastfetch source import and advanced single-item documents | Existing document and source-preserving import/export paths | Existing non-ignored document/source tests; no new mutable design copy |
| Inspect | Existing design preview map | Input and tab controls not interpreted as palette clicks |
| Kitty/Ptyxis real preview | Advanced experimental native mode | Existing enabled-feature safety and lifecycle tests |
| Try / Use Theme / recovery / source preservation | Existing target adapters, unchanged | No sample can grant verification or write configuration |
| Theme Save/Ctrl+S, Undo/Redo and reopen | Existing theme store/history; focused sample text uses GTK text undo | Actual keyboard Save/Undo test and selected save/reopen tests; runtime state is not serialized |

The native truncation, IME/GPU/environment and protocol gaps in
[native-preview-qa.md](native-preview-qa.md) remain open; changing the default UI
does not resolve them.

## Architecture and limits

`interactive_samples.rs` parses text into a finite `Action` and reduces it into
window-local `Session` / `Tab` records. It imports the existing scenario catalog,
not process/filesystem/network services. No action contains executable arguments.
Each tab has a stable ID, draft, virtual directory, 100-item command history,
stable output-block IDs and a scroll anchor. Limits are eight tabs, 48 output
records per tab, and approximately 900 displayed character rows. Oversized/old
records outside the display budget are explicitly omitted, not left as orphan images.

`window/interactive_samples.rs` mounts GTK Entry/Text input and accessible tab
buttons over the existing target top renderer. GTK handles Unicode editing and
IME; both Entry and its Text delegate reject control-character insertion. Paste
does not submit. Per-input GTK undo resets on tab remount; drafts and command
history remain independent. Theme undo/Save/apply do not own this runtime state.

Full reuses its existing VTE feed/map and cell-positioned overlay. There is no PTY
in the sample renderer. A projection refresh never invokes the reducer; unchanged
text is not fed again on palette refresh. Greeting is projected once per redraw,
and repeated `fastfetch` records reuse the prepared image/animation texture.
Clear removes all anchors in that tab. Default **Actual size** uses the theme font
size with a viewport-derived observation grid. **100% · fixed grid** uses the theme's
initial columns, with local scrolling where necessary. **Fit window** transforms
the complete finite window (title, tabs, viewport, image and inline GTK input), not
an arbitrarily tall transcript. Columns remain character occupancy, not zoom.
The GTK input is positioned at VTE's final Prompt, inside the content scroller;
VTE's artificial input cursor is hidden. See [geometry/input QA](preview-geometry-input-qa.md).

Designer prompts use the current PromptSettings with the virtual directory.
Imported Starship uses three immutable declarative directory projections in the
existing bounded, offline renderer. The original source is preserved; generated
scenes do not contain imported custom commands. Cache results are source/width
keyed and pass the existing generation checks. Unsupported formats or unavailable
sandbox rendering fall back to a plain prompt, not execution without isolation.

## Finite command table

| Input | Internal behavior |
| --- | --- |
| `help` | Actual command list and non-execution notice |
| `pwd` | Virtual `/`, `/demo` or `/demo/src` |
| `ls`, `ls -l` | Fixed fixture entries; metadata marked synthetic |
| `cd /`, `cd demo`, `cd src`, `cd ..` | Navigate only the tiny virtual tree |
| `git status`, `git diff` | Existing ANSI fixtures, with the current virtual prompt; `/` is not a fixture repo |
| `cargo test` | Instant canned test transcript, explicitly not a project test run |
| `fastfetch` | Append the current Greeting design projection; no Fastfetch invocation from input |
| `clear`, Ctrl+L | Remove current tab's visible records and picture anchors, keep directory/history |
| Anything else / shell syntax | Non-modal rejection; never delegated to the OS |

The existing Codex, code, htop and typography specimens remain in the sample
selector. Current Folder remains an explicit, separate Terminal snapshot and is
not a command alias. Native trial, image conversion and existing sandboxed prompt
renderers are distinct from command execution and retain their own boundaries.

## Target presentation

| Setting | Kitty sample | Ptyxis sample |
| --- | --- | --- |
| Colors/font/cell metrics | Current theme + VTE metrics | Current theme + VTE metrics |
| Tabs | Dynamic titles, count/threshold, edge, four style projections and active/inactive colors | Dynamic tabs, shared titlebar palette and Adwaita-like selection shading |
| Title bar | Existing capability-gated system/background/custom projection | Existing TitlebarBackground/TitlebarForeground projection |
| Greeting image/GIF | GTK design projection, **not** a protocol result | Character fallback; separate artwork editing is retained |
| Native interactive terminal | Advanced experimental feature, explicit start/stop authorization | Same, with private settings/service isolation |

GTK/VTE, Kitty's own font rendering and system decorations can differ. Native
comparison evidence records the theme and metrics; differing native viewport grids
are disclosed rather than scored by image similarity. Simulated samples never
grant Kitty/Sixel/animation verification or enable a real application transaction.

## Running

Default: `cargo run -p termimochi --locked` from the repository.
Advanced native build: `bash scripts/run-native-preview.sh` with the existing local
native dependency prefix. Nothing in either launcher installs or applies a theme.

See the [Chinese acceptance card](interactive-samples-acceptance-zh.md) and
[QA ledger](interactive-samples-qa.md) for executed checks and remaining gaps.
