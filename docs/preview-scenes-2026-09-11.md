# Independent preview scenes — 2026-09-11

## Delivered

- The fixed action is **Try Greeting**, with a tooltip explicitly excluding a
  whole font/color/prompt scheme trial. Its existing safe trial action is unchanged.
- The right header independently selects **Terminal / Prompt / Greeting**.
  Left module switches and Inspect navigation do not replace the observed scene.
- Greeting images/GIFs stay visible while editing Palette, Typography, Layout or
  Prompt. Playback continues; theme background updates without decoding the image
  again. Ordinary hidden-scene edits do not select that scene automatically.
- View selection is session-only, not a saved workspace setting. Explicit workspace
  loading and preview-reset actions retain their existing initialization behavior.
- Import feedback and the [acceptance card](acceptance-card-zh.md) explain how to
  select Greeting for observation. Existing native tests now explicitly select
  their intended scene instead of relying on editor navigation as a side effect.

No daily terminal configuration was changed. No install, commit, push, merge or
release publication was performed. The user-owned implementation brief is untouched.

## Executed checks

- `cargo fmt --all -- --check` and `git diff --check`: passed.
- `cargo clippy --workspace --all-targets --locked -- -D warnings`: passed.
- `cargo test --workspace --locked`: **351 passed**, **57 opt-in native tests ignored**.
  Ignored tests are not counted as passing in this result.
- Native Xvfb checks with independent HOME/XDG/D-Bus and memory GSettings:
  **29 passing cases across the reports below**. Core cases run at both 1× and
  2×; the extended compatibility regression runs at 1×.
  - **16/16 core:** independent scenes through all five editor modules; GIF
    playback and retained decoded source across navigation; exact dark/light
    background pixel sampling; save/reopen/undo; Greeting editing; actual VTE
    Inspect pointer navigation; independent scrolling and shared color controls.
    Scene selector/action bounds also checked at 1024×700 and 1280×900.
  - **3/3 extended:** workspace restore/safety, artwork import, field editor.
    This runner was subsequently interrupted; only its three completed results
    are counted. The remaining cases were run in a new runner.
  - **10/10 extended:** image import, bounded edits, output-check cache safety,
    synchronization, applied artwork refresh, imported fields, official presets,
    brand starter, output actions and preview-fit hint.
- Release build passed. Isolated release GUI stayed alive for the 10-second smoke
  window (expected timeout status 124), without panic or GTK criticals. Xvfb portal
  and software-rendering warnings are not interpreted as terminal compatibility.
- Desktop resource test suite: **9/9**. Metadata, **17 embedded resources** and
  packaged-copy validation passed. Minimum/common-window screenshots inspected.

Local evidence (ignored build artifacts, not uploaded):

- `target/qa/preview-scenes-workspace-tests.log`
- `target/qa/termimochi-regression-i21mrzs8/results.json` — 16 core cases
- `target/qa/termimochi-regression-wpbeq8z1/results.json` — 3 completed extended cases
- `target/qa/termimochi-regression-pa4v7gbz/results.json` — 10 extended cases
- `target/qa/preview-scenes-release-smoke.log`

The earlier intermediate reports include a test sampling caption text instead of
empty background and an old scroll fixture relying on automatic Greeting scene
selection. Both test setups were corrected and the affected cases rerun above.
Build fingerprints for each native runner are in its `build.json`.

## Run and accept

```bash
cd /home/xiaozhouchao/Projects/personal/TermiMochi
bash scripts/run-isolated.sh
```

Rebuild if needed: `cargo build --workspace --release --locked`.
GUI: `target/release/termimochi`.
SHA-256: `30526f79572d80eaf1e9d751959ceacd286fef1a04b433804cc5ca3be22873f5`.

Import a test GIF in Greeting, choose **Greeting** in the right header and start
**Play animation**. Switch the left side to Palette and change Background: the
same animated artwork stays visible. Switch right to Terminal or Prompt, then
switch left modules again: the right scene remains selected. **Try Greeting**
continues to mean greeting-only trial; Save does not apply terminal settings.

## Still unverified

Real-user discoverability/usability; Wayland fractional scaling; other GTK/terminal
versions; remote/multiplexer behavior. This change did not rerun Kitty/Sixel
protocol trials or the known isolated Ptyxis user-manager limitation. GTK GIF
playback is not evidence of animation support in an external terminal. These
local checks also do not represent a new online CI run: this version is unpushed.

## Pre-merge review follow-up

After the initial push, PR #1 identified repeated Kitty configuration roots
consuming the shared verification budget more than once. Roots are now
canonicalized and deduplicated before traversal. Two new tests cover repeated
defaults, symlink aliases, missing roots, content changes, stable ordering and
the unchanged shared 4 MiB limit. No real configuration was modified.

Follow-up local results: formatting, strict Clippy, **353 workspace tests**
(57 opt-in cases ignored), Release build and **3/3 isolated native checks**
(trial review/recovery, advisory cache safety, resolved destination status).
Evidence: `target/qa/configuration-roots-workspace-tests.log` and
`target/qa/termimochi-regression-8he_kwvm/results.json`.
Updated GUI SHA-256:
`a1b1e5f95c6d94cc80002741dd3fd33c5a00687a421e6fb2025740faeabf31b9`.
Online checks on this follow-up must pass separately before merge.
